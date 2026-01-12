//! 电量数据模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

/// 省电模式枚举
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "power_saving_mode", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PowerSavingMode {
    Off,
    Low,
    Medium,
    High,
    Extreme,
}

impl Default for PowerSavingMode {
    fn default() -> Self {
        PowerSavingMode::Off
    }
}

/// 电量数据点
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BatteryData {
    pub id: Uuid,
    pub device_id: Uuid,
    pub battery_level: i32,
    pub is_charging: bool,
    pub power_saving_mode: PowerSavingMode,
    pub temperature: Option<f64>,
    pub voltage: Option<f64>,
    pub recorded_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// 电量上报请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct BatteryReportRequest {
    #[validate(range(min = 0, max = 100, message = "电量值应在 0-100 之间"))]
    pub battery_level: i32,
    
    #[serde(default)]
    pub is_charging: bool,
    
    #[serde(default)]
    pub power_saving_mode: PowerSavingMode,
    
    #[validate(range(min = -40.0, max = 85.0, message = "温度值应在 -40 到 85 摄氏度之间"))]
    pub temperature: Option<f64>,
    
    #[validate(range(min = 0.0, max = 10.0, message = "电压值应在 0-10V 之间"))]
    pub voltage: Option<f64>,
    
    /// 设备端记录时间（可选，默认使用服务器时间）
    pub recorded_at: Option<DateTime<Utc>>,
}

/// 批量上报请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct BatchBatteryReportRequest {
    #[validate(length(min = 1, max = 1000, message = "批量上报数据条数应在 1-1000 之间"))]
    #[validate]
    pub data: Vec<BatteryReportRequest>,
}

/// 电量查询请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct BatteryQueryRequest {
    /// 开始时间
    pub start_time: DateTime<Utc>,
    
    /// 结束时间
    pub end_time: DateTime<Utc>,
    
    #[validate(range(min = 1, max = 1000, message = "每页数量应在 1-1000 之间"))]
    #[serde(default = "default_limit")]
    pub limit: i64,
    
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 { 100 }

impl BatteryQueryRequest {
    /// 验证时间范围（最大 30 天）
    pub fn validate_time_range(&self) -> Result<(), String> {
        let duration = self.end_time - self.start_time;
        let max_days = 30;
        
        if duration.num_days() > max_days {
            return Err(format!("查询时间范围不能超过 {} 天", max_days));
        }
        
        if self.start_time > self.end_time {
            return Err("开始时间不能晚于结束时间".to_string());
        }
        
        if self.end_time > Utc::now() {
            return Err("结束时间不能是未来时间".to_string());
        }
        
        Ok(())
    }
}

/// 最新电量响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestBatteryResponse {
    pub device_id: Uuid,
    pub battery_level: i32,
    pub is_charging: bool,
    pub power_saving_mode: PowerSavingMode,
    pub recorded_at: DateTime<Utc>,
    pub is_low_battery: bool,
    pub is_critical: bool,
}

/// 电量统计响应
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BatteryStatsResponse {
    pub device_id: Uuid,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub avg_battery_level: f64,
    pub min_battery_level: i32,
    pub max_battery_level: i32,
    pub total_records: i64,
    pub charging_duration_minutes: i64,
    pub low_battery_count: i64,
}

/// 时间聚合间隔
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AggregateInterval {
    Minute,
    Hour,
    Day,
}

impl AggregateInterval {
    pub fn to_timescaledb_interval(&self) -> &'static str {
        match self {
            AggregateInterval::Minute => "1 minute",
            AggregateInterval::Hour => "1 hour",
            AggregateInterval::Day => "1 day",
        }
    }
}

/// 聚合查询请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct BatteryAggregateRequest {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    
    #[serde(default = "default_interval")]
    pub interval: AggregateInterval,
}

fn default_interval() -> AggregateInterval { AggregateInterval::Hour }

/// 聚合数据点
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BatteryAggregatePoint {
    pub bucket: DateTime<Utc>,
    pub avg_level: f64,
    pub min_level: i32,
    pub max_level: i32,
    pub count: i64,
}
