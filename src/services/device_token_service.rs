//! 设备访问令牌服务

use crate::db::RedisPool;
use crate::errors::AppError;
use crate::models::{
    AccessTokenInfo, CreateAccessTokenRequest, CreateAccessTokenResponse, DeviceAccessToken,
};
use crate::repositories::{CreateTokenParams, DeviceAccessTokenRepository, DeviceRepository};
use crate::security::{generate_token, verify_token, TokenType};
use chrono::{Duration, Utc};
use std::sync::Arc;
use uuid::Uuid;

/// 最大令牌数量（每设备）
const MAX_TOKENS_PER_DEVICE: i64 = 20;

/// 设备访问令牌服务
pub struct DeviceAccessTokenService {
    token_repo: DeviceAccessTokenRepository,
    device_repo: Arc<DeviceRepository>,
    #[allow(dead_code)]
    redis_pool: Arc<RedisPool>,
}

impl DeviceAccessTokenService {
    pub fn new(
        token_repo: DeviceAccessTokenRepository,
        device_repo: Arc<DeviceRepository>,
        redis_pool: Arc<RedisPool>,
    ) -> Self {
        Self {
            token_repo,
            device_repo,
            redis_pool,
        }
    }

    /// 创建访问令牌
    pub async fn create_token(
        &self,
        device_id: Uuid,
        user_id: Uuid,
        request: CreateAccessTokenRequest,
    ) -> Result<CreateAccessTokenResponse, AppError> {
        // 验证设备存在且属于该用户
        let device = self
            .device_repo
            .find_by_id(device_id)
            .await?
            .ok_or_else(|| AppError::NotFound("设备不存在".to_string()))?;

        if device.owner_id != Some(user_id) {
            return Err(AppError::Forbidden("您无权为此设备创建令牌".to_string()));
        }

        // 检查令牌数量限制
        let token_count = self.token_repo.count_valid_tokens(device_id).await?;
        if token_count >= MAX_TOKENS_PER_DEVICE {
            return Err(AppError::ValidationError(format!(
                "每个设备最多只能有 {} 个有效令牌",
                MAX_TOKENS_PER_DEVICE
            )));
        }

        // 生成令牌
        let (token, token_hash, token_prefix) = self.generate_access_token()?;

        // 计算过期时间
        let expires_at = request
            .expires_in_hours
            .map(|hours| Utc::now() + Duration::hours(hours));

        // 创建令牌记录
        let params = CreateTokenParams {
            device_id,
            created_by: user_id,
            token_hash,
            token_prefix,
            name: request.name.clone(),
            permission: request.permission.clone(),
            expires_at,
            allowed_ips: request.allowed_ips,
            rate_limit_per_minute: request.rate_limit_per_minute,
        };
        let saved_token = self.token_repo.create(params).await?;

        Ok(CreateAccessTokenResponse {
            id: saved_token.id,
            device_id: saved_token.device_id,
            name: saved_token.name,
            token, // 仅此一次返回完整令牌
            token_prefix: saved_token.token_prefix,
            permission: saved_token.permission,
            expires_at: saved_token.expires_at,
            created_at: saved_token.created_at,
        })
    }

    /// 生成令牌（使用统一的 token 模块）
    fn generate_access_token(&self) -> Result<(String, String, String), AppError> {
        let result = generate_token(TokenType::DeviceAccessToken)?;
        Ok((result.token, result.hash, result.display_prefix))
    }

    /// 验证令牌并返回设备信息
    pub async fn validate_token(
        &self,
        token: &str,
        client_ip: Option<&str>,
    ) -> Result<(DeviceAccessToken, Uuid), AppError> {
        // 检查令牌格式
        let token_type = TokenType::from_token(token)
            .ok_or_else(|| AppError::Unauthorized("无效的令牌格式".to_string()))?;

        if token_type != TokenType::DeviceAccessToken {
            return Err(AppError::Unauthorized("令牌类型不正确".to_string()));
        }

        // 提取搜索前缀
        let search_prefix = crate::security::extract_search_prefix(token)?;

        // 查找令牌
        let db_token = self
            .token_repo
            .find_valid_by_prefix(&search_prefix)
            .await?
            .ok_or_else(|| AppError::Unauthorized("令牌无效或已过期".to_string()))?;

        // 验证令牌哈希
        if !verify_token(token, &db_token.token_hash)? {
            return Err(AppError::Unauthorized("令牌验证失败".to_string()));
        }

        // 检查 IP 白名单
        if let Some(ip) = client_ip {
            if !db_token.is_ip_allowed(ip) {
                return Err(AppError::Forbidden("IP 地址不在白名单中".to_string()));
            }
        }

        // 更新使用记录（异步，不阻塞请求）
        let token_id = db_token.id;
        let repo = self.token_repo.clone();
        tokio::spawn(async move {
            let _ = repo.record_usage(token_id).await;
        });

        let device_id = db_token.device_id;
        Ok((db_token, device_id))
    }

    /// 列出设备的所有令牌
    pub async fn list_tokens(
        &self,
        device_id: Uuid,
        user_id: Uuid,
        include_revoked: bool,
        include_expired: bool,
    ) -> Result<Vec<AccessTokenInfo>, AppError> {
        // 验证权限
        let device = self
            .device_repo
            .find_by_id(device_id)
            .await?
            .ok_or_else(|| AppError::NotFound("设备不存在".to_string()))?;

        if device.owner_id != Some(user_id) {
            return Err(AppError::Forbidden("无权查看此设备的令牌".to_string()));
        }

        let tokens = self
            .token_repo
            .list_by_device(device_id, include_revoked, include_expired)
            .await?;

        Ok(tokens.into_iter().map(AccessTokenInfo::from).collect())
    }

    /// 吊销令牌
    pub async fn revoke_token(&self, token_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        // 验证权限
        if !self.token_repo.user_owns_token(token_id, user_id).await? {
            return Err(AppError::Forbidden("无权吊销此令牌".to_string()));
        }

        self.token_repo.revoke(token_id).await?;
        Ok(())
    }

    /// 吊销设备的所有令牌
    pub async fn revoke_all_tokens(&self, device_id: Uuid, user_id: Uuid) -> Result<u64, AppError> {
        // 验证权限
        let device = self
            .device_repo
            .find_by_id(device_id)
            .await?
            .ok_or_else(|| AppError::NotFound("设备不存在".to_string()))?;

        if device.owner_id != Some(user_id) {
            return Err(AppError::Forbidden("无权操作此设备".to_string()));
        }

        let count = self.token_repo.revoke_all_for_device(device_id).await?;
        Ok(count)
    }
}
