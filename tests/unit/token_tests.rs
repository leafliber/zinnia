//! 令牌系统单元测试

use zinnia::security::{
    // 统一令牌模块
    generate_token, verify_token, validate_token_format, mask_token, 
    extract_search_prefix, TokenType,
};

mod token_generation {
    use super::*;

    #[test]
    fn test_generate_device_api_key_live() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        
        assert!(result.token.starts_with("zn_live_"), "令牌应以 zn_live_ 开头");
        assert!(result.display_prefix.starts_with("zn_live_"), "显示前缀应以 zn_live_ 开头");
        assert!(result.display_prefix.ends_with("..."), "显示前缀应以 ... 结尾");
        assert!(!result.hash.is_empty(), "哈希不应为空");
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
        
        assert!(result.token.starts_with("zn_dat_"), "令牌应以 zn_dat_ 开头");
        assert!(result.display_prefix.starts_with("zn_dat_"));
        assert_eq!(result.token_type, TokenType::DeviceAccessToken);
    }

    #[test]
    fn test_token_uniqueness() {
        let token1 = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        let token2 = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        
        assert_ne!(token1.token, token2.token, "两次生成的令牌应不同");
        assert_ne!(token1.hash, token2.hash, "两次生成的哈希应不同");
    }

    #[test]
    fn test_token_length() {
        let live_key = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        let test_key = generate_token(TokenType::DeviceApiKeyTest).unwrap();
        let dat = generate_token(TokenType::DeviceAccessToken).unwrap();
        
        // 所有令牌长度应该合理（前缀 + 32 字节的 Base64 编码 ≈ 43 字符）
        assert!(live_key.token.len() > 40, "Live key 长度应 > 40");
        assert!(test_key.token.len() > 40, "Test key 长度应 > 40");
        assert!(dat.token.len() > 40, "DAT 长度应 > 40");
    }
}

mod token_verification {
    use super::*;

    #[test]
    fn test_verify_token_success() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        
        assert!(verify_token(&result.token, &result.hash).unwrap(), "正确的令牌应验证通过");
    }

    #[test]
    fn test_verify_token_wrong_token() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        
        assert!(!verify_token("wrong_token", &result.hash).unwrap(), "错误的令牌应验证失败");
    }

    #[test]
    fn test_verify_token_modified_token() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        let modified = format!("{}x", &result.token[..result.token.len()-1]);
        
        assert!(!verify_token(&modified, &result.hash).unwrap(), "篡改的令牌应验证失败");
    }

    #[test]
    fn test_verify_different_token_types() {
        // 确保不同类型的令牌都能正确验证
        for token_type in [
            TokenType::DeviceApiKeyLive,
            TokenType::DeviceApiKeyTest,
            TokenType::DeviceAccessToken,
        ] {
            let result = generate_token(token_type).unwrap();
            assert!(verify_token(&result.token, &result.hash).unwrap());
        }
    }
}

mod token_format_validation {
    use super::*;

    #[test]
    fn test_validate_token_format_live() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        let validated = validate_token_format(&result.token).unwrap();
        
        assert_eq!(validated, TokenType::DeviceApiKeyLive);
    }

    #[test]
    fn test_validate_token_format_test() {
        let result = generate_token(TokenType::DeviceApiKeyTest).unwrap();
        let validated = validate_token_format(&result.token).unwrap();
        
        assert_eq!(validated, TokenType::DeviceApiKeyTest);
    }

    #[test]
    fn test_validate_token_format_dat() {
        let result = generate_token(TokenType::DeviceAccessToken).unwrap();
        let validated = validate_token_format(&result.token).unwrap();
        
        assert_eq!(validated, TokenType::DeviceAccessToken);
    }

    #[test]
    fn test_validate_token_format_invalid() {
        let result = validate_token_format("invalid_token");
        assert!(result.is_err(), "无效令牌格式应返回错误");
    }

    #[test]
    fn test_validate_token_format_too_short() {
        let result = validate_token_format("zn_live_abc");
        assert!(result.is_err(), "过短的令牌应返回错误");
    }
}

mod token_type_detection {
    use super::*;

    #[test]
    fn test_token_type_from_token() {
        assert_eq!(TokenType::from_token("zn_live_abc123"), Some(TokenType::DeviceApiKeyLive));
        assert_eq!(TokenType::from_token("zn_test_abc123"), Some(TokenType::DeviceApiKeyTest));
        assert_eq!(TokenType::from_token("zn_dat_abc123"), Some(TokenType::DeviceAccessToken));
        assert_eq!(TokenType::from_token("unknown"), None);
        assert_eq!(TokenType::from_token(""), None);
    }

    #[test]
    fn test_token_type_prefix() {
        assert_eq!(TokenType::DeviceApiKeyLive.prefix(), "zn_live_");
        assert_eq!(TokenType::DeviceApiKeyTest.prefix(), "zn_test_");
        assert_eq!(TokenType::DeviceAccessToken.prefix(), "zn_dat_");
    }
}

mod token_masking {
    use super::*;

    #[test]
    fn test_mask_token_live() {
        let token = "zn_live_abcdefghijklmnopqrstuvwxyz";
        let masked = mask_token(token);
        
        assert!(masked.starts_with("zn_live_"), "遮蔽后应保留前缀");
        assert!(masked.ends_with("..."), "遮蔽后应以 ... 结尾");
        assert!(!masked.contains("mnop"), "中间部分应被遮蔽");
    }

    #[test]
    fn test_mask_token_dat() {
        let result = generate_token(TokenType::DeviceAccessToken).unwrap();
        let masked = mask_token(&result.token);
        
        assert!(masked.starts_with("zn_dat_"));
        assert!(masked.ends_with("..."));
        assert!(masked.len() < result.token.len(), "遮蔽后长度应更短");
    }

    #[test]
    fn test_mask_token_short() {
        let masked = mask_token("short");
        assert_eq!(masked, "***", "过短的令牌应返回 ***");
    }
}

mod search_prefix_extraction {
    use super::*;

    #[test]
    fn test_extract_search_prefix() {
        let result = generate_token(TokenType::DeviceAccessToken).unwrap();
        let search_prefix = extract_search_prefix(&result.token).unwrap();
        
        assert!(search_prefix.starts_with("zn_dat_"));
        assert!(search_prefix.ends_with("..."));
        assert_eq!(search_prefix, result.display_prefix, "搜索前缀应与显示前缀相同");
    }

    #[test]
    fn test_extract_search_prefix_invalid() {
        let result = extract_search_prefix("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_search_prefix_too_short() {
        let result = extract_search_prefix("zn_dat_abc");
        assert!(result.is_err(), "过短的令牌应返回错误");
    }
    
    #[test]
    fn test_extract_search_prefix_for_api_key() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        let search_prefix = extract_search_prefix(&result.token).unwrap();
        
        assert!(search_prefix.starts_with("zn_live_"));
        assert!(search_prefix.ends_with("..."));
    }
}

mod api_key_via_token_module {
    use super::*;

    #[test]
    fn test_generate_api_key_live() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        
        assert!(result.token.starts_with("zn_live_"));
        assert!(result.display_prefix.ends_with("..."));
        assert!(!result.hash.is_empty());
    }

    #[test]
    fn test_generate_api_key_test() {
        let result = generate_token(TokenType::DeviceApiKeyTest).unwrap();
        
        assert!(result.token.starts_with("zn_test_"));
    }

    #[test]
    fn test_verify_api_key() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        
        assert!(verify_token(&result.token, &result.hash).unwrap());
        assert!(!verify_token("wrong", &result.hash).unwrap());
    }

    #[test]
    fn test_validate_api_key_format() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        let token_type = validate_token_format(&result.token).unwrap();
        
        assert_eq!(token_type, TokenType::DeviceApiKeyLive);
    }

    #[test]
    fn test_parse_token_type() {
        assert_eq!(TokenType::from_token("zn_live_abc"), Some(TokenType::DeviceApiKeyLive));
        assert_eq!(TokenType::from_token("zn_test_abc"), Some(TokenType::DeviceApiKeyTest));
        assert_eq!(TokenType::from_token("zn_dat_abc"), Some(TokenType::DeviceAccessToken));
        assert_eq!(TokenType::from_token("invalid"), None);
    }

    #[test]
    fn test_mask_api_key() {
        let result = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        let masked = mask_token(&result.token);
        
        assert!(masked.starts_with("zn_live_"));
        assert!(masked.ends_with("..."));
    }
}

mod cross_type_security {
    use super::*;

    #[test]
    fn test_api_key_not_dat() {
        let api_key = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        let token_type = TokenType::from_token(&api_key.token).unwrap();
        
        assert_ne!(token_type, TokenType::DeviceAccessToken, 
            "API Key 不应被识别为 Device Access Token");
    }

    #[test]
    fn test_dat_not_api_key() {
        let dat = generate_token(TokenType::DeviceAccessToken).unwrap();
        let token_type = TokenType::from_token(&dat.token).unwrap();
        
        assert_ne!(token_type, TokenType::DeviceApiKeyLive);
        assert_ne!(token_type, TokenType::DeviceApiKeyTest);
    }

    #[test]
    fn test_hash_not_interchangeable() {
        let api_key = generate_token(TokenType::DeviceApiKeyLive).unwrap();
        let dat = generate_token(TokenType::DeviceAccessToken).unwrap();
        
        // 不同类型的令牌的哈希不应能互相验证
        assert!(!verify_token(&api_key.token, &dat.hash).unwrap());
        assert!(!verify_token(&dat.token, &api_key.hash).unwrap());
    }
}
