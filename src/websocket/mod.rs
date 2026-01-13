//! WebSocket 模块
//!
//! 提供 WebSocket 支持，用于：
//! - 设备实时电量上报
//! - 用户订阅设备数据推送
//! - 低延迟双向通信

mod handler;
mod messages;
mod session;

pub use handler::{configure as configure_ws_routes, ws_handler};
pub use messages::*;
pub use session::WsSession;
