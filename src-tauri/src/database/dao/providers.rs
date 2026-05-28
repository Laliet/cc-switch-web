use super::super::{from_json_string, to_json_string, Database};
use crate::{
    app_config::{AppType, MultiAppConfig},
    error::AppError,
    provider::{Provider, ProviderManager},
};
use rusqlite::{params, Connection};
use std::collections::HashMap;

impl Database {
    pub(crate) fn load_provider_managers(
        &self,
        conn: &Connection,
    ) -> Result<HashMap<String, ProviderManager>, AppError> {
        let mut apps = HashMap::new();
        for app in [
            AppType::Claude,
            AppType::Codex,
            AppType::Gemini,
            AppType::Opencode,
            AppType::Omo,
            AppType::OmoSlim,
        ] {
            apps.insert(app.as_str().to_string(), ProviderManager::default());
        }

        let mut stmt = conn
            .prepare(
                "SELECT id, app_type, name, settings_config, website_url, category,
                        created_at, sort_index, notes, meta, is_current, backup_current
                 FROM providers
                 ORDER BY app_type, COALESCE(sort_index, 9223372036854775807), created_at, name",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, Option<String>>(11)?,
                ))
            })
            .map_err(|e| AppError::Database(e.to_string()))?;

        for row in rows {
            let (
                id,
                app_type,
                name,
                settings_config_raw,
                website_url,
                category,
                created_at,
                sort_index,
                notes,
                meta_raw,
                is_current,
                backup_current,
            ) = row.map_err(|e| AppError::Database(e.to_string()))?;

            let mut meta: crate::provider::ProviderMeta =
                from_json_string(&meta_raw, "provider meta")?;
            meta.custom_endpoints =
                Self::load_provider_endpoints(conn, &app_type, &id).unwrap_or_default();

            let provider = Provider {
                id: id.clone(),
                name,
                settings_config: from_json_string(&settings_config_raw, "provider settings")?,
                website_url,
                category,
                created_at,
                sort_index: sort_index.and_then(|v| usize::try_from(v).ok()),
                notes,
                meta: Some(meta).filter(|m| {
                    !m.custom_endpoints.is_empty()
                        || m.usage_script.is_some()
                        || m.is_partner.is_some()
                        || m.partner_promotion_key.is_some()
                }),
            };

            let manager = apps
                .entry(app_type)
                .or_insert_with(ProviderManager::default);
            if is_current != 0 {
                manager.current = id.clone();
            }
            if manager.backup_current.is_none() {
                manager.backup_current = backup_current;
            }
            manager.providers.insert(id, provider);
        }

        Ok(apps)
    }

    fn load_provider_endpoints(
        conn: &Connection,
        app_type: &str,
        provider_id: &str,
    ) -> Result<HashMap<String, crate::settings::CustomEndpoint>, AppError> {
        let mut stmt = conn
            .prepare(
                "SELECT url, added_at, last_used
                 FROM provider_endpoints
                 WHERE app_type = ?1 AND provider_id = ?2",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(params![app_type, provider_id], |row| {
                Ok(crate::settings::CustomEndpoint {
                    url: row.get(0)?,
                    added_at: row.get(1)?,
                    last_used: row.get(2)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;

        let mut result = HashMap::new();
        for row in rows {
            let endpoint = row.map_err(|e| AppError::Database(e.to_string()))?;
            result.insert(endpoint.url.clone(), endpoint);
        }
        Ok(result)
    }

    pub(crate) fn save_providers_tx(
        tx: &rusqlite::Transaction<'_>,
        config: &MultiAppConfig,
    ) -> Result<(), AppError> {
        for (app_type, manager) in &config.apps {
            for (id, provider) in &manager.providers {
                let is_current = i64::from(manager.current == *id);
                let mut meta = provider.meta.clone().unwrap_or_default();
                let endpoints = std::mem::take(&mut meta.custom_endpoints);
                tx.execute(
                    "INSERT OR REPLACE INTO providers (
                        id, app_type, name, settings_config, website_url, category,
                        created_at, sort_index, notes, meta, is_current, backup_current
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        id,
                        app_type,
                        provider.name,
                        to_json_string(&provider.settings_config)?,
                        provider.website_url,
                        provider.category,
                        provider.created_at,
                        provider.sort_index.map(|v| v as i64),
                        provider.notes,
                        to_json_string(&meta)?,
                        is_current,
                        manager.backup_current,
                    ],
                )
                .map_err(|e| AppError::Database(e.to_string()))?;

                for endpoint in endpoints.values() {
                    tx.execute(
                        "INSERT OR REPLACE INTO provider_endpoints
                         (provider_id, app_type, url, added_at, last_used)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            id,
                            app_type,
                            endpoint.url,
                            endpoint.added_at,
                            endpoint.last_used,
                        ],
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                }
            }
        }
        Ok(())
    }
}
