//! Zinnia - 高性能时间序列后端服务
//!
//! 设备电量监控与预警系统，支持：
//! - 设备电量数据上报
//! - 实时查询与缓存
//! - 低电量预警
//! - 省电模式管理
//! - WebSocket 实时通信

pub mod config;
pub mod db;
pub mod errors;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod repositories;
pub mod routes;
pub mod security;
pub mod services;
pub mod utils;
pub mod websocket;

pub use errors::AppError;
