//! 设备访问令牌管理处理器

use crate::errors::AppError;
use crate::middleware::AuthInfo;
use crate::models::{ApiResponse, CreateAccessTokenRequest, RevokeAllTokensRequest};
use crate::services::DeviceAccessTokenService;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

/// 令牌列表查询参数
#[derive(Debug, Deserialize)]
pub struct ListTokensQuery {
    /// 是否包含已吊销的令牌
    #[serde(default)]
    pub include_revoked: bool,
    /// 是否包含已过期的令牌
    #[serde(default)]
    pub include_expired: bool,
}

/// 创建访问令牌
/// POST /api/v1/devices/{device_id}/tokens
pub async fn create_device_token(
    token_service: web::Data<Arc<DeviceAccessTokenService>>,
    path: web::Path<Uuid>,
    body: web::Json<CreateAccessTokenRequest>,
    auth: web::ReqData<AuthInfo>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    // 需要用户认证
    let user_id = auth
        .user_id
        .ok_or_else(|| AppError::Unauthorized("需要用户认证".to_string()))?;

    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let response = token_service
        .create_token(device_id, user_id, body.into_inner())
        .await?;

    Ok(HttpResponse::Created().json(ApiResponse::created(response)))
}

/// 列出设备的所有令牌
/// GET /api/v1/devices/{device_id}/tokens
pub async fn list_device_tokens(
    token_service: web::Data<Arc<DeviceAccessTokenService>>,
    path: web::Path<Uuid>,
    query: web::Query<ListTokensQuery>,
    auth: web::ReqData<AuthInfo>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    // 需要用户认证
    let user_id = auth
        .user_id
        .ok_or_else(|| AppError::Unauthorized("需要用户认证".to_string()))?;

    let tokens = token_service
        .list_tokens(
            device_id,
            user_id,
            query.include_revoked,
            query.include_expired,
        )
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(tokens)))
}

/// 吊销单个令牌
/// DELETE /api/v1/devices/{device_id}/tokens/{token_id}
pub async fn revoke_device_token(
    token_service: web::Data<Arc<DeviceAccessTokenService>>,
    path: web::Path<(Uuid, Uuid)>,
    auth: web::ReqData<AuthInfo>,
) -> Result<HttpResponse, AppError> {
    let (_device_id, token_id) = path.into_inner();

    // 需要用户认证
    let user_id = auth
        .user_id
        .ok_or_else(|| AppError::Unauthorized("需要用户认证".to_string()))?;

    token_service.revoke_token(token_id, user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_message("令牌已吊销")))
}

/// 吊销设备的所有令牌
/// DELETE /api/v1/devices/{device_id}/tokens
pub async fn revoke_all_device_tokens(
    token_service: web::Data<Arc<DeviceAccessTokenService>>,
    path: web::Path<Uuid>,
    _body: Option<web::Json<RevokeAllTokensRequest>>,
    auth: web::ReqData<AuthInfo>,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();

    // 需要用户认证
    let user_id = auth
        .user_id
        .ok_or_else(|| AppError::Unauthorized("需要用户认证".to_string()))?;

    let count = token_service.revoke_all_tokens(device_id, user_id).await?;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "revoked_count": count,
            "message": format!("已吊销 {} 个令牌", count)
        }))),
    )
}
