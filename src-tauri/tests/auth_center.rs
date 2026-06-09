use base64::Engine;
use cc_switch_lib::{
    database::Database,
    proxy::adapters::{adapter_for, insert_auth_headers, resolve_auth_for_provider},
    AppError, AppState, AppType, ManagedAuthAccountInput, ManagedAuthProvider, ManagedAuthTokenSet,
    MultiAppConfig, Provider, ProviderAuthBinding, ProviderMeta,
};
use reqwest::header::HeaderMap;
use serde_json::json;

#[test]
fn managed_auth_accounts_roundtrip_and_default_switches() {
    let db = Database::memory().expect("memory db");

    let first = db
        .upsert_managed_auth_account(ManagedAuthAccountInput {
            provider: ManagedAuthProvider::GithubCopilot,
            id: Some("gh-1".to_string()),
            label: "GitHub One".to_string(),
            username: Some("octo".to_string()),
            avatar_url: None,
            plan: Some("copilot".to_string()),
            make_default: false,
            tokens: ManagedAuthTokenSet {
                access_token: "token-1".to_string(),
                refresh_token: Some("refresh-1".to_string()),
                expires_at: None,
                scope: Some("copilot".to_string()),
                token_type: Some("Bearer".to_string()),
            },
        })
        .expect("insert first");
    assert!(first.is_default);

    let second = db
        .upsert_managed_auth_account(ManagedAuthAccountInput {
            provider: ManagedAuthProvider::GithubCopilot,
            id: Some("gh-2".to_string()),
            label: "GitHub Two".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            make_default: true,
            tokens: ManagedAuthTokenSet {
                access_token: "token-2".to_string(),
                refresh_token: None,
                expires_at: None,
                scope: None,
                token_type: None,
            },
        })
        .expect("insert second");
    assert!(second.is_default);

    let accounts = db
        .list_managed_auth_accounts(Some(ManagedAuthProvider::GithubCopilot))
        .expect("list accounts");
    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[0].id, "gh-2");
    assert!(accounts[0].is_default);
    assert!(!accounts[1].is_default);

    let secret = db
        .get_default_managed_auth_account(ManagedAuthProvider::GithubCopilot)
        .expect("load default")
        .expect("default account");
    assert_eq!(secret.account.id, "gh-2");
    assert_eq!(secret.tokens.access_token, "token-2");

    db.set_default_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-1")
        .expect("set default");
    let secret = db
        .get_default_managed_auth_account(ManagedAuthProvider::GithubCopilot)
        .expect("load default")
        .expect("default account");
    assert_eq!(secret.account.id, "gh-1");
    assert_eq!(secret.tokens.refresh_token.as_deref(), Some("refresh-1"));
}

#[test]
fn managed_auth_upsert_preserves_existing_default_account() {
    let db = Database::memory().expect("memory db");

    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-default".to_string()),
        label: "GitHub Default".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: true,
        tokens: ManagedAuthTokenSet {
            access_token: "token-old".to_string(),
            refresh_token: Some("refresh-old".to_string()),
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert default");

    let refreshed = db
        .upsert_managed_auth_account(ManagedAuthAccountInput {
            provider: ManagedAuthProvider::GithubCopilot,
            id: Some("gh-default".to_string()),
            label: "GitHub Default".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            make_default: false,
            tokens: ManagedAuthTokenSet {
                access_token: "token-new".to_string(),
                refresh_token: Some("refresh-new".to_string()),
                expires_at: None,
                scope: None,
                token_type: Some("Bearer".to_string()),
            },
        })
        .expect("refresh default account");

    assert!(refreshed.is_default);
    let default = db
        .get_default_managed_auth_account(ManagedAuthProvider::GithubCopilot)
        .expect("load default")
        .expect("default account");
    assert_eq!(default.account.id, "gh-default");
    assert_eq!(default.tokens.access_token, "token-new");
}

#[test]
fn managed_auth_logout_clears_tokens_without_deleting_account() {
    let db = Database::memory().expect("memory db");

    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-logout".to_string()),
        label: "GitHub Logout".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: true,
        tokens: ManagedAuthTokenSet {
            access_token: "token-before-logout".to_string(),
            refresh_token: Some("refresh-before-logout".to_string()),
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert account");

    assert!(db
        .logout_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-logout")
        .expect("logout account"));

    let accounts = db
        .list_managed_auth_accounts(Some(ManagedAuthProvider::GithubCopilot))
        .expect("list accounts");
    assert_eq!(accounts.len(), 1);
    assert!(!accounts[0].is_default);
    assert_eq!(accounts[0].status.as_deref(), Some("logged_out"));

    let secret = db
        .get_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-logout")
        .expect("load account")
        .expect("account remains after logout");
    assert_eq!(secret.tokens.access_token, "");
    assert!(secret.tokens.refresh_token.is_none());

    assert!(db
        .delete_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-logout")
        .expect("delete logged out account"));
}

#[test]
fn managed_auth_logout_default_promotes_another_active_account() {
    let db = Database::memory().expect("memory db");

    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-active".to_string()),
        label: "GitHub Active".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: false,
        tokens: ManagedAuthTokenSet {
            access_token: "active-token".to_string(),
            refresh_token: None,
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert active account");
    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-default".to_string()),
        label: "GitHub Default".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: true,
        tokens: ManagedAuthTokenSet {
            access_token: "default-token".to_string(),
            refresh_token: None,
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert default account");

    assert!(db
        .logout_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-default")
        .expect("logout default account"));

    let default = db
        .get_default_managed_auth_account(ManagedAuthProvider::GithubCopilot)
        .expect("load default")
        .expect("active account promoted");
    assert_eq!(default.account.id, "gh-active");
    assert_eq!(default.tokens.access_token, "active-token");

    let logged_out = db
        .get_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-default")
        .expect("load logged out")
        .expect("logged out account remains");
    assert!(!logged_out.account.is_default);
    assert_eq!(logged_out.account.status.as_deref(), Some("logged_out"));
}

#[test]
fn managed_auth_delete_default_does_not_promote_logged_out_account() {
    let db = Database::memory().expect("memory db");

    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-logged-out".to_string()),
        label: "GitHub Logged Out".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: false,
        tokens: ManagedAuthTokenSet {
            access_token: "logout-token".to_string(),
            refresh_token: None,
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert logged out account");
    db.logout_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-logged-out")
        .expect("logout account");

    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-default".to_string()),
        label: "GitHub Default".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: true,
        tokens: ManagedAuthTokenSet {
            access_token: "default-token".to_string(),
            refresh_token: None,
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert default account");

    assert!(db
        .delete_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-default")
        .expect("delete default account"));

    let accounts = db
        .list_managed_auth_accounts(Some(ManagedAuthProvider::GithubCopilot))
        .expect("list accounts");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].id, "gh-logged-out");
    assert!(!accounts[0].is_default);
    assert_eq!(accounts[0].status.as_deref(), Some("logged_out"));
    assert!(db
        .get_default_managed_auth_account(ManagedAuthProvider::GithubCopilot)
        .expect("load default")
        .is_none());
}

#[test]
fn managed_auth_logged_out_account_cannot_be_set_default() {
    let db = Database::memory().expect("memory db");

    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-active".to_string()),
        label: "GitHub Active".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: true,
        tokens: ManagedAuthTokenSet {
            access_token: "active-token".to_string(),
            refresh_token: None,
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert active account");
    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-logged-out".to_string()),
        label: "GitHub Logged Out".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: false,
        tokens: ManagedAuthTokenSet {
            access_token: "logout-token".to_string(),
            refresh_token: None,
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert logout account");
    db.logout_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-logged-out")
        .expect("logout account");

    let err = db
        .set_default_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-logged-out")
        .expect_err("logged out account cannot be default");
    assert!(err.to_string().contains("logged out"));

    let default = db
        .get_default_managed_auth_account(ManagedAuthProvider::GithubCopilot)
        .expect("load default")
        .expect("active default");
    assert_eq!(default.account.id, "gh-active");
}

#[test]
fn managed_auth_default_lookup_falls_back_to_recent_active_account() {
    let db = Database::memory().expect("memory db");

    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-logged-out".to_string()),
        label: "GitHub Logged Out".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: true,
        tokens: ManagedAuthTokenSet {
            access_token: "logout-token".to_string(),
            refresh_token: None,
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert default account");
    db.logout_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-logged-out")
        .expect("logout only account");
    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-active".to_string()),
        label: "GitHub Active".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: false,
        tokens: ManagedAuthTokenSet {
            access_token: "active-token".to_string(),
            refresh_token: None,
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert active account");

    let default = db
        .get_default_managed_auth_account(ManagedAuthProvider::GithubCopilot)
        .expect("load default")
        .expect("active fallback");
    assert_eq!(default.account.id, "gh-active");
    assert_eq!(default.tokens.access_token, "active-token");
}

#[test]
fn managed_auth_tokens_are_encrypted_at_rest_and_read_transparently() {
    let db = Database::memory().expect("memory db");
    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-secure".to_string()),
        label: "GitHub Secure".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: true,
        tokens: ManagedAuthTokenSet {
            access_token: "plain-access-token".to_string(),
            refresh_token: Some("plain-refresh-token".to_string()),
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert account");

    let raw = db
        .get_raw_managed_auth_tokens_for_tests(ManagedAuthProvider::GithubCopilot, "gh-secure")
        .expect("load raw tokens")
        .expect("raw tokens");
    assert_ne!(raw.0, "plain-access-token");
    assert_ne!(raw.1.as_deref(), Some("plain-refresh-token"));
    assert!(raw.0.starts_with("ccs2:"));
    assert!(raw
        .1
        .as_deref()
        .is_some_and(|value| value.starts_with("ccs2:")));

    let secret = db
        .get_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-secure")
        .expect("load secret")
        .expect("secret account");
    assert_eq!(secret.tokens.access_token, "plain-access-token");
    assert_eq!(
        secret.tokens.refresh_token.as_deref(),
        Some("plain-refresh-token")
    );
}

#[test]
fn managed_auth_tampered_tokens_fail_integrity_check() {
    let db = Database::memory().expect("memory db");
    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-tampered".to_string()),
        label: "GitHub Tampered".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: true,
        tokens: ManagedAuthTokenSet {
            access_token: "plain-access-token".to_string(),
            refresh_token: Some("plain-refresh-token".to_string()),
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert account");

    let raw = db
        .get_raw_managed_auth_tokens_for_tests(ManagedAuthProvider::GithubCopilot, "gh-tampered")
        .expect("load raw tokens")
        .expect("raw tokens");
    assert!(raw.0.starts_with("ccs2:"));
    let tampered_access_token = tamper_ciphertext(&raw.0);
    assert_ne!(tampered_access_token, raw.0);

    assert!(db
        .set_raw_managed_auth_tokens_for_tests(
            ManagedAuthProvider::GithubCopilot,
            "gh-tampered",
            &tampered_access_token,
            raw.1.as_deref(),
        )
        .expect("tamper raw token"));

    let err = db
        .get_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-tampered")
        .expect_err("tampered token should fail");
    assert!(matches!(err, AppError::Unauthorized(_)));
}

#[test]
fn managed_auth_legacy_plaintext_tokens_read_from_database() {
    let db = Database::memory().expect("memory db");
    db.upsert_managed_auth_account(ManagedAuthAccountInput {
        provider: ManagedAuthProvider::GithubCopilot,
        id: Some("gh-legacy-token".to_string()),
        label: "GitHub Legacy Token".to_string(),
        username: None,
        avatar_url: None,
        plan: None,
        make_default: true,
        tokens: ManagedAuthTokenSet {
            access_token: "encrypted-access-placeholder".to_string(),
            refresh_token: Some("encrypted-refresh-placeholder".to_string()),
            expires_at: None,
            scope: None,
            token_type: Some("Bearer".to_string()),
        },
    })
    .expect("insert account");

    assert!(db
        .set_raw_managed_auth_tokens_for_tests(
            ManagedAuthProvider::GithubCopilot,
            "gh-legacy-token",
            "legacy-access-token",
            Some("legacy-refresh-token"),
        )
        .expect("write legacy raw tokens"));

    let secret = db
        .get_managed_auth_account(ManagedAuthProvider::GithubCopilot, "gh-legacy-token")
        .expect("load legacy secret")
        .expect("secret account");
    assert_eq!(secret.tokens.access_token, "legacy-access-token");
    assert_eq!(
        secret.tokens.refresh_token.as_deref(),
        Some("legacy-refresh-token")
    );
}

#[tokio::test]
async fn proxy_auth_resolver_uses_managed_account_binding() {
    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Claude);
    let state = std::sync::Arc::new(AppState::new_for_tests(config).expect("state"));
    state
        .db
        .upsert_managed_auth_account(ManagedAuthAccountInput {
            provider: ManagedAuthProvider::GithubCopilot,
            id: Some("github-1".to_string()),
            label: "GitHub One".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            make_default: true,
            tokens: ManagedAuthTokenSet {
                access_token: "managed-token".to_string(),
                refresh_token: None,
                expires_at: None,
                scope: None,
                token_type: Some("Bearer".to_string()),
            },
        })
        .expect("insert account");

    let provider = Provider {
        id: "copilot".to_string(),
        name: "Copilot".to_string(),
        settings_config: json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com",
                "ANTHROPIC_AUTH_TOKEN": "placeholder"
            }
        }),
        website_url: None,
        category: None,
        created_at: None,
        sort_index: None,
        notes: None,
        meta: Some(ProviderMeta {
            provider_type: Some("github_copilot".to_string()),
            auth_binding: Some(ProviderAuthBinding {
                mode: "managed".to_string(),
                provider_type: Some("github_copilot".to_string()),
                account_id: Some("github-1".to_string()),
                use_default: Some(false),
            }),
            ..ProviderMeta::default()
        }),
    };

    let auth = resolve_auth_for_provider(
        &state,
        &AppType::Claude,
        &provider,
        adapter_for(&AppType::Claude),
    )
    .await
    .expect("resolve auth")
    .expect("auth");
    assert_eq!(auth.api_key, "managed-token");
}

#[tokio::test]
async fn proxy_auth_headers_include_copilot_integration_metadata() {
    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Claude);
    let state = std::sync::Arc::new(AppState::new_for_tests(config).expect("state"));
    state
        .db
        .upsert_managed_auth_account(ManagedAuthAccountInput {
            provider: ManagedAuthProvider::GithubCopilot,
            id: Some("github-1".to_string()),
            label: "GitHub One".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            make_default: true,
            tokens: ManagedAuthTokenSet {
                access_token: "managed-token".to_string(),
                refresh_token: None,
                expires_at: None,
                scope: None,
                token_type: Some("Bearer".to_string()),
            },
        })
        .expect("insert account");

    let provider = Provider {
        id: "copilot".to_string(),
        name: "Copilot".to_string(),
        settings_config: json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com",
                "ANTHROPIC_AUTH_TOKEN": "placeholder"
            }
        }),
        website_url: None,
        category: None,
        created_at: None,
        sort_index: None,
        notes: None,
        meta: Some(ProviderMeta {
            provider_type: Some("github_copilot".to_string()),
            auth_binding: Some(ProviderAuthBinding {
                mode: "managed".to_string(),
                provider_type: Some("github_copilot".to_string()),
                account_id: Some("github-1".to_string()),
                use_default: Some(false),
            }),
            ..ProviderMeta::default()
        }),
    };
    let adapter = adapter_for(&AppType::Claude);
    let auth = resolve_auth_for_provider(&state, &AppType::Claude, &provider, adapter)
        .await
        .expect("resolve auth")
        .expect("auth");
    let mut headers = HeaderMap::new();
    insert_auth_headers(&mut headers, adapter, &auth);

    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer managed-token")
    );
    assert!(headers.get("x-api-key").is_none());
    assert_eq!(
        headers
            .get("copilot-integration-id")
            .and_then(|value| value.to_str().ok()),
        Some("vscode-chat")
    );
    assert_eq!(
        headers
            .get("editor-version")
            .and_then(|value| value.to_str().ok()),
        Some("vscode/1.110.1")
    );
    assert_eq!(
        headers
            .get("editor-plugin-version")
            .and_then(|value| value.to_str().ok()),
        Some("copilot-chat/0.38.2")
    );
}

#[tokio::test]
async fn proxy_auth_resolver_uses_default_managed_account_when_binding_has_no_account_id() {
    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::ClaudeDesktop);
    let state = std::sync::Arc::new(AppState::new_for_tests(config).expect("state"));
    state
        .db
        .upsert_managed_auth_account(ManagedAuthAccountInput {
            provider: ManagedAuthProvider::CodexOauth,
            id: Some("codex-old".to_string()),
            label: "Codex Old".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            make_default: false,
            tokens: ManagedAuthTokenSet {
                access_token: "old-token".to_string(),
                refresh_token: None,
                expires_at: None,
                scope: None,
                token_type: Some("Bearer".to_string()),
            },
        })
        .expect("insert old account");
    state
        .db
        .upsert_managed_auth_account(ManagedAuthAccountInput {
            provider: ManagedAuthProvider::CodexOauth,
            id: Some("codex-default".to_string()),
            label: "Codex Default".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            make_default: true,
            tokens: ManagedAuthTokenSet {
                access_token: "default-codex-token".to_string(),
                refresh_token: None,
                expires_at: None,
                scope: None,
                token_type: Some("Bearer".to_string()),
            },
        })
        .expect("insert default account");

    let provider = Provider {
        id: "codex-oauth".to_string(),
        name: "Codex OAuth".to_string(),
        settings_config: json!({}),
        website_url: None,
        category: None,
        created_at: None,
        sort_index: None,
        notes: None,
        meta: Some(ProviderMeta {
            provider_type: Some("codex_oauth".to_string()),
            auth_binding: Some(ProviderAuthBinding {
                mode: "managed".to_string(),
                provider_type: Some("codex_oauth".to_string()),
                account_id: None,
                use_default: Some(true),
            }),
            ..ProviderMeta::default()
        }),
    };

    let auth = resolve_auth_for_provider(
        &state,
        &AppType::ClaudeDesktop,
        &provider,
        adapter_for(&AppType::ClaudeDesktop),
    )
    .await
    .expect("resolve auth")
    .expect("auth");
    assert_eq!(auth.api_key, "default-codex-token");
    assert_eq!(auth.provider_type.as_deref(), Some("codex_oauth"));
}

#[tokio::test]
async fn proxy_auth_headers_use_bearer_for_codex_oauth_under_claude_desktop() {
    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::ClaudeDesktop);
    let state = std::sync::Arc::new(AppState::new_for_tests(config).expect("state"));
    state
        .db
        .upsert_managed_auth_account(ManagedAuthAccountInput {
            provider: ManagedAuthProvider::CodexOauth,
            id: Some("codex-default".to_string()),
            label: "Codex Default".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            make_default: true,
            tokens: ManagedAuthTokenSet {
                access_token: "codex-access-token".to_string(),
                refresh_token: None,
                expires_at: None,
                scope: None,
                token_type: Some("Bearer".to_string()),
            },
        })
        .expect("insert default account");

    let provider = Provider {
        id: "codex-oauth".to_string(),
        name: "Codex OAuth".to_string(),
        settings_config: json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://chatgpt.com/backend-api/codex"
            }
        }),
        website_url: None,
        category: None,
        created_at: None,
        sort_index: None,
        notes: None,
        meta: Some(ProviderMeta {
            provider_type: Some("codex_oauth".to_string()),
            auth_binding: Some(ProviderAuthBinding {
                mode: "managed".to_string(),
                provider_type: Some("codex_oauth".to_string()),
                account_id: None,
                use_default: Some(true),
            }),
            ..ProviderMeta::default()
        }),
    };
    let adapter = adapter_for(&AppType::ClaudeDesktop);
    let auth = resolve_auth_for_provider(&state, &AppType::ClaudeDesktop, &provider, adapter)
        .await
        .expect("resolve auth")
        .expect("auth");
    let mut headers = HeaderMap::new();
    insert_auth_headers(&mut headers, adapter, &auth);

    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer codex-access-token")
    );
    assert!(headers.get("x-api-key").is_none());
}

#[tokio::test]
async fn proxy_auth_resolver_rejects_expired_managed_account_without_refresh_source() {
    let state =
        std::sync::Arc::new(AppState::new_for_tests(MultiAppConfig::default()).expect("state"));
    state
        .db
        .upsert_managed_auth_account(ManagedAuthAccountInput {
            provider: ManagedAuthProvider::CodexOauth,
            id: Some("codex-expired".to_string()),
            label: "Codex Expired".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            make_default: true,
            tokens: ManagedAuthTokenSet {
                access_token: "expired-token".to_string(),
                refresh_token: None,
                expires_at: Some(chrono::Utc::now() - chrono::Duration::minutes(5)),
                scope: None,
                token_type: Some("Bearer".to_string()),
            },
        })
        .expect("insert expired account");

    let provider = Provider {
        id: "codex-oauth".to_string(),
        name: "Codex OAuth".to_string(),
        settings_config: json!({}),
        website_url: None,
        category: None,
        created_at: None,
        sort_index: None,
        notes: None,
        meta: Some(ProviderMeta {
            provider_type: Some("codex_oauth".to_string()),
            auth_binding: Some(ProviderAuthBinding {
                mode: "managed".to_string(),
                provider_type: Some("codex_oauth".to_string()),
                account_id: Some("codex-expired".to_string()),
                use_default: Some(false),
            }),
            ..ProviderMeta::default()
        }),
    };

    let err = resolve_auth_for_provider(
        &state,
        &AppType::ClaudeDesktop,
        &provider,
        adapter_for(&AppType::ClaudeDesktop),
    )
    .await
    .expect_err("expired account without refresh token should fail");
    assert!(err.to_string().contains("has no refresh token"));
}

#[tokio::test]
async fn proxy_auth_headers_use_bearer_for_manual_openai_desktop_provider() {
    let state =
        std::sync::Arc::new(AppState::new_for_tests(MultiAppConfig::default()).expect("state"));
    let provider = Provider {
        id: "nvidia-openai".to_string(),
        name: "Nvidia OpenAI".to_string(),
        settings_config: json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://integrate.api.nvidia.com",
                "ANTHROPIC_AUTH_TOKEN": "manual-openai-token"
            }
        }),
        website_url: None,
        category: None,
        created_at: None,
        sort_index: None,
        notes: None,
        meta: Some(ProviderMeta {
            api_format: Some("OpenAI Chat".to_string()),
            ..ProviderMeta::default()
        }),
    };
    let adapter = adapter_for(&AppType::ClaudeDesktop);
    let auth = resolve_auth_for_provider(&state, &AppType::ClaudeDesktop, &provider, adapter)
        .await
        .expect("resolve auth")
        .expect("auth");
    let mut headers = HeaderMap::new();
    insert_auth_headers(&mut headers, adapter, &auth);

    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer manual-openai-token")
    );
    assert!(headers.get("x-api-key").is_none());
}

#[tokio::test]
async fn proxy_auth_resolver_keeps_manual_api_key_mode() {
    let state =
        std::sync::Arc::new(AppState::new_for_tests(MultiAppConfig::default()).expect("state"));
    let provider = Provider {
        id: "copilot".to_string(),
        name: "Copilot".to_string(),
        settings_config: json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com",
                "ANTHROPIC_AUTH_TOKEN": "manual-token"
            }
        }),
        website_url: None,
        category: None,
        created_at: None,
        sort_index: None,
        notes: None,
        meta: Some(ProviderMeta {
            provider_type: Some("github_copilot".to_string()),
            auth_binding: Some(ProviderAuthBinding {
                mode: " api-key ".to_string(),
                provider_type: Some("github_copilot".to_string()),
                account_id: None,
                use_default: None,
            }),
            ..ProviderMeta::default()
        }),
    };

    let auth = resolve_auth_for_provider(
        &state,
        &AppType::Claude,
        &provider,
        adapter_for(&AppType::Claude),
    )
    .await
    .expect("resolve auth")
    .expect("auth");
    assert_eq!(auth.api_key, "manual-token");
}

#[tokio::test]
async fn proxy_auth_resolver_keeps_legacy_oauth_provider_with_manual_key() {
    let state =
        std::sync::Arc::new(AppState::new_for_tests(MultiAppConfig::default()).expect("state"));
    let provider = Provider {
        id: "legacy-copilot".to_string(),
        name: "Legacy Copilot".to_string(),
        settings_config: json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com",
                "ANTHROPIC_AUTH_TOKEN": "legacy-manual-token"
            }
        }),
        website_url: None,
        category: None,
        created_at: None,
        sort_index: None,
        notes: None,
        meta: Some(ProviderMeta {
            provider_type: Some("github_copilot".to_string()),
            ..ProviderMeta::default()
        }),
    };

    let auth = resolve_auth_for_provider(
        &state,
        &AppType::Claude,
        &provider,
        adapter_for(&AppType::Claude),
    )
    .await
    .expect("resolve auth")
    .expect("auth");

    assert_eq!(auth.api_key, "legacy-manual-token");
}

#[tokio::test]
async fn proxy_auth_resolver_rejects_invalid_managed_provider_type() {
    let state =
        std::sync::Arc::new(AppState::new_for_tests(MultiAppConfig::default()).expect("state"));
    let provider = Provider {
        id: "bad-managed".to_string(),
        name: "Bad Managed".to_string(),
        settings_config: json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.example.com",
                "ANTHROPIC_AUTH_TOKEN": "manual-token"
            }
        }),
        website_url: None,
        category: None,
        created_at: None,
        sort_index: None,
        notes: None,
        meta: Some(ProviderMeta {
            auth_binding: Some(ProviderAuthBinding {
                mode: " Managed ".to_string(),
                provider_type: Some("not_supported".to_string()),
                account_id: None,
                use_default: Some(true),
            }),
            ..ProviderMeta::default()
        }),
    };

    let err = resolve_auth_for_provider(
        &state,
        &AppType::Claude,
        &provider,
        adapter_for(&AppType::Claude),
    )
    .await
    .expect_err("invalid provider type should fail");
    assert!(err.to_string().contains("Unsupported managed providerType"));
}

#[tokio::test]
async fn proxy_auth_resolver_rejects_logged_out_managed_account() {
    let state =
        std::sync::Arc::new(AppState::new_for_tests(MultiAppConfig::default()).expect("state"));
    state
        .db
        .upsert_managed_auth_account(ManagedAuthAccountInput {
            provider: ManagedAuthProvider::GithubCopilot,
            id: Some("github-logged-out".to_string()),
            label: "GitHub Logged Out".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            make_default: true,
            tokens: ManagedAuthTokenSet {
                access_token: "token-before-logout".to_string(),
                refresh_token: None,
                expires_at: None,
                scope: None,
                token_type: Some("Bearer".to_string()),
            },
        })
        .expect("insert account");
    state
        .db
        .logout_managed_auth_account(ManagedAuthProvider::GithubCopilot, "github-logged-out")
        .expect("logout account");

    let provider = Provider {
        id: "logged-out-managed".to_string(),
        name: "Logged Out Managed".to_string(),
        settings_config: json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com",
                "ANTHROPIC_AUTH_TOKEN": "placeholder"
            }
        }),
        website_url: None,
        category: None,
        created_at: None,
        sort_index: None,
        notes: None,
        meta: Some(ProviderMeta {
            auth_binding: Some(ProviderAuthBinding {
                mode: "managed".to_string(),
                provider_type: Some("github_copilot".to_string()),
                account_id: Some("github-logged-out".to_string()),
                use_default: Some(false),
            }),
            ..ProviderMeta::default()
        }),
    };

    let err = resolve_auth_for_provider(
        &state,
        &AppType::Claude,
        &provider,
        adapter_for(&AppType::Claude),
    )
    .await
    .expect_err("logged out managed account should fail");
    assert!(err.to_string().contains("logged out"));
}

#[tokio::test]
async fn proxy_auth_resolver_rejects_specific_managed_binding_without_account_id() {
    let state =
        std::sync::Arc::new(AppState::new_for_tests(MultiAppConfig::default()).expect("state"));
    state
        .db
        .upsert_managed_auth_account(ManagedAuthAccountInput {
            provider: ManagedAuthProvider::GithubCopilot,
            id: Some("github-default".to_string()),
            label: "GitHub Default".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            make_default: true,
            tokens: ManagedAuthTokenSet {
                access_token: "default-token".to_string(),
                refresh_token: None,
                expires_at: None,
                scope: None,
                token_type: Some("Bearer".to_string()),
            },
        })
        .expect("insert default account");

    let provider = Provider {
        id: "bad-specific-managed".to_string(),
        name: "Bad Specific Managed".to_string(),
        settings_config: json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com",
                "ANTHROPIC_AUTH_TOKEN": "placeholder"
            }
        }),
        website_url: None,
        category: None,
        created_at: None,
        sort_index: None,
        notes: None,
        meta: Some(ProviderMeta {
            auth_binding: Some(ProviderAuthBinding {
                mode: "managed".to_string(),
                provider_type: Some("github_copilot".to_string()),
                account_id: Some("  ".to_string()),
                use_default: Some(false),
            }),
            ..ProviderMeta::default()
        }),
    };

    let err = resolve_auth_for_provider(
        &state,
        &AppType::Claude,
        &provider,
        adapter_for(&AppType::Claude),
    )
    .await
    .expect_err("specific managed binding without account id should fail");
    assert!(err.to_string().contains("requires accountId"));
}

fn tamper_ciphertext(value: &str) -> String {
    let mut parts = value.split(':');
    let prefix = parts.next().expect("prefix");
    let nonce = parts.next().expect("nonce");
    let ciphertext = parts.next().expect("ciphertext");
    assert_eq!(parts.next(), None);

    let mut ciphertext = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(ciphertext)
        .expect("decode ciphertext");
    ciphertext[0] ^= 0x01;

    format!(
        "{prefix}:{nonce}:{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(ciphertext)
    )
}
