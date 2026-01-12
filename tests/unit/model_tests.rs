//! 模型单元测试

use chrono::{Duration, Utc};
use uuid::Uuid;
use zinnia::models::{
    DeviceAccessToken, TokenPermission, CreateAccessTokenRequest,
};

mod device_access_token {
    use super::*;

    fn create_test_token(
        is_revoked: bool, 
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        permission: TokenPermission,
        allowed_ips: Option<Vec<String>>,
    ) -> DeviceAccessToken {
        DeviceAccessToken {
            id: Uuid::new_v4(),
            device_id: Uuid::new_v4(),
            created_by: Uuid::new_v4(),
            token_hash: "test_hash".to_string(),
            token_prefix: "zn_dat_test...".to_string(),
            name: "Test Token".to_string(),
            permission,
            expires_at,
            last_used_at: None,
            use_count: 0,
            is_revoked,
            revoked_at: None,
            allowed_ips,
            rate_limit_per_minute: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_is_valid_active_token() {
        let token = create_test_token(false, None, TokenPermission::All, None);
        assert!(token.is_valid(), "未吊销且无过期时间的令牌应有效");
    }

    #[test]
    fn test_is_valid_revoked_token() {
        let token = create_test_token(true, None, TokenPermission::All, None);
        assert!(!token.is_valid(), "已吊销的令牌应无效");
    }

    #[test]
    fn test_is_valid_expired_token() {
        let expired = Utc::now() - Duration::hours(1);
        let token = create_test_token(false, Some(expired), TokenPermission::All, None);
        assert!(!token.is_valid(), "已过期的令牌应无效");
    }

    #[test]
    fn test_is_valid_future_expiry() {
        let future = Utc::now() + Duration::hours(1);
        let token = create_test_token(false, Some(future), TokenPermission::All, None);
        assert!(token.is_valid(), "未过期的令牌应有效");
    }

    #[test]
    fn test_is_ip_allowed_no_restriction() {
        let token = create_test_token(false, None, TokenPermission::All, None);
        assert!(token.is_ip_allowed("192.168.1.1"), "无 IP 限制时应允许所有 IP");
        assert!(token.is_ip_allowed("10.0.0.1"));
    }

    #[test]
    fn test_is_ip_allowed_empty_list() {
        let token = create_test_token(false, None, TokenPermission::All, Some(vec![]));
        assert!(token.is_ip_allowed("192.168.1.1"), "空白名单应允许所有 IP");
    }

    #[test]
    fn test_is_ip_allowed_in_whitelist() {
        let allowed = vec!["192.168.1.1".to_string(), "10.0.0.1".to_string()];
        let token = create_test_token(false, None, TokenPermission::All, Some(allowed));
        
        assert!(token.is_ip_allowed("192.168.1.1"), "白名单中的 IP 应被允许");
        assert!(token.is_ip_allowed("10.0.0.1"));
    }

    #[test]
    fn test_is_ip_allowed_not_in_whitelist() {
        let allowed = vec!["192.168.1.1".to_string()];
        let token = create_test_token(false, None, TokenPermission::All, Some(allowed));
        
        assert!(!token.is_ip_allowed("192.168.1.2"), "不在白名单中的 IP 应被拒绝");
    }

    #[test]
    fn test_can_read_permission() {
        let read_token = create_test_token(false, None, TokenPermission::Read, None);
        let write_token = create_test_token(false, None, TokenPermission::Write, None);
        let all_token = create_test_token(false, None, TokenPermission::All, None);
        
        assert!(read_token.can_read(), "Read 权限应允许读取");
        assert!(!write_token.can_read(), "Write 权限不应允许读取");
        assert!(all_token.can_read(), "All 权限应允许读取");
    }

    #[test]
    fn test_can_write_permission() {
        let read_token = create_test_token(false, None, TokenPermission::Read, None);
        let write_token = create_test_token(false, None, TokenPermission::Write, None);
        let all_token = create_test_token(false, None, TokenPermission::All, None);
        
        assert!(!read_token.can_write(), "Read 权限不应允许写入");
        assert!(write_token.can_write(), "Write 权限应允许写入");
        assert!(all_token.can_write(), "All 权限应允许写入");
    }
}

mod token_permission {
    use super::*;

    #[test]
    fn test_default_permission() {
        let default = TokenPermission::default();
        assert_eq!(default, TokenPermission::Write, "默认权限应为 Write");
    }

    #[test]
    fn test_permission_display() {
        assert_eq!(TokenPermission::Read.to_string(), "read");
        assert_eq!(TokenPermission::Write.to_string(), "write");
        assert_eq!(TokenPermission::All.to_string(), "all");
    }
}

mod create_access_token_request {
    use super::*;
    use validator::Validate;

    #[test]
    fn test_valid_request() {
        let request = CreateAccessTokenRequest {
            name: "Test Token".to_string(),
            permission: TokenPermission::Write,
            expires_in_hours: Some(24),
            allowed_ips: None,
            rate_limit_per_minute: None,
        };
        
        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_empty_name() {
        let request = CreateAccessTokenRequest {
            name: "".to_string(),
            permission: TokenPermission::Write,
            expires_in_hours: Some(24),
            allowed_ips: None,
            rate_limit_per_minute: None,
        };
        
        assert!(request.validate().is_err(), "空名称应验证失败");
    }

    #[test]
    fn test_name_too_long() {
        let request = CreateAccessTokenRequest {
            name: "a".repeat(101),
            permission: TokenPermission::Write,
            expires_in_hours: Some(24),
            allowed_ips: None,
            rate_limit_per_minute: None,
        };
        
        assert!(request.validate().is_err(), "过长名称应验证失败");
    }

    #[test]
    fn test_expires_in_hours_too_small() {
        let request = CreateAccessTokenRequest {
            name: "Test".to_string(),
            permission: TokenPermission::Write,
            expires_in_hours: Some(0),
            allowed_ips: None,
            rate_limit_per_minute: None,
        };
        
        assert!(request.validate().is_err(), "过期时间为 0 应验证失败");
    }

    #[test]
    fn test_expires_in_hours_too_large() {
        let request = CreateAccessTokenRequest {
            name: "Test".to_string(),
            permission: TokenPermission::Write,
            expires_in_hours: Some(8761), // > 8760 (1 year)
            allowed_ips: None,
            rate_limit_per_minute: None,
        };
        
        assert!(request.validate().is_err(), "过期时间超过 1 年应验证失败");
    }

    #[test]
    fn test_no_expiry() {
        let request = CreateAccessTokenRequest {
            name: "Test".to_string(),
            permission: TokenPermission::Write,
            expires_in_hours: None,
            allowed_ips: None,
            rate_limit_per_minute: None,
        };
        
        assert!(request.validate().is_ok(), "无过期时间应验证通过");
    }
}
