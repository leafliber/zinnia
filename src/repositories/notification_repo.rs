//! 通知偏好数据仓库

use crate::db::PostgresPool;
use crate::errors::AppError;
use crate::models::{
    NotificationChannel, NotificationHistory, SubscribeWebPushRequest,
    UpdateNotificationPreferenceRequest, UserNotificationPreference, WebPushSubscription,
};
use chrono::{NaiveTime, Utc};
use uuid::Uuid;

/// 通知偏好数据仓库
#[derive(Clone)]
pub struct NotificationRepository {
    pool: PostgresPool,
}

impl NotificationRepository {
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    // ========== 用户通知偏好 ==========

    /// 获取用户的通知偏好
    pub async fn get_user_preference(
        &self,
        user_id: Uuid,
    ) -> Result<Option<UserNotificationPreference>, AppError> {
        let pref = sqlx::query_as::<_, UserNotificationPreference>(
            "SELECT * FROM user_notification_preferences WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(pref)
    }

    /// 创建或更新用户的通知偏好
    pub async fn upsert_user_preference(
        &self,
        user_id: Uuid,
        request: &UpdateNotificationPreferenceRequest,
    ) -> Result<UserNotificationPreference, AppError> {
        let now = Utc::now();

        // 解析安静时段
        let quiet_start = request
            .quiet_hours_start
            .as_ref()
            .and_then(|s| NaiveTime::parse_from_str(s, "%H:%M").ok());

        let quiet_end = request
            .quiet_hours_end
            .as_ref()
            .and_then(|s| NaiveTime::parse_from_str(s, "%H:%M").ok());

        // 序列化配置为 JSONB
        let email_config = request
            .email_config
            .as_ref()
            .and_then(|c| serde_json::to_value(c).ok());

        let webhook_config = request
            .webhook_config
            .as_ref()
            .and_then(|c| serde_json::to_value(c).ok());

        let pref = sqlx::query_as::<_, UserNotificationPreference>(
            r#"
            INSERT INTO user_notification_preferences (
                id, user_id, enabled,
                email_config, webhook_config,
                notify_info, notify_warning, notify_critical,
                quiet_hours_start, quiet_hours_end, quiet_hours_timezone,
                min_notification_interval,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3,
                $4, $5,
                $6, $7, $8,
                $9, $10, $11,
                $12,
                $13, $14
            )
            ON CONFLICT (user_id) DO UPDATE SET
                enabled = COALESCE($3, user_notification_preferences.enabled),
                email_config = COALESCE($4, user_notification_preferences.email_config),
                webhook_config = COALESCE($5, user_notification_preferences.webhook_config),
                notify_info = COALESCE($6, user_notification_preferences.notify_info),
                notify_warning = COALESCE($7, user_notification_preferences.notify_warning),
                notify_critical = COALESCE($8, user_notification_preferences.notify_critical),
                quiet_hours_start = COALESCE($9, user_notification_preferences.quiet_hours_start),
                quiet_hours_end = COALESCE($10, user_notification_preferences.quiet_hours_end),
                quiet_hours_timezone = COALESCE($11, user_notification_preferences.quiet_hours_timezone),
                min_notification_interval = COALESCE($12, user_notification_preferences.min_notification_interval),
                updated_at = $14
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(request.enabled)
        .bind(email_config)
        .bind(webhook_config)
        .bind(request.notify_info)
        .bind(request.notify_warning)
        .bind(request.notify_critical)
        .bind(quiet_start)
        .bind(quiet_end)
        .bind(&request.quiet_hours_timezone)
        .bind(request.min_notification_interval)
        .bind(now)
        .bind(now)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(pref)
    }

    // ========== 通知历史 ==========

    /// 创建通知历史记录
    pub async fn create_notification_history(
        &self,
        alert_event_id: Uuid,
        user_id: Uuid,
        channel: NotificationChannel,
        recipient: &str,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<NotificationHistory, AppError> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let history = sqlx::query_as::<_, NotificationHistory>(
            r#"
            INSERT INTO notification_history (
                id, alert_event_id, user_id, channel, recipient,
                status, error_message, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(alert_event_id)
        .bind(user_id)
        .bind(channel)
        .bind(recipient)
        .bind(status)
        .bind(error_message)
        .bind(now)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(history)
    }

    /// 更新通知历史状态
    pub async fn update_notification_status(
        &self,
        history_id: Uuid,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<(), AppError> {
        let now = Utc::now();

        sqlx::query(
            r#"
            UPDATE notification_history
            SET status = $2, error_message = $3, sent_at = $4
            WHERE id = $1
            "#,
        )
        .bind(history_id)
        .bind(status)
        .bind(error_message)
        .bind(now)
        .execute(self.pool.pool())
        .await?;

        Ok(())
    }

    /// 获取用户的通知历史
    pub async fn get_notification_history(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<NotificationHistory>, i64), AppError> {
        let histories = sqlx::query_as::<_, NotificationHistory>(
            r#"
            SELECT * FROM notification_history
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool.pool())
        .await?;

        let total: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM notification_history WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(self.pool.pool())
                .await?;

        Ok((histories, total.0))
    }

    /// 检查最近的通知时间（用于频率控制）
    pub async fn get_last_notification_time(
        &self,
        user_id: Uuid,
        channel: NotificationChannel,
    ) -> Result<Option<chrono::DateTime<Utc>>, AppError> {
        let result: Option<(chrono::DateTime<Utc>,)> = sqlx::query_as(
            r#"
            SELECT created_at FROM notification_history
            WHERE user_id = $1 AND channel = $2 AND status = 'sent'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(channel)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(result.map(|r| r.0))
    }

    // ========== Web Push 订阅管理 ==========

    /// 创建或更新 Web Push 订阅
    pub async fn upsert_web_push_subscription(
        &self,
        user_id: Uuid,
        request: &SubscribeWebPushRequest,
        user_agent: Option<&str>,
    ) -> Result<WebPushSubscription, AppError> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let subscription = sqlx::query_as::<_, WebPushSubscription>(
            r#"
            INSERT INTO web_push_subscriptions (
                id, user_id, endpoint, p256dh_key, auth_secret,
                user_agent, device_name, is_active, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (user_id, endpoint) DO UPDATE SET
                p256dh_key = EXCLUDED.p256dh_key,
                auth_secret = EXCLUDED.auth_secret,
                device_name = EXCLUDED.device_name,
                is_active = TRUE,
                updated_at = EXCLUDED.updated_at
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(user_id)
        .bind(&request.endpoint)
        .bind(&request.p256dh_key)
        .bind(&request.auth_secret)
        .bind(user_agent)
        .bind(&request.device_name)
        .bind(true)
        .bind(now)
        .bind(now)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(subscription)
    }

    /// 获取用户的所有活跃订阅
    pub async fn get_active_web_push_subscriptions(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<WebPushSubscription>, AppError> {
        let subscriptions = sqlx::query_as::<_, WebPushSubscription>(
            r#"
            SELECT * FROM web_push_subscriptions
            WHERE user_id = $1 AND is_active = TRUE
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(self.pool.pool())
        .await?;

        Ok(subscriptions)
    }

    /// 删除订阅（取消订阅）
    pub async fn delete_web_push_subscription(
        &self,
        user_id: Uuid,
        subscription_id: Uuid,
    ) -> Result<(), AppError> {
        let result =
            sqlx::query("DELETE FROM web_push_subscriptions WHERE id = $1 AND user_id = $2")
                .bind(subscription_id)
                .bind(user_id)
                .execute(self.pool.pool())
                .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "订阅不存在或无权删除: {}",
                subscription_id
            )));
        }

        Ok(())
    }

    /// 标记订阅为不活跃（推送失败时）
    pub async fn deactivate_web_push_subscription(
        &self,
        subscription_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE web_push_subscriptions
            SET is_active = FALSE, updated_at = $2
            WHERE id = $1
            "#,
        )
        .bind(subscription_id)
        .bind(Utc::now())
        .execute(self.pool.pool())
        .await?;

        Ok(())
    }

    /// 更新订阅的最后使用时间
    pub async fn update_web_push_subscription_last_used(
        &self,
        subscription_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE web_push_subscriptions SET last_used_at = $2 WHERE id = $1")
            .bind(subscription_id)
            .bind(Utc::now())
            .execute(self.pool.pool())
            .await?;

        Ok(())
    }

    /// 获取用户的活跃订阅数量
    pub async fn count_active_web_push_subscriptions(
        &self,
        user_id: Uuid,
    ) -> Result<i64, AppError> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM web_push_subscriptions WHERE user_id = $1 AND is_active = TRUE",
        )
        .bind(user_id)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(count.0)
    }
}
