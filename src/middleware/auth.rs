//! 认证中间件

use crate::db::RedisPool;
use crate::errors::AppError;
use crate::security::{JwtManager, mask_token};
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::AUTHORIZATION,
    Error, HttpMessage,
};
use futures::future::{ok, LocalBoxFuture, Ready};
use std::rc::Rc;
use std::sync::Arc;
use uuid::Uuid;

/// 认证信息（存储在请求扩展中）
#[derive(Debug, Clone)]
pub struct AuthInfo {
    /// 主体 ID（用户 ID 或设备 ID 的字符串形式）
    pub actor_id: String,
    /// 用户 ID（如果是用户认证）
    pub user_id: Option<Uuid>,
    /// 设备 ID（如果是设备认证）
    pub device_id: Option<Uuid>,
    /// 角色：admin, user, readonly, device
    pub role: Option<String>,
    /// 认证类型
    pub auth_type: AuthType,
}

impl AuthInfo {
    /// 检查是否是管理员
    pub fn is_admin(&self) -> bool {
        self.role.as_deref() == Some("admin")
    }
    
    /// 检查是否是用户（包括管理员）
    pub fn is_user(&self) -> bool {
        matches!(self.role.as_deref(), Some("admin") | Some("user") | Some("readonly"))
    }
    
    /// 检查是否是设备
    pub fn is_device(&self) -> bool {
        self.role.as_deref() == Some("device")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuthType {
    Jwt,
    ApiKey,
}

/// JWT 认证中间件
#[derive(Clone)]
pub struct JwtAuth {
    jwt_manager: Arc<JwtManager>,
    redis_pool: Arc<RedisPool>,
}

impl JwtAuth {
    pub fn new(jwt_manager: Arc<JwtManager>, redis_pool: Arc<RedisPool>) -> Self {
        Self { jwt_manager, redis_pool }
    }
}

impl<S, B> Transform<S, ServiceRequest> for JwtAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = JwtAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(JwtAuthMiddleware {
            service: Rc::new(service),
            jwt_manager: self.jwt_manager.clone(),
            redis_pool: self.redis_pool.clone(),
        })
    }
}

pub struct JwtAuthMiddleware<S> {
    service: Rc<S>,
    jwt_manager: Arc<JwtManager>,
    redis_pool: Arc<RedisPool>,
}

impl<S, B> Service<ServiceRequest> for JwtAuthMiddleware<S>
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
        let jwt_manager = self.jwt_manager.clone();
        let redis_pool = self.redis_pool.clone();

        Box::pin(async move {
            // 提取 Authorization 头
            let auth_header = req
                .headers()
                .get(AUTHORIZATION)
                .and_then(|h| h.to_str().ok());

            let token = match auth_header {
                Some(header) if header.starts_with("Bearer ") => &header[7..],
                _ => {
                    return Err(AppError::Unauthorized("缺少认证令牌".to_string()).into());
                }
            };

            // 验证 JWT
            let claims = jwt_manager.validate_access_token(token)?;

            // 检查令牌是否在黑名单中
            let blacklist_key = format!("token:blacklist:{}", claims.jti);
            let is_blacklisted: Option<String> = redis_pool.get(&blacklist_key).await?;
            if is_blacklisted.is_some() {
                return Err(AppError::Unauthorized("令牌已被吊销".to_string()).into());
            }

            // 解析用户 ID（如果是用户令牌）
            let user_id = if claims.device_id.is_none() {
                // 没有 device_id 说明是用户令牌，sub 是用户 ID
                Uuid::parse_str(&claims.sub).ok()
            } else {
                None
            };

            // 将认证信息存入请求扩展
            let auth_info = AuthInfo {
                actor_id: claims.sub.clone(),
                user_id,
                device_id: claims.device_id,
                role: claims.role.clone(),
                auth_type: AuthType::Jwt,
            };
            req.extensions_mut().insert(auth_info);

            service.call(req).await
        })
    }
}

/// API Key 认证中间件
#[derive(Clone)]
pub struct ApiKeyAuth {
    device_service: Arc<crate::services::DeviceService>,
}

impl ApiKeyAuth {
    pub fn new(device_service: Arc<crate::services::DeviceService>) -> Self {
        Self { device_service }
    }
}

impl<S, B> Transform<S, ServiceRequest> for ApiKeyAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = ApiKeyAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(ApiKeyAuthMiddleware {
            service: Rc::new(service),
            device_service: self.device_service.clone(),
        })
    }
}

pub struct ApiKeyAuthMiddleware<S> {
    service: Rc<S>,
    device_service: Arc<crate::services::DeviceService>,
}

impl<S, B> Service<ServiceRequest> for ApiKeyAuthMiddleware<S>
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
        let device_service = self.device_service.clone();

        Box::pin(async move {
            // 提取 X-API-Key 头
            let api_key = req
                .headers()
                .get("X-API-Key")
                .and_then(|h| h.to_str().ok());

            let key = match api_key {
                Some(k) => k,
                None => {
                    return Err(AppError::Unauthorized("缺少 API Key".to_string()).into());
                }
            };

            // 记录日志（脱敏）
            tracing::debug!(api_key = %mask_token(key), "API Key 认证请求");

            // 验证 API Key 并获取设备信息
            let device = device_service.verify_by_api_key(key).await?;

            // 将认证信息存入请求扩展
            let auth_info = AuthInfo {
                actor_id: device.id.to_string(),
                user_id: None,
                device_id: Some(device.id),
                role: Some("device".to_string()),
                auth_type: AuthType::ApiKey,
            };
            req.extensions_mut().insert(auth_info);

            service.call(req).await
        })
    }
}

/// API Key 值包装（备用，用于需要原始 API Key 的场景）
#[derive(Debug, Clone)]
pub struct ApiKeyValue(pub String);

/// 从请求中提取认证信息
pub fn get_auth_info(req: &ServiceRequest) -> Option<AuthInfo> {
    req.extensions().get::<AuthInfo>().cloned()
}

/// JWT 或 API Key 认证中间件（支持两种认证方式）
#[derive(Clone)]
pub struct JwtOrApiKeyAuth {
    jwt_manager: Arc<JwtManager>,
    redis_pool: Arc<RedisPool>,
    device_service: Arc<crate::services::DeviceService>,
}

impl JwtOrApiKeyAuth {
    pub fn new(
        jwt_manager: Arc<JwtManager>,
        redis_pool: Arc<RedisPool>,
        device_service: Arc<crate::services::DeviceService>,
    ) -> Self {
        Self {
            jwt_manager,
            redis_pool,
            device_service,
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for JwtOrApiKeyAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = JwtOrApiKeyAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(JwtOrApiKeyAuthMiddleware {
            service: Rc::new(service),
            jwt_manager: self.jwt_manager.clone(),
            redis_pool: self.redis_pool.clone(),
            device_service: self.device_service.clone(),
        })
    }
}

pub struct JwtOrApiKeyAuthMiddleware<S> {
    service: Rc<S>,
    jwt_manager: Arc<JwtManager>,
    redis_pool: Arc<RedisPool>,
    device_service: Arc<crate::services::DeviceService>,
}

impl<S, B> Service<ServiceRequest> for JwtOrApiKeyAuthMiddleware<S>
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
        let jwt_manager = self.jwt_manager.clone();
        let redis_pool = self.redis_pool.clone();
        let device_service = self.device_service.clone();

        Box::pin(async move {
            // 尝试 JWT 认证
            if let Some(auth_header) = req.headers().get(AUTHORIZATION).and_then(|h| h.to_str().ok()) {
                if let Some(token) = auth_header.strip_prefix("Bearer ") {
                    
                    // 验证 JWT
                    if let Ok(claims) = jwt_manager.validate_access_token(token) {
                        // 检查令牌是否在黑名单中
                        let blacklist_key = format!("token:blacklist:{}", claims.jti);
                        let is_blacklisted: Option<String> = redis_pool.get(&blacklist_key).await?;
                        
                        if is_blacklisted.is_none() {
                            // 解析用户 ID
                            let user_id = if claims.device_id.is_none() {
                                Uuid::parse_str(&claims.sub).ok()
                            } else {
                                None
                            };
                            
                            // JWT 认证成功
                            let auth_info = AuthInfo {
                                actor_id: claims.sub.clone(),
                                user_id,
                                device_id: claims.device_id,
                                role: claims.role.clone(),
                                auth_type: AuthType::Jwt,
                            };
                            req.extensions_mut().insert(auth_info);
                            return service.call(req).await;
                        }
                    }
                }
            }
            
            // 尝试 API Key 认证
            if let Some(api_key) = req.headers().get("X-API-Key").and_then(|h| h.to_str().ok()) {
                tracing::debug!(api_key = %mask_token(api_key), "尝试 API Key 认证");
                
                // 验证 API Key
                if let Ok(device) = device_service.verify_by_api_key(api_key).await {
                    let auth_info = AuthInfo {
                        actor_id: device.id.to_string(),
                        user_id: None,
                        device_id: Some(device.id),
                        role: Some("device".to_string()),
                        auth_type: AuthType::ApiKey,
                    };
                    req.extensions_mut().insert(auth_info);
                    return service.call(req).await;
                }
            }
            
            // 两种认证都失败
            Err(AppError::Unauthorized("需要 JWT 令牌或 API Key 认证".to_string()).into())
        })
    }
}
