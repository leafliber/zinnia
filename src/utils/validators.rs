//! 数据验证工具

use crate::errors::AppError;
use uuid::Uuid;

/// 验证 UUID 格式
pub fn validate_uuid(s: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(s).map_err(|_| AppError::ValidationError("无效的 UUID 格式".to_string()))
}

/// 验证分页参数
pub fn validate_pagination(page: i64, page_size: i64) -> Result<(), AppError> {
    if page < 1 {
        return Err(AppError::ValidationError("页码必须大于 0".to_string()));
    }
    
    if page_size < 1 || page_size > 100 {
        return Err(AppError::ValidationError(
            "每页数量必须在 1-100 之间".to_string(),
        ));
    }

    Ok(())
}

/// 验证电量值
pub fn validate_battery_level(level: i32) -> Result<(), AppError> {
    if level < 0 || level > 100 {
        return Err(AppError::ValidationError(
            "电量值必须在 0-100 之间".to_string(),
        ));
    }
    Ok(())
}

/// 验证温度值
pub fn validate_temperature(temp: f64) -> Result<(), AppError> {
    if temp < -40.0 || temp > 85.0 {
        return Err(AppError::ValidationError(
            "温度值必须在 -40 到 85 摄氏度之间".to_string(),
        ));
    }
    Ok(())
}

/// 验证字符串长度
pub fn validate_string_length(
    s: &str,
    field_name: &str,
    min: usize,
    max: usize,
) -> Result<(), AppError> {
    let len = s.len();
    if len < min || len > max {
        return Err(AppError::ValidationError(format!(
            "{} 长度必须在 {}-{} 字符之间",
            field_name, min, max
        )));
    }
    Ok(())
}

/// 清理输入字符串（移除危险字符）
pub fn sanitize_input(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '<' | '>' | '"' | '\'' | '\\' | '\0'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_uuid() {
        assert!(validate_uuid("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(validate_uuid("invalid").is_err());
    }

    #[test]
    fn test_validate_battery_level() {
        assert!(validate_battery_level(50).is_ok());
        assert!(validate_battery_level(0).is_ok());
        assert!(validate_battery_level(100).is_ok());
        assert!(validate_battery_level(-1).is_err());
        assert!(validate_battery_level(101).is_err());
    }

    #[test]
    fn test_sanitize_input() {
        assert_eq!(sanitize_input("hello<script>"), "helloscript");
        assert_eq!(sanitize_input("normal text"), "normal text");
    }
}
