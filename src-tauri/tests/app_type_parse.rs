use std::str::FromStr;

use cc_switch_lib::{
    AppType, ClaudeDesktopMode, ClaudeDesktopModelRoute, MultiAppConfig, ProviderMeta,
};

#[test]
fn parse_known_apps_case_insensitive_and_trim() {
    assert!(matches!(AppType::from_str("claude"), Ok(AppType::Claude)));
    assert!(matches!(AppType::from_str("codex"), Ok(AppType::Codex)));
    assert!(matches!(AppType::from_str("gemini"), Ok(AppType::Gemini)));
    assert!(matches!(
        AppType::from_str("opencode"),
        Ok(AppType::Opencode)
    ));
    assert!(matches!(
        AppType::from_str("claude-desktop"),
        Ok(AppType::ClaudeDesktop)
    ));
    assert!(matches!(
        AppType::from_str("claude_desktop"),
        Ok(AppType::ClaudeDesktop)
    ));
    assert!(matches!(
        AppType::from_str("claudedesktop"),
        Ok(AppType::ClaudeDesktop)
    ));
    assert!(matches!(AppType::from_str("omo"), Ok(AppType::Omo)));
    assert!(matches!(
        AppType::from_str(" ClAuDe \n"),
        Ok(AppType::Claude)
    ));
    assert!(matches!(AppType::from_str("\tcoDeX\t"), Ok(AppType::Codex)));
}

#[test]
fn parse_supported_accepts_opencode_and_rejects_omo() {
    assert!(matches!(
        AppType::parse_supported("claude-desktop"),
        Ok(AppType::ClaudeDesktop)
    ));
    assert!(matches!(
        AppType::parse_supported("opencode"),
        Ok(AppType::Opencode)
    ));
    let err = AppType::parse_supported("omo").unwrap_err();
    assert!(err.to_string().contains("暂未支持") || err.to_string().contains("not supported yet"));
}

#[test]
fn parse_skills_app_maps_omo_to_opencode() {
    assert!(matches!(
        AppType::parse_skills_app("omo"),
        Ok(AppType::Opencode)
    ));
}

#[test]
fn parse_skills_app_rejects_claude_desktop() {
    let err = AppType::parse_skills_app("claude-desktop").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Claude Desktop"));
    assert!(msg.contains("Skills") || msg.contains("skills"));
}

#[test]
fn mcp_for_omo_uses_opencode_storage() {
    let mut config = MultiAppConfig::default();
    config.mcp_for_mut(&AppType::Omo).servers.insert(
        "omo-shared".to_string(),
        serde_json::json!({ "type": "stdio" }),
    );

    assert!(config
        .mcp_for(&AppType::Opencode)
        .servers
        .contains_key("omo-shared"));
    assert!(!config
        .mcp_for(&AppType::Codex)
        .servers
        .contains_key("omo-shared"));
}

#[test]
fn provider_meta_claude_desktop_fields_roundtrip_with_camel_case() {
    let mut meta = ProviderMeta {
        claude_desktop_mode: Some(ClaudeDesktopMode::Proxy),
        api_format: Some("anthropic".to_string()),
        api_key_field: Some("ANTHROPIC_API_KEY".to_string()),
        is_full_url: Some(true),
        prompt_cache_key: Some("cache".to_string()),
        codex_fast_mode: Some(false),
        provider_type: Some("custom".to_string()),
        github_account_id: Some("github-1".to_string()),
        ..ProviderMeta::default()
    };
    meta.claude_desktop_model_routes.insert(
        "claude-sonnet-4-20250514".to_string(),
        ClaudeDesktopModelRoute {
            model: "sonnet-real".to_string(),
            label_override: Some("Sonnet".to_string()),
            supports_1m: Some(true),
        },
    );

    let value = serde_json::to_value(&meta).expect("serialize provider meta");
    assert_eq!(value["claudeDesktopMode"], "proxy");
    assert_eq!(
        value["claudeDesktopModelRoutes"]["claude-sonnet-4-20250514"]["labelOverride"],
        "Sonnet"
    );
    assert_eq!(
        value["claudeDesktopModelRoutes"]["claude-sonnet-4-20250514"]["supports1m"],
        true
    );
    assert_eq!(value["apiFormat"], "anthropic");
    assert_eq!(value["apiKeyField"], "ANTHROPIC_API_KEY");
    assert_eq!(value["isFullUrl"], true);
    assert_eq!(value["promptCacheKey"], "cache");
    assert_eq!(value["codexFastMode"], false);
    assert_eq!(value["providerType"], "custom");
    assert_eq!(value["githubAccountId"], "github-1");

    let decoded: ProviderMeta = serde_json::from_value(value).expect("deserialize provider meta");
    assert_eq!(decoded.claude_desktop_mode, Some(ClaudeDesktopMode::Proxy));
    assert_eq!(
        decoded
            .claude_desktop_model_routes
            .get("claude-sonnet-4-20250514")
            .map(|route| route.model.as_str()),
        Some("sonnet-real")
    );
}

#[test]
fn parse_unknown_app_returns_localized_error_message() {
    let err = AppType::from_str("unknown").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("可选值") || msg.contains("Allowed"));
    assert!(msg.contains("unknown"));
}
