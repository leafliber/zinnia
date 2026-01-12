//! 审计日志模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::net::IpAddr;
use uuid::Uuid;

/// 操作者类型
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "actor_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ActorType {
    Device,
    Admin,
    System,
}

/// 审计状态
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "audit_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AuditStatus {
    Success,
    Failure,
}

/// 审计操作类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditAction {
    Create,
    Read,
    Update,
    Delete,
    Login,
    Logout,
    AuthFailure,
    RateLimited,
    ConfigChange,
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditAction::Create => write!(f, "CREATE"),
            AuditAction::Read => write!(f, "READ"),
            AuditAction::Update => write!(f, "UPDATE"),
            AuditAction::Delete => write!(f, "DELETE"),
            AuditAction::Login => write!(f, "LOGIN"),
            AuditAction::Logout => write!(f, "LOGOUT"),
            AuditAction::AuthFailure => write!(f, "AUTH_FAILURE"),
            AuditAction::RateLimited => write!(f, "RATE_LIMITED"),
            AuditAction::ConfigChange => write!(f, "CONFIG_CHANGE"),
        }
    }
}

/// 审计日志
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditLog {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub actor_type: ActorType,
    pub actor_id: String,
    pub action: String,
    pub resource: String,
    pub resource_id: Option<String>,
    pub ip_address: String,
    pub user_agent: Option<String>,
    pub status: AuditStatus,
    pub details: Option<serde_json::Value>,
    pub request_id: Option<String>,
}

/// 创建审计日志请求
#[derive(Debug, Clone)]
pub struct CreateAuditLogRequest {
    pub actor_type: ActorType,
    pub actor_id: String,
    pub action: AuditAction,
    pub resource: String,
    pub resource_id: Option<String>,
    pub ip_address: IpAddr,
    pub user_agent: Option<String>,
    pub status: AuditStatus,
    pub details: Option<serde_json::Value>,
    pub request_id: Option<String>,
}

/// 审计日志查询参数
#[derive(Debug, Clone, Deserialize)]
pub struct AuditLogQuery {
    pub actor_type: Option<ActorType>,
    pub actor_id: Option<String>,
    pub action: Option<String>,
    pub resource: Option<String>,
    pub status: Option<AuditStatus>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    #[serde(default = "default_page")]
    pub page: i64,
}

fn default_page_size() -> i64 { 50 }
fn default_page() -> i64 { 1 }
