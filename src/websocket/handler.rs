//! WebSocket 路由处理器

use crate::repositories::DeviceRepository;
use crate::security::JwtManager;
use crate::services::{BatteryService, DeviceAccessTokenService};
use crate::websocket::session::WsSession;

use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use std::sync::Arc;
use tracing::info;

/// 获取客户端 IP
fn get_client_ip(req: &HttpRequest) -> Option<String> {
    // 尝试从 X-Forwarded-For 获取
    if let Some(forwarded) = req.headers().get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            return forwarded_str.split(',').next().map(|s| s.trim().to_string());
        }
    }
    
    // 尝试从 X-Real-IP 获取
    if let Some(real_ip) = req.headers().get("X-Real-IP") {
        if let Ok(ip) = real_ip.to_str() {
            return Some(ip.to_string());
        }
    }
    
    // 从连接信息获取
    req.peer_addr().map(|addr| addr.ip().to_string())
}

/// WebSocket 连接升级处理器
///
/// 端点: GET /ws
///
/// 支持通过 query parameter 传递 token 进行预认证：
/// - `/ws?token=<device_token>` - 设备令牌认证
/// - `/ws?token=<jwt>&auth_type=jwt` - JWT 用户认证
///
/// 也可以在连接建立后通过消息进行认证
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    battery_service: web::Data<Arc<BatteryService>>,
    device_token_service: web::Data<Arc<DeviceAccessTokenService>>,
    jwt_manager: web::Data<Arc<JwtManager>>,
    device_repo: web::Data<Arc<DeviceRepository>>,
) -> Result<HttpResponse, Error> {
    let client_ip = get_client_ip(&req);
    
    info!("WebSocket 连接请求: ip={:?}", client_ip);
    
    // 创建 session
    let session = WsSession::new(
        client_ip,
        battery_service.get_ref().clone(),
        device_token_service.get_ref().clone(),
        jwt_manager.get_ref().clone(),
        device_repo.get_ref().clone(),
    );
    
    // 升级到 WebSocket 连接
    ws::start(session, &req, stream)
}

/// 配置 WebSocket 路由
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/ws")
            .route(web::get().to(ws_handler))
    );
}
