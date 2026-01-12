//! Redis 连接池管理

use crate::config::Settings;
use crate::errors::AppError;
use redis::aio::ConnectionManager;
use redis::Client;
use secrecy::ExposeSecret;

/// Redis 连接池包装
#[derive(Clone)]
pub struct RedisPool {
    manager: ConnectionManager,
}

impl RedisPool {
    /// 创建新的 Redis 连接
    pub async fn new(_settings: &Settings) -> Result<Self, AppError> {
        let redis_url = Settings::redis_url();
        
        let client = Client::open(redis_url.expose_secret().as_str())
            .map_err(|e| AppError::ConfigError(format!("Redis URL 无效: {}", e)))?;

        let manager = ConnectionManager::new(client)
            .await
            .map_err(|e| {
                tracing::error!("Redis 连接失败: {}", e);
                AppError::RedisError(e)
            })?;

        tracing::info!("Redis 连接已建立");

        Ok(Self { manager })
    }

    /// 获取连接管理器
    pub fn connection(&self) -> ConnectionManager {
        self.manager.clone()
    }

    /// 健康检查
    pub async fn health_check(&self) -> Result<(), AppError> {
        let mut conn = self.manager.clone();
        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map(|_| ())
            .map_err(AppError::RedisError)
    }

    /// 设置缓存（带过期时间）
    pub async fn set_ex<T: serde::Serialize>(
        &self,
        key: &str,
        value: &T,
        expiry_seconds: u64,
    ) -> Result<(), AppError> {
        let mut conn = self.manager.clone();
        let serialized = serde_json::to_string(value)
            .map_err(|e| AppError::InternalError(format!("序列化失败: {}", e)))?;
        
        redis::cmd("SETEX")
            .arg(key)
            .arg(expiry_seconds)
            .arg(serialized)
            .query_async(&mut conn)
            .await
            .map_err(AppError::RedisError)
    }

    /// 获取缓存
    pub async fn get<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, AppError> {
        let mut conn = self.manager.clone();
        let result: Option<String> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(AppError::RedisError)?;

        match result {
            Some(data) => {
                let value = serde_json::from_str(&data)
                    .map_err(|e| AppError::InternalError(format!("反序列化失败: {}", e)))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// 删除缓存
    pub async fn del(&self, key: &str) -> Result<(), AppError> {
        let mut conn = self.manager.clone();
        redis::cmd("DEL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(AppError::RedisError)
    }

    /// 获取 key 的剩余 TTL（秒）
    pub async fn ttl(&self, key: &str) -> Result<i64, AppError> {
        let mut conn = self.manager.clone();
        redis::cmd("TTL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(AppError::RedisError)
    }

    /// 递增计数器
    pub async fn incr(&self, key: &str) -> Result<i64, AppError> {
        let mut conn = self.manager.clone();
        redis::cmd("INCR")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(AppError::RedisError)
    }

    /// 递增计数器并设置过期时间（如果是新 key）
    pub async fn incr_ex(&self, key: &str, expiry_seconds: u64) -> Result<i64, AppError> {
        let mut conn = self.manager.clone();
        
        // 先递增
        let count: i64 = redis::cmd("INCR")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(AppError::RedisError)?;
        
        // 如果是第一次（count == 1），设置过期时间
        if count == 1 {
            let _: () = redis::cmd("EXPIRE")
                .arg(key)
                .arg(expiry_seconds)
                .query_async(&mut conn)
                .await
                .map_err(AppError::RedisError)?;
        }
        
        Ok(count)
    }
}
