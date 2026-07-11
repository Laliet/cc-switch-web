use super::super::{lock_conn, Database};
use crate::error::AppError;
use rusqlite::params;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailoverQueueItem {
    pub app_type: String,
    pub provider_id: String,
    pub position: i64,
}

impl Database {
    pub fn list_failover_queue(&self, app_type: &str) -> Result<Vec<FailoverQueueItem>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn
            .prepare(
                "SELECT app_type, provider_id, position
                 FROM failover_queue
                 WHERE app_type = ?1
                 ORDER BY position ASC, provider_id ASC",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(params![app_type], |row| {
                Ok(FailoverQueueItem {
                    app_type: row.get(0)?,
                    provider_id: row.get(1)?,
                    position: row.get(2)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(items)
    }

    pub fn replace_failover_queue(
        &self,
        app_type: &str,
        provider_ids: &[String],
    ) -> Result<(), AppError> {
        let mut conn = lock_conn!(self.conn);
        let tx = conn
            .transaction()
            .map_err(|e| AppError::Database(e.to_string()))?;
        tx.execute(
            "DELETE FROM failover_queue WHERE app_type = ?1",
            params![app_type],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        for (index, provider_id) in provider_ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO failover_queue (app_type, provider_id, position)
                 VALUES (?1, ?2, ?3)",
                params![app_type, provider_id, index as i64],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
        tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
        crate::webdav_auto_sync::notify_db_changed("failover_queue");
        Ok(())
    }

    pub fn add_failover_provider(&self, app_type: &str, provider_id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let next_position = conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) + 1
                 FROM failover_queue
                 WHERE app_type = ?1",
                params![app_type],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let changed = conn
            .execute(
                "INSERT OR IGNORE INTO failover_queue (app_type, provider_id, position)
             VALUES (?1, ?2, ?3)",
                params![app_type, provider_id, next_position],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        if changed > 0 {
            crate::webdav_auto_sync::notify_db_changed("failover_queue");
        }
        Ok(())
    }

    pub fn remove_failover_provider(
        &self,
        app_type: &str,
        provider_id: &str,
    ) -> Result<(), AppError> {
        let remaining = self
            .list_failover_queue(app_type)?
            .into_iter()
            .filter(|item| item.provider_id != provider_id)
            .map(|item| item.provider_id)
            .collect::<Vec<_>>();
        self.replace_failover_queue(app_type, &remaining)
    }

    pub fn clear_failover_queue(&self, app_type: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let changed = conn
            .execute(
                "DELETE FROM failover_queue WHERE app_type = ?1",
                params![app_type],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        if changed > 0 {
            crate::webdav_auto_sync::notify_db_changed("failover_queue");
        }
        Ok(())
    }
}
