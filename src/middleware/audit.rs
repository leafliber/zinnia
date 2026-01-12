//! 审计日志中间件

use crate::db::PostgresPool;
use crate::models::{ActorType, AuditAction, AuditStatus, CreateAuditLogRequest};
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures::future::{ok, LocalBoxFuture, Ready};
use std::net::IpAddr;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use super::{AuthInfo, RequestId};

/// 需要审计的操作配置
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// 审计的 HTTP 方法
    pub methods: Vec<String>,
    /// 审计的路径前缀
    pub path_prefixes: Vec<String>,
    /// 是否记录成功操作
    pub log_success: bool,
    /// 是否记录失败操作
    pub log_failure: bool,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            methods: vec![
                "POST".to_string(),
                "PUT".to_string(),
                "PATCH".to_string(),
                "DELETE".to_string(),
            ],
            path_prefixes: vec![
                "/api/v1/devices".to_string(),
                "/api/v1/auth".to_string(),
                "/admin".to_string(),
            ],
            log_success: true,
            log_failure: true,
        }
    }
}

/// 审计中间件
pub struct AuditLogger {
    config: AuditConfig,
    db_pool: Arc<PostgresPool>,
}

impl AuditLogger {
    pub fn new(config: AuditConfig, db_pool: Arc<PostgresPool>) -> Self {
        Self { config, db_pool }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuditLogger
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuditLoggerMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(AuditLoggerMiddleware {
            service: Rc::new(service),
            config: self.config.clone(),
            db_pool: self.db_pool.clone(),
        })
    }
}

pub struct AuditLoggerMiddleware<S> {
    service: Rc<S>,
    config: AuditConfig,
    db_pool: Arc<PostgresPool>,
}

impl<S, B> Service<ServiceRequest> for AuditLoggerMiddleware<S>
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
        let db_pool = self.db_pool.clone();

        // 提取请求信息
        let method = req.method().to_string();
        let path = req.path().to_string();
        let should_audit = should_audit_request(&config, &method, &path);

        if !should_audit {
            return Box::pin(async move { service.call(req).await });
        }

        // 提取审计信息
        let client_ip = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("0.0.0.0")
            .to_string();
        let user_agent = req
            .headers()
            .get("User-Agent")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());
        let request_id = req
            .extensions()
            .get::<RequestId>()
            .map(|r| r.0.clone());
        let auth_info = req.extensions().get::<AuthInfo>().cloned();

        Box::pin(async move {
            let result = service.call(req).await;

            // 确定审计状态
            let (status, should_log) = match &result {
                Ok(res) => {
                    let status_code = res.status().as_u16();
                    if status_code >= 400 {
                        (AuditStatus::Failure, config.log_failure)
                    } else {
                        (AuditStatus::Success, config.log_success)
                    }
                }
                Err(_) => (AuditStatus::Failure, config.log_failure),
            };

            if should_log {
                // 异步记录审计日志（不阻塞响应）
                let audit_request = CreateAuditLogRequest {
                    actor_type: auth_info
                        .as_ref()
                        .map(|a| match a.auth_type {
                            super::AuthType::Jwt => ActorType::Admin,
                            super::AuthType::ApiKey => ActorType::Device,
                        })
                        .unwrap_or(ActorType::System),
                    actor_id: auth_info
                        .map(|a| a.actor_id)
                        .unwrap_or_else(|| "anonymous".to_string()),
                    action: method_to_action(&method),
                    resource: extract_resource(&path),
                    resource_id: extract_resource_id(&path),
                    ip_address: IpAddr::from_str(&client_ip).unwrap_or(IpAddr::from([0, 0, 0, 0])),
                    user_agent,
                    status,
                    details: None,
                    request_id,
                };

                // 在后台任务中记录审计日志
                tokio::spawn(async move {
                    if let Err(e) = log_audit(&db_pool, audit_request).await {
                        tracing::error!(error = %e, "审计日志记录失败");
                    }
                });
            }

            result
        })
    }
}

/// 判断是否需要审计
fn should_audit_request(config: &AuditConfig, method: &str, path: &str) -> bool {
    let method_match = config.methods.iter().any(|m| m == method);
    let path_match = config
        .path_prefixes
        .iter()
        .any(|prefix| path.starts_with(prefix));

    method_match && path_match
}

/// HTTP 方法转换为审计操作
fn method_to_action(method: &str) -> AuditAction {
    match method {
        "POST" => AuditAction::Create,
        "PUT" | "PATCH" => AuditAction::Update,
        "DELETE" => AuditAction::Delete,
        "GET" => AuditAction::Read,
        _ => AuditAction::Read,
    }
}

/// 从路径提取资源类型
fn extract_resource(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    // /api/v1/devices/xxx -> devices
    if parts.len() >= 4 {
        parts[3].to_string()
    } else {
        path.to_string()
    }
}

/// 从路径提取资源 ID
fn extract_resource_id(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    // /api/v1/devices/xxx -> xxx
    if parts.len() >= 5 {
        Some(parts[4].to_string())
    } else {
        None
    }
}

/// 记录审计日志到数据库
async fn log_audit(
    db_pool: &PostgresPool,
    request: CreateAuditLogRequest,
) -> Result<(), crate::errors::AppError> {
    sqlx::query(
        r#"
        INSERT INTO audit_logs (
            id, timestamp, actor_type, actor_id, action, resource, 
            resource_id, ip_address, user_agent, status, details, request_id
        ) VALUES (
            gen_random_uuid(), NOW(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10
        )
        "#,
    )
    .bind(format!("{:?}", request.actor_type).to_lowercase())
    .bind(&request.actor_id)
    .bind(request.action.to_string())
    .bind(&request.resource)
    .bind(&request.resource_id)
    .bind(request.ip_address.to_string())
    .bind(&request.user_agent)
    .bind(format!("{:?}", request.status).to_lowercase())
    .bind(&request.details)
    .bind(&request.request_id)
    .execute(db_pool.pool())
    .await?;

    Ok(())
}
