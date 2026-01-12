//! 用户数据模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

/// 用户角色枚举
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    User,
    Readonly,
}

impl Default for UserRole {
    fn default() -> Self {
        UserRole::User
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "admin"),
            UserRole::User => write!(f, "user"),
            UserRole::Readonly => write!(f, "readonly"),
        }
    }
}

/// 用户实体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    /// 密码哈希（不序列化）
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: UserRole,
    pub is_active: bool,
    pub email_verified: bool,
    
    /// 登录失败次数
    #[serde(skip_serializing)]
    pub failed_login_attempts: i32,
    /// 锁定截止时间
    #[serde(skip_serializing)]
    pub locked_until: Option<DateTime<Utc>>,
    pub last_login_at: Option<DateTime<Utc>>,
    
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// 用户刷新令牌
#[derive(Debug, Clone, FromRow)]
pub struct UserRefreshToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub device_info: Option<String>,
    pub ip_address: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// 用户注册请求（第一步：发送验证码）
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email(message = "邮箱格式无效"))]
    pub email: String,
    
    #[validate(length(min = 3, max = 50, message = "用户名长度应在 3-50 字符之间"))]
    #[validate(custom(function = "validate_username"))]
    pub username: String,
    
    #[validate(length(min = 8, max = 128, message = "密码长度应在 8-128 字符之间"))]
    pub password: String,
    
    /// 确认密码
    #[validate(length(min = 8, max = 128, message = "密码长度应在 8-128 字符之间"))]
    pub confirm_password: String,
    
    /// reCAPTCHA 响应令牌（如果启用）
    #[serde(default)]
    pub recaptcha_token: Option<String>,
    
    /// 邮箱验证码（如果启用邮箱验证）
    #[serde(default)]
    pub verification_code: Option<String>,
}

/// 发送验证码请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SendVerificationCodeRequest {
    #[validate(email(message = "邮箱格式无效"))]
    pub email: String,
    
    /// reCAPTCHA 响应令牌
    #[serde(default)]
    pub recaptcha_token: Option<String>,
}

/// 验证验证码请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct VerifyCodeRequest {
    #[validate(email(message = "邮箱格式无效"))]
    pub email: String,
    
    #[validate(length(equal = 6, message = "验证码应为6位数字"))]
    pub code: String,
}

/// 验证码发送响应
#[derive(Debug, Clone, Serialize)]
pub struct VerificationCodeResponse {
    pub message: String,
    pub expires_in_minutes: u64,
}

fn validate_username(username: &str) -> Result<(), validator::ValidationError> {
    lazy_static::lazy_static! {
        static ref USERNAME_REGEX: regex::Regex = regex::Regex::new(r"^[a-zA-Z0-9_]+$").unwrap();
    }
    if USERNAME_REGEX.is_match(username) {
        Ok(())
    } else {
        Err(validator::ValidationError::new("用户名只能包含字母、数字和下划线"))
    }
}

/// 用户登录请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct LoginRequest {
    /// 邮箱或用户名
    #[validate(length(min = 1, message = "请输入邮箱或用户名"))]
    pub login: String,
    
    #[validate(length(min = 1, message = "请输入密码"))]
    pub password: String,
    
    /// 设备信息（可选）
    pub device_info: Option<String>,
}

/// 登录响应
#[derive(Debug, Clone, Serialize)]
pub struct LoginResponse {
    pub user: UserInfo,
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

/// 用户信息（安全返回）
#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    pub role: UserRole,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl From<User> for UserInfo {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            username: user.username,
            role: user.role,
            email_verified: user.email_verified,
            created_at: user.created_at,
            last_login_at: user.last_login_at,
        }
    }
}

/// 更新用户请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(min = 3, max = 50, message = "用户名长度应在 3-50 字符之间"))]
    pub username: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// 修改密码请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 1, message = "请输入当前密码"))]
    pub current_password: String,
    
    #[validate(length(min = 8, max = 128, message = "新密码长度应在 8-128 字符之间"))]
    pub new_password: String,
    
    #[validate(must_match(other = "new_password", message = "两次输入的密码不一致"))]
    pub confirm_password: String,
}

/// 刷新令牌请求
#[derive(Debug, Clone, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

/// 用户列表查询参数
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UserListQuery {
    #[validate(range(min = 1, max = 100, message = "每页数量应在 1-100 之间"))]
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    
    #[validate(range(min = 1, message = "页码应大于 0"))]
    #[serde(default = "default_page")]
    pub page: i64,
    
    pub role: Option<UserRole>,
    pub is_active: Option<bool>,
    pub search: Option<String>,
}

fn default_page_size() -> i64 { 20 }
fn default_page() -> i64 { 1 }

/// 设备共享记录
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeviceShare {
    pub device_id: Uuid,
    pub user_id: Uuid,
    pub permission: String,
    pub created_at: DateTime<Utc>,
}

/// 共享权限
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SharePermission {
    Read,
    Write,
    Admin,
}

impl std::fmt::Display for SharePermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SharePermission::Read => write!(f, "read"),
            SharePermission::Write => write!(f, "write"),
            SharePermission::Admin => write!(f, "admin"),
        }
    }
}

/// 共享设备请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ShareDeviceRequest {
    /// 目标用户邮箱或用户名
    #[validate(length(min = 1, message = "请指定用户"))]
    pub user_identifier: String,
    
    #[serde(default = "default_permission")]
    pub permission: SharePermission,
}

fn default_permission() -> SharePermission { SharePermission::Read }

/// 共享详情响应
#[derive(Debug, Clone, Serialize)]
pub struct DeviceShareInfo {
    pub device_id: Uuid,
    pub user: UserInfo,
    pub permission: String,
    pub created_at: DateTime<Utc>,
}
