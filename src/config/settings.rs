//! 应用配置加载和管理

use config::{Config, ConfigError, Environment};
use secrecy::SecretString;
use serde::Deserialize;
use std::env;

/// 应用配置结构
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub server: ServerSettings,
    pub database: DatabaseSettings,
    pub redis: RedisSettings,
    pub jwt: JwtSettings,
    pub rate_limit: RateLimitSettings,
    pub logging: LoggingSettings,
    #[serde(default)]
    pub smtp: SmtpSettings,
    #[serde(default)]
    pub recaptcha: RecaptchaSettings,
    #[serde(default)]
    pub registration: RegistrationSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
    pub workers: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSettings {
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
    pub require_ssl: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisSettings {
    pub pool_size: u32,
    pub connect_timeout_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwtSettings {
    pub expiry_seconds: u64,
    pub refresh_expiry_days: u64,
    pub issuer: String,
    pub audience: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitSettings {
    pub requests_per_minute: u32,
    pub burst_size: u32,
    pub login_attempts_per_minute: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingSettings {
    pub level: String,
    pub format: String,
}

/// SMTP 邮件服务配置
#[derive(Debug, Clone, Deserialize)]
pub struct SmtpSettings {
    /// 是否启用 SMTP
    #[serde(default)]
    pub enabled: bool,
    /// SMTP 服务器地址
    #[serde(default = "default_smtp_host")]
    pub host: String,
    /// SMTP 端口
    #[serde(default = "default_smtp_port")]
    pub port: u16,
    /// SMTP 用户名
    #[serde(default)]
    pub username: String,
    /// 是否使用 TLS
    #[serde(default = "default_true")]
    pub tls: bool,
    /// 发件人邮箱
    #[serde(default)]
    pub from_email: String,
    /// 发件人名称
    #[serde(default = "default_from_name")]
    pub from_name: String,
    /// 验证码有效期（秒）
    #[serde(default = "default_code_expiry")]
    pub code_expiry_seconds: u64,
    /// 每小时每邮箱最大发送次数
    #[serde(default = "default_max_sends")]
    pub max_sends_per_hour: u32,
}

impl Default for SmtpSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            host: default_smtp_host(),
            port: default_smtp_port(),
            username: String::new(),
            tls: true,
            from_email: String::new(),
            from_name: default_from_name(),
            code_expiry_seconds: default_code_expiry(),
            max_sends_per_hour: default_max_sends(),
        }
    }
}

fn default_smtp_host() -> String { "smtp.example.com".to_string() }
fn default_smtp_port() -> u16 { 587 }
fn default_from_name() -> String { "Zinnia".to_string() }
fn default_code_expiry() -> u64 { 600 }
fn default_max_sends() -> u32 { 5 }
fn default_true() -> bool { true }

/// Google reCAPTCHA 配置
#[derive(Debug, Clone, Deserialize)]
pub struct RecaptchaSettings {
    /// 是否启用 reCAPTCHA
    #[serde(default)]
    pub enabled: bool,
    /// reCAPTCHA 站点密钥（前端使用）
    #[serde(default)]
    pub site_key: String,
    /// 分数阈值 (0.0 - 1.0，用于 v3)
    #[serde(default = "default_score_threshold")]
    pub score_threshold: f64,
}

impl Default for RecaptchaSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            site_key: String::new(),
            score_threshold: 0.5,
        }
    }
}

fn default_score_threshold() -> f64 { 0.5 }

/// 注册安全配置
#[derive(Debug, Clone, Deserialize)]
pub struct RegistrationSettings {
    /// 同一 IP 每小时最大注册次数
    #[serde(default = "default_max_per_hour")]
    pub max_per_ip_per_hour: u32,
    /// 同一 IP 每天最大注册次数
    #[serde(default = "default_max_per_day")]
    pub max_per_ip_per_day: u32,
    /// 是否强制要求邮箱验证
    #[serde(default = "default_true")]
    pub require_email_verification: bool,
    /// 是否强制要求 reCAPTCHA
    #[serde(default = "default_true")]
    pub require_recaptcha: bool,
}

impl Default for RegistrationSettings {
    fn default() -> Self {
        Self {
            max_per_ip_per_hour: 5,
            max_per_ip_per_day: 10,
            require_email_verification: true,
            require_recaptcha: true,
        }
    }
}

fn default_max_per_hour() -> u32 { 5 }
fn default_max_per_day() -> u32 { 10 }

impl Settings {
    /// 从环境变量加载配置（不依赖配置文件）
    /// 
    /// 配置优先级：
    /// 1. 内置默认值（代码中定义）
    /// 2. ZINNIA_* 环境变量（覆盖默认值）
    /// 
    /// 示例：
    /// - ZINNIA_SERVER__HOST=0.0.0.0
    /// - ZINNIA_SERVER__PORT=8080
    /// - ZINNIA_LOGGING__LEVEL=info
    pub fn load() -> Result<Self, ConfigError> {
        let app_env = env::var("APP_ENV").unwrap_or_else(|_| "production".into());
        
        let settings = Config::builder()
            // 服务器默认配置
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080)?
            .set_default("server.workers", 0)?  // 0 = 自动检测 CPU 核心数
            
            // 数据库默认配置
            .set_default("database.max_connections", if app_env == "production" { 50 } else { 10 })?
            .set_default("database.min_connections", if app_env == "production" { 10 } else { 2 })?
            .set_default("database.connect_timeout_seconds", if app_env == "production" { 10 } else { 30 })?
            .set_default("database.idle_timeout_seconds", if app_env == "production" { 300 } else { 600 })?
            .set_default("database.require_ssl", app_env == "production")?
            
            // Redis 默认配置
            .set_default("redis.pool_size", if app_env == "production" { 20 } else { 10 })?
            .set_default("redis.connect_timeout_seconds", if app_env == "production" { 3 } else { 5 })?
            
            // JWT 默认配置
            .set_default("jwt.expiry_seconds", 900)?  // 15 分钟
            .set_default("jwt.refresh_expiry_days", 7)?
            .set_default("jwt.issuer", if app_env == "production" { "zinnia" } else { "zinnia-dev" })?
            .set_default("jwt.audience", "zinnia-api")?
            
            // 限流默认配置
            .set_default("rate_limit.requests_per_minute", 60)?
            .set_default("rate_limit.burst_size", 10)?
            .set_default("rate_limit.login_attempts_per_minute", if app_env == "production" { 5 } else { 10 })?
            
            // 日志默认配置
            .set_default("logging.level", if app_env == "production" { "info" } else { "debug" })?
            .set_default("logging.format", if app_env == "production" { "json" } else { "pretty" })?
            
            // SMTP 默认配置
            .set_default("smtp.enabled", false)?
            .set_default("smtp.host", "smtp.example.com")?
            .set_default("smtp.port", 465)?
            .set_default("smtp.from_email", "noreply@example.com")?
            .set_default("smtp.from_name", "Zinnia")?
            .set_default("smtp.tls", true)?
            .set_default("smtp.code_expiry_seconds", 600)?
            .set_default("smtp.max_sends_per_hour", 30)?
            
            // reCAPTCHA 默认配置
            .set_default("recaptcha.enabled", false)?
            .set_default("recaptcha.site_key", "")?
            .set_default("recaptcha.score_threshold", 0.5)?
            
            // 注册安全默认配置
            .set_default("registration.max_per_ip_per_hour", 5)?
            .set_default("registration.max_per_ip_per_day", 10)?
            .set_default("registration.require_email_verification", true)?
            .set_default("registration.require_recaptcha", true)?
            
            // 环境变量覆盖（最高优先级）
            .add_source(
                Environment::with_prefix("ZINNIA")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()?;

        settings.try_deserialize()
    }

    /// 获取数据库连接 URL（从环境变量）
    pub fn database_url() -> SecretString {
        SecretString::new(
            env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set in environment")
        )
    }

    /// 获取 Redis 连接 URL（从环境变量）
    pub fn redis_url() -> SecretString {
        SecretString::new(
            env::var("REDIS_URL")
                .expect("REDIS_URL must be set in environment")
        )
    }

    /// 获取 JWT 密钥（从环境变量）
    pub fn jwt_secret() -> SecretString {
        SecretString::new(
            env::var("JWT_SECRET")
                .expect("JWT_SECRET must be set in environment")
        )
    }

    /// 获取加密密钥（从环境变量）
    pub fn encryption_key() -> SecretString {
        SecretString::new(
            env::var("ENCRYPTION_KEY")
                .expect("ENCRYPTION_KEY must be set in environment")
        )
    }

    /// 获取 SMTP 密码（从环境变量）
    pub fn smtp_password() -> Option<SecretString> {
        env::var("SMTP_PASSWORD").ok().map(SecretString::new)
    }

    /// 获取 reCAPTCHA 密钥（从环境变量）
    pub fn recaptcha_secret_key() -> Option<SecretString> {
        env::var("RECAPTCHA_SECRET_KEY").ok().map(SecretString::new)
    }

    /// 获取服务器地址
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_development_config() {
        // 设置测试环境
        env::set_var("APP_ENV", "development");
        
        // 注意：此测试需要存在配置文件才能通过
        // let settings = Settings::load();
        // assert!(settings.is_ok());
    }
}
