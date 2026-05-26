use super::{database_path, lock_conn, Database, SETTINGS_DB_MIGRATED_FROM_JSON};
use crate::{app_config::MultiAppConfig, config::get_app_config_path, error::AppError};

impl Database {
    pub(crate) fn migrate_legacy_json_if_needed(&self) -> Result<(), AppError> {
        if self.get_setting(SETTINGS_DB_MIGRATED_FROM_JSON)?.is_some() {
            return Ok(());
        }
        if !self.is_empty()? {
            self.set_setting(SETTINGS_DB_MIGRATED_FROM_JSON, "skipped-existing-db")?;
            return Ok(());
        }

        let config_path = get_app_config_path()?;
        let config = if config_path.exists() {
            Some(MultiAppConfig::load()?)
        } else {
            Some(MultiAppConfig::default_with_auto_import()?)
        };

        if let Some(config) = config {
            self.replace_config(&config)?;
            self.save_proxy_config(&crate::settings::get_settings().proxy)?;
            self.set_setting(SETTINGS_DB_MIGRATED_FROM_JSON, "true")?;
            log::info!(
                "Migrated legacy config snapshot into SQLite database: {}",
                database_path()?.display()
            );
        }
        Ok(())
    }

    pub(crate) fn is_empty(&self) -> Result<bool, AppError> {
        let conn = lock_conn!(self.conn);
        let provider_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM providers", [], |row| row.get(0))
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mcp_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM mcp_servers", [], |row| row.get(0))
            .map_err(|e| AppError::Database(e.to_string()))?;
        let prompt_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM prompts", [], |row| row.get(0))
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(provider_count == 0 && mcp_count == 0 && prompt_count == 0)
    }
}
