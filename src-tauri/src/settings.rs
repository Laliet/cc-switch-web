use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use crate::{
    config::{atomic_write, get_account_home_dir, get_home_dir},
    error::AppError,
    services::skill::{SkillStorageLocation, SyncMethod},
};

fn select_preferred_home_dir(
    service_home: Option<PathBuf>,
    account_home: Option<PathBuf>,
    prefer_account_home: bool,
) -> Option<PathBuf> {
    match (service_home, account_home) {
        (Some(service), Some(account)) if prefer_account_home && service != account => {
            Some(account)
        }
        (Some(service), _) => Some(service),
        (None, Some(account)) => Some(account),
        (None, None) => None,
    }
}

fn get_preferred_home_dir() -> Option<PathBuf> {
    select_preferred_home_dir(
        get_home_dir(),
        get_account_home_dir(),
        cfg!(feature = "web-server"),
    )
}

fn settings_path_for_home(home: &Path) -> PathBuf {
    home.join(".cc-switch").join("settings.json")
}

fn get_legacy_service_settings_path() -> Option<PathBuf> {
    let service_home = get_home_dir()?;
    let preferred_home = get_preferred_home_dir()?;

    if service_home == preferred_home {
        return None;
    }

    Some(settings_path_for_home(&service_home))
}

fn migrate_legacy_default_override_with_homes(
    raw: Option<String>,
    default_dir: &Path,
    service_home: Option<PathBuf>,
    preferred_home: Option<PathBuf>,
    prefer_account_home: bool,
) -> Option<String> {
    let raw = raw?;
    if !prefer_account_home {
        return Some(raw);
    }

    let service_home = match service_home {
        Some(home) => home,
        None => return Some(raw),
    };
    let preferred_home = match preferred_home {
        Some(home) => home,
        None => return Some(raw),
    };

    if service_home == preferred_home {
        return Some(raw);
    }

    let legacy_default = service_home.join(default_dir);
    if Path::new(&raw) == legacy_default.as_path() {
        return Some(
            preferred_home
                .join(default_dir)
                .to_string_lossy()
                .to_string(),
        );
    }

    Some(raw)
}

fn migrate_legacy_default_override(raw: Option<String>, default_dir: &Path) -> Option<String> {
    migrate_legacy_default_override_with_homes(
        raw,
        default_dir,
        get_home_dir(),
        get_preferred_home_dir(),
        cfg!(feature = "web-server"),
    )
}

fn resolve_override_path_with_home(raw: &str, preferred_home: Option<PathBuf>) -> PathBuf {
    if raw == "~" {
        if let Some(home) = preferred_home {
            return home;
        }
    } else if let Some(stripped) = raw.strip_prefix("~/") {
        if let Some(home) = preferred_home {
            return home.join(stripped);
        }
    } else if let Some(stripped) = raw.strip_prefix("~\\") {
        if let Some(home) = preferred_home {
            return home.join(stripped);
        }
    }

    PathBuf::from(raw)
}

/// 自定义端点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomEndpoint {
    pub url: String,
    pub added_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecurityAuthSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecuritySettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<SecurityAuthSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_proxy_host")]
    pub host: String,
    #[serde(default = "default_proxy_port")]
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_proxy: Option<String>,
    #[serde(default = "default_proxy_bind_app")]
    pub bind_app: String,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub enable_logging: bool,
    #[serde(default)]
    pub live_takeover_active: bool,
    #[serde(default = "default_streaming_first_byte_timeout")]
    pub streaming_first_byte_timeout: u64,
    #[serde(default = "default_streaming_idle_timeout")]
    pub streaming_idle_timeout: u64,
    #[serde(default = "default_non_streaming_timeout")]
    pub non_streaming_timeout: u64,
    #[serde(default = "default_proxy_circuit_failure_threshold")]
    pub circuit_failure_threshold: u64,
    #[serde(default = "default_proxy_circuit_recovery_threshold")]
    pub circuit_recovery_threshold: u64,
    #[serde(default = "default_proxy_circuit_recovery_wait")]
    pub circuit_recovery_wait_seconds: u64,
    #[serde(default = "default_proxy_circuit_error_rate_threshold")]
    pub circuit_error_rate_threshold: f64,
    #[serde(default = "default_rectifier_enabled")]
    pub rectify_thinking_signature: bool,
    #[serde(default = "default_rectifier_enabled")]
    pub rectify_thinking_budget: bool,
    #[serde(default)]
    pub optimizer_enabled: bool,
    #[serde(default = "default_optimizer_subfeature_enabled")]
    pub optimizer_thinking: bool,
    #[serde(default = "default_optimizer_subfeature_enabled")]
    pub optimizer_cache_injection: bool,
    #[serde(default = "default_optimizer_cache_ttl")]
    pub optimizer_cache_ttl: String,
    #[serde(default)]
    pub apps: ProxyAppsSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyAppSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub auto_failover_enabled: bool,
    #[serde(default = "default_proxy_max_retries")]
    pub max_retries: u8,
    #[serde(default = "default_cost_multiplier")]
    pub default_cost_multiplier: String,
    #[serde(default = "default_pricing_model_source")]
    pub pricing_model_source: String,
}

impl Default for ProxyAppSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_failover_enabled: false,
            max_retries: default_proxy_max_retries(),
            default_cost_multiplier: default_cost_multiplier(),
            pricing_model_source: default_pricing_model_source(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProxyAppsSettings {
    #[serde(default)]
    pub claude: ProxyAppSettings,
    #[serde(default)]
    pub codex: ProxyAppSettings,
    #[serde(default)]
    pub gemini: ProxyAppSettings,
    #[serde(default)]
    pub opencode: ProxyAppSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub auto_sync: bool,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_webdav_remote_dir")]
    pub remote_dir: String,
    #[serde(default = "default_webdav_profile")]
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_config_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_remote_snapshot_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSettings {
    #[serde(default)]
    pub github_mirror_base_url: String,
}

impl Default for WebDavSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_sync: false,
            base_url: String::new(),
            username: String::new(),
            password: String::new(),
            remote_dir: default_webdav_remote_dir(),
            profile: default_webdav_profile(),
            last_sync_config_hash: None,
            last_sync_at: None,
            last_sync_remote_snapshot_id: None,
        }
    }
}

fn default_proxy_host() -> String {
    "127.0.0.1".to_string()
}

fn default_proxy_port() -> u16 {
    3456
}

fn default_proxy_bind_app() -> String {
    "claude".to_string()
}

fn default_streaming_first_byte_timeout() -> u64 {
    90
}

fn default_streaming_idle_timeout() -> u64 {
    120
}

fn default_non_streaming_timeout() -> u64 {
    600
}

fn default_proxy_circuit_failure_threshold() -> u64 {
    3
}

fn default_proxy_circuit_recovery_threshold() -> u64 {
    2
}

fn default_proxy_circuit_recovery_wait() -> u64 {
    60
}

fn default_proxy_circuit_error_rate_threshold() -> f64 {
    80.0
}

fn default_rectifier_enabled() -> bool {
    true
}

fn default_optimizer_subfeature_enabled() -> bool {
    true
}

fn default_optimizer_cache_ttl() -> String {
    "1h".to_string()
}

fn default_proxy_max_retries() -> u8 {
    0
}

fn default_cost_multiplier() -> String {
    "1".to_string()
}

fn default_pricing_model_source() -> String {
    "response".to_string()
}

fn default_webdav_remote_dir() -> String {
    "cc-switch-web".to_string()
}

fn default_webdav_profile() -> String {
    "default".to_string()
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            host: default_proxy_host(),
            port: default_proxy_port(),
            upstream_proxy: None,
            bind_app: default_proxy_bind_app(),
            auto_start: false,
            enable_logging: false,
            live_takeover_active: false,
            streaming_first_byte_timeout: default_streaming_first_byte_timeout(),
            streaming_idle_timeout: default_streaming_idle_timeout(),
            non_streaming_timeout: default_non_streaming_timeout(),
            circuit_failure_threshold: default_proxy_circuit_failure_threshold(),
            circuit_recovery_threshold: default_proxy_circuit_recovery_threshold(),
            circuit_recovery_wait_seconds: default_proxy_circuit_recovery_wait(),
            circuit_error_rate_threshold: default_proxy_circuit_error_rate_threshold(),
            rectify_thinking_signature: default_rectifier_enabled(),
            rectify_thinking_budget: default_rectifier_enabled(),
            optimizer_enabled: false,
            optimizer_thinking: default_optimizer_subfeature_enabled(),
            optimizer_cache_injection: default_optimizer_subfeature_enabled(),
            optimizer_cache_ttl: default_optimizer_cache_ttl(),
            apps: ProxyAppsSettings::default(),
        }
    }
}

/// 应用设置结构，允许覆盖默认配置目录
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_show_in_tray")]
    pub show_in_tray: bool,
    #[serde(default = "default_minimize_to_tray_on_close")]
    pub minimize_to_tray_on_close: bool,
    /// 是否启用 Claude 插件联动
    #[serde(default)]
    pub enable_claude_plugin_integration: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini_config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opencode_config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<SecuritySettings>,
    #[serde(default)]
    pub proxy: ProxySettings,
    #[serde(default, rename = "webDav")]
    pub webdav: WebDavSettings,
    #[serde(default)]
    pub network: NetworkSettings,
    /// Skill 同步方式：auto（默认，优先 symlink）、symlink、copy
    #[serde(default)]
    pub skill_sync_method: SyncMethod,
    /// Skill 存储位置：cc_switch（默认）或 unified（~/.agents/skills/）
    #[serde(default)]
    pub skill_storage_location: SkillStorageLocation,
    /// Claude 自定义端点列表
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom_endpoints_claude: HashMap<String, CustomEndpoint>,
    /// Codex 自定义端点列表
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom_endpoints_codex: HashMap<String, CustomEndpoint>,
}

fn default_show_in_tray() -> bool {
    true
}

fn default_minimize_to_tray_on_close() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            show_in_tray: true,
            minimize_to_tray_on_close: true,
            enable_claude_plugin_integration: false,
            claude_config_dir: None,
            codex_config_dir: None,
            gemini_config_dir: None,
            opencode_config_dir: None,
            language: None,
            security: None,
            proxy: ProxySettings::default(),
            webdav: WebDavSettings::default(),
            network: NetworkSettings::default(),
            skill_sync_method: SyncMethod::default(),
            skill_storage_location: SkillStorageLocation::default(),
            custom_endpoints_claude: HashMap::new(),
            custom_endpoints_codex: HashMap::new(),
        }
    }
}

impl AppSettings {
    fn settings_path() -> Result<PathBuf, AppError> {
        // settings.json 必须使用固定路径，不能被 app_config_dir 覆盖
        // 否则会造成循环依赖：读取 settings 需要知道路径，但路径在 settings 中
        let home = get_preferred_home_dir()
            .ok_or_else(|| AppError::Config("无法获取用户主目录".into()))?;
        Ok(settings_path_for_home(&home))
    }

    fn load_from_path(path: &Path) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        match serde_json::from_str::<AppSettings>(&content) {
            Ok(mut settings) => {
                settings.normalize_paths();
                Some(settings)
            }
            Err(err) => {
                log::warn!(
                    "解析设置文件失败，将使用默认设置。路径: {}, 错误: {}",
                    path.display(),
                    err
                );
                None
            }
        }
    }

    fn save_to_path(&self, path: &Path) -> Result<(), AppError> {
        let mut normalized = self.clone();
        normalized.normalize_paths();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
        }

        let json = serde_json::to_string_pretty(&normalized)
            .map_err(|e| AppError::JsonSerialize { source: e })?;
        atomic_write(path, json.as_bytes())?;
        Ok(())
    }

    fn load_with_fallback_paths(path: &Path, legacy_path: Option<&Path>) -> Self {
        if let Some(settings) = Self::load_from_path(path) {
            return settings;
        }

        if !path.exists() {
            if let Some(legacy_path) = legacy_path {
                if let Some(settings) = Self::load_from_path(legacy_path) {
                    if let Err(err) = settings.save_to_path(path) {
                        log::warn!(
                            "迁移旧设置文件失败。来源: {}, 目标: {}, 错误: {}",
                            legacy_path.display(),
                            path.display(),
                            err
                        );
                    } else {
                        log::info!(
                            "已迁移旧设置文件。来源: {}, 目标: {}",
                            legacy_path.display(),
                            path.display()
                        );
                    }
                    return settings;
                }
            }
        }

        Self::default()
    }

    fn normalize_paths(&mut self) {
        self.claude_config_dir = self
            .claude_config_dir
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        self.codex_config_dir = self
            .codex_config_dir
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        self.gemini_config_dir = self
            .gemini_config_dir
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        self.opencode_config_dir = self
            .opencode_config_dir
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        self.language = self
            .language
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| matches!(*s, "en" | "zh"))
            .map(|s| s.to_string());

        self.network.github_mirror_base_url =
            self.network.github_mirror_base_url.trim().to_string();

        self.claude_config_dir =
            migrate_legacy_default_override(self.claude_config_dir.take(), Path::new(".claude"));
        self.codex_config_dir =
            migrate_legacy_default_override(self.codex_config_dir.take(), Path::new(".codex"));
        self.gemini_config_dir =
            migrate_legacy_default_override(self.gemini_config_dir.take(), Path::new(".gemini"));
        let opencode_default_dir = Path::new(".config").join("opencode");
        self.opencode_config_dir = migrate_legacy_default_override(
            self.opencode_config_dir.take(),
            opencode_default_dir.as_path(),
        );
    }

    pub fn load() -> Self {
        let path = match Self::settings_path() {
            Ok(path) => path,
            Err(err) => {
                log::warn!("无法获取设置路径，将使用默认设置: {err}");
                return Self::default();
            }
        };
        let legacy_path = get_legacy_service_settings_path();
        Self::load_with_fallback_paths(&path, legacy_path.as_deref())
    }

    pub fn save(&self) -> Result<(), AppError> {
        let path = Self::settings_path()?;
        self.save_to_path(&path)
    }
}

fn settings_store() -> &'static RwLock<AppSettings> {
    static STORE: OnceLock<RwLock<AppSettings>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(AppSettings::load()))
}

fn resolve_override_path(raw: &str) -> PathBuf {
    resolve_override_path_with_home(raw, get_preferred_home_dir())
}

pub fn get_settings() -> AppSettings {
    settings_store()
        .read()
        .map(|g| g.clone())
        .unwrap_or_else(|e| {
            log::error!("读取设置锁失败，返回默认设置: {e}");
            AppSettings::default()
        })
}

pub fn update_settings(mut new_settings: AppSettings) -> Result<(), AppError> {
    new_settings.normalize_paths();
    new_settings.save()?;

    if let Ok(mut guard) = settings_store().write() {
        *guard = new_settings;
    } else {
        log::error!("写入设置锁失败，内存缓存可能未更新");
    }
    Ok(())
}

pub fn ensure_security_auth_selected_type(selected_type: &str) -> Result<(), AppError> {
    let mut settings = get_settings();
    let current = settings
        .security
        .as_ref()
        .and_then(|sec| sec.auth.as_ref())
        .and_then(|auth| auth.selected_type.as_deref());

    if current == Some(selected_type) {
        return Ok(());
    }

    let mut security = settings.security.unwrap_or_default();
    let mut auth = security.auth.unwrap_or_default();
    auth.selected_type = Some(selected_type.to_string());
    security.auth = Some(auth);
    settings.security = Some(security);

    update_settings(settings)
}

pub fn get_skill_sync_method() -> SyncMethod {
    settings_store()
        .read()
        .unwrap_or_else(|e| {
            log::warn!("设置锁已毒化，使用恢复值: {e}");
            e.into_inner()
        })
        .skill_sync_method
}

pub fn get_skill_storage_location() -> SkillStorageLocation {
    settings_store()
        .read()
        .unwrap_or_else(|e| {
            log::warn!("设置锁已毒化，使用恢复值: {e}");
            e.into_inner()
        })
        .skill_storage_location
}

pub fn set_skill_storage_location(location: SkillStorageLocation) -> Result<(), AppError> {
    let mut settings = get_settings();
    settings.skill_storage_location = location;
    update_settings(settings)
}

pub fn get_claude_override_dir() -> Option<PathBuf> {
    let settings = settings_store().read().ok()?;
    settings
        .claude_config_dir
        .as_ref()
        .map(|p| resolve_override_path(p))
}

pub fn get_codex_override_dir() -> Option<PathBuf> {
    let settings = settings_store().read().ok()?;
    settings
        .codex_config_dir
        .as_ref()
        .map(|p| resolve_override_path(p))
}

pub fn get_gemini_override_dir() -> Option<PathBuf> {
    let settings = settings_store().read().ok()?;
    settings
        .gemini_config_dir
        .as_ref()
        .map(|p| resolve_override_path(p))
}

pub fn get_opencode_override_dir() -> Option<PathBuf> {
    let settings = settings_store().read().ok()?;
    settings
        .opencode_config_dir
        .as_ref()
        .map(|p| resolve_override_path(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use serial_test::serial;
    use std::{env, fs};
    use tempfile::tempdir;

    struct EnvGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = env::var(key).ok();
            env::set_var(key, value);
            Self { key, original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(ref original) = self.original {
                env::set_var(self.key, original);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn prefers_account_home_when_web_server_homes_diverge() {
        let service_home = Some(PathBuf::from("/srv/cc-switch-home"));
        let account_home = Some(PathBuf::from("/root"));

        let preferred = select_preferred_home_dir(service_home, account_home, true)
            .expect("preferred home should resolve");

        assert_eq!(preferred, PathBuf::from("/root"));
        assert_eq!(
            settings_path_for_home(&preferred),
            PathBuf::from("/root/.cc-switch/settings.json")
        );
    }

    #[test]
    fn expands_tilde_using_preferred_home() {
        let resolved =
            resolve_override_path_with_home("~/.codex", Some(PathBuf::from("/home/test-user")));

        assert_eq!(resolved, PathBuf::from("/home/test-user/.codex"));
    }

    #[test]
    fn migrates_legacy_default_override_to_preferred_home() {
        let migrated = migrate_legacy_default_override_with_homes(
            Some("/srv/cc-switch-home/.codex".to_string()),
            Path::new(".codex"),
            Some(PathBuf::from("/srv/cc-switch-home")),
            Some(PathBuf::from("/root")),
            true,
        );

        let expected = PathBuf::from("/root").join(".codex");
        let expected = expected.to_string_lossy().to_string();
        assert_eq!(migrated.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn defaults_missing_rectifier_flags_to_enabled() {
        let loaded: AppSettings = serde_json::from_value(json!({
            "showInTray": true,
            "minimizeToTrayOnClose": true,
            "proxy": {
                "enabled": false,
                "host": "127.0.0.1",
                "port": 3456,
                "bindApp": "claude",
                "autoStart": false,
                "enableLogging": false,
                "liveTakeoverActive": false,
                "apps": {}
            }
        }))
        .expect("settings should deserialize");

        assert!(loaded.proxy.rectify_thinking_signature);
        assert!(loaded.proxy.rectify_thinking_budget);
        assert_eq!(loaded.proxy.non_streaming_timeout, 600);
    }

    #[test]
    fn defaults_and_normalizes_network_settings() {
        let loaded: AppSettings = serde_json::from_value(json!({
            "showInTray": true,
            "minimizeToTrayOnClose": true
        }))
        .expect("settings should deserialize");
        assert!(loaded.network.github_mirror_base_url.is_empty());

        let mut settings = loaded;
        settings.network.github_mirror_base_url = " https://ghproxy.net/ ".to_string();
        settings.normalize_paths();
        assert_eq!(
            settings.network.github_mirror_base_url,
            "https://ghproxy.net/"
        );
    }

    #[test]
    #[serial]
    fn loads_legacy_service_settings_when_preferred_settings_missing() {
        let service_home_dir = tempdir().expect("service home temp dir");
        let account_home_dir = tempdir().expect("account home temp dir");
        let service_home = service_home_dir.path().to_string_lossy().to_string();
        let account_home = account_home_dir.path().to_string_lossy().to_string();
        let legacy_codex_dir = PathBuf::from(&account_home).join(".codex");
        let legacy_codex_dir_str = legacy_codex_dir.to_string_lossy().to_string();

        let _home_guard = EnvGuard::set("HOME", &service_home);

        let legacy_settings_path = PathBuf::from(&service_home)
            .join(".cc-switch")
            .join("settings.json");
        fs::create_dir_all(legacy_settings_path.parent().expect("legacy parent"))
            .expect("create legacy settings dir");
        fs::write(
            &legacy_settings_path,
            serde_json::to_string_pretty(&json!({
                "showInTray": true,
                "minimizeToTrayOnClose": true,
                "codexConfigDir": legacy_codex_dir_str.clone(),
            }))
            .expect("serialize legacy settings"),
        )
        .expect("write legacy settings");

        let primary_settings_path = PathBuf::from(&account_home)
            .join(".cc-switch")
            .join("settings.json");
        let loaded = AppSettings::load_with_fallback_paths(
            &primary_settings_path,
            Some(&legacy_settings_path),
        );

        assert_eq!(
            loaded.codex_config_dir.as_deref(),
            Some(legacy_codex_dir_str.as_str())
        );
        assert!(
            primary_settings_path.exists(),
            "migrated settings should be written"
        );

        let migrated = fs::read_to_string(primary_settings_path).expect("read migrated settings");
        let migrated: serde_json::Value =
            serde_json::from_str(&migrated).expect("parse migrated settings");
        assert!(
            migrated
                .get("codexConfigDir")
                .and_then(|value| value.as_str())
                == Some(legacy_codex_dir_str.as_str()),
            "fallback should preserve legacy settings content while migrating file location"
        );
    }
}
