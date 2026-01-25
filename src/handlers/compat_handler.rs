//! 兼容模式 API 处理器
//!
//! 用于不支持设置 HTTP 请求头的设备（如某些 IoT 设备、低功耗设备）
//! 通过 URL 查询参数传递认证令牌和数据

use crate::errors::AppError;
use crate::models::{
    ApiResponse, BatteryReportRequest, CompatBatteryReportQuery, PowerSavingMode, TokenPermission,
};
use crate::services::{BatteryService, DeviceAccessTokenService};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 兼容模式电量查询参数
#[derive(Debug, Deserialize)]
pub struct CompatBatteryQuery {
    /// 设备访问令牌
    pub token: String,
}

/// 简化的电量响应
#[derive(Debug, Serialize)]
pub struct CompatBatteryResponse {
    pub level: i32,
    pub charging: bool,
    pub timestamp: i64,
}

/// 获取客户端 IP
fn get_client_ip(req: &HttpRequest) -> Option<String> {
    // 尝试从 X-Forwarded-For 获取
    if let Some(forwarded) = req.headers().get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // 获取第一个 IP（客户端 IP）
            return forwarded_str
                .split(',')
                .next()
                .map(|s| s.trim().to_string());
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

/// 兼容模式 - 上报电量
/// GET/POST /api/v1/compat/battery/report?token=xxx&level=75&charging=true&...
///
/// 支持 GET 方法，便于资源受限的设备直接通过 URL 上报数据
pub async fn compat_report_battery(
    req: HttpRequest,
    token_service: web::Data<Arc<DeviceAccessTokenService>>,
    battery_service: web::Data<Arc<BatteryService>>,
    query: web::Query<CompatBatteryReportQuery>,
) -> Result<HttpResponse, AppError> {
    let client_ip = get_client_ip(&req);

    // 验证令牌
    let (token_info, device_id) = token_service
        .validate_token(&query.token, client_ip.as_deref())
        .await?;

    // 检查写入权限
    if !token_info.can_write() {
        return Err(AppError::Forbidden("令牌没有写入权限".to_string()));
    }

    // 验证电量值
    if query.level < 0 || query.level > 100 {
        return Err(AppError::ValidationError(
            "电量值必须在 0-100 之间".to_string(),
        ));
    }

    // 构建上报请求
    let report = BatteryReportRequest {
        battery_level: query.level,
        is_charging: query.charging.map(|c| c != 0).unwrap_or(false),
        power_saving_mode: PowerSavingMode::Off,
        temperature: query.temp,
        voltage: query.voltage,
        recorded_at: query
            .ts
            .and_then(|ts| chrono::TimeZone::timestamp_opt(&chrono::Utc, ts, 0).single()),
    };

    // 上报数据
    let data = battery_service.report(device_id, report).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(data)))
}

/// 兼容模式 - 获取最新电量
/// GET /api/v1/compat/battery/latest?token=xxx
pub async fn compat_get_latest_battery(
    req: HttpRequest,
    token_service: web::Data<Arc<DeviceAccessTokenService>>,
    battery_service: web::Data<Arc<BatteryService>>,
    query: web::Query<CompatBatteryQuery>,
) -> Result<HttpResponse, AppError> {
    let client_ip = get_client_ip(&req);

    // 验证令牌
    let (token_info, device_id) = token_service
        .validate_token(&query.token, client_ip.as_deref())
        .await?;

    // 检查读取权限
    if !token_info.can_read() {
        return Err(AppError::Forbidden("令牌没有读取权限".to_string()));
    }

    // 获取最新电量
    let response = battery_service.get_latest(device_id).await?;

    // 转换为简化响应
    let compat_response = CompatBatteryResponse {
        level: response.battery_level,
        charging: response.is_charging,
        timestamp: response.recorded_at.timestamp(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(compat_response)))
}

/// 兼容模式 - 极简上报（仅电量和充电状态）
/// GET /api/v1/compat/battery/simple?token=xxx&l=75&c=1
///
/// 用于带宽/资源极其受限的设备，使用最短的参数名
#[derive(Debug, Deserialize)]
pub struct SimpleReportQuery {
    /// 令牌
    pub token: String,
    /// 电量 (level)
    pub l: i32,
    /// 充电状态 (charging): 1=true, 0=false
    #[serde(default)]
    pub c: u8,
}

pub async fn compat_simple_report(
    req: HttpRequest,
    token_service: web::Data<Arc<DeviceAccessTokenService>>,
    battery_service: web::Data<Arc<BatteryService>>,
    query: web::Query<SimpleReportQuery>,
) -> Result<HttpResponse, AppError> {
    let client_ip = get_client_ip(&req);

    // 验证令牌
    let (token_info, device_id) = token_service
        .validate_token(&query.token, client_ip.as_deref())
        .await?;

    // 检查写入权限
    if !token_info.can_write() {
        return Err(AppError::Forbidden("令牌没有写入权限".to_string()));
    }

    // 验证电量值
    if query.l < 0 || query.l > 100 {
        return Err(AppError::ValidationError(
            "电量值必须在 0-100 之间".to_string(),
        ));
    }

    // 构建上报请求
    let report = BatteryReportRequest {
        battery_level: query.l,
        is_charging: query.c != 0,
        power_saving_mode: PowerSavingMode::Off,
        temperature: None,
        voltage: None,
        recorded_at: None,
    };

    // 上报数据
    let data = battery_service.report(device_id, report).await?;

    // 返回极简响应
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "ok": true,
        "ts": data.recorded_at.timestamp()
    })))
}

/// 兼容模式 - 健康检查
/// GET /api/v1/compat/ping?token=xxx
///
/// 用于设备验证令牌是否有效
pub async fn compat_ping(
    req: HttpRequest,
    token_service: web::Data<Arc<DeviceAccessTokenService>>,
    query: web::Query<CompatBatteryQuery>,
) -> Result<HttpResponse, AppError> {
    let client_ip = get_client_ip(&req);

    // 验证令牌
    let (token_info, device_id) = token_service
        .validate_token(&query.token, client_ip.as_deref())
        .await?;

    let permission = match token_info.permission {
        TokenPermission::Read => "read",
        TokenPermission::Write => "write",
        TokenPermission::All => "all",
    };

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "ok": true,
        "device_id": device_id,
        "permission": permission,
        "expires_at": token_info.expires_at
    })))
}
