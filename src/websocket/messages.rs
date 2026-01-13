//! WebSocket 消息类型定义
//!
//! 定义客户端和服务器之间的消息协议

use crate::models::{BatteryData, LatestBatteryResponse, PowerSavingMode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 客户端发送的消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// 认证消息
    Auth(AuthMessage),
    
    /// 电量上报
    BatteryReport(BatteryReportMessage),
    
    /// 批量电量上报
    BatchBatteryReport(BatchBatteryReportMessage),
    
    /// 心跳
    Ping,
    
    /// 订阅设备数据推送（用户端）
    Subscribe(SubscribeMessage),
    
    /// 取消订阅
    Unsubscribe(UnsubscribeMessage),
}

/// 服务器发送的消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// 认证结果
    AuthResult(AuthResultMessage),
    
    /// 电量上报结果
    BatteryReportResult(BatteryReportResultMessage),
    
    /// 批量上报结果
    BatchBatteryReportResult(BatchReportResultMessage),
    
    /// 心跳响应
    Pong,
    
    /// 订阅结果
    SubscribeResult(SubscribeResultMessage),
    
    /// 推送的电量数据（用户订阅后收到）
    BatteryPush(BatteryPushMessage),
    
    /// 预警推送
    AlertPush(AlertPushMessage),
    
    /// 错误消息
    Error(ErrorMessage),
    
    /// 连接成功消息
    Connected(ConnectedMessage),
}

/// 认证消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMessage {
    /// 认证令牌（设备访问令牌或 JWT）
    pub token: String,
    
    /// 认证类型
    #[serde(default)]
    pub auth_type: AuthType,
}

/// 认证类型
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    /// 设备访问令牌
    #[default]
    DeviceToken,
    
    /// JWT（用户令牌）
    Jwt,
}

/// 认证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResultMessage {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Uuid>,
}

/// 电量上报消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryReportMessage {
    /// 电量值 (0-100)
    pub battery_level: i32,
    
    /// 是否正在充电
    #[serde(default)]
    pub is_charging: bool,
    
    /// 省电模式
    #[serde(default)]
    pub power_saving_mode: PowerSavingMode,
    
    /// 温度（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    
    /// 电压（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voltage: Option<f64>,
    
    /// 设备端记录时间（可选，默认服务器时间）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<DateTime<Utc>>,
    
    /// 消息 ID（可选，用于追踪请求响应）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<String>,
}

/// 批量电量上报消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchBatteryReportMessage {
    /// 批量数据
    pub data: Vec<BatteryReportMessage>,
    
    /// 消息 ID（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<String>,
}

/// 电量上报结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryReportResultMessage {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<BatteryData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<String>,
}

/// 批量上报结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchReportResultMessage {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inserted_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<String>,
}

/// 订阅消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeMessage {
    /// 要订阅的设备 ID 列表
    pub device_ids: Vec<Uuid>,
}

/// 取消订阅消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeMessage {
    /// 要取消订阅的设备 ID 列表（为空则取消所有订阅）
    #[serde(default)]
    pub device_ids: Vec<Uuid>,
}

/// 订阅结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeResultMessage {
    pub success: bool,
    pub subscribed_devices: Vec<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 电量数据推送
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryPushMessage {
    pub device_id: Uuid,
    pub data: LatestBatteryResponse,
}

/// 预警推送
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertPushMessage {
    pub device_id: Uuid,
    pub alert_type: String,
    pub message: String,
    pub severity: String,
    pub timestamp: DateTime<Utc>,
}

/// 错误消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub code: String,
    pub message: String,
}

/// 连接成功消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedMessage {
    pub message: String,
    pub server_time: DateTime<Utc>,
    /// 需要在此时间内完成认证（秒）
    pub auth_timeout: u64,
}

impl ServerMessage {
    /// 创建错误消息
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        ServerMessage::Error(ErrorMessage {
            code: code.into(),
            message: message.into(),
        })
    }
    
    /// 创建认证成功消息
    pub fn auth_success(device_id: Option<Uuid>, user_id: Option<Uuid>) -> Self {
        ServerMessage::AuthResult(AuthResultMessage {
            success: true,
            message: "认证成功".to_string(),
            device_id,
            user_id,
        })
    }
    
    /// 创建认证失败消息
    pub fn auth_failed(message: impl Into<String>) -> Self {
        ServerMessage::AuthResult(AuthResultMessage {
            success: false,
            message: message.into(),
            device_id: None,
            user_id: None,
        })
    }
    
    /// 创建电量上报成功消息
    pub fn battery_report_success(data: BatteryData, msg_id: Option<String>) -> Self {
        ServerMessage::BatteryReportResult(BatteryReportResultMessage {
            success: true,
            data: Some(data),
            error: None,
            msg_id,
        })
    }
    
    /// 创建电量上报失败消息
    pub fn battery_report_failed(error: impl Into<String>, msg_id: Option<String>) -> Self {
        ServerMessage::BatteryReportResult(BatteryReportResultMessage {
            success: false,
            data: None,
            error: Some(error.into()),
            msg_id,
        })
    }
}
