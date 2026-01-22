//! 通知服务模块
//! 
//! 提供统一的通知接口，支持多种通知渠道（邮件、Webhook等）

use crate::errors::AppError;
use crate::models::{
    AlertEvent, AlertLevel, EmailNotificationConfig, NotificationChannel,
    SubscribeWebPushRequest, UpdateNotificationPreferenceRequest, UserNotificationPreference, 
    WebhookNotificationConfig, WebPushNotificationConfig, WebPushSubscription,
};
use crate::repositories::{DeviceRepository, NotificationRepository};
use crate::services::alert_service::NotificationSender;
use crate::services::{EmailService, WebPushService};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

/// 通知服务
pub struct NotificationService {
    notification_repo: NotificationRepository,
    device_repo: DeviceRepository,
    email_service: Arc<EmailService>,
    web_push_service: Option<Arc<WebPushService>>,
}

#[async_trait::async_trait]
impl NotificationSender for NotificationService {
    async fn send_alert_notification(&self, alert_event: &AlertEvent, user_id: Uuid) -> Result<(), AppError> {
        self.send_alert_notification(alert_event, user_id).await
    }
}

impl NotificationService {
    pub fn new(
        notification_repo: NotificationRepository,
        device_repo: DeviceRepository,
        email_service: Arc<EmailService>,
    ) -> Self {
        Self {
            notification_repo,
            device_repo,
            email_service,
            web_push_service: None,
        }
    }

    /// 设置 Web Push 服务（可选，需要配置 VAPID 密钥）
    pub fn set_web_push_service(&mut self, web_push_service: Arc<WebPushService>) {
        self.web_push_service = Some(web_push_service);
    }

    // ========== 通知偏好管理 ==========

    /// 获取用户的通知偏好
    pub async fn get_user_preference(&self, user_id: Uuid) -> Result<Option<UserNotificationPreference>, AppError> {
        self.notification_repo.get_user_preference(user_id).await
    }

    /// 更新用户的通知偏好
    pub async fn update_user_preference(
        &self,
        user_id: Uuid,
        request: UpdateNotificationPreferenceRequest,
    ) -> Result<UserNotificationPreference, AppError> {
        // 验证邮箱配置
        if let Some(ref email_config) = request.email_config {
            if email_config.enabled && !self.email_service.is_enabled() {
                return Err(AppError::ConfigError("邮件服务未启用".to_string()));
            }
        }

        self.notification_repo
            .upsert_user_preference(user_id, &request)
            .await
    }

    // ========== Web Push 订阅管理 ==========

    /// 订阅 Web Push
    pub async fn subscribe_web_push(
        &self,
        user_id: Uuid,
        request: SubscribeWebPushRequest,
        user_agent: Option<&str>,
    ) -> Result<WebPushSubscription, AppError> {
        self.notification_repo
            .upsert_web_push_subscription(user_id, &request, user_agent)
            .await
    }

    /// 获取用户的 Web Push 订阅列表
    pub async fn get_web_push_subscriptions(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<WebPushSubscription>, AppError> {
        self.notification_repo
            .get_active_web_push_subscriptions(user_id)
            .await
    }

    /// 删除 Web Push 订阅
    pub async fn delete_web_push_subscription(
        &self,
        user_id: Uuid,
        subscription_id: Uuid,
    ) -> Result<(), AppError> {
        self.notification_repo
            .delete_web_push_subscription(user_id, subscription_id)
            .await
    }

    // ========== 预警通知发送 ==========

    /// 发送预警通知（根据用户偏好选择渠道）
    pub async fn send_alert_notification(
        &self,
        alert_event: &AlertEvent,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        // 获取用户通知偏好
        let preference = match self.notification_repo.get_user_preference(user_id).await? {
            Some(pref) => pref,
            None => {
                tracing::warn!(user_id = %user_id, "用户未配置通知偏好");
                return Ok(());
            }
        };

        // 检查是否启用通知
        if !preference.enabled {
            tracing::debug!(user_id = %user_id, "用户已禁用通知");
            return Ok(());
        }

        // 检查预警级别是否需要通知
        if !self.should_notify_for_level(&preference, &alert_event.level) {
            tracing::debug!(
                user_id = %user_id,
                level = ?alert_event.level,
                "预警级别不需要通知"
            );
            return Ok(());
        }

        // 检查是否在安静时段
        if self.is_in_quiet_hours(&preference) {
            tracing::debug!(user_id = %user_id, "当前处于安静时段");
            return Ok(());
        }

        // 获取设备信息
        let device = self.device_repo
            .find_by_id(alert_event.device_id)
            .await?
            .ok_or_else(|| AppError::NotFound("设备不存在".to_string()))?;

        // 发送各渠道通知
        let mut sent_any = false;

        // 1. 邮件通知
        if let Err(e) = self.send_email_notification(&preference, alert_event, &device.name).await {
            tracing::error!(
                error = %e,
                user_id = %user_id,
                alert_id = %alert_event.id,
                "邮件通知发送失败"
            );
        } else {
            sent_any = true;
        }

        // 2. Webhook 通知（预留）
        if let Err(e) = self.send_webhook_notification(&preference, alert_event, &device.name).await {
            tracing::error!(
                error = %e,
                user_id = %user_id,
                alert_id = %alert_event.id,
                "Webhook通知发送失败"
            );
        } else if self.is_webhook_enabled(&preference) {
            sent_any = true;
        }

        // 3. Web Push 通知
        if let Err(e) = self.send_web_push_notification(&preference, alert_event, &device.name).await {
            tracing::error!(
                error = %e,
                user_id = %user_id,
                alert_id = %alert_event.id,
                "Web Push 通知发送失败"
            );
        } else if self.is_web_push_enabled(&preference) {
            sent_any = true;
        }

        if sent_any {
            tracing::info!(
                user_id = %user_id,
                alert_id = %alert_event.id,
                "预警通知已发送"
            );
        }

        Ok(())
    }

    /// 发送邮件通知
    async fn send_email_notification(
        &self,
        preference: &UserNotificationPreference,
        alert_event: &AlertEvent,
        device_name: &str,
    ) -> Result<(), AppError> {
        // 解析邮件配置
        let email_config: EmailNotificationConfig = match &preference.email_config {
            Some(config) => serde_json::from_value(config.clone())
                .map_err(|e| AppError::InternalError(format!("邮件配置解析失败: {}", e)))?,
            None => return Ok(()),
        };

        if !email_config.enabled {
            return Ok(());
        }

        // 检查频率限制
        if let Some(last_time) = self.notification_repo
            .get_last_notification_time(preference.user_id, NotificationChannel::Email)
            .await?
        {
            let elapsed = Utc::now().signed_duration_since(last_time);
            if elapsed.num_minutes() < preference.min_notification_interval as i64 {
                tracing::debug!(
                    user_id = %preference.user_id,
                    "邮件通知频率限制中"
                );
                
                // 记录跳过
                self.notification_repo
                    .create_notification_history(
                        alert_event.id,
                        preference.user_id,
                        NotificationChannel::Email,
                        &email_config.email,
                        "skipped",
                        Some("频率限制"),
                    )
                    .await?;
                
                return Ok(());
            }
        }

        // 创建待发送记录
        let history = self.notification_repo
            .create_notification_history(
                alert_event.id,
                preference.user_id,
                NotificationChannel::Email,
                &email_config.email,
                "pending",
                None,
            )
            .await?;

        // 发送邮件
        let params = crate::services::email_service::AlertNotificationParams {
            to_email: &email_config.email,
            alert_type: &format!("{:?}", alert_event.alert_type),
            level: &format!("{:?}", alert_event.level),
            message: &alert_event.message,
            device_name,
            value: alert_event.value,
            threshold: alert_event.threshold,
            triggered_at: &alert_event.triggered_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        };
        let result = self.email_service
            .send_alert_notification(params)
            .await;

        // 更新发送状态
        match result {
            Ok(_) => {
                self.notification_repo
                    .update_notification_status(history.id, "sent", None)
                    .await?;
            }
            Err(e) => {
                self.notification_repo
                    .update_notification_status(history.id, "failed", Some(&e.to_string()))
                    .await?;
                return Err(e);
            }
        }

        Ok(())
    }

    /// 发送Webhook通知（预留扩展）
    async fn send_webhook_notification(
        &self,
        preference: &UserNotificationPreference,
        alert_event: &AlertEvent,
        device_name: &str,
    ) -> Result<(), AppError> {
        // 解析Webhook配置
        let webhook_config: WebhookNotificationConfig = match &preference.webhook_config {
            Some(config) => serde_json::from_value(config.clone())
                .map_err(|e| AppError::InternalError(format!("Webhook配置解析失败: {}", e)))?,
            None => return Ok(()),
        };

        if !webhook_config.enabled {
            return Ok(());
        }

        // 检查频率限制
        if let Some(last_time) = self.notification_repo
            .get_last_notification_time(preference.user_id, NotificationChannel::Webhook)
            .await?
        {
            let elapsed = Utc::now().signed_duration_since(last_time);
            if elapsed.num_minutes() < preference.min_notification_interval as i64 {
                self.notification_repo
                    .create_notification_history(
                        alert_event.id,
                        preference.user_id,
                        NotificationChannel::Webhook,
                        &webhook_config.url,
                        "skipped",
                        Some("频率限制"),
                    )
                    .await?;
                return Ok(());
            }
        }

        // 构建Webhook负载
        let _payload = serde_json::json!({
            "alert_id": alert_event.id,
            "device_name": device_name,
            "alert_type": alert_event.alert_type,
            "level": alert_event.level,
            "message": alert_event.message,
            "value": alert_event.value,
            "threshold": alert_event.threshold,
            "triggered_at": alert_event.triggered_at,
        });

        // 这里可以实现实际的HTTP请求发送
        // 目前记录为待实现

        // 记录通知历史
        self.notification_repo
            .create_notification_history(
                alert_event.id,
                preference.user_id,
                NotificationChannel::Webhook,
                &webhook_config.url,
                "sent",
                None,
            )
            .await?;

        Ok(())
    }

    /// 发送 Web Push 通知
    async fn send_web_push_notification(
        &self,
        preference: &UserNotificationPreference,
        alert_event: &AlertEvent,
        device_name: &str,
    ) -> Result<(), AppError> {
        // 检查 Web Push 服务是否可用
        let web_push_service = match &self.web_push_service {
            Some(service) => service,
            None => return Ok(()), // 未配置 Web Push 服务
        };

        // 解析配置
        let web_push_config: WebPushNotificationConfig = match &preference.web_push_config {
            Some(config) => serde_json::from_value(config.clone())
                .map_err(|e| AppError::InternalError(format!("Web Push 配置解析失败: {}", e)))?,
            None => return Ok(()),
        };

        if !web_push_config.enabled {
            return Ok(());
        }

        // 检查频率限制
        if let Some(last_time) = self.notification_repo
            .get_last_notification_time(preference.user_id, NotificationChannel::Push)
            .await?
        {
            let elapsed = Utc::now().signed_duration_since(last_time);
            if elapsed.num_minutes() < preference.min_notification_interval as i64 {
                tracing::debug!(
                    user_id = %preference.user_id,
                    "Web Push 通知频率限制中"
                );
                
                // 记录跳过
                self.notification_repo
                    .create_notification_history(
                        alert_event.id,
                        preference.user_id,
                        NotificationChannel::Push,
                        "web_push",
                        "skipped",
                        Some("频率限制"),
                    )
                    .await?;
                
                return Ok(());
            }
        }

        // 创建待发送记录
        let history = self.notification_repo
            .create_notification_history(
                alert_event.id,
                preference.user_id,
                NotificationChannel::Push,
                "web_push",
                "pending",
                None,
            )
            .await?;

        // 构建通知内容
        let title = format!("{:?} - {:?}", alert_event.level, alert_event.alert_type);
        let body = format!("{} | {}", device_name, alert_event.message);
        let data = Some(serde_json::json!({
            "alert_id": alert_event.id,
            "device_id": alert_event.device_id,
            "alert_type": alert_event.alert_type,
            "level": alert_event.level,
        }));

        // 发送到用户的所有订阅
        let result = web_push_service
            .send_to_user(preference.user_id, &title, &body, data)
            .await;

        // 更新发送状态
        match result {
            Ok(count) if count > 0 => {
                self.notification_repo
                    .update_notification_status(history.id, "sent", None)
                    .await?;
            }
            Ok(_) => {
                self.notification_repo
                    .update_notification_status(history.id, "skipped", Some("无活跃订阅"))
                    .await?;
            }
            Err(e) => {
                self.notification_repo
                    .update_notification_status(history.id, "failed", Some(&e.to_string()))
                    .await?;
                return Err(e);
            }
        }

        Ok(())
    }

    // ========== Webhook 通知（待实现）==========
    /*
    async fn send_webhook_notification(
        &self,
        preference: &UserNotificationPreference,
        alert_event: &AlertEvent,
        device_name: &str,
    ) -> Result<(), AppError> {
        let webhook_config: WebhookNotificationConfig = match &preference.webhook_config {
            Some(v) => serde_json::from_value(v.clone())
                .map_err(|_| AppError::ConfigError("Webhook配置无效".to_string()))?,
            None => return Ok(()),
        };

        if !webhook_config.enabled {
            return Ok(());
        }

        // TODO: 实现 Webhook 发送逻辑
        tracing::info!(
            webhook_url = %webhook_config.url,
            "Webhook通知已准备（实现待补充）"
        );

        Ok(())
    }
    */

    // ========== 辅助方法 ==========

    /// 检查预警级别是否需要通知
    fn should_notify_for_level(&self, preference: &UserNotificationPreference, level: &AlertLevel) -> bool {
        match level {
            AlertLevel::Info => preference.notify_info,
            AlertLevel::Warning => preference.notify_warning,
            AlertLevel::Critical => preference.notify_critical,
        }
    }

    /// 检查是否在安静时段
    fn is_in_quiet_hours(&self, preference: &UserNotificationPreference) -> bool {
        let (start, end) = match (&preference.quiet_hours_start, &preference.quiet_hours_end) {
            (Some(s), Some(e)) => (s, e),
            _ => return false,
        };

        // 获取用户时区的当前时间
        let tz: chrono_tz::Tz = preference.quiet_hours_timezone
            .parse()
            .unwrap_or(chrono_tz::UTC);
        
        let now = Utc::now().with_timezone(&tz).time();

        // 处理跨午夜的情况
        if start < end {
            now >= *start && now < *end
        } else {
            now >= *start || now < *end
        }
    }

    /// 检查Webhook是否启用
    fn is_webhook_enabled(&self, preference: &UserNotificationPreference) -> bool {
        preference.webhook_config
            .as_ref()
            .and_then(|v| serde_json::from_value::<WebhookNotificationConfig>(v.clone()).ok())
            .is_some_and(|c| c.enabled)
    }

    /// 检查Web Push是否启用
    fn is_web_push_enabled(&self, preference: &UserNotificationPreference) -> bool {
        preference.web_push_config
            .as_ref()
            .and_then(|v| serde_json::from_value::<WebPushNotificationConfig>(v.clone()).ok())
            .is_some_and(|c| c.enabled)
    }
}
