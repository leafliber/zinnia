//! 电量业务服务

use crate::db::RedisPool;
use crate::errors::AppError;
use crate::models::{
    AggregateInterval, BatteryAggregatePoint, BatteryData, BatteryQueryRequest,
    BatteryReportRequest, BatteryStatsResponse, LatestBatteryResponse,
};
use crate::repositories::{BatteryRepository, DeviceRepository};
use crate::services::AlertService;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

/// 电量业务服务
pub struct BatteryService {
    battery_repo: BatteryRepository,
    device_repo: DeviceRepository,
    alert_service: Arc<AlertService>,
    redis_pool: Arc<RedisPool>,
}

impl BatteryService {
    pub fn new(
        battery_repo: BatteryRepository,
        device_repo: DeviceRepository,
        alert_service: Arc<AlertService>,
        redis_pool: Arc<RedisPool>,
    ) -> Self {
        Self {
            battery_repo,
            device_repo,
            alert_service,
            redis_pool,
        }
    }

    /// 上报电量数据
    pub async fn report(
        &self,
        device_id: Uuid,
        request: BatteryReportRequest,
    ) -> Result<BatteryData, AppError> {
        // 验证电量值范围
        if request.battery_level < 0 || request.battery_level > 100 {
            return Err(AppError::ValidationError(
                "电量值必须在 0-100 之间".to_string(),
            ));
        }

        // 验证时间戳（不能是未来时间）
        if let Some(recorded_at) = request.recorded_at {
            if recorded_at > Utc::now() {
                return Err(AppError::ValidationError(
                    "记录时间不能是未来时间".to_string(),
                ));
            }
        }

        // 插入数据
        let data = self.battery_repo.insert(device_id, &request).await?;

        // 更新设备最后在线时间
        self.device_repo.update_last_seen(device_id).await?;

        // 更新缓存
        self.update_latest_cache(device_id, &data).await?;

        // 检查预警
        self.check_alerts(device_id, &data).await?;

        Ok(data)
    }

    /// 批量上报电量数据
    pub async fn batch_report(
        &self,
        device_id: Uuid,
        requests: Vec<BatteryReportRequest>,
    ) -> Result<usize, AppError> {
        // 验证所有数据
        for request in &requests {
            if request.battery_level < 0 || request.battery_level > 100 {
                return Err(AppError::ValidationError(
                    "电量值必须在 0-100 之间".to_string(),
                ));
            }
        }

        // 批量插入
        let count = self.battery_repo.batch_insert(device_id, &requests).await?;

        // 更新设备最后在线时间
        self.device_repo.update_last_seen(device_id).await?;

        // 检查最新数据的预警
        if let Some(latest) = requests.last() {
            let data = BatteryData {
                id: Uuid::new_v4(),
                device_id,
                battery_level: latest.battery_level,
                is_charging: latest.is_charging,
                power_saving_mode: latest.power_saving_mode.clone(),
                temperature: latest.temperature,
                voltage: latest.voltage,
                recorded_at: latest.recorded_at.unwrap_or_else(Utc::now),
                created_at: Utc::now(),
            };
            self.check_alerts(device_id, &data).await?;
        }

        Ok(count)
    }

    /// 获取最新电量
    pub async fn get_latest(&self, device_id: Uuid) -> Result<LatestBatteryResponse, AppError> {
        // 先尝试从缓存获取
        let cache_key = format!("battery:latest:{}", device_id);
        if let Some(cached) = self
            .redis_pool
            .get::<LatestBatteryResponse>(&cache_key)
            .await?
        {
            return Ok(cached);
        }

        // 从数据库查询
        let data = self
            .battery_repo
            .query_latest(device_id)
            .await?
            .ok_or_else(|| AppError::NotFound("暂无电量数据".to_string()))?;

        // 获取设备配置以判断低电量阈值
        let config = self
            .device_repo
            .get_config(device_id)
            .await?
            .unwrap_or_default();

        let response = LatestBatteryResponse {
            device_id,
            battery_level: data.battery_level,
            is_charging: data.is_charging,
            power_saving_mode: data.power_saving_mode,
            recorded_at: data.recorded_at,
            is_low_battery: data.battery_level < config.low_battery_threshold,
            is_critical: data.battery_level < config.critical_battery_threshold,
        };

        // 更新缓存
        self.redis_pool.set_ex(&cache_key, &response, 60).await?;

        Ok(response)
    }

    /// 查询历史数据
    pub async fn get_history(
        &self,
        device_id: Uuid,
        request: BatteryQueryRequest,
    ) -> Result<Vec<BatteryData>, AppError> {
        self.battery_repo
            .query_by_time_range(device_id, &request)
            .await
    }

    /// 获取聚合统计
    pub async fn get_aggregated(
        &self,
        device_id: Uuid,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        interval: AggregateInterval,
    ) -> Result<Vec<BatteryAggregatePoint>, AppError> {
        self.battery_repo
            .aggregate_by_interval(device_id, start_time, end_time, &interval)
            .await
    }

    /// 获取统计信息
    pub async fn get_stats(
        &self,
        device_id: Uuid,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<BatteryStatsResponse, AppError> {
        self.battery_repo
            .get_stats(device_id, start_time, end_time)
            .await
    }

    /// 更新最新电量缓存
    async fn update_latest_cache(
        &self,
        device_id: Uuid,
        data: &BatteryData,
    ) -> Result<(), AppError> {
        let config = self
            .device_repo
            .get_config(device_id)
            .await?
            .unwrap_or_default();

        let response = LatestBatteryResponse {
            device_id,
            battery_level: data.battery_level,
            is_charging: data.is_charging,
            power_saving_mode: data.power_saving_mode.clone(),
            recorded_at: data.recorded_at,
            is_low_battery: data.battery_level < config.low_battery_threshold,
            is_critical: data.battery_level < config.critical_battery_threshold,
        };

        let cache_key = format!("battery:latest:{}", device_id);
        self.redis_pool.set_ex(&cache_key, &response, 300).await?;

        Ok(())
    }

    /// 检查预警
    async fn check_alerts(&self, device_id: Uuid, data: &BatteryData) -> Result<(), AppError> {
        // 获取设备信息（需要 owner_id 来触发预警）
        let device = match self.device_repo.find_by_id(device_id).await? {
            Some(d) => d,
            None => {
                tracing::warn!(device_id = %device_id, "设备不存在，跳过预警检查");
                return Ok(());
            }
        };

        let user_id = match device.owner_id {
            Some(uid) => uid,
            None => {
                tracing::debug!(device_id = %device_id, "设备无所有者，跳过预警检查");
                return Ok(());
            }
        };

        // 获取设备配置
        let config = self
            .device_repo
            .get_config(device_id)
            .await?
            .unwrap_or_default();

        // 检查低电量预警
        if data.battery_level < config.critical_battery_threshold && !data.is_charging {
            self.alert_service
                .trigger_critical_battery(
                    device_id,
                    user_id,
                    data.battery_level as f64,
                    config.critical_battery_threshold as f64,
                )
                .await?;
        } else if data.battery_level < config.low_battery_threshold && !data.is_charging {
            self.alert_service
                .trigger_low_battery(
                    device_id,
                    user_id,
                    data.battery_level as f64,
                    config.low_battery_threshold as f64,
                )
                .await?;
        }

        // 检查温度预警
        if let Some(temp) = data.temperature {
            if temp > config.high_temperature_threshold {
                self.alert_service
                    .trigger_high_temperature(
                        device_id,
                        user_id,
                        temp,
                        config.high_temperature_threshold,
                    )
                    .await?;
            }
        }

        Ok(())
    }
}
