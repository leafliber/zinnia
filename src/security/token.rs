//! 统一令牌生成工具
//! 
//! 提供设备 API Key 和访问令牌的通用生成逻辑

use crate::errors::AppError;
use crate::security::{generate_random_bytes, hash_password, verify_password};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as BASE64};

/// 令牌类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    /// 设备 API Key（生产环境）
    DeviceApiKeyLive,
    /// 设备 API Key（测试环境）
    DeviceApiKeyTest,
    /// 设备访问令牌
    DeviceAccessToken,
}

impl TokenType {
    /// 获取令牌前缀
    pub fn prefix(&self) -> &'static str {
        match self {
            TokenType::DeviceApiKeyLive => "zn_live_",
            TokenType::DeviceApiKeyTest => "zn_test_",
            TokenType::DeviceAccessToken => "zn_dat_",
        }
    }

    /// 获取随机部分的字节长度
    pub fn random_bytes_len(&self) -> usize {
        match self {
            TokenType::DeviceApiKeyLive | TokenType::DeviceApiKeyTest => 32,
            TokenType::DeviceAccessToken => 32,
        }
    }

    /// 获取显示前缀的长度（随机部分取多少字符）
    pub fn display_prefix_len(&self) -> usize {
        match self {
            TokenType::DeviceApiKeyLive | TokenType::DeviceApiKeyTest => 8,
            TokenType::DeviceAccessToken => 12,
        }
    }

    /// 从字符串解析令牌类型
    pub fn from_token(token: &str) -> Option<Self> {
        if token.starts_with("zn_live_") {
            Some(TokenType::DeviceApiKeyLive)
        } else if token.starts_with("zn_test_") {
            Some(TokenType::DeviceApiKeyTest)
        } else if token.starts_with("zn_dat_") {
            Some(TokenType::DeviceAccessToken)
        } else {
            None
        }
    }
}

/// 生成的令牌结果
#[derive(Debug, Clone)]
pub struct GeneratedToken {
    /// 完整令牌（仅返回一次）
    pub token: String,
    /// 令牌哈希值（用于安全存储）
    pub hash: String,
    /// 显示前缀（用于识别）
    pub display_prefix: String,
    /// 令牌类型
    pub token_type: TokenType,
}

/// 生成新令牌
pub fn generate_token(token_type: TokenType) -> Result<GeneratedToken, AppError> {
    // 生成随机字节
    let random_bytes = generate_random_bytes(token_type.random_bytes_len())?;
    
    // Base64 编码
    let random_part = BASE64.encode(&random_bytes);
    
    // 组合完整令牌
    let prefix = token_type.prefix();
    let token = format!("{}{}", prefix, random_part);
    
    // 哈希存储
    let hash = hash_password(&token)?;
    
    // 生成显示前缀
    let display_len = token_type.display_prefix_len();
    let display_prefix = format!("{}{}...", prefix, &random_part[..display_len]);
    
    Ok(GeneratedToken {
        token,
        hash,
        display_prefix,
        token_type,
    })
}

/// 验证令牌
pub fn verify_token(token: &str, hash: &str) -> Result<bool, AppError> {
    verify_password(token, hash)
}

/// 验证令牌格式
pub fn validate_token_format(token: &str) -> Result<TokenType, AppError> {
    let token_type = TokenType::from_token(token)
        .ok_or_else(|| AppError::ValidationError("无效的令牌格式".to_string()))?;
    
    let prefix_len = token_type.prefix().len();
    let expected_base64_len = (token_type.random_bytes_len() * 4 + 2) / 3; // Base64 编码长度
    let expected_total = prefix_len + expected_base64_len;
    
    // 允许一定的长度偏差（Base64 padding）
    if token.len() < expected_total - 2 || token.len() > expected_total + 2 {
        return Err(AppError::ValidationError("无效的令牌长度".to_string()));
    }
    
    Ok(token_type)
}

/// 遮蔽令牌（用于日志）
pub fn mask_token(token: &str) -> String {
    if let Some(token_type) = TokenType::from_token(token) {
        let prefix = token_type.prefix();
        let prefix_len = prefix.len();
        
        if token.len() > prefix_len + 4 {
            return format!("{}{}...", prefix, &token[prefix_len..prefix_len + 4]);
        }
    }
    
    if token.len() > 8 {
        format!("{}...", &token[..8])
    } else {
        "***".to_string()
    }
}

/// 从令牌提取搜索前缀（用于数据库查询）
pub fn extract_search_prefix(token: &str) -> Result<String, AppError> {
    let token_type = TokenType::from_token(token)
        .ok_or_else(|| AppError::ValidationError("无效的令牌格式".to_string()))?;
    
    let prefix = token_type.prefix();
    let prefix_len = prefix.len();
    let display_len = token_type.display_prefix_len();
    
    if token.len() < prefix_len + display_len {
        return Err(AppError::ValidationError("令牌过短".to_string()));
    }
    
    let random_part = &token[prefix_len..prefix_len + display_len];
    Ok(format!("{}{}...", prefix, random_part))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_device_api_key_live() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        
        assert!(result.token.starts_with("zn_live_"));
        assert!(result.display_prefix.starts_with("zn_live_"));
        assert!(result.display_prefix.ends_with("..."));
        assert!(!result.hash.is_empty());
        assert_eq!(result.token_type, TokenType::DeviceApiKeyLive);
    }

    #[test]
    fn test_generate_device_api_key_test() {
        let result = generate_token(TokenType::DeviceApiKeyTest).unwrap();
        
        assert!(result.token.starts_with("zn_test_"));
        assert_eq!(result.token_type, TokenType::DeviceApiKeyTest);
    }

    #[test]
    fn test_generate_device_access_token() {
        let result = generate_token(TokenType::DeviceAccessToken).unwrap();
        
        assert!(result.token.starts_with("zn_dat_"));
        assert!(result.display_prefix.starts_with("zn_dat_"));
        assert_eq!(result.token_type, TokenType::DeviceAccessToken);
    }

    #[test]
    fn test_verify_token_success() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        
        assert!(verify_token(&result.token, &result.hash).unwrap());
    }

    #[test]
    fn test_verify_token_failure() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        
        assert!(!verify_token("wrong_token", &result.hash).unwrap());
    }

    #[test]
    fn test_validate_token_format() {
        let live_key = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        let result = validate_token_format(&live_key.token).unwrap();
        assert_eq!(result, TokenType::DeviceApiKeyLive);
        
        let dat = generate_token(TokenType::DeviceAccessToken).unwrap();
        let result = validate_token_format(&dat.token).unwrap();
        assert_eq!(result, TokenType::DeviceAccessToken);
    }

    #[test]
    fn test_validate_token_format_invalid() {
        let result = validate_token_format("invalid_token");
        assert!(result.is_err());
    }

    #[test]
    fn test_token_type_from_token() {
        assert_eq!(TokenType::from_token("zn_live_abc"), Some(TokenType::DeviceApiKeyLive));
        assert_eq!(TokenType::from_token("zn_test_abc"), Some(TokenType::DeviceApiKeyTest));
        assert_eq!(TokenType::from_token("zn_dat_abc"), Some(TokenType::DeviceAccessToken));
        assert_eq!(TokenType::from_token("unknown"), None);
    }

    #[test]
    fn test_mask_token() {
        let token = "zn_live_abcdefghijklmnopqrstuvwxyz";
        let masked = mask_token(token);
        
        assert!(masked.starts_with("zn_live_"));
        assert!(masked.ends_with("..."));
        assert!(!masked.contains("mnop")); // 中间部分被隐藏
    }

    #[test]
    fn test_extract_search_prefix() {
        let token = generate_token(TokenType::DeviceAccessToken).unwrap();
        let search_prefix = extract_search_prefix(&token.token).unwrap();
        
        assert!(search_prefix.starts_with("zn_dat_"));
        assert!(search_prefix.ends_with("..."));
        assert_eq!(search_prefix, token.display_prefix);
    }

    #[test]
    fn test_token_uniqueness() {
        let token1 = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        let token2 = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        
        assert_ne!(token1.token, token2.token);
        assert_ne!(token1.hash, token2.hash);
    }
}
