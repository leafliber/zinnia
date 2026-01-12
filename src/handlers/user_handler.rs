//! 用户 API 处理器

use crate::errors::AppError;
use crate::middleware::AuthInfo;
use crate::models::{
    ApiResponse, ChangePasswordRequest, LoginRequest, RefreshTokenRequest,
    RegisterRequest, UpdateUserRequest, UserInfo, UserListQuery, UserRole,
    ShareDeviceRequest,
};
use crate::repositories::DeviceRepository;
use crate::services::UserService;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

// ========== 公开接口 ==========

/// 用户注册
pub async fn register(
    user_service: web::Data<Arc<UserService>>,
    body: web::Json<RegisterRequest>,
) -> Result<HttpResponse, AppError> {
    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let user_info = user_service.register(body.into_inner()).await?;

    Ok(HttpResponse::Created().json(ApiResponse::success(user_info)))
}

/// 用户登录
pub async fn login(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, AppError> {
    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    // 获取客户端 IP
    let ip_address = req
        .connection_info()
        .realip_remote_addr()
        .map(|s| s.to_string());

    let response = user_service
        .login(body.into_inner(), ip_address.as_deref())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// 刷新用户令牌
pub async fn user_refresh_token(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    body: web::Json<RefreshTokenRequest>,
) -> Result<HttpResponse, AppError> {
    let ip_address = req
        .connection_info()
        .realip_remote_addr()
        .map(|s| s.to_string());

    let response = user_service
        .refresh_token(&body.refresh_token, ip_address.as_deref())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// 用户登出
pub async fn user_logout(
    user_service: web::Data<Arc<UserService>>,
    body: web::Json<RefreshTokenRequest>,
) -> Result<HttpResponse, AppError> {
    user_service.logout(&body.refresh_token).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_message("已登出")))
}

// ========== 需要认证的接口 ==========

/// 获取当前用户信息
pub async fn get_me(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let user_info = user_service.get_current_user(user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(user_info)))
}

/// 更新当前用户信息
pub async fn update_me(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    body: web::Json<UpdateUserRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let user_id = extract_user_id(&req)?;
    let user_info = user_service.update_user(user_id, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(user_info)))
}

/// 修改密码
pub async fn change_password(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    body: web::Json<ChangePasswordRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let user_id = extract_user_id(&req)?;
    user_service.change_password(user_id, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_message("密码修改成功")))
}

/// 登出所有设备
pub async fn logout_all(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let count = user_service.logout_all(user_id).await?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "message": "已登出所有设备",
        "sessions_revoked": count
    }))))
}

// ========== 管理员接口 ==========

/// 获取用户列表（管理员）
pub async fn list_users(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    query: web::Query<UserListQuery>,
) -> Result<HttpResponse, AppError> {
    require_admin(&req)?;
    
    query.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let response = user_service.list_users(query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// 获取用户详情（管理员）
pub async fn get_user(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    require_admin(&req)?;
    
    let user_id = path.into_inner();
    let user = user_service.get_user_by_id(user_id).await?;
    let user_info: UserInfo = user.into();
    Ok(HttpResponse::Ok().json(ApiResponse::success(user_info)))
}

/// 更新用户（管理员）
pub async fn update_user(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateUserRequest>,
) -> Result<HttpResponse, AppError> {
    require_admin(&req)?;
    
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let user_id = path.into_inner();
    let user_info = user_service.update_user(user_id, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(user_info)))
}

/// 更新用户角色（管理员）
#[derive(serde::Deserialize)]
pub struct UpdateRoleRequest {
    pub role: UserRole,
}

pub async fn update_user_role(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateRoleRequest>,
) -> Result<HttpResponse, AppError> {
    require_admin(&req)?;
    
    let user_id = path.into_inner();
    let user_info = user_service.update_user_role(user_id, body.role.clone()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(user_info)))
}

/// 禁用/启用用户（管理员）
#[derive(serde::Deserialize)]
pub struct SetActiveRequest {
    pub is_active: bool,
}

pub async fn set_user_active(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    path: web::Path<Uuid>,
    body: web::Json<SetActiveRequest>,
) -> Result<HttpResponse, AppError> {
    require_admin(&req)?;
    
    let user_id = path.into_inner();
    user_service.set_user_active(user_id, body.is_active).await?;
    
    let message = if body.is_active { "用户已启用" } else { "用户已禁用" };
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_message(message)))
}

/// 删除用户（管理员）
pub async fn delete_user(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    require_admin(&req)?;
    
    let user_id = path.into_inner();
    user_service.delete_user(user_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

// ========== 设备共享接口 ==========

/// 共享设备
pub async fn share_device(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    device_repo: web::Data<Arc<DeviceRepository>>,
    path: web::Path<Uuid>,
    body: web::Json<ShareDeviceRequest>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let device_id = path.into_inner();
    
    // 验证用户是否有权限共享此设备（需要是设备所有者或管理员）
    verify_device_ownership(&req, device_id, &device_repo, user_id).await?;
    
    let share = user_service
        .share_device(device_id, &body.user_identifier, body.permission.clone())
        .await?;

    Ok(HttpResponse::Created().json(ApiResponse::success(share)))
}

/// 取消设备共享
pub async fn remove_device_share(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    device_repo: web::Data<Arc<DeviceRepository>>,
    path: web::Path<(Uuid, Uuid)>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let (device_id, target_user_id) = path.into_inner();
    
    // 验证用户是否有权限取消共享
    verify_device_ownership(&req, device_id, &device_repo, user_id).await?;
    
    user_service.unshare_device(device_id, target_user_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// 获取设备共享列表
pub async fn get_device_shares(
    req: HttpRequest,
    user_service: web::Data<Arc<UserService>>,
    device_repo: web::Data<Arc<DeviceRepository>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let user_id = extract_user_id(&req)?;
    let device_id = path.into_inner();
    
    // 验证用户是否有权限查看共享列表（所有者或被共享者或管理员）
    verify_device_access(&req, device_id, &device_repo, user_id).await?;
    
    let shares = user_service.get_device_shares(device_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(shares)))
}

// ========== 辅助函数 ==========

/// 从请求中提取用户 ID
fn extract_user_id(req: &HttpRequest) -> Result<Uuid, AppError> {
    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    // 用户 ID 存储在 actor_id 中
    Uuid::parse_str(&auth_info.actor_id)
        .map_err(|_| AppError::Unauthorized("无效的用户令牌".to_string()))
}

/// 检查是否是管理员
fn require_admin(req: &HttpRequest) -> Result<(), AppError> {
    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    match auth_info.role.as_deref() {
        Some("admin") => Ok(()),
        _ => Err(AppError::Forbidden("需要管理员权限".to_string())),
    }
}

/// 检查是否是管理员（不返回错误）
fn is_admin(req: &HttpRequest) -> bool {
    req.extensions()
        .get::<AuthInfo>()
        .map(|auth| auth.role.as_deref() == Some("admin"))
        .unwrap_or(false)
}

/// 验证用户是否是设备所有者（管理员也允许）
async fn verify_device_ownership(
    req: &HttpRequest,
    device_id: Uuid,
    device_repo: &DeviceRepository,
    user_id: Uuid,
) -> Result<(), AppError> {
    // 管理员可以操作任何设备
    if is_admin(req) {
        return Ok(());
    }
    
    // 检查设备是否存在
    let device = device_repo
        .find_by_id(device_id)
        .await?
        .ok_or_else(|| AppError::NotFound("设备不存在".to_string()))?;
    
    // 检查是否是设备所有者
    match device.owner_id {
        Some(owner_id) if owner_id == user_id => Ok(()),
        _ => Err(AppError::Forbidden("无权操作此设备".to_string())),
    }
}

/// 验证用户是否有权访问设备（所有者、被共享者或管理员）
async fn verify_device_access(
    req: &HttpRequest,
    device_id: Uuid,
    device_repo: &DeviceRepository,
    user_id: Uuid,
) -> Result<(), AppError> {
    // 管理员可以访问任何设备
    if is_admin(req) {
        return Ok(());
    }
    
    // 检查用户是否有访问权限（所有者或被共享者）
    let has_access = device_repo.user_can_access(device_id, user_id).await?;
    if has_access {
        return Ok(());
    }
    
    Err(AppError::Forbidden("无权访问此设备".to_string()))
}
