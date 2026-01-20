//! 预警业务服务

use crate::errors::AppError;
use crate::models::{
    AlertEvent, AlertListQuery, AlertRule, AlertStatus, AlertType,
    CreateAlertRuleRequest, PaginatedResponse, Pagination, UpdateAlertRuleRequest, UpdateAlertStatusRequest,
};
use crate::repositories::AlertRepository;
use std::sync::Arc;
use uuid::Uuid;

/// 预警业务服务
pub struct AlertService {
    alert_repo: AlertRepository,
    notification_service: Option<Arc<dyn NotificationSender>>,
}

/// 通知发送器trait（用于依赖注入）
#[async_trait::async_trait]
pub trait NotificationSender: Send + Sync {
    async fn send_alert_notification(&self, alert_event: &AlertEvent, user_id: Uuid) -> Result<(), AppError>;
}

impl AlertService {
    pub fn new(alert_repo: AlertRepository) -> Self {
        Self { 
            alert_repo,
            notification_service: None,
        }
    }

    /// 设置通知服务（延迟注入，避免循环依赖）
    pub fn set_notification_service(&mut self, notification_service: Arc<dyn NotificationSender>) {
        self.notification_service = Some(notification_service);
    }

    /// 创建预警规则（用户独立）
    pub async fn create_rule(&self, user_id: Uuid, request: CreateAlertRuleRequest) -> Result<AlertRule, AppError> {
        self.alert_repo.create_rule(user_id, &request).await
    }

    /// 获取用户的所有启用规则
    pub async fn get_enabled_rules(&self, user_id: Uuid) -> Result<Vec<AlertRule>, AppError> {
        self.alert_repo.get_enabled_rules(user_id).await
    }

    /// 获取预警规则（仅限用户自己的规则）
    pub async fn get_rule(&self, rule_id: Uuid, user_id: Uuid) -> Result<AlertRule, AppError> {
        self.alert_repo
            .get_rule_by_id(rule_id, user_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("预警规则不存在或无权访问: {}", rule_id)))
    }

    /// 更新预警规则（仅限用户自己的规则）
    pub async fn update_rule(&self, rule_id: Uuid, user_id: Uuid, request: UpdateAlertRuleRequest) -> Result<AlertRule, AppError> {
        self.alert_repo.update_rule(rule_id, user_id, &request).await
    }

    /// 删除预警规则（仅限用户自己的规则）
    pub async fn delete_rule(&self, rule_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        self.alert_repo.delete_rule(rule_id, user_id).await
    }

    /// 触发低电量预警
    pub async fn trigger_low_battery(&self, device_id: Uuid, user_id: Uuid, level: f64, threshold: f64) -> Result<Option<AlertEvent>, AppError> {
        self.trigger_alert(
            device_id,
            user_id,
            AlertType::LowBattery,
            level,
            threshold,
            &format!("设备电量低: {}%", level as i32),
        )
        .await
    }

    /// 触发临界电量预警
    pub async fn trigger_critical_battery(
        &self,
        device_id: Uuid,
        user_id: Uuid,
        level: f64,
        threshold: f64,
    ) -> Result<Option<AlertEvent>, AppError> {
        self.trigger_alert(
            device_id,
            user_id,
            AlertType::CriticalBattery,
            level,
            threshold,
            &format!("设备电量临界: {}%", level as i32),
        )
        .await
    }

    /// 触发高温预警
    pub async fn trigger_high_temperature(
        &self,
        device_id: Uuid,
        user_id: Uuid,
        temperature: f64,
        threshold: f64,
    ) -> Result<Option<AlertEvent>, AppError> {
        self.trigger_alert(
            device_id,
            user_id,
            AlertType::HighTemperature,
            temperature,
            threshold,
            &format!("设备温度过高: {:.1}°C", temperature),
        )
        .await
    }

    /// 触发设备离线预警
    pub async fn trigger_device_offline(&self, device_id: Uuid, user_id: Uuid) -> Result<Option<AlertEvent>, AppError> {
        self.trigger_alert(
            device_id,
            user_id,
            AlertType::DeviceOffline,
            0.0,
            0.0,
            "设备已离线",
        )
        .await
    }

    /// 触发预警
    async fn trigger_alert(
        &self,
        device_id: Uuid,
        user_id: Uuid,
        alert_type: AlertType,
        value: f64,
        threshold: f64,
        message: &str,
    ) -> Result<Option<AlertEvent>, AppError> {
        // 获取对应的预警规则（用于级别和冷却时间）
        let rule = match self.alert_repo.get_rule_by_type(user_id, &alert_type).await? {
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

        // 创建预警事件（使用设备实际阈值）
        let event = self
            .alert_repo
            .create_event(device_id, &rule, value, threshold, message)
            .await?;

        tracing::info!(
            device_id = %device_id,
            alert_type = ?alert_type,
            level = ?rule.level,
            value = value,
            threshold = threshold,
            "触发预警"
        );

        // TODO: 发送通知（webhook、邮件等）
        // 发送通知
        if let Some(ref notification_service) = self.notification_service {
            // 获取设备所属用户ID
            if let Err(e) = notification_service.send_alert_notification(&event, user_id).await {
                tracing::error!(
                    error = %e,
                    alert_id = %event.id,
                    user_id = %user_id,
                    "通知发送失败"
                );
                // 通知发送失败不影响预警记录
            }
        }

        Ok(Some(event))
    }

    /// 更新预警状态（仅限用户设备的预警）
    pub async fn update_status(
        &self,
        event_id: Uuid,
        user_id: Uuid,
        request: UpdateAlertStatusRequest,
    ) -> Result<AlertEvent, AppError> {
        self.alert_repo.update_event_status(event_id, user_id, &request).await
    }

    /// 确认预警
    pub async fn acknowledge(&self, event_id: Uuid, user_id: Uuid) -> Result<AlertEvent, AppError> {
        self.update_status(
            event_id,
            user_id,
            UpdateAlertStatusRequest {
                status: AlertStatus::Acknowledged,
            },
        )
        .await
    }

    /// 解决预警
    pub async fn resolve(&self, event_id: Uuid, user_id: Uuid) -> Result<AlertEvent, AppError> {
        self.update_status(
            event_id,
            user_id,
            UpdateAlertStatusRequest {
                status: AlertStatus::Resolved,
            },
        )
        .await
    }

    /// 查询预警列表（仅限用户设备）
    pub async fn list(&self, user_id: Uuid, query: AlertListQuery) -> Result<PaginatedResponse<AlertEvent>, AppError> {
        let (events, total) = self.alert_repo.list_events(user_id, &query).await?;

        let pagination = Pagination::new(query.page, query.page_size, total);

        Ok(PaginatedResponse::new(events, pagination))
    }

    /// 获取设备活跃预警数
    pub async fn count_active(&self, device_id: Uuid) -> Result<i64, AppError> {
        self.alert_repo.count_active_alerts(device_id).await
    }
}
