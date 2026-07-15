use std::collections::HashMap;

use crate::app_config::{AppType, McpApps, McpServer, MultiAppConfig};
use crate::error::AppError;
use crate::mcp;
use crate::store::AppState;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpImportSourceResult {
    pub source: String,
    pub discovered: usize,
    pub imported: usize,
    pub merged: usize,
    pub conflicts: usize,
    pub unchanged: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpImportResult {
    pub imported: usize,
    pub merged: usize,
    pub conflicts: usize,
    pub sources: Vec<McpImportSourceResult>,
}

/// MCP 相关业务逻辑（v3.7.0 统一结构）
pub struct McpService;

impl McpService {
    /// 获取所有 MCP 服务器（统一结构）
    pub fn get_all_servers(state: &AppState) -> Result<HashMap<String, McpServer>, AppError> {
        let mut cfg = state.load_config()?;
        let mut need_save = cfg.mcp.servers.is_none();

        // 新结构：空表示尚未配置任何 MCP 服务器，返回空 Map 而不是报错，避免初始加载失败。
        let mut servers = cfg.mcp.servers.clone().unwrap_or_default();

        // 兼容旧结构：如果旧的分应用配置仍有启用项，则合并到统一结构并标记对应 app。
        let mut merge_legacy = |legacy: &crate::app_config::McpConfig, app: &AppType| {
            for (id, entry) in legacy.servers.iter() {
                let enabled = entry
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !enabled {
                    continue;
                }

                let spec = entry
                    .get("server")
                    .cloned()
                    .unwrap_or_else(|| entry.clone());

                if let Some(existing) = servers.get_mut(id) {
                    existing.apps.set_enabled_for(app, true);
                } else {
                    servers.insert(
                        id.clone(),
                        McpServer {
                            id: id.clone(),
                            name: id.clone(),
                            server: spec.clone(),
                            apps: {
                                let mut apps = McpApps::default();
                                apps.set_enabled_for(app, true);
                                apps
                            },
                            description: None,
                            homepage: None,
                            docs: None,
                            tags: Vec::new(),
                        },
                    );
                }
            }
        };

        merge_legacy(&cfg.mcp.claude, &AppType::Claude);
        merge_legacy(&cfg.mcp.codex, &AppType::Codex);
        merge_legacy(&cfg.mcp.gemini, &AppType::Gemini);
        merge_legacy(&cfg.mcp.opencode, &AppType::Opencode);

        if cfg.mcp.servers.is_none() {
            need_save = true;
        }
        cfg.mcp.servers = Some(servers.clone());
        if need_save {
            state.replace_config(&cfg)?;
        }
        Ok(servers)
    }

    /// 添加或更新 MCP 服务器
    pub fn upsert_server(state: &AppState, server: McpServer) -> Result<(), AppError> {
        state.update_config(|cfg| {
            // 确保 servers 字段存在
            if cfg.mcp.servers.is_none() {
                cfg.mcp.servers = Some(HashMap::new());
            }

            let servers = cfg.mcp.servers.as_mut().unwrap();
            let id = server.id.clone();

            // 插入或更新
            servers.insert(id, server.clone());
            Ok(())
        })?;

        // 同步到各个启用的应用
        Self::sync_server_to_apps(state, &server)?;

        Ok(())
    }

    /// 删除 MCP 服务器
    pub fn delete_server(state: &AppState, id: &str) -> Result<bool, AppError> {
        let server = state.update_config(|cfg| {
            if let Some(servers) = &mut cfg.mcp.servers {
                Ok(servers.remove(id))
            } else {
                Ok(None)
            }
        })?;

        if let Some(server) = server {
            // 从所有应用的 live 配置中移除
            Self::remove_server_from_all_apps(state, id, &server)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 切换指定应用的启用状态
    pub fn toggle_app(
        state: &AppState,
        server_id: &str,
        app: AppType,
        enabled: bool,
    ) -> Result<(), AppError> {
        let server = state.update_config(|cfg| {
            if let Some(servers) = &mut cfg.mcp.servers {
                if let Some(server) = servers.get_mut(server_id) {
                    server.apps.set_enabled_for(&app, enabled);
                    Ok(Some(server.clone()))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        })?;

        if let Some(server) = server {
            // 同步到对应应用
            if enabled {
                Self::sync_server_to_app(state, &server, &app)?;
            } else {
                Self::remove_server_from_app(state, server_id, &app)?;
            }
        }

        Ok(())
    }

    /// 将 MCP 服务器同步到所有启用的应用
    fn sync_server_to_apps(state: &AppState, server: &McpServer) -> Result<(), AppError> {
        let cfg = state.load_config()?;

        for app in server.apps.enabled_apps() {
            Self::sync_server_to_app_internal(&cfg, server, &app)?;
        }

        Ok(())
    }

    /// 将 MCP 服务器同步到指定应用
    fn sync_server_to_app(
        state: &AppState,
        server: &McpServer,
        app: &AppType,
    ) -> Result<(), AppError> {
        let cfg = state.load_config()?;
        Self::sync_server_to_app_internal(&cfg, server, app)
    }

    fn sync_server_to_app_internal(
        cfg: &MultiAppConfig,
        server: &McpServer,
        app: &AppType,
    ) -> Result<(), AppError> {
        match app {
            AppType::Claude => {
                mcp::sync_single_server_to_claude(cfg, &server.id, &server.server)?;
            }
            AppType::Codex => {
                mcp::sync_single_server_to_codex(cfg, &server.id, &server.server)?;
            }
            AppType::Gemini => {
                mcp::sync_single_server_to_gemini(cfg, &server.id, &server.server)?;
            }
            AppType::Opencode | AppType::Omo | AppType::OmoSlim => {
                mcp::sync_single_server_to_opencode(cfg, &server.id, &server.server)?;
            }
            AppType::ClaudeDesktop | AppType::OpenClaw => {
                return Err(AppError::localized(
                    "mcp.app_not_supported",
                    format!("{} 暂不支持 MCP 同步。", app.as_str()),
                    format!("{} does not support MCP sync yet.", app.as_str()),
                ));
            }
        }
        Ok(())
    }

    /// 从所有曾启用过该服务器的应用中移除
    fn remove_server_from_all_apps(
        state: &AppState,
        id: &str,
        server: &McpServer,
    ) -> Result<(), AppError> {
        // 从所有曾启用的应用中移除
        for app in server.apps.enabled_apps() {
            Self::remove_server_from_app(state, id, &app)?;
        }
        Ok(())
    }

    fn remove_server_from_app(_state: &AppState, id: &str, app: &AppType) -> Result<(), AppError> {
        match app {
            AppType::Claude => mcp::remove_server_from_claude(id)?,
            AppType::Codex => mcp::remove_server_from_codex(id)?,
            AppType::Gemini => mcp::remove_server_from_gemini(id)?,
            AppType::Opencode | AppType::Omo | AppType::OmoSlim => {
                mcp::remove_server_from_opencode(id)?
            }
            AppType::ClaudeDesktop | AppType::OpenClaw => {}
        }
        Ok(())
    }

    /// 手动同步所有启用的 MCP 服务器到对应的应用
    pub fn sync_all_enabled(state: &AppState) -> Result<(), AppError> {
        let servers = Self::get_all_servers(state)?;
        let mut cfg = state.load_config()?;
        let mut sync_claude = false;
        let mut sync_codex = false;
        let mut sync_gemini = false;
        let mut sync_opencode = false;

        cfg.mcp.claude.servers.clear();
        cfg.mcp.codex.servers.clear();
        cfg.mcp.gemini.servers.clear();
        cfg.mcp.opencode.servers.clear();

        for (id, server) in servers {
            let entry = serde_json::json!({
                "id": id,
                "name": server.name,
                "server": server.server,
                "enabled": true,
                "description": server.description,
                "homepage": server.homepage,
                "docs": server.docs,
                "tags": server.tags,
            });

            if server.apps.claude {
                sync_claude = true;
                cfg.mcp
                    .claude
                    .servers
                    .insert(server.id.clone(), entry.clone());
            }
            if server.apps.codex {
                sync_codex = true;
                cfg.mcp
                    .codex
                    .servers
                    .insert(server.id.clone(), entry.clone());
            }
            if server.apps.gemini {
                sync_gemini = true;
                cfg.mcp
                    .gemini
                    .servers
                    .insert(server.id.clone(), entry.clone());
            }
            if server.apps.opencode {
                sync_opencode = true;
                cfg.mcp.opencode.servers.insert(server.id, entry);
            }
        }

        if sync_claude {
            mcp::sync_enabled_to_claude(&cfg)?;
        }
        if sync_codex {
            mcp::sync_enabled_to_codex(&cfg)?;
        }
        if sync_gemini {
            mcp::sync_enabled_to_gemini(&cfg)?;
        }
        if sync_opencode {
            for (id, entry) in &cfg.mcp.opencode.servers {
                if let Some(server_spec) = entry.get("server") {
                    mcp::sync_single_server_to_opencode(&cfg, id, server_spec)?;
                }
            }
        }

        Ok(())
    }

    // ========================================================================
    // 兼容层：支持旧的 v3.6.x 命令（已废弃，将在 v4.0 移除）
    // ========================================================================

    /// [已废弃] 获取指定应用的 MCP 服务器（兼容旧 API）
    #[deprecated(since = "3.7.0", note = "Use get_all_servers instead")]
    pub fn get_servers(
        state: &AppState,
        app: AppType,
    ) -> Result<HashMap<String, serde_json::Value>, AppError> {
        let all_servers = Self::get_all_servers(state)?;
        let mut result = HashMap::new();

        for (id, server) in all_servers {
            if server.apps.is_enabled_for(&app) {
                result.insert(id, server.server);
            }
        }

        Ok(result)
    }

    /// [已废弃] 设置 MCP 服务器在指定应用的启用状态（兼容旧 API）
    #[deprecated(since = "3.7.0", note = "Use toggle_app instead")]
    pub fn set_enabled(
        state: &AppState,
        app: AppType,
        id: &str,
        enabled: bool,
    ) -> Result<bool, AppError> {
        Self::toggle_app(state, id, app, enabled)?;
        Ok(true)
    }

    /// [已废弃] 同步启用的 MCP 到指定应用（兼容旧 API）
    #[deprecated(since = "3.7.0", note = "Use sync_all_enabled instead")]
    pub fn sync_enabled(state: &AppState, app: AppType) -> Result<(), AppError> {
        let servers = Self::get_all_servers(state)?;

        for server in servers.values() {
            if server.apps.is_enabled_for(&app) {
                Self::sync_server_to_app(state, server, &app)?;
            }
        }

        Ok(())
    }

    /// 从 Claude 导入 MCP（v3.7.0 已更新为统一结构）
    pub fn import_from_claude(state: &AppState) -> Result<usize, AppError> {
        state.update_config(mcp::import_from_claude)
    }

    /// 从 Codex 导入 MCP（v3.7.0 已更新为统一结构）
    pub fn import_from_codex(state: &AppState) -> Result<usize, AppError> {
        state.update_config(mcp::import_from_codex)
    }

    /// 从 Gemini 导入 MCP（v3.7.0 已更新为统一结构）
    pub fn import_from_gemini(state: &AppState) -> Result<usize, AppError> {
        state.update_config(mcp::import_from_gemini)
    }

    pub fn import_from_opencode(state: &AppState) -> Result<usize, AppError> {
        state.update_config(mcp::import_from_opencode)
    }

    pub fn import_from_all_apps(state: &AppState) -> Result<McpImportResult, AppError> {
        let mut sources = Vec::new();
        for (name, app, importer) in [
            (
                "claude",
                AppType::Claude,
                mcp::import_from_claude as fn(&mut MultiAppConfig) -> Result<usize, AppError>,
            ),
            ("codex", AppType::Codex, mcp::import_from_codex),
            ("gemini", AppType::Gemini, mcp::import_from_gemini),
            ("opencode", AppType::Opencode, mcp::import_from_opencode),
        ] {
            sources.push(Self::import_source(state, name, app, importer));
        }
        Ok(McpImportResult {
            imported: sources.iter().map(|source| source.imported).sum(),
            merged: sources.iter().map(|source| source.merged).sum(),
            conflicts: sources.iter().map(|source| source.conflicts).sum(),
            sources,
        })
    }

    fn import_source(
        state: &AppState,
        source: &str,
        app: AppType,
        importer: fn(&mut MultiAppConfig) -> Result<usize, AppError>,
    ) -> McpImportSourceResult {
        let mut temporary = MultiAppConfig::default();
        if let Err(error) = importer(&mut temporary) {
            return McpImportSourceResult {
                source: source.to_string(),
                discovered: 0,
                imported: 0,
                merged: 0,
                conflicts: 0,
                unchanged: 0,
                error: Some(error.to_string()),
            };
        }
        let discovered = temporary.mcp.servers.unwrap_or_default();
        let mut result = McpImportSourceResult {
            source: source.to_string(),
            discovered: discovered.len(),
            imported: 0,
            merged: 0,
            conflicts: 0,
            unchanged: 0,
            error: None,
        };
        let update = state.update_config(|config| {
            let existing = config.mcp.servers.get_or_insert_with(HashMap::new);
            merge_imported_servers(existing, discovered, &app, &mut result);
            Ok(())
        });
        if let Err(error) = update {
            result.error = Some(error.to_string());
            result.imported = 0;
            result.merged = 0;
        }
        result
    }
}

fn merge_imported_servers(
    existing: &mut HashMap<String, McpServer>,
    discovered: HashMap<String, McpServer>,
    app: &AppType,
    result: &mut McpImportSourceResult,
) {
    for (id, imported) in discovered {
        if let Some(current) = existing.get_mut(&id) {
            if current.server != imported.server {
                result.conflicts += 1;
            }
            if current.apps.is_enabled_for(app) {
                result.unchanged += 1;
            } else {
                current.apps.set_enabled_for(app, true);
                result.merged += 1;
            }
        } else {
            existing.insert(id, imported);
            result.imported += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn server(id: &str, command: &str, apps: McpApps) -> McpServer {
        McpServer {
            id: id.to_string(),
            name: id.to_string(),
            server: json!({"type": "stdio", "command": command}),
            apps,
            description: None,
            homepage: None,
            docs: None,
            tags: Vec::new(),
        }
    }

    #[test]
    fn import_preserves_local_conflict_and_only_enables_source_app() {
        let mut existing = HashMap::from([(
            "same".to_string(),
            server("same", "local", McpApps::default()),
        )]);
        let discovered = HashMap::from([
            (
                "same".to_string(),
                server("same", "remote", McpApps::default()),
            ),
            (
                "new".to_string(),
                server(
                    "new",
                    "new-command",
                    McpApps {
                        claude: true,
                        ..McpApps::default()
                    },
                ),
            ),
        ]);
        let mut result = McpImportSourceResult {
            source: "claude".to_string(),
            discovered: 2,
            imported: 0,
            merged: 0,
            conflicts: 0,
            unchanged: 0,
            error: None,
        };
        merge_imported_servers(&mut existing, discovered, &AppType::Claude, &mut result);

        assert_eq!(result.imported, 1);
        assert_eq!(result.merged, 1);
        assert_eq!(result.conflicts, 1);
        assert_eq!(existing["same"].server["command"], "local");
        assert!(existing["same"].apps.claude);
    }
}
