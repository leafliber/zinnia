//! 预警 API 处理器

use crate::errors::AppError;
use crate::middleware::AuthInfo;
use crate::models::{
    AlertListQuery, ApiResponse, CreateAlertRuleRequest, UpdateAlertRuleRequest,
    UpdateAlertStatusRequest,
};
use crate::services::AlertService;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

/// 创建预警规则
pub async fn create_alert_rule(
    req: HttpRequest,
    alert_service: web::Data<Arc<AlertService>>,
    body: web::Json<CreateAlertRuleRequest>,
) -> Result<HttpResponse, AppError> {
    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    // 获取用户 ID
    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let user_id = auth_info
        .user_id
        .ok_or_else(|| AppError::Forbidden("仅限用户可创建预警规则".to_string()))?;

    let rule = alert_service
        .create_rule(user_id, body.into_inner())
        .await?;

    Ok(HttpResponse::Created().json(ApiResponse::created(rule)))
}

/// 获取用户的所有启用规则
pub async fn list_alert_rules(
    req: HttpRequest,
    alert_service: web::Data<Arc<AlertService>>,
) -> Result<HttpResponse, AppError> {
    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let user_id = auth_info
        .user_id
        .ok_or_else(|| AppError::Forbidden("仅限用户可查看预警规则".to_string()))?;

    let rules = alert_service.get_enabled_rules(user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(rules)))
}

/// 更新预警规则
pub async fn update_alert_rule(
    req: HttpRequest,
    alert_service: web::Data<Arc<AlertService>>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAlertRuleRequest>,
) -> Result<HttpResponse, AppError> {
    let rule_id = path.into_inner();

    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let user_id = auth_info
        .user_id
        .ok_or_else(|| AppError::Forbidden("仅限用户可更新预警规则".to_string()))?;

    let rule = alert_service
        .update_rule(rule_id, user_id, body.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(rule)))
}

/// 删除预警规则
pub async fn delete_alert_rule(
    req: HttpRequest,
    alert_service: web::Data<Arc<AlertService>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let rule_id = path.into_inner();

    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let user_id = auth_info
        .user_id
        .ok_or_else(|| AppError::Forbidden("仅限用户可删除预警规则".to_string()))?;

    alert_service.delete_rule(rule_id, user_id).await?;

    Ok(HttpResponse::NoContent().finish())
}

/// 查询预警事件列表（仅限用户设备）
pub async fn list_alert_events(
    req: HttpRequest,
    alert_service: web::Data<Arc<AlertService>>,
    query: web::Query<AlertListQuery>,
) -> Result<HttpResponse, AppError> {
    // 验证请求
    query
        .validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let user_id = auth_info
        .user_id
        .ok_or_else(|| AppError::Forbidden("仅限用户可查看预警事件".to_string()))?;

    let response = alert_service.list(user_id, query.into_inner()).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// 确认预警
pub async fn acknowledge_alert(
    req: HttpRequest,
    alert_service: web::Data<Arc<AlertService>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let event_id = path.into_inner();

    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let user_id = auth_info
        .user_id
        .ok_or_else(|| AppError::Forbidden("仅限用户可操作预警".to_string()))?;

    let event = alert_service.acknowledge(event_id, user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(event)))
}

/// 解决预警
pub async fn resolve_alert(
    req: HttpRequest,
    alert_service: web::Data<Arc<AlertService>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let event_id = path.into_inner();

    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let user_id = auth_info
        .user_id
        .ok_or_else(|| AppError::Forbidden("仅限用户可操作预警".to_string()))?;

    let event = alert_service.resolve(event_id, user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(event)))
}

/// 更新预警状态
pub async fn update_alert_status(
    req: HttpRequest,
    alert_service: web::Data<Arc<AlertService>>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAlertStatusRequest>,
) -> Result<HttpResponse, AppError> {
    let event_id = path.into_inner();

    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let user_id = auth_info
        .user_id
        .ok_or_else(|| AppError::Forbidden("仅限用户可操作预警".to_string()))?;

    let event = alert_service
        .update_status(event_id, user_id, body.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(event)))
}

/// 获取设备活跃预警数
pub async fn count_active_alerts(
    alert_service: web::Data<Arc<AlertService>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    let count = alert_service.count_active(device_id).await?;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "device_id": device_id,
            "active_alerts": count
        }))),
    )
}
