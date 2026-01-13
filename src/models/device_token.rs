//! 设备访问令牌模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

/// 令牌权限枚举
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "token_permission", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TokenPermission {
    Read,   // 只读：只能查询数据
    #[default]
    Write,  // 只写：只能上报数据
    All,    // 全部：读写都可以
}

impl std::fmt::Display for TokenPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenPermission::Read => write!(f, "read"),
            TokenPermission::Write => write!(f, "write"),
            TokenPermission::All => write!(f, "all"),
        }
    }
}

/// 设备访问令牌实体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeviceAccessToken {
    pub id: Uuid,
    pub device_id: Uuid,
    pub created_by: Uuid,
    
    /// 令牌哈希值（不返回给客户端）
    #[serde(skip_serializing)]
    pub token_hash: String,
    
    /// 令牌前缀（用于显示）
    pub token_prefix: String,
    
    /// 令牌名称
    pub name: String,
    
    /// 权限
    pub permission: TokenPermission,
    
    /// 过期时间（None 表示永不过期）
    pub expires_at: Option<DateTime<Utc>>,
    
    /// 最后使用时间
    pub last_used_at: Option<DateTime<Utc>>,
    
    /// 使用次数
    pub use_count: i32,
    
    /// 是否已吊销
    pub is_revoked: bool,
    
    /// 吊销时间
    pub revoked_at: Option<DateTime<Utc>>,
    
    /// 允许的 IP 白名单
    pub allowed_ips: Option<Vec<String>>,
    
    /// 每分钟请求限制
    pub rate_limit_per_minute: Option<i32>,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl DeviceAccessToken {
    /// 检查令牌是否有效
    pub fn is_valid(&self) -> bool {
        if self.is_revoked {
            return false;
        }
        
        if let Some(expires_at) = self.expires_at {
            if expires_at < Utc::now() {
                return false;
            }
        }
        
        true
    }
    
    /// 检查 IP 是否在白名单中
    pub fn is_ip_allowed(&self, ip: &str) -> bool {
        match &self.allowed_ips {
            None => true, // 没有限制
            Some(ips) if ips.is_empty() => true,
            Some(ips) => ips.iter().any(|allowed| allowed == ip),
        }
    }
    
    /// 检查权限是否允许读取
    pub fn can_read(&self) -> bool {
        matches!(self.permission, TokenPermission::Read | TokenPermission::All)
    }
    
    /// 检查权限是否允许写入
    pub fn can_write(&self) -> bool {
        matches!(self.permission, TokenPermission::Write | TokenPermission::All)
    }
}

/// 创建令牌请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateAccessTokenRequest {
    /// 令牌名称
    #[validate(length(min = 1, max = 100, message = "令牌名称长度应在 1-100 字符之间"))]
    pub name: String,
    
    /// 权限（默认 write）
    #[serde(default)]
    pub permission: TokenPermission,
    
    /// 有效期（小时），null 表示永不过期
    #[validate(range(min = 1, max = 8760, message = "有效期应在 1-8760 小时之间（最长1年）"))]
    pub expires_in_hours: Option<i64>,
    
    /// IP 白名单（可选）
    pub allowed_ips: Option<Vec<String>>,
    
    /// 每分钟请求限制（可选）
    #[validate(range(min = 1, max = 1000, message = "请求限制应在 1-1000 之间"))]
    pub rate_limit_per_minute: Option<i32>,
}

/// 创建令牌响应（包含一次性显示的完整令牌）
#[derive(Debug, Clone, Serialize)]
pub struct CreateAccessTokenResponse {
    pub id: Uuid,
    pub device_id: Uuid,
    pub name: String,
    
    /// 完整令牌（仅返回一次！）
    pub token: String,
    
    /// 令牌前缀（用于后续识别）
    pub token_prefix: String,
    
    pub permission: TokenPermission,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// 令牌列表项（不包含敏感信息）
#[derive(Debug, Clone, Serialize)]
pub struct AccessTokenInfo {
    pub id: Uuid,
    pub device_id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub permission: TokenPermission,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub use_count: i32,
    pub is_revoked: bool,
    pub is_expired: bool,
    pub created_at: DateTime<Utc>,
}

impl From<DeviceAccessToken> for AccessTokenInfo {
    fn from(token: DeviceAccessToken) -> Self {
        let is_expired = token.expires_at
            .map(|exp| exp < Utc::now())
            .unwrap_or(false);
        
        Self {
            id: token.id,
            device_id: token.device_id,
            name: token.name,
            token_prefix: token.token_prefix,
            permission: token.permission,
            expires_at: token.expires_at,
            last_used_at: token.last_used_at,
            use_count: token.use_count,
            is_revoked: token.is_revoked,
            is_expired,
            created_at: token.created_at,
        }
    }
}

/// 令牌列表查询参数
#[derive(Debug, Clone, Deserialize)]
pub struct AccessTokenListQuery {
    /// 是否包含已吊销的令牌
    #[serde(default)]
    pub include_revoked: bool,
    
    /// 是否包含已过期的令牌
    #[serde(default)]
    pub include_expired: bool,
}

/// 兼容模式电量上报请求（URL 参数）
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CompatBatteryReportQuery {
    /// 访问令牌
    pub token: String,
    
    /// 电量百分比
    #[validate(range(min = 0, max = 100, message = "电量应在 0-100 之间"))]
    pub level: i32,
    
    /// 是否充电（0 或 1）
    #[serde(default)]
    pub charging: Option<i32>,
    
    /// 温度
    pub temp: Option<f64>,
    
    /// 电压
    pub voltage: Option<f64>,
    
    /// 时间戳（Unix 秒）
    pub ts: Option<i64>,
}

impl CompatBatteryReportQuery {
    /// 转换为标准电量上报请求
    pub fn to_battery_report(&self) -> crate::models::BatteryReportRequest {
        use chrono::TimeZone;
        
        let recorded_at = self.ts
            .and_then(|ts| Utc.timestamp_opt(ts, 0).single());
        
        crate::models::BatteryReportRequest {
            battery_level: self.level,
            is_charging: self.charging.map(|c| c != 0).unwrap_or(false),
            power_saving_mode: crate::models::PowerSavingMode::Off,
            temperature: self.temp,
            voltage: self.voltage,
            recorded_at,
        }
    }
}

/// 兼容模式查询最新电量（URL 参数）
#[derive(Debug, Clone, Deserialize)]
pub struct CompatLatestBatteryQuery {
    /// 访问令牌
    pub token: String,
}

/// 吊销所有令牌请求
#[derive(Debug, Clone, Deserialize)]
pub struct RevokeAllTokensRequest {
    /// 确认吊销（可选的安全确认）
    #[serde(default)]
    pub confirm: bool,
}
