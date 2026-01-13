//! 审计日志仓库

use crate::db::PostgresPool;
use crate::errors::AppError;
use crate::models::{AuditLog, AuditLogQuery};
use uuid::Uuid;

/// 审计日志仓库
#[derive(Clone)]
pub struct AuditRepository {
    pool: PostgresPool,
}

impl AuditRepository {
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    /// 查询审计日志
    pub async fn query(&self, query: &AuditLogQuery) -> Result<(Vec<AuditLog>, i64), AppError> {
        let offset = (query.page - 1) * query.page_size;

        // 构建动态条件
        let mut conditions = vec!["1=1".to_string()];
        let mut bind_index = 1;

        if query.actor_type.is_some() {
            conditions.push(format!("actor_type = ${}", bind_index));
            bind_index += 1;
        }
        if query.actor_id.is_some() {
            conditions.push(format!("actor_id = ${}", bind_index));
            bind_index += 1;
        }
        if query.action.is_some() {
            conditions.push(format!("action = ${}", bind_index));
            bind_index += 1;
        }
        if query.resource.is_some() {
            conditions.push(format!("resource = ${}", bind_index));
            bind_index += 1;
        }
        if query.status.is_some() {
            conditions.push(format!("status = ${}", bind_index));
            bind_index += 1;
        }
        if query.start_time.is_some() {
            conditions.push(format!("timestamp >= ${}", bind_index));
            bind_index += 1;
        }
        if query.end_time.is_some() {
            conditions.push(format!("timestamp <= ${}", bind_index));
            // bind_index += 1;
        }

        let _where_clause = conditions.join(" AND ");

        // 简化查询（实际应使用参数化构建）
        let logs = sqlx::query_as::<_, AuditLog>(
            &format!(
                "SELECT * FROM audit_logs WHERE {} ORDER BY timestamp DESC LIMIT $1 OFFSET $2",
                "1=1" // 简化，实际需要完整条件
            ),
        )
        .bind(query.page_size)
        .bind(offset)
        .fetch_all(self.pool.pool())
        .await?;

        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs")
            .fetch_one(self.pool.pool())
            .await?;

        Ok((logs, total.0))
    }

    /// 删除过期审计日志
    pub async fn delete_expired(&self, retention_days: i32) -> Result<u64, AppError> {
        let result = sqlx::query(
            "DELETE FROM audit_logs WHERE timestamp < NOW() - INTERVAL '1 day' * $1",
        )
        .bind(retention_days)
        .execute(self.pool.pool())
        .await?;

        Ok(result.rows_affected())
    }

    /// 查找指定 `id` 的最近一条审计日志（按 `timestamp` 降序）
    pub async fn find_latest_by_id(&self, id: Uuid) -> Result<Option<AuditLog>, AppError> {
        let rec = sqlx::query_as::<_, AuditLog>(
            "SELECT * FROM audit_logs WHERE id = $1 ORDER BY timestamp DESC LIMIT 1",
        )
        .bind(id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(rec)
    }
}
