use super::super::{from_json_string, lock_conn, to_json_string, Database};
use crate::{error::AppError, provider::UniversalProvider};
use rusqlite::{params, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;

impl Database {
    pub fn list_universal_provider_ids(&self) -> Result<Vec<String>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn
            .prepare("SELECT id FROM universal_providers ORDER BY id ASC")
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(ids)
    }

    pub fn get_universal_provider_json<T: DeserializeOwned>(
        &self,
        id: &str,
    ) -> Result<Option<T>, AppError> {
        let conn = lock_conn!(self.conn);
        conn.query_row(
            "SELECT value FROM universal_providers WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| AppError::Database(e.to_string()))?
        .map(|raw| from_json_string(&raw, "universal provider"))
        .transpose()
    }

    pub fn save_universal_provider_json<T: Serialize>(
        &self,
        id: &str,
        value: &T,
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "INSERT OR REPLACE INTO universal_providers (id, value)
             VALUES (?1, ?2)",
            params![id, to_json_string(value)?],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        crate::webdav_auto_sync::notify_db_changed("universal_providers");
        Ok(())
    }

    pub fn delete_universal_provider(&self, id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute("DELETE FROM universal_providers WHERE id = ?1", params![id])
            .map_err(|e| AppError::Database(e.to_string()))?;
        crate::webdav_auto_sync::notify_db_changed("universal_providers");
        Ok(())
    }

    pub fn get_all_universal_providers(
        &self,
    ) -> Result<HashMap<String, UniversalProvider>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn
            .prepare("SELECT id, value FROM universal_providers ORDER BY id ASC")
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| AppError::Database(e.to_string()))?;

        let mut providers = HashMap::new();
        for row in rows {
            let (id, raw) = row.map_err(|e| AppError::Database(e.to_string()))?;
            let provider: UniversalProvider = from_json_string(&raw, "universal provider")?;
            providers.insert(id, provider);
        }
        Ok(providers)
    }

    pub fn get_universal_provider(&self, id: &str) -> Result<Option<UniversalProvider>, AppError> {
        self.get_universal_provider_json(id)
    }

    pub fn save_universal_provider(&self, provider: &UniversalProvider) -> Result<(), AppError> {
        self.save_universal_provider_json(&provider.id, provider)
    }

    pub fn delete_universal_provider_typed(&self, id: &str) -> Result<bool, AppError> {
        let conn = lock_conn!(self.conn);
        let changed = conn
            .execute("DELETE FROM universal_providers WHERE id = ?1", params![id])
            .map_err(|e| AppError::Database(e.to_string()))?;
        if changed > 0 {
            crate::webdav_auto_sync::notify_db_changed("universal_providers");
        }
        Ok(changed > 0)
    }
}
