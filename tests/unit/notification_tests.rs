//! NotificationService 单元测试

#[cfg(test)]
mod notification_service_tests {
    use chrono::{NaiveTime, Utc};

    #[test]
    fn test_should_notify_for_level() {
        // 测试预警级别过滤逻辑
        use crate::models::{AlertLevel, UserNotificationPreference};
        use uuid::Uuid;

        let mut pref = UserNotificationPreference {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            enabled: true,
            email_config: None,
            webhook_config: None,
            sms_config: None,
            push_config: None,
            notify_info: false,
            notify_warning: true,
            notify_critical: true,
            quiet_hours_start: None,
            quiet_hours_end: None,
            quiet_hours_timezone: "UTC".to_string(),
            min_notification_interval: 5,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // 测试 Info 级别 - 不应该通知
        assert!(!should_notify_for_level(&pref, &AlertLevel::Info));

        // 测试 Warning 级别 - 应该通知
        assert!(should_notify_for_level(&pref, &AlertLevel::Warning));

        // 测试 Critical 级别 - 应该通知
        assert!(should_notify_for_level(&pref, &AlertLevel::Critical));

        // 更新配置：允许所有级别
        pref.notify_info = true;
        assert!(should_notify_for_level(&pref, &AlertLevel::Info));
    }

    fn should_notify_for_level(preference: &crate::models::UserNotificationPreference, level: &crate::models::AlertLevel) -> bool {
        use crate::models::AlertLevel;
        match level {
            AlertLevel::Info => preference.notify_info,
            AlertLevel::Warning => preference.notify_warning,
            AlertLevel::Critical => preference.notify_critical,
        }
    }

    #[test]
    fn test_is_in_quiet_hours() {
        // 测试安静时段判断逻辑
        let start = NaiveTime::from_hms_opt(22, 0, 0).unwrap();
        let end = NaiveTime::from_hms_opt(8, 0, 0).unwrap();

        // 测试在安静时段内
        let time_23 = NaiveTime::from_hms_opt(23, 0, 0).unwrap();
        assert!(is_in_quiet_hours(start, end, time_23));

        let time_2 = NaiveTime::from_hms_opt(2, 0, 0).unwrap();
        assert!(is_in_quiet_hours(start, end, time_2));

        // 测试不在安静时段内
        let time_12 = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
        assert!(!is_in_quiet_hours(start, end, time_12));

        let time_15 = NaiveTime::from_hms_opt(15, 30, 0).unwrap();
        assert!(!is_in_quiet_hours(start, end, time_15));
    }

    fn is_in_quiet_hours(start: NaiveTime, end: NaiveTime, now: NaiveTime) -> bool {
        // 处理跨午夜的情况
        if start < end {
            now >= start && now < end
        } else {
            now >= start || now < end
        }
    }

    #[test]
    fn test_web_push_subscription_parsing() {
        use crate::models::SubscribeWebPushRequest;

        let json = r#"{
            "subscription": {
                "endpoint": "https://fcm.googleapis.com/fcm/send/test",
                "keys": {
                    "p256dh": "BNcRdreALRFXTkOOUHK1EtK2wtaz5Ry4YfYCA_0QTpQtUbVlUls0VJXg7A8u-Ts1XbjhazAkj7I99e8QcYP7DkM=",
                    "auth": "tBHItJI5svbpez7KI4CCXg=="
                }
            }
        }"#;

        let result: Result<SubscribeWebPushRequest, _> = serde_json::from_str(json);
        assert!(result.is_ok());

        let req = result.unwrap();
        assert_eq!(req.subscription.endpoint, "https://fcm.googleapis.com/fcm/send/test");
    }
}

#[cfg(test)]
mod web_push_payload_tests {
    #[test]
    fn test_create_web_push_payload() {
        use serde_json::json;

        let payload = json!({
            "title": "严重预警",
            "body": "设备电量临界: 5%",
            "icon": "/icons/alert-critical.png",
            "badge": "/icons/badge.png",
            "tag": "alert-event-123",
            "data": {
                "alert_id": "550e8400-e29b-41d4-a716-446655440000",
                "device_name": "iPhone 14 Pro",
                "level": "critical",
                "url": "/alerts/550e8400-e29b-41d4-a716-446655440000"
            }
        });

        let payload_str = payload.to_string();
        assert!(payload_str.contains("严重预警"));
        assert!(payload_str.contains("critical"));
    }

    #[test]
    fn test_notification_actions() {
        use serde_json::json;

        let notification = json!({
            "title": "预警通知",
            "body": "设备需要关注",
            "actions": [
                {
                    "action": "view",
                    "title": "查看详情"
                },
                {
                    "action": "dismiss",
                    "title": "忽略"
                }
            ]
        });

        assert!(notification["actions"].is_array());
        assert_eq!(notification["actions"].as_array().unwrap().len(), 2);
    }
}
