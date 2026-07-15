use super::super::{lock_conn, Database};
use crate::error::AppError;
use rusqlite::{params, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCheckLogRecord {
    pub id: i64,
    pub provider_id: String,
    pub provider_name: String,
    pub app_type: String,
    pub status: String,
    pub success: bool,
    pub message: String,
    pub response_time_ms: Option<i64>,
    pub http_status: Option<i64>,
    pub model_used: String,
    pub retry_count: i64,
    pub error_category: Option<String>,
    pub tested_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamCheckLogFilters {
    pub app_type: Option<String>,
    pub provider_id: Option<String>,
    pub status: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: u32,
    pub offset: u32,
}

impl Database {
    pub fn insert_stream_check_log(
        &self,
        record: &StreamCheckLogRecord,
    ) -> Result<StreamCheckLogRecord, AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "INSERT INTO stream_check_logs (
                provider_id, provider_name, app_type, status, success, message,
                response_time_ms, http_status, model_used, retry_count,
                error_category, tested_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                record.provider_id,
                record.provider_name,
                record.app_type,
                record.status,
                i64::from(record.success),
                record.message,
                record.response_time_ms,
                record.http_status,
                record.model_used,
                record.retry_count,
                record.error_category,
                record.tested_at,
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        let id = conn.last_insert_rowid();

        // Keep diagnostics bounded. Stream checks are operational history, not an
        // archival log, and must not increase WebDAV snapshots without limit.
        conn.execute(
            "DELETE FROM stream_check_logs
             WHERE id NOT IN (
                 SELECT id FROM stream_check_logs
                 ORDER BY tested_at DESC, id DESC
                 LIMIT 5000
             )",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        Self::get_stream_check_log_by_id(&conn, id)?.ok_or_else(|| {
            AppError::Database("inserted stream check log could not be read".to_string())
        })
    }

    pub fn list_stream_check_logs(
        &self,
        filters: &StreamCheckLogFilters,
    ) -> Result<Vec<StreamCheckLogRecord>, AppError> {
        let conn = lock_conn!(self.conn);
        let limit = if filters.limit == 0 {
            100
        } else {
            filters.limit.min(500)
        };
        let offset = filters.offset.min(100_000);
        let mut stmt = conn
            .prepare(
                "SELECT id, provider_id, provider_name, app_type, status, success,
                        message, response_time_ms, http_status, model_used,
                        retry_count, error_category, tested_at
                 FROM stream_check_logs
                 WHERE (?1 IS NULL OR app_type = ?1)
                   AND (?2 IS NULL OR provider_id = ?2)
                   AND (?3 IS NULL OR status = ?3)
                   AND (?4 IS NULL OR tested_at >= ?4)
                   AND (?5 IS NULL OR tested_at <= ?5)
                 ORDER BY tested_at DESC, id DESC
                 LIMIT ?6 OFFSET ?7",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(
                params![
                    filters.app_type,
                    filters.provider_id,
                    filters.status,
                    filters.since,
                    filters.until,
                    i64::from(limit),
                    i64::from(offset),
                ],
                row_to_stream_check_log,
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        rows.map(|row| row.map_err(|e| AppError::Database(e.to_string())))
            .collect()
    }

    pub fn list_latest_stream_check_logs(
        &self,
        app_type: Option<&str>,
    ) -> Result<Vec<StreamCheckLogRecord>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn
            .prepare(
                "SELECT l.id, l.provider_id, l.provider_name, l.app_type, l.status,
                        l.success, l.message, l.response_time_ms, l.http_status,
                        l.model_used, l.retry_count, l.error_category, l.tested_at
                 FROM stream_check_logs l
                 WHERE (?1 IS NULL OR l.app_type = ?1)
                   AND NOT EXISTS (
                       SELECT 1 FROM stream_check_logs newer
                       WHERE newer.app_type = l.app_type
                         AND newer.provider_id = l.provider_id
                         AND (newer.tested_at > l.tested_at
                              OR (newer.tested_at = l.tested_at AND newer.id > l.id))
                   )
                 ORDER BY l.app_type ASC, l.provider_name ASC, l.id DESC",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(params![app_type], row_to_stream_check_log)
            .map_err(|e| AppError::Database(e.to_string()))?;
        rows.map(|row| row.map_err(|e| AppError::Database(e.to_string())))
            .collect()
    }

    fn get_stream_check_log_by_id(
        conn: &rusqlite::Connection,
        id: i64,
    ) -> Result<Option<StreamCheckLogRecord>, AppError> {
        conn.query_row(
            "SELECT id, provider_id, provider_name, app_type, status, success,
                    message, response_time_ms, http_status, model_used,
                    retry_count, error_category, tested_at
             FROM stream_check_logs WHERE id = ?1",
            params![id],
            row_to_stream_check_log,
        )
        .optional()
        .map_err(|e| AppError::Database(e.to_string()))
    }
}

fn row_to_stream_check_log(row: &rusqlite::Row<'_>) -> rusqlite::Result<StreamCheckLogRecord> {
    Ok(StreamCheckLogRecord {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        provider_name: row.get(2)?,
        app_type: row.get(3)?,
        status: row.get(4)?,
        success: row.get::<_, i64>(5)? != 0,
        message: row.get(6)?,
        response_time_ms: row.get(7)?,
        http_status: row.get(8)?,
        model_used: row.get(9)?,
        retry_count: row.get(10)?,
        error_category: row.get(11)?,
        tested_at: row.get(12)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{StreamCheckLogFilters, StreamCheckLogRecord};
    use crate::database::Database;

    fn record(provider_id: &str, tested_at: i64) -> StreamCheckLogRecord {
        StreamCheckLogRecord {
            id: 0,
            provider_id: provider_id.to_string(),
            provider_name: provider_id.to_string(),
            app_type: "claude".to_string(),
            status: "operational".to_string(),
            success: true,
            message: "ok".to_string(),
            response_time_ms: Some(42),
            http_status: Some(200),
            model_used: "claude-haiku".to_string(),
            retry_count: 0,
            error_category: None,
            tested_at,
        }
    }

    #[test]
    fn stream_check_logs_roundtrip_and_latest() {
        let db = Database::memory().expect("memory db");
        db.insert_stream_check_log(&record("p1", 10))
            .expect("insert");
        db.insert_stream_check_log(&record("p1", 20))
            .expect("insert");
        db.insert_stream_check_log(&record("p2", 15))
            .expect("insert");

        let latest = db
            .list_latest_stream_check_logs(Some("claude"))
            .expect("latest");
        assert_eq!(latest.len(), 2);
        assert_eq!(
            latest
                .iter()
                .find(|item| item.provider_id == "p1")
                .unwrap()
                .tested_at,
            20
        );

        let filtered = db
            .list_stream_check_logs(&StreamCheckLogFilters {
                provider_id: Some("p2".to_string()),
                limit: 10,
                ..Default::default()
            })
            .expect("filtered");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].provider_id, "p2");
    }
}
