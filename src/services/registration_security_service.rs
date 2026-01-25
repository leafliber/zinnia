//! 注册安全服务模块
//!
//! 提供注册过程中的安全检查，包括 IP 频率限制、恶意行为检测等

use crate::config::{RegistrationSettings, Settings};
use crate::db::RedisPool;
use crate::errors::AppError;
use serde::Serialize;
use std::sync::Arc;

/// 注册限制检查结果
#[derive(Debug, Clone, Serialize)]
pub struct RegistrationCheckResult {
    /// 是否允许注册
    pub allowed: bool,
    /// 拒绝原因
    pub reason: Option<String>,
    /// 剩余允许次数（当前小时）
    pub remaining_hourly: u32,
    /// 剩余允许次数（当天）
    pub remaining_daily: u32,
}

/// 注册安全服务
pub struct RegistrationSecurityService {
    redis_pool: Arc<RedisPool>,
    settings: RegistrationSettings,
}

impl RegistrationSecurityService {
    pub fn new(redis_pool: Arc<RedisPool>, settings: &Settings) -> Self {
        Self {
            redis_pool,
            settings: settings.registration.clone(),
        }
    }

    /// 获取小时级 Redis 键
    fn get_hourly_key(&self, ip: &str) -> String {
        let hour = chrono::Utc::now().format("%Y%m%d%H");
        format!("reg:ip:hourly:{}:{}", ip, hour)
    }

    /// 获取日级 Redis 键
    fn get_daily_key(&self, ip: &str) -> String {
        let day = chrono::Utc::now().format("%Y%m%d");
        format!("reg:ip:daily:{}:{}", ip, day)
    }

    /// 获取可疑 IP 键
    fn get_suspicious_key(&self, ip: &str) -> String {
        format!("reg:suspicious:{}", ip)
    }

    /// 检查 IP 是否可以注册
    pub async fn check_ip(&self, ip: &str) -> Result<RegistrationCheckResult, AppError> {
        // 检查是否是可疑 IP
        let suspicious_key = self.get_suspicious_key(ip);
        let is_suspicious: Option<bool> = self.redis_pool.get(&suspicious_key).await?;

        if is_suspicious == Some(true) {
            return Ok(RegistrationCheckResult {
                allowed: false,
                reason: Some("该 IP 地址已被临时限制注册".to_string()),
                remaining_hourly: 0,
                remaining_daily: 0,
            });
        }

        // 获取小时级计数
        let hourly_key = self.get_hourly_key(ip);
        let hourly_count: Option<u32> = self.redis_pool.get(&hourly_key).await?;
        let hourly_count = hourly_count.unwrap_or(0);

        // 获取日级计数
        let daily_key = self.get_daily_key(ip);
        let daily_count: Option<u32> = self.redis_pool.get(&daily_key).await?;
        let daily_count = daily_count.unwrap_or(0);

        // 计算剩余次数
        let remaining_hourly = self
            .settings
            .max_per_ip_per_hour
            .saturating_sub(hourly_count);
        let remaining_daily = self.settings.max_per_ip_per_day.saturating_sub(daily_count);

        // 检查是否超限
        if hourly_count >= self.settings.max_per_ip_per_hour {
            tracing::warn!(ip = %ip, hourly_count = hourly_count, "IP 每小时注册次数超限");
            return Ok(RegistrationCheckResult {
                allowed: false,
                reason: Some("注册过于频繁，请稍后再试".to_string()),
                remaining_hourly: 0,
                remaining_daily,
            });
        }

        if daily_count >= self.settings.max_per_ip_per_day {
            tracing::warn!(ip = %ip, daily_count = daily_count, "IP 每日注册次数超限");
            return Ok(RegistrationCheckResult {
                allowed: false,
                reason: Some("今日注册次数已达上限，请明天再试".to_string()),
                remaining_hourly,
                remaining_daily: 0,
            });
        }

        Ok(RegistrationCheckResult {
            allowed: true,
            reason: None,
            remaining_hourly,
            remaining_daily,
        })
    }

    /// 记录一次注册
    pub async fn record_registration(&self, ip: &str) -> Result<(), AppError> {
        // 增加小时级计数
        let hourly_key = self.get_hourly_key(ip);
        let hourly_count: Option<u32> = self.redis_pool.get(&hourly_key).await?;
        let new_hourly = hourly_count.unwrap_or(0) + 1;
        // 设置 1 小时过期
        self.redis_pool
            .set_ex(&hourly_key, &new_hourly, 3600)
            .await?;

        // 增加日级计数
        let daily_key = self.get_daily_key(ip);
        let daily_count: Option<u32> = self.redis_pool.get(&daily_key).await?;
        let new_daily = daily_count.unwrap_or(0) + 1;
        // 设置 24 小时过期
        self.redis_pool
            .set_ex(&daily_key, &new_daily, 86400)
            .await?;

        // 检测可疑行为
        self.detect_suspicious_behavior(ip, new_hourly, new_daily)
            .await?;

        tracing::info!(
            ip = %ip,
            hourly_count = new_hourly,
            daily_count = new_daily,
            "记录注册行为"
        );

        Ok(())
    }

    /// 检测可疑行为
    async fn detect_suspicious_behavior(
        &self,
        ip: &str,
        hourly_count: u32,
        daily_count: u32,
    ) -> Result<(), AppError> {
        // 如果超过限制的 80%，标记为可疑
        let hourly_threshold = (self.settings.max_per_ip_per_hour as f64 * 0.8) as u32;
        let daily_threshold = (self.settings.max_per_ip_per_day as f64 * 0.8) as u32;

        if hourly_count >= hourly_threshold || daily_count >= daily_threshold {
            let suspicious_key = self.get_suspicious_key(ip);
            // 设置可疑标记，24 小时后自动解除
            self.redis_pool
                .set_ex(&suspicious_key, &true, 86400)
                .await?;

            tracing::warn!(
                ip = %ip,
                hourly_count = hourly_count,
                daily_count = daily_count,
                "检测到可疑注册行为，已标记 IP"
            );
        }

        Ok(())
    }

    /// 标记 IP 为可疑
    pub async fn mark_suspicious(&self, ip: &str, duration_hours: u64) -> Result<(), AppError> {
        let suspicious_key = self.get_suspicious_key(ip);
        self.redis_pool
            .set_ex(&suspicious_key, &true, duration_hours * 3600)
            .await?;

        tracing::warn!(ip = %ip, duration_hours = duration_hours, "手动标记 IP 为可疑");

        Ok(())
    }

    /// 解除 IP 可疑标记
    pub async fn clear_suspicious(&self, ip: &str) -> Result<(), AppError> {
        let suspicious_key = self.get_suspicious_key(ip);
        self.redis_pool.del(&suspicious_key).await?;

        tracing::info!(ip = %ip, "解除 IP 可疑标记");

        Ok(())
    }

    /// 获取 IP 注册统计
    pub async fn get_ip_stats(&self, ip: &str) -> Result<serde_json::Value, AppError> {
        let hourly_key = self.get_hourly_key(ip);
        let daily_key = self.get_daily_key(ip);
        let suspicious_key = self.get_suspicious_key(ip);

        let hourly_count: Option<u32> = self.redis_pool.get(&hourly_key).await?;
        let daily_count: Option<u32> = self.redis_pool.get(&daily_key).await?;
        let is_suspicious: Option<bool> = self.redis_pool.get(&suspicious_key).await?;

        Ok(serde_json::json!({
            "ip": ip,
            "hourly_count": hourly_count.unwrap_or(0),
            "daily_count": daily_count.unwrap_or(0),
            "max_hourly": self.settings.max_per_ip_per_hour,
            "max_daily": self.settings.max_per_ip_per_day,
            "is_suspicious": is_suspicious.unwrap_or(false),
        }))
    }

    /// 检查是否需要邮箱验证
    pub fn require_email_verification(&self) -> bool {
        self.settings.require_email_verification
    }

    /// 检查是否需要 reCAPTCHA
    pub fn require_recaptcha(&self) -> bool {
        self.settings.require_recaptcha
    }
}
