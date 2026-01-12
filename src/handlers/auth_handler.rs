//! 认证 API 处理器

use crate::errors::AppError;
use crate::models::ApiResponse;
use crate::services::AuthService;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::sync::Arc;

/// 认证请求（使用 API Key）
#[derive(Debug, Deserialize)]
pub struct AuthRequest {
    pub api_key: String,
}

/// 刷新 Token 请求
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// 吊销 Token 请求
#[derive(Debug, Deserialize)]
pub struct RevokeRequest {
    pub token: String,
}

/// 使用 API Key 获取 Token
pub async fn authenticate(
    auth_service: web::Data<Arc<AuthService>>,
    body: web::Json<AuthRequest>,
) -> Result<HttpResponse, AppError> {
    let token_pair = auth_service
        .authenticate_device(&body.api_key)
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(token_pair)))
}

/// 刷新 Token
pub async fn refresh_token(
    auth_service: web::Data<Arc<AuthService>>,
    body: web::Json<RefreshRequest>,
) -> Result<HttpResponse, AppError> {
    let token_pair = auth_service
        .refresh_token(&body.refresh_token)
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(token_pair)))
}

/// 吊销 Token
pub async fn revoke_token(
    auth_service: web::Data<Arc<AuthService>>,
    body: web::Json<RevokeRequest>,
) -> Result<HttpResponse, AppError> {
    auth_service.revoke_token(&body.token).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_message("Token 已吊销")))
}

/// 从请求头获取 Token 并吊销（用于登出）
pub async fn logout(
    req: HttpRequest,
    auth_service: web::Data<Arc<AuthService>>,
) -> Result<HttpResponse, AppError> {
    // 从 Authorization 头提取 Token
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("缺少认证令牌".to_string()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Unauthorized("无效的认证格式".to_string()))?;

    auth_service.revoke_token(token).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_message("已登出")))
}
