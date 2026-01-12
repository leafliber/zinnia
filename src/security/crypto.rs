//! 加密解密工具

use crate::errors::AppError;
use ring::aead::{self, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// 加密上下文
pub struct CryptoContext {
    key: LessSafeKey,
    rng: SystemRandom,
}

impl CryptoContext {
    /// 从 Base64 编码的密钥创建加密上下文
    pub fn new(key_base64: &str) -> Result<Self, AppError> {
        let key_bytes = BASE64
            .decode(key_base64)
            .map_err(|e| AppError::ConfigError(format!("无效的加密密钥格式: {}", e)))?;

        if key_bytes.len() != 32 {
            return Err(AppError::ConfigError(
                "加密密钥必须是 32 字节（256 位）".to_string(),
            ));
        }

        let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
            .map_err(|_| AppError::ConfigError("无法创建加密密钥".to_string()))?;

        Ok(Self {
            key: LessSafeKey::new(unbound_key),
            rng: SystemRandom::new(),
        })
    }

    /// 生成随机 Nonce
    fn generate_nonce(&self) -> Result<[u8; 12], AppError> {
        let mut nonce_bytes = [0u8; 12];
        self.rng
            .fill(&mut nonce_bytes)
            .map_err(|_| AppError::InternalError("随机数生成失败".to_string()))?;
        Ok(nonce_bytes)
    }

    /// 加密数据
    /// 返回格式：nonce (12 bytes) || ciphertext || tag (16 bytes)
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, AppError> {
        let nonce_bytes = self.generate_nonce()?;
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);

        let mut in_out = plaintext.to_vec();
        self.key
            .seal_in_place_append_tag(nonce, aead::Aad::empty(), &mut in_out)
            .map_err(|_| AppError::InternalError("加密失败".to_string()))?;

        // 将 nonce 放在密文前面
        let mut result = nonce_bytes.to_vec();
        result.extend(in_out);
        Ok(result)
    }

    /// 解密数据
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, AppError> {
        if ciphertext.len() < 12 + 16 {
            return Err(AppError::ValidationError("密文格式无效".to_string()));
        }

        let (nonce_bytes, encrypted) = ciphertext.split_at(12);
        let nonce = Nonce::assume_unique_for_key(
            nonce_bytes.try_into().map_err(|_| {
                AppError::InternalError("Nonce 格式错误".to_string())
            })?,
        );

        let mut in_out = encrypted.to_vec();
        let plaintext = self
            .key
            .open_in_place(nonce, aead::Aad::empty(), &mut in_out)
            .map_err(|_| AppError::ValidationError("解密失败：数据可能已被篡改".to_string()))?;

        Ok(plaintext.to_vec())
    }

    /// 加密并返回 Base64 编码
    pub fn encrypt_to_base64(&self, plaintext: &[u8]) -> Result<String, AppError> {
        let ciphertext = self.encrypt(plaintext)?;
        Ok(BASE64.encode(ciphertext))
    }

    /// 从 Base64 解密
    pub fn decrypt_from_base64(&self, ciphertext_base64: &str) -> Result<Vec<u8>, AppError> {
        let ciphertext = BASE64
            .decode(ciphertext_base64)
            .map_err(|e| AppError::ValidationError(format!("无效的 Base64 格式: {}", e)))?;
        self.decrypt(&ciphertext)
    }
}

/// 生成安全随机字节
pub fn generate_random_bytes(len: usize) -> Result<Vec<u8>, AppError> {
    let rng = SystemRandom::new();
    let mut bytes = vec![0u8; len];
    rng.fill(&mut bytes)
        .map_err(|_| AppError::InternalError("随机数生成失败".to_string()))?;
    Ok(bytes)
}

/// 生成 32 字节加密密钥（Base64 编码）
pub fn generate_encryption_key() -> Result<String, AppError> {
    let bytes = generate_random_bytes(32)?;
    Ok(BASE64.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = generate_encryption_key().unwrap();
        let ctx = CryptoContext::new(&key).unwrap();

        let plaintext = b"Hello, World!";
        let ciphertext = ctx.encrypt(plaintext).unwrap();
        let decrypted = ctx.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_encrypt_decrypt_base64() {
        let key = generate_encryption_key().unwrap();
        let ctx = CryptoContext::new(&key).unwrap();

        let plaintext = b"Sensitive data";
        let encrypted = ctx.encrypt_to_base64(plaintext).unwrap();
        let decrypted = ctx.decrypt_from_base64(&encrypted).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }
}
