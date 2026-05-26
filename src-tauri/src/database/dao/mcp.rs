use super::super::{from_json_string, to_json_string, Database};
use crate::{
    app_config::{McpApps, McpRoot, McpServer, MultiAppConfig},
    error::AppError,
};
use rusqlite::{params, Connection};
use std::collections::HashMap;

impl Database {
    pub(crate) fn load_mcp_root(&self, conn: &Connection) -> Result<McpRoot, AppError> {
        let mut root = McpRoot::default();
        let mut servers = HashMap::new();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, server_config, description, homepage, docs, tags,
                        enabled_claude, enabled_codex, enabled_gemini, enabled_opencode
                 FROM mcp_servers",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                ))
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        for row in rows {
            let (
                id,
                name,
                server_raw,
                description,
                homepage,
                docs,
                tags_raw,
                claude,
                codex,
                gemini,
                opencode,
            ) = row.map_err(|e| AppError::Database(e.to_string()))?;
            servers.insert(
                id.clone(),
                McpServer {
                    id,
                    name,
                    server: from_json_string(&server_raw, "mcp server")?,
                    apps: McpApps {
                        claude: claude != 0,
                        codex: codex != 0,
                        gemini: gemini != 0,
                        opencode: opencode != 0,
                    },
                    description,
                    homepage,
                    docs,
                    tags: from_json_string(&tags_raw, "mcp tags")?,
                },
            );
        }
        root.servers = Some(servers);
        Ok(root)
    }

    pub(crate) fn save_mcp_tx(
        tx: &rusqlite::Transaction<'_>,
        config: &MultiAppConfig,
    ) -> Result<(), AppError> {
        if let Some(servers) = &config.mcp.servers {
            for (id, server) in servers {
                tx.execute(
                    "INSERT OR REPLACE INTO mcp_servers (
                        id, name, server_config, description, homepage, docs, tags,
                        enabled_claude, enabled_codex, enabled_gemini, enabled_opencode
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        id,
                        server.name,
                        to_json_string(&server.server)?,
                        server.description,
                        server.homepage,
                        server.docs,
                        to_json_string(&server.tags)?,
                        i64::from(server.apps.claude),
                        i64::from(server.apps.codex),
                        i64::from(server.apps.gemini),
                        i64::from(server.apps.opencode),
                    ],
                )
                .map_err(|e| AppError::Database(e.to_string()))?;
            }
        }
        Ok(())
    }
}
