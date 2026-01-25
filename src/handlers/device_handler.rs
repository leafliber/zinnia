//! 设备管理 API 处理器

use crate::errors::AppError;
use crate::middleware::AuthInfo;
use crate::models::{
    ApiResponse, CreateDeviceRequest, DeviceListQuery, UpdateDeviceConfigRequest,
    UpdateDeviceRequest,
};
use crate::services::DeviceService;
use actix_web::{web, HttpResponse};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

/// 注册设备
pub async fn create_device(
    device_service: web::Data<Arc<DeviceService>>,
    body: web::Json<CreateDeviceRequest>,
    auth: web::ReqData<AuthInfo>,
) -> Result<HttpResponse, AppError> {
    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    // 强制要求用户认证，设备必须绑定到用户
    let owner_id = auth
        .user_id
        .ok_or_else(|| AppError::Unauthorized("创建设备需要用户认证".to_string()))?;

    let response = device_service
        .register(body.into_inner(), Some(owner_id))
        .await?;

    // 返回 201 Created
    Ok(HttpResponse::Created().json(ApiResponse::created(response)))
}

/// 获取设备详情
pub async fn get_device(
    device_service: web::Data<Arc<DeviceService>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    let device = device_service.get_by_id(device_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(device)))
}

/// 更新设备
pub async fn update_device(
    device_service: web::Data<Arc<DeviceService>>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateDeviceRequest>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let device = device_service.update(device_id, body.into_inner()).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(device)))
}

/// 删除设备
pub async fn delete_device(
    device_service: web::Data<Arc<DeviceService>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    device_service.delete(device_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_message("设备已删除")))
}

/// 获取设备列表
pub async fn list_devices(
    device_service: web::Data<Arc<DeviceService>>,
    query: web::Query<DeviceListQuery>,
) -> Result<HttpResponse, AppError> {
    // 验证请求
    query
        .validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let response = device_service.list(query.into_inner()).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// 获取设备配置
pub async fn get_device_config(
    device_service: web::Data<Arc<DeviceService>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    let config = device_service.get_config(device_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(config)))
}

/// 更新设备配置
pub async fn update_device_config(
    device_service: web::Data<Arc<DeviceService>>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateDeviceConfigRequest>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let config = device_service
        .update_config(device_id, body.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(config)))
}

/// 轮换设备 API Key
pub async fn rotate_device_api_key(
    device_service: web::Data<Arc<DeviceService>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    let new_api_key = device_service.rotate_api_key(device_id).await?;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "api_key": new_api_key,
            "message": "API Key 已更新，请妥善保管新的 API Key"
        }))),
    )
}
