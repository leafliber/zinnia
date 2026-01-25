//! 电量相关 API 处理器

use crate::errors::AppError;
use crate::middleware::AuthInfo;
use crate::models::{
    ApiResponse, BatchBatteryReportRequest, BatteryAggregateRequest, BatteryQueryRequest,
    BatteryReportRequest,
};
use crate::repositories::DeviceRepository;
use crate::services::BatteryService;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

/// 上报电量数据
pub async fn report_battery(
    req: HttpRequest,
    battery_service: web::Data<Arc<BatteryService>>,
    body: web::Json<BatteryReportRequest>,
) -> Result<HttpResponse, AppError> {
    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    // 从认证信息获取设备 ID
    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let device_id = auth_info
        .device_id
        .ok_or_else(|| AppError::Unauthorized("无效的设备令牌".to_string()))?;

    // 上报数据
    let data = battery_service.report(device_id, body.into_inner()).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(data)))
}

/// 批量上报电量数据
pub async fn batch_report_battery(
    req: HttpRequest,
    battery_service: web::Data<Arc<BatteryService>>,
    body: web::Json<BatchBatteryReportRequest>,
) -> Result<HttpResponse, AppError> {
    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    // 从认证信息获取设备 ID
    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let device_id = auth_info
        .device_id
        .ok_or_else(|| AppError::Unauthorized("无效的设备令牌".to_string()))?;

    // 批量上报
    let count = battery_service
        .batch_report(device_id, body.into_inner().data)
        .await?;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "inserted_count": count
        }))),
    )
}

/// 获取最新电量
pub async fn get_latest_battery(
    req: HttpRequest,
    battery_service: web::Data<Arc<BatteryService>>,
    device_repo: web::Data<Arc<DeviceRepository>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    // 验证访问权限（用户只能访问自己绑定的设备）
    verify_device_access(&req, device_id, &device_repo).await?;

    let response = battery_service.get_latest(device_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// 查询历史数据
pub async fn get_battery_history(
    req: HttpRequest,
    battery_service: web::Data<Arc<BatteryService>>,
    device_repo: web::Data<Arc<DeviceRepository>>,
    path: web::Path<Uuid>,
    query: web::Query<BatteryQueryRequest>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    // 验证访问权限
    verify_device_access(&req, device_id, &device_repo).await?;

    // 验证请求
    query
        .validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let data = battery_service
        .get_history(device_id, query.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(data)))
}

/// 获取聚合统计
pub async fn get_battery_aggregated(
    req: HttpRequest,
    battery_service: web::Data<Arc<BatteryService>>,
    device_repo: web::Data<Arc<DeviceRepository>>,
    path: web::Path<Uuid>,
    query: web::Query<BatteryAggregateRequest>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    // 验证访问权限
    verify_device_access(&req, device_id, &device_repo).await?;

    let data = battery_service
        .get_aggregated(
            device_id,
            query.start_time,
            query.end_time,
            query.interval.clone(),
        )
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(data)))
}

/// 获取统计信息
pub async fn get_battery_stats(
    req: HttpRequest,
    battery_service: web::Data<Arc<BatteryService>>,
    device_repo: web::Data<Arc<DeviceRepository>>,
    path: web::Path<Uuid>,
    query: web::Query<BatteryAggregateRequest>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    // 验证访问权限
    verify_device_access(&req, device_id, &device_repo).await?;

    let stats = battery_service
        .get_stats(device_id, query.start_time, query.end_time)
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(stats)))
}

/// 验证设备访问权限
///
/// 检查顺序：
/// 1. 管理员 → 允许访问所有设备
/// 2. 设备 → 只能访问自己的数据
/// 3. 用户 → 只能访问自己拥有或被共享的设备
async fn verify_device_access(
    req: &HttpRequest,
    device_id: Uuid,
    device_repo: &DeviceRepository,
) -> Result<(), AppError> {
    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    // 管理员可以访问所有设备
    if auth_info.is_admin() {
        return Ok(());
    }

    // 设备只能访问自己的数据
    if let Some(auth_device_id) = auth_info.device_id {
        if auth_device_id == device_id {
            return Ok(());
        }
        return Err(AppError::Forbidden("无权访问此设备的数据".to_string()));
    }

    // 用户需要检查设备绑定关系
    if let Some(user_id) = auth_info.user_id {
        // 检查用户是否拥有该设备或被共享
        let has_access = device_repo.user_can_access(device_id, user_id).await?;
        if has_access {
            return Ok(());
        }
        return Err(AppError::Forbidden("无权访问此设备的数据".to_string()));
    }

    Err(AppError::Forbidden("无权访问此设备的数据".to_string()))
}
