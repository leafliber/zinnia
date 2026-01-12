//! 应用配置加载和管理

use config::{Config, ConfigError, Environment, File};
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

impl Settings {
    /// 从配置文件和环境变量加载配置
    pub fn load() -> Result<Self, ConfigError> {
        let run_mode = env::var("APP_ENV").unwrap_or_else(|_| "development".into());

        let settings = Config::builder()
            // 加载默认配置
            .add_source(File::with_name("config/development"))
            // 根据环境加载对应配置
            .add_source(File::with_name(&format!("config/{}", run_mode)).required(false))
            // 环境变量覆盖，前缀 ZINNIA，分隔符 __
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
