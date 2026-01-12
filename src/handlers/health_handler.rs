//! 健康检查 API 处理器

use crate::db::{PostgresPool, RedisPool};
use crate::models::{HealthCheckResponse, ServiceStatus};
use actix_web::{web, HttpResponse};
use std::sync::Arc;
use std::time::Instant;

/// 应用启动时间
static START_TIME: once_cell::sync::Lazy<Instant> = once_cell::sync::Lazy::new(Instant::now);

/// 简单健康检查（用于负载均衡器）
pub async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok"
    }))
}

/// 详细健康检查
pub async fn health_detailed(
    pg_pool: web::Data<Arc<PostgresPool>>,
    redis_pool: web::Data<Arc<RedisPool>>,
) -> HttpResponse {
    // 检查数据库
    let db_start = Instant::now();
    let db_status = match pg_pool.health_check().await {
        Ok(_) => ServiceStatus::healthy(db_start.elapsed().as_millis() as u64),
        Err(_) => ServiceStatus::unhealthy(),
    };

    // 检查 Redis
    let redis_start = Instant::now();
    let redis_status = match redis_pool.health_check().await {
        Ok(_) => ServiceStatus::healthy(redis_start.elapsed().as_millis() as u64),
        Err(_) => ServiceStatus::unhealthy(),
    };

    // 计算运行时间
    let uptime = START_TIME.elapsed().as_secs();

    let response = HealthCheckResponse {
        status: if db_status.status == "healthy" && redis_status.status == "healthy" {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        version: env!("CARGO_PKG_VERSION").to_string(),
        database: db_status,
        redis: redis_status,
        uptime_seconds: uptime,
    };

    HttpResponse::Ok().json(response)
}

/// 就绪检查（用于 Kubernetes）
pub async fn ready(
    pg_pool: web::Data<Arc<PostgresPool>>,
    redis_pool: web::Data<Arc<RedisPool>>,
) -> HttpResponse {
    // 检查所有依赖服务
    let db_ok = pg_pool.health_check().await.is_ok();
    let redis_ok = redis_pool.health_check().await.is_ok();

    if db_ok && redis_ok {
        HttpResponse::Ok().json(serde_json::json!({
            "ready": true
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "ready": false,
            "database": db_ok,
            "redis": redis_ok
        }))
    }
}

/// 存活检查（用于 Kubernetes）
pub async fn live() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "alive": true
    }))
}
