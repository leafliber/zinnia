//! 密钥管理

use crate::errors::AppError;
use once_cell::sync::OnceCell;
use secrecy::SecretString;
use std::env;

/// 全局密钥存储
static SECRETS: OnceCell<Secrets> = OnceCell::new();

/// 应用密钥集合
pub struct Secrets {
    jwt_secret: SecretString,
    encryption_key: SecretString,
    database_url: SecretString,
    redis_url: SecretString,
}

impl Secrets {
    /// 从环境变量加载密钥
    pub fn load_from_env() -> Result<Self, AppError> {
        Ok(Self {
            jwt_secret: SecretString::new(
                env::var("JWT_SECRET")
                    .map_err(|_| AppError::ConfigError("JWT_SECRET 未设置".to_string()))?
            ),
            encryption_key: SecretString::new(
                env::var("ENCRYPTION_KEY")
                    .map_err(|_| AppError::ConfigError("ENCRYPTION_KEY 未设置".to_string()))?
            ),
            database_url: SecretString::new(
                env::var("DATABASE_URL")
                    .map_err(|_| AppError::ConfigError("DATABASE_URL 未设置".to_string()))?
            ),
            redis_url: SecretString::new(
                env::var("REDIS_URL")
                    .map_err(|_| AppError::ConfigError("REDIS_URL 未设置".to_string()))?
            ),
        })
    }

    /// 初始化全局密钥
    pub fn init() -> Result<(), AppError> {
        let secrets = Self::load_from_env()?;
        SECRETS
            .set(secrets)
            .map_err(|_| AppError::ConfigError("密钥已初始化".to_string()))?;
        Ok(())
    }

    /// 获取全局密钥实例
    pub fn get() -> Result<&'static Secrets, AppError> {
        SECRETS
            .get()
            .ok_or_else(|| AppError::ConfigError("密钥未初始化".to_string()))
    }

    /// 获取 JWT 密钥
    pub fn jwt_secret(&self) -> &SecretString {
        &self.jwt_secret
    }

    /// 获取加密密钥
    pub fn encryption_key(&self) -> &SecretString {
        &self.encryption_key
    }

    /// 获取数据库 URL
    pub fn database_url(&self) -> &SecretString {
        &self.database_url
    }

    /// 获取 Redis URL
    pub fn redis_url(&self) -> &SecretString {
        &self.redis_url
    }
}

/// 验证密钥强度
pub fn validate_secret_strength(secret: &str, min_length: usize) -> Result<(), AppError> {
    if secret.len() < min_length {
        return Err(AppError::ConfigError(format!(
            "密钥长度不足，最少需要 {} 字符",
            min_length
        )));
    }

    // 检查是否包含足够的熵
    let has_upper = secret.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = secret.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = secret.chars().any(|c| c.is_ascii_digit());

    if !has_upper || !has_lower || !has_digit {
        return Err(AppError::ConfigError(
            "密钥应包含大写字母、小写字母和数字".to_string(),
        ));
    }

    Ok(())
}

/// 安全清除内存
/// 注意：Rust 编译器可能优化掉这个操作，使用 secrecy 库更可靠
pub fn secure_zero(data: &mut [u8]) {
    for byte in data.iter_mut() {
        unsafe {
            std::ptr::write_volatile(byte, 0);
        }
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}
