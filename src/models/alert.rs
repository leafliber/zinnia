//! 预警模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

/// 预警级别
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "alert_level", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

/// 预警状态
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "alert_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AlertStatus {
    Active,
    Acknowledged,
    Resolved,
}

/// 预警类型
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "alert_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AlertType {
    LowBattery,
    CriticalBattery,
    HighTemperature,
    DeviceOffline,
    RapidDrain,
}

/// 预警规则
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AlertRule {
    pub id: Uuid,
    pub name: String,
    pub alert_type: AlertType,
    pub level: AlertLevel,
    pub threshold_value: f64,
    pub cooldown_minutes: i32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 预警事件
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AlertEvent {
    pub id: Uuid,
    pub device_id: Uuid,
    pub rule_id: Uuid,
    pub alert_type: AlertType,
    pub level: AlertLevel,
    pub status: AlertStatus,
    pub message: String,
    pub value: f64,
    pub threshold: f64,
    pub triggered_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
}

/// 创建预警规则请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateAlertRuleRequest {
    #[validate(length(min = 1, max = 100, message = "规则名称长度应在 1-100 字符之间"))]
    pub name: String,
    
    pub alert_type: AlertType,
    pub level: AlertLevel,
    pub threshold_value: f64,
    
    #[validate(range(min = 1, max = 1440, message = "冷却时间应在 1-1440 分钟之间"))]
    #[serde(default = "default_cooldown")]
    pub cooldown_minutes: i32,
    
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_cooldown() -> i32 { 30 }
fn default_enabled() -> bool { true }

/// 更新预警状态请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAlertStatusRequest {
    pub status: AlertStatus,
}

/// 预警列表查询参数
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct AlertListQuery {
    pub device_id: Option<Uuid>,
    pub level: Option<AlertLevel>,
    pub status: Option<AlertStatus>,
    pub alert_type: Option<AlertType>,
    
    #[validate(range(min = 1, max = 100, message = "每页数量应在 1-100 之间"))]
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    
    #[validate(range(min = 1, message = "页码应大于 0"))]
    #[serde(default = "default_page")]
    pub page: i64,
}

fn default_page_size() -> i64 { 20 }
fn default_page() -> i64 { 1 }
