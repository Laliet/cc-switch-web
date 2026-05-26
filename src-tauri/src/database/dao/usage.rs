use super::super::{lock_conn, Database};
use crate::error::AppError;
use chrono::{Duration, Local, TimeZone};
use rusqlite::{params, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderHealthRecord {
    pub provider_id: String,
    pub app_type: String,
    pub is_healthy: bool,
    pub consecutive_failures: i64,
    pub last_success_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyRequestLogRecord {
    pub request_id: String,
    pub provider_id: String,
    pub app_type: String,
    pub model: String,
    pub request_model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub input_cost_usd: String,
    pub output_cost_usd: String,
    pub cache_read_cost_usd: String,
    pub cache_creation_cost_usd: String,
    pub total_cost_usd: String,
    pub latency_ms: i64,
    pub first_token_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub status_code: i64,
    pub error_message: Option<String>,
    pub session_id: Option<String>,
    pub provider_type: Option<String>,
    pub is_streaming: bool,
    pub cost_multiplier: String,
    pub created_at: i64,
    pub data_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyRequestUsageUpdate {
    pub request_id: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub input_cost_usd: String,
    pub output_cost_usd: String,
    pub cache_read_cost_usd: String,
    pub cache_creation_cost_usd: String,
    pub total_cost_usd: String,
    pub first_token_ms: Option<i64>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageDailyRollupRecord {
    pub date: String,
    pub app_type: String,
    pub provider_id: String,
    pub model: String,
    pub request_count: i64,
    pub success_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub total_cost_usd: String,
    pub avg_latency_ms: i64,
}

fn local_midnight_cutoff_millis(
    now: chrono::DateTime<Local>,
    retain_days: i64,
) -> Result<i64, AppError> {
    let target_day = now
        .checked_sub_signed(Duration::days(retain_days))
        .ok_or_else(|| AppError::Database("rollup cutoff overflow".to_string()))?
        .date_naive();
    let next_day = target_day
        .succ_opt()
        .ok_or_else(|| AppError::Database("rollup cutoff next-day overflow".to_string()))?;
    let naive_midnight = next_day
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::Database("rollup cutoff midnight overflow".to_string()))?;
    let local_dt = match Local.from_local_datetime(&naive_midnight) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(earliest, _) => earliest,
        chrono::LocalResult::None => {
            let bumped = naive_midnight + Duration::hours(1);
            match Local.from_local_datetime(&bumped) {
                chrono::LocalResult::Single(dt) => dt,
                chrono::LocalResult::Ambiguous(earliest, _) => earliest,
                chrono::LocalResult::None => {
                    return Err(AppError::Database(
                        "rollup cutoff fell into DST gap".to_string(),
                    ));
                }
            }
        }
    };
    Ok(local_dt.timestamp_millis())
}

impl Database {
    pub fn record_provider_success(
        &self,
        app_type: &str,
        provider_id: &str,
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "INSERT INTO provider_health (
                provider_id, app_type, is_healthy, consecutive_failures,
                last_success_at, last_error, updated_at
            ) VALUES (?1, ?2, 1, 0, datetime('now'), NULL, datetime('now'))
            ON CONFLICT(provider_id, app_type) DO UPDATE SET
                is_healthy = 1,
                consecutive_failures = 0,
                last_success_at = datetime('now'),
                last_error = NULL,
                updated_at = datetime('now')",
            params![provider_id, app_type],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn record_provider_failure(
        &self,
        app_type: &str,
        provider_id: &str,
        last_error: Option<&str>,
        unhealthy: bool,
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "INSERT INTO provider_health (
                provider_id, app_type, is_healthy, consecutive_failures,
                last_failure_at, last_error, updated_at
            ) VALUES (?1, ?2, ?3, 1, datetime('now'), ?4, datetime('now'))
            ON CONFLICT(provider_id, app_type) DO UPDATE SET
                is_healthy = ?3,
                consecutive_failures = consecutive_failures + 1,
                last_failure_at = datetime('now'),
                last_error = ?4,
                updated_at = datetime('now')",
            params![provider_id, app_type, i64::from(!unhealthy), last_error],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_provider_health(
        &self,
        app_type: &str,
        provider_id: &str,
    ) -> Result<Option<ProviderHealthRecord>, AppError> {
        let conn = lock_conn!(self.conn);
        conn.query_row(
            "SELECT provider_id, app_type, is_healthy, consecutive_failures,
                    last_success_at, last_failure_at, last_error, updated_at
             FROM provider_health
             WHERE app_type = ?1 AND provider_id = ?2",
            params![app_type, provider_id],
            |row| {
                Ok(ProviderHealthRecord {
                    provider_id: row.get(0)?,
                    app_type: row.get(1)?,
                    is_healthy: row.get::<_, i64>(2)? != 0,
                    consecutive_failures: row.get(3)?,
                    last_success_at: row.get(4)?,
                    last_failure_at: row.get(5)?,
                    last_error: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(|e| AppError::Database(e.to_string()))
    }

    pub fn list_provider_health(
        &self,
        app_type: &str,
    ) -> Result<Vec<ProviderHealthRecord>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn
            .prepare(
                "SELECT provider_id, app_type, is_healthy, consecutive_failures,
                        last_success_at, last_failure_at, last_error, updated_at
                 FROM provider_health
                 WHERE app_type = ?1
                 ORDER BY provider_id ASC",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(params![app_type], |row| {
                Ok(ProviderHealthRecord {
                    provider_id: row.get(0)?,
                    app_type: row.get(1)?,
                    is_healthy: row.get::<_, i64>(2)? != 0,
                    consecutive_failures: row.get(3)?,
                    last_success_at: row.get(4)?,
                    last_failure_at: row.get(5)?,
                    last_error: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(records)
    }

    pub fn insert_proxy_request_log(&self, record: &ProxyRequestLogRecord) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "INSERT OR REPLACE INTO proxy_request_logs (
                request_id, provider_id, app_type, model, request_model,
                input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
                input_cost_usd, output_cost_usd, cache_read_cost_usd, cache_creation_cost_usd,
                total_cost_usd, latency_ms, first_token_ms, duration_ms, status_code,
                error_message, session_id, provider_type, is_streaming, cost_multiplier,
                created_at, data_source
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                ?21, ?22, ?23, ?24, ?25
            )",
            params![
                record.request_id,
                record.provider_id,
                record.app_type,
                record.model,
                record.request_model,
                record.input_tokens,
                record.output_tokens,
                record.cache_read_tokens,
                record.cache_creation_tokens,
                record.input_cost_usd,
                record.output_cost_usd,
                record.cache_read_cost_usd,
                record.cache_creation_cost_usd,
                record.total_cost_usd,
                record.latency_ms,
                record.first_token_ms,
                record.duration_ms,
                record.status_code,
                record.error_message,
                record.session_id,
                record.provider_type,
                i64::from(record.is_streaming),
                record.cost_multiplier,
                record.created_at,
                record.data_source,
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn update_proxy_request_log_usage(
        &self,
        update: &ProxyRequestUsageUpdate,
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "UPDATE proxy_request_logs SET
                model = ?2,
                input_tokens = ?3,
                output_tokens = ?4,
                cache_read_tokens = ?5,
                cache_creation_tokens = ?6,
                input_cost_usd = ?7,
                output_cost_usd = ?8,
                cache_read_cost_usd = ?9,
                cache_creation_cost_usd = ?10,
                total_cost_usd = ?11,
                first_token_ms = COALESCE(first_token_ms, ?12),
                duration_ms = ?13
             WHERE request_id = ?1",
            params![
                update.request_id,
                update.model,
                update.input_tokens,
                update.output_tokens,
                update.cache_read_tokens,
                update.cache_creation_tokens,
                update.input_cost_usd,
                update.output_cost_usd,
                update.cache_read_cost_usd,
                update.cache_creation_cost_usd,
                update.total_cost_usd,
                update.first_token_ms,
                update.duration_ms,
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn recent_proxy_request_logs(
        &self,
        limit: usize,
    ) -> Result<Vec<ProxyRequestLogRecord>, AppError> {
        let conn = lock_conn!(self.conn);
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut stmt = conn
            .prepare(
                "SELECT request_id, provider_id, app_type, model, request_model,
                        input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
                        input_cost_usd, output_cost_usd, cache_read_cost_usd, cache_creation_cost_usd,
                        total_cost_usd, latency_ms, first_token_ms, duration_ms, status_code,
                        error_message, session_id, provider_type, is_streaming, cost_multiplier,
                        created_at, data_source
                 FROM proxy_request_logs
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(ProxyRequestLogRecord {
                    request_id: row.get(0)?,
                    provider_id: row.get(1)?,
                    app_type: row.get(2)?,
                    model: row.get(3)?,
                    request_model: row.get(4)?,
                    input_tokens: row.get(5)?,
                    output_tokens: row.get(6)?,
                    cache_read_tokens: row.get(7)?,
                    cache_creation_tokens: row.get(8)?,
                    input_cost_usd: row.get(9)?,
                    output_cost_usd: row.get(10)?,
                    cache_read_cost_usd: row.get(11)?,
                    cache_creation_cost_usd: row.get(12)?,
                    total_cost_usd: row.get(13)?,
                    latency_ms: row.get(14)?,
                    first_token_ms: row.get(15)?,
                    duration_ms: row.get(16)?,
                    status_code: row.get(17)?,
                    error_message: row.get(18)?,
                    session_id: row.get(19)?,
                    provider_type: row.get(20)?,
                    is_streaming: row.get::<_, i64>(21)? != 0,
                    cost_multiplier: row.get(22)?,
                    created_at: row.get(23)?,
                    data_source: row.get(24)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(records)
    }

    pub fn rollup_and_prune_proxy_request_logs(&self, retain_days: i64) -> Result<u64, AppError> {
        let cutoff = local_midnight_cutoff_millis(Local::now(), retain_days)?;
        let conn = lock_conn!(self.conn);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM proxy_request_logs WHERE created_at < ?1",
                params![cutoff],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        if count == 0 {
            return Ok(0);
        }

        conn.execute("SAVEPOINT rollup_prune", [])
            .map_err(|e| AppError::Database(e.to_string()))?;
        let result = (|| {
            conn.execute(
                "INSERT OR REPLACE INTO usage_daily_rollups (
                    date, app_type, provider_id, model, request_count, success_count,
                    input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
                    total_cost_usd, avg_latency_ms
                )
                SELECT
                    d, a, p, m,
                    COALESCE(old.request_count, 0) + new_req,
                    COALESCE(old.success_count, 0) + new_succ,
                    COALESCE(old.input_tokens, 0) + new_in,
                    COALESCE(old.output_tokens, 0) + new_out,
                    COALESCE(old.cache_read_tokens, 0) + new_cr,
                    COALESCE(old.cache_creation_tokens, 0) + new_cc,
                    CAST(COALESCE(CAST(old.total_cost_usd AS REAL), 0) + new_cost AS TEXT),
                    CASE WHEN COALESCE(old.request_count, 0) + new_req > 0
                        THEN (COALESCE(old.avg_latency_ms, 0) * COALESCE(old.request_count, 0)
                              + new_lat * new_req)
                             / (COALESCE(old.request_count, 0) + new_req)
                        ELSE 0 END
                FROM (
                    SELECT
                        date(created_at / 1000, 'unixepoch', 'localtime') as d,
                        app_type as a,
                        provider_id as p,
                        model as m,
                        COUNT(*) as new_req,
                        SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END) as new_succ,
                        COALESCE(SUM(input_tokens), 0) as new_in,
                        COALESCE(SUM(output_tokens), 0) as new_out,
                        COALESCE(SUM(cache_read_tokens), 0) as new_cr,
                        COALESCE(SUM(cache_creation_tokens), 0) as new_cc,
                        COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0) as new_cost,
                        COALESCE(AVG(latency_ms), 0) as new_lat
                    FROM proxy_request_logs
                    WHERE created_at < ?1
                    GROUP BY d, a, p, m
                ) agg
                LEFT JOIN usage_daily_rollups old
                    ON old.date = agg.d
                    AND old.app_type = agg.a
                    AND old.provider_id = agg.p
                    AND old.model = agg.m",
                params![cutoff],
            )
            .map_err(|e| AppError::Database(format!("Rollup aggregation failed: {e}")))?;

            let deleted = conn
                .execute(
                    "DELETE FROM proxy_request_logs WHERE created_at < ?1",
                    params![cutoff],
                )
                .map_err(|e| AppError::Database(format!("Pruning old logs failed: {e}")))?;
            Ok::<u64, AppError>(deleted as u64)
        })();

        match result {
            Ok(deleted) => {
                conn.execute("RELEASE rollup_prune", [])
                    .map_err(|e| AppError::Database(e.to_string()))?;
                Ok(deleted)
            }
            Err(err) => {
                let _ = conn.execute("ROLLBACK TO rollup_prune", []);
                let _ = conn.execute("RELEASE rollup_prune", []);
                Err(err)
            }
        }
    }

    pub fn list_usage_daily_rollups(
        &self,
        app_type: &str,
    ) -> Result<Vec<UsageDailyRollupRecord>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn
            .prepare(
                "SELECT date, app_type, provider_id, model, request_count, success_count,
                        input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
                        total_cost_usd, avg_latency_ms
                 FROM usage_daily_rollups
                 WHERE app_type = ?1
                 ORDER BY date DESC, provider_id ASC, model ASC",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(params![app_type], |row| {
                Ok(UsageDailyRollupRecord {
                    date: row.get(0)?,
                    app_type: row.get(1)?,
                    provider_id: row.get(2)?,
                    model: row.get(3)?,
                    request_count: row.get(4)?,
                    success_count: row.get(5)?,
                    input_tokens: row.get(6)?,
                    output_tokens: row.get(7)?,
                    cache_read_tokens: row.get(8)?,
                    cache_creation_tokens: row.get(9)?,
                    total_cost_usd: row.get(10)?,
                    avg_latency_ms: row.get(11)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(records)
    }
}
