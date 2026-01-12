-- 004: 创建预警相关表
-- 预警级别枚举
CREATE TYPE alert_level AS ENUM ('info', 'warning', 'critical');

-- 预警状态枚举
CREATE TYPE alert_status AS ENUM ('active', 'acknowledged', 'resolved');

-- 预警类型枚举
CREATE TYPE alert_type AS ENUM ('low_battery', 'critical_battery', 'high_temperature', 'device_offline', 'rapid_drain');

-- 预警规则表
CREATE TABLE IF NOT EXISTS alert_rules (
    id UUID PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    alert_type alert_type NOT NULL,
    level alert_level NOT NULL,
    threshold_value DOUBLE PRECISION NOT NULL,
    cooldown_minutes INTEGER NOT NULL DEFAULT 30,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 唯一约束：每种类型只有一个规则
CREATE UNIQUE INDEX idx_alert_rules_type ON alert_rules(alert_type) WHERE enabled = TRUE;

-- 预警事件表
CREATE TABLE IF NOT EXISTS alert_events (
    id UUID PRIMARY KEY,
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    rule_id UUID NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,
    alert_type alert_type NOT NULL,
    level alert_level NOT NULL,
    status alert_status NOT NULL DEFAULT 'active',
    message TEXT NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    threshold DOUBLE PRECISION NOT NULL,
    triggered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acknowledged_at TIMESTAMPTZ,
    resolved_at TIMESTAMPTZ
);

-- 索引
CREATE INDEX idx_alert_events_device_id ON alert_events(device_id);
CREATE INDEX idx_alert_events_status ON alert_events(status);
CREATE INDEX idx_alert_events_level ON alert_events(level);
CREATE INDEX idx_alert_events_triggered_at ON alert_events(triggered_at DESC);
CREATE INDEX idx_alert_events_device_type ON alert_events(device_id, alert_type);

-- 插入默认预警规则
INSERT INTO alert_rules (id, name, alert_type, level, threshold_value, cooldown_minutes, enabled)
VALUES
    (gen_random_uuid(), '低电量预警', 'low_battery', 'warning', 20, 30, TRUE),
    (gen_random_uuid(), '临界电量预警', 'critical_battery', 'critical', 10, 15, TRUE),
    (gen_random_uuid(), '高温预警', 'high_temperature', 'warning', 45, 60, TRUE),
    (gen_random_uuid(), '设备离线预警', 'device_offline', 'info', 0, 120, TRUE)
ON CONFLICT DO NOTHING;
