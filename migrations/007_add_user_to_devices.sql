-- 007: 给设备表添加用户关联

-- 添加设备所有者字段
ALTER TABLE devices ADD COLUMN owner_id UUID REFERENCES users(id) ON DELETE SET NULL;

-- 添加索引
CREATE INDEX idx_devices_owner_id ON devices(owner_id);

-- 设备共享表（多用户共享设备）
CREATE TABLE IF NOT EXISTS device_shares (
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission VARCHAR(20) NOT NULL DEFAULT 'read',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (device_id, user_id)
);

-- 共享权限检查约束
ALTER TABLE device_shares ADD CONSTRAINT chk_permission 
    CHECK (permission IN ('read', 'write', 'admin'));

-- 索引
CREATE INDEX idx_device_shares_user_id ON device_shares(user_id);
CREATE INDEX idx_device_shares_device_id ON device_shares(device_id);
