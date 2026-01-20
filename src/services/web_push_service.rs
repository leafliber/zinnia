//! Web Push 推送服务
//! 
//! 提供 PWA Web Push 通知功能

use crate::config::Settings;
use crate::errors::AppError;
use crate::models::{WebPushSubscription};
use crate::repositories::NotificationRepository;
use base64::{engine::general_purpose, Engine};
use std::sync::Arc;
use uuid::Uuid;
use web_push::{
    ContentEncoding, SubscriptionInfo, VapidSignatureBuilder, WebPushClient, WebPushMessageBuilder,
};

/// Web Push 服务
pub struct WebPushService {
    client: WebPushClient,
    vapid_private_key: Vec<u8>,
    vapid_public_key: String,
    subject: String,
    notification_repo: Arc<NotificationRepository>,
}

impl WebPushService {
    /// 创建 Web Push 服务实例
    pub fn new(settings: &Settings, notification_repo: Arc<NotificationRepository>) -> Result<Self, AppError> {
        // 获取 VAPID 密钥
        let vapid_private_key_base64 = Settings::vapid_private_key()
            .ok_or_else(|| AppError::ConfigError("VAPID_PRIVATE_KEY 未设置".to_string()))?;
        
        let vapid_public_key = Settings::vapid_public_key()
            .ok_or_else(|| AppError::ConfigError("VAPID_PUBLIC_KEY 未设置".to_string()))?;

        // 解码私钥
        let vapid_private_key = general_purpose::URL_SAFE_NO_PAD
            .decode(vapid_private_key_base64.expose_secret())
            .map_err(|e| AppError::ConfigError(format!("VAPID 私钥解码失败: {}", e)))?;

        // 构建 subject (mailto: 或 https:)
        let subject = format!("mailto:{}", settings.smtp.from_email);

        let client = WebPushClient::new()
            .map_err(|e| AppError::InternalError(format!("创建 WebPush 客户端失败: {}", e)))?;

        Ok(Self {
            client,
            vapid_private_key,
            vapid_public_key,
            subject,
            notification_repo,
        })
    }

    /// 获取 VAPID 公钥（用于前端订阅）
    pub fn get_vapid_public_key(&self) -> &str {
        &self.vapid_public_key
    }

    /// 发送 Web Push 通知
    pub async fn send_notification(
        &self,
        subscription: &WebPushSubscription,
        title: &str,
        body: &str,
        data: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        // 构建通知负载
        let payload = serde_json::json!({
            "title": title,
            "body": body,
            "icon": "/icon-192.png",
            "badge": "/badge-72.png",
            "data": data.unwrap_or(serde_json::json!({})),
            "timestamp": chrono::Utc::now().timestamp_millis(),
        });

        let payload_json = serde_json::to_string(&payload)
            .map_err(|e| AppError::InternalError(format!("序列化通知负载失败: {}", e)))?;

        // 构建订阅信息
        let subscription_info = SubscriptionInfo {
            endpoint: subscription.endpoint.clone(),
            keys: web_push::SubscriptionKeys {
                p256dh: subscription.p256dh_key.clone(),
                auth: subscription.auth_secret.clone(),
            },
        };

        // 构建签名
        let sig_builder = VapidSignatureBuilder::from_base64_no_sub(
            &self.vapid_private_key,
            subscription_info.endpoint.as_str(),
        )
        .map_err(|e| AppError::InternalError(format!("创建 VAPID 签名失败: {}", e)))?;

        let signature = sig_builder
            .add_claim("sub", self.subject.as_str())
            .build()
            .map_err(|e| AppError::InternalError(format!("构建 VAPID 签名失败: {}", e)))?;

        // 构建消息
        let mut message_builder = WebPushMessageBuilder::new(&subscription_info);
        message_builder.set_payload(ContentEncoding::Aes128Gcm, payload_json.as_bytes());
        message_builder.set_vapid_signature(signature);

        let message = message_builder
            .build()
            .map_err(|e| AppError::InternalError(format!("构建推送消息失败: {}", e)))?;

        // 发送推送
        let response = self
            .client
            .send(message)
            .await
            .map_err(|e| {
                tracing::error!(
                    error = %e,
                    subscription_id = %subscription.id,
                    "Web Push 发送失败"
                );
                
                // 如果是 410 Gone 或 404 Not Found，标记订阅为不活跃
                if let web_push::WebPushError::EndpointNotValid = e {
                    // 异步标记订阅为不活跃（不阻塞）
                    let repo = self.notification_repo.clone();
                    let sub_id = subscription.id;
                    tokio::spawn(async move {
                        if let Err(e) = repo.deactivate_web_push_subscription(sub_id).await {
                            tracing::error!(error = %e, "标记订阅为不活跃失败");
                        }
                    });
                }
                
                AppError::InternalError(format!("Web Push 发送失败: {}", e))
            })?;

        // 检查响应状态
        if !response.is_success() {
            return Err(AppError::InternalError(format!(
                "Web Push 响应失败: {}",
                response.status_code
            )));
        }

        // 更新最后使用时间
        self.notification_repo
            .update_web_push_subscription_last_used(subscription.id)
            .await?;

        tracing::info!(
            subscription_id = %subscription.id,
            user_id = %subscription.user_id,
            "Web Push 通知已发送"
        );

        Ok(())
    }

    /// 批量发送通知到用户的所有订阅
    pub async fn send_to_user(
        &self,
        user_id: Uuid,
        title: &str,
        body: &str,
        data: Option<serde_json::Value>,
    ) -> Result<usize, AppError> {
        // 获取用户的所有活跃订阅
        let subscriptions = self
            .notification_repo
            .get_active_web_push_subscriptions(user_id)
            .await?;

        if subscriptions.is_empty() {
            tracing::debug!(user_id = %user_id, "用户没有活跃的 Web Push 订阅");
            return Ok(0);
        }

        let mut success_count = 0;

        // 并发发送到所有订阅
        let futures: Vec<_> = subscriptions
            .iter()
            .map(|sub| self.send_notification(sub, title, body, data.clone()))
            .collect();

        let results = futures::future::join_all(futures).await;

        for (idx, result) in results.iter().enumerate() {
            if result.is_ok() {
                success_count += 1;
            } else {
                tracing::warn!(
                    subscription_id = %subscriptions[idx].id,
                    "订阅推送失败"
                );
            }
        }

        tracing::info!(
            user_id = %user_id,
            total = subscriptions.len(),
            success = success_count,
            "批量 Web Push 发送完成"
        );

        Ok(success_count)
    }
}
