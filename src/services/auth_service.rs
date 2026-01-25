//! 认证服务

use crate::errors::AppError;
use crate::security::{Claims, JwtManager, TokenPair};
use crate::services::{CacheService, DeviceService};
use std::sync::Arc;

/// 认证服务
pub struct AuthService {
    jwt_manager: Arc<JwtManager>,
    device_service: Arc<DeviceService>,
    cache_service: Arc<CacheService>,
}

impl AuthService {
    pub fn new(
        jwt_manager: Arc<JwtManager>,
        device_service: Arc<DeviceService>,
        cache_service: Arc<CacheService>,
    ) -> Self {
        Self {
            jwt_manager,
            device_service,
            cache_service,
        }
    }

    /// 使用 API Key 换取 JWT Token
    pub async fn authenticate_device(&self, api_key: &str) -> Result<TokenPair, AppError> {
        // 验证 API Key
        let device = self.device_service.verify_by_api_key(api_key).await?;

        // 生成 Token 对
        let access_token = self.jwt_manager.generate_access_token(
            &device.id.to_string(),
            Some(device.id),
            Some("device".to_string()),
        )?;

        let refresh_token = self
            .jwt_manager
            .generate_refresh_token(&device.id.to_string(), Some(device.id))?;

        // 从 JWT 管理器获取过期时间
        let expires_in = self.jwt_manager.access_expiry_seconds();

        Ok(TokenPair::new(access_token, refresh_token, expires_in))
    }

    /// 刷新 Token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenPair, AppError> {
        // 验证 Refresh Token
        let claims = self.jwt_manager.validate_refresh_token(refresh_token)?;

        // 检查是否在黑名单中
        if self.cache_service.is_token_blacklisted(&claims.jti).await? {
            return Err(AppError::Unauthorized("令牌已被吊销".to_string()));
        }

        // 生成新的 Token 对
        let access_token = self.jwt_manager.generate_access_token(
            &claims.sub,
            claims.device_id,
            claims.role.clone(),
        )?;

        let new_refresh_token = self
            .jwt_manager
            .generate_refresh_token(&claims.sub, claims.device_id)?;

        // 将旧的 Refresh Token 加入黑名单
        let remaining_expiry = (claims.exp - chrono::Utc::now().timestamp()) as u64;
        if remaining_expiry > 0 {
            self.cache_service
                .blacklist_token(&claims.jti, remaining_expiry)
                .await?;
        }

        // 从 JWT 管理器获取过期时间
        let expires_in = self.jwt_manager.access_expiry_seconds();

        Ok(TokenPair::new(access_token, new_refresh_token, expires_in))
    }

    /// 吊销 Token
    pub async fn revoke_token(&self, token: &str) -> Result<(), AppError> {
        // 解析 Token（不验证过期，因为可能已经过期但仍需吊销）
        let claims = self.jwt_manager.validate_token(token)?;

        // 计算剩余过期时间
        let now = chrono::Utc::now().timestamp();
        let remaining_expiry = if claims.exp > now {
            (claims.exp - now) as u64
        } else {
            // Token 已过期，仍然加入黑名单（防止重放）
            3600 // 保留 1 小时
        };

        // 加入黑名单
        self.cache_service
            .blacklist_token(&claims.jti, remaining_expiry)
            .await?;

        tracing::info!(
            jti = %claims.jti,
            subject = %claims.sub,
            "Token 已吊销"
        );

        Ok(())
    }

    /// 验证 Access Token
    pub async fn validate_access_token(&self, token: &str) -> Result<Claims, AppError> {
        let claims = self.jwt_manager.validate_access_token(token)?;

        // 检查是否在黑名单中
        if self.cache_service.is_token_blacklisted(&claims.jti).await? {
            return Err(AppError::Unauthorized("令牌已被吊销".to_string()));
        }

        Ok(claims)
    }
}
