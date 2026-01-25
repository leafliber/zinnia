//! 日志中间件

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures::future::{ok, LocalBoxFuture, Ready};
use std::rc::Rc;
use std::time::Instant;
use tracing::{info, warn};
use uuid::Uuid;

/// 请求 ID（存储在请求扩展中）
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

/// 日志中间件
pub struct RequestLogger;

impl RequestLogger {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RequestLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, B> Transform<S, ServiceRequest> for RequestLogger
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestLoggerMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequestLoggerMiddleware {
            service: Rc::new(service),
        })
    }
}

pub struct RequestLoggerMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for RequestLoggerMiddleware<S>
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
        let start = Instant::now();

        // 生成请求 ID
        let request_id = req
            .headers()
            .get("X-Request-ID")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // 提取请求信息
        let method = req.method().to_string();
        let path = req.path().to_string();
        let query = req.query_string().to_string();
        let client_ip = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();
        let user_agent = req
            .headers()
            .get("User-Agent")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        // 将请求 ID 存入扩展
        req.extensions_mut().insert(RequestId(request_id.clone()));

        // 脱敏处理：不记录敏感头
        let has_auth =
            req.headers().contains_key("Authorization") || req.headers().contains_key("X-API-Key");

        Box::pin(async move {
            // 记录请求开始
            info!(
                request_id = %request_id,
                method = %method,
                path = %path,
                query = %query,
                client_ip = %client_ip,
                user_agent = %sanitize_user_agent(&user_agent),
                has_auth = has_auth,
                "请求开始"
            );

            // 处理请求
            let result = service.call(req).await;

            let duration = start.elapsed();

            match &result {
                Ok(res) => {
                    let status = res.status().as_u16();

                    if status >= 400 {
                        warn!(
                            request_id = %request_id,
                            method = %method,
                            path = %path,
                            status = status,
                            duration_ms = duration.as_millis() as u64,
                            "请求完成（错误）"
                        );
                    } else {
                        info!(
                            request_id = %request_id,
                            method = %method,
                            path = %path,
                            status = status,
                            duration_ms = duration.as_millis() as u64,
                            "请求完成"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        request_id = %request_id,
                        method = %method,
                        path = %path,
                        error = %e,
                        duration_ms = duration.as_millis() as u64,
                        "请求失败"
                    );
                }
            }

            result
        })
    }
}

/// 脱敏 User-Agent（移除可能的敏感信息）
fn sanitize_user_agent(ua: &str) -> String {
    // 截断过长的 User-Agent
    if ua.len() > 200 {
        format!("{}...", &ua[..200])
    } else {
        ua.to_string()
    }
}

/// 从请求中获取请求 ID
pub fn get_request_id(req: &ServiceRequest) -> Option<String> {
    req.extensions().get::<RequestId>().map(|r| r.0.clone())
}
