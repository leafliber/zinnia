//! JWT 单元测试

use zinnia::security::{JwtTokenType, Claims};

mod jwt_token_type {
    use super::*;

    #[test]
    fn test_jwt_token_type_serialization() {
        let access = JwtTokenType::Access;
        let refresh = JwtTokenType::Refresh;
        
        let access_json = serde_json::to_string(&access).unwrap();
        let refresh_json = serde_json::to_string(&refresh).unwrap();
        
        assert_eq!(access_json, "\"access\"");
        assert_eq!(refresh_json, "\"refresh\"");
    }

    #[test]
    fn test_jwt_token_type_deserialization() {
        let access: JwtTokenType = serde_json::from_str("\"access\"").unwrap();
        let refresh: JwtTokenType = serde_json::from_str("\"refresh\"").unwrap();
        
        assert_eq!(access, JwtTokenType::Access);
        assert_eq!(refresh, JwtTokenType::Refresh);
    }
}

mod claims {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_claims_serialization() {
        let claims = Claims {
            sub: "test_subject".to_string(),
            token_type: JwtTokenType::Access,
            iss: "zinnia".to_string(),
            aud: "zinnia-client".to_string(),
            exp: 1234567890,
            iat: 1234567800,
            jti: Uuid::new_v4().to_string(),
            device_id: Some(Uuid::new_v4()),
            role: Some("user".to_string()),
        };
        
        let json = serde_json::to_string(&claims).unwrap();
        assert!(json.contains("\"sub\":\"test_subject\""));
        assert!(json.contains("\"token_type\":\"access\""));
    }

    #[test]
    fn test_claims_deserialization() {
        let json = r#"{
            "sub": "user123",
            "token_type": "refresh",
            "iss": "zinnia",
            "aud": "client",
            "exp": 9999999999,
            "iat": 1000000000,
            "jti": "abc-123",
            "device_id": null,
            "role": null
        }"#;
        
        let claims: Claims = serde_json::from_str(json).unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.token_type, JwtTokenType::Refresh);
        assert!(claims.device_id.is_none());
    }
}
