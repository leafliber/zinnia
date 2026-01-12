//! 验证相关 API 处理器
//!
//! 提供验证码发送、reCAPTCHA 配置等接口

use crate::errors::AppError;
use crate::models::{
    ApiResponse, SendVerificationCodeRequest, VerifyCodeRequest, VerificationCodeResponse,
};
use crate::services::{
    RecaptchaService, RegistrationSecurityService, VerificationCodeType, VerificationService,
};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use validator::Validate;

/// reCAPTCHA 配置响应
#[derive(Debug, Serialize)]
pub struct RecaptchaConfigResponse {
    /// 是否启用 reCAPTCHA
    pub enabled: bool,
    /// 站点密钥（供前端使用）
    pub site_key: Option<String>,
}

/// 注册安全配置响应
#[derive(Debug, Serialize)]
pub struct RegistrationSecurityConfigResponse {
    /// 是否需要邮箱验证
    pub require_email_verification: bool,
    /// 是否需要 reCAPTCHA
    pub require_recaptcha: bool,
    /// reCAPTCHA 站点密钥
    pub recaptcha_site_key: Option<String>,
}

/// 获取客户端 IP
fn get_client_ip(req: &HttpRequest) -> Option<String> {
    // 尝试从 X-Forwarded-For 获取
    if let Some(forwarded) = req.headers().get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            return forwarded_str.split(',').next().map(|s| s.trim().to_string());
        }
    }
    
    // 尝试从 X-Real-IP 获取
    if let Some(real_ip) = req.headers().get("X-Real-IP") {
        if let Ok(ip) = real_ip.to_str() {
            return Some(ip.to_string());
        }
    }
    
    // 从连接信息获取
    req.peer_addr().map(|addr| addr.ip().to_string())
}

/// 获取 reCAPTCHA 配置
/// GET /api/v1/auth/recaptcha/config
pub async fn get_recaptcha_config(
    recaptcha_service: web::Data<Arc<RecaptchaService>>,
) -> Result<HttpResponse, AppError> {
    let response = RecaptchaConfigResponse {
        enabled: recaptcha_service.is_enabled(),
        site_key: recaptcha_service.get_site_key().map(String::from),
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// 获取注册安全配置
/// GET /api/v1/auth/registration/config
pub async fn get_registration_security_config(
    reg_security: web::Data<Arc<RegistrationSecurityService>>,
    recaptcha_service: web::Data<Arc<RecaptchaService>>,
) -> Result<HttpResponse, AppError> {
    let response = RegistrationSecurityConfigResponse {
        require_email_verification: reg_security.require_email_verification(),
        require_recaptcha: reg_security.require_recaptcha() && recaptcha_service.is_enabled(),
        recaptcha_site_key: recaptcha_service.get_site_key().map(String::from),
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// 发送注册验证码
/// POST /api/v1/auth/verification/send
pub async fn send_verification_code(
    req: HttpRequest,
    verification_service: web::Data<Arc<VerificationService>>,
    recaptcha_service: web::Data<Arc<RecaptchaService>>,
    reg_security: web::Data<Arc<RegistrationSecurityService>>,
    body: web::Json<SendVerificationCodeRequest>,
) -> Result<HttpResponse, AppError> {
    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let client_ip = get_client_ip(&req);
    
    // 检查 IP 限制
    if let Some(ref ip) = client_ip {
        let check = reg_security.check_ip(ip).await?;
        if !check.allowed {
            return Err(AppError::RateLimitExceeded(
                check.reason.unwrap_or_else(|| "请求过于频繁".to_string())
            ));
        }
    }

    // 验证 reCAPTCHA（如果启用）
    if reg_security.require_recaptcha() && recaptcha_service.is_enabled() {
        let token = body.recaptcha_token.as_deref()
            .ok_or_else(|| AppError::ValidationError("请完成人机验证".to_string()))?;
        
        recaptcha_service.verify(token, client_ip.as_deref()).await?;
    }

    // 发送验证码
    verification_service
        .send_code(&body.email, VerificationCodeType::EmailVerification)
        .await?;

    let response = VerificationCodeResponse {
        message: "验证码已发送到您的邮箱".to_string(),
        expires_in_minutes: verification_service.get_expiry_minutes(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// 验证验证码（不完成注册，仅验证）
/// POST /api/v1/auth/verification/verify
pub async fn verify_code(
    verification_service: web::Data<Arc<VerificationService>>,
    body: web::Json<VerifyCodeRequest>,
) -> Result<HttpResponse, AppError> {
    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    // 这里只是检查验证码是否正确，但不消耗它
    // 实际的消耗会在注册时进行
    verification_service
        .verify_code(&body.email, &body.code, VerificationCodeType::EmailVerification)
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_message("验证码正确")))
}

/// 发送密码重置验证码
/// POST /api/v1/auth/password-reset/send
pub async fn send_password_reset_code(
    req: HttpRequest,
    verification_service: web::Data<Arc<VerificationService>>,
    recaptcha_service: web::Data<Arc<RecaptchaService>>,
    body: web::Json<SendVerificationCodeRequest>,
) -> Result<HttpResponse, AppError> {
    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let client_ip = get_client_ip(&req);

    // 验证 reCAPTCHA（如果提供）
    if let Some(ref token) = body.recaptcha_token {
        if recaptcha_service.is_enabled() {
            recaptcha_service.verify(token, client_ip.as_deref()).await?;
        }
    }

    // 发送验证码
    verification_service
        .send_code(&body.email, VerificationCodeType::PasswordReset)
        .await?;

    let response = VerificationCodeResponse {
        message: "验证码已发送到您的邮箱".to_string(),
        expires_in_minutes: verification_service.get_expiry_minutes(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// 密码重置请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct PasswordResetRequest {
    #[validate(email(message = "邮箱格式无效"))]
    pub email: String,
    
    #[validate(length(equal = 6, message = "验证码应为6位数字"))]
    pub code: String,
    
    #[validate(length(min = 8, max = 128, message = "密码长度应在 8-128 字符之间"))]
    pub new_password: String,
    
    #[validate(length(min = 8, max = 128, message = "密码长度应在 8-128 字符之间"))]
    pub confirm_password: String,
}

/// 重置密码
/// POST /api/v1/auth/password-reset/confirm
pub async fn confirm_password_reset(
    verification_service: web::Data<Arc<VerificationService>>,
    user_service: web::Data<Arc<crate::services::UserService>>,
    body: web::Json<PasswordResetRequest>,
) -> Result<HttpResponse, AppError> {
    // 验证请求
    body.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    // 检查密码是否一致
    if body.new_password != body.confirm_password {
        return Err(AppError::ValidationError("两次输入的密码不一致".to_string()));
    }

    // 验证验证码
    verification_service
        .verify_code(&body.email, &body.code, VerificationCodeType::PasswordReset)
        .await?;

    // 重置密码
    user_service
        .reset_password_by_email(&body.email, &body.new_password)
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_message("密码已重置，请使用新密码登录")))
}
