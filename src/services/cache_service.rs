//! 缓存服务

use crate::db::RedisPool;
use crate::errors::AppError;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

/// 缓存键前缀
pub mod cache_keys {
    pub const DEVICE_CONFIG: &str = "zinnia:device:config";
    pub const BATTERY_LATEST: &str = "zinnia:battery:latest";
    pub const TOKEN_BLACKLIST: &str = "zinnia:token:blacklist";
    /// 限流缓存前缀（预留用于分布式限流）
    #[allow(dead_code)]
    pub const RATE_LIMIT: &str = "zinnia:ratelimit";
}

/// 缓存服务
pub struct CacheService {
    redis_pool: Arc<RedisPool>,
}

impl CacheService {
    pub fn new(redis_pool: Arc<RedisPool>) -> Self {
        Self { redis_pool }
    }

    /// 获取缓存
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, AppError> {
        self.redis_pool.get(key).await
    }

    /// 设置缓存（带过期时间）
    pub async fn set<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl_seconds: u64,
    ) -> Result<(), AppError> {
        self.redis_pool.set_ex(key, value, ttl_seconds).await
    }

    /// 删除缓存
    pub async fn delete(&self, key: &str) -> Result<(), AppError> {
        self.redis_pool.del(key).await
    }

    /// 批量删除缓存（按模式）
    pub async fn delete_pattern(&self, pattern: &str) -> Result<u64, AppError> {
        let mut conn = self.redis_pool.connection();
        
        // 查找匹配的键
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .map_err(AppError::RedisError)?;

        if keys.is_empty() {
            return Ok(0);
        }

        // 批量删除
        let count: u64 = redis::cmd("DEL")
            .arg(&keys)
            .query_async(&mut conn)
            .await
            .map_err(AppError::RedisError)?;

        Ok(count)
    }

    /// 检查键是否存在
    pub async fn exists(&self, key: &str) -> Result<bool, AppError> {
        let mut conn = self.redis_pool.connection();
        let exists: bool = redis::cmd("EXISTS")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(AppError::RedisError)?;

        Ok(exists)
    }

    /// 设置键过期时间
    pub async fn expire(&self, key: &str, seconds: u64) -> Result<(), AppError> {
        let mut conn = self.redis_pool.connection();
        redis::cmd("EXPIRE")
            .arg(key)
            .arg(seconds)
            .query_async(&mut conn)
            .await
            .map_err(AppError::RedisError)
    }

    /// 获取键的剩余过期时间
    pub async fn ttl(&self, key: &str) -> Result<i64, AppError> {
        let mut conn = self.redis_pool.connection();
        let ttl: i64 = redis::cmd("TTL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(AppError::RedisError)?;

        Ok(ttl)
    }

    // ========== Token 黑名单 ==========

    /// 将 Token 加入黑名单
    pub async fn blacklist_token(&self, jti: &str, expiry_seconds: u64) -> Result<(), AppError> {
        let key = format!("{}:{}", cache_keys::TOKEN_BLACKLIST, jti);
        self.set(&key, &"revoked", expiry_seconds).await
    }

    /// 检查 Token 是否在黑名单中
    pub async fn is_token_blacklisted(&self, jti: &str) -> Result<bool, AppError> {
        let key = format!("{}:{}", cache_keys::TOKEN_BLACKLIST, jti);
        self.exists(&key).await
    }

    // ========== 设备配置缓存 ==========

    /// 获取设备配置缓存键
    pub fn device_config_key(device_id: &str) -> String {
        format!("{}:{}", cache_keys::DEVICE_CONFIG, device_id)
    }

    /// 获取电量数据缓存键
    pub fn battery_latest_key(device_id: &str) -> String {
        format!("{}:{}", cache_keys::BATTERY_LATEST, device_id)
    }
}
