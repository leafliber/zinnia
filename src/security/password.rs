//! 密码哈希处理

use crate::errors::AppError;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Params, Version,
};

/// Argon2 配置参数（OWASP 推荐）
/// - 内存：64 MB
/// - 迭代：3 次
/// - 并行度：4
const MEMORY_COST: u32 = 65536; // 64 MB
const TIME_COST: u32 = 3;
const PARALLELISM: u32 = 4;
const OUTPUT_LENGTH: usize = 32;

/// 创建 Argon2 实例
fn create_argon2() -> Result<Argon2<'static>, AppError> {
    let params = Params::new(MEMORY_COST, TIME_COST, PARALLELISM, Some(OUTPUT_LENGTH))
        .map_err(|e| AppError::InternalError(format!("Argon2 参数错误: {}", e)))?;

    Ok(Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params))
}

/// 哈希密码
pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = create_argon2()?;

    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::InternalError(format!("密码哈希失败: {}", e)))?;

    Ok(password_hash.to_string())
}

/// 验证密码
pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AppError::InternalError(format!("哈希格式无效: {}", e)))?;

    let argon2 = create_argon2()?;

    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(AppError::InternalError(format!("密码验证失败: {}", e))),
    }
}

/// 检查密码强度
pub fn check_password_strength(password: &str) -> Result<(), AppError> {
    // 新策略：最少 8 字符，包含字母和数字
    let min_length = 8;

    if password.len() < min_length {
        return Err(AppError::ValidationError(format!(
            "密码长度至少需要 {} 个字符",
            min_length
        )));
    }

    let has_alpha = password.chars().any(|c| c.is_ascii_alphabetic());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());

    if !has_alpha {
        return Err(AppError::ValidationError(
            "密码必须包含至少一个字母".to_string(),
        ));
    }

    if !has_digit {
        return Err(AppError::ValidationError(
            "密码必须包含至少一个数字".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let password = "MySecurePassword123!";
        let hash = hash_password(password).unwrap();
        
        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_password_strength() {
        // 太短
        assert!(check_password_strength("Short1!").is_err());

        // 现在策略允许没有特殊字符，但仍需字母和数字
        assert!(check_password_strength("NoSpecialChar123").is_ok());

        // 合格密码
        assert!(check_password_strength("StrongPassword123!").is_ok());
    }
}
