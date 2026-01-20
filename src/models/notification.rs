//! 通知模型

use chrono::{DateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

/// 通知渠道
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "notification_channel", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum NotificationChannel {
    Email,
    Webhook,
    Sms,
    Push,
}

impl std::fmt::Display for NotificationChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationChannel::Email => write!(f, "email"),
            NotificationChannel::Webhook => write!(f, "webhook"),
            NotificationChannel::Sms => write!(f, "sms"),
            NotificationChannel::Push => write!(f, "push"),
        }
    }
}

/// 邮件通知配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailNotificationConfig {
    pub enabled: bool,
    pub email: String,
}

/// Webhook 通知配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookNotificationConfig {
    pub enabled: bool,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

/// Web Push 通知配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebPushNotificationConfig {
    pub enabled: bool,
}

/// Web Push 订阅信息（来自浏览器 PushSubscription）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WebPushSubscription {
    pub id: Uuid,
    pub user_id: Uuid,
    
    /// 推送端点 URL
    pub endpoint: String,
    pub web_push_config: Option<serde_json::Value>,
    /// P-256 ECDH 公钥 (Base64)
    pub p256dh_key: String,
    /// 认证密钥 (Base64)
    pub auth_secret: String,
    
    /// 设备信息
    pub user_agent: Option<String>,
    pub device_name: Option<String>,
    
    /// 状态
    pub is_active: bool,
    
    /// 时间戳
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Web Push 订阅请求（来自前端）
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SubscribeWebPushRequest {
    /// 订阅端点
    #[validate(url(message = "端点 URL 格式无效"))]
    pub endpoint: String,
    
    /// P-256 ECDH 公钥 (Base64)
    #[validate(length(min = 1, message = "公钥不能为空"))]
    pub p256dh_key: String,
    
    /// 认证密钥 (Base64)
    #[validate(length(min = 1, message = "认证密钥不能为空"))]
    pub auth_secret: String,
    
    /// 设备名称（可选）
    pub device_name: Option<String>,
}

/// Web Push 订阅响应
#[derive(Debug, Clone, Serialize)]
pub struct WebPushSubscriptionResponse {
    pub id: Uuid,
    pub endpoint: String,
    pub device_name: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// 用户通知偏好
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserNotificationPreference {
    pub id: Uuid,
    pub user_id: Uuid,
    
    /// 全局通知开关
    pub enabled: bool,
    
    /// 各渠道配置
    pub email_config: Option<serde_json::Value>,
    pub webhook_config: Option<serde_json::Value>,
    pub sms_config: Option<serde_json::Value>,
    pub push_config: Option<serde_json::Value>,
    pub web_push_config: Option<serde_json::Value>,
    
    /// 预警级别过滤
    pub notify_info: bool,
    pub notify_warning: bool,
    pub notify_critical: bool,
    
    /// 安静时段
    pub quiet_hours_start: Option<NaiveTime>,
    pub quiet_hours_end: Option<NaiveTime>,
    pub quiet_hours_timezone: String,
    
    /// 通知频率控制（分钟）
    pub min_notification_interval: i32,
    
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 通知历史记录
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NotificationHistory {
    pub id: Uuid,
    pub alert_event_id: Uuid,
    pub user_id: Uuid,
    
    pub channel: NotificationChannel,
    pub recipient: String,
    
    pub status: String,  // 'pending', 'sent', 'failed', 'skipped'
    pub error_message: Option<String>,
    
    pub sent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// 创建/更新通知偏好请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateNotificationPreferenceRequest {
    /// 全局通知开关
    pub enabled: Option<bool>,
    
    /// 邮件通知配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_config: Option<EmailNotificationConfig>,
    
    /// Webhook通知配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_config: Option<WebhookNotificationConfig>,
    
    /// Web Push通知配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_push_config: Option<WebPushNotificationConfig>,
    
    /// 预警级别过滤
    pub notify_info: Option<bool>,
    pub notify_warning: Option<bool>,
    pub notify_critical: Option<bool>,
    
    /// 安静时段
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiet_hours_start: Option<String>,  // "HH:MM" 格式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiet_hours_end: Option<String>,    // "HH:MM" 格式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiet_hours_timezone: Option<String>,
    
    /// 通知频率控制（分钟）
    #[validate(range(min = 1, max = 1440, message = "通知间隔应在 1-1440 分钟之间"))]
    pub min_notification_interval: Option<i32>,
}

/// 通知偏好响应
#[derive(Debug, Clone, Serialize)]
pub struct NotificationPreferenceResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub enabled: bool,
    
    pub email_enabled: bool,
    pub web_push_enabled: bool,
    pub web_push_subscriptions_count: usize,
    
    pub email_address: Option<String>,
    
    pub webhook_enabled: bool,
    pub webhook_url: Option<String>,
    
    pub notify_info: bool,
    pub notify_warning: bool,
    pub notify_critical: bool,
    
    pub quiet_hours_start: Option<String>,
    pub quiet_hours_end: Option<String>,
    pub quiet_hours_timezone: String,
    
    pub min_notification_interval: i32,
    
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl NotificationPreferenceResponse {
    pub fn from(pref: UserNotificationPreference) -> Self {
        let email_config: Option<EmailNotificationConfig> = pref.email_config
            .and_then(|v| serde_json::from_value(v).ok());
        
        let webhook_config: Option<WebhookNotificationConfig> = pref.webhook_config
            .and_then(|v| serde_json::from_value(v).ok());
        
        let web_push_config: Option<WebPushNotificationConfig> = pref.web_push_config
            .and_then(|v| serde_json::from_value(v).ok());
        
        Self {
            id: pref.id,
            user_id: pref.user_id,
            enabled: pref.enabled,
            
            email_enabled: email_config.as_ref().map_or(false, |c| c.enabled),
            email_address: email_config.map(|c| c.email),
            
            webhook_enabled: webhook_config.as_ref().map_or(false, |c| c.enabled),
            webhook_url: webhook_config.map(|c| c.url),
            
            web_push_enabled: web_push_config.as_ref().map_or(false, |c| c.enabled),
            web_push_subscriptions_count: 0,  // 需要单独查询
            
            notify_info: pref.notify_info,
            notify_warning: pref.notify_warning,
            notify_critical: pref.notify_critical,
            
            quiet_hours_start: pref.quiet_hours_start.map(|t| t.to_string()),
            quiet_hours_end: pref.quiet_hours_end.map(|t| t.to_string()),
            quiet_hours_timezone: pref.quiet_hours_timezone,
            
            min_notification_interval: pref.min_notification_interval,
            
            created_at: pref.created_at,
            updated_at: pref.updated_at,
        }
    }
}
