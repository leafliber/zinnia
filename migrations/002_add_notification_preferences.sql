-- 002: 添加通知偏好设置
-- 为用户添加通知配置，支持多种通知渠道的扩展

-- ============================================
-- 1. 通知渠道枚举类型
-- ============================================
CREATE TYPE notification_channel AS ENUM ('email', 'webhook', 'sms', 'push');

-- ============================================
-- 2. 用户通知偏好表
-- ============================================
CREATE TABLE IF NOT EXISTS user_notification_preferences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- 全局通知开关
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    
    -- 各渠道配置 (JSONB 存储灵活配置)
    -- 邮件配置示例: {"enabled": true, "email": "user@example.com"}
    -- Webhook配置示例: {"enabled": true, "url": "https://...", "secret": "..."}
    email_config JSONB,
    webhook_config JSONB,
    sms_config JSONB,
    push_config JSONB,
    
    -- 预警级别过滤 (哪些级别的预警需要通知)
    notify_info BOOLEAN NOT NULL DEFAULT FALSE,
    notify_warning BOOLEAN NOT NULL DEFAULT TRUE,
    notify_critical BOOLEAN NOT NULL DEFAULT TRUE,
    
    -- 时间窗口限制 (可选：安静时段)
    quiet_hours_start TIME,
    quiet_hours_end TIME,
    quiet_hours_timezone VARCHAR(50) DEFAULT 'UTC',
    
    -- 通知频率控制 (分钟)
    min_notification_interval INTEGER NOT NULL DEFAULT 5,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 每个用户只能有一个通知偏好配置
CREATE UNIQUE INDEX idx_notification_prefs_user_id ON user_notification_preferences(user_id);
CREATE INDEX idx_notification_prefs_enabled ON user_notification_preferences(enabled) WHERE enabled = TRUE;

-- ============================================
-- 3. 通知历史记录表
-- ============================================
CREATE TABLE IF NOT EXISTS notification_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    alert_event_id UUID NOT NULL REFERENCES alert_events(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    channel notification_channel NOT NULL,
    recipient TEXT NOT NULL,  -- 邮箱地址、Webhook URL等
    
    status VARCHAR(20) NOT NULL,  -- 'pending', 'sent', 'failed', 'skipped'
    error_message TEXT,
    
    sent_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_notification_history_alert_event ON notification_history(alert_event_id);
CREATE INDEX idx_notification_history_user_id ON notification_history(user_id);
CREATE INDEX idx_notification_history_status ON notification_history(status);
CREATE INDEX idx_notification_history_created_at ON notification_history(created_at DESC);

-- ============================================
-- 4. 为现有用户创建默认通知偏好
-- ============================================
INSERT INTO user_notification_preferences (user_id, email_config)
SELECT 
    id,
    jsonb_build_object(
        'enabled', TRUE,
        'email', email
    )
FROM users
WHERE NOT EXISTS (
    SELECT 1 FROM user_notification_preferences WHERE user_id = users.id
);

-- ============================================
-- 5. 触发器：新用户自动创建通知偏好
-- ============================================
CREATE OR REPLACE FUNCTION create_default_notification_prefs()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO user_notification_preferences (user_id, email_config)
    VALUES (
        NEW.id,
        jsonb_build_object(
            'enabled', TRUE,
            'email', NEW.email
        )
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_create_notification_prefs
AFTER INSERT ON users
FOR EACH ROW
EXECUTE FUNCTION create_default_notification_prefs();

-- ============================================
-- 6. 注释
-- ============================================
COMMENT ON TABLE user_notification_preferences IS '用户通知偏好配置，支持多渠道扩展';
COMMENT ON TABLE notification_history IS '通知发送历史记录';
COMMENT ON COLUMN user_notification_preferences.email_config IS '邮件通知配置 (JSONB)';
COMMENT ON COLUMN user_notification_preferences.webhook_config IS 'Webhook通知配置 (JSONB)';
COMMENT ON COLUMN user_notification_preferences.quiet_hours_start IS '安静时段开始时间';
COMMENT ON COLUMN user_notification_preferences.quiet_hours_end IS '安静时段结束时间';
