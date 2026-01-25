//! 通知偏好 API 处理器

use crate::errors::AppError;
use crate::middleware::AuthInfo;
use crate::models::{
    ApiResponse, NotificationPreferenceResponse, SubscribeWebPushRequest,
    UpdateNotificationPreferenceRequest, WebPushSubscriptionResponse,
};
use crate::services::{NotificationService, WebPushService};
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use std::sync::Arc;
use validator::Validate;

/// 获取当前用户的通知偏好
pub async fn get_notification_preference(
    req: HttpRequest,
    notification_service: web::Data<Arc<NotificationService>>,
) -> Result<HttpResponse, AppError> {
    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let user_id = auth_info
        .user_id
        .ok_or_else(|| AppError::Forbidden("仅限用户可访问".to_string()))?;

    let preference = notification_service
        .get_user_preference(user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("通知偏好未配置".to_string()))?;

    let response = NotificationPreferenceResponse::from(preference);

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// 更新当前用户的通知偏好
pub async fn update_notification_preference(
    req: HttpRequest,
    notification_service: web::Data<Arc<NotificationService>>,
    body: web::Json<UpdateNotificationPreferenceRequest>,
) -> Result<HttpResponse, AppError> {
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
        .ok_or_else(|| AppError::Forbidden("仅限用户可访问".to_string()))?;

    let preference = notification_service
        .update_user_preference(user_id, body.into_inner())
        .await?;

    let response = NotificationPreferenceResponse::from(preference);

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

// ========== Web Push 订阅管理 ==========

/// 获取 VAPID 公钥
pub async fn get_vapid_public_key(
    web_push_service: web::Data<Option<Arc<WebPushService>>>,
) -> Result<HttpResponse, AppError> {
    let service = web_push_service
        .as_ref()
        .as_ref()
        .ok_or_else(|| AppError::ConfigError("Web Push 服务未配置".to_string()))?;

    let public_key = service.get_vapid_public_key();

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "public_key": public_key
        }))),
    )
}

/// 订阅 Web Push
pub async fn subscribe_web_push(
    req: HttpRequest,
    notification_service: web::Data<Arc<NotificationService>>,
    body: web::Json<SubscribeWebPushRequest>,
) -> Result<HttpResponse, AppError> {
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
        .ok_or_else(|| AppError::Forbidden("仅限用户可访问".to_string()))?;

    // 获取 User-Agent
    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let subscription = notification_service
        .subscribe_web_push(user_id, body.into_inner(), user_agent.as_deref())
        .await?;

    let response = WebPushSubscriptionResponse {
        id: subscription.id,
        endpoint: subscription.endpoint,
        device_name: subscription.device_name,
        is_active: subscription.is_active,
        created_at: subscription.created_at,
    };

    Ok(HttpResponse::Created().json(ApiResponse::created(response)))
}

/// 获取当前用户的所有订阅
pub async fn list_web_push_subscriptions(
    req: HttpRequest,
    notification_service: web::Data<Arc<NotificationService>>,
) -> Result<HttpResponse, AppError> {
    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let user_id = auth_info
        .user_id
        .ok_or_else(|| AppError::Forbidden("仅限用户可访问".to_string()))?;

    let subscriptions = notification_service
        .get_web_push_subscriptions(user_id)
        .await?;

    let responses: Vec<WebPushSubscriptionResponse> = subscriptions
        .into_iter()
        .map(|sub| WebPushSubscriptionResponse {
            id: sub.id,
            endpoint: sub.endpoint,
            device_name: sub.device_name,
            is_active: sub.is_active,
            created_at: sub.created_at,
        })
        .collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(responses)))
}

/// 删除订阅
pub async fn unsubscribe_web_push(
    req: HttpRequest,
    notification_service: web::Data<Arc<NotificationService>>,
    path: web::Path<uuid::Uuid>,
) -> Result<HttpResponse, AppError> {
    let subscription_id = path.into_inner();

    let auth_info = req
        .extensions()
        .get::<AuthInfo>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("未认证".to_string()))?;

    let user_id = auth_info
        .user_id
        .ok_or_else(|| AppError::Forbidden("仅限用户可访问".to_string()))?;

    notification_service
        .delete_web_push_subscription(user_id, subscription_id)
        .await?;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "message": "订阅已删除"
        }))),
    )
}
