use super::provider::ProviderService;
use crate::app_config::{AppType, MultiAppConfig};
use crate::config::atomic_write;
use crate::error::AppError;
use crate::provider::Provider;
use crate::store::AppState;
use chrono::Utc;
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_BACKUPS: usize = 10;
static BACKUP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 配置导入导出相关业务逻辑
pub struct ConfigService;

impl ConfigService {
    pub fn sanitize_transfer_path(raw_path: &str) -> Result<PathBuf, AppError> {
        let trimmed = raw_path.trim();
        if trimmed.is_empty() {
            return Err(AppError::InvalidInput("filePath is required".into()));
        }
        if trimmed.contains('\0') {
            return Err(AppError::InvalidInput(
                "filePath contains invalid characters".into(),
            ));
        }

        let expanded = Self::expand_home_path(trimmed)?;
        let path = PathBuf::from(expanded);

        if cfg!(feature = "web-server") {
            Self::normalize_transfer_path(&path)
        } else {
            Ok(path)
        }
    }

    fn expand_home_path(raw: &str) -> Result<String, AppError> {
        if raw == "~" || raw.starts_with("~/") || raw.starts_with("~\\") {
            let home = crate::config::get_home_dir()
                .ok_or_else(|| AppError::InvalidInput("Home directory not available".into()))?;
            if raw.len() == 1 {
                return Ok(home.to_string_lossy().to_string());
            }
            let suffix = &raw[2..];
            return Ok(home.join(suffix).to_string_lossy().to_string());
        }
        Ok(raw.to_string())
    }

    fn sanitize_relative_path(path: &Path) -> Result<PathBuf, AppError> {
        let mut sanitized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::Normal(part) => sanitized.push(part),
                Component::CurDir => {}
                Component::ParentDir => {
                    return Err(AppError::InvalidInput(
                        "filePath must not contain '..'".into(),
                    ));
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(AppError::InvalidInput(
                        "filePath must be a relative path".into(),
                    ));
                }
            }
        }
        if sanitized.as_os_str().is_empty() {
            return Err(AppError::InvalidInput("filePath is required".into()));
        }
        Ok(sanitized)
    }

    fn normalize_transfer_path(path: &Path) -> Result<PathBuf, AppError> {
        if !cfg!(feature = "web-server") {
            return Ok(path.to_path_buf());
        }

        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            let relative = Self::sanitize_relative_path(path)?;
            crate::config::get_app_config_dir()?.join(relative)
        };

        Self::canonicalize_for_validation(&resolved)
    }

    fn validate_transfer_path(path: &Path) -> Result<PathBuf, AppError> {
        if !cfg!(feature = "web-server") {
            return Ok(path.to_path_buf());
        }

        let normalized = Self::normalize_transfer_path(path)?;
        let allowed_dirs = Self::allowed_transfer_dirs()?;
        let allowed_normalized = allowed_dirs
            .iter()
            .map(|p| Self::canonicalize_for_validation(p.as_path()))
            .collect::<Result<Vec<_>, _>>()?;

        if allowed_normalized
            .iter()
            .any(|allowed| normalized.starts_with(allowed))
        {
            Ok(normalized)
        } else {
            Err(AppError::InvalidInput(
                "filePath is outside allowed directories".into(),
            ))
        }
    }

    fn allowed_transfer_dirs() -> Result<Vec<PathBuf>, AppError> {
        Ok(vec![crate::config::get_app_config_dir()?])
    }

    fn canonicalize_for_validation(path: &Path) -> Result<PathBuf, AppError> {
        if path.as_os_str().is_empty() {
            return Err(AppError::InvalidInput("Invalid file path".into()));
        }
        if path.exists() {
            return fs::canonicalize(path).map_err(|e| AppError::io(path, e));
        }

        let mut missing: Vec<OsString> = Vec::new();
        let mut cursor = path;
        while !cursor.exists() {
            if let Some(name) = cursor.file_name() {
                missing.push(name.to_os_string());
            } else {
                break;
            }
            if let Some(parent) = cursor.parent() {
                cursor = parent;
            } else {
                break;
            }
        }

        let mut resolved = fs::canonicalize(cursor).map_err(|e| AppError::io(cursor, e))?;
        for component in missing.iter().rev() {
            resolved.push(component);
        }
        Ok(resolved)
    }

    /// 为当前 config.json 创建备份，返回备份 ID（若文件不存在则返回空字符串）。
    pub fn create_backup(config_path: &Path) -> Result<String, AppError> {
        if !config_path.exists() {
            return Ok(String::new());
        }

        let backup_dir = config_path
            .parent()
            .ok_or_else(|| AppError::Config("Invalid config path".into()))?
            .join("backups");

        fs::create_dir_all(&backup_dir).map_err(|e| AppError::io(&backup_dir, e))?;

        let timestamp_ms = Utc::now().timestamp_millis();
        let counter = BACKUP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let backup_id = format!("backup_{timestamp_ms}_{counter}");

        let backup_path = backup_dir.join(format!("{backup_id}.json"));
        let contents = fs::read(config_path).map_err(|e| AppError::io(config_path, e))?;
        atomic_write(&backup_path, &contents)?;

        Self::cleanup_old_backups(&backup_dir, MAX_BACKUPS)?;

        Ok(backup_id)
    }

    fn cleanup_old_backups(backup_dir: &Path, retain: usize) -> Result<(), AppError> {
        if retain == 0 {
            return Ok(());
        }

        let entries = match fs::read_dir(backup_dir) {
            Ok(iter) => iter
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .map(|ext| ext == "json")
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>(),
            Err(_) => return Ok(()),
        };

        if entries.len() <= retain {
            return Ok(());
        }

        let remove_count = entries.len().saturating_sub(retain);
        let mut sorted = entries;

        sorted.sort_by(|a, b| {
            let a_time = a.metadata().and_then(|m| m.modified()).ok();
            let b_time = b.metadata().and_then(|m| m.modified()).ok();
            a_time.cmp(&b_time)
        });

        for entry in sorted.into_iter().take(remove_count) {
            if let Err(err) = fs::remove_file(entry.path()) {
                log::warn!(
                    "Failed to remove old backup {}: {}",
                    entry.path().display(),
                    err
                );
            }
        }

        Ok(())
    }

    /// 将当前 config.json 拷贝到目标路径。
    pub fn export_config_to_path(target_path: &Path) -> Result<(), AppError> {
        let target_path = Self::validate_transfer_path(target_path)?;
        let config_path = crate::config::get_app_config_path()?;
        let config_content =
            fs::read_to_string(&config_path).map_err(|e| AppError::io(&config_path, e))?;
        atomic_write(&target_path, config_content.as_bytes())
    }

    /// 从磁盘文件加载配置并进行校验，返回新配置。
    pub fn load_config_for_import(file_path: &Path) -> Result<MultiAppConfig, AppError> {
        let file_path = Self::validate_transfer_path(file_path)?;
        let import_content =
            fs::read_to_string(&file_path).map_err(|e| AppError::io(&file_path, e))?;

        let value: serde_json::Value =
            serde_json::from_str(&import_content).map_err(|e| AppError::json(&file_path, e))?;
        MultiAppConfig::ensure_not_v1_value(&value)?;
        let has_skills_in_config = value
            .as_object()
            .is_some_and(|map| map.contains_key("skills"));
        let mut new_config: MultiAppConfig =
            serde_json::from_value(value).map_err(|e| AppError::json(&file_path, e))?;
        let _ = new_config.normalize_after_load(has_skills_in_config)?;
        Ok(new_config)
    }

    /// 将外部配置文件内容加载并写入应用状态。
    pub fn import_config_from_path(file_path: &Path, state: &AppState) -> Result<String, AppError> {
        let new_config = Self::load_config_for_import(file_path)?;
        Self::apply_import_config(new_config, state)
    }

    /// 将导入配置写入磁盘并同步到 AppState，返回备份 ID。
    pub fn apply_import_config(
        new_config: MultiAppConfig,
        state: &AppState,
    ) -> Result<String, AppError> {
        let mut guard = state.config.write().map_err(AppError::from)?;
        let config_path = crate::config::get_app_config_path()?;
        let backup_id = Self::create_backup(&config_path)?;

        Self::save_config_to_path(&new_config, &config_path)?;
        *guard = new_config;

        Ok(backup_id)
    }

    fn save_config_to_path(config: &MultiAppConfig, config_path: &Path) -> Result<(), AppError> {
        use crate::config::{copy_file, write_json_file};

        if config_path.exists() {
            let backup_path = config_path.with_extension("json.bak");
            if let Err(e) = copy_file(config_path, &backup_path) {
                log::warn!("备份 config.json 到 .bak 失败: {e}");
            }
        }

        write_json_file(config_path, config)?;
        Ok(())
    }

    /// 同步当前供应商到对应的 live 配置。
    pub fn sync_current_providers_to_live(config: &mut MultiAppConfig) -> Result<(), AppError> {
        Self::sync_current_provider_for_app(config, &AppType::Claude)?;
        Self::sync_current_provider_for_app(config, &AppType::Codex)?;
        Self::sync_current_provider_for_app(config, &AppType::Gemini)?;
        Ok(())
    }

    fn sync_current_provider_for_app(
        config: &mut MultiAppConfig,
        app_type: &AppType,
    ) -> Result<(), AppError> {
        let (current_id, provider) = {
            let manager = match config.get_manager(app_type) {
                Some(manager) => manager,
                None => return Ok(()),
            };

            if manager.current.is_empty() {
                return Ok(());
            }

            let current_id = manager.current.clone();
            let provider = match manager.providers.get(&current_id) {
                Some(provider) => provider.clone(),
                None => {
                    log::warn!(
                        "当前应用 {app_type:?} 的供应商 {current_id} 不存在，跳过 live 同步"
                    );
                    return Ok(());
                }
            };
            (current_id, provider)
        };

        match app_type {
            AppType::Codex => Self::sync_codex_live(config, &current_id, &provider)?,
            AppType::Claude => Self::sync_claude_live(config, &current_id, &provider)?,
            AppType::Gemini => Self::sync_gemini_live(config, &current_id, &provider)?,
            AppType::Opencode | AppType::Omo => {
                return Err(AppError::localized(
                    "app_not_supported_yet",
                    format!("应用 '{}' 暂未支持，敬请期待。", app_type.as_str()),
                    format!("App '{}' is not supported yet.", app_type.as_str()),
                ));
            }
        }

        Ok(())
    }

    fn sync_codex_live(
        config: &mut MultiAppConfig,
        provider_id: &str,
        provider: &Provider,
    ) -> Result<(), AppError> {
        let settings = provider.settings_config.as_object().ok_or_else(|| {
            AppError::Config(format!("供应商 {provider_id} 的 Codex 配置必须是对象"))
        })?;
        let auth = settings.get("auth").ok_or_else(|| {
            AppError::Config(format!("供应商 {provider_id} 的 Codex 配置缺少 auth 字段"))
        })?;
        if !auth.is_object() {
            return Err(AppError::Config(format!(
                "供应商 {provider_id} 的 Codex auth 配置必须是 JSON 对象"
            )));
        }
        let cfg_text = match settings.get("config") {
            None => None,
            Some(Value::String(s)) => Some(s.as_str()),
            Some(_) => {
                return Err(AppError::Config(format!(
                    "供应商 {provider_id} 的 Codex config 必须是字符串"
                )));
            }
        };

        crate::codex_config::write_codex_live_atomic(auth, cfg_text)?;
        crate::mcp::sync_enabled_to_codex(config)?;

        let cfg_text_after = crate::codex_config::read_and_validate_codex_config_text()?;
        if let Some(manager) = config.get_manager_mut(&AppType::Codex) {
            if let Some(target) = manager.providers.get_mut(provider_id) {
                if let Some(obj) = target.settings_config.as_object_mut() {
                    obj.insert(
                        "config".to_string(),
                        serde_json::Value::String(cfg_text_after),
                    );
                }
            }
        }

        Ok(())
    }

    fn sync_claude_live(
        config: &mut MultiAppConfig,
        provider_id: &str,
        provider: &Provider,
    ) -> Result<(), AppError> {
        use crate::config::{read_json_file, write_json_file};

        let settings_path = crate::config::get_claude_settings_path()?;
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
        }

        write_json_file(&settings_path, &provider.settings_config)?;

        let live_after = read_json_file::<serde_json::Value>(&settings_path)?;
        if let Some(manager) = config.get_manager_mut(&AppType::Claude) {
            if let Some(target) = manager.providers.get_mut(provider_id) {
                target.settings_config = live_after;
            }
        }

        Ok(())
    }

    fn sync_gemini_live(
        config: &mut MultiAppConfig,
        provider_id: &str,
        provider: &Provider,
    ) -> Result<(), AppError> {
        use crate::gemini_config::{env_to_json, read_gemini_env};

        ProviderService::write_gemini_live(provider)?;

        // 读回实际写入的内容并更新到配置中（包含 settings.json）
        let live_after_env = read_gemini_env()?;
        let settings_path = crate::gemini_config::get_gemini_settings_path()?;
        let live_after_config = if settings_path.exists() {
            crate::config::read_json_file(&settings_path)?
        } else {
            serde_json::json!({})
        };
        let mut live_after = env_to_json(&live_after_env);
        if let Some(obj) = live_after.as_object_mut() {
            obj.insert("config".to_string(), live_after_config);
        }

        if let Some(manager) = config.get_manager_mut(&AppType::Gemini) {
            if let Some(target) = manager.providers.get_mut(provider_id) {
                target.settings_config = live_after;
            }
        }

        Ok(())
    }
}
