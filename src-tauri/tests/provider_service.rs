use serde_json::json;
use std::{path::PathBuf, sync::RwLock};

use cc_switch_lib::{
    get_claude_settings_path, get_codex_config_path, read_json_file, write_codex_live_atomic,
    AppError, AppState, AppType, MultiAppConfig, Provider, ProviderMeta, ProviderService,
};

#[path = "support.rs"]
mod support;
use support::{ensure_test_home, reset_test_fs, test_mutex};

fn unwrap_path(result: Result<PathBuf, AppError>) -> PathBuf {
    result.expect("path should resolve")
}

fn sanitize_provider_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            _ => c,
        })
        .collect::<String>()
        .to_lowercase()
}

#[test]
fn provider_service_switch_codex_updates_live_and_config() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let legacy_auth = json!({ "OPENAI_API_KEY": "legacy-key" });
    let legacy_config = r#"[mcp_servers.legacy]
type = "stdio"
command = "echo"
"#;
    write_codex_live_atomic(&legacy_auth, Some(legacy_config))
        .expect("seed existing codex live config");

    let mut initial_config = MultiAppConfig::default();
    {
        let manager = initial_config
            .get_manager_mut(&AppType::Codex)
            .expect("codex manager");
        manager.current = "old-provider".to_string();
        manager.providers.insert(
            "old-provider".to_string(),
            Provider::with_id(
                "old-provider".to_string(),
                "Legacy".to_string(),
                json!({
                    "auth": {"OPENAI_API_KEY": "stale"},
                    "config": "stale-config"
                }),
                None,
            ),
        );
        manager.providers.insert(
            "new-provider".to_string(),
            Provider::with_id(
                "new-provider".to_string(),
                "Latest".to_string(),
                json!({
                    "auth": {"OPENAI_API_KEY": "fresh-key"},
                    "config": r#"[mcp_servers.latest]
type = "stdio"
command = "say"
"#
                }),
                None,
            ),
        );
    }

    initial_config.mcp.codex.servers.insert(
        "echo-server".into(),
        json!({
            "id": "echo-server",
            "enabled": true,
            "server": {
                "type": "stdio",
                "command": "echo"
            }
        }),
    );

    let state = AppState {
        config: RwLock::new(initial_config),
    };

    ProviderService::switch(&state, AppType::Codex, "new-provider")
        .expect("switch provider should succeed");

    let auth_path = unwrap_path(cc_switch_lib::get_codex_auth_path());
    let auth_value: serde_json::Value = read_json_file(&auth_path).expect("read auth.json");
    assert_eq!(
        auth_value.get("OPENAI_API_KEY").and_then(|v| v.as_str()),
        Some("fresh-key"),
        "live auth.json should reflect new provider"
    );

    let config_path = unwrap_path(cc_switch_lib::get_codex_config_path());
    let config_text = std::fs::read_to_string(&config_path).expect("read config.toml");
    assert!(
        config_text.contains("mcp_servers.echo-server"),
        "config.toml should contain synced MCP servers"
    );

    let guard = state.config.read().expect("read config after switch");
    let manager = guard
        .get_manager(&AppType::Codex)
        .expect("codex manager after switch");
    assert_eq!(manager.current, "new-provider", "current provider updated");

    let new_provider = manager
        .providers
        .get("new-provider")
        .expect("new provider exists");
    let new_config_text = new_provider
        .settings_config
        .get("config")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert_eq!(
        new_config_text, config_text,
        "provider config snapshot should match live file"
    );

    let legacy = manager
        .providers
        .get("old-provider")
        .expect("legacy provider still exists");
    let legacy_auth_value = legacy
        .settings_config
        .get("auth")
        .and_then(|v| v.get("OPENAI_API_KEY"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(
        legacy_auth_value, "legacy-key",
        "previous provider should be backfilled with live auth"
    );
}

#[test]
fn switch_packycode_gemini_updates_security_selected_type() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Gemini)
            .expect("gemini manager");
        manager.current = "packy-gemini".to_string();
        manager.providers.insert(
            "packy-gemini".to_string(),
            Provider::with_id(
                "packy-gemini".to_string(),
                "PackyCode".to_string(),
                json!({
                    "env": {
                        "GEMINI_API_KEY": "pk-key",
                        "GOOGLE_GEMINI_BASE_URL": "https://www.packyapi.com"
                    }
                }),
                Some("https://www.packyapi.com".to_string()),
            ),
        );
    }

    let state = AppState {
        config: RwLock::new(config),
    };

    ProviderService::switch(&state, AppType::Gemini, "packy-gemini")
        .expect("switching to PackyCode Gemini should succeed");

    let settings_path = home.join(".cc-switch").join("settings.json");
    assert!(
        settings_path.exists(),
        "settings.json should exist at {}",
        settings_path.display()
    );
    let raw = std::fs::read_to_string(&settings_path).expect("read settings.json");
    let value: serde_json::Value =
        serde_json::from_str(&raw).expect("parse settings.json after switch");

    assert_eq!(
        value
            .pointer("/security/auth/selectedType")
            .and_then(|v| v.as_str()),
        Some("gemini-api-key"),
        "PackyCode Gemini should set security.auth.selectedType"
    );
}

#[test]
fn packycode_partner_meta_triggers_security_flag_even_without_keywords() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Gemini)
            .expect("gemini manager");
        manager.current = "packy-meta".to_string();
        let mut provider = Provider::with_id(
            "packy-meta".to_string(),
            "Generic Gemini".to_string(),
            json!({
                "env": {
                    "GEMINI_API_KEY": "pk-meta",
                    "GOOGLE_GEMINI_BASE_URL": "https://generativelanguage.googleapis.com"
                }
            }),
            Some("https://example.com".to_string()),
        );
        provider.meta = Some(ProviderMeta {
            partner_promotion_key: Some("packycode".to_string()),
            ..ProviderMeta::default()
        });
        manager.providers.insert("packy-meta".to_string(), provider);
    }

    let state = AppState {
        config: RwLock::new(config),
    };

    ProviderService::switch(&state, AppType::Gemini, "packy-meta")
        .expect("switching to partner meta provider should succeed");

    let settings_path = home.join(".cc-switch").join("settings.json");
    assert!(
        settings_path.exists(),
        "settings.json should exist at {}",
        settings_path.display()
    );
    let raw = std::fs::read_to_string(&settings_path).expect("read settings.json");
    let value: serde_json::Value =
        serde_json::from_str(&raw).expect("parse settings.json after switch");

    assert_eq!(
        value
            .pointer("/security/auth/selectedType")
            .and_then(|v| v.as_str()),
        Some("gemini-api-key"),
        "Partner meta should set security.auth.selectedType even without packy keywords"
    );
}

#[test]
fn switch_google_official_gemini_sets_oauth_security() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Gemini)
            .expect("gemini manager");
        manager.current = "google-official".to_string();
        let mut provider = Provider::with_id(
            "google-official".to_string(),
            "Google".to_string(),
            json!({
                "env": {}
            }),
            Some("https://ai.google.dev".to_string()),
        );
        provider.meta = Some(ProviderMeta {
            partner_promotion_key: Some("google-official".to_string()),
            ..ProviderMeta::default()
        });
        manager
            .providers
            .insert("google-official".to_string(), provider);
    }

    let state = AppState {
        config: RwLock::new(config),
    };

    ProviderService::switch(&state, AppType::Gemini, "google-official")
        .expect("switching to Google official Gemini should succeed");

    let settings_path = home.join(".cc-switch").join("settings.json");
    assert!(
        settings_path.exists(),
        "settings.json should exist at {}",
        settings_path.display()
    );

    let raw = std::fs::read_to_string(&settings_path).expect("read settings.json");
    let value: serde_json::Value = serde_json::from_str(&raw).expect("parse settings.json");
    assert_eq!(
        value
            .pointer("/security/auth/selectedType")
            .and_then(|v| v.as_str()),
        Some("oauth-personal"),
        "Google official Gemini should set oauth-personal selectedType in app settings"
    );

    let gemini_settings = home.join(".gemini").join("settings.json");
    assert!(
        gemini_settings.exists(),
        "Gemini settings.json should exist at {}",
        gemini_settings.display()
    );
    let gemini_raw = std::fs::read_to_string(&gemini_settings).expect("read gemini settings");
    let gemini_value: serde_json::Value =
        serde_json::from_str(&gemini_raw).expect("parse gemini settings");

    assert_eq!(
        gemini_value
            .pointer("/security/auth/selectedType")
            .and_then(|v| v.as_str()),
        Some("oauth-personal"),
        "Gemini settings json should also reflect oauth-personal"
    );
}

#[test]
fn provider_service_switch_claude_updates_live_and_state() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let settings_path = unwrap_path(get_claude_settings_path());
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent).expect("create claude settings dir");
    }
    let legacy_live = json!({
        "env": {
            "ANTHROPIC_API_KEY": "legacy-key"
        },
        "workspace": {
            "path": "/tmp/workspace"
        }
    });
    std::fs::write(
        &settings_path,
        serde_json::to_string_pretty(&legacy_live).expect("serialize legacy live"),
    )
    .expect("seed claude live config");

    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Claude)
            .expect("claude manager");
        manager.current = "old-provider".to_string();
        manager.providers.insert(
            "old-provider".to_string(),
            Provider::with_id(
                "old-provider".to_string(),
                "Legacy Claude".to_string(),
                json!({
                    "env": { "ANTHROPIC_API_KEY": "stale-key" }
                }),
                None,
            ),
        );
        manager.providers.insert(
            "new-provider".to_string(),
            Provider::with_id(
                "new-provider".to_string(),
                "Fresh Claude".to_string(),
                json!({
                    "env": { "ANTHROPIC_API_KEY": "fresh-key" },
                    "workspace": { "path": "/tmp/new-workspace" }
                }),
                None,
            ),
        );
    }

    let state = AppState {
        config: RwLock::new(config),
    };

    ProviderService::switch(&state, AppType::Claude, "new-provider")
        .expect("switch provider should succeed");

    let live_after: serde_json::Value =
        read_json_file(&settings_path).expect("read claude live settings");
    assert_eq!(
        live_after
            .get("env")
            .and_then(|env| env.get("ANTHROPIC_API_KEY"))
            .and_then(|key| key.as_str()),
        Some("fresh-key"),
        "live settings.json should reflect new provider auth"
    );

    let guard = state
        .config
        .read()
        .expect("read claude config after switch");
    let manager = guard
        .get_manager(&AppType::Claude)
        .expect("claude manager after switch");
    assert_eq!(manager.current, "new-provider", "current provider updated");

    let legacy_provider = manager
        .providers
        .get("old-provider")
        .expect("legacy provider still exists");
    assert_eq!(
        legacy_provider.settings_config, legacy_live,
        "previous provider should receive backfilled live config"
    );
}

#[test]
fn sync_default_provider_from_live_preserves_current_and_category() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Claude)
            .expect("claude manager");
        manager.current = "current-provider".to_string();
        manager.providers.insert(
            "current-provider".to_string(),
            Provider::with_id(
                "current-provider".to_string(),
                "Current".to_string(),
                json!({ "env": { "ANTHROPIC_API_KEY": "current-key" } }),
                None,
            ),
        );
        let mut default_provider = Provider::with_id(
            "default".to_string(),
            "Default".to_string(),
            json!({ "model": "old-model" }),
            None,
        );
        default_provider.category = Some("official".to_string());
        manager
            .providers
            .insert("default".to_string(), default_provider);
    }

    let state = AppState {
        config: RwLock::new(config),
    };

    ProviderService::sync_default_provider_from_live(
        &state,
        AppType::Claude,
        json!({ "model": "claude-3" }),
    )
    .expect("sync default provider from live should succeed");

    let guard = state.config.read().expect("read config after sync default");
    let manager = guard
        .get_manager(&AppType::Claude)
        .expect("claude manager after sync");
    assert_eq!(
        manager.current, "current-provider",
        "current provider should remain unchanged"
    );
    let default_provider = manager
        .providers
        .get("default")
        .expect("default provider should exist");
    assert_eq!(
        default_provider.settings_config,
        json!({ "model": "claude-3" }),
        "default provider settings should be updated from live"
    );
    assert_eq!(
        default_provider.category.as_deref(),
        Some("official"),
        "default provider category should be preserved"
    );
}

#[test]
fn sync_default_provider_from_live_creates_default_when_missing() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Claude)
            .expect("claude manager");
        manager.current = "current-provider".to_string();
        manager.providers.insert(
            "current-provider".to_string(),
            Provider::with_id(
                "current-provider".to_string(),
                "Current".to_string(),
                json!({ "env": { "ANTHROPIC_API_KEY": "current-key" } }),
                None,
            ),
        );
    }

    let state = AppState {
        config: RwLock::new(config),
    };

    ProviderService::sync_default_provider_from_live(
        &state,
        AppType::Claude,
        json!({ "model": "claude-3" }),
    )
    .expect("sync default provider from live should succeed");

    let guard = state.config.read().expect("read config after sync default");
    let manager = guard
        .get_manager(&AppType::Claude)
        .expect("claude manager after sync");
    assert_eq!(
        manager.current, "current-provider",
        "current provider should remain unchanged"
    );
    let default_provider = manager
        .providers
        .get("default")
        .expect("default provider should be created");
    assert_eq!(
        default_provider.category.as_deref(),
        Some("custom"),
        "default provider should be created as custom"
    );
    assert_eq!(
        default_provider
            .settings_config
            .get("model")
            .and_then(|v| v.as_str()),
        Some("claude-3"),
        "default provider settings should include model from live"
    );
}

#[test]
fn provider_service_switch_missing_provider_returns_error() {
    let state = AppState {
        config: RwLock::new(MultiAppConfig::default()),
    };

    let err = ProviderService::switch(&state, AppType::Claude, "missing")
        .expect_err("switching missing provider should fail");
    match err {
        AppError::Localized { key, .. } => assert_eq!(key, "provider.not_found"),
        other => panic!("expected Localized error for provider not found, got {other:?}"),
    }
}

#[test]
fn provider_service_switch_codex_missing_auth_returns_error() {
    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Codex)
            .expect("codex manager");
        manager.providers.insert(
            "invalid".to_string(),
            Provider::with_id(
                "invalid".to_string(),
                "Broken Codex".to_string(),
                json!({
                    "config": "[mcp_servers.test]\ncommand = \"noop\""
                }),
                None,
            ),
        );
    }

    let state = AppState {
        config: RwLock::new(config),
    };

    let err = ProviderService::switch(&state, AppType::Codex, "invalid")
        .expect_err("switching should fail without auth");
    match err {
        AppError::Config(msg) => assert!(
            msg.contains("auth"),
            "expected auth related message, got {msg}"
        ),
        other => panic!("expected config error, got {other:?}"),
    }
}

#[test]
fn provider_service_delete_codex_removes_provider_and_files() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Codex)
            .expect("codex manager");
        manager.current = "keep".to_string();
        manager.providers.insert(
            "keep".to_string(),
            Provider::with_id(
                "keep".to_string(),
                "Keep".to_string(),
                json!({
                    "auth": {"OPENAI_API_KEY": "keep-key"},
                    "config": ""
                }),
                None,
            ),
        );
        manager.providers.insert(
            "to-delete".to_string(),
            Provider::with_id(
                "to-delete".to_string(),
                "DeleteCodex".to_string(),
                json!({
                    "auth": {"OPENAI_API_KEY": "delete-key"},
                    "config": ""
                }),
                None,
            ),
        );
    }

    let sanitized = sanitize_provider_name("DeleteCodex");
    let codex_dir = home.join(".codex");
    std::fs::create_dir_all(&codex_dir).expect("create codex dir");
    let auth_path = codex_dir.join(format!("auth-{sanitized}.json"));
    let cfg_path = codex_dir.join(format!("config-{sanitized}.toml"));
    std::fs::write(&auth_path, "{}").expect("seed auth file");
    std::fs::write(&cfg_path, "base_url = \"https://example\"").expect("seed config file");

    let app_state = AppState {
        config: RwLock::new(config),
    };

    ProviderService::delete(&app_state, AppType::Codex, "to-delete")
        .expect("delete provider should succeed");

    let locked = app_state.config.read().expect("lock config after delete");
    let manager = locked.get_manager(&AppType::Codex).expect("codex manager");
    assert!(
        !manager.providers.contains_key("to-delete"),
        "provider entry should be removed"
    );
    assert!(
        !auth_path.exists() && !cfg_path.exists(),
        "provider-specific files should be deleted"
    );
}

#[test]
fn provider_service_delete_claude_removes_provider_files() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Claude)
            .expect("claude manager");
        manager.current = "keep".to_string();
        manager.providers.insert(
            "keep".to_string(),
            Provider::with_id(
                "keep".to_string(),
                "Keep".to_string(),
                json!({
                    "env": { "ANTHROPIC_API_KEY": "keep-key" }
                }),
                None,
            ),
        );
        manager.providers.insert(
            "delete".to_string(),
            Provider::with_id(
                "delete".to_string(),
                "DeleteClaude".to_string(),
                json!({
                    "env": { "ANTHROPIC_API_KEY": "delete-key" }
                }),
                None,
            ),
        );
    }

    let sanitized = sanitize_provider_name("DeleteClaude");
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).expect("create claude dir");
    let by_name = claude_dir.join(format!("settings-{sanitized}.json"));
    let by_id = claude_dir.join("settings-delete.json");
    std::fs::write(&by_name, "{}").expect("seed settings by name");
    std::fs::write(&by_id, "{}").expect("seed settings by id");

    let app_state = AppState {
        config: RwLock::new(config),
    };

    ProviderService::delete(&app_state, AppType::Claude, "delete").expect("delete claude provider");

    let locked = app_state.config.read().expect("lock config after delete");
    let manager = locked
        .get_manager(&AppType::Claude)
        .expect("claude manager");
    assert!(
        !manager.providers.contains_key("delete"),
        "claude provider should be removed"
    );
    assert!(
        !by_name.exists() && !by_id.exists(),
        "provider config files should be deleted"
    );
}

#[test]
fn provider_service_delete_current_provider_returns_error() {
    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Claude)
            .expect("claude manager");
        manager.current = "keep".to_string();
        manager.providers.insert(
            "keep".to_string(),
            Provider::with_id(
                "keep".to_string(),
                "Keep".to_string(),
                json!({
                    "env": { "ANTHROPIC_API_KEY": "keep-key" }
                }),
                None,
            ),
        );
    }

    let app_state = AppState {
        config: RwLock::new(config),
    };

    let err = ProviderService::delete(&app_state, AppType::Claude, "keep")
        .expect_err("deleting current provider should fail");
    match err {
        AppError::Localized { zh, .. } => assert!(
            zh.contains("不能删除当前正在使用的供应商"),
            "unexpected message: {zh}"
        ),
        AppError::Config(msg) => assert!(
            msg.contains("不能删除当前正在使用的供应商"),
            "unexpected message: {msg}"
        ),
        other => panic!("expected Config error, got {other:?}"),
    }
}


#[test]
fn provider_service_update_current_codex_preserves_mcp_in_live_and_snapshot() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Codex)
            .expect("codex manager");
        manager.current = "current".to_string();
        manager.providers.insert(
            "current".to_string(),
            Provider::with_id(
                "current".to_string(),
                "Current Codex".to_string(),
                json!({
                    "auth": {"OPENAI_API_KEY": "stale-key"},
                    "config": "model = \"gpt-4.1\"\n"
                }),
                None,
            ),
        );
    }

    config.mcp.codex.servers.insert(
        "relay-pulse".into(),
        json!({
            "id": "relay-pulse",
            "enabled": true,
            "server": {
                "type": "stdio",
                "command": "relay-pulse"
            }
        }),
    );

    let state = AppState {
        config: RwLock::new(config),
    };

    ProviderService::update(
        &state,
        AppType::Codex,
        Provider::with_id(
            "current".to_string(),
            "Current Codex".to_string(),
            json!({
                "auth": {"OPENAI_API_KEY": "fresh-key"},
                "config": "model = \"gpt-5\"\n"
            }),
            None,
        ),
    )
    .expect("updating current codex provider should succeed");

    let auth_path = unwrap_path(cc_switch_lib::get_codex_auth_path());
    let auth_value: serde_json::Value = read_json_file(&auth_path).expect("read auth.json");
    assert_eq!(
        auth_value.get("OPENAI_API_KEY").and_then(|v| v.as_str()),
        Some("fresh-key"),
        "live auth.json should reflect updated provider auth"
    );

    let config_path = unwrap_path(get_codex_config_path());
    let config_text = std::fs::read_to_string(&config_path).expect("read config.toml");
    assert!(
        config_text.contains("mcp_servers.relay-pulse"),
        "config.toml should preserve enabled MCP servers after updating current provider"
    );
    assert!(
        config_text.contains("command = \"relay-pulse\""),
        "config.toml should contain relay-pulse command after update"
    );

    let guard = state.config.read().expect("read config after update");
    let manager = guard
        .get_manager(&AppType::Codex)
        .expect("codex manager after update");
    let current = manager
        .providers
        .get("current")
        .expect("current provider should still exist");
    let snapshot_config = current
        .settings_config
        .get("config")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        snapshot_config.contains("mcp_servers.relay-pulse"),
        "stored provider snapshot should match live config with MCP servers"
    );
}
