//! 设备数据仓库

use crate::db::PostgresPool;
use crate::errors::AppError;
use crate::models::{
    CreateDeviceRequest, Device, DeviceConfig, DeviceListQuery, DeviceStatus,
    UpdateDeviceConfigRequest, UpdateDeviceRequest,
};
use chrono::Utc;
use uuid::Uuid;

/// 设备数据仓库
#[derive(Clone)]
pub struct DeviceRepository {
    pool: PostgresPool,
}

impl DeviceRepository {
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    /// 创建设备
    pub async fn create(
        &self,
        request: &CreateDeviceRequest,
        api_key_hash: &str,
        api_key_prefix: &str,
        owner_id: Option<Uuid>,
    ) -> Result<Device, AppError> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let device = sqlx::query_as::<_, Device>(
            r#"
            INSERT INTO devices (id, owner_id, name, device_type, status, api_key_hash, api_key_prefix, created_at, updated_at, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(owner_id)
        .bind(&request.name)
        .bind(&request.device_type)
        .bind(DeviceStatus::Offline)
        .bind(api_key_hash)
        .bind(api_key_prefix)
        .bind(now)
        .bind(now)
        .bind(&request.metadata)
        .fetch_one(self.pool.pool())
        .await?;

        // 创建默认配置
        self.create_default_config(id).await?;

        Ok(device)
    }

    /// 创建默认设备配置
    async fn create_default_config(&self, device_id: Uuid) -> Result<(), AppError> {
        let config = DeviceConfig::default();

        sqlx::query(
            r#"
            INSERT INTO device_configs (device_id, low_battery_threshold, critical_battery_threshold, report_interval_seconds, high_temperature_threshold, updated_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            "#,
        )
        .bind(device_id)
        .bind(config.low_battery_threshold)
        .bind(config.critical_battery_threshold)
        .bind(config.report_interval_seconds)
        .bind(config.high_temperature_threshold)
        .execute(self.pool.pool())
        .await?;

        Ok(())
    }

    /// 根据 ID 查找设备
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Device>, AppError> {
        let device = sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE id = $1")
            .bind(id)
            .fetch_optional(self.pool.pool())
            .await?;

        Ok(device)
    }

    /// 根据 API Key 前缀查找设备
    pub async fn find_by_api_key_prefix(&self, prefix: &str) -> Result<Option<Device>, AppError> {
        let device = sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE api_key_prefix = $1")
            .bind(prefix)
            .fetch_optional(self.pool.pool())
            .await?;

        Ok(device)
    }

    /// 更新设备
    pub async fn update(
        &self,
        id: Uuid,
        request: &UpdateDeviceRequest,
    ) -> Result<Device, AppError> {
        let device = sqlx::query_as::<_, Device>(
            r#"
            UPDATE devices
            SET name = COALESCE($2, name),
                status = COALESCE($3, status),
                metadata = COALESCE($4, metadata),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(&request.name)
        .bind(&request.status)
        .bind(&request.metadata)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(device)
    }

    /// 更新设备最后在线时间
    pub async fn update_last_seen(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE devices SET last_seen_at = NOW(), status = 'online' WHERE id = $1")
            .bind(id)
            .execute(self.pool.pool())
            .await?;

        Ok(())
    }

    /// 轮换 API Key
    pub async fn rotate_api_key(
        &self,
        id: Uuid,
        new_hash: &str,
        new_prefix: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE devices SET api_key_hash = $2, api_key_prefix = $3, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(new_hash)
        .bind(new_prefix)
        .execute(self.pool.pool())
        .await?;

        Ok(())
    }

    /// 删除设备
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM devices WHERE id = $1")
            .bind(id)
            .execute(self.pool.pool())
            .await?;

        Ok(())
    }

    /// 查询设备列表
    pub async fn list(&self, query: &DeviceListQuery) -> Result<(Vec<Device>, i64), AppError> {
        let offset = (query.page - 1) * query.page_size;

        // 构建查询条件
        let mut conditions = vec!["1=1".to_string()];

        if let Some(ref status) = query.status {
            conditions.push(format!("status = '{:?}'", status).to_lowercase());
        }

        if let Some(ref device_type) = query.device_type {
            conditions.push(format!("device_type = '{}'", device_type));
        }

        // 按所有者筛选
        if let Some(owner_id) = query.owner_id {
            if query.include_shared {
                // 包含自己拥有的设备和共享给自己的设备
                conditions.push(format!(
                    "(owner_id = '{}' OR id IN (SELECT device_id FROM device_shares WHERE user_id = '{}'))",
                    owner_id, owner_id
                ));
            } else {
                // 只查询自己拥有的设备
                conditions.push(format!("owner_id = '{}'", owner_id));
            }
        }

        let where_clause = conditions.join(" AND ");

        // 查询总数
        let count_sql = format!("SELECT COUNT(*) FROM devices WHERE {}", where_clause);
        let total: (i64,) = sqlx::query_as(&count_sql)
            .fetch_one(self.pool.pool())
            .await?;

        // 查询数据
        let list_sql = format!(
            "SELECT * FROM devices WHERE {} ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            where_clause
        );
        let devices = sqlx::query_as::<_, Device>(&list_sql)
            .bind(query.page_size)
            .bind(offset)
            .fetch_all(self.pool.pool())
            .await?;

        Ok((devices, total.0))
    }

    /// 获取设备配置
    pub async fn get_config(&self, device_id: Uuid) -> Result<Option<DeviceConfig>, AppError> {
        let config =
            sqlx::query_as::<_, DeviceConfig>("SELECT * FROM device_configs WHERE device_id = $1")
                .bind(device_id)
                .fetch_optional(self.pool.pool())
                .await?;

        Ok(config)
    }

    /// 更新设备配置
    pub async fn update_config(
        &self,
        device_id: Uuid,
        request: &UpdateDeviceConfigRequest,
    ) -> Result<DeviceConfig, AppError> {
        let config = sqlx::query_as::<_, DeviceConfig>(
            r#"
            UPDATE device_configs
            SET low_battery_threshold = COALESCE($2, low_battery_threshold),
                critical_battery_threshold = COALESCE($3, critical_battery_threshold),
                report_interval_seconds = COALESCE($4, report_interval_seconds),
                high_temperature_threshold = COALESCE($5, high_temperature_threshold),
                updated_at = NOW()
            WHERE device_id = $1
            RETURNING *
            "#,
        )
        .bind(device_id)
        .bind(request.low_battery_threshold)
        .bind(request.critical_battery_threshold)
        .bind(request.report_interval_seconds)
        .bind(request.high_temperature_threshold)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(config)
    }

    /// 检查用户是否有权访问设备
    pub async fn user_can_access(&self, device_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        let result: Option<(i32,)> = sqlx::query_as(
            r#"
            SELECT 1 FROM devices WHERE id = $1 AND owner_id = $2
            UNION
            SELECT 1 FROM device_shares WHERE device_id = $1 AND user_id = $2
            "#,
        )
        .bind(device_id)
        .bind(user_id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(result.is_some())
    }

    /// 检查用户是否拥有设备
    pub async fn user_owns_device(&self, device_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        let result: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM devices WHERE id = $1 AND owner_id = $2")
                .bind(device_id)
                .bind(user_id)
                .fetch_optional(self.pool.pool())
                .await?;

        Ok(result.is_some())
    }

    /// 获取用户拥有的设备数量
    pub async fn count_user_devices(&self, user_id: Uuid) -> Result<i64, AppError> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices WHERE owner_id = $1")
            .bind(user_id)
            .fetch_one(self.pool.pool())
            .await?;

        Ok(result.0)
    }
}
