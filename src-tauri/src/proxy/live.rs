use std::{collections::HashMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    app_config::AppType,
    codex_config::{get_codex_auth_path, get_codex_config_path, write_codex_live_atomic},
    config::{
        delete_file, get_app_config_dir, get_claude_settings_path, read_json_file, write_json_file,
        write_text_file,
    },
    error::AppError,
    gemini_config::{
        get_gemini_env_path, get_gemini_settings_path, read_gemini_env, serialize_env_file,
        write_gemini_env_atomic,
    },
    opencode_config::{get_opencode_config_path, read_opencode_config, write_opencode_config},
    provider::Provider,
    settings::{self, ProxySettings},
    store::AppState,
};

use super::{
    adapters::{codex::normalize_openai_base, gemini::normalize_gemini_base},
    types::PROXY_MANAGED_TOKEN,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProxyBackupStore {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude: Option<ClaudeBackup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex: Option<CodexBackup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini: Option<GeminiBackup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opencode: Option<OpencodeBackup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeBackup {
    pub settings_path: String,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexBackup {
    pub auth_path: String,
    pub config_path: String,
    pub auth: Option<Value>,
    pub config: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiBackup {
    pub env_path: String,
    pub settings_path: String,
    pub env: Option<HashMap<String, String>>,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpencodeBackup {
    pub config_path: String,
    pub config: Option<Value>,
}

pub fn backup_path() -> Result<PathBuf, AppError> {
    Ok(get_app_config_dir()?.join("proxy-backups.json"))
}

pub fn load_backups() -> Result<ProxyBackupStore, AppError> {
    let path = backup_path()?;
    if !path.exists() {
        return Ok(ProxyBackupStore::default());
    }
    read_json_file(&path)
}

pub fn save_backups(backups: &ProxyBackupStore) -> Result<(), AppError> {
    let path = backup_path()?;
    if backups.is_empty() {
        delete_file(&path)?;
    } else {
        write_json_file(&path, backups)?;
    }
    Ok(())
}

impl ProxyBackupStore {
    pub fn is_empty(&self) -> bool {
        self.claude.is_none()
            && self.codex.is_none()
            && self.gemini.is_none()
            && self.opencode.is_none()
    }

    pub fn has_app(&self, app: &AppType) -> bool {
        match app {
            AppType::Claude => self.claude.is_some(),
            AppType::Codex => self.codex.is_some(),
            AppType::Gemini => self.gemini.is_some(),
            AppType::Opencode => self.opencode.is_some(),
            AppType::Omo | AppType::OmoSlim => false,
        }
    }
}

pub fn apply_takeover(
    app: &AppType,
    provider: &Provider,
    listen_url: &str,
) -> Result<(), AppError> {
    let mut backups = load_backups()?;
    match app {
        AppType::Claude => {
            if backups.claude.is_none() {
                backups.claude = Some(capture_claude()?);
            }
            write_claude_takeover(listen_url)?;
        }
        AppType::Codex => {
            if backups.codex.is_none() {
                backups.codex = Some(capture_codex()?);
            }
            write_codex_takeover(listen_url)?;
        }
        AppType::Gemini => {
            if backups.gemini.is_none() {
                backups.gemini = Some(capture_gemini()?);
            }
            write_gemini_takeover(provider, listen_url)?;
        }
        AppType::Opencode => {
            if backups.opencode.is_none() {
                backups.opencode = Some(capture_opencode()?);
            }
            write_opencode_takeover(listen_url)?;
        }
        AppType::Omo | AppType::OmoSlim => {
            return Err(AppError::localized(
                "proxy.omo.unsupported",
                "代理暂不支持 OMO。",
                "Proxy does not support OMO yet.",
            ));
        }
    }
    save_backups(&backups)
}

pub fn sync_current_provider_from_live(state: &AppState, app: &AppType) -> Result<(), AppError> {
    match app {
        AppType::Claude => sync_claude_provider_from_live(state),
        AppType::Codex => sync_codex_provider_from_live(state),
        AppType::Gemini => sync_gemini_provider_from_live(state),
        AppType::Opencode | AppType::Omo | AppType::OmoSlim => Ok(()),
    }
}

pub fn restore_takeover(app: &AppType) -> Result<(), AppError> {
    let mut backups = load_backups()?;
    match app {
        AppType::Claude => {
            if let Some(backup) = backups.claude.take() {
                restore_json_file(&backup.settings_path, backup.settings)?;
            }
        }
        AppType::Codex => {
            if let Some(backup) = backups.codex.take() {
                restore_json_file(&backup.auth_path, backup.auth)?;
                restore_text_file(&backup.config_path, backup.config)?;
            }
        }
        AppType::Gemini => {
            if let Some(backup) = backups.gemini.take() {
                restore_env_file(&backup.env_path, backup.env)?;
                restore_json_file(&backup.settings_path, backup.settings)?;
            }
        }
        AppType::Opencode => {
            if let Some(backup) = backups.opencode.take() {
                restore_json_file(&backup.config_path, backup.config)?;
            }
        }
        AppType::Omo | AppType::OmoSlim => {}
    }
    save_backups(&backups)
}

pub fn restore_all() -> Result<ProxySettings, AppError> {
    for app in [
        AppType::Claude,
        AppType::Codex,
        AppType::Gemini,
        AppType::Opencode,
    ] {
        restore_takeover(&app)?;
    }
    let mut app_settings = settings::get_settings();
    app_settings.proxy.live_takeover_active = false;
    app_settings.proxy.apps.claude.enabled = false;
    app_settings.proxy.apps.codex.enabled = false;
    app_settings.proxy.apps.gemini.enabled = false;
    app_settings.proxy.apps.opencode.enabled = false;
    settings::update_settings(app_settings)?;
    Ok(settings::get_settings().proxy)
}

fn capture_claude() -> Result<ClaudeBackup, AppError> {
    let path = get_claude_settings_path()?;
    let settings = if path.exists() {
        Some(read_json_file(&path)?)
    } else {
        None
    };
    Ok(ClaudeBackup {
        settings_path: path.to_string_lossy().to_string(),
        settings,
    })
}

fn sync_claude_provider_from_live(state: &AppState) -> Result<(), AppError> {
    let path = get_claude_settings_path()?;
    if !path.exists() {
        return Ok(());
    }
    let live: Value = read_json_file(&path)?;
    let Some(env) = live.get("env").and_then(|value| value.as_object()) else {
        return Ok(());
    };
    let raw_token = env
        .get("ANTHROPIC_AUTH_TOKEN")
        .or_else(|| env.get("ANTHROPIC_API_KEY"))
        .and_then(|value| value.as_str());
    if raw_token.is_some_and(is_managed_token) {
        return Ok(());
    }
    let token = raw_token.filter(|value| is_real_token(value));
    let base_url = env
        .get("ANTHROPIC_BASE_URL")
        .and_then(|value| value.as_str());
    if token.is_none() && base_url.is_none() {
        return Ok(());
    }

    state.update_config(|cfg| {
        let Some(manager) = cfg.get_manager_mut(&AppType::Claude) else {
            return Ok(());
        };
        let current = manager.current.clone();
        let Some(provider) = manager.providers.get_mut(&current) else {
            return Ok(());
        };
        if !provider.settings_config.is_object() {
            provider.settings_config = json!({});
        }
        let root = provider
            .settings_config
            .as_object_mut()
            .expect("provider object");
        let env_value = root.entry("env").or_insert_with(|| json!({}));
        if !env_value.is_object() {
            *env_value = json!({});
        }
        let target_env = env_value.as_object_mut().expect("env object");
        if let Some(token) = token {
            target_env.insert("ANTHROPIC_AUTH_TOKEN".to_string(), json!(token));
        }
        if let Some(base_url) = base_url {
            target_env.insert("ANTHROPIC_BASE_URL".to_string(), json!(base_url));
        }
        Ok(())
    })
}

fn sync_codex_provider_from_live(state: &AppState) -> Result<(), AppError> {
    let auth_path = get_codex_auth_path()?;
    let config_path = get_codex_config_path()?;
    if !auth_path.exists() && !config_path.exists() {
        return Ok(());
    }
    let auth = if auth_path.exists() {
        Some(read_json_file::<Value>(&auth_path)?)
    } else {
        None
    };
    let token = auth
        .as_ref()
        .and_then(|value| value.get("OPENAI_API_KEY"))
        .and_then(|value| value.as_str());
    if token.is_some_and(is_managed_token) {
        return Ok(());
    }
    let token = token.filter(|value| is_real_token(value));
    let config_text = if config_path.exists() {
        Some(fs::read_to_string(&config_path).map_err(|e| AppError::io(&config_path, e))?)
    } else {
        None
    };
    if token.is_none() && config_text.is_none() {
        return Ok(());
    }

    state.update_config(|cfg| {
        let Some(manager) = cfg.get_manager_mut(&AppType::Codex) else {
            return Ok(());
        };
        let current = manager.current.clone();
        let Some(provider) = manager.providers.get_mut(&current) else {
            return Ok(());
        };
        if !provider.settings_config.is_object() {
            provider.settings_config = json!({});
        }
        let root = provider
            .settings_config
            .as_object_mut()
            .expect("provider object");
        let auth_value = root.entry("auth").or_insert_with(|| json!({}));
        if !auth_value.is_object() {
            *auth_value = json!({});
        }
        if let Some(token) = token {
            auth_value
                .as_object_mut()
                .expect("auth object")
                .insert("OPENAI_API_KEY".to_string(), json!(token));
        }
        if let Some(config_text) = config_text {
            root.insert("config".to_string(), json!(config_text));
        }
        Ok(())
    })
}

fn sync_gemini_provider_from_live(state: &AppState) -> Result<(), AppError> {
    let env_path = get_gemini_env_path()?;
    if !env_path.exists() {
        return Ok(());
    }
    let env = read_gemini_env()?;
    let raw_token = env.get("GEMINI_API_KEY").map(String::as_str);
    if raw_token.is_some_and(is_managed_token) {
        return Ok(());
    }
    let token = raw_token.filter(|value| is_real_token(value));
    let base_url = env.get("GOOGLE_GEMINI_BASE_URL").map(String::as_str);
    if token.is_none() && base_url.is_none() {
        return Ok(());
    }

    state.update_config(|cfg| {
        let Some(manager) = cfg.get_manager_mut(&AppType::Gemini) else {
            return Ok(());
        };
        let current = manager.current.clone();
        let Some(provider) = manager.providers.get_mut(&current) else {
            return Ok(());
        };
        if !provider.settings_config.is_object() {
            provider.settings_config = json!({});
        }
        let root = provider
            .settings_config
            .as_object_mut()
            .expect("provider object");
        let env_value = root.entry("env").or_insert_with(|| json!({}));
        if !env_value.is_object() {
            *env_value = json!({});
        }
        let target_env = env_value.as_object_mut().expect("env object");
        if let Some(token) = token {
            target_env.insert("GEMINI_API_KEY".to_string(), json!(token));
        }
        if let Some(base_url) = base_url {
            target_env.insert("GOOGLE_GEMINI_BASE_URL".to_string(), json!(base_url));
        }
        Ok(())
    })
}

fn capture_codex() -> Result<CodexBackup, AppError> {
    let auth_path = get_codex_auth_path()?;
    let config_path = get_codex_config_path()?;
    let auth = if auth_path.exists() {
        Some(read_json_file(&auth_path)?)
    } else {
        None
    };
    let config = if config_path.exists() {
        Some(fs::read_to_string(&config_path).map_err(|e| AppError::io(&config_path, e))?)
    } else {
        None
    };
    Ok(CodexBackup {
        auth_path: auth_path.to_string_lossy().to_string(),
        config_path: config_path.to_string_lossy().to_string(),
        auth,
        config,
    })
}

fn capture_gemini() -> Result<GeminiBackup, AppError> {
    let env_path = get_gemini_env_path()?;
    let settings_path = get_gemini_settings_path()?;
    let env = if env_path.exists() {
        Some(read_gemini_env()?)
    } else {
        None
    };
    let settings = if settings_path.exists() {
        Some(read_json_file(&settings_path)?)
    } else {
        None
    };
    Ok(GeminiBackup {
        env_path: env_path.to_string_lossy().to_string(),
        settings_path: settings_path.to_string_lossy().to_string(),
        env,
        settings,
    })
}

fn capture_opencode() -> Result<OpencodeBackup, AppError> {
    let path = get_opencode_config_path();
    let config = if path.exists() {
        Some(read_json_file(&path)?)
    } else {
        None
    };
    Ok(OpencodeBackup {
        config_path: path.to_string_lossy().to_string(),
        config,
    })
}

fn write_claude_takeover(listen_url: &str) -> Result<(), AppError> {
    let path = get_claude_settings_path()?;
    let mut settings = if path.exists() {
        read_json_file::<Value>(&path)?
    } else {
        json!({})
    };
    if !settings.is_object() {
        settings = json!({});
    }
    let obj = settings.as_object_mut().expect("settings object");
    let env_value = obj.entry("env").or_insert_with(|| json!({}));
    if !env_value.is_object() {
        *env_value = json!({});
    }
    let env = env_value.as_object_mut().expect("env object");
    env.insert("ANTHROPIC_BASE_URL".to_string(), json!(listen_url));
    env.insert(
        "ANTHROPIC_AUTH_TOKEN".to_string(),
        json!(PROXY_MANAGED_TOKEN),
    );
    env.remove("ANTHROPIC_API_KEY");
    env.remove("ANTHROPIC_MODEL");
    env.remove("ANTHROPIC_DEFAULT_MODEL");
    env.remove("ANTHROPIC_DEFAULT_OPUS_MODEL");
    env.remove("ANTHROPIC_DEFAULT_HAIKU_MODEL");
    env.remove("ANTHROPIC_DEFAULT_SONNET_MODEL");
    write_json_file(&path, &settings)
}

fn write_codex_takeover(listen_url: &str) -> Result<(), AppError> {
    let auth = json!({ "OPENAI_API_KEY": PROXY_MANAGED_TOKEN });
    let base = normalize_openai_base(listen_url);
    let config = format!(
        r#"model_provider = "cc-switch-proxy"

[model_providers.cc-switch-proxy]
name = "cc-switch-proxy"
base_url = "{base}"
wire_api = "responses"
requires_openai_auth = true
"#
    );
    write_codex_live_atomic(&auth, Some(&config))
}

fn write_gemini_takeover(provider: &Provider, listen_url: &str) -> Result<(), AppError> {
    let mut env = crate::gemini_config::json_to_env(&provider.settings_config)?;
    env.insert(
        "GEMINI_API_KEY".to_string(),
        PROXY_MANAGED_TOKEN.to_string(),
    );
    env.insert(
        "GOOGLE_GEMINI_BASE_URL".to_string(),
        normalize_gemini_base(listen_url),
    );
    write_gemini_env_atomic(&env)
}

fn write_opencode_takeover(listen_url: &str) -> Result<(), AppError> {
    let mut config = read_opencode_config()?;
    if !config.is_object() {
        config = json!({});
    }
    let obj = config.as_object_mut().expect("opencode object");
    let provider = obj.entry("provider").or_insert_with(|| json!({}));
    if !provider.is_object() {
        *provider = json!({});
    }
    provider.as_object_mut().expect("provider object").insert(
        "cc-switch-proxy".to_string(),
        json!({
            "npm": "@ai-sdk/openai-compatible",
            "name": "cc-switch-proxy",
            "options": {
                "baseURL": normalize_openai_base(listen_url),
                "apiKey": PROXY_MANAGED_TOKEN
            }
        }),
    );
    write_opencode_config(&config)
}

fn restore_json_file(path: &str, value: Option<Value>) -> Result<(), AppError> {
    let path = PathBuf::from(path);
    if let Some(value) = value {
        write_json_file(&path, &value)
    } else {
        delete_file(&path)
    }
}

fn restore_text_file(path: &str, value: Option<String>) -> Result<(), AppError> {
    let path = PathBuf::from(path);
    if let Some(value) = value {
        write_text_file(&path, &value)
    } else {
        delete_file(&path)
    }
}

fn restore_env_file(path: &str, value: Option<HashMap<String, String>>) -> Result<(), AppError> {
    let path = PathBuf::from(path);
    if let Some(value) = value {
        write_text_file(&path, &serialize_env_file(&value))
    } else {
        delete_file(&path)
    }
}

fn is_real_token(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && trimmed != PROXY_MANAGED_TOKEN
}

fn is_managed_token(value: &str) -> bool {
    value.trim() == PROXY_MANAGED_TOKEN
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, env, ffi::OsString, path::Path};

    use serde_json::{json, Value};
    use serial_test::serial;
    use tempfile::{tempdir, TempDir};

    use crate::{
        app_config::{AppType, MultiAppConfig},
        codex_config::{get_codex_auth_path, get_codex_config_path},
        config::{get_claude_settings_path, read_json_file, write_json_file, write_text_file},
        gemini_config::{get_gemini_env_path, get_gemini_settings_path, read_gemini_env},
        opencode_config::get_opencode_config_path,
        provider::Provider,
        settings::{self, AppSettings},
        store::AppState,
    };

    use super::{
        apply_takeover, backup_path, restore_all, restore_takeover,
        sync_current_provider_from_live, write_claude_takeover, write_codex_takeover,
        PROXY_MANAGED_TOKEN,
    };

    struct EnvGuard {
        home: Option<OsString>,
        userprofile: Option<OsString>,
        user: Option<OsString>,
        logname: Option<OsString>,
    }

    impl EnvGuard {
        fn isolated(home: &Path) -> Self {
            let guard = Self {
                home: env::var_os("HOME"),
                userprofile: env::var_os("USERPROFILE"),
                user: env::var_os("USER"),
                logname: env::var_os("LOGNAME"),
            };
            env::set_var("HOME", home);
            env::set_var("USERPROFILE", home);
            env::set_var("USER", "");
            env::set_var("LOGNAME", "");
            guard
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            restore_env("HOME", self.home.take());
            restore_env("USERPROFILE", self.userprofile.take());
            restore_env("USER", self.user.take());
            restore_env("LOGNAME", self.logname.take());
        }
    }

    fn restore_env(key: &str, value: Option<OsString>) {
        if let Some(value) = value {
            env::set_var(key, value);
        } else {
            env::remove_var(key);
        }
    }

    fn isolated_settings() -> (TempDir, EnvGuard) {
        let temp = tempdir().expect("tempdir");
        let guard = EnvGuard::isolated(temp.path());
        let mut app_settings = AppSettings::default();
        app_settings.claude_config_dir =
            Some(temp.path().join(".claude").to_string_lossy().to_string());
        app_settings.codex_config_dir =
            Some(temp.path().join(".codex").to_string_lossy().to_string());
        app_settings.gemini_config_dir =
            Some(temp.path().join(".gemini").to_string_lossy().to_string());
        app_settings.opencode_config_dir = Some(
            temp.path()
                .join(".config")
                .join("opencode")
                .to_string_lossy()
                .to_string(),
        );
        settings::update_settings(app_settings).expect("update settings");
        (temp, guard)
    }

    fn dummy_provider() -> Provider {
        Provider::with_id(
            "provider-1".to_string(),
            "Provider 1".to_string(),
            json!({
                "env": {
                    "GEMINI_API_KEY": "provider-gemini-key"
                }
            }),
            None,
        )
    }

    fn state_with_providers() -> AppState {
        let mut config = MultiAppConfig::default();
        insert_provider(
            &mut config,
            AppType::Claude,
            json!({
                "env": {
                    "ANTHROPIC_AUTH_TOKEN": "provider-claude-key",
                    "ANTHROPIC_BASE_URL": "https://provider-claude.example"
                }
            }),
        );
        insert_provider(
            &mut config,
            AppType::Codex,
            json!({
                "auth": {
                    "OPENAI_API_KEY": "provider-openai-key"
                },
                "config": "model_provider = \"provider\"\n"
            }),
        );
        insert_provider(
            &mut config,
            AppType::Gemini,
            json!({
                "env": {
                    "GEMINI_API_KEY": "provider-gemini-key",
                    "GOOGLE_GEMINI_BASE_URL": "https://provider-gemini.example"
                }
            }),
        );
        AppState::new_for_tests(config).expect("test app state")
    }

    fn insert_provider(config: &mut MultiAppConfig, app: AppType, settings_config: Value) {
        let provider = Provider::with_id(
            "current".to_string(),
            "Current".to_string(),
            settings_config,
            None,
        );
        let manager = config.get_manager_mut(&app).expect("manager exists");
        manager.current = provider.id.clone();
        manager.providers.insert(provider.id.clone(), provider);
    }

    #[test]
    #[serial]
    fn claude_takeover_preserves_env_and_removes_model_overrides() {
        let (_temp, _env) = isolated_settings();

        let path = get_claude_settings_path().expect("claude settings path");
        write_json_file(
            &path,
            &json!({
                "env": {
                    "ANTHROPIC_AUTH_TOKEN": "real-token",
                    "ANTHROPIC_MODEL": "old-model",
                    "KEEP_ME": "yes"
                },
                "permissions": { "allow": ["Bash(ls)"] }
            }),
        )
        .expect("write initial claude settings");

        write_claude_takeover("http://127.0.0.1:3456").expect("write takeover");

        let value: Value = read_json_file(&path).expect("read claude settings");
        assert_eq!(
            value["env"]["ANTHROPIC_BASE_URL"],
            json!("http://127.0.0.1:3456")
        );
        assert_eq!(
            value["env"]["ANTHROPIC_AUTH_TOKEN"],
            json!(PROXY_MANAGED_TOKEN)
        );
        assert_eq!(value["env"]["KEEP_ME"], json!("yes"));
        assert!(value["env"].get("ANTHROPIC_MODEL").is_none());
        assert_eq!(value["permissions"]["allow"][0], json!("Bash(ls)"));
    }

    #[test]
    #[serial]
    fn codex_takeover_writes_placeholder_auth_and_single_v1_base_url() {
        let (_temp, _env) = isolated_settings();

        write_codex_takeover("http://127.0.0.1:3456").expect("write codex takeover");

        let auth: Value =
            read_json_file(&get_codex_auth_path().expect("auth path")).expect("read codex auth");
        let config = std::fs::read_to_string(get_codex_config_path().expect("config path"))
            .expect("read codex config");

        assert_eq!(auth["OPENAI_API_KEY"], json!(PROXY_MANAGED_TOKEN));
        assert!(config.contains(r#"base_url = "http://127.0.0.1:3456/v1""#));
        assert!(!config.contains("/v1/v1"));
    }

    #[test]
    #[serial]
    fn claude_restore_restores_existing_settings_and_removes_backup() {
        let (_temp, _env) = isolated_settings();
        let provider = dummy_provider();
        let path = get_claude_settings_path().expect("claude settings path");
        let original = json!({
            "env": {
                "ANTHROPIC_AUTH_TOKEN": "real-token",
                "ANTHROPIC_BASE_URL": "https://anthropic.example"
            },
            "permissions": { "allow": ["Bash(ls)"] }
        });
        write_json_file(&path, &original).expect("write original claude settings");

        apply_takeover(&AppType::Claude, &provider, "http://127.0.0.1:3456")
            .expect("apply claude takeover");
        assert_eq!(
            read_json_file::<Value>(&path).expect("read takeover")["env"]["ANTHROPIC_AUTH_TOKEN"],
            json!(PROXY_MANAGED_TOKEN)
        );

        restore_takeover(&AppType::Claude).expect("restore claude takeover");

        assert_eq!(
            read_json_file::<Value>(&path).expect("read restored claude"),
            original
        );
        assert!(!backup_path().expect("backup path").exists());
    }

    #[test]
    #[serial]
    fn restore_deletes_files_created_by_takeover_when_no_original_existed() {
        let (_temp, _env) = isolated_settings();
        let provider = dummy_provider();

        let claude_path = get_claude_settings_path().expect("claude settings path");
        apply_takeover(&AppType::Claude, &provider, "http://127.0.0.1:3456")
            .expect("apply claude takeover");
        assert!(claude_path.exists());
        restore_takeover(&AppType::Claude).expect("restore claude takeover");
        assert!(!claude_path.exists());

        let auth_path = get_codex_auth_path().expect("codex auth path");
        let config_path = get_codex_config_path().expect("codex config path");
        apply_takeover(&AppType::Codex, &provider, "http://127.0.0.1:3456")
            .expect("apply codex takeover");
        assert!(auth_path.exists());
        assert!(config_path.exists());
        restore_takeover(&AppType::Codex).expect("restore codex takeover");
        assert!(!auth_path.exists());
        assert!(!config_path.exists());
    }

    #[test]
    #[serial]
    fn codex_gemini_and_opencode_restore_original_files() {
        let (_temp, _env) = isolated_settings();
        let provider = dummy_provider();

        let codex_auth_path = get_codex_auth_path().expect("codex auth path");
        let codex_config_path = get_codex_config_path().expect("codex config path");
        let original_auth = json!({ "OPENAI_API_KEY": "real-openai-key" });
        let original_config = "model_provider = \"real\"\n";
        write_json_file(&codex_auth_path, &original_auth).expect("write codex auth");
        write_text_file(&codex_config_path, original_config).expect("write codex config");
        apply_takeover(&AppType::Codex, &provider, "http://127.0.0.1:3456")
            .expect("apply codex takeover");
        restore_takeover(&AppType::Codex).expect("restore codex takeover");
        assert_eq!(
            read_json_file::<Value>(&codex_auth_path).expect("read codex auth"),
            original_auth
        );
        assert_eq!(
            std::fs::read_to_string(&codex_config_path).expect("read codex config"),
            original_config
        );

        let gemini_env_path = get_gemini_env_path().expect("gemini env path");
        let gemini_settings_path = get_gemini_settings_path().expect("gemini settings path");
        let original_env = HashMap::from([
            ("GEMINI_API_KEY".to_string(), "real-gemini-key".to_string()),
            (
                "GOOGLE_GEMINI_BASE_URL".to_string(),
                "https://gemini.example".to_string(),
            ),
        ]);
        let original_gemini_settings = json!({ "selectedAuthType": "api-key" });
        write_text_file(
            &gemini_env_path,
            &crate::gemini_config::serialize_env_file(&original_env),
        )
        .expect("write gemini env");
        write_json_file(&gemini_settings_path, &original_gemini_settings)
            .expect("write gemini settings");
        apply_takeover(&AppType::Gemini, &provider, "http://127.0.0.1:3456")
            .expect("apply gemini takeover");
        restore_takeover(&AppType::Gemini).expect("restore gemini takeover");
        assert_eq!(read_gemini_env().expect("read gemini env"), original_env);
        assert_eq!(
            read_json_file::<Value>(&gemini_settings_path).expect("read gemini settings"),
            original_gemini_settings
        );

        let opencode_path = get_opencode_config_path();
        let original_opencode = json!({
            "$schema": "https://opencode.ai/config.json",
            "provider": {
                "existing": {
                    "name": "existing"
                }
            }
        });
        write_json_file(&opencode_path, &original_opencode).expect("write opencode config");
        apply_takeover(&AppType::Opencode, &provider, "http://127.0.0.1:3456")
            .expect("apply opencode takeover");
        restore_takeover(&AppType::Opencode).expect("restore opencode takeover");
        assert_eq!(
            read_json_file::<Value>(&opencode_path).expect("read opencode config"),
            original_opencode
        );
    }

    #[test]
    #[serial]
    fn backup_store_merges_apps_and_restore_all_deletes_backup_file() {
        let (_temp, _env) = isolated_settings();
        let provider = dummy_provider();

        apply_takeover(&AppType::Claude, &provider, "http://127.0.0.1:3456")
            .expect("apply claude takeover");
        apply_takeover(&AppType::Codex, &provider, "http://127.0.0.1:3456")
            .expect("apply codex takeover");

        let backup_file = backup_path().expect("backup path");
        let backups: Value = read_json_file(&backup_file).expect("read merged backups");
        assert!(backups.get("claude").is_some());
        assert!(backups.get("codex").is_some());

        restore_takeover(&AppType::Claude).expect("restore only claude");
        let backups: Value = read_json_file(&backup_file).expect("read partial backups");
        assert!(backups.get("claude").is_none());
        assert!(backups.get("codex").is_some());

        restore_all().expect("restore all");
        assert!(!backup_file.exists());
    }

    #[test]
    #[serial]
    fn sync_current_provider_from_live_copies_real_tokens_to_current_provider() {
        let (_temp, _env) = isolated_settings();
        let state = state_with_providers();

        write_json_file(
            &get_claude_settings_path().expect("claude settings path"),
            &json!({
                "env": {
                    "ANTHROPIC_AUTH_TOKEN": "live-claude-key",
                    "ANTHROPIC_BASE_URL": "https://live-claude.example"
                }
            }),
        )
        .expect("write claude live config");
        write_json_file(
            &get_codex_auth_path().expect("codex auth path"),
            &json!({ "OPENAI_API_KEY": "live-openai-key" }),
        )
        .expect("write codex auth");
        write_text_file(
            &get_codex_config_path().expect("codex config path"),
            "model_provider = \"live\"\n",
        )
        .expect("write codex config");
        write_text_file(
            &get_gemini_env_path().expect("gemini env path"),
            "GEMINI_API_KEY=live-gemini-key\nGOOGLE_GEMINI_BASE_URL=https://live-gemini.example",
        )
        .expect("write gemini env");

        sync_current_provider_from_live(&state, &AppType::Claude).expect("sync claude");
        sync_current_provider_from_live(&state, &AppType::Codex).expect("sync codex");
        sync_current_provider_from_live(&state, &AppType::Gemini).expect("sync gemini");

        let guard = state.load_config().expect("read state");
        let claude = &guard
            .get_manager(&AppType::Claude)
            .expect("claude manager")
            .providers["current"]
            .settings_config;
        assert_eq!(
            claude["env"]["ANTHROPIC_AUTH_TOKEN"],
            json!("live-claude-key")
        );
        assert_eq!(
            claude["env"]["ANTHROPIC_BASE_URL"],
            json!("https://live-claude.example")
        );
        let codex = &guard
            .get_manager(&AppType::Codex)
            .expect("codex manager")
            .providers["current"]
            .settings_config;
        assert_eq!(codex["auth"]["OPENAI_API_KEY"], json!("live-openai-key"));
        assert_eq!(codex["config"], json!("model_provider = \"live\"\n"));
        let gemini = &guard
            .get_manager(&AppType::Gemini)
            .expect("gemini manager")
            .providers["current"]
            .settings_config;
        assert_eq!(gemini["env"]["GEMINI_API_KEY"], json!("live-gemini-key"));
        assert_eq!(
            gemini["env"]["GOOGLE_GEMINI_BASE_URL"],
            json!("https://live-gemini.example")
        );
    }

    #[test]
    #[serial]
    fn sync_current_provider_from_live_skips_proxy_managed_configs() {
        let (_temp, _env) = isolated_settings();
        let state = state_with_providers();

        write_json_file(
            &get_claude_settings_path().expect("claude settings path"),
            &json!({
                "env": {
                    "ANTHROPIC_AUTH_TOKEN": PROXY_MANAGED_TOKEN,
                    "ANTHROPIC_BASE_URL": "http://127.0.0.1:3456"
                }
            }),
        )
        .expect("write claude managed live config");
        write_json_file(
            &get_codex_auth_path().expect("codex auth path"),
            &json!({ "OPENAI_API_KEY": PROXY_MANAGED_TOKEN }),
        )
        .expect("write codex managed auth");
        write_text_file(
            &get_codex_config_path().expect("codex config path"),
            "model_provider = \"cc-switch-proxy\"\n",
        )
        .expect("write codex managed config");
        write_text_file(
            &get_gemini_env_path().expect("gemini env path"),
            "GEMINI_API_KEY=PROXY_MANAGED\nGOOGLE_GEMINI_BASE_URL=http://127.0.0.1:3456/v1beta",
        )
        .expect("write gemini managed env");

        sync_current_provider_from_live(&state, &AppType::Claude).expect("sync claude");
        sync_current_provider_from_live(&state, &AppType::Codex).expect("sync codex");
        sync_current_provider_from_live(&state, &AppType::Gemini).expect("sync gemini");

        let guard = state.load_config().expect("read state");
        let claude = &guard
            .get_manager(&AppType::Claude)
            .expect("claude manager")
            .providers["current"]
            .settings_config;
        assert_eq!(
            claude["env"]["ANTHROPIC_AUTH_TOKEN"],
            json!("provider-claude-key")
        );
        assert_eq!(
            claude["env"]["ANTHROPIC_BASE_URL"],
            json!("https://provider-claude.example")
        );
        let codex = &guard
            .get_manager(&AppType::Codex)
            .expect("codex manager")
            .providers["current"]
            .settings_config;
        assert_eq!(
            codex["auth"]["OPENAI_API_KEY"],
            json!("provider-openai-key")
        );
        assert_eq!(codex["config"], json!("model_provider = \"provider\"\n"));
        let gemini = &guard
            .get_manager(&AppType::Gemini)
            .expect("gemini manager")
            .providers["current"]
            .settings_config;
        assert_eq!(
            gemini["env"]["GEMINI_API_KEY"],
            json!("provider-gemini-key")
        );
        assert_eq!(
            gemini["env"]["GOOGLE_GEMINI_BASE_URL"],
            json!("https://provider-gemini.example")
        );
    }
}
