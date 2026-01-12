//! 限流中间件

use crate::db::RedisPool;
use crate::errors::AppError;
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue},
    Error,
};
use futures::future::{ok, LocalBoxFuture, Ready};
use std::rc::Rc;
use std::sync::Arc;

/// 限流配置
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// 每分钟请求数
    pub requests_per_minute: u32,
    /// 突发请求数
    pub burst_size: u32,
    /// 限流键前缀
    pub key_prefix: String,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            burst_size: 10,
            key_prefix: "ratelimit".to_string(),
        }
    }
}

/// 限流中间件
pub struct RateLimiter {
    config: RateLimitConfig,
    redis_pool: Arc<RedisPool>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig, redis_pool: Arc<RedisPool>) -> Self {
        Self { config, redis_pool }
    }

    /// 基于 IP 的限流
    pub fn by_ip(redis_pool: Arc<RedisPool>, requests_per_minute: u32) -> Self {
        Self::new(
            RateLimitConfig {
                requests_per_minute,
                burst_size: requests_per_minute / 6,
                key_prefix: "ratelimit:ip".to_string(),
            },
            redis_pool,
        )
    }

    /// 基于设备的限流
    pub fn by_device(redis_pool: Arc<RedisPool>, requests_per_minute: u32) -> Self {
        Self::new(
            RateLimitConfig {
                requests_per_minute,
                burst_size: requests_per_minute / 6,
                key_prefix: "ratelimit:device".to_string(),
            },
            redis_pool,
        )
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimiterMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RateLimiterMiddleware {
            service: Rc::new(service),
            config: self.config.clone(),
            redis_pool: self.redis_pool.clone(),
        })
    }
}

pub struct RateLimiterMiddleware<S> {
    service: Rc<S>,
    config: RateLimitConfig,
    redis_pool: Arc<RedisPool>,
}

impl<S, B> Service<ServiceRequest> for RateLimiterMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let config = self.config.clone();
        let redis_pool = self.redis_pool.clone();

        Box::pin(async move {
            // 获取客户端 IP
            let client_ip = req
                .connection_info()
                .realip_remote_addr()
                .unwrap_or("unknown")
                .to_string();

            // 构建限流键
            let rate_key = format!("{}:{}", config.key_prefix, client_ip);

            // 执行滑动窗口限流
            let result = check_rate_limit(
                &redis_pool,
                &rate_key,
                config.requests_per_minute,
                60,
            )
            .await;

            match result {
                Ok(rate_info) => {
                    if rate_info.is_limited {
                        tracing::warn!(
                            ip = %client_ip,
                            remaining = rate_info.remaining,
                            "请求被限流"
                        );
                        return Err(AppError::RateLimited(format!(
                            "请求过于频繁，请 {} 秒后重试",
                            rate_info.retry_after
                        ))
                        .into());
                    }

                    // 继续处理请求，添加限流信息头
                    let fut = service.call(req);
                    let mut res = fut.await?;

                    // 添加限流响应头
                    let headers = res.headers_mut();
                    headers.insert(
                        HeaderName::from_static("x-ratelimit-limit"),
                        HeaderValue::from_str(&config.requests_per_minute.to_string()).unwrap(),
                    );
                    headers.insert(
                        HeaderName::from_static("x-ratelimit-remaining"),
                        HeaderValue::from_str(&rate_info.remaining.to_string()).unwrap(),
                    );
                    headers.insert(
                        HeaderName::from_static("x-ratelimit-reset"),
                        HeaderValue::from_str(&rate_info.reset_at.to_string()).unwrap(),
                    );

                    Ok(res)
                }
                Err(e) => {
                    // 限流检查失败时，允许请求通过（fail-open）
                    tracing::error!(error = %e, "限流检查失败");
                    service.call(req).await
                }
            }
        })
    }
}

/// 限流信息
#[derive(Debug)]
pub struct RateLimitInfo {
    /// 是否被限流
    pub is_limited: bool,
    /// 剩余请求数
    pub remaining: u32,
    /// 重置时间（Unix 时间戳）
    pub reset_at: u64,
    /// 重试等待秒数
    pub retry_after: u32,
}

/// 检查限流（滑动窗口算法）
async fn check_rate_limit(
    redis_pool: &RedisPool,
    key: &str,
    limit: u32,
    window_seconds: u64,
) -> Result<RateLimitInfo, AppError> {
    let _conn = redis_pool.connection();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let _window_start = now - window_seconds;
    let reset_at = now + window_seconds;

    // 使用 Redis 事务执行滑动窗口算法
    // 1. 移除过期的请求记录
    // 2. 添加当前请求
    // 3. 获取窗口内的请求数

    // 简化实现：使用计数器
    let count_key = format!("{}:count", key);
    let count: Option<u32> = redis_pool.get(&count_key).await?;
    let current_count = count.unwrap_or(0);

    if current_count >= limit {
        return Ok(RateLimitInfo {
            is_limited: true,
            remaining: 0,
            reset_at,
            retry_after: window_seconds as u32,
        });
    }

    // 增加计数
    let mut conn = redis_pool.connection();
    redis::cmd("INCR")
        .arg(&count_key)
        .query_async::<u32>(&mut conn)
        .await
        .map_err(AppError::RedisError)?;

    // 设置过期时间
    redis::cmd("EXPIRE")
        .arg(&count_key)
        .arg(window_seconds)
        .query_async::<()>(&mut conn)
        .await
        .map_err(AppError::RedisError)?;

    Ok(RateLimitInfo {
        is_limited: false,
        remaining: limit - current_count - 1,
        reset_at,
        retry_after: 0,
    })
}
