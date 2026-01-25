//! 认证 API 处理器

use crate::errors::AppError;
use crate::models::ApiResponse;
use crate::services::AuthService;
use crate::utils::{
    clear_auth_cookies, extract_access_token, extract_refresh_token, set_auth_cookies,
};
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
/// 同时支持返回 JSON 和设置 httponly cookie 两种方式
pub async fn authenticate(
    auth_service: web::Data<Arc<AuthService>>,
    body: web::Json<AuthRequest>,
) -> Result<HttpResponse, AppError> {
    let token_pair = auth_service.authenticate_device(&body.api_key).await?;

    // 设置 httpOnly cookie
    let res = HttpResponse::Ok().json(ApiResponse::success(token_pair.clone()));
    let res = set_auth_cookies(res, &token_pair.access_token, &token_pair.refresh_token);

    Ok(res)
}

/// 刷新 Token
/// 支持从请求体或 cookie 中获取 refresh token
pub async fn refresh_token(
    req: HttpRequest,
    auth_service: web::Data<Arc<AuthService>>,
    body: Option<web::Json<RefreshRequest>>, // 可以从请求体获取，也可以从 cookie 获取
) -> Result<HttpResponse, AppError> {
    // 优先使用请求体中的 refresh_token，如果未提供则从 cookie 获取
    let refresh_token = match body {
        Some(b) => b.refresh_token.clone(),
        None => extract_refresh_token(&req)
            .ok_or_else(|| AppError::ValidationError("缺少刷新令牌".to_string()))?,
    };

    let token_pair = auth_service.refresh_token(&refresh_token).await?;

    // 更新 httpOnly cookie
    let res = HttpResponse::Ok().json(ApiResponse::success(token_pair.clone()));
    let res = set_auth_cookies(res, &token_pair.access_token, &token_pair.refresh_token);

    Ok(res)
}

/// 吊销 Token
pub async fn revoke_token(
    auth_service: web::Data<Arc<AuthService>>,
    body: web::Json<RevokeRequest>,
) -> Result<HttpResponse, AppError> {
    auth_service.revoke_token(&body.token).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_message("Token 已吊销")))
}

/// 从请求头或 cookie 获取 Token 并吊销（用于登出）
pub async fn logout(
    req: HttpRequest,
    auth_service: web::Data<Arc<AuthService>>,
) -> Result<HttpResponse, AppError> {
    // 先尝试从 Authorization header 提取 Token
    let token_opt = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer ").map(|t| t.to_string()));

    // 如果 header 中没有，从 cookie 中提取 access token
    let token = token_opt
        .or_else(|| extract_access_token(&req))
        .ok_or_else(|| AppError::Unauthorized("缺少认证令牌".to_string()))?;

    auth_service.revoke_token(&token).await?;

    // 清除 httpOnly cookie
    let res = HttpResponse::Ok().json(ApiResponse::<()>::success_message("已登出"));
    let res = clear_auth_cookies(res);

    Ok(res)
}
