//! 用户业务服务

use crate::db::RedisPool;
use crate::errors::AppError;
use crate::models::{
    ChangePasswordRequest, LoginRequest, LoginResponse, RegisterRequest,
    UpdateUserRequest, User, UserInfo, UserListQuery, UserRole,
    DeviceShare, SharePermission, DeviceShareInfo,
};
use crate::repositories::UserRepository;
use crate::security::{hash_password, verify_password, check_password_strength, JwtManager};
use crate::models::PaginatedResponse;
use crate::models::Pagination;
use std::sync::Arc;
use uuid::Uuid;
use sha2::{Sha256, Digest};

/// 用户业务服务
pub struct UserService {
    user_repo: UserRepository,
    jwt_manager: Arc<JwtManager>,
    /// 预留用于会话缓存和令牌黑名单
    #[allow(dead_code)]
    redis_pool: Arc<RedisPool>,
}

impl UserService {
    pub fn new(
        user_repo: UserRepository,
        jwt_manager: Arc<JwtManager>,
        redis_pool: Arc<RedisPool>,
    ) -> Self {
        Self {
            user_repo,
            jwt_manager,
            redis_pool,
        }
    }

    /// 用户注册
    pub async fn register(&self, request: RegisterRequest) -> Result<UserInfo, AppError> {
        // 检查密码强度
        check_password_strength(&request.password)?;

        // 如果提供了 confirm_password，则校验一致性
        if let Some(ref cp) = request.confirm_password {
            if cp != &request.password {
                return Err(AppError::ValidationError("密码与确认密码不一致".to_string()));
            }
        }

        // 检查邮箱是否已存在
        if self.user_repo.email_exists(&request.email).await? {
            return Err(AppError::ValidationError("邮箱已被注册".to_string()));
        }

        // 检查用户名是否已存在
        if self.user_repo.username_exists(&request.username).await? {
            return Err(AppError::ValidationError("用户名已被占用".to_string()));
        }

        // 哈希密码
        let password_hash = hash_password(&request.password)?;

        // 创建用户
        let user = self
            .user_repo
            .create(&request.email, &request.username, &password_hash)
            .await?;

        tracing::info!(
            user_id = %user.id,
            email = %user.email,
            "新用户注册成功"
        );

        Ok(user.into())
    }

    /// 用户登录
    pub async fn login(
        &self,
        request: LoginRequest,
        ip_address: Option<&str>,
    ) -> Result<LoginResponse, AppError> {
        // 查找用户
        let user = self
            .user_repo
            .find_by_login(&request.login)
            .await?
            .ok_or_else(|| AppError::Unauthorized("用户名或密码错误".to_string()))?;

        // 检查用户是否激活
        if !user.is_active {
            return Err(AppError::Unauthorized("账户已被禁用".to_string()));
        }

        // 检查是否被锁定
        if self.user_repo.is_locked(user.id).await? {
            return Err(AppError::Unauthorized("账户已被锁定，请 15 分钟后重试".to_string()));
        }

        // 验证密码
        if !verify_password(&request.password, &user.password_hash)? {
            let attempts = self.user_repo.record_failed_login(user.id).await?;
            
            if attempts >= 5 {
                return Err(AppError::Unauthorized("登录失败次数过多，账户已被锁定 15 分钟".to_string()));
            }
            
            return Err(AppError::Unauthorized("用户名或密码错误".to_string()));
        }

        // 更新最后登录时间
        self.user_repo.update_last_login(user.id).await?;

        // 生成令牌
        let access_token = self.jwt_manager.generate_access_token(
            &user.id.to_string(),
            None, // 用户登录不关联设备
            Some(user.role.to_string()),
        )?;

        let refresh_token = self.jwt_manager.generate_refresh_token(
            &user.id.to_string(),
            None,
        )?;

        // 保存刷新令牌
        let token_hash = self.hash_token(&refresh_token);
        self.user_repo
            .save_refresh_token(
                user.id,
                &token_hash,
                request.device_info.as_deref(),
                ip_address,
                7, // 7 天有效期
            )
            .await?;

        tracing::info!(
            user_id = %user.id,
            email = %user.email,
            "用户登录成功"
        );

        Ok(LoginResponse {
            user: user.into(),
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: 900, // 15 分钟
        })
    }

    /// 刷新令牌
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
        ip_address: Option<&str>,
    ) -> Result<LoginResponse, AppError> {
        // 验证刷新令牌
        let claims = self.jwt_manager.validate_refresh_token(refresh_token)?;

        // 检查令牌是否在数据库中
        let token_hash = self.hash_token(refresh_token);
        let stored_token = self
            .user_repo
            .find_refresh_token_by_hash(&token_hash)
            .await?
            .ok_or_else(|| AppError::Unauthorized("无效的刷新令牌".to_string()))?;

        // 获取用户
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AppError::Unauthorized("无效的令牌".to_string()))?;
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::Unauthorized("用户不存在".to_string()))?;

        if !user.is_active {
            return Err(AppError::Unauthorized("账户已被禁用".to_string()));
        }

        // 删除旧的刷新令牌
        self.user_repo.delete_refresh_token(&token_hash).await?;

        // 生成新令牌
        let new_access_token = self.jwt_manager.generate_access_token(
            &user.id.to_string(),
            None,
            Some(user.role.to_string()),
        )?;

        let new_refresh_token = self.jwt_manager.generate_refresh_token(
            &user.id.to_string(),
            None,
        )?;

        // 保存新的刷新令牌
        let new_token_hash = self.hash_token(&new_refresh_token);
        self.user_repo
            .save_refresh_token(
                user.id,
                &new_token_hash,
                stored_token.device_info.as_deref(),
                ip_address,
                7,
            )
            .await?;

        Ok(LoginResponse {
            user: user.into(),
            access_token: new_access_token,
            refresh_token: new_refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: 900,
        })
    }

    /// 用户登出
    pub async fn logout(&self, refresh_token: &str) -> Result<(), AppError> {
        let token_hash = self.hash_token(refresh_token);
        self.user_repo.delete_refresh_token(&token_hash).await?;
        Ok(())
    }

    /// 登出所有设备
    pub async fn logout_all(&self, user_id: Uuid) -> Result<u64, AppError> {
        let count = self.user_repo.delete_all_refresh_tokens(user_id).await?;
        
        tracing::info!(
            user_id = %user_id,
            sessions = count,
            "用户已登出所有设备"
        );
        
        Ok(count)
    }

    /// 获取当前用户信息
    pub async fn get_current_user(&self, user_id: Uuid) -> Result<UserInfo, AppError> {
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("用户不存在".to_string()))?;

        Ok(user.into())
    }

    /// 更新用户信息
    pub async fn update_user(
        &self,
        user_id: Uuid,
        request: UpdateUserRequest,
    ) -> Result<UserInfo, AppError> {
        // 如果要更新用户名，检查是否重复
        if let Some(ref username) = request.username {
            let existing = self.user_repo.find_by_username(username).await?;
            if let Some(existing_user) = existing {
                if existing_user.id != user_id {
                    return Err(AppError::ValidationError("用户名已被占用".to_string()));
                }
            }
        }

        let user = self.user_repo.update(user_id, &request).await?;
        Ok(user.into())
    }

    /// 修改密码
    pub async fn change_password(
        &self,
        user_id: Uuid,
        request: ChangePasswordRequest,
    ) -> Result<(), AppError> {
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("用户不存在".to_string()))?;

        // 验证当前密码
        if !verify_password(&request.current_password, &user.password_hash)? {
            return Err(AppError::Unauthorized("当前密码错误".to_string()));
        }

        // 检查新密码强度
        check_password_strength(&request.new_password)?;

        // 更新密码
        let new_hash = hash_password(&request.new_password)?;
        self.user_repo.update_password(user_id, &new_hash).await?;

        // 登出所有其他设备
        self.user_repo.delete_all_refresh_tokens(user_id).await?;

        tracing::info!(user_id = %user_id, "用户密码已修改");

        Ok(())
    }

    /// 管理员：获取用户列表
    pub async fn list_users(
        &self,
        query: UserListQuery,
    ) -> Result<PaginatedResponse<UserInfo>, AppError> {
        let (users, total) = self.user_repo.list(&query).await?;

        let user_infos: Vec<UserInfo> = users.into_iter().map(|u| u.into()).collect();
        let pagination = Pagination::new(query.page, query.page_size, total);

        Ok(PaginatedResponse::new(user_infos, pagination))
    }

    /// 管理员：根据 ID 获取用户
    pub async fn get_user_by_id(&self, id: Uuid) -> Result<User, AppError> {
        self.user_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound("用户不存在".to_string()))
    }

    /// 管理员：更新用户角色
    pub async fn update_user_role(
        &self,
        user_id: Uuid,
        role: UserRole,
    ) -> Result<UserInfo, AppError> {
        let user = self.user_repo.update_role(user_id, role).await?;
        
        tracing::info!(
            user_id = %user_id,
            role = %user.role,
            "用户角色已更新"
        );
        
        Ok(user.into())
    }

    /// 管理员：禁用/启用用户
    pub async fn set_user_active(
        &self,
        user_id: Uuid,
        is_active: bool,
    ) -> Result<(), AppError> {
        self.user_repo.set_active(user_id, is_active).await?;
        
        tracing::info!(
            user_id = %user_id,
            is_active = is_active,
            "用户状态已更新"
        );
        
        Ok(())
    }

    /// 管理员：删除用户
    pub async fn delete_user(&self, user_id: Uuid) -> Result<(), AppError> {
        self.user_repo.delete(user_id).await?;
        
        tracing::info!(user_id = %user_id, "用户已删除");
        
        Ok(())
    }

    /// 通过邮箱重置密码
    pub async fn reset_password_by_email(
        &self,
        email: &str,
        new_password: &str,
    ) -> Result<(), AppError> {
        // 查找用户
        let user = self
            .user_repo
            .find_by_email(email)
            .await?
            .ok_or_else(|| AppError::NotFound("用户不存在".to_string()))?;

        // 检查新密码强度
        check_password_strength(new_password)?;

        // 更新密码
        let new_hash = hash_password(new_password)?;
        self.user_repo.update_password(user.id, &new_hash).await?;

        // 登出所有设备
        self.user_repo.delete_all_refresh_tokens(user.id).await?;

        tracing::info!(user_id = %user.id, email = %email, "用户密码已通过邮箱重置");

        Ok(())
    }

    // ========== 设备共享 ==========

    /// 共享设备给用户
    pub async fn share_device(
        &self,
        device_id: Uuid,
        user_identifier: &str,
        permission: SharePermission,
    ) -> Result<DeviceShare, AppError> {
        // 查找目标用户
        let target_user = self
            .user_repo
            .find_by_login(user_identifier)
            .await?
            .ok_or_else(|| AppError::NotFound("目标用户不存在".to_string()))?;

        let share = self
            .user_repo
            .add_device_share(device_id, target_user.id, &permission.to_string())
            .await?;

        tracing::info!(
            device_id = %device_id,
            user_id = %target_user.id,
            permission = %permission,
            "设备已共享"
        );

        Ok(share)
    }

    /// 取消设备共享
    pub async fn unshare_device(
        &self,
        device_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        self.user_repo.remove_device_share(device_id, user_id).await?;
        
        tracing::info!(
            device_id = %device_id,
            user_id = %user_id,
            "设备共享已取消"
        );
        
        Ok(())
    }

    /// 获取设备共享列表
    pub async fn get_device_shares(
        &self,
        device_id: Uuid,
    ) -> Result<Vec<DeviceShareInfo>, AppError> {
        let shares = self.user_repo.get_device_shares(device_id).await?;
        
        let mut share_infos = Vec::new();
        for share in shares {
            if let Some(user) = self.user_repo.find_by_id(share.user_id).await? {
                share_infos.push(DeviceShareInfo {
                    device_id: share.device_id,
                    user: user.into(),
                    permission: share.permission,
                    created_at: share.created_at,
                });
            }
        }
        
        Ok(share_infos)
    }

    /// 检查用户对设备的权限
    pub async fn check_device_permission(
        &self,
        device_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        self.user_repo.check_device_permission(device_id, user_id).await
    }

    /// 哈希令牌（用于存储）
    fn hash_token(&self, token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
