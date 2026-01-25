//! 通用数据结构

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 统一 API 响应结构
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: u16,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    /// 创建成功响应
    pub fn success(data: T) -> Self {
        Self {
            code: 200,
            message: "success".to_string(),
            data: Some(data),
            timestamp: Utc::now(),
            request_id: None,
        }
    }

    /// 创建成功响应（无数据）
    pub fn success_message(message: &str) -> ApiResponse<()> {
        ApiResponse {
            code: 200,
            message: message.to_string(),
            data: None,
            timestamp: Utc::now(),
            request_id: None,
        }
    }

    /// 创建创建成功响应 (201)
    pub fn created(data: T) -> Self {
        Self {
            code: 201,
            message: "created".to_string(),
            data: Some(data),
            timestamp: Utc::now(),
            request_id: None,
        }
    }

    /// 设置请求 ID
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
}

/// 分页信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub page: i64,
    pub page_size: i64,
    pub total_items: i64,
    pub total_pages: i64,
}

impl Pagination {
    pub fn new(page: i64, page_size: i64, total_items: i64) -> Self {
        let total_pages = (total_items as f64 / page_size as f64).ceil() as i64;
        Self {
            page,
            page_size,
            total_items,
            total_pages,
        }
    }

    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }
}

/// 分页响应
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub items: Vec<T>,
    pub pagination: Pagination,
}

impl<T: Serialize> PaginatedResponse<T> {
    pub fn new(items: Vec<T>, pagination: Pagination) -> Self {
        Self { items, pagination }
    }
}

/// 时间范围
#[derive(Debug, Clone, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeRange {
    /// 验证时间范围
    pub fn validate(&self, max_days: i64) -> Result<(), String> {
        if self.start > self.end {
            return Err("开始时间不能晚于结束时间".to_string());
        }

        let duration = self.end - self.start;
        if duration.num_days() > max_days {
            return Err(format!("时间范围不能超过 {} 天", max_days));
        }

        Ok(())
    }
}

/// 健康检查响应
#[derive(Debug, Serialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub version: String,
    pub database: ServiceStatus,
    pub redis: ServiceStatus,
    pub uptime_seconds: u64,
}

/// 服务状态
#[derive(Debug, Serialize)]
pub struct ServiceStatus {
    pub status: String,
    pub latency_ms: Option<u64>,
}

impl ServiceStatus {
    pub fn healthy(latency_ms: u64) -> Self {
        Self {
            status: "healthy".to_string(),
            latency_ms: Some(latency_ms),
        }
    }

    pub fn unhealthy() -> Self {
        Self {
            status: "unhealthy".to_string(),
            latency_ms: None,
        }
    }
}
