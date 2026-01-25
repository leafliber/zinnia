//! 测试辅助工具

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 生成测试用 UUID
pub fn test_uuid() -> Uuid {
    Uuid::new_v4()
}

/// 生成固定的测试 UUID（用于可重复测试）
pub fn fixed_uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, seed])
}

/// 获取当前时间
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

/// 生成测试设备名称
pub fn test_device_name() -> String {
    format!("test-device-{}", &Uuid::new_v4().to_string()[..8])
}

/// 生成测试用户名
pub fn test_username() -> String {
    format!("testuser_{}", &Uuid::new_v4().to_string()[..8])
}

/// 生成测试邮箱
pub fn test_email() -> String {
    format!("test_{}@example.com", &Uuid::new_v4().to_string()[..8])
}

/// 断言结果是成功的
#[macro_export]
macro_rules! assert_ok {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => panic!("Expected Ok, got Err: {:?}", e),
        }
    };
}

/// 断言结果是错误的
#[macro_export]
macro_rules! assert_err {
    ($expr:expr) => {
        match $expr {
            Ok(val) => panic!("Expected Err, got Ok: {:?}", val),
            Err(e) => e,
        }
    };
}
