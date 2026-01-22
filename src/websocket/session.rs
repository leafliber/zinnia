//! WebSocket 连接 Actor
//!
//! 每个 WebSocket 连接对应一个 Actor 实例，负责处理消息收发和状态管理

use crate::models::BatteryReportRequest;
use crate::repositories::DeviceRepository;
use crate::security::JwtManager;
use crate::services::{BatteryService, DeviceAccessTokenService};
use crate::websocket::messages::*;

use actix::{Actor, ActorContext, ActorFutureExt, AsyncContext, Handler, Message, Running, StreamHandler};
use actix_web_actors::ws;
use chrono::Utc;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// 心跳间隔
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
/// 客户端超时时间
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);
/// 认证超时时间（秒）
const AUTH_TIMEOUT_SECS: u64 = 30;
/// 最大订阅设备数量
const MAX_SUBSCRIBED_DEVICES: usize = 100;

/// WebSocket 连接状态
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// 等待认证
    WaitingAuth,
    /// 已认证
    Authenticated,
    /// 已关闭
    Closed,
}

/// WebSocket 连接 Session
pub struct WsSession {
    /// 连接唯一 ID
    pub id: Uuid,
    
    /// 最后心跳时间
    pub last_heartbeat: Instant,
    
    /// 连接建立时间
    pub connected_at: Instant,
    
    /// 连接状态
    pub state: ConnectionState,
    
    /// 设备 ID（设备认证后设置）
    pub device_id: Option<Uuid>,
    
    /// 用户 ID（用户认证后设置）
    pub user_id: Option<Uuid>,
    
    /// 用户订阅的设备列表
    pub subscribed_devices: HashSet<Uuid>,
    
    /// 客户端 IP
    pub client_ip: Option<String>,
    
    // 服务依赖
    pub battery_service: Arc<BatteryService>,
    pub device_token_service: Arc<DeviceAccessTokenService>,
    pub jwt_manager: Arc<JwtManager>,
    pub device_repo: Arc<DeviceRepository>,
}

impl WsSession {
    pub fn new(
        client_ip: Option<String>,
        battery_service: Arc<BatteryService>,
        device_token_service: Arc<DeviceAccessTokenService>,
        jwt_manager: Arc<JwtManager>,
        device_repo: Arc<DeviceRepository>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            last_heartbeat: Instant::now(),
            connected_at: Instant::now(),
            state: ConnectionState::WaitingAuth,
            device_id: None,
            user_id: None,
            subscribed_devices: HashSet::new(),
            client_ip,
            battery_service,
            device_token_service,
            jwt_manager,
            device_repo,
        }
    }
    
    /// 启动心跳检查
    fn start_heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // 检查心跳超时
            if Instant::now().duration_since(act.last_heartbeat) > CLIENT_TIMEOUT {
                warn!("WebSocket 客户端心跳超时，断开连接: {}", act.id);
                ctx.stop();
                return;
            }
            
            // 检查认证超时
            if act.state == ConnectionState::WaitingAuth {
                let auth_timeout = Duration::from_secs(AUTH_TIMEOUT_SECS);
                if Instant::now().duration_since(act.connected_at) > auth_timeout {
                    warn!("WebSocket 客户端认证超时，断开连接: {}", act.id);
                    let msg = ServerMessage::error("AUTH_TIMEOUT", "认证超时");
                    if let Ok(json) = serde_json::to_string(&msg) {
                        ctx.text(json);
                    }
                    ctx.stop();
                    return;
                }
            }
            
            // 发送 ping
            ctx.ping(b"");
        });
    }
    
    /// 发送服务器消息
    fn send_message(&self, ctx: &mut ws::WebsocketContext<Self>, msg: ServerMessage) {
        match serde_json::to_string(&msg) {
            Ok(json) => ctx.text(json),
            Err(e) => error!("序列化消息失败: {}", e),
        }
    }
    
    /// 处理认证消息
    fn handle_auth(&mut self, ctx: &mut ws::WebsocketContext<Self>, auth: AuthMessage) {
        let token = auth.token.clone();
        let auth_type = auth.auth_type.clone();
        let client_ip = self.client_ip.clone();
        let device_token_service = self.device_token_service.clone();
        let jwt_manager = self.jwt_manager.clone();
        
        let session_id = self.id;
        
        let fut = async move {
            match auth_type {
                AuthType::DeviceToken => {
                    // 验证设备访问令牌
                    match device_token_service.validate_token(&token, client_ip.as_deref()).await {
                        Ok((_token_info, device_id)) => {
                            info!("WebSocket 设备认证成功: session={}, device={}", session_id, device_id);
                            AuthResult::DeviceAuth(device_id)
                        }
                        Err(e) => {
                            warn!("WebSocket 设备认证失败: session={}, error={}", session_id, e);
                            AuthResult::Failed(e.to_string())
                        }
                    }
                }
                AuthType::Jwt => {
                    // 验证 JWT
                    match jwt_manager.validate_access_token(&token) {
                        Ok(claims) => {
                            // 解析 user_id
                            match Uuid::parse_str(&claims.sub) {
                                Ok(user_id) => {
                                    info!("WebSocket 用户认证成功: session={}, user={}", session_id, user_id);
                                    AuthResult::UserAuth(user_id, claims.role)
                                }
                                Err(_) => {
                                    warn!("WebSocket JWT claims.sub 格式错误: session={}", session_id);
                                    AuthResult::Failed("用户 ID 格式错误".to_string())
                                }
                            }
                        }
                        Err(e) => {
                            warn!("WebSocket JWT 认证失败: session={}, error={}", session_id, e);
                            AuthResult::Failed(e.to_string())
                        }
                    }
                }
            }
        };
        
        // 使用 actix 异步执行
        ctx.spawn(actix::fut::wrap_future(fut).map(|result, act: &mut Self, ctx| {
            match result {
                AuthResult::DeviceAuth(device_id) => {
                    act.device_id = Some(device_id);
                    act.state = ConnectionState::Authenticated;
                    act.send_message(ctx, ServerMessage::auth_success(Some(device_id), None));
                }
                AuthResult::UserAuth(user_id, _role) => {
                    act.user_id = Some(user_id);
                    act.state = ConnectionState::Authenticated;
                    act.send_message(ctx, ServerMessage::auth_success(None, Some(user_id)));
                }
                AuthResult::Failed(error) => {
                    act.send_message(ctx, ServerMessage::auth_failed(error));
                }
            }
        }));
    }
    
    /// 处理电量上报
    fn handle_battery_report(&mut self, ctx: &mut ws::WebsocketContext<Self>, report: BatteryReportMessage) {
        // 检查认证状态
        if self.state != ConnectionState::Authenticated {
            self.send_message(ctx, ServerMessage::error("UNAUTHORIZED", "请先完成认证"));
            return;
        }
        
        // 检查是否是设备连接
        let device_id = match self.device_id {
            Some(id) => id,
            None => {
                self.send_message(ctx, ServerMessage::battery_report_failed(
                    "只有设备可以上报电量数据",
                    report.msg_id.clone()
                ));
                return;
            }
        };
        
        // 验证电量值
        if report.battery_level < 0 || report.battery_level > 100 {
            self.send_message(ctx, ServerMessage::battery_report_failed(
                "电量值必须在 0-100 之间",
                report.msg_id.clone()
            ));
            return;
        }
        
        let battery_service = self.battery_service.clone();
        let msg_id = report.msg_id.clone();
        
        // 转换为上报请求
        let request = BatteryReportRequest {
            battery_level: report.battery_level,
            is_charging: report.is_charging,
            power_saving_mode: report.power_saving_mode,
            temperature: report.temperature,
            voltage: report.voltage,
            recorded_at: report.recorded_at,
        };
        
        let fut = async move {
            battery_service.report(device_id, request).await
        };
        
        ctx.spawn(actix::fut::wrap_future(fut).map(move |result: Result<_, crate::errors::AppError>, act: &mut Self, ctx| {
            match result {
                Ok(data) => {
                    debug!("WebSocket 电量上报成功: device={}, level={}", device_id, data.battery_level);
                    act.send_message(ctx, ServerMessage::battery_report_success(data, msg_id));
                }
                Err(e) => {
                    error!("WebSocket 电量上报失败: device={}, error={}", device_id, e);
                    act.send_message(ctx, ServerMessage::battery_report_failed(e.to_string(), msg_id));
                }
            }
        }));
    }
    
    /// 处理批量电量上报
    fn handle_batch_report(&mut self, ctx: &mut ws::WebsocketContext<Self>, batch: BatchBatteryReportMessage) {
        // 检查认证状态
        if self.state != ConnectionState::Authenticated {
            self.send_message(ctx, ServerMessage::error("UNAUTHORIZED", "请先完成认证"));
            return;
        }
        
        // 检查是否是设备连接
        let device_id = match self.device_id {
            Some(id) => id,
            None => {
                self.send_message(ctx, ServerMessage::BatchBatteryReportResult(BatchReportResultMessage {
                    success: false,
                    inserted_count: None,
                    error: Some("只有设备可以上报电量数据".to_string()),
                    msg_id: batch.msg_id.clone(),
                }));
                return;
            }
        };
        
        // 验证数据
        if batch.data.is_empty() {
            self.send_message(ctx, ServerMessage::BatchBatteryReportResult(BatchReportResultMessage {
                success: false,
                inserted_count: None,
                error: Some("批量数据不能为空".to_string()),
                msg_id: batch.msg_id.clone(),
            }));
            return;
        }
        
        if batch.data.len() > 1000 {
            self.send_message(ctx, ServerMessage::BatchBatteryReportResult(BatchReportResultMessage {
                success: false,
                inserted_count: None,
                error: Some("批量数据条数不能超过 1000".to_string()),
                msg_id: batch.msg_id.clone(),
            }));
            return;
        }
        
        let battery_service = self.battery_service.clone();
        let msg_id = batch.msg_id.clone();
        
        // 转换为上报请求列表
        let requests: Vec<BatteryReportRequest> = batch.data.into_iter().map(|r| BatteryReportRequest {
            battery_level: r.battery_level,
            is_charging: r.is_charging,
            power_saving_mode: r.power_saving_mode,
            temperature: r.temperature,
            voltage: r.voltage,
            recorded_at: r.recorded_at,
        }).collect();
        
        let fut = async move {
            battery_service.batch_report(device_id, requests).await
        };
        
        ctx.spawn(actix::fut::wrap_future(fut).map(move |result: Result<usize, crate::errors::AppError>, act: &mut Self, ctx| {
            match result {
                Ok(count) => {
                    debug!("WebSocket 批量上报成功: device={}, count={}", device_id, count);
                    act.send_message(ctx, ServerMessage::BatchBatteryReportResult(BatchReportResultMessage {
                        success: true,
                        inserted_count: Some(count),
                        error: None,
                        msg_id,
                    }));
                }
                Err(e) => {
                    error!("WebSocket 批量上报失败: device={}, error={}", device_id, e);
                    act.send_message(ctx, ServerMessage::BatchBatteryReportResult(BatchReportResultMessage {
                        success: false,
                        inserted_count: None,
                        error: Some(e.to_string()),
                        msg_id,
                    }));
                }
            }
        }));
    }
    
    /// 处理订阅请求（用户）
    fn handle_subscribe(&mut self, ctx: &mut ws::WebsocketContext<Self>, sub: SubscribeMessage) {
        // 检查认证状态
        if self.state != ConnectionState::Authenticated {
            self.send_message(ctx, ServerMessage::error("UNAUTHORIZED", "请先完成认证"));
            return;
        }
        
        // 只有用户可以订阅
        let user_id = match self.user_id {
            Some(id) => id,
            None => {
                self.send_message(ctx, ServerMessage::SubscribeResult(SubscribeResultMessage {
                    success: false,
                    subscribed_devices: vec![],
                    error: Some("只有用户可以订阅设备数据".to_string()),
                }));
                return;
            }
        };
        
        let device_repo = self.device_repo.clone();
        let device_ids = sub.device_ids.clone();
        
        // 验证用户是否有权访问这些设备
        let fut = async move {
            let mut accessible_devices = Vec::new();
            for device_id in device_ids {
                if device_repo.user_can_access(device_id, user_id).await.unwrap_or(false) {
                    accessible_devices.push(device_id);
                }
            }
            accessible_devices
        };
        
        ctx.spawn(actix::fut::wrap_future(fut).map(|accessible_devices: Vec<Uuid>, act: &mut Self, ctx| {
            // 检查订阅数量限制
            let new_subscriptions = accessible_devices.len();
            let current_subscriptions = act.subscribed_devices.len();

            if current_subscriptions + new_subscriptions > MAX_SUBSCRIBED_DEVICES {
                act.send_message(ctx, ServerMessage::SubscribeResult(SubscribeResultMessage {
                    success: false,
                    subscribed_devices: vec![],
                    error: Some(format!("订阅设备数量超过限制 (最大 {})", MAX_SUBSCRIBED_DEVICES)),
                }));
                return;
            }

            // 添加到订阅列表
            for device_id in &accessible_devices {
                act.subscribed_devices.insert(*device_id);
            }

            info!("用户 {} 订阅了 {} 个设备", act.user_id.unwrap_or_default(), new_subscriptions);

            act.send_message(ctx, ServerMessage::SubscribeResult(SubscribeResultMessage {
                success: true,
                subscribed_devices: accessible_devices,
                error: None,
            }));
        }));
    }
    
    /// 处理取消订阅请求
    fn handle_unsubscribe(&mut self, ctx: &mut ws::WebsocketContext<Self>, unsub: UnsubscribeMessage) {
        if unsub.device_ids.is_empty() {
            // 取消所有订阅
            self.subscribed_devices.clear();
        } else {
            // 取消指定订阅
            for device_id in unsub.device_ids {
                self.subscribed_devices.remove(&device_id);
            }
        }
        
        self.send_message(ctx, ServerMessage::SubscribeResult(SubscribeResultMessage {
            success: true,
            subscribed_devices: self.subscribed_devices.iter().cloned().collect(),
            error: None,
        }));
    }
    
    /// 处理客户端消息
    fn handle_client_message(&mut self, ctx: &mut ws::WebsocketContext<Self>, text: &str) {
        // 解析消息
        let msg: ClientMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                self.send_message(ctx, ServerMessage::error("INVALID_MESSAGE", format!("消息格式错误: {}", e)));
                return;
            }
        };
        
        match msg {
            ClientMessage::Auth(auth) => {
                self.handle_auth(ctx, auth);
            }
            ClientMessage::BatteryReport(report) => {
                self.handle_battery_report(ctx, report);
            }
            ClientMessage::BatchBatteryReport(batch) => {
                self.handle_batch_report(ctx, batch);
            }
            ClientMessage::Ping => {
                self.send_message(ctx, ServerMessage::Pong);
            }
            ClientMessage::Subscribe(sub) => {
                self.handle_subscribe(ctx, sub);
            }
            ClientMessage::Unsubscribe(unsub) => {
                self.handle_unsubscribe(ctx, unsub);
            }
        }
    }
}

/// 认证结果内部类型
enum AuthResult {
    DeviceAuth(Uuid),
    UserAuth(Uuid, Option<String>),
    Failed(String),
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        info!("WebSocket 连接建立: session={}, ip={:?}", self.id, self.client_ip);
        
        // 启动心跳检查
        self.start_heartbeat(ctx);
        
        // 发送连接成功消息
        self.send_message(ctx, ServerMessage::Connected(ConnectedMessage {
            message: "WebSocket 连接已建立".to_string(),
            server_time: Utc::now(),
            auth_timeout: AUTH_TIMEOUT_SECS,
        }));
    }
    
    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        info!("WebSocket 连接关闭: session={}", self.id);
        self.state = ConnectionState::Closed;
        Running::Stop
    }
}

/// 处理 WebSocket 消息
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                error!("WebSocket 协议错误: session={}, error={}", self.id, e);
                ctx.stop();
                return;
            }
        };
        
        match msg {
            ws::Message::Text(text) => {
                self.last_heartbeat = Instant::now();
                self.handle_client_message(ctx, &text);
            }
            ws::Message::Binary(bin) => {
                // 尝试将二进制数据作为 JSON 处理
                if let Ok(text) = String::from_utf8(bin.to_vec()) {
                    self.last_heartbeat = Instant::now();
                    self.handle_client_message(ctx, &text);
                } else {
                    self.send_message(ctx, ServerMessage::error("INVALID_FORMAT", "不支持二进制消息格式"));
                }
            }
            ws::Message::Ping(msg) => {
                self.last_heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.last_heartbeat = Instant::now();
            }
            ws::Message::Close(reason) => {
                info!("WebSocket 客户端关闭连接: session={}, reason={:?}", self.id, reason);
                ctx.close(reason);
                ctx.stop();
            }
            ws::Message::Continuation(_) => {
                // 忽略
            }
            ws::Message::Nop => {}
        }
    }
}

/// 用于向 Session 推送数据的消息
#[derive(Message)]
#[rtype(result = "()")]
pub struct PushBatteryData {
    pub device_id: Uuid,
    pub data: crate::models::LatestBatteryResponse,
}

impl Handler<PushBatteryData> for WsSession {
    type Result = ();
    
    fn handle(&mut self, msg: PushBatteryData, ctx: &mut Self::Context) {
        // 检查是否订阅了该设备
        if self.subscribed_devices.contains(&msg.device_id) {
            self.send_message(ctx, ServerMessage::BatteryPush(BatteryPushMessage {
                device_id: msg.device_id,
                data: msg.data,
            }));
        }
    }
}
