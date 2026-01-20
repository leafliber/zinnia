-- 003: 添加 Web Push 推送支持
-- 为 PWA 应用添加 Web Push 通知功能

-- ============================================
-- 1. Web Push 订阅表
-- ============================================
CREATE TABLE IF NOT EXISTS web_push_subscriptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- 订阅端点和密钥（来自浏览器 PushSubscription）
    endpoint TEXT NOT NULL,
    p256dh_key TEXT NOT NULL,  -- 公钥
    auth_secret TEXT NOT NULL,  -- 认证密钥
    
    -- 设备信息
    user_agent TEXT,
    device_name VARCHAR(100),
    
    -- 状态
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    
    -- 时间戳
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 索引
CREATE INDEX idx_web_push_subscriptions_user_id ON web_push_subscriptions(user_id);
CREATE INDEX idx_web_push_subscriptions_endpoint ON web_push_subscriptions(endpoint);
CREATE INDEX idx_web_push_subscriptions_active ON web_push_subscriptions(is_active) WHERE is_active = TRUE;
CREATE INDEX idx_web_push_subscriptions_expires ON web_push_subscriptions(expires_at) WHERE expires_at IS NOT NULL;

-- 确保同一用户同一端点只有一个订阅
CREATE UNIQUE INDEX idx_web_push_subscriptions_user_endpoint ON web_push_subscriptions(user_id, endpoint);

-- ============================================
-- 2. 更新通知偏好表，添加 Web Push 配置
-- ============================================
ALTER TABLE user_notification_preferences 
ADD COLUMN IF NOT EXISTS web_push_config JSONB;

-- ============================================
-- 3. Web Push 通知历史关联
-- ============================================
-- notification_history 表已支持 push 渠道，无需修改

-- ============================================
-- 4. 清理过期订阅的定时任务支持
-- ============================================
-- 创建函数：标记过期订阅为不活跃
CREATE OR REPLACE FUNCTION deactivate_expired_web_push_subscriptions()
RETURNS INTEGER AS $$
DECLARE
    affected_rows INTEGER;
BEGIN
    UPDATE web_push_subscriptions
    SET is_active = FALSE,
        updated_at = NOW()
    WHERE expires_at IS NOT NULL 
      AND expires_at < NOW()
      AND is_active = TRUE;
    
    GET DIAGNOSTICS affected_rows = ROW_COUNT;
    RETURN affected_rows;
END;
$$ LANGUAGE plpgsql;

-- ============================================
-- 5. 注释
-- ============================================
COMMENT ON TABLE web_push_subscriptions IS 'PWA Web Push 订阅信息';
COMMENT ON COLUMN web_push_subscriptions.endpoint IS '推送服务端点 URL';
COMMENT ON COLUMN web_push_subscriptions.p256dh_key IS 'P-256 ECDH 公钥 (Base64)';
COMMENT ON COLUMN web_push_subscriptions.auth_secret IS '认证密钥 (Base64)';
COMMENT ON COLUMN web_push_subscriptions.expires_at IS '订阅过期时间（可选）';
COMMENT ON COLUMN user_notification_preferences.web_push_config IS 'Web Push 通知配置 (JSONB)';

-- ============================================
-- 6. 为现有用户添加默认 Web Push 配置
-- ============================================
UPDATE user_notification_preferences
SET web_push_config = jsonb_build_object('enabled', FALSE)
WHERE web_push_config IS NULL;
