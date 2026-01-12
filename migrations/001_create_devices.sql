-- 001: 创建设备表
-- 设备状态枚举
CREATE TYPE device_status AS ENUM ('online', 'offline', 'maintenance', 'disabled');

-- 设备表
CREATE TABLE IF NOT EXISTS devices (
    id UUID PRIMARY KEY,
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

-- 索引
CREATE INDEX idx_devices_status ON devices(status);
CREATE INDEX idx_devices_device_type ON devices(device_type);
CREATE INDEX idx_devices_api_key_prefix ON devices(api_key_prefix);
CREATE INDEX idx_devices_created_at ON devices(created_at DESC);

-- 设备配置表
CREATE TABLE IF NOT EXISTS device_configs (
    device_id UUID PRIMARY KEY REFERENCES devices(id) ON DELETE CASCADE,
    low_battery_threshold INTEGER NOT NULL DEFAULT 20,
    critical_battery_threshold INTEGER NOT NULL DEFAULT 10,
    report_interval_seconds INTEGER NOT NULL DEFAULT 60,
    power_saving_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
