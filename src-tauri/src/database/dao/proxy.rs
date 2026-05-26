use super::super::{lock_conn, Database};
use crate::{
    error::AppError,
    settings::{ProxyAppSettings, ProxyAppsSettings, ProxySettings},
};
use rusqlite::{params, Connection, OptionalExtension};

impl Database {
    pub fn get_proxy_config(&self) -> Result<ProxySettings, AppError> {
        let conn = lock_conn!(self.conn);
        let global = conn
            .query_row(
                "SELECT enabled, host, port, upstream_proxy, bind_app, auto_start,
                        enable_logging, live_takeover_active, streaming_first_byte_timeout,
                        streaming_idle_timeout, non_streaming_timeout
                 FROM proxy_config
                 WHERE app_type = 'global'",
                [],
                |row| {
                    Ok(ProxySettings {
                        enabled: row.get::<_, i64>(0)? != 0,
                        host: row.get(1)?,
                        port: row.get::<_, i64>(2)? as u16,
                        upstream_proxy: row.get(3)?,
                        bind_app: row.get(4)?,
                        auto_start: row.get::<_, i64>(5)? != 0,
                        enable_logging: row.get::<_, i64>(6)? != 0,
                        live_takeover_active: row.get::<_, i64>(7)? != 0,
                        streaming_first_byte_timeout: row.get::<_, i64>(8)? as u64,
                        streaming_idle_timeout: row.get::<_, i64>(9)? as u64,
                        non_streaming_timeout: row.get::<_, i64>(10)? as u64,
                        apps: ProxyAppsSettings::default(),
                    })
                },
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))?
            .unwrap_or_default();

        let mut config = global;
        config.apps.claude = Self::load_proxy_app_config(&conn, "claude")?;
        config.apps.codex = Self::load_proxy_app_config(&conn, "codex")?;
        config.apps.gemini = Self::load_proxy_app_config(&conn, "gemini")?;
        config.apps.opencode = Self::load_proxy_app_config(&conn, "opencode")?;
        Ok(config)
    }

    pub fn save_proxy_config(&self, config: &ProxySettings) -> Result<(), AppError> {
        let mut conn = lock_conn!(self.conn);
        let tx = conn
            .transaction()
            .map_err(|e| AppError::Database(e.to_string()))?;

        tx.execute(
            "INSERT OR REPLACE INTO proxy_config (
                app_type, enabled, host, port, upstream_proxy, bind_app, auto_start,
                enable_logging, live_takeover_active, streaming_first_byte_timeout,
                streaming_idle_timeout, non_streaming_timeout, updated_at
            ) VALUES (
                'global', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now')
            )",
            params![
                i64::from(config.enabled),
                config.host,
                config.port as i64,
                config.upstream_proxy,
                config.bind_app,
                i64::from(config.auto_start),
                i64::from(config.enable_logging),
                i64::from(config.live_takeover_active),
                config.streaming_first_byte_timeout as i64,
                config.streaming_idle_timeout as i64,
                config.non_streaming_timeout as i64,
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        Self::save_proxy_app_config_tx(&tx, "claude", &config.apps.claude)?;
        Self::save_proxy_app_config_tx(&tx, "codex", &config.apps.codex)?;
        Self::save_proxy_app_config_tx(&tx, "gemini", &config.apps.gemini)?;
        Self::save_proxy_app_config_tx(&tx, "opencode", &config.apps.opencode)?;

        tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    fn load_proxy_app_config(
        conn: &Connection,
        app_type: &str,
    ) -> Result<ProxyAppSettings, AppError> {
        conn.query_row(
            "SELECT enabled, auto_failover_enabled, max_retries,
                    default_cost_multiplier, pricing_model_source
             FROM proxy_config
             WHERE app_type = ?1",
            params![app_type],
            |row| {
                Ok(ProxyAppSettings {
                    enabled: row.get::<_, i64>(0)? != 0,
                    auto_failover_enabled: row.get::<_, i64>(1)? != 0,
                    max_retries: row.get::<_, i64>(2)? as u8,
                    default_cost_multiplier: row.get(3)?,
                    pricing_model_source: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(|e| AppError::Database(e.to_string()))
        .map(|value| value.unwrap_or_default())
    }

    fn save_proxy_app_config_tx(
        tx: &rusqlite::Transaction<'_>,
        app_type: &str,
        config: &ProxyAppSettings,
    ) -> Result<(), AppError> {
        tx.execute(
            "INSERT OR REPLACE INTO proxy_config (
                app_type, enabled, auto_failover_enabled, max_retries,
                default_cost_multiplier, pricing_model_source, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            params![
                app_type,
                i64::from(config.enabled),
                i64::from(config.auto_failover_enabled),
                config.max_retries as i64,
                config.default_cost_multiplier,
                config.pricing_model_source,
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }
}
