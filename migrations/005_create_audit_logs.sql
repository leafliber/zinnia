-- 005: 创建审计日志表
-- 操作者类型枚举
CREATE TYPE actor_type AS ENUM ('device', 'admin', 'system');

-- 审计状态枚举
CREATE TYPE audit_status AS ENUM ('success', 'failure');

-- 审计日志表
CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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

-- 索引
CREATE INDEX idx_audit_logs_timestamp ON audit_logs(timestamp DESC);
CREATE INDEX idx_audit_logs_actor ON audit_logs(actor_type, actor_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_resource ON audit_logs(resource);
CREATE INDEX idx_audit_logs_status ON audit_logs(status);

-- 将审计日志也转换为 hypertable（可选）
SELECT create_hypertable(
    'audit_logs',
    'timestamp',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

-- 添加数据保留策略：保留 90 天
SELECT add_retention_policy('audit_logs', INTERVAL '90 days', if_not_exists => TRUE);
