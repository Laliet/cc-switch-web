use super::super::{from_json_string, lock_conn, to_json_string, Database};
use crate::error::AppError;
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
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, AppError> {
        let conn = lock_conn!(self.conn);
        self.get_setting_with_conn(&conn, key)
    }
}
