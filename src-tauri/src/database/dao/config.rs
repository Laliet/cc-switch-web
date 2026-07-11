use super::super::{lock_conn, Database, SETTINGS_COMMON_SNIPPETS, SETTINGS_CONFIG_VERSION};
use crate::{app_config::MultiAppConfig, error::AppError};

impl Database {
    pub fn load_config(&self) -> Result<MultiAppConfig, AppError> {
        let conn = lock_conn!(self.conn);
        Ok(MultiAppConfig {
            version: self
                .get_setting_with_conn(&conn, SETTINGS_CONFIG_VERSION)?
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(2),
            apps: self.load_provider_managers(&conn)?,
            mcp: self.load_mcp_root(&conn)?,
            prompts: self.load_prompt_root(&conn)?,
            skills: self.load_skill_store(&conn)?,
            common_config_snippets: self
                .load_json_setting_with_conn(&conn, SETTINGS_COMMON_SNIPPETS)?
                .unwrap_or_default(),
            claude_common_config_snippet: None,
        })
    }

    pub fn replace_config(&self, config: &MultiAppConfig) -> Result<(), AppError> {
        let mut conn = lock_conn!(self.conn);
        let tx = conn
            .transaction()
            .map_err(|e| AppError::Database(e.to_string()))?;

        tx.execute("DELETE FROM provider_endpoints", [])
            .map_err(|e| AppError::Database(e.to_string()))?;
        tx.execute("DELETE FROM providers", [])
            .map_err(|e| AppError::Database(e.to_string()))?;
        tx.execute("DELETE FROM mcp_servers", [])
            .map_err(|e| AppError::Database(e.to_string()))?;
        tx.execute("DELETE FROM prompts", [])
            .map_err(|e| AppError::Database(e.to_string()))?;
        tx.execute("DELETE FROM skill_repos", [])
            .map_err(|e| AppError::Database(e.to_string()))?;
        tx.execute("DELETE FROM skill_states", [])
            .map_err(|e| AppError::Database(e.to_string()))?;
        tx.execute("DELETE FROM skill_repo_cache", [])
            .map_err(|e| AppError::Database(e.to_string()))?;

        Self::save_providers_tx(&tx, config)?;
        Self::save_mcp_tx(&tx, config)?;
        Self::save_prompts_tx(&tx, config)?;
        Self::save_skills_tx(&tx, &config.skills)?;
        Self::set_setting_tx(&tx, SETTINGS_CONFIG_VERSION, &config.version.to_string())?;
        Self::set_json_setting_tx(
            &tx,
            SETTINGS_COMMON_SNIPPETS,
            &config.common_config_snippets,
        )?;

        tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
        crate::webdav_auto_sync::notify_db_changed("providers");
        Ok(())
    }

    pub fn update_config<F, R>(&self, f: F) -> Result<R, AppError>
    where
        F: FnOnce(&mut MultiAppConfig) -> Result<R, AppError>,
    {
        let mut config = self.load_config()?;
        let result = f(&mut config)?;
        self.replace_config(&config)?;
        Ok(result)
    }
}
