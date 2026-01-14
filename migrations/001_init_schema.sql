-- 001: 初始化数据库架构
-- 整合所有表和类型定义，确保依赖顺序正确

-- ============================================
-- 1. TimescaleDB 扩展
-- ============================================
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- ============================================
-- 2. 枚举类型定义
-- ============================================

-- 设备相关
CREATE TYPE device_status AS ENUM ('online', 'offline', 'maintenance', 'disabled');
CREATE TYPE power_saving_mode AS ENUM ('off', 'low', 'medium', 'high', 'extreme');

-- 用户相关
CREATE TYPE user_role AS ENUM ('admin', 'user', 'readonly');

-- 预警相关
CREATE TYPE alert_level AS ENUM ('info', 'warning', 'critical');
CREATE TYPE alert_status AS ENUM ('active', 'acknowledged', 'resolved');
CREATE TYPE alert_type AS ENUM ('low_battery', 'critical_battery', 'high_temperature', 'device_offline', 'rapid_drain');

-- 审计相关
CREATE TYPE actor_type AS ENUM ('device', 'admin', 'system');
CREATE TYPE audit_status AS ENUM ('success', 'failure');

-- 令牌相关
CREATE TYPE token_permission AS ENUM ('read', 'write', 'all');

-- ============================================
-- 3. 用户表（基础依赖）
-- ============================================

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    username VARCHAR(50) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    role user_role NOT NULL DEFAULT 'user',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    
    -- 安全字段
    failed_login_attempts INTEGER NOT NULL DEFAULT 0,
    locked_until TIMESTAMPTZ,
    last_login_at TIMESTAMPTZ,
    
    -- 时间戳
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- 元数据
    metadata JSONB
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_role ON users(role);
CREATE INDEX idx_users_is_active ON users(is_active);
CREATE INDEX idx_users_created_at ON users(created_at DESC);

-- 用户刷新令牌表
CREATE TABLE IF NOT EXISTS user_refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) NOT NULL,
    device_info VARCHAR(255),
    ip_address VARCHAR(45),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_user_refresh_tokens_user_id ON user_refresh_tokens(user_id);
CREATE INDEX idx_user_refresh_tokens_expires_at ON user_refresh_tokens(expires_at);

-- ============================================
-- 4. 设备表
-- ============================================

CREATE TABLE IF NOT EXISTS devices (
    id UUID PRIMARY KEY,
    owner_id UUID REFERENCES users(id) ON DELETE SET NULL,
    name VARCHAR(100) NOT NULL,
    device_type VARCHAR(50) NOT NULL,
    status device_status NOT NULL DEFAULT 'offline',
    api_key_hash VARCHAR(255) NOT NULL,
    api_key_prefix VARCHAR(20) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ,
    metadata JSONB
);

CREATE INDEX idx_devices_status ON devices(status);
CREATE INDEX idx_devices_device_type ON devices(device_type);
CREATE INDEX idx_devices_api_key_prefix ON devices(api_key_prefix);
CREATE INDEX idx_devices_created_at ON devices(created_at DESC);
CREATE INDEX idx_devices_owner_id ON devices(owner_id);

-- 设备配置表
CREATE TABLE IF NOT EXISTS device_configs (
    device_id UUID PRIMARY KEY REFERENCES devices(id) ON DELETE CASCADE,
    low_battery_threshold INTEGER NOT NULL DEFAULT 20,
    critical_battery_threshold INTEGER NOT NULL DEFAULT 10,
    report_interval_seconds INTEGER NOT NULL DEFAULT 60,
    high_temperature_threshold DOUBLE PRECISION NOT NULL DEFAULT 45.0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 设备共享表
CREATE TABLE IF NOT EXISTS device_shares (
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission VARCHAR(20) NOT NULL DEFAULT 'read',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (device_id, user_id)
);

ALTER TABLE device_shares ADD CONSTRAINT chk_permission 
    CHECK (permission IN ('read', 'write', 'admin'));

CREATE INDEX idx_device_shares_user_id ON device_shares(user_id);
CREATE INDEX idx_device_shares_device_id ON device_shares(device_id);

-- 设备访问令牌表
CREATE TABLE IF NOT EXISTS device_access_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- 令牌信息
    token_hash VARCHAR(255) NOT NULL,
    token_prefix VARCHAR(30) NOT NULL,
    name VARCHAR(100) NOT NULL,
    
    -- 权限和有效期
    permission token_permission NOT NULL DEFAULT 'write',
    expires_at TIMESTAMPTZ,
    
    -- 状态跟踪
    last_used_at TIMESTAMPTZ,
    use_count INTEGER NOT NULL DEFAULT 0,
    is_revoked BOOLEAN NOT NULL DEFAULT FALSE,
    revoked_at TIMESTAMPTZ,
    
    -- 安全限制
    allowed_ips TEXT[],
    rate_limit_per_minute INTEGER,
    
    -- 时间戳
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_device_access_tokens_device ON device_access_tokens(device_id);
CREATE INDEX idx_device_access_tokens_created_by ON device_access_tokens(created_by);
CREATE INDEX idx_device_access_tokens_prefix ON device_access_tokens(token_prefix);
CREATE INDEX idx_device_access_tokens_expires ON device_access_tokens(expires_at) 
    WHERE expires_at IS NOT NULL AND is_revoked = FALSE;

COMMENT ON TABLE device_access_tokens IS '设备访问令牌，支持自定义有效期和权限';
COMMENT ON COLUMN device_access_tokens.token_hash IS '令牌哈希值（Argon2）';
COMMENT ON COLUMN device_access_tokens.token_prefix IS '令牌前缀，用于显示和快速查找';
COMMENT ON COLUMN device_access_tokens.permission IS '权限级别：read=只读, write=只写, all=全部';
COMMENT ON COLUMN device_access_tokens.expires_at IS '过期时间，NULL表示永不过期';
COMMENT ON COLUMN device_access_tokens.allowed_ips IS 'IP白名单，NULL表示不限制';

-- ============================================
-- 5. 电量数据表（TimescaleDB Hypertable）
-- ============================================

CREATE TABLE IF NOT EXISTS battery_data (
    id UUID NOT NULL,
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    battery_level INTEGER NOT NULL CHECK (battery_level >= 0 AND battery_level <= 100),
    is_charging BOOLEAN NOT NULL DEFAULT FALSE,
    power_saving_mode power_saving_mode NOT NULL DEFAULT 'off',
    temperature DOUBLE PRECISION,
    voltage DOUBLE PRECISION,
    recorded_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id, recorded_at)
);

CREATE INDEX idx_battery_data_device_id ON battery_data(device_id);
CREATE INDEX idx_battery_data_device_recorded ON battery_data(device_id, recorded_at DESC);

-- 转换为 hypertable
SELECT create_hypertable(
    'battery_data',
    'recorded_at',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

-- 启用压缩
ALTER TABLE battery_data SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'device_id'
);

-- 压缩策略：30 天后压缩
SELECT add_compression_policy('battery_data', INTERVAL '30 days', if_not_exists => TRUE);

-- 数据保留策略：保留 365 天
SELECT add_retention_policy('battery_data', INTERVAL '365 days', if_not_exists => TRUE);

-- 连续聚合视图：每小时统计
CREATE MATERIALIZED VIEW battery_hourly_stats
WITH (timescaledb.continuous) AS
SELECT
    device_id,
    time_bucket('1 hour', recorded_at) AS bucket,
    AVG(battery_level) AS avg_level,
    MIN(battery_level) AS min_level,
    MAX(battery_level) AS max_level,
    COUNT(*) AS sample_count,
    SUM(CASE WHEN is_charging THEN 1 ELSE 0 END) AS charging_samples
FROM battery_data
GROUP BY device_id, bucket
WITH NO DATA;

SELECT add_continuous_aggregate_policy('battery_hourly_stats',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- 连续聚合视图：每日统计
CREATE MATERIALIZED VIEW battery_daily_stats
WITH (timescaledb.continuous) AS
SELECT
    device_id,
    time_bucket('1 day', recorded_at) AS bucket,
    AVG(battery_level) AS avg_level,
    MIN(battery_level) AS min_level,
    MAX(battery_level) AS max_level,
    COUNT(*) AS sample_count
FROM battery_data
GROUP BY device_id, bucket
WITH NO DATA;

SELECT add_continuous_aggregate_policy('battery_daily_stats',
    start_offset => INTERVAL '3 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- ============================================
-- 6. 预警表
-- ============================================

-- 预警规则表（用户独立）
CREATE TABLE IF NOT EXISTS alert_rules (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    alert_type alert_type NOT NULL,
    level alert_level NOT NULL,
    cooldown_minutes INTEGER NOT NULL DEFAULT 30,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_alert_rules_user_id ON alert_rules(user_id);
CREATE INDEX idx_alert_rules_user_type ON alert_rules(user_id, alert_type);
CREATE UNIQUE INDEX idx_alert_rules_user_type_enabled ON alert_rules(user_id, alert_type) WHERE enabled = TRUE;

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

CREATE INDEX idx_alert_events_device_id ON alert_events(device_id);
CREATE INDEX idx_alert_events_status ON alert_events(status);
CREATE INDEX idx_alert_events_level ON alert_events(level);
CREATE INDEX idx_alert_events_triggered_at ON alert_events(triggered_at DESC);
CREATE INDEX idx_alert_events_device_type ON alert_events(device_id, alert_type);

-- ============================================
-- 7. 审计日志表（TimescaleDB Hypertable）
-- ============================================

CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_type actor_type NOT NULL,
    actor_id VARCHAR(100) NOT NULL,
    action VARCHAR(50) NOT NULL,
    resource VARCHAR(50) NOT NULL,
    resource_id VARCHAR(100),
    ip_address VARCHAR(45) NOT NULL,
    user_agent TEXT,
    status audit_status NOT NULL,
    details JSONB,
    request_id VARCHAR(100)
);

ALTER TABLE audit_logs
    ADD CONSTRAINT audit_logs_pkey PRIMARY KEY (id, timestamp);

CREATE INDEX idx_audit_logs_timestamp ON audit_logs(timestamp DESC);
CREATE INDEX idx_audit_logs_actor ON audit_logs(actor_type, actor_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_resource ON audit_logs(resource);
CREATE INDEX idx_audit_logs_status ON audit_logs(status);

-- 转换为 hypertable
SELECT create_hypertable(
    'audit_logs',
    'timestamp',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

-- 数据保留策略：保留 90 天
SELECT add_retention_policy('audit_logs', INTERVAL '90 days', if_not_exists => TRUE);
