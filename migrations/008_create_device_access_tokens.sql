-- 008: 创建设备访问令牌表
-- 用于支持自定义有效期的设备访问令牌

-- 令牌权限枚举
CREATE TYPE token_permission AS ENUM ('read', 'write', 'all');

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
    expires_at TIMESTAMPTZ,  -- NULL 表示永不过期
    
    -- 状态跟踪
    last_used_at TIMESTAMPTZ,
    use_count INTEGER NOT NULL DEFAULT 0,
    is_revoked BOOLEAN NOT NULL DEFAULT FALSE,
    revoked_at TIMESTAMPTZ,
    
    -- 安全限制
    allowed_ips TEXT[],  -- 允许的 IP 白名单（NULL 表示不限制）
    rate_limit_per_minute INTEGER,  -- 每分钟请求限制（NULL 表示使用默认）
    
    -- 时间戳
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 索引
CREATE INDEX idx_device_access_tokens_device ON device_access_tokens(device_id);
CREATE INDEX idx_device_access_tokens_created_by ON device_access_tokens(created_by);
CREATE INDEX idx_device_access_tokens_prefix ON device_access_tokens(token_prefix);
CREATE INDEX idx_device_access_tokens_expires ON device_access_tokens(expires_at) 
    WHERE expires_at IS NOT NULL AND is_revoked = FALSE;

-- 注释
COMMENT ON TABLE device_access_tokens IS '设备访问令牌，支持自定义有效期和权限';
COMMENT ON COLUMN device_access_tokens.token_hash IS '令牌哈希值（Argon2）';
COMMENT ON COLUMN device_access_tokens.token_prefix IS '令牌前缀，用于显示和快速查找';
COMMENT ON COLUMN device_access_tokens.permission IS '权限级别：read=只读, write=只写, all=全部';
COMMENT ON COLUMN device_access_tokens.expires_at IS '过期时间，NULL表示永不过期';
COMMENT ON COLUMN device_access_tokens.allowed_ips IS 'IP白名单，NULL表示不限制';
