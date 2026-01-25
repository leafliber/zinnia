//! 设备数据模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

/// 设备状态枚举
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "device_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DeviceStatus {
    Online,
    #[default]
    Offline,
    Maintenance,
    Disabled,
}

/// 设备实体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Device {
    pub id: Uuid,
    /// 设备所有者（用户 ID）
    pub owner_id: Option<Uuid>,
    pub name: String,
    pub device_type: String,
    pub status: DeviceStatus,
    /// API Key 哈希值（不返回给客户端）
    #[serde(skip_serializing)]
    pub api_key_hash: String,
    /// API Key 前缀（用于识别）
    pub api_key_prefix: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// 设备配置
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeviceConfig {
    pub device_id: Uuid,
    pub low_battery_threshold: i32,
    pub critical_battery_threshold: i32,
    pub report_interval_seconds: i32,
    pub high_temperature_threshold: f64,
    pub updated_at: DateTime<Utc>,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            device_id: Uuid::nil(),
            low_battery_threshold: 20,
            critical_battery_threshold: 10,
            report_interval_seconds: 60,
            high_temperature_threshold: 45.0,
            updated_at: Utc::now(),
        }
    }
}

/// 创建设备请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateDeviceRequest {
    #[validate(length(min = 1, max = 100, message = "设备名称长度应在 1-100 字符之间"))]
    pub name: String,

    #[validate(length(min = 1, max = 50, message = "设备类型长度应在 1-50 字符之间"))]
    pub device_type: String,

    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// 创建设备响应（包含一次性 API Key）
#[derive(Debug, Serialize)]
pub struct CreateDeviceResponse {
    pub device: Device,
    /// API Key 仅在创建时返回一次，请妥善保管
    pub api_key: String,
    pub config: DeviceConfig,
}

/// 更新设备请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateDeviceRequest {
    #[validate(length(min = 1, max = 100, message = "设备名称长度应在 1-100 字符之间"))]
    pub name: Option<String>,

    pub status: Option<DeviceStatus>,

    pub metadata: Option<serde_json::Value>,
}

/// 更新设备配置请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateDeviceConfigRequest {
    #[validate(range(min = 1, max = 100, message = "低电量阈值应在 1-100 之间"))]
    pub low_battery_threshold: Option<i32>,

    #[validate(range(min = 1, max = 100, message = "临界电量阈值应在 1-100 之间"))]
    pub critical_battery_threshold: Option<i32>,

    #[validate(range(min = 10, max = 3600, message = "上报间隔应在 10-3600 秒之间"))]
    pub report_interval_seconds: Option<i32>,

    #[validate(range(min = -40.0, max = 200.0, message = "温度阈值应在 -40 到 200 摄氏度之间"))]
    pub high_temperature_threshold: Option<f64>,
}

/// 设备列表查询参数
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct DeviceListQuery {
    #[validate(range(min = 1, max = 100, message = "每页数量应在 1-100 之间"))]
    #[serde(default = "default_page_size")]
    pub page_size: i64,

    #[validate(range(min = 1, message = "页码应大于 0"))]
    #[serde(default = "default_page")]
    pub page: i64,

    pub status: Option<DeviceStatus>,
    pub device_type: Option<String>,

    /// 按所有者筛选（用于用户查看自己的设备）
    #[serde(skip)]
    pub owner_id: Option<Uuid>,

    /// 包含共享给用户的设备
    #[serde(skip)]
    pub include_shared: bool,
}

fn default_page_size() -> i64 {
    20
}
fn default_page() -> i64 {
    1
}
