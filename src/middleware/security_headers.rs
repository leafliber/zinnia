//! 安全头中间件

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue},
    Error,
};
use futures::future::{ok, LocalBoxFuture, Ready};
use std::rc::Rc;

/// 安全头中间件
pub struct SecurityHeaders;

impl SecurityHeaders {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SecurityHeaders {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, B> Transform<S, ServiceRequest> for SecurityHeaders
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = SecurityHeadersMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(SecurityHeadersMiddleware {
            service: Rc::new(service),
        })
    }
}

pub struct SecurityHeadersMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for SecurityHeadersMiddleware<S>
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
        let is_sensitive = is_sensitive_endpoint(req.path());

        Box::pin(async move {
            let mut res = service.call(req).await?;
            let headers = res.headers_mut();

            // 防止 MIME 类型嗅探
            headers.insert(
                HeaderName::from_static("x-content-type-options"),
                HeaderValue::from_static("nosniff"),
            );

            // 防止点击劫持
            headers.insert(
                HeaderName::from_static("x-frame-options"),
                HeaderValue::from_static("DENY"),
            );

            // XSS 保护
            headers.insert(
                HeaderName::from_static("x-xss-protection"),
                HeaderValue::from_static("1; mode=block"),
            );

            // 强制 HTTPS（仅生产环境）
            #[cfg(not(debug_assertions))]
            headers.insert(
                HeaderName::from_static("strict-transport-security"),
                HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
            );

            // 内容安全策略（API 服务通常不需要加载外部资源）
            headers.insert(
                HeaderName::from_static("content-security-policy"),
                HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
            );

            // 禁用 Referrer（隐私保护）
            headers.insert(
                HeaderName::from_static("referrer-policy"),
                HeaderValue::from_static("no-referrer"),
            );

            // 权限策略
            headers.insert(
                HeaderName::from_static("permissions-policy"),
                HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
            );

            // 敏感接口禁用缓存
            if is_sensitive {
                headers.insert(
                    HeaderName::from_static("cache-control"),
                    HeaderValue::from_static("no-store, no-cache, must-revalidate, private"),
                );
                headers.insert(
                    HeaderName::from_static("pragma"),
                    HeaderValue::from_static("no-cache"),
                );
                headers.insert(
                    HeaderName::from_static("expires"),
                    HeaderValue::from_static("0"),
                );
            }

            Ok(res)
        })
    }
}

/// 判断是否为敏感接口
fn is_sensitive_endpoint(path: &str) -> bool {
    let sensitive_patterns = [
        "/api/v1/auth",
        "/api/v1/devices",
        "/api/v1/battery",
        "/admin",
    ];

    sensitive_patterns
        .iter()
        .any(|pattern| path.starts_with(pattern))
}
