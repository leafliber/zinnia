//! 用户数据仓库

use crate::db::PostgresPool;
use crate::errors::AppError;
use crate::models::{
    DeviceShare, UpdateUserRequest, User, UserListQuery, UserRefreshToken, UserRole,
};
use chrono::{Duration, Utc};
use uuid::Uuid;

/// 用户数据仓库
#[derive(Clone)]
pub struct UserRepository {
    pool: PostgresPool,
}

impl UserRepository {
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    // ========== 用户 CRUD ==========

    /// 创建用户
    pub async fn create(
        &self,
        email: &str,
        username: &str,
        password_hash: &str,
    ) -> Result<User, AppError> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (id, email, username, password_hash, role, is_active, email_verified, failed_login_attempts, created_at, updated_at)
            VALUES ($1, $2, $3, $4, 'user', TRUE, FALSE, 0, $5, $6)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(email.to_lowercase())
        .bind(username)
        .bind(password_hash)
        .bind(now)
        .bind(now)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(user)
    }

    /// 根据 ID 查找用户
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, AppError> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(self.pool.pool())
            .await?;

        Ok(user)
    }

    /// 根据邮箱查找用户
    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, AppError> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE LOWER(email) = LOWER($1)")
            .bind(email)
            .fetch_optional(self.pool.pool())
            .await?;

        Ok(user)
    }

    /// 根据用户名查找用户
    pub async fn find_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        let user =
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE LOWER(username) = LOWER($1)")
                .bind(username)
                .fetch_optional(self.pool.pool())
                .await?;

        Ok(user)
    }

    /// 根据邮箱或用户名查找用户
    pub async fn find_by_login(&self, login: &str) -> Result<Option<User>, AppError> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE LOWER(email) = LOWER($1) OR LOWER(username) = LOWER($1)",
        )
        .bind(login)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(user)
    }

    /// 检查邮箱是否已存在
    pub async fn email_exists(&self, email: &str) -> Result<bool, AppError> {
        let result: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users WHERE LOWER(email) = LOWER($1)")
                .bind(email)
                .fetch_one(self.pool.pool())
                .await?;

        Ok(result.0 > 0)
    }

    /// 检查用户名是否已存在
    pub async fn username_exists(&self, username: &str) -> Result<bool, AppError> {
        let result: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users WHERE LOWER(username) = LOWER($1)")
                .bind(username)
                .fetch_one(self.pool.pool())
                .await?;

        Ok(result.0 > 0)
    }

    /// 更新用户信息
    pub async fn update(&self, id: Uuid, request: &UpdateUserRequest) -> Result<User, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET username = COALESCE($2, username),
                metadata = COALESCE($3, metadata),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(&request.username)
        .bind(&request.metadata)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(user)
    }

    /// 更新密码
    pub async fn update_password(&self, id: Uuid, password_hash: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE users SET password_hash = $2, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .bind(password_hash)
            .execute(self.pool.pool())
            .await?;

        Ok(())
    }

    /// 更新最后登录时间
    pub async fn update_last_login(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE users SET last_login_at = NOW(), failed_login_attempts = 0, locked_until = NULL WHERE id = $1",
        )
        .bind(id)
        .execute(self.pool.pool())
        .await?;

        Ok(())
    }

    /// 记录登录失败
    pub async fn record_failed_login(&self, id: Uuid) -> Result<i32, AppError> {
        let result: (i32,) = sqlx::query_as(
            r#"
            UPDATE users
            SET failed_login_attempts = failed_login_attempts + 1,
                locked_until = CASE
                    WHEN failed_login_attempts + 1 >= 5 THEN NOW() + INTERVAL '15 minutes'
                    ELSE locked_until
                END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING failed_login_attempts
            "#,
        )
        .bind(id)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(result.0)
    }

    /// 检查用户是否被锁定
    pub async fn is_locked(&self, id: Uuid) -> Result<bool, AppError> {
        let user = self.find_by_id(id).await?;

        if let Some(user) = user {
            if let Some(locked_until) = user.locked_until {
                return Ok(locked_until > Utc::now());
            }
        }

        Ok(false)
    }

    /// 解锁用户
    pub async fn unlock(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE users SET failed_login_attempts = 0, locked_until = NULL, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(self.pool.pool())
        .await?;

        Ok(())
    }

    /// 更新用户角色（管理员操作）
    pub async fn update_role(&self, id: Uuid, role: UserRole) -> Result<User, AppError> {
        let user = sqlx::query_as::<_, User>(
            "UPDATE users SET role = $2, updated_at = NOW() WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(role)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(user)
    }

    /// 禁用/启用用户
    pub async fn set_active(&self, id: Uuid, is_active: bool) -> Result<(), AppError> {
        sqlx::query("UPDATE users SET is_active = $2, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .bind(is_active)
            .execute(self.pool.pool())
            .await?;

        Ok(())
    }

    /// 删除用户
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(self.pool.pool())
            .await?;

        Ok(())
    }

    /// 查询用户列表
    pub async fn list(&self, query: &UserListQuery) -> Result<(Vec<User>, i64), AppError> {
        let offset = (query.page - 1) * query.page_size;

        // 使用完全参数化查询防止 SQL 注入
        // 根据不同的筛选条件组合选择对应的查询
        match (&query.role, query.is_active, &query.search) {
            // 有角色 + 有状态 + 有搜索
            (Some(role), Some(is_active), Some(search)) => {
                let search_pattern = format!("%{}%", search);
                let total: (i64,) = sqlx::query_as(
                    r#"SELECT COUNT(*) FROM users 
                       WHERE role = $1 AND is_active = $2 
                       AND (LOWER(email) LIKE LOWER($3) OR LOWER(username) LIKE LOWER($3))"#,
                )
                .bind(role)
                .bind(is_active)
                .bind(&search_pattern)
                .fetch_one(self.pool.pool())
                .await?;

                let users = sqlx::query_as::<_, User>(
                    r#"SELECT * FROM users 
                       WHERE role = $1 AND is_active = $2 
                       AND (LOWER(email) LIKE LOWER($3) OR LOWER(username) LIKE LOWER($3))
                       ORDER BY created_at DESC LIMIT $4 OFFSET $5"#,
                )
                .bind(role)
                .bind(is_active)
                .bind(&search_pattern)
                .bind(query.page_size)
                .bind(offset)
                .fetch_all(self.pool.pool())
                .await?;

                Ok((users, total.0))
            }
            // 有角色 + 有状态
            (Some(role), Some(is_active), None) => {
                let total: (i64,) =
                    sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = $1 AND is_active = $2")
                        .bind(role)
                        .bind(is_active)
                        .fetch_one(self.pool.pool())
                        .await?;

                let users = sqlx::query_as::<_, User>(
                    "SELECT * FROM users WHERE role = $1 AND is_active = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4"
                )
                .bind(role)
                .bind(is_active)
                .bind(query.page_size)
                .bind(offset)
                .fetch_all(self.pool.pool())
                .await?;

                Ok((users, total.0))
            }
            // 有角色 + 有搜索
            (Some(role), None, Some(search)) => {
                let search_pattern = format!("%{}%", search);
                let total: (i64,) = sqlx::query_as(
                    r#"SELECT COUNT(*) FROM users 
                       WHERE role = $1 
                       AND (LOWER(email) LIKE LOWER($2) OR LOWER(username) LIKE LOWER($2))"#,
                )
                .bind(role)
                .bind(&search_pattern)
                .fetch_one(self.pool.pool())
                .await?;

                let users = sqlx::query_as::<_, User>(
                    r#"SELECT * FROM users 
                       WHERE role = $1 
                       AND (LOWER(email) LIKE LOWER($2) OR LOWER(username) LIKE LOWER($2))
                       ORDER BY created_at DESC LIMIT $3 OFFSET $4"#,
                )
                .bind(role)
                .bind(&search_pattern)
                .bind(query.page_size)
                .bind(offset)
                .fetch_all(self.pool.pool())
                .await?;

                Ok((users, total.0))
            }
            // 有状态 + 有搜索
            (None, Some(is_active), Some(search)) => {
                let search_pattern = format!("%{}%", search);
                let total: (i64,) = sqlx::query_as(
                    r#"SELECT COUNT(*) FROM users 
                       WHERE is_active = $1 
                       AND (LOWER(email) LIKE LOWER($2) OR LOWER(username) LIKE LOWER($2))"#,
                )
                .bind(is_active)
                .bind(&search_pattern)
                .fetch_one(self.pool.pool())
                .await?;

                let users = sqlx::query_as::<_, User>(
                    r#"SELECT * FROM users 
                       WHERE is_active = $1 
                       AND (LOWER(email) LIKE LOWER($2) OR LOWER(username) LIKE LOWER($2))
                       ORDER BY created_at DESC LIMIT $3 OFFSET $4"#,
                )
                .bind(is_active)
                .bind(&search_pattern)
                .bind(query.page_size)
                .bind(offset)
                .fetch_all(self.pool.pool())
                .await?;

                Ok((users, total.0))
            }
            // 只有角色
            (Some(role), None, None) => {
                let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = $1")
                    .bind(role)
                    .fetch_one(self.pool.pool())
                    .await?;

                let users = sqlx::query_as::<_, User>(
                    "SELECT * FROM users WHERE role = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
                )
                .bind(role)
                .bind(query.page_size)
                .bind(offset)
                .fetch_all(self.pool.pool())
                .await?;

                Ok((users, total.0))
            }
            // 只有状态
            (None, Some(is_active), None) => {
                let total: (i64,) =
                    sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_active = $1")
                        .bind(is_active)
                        .fetch_one(self.pool.pool())
                        .await?;

                let users = sqlx::query_as::<_, User>(
                    "SELECT * FROM users WHERE is_active = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
                )
                .bind(is_active)
                .bind(query.page_size)
                .bind(offset)
                .fetch_all(self.pool.pool())
                .await?;

                Ok((users, total.0))
            }
            // 只有搜索
            (None, None, Some(search)) => {
                let search_pattern = format!("%{}%", search);
                let total: (i64,) = sqlx::query_as(
                    r#"SELECT COUNT(*) FROM users 
                       WHERE LOWER(email) LIKE LOWER($1) OR LOWER(username) LIKE LOWER($1)"#,
                )
                .bind(&search_pattern)
                .fetch_one(self.pool.pool())
                .await?;

                let users = sqlx::query_as::<_, User>(
                    r#"SELECT * FROM users 
                       WHERE LOWER(email) LIKE LOWER($1) OR LOWER(username) LIKE LOWER($1)
                       ORDER BY created_at DESC LIMIT $2 OFFSET $3"#,
                )
                .bind(&search_pattern)
                .bind(query.page_size)
                .bind(offset)
                .fetch_all(self.pool.pool())
                .await?;

                Ok((users, total.0))
            }
            // 无筛选条件
            (None, None, None) => {
                let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
                    .fetch_one(self.pool.pool())
                    .await?;

                let users = sqlx::query_as::<_, User>(
                    "SELECT * FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2",
                )
                .bind(query.page_size)
                .bind(offset)
                .fetch_all(self.pool.pool())
                .await?;

                Ok((users, total.0))
            }
        }
    }

    // ========== 刷新令牌管理 ==========

    /// 保存刷新令牌
    pub async fn save_refresh_token(
        &self,
        user_id: Uuid,
        token_hash: &str,
        device_info: Option<&str>,
        ip_address: Option<&str>,
        expires_days: i64,
    ) -> Result<UserRefreshToken, AppError> {
        let id = Uuid::new_v4();
        let expires_at = Utc::now() + Duration::days(expires_days);

        let token = sqlx::query_as::<_, UserRefreshToken>(
            r#"
            INSERT INTO user_refresh_tokens (id, user_id, token_hash, device_info, ip_address, expires_at, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(user_id)
        .bind(token_hash)
        .bind(device_info)
        .bind(ip_address)
        .bind(expires_at)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(token)
    }

    /// 根据令牌哈希查找
    pub async fn find_refresh_token_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<UserRefreshToken>, AppError> {
        let token = sqlx::query_as::<_, UserRefreshToken>(
            "SELECT * FROM user_refresh_tokens WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(token_hash)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(token)
    }

    /// 删除刷新令牌
    pub async fn delete_refresh_token(&self, token_hash: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM user_refresh_tokens WHERE token_hash = $1")
            .bind(token_hash)
            .execute(self.pool.pool())
            .await?;

        Ok(())
    }

    /// 删除用户所有刷新令牌（登出所有设备）
    pub async fn delete_all_refresh_tokens(&self, user_id: Uuid) -> Result<u64, AppError> {
        let result = sqlx::query("DELETE FROM user_refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .execute(self.pool.pool())
            .await?;

        Ok(result.rows_affected())
    }

    /// 清理过期的刷新令牌
    pub async fn cleanup_expired_tokens(&self) -> Result<u64, AppError> {
        let result = sqlx::query("DELETE FROM user_refresh_tokens WHERE expires_at < NOW()")
            .execute(self.pool.pool())
            .await?;

        Ok(result.rows_affected())
    }

    // ========== 设备共享 ==========

    /// 添加设备共享
    pub async fn add_device_share(
        &self,
        device_id: Uuid,
        user_id: Uuid,
        permission: &str,
    ) -> Result<DeviceShare, AppError> {
        let share = sqlx::query_as::<_, DeviceShare>(
            r#"
            INSERT INTO device_shares (device_id, user_id, permission, created_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (device_id, user_id) DO UPDATE SET permission = $3
            RETURNING *
            "#,
        )
        .bind(device_id)
        .bind(user_id)
        .bind(permission)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(share)
    }

    /// 移除设备共享
    pub async fn remove_device_share(
        &self,
        device_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query("DELETE FROM device_shares WHERE device_id = $1 AND user_id = $2")
            .bind(device_id)
            .bind(user_id)
            .execute(self.pool.pool())
            .await?;

        Ok(())
    }

    /// 获取设备的共享列表
    pub async fn get_device_shares(&self, device_id: Uuid) -> Result<Vec<DeviceShare>, AppError> {
        let shares = sqlx::query_as::<_, DeviceShare>(
            "SELECT * FROM device_shares WHERE device_id = $1 ORDER BY created_at",
        )
        .bind(device_id)
        .fetch_all(self.pool.pool())
        .await?;

        Ok(shares)
    }

    /// 获取用户被共享的设备
    pub async fn get_shared_devices(&self, user_id: Uuid) -> Result<Vec<DeviceShare>, AppError> {
        let shares = sqlx::query_as::<_, DeviceShare>(
            "SELECT * FROM device_shares WHERE user_id = $1 ORDER BY created_at",
        )
        .bind(user_id)
        .fetch_all(self.pool.pool())
        .await?;

        Ok(shares)
    }

    /// 检查用户对设备的权限
    pub async fn check_device_permission(
        &self,
        device_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT permission FROM device_shares WHERE device_id = $1 AND user_id = $2",
        )
        .bind(device_id)
        .bind(user_id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(result.map(|r| r.0))
    }
}
