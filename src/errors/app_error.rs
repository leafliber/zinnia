//! 统一错误类型定义

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::Serialize;

/// 应用错误类型
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // 认证错误 (401)
    #[error("认证失败")]
    Unauthorized(String),

    // 权限错误 (403)
    #[error("权限不足")]
    Forbidden(String),

    // 资源不存在 (404)
    #[error("资源不存在")]
    NotFound(String),

    // 请求验证错误 (400)
    #[error("请求参数无效")]
    ValidationError(String),

    // 冲突错误 (409)
    #[error("资源冲突")]
    Conflict(String),

    // 限流错误 (429)
    #[error("请求过于频繁")]
    RateLimited(String),

    // 限流错误 (429) - 别名，用于更明确的语义
    #[error("超出限制")]
    RateLimitExceeded(String),

    // 数据库错误 (500)
    #[error("数据库错误")]
    DatabaseError(#[from] sqlx::Error),

    // Redis 错误 (500)
    #[error("缓存服务错误")]
    RedisError(#[from] redis::RedisError),

    // 内部错误 (500)
    #[error("内部服务错误")]
    InternalError(String),

    // 配置错误
    #[error("配置错误")]
    ConfigError(String),
}

/// API 错误响应结构
#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    // 人类可读的错误信息（用于展示）
    message: String,
    // 机器可识别的错误类型（例如: "validation_error", "unauthorized"）
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    // 可选的详细错误信息（字段级错误或内部提示）
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            AppError::RateLimitExceeded(_) => StatusCode::TOO_MANY_REQUESTS,
            AppError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::RedisError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::ConfigError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        let is_dev = std::env::var("APP_ENV").unwrap_or_default() == "development";
        // 生产环境下不暴露内部错误细节，开发环境输出详细错误
        let message = match self {
            // 安全错误信息：不泄露具体原因
            AppError::Unauthorized(_) => "认证失败".to_string(),
            AppError::Forbidden(_) => "权限不足".to_string(),
            AppError::NotFound(_) => "资源不存在".to_string(),
            AppError::ValidationError(msg) => msg.clone(),
            AppError::Conflict(msg) => msg.clone(),
            AppError::RateLimited(_) => "请求过于频繁，请稍后重试".to_string(),
            AppError::RateLimitExceeded(msg) => msg.clone(),
            // 内部错误：隐藏具体细节（dev 环境下输出详细错误）
            AppError::DatabaseError(e) => {
                if is_dev {
                    format!("数据库错误: {e:?}")
                } else {
                    "服务暂时不可用".to_string()
                }
            }
            AppError::RedisError(e) => {
                if is_dev {
                    format!("Redis错误: {e:?}")
                } else {
                    "服务暂时不可用".to_string()
                }
            }
            AppError::InternalError(msg) => {
                if is_dev {
                    format!("内部服务错误: {msg}")
                } else {
                    "服务内部错误".to_string()
                }
            }
            AppError::ConfigError(msg) => {
                if is_dev {
                    format!("配置错误: {msg}")
                } else {
                    "服务配置错误".to_string()
                }
            }
        };

        // 记录错误日志（开发环境记录详细信息）
        if is_dev {
            tracing::error!(
                error_type = %self,
                status = %status,
                error_detail = ?self,
                "请求处理错误(dev)"
            );
        } else {
            tracing::error!(
                error_type = %self,
                status = %status,
                "请求处理错误"
            );
        }

        // 构建附加字段：对于 ValidationError 我们在生产环境也要将具体信息传回前端
        let (err_type, details) = match self {
            AppError::ValidationError(msg) => {
                (Some("validation_error".to_string()), Some(msg.clone()))
            }
            AppError::Unauthorized(_) => (Some("unauthorized".to_string()), None),
            AppError::Forbidden(_) => (Some("forbidden".to_string()), None),
            AppError::NotFound(_) => (Some("not_found".to_string()), None),
            AppError::Conflict(msg) => (Some("conflict".to_string()), Some(msg.clone())),
            AppError::RateLimitExceeded(msg) => {
                (Some("rate_limit_exceeded".to_string()), Some(msg.clone()))
            }
            _ => (None, None),
        };

        HttpResponse::build(status).json(ErrorResponse {
            code: status.as_u16(),
            message,
            error: err_type,
            details,
            request_id: None, // TODO: 从请求上下文获取
        })
    }
}

impl From<config::ConfigError> for AppError {
    fn from(err: config::ConfigError) -> Self {
        AppError::ConfigError(err.to_string())
    }
}
