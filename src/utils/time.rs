//! 时间处理工具

use chrono::{DateTime, Duration, Utc};

/// 获取 N 天前的时间
pub fn days_ago(days: i64) -> DateTime<Utc> {
    Utc::now() - Duration::days(days)
}

/// 获取 N 小时前的时间
pub fn hours_ago(hours: i64) -> DateTime<Utc> {
    Utc::now() - Duration::hours(hours)
}

/// 获取 N 分钟前的时间
pub fn minutes_ago(minutes: i64) -> DateTime<Utc> {
    Utc::now() - Duration::minutes(minutes)
}

/// 获取今天开始时间（UTC 0点）
pub fn today_start() -> DateTime<Utc> {
    Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc()
}

/// 获取今天结束时间
pub fn today_end() -> DateTime<Utc> {
    Utc::now().date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc()
}

/// 格式化为 ISO 8601
pub fn format_iso8601(dt: &DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// 解析 ISO 8601 时间字符串
pub fn parse_iso8601(s: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
    DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_days_ago() {
        let week_ago = days_ago(7);
        let now = Utc::now();
        assert!(week_ago < now);
        // 由于两次 Utc::now() 调用之间有微小时间差，允许 6-7 天的范围
        let diff_days = (now - week_ago).num_days();
        assert!((6..=7).contains(&diff_days), "Expected 6-7 days, got {}", diff_days);
    }

    #[test]
    fn test_format_parse_iso8601() {
        let now = Utc::now();
        let formatted = format_iso8601(&now);
        let parsed = parse_iso8601(&formatted).unwrap();
        
        // 由于毫秒截断，允许 1 秒误差
        assert!((now - parsed).num_milliseconds().abs() < 1000);
    }
}
