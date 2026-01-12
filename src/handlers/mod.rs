//! HTTP 处理器模块

mod battery_handler;
mod device_handler;
mod auth_handler;
mod health_handler;
mod alert_handler;
mod user_handler;
mod device_token_handler;
mod compat_handler;

pub use battery_handler::*;
pub use device_handler::*;
pub use auth_handler::*;
pub use health_handler::*;
pub use alert_handler::*;
pub use user_handler::*;
pub use device_token_handler::*;
pub use compat_handler::*;
