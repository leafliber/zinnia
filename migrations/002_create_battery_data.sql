-- 002: 创建电量数据表
-- 省电模式枚举
CREATE TYPE power_saving_mode AS ENUM ('off', 'low', 'medium', 'high', 'extreme');

-- 电量数据表
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

-- 普通索引（在转换为 hypertable 之前创建）
CREATE INDEX idx_battery_data_device_id ON battery_data(device_id);
CREATE INDEX idx_battery_data_device_recorded ON battery_data(device_id, recorded_at DESC);
