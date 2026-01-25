//! 电量数据仓库

use crate::db::PostgresPool;
use crate::errors::AppError;
use crate::models::{
    AggregateInterval, BatteryAggregatePoint, BatteryData, BatteryQueryRequest,
    BatteryReportRequest, BatteryStatsResponse,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 电量数据仓库
#[derive(Clone)]
pub struct BatteryRepository {
    pool: PostgresPool,
}

impl BatteryRepository {
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    /// 插入电量数据
    pub async fn insert(
        &self,
        device_id: Uuid,
        request: &BatteryReportRequest,
    ) -> Result<BatteryData, AppError> {
        let id = Uuid::new_v4();
        let recorded_at = request.recorded_at.unwrap_or_else(Utc::now);

        let data = sqlx::query_as::<_, BatteryData>(
            r#"
            INSERT INTO battery_data (id, device_id, battery_level, is_charging, power_saving_mode, temperature, voltage, recorded_at, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(device_id)
        .bind(request.battery_level)
        .bind(request.is_charging)
        .bind(&request.power_saving_mode)
        .bind(request.temperature)
        .bind(request.voltage)
        .bind(recorded_at)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(data)
    }

    /// 批量插入电量数据
    pub async fn batch_insert(
        &self,
        device_id: Uuid,
        requests: &[BatteryReportRequest],
    ) -> Result<usize, AppError> {
        if requests.is_empty() {
            return Ok(0);
        }

        // 限制单次批量插入数量
        let max_batch_size = 1000;
        if requests.len() > max_batch_size {
            return Err(AppError::ValidationError(format!(
                "批量插入数量不能超过 {}",
                max_batch_size
            )));
        }

        // 使用事务进行批量插入
        let mut tx = self.pool.pool().begin().await?;
        let mut count = 0;

        for request in requests {
            let id = Uuid::new_v4();
            let recorded_at = request.recorded_at.unwrap_or_else(Utc::now);

            sqlx::query(
                r#"
                INSERT INTO battery_data (id, device_id, battery_level, is_charging, power_saving_mode, temperature, voltage, recorded_at, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
                "#,
            )
            .bind(id)
            .bind(device_id)
            .bind(request.battery_level)
            .bind(request.is_charging)
            .bind(&request.power_saving_mode)
            .bind(request.temperature)
            .bind(request.voltage)
            .bind(recorded_at)
            .execute(&mut *tx)
            .await?;

            count += 1;
        }

        tx.commit().await?;
        Ok(count)
    }

    /// 查询时间范围内的电量数据
    pub async fn query_by_time_range(
        &self,
        device_id: Uuid,
        request: &BatteryQueryRequest,
    ) -> Result<Vec<BatteryData>, AppError> {
        // 验证时间范围
        request
            .validate_time_range()
            .map_err(AppError::ValidationError)?;

        let data = sqlx::query_as::<_, BatteryData>(
            r#"
            SELECT * FROM battery_data
            WHERE device_id = $1 AND recorded_at >= $2 AND recorded_at <= $3
            ORDER BY recorded_at DESC
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(device_id)
        .bind(request.start_time)
        .bind(request.end_time)
        .bind(request.limit)
        .bind(request.offset)
        .fetch_all(self.pool.pool())
        .await?;

        Ok(data)
    }

    /// 查询最新电量数据
    pub async fn query_latest(&self, device_id: Uuid) -> Result<Option<BatteryData>, AppError> {
        let data = sqlx::query_as::<_, BatteryData>(
            r#"
            SELECT * FROM battery_data
            WHERE device_id = $1
            ORDER BY recorded_at DESC
            LIMIT 1
            "#,
        )
        .bind(device_id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(data)
    }

    /// 时间聚合查询（利用 TimescaleDB 的 time_bucket）
    pub async fn aggregate_by_interval(
        &self,
        device_id: Uuid,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        interval: &AggregateInterval,
    ) -> Result<Vec<BatteryAggregatePoint>, AppError> {
        let interval_str = interval.to_timescaledb_interval();

        let data = sqlx::query_as::<_, BatteryAggregatePoint>(&format!(
            r#"
                SELECT 
                    time_bucket('{}', recorded_at) AS bucket,
                    AVG(battery_level)::float8 AS avg_level,
                    MIN(battery_level) AS min_level,
                    MAX(battery_level) AS max_level,
                    COUNT(*) AS count
                FROM battery_data
                WHERE device_id = $1 AND recorded_at >= $2 AND recorded_at <= $3
                GROUP BY bucket
                ORDER BY bucket DESC
                "#,
            interval_str
        ))
        .bind(device_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(self.pool.pool())
        .await?;

        Ok(data)
    }

    /// 获取电量统计
    pub async fn get_stats(
        &self,
        device_id: Uuid,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<BatteryStatsResponse, AppError> {
        let stats = sqlx::query_as::<_, BatteryStatsResponse>(
            r#"
            SELECT 
                $1::uuid AS device_id,
                $2::timestamptz AS period_start,
                $3::timestamptz AS period_end,
                COALESCE(AVG(battery_level), 0)::float8 AS avg_battery_level,
                COALESCE(MIN(battery_level), 0) AS min_battery_level,
                COALESCE(MAX(battery_level), 100) AS max_battery_level,
                COUNT(*) AS total_records,
                COALESCE(SUM(CASE WHEN is_charging THEN 1 ELSE 0 END), 0) AS charging_duration_minutes,
                COALESCE(SUM(CASE WHEN battery_level < 20 THEN 1 ELSE 0 END), 0) AS low_battery_count
            FROM battery_data
            WHERE device_id = $1 AND recorded_at >= $2 AND recorded_at <= $3
            "#,
        )
        .bind(device_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(stats)
    }

    /// 删除过期数据（用于数据保留策略）
    pub async fn delete_expired(&self, retention_days: i32) -> Result<u64, AppError> {
        let result = sqlx::query(
            r#"
            DELETE FROM battery_data
            WHERE recorded_at < NOW() - INTERVAL '1 day' * $1
            "#,
        )
        .bind(retention_days)
        .execute(self.pool.pool())
        .await?;

        Ok(result.rows_affected())
    }
}
