//! JWT 令牌处理

use crate::config::Settings;
use crate::errors::AppError;
use crate::security::Secrets;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT 令牌类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JwtTokenType {
    Access,
    Refresh,
}

/// JWT Claims（载荷）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// 主题（设备 ID 或用户 ID）
    pub sub: String,
    /// 令牌类型
    pub token_type: JwtTokenType,
    /// 签发者
    pub iss: String,
    /// 受众
    pub aud: String,
    /// 过期时间（Unix 时间戳）
    pub exp: i64,
    /// 签发时间
    pub iat: i64,
    /// 令牌 ID（用于吊销）
    pub jti: String,
    /// 设备 ID（如果是设备令牌）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<Uuid>,
    /// 角色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// JWT 管理器
pub struct JwtManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    issuer: String,
    audience: String,
    access_expiry_seconds: i64,
    refresh_expiry_days: i64,
}

impl JwtManager {
    /// 创建 JWT 管理器
    pub fn new(settings: &Settings) -> Result<Self, AppError> {
        let secrets = Secrets::get()?;
        let secret = secrets.jwt_secret().expose_secret().as_bytes();

        Ok(Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            issuer: settings.jwt.issuer.clone(),
            audience: settings.jwt.audience.clone(),
            access_expiry_seconds: settings.jwt.expiry_seconds as i64,
            refresh_expiry_days: settings.jwt.refresh_expiry_days as i64,
        })
    }

    /// 生成访问令牌
    pub fn generate_access_token(
        &self,
        subject: &str,
        device_id: Option<Uuid>,
        role: Option<String>,
    ) -> Result<String, AppError> {
        self.generate_token(subject, JwtTokenType::Access, device_id, role)
    }

    /// 生成刷新令牌
    pub fn generate_refresh_token(
        &self,
        subject: &str,
        device_id: Option<Uuid>,
    ) -> Result<String, AppError> {
        self.generate_token(subject, JwtTokenType::Refresh, device_id, None)
    }

    /// 生成令牌
    fn generate_token(
        &self,
        subject: &str,
        token_type: JwtTokenType,
        device_id: Option<Uuid>,
        role: Option<String>,
    ) -> Result<String, AppError> {
        let now = Utc::now();
        let expiry = match token_type {
            JwtTokenType::Access => now + Duration::seconds(self.access_expiry_seconds),
            JwtTokenType::Refresh => now + Duration::days(self.refresh_expiry_days),
        };

        let claims = Claims {
            sub: subject.to_string(),
            token_type,
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            exp: expiry.timestamp(),
            iat: now.timestamp(),
            jti: Uuid::new_v4().to_string(),
            device_id,
            role,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| AppError::InternalError(format!("令牌生成失败: {}", e)))
    }

    /// 验证令牌
    pub fn validate_token(&self, token: &str) -> Result<Claims, AppError> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);

        let token_data: TokenData<Claims> = decode(token, &self.decoding_key, &validation)
            .map_err(|e| {
                tracing::debug!("令牌验证失败: {}", e);
                AppError::Unauthorized("无效的令牌".to_string())
            })?;

        Ok(token_data.claims)
    }

    /// 验证访问令牌
    pub fn validate_access_token(&self, token: &str) -> Result<Claims, AppError> {
        let claims = self.validate_token(token)?;
        
        if claims.token_type != JwtTokenType::Access {
            return Err(AppError::Unauthorized("令牌类型错误".to_string()));
        }

        Ok(claims)
    }

    /// 验证刷新令牌
    pub fn validate_refresh_token(&self, token: &str) -> Result<Claims, AppError> {
        let claims = self.validate_token(token)?;
        
        if claims.token_type != JwtTokenType::Refresh {
            return Err(AppError::Unauthorized("令牌类型错误".to_string()));
        }

        Ok(claims)
    }

    /// 获取令牌 ID（用于黑名单）
    pub fn get_token_id(&self, token: &str) -> Result<String, AppError> {
        let claims = self.validate_token(token)?;
        Ok(claims.jti)
    }
}

/// 令牌对
#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

impl TokenPair {
    pub fn new(access_token: String, refresh_token: String, expires_in: i64) -> Self {
        Self {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in,
        }
    }
}
