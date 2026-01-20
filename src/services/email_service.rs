//! 邮件服务模块
//! 
//! 提供 SMTP 邮件发送功能，包括验证码发送等

use crate::config::{Settings, SmtpSettings};
use crate::db::RedisPool;
use crate::errors::AppError;
use lettre::{
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use secrecy::ExposeSecret;
use std::sync::Arc;

/// 邮件服务
pub struct EmailService {
    mailer: Option<AsyncSmtpTransport<Tokio1Executor>>,
    settings: SmtpSettings,
    redis_pool: Arc<RedisPool>,
}

impl EmailService {
    /// 创建新的邮件服务实例
    pub fn new(settings: &Settings, redis_pool: Arc<RedisPool>) -> Result<Self, AppError> {
        let smtp_settings = settings.smtp.clone();
        
        let mailer = if smtp_settings.enabled {
            let password = Settings::smtp_password()
                .ok_or_else(|| AppError::ConfigError("SMTP_PASSWORD 未设置".to_string()))?;
            
            let creds = Credentials::new(
                smtp_settings.username.clone(),
                password.expose_secret().clone(),
            );

            let transport = if smtp_settings.tls {
                // 如果使用隐式 TLS（通常端口 465），使用 relay（implicit TLS）。
                // 否则使用 STARTTLS（常见于 587）。
                if smtp_settings.port == 465 {
                    AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_settings.host)
                        .map_err(|e| AppError::ConfigError(format!("SMTP 配置错误: {}", e)))?
                        .port(smtp_settings.port)
                        .credentials(creds)
                        .build()
                } else {
                    AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_settings.host)
                        .map_err(|e| AppError::ConfigError(format!("SMTP 配置错误: {}", e)))?
                        .port(smtp_settings.port)
                        .credentials(creds)
                        .build()
                }
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp_settings.host)
                    .port(smtp_settings.port)
                    .credentials(creds)
                    .build()
            };

            Some(transport)
        } else {
            tracing::warn!("SMTP 未启用，邮件功能将不可用");
            None
        };

        Ok(Self {
            mailer,
            settings: smtp_settings,
            redis_pool,
        })
    }

    /// 检查邮件服务是否可用
    pub fn is_enabled(&self) -> bool {
        self.mailer.is_some()
    }

    /// 检查是否超过发送频率限制
    async fn check_rate_limit(&self, email: &str) -> Result<(), AppError> {
        let key = format!("email:rate_limit:{}", email);
        let count: Option<u32> = self.redis_pool.get(&key).await?;
        
        if let Some(count) = count {
            if count >= self.settings.max_sends_per_hour {
                return Err(AppError::RateLimitExceeded(
                    "邮件发送过于频繁，请稍后再试".to_string()
                ));
            }
        }
        
        Ok(())
    }

    /// 记录发送次数
    async fn record_send(&self, email: &str) -> Result<(), AppError> {
        let key = format!("email:rate_limit:{}", email);
        let count: Option<u32> = self.redis_pool.get(&key).await?;
        
        let new_count = count.unwrap_or(0) + 1;
        // 设置 1 小时过期
        self.redis_pool.set_ex(&key, &new_count, 3600).await?;
        
        Ok(())
    }

    /// 发送验证码邮件
    pub async fn send_verification_code(
        &self,
        to_email: &str,
        code: &str,
        expires_minutes: u64,
    ) -> Result<(), AppError> {
        // 检查频率限制
        self.check_rate_limit(to_email).await?;

        let mailer = self.mailer.as_ref()
            .ok_or_else(|| AppError::ConfigError("邮件服务未启用".to_string()))?;

        let from = format!("{} <{}>", self.settings.from_name, self.settings.from_email);

        let email = Message::builder()
            .from(from.parse().map_err(|e| AppError::ConfigError(format!("发件人地址无效: {}", e)))?)
            .to(to_email.parse().map_err(|_| AppError::ValidationError("收件人邮箱格式无效".to_string()))?)
            .subject("【Zinnia】邮箱验证码")
            .body(format!(
                "您好！\n\n您的邮箱验证码是：{}\n\n验证码有效期为 {} 分钟，请尽快完成验证。\n\n如非本人操作，请忽略此邮件。\n\n——Zinnia 团队",
                code,
                expires_minutes
            ))
            .map_err(|e| AppError::InternalError(format!("邮件构建失败: {}", e)))?;

        mailer
            .send(email)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, to = %to_email, "邮件发送失败");
                AppError::InternalError("邮件发送失败，请稍后重试".to_string())
            })?;

        // 记录发送次数
        self.record_send(to_email).await?;

        tracing::info!(to = %to_email, "验证码邮件已发送");
        Ok(())
    }

    /// 发送密码重置邮件
    pub async fn send_password_reset_code(
        &self,
        to_email: &str,
        code: &str,
        expires_minutes: u64,
    ) -> Result<(), AppError> {
        // 检查频率限制
        self.check_rate_limit(to_email).await?;

        let mailer = self.mailer.as_ref()
            .ok_or_else(|| AppError::ConfigError("邮件服务未启用".to_string()))?;

        let from = format!("{} <{}>", self.settings.from_name, self.settings.from_email);

        let email = Message::builder()
            .from(from.parse().map_err(|e| AppError::ConfigError(format!("发件人地址无效: {}", e)))?)
            .to(to_email.parse().map_err(|_| AppError::ValidationError("收件人邮箱格式无效".to_string()))?)
            .subject("【Zinnia】密码重置验证码")
            .body(format!(
                "您好！\n\n您正在重置密码，验证码是：{}\n\n验证码有效期为 {} 分钟。\n\n如非本人操作，请立即修改您的密码。\n\n——Zinnia 团队",
                code,
                expires_minutes
            ))
            .map_err(|e| AppError::InternalError(format!("邮件构建失败: {}", e)))?;

        mailer
            .send(email)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, to = %to_email, "邮件发送失败");
                AppError::InternalError("邮件发送失败，请稍后重试".to_string())
            })?;

        // 记录发送次数
        self.record_send(to_email).await?;

        tracing::info!(to = %to_email, "密码重置邮件已发送");
        Ok(())
    }

    /// 发送欢迎邮件
    pub async fn send_welcome_email(&self, to_email: &str, username: &str) -> Result<(), AppError> {
        let mailer = self.mailer.as_ref()
            .ok_or_else(|| AppError::ConfigError("邮件服务未启用".to_string()))?;

        let from = format!("{} <{}>", self.settings.from_name, self.settings.from_email);

        let email = Message::builder()
            .from(from.parse().map_err(|e| AppError::ConfigError(format!("发件人地址无效: {}", e)))?)
            .to(to_email.parse().map_err(|_| AppError::ValidationError("收件人邮箱格式无效".to_string()))?)
            .subject("【Zinnia】欢迎加入")
            .body(format!(
                "亲爱的 {}，\n\n欢迎加入 Zinnia！\n\n您的账户已成功创建。现在您可以开始使用我们的服务了。\n\n如有任何问题，请随时联系我们。\n\n——Zinnia 团队",
                username
            ))
            .map_err(|e| AppError::InternalError(format!("邮件构建失败: {}", e)))?;

        mailer
            .send(email)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, to = %to_email, "欢迎邮件发送失败");
                // 欢迎邮件发送失败不应阻止注册流程
                AppError::InternalError("邮件发送失败".to_string())
            })?;

        tracing::info!(to = %to_email, "欢迎邮件已发送");
        Ok(())
    }

    /// 发送预警通知邮件
    pub async fn send_alert_notification(
        &self,
        to_email: &str,
        alert_type: &str,
        level: &str,
        message: &str,
        device_name: &str,
        value: f64,
        threshold: f64,
        triggered_at: &str,
    ) -> Result<(), AppError> {
        let mailer = self.mailer.as_ref()
            .ok_or_else(|| AppError::ConfigError("邮件服务未启用".to_string()))?;

        let from = format!("{} <{}>", self.settings.from_name, self.settings.from_email);

        // 根据级别确定邮件主题前缀
        let level_prefix = match level {
            "critical" => "🔴 严重预警",
            "warning" => "🟡 警告",
            "info" => "ℹ️ 信息",
            _ => "预警通知",
        };

        let subject = format!("【Zinnia】{} - {}", level_prefix, alert_type);

        // 构建详细的邮件正文
        let body = format!(
            r#"您好！

您的设备触发了预警：

📱 设备名称：{}
⚠️  预警类型：{}
📊 预警级别：{}
📝 预警信息：{}

详细信息：
• 当前值：{:.2}
• 阈值：{:.2}
• 触发时间：{}

建议：
{}

如需查看更多详情或管理预警，请登录 Zinnia 控制台。

此邮件由系统自动发送，请勿直接回复。

——Zinnia 团队"#,
            device_name,
            alert_type,
            level,
            message,
            value,
            threshold,
            triggered_at,
            get_alert_suggestion(alert_type, level)
        );

        let email = Message::builder()
            .from(from.parse().map_err(|e| AppError::ConfigError(format!("发件人地址无效: {}", e)))?)
            .to(to_email.parse().map_err(|_| AppError::ValidationError("收件人邮箱格式无效".to_string()))?)
            .subject(subject)
            .body(body)
            .map_err(|e| AppError::InternalError(format!("邮件构建失败: {}", e)))?;

        mailer
            .send(email)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, to = %to_email, "预警邮件发送失败");
                AppError::InternalError("邮件发送失败，请稍后重试".to_string())
            })?;

        tracing::info!(to = %to_email, alert_type = %alert_type, level = %level, "预警邮件已发送");
        Ok(())
    }
}

/// 根据预警类型和级别获取建议
fn get_alert_suggestion(alert_type: &str, level: &str) -> &'static str {
    match (alert_type, level) {
        ("LowBattery" | "low_battery", _) => "• 建议及时为设备充电\n• 检查是否有后台应用过度耗电",
        ("CriticalBattery" | "critical_battery", _) => "• 请立即为设备充电\n• 关闭非必要应用以延长续航",
        ("HighTemperature" | "high_temperature", _) => "• 请将设备移至通风良好的环境\n• 避免在充电时使用高负载应用\n• 如持续高温请检查设备状态",
        ("DeviceOffline" | "device_offline", _) => "• 检查设备网络连接\n• 确认设备是否正常运行\n• 查看设备电池状态",
        ("RapidDrain" | "rapid_drain", _) => "• 检查是否有异常应用占用资源\n• 考虑启用省电模式\n• 检查系统更新",
        _ => "• 请及时检查设备状态\n• 如有疑问请联系技术支持",
    }
}
