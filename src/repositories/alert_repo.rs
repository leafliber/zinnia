//! 预警数据仓库

use crate::db::PostgresPool;
use crate::errors::AppError;
use crate::models::{
    AlertEvent, AlertListQuery, AlertRule, AlertStatus, AlertType,
    CreateAlertRuleRequest, UpdateAlertRuleRequest, UpdateAlertStatusRequest,
};
use chrono::Utc;
use uuid::Uuid;

/// 预警数据仓库
#[derive(Clone)]
pub struct AlertRepository {
    pool: PostgresPool,
}

impl AlertRepository {
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    // ========== 预警规则 ==========

    /// 创建预警规则（用户独立）
    pub async fn create_rule(&self, user_id: Uuid, request: &CreateAlertRuleRequest) -> Result<AlertRule, AppError> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let rule = sqlx::query_as::<_, AlertRule>(
            r#"
            INSERT INTO alert_rules (id, user_id, name, alert_type, level, cooldown_minutes, enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(user_id)
        .bind(&request.name)
        .bind(&request.alert_type)
        .bind(&request.level)
        .bind(request.cooldown_minutes)
        .bind(request.enabled)
        .bind(now)
        .bind(now)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(rule)
    }

    /// 获取用户的所有启用规则
    pub async fn get_enabled_rules(&self, user_id: Uuid) -> Result<Vec<AlertRule>, AppError> {
        let rules = sqlx::query_as::<_, AlertRule>(
            "SELECT * FROM alert_rules WHERE user_id = $1 AND enabled = true ORDER BY created_at",
        )
        .bind(user_id)
        .fetch_all(self.pool.pool())
        .await?;

        Ok(rules)
    }

    /// 根据类型获取用户的规则
    pub async fn get_rule_by_type(&self, user_id: Uuid, alert_type: &AlertType) -> Result<Option<AlertRule>, AppError> {
        let rule = sqlx::query_as::<_, AlertRule>(
            "SELECT * FROM alert_rules WHERE user_id = $1 AND alert_type = $2 AND enabled = true",
        )
        .bind(user_id)
        .bind(alert_type)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(rule)
    }

    /// 根据 ID 获取规则（仅限用户自己的规则）
    pub async fn get_rule_by_id(&self, rule_id: Uuid, user_id: Uuid) -> Result<Option<AlertRule>, AppError> {
        let rule = sqlx::query_as::<_, AlertRule>(
            "SELECT * FROM alert_rules WHERE id = $1 AND user_id = $2",
        )
        .bind(rule_id)
        .bind(user_id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(rule)
    }

    /// 更新预警规则（仅限用户自己的规则）
    pub async fn update_rule(&self, rule_id: Uuid, user_id: Uuid, request: &UpdateAlertRuleRequest) -> Result<AlertRule, AppError> {
        let now = Utc::now();
        
        // 使用 COALESCE 实现部分更新
        let rule = sqlx::query_as::<_, AlertRule>(
            r#"
            UPDATE alert_rules SET
                name = COALESCE($3, name),
                alert_type = COALESCE($4, alert_type),
                level = COALESCE($5, level),
                cooldown_minutes = COALESCE($6, cooldown_minutes),
                enabled = COALESCE($7, enabled),
                updated_at = $8
            WHERE id = $1 AND user_id = $2
            RETURNING *
            "#,
        )
        .bind(rule_id)
        .bind(user_id)
        .bind(&request.name)
        .bind(&request.alert_type)
        .bind(&request.level)
        .bind(request.cooldown_minutes)
        .bind(request.enabled)
        .bind(now)
        .fetch_one(self.pool.pool())
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => AppError::NotFound(format!("预警规则不存在或无权访问: {}", rule_id)),
            _ => e.into(),
        })?;

        Ok(rule)
    }

    /// 删除预警规则（仅限用户自己的规则）
    pub async fn delete_rule(&self, rule_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM alert_rules WHERE id = $1 AND user_id = $2")
            .bind(rule_id)
            .bind(user_id)
            .execute(self.pool.pool())
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("预警规则不存在或无权访问: {}", rule_id)));
        }

        Ok(())
    }

    // ========== 预警事件 ==========

    /// 创建预警事件（使用设备配置的阈值）
    pub async fn create_event(
        &self,
        device_id: Uuid,
        rule: &AlertRule,
        value: f64,
        threshold: f64,
        message: &str,
    ) -> Result<AlertEvent, AppError> {
        let id = Uuid::new_v4();

        let event = sqlx::query_as::<_, AlertEvent>(
            r#"
            INSERT INTO alert_events (id, device_id, rule_id, alert_type, level, status, message, value, threshold, triggered_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(device_id)
        .bind(rule.id)
        .bind(&rule.alert_type)
        .bind(&rule.level)
        .bind(AlertStatus::Active)
        .bind(message)
        .bind(value)
        .bind(threshold)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(event)
    }

    /// 检查是否在冷却期内
    pub async fn is_in_cooldown(
        &self,
        device_id: Uuid,
        alert_type: &AlertType,
        cooldown_minutes: i32,
    ) -> Result<bool, AppError> {
        let result: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM alert_events
            WHERE device_id = $1 
              AND alert_type = $2 
              AND triggered_at > NOW() - INTERVAL '1 minute' * $3
            "#,
        )
        .bind(device_id)
        .bind(alert_type)
        .bind(cooldown_minutes)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(result.map(|r| r.0 > 0).unwrap_or(false))
    }

    /// 更新预警状态（限制用户只能操作自己设备的预警）
    pub async fn update_event_status(
        &self,
        event_id: Uuid,
        user_id: Uuid,
        request: &UpdateAlertStatusRequest,
    ) -> Result<AlertEvent, AppError> {
        let event = match request.status {
            AlertStatus::Acknowledged => {
                sqlx::query_as::<_, AlertEvent>(
                    r#"
                    UPDATE alert_events SET status = $2, acknowledged_at = NOW() 
                    WHERE id = $1 AND device_id IN (
                        SELECT id FROM devices WHERE owner_id = $3
                        UNION
                        SELECT device_id FROM device_shares WHERE user_id = $3
                    )
                    RETURNING *
                    "#,
                )
                .bind(event_id)
                .bind(&request.status)
                .bind(user_id)
                .fetch_one(self.pool.pool())
                .await
                .map_err(|e| match e {
                    sqlx::Error::RowNotFound => AppError::NotFound("预警不存在或无权访问".to_string()),
                    _ => e.into(),
                })?
            }
            AlertStatus::Resolved => {
                sqlx::query_as::<_, AlertEvent>(
                    r#"
                    UPDATE alert_events SET status = $2, resolved_at = NOW() 
                    WHERE id = $1 AND device_id IN (
                        SELECT id FROM devices WHERE owner_id = $3
                        UNION
                        SELECT device_id FROM device_shares WHERE user_id = $3
                    )
                    RETURNING *
                    "#,
                )
                .bind(event_id)
                .bind(&request.status)
                .bind(user_id)
                .fetch_one(self.pool.pool())
                .await
                .map_err(|e| match e {
                    sqlx::Error::RowNotFound => AppError::NotFound("预警不存在或无权访问".to_string()),
                    _ => e.into(),
                })?
            }
            _ => {
                sqlx::query_as::<_, AlertEvent>(
                    r#"
                    UPDATE alert_events SET status = $2 
                    WHERE id = $1 AND device_id IN (
                        SELECT id FROM devices WHERE owner_id = $3
                        UNION
                        SELECT device_id FROM device_shares WHERE user_id = $3
                    )
                    RETURNING *
                    "#,
                )
                .bind(event_id)
                .bind(&request.status)
                .bind(user_id)
                .fetch_one(self.pool.pool())
                .await
                .map_err(|e| match e {
                    sqlx::Error::RowNotFound => AppError::NotFound("预警不存在或无权访问".to_string()),
                    _ => e.into(),
                })?
            }
        };

        Ok(event)
    }

    /// 查询预警事件列表（限制用户只能查询自己设备的预警）
    pub async fn list_events(&self, user_id: Uuid, query: &AlertListQuery) -> Result<(Vec<AlertEvent>, i64), AppError> {
        let offset = (query.page - 1) * query.page_size;

        // 构建基础条件：只查询用户设备的预警
        let mut conditions = vec![
            r#"device_id IN (
                SELECT id FROM devices WHERE owner_id = $1
                UNION
                SELECT device_id FROM device_shares WHERE user_id = $1
            )"#.to_string(),
        ];
        let mut param_index = 2;

        // 添加可选过滤条件
        if query.device_id.is_some() {
            conditions.push(format!("device_id = ${}", param_index));
            param_index += 1;
        }
        if query.level.is_some() {
            conditions.push(format!("level = ${}", param_index));
            param_index += 1;
        }
        if query.status.is_some() {
            conditions.push(format!("status = ${}", param_index));
            param_index += 1;
        }
        if query.alert_type.is_some() {
            conditions.push(format!("alert_type = ${}", param_index));
        }

        let where_clause = conditions.join(" AND ");

        // 查询总数
        let count_sql = format!("SELECT COUNT(*) FROM alert_events WHERE {}", where_clause);
        let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql).bind(user_id);
        if let Some(device_id) = query.device_id {
            count_query = count_query.bind(device_id);
        }
        if let Some(ref level) = query.level {
            count_query = count_query.bind(level);
        }
        if let Some(ref status) = query.status {
            count_query = count_query.bind(status);
        }
        if let Some(ref alert_type) = query.alert_type {
            count_query = count_query.bind(alert_type);
        }
        let total = count_query.fetch_one(self.pool.pool()).await?.0;

        // 查询数据
        let list_sql = format!(
            "SELECT * FROM alert_events WHERE {} ORDER BY triggered_at DESC LIMIT ${} OFFSET ${}",
            where_clause,
            param_index,
            param_index + 1
        );
        let mut list_query = sqlx::query_as::<_, AlertEvent>(&list_sql).bind(user_id);
        if let Some(device_id) = query.device_id {
            list_query = list_query.bind(device_id);
        }
        if let Some(ref level) = query.level {
            list_query = list_query.bind(level);
        }
        if let Some(ref status) = query.status {
            list_query = list_query.bind(status);
        }
        if let Some(ref alert_type) = query.alert_type {
            list_query = list_query.bind(alert_type);
        }
        let events = list_query
            .bind(query.page_size)
            .bind(offset)
            .fetch_all(self.pool.pool())
            .await?;

        Ok((events, total))
    }

    /// 获取设备的活跃预警数
    pub async fn count_active_alerts(&self, device_id: Uuid) -> Result<i64, AppError> {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM alert_events WHERE device_id = $1 AND status = 'active'",
        )
        .bind(device_id)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(result.0)
    }
}
