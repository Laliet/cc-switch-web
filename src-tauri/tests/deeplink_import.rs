use cc_switch_lib::{
    import_provider_from_deeplink, parse_deeplink_url, AppState, AppType, MultiAppConfig,
};

#[path = "support.rs"]
mod support;
use support::{ensure_test_home, reset_test_fs, test_mutex};

#[test]
fn deeplink_import_claude_provider_persists_to_config() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    ensure_test_home();

    let url = "ccswitch://v1/import?resource=provider&app=claude&name=DeepLink%20Claude&homepage=https%3A%2F%2Fexample.com&endpoint=https%3A%2F%2Fapi.example.com%2Fv1&apiKey=sk-test-claude-key&model=claude-sonnet-4";
    let request = parse_deeplink_url(url).expect("parse deeplink url");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Claude);

    let state = AppState::new_for_tests(config).expect("test app state");

    let provider_id = import_provider_from_deeplink(&state, request.clone())
        .expect("import provider from deeplink");

    // 验证内存状态
    let guard = state.load_config().expect("read config");
    let manager = guard
        .get_manager(&AppType::Claude)
        .expect("claude manager should exist");
    let provider = manager
        .providers
        .get(&provider_id)
        .expect("provider created via deeplink");
    assert_eq!(Some(provider.name.as_str()), request.name.as_deref());
    assert_eq!(provider.website_url.as_deref(), request.homepage.as_deref());
    let auth_token = provider
        .settings_config
        .pointer("/env/ANTHROPIC_AUTH_TOKEN")
        .and_then(|v| v.as_str());
    let base_url = provider
        .settings_config
        .pointer("/env/ANTHROPIC_BASE_URL")
        .and_then(|v| v.as_str());
    assert_eq!(auth_token, request.api_key.as_deref());
    assert_eq!(base_url, request.endpoint.as_deref());
    drop(guard);

    let reloaded = state.load_config().expect("reload config from database");
    assert!(
        reloaded
            .get_manager(&AppType::Claude)
            .expect("claude manager should exist")
            .providers
            .contains_key(&provider_id),
        "importing provider from deeplink should persist provider in SQLite"
    );
}

#[test]
fn deeplink_import_codex_provider_builds_auth_and_config() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    ensure_test_home();

    let url = "ccswitch://v1/import?resource=provider&app=codex&name=DeepLink%20Codex&homepage=https%3A%2F%2Fopenai.example&endpoint=https%3A%2F%2Fapi.openai.example%2Fv1&apiKey=sk-test-codex-key&model=gpt-4o";
    let request = parse_deeplink_url(url).expect("parse deeplink url");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Codex);

    let state = AppState::new_for_tests(config).expect("test app state");

    let provider_id = import_provider_from_deeplink(&state, request.clone())
        .expect("import provider from deeplink");

    let guard = state.load_config().expect("read config");
    let manager = guard
        .get_manager(&AppType::Codex)
        .expect("codex manager should exist");
    let provider = manager
        .providers
        .get(&provider_id)
        .expect("provider created via deeplink");
    assert_eq!(Some(provider.name.as_str()), request.name.as_deref());
    assert_eq!(provider.website_url.as_deref(), request.homepage.as_deref());
    let auth_value = provider
        .settings_config
        .pointer("/auth/OPENAI_API_KEY")
        .and_then(|v| v.as_str());
    let config_text = provider
        .settings_config
        .get("config")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert_eq!(auth_value, request.api_key.as_deref());
    assert!(
        request
            .endpoint
            .as_deref()
            .is_some_and(|endpoint| config_text.contains(endpoint)),
        "config.toml content should contain endpoint"
    );
    assert!(
        config_text.contains("model = \"gpt-4o\""),
        "config.toml content should contain model setting"
    );
    drop(guard);

    let reloaded = state.load_config().expect("reload config from database");
    assert!(
        reloaded
            .get_manager(&AppType::Codex)
            .expect("codex manager should exist")
            .providers
            .contains_key(&provider_id),
        "importing provider from deeplink should persist provider in SQLite"
    );
}
