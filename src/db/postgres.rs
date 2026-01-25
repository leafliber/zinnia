//! PostgreSQL/TimescaleDB 连接池管理

use crate::config::Settings;
use crate::errors::AppError;
use secrecy::ExposeSecret;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;

/// PostgreSQL 连接池包装
#[derive(Clone)]
pub struct PostgresPool {
    pool: PgPool,
}

impl PostgresPool {
    /// 创建新的数据库连接池
    pub async fn new(settings: &Settings) -> Result<Self, AppError> {
        let database_url = Settings::database_url();

        // 解析连接选项
        let mut options = PgConnectOptions::from_str(database_url.expose_secret())
            .map_err(|e| AppError::ConfigError(format!("数据库 URL 无效: {}", e)))?;

        // 设置 SSL 模式
        if settings.database.require_ssl {
            options = options.ssl_mode(PgSslMode::Require);
        }

        // 创建连接池
        let pool = PgPoolOptions::new()
            .max_connections(settings.database.max_connections)
            .min_connections(settings.database.min_connections)
            .acquire_timeout(Duration::from_secs(
                settings.database.connect_timeout_seconds,
            ))
            .idle_timeout(Duration::from_secs(settings.database.idle_timeout_seconds))
            .connect_with(options)
            .await
            .map_err(|e| {
                tracing::error!("数据库连接失败: {}", e);
                AppError::DatabaseError(e)
            })?;

        tracing::info!("数据库连接池已创建");

        Ok(Self { pool })
    }

    /// 获取内部连接池引用
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// 健康检查
    pub async fn health_check(&self) -> Result<(), AppError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::DatabaseError)
    }

    /// 运行数据库迁移
    pub async fn run_migrations(&self) -> Result<(), AppError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| AppError::InternalError(format!("迁移失败: {}", e)))
    }
}
