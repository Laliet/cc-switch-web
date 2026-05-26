use serde_json::json;
use std::{collections::HashMap, path::PathBuf};

use cc_switch_lib::{
    get_codex_auth_path, get_codex_config_path, read_json_file, switch_provider_test_hook,
    write_codex_live_atomic, AppError, AppState, AppType, McpApps, McpServer, MultiAppConfig,
    Provider,
};

#[path = "support.rs"]
mod support;
use support::{ensure_test_home, reset_test_fs, test_mutex};

fn unwrap_path(result: Result<PathBuf, AppError>) -> PathBuf {
    result.expect("path should resolve")
}

#[test]
fn switch_provider_updates_codex_live_and_state() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let legacy_auth = json!({"OPENAI_API_KEY": "legacy-key"});
    let legacy_config = r#"[mcp_servers.legacy]
type = "stdio"
command = "echo"
"#;
    write_codex_live_atomic(&legacy_auth, Some(legacy_config))
        .expect("seed existing codex live config");

    let mut config = MultiAppConfig::default();
    {
        let manager = config
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

    config.mcp.codex.servers.insert(
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

    let app_state = AppState::new_for_tests(config).expect("test app state");

    switch_provider_test_hook(&app_state, AppType::Codex, "new-provider")
        .expect("switch provider should succeed");

    let auth_path = unwrap_path(get_codex_auth_path());
    let auth_value: serde_json::Value = read_json_file(&auth_path).expect("read auth.json");
    assert_eq!(
        auth_value
            .get("OPENAI_API_KEY")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        "fresh-key",
        "live auth.json should reflect new provider"
    );

    let config_path = unwrap_path(get_codex_config_path());
    let config_text = std::fs::read_to_string(&config_path).expect("read config.toml");
    assert!(
        config_text.contains("mcp_servers.echo-server"),
        "config.toml should contain synced MCP servers"
    );

    let locked = app_state.load_config().expect("lock config after switch");
    let manager = locked
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
fn switch_provider_missing_provider_returns_error() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();

    let mut config = MultiAppConfig::default();
    config
        .get_manager_mut(&AppType::Claude)
        .expect("claude manager")
        .current = "does-not-exist".to_string();

    let app_state = AppState::new_for_tests(config).expect("test app state");

    let err = switch_provider_test_hook(&app_state, AppType::Claude, "missing-provider")
        .expect_err("switching to a missing provider should fail");

    assert!(
        err.to_string().contains("供应商不存在"),
        "error message should mention missing provider"
    );
}

#[test]
fn switch_provider_updates_claude_live_and_state() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let settings_path = unwrap_path(cc_switch_lib::get_claude_settings_path());
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

    let app_state = AppState::new_for_tests(config).expect("test app state");

    switch_provider_test_hook(&app_state, AppType::Claude, "new-provider")
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

    let locked = app_state.load_config().expect("lock config after switch");
    let manager = locked
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

    let new_provider = manager
        .providers
        .get("new-provider")
        .expect("new provider exists");
    assert_eq!(
        new_provider
            .settings_config
            .get("env")
            .and_then(|env| env.get("ANTHROPIC_API_KEY"))
            .and_then(|key| key.as_str()),
        Some("fresh-key"),
        "new provider snapshot should retain fresh auth"
    );

    drop(locked);

    let persisted = app_state
        .load_config()
        .expect("reload provider state from database");
    assert_eq!(
        persisted
            .get_manager(&AppType::Claude)
            .expect("claude manager")
            .current,
        "new-provider",
        "SQLite state should record the new current provider"
    );
}

#[test]
fn switch_provider_codex_missing_auth_returns_error_and_keeps_state() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

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

    let app_state = AppState::new_for_tests(config).expect("test app state");

    let err = switch_provider_test_hook(&app_state, AppType::Codex, "invalid")
        .expect_err("switching should fail when auth missing");
    match err {
        AppError::Config(msg) => assert!(
            msg.contains("auth"),
            "expected auth missing error message, got {msg}"
        ),
        other => panic!("expected config error, got {other:?}"),
    }

    let locked = app_state.load_config().expect("lock config after failure");
    let manager = locked.get_manager(&AppType::Codex).expect("codex manager");
    assert!(
        manager.current.is_empty(),
        "current provider should remain empty on failure"
    );
}

#[test]
fn switch_provider_omo_rolls_back_opencode_plugin_when_post_commit_fails() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let opencode_dir = home.join(".config").join("opencode");
    std::fs::create_dir_all(&opencode_dir).expect("create opencode dir");

    let opencode_path = opencode_dir.join("opencode.json");
    std::fs::write(
        &opencode_path,
        serde_json::to_string_pretty(&json!({
            "$schema": "https://opencode.ai/config.json",
            "plugin": ["existing-plugin@1.0.0"]
        }))
        .expect("serialize opencode config"),
    )
    .expect("write opencode config");

    let omo_path = opencode_dir.join("oh-my-opencode.jsonc");
    std::fs::write(
        &omo_path,
        serde_json::to_string_pretty(&json!({
            "agents": { "legacy-agent": {} },
            "categories": {}
        }))
        .expect("serialize omo config"),
    )
    .expect("write omo config");

    let mut config = MultiAppConfig::default();
    {
        let manager = config.get_manager_mut(&AppType::Omo).expect("omo manager");
        manager.current = "old-omo".to_string();
        manager.providers.insert(
            "old-omo".to_string(),
            Provider::with_id(
                "old-omo".to_string(),
                "Old OMO".to_string(),
                json!({
                    "agents": { "legacy-agent": {} },
                    "categories": {}
                }),
                None,
            ),
        );
        manager.providers.insert(
            "new-omo".to_string(),
            Provider::with_id(
                "new-omo".to_string(),
                "New OMO".to_string(),
                json!({
                    "agents": { "new-agent": {} },
                    "categories": {}
                }),
                None,
            ),
        );
    }

    config.mcp.servers = Some(HashMap::from([(
        "broken-server".to_string(),
        McpServer {
            id: "broken-server".to_string(),
            name: "Broken Server".to_string(),
            server: json!({
                "type": "unknown"
            }),
            apps: McpApps {
                opencode: true,
                ..McpApps::default()
            },
            description: None,
            homepage: None,
            docs: None,
            tags: Vec::new(),
        },
    )]));

    let app_state = AppState::new_for_tests(config).expect("test app state");

    let err = switch_provider_test_hook(&app_state, AppType::Omo, "new-omo")
        .expect_err("post-commit failure should bubble up");
    assert!(
        err.to_string().contains("Unknown MCP type"),
        "unexpected error: {err}"
    );

    let locked = app_state.load_config().expect("lock config after failure");
    let manager = locked.get_manager(&AppType::Omo).expect("omo manager");
    assert_eq!(
        manager.current, "old-omo",
        "failed switch should restore previous current provider"
    );
    drop(locked);

    let restored_omo: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&omo_path).expect("read restored omo"))
            .expect("parse restored omo");
    assert!(
        restored_omo
            .get("agents")
            .and_then(|agents| agents.get("legacy-agent"))
            .is_some(),
        "omo config should roll back to the previous file contents"
    );
    assert!(
        restored_omo
            .get("agents")
            .and_then(|agents| agents.get("new-agent"))
            .is_none(),
        "new omo config should not remain after rollback"
    );

    let restored_opencode: serde_json::Value =
        read_json_file(&opencode_path).expect("read restored opencode config");
    assert_eq!(
        restored_opencode
            .get("plugin")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default(),
        vec![json!("existing-plugin@1.0.0")],
        "opencode plugin list should be restored after rollback"
    );
}

#[test]
fn switch_provider_omo_replaces_old_plugin_versions_with_latest() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let opencode_dir = home.join(".config").join("opencode");
    std::fs::create_dir_all(&opencode_dir).expect("create opencode dir");

    let opencode_path = opencode_dir.join("opencode.json");
    std::fs::write(
        &opencode_path,
        serde_json::to_string_pretty(&json!({
            "$schema": "https://opencode.ai/config.json",
            "plugin": [
                "existing-plugin@1.0.0",
                "oh-my-opencode@0.3.0"
            ]
        }))
        .expect("serialize opencode config"),
    )
    .expect("write opencode config");

    let mut config = MultiAppConfig::default();
    {
        let manager = config.get_manager_mut(&AppType::Omo).expect("omo manager");
        manager.current = "omo".to_string();
        manager.providers.insert(
            "omo".to_string(),
            Provider::with_id(
                "omo".to_string(),
                "OMO".to_string(),
                json!({
                    "agents": { "agent-a": {} },
                    "categories": {}
                }),
                None,
            ),
        );
    }

    let app_state = AppState::new_for_tests(config).expect("test app state");

    switch_provider_test_hook(&app_state, AppType::Omo, "omo")
        .expect("switch provider should succeed");

    let stored: serde_json::Value =
        read_json_file(&opencode_path).expect("read updated opencode config");
    assert_eq!(
        stored
            .get("plugin")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default(),
        vec![
            json!("existing-plugin@1.0.0"),
            json!("oh-my-opencode@latest")
        ],
        "legacy OMO plugin versions should be replaced with a single latest entry"
    );
}

#[test]
fn switch_provider_opencode_rejects_full_config_without_current_provider_fragment() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let opencode_dir = home.join(".config").join("opencode");
    std::fs::create_dir_all(&opencode_dir).expect("create opencode dir");

    let opencode_path = opencode_dir.join("opencode.json");
    std::fs::write(
        &opencode_path,
        serde_json::to_string_pretty(&json!({
            "$schema": "https://opencode.ai/config.json",
            "provider": {
                "legacy": {
                    "options": {
                        "apiKey": "legacy-key",
                        "baseURL": "https://legacy.example.com"
                    }
                }
            }
        }))
        .expect("serialize opencode config"),
    )
    .expect("write opencode config");

    let mut config = MultiAppConfig::default();
    {
        let manager = config
            .get_manager_mut(&AppType::Opencode)
            .expect("opencode manager");
        manager.current = "legacy".to_string();
        manager.providers.insert(
            "legacy".to_string(),
            Provider::with_id(
                "legacy".to_string(),
                "Legacy".to_string(),
                json!({
                    "options": {
                        "apiKey": "legacy-key",
                        "baseURL": "https://legacy.example.com"
                    }
                }),
                None,
            ),
        );
        manager.providers.insert(
            "broken".to_string(),
            Provider::with_id(
                "broken".to_string(),
                "Broken".to_string(),
                json!({
                    "$schema": "https://opencode.ai/config.json",
                    "provider": {
                        "other-provider": {
                            "options": {
                                "apiKey": "other-key",
                                "baseURL": "https://other.example.com"
                            }
                        }
                    }
                }),
                None,
            ),
        );
    }

    let app_state = AppState::new_for_tests(config).expect("test app state");

    let err = switch_provider_test_hook(&app_state, AppType::Opencode, "broken")
        .expect_err("switch should reject malformed full config");
    match err {
        AppError::Localized { key, .. } => {
            assert_eq!(key, "provider.opencode.fragment.missing");
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let stored: serde_json::Value =
        read_json_file(&opencode_path).expect("read opencode config after failed switch");
    assert_eq!(
        stored
            .get("provider")
            .and_then(|providers| providers.get("legacy"))
            .and_then(|provider| provider.get("options"))
            .and_then(|options| options.get("baseURL"))
            .and_then(|value| value.as_str()),
        Some("https://legacy.example.com"),
        "existing live opencode config should remain unchanged after rejection"
    );

    let locked = app_state.load_config().expect("lock config after failure");
    let manager = locked
        .get_manager(&AppType::Opencode)
        .expect("opencode manager after failure");
    assert_eq!(
        manager.current, "legacy",
        "failed switch should keep the previous current provider"
    );
}
