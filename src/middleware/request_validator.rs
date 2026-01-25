//! 请求验证中间件

use crate::errors::AppError;
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::CONTENT_TYPE,
    Error,
};
use futures::future::{ok, LocalBoxFuture, Ready};
use std::rc::Rc;

/// 请求验证配置
#[derive(Debug, Clone)]
pub struct RequestValidatorConfig {
    /// 最大请求体大小（字节）
    pub max_body_size: usize,
    /// 请求超时（秒）
    pub timeout_seconds: u64,
    /// 允许的 Content-Type
    pub allowed_content_types: Vec<String>,
}

impl Default for RequestValidatorConfig {
    fn default() -> Self {
        Self {
            max_body_size: 1024 * 1024, // 1 MB
            timeout_seconds: 30,
            allowed_content_types: vec![
                "application/json".to_string(),
                "application/json; charset=utf-8".to_string(),
            ],
        }
    }
}

/// 请求验证中间件
pub struct RequestValidator {
    config: RequestValidatorConfig,
}

impl RequestValidator {
    pub fn new(config: RequestValidatorConfig) -> Self {
        Self { config }
    }
}

impl Default for RequestValidator {
    fn default() -> Self {
        Self::new(RequestValidatorConfig::default())
    }
}

impl<S, B> Transform<S, ServiceRequest> for RequestValidator
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestValidatorMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequestValidatorMiddleware {
            service: Rc::new(service),
            config: self.config.clone(),
        })
    }
}

pub struct RequestValidatorMiddleware<S> {
    service: Rc<S>,
    config: RequestValidatorConfig,
}

impl<S, B> Service<ServiceRequest> for RequestValidatorMiddleware<S>
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

        Box::pin(async move {
            // 检查 Content-Length
            if let Some(content_length) = req.headers().get("Content-Length") {
                if let Ok(length) = content_length.to_str().unwrap_or("0").parse::<usize>() {
                    if length > config.max_body_size {
                        return Err(AppError::ValidationError(format!(
                            "请求体过大，最大允许 {} 字节",
                            config.max_body_size
                        ))
                        .into());
                    }
                }
            }

            // 对 POST/PUT/PATCH 请求检查 Content-Type
            let method = req.method().as_str();
            if matches!(method, "POST" | "PUT" | "PATCH") {
                if let Some(content_type) = req.headers().get(CONTENT_TYPE) {
                    let ct = content_type.to_str().unwrap_or("").to_lowercase();
                    let is_valid = config
                        .allowed_content_types
                        .iter()
                        .any(|allowed| ct.starts_with(&allowed.to_lowercase()));

                    if !is_valid {
                        return Err(AppError::ValidationError(
                            "不支持的 Content-Type，请使用 application/json".to_string(),
                        )
                        .into());
                    }
                } else {
                    // 检查是否有请求体
                    if req.headers().contains_key("Content-Length") {
                        return Err(
                            AppError::ValidationError("缺少 Content-Type 头".to_string()).into(),
                        );
                    }
                }
            }

            // 检查潜在的恶意请求头
            if let Err(e) = validate_headers(&req) {
                return Err(e.into());
            }

            service.call(req).await
        })
    }
}

/// 验证请求头
fn validate_headers(req: &ServiceRequest) -> Result<(), AppError> {
    // 检查 Host 头注入
    if let Some(host) = req.headers().get("Host") {
        let host_str = host.to_str().unwrap_or("");
        if host_str.contains("..") || host_str.contains("//") {
            tracing::warn!(host = %host_str, "可疑的 Host 头");
            return Err(AppError::ValidationError("无效的请求".to_string()));
        }
    }

    // 检查过长的头
    for (name, value) in req.headers().iter() {
        let value_str = value.to_str().unwrap_or("");
        if value_str.len() > 8192 {
            tracing::warn!(header = %name, "请求头过长");
            return Err(AppError::ValidationError("请求头过长".to_string()));
        }
    }

    Ok(())
}
