use std::collections::HashMap;

use crate::app_config::{AppType, McpApps, McpServer, MultiAppConfig};
use crate::error::AppError;
use crate::mcp;
use crate::store::AppState;

/// MCP 相关业务逻辑（v3.7.0 统一结构）
pub struct McpService;

impl McpService {
    /// 获取所有 MCP 服务器（统一结构）
    pub fn get_all_servers(state: &AppState) -> Result<HashMap<String, McpServer>, AppError> {
        let (servers, need_save) = {
            let mut cfg = state.config.write()?;

            // 从各客户端配置导入 MCP，确保统一结构完整
            let mut need_save = cfg.mcp.servers.is_none();
            let imported_from_claude = mcp::import_from_claude(&mut cfg)?;
            let imported_from_codex = mcp::import_from_codex(&mut cfg)?;
            let imported_from_gemini = mcp::import_from_gemini(&mut cfg)?;
            if imported_from_claude > 0 || imported_from_codex > 0 || imported_from_gemini > 0 {
                need_save = true;
            }

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

                    // 旧结构可能以 { enabled, server: {...} } 存储，也可能直接是 server 规范
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

            (servers, need_save)
        };

        if need_save {
            // 释放写锁后再保存，避免与 save() 内部的读锁互斥导致死锁
            state.save()?;
        }

        Ok(servers)
    }

    /// 添加或更新 MCP 服务器
    pub fn upsert_server(state: &AppState, server: McpServer) -> Result<(), AppError> {
        {
            let mut cfg = state.config.write()?;

            // 确保 servers 字段存在
            if cfg.mcp.servers.is_none() {
                cfg.mcp.servers = Some(HashMap::new());
            }

            let servers = cfg.mcp.servers.as_mut().unwrap();
            let id = server.id.clone();

            // 插入或更新
            servers.insert(id, server.clone());
        }

        state.save()?;

        // 同步到各个启用的应用
        Self::sync_server_to_apps(state, &server)?;

        Ok(())
    }

    /// 删除 MCP 服务器
    pub fn delete_server(state: &AppState, id: &str) -> Result<bool, AppError> {
        let server = {
            let mut cfg = state.config.write()?;

            if let Some(servers) = &mut cfg.mcp.servers {
                servers.remove(id)
            } else {
                None
            }
        };

        if let Some(server) = server {
            state.save()?;

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
        let server = {
            let mut cfg = state.config.write()?;

            if let Some(servers) = &mut cfg.mcp.servers {
                if let Some(server) = servers.get_mut(server_id) {
                    server.apps.set_enabled_for(&app, enabled);
                    Some(server.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(server) = server {
            state.save()?;

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
        let cfg = state.config.read()?;

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
        let cfg = state.config.read()?;
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
            AppType::Opencode | AppType::Omo => {
                return Err(AppError::localized(
                    "app_not_supported_yet",
                    format!("应用 '{}' 暂未支持，敬请期待。", app.as_str()),
                    format!("App '{}' is not supported yet.", app.as_str()),
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
            AppType::Opencode | AppType::Omo => {
                return Err(AppError::localized(
                    "app_not_supported_yet",
                    format!("应用 '{}' 暂未支持，敬请期待。", app.as_str()),
                    format!("App '{}' is not supported yet.", app.as_str()),
                ));
            }
        }
        Ok(())
    }

    /// 手动同步所有启用的 MCP 服务器到对应的应用
    pub fn sync_all_enabled(state: &AppState) -> Result<(), AppError> {
        let servers = Self::get_all_servers(state)?;

        for server in servers.values() {
            Self::sync_server_to_apps(state, server)?;
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
        let mut cfg = state.config.write()?;
        let count = mcp::import_from_claude(&mut cfg)?;
        drop(cfg);
        state.save()?;
        Ok(count)
    }

    /// 从 Codex 导入 MCP（v3.7.0 已更新为统一结构）
    pub fn import_from_codex(state: &AppState) -> Result<usize, AppError> {
        let mut cfg = state.config.write()?;
        let count = mcp::import_from_codex(&mut cfg)?;
        drop(cfg);
        state.save()?;
        Ok(count)
    }

    /// 从 Gemini 导入 MCP（v3.7.0 已更新为统一结构）
    pub fn import_from_gemini(state: &AppState) -> Result<usize, AppError> {
        let mut cfg = state.config.write()?;
        let count = mcp::import_from_gemini(&mut cfg)?;
        drop(cfg);
        state.save()?;
        Ok(count)
    }
}
