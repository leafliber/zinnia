//! 验证码服务模块
//! 
//! 管理邮箱验证码的生成、存储和验证

use crate::config::Settings;
use crate::db::RedisPool;
use crate::errors::AppError;
use crate::services::EmailService;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 验证码类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationCodeType {
    /// 邮箱验证（注册）
    EmailVerification,
    /// 密码重置
    PasswordReset,
    /// 登录二次验证
    LoginVerification,
}

impl VerificationCodeType {
    fn redis_prefix(&self) -> &'static str {
        match self {
            VerificationCodeType::EmailVerification => "verify:email",
            VerificationCodeType::PasswordReset => "verify:password",
            VerificationCodeType::LoginVerification => "verify:login",
        }
    }
}

/// 存储的验证码信息
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredCode {
    code: String,
    attempts: u32,
    email: String,
}

/// 验证码服务
pub struct VerificationService {
    redis_pool: Arc<RedisPool>,
    email_service: Arc<EmailService>,
    code_expiry_seconds: u64,
}

impl VerificationService {
    pub fn new(
        redis_pool: Arc<RedisPool>,
        email_service: Arc<EmailService>,
        settings: &Settings,
    ) -> Self {
        Self {
            redis_pool,
            email_service,
            code_expiry_seconds: settings.smtp.code_expiry_seconds,
        }
    }

    /// 生成 6 位数字验证码
    fn generate_code() -> String {
        let mut rng = rand::thread_rng();
        format!("{:06}", rng.gen_range(0..1000000))
    }

    /// 获取 Redis 键名
    fn get_key(&self, code_type: VerificationCodeType, identifier: &str) -> String {
        format!("{}:{}", code_type.redis_prefix(), identifier)
    }

    /// 发送验证码
    pub async fn send_code(
        &self,
        email: &str,
        code_type: VerificationCodeType,
    ) -> Result<(), AppError> {
        // 检查邮件服务是否可用
        if !self.email_service.is_enabled() {
            return Err(AppError::ConfigError("邮件服务未启用".to_string()));
        }

        // 检查是否存在未过期的验证码（防止频繁请求）
        let key = self.get_key(code_type, email);
        let existing: Option<StoredCode> = self.redis_pool.get(&key).await?;
        
        if existing.is_some() {
            // 获取剩余 TTL
            let ttl = self.redis_pool.ttl(&key).await.unwrap_or(0);
            let cooldown = self.code_expiry_seconds as i64 - 60; // 至少等待 1 分钟
            
            if ttl > cooldown {
                return Err(AppError::RateLimitExceeded(
                    format!("请等待 {} 秒后再重新发送", ttl - cooldown)
                ));
            }
        }

        // 生成新验证码
        let code = Self::generate_code();
        
        // 存储验证码
        let stored = StoredCode {
            code: code.clone(),
            attempts: 0,
            email: email.to_string(),
        };
        
        self.redis_pool
            .set_ex(&key, &stored, self.code_expiry_seconds)
            .await?;

        // 发送邮件
        let expires_minutes = self.code_expiry_seconds / 60;
        match code_type {
            VerificationCodeType::EmailVerification => {
                self.email_service
                    .send_verification_code(email, &code, expires_minutes)
                    .await?;
            }
            VerificationCodeType::PasswordReset => {
                self.email_service
                    .send_password_reset_code(email, &code, expires_minutes)
                    .await?;
            }
            VerificationCodeType::LoginVerification => {
                self.email_service
                    .send_verification_code(email, &code, expires_minutes)
                    .await?;
            }
        }

        tracing::info!(
            email = %email,
            code_type = ?code_type,
            "验证码已发送"
        );

        Ok(())
    }

    /// 验证验证码
    pub async fn verify_code(
        &self,
        email: &str,
        code: &str,
        code_type: VerificationCodeType,
    ) -> Result<bool, AppError> {
        let key = self.get_key(code_type, email);
        
        // 获取存储的验证码
        let stored: Option<StoredCode> = self.redis_pool.get(&key).await?;
        
        let mut stored = match stored {
            Some(s) => s,
            None => {
                return Err(AppError::ValidationError("验证码不存在或已过期".to_string()));
            }
        };

        // 检查尝试次数
        if stored.attempts >= 5 {
            // 删除验证码
            self.redis_pool.del(&key).await?;
            return Err(AppError::ValidationError("验证码尝试次数过多，请重新获取".to_string()));
        }

        // 验证
        if stored.code != code {
            // 增加尝试次数
            stored.attempts += 1;
            
            // 获取剩余 TTL
            let ttl = self.redis_pool.ttl(&key).await.unwrap_or(self.code_expiry_seconds as i64);
            
            self.redis_pool
                .set_ex(&key, &stored, ttl as u64)
                .await?;

            return Err(AppError::ValidationError(
                format!("验证码错误，还剩 {} 次尝试机会", 5 - stored.attempts)
            ));
        }

        // 验证成功，删除验证码
        self.redis_pool.del(&key).await?;

        tracing::info!(
            email = %email,
            code_type = ?code_type,
            "验证码验证成功"
        );

        Ok(true)
    }

    /// 检查是否存在有效的验证码
    pub async fn has_valid_code(
        &self,
        email: &str,
        code_type: VerificationCodeType,
    ) -> Result<bool, AppError> {
        let key = self.get_key(code_type, email);
        let exists: Option<StoredCode> = self.redis_pool.get(&key).await?;
        Ok(exists.is_some())
    }

    /// 获取验证码有效期（分钟）
    pub fn get_expiry_minutes(&self) -> u64 {
        self.code_expiry_seconds / 60
    }
}
