//! 预警业务服务

use crate::errors::AppError;
use crate::models::{
    AlertEvent, AlertListQuery, AlertRule, AlertStatus, AlertType,
    CreateAlertRuleRequest, PaginatedResponse, Pagination, UpdateAlertStatusRequest,
};
use crate::repositories::AlertRepository;
use uuid::Uuid;

/// 预警业务服务
pub struct AlertService {
    alert_repo: AlertRepository,
}

impl AlertService {
    pub fn new(alert_repo: AlertRepository) -> Self {
        Self { alert_repo }
    }

    /// 创建预警规则
    pub async fn create_rule(&self, request: CreateAlertRuleRequest) -> Result<AlertRule, AppError> {
        self.alert_repo.create_rule(&request).await
    }

    /// 获取所有启用的规则
    pub async fn get_enabled_rules(&self) -> Result<Vec<AlertRule>, AppError> {
        self.alert_repo.get_enabled_rules().await
    }

    /// 触发低电量预警
    pub async fn trigger_low_battery(&self, device_id: Uuid, level: f64) -> Result<Option<AlertEvent>, AppError> {
        self.trigger_alert(
            device_id,
            AlertType::LowBattery,
            level,
            &format!("设备电量低: {}%", level as i32),
        )
        .await
    }

    /// 触发临界电量预警
    pub async fn trigger_critical_battery(
        &self,
        device_id: Uuid,
        level: f64,
    ) -> Result<Option<AlertEvent>, AppError> {
        self.trigger_alert(
            device_id,
            AlertType::CriticalBattery,
            level,
            &format!("设备电量临界: {}%", level as i32),
        )
        .await
    }

    /// 触发高温预警
    pub async fn trigger_high_temperature(
        &self,
        device_id: Uuid,
        temperature: f64,
    ) -> Result<Option<AlertEvent>, AppError> {
        self.trigger_alert(
            device_id,
            AlertType::HighTemperature,
            temperature,
            &format!("设备温度过高: {:.1}°C", temperature),
        )
        .await
    }

    /// 触发设备离线预警
    pub async fn trigger_device_offline(&self, device_id: Uuid) -> Result<Option<AlertEvent>, AppError> {
        self.trigger_alert(
            device_id,
            AlertType::DeviceOffline,
            0.0,
            "设备已离线",
        )
        .await
    }

    /// 触发预警
    async fn trigger_alert(
        &self,
        device_id: Uuid,
        alert_type: AlertType,
        value: f64,
        message: &str,
    ) -> Result<Option<AlertEvent>, AppError> {
        // 获取对应的预警规则
        let rule = match self.alert_repo.get_rule_by_type(&alert_type).await? {
            Some(r) => r,
            None => {
                tracing::debug!(
                    device_id = %device_id,
                    alert_type = ?alert_type,
                    "未找到对应的预警规则"
                );
                return Ok(None);
            }
        };

        // 检查是否在冷却期内
        if self
            .alert_repo
            .is_in_cooldown(device_id, &alert_type, rule.cooldown_minutes)
            .await?
        {
            tracing::debug!(
                device_id = %device_id,
                alert_type = ?alert_type,
                "预警处于冷却期内"
            );
            return Ok(None);
        }

        // 创建预警事件
        let event = self
            .alert_repo
            .create_event(device_id, &rule, value, message)
            .await?;

        tracing::info!(
            device_id = %device_id,
            alert_type = ?alert_type,
            level = ?rule.level,
            value = value,
            "触发预警"
        );

        // TODO: 发送通知（webhook、邮件等）

        Ok(Some(event))
    }

    /// 更新预警状态
    pub async fn update_status(
        &self,
        event_id: Uuid,
        request: UpdateAlertStatusRequest,
    ) -> Result<AlertEvent, AppError> {
        self.alert_repo.update_event_status(event_id, &request).await
    }

    /// 确认预警
    pub async fn acknowledge(&self, event_id: Uuid) -> Result<AlertEvent, AppError> {
        self.update_status(
            event_id,
            UpdateAlertStatusRequest {
                status: AlertStatus::Acknowledged,
            },
        )
        .await
    }

    /// 解决预警
    pub async fn resolve(&self, event_id: Uuid) -> Result<AlertEvent, AppError> {
        self.update_status(
            event_id,
            UpdateAlertStatusRequest {
                status: AlertStatus::Resolved,
            },
        )
        .await
    }

    /// 查询预警列表
    pub async fn list(&self, query: AlertListQuery) -> Result<PaginatedResponse<AlertEvent>, AppError> {
        let (events, total) = self.alert_repo.list_events(&query).await?;

        let pagination = Pagination::new(query.page, query.page_size, total);

        Ok(PaginatedResponse::new(events, pagination))
    }

    /// 获取设备活跃预警数
    pub async fn count_active(&self, device_id: Uuid) -> Result<i64, AppError> {
        self.alert_repo.count_active_alerts(device_id).await
    }
}
