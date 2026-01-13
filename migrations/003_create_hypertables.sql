-- 003: 创建 TimescaleDB hypertable
-- 确保 TimescaleDB 扩展已启用
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- 将电量数据表转换为 hypertable
-- 按 recorded_at 字段自动分片，每 7 天一个分区
SELECT create_hypertable(
    'battery_data',
    'recorded_at',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

-- 启用压缩（保留 30 天后自动压缩）
ALTER TABLE battery_data SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'device_id'
);

-- 添加压缩策略：30 天后压缩
SELECT add_compression_policy('battery_data', INTERVAL '30 days', if_not_exists => TRUE);

-- 添加数据保留策略：保留 365 天
SELECT add_retention_policy('battery_data', INTERVAL '365 days', if_not_exists => TRUE);

-- 创建连续聚合视图（每小时统计）
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

-- 设置连续聚合刷新策略
-- 增加 start_offset 使刷新窗口大于 schedule_interval (要求: start_offset - end_offset > schedule_interval)
SELECT add_continuous_aggregate_policy('battery_hourly_stats',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- 创建每日统计视图
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

-- 增加 start_offset 以满足刷新窗口要求
SELECT add_continuous_aggregate_policy('battery_daily_stats',
    start_offset => INTERVAL '3 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);
