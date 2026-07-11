use super::super::{
    from_json_string, lock_conn, to_json_string, Database, SETTINGS_STREAM_CHECK_CONFIG,
};
use crate::error::AppError;
use crate::services::stream_check::StreamCheckConfig;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};

impl Database {
    pub(crate) fn get_setting_with_conn(
        &self,
        conn: &Connection,
        key: &str,
    ) -> Result<Option<String>, AppError> {
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| AppError::Database(e.to_string()))
    }

    pub(crate) fn load_json_setting_with_conn<T: DeserializeOwned>(
        &self,
        conn: &Connection,
        key: &str,
    ) -> Result<Option<T>, AppError> {
        self.get_setting_with_conn(conn, key)?
            .map(|raw| from_json_string(&raw, key))
            .transpose()
    }

    pub(crate) fn set_setting_tx(
        tx: &rusqlite::Transaction<'_>,
        key: &str,
        value: &str,
    ) -> Result<(), AppError> {
        tx.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub(crate) fn set_json_setting_tx<T: Serialize>(
        tx: &rusqlite::Transaction<'_>,
        key: &str,
        value: &T,
    ) -> Result<(), AppError> {
        Self::set_setting_tx(tx, key, &to_json_string(value)?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        crate::webdav_auto_sync::notify_db_changed("settings");
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, AppError> {
        let conn = lock_conn!(self.conn);
        self.get_setting_with_conn(&conn, key)
    }

    pub fn get_stream_check_config(&self) -> Result<StreamCheckConfig, AppError> {
        let conn = lock_conn!(self.conn);
        Ok(self
            .load_json_setting_with_conn(&conn, SETTINGS_STREAM_CHECK_CONFIG)?
            .unwrap_or_default())
    }

    pub fn save_stream_check_config(&self, config: &StreamCheckConfig) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![SETTINGS_STREAM_CHECK_CONFIG, to_json_string(config)?],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        crate::webdav_auto_sync::notify_db_changed("settings");
        Ok(())
    }
}
