use std::str::FromStr;

use cc_switch_lib::{
    AppType, ClaudeDesktopMode, ClaudeDesktopModelRoute, ManagedAuthProvider, MultiAppConfig,
    ProviderAuthBinding, ProviderMeta, ProviderType,
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
        auth_binding: Some(ProviderAuthBinding {
            mode: "managed".to_string(),
            provider_type: Some("github_copilot".to_string()),
            account_id: Some("github-1".to_string()),
            use_default: Some(false),
        }),
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
    assert_eq!(value["authBinding"]["mode"], "managed");
    assert_eq!(value["authBinding"]["providerType"], "github_copilot");
    assert_eq!(value["authBinding"]["accountId"], "github-1");
    assert_eq!(value["authBinding"]["useDefault"], false);

    let decoded: ProviderMeta = serde_json::from_value(value).expect("deserialize provider meta");
    assert_eq!(decoded.claude_desktop_mode, Some(ClaudeDesktopMode::Proxy));
    assert_eq!(
        decoded
            .claude_desktop_model_routes
            .get("claude-sonnet-4-20250514")
            .map(|route| route.model.as_str()),
        Some("sonnet-real")
    );
    assert_eq!(
        decoded
            .auth_binding
            .as_ref()
            .and_then(|binding| binding.provider_type.as_deref()),
        Some("github_copilot")
    );
}

#[test]
fn provider_type_accepts_oauth_aliases_and_maps_managed_provider() {
    assert_eq!(
        ProviderType::parse("github_copilot"),
        Some(ProviderType::GithubCopilot)
    );
    assert_eq!(
        ProviderType::parse("github-copilot"),
        Some(ProviderType::GithubCopilot)
    );
    assert_eq!(
        ProviderType::parse("GitHub Copilot"),
        Some(ProviderType::GithubCopilot)
    );
    assert_eq!(
        ProviderType::parse(" copilot "),
        Some(ProviderType::GithubCopilot)
    );
    assert_eq!(
        ProviderType::parse("codex_oauth"),
        Some(ProviderType::CodexOauth)
    );
    assert_eq!(
        ProviderType::parse("codex-oauth"),
        Some(ProviderType::CodexOauth)
    );
    assert_eq!(
        ProviderType::parse("Codex OAuth"),
        Some(ProviderType::CodexOauth)
    );
    assert_eq!(
        ProviderType::parse("ChatGPT"),
        Some(ProviderType::CodexOauth)
    );
    assert_eq!(
        ProviderType::GithubCopilot.managed_auth_provider().as_str(),
        "github_copilot"
    );
    assert_eq!(
        ProviderType::CodexOauth.managed_auth_provider().as_str(),
        "codex_oauth"
    );
}

#[test]
fn managed_auth_provider_deserialize_accepts_oauth_aliases() {
    assert_eq!(
        serde_json::from_value::<ManagedAuthProvider>(serde_json::json!("github-copilot"))
            .expect("github alias"),
        ManagedAuthProvider::GithubCopilot
    );
    assert_eq!(
        serde_json::from_value::<ManagedAuthProvider>(serde_json::json!("GitHub Copilot"))
            .expect("github display alias"),
        ManagedAuthProvider::GithubCopilot
    );
    assert_eq!(
        serde_json::from_value::<ManagedAuthProvider>(serde_json::json!("ChatGPT"))
            .expect("chatgpt alias"),
        ManagedAuthProvider::CodexOauth
    );
    assert_eq!(
        serde_json::from_value::<ManagedAuthProvider>(serde_json::json!("Codex OAuth"))
            .expect("codex display alias"),
        ManagedAuthProvider::CodexOauth
    );
}

#[test]
fn parse_unknown_app_returns_localized_error_message() {
    let err = AppType::from_str("unknown").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("可选值") || msg.contains("Allowed"));
    assert!(msg.contains("unknown"));
}
