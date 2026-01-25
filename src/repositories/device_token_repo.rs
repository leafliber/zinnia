//! 设备访问令牌数据仓库

use crate::db::PostgresPool;
use crate::errors::AppError;
use crate::models::{DeviceAccessToken, TokenPermission};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 创建令牌的参数
pub struct CreateTokenParams {
    pub device_id: Uuid,
    pub created_by: Uuid,
    pub token_hash: String,
    pub token_prefix: String,
    pub name: String,
    pub permission: TokenPermission,
    pub expires_at: Option<DateTime<Utc>>,
    pub allowed_ips: Option<Vec<String>>,
    pub rate_limit_per_minute: Option<i32>,
}

/// 设备访问令牌仓库
#[derive(Clone)]
pub struct DeviceAccessTokenRepository {
    pool: PostgresPool,
}

impl DeviceAccessTokenRepository {
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    /// 创建访问令牌
    pub async fn create(&self, params: CreateTokenParams) -> Result<DeviceAccessToken, AppError> {
        let token = sqlx::query_as::<_, DeviceAccessToken>(
            r#"
            INSERT INTO device_access_tokens 
                (device_id, created_by, token_hash, token_prefix, name, permission, 
                 expires_at, allowed_ips, rate_limit_per_minute)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#,
        )
        .bind(params.device_id)
        .bind(params.created_by)
        .bind(&params.token_hash)
        .bind(&params.token_prefix)
        .bind(&params.name)
        .bind(&params.permission)
        .bind(params.expires_at)
        .bind(&params.allowed_ips)
        .bind(params.rate_limit_per_minute)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(token)
    }

    /// 根据 ID 查找令牌
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<DeviceAccessToken>, AppError> {
        let token = sqlx::query_as::<_, DeviceAccessToken>(
            "SELECT * FROM device_access_tokens WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(token)
    }

    /// 根据令牌前缀查找（用于认证）
    pub async fn find_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Option<DeviceAccessToken>, AppError> {
        let token = sqlx::query_as::<_, DeviceAccessToken>(
            "SELECT * FROM device_access_tokens WHERE token_prefix = $1",
        )
        .bind(prefix)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(token)
    }

    /// 查找有效的令牌（未过期、未吊销）
    pub async fn find_valid_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Option<DeviceAccessToken>, AppError> {
        let token = sqlx::query_as::<_, DeviceAccessToken>(
            r#"
            SELECT * FROM device_access_tokens 
            WHERE token_prefix = $1 
              AND is_revoked = FALSE
              AND (expires_at IS NULL OR expires_at > NOW())
            "#,
        )
        .bind(prefix)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(token)
    }

    /// 列出设备的所有令牌
    pub async fn list_by_device(
        &self,
        device_id: Uuid,
        include_revoked: bool,
        include_expired: bool,
    ) -> Result<Vec<DeviceAccessToken>, AppError> {
        let tokens = if include_revoked && include_expired {
            sqlx::query_as::<_, DeviceAccessToken>(
                "SELECT * FROM device_access_tokens WHERE device_id = $1 ORDER BY created_at DESC",
            )
            .bind(device_id)
            .fetch_all(self.pool.pool())
            .await?
        } else if include_revoked {
            sqlx::query_as::<_, DeviceAccessToken>(
                r#"
                SELECT * FROM device_access_tokens 
                WHERE device_id = $1 
                  AND (expires_at IS NULL OR expires_at > NOW())
                ORDER BY created_at DESC
                "#,
            )
            .bind(device_id)
            .fetch_all(self.pool.pool())
            .await?
        } else if include_expired {
            sqlx::query_as::<_, DeviceAccessToken>(
                r#"
                SELECT * FROM device_access_tokens 
                WHERE device_id = $1 AND is_revoked = FALSE
                ORDER BY created_at DESC
                "#,
            )
            .bind(device_id)
            .fetch_all(self.pool.pool())
            .await?
        } else {
            sqlx::query_as::<_, DeviceAccessToken>(
                r#"
                SELECT * FROM device_access_tokens 
                WHERE device_id = $1 
                  AND is_revoked = FALSE
                  AND (expires_at IS NULL OR expires_at > NOW())
                ORDER BY created_at DESC
                "#,
            )
            .bind(device_id)
            .fetch_all(self.pool.pool())
            .await?
        };

        Ok(tokens)
    }

    /// 吊销令牌
    pub async fn revoke(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE device_access_tokens 
            SET is_revoked = TRUE, revoked_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(self.pool.pool())
        .await?;

        Ok(())
    }

    /// 吊销设备的所有令牌
    pub async fn revoke_all_for_device(&self, device_id: Uuid) -> Result<u64, AppError> {
        let result = sqlx::query(
            r#"
            UPDATE device_access_tokens 
            SET is_revoked = TRUE, revoked_at = NOW()
            WHERE device_id = $1 AND is_revoked = FALSE
            "#,
        )
        .bind(device_id)
        .execute(self.pool.pool())
        .await?;

        Ok(result.rows_affected())
    }

    /// 更新令牌使用记录
    pub async fn record_usage(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE device_access_tokens 
            SET last_used_at = NOW(), use_count = use_count + 1
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(self.pool.pool())
        .await?;

        Ok(())
    }

    /// 统计设备的有效令牌数量
    pub async fn count_valid_tokens(&self, device_id: Uuid) -> Result<i64, AppError> {
        let result: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM device_access_tokens 
            WHERE device_id = $1 
              AND is_revoked = FALSE
              AND (expires_at IS NULL OR expires_at > NOW())
            "#,
        )
        .bind(device_id)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(result.0)
    }

    /// 删除过期的令牌（清理任务用）
    pub async fn cleanup_expired(&self, days_old: i32) -> Result<u64, AppError> {
        let result = sqlx::query(
            r#"
            DELETE FROM device_access_tokens 
            WHERE (is_revoked = TRUE AND revoked_at < NOW() - INTERVAL '1 day' * $1)
               OR (expires_at IS NOT NULL AND expires_at < NOW() - INTERVAL '1 day' * $1)
            "#,
        )
        .bind(days_old)
        .execute(self.pool.pool())
        .await?;

        Ok(result.rows_affected())
    }

    /// 检查用户是否拥有该令牌的设备
    pub async fn user_owns_token(&self, token_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        let result: Option<(i32,)> = sqlx::query_as(
            r#"
            SELECT 1 FROM device_access_tokens t
            JOIN devices d ON t.device_id = d.id
            WHERE t.id = $1 AND d.owner_id = $2
            "#,
        )
        .bind(token_id)
        .bind(user_id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(result.is_some())
    }
}
