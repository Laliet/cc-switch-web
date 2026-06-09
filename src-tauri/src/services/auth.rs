use std::{collections::HashMap, sync::Arc, time::Duration};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{
        ManagedAuthAccount, ManagedAuthAccountInput, ManagedAuthDevicePoll,
        ManagedAuthDevicePollResult, ManagedAuthDeviceSession, ManagedAuthDeviceStart,
        ManagedAuthProvider, ManagedAuthTokenSet, ManagedAuthUsage,
    },
    error::AppError,
    store::AppState,
};

pub struct AuthService;
pub struct CopilotAuthManager;
pub struct CodexOAuthManager;

const GITHUB_COPILOT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const GITHUB_COPILOT_DEVICE_SCOPE: &str = "read:user";
const GITHUB_API_BASE: &str = "https://api.github.com";
pub(crate) const GITHUB_COPILOT_API_BASE: &str = "https://api.githubcopilot.com";
pub(crate) const GITHUB_COPILOT_EDITOR_VERSION: &str = "vscode/1.110.1";
pub(crate) const GITHUB_COPILOT_PLUGIN_VERSION: &str = "copilot-chat/0.38.2";
pub(crate) const GITHUB_COPILOT_USER_AGENT: &str = "GitHubCopilotChat/0.38.2";
pub(crate) const GITHUB_COPILOT_API_VERSION: &str = "2025-10-01";
pub(crate) const GITHUB_COPILOT_INTEGRATION_ID: &str = "vscode-chat";
const CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CODEX_OAUTH_ISSUER: &str = "https://auth.openai.com";
const CODEX_OAUTH_USER_AGENT: &str = "cc-switch-codex-oauth";
const CODEX_OAUTH_DEVICE_INTERVAL_SECS: u64 = 5;
const CODEX_OAUTH_POLLING_SAFETY_MARGIN_SECS: u64 = 3;
const AUTH_HTTP_TIMEOUT_SECS: u64 = 20;
const AUTH_REFRESH_SKEW_SECS: i64 = 60;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexDeviceSessionPayload {
    device_auth_id: String,
    user_code: String,
}

#[derive(Debug, Deserialize)]
struct CodexDeviceCodeSuccessResponse {
    #[serde(alias = "code")]
    authorization_code: String,
    #[serde(rename = "code_challenge", alias = "codeChallenge")]
    _code_challenge: Option<String>,
    #[serde(alias = "codeVerifier")]
    code_verifier: Option<String>,
}

impl AuthService {
    pub fn list_accounts(
        state: &Arc<AppState>,
        provider: Option<ManagedAuthProvider>,
    ) -> Result<Vec<ManagedAuthAccount>, AppError> {
        state.db.list_managed_auth_accounts(provider)
    }

    pub fn import_account(
        state: &Arc<AppState>,
        input: ManagedAuthAccountInput,
    ) -> Result<ManagedAuthAccount, AppError> {
        state.db.upsert_managed_auth_account(input)
    }

    pub fn set_default(
        state: &Arc<AppState>,
        provider: ManagedAuthProvider,
        account_id: &str,
    ) -> Result<(), AppError> {
        state
            .db
            .set_default_managed_auth_account(provider, account_id)
    }

    pub fn delete_account(
        state: &Arc<AppState>,
        provider: ManagedAuthProvider,
        account_id: &str,
    ) -> Result<bool, AppError> {
        state.db.delete_managed_auth_account(provider, account_id)
    }

    pub fn logout_account(
        state: &Arc<AppState>,
        provider: ManagedAuthProvider,
        account_id: &str,
    ) -> Result<bool, AppError> {
        state.db.logout_managed_auth_account(provider, account_id)
    }

    pub async fn resolve_token(
        state: &Arc<AppState>,
        provider: ManagedAuthProvider,
        account_id: Option<&str>,
    ) -> Result<(ManagedAuthAccount, ManagedAuthTokenSet), AppError> {
        let mut secret = if let Some(account_id) = account_id.filter(|v| !v.trim().is_empty()) {
            state
                .db
                .get_managed_auth_account(provider, account_id.trim())?
        } else {
            state.db.get_default_managed_auth_account(provider)?
        }
        .ok_or_else(|| {
            AppError::localized(
                "auth.account_missing",
                format!(
                    "缺少 {} 托管账号，请先在认证中心登录或导入 token。",
                    provider.as_str()
                ),
                format!(
                    "Missing {} managed account. Sign in or import a token in Auth Center first.",
                    provider.as_str()
                ),
            )
        })?;

        if secret
            .account
            .status
            .as_deref()
            .map(str::trim)
            .is_some_and(|status| status.eq_ignore_ascii_case("logged_out"))
        {
            return Err(AppError::Unauthorized(format!(
                "{} managed account '{}' is logged out",
                provider.as_str(),
                secret.account.id
            )));
        }
        if secret.tokens.access_token.trim().is_empty() {
            return Err(AppError::Unauthorized(format!(
                "{} managed account token is empty",
                provider.as_str()
            )));
        }
        if token_needs_refresh(secret.tokens.expires_at, Utc::now()) {
            secret = refresh_managed_token(state, secret.account, secret.tokens).await?;
        }

        state
            .db
            .mark_managed_auth_account_used(provider, &secret.account.id)?;
        Ok((secret.account, secret.tokens))
    }

    pub async fn start_device_login(
        _state: &Arc<AppState>,
        request: ManagedAuthDeviceStart,
    ) -> Result<ManagedAuthDeviceSession, AppError> {
        match request.provider {
            ManagedAuthProvider::GithubCopilot => CopilotAuthManager::start_device_login().await,
            ManagedAuthProvider::CodexOauth => CodexOAuthManager::start_device_login().await,
        }
    }

    pub async fn poll_device_login(
        state: &Arc<AppState>,
        request: ManagedAuthDevicePoll,
    ) -> Result<ManagedAuthDevicePollResult, AppError> {
        match request.provider {
            ManagedAuthProvider::GithubCopilot => {
                CopilotAuthManager::poll_device_login(state, &request.session_id).await
            }
            ManagedAuthProvider::CodexOauth => {
                CodexOAuthManager::poll_device_login(state, &request.session_id).await
            }
        }
    }

    pub fn account_counts(state: &Arc<AppState>) -> Result<HashMap<String, usize>, AppError> {
        let mut counts = HashMap::new();
        for provider in [
            ManagedAuthProvider::GithubCopilot,
            ManagedAuthProvider::CodexOauth,
        ] {
            counts.insert(
                provider.as_str().to_string(),
                state.db.list_managed_auth_accounts(Some(provider))?.len(),
            );
        }
        Ok(counts)
    }

    pub async fn query_usage(
        state: &Arc<AppState>,
        provider: ManagedAuthProvider,
        account_id: Option<&str>,
    ) -> Result<ManagedAuthUsage, AppError> {
        match provider {
            ManagedAuthProvider::GithubCopilot => {
                CopilotAuthManager::query_usage(state, account_id).await
            }
            ManagedAuthProvider::CodexOauth => {
                CodexOAuthManager::query_usage(state, account_id).await
            }
        }
    }
}

impl CopilotAuthManager {
    pub async fn resolve_token(
        state: &Arc<AppState>,
        account_id: Option<&str>,
    ) -> Result<(ManagedAuthAccount, ManagedAuthTokenSet), AppError> {
        AuthService::resolve_token(state, ManagedAuthProvider::GithubCopilot, account_id).await
    }

    pub async fn start_device_login() -> Result<ManagedAuthDeviceSession, AppError> {
        start_github_copilot_device_login().await
    }

    pub async fn poll_device_login(
        state: &Arc<AppState>,
        device_code: &str,
    ) -> Result<ManagedAuthDevicePollResult, AppError> {
        poll_github_copilot_device_login(state, device_code).await
    }

    pub async fn query_usage(
        state: &Arc<AppState>,
        account_id: Option<&str>,
    ) -> Result<ManagedAuthUsage, AppError> {
        query_github_copilot_usage(state, account_id).await
    }
}

impl CodexOAuthManager {
    pub async fn resolve_token(
        state: &Arc<AppState>,
        account_id: Option<&str>,
    ) -> Result<(ManagedAuthAccount, ManagedAuthTokenSet), AppError> {
        AuthService::resolve_token(state, ManagedAuthProvider::CodexOauth, account_id).await
    }

    pub async fn start_device_login() -> Result<ManagedAuthDeviceSession, AppError> {
        start_codex_oauth_device_login().await
    }

    pub async fn poll_device_login(
        state: &Arc<AppState>,
        session_id: &str,
    ) -> Result<ManagedAuthDevicePollResult, AppError> {
        poll_codex_oauth_device_login(state, session_id).await
    }

    pub async fn query_usage(
        state: &Arc<AppState>,
        account_id: Option<&str>,
    ) -> Result<ManagedAuthUsage, AppError> {
        query_codex_oauth_usage(state, account_id).await
    }
}

async fn query_github_copilot_usage(
    state: &Arc<AppState>,
    account_id: Option<&str>,
) -> Result<ManagedAuthUsage, AppError> {
    let (account, tokens) =
        AuthService::resolve_token(state, ManagedAuthProvider::GithubCopilot, account_id).await?;
    let client = auth_client()?;
    let github_token = github_token_source(&tokens)?;
    let raw = fetch_github_copilot_user(&client, github_token).await?;
    Ok(usage_from_raw(
        ManagedAuthProvider::GithubCopilot,
        account,
        raw,
        &[
            &["sku"],
            &["plan"],
            &["copilot_plan"],
            &["access_type_sku"],
            &["assigned_date"],
        ],
    ))
}

async fn query_codex_oauth_usage(
    state: &Arc<AppState>,
    account_id: Option<&str>,
) -> Result<ManagedAuthUsage, AppError> {
    let (account, tokens) =
        AuthService::resolve_token(state, ManagedAuthProvider::CodexOauth, account_id).await?;
    let client = auth_client()?;
    let raw = fetch_codex_oauth_usage(&client, &tokens.access_token).await?;
    Ok(usage_from_raw(
        ManagedAuthProvider::CodexOauth,
        account,
        raw,
        &[
            &["plan"],
            &["plan_type"],
            &["account", "plan"],
            &["subscription", "plan"],
            &["subscription", "plan_name"],
            &["workspace", "plan"],
        ],
    ))
}

fn usage_from_raw(
    provider: ManagedAuthProvider,
    account: ManagedAuthAccount,
    raw: serde_json::Value,
    plan_paths: &[&[&str]],
) -> ManagedAuthUsage {
    let quota_root = find_quota_root(&raw).unwrap_or(&raw);
    let used_percent = number_at_any_path(
        quota_root,
        &[
            &["primary_window", "used_percent"],
            &["primaryWindow", "usedPercent"],
            &["secondary_window", "used_percent"],
            &["secondaryWindow", "usedPercent"],
            &["rate_limit", "primary_window", "used_percent"],
            &["rateLimit", "primaryWindow", "usedPercent"],
        ],
    );
    let explicit_total = number_at_any_path(
        quota_root,
        &[
            &["total"],
            &["quota"],
            &["limit"],
            &["quota_limit"],
            &["quotaLimit"],
            &["premium_requests"],
            &["premium_requests_limit"],
            &["premiumRequests"],
            &["premiumRequestsLimit"],
            &["messages"],
            &["messages_limit"],
            &["message_limit"],
            &["messagesLimit"],
            &["messageLimit"],
            &["requests"],
            &["requests_limit"],
            &["requestsLimit"],
            &["cap"],
            &["usage_cap"],
            &["usageCap"],
            &["chat", "limit"],
            &["chat", "message_limit"],
            &["chat", "messages_limit"],
            &["chat", "messageLimit"],
            &["chat", "messagesLimit"],
            &["limited_user_quotas", "limit"],
            &["limited_user_quotas", "chat", "limit"],
            &["limitedUserQuotas", "limit"],
            &["limitedUserQuotas", "chat", "limit"],
        ],
    );
    let remaining = number_at_any_path(
        quota_root,
        &[
            &["remaining"],
            &["remaining_quota"],
            &["quota_remaining"],
            &["remainingQuota"],
            &["quotaRemaining"],
            &["premium_requests_remaining"],
            &["premiumRequestsRemaining"],
            &["messages_remaining"],
            &["remaining_messages"],
            &["messagesRemaining"],
            &["remainingMessages"],
            &["requests_remaining"],
            &["requestsRemaining"],
            &["remaining_requests"],
            &["remainingRequests"],
            &["chat", "remaining"],
            &["chat", "messages_remaining"],
            &["chat", "messagesRemaining"],
            &["limited_user_quotas", "remaining"],
            &["limited_user_quotas", "chat", "remaining"],
            &["limitedUserQuotas", "remaining"],
            &["limitedUserQuotas", "chat", "remaining"],
        ],
    )
    .or_else(|| used_percent.map(|used| (100.0 - used).max(0.0)));
    let used = number_at_any_path(
        quota_root,
        &[
            &["used"],
            &["used_quota"],
            &["quota_used"],
            &["usedQuota"],
            &["quotaUsed"],
            &["premium_requests_used"],
            &["premiumRequestsUsed"],
            &["messages_used"],
            &["used_messages"],
            &["messagesUsed"],
            &["usedMessages"],
            &["requests_used"],
            &["requestsUsed"],
            &["used_requests"],
            &["usedRequests"],
            &["chat", "used"],
            &["chat", "messages_used"],
            &["chat", "messagesUsed"],
            &["limited_user_quotas", "used"],
            &["limited_user_quotas", "chat", "used"],
            &["limitedUserQuotas", "used"],
            &["limitedUserQuotas", "chat", "used"],
        ],
    )
    .or_else(|| {
        explicit_total
            .zip(remaining)
            .map(|(total, remaining)| (total - remaining).max(0.0))
    })
    .or(used_percent);
    let remaining = remaining.or_else(|| {
        explicit_total
            .zip(used)
            .map(|(total, used)| (total - used).max(0.0))
    });
    let total = explicit_total
        .or_else(|| {
            remaining
                .zip(used)
                .map(|(remaining, used)| remaining + used)
        })
        .or_else(|| used_percent.map(|_| 100.0));
    let reset_at = datetime_at_any_path(
        quota_root,
        &[
            &["reset_at"],
            &["resets_at"],
            &["resetAt"],
            &["quota_reset_at"],
            &["usage_reset_at"],
            &["period_end"],
            &["billing_period_end"],
            &["chat", "reset_at"],
            &["primary_window", "reset_at"],
            &["primaryWindow", "resetAt"],
            &["secondary_window", "reset_at"],
            &["secondaryWindow", "resetAt"],
            &["rate_limit", "primary_window", "reset_at"],
            &["rateLimit", "primaryWindow", "resetAt"],
            &["limited_user_quotas", "reset_at"],
            &["limited_user_quotas", "chat", "reset_at"],
        ],
    );
    let plan = string_at_any_path(&raw, plan_paths);

    ManagedAuthUsage {
        provider,
        account_id: Some(account.id),
        plan: plan.or(account.plan),
        remaining,
        used,
        total,
        reset_at,
        raw: Some(raw),
    }
}

async fn fetch_github_copilot_user(
    client: &reqwest::Client,
    github_token: &str,
) -> Result<serde_json::Value, AppError> {
    let response = client
        .get(format!("{GITHUB_API_BASE}/copilot_internal/user"))
        .header("Authorization", format!("token {github_token}"))
        .header("content-type", "application/json")
        .header("editor-version", GITHUB_COPILOT_EDITOR_VERSION)
        .header("editor-plugin-version", GITHUB_COPILOT_PLUGIN_VERSION)
        .header("user-agent", GITHUB_COPILOT_USER_AGENT)
        .header("x-github-api-version", GITHUB_COPILOT_API_VERSION)
        .send()
        .await
        .map_err(|err| AppError::Config(format!("Failed to fetch GitHub Copilot usage: {err}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(AppError::Unauthorized(format!(
            "GitHub Copilot usage rejected token: {}",
            truncate_auth_body(&body)
        )));
    }
    if !status.is_success() {
        return Err(AppError::Config(format!(
            "Failed to fetch GitHub Copilot usage with HTTP {status}: {}",
            truncate_auth_body(&body)
        )));
    }

    serde_json::from_str(&body)
        .map_err(|e| AppError::Config(format!("Failed to parse Copilot usage: {e}")))
}

async fn fetch_codex_oauth_usage(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<serde_json::Value, AppError> {
    let candidates = [
        "https://chatgpt.com/backend-api/wham/usage",
        "https://chatgpt.com/backend-api/codex/usage",
        "https://chat.openai.com/backend-api/wham/usage",
    ];
    let mut last_err: Option<String> = None;
    for url in candidates {
        let response = match client
            .get(url)
            .bearer_auth(access_token)
            .header("accept", "application/json")
            .send()
            .await
        {
            Ok(response) => response,
            Err(err) => {
                last_err = Some(format!("Request failed: {err}"));
                continue;
            }
        };
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            last_err = Some(format!("HTTP {status}: {}", truncate_auth_body(&body)));
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                return Err(AppError::Unauthorized(last_err.unwrap_or_default()));
            }
            continue;
        }
        return serde_json::from_str(&body)
            .map_err(|e| AppError::Config(format!("Failed to parse Codex OAuth usage: {e}")));
    }

    Err(AppError::Config(format!(
        "Failed to fetch Codex OAuth usage: {}",
        last_err.unwrap_or_else(|| "no candidate endpoints".to_string())
    )))
}

async fn refresh_managed_token(
    state: &Arc<AppState>,
    account: ManagedAuthAccount,
    tokens: ManagedAuthTokenSet,
) -> Result<crate::auth::ManagedAuthAccountSecret, AppError> {
    match account.provider {
        ManagedAuthProvider::GithubCopilot => {
            let github_token = tokens
                .refresh_token
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    AppError::Unauthorized(
                        "GitHub Copilot token expired and has no GitHub refresh source".to_string(),
                    )
                })?;
            let client = auth_client()?;
            let copilot = exchange_github_token_for_copilot(&client, github_token).await?;
            let refreshed = AuthService::import_account(
                state,
                ManagedAuthAccountInput {
                    provider: account.provider,
                    id: Some(account.id),
                    label: account.label,
                    username: account.username,
                    avatar_url: account.avatar_url,
                    plan: account.plan,
                    make_default: account.is_default,
                    tokens: ManagedAuthTokenSet {
                        access_token: copilot.token,
                        refresh_token: Some(github_token.to_string()),
                        expires_at: copilot.expires_at,
                        scope: tokens.scope,
                        token_type: tokens.token_type.or_else(|| Some("Bearer".to_string())),
                    },
                },
            )?;
            state
                .db
                .get_managed_auth_account(ManagedAuthProvider::GithubCopilot, &refreshed.id)?
                .ok_or_else(|| {
                    AppError::Database(
                        "GitHub Copilot account was not found after refresh".to_string(),
                    )
                })
        }
        ManagedAuthProvider::CodexOauth => {
            let refresh_token = tokens
                .refresh_token
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    AppError::Unauthorized(
                        "Codex OAuth token expired and has no refresh token".to_string(),
                    )
                })?;
            let client = auth_client()?;
            let refreshed_tokens = refresh_codex_oauth_tokens(&client, refresh_token).await?;
            let refreshed = AuthService::import_account(
                state,
                ManagedAuthAccountInput {
                    provider: account.provider,
                    id: Some(account.id),
                    label: account.label,
                    username: account.username,
                    avatar_url: account.avatar_url,
                    plan: account.plan,
                    make_default: account.is_default,
                    tokens: ManagedAuthTokenSet {
                        access_token: refreshed_tokens.access_token,
                        refresh_token: Some(refreshed_tokens.refresh_token),
                        expires_at: refreshed_tokens.expires_at,
                        scope: tokens.scope,
                        token_type: tokens.token_type.or_else(|| Some("Bearer".to_string())),
                    },
                },
            )?;
            state
                .db
                .get_managed_auth_account(ManagedAuthProvider::CodexOauth, &refreshed.id)?
                .ok_or_else(|| {
                    AppError::Database(
                        "Codex OAuth account was not found after refresh".to_string(),
                    )
                })
        }
    }
}

async fn start_codex_oauth_device_login() -> Result<ManagedAuthDeviceSession, AppError> {
    #[derive(Debug, Deserialize)]
    struct UserCodeResponse {
        device_auth_id: String,
        #[serde(alias = "user_code", alias = "usercode")]
        user_code: String,
        #[serde(default, deserialize_with = "deserialize_optional_interval")]
        interval: Option<u64>,
        #[serde(default, deserialize_with = "deserialize_optional_i64")]
        expires_in: Option<i64>,
    }

    let client = auth_client()?;
    let response = client
        .post(format!(
            "{}/api/accounts/deviceauth/usercode",
            CODEX_OAUTH_ISSUER
        ))
        .header("user-agent", CODEX_OAUTH_USER_AGENT)
        .json(&serde_json::json!({
            "client_id": CODEX_OAUTH_CLIENT_ID,
        }))
        .send()
        .await
        .map_err(|e| AppError::Config(format!("Codex OAuth device request failed: {e}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::Config(format!(
            "Codex OAuth device request failed with HTTP {status}: {}",
            truncate_auth_body(&body)
        )));
    }
    let payload: UserCodeResponse = serde_json::from_str(&body)
        .map_err(|e| AppError::Config(format!("Failed to parse Codex device login: {e}")))?;
    let session = CodexDeviceSessionPayload {
        device_auth_id: payload.device_auth_id,
        user_code: payload.user_code.clone(),
    };
    let session_id = serde_json::to_string(&session)
        .map_err(|e| AppError::Config(format!("Failed to encode Codex device session: {e}")))?;

    Ok(ManagedAuthDeviceSession {
        provider: ManagedAuthProvider::CodexOauth,
        session_id,
        user_code: payload.user_code,
        verification_uri: format!("{CODEX_OAUTH_ISSUER}/codex/device"),
        verification_uri_complete: None,
        interval_seconds: payload
            .interval
            .unwrap_or(CODEX_OAUTH_DEVICE_INTERVAL_SECS)
            .max(1)
            + CODEX_OAUTH_POLLING_SAFETY_MARGIN_SECS,
        expires_at: codex_device_expires_at(payload.expires_in, Utc::now()),
    })
}

fn codex_device_expires_at(expires_in: Option<i64>, now: DateTime<Utc>) -> DateTime<Utc> {
    let seconds = expires_in.filter(|seconds| *seconds > 0).unwrap_or(15 * 60);
    now + chrono::Duration::seconds(seconds)
}

async fn poll_codex_oauth_device_login(
    state: &Arc<AppState>,
    session_id: &str,
) -> Result<ManagedAuthDevicePollResult, AppError> {
    let session: CodexDeviceSessionPayload = serde_json::from_str(session_id)
        .map_err(|e| AppError::InvalidInput(format!("Invalid Codex device session: {e}")))?;
    let client = auth_client()?;
    let response = client
        .post(format!(
            "{}/api/accounts/deviceauth/token",
            CODEX_OAUTH_ISSUER
        ))
        .header("user-agent", CODEX_OAUTH_USER_AGENT)
        .json(&serde_json::json!({
            "device_auth_id": session.device_auth_id,
            "user_code": session.user_code,
        }))
        .send()
        .await
        .map_err(|e| AppError::Config(format!("Codex OAuth device poll failed: {e}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if let Some(result) = codex_device_poll_error_result(status, &body) {
        return Ok(result);
    }
    if !status.is_success() {
        return Ok(ManagedAuthDevicePollResult {
            status: "error".to_string(),
            account: None,
            message: Some(format!(
                "Codex OAuth device poll failed with HTTP {status}: {}",
                truncate_auth_body(&body)
            )),
        });
    }

    let code_response: CodexDeviceCodeSuccessResponse = serde_json::from_str(&body)
        .map_err(|e| AppError::Config(format!("Failed to parse Codex device token: {e}")))?;
    let verifier = code_response.code_verifier.ok_or_else(|| {
        AppError::Config("Codex device token response did not include code_verifier".to_string())
    })?;
    let tokens =
        exchange_codex_code_for_tokens(&client, &code_response.authorization_code, &verifier)
            .await?;
    let account_id = codex_account_id_from_tokens(tokens.id_token.as_deref(), &tokens.access_token)
        .unwrap_or_else(|| format!("codex-oauth-{}", Utc::now().timestamp_millis()));
    let username = codex_username_from_id_token(tokens.id_token.as_deref());
    let label = username
        .clone()
        .map(|username| format!("Codex OAuth ({username})"))
        .unwrap_or_else(|| "Codex OAuth".to_string());

    let account = AuthService::import_account(
        state,
        ManagedAuthAccountInput {
            provider: ManagedAuthProvider::CodexOauth,
            id: Some(account_id),
            label,
            username,
            avatar_url: None,
            plan: Some("ChatGPT".to_string()),
            make_default: true,
            tokens: ManagedAuthTokenSet {
                access_token: tokens.access_token,
                refresh_token: Some(tokens.refresh_token),
                expires_at: tokens.expires_at,
                scope: Some("openid profile email offline_access".to_string()),
                token_type: Some("Bearer".to_string()),
            },
        },
    )?;

    Ok(ManagedAuthDevicePollResult {
        status: "authorized".to_string(),
        account: Some(account),
        message: None,
    })
}

fn codex_device_poll_error_result(
    status: StatusCode,
    body: &str,
) -> Option<ManagedAuthDevicePollResult> {
    let code = device_error_code(body);
    let poll_status = match code.as_deref() {
        Some("authorization_pending" | "pending" | "not_found") => Some("pending"),
        Some("slow_down") => Some("slow_down"),
        Some("expired_token" | "expired") => Some("expired"),
        Some("access_denied" | "denied") => Some("denied"),
        Some(_) => Some("error"),
        None if status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND => {
            Some("pending")
        }
        None => None,
    }?;
    Some(ManagedAuthDevicePollResult {
        status: poll_status.to_string(),
        account: None,
        message: if poll_status == "pending" {
            None
        } else {
            device_error_message(body).or(code)
        },
    })
}

fn device_error_code(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    if let Some(error) = value.get("error") {
        if let Some(value) = error
            .as_str()
            .or_else(|| error.get("code").and_then(serde_json::Value::as_str))
            .or_else(|| error.get("type").and_then(serde_json::Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_ascii_lowercase());
        }
    }
    for key in ["error", "error_code", "code", "status"] {
        if let Some(value) = value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_ascii_lowercase());
        }
    }
    None
}

fn device_error_message(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    if let Some(error) = value.get("error") {
        if let Some(value) = error
            .get("message")
            .or_else(|| error.get("error_description"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    for key in ["error_description", "message", "detail"] {
        if let Some(value) = value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    None
}

#[derive(Debug)]
struct CodexTokenSet {
    access_token: String,
    refresh_token: String,
    id_token: Option<String>,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct CodexTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    expires_in: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_auth_datetime")]
    expires_at: Option<DateTime<Utc>>,
}

async fn exchange_codex_code_for_tokens(
    client: &reqwest::Client,
    code: &str,
    code_verifier: &str,
) -> Result<CodexTokenSet, AppError> {
    let redirect_uri = format!("{CODEX_OAUTH_ISSUER}/deviceauth/callback");
    let body = form_body(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", &redirect_uri),
        ("client_id", CODEX_OAUTH_CLIENT_ID),
        ("code_verifier", code_verifier),
    ]);
    let response = client
        .post(format!("{CODEX_OAUTH_ISSUER}/oauth/token"))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("user-agent", CODEX_OAUTH_USER_AGENT)
        .body(body)
        .send()
        .await
        .map_err(|e| AppError::Config(format!("Codex OAuth token exchange failed: {e}")))?;
    parse_codex_token_response(response, None).await
}

async fn refresh_codex_oauth_tokens(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<CodexTokenSet, AppError> {
    let body = form_body(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", CODEX_OAUTH_CLIENT_ID),
        ("scope", "openid profile email"),
    ]);
    let response = client
        .post(format!("{CODEX_OAUTH_ISSUER}/oauth/token"))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("user-agent", CODEX_OAUTH_USER_AGENT)
        .body(body)
        .send()
        .await
        .map_err(|e| AppError::Config(format!("Codex OAuth refresh failed: {e}")))?;
    parse_codex_token_response(response, Some(refresh_token)).await
}

async fn parse_codex_token_response(
    response: reqwest::Response,
    fallback_refresh_token: Option<&str>,
) -> Result<CodexTokenSet, AppError> {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(AppError::Unauthorized(format!(
            "Codex OAuth token endpoint rejected credentials: {}",
            truncate_auth_body(&body)
        )));
    }
    if !status.is_success() {
        return Err(AppError::Config(format!(
            "Codex OAuth token endpoint failed with HTTP {status}: {}",
            truncate_auth_body(&body)
        )));
    }
    let payload: CodexTokenResponse = serde_json::from_str(&body)
        .map_err(|e| AppError::Config(format!("Failed to parse Codex OAuth tokens: {e}")))?;
    let refresh_token = payload
        .refresh_token
        .or_else(|| fallback_refresh_token.map(ToString::to_string))
        .ok_or_else(|| {
            AppError::Config("Codex OAuth token response did not include refresh_token".to_string())
        })?;
    let expires_at = payload.expires_at.or_else(|| {
        payload
            .expires_in
            .filter(|seconds| *seconds > 0)
            .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds))
    });
    Ok(CodexTokenSet {
        access_token: payload.access_token,
        refresh_token,
        id_token: payload.id_token,
        expires_at,
    })
}

fn form_body(params: &[(&str, &str)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in params {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

async fn start_github_copilot_device_login() -> Result<ManagedAuthDeviceSession, AppError> {
    #[derive(Debug, Deserialize)]
    struct DeviceCodeResponse {
        device_code: String,
        user_code: String,
        verification_uri: String,
        verification_uri_complete: Option<String>,
        expires_in: u64,
        interval: Option<u64>,
    }

    let client = auth_client()?;
    let response = client
        .post("https://github.com/login/device/code")
        .header("accept", "application/json")
        .header("user-agent", GITHUB_COPILOT_USER_AGENT)
        .form(&[
            ("client_id", GITHUB_COPILOT_CLIENT_ID),
            ("scope", GITHUB_COPILOT_DEVICE_SCOPE),
        ])
        .send()
        .await
        .map_err(|e| AppError::Config(format!("GitHub device login request failed: {e}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::Config(format!(
            "GitHub device login failed with HTTP {status}: {}",
            truncate_auth_body(&body)
        )));
    }
    let payload: DeviceCodeResponse = serde_json::from_str(&body)
        .map_err(|e| AppError::Config(format!("Failed to parse GitHub device login: {e}")))?;

    Ok(ManagedAuthDeviceSession {
        provider: ManagedAuthProvider::GithubCopilot,
        session_id: payload.device_code,
        user_code: payload.user_code,
        verification_uri: payload.verification_uri,
        verification_uri_complete: payload.verification_uri_complete,
        interval_seconds: payload.interval.unwrap_or(5).max(1),
        expires_at: Utc::now() + chrono::Duration::seconds(payload.expires_in as i64),
    })
}

async fn poll_github_copilot_device_login(
    state: &Arc<AppState>,
    device_code: &str,
) -> Result<ManagedAuthDevicePollResult, AppError> {
    #[derive(Debug, Deserialize)]
    struct AccessTokenResponse {
        access_token: Option<String>,
        token_type: Option<String>,
        scope: Option<String>,
        error: Option<String>,
        error_description: Option<String>,
    }

    let client = auth_client()?;
    let response = client
        .post("https://github.com/login/oauth/access_token")
        .header("accept", "application/json")
        .header("user-agent", GITHUB_COPILOT_USER_AGENT)
        .form(&[
            ("client_id", GITHUB_COPILOT_CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::Config(format!("GitHub device poll request failed: {e}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::Config(format!(
            "GitHub device poll failed with HTTP {status}: {}",
            truncate_auth_body(&body)
        )));
    }
    let payload: AccessTokenResponse = serde_json::from_str(&body)
        .map_err(|e| AppError::Config(format!("Failed to parse GitHub device poll: {e}")))?;

    if let Some(error) = payload.error.as_deref() {
        let status = match error {
            "authorization_pending" => "pending",
            "slow_down" => "slow_down",
            "expired_token" => "expired",
            "access_denied" => "denied",
            _ => "error",
        };
        return Ok(ManagedAuthDevicePollResult {
            status: status.to_string(),
            account: None,
            message: payload.error_description.or(payload.error),
        });
    }

    let github_token = payload.access_token.ok_or_else(|| {
        AppError::Config("GitHub device poll response did not include access_token".to_string())
    })?;
    let copilot = exchange_github_token_for_copilot(&client, &github_token).await?;
    let profile = fetch_github_profile(&client, &github_token).await.ok();
    let username = profile
        .as_ref()
        .and_then(|profile| profile.login.clone())
        .filter(|value| !value.trim().is_empty());
    let label = username
        .clone()
        .map(|login| format!("GitHub Copilot ({login})"))
        .unwrap_or_else(|| "GitHub Copilot".to_string());

    let account = AuthService::import_account(
        state,
        ManagedAuthAccountInput {
            provider: ManagedAuthProvider::GithubCopilot,
            id: username
                .as_ref()
                .map(|login| format!("github-copilot-{login}")),
            label,
            username,
            avatar_url: profile.and_then(|profile| profile.avatar_url),
            plan: None,
            make_default: true,
            tokens: ManagedAuthTokenSet {
                access_token: copilot.token,
                refresh_token: Some(github_token),
                expires_at: copilot.expires_at,
                scope: payload.scope,
                token_type: payload.token_type.or_else(|| Some("Bearer".to_string())),
            },
        },
    )?;

    Ok(ManagedAuthDevicePollResult {
        status: "authorized".to_string(),
        account: Some(account),
        message: None,
    })
}

#[derive(Debug, Deserialize)]
struct GithubCopilotTokenResponse {
    token: String,
    #[serde(default, deserialize_with = "deserialize_optional_auth_datetime")]
    expires_at: Option<DateTime<Utc>>,
}

async fn exchange_github_token_for_copilot(
    client: &reqwest::Client,
    github_token: &str,
) -> Result<GithubCopilotTokenResponse, AppError> {
    let response = client
        .get(format!("{GITHUB_API_BASE}/copilot_internal/v2/token"))
        .header("Authorization", format!("token {github_token}"))
        .header("accept", "application/json")
        .header("user-agent", GITHUB_COPILOT_USER_AGENT)
        .header("x-github-api-version", GITHUB_COPILOT_API_VERSION)
        .send()
        .await
        .map_err(|e| AppError::Config(format!("Copilot token exchange failed: {e}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(AppError::Unauthorized(format!(
            "GitHub token was rejected by Copilot: {}",
            truncate_auth_body(&body)
        )));
    }
    if !status.is_success() {
        return Err(AppError::Config(format!(
            "Copilot token exchange failed with HTTP {status}: {}",
            truncate_auth_body(&body)
        )));
    }
    serde_json::from_str(&body)
        .map_err(|e| AppError::Config(format!("Failed to parse Copilot token response: {e}")))
}

#[derive(Debug, Deserialize)]
struct GithubProfile {
    login: Option<String>,
    avatar_url: Option<String>,
}

async fn fetch_github_profile(
    client: &reqwest::Client,
    github_token: &str,
) -> Result<GithubProfile, AppError> {
    let response = client
        .get(format!("{GITHUB_API_BASE}/user"))
        .header("Authorization", format!("token {github_token}"))
        .header("accept", "application/json")
        .header("user-agent", GITHUB_COPILOT_USER_AGENT)
        .header("x-github-api-version", GITHUB_COPILOT_API_VERSION)
        .send()
        .await
        .map_err(|e| AppError::Config(format!("GitHub profile request failed: {e}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::Config(format!(
            "GitHub profile request failed with HTTP {status}: {}",
            truncate_auth_body(&body)
        )));
    }
    serde_json::from_str(&body)
        .map_err(|e| AppError::Config(format!("Failed to parse GitHub profile: {e}")))
}

fn auth_client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(AUTH_HTTP_TIMEOUT_SECS))
        .user_agent("cc-switch/0.17")
        .build()
        .map_err(|e| AppError::Config(format!("Failed to build HTTP client: {e}")))
}

fn token_needs_refresh(expires_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    expires_at.is_some_and(|expires_at| {
        expires_at <= now + chrono::Duration::seconds(AUTH_REFRESH_SKEW_SECS)
    })
}

fn truncate_auth_body(body: &str) -> String {
    const MAX_CHARS: usize = 512;
    let trimmed = body.trim();
    if trimmed.chars().count() <= MAX_CHARS {
        return trimmed.to_string();
    }
    let mut truncated = trimmed.chars().take(MAX_CHARS).collect::<String>();
    truncated.push_str("...");
    truncated
}

pub(crate) fn github_token_source(tokens: &ManagedAuthTokenSet) -> Result<&str, AppError> {
    tokens
        .refresh_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            tokens
                .access_token
                .trim()
                .starts_with("gh")
                .then_some(tokens.access_token.trim())
        })
        .ok_or_else(|| {
            AppError::Unauthorized(
                "GitHub Copilot account is missing the GitHub OAuth token".to_string(),
            )
        })
}

pub(crate) async fn fetch_github_copilot_api_endpoint(
    client: &reqwest::Client,
    github_token: &str,
) -> Result<String, AppError> {
    let usage = fetch_github_copilot_user(client, github_token).await?;
    Ok(usage
        .get("endpoints")
        .and_then(|value| value.get("api"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(GITHUB_COPILOT_API_BASE)
        .trim_end_matches('/')
        .to_string())
}

fn codex_account_id_from_tokens(id_token: Option<&str>, access_token: &str) -> Option<String> {
    extract_codex_identity_claim(id_token)
        .or_else(|| extract_codex_identity_claim(Some(access_token)))
}

fn extract_codex_identity_claim(token: Option<&str>) -> Option<String> {
    let claims = parse_jwt_claims(token?)?;
    if let Some(value) = claims
        .get("chatgpt_account_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_string());
    }
    if let Some(value) = claims
        .get("https://api.openai.com/auth")
        .and_then(|value| value.get("chatgpt_account_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_string());
    }
    if let Some(value) = claims
        .get("organizations")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first())
        .and_then(|value| value.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_string());
    }
    for key in ["sub", "email"] {
        if let Some(value) = claims
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(format!("codex-oauth-{}", sanitize_account_id(value)));
        }
    }
    None
}

fn codex_username_from_id_token(id_token: Option<&str>) -> Option<String> {
    let claims = parse_jwt_claims(id_token?)?;
    for key in ["email", "name", "preferred_username"] {
        if let Some(value) = claims
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    None
}

fn parse_jwt_claims(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.as_bytes()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn sanitize_account_id(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        Utc::now().timestamp_millis().to_string()
    } else {
        sanitized
    }
}

fn find_quota_root(value: &serde_json::Value) -> Option<&serde_json::Value> {
    match value {
        serde_json::Value::Object(map) => {
            for key in [
                "quota",
                "quotas",
                "usage",
                "premium_requests",
                "premiumRequests",
                "rate_limit",
                "rateLimit",
                "limited_user_quotas",
            ] {
                if let Some(child) = map.get(key) {
                    return Some(child);
                }
            }
            for child in map.values() {
                if let Some(found) = find_quota_root(child) {
                    return Some(found);
                }
            }
            None
        }
        serde_json::Value::Array(items) => items.iter().find_map(find_quota_root),
        _ => None,
    }
}

fn number_at_any_path(value: &serde_json::Value, paths: &[&[&str]]) -> Option<f64> {
    paths
        .iter()
        .find_map(|path| value_at_path(value, path).and_then(json_number))
}

fn string_at_any_path(value: &serde_json::Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        value_at_path(value, path)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

fn datetime_at_any_path(value: &serde_json::Value, paths: &[&[&str]]) -> Option<DateTime<Utc>> {
    paths.iter().find_map(|path| {
        let value = value_at_path(value, path)?;
        match value {
            serde_json::Value::Number(number) => number.as_i64().and_then(timestamp_to_utc),
            serde_json::Value::String(raw) => {
                if let Ok(timestamp) = raw.parse::<i64>() {
                    return timestamp_to_utc(timestamp);
                }
                DateTime::parse_from_rfc3339(raw)
                    .map(|value| value.with_timezone(&Utc))
                    .ok()
            }
            _ => None,
        }
    })
}

fn timestamp_to_utc(timestamp: i64) -> Option<DateTime<Utc>> {
    if timestamp.abs() >= 1_000_000_000_000 {
        return DateTime::<Utc>::from_timestamp_millis(timestamp);
    }
    DateTime::<Utc>::from_timestamp(timestamp, 0)
}

fn value_at_path<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn json_number(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(raw) => raw.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn deserialize_optional_interval<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        serde_json::Value::Number(number) => Ok(number.as_u64()),
        serde_json::Value::String(raw) => raw
            .trim()
            .parse::<u64>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        _ => Ok(None),
    }
}

fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        serde_json::Value::Number(number) => Ok(number.as_i64()),
        serde_json::Value::String(raw) => raw
            .trim()
            .parse::<i64>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        _ => Ok(None),
    }
}

fn deserialize_optional_auth_datetime<'de, D>(
    deserializer: D,
) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        serde_json::Value::Number(number) => {
            let Some(timestamp) = number.as_i64() else {
                return Ok(None);
            };
            timestamp_to_utc(timestamp)
                .ok_or_else(|| serde::de::Error::custom("invalid auth expiry timestamp"))
                .map(Some)
        }
        serde_json::Value::String(raw) => {
            if let Ok(timestamp) = raw.parse::<i64>() {
                return timestamp_to_utc(timestamp)
                    .ok_or_else(|| serde::de::Error::custom("invalid auth expiry timestamp"))
                    .map(Some);
            }
            DateTime::parse_from_rfc3339(&raw)
                .map(|value| Some(value.with_timezone(&Utc)))
                .map_err(serde::de::Error::custom)
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use chrono::Utc;

    use super::{
        codex_account_id_from_tokens, codex_device_expires_at, codex_device_poll_error_result,
        datetime_at_any_path, find_quota_root, number_at_any_path, token_needs_refresh,
        usage_from_raw, CodexDeviceCodeSuccessResponse, CodexDeviceSessionPayload,
        CodexTokenResponse, GithubCopilotTokenResponse,
    };

    #[test]
    fn copilot_token_expiry_accepts_epoch_timestamp() {
        let parsed: GithubCopilotTokenResponse = serde_json::from_value(serde_json::json!({
            "token": "copilot-token",
            "expires_at": 1_800_000_000
        }))
        .expect("parse token");

        assert_eq!(parsed.token, "copilot-token");
        assert!(parsed.expires_at.is_some());
    }

    #[test]
    fn copilot_token_expiry_accepts_epoch_millis_timestamp() {
        let seconds: GithubCopilotTokenResponse = serde_json::from_value(serde_json::json!({
            "token": "copilot-token",
            "expires_at": 1_800_000_000
        }))
        .expect("seconds response");
        let millis: GithubCopilotTokenResponse = serde_json::from_value(serde_json::json!({
            "token": "copilot-token",
            "expires_at": 1_800_000_000_000i64
        }))
        .expect("millis response");

        assert_eq!(millis.expires_at, seconds.expires_at);
    }

    #[test]
    fn quota_helpers_extract_nested_numeric_fields() {
        let raw = serde_json::json!({
            "user": {
                "limited_user_quotas": {
                    "chat": {
                        "remaining": "42",
                        "limit": 100,
                        "reset_at": "2026-06-08T12:00:00Z"
                    }
                }
            }
        });
        let quota = find_quota_root(&raw).expect("quota root");

        assert_eq!(
            number_at_any_path(quota, &[&["chat", "remaining"]]),
            Some(42.0)
        );
        assert_eq!(
            number_at_any_path(quota, &[&["chat", "limit"]]),
            Some(100.0)
        );
        assert!(datetime_at_any_path(quota, &[&["chat", "reset_at"]]).is_some());
    }

    #[test]
    fn quota_helpers_parse_second_and_millisecond_timestamps() {
        let raw = serde_json::json!({
            "seconds": 1_779_974_400,
            "millis": 1_779_974_400_000i64,
            "millis_string": "1779974400000"
        });

        let seconds = datetime_at_any_path(&raw, &[&["seconds"]]).expect("seconds timestamp");
        let millis = datetime_at_any_path(&raw, &[&["millis"]]).expect("millis timestamp");
        let millis_string =
            datetime_at_any_path(&raw, &[&["millis_string"]]).expect("millis string timestamp");

        assert_eq!(seconds, millis);
        assert_eq!(seconds, millis_string);
    }

    #[test]
    fn usage_from_raw_extracts_codex_shaped_quota_fields() {
        let raw = serde_json::json!({
            "plan_type": "plus",
            "rate_limit": {
                "primary_window": {
                    "used_percent": 19,
                    "reset_at": "2026-06-08T12:00:00Z",
                    "limit_window_seconds": 18000
                }
            }
        });
        let account = crate::auth::ManagedAuthAccount {
            id: "codex-1".to_string(),
            provider: crate::auth::ManagedAuthProvider::CodexOauth,
            label: "Codex".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            is_default: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: None,
            expires_at: None,
            scopes: None,
            status: None,
        };

        let usage = usage_from_raw(
            crate::auth::ManagedAuthProvider::CodexOauth,
            account,
            raw,
            &[&["plan_type"]],
        );

        assert_eq!(usage.provider, crate::auth::ManagedAuthProvider::CodexOauth);
        assert_eq!(usage.account_id.as_deref(), Some("codex-1"));
        assert_eq!(usage.plan.as_deref(), Some("plus"));
        assert_eq!(usage.remaining, Some(81.0));
        assert_eq!(usage.used, Some(19.0));
        assert_eq!(usage.total, Some(100.0));
        assert!(usage.reset_at.is_some());
    }

    #[test]
    fn usage_from_raw_derives_missing_usage_fields_from_camel_case_quota() {
        let raw = serde_json::json!({
            "subscription": {
                "plan_name": "team"
            },
            "usage": {
                "messagesLimit": 50,
                "messagesRemaining": 12,
                "resetAt": "2026-06-08T12:00:00Z"
            }
        });
        let account = crate::auth::ManagedAuthAccount {
            id: "codex-team".to_string(),
            provider: crate::auth::ManagedAuthProvider::CodexOauth,
            label: "Codex Team".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            is_default: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: None,
            expires_at: None,
            scopes: None,
            status: None,
        };

        let usage = usage_from_raw(
            crate::auth::ManagedAuthProvider::CodexOauth,
            account,
            raw,
            &[&["subscription", "plan_name"]],
        );

        assert_eq!(usage.plan.as_deref(), Some("team"));
        assert_eq!(usage.remaining, Some(12.0));
        assert_eq!(usage.used, Some(38.0));
        assert_eq!(usage.total, Some(50.0));
        assert!(usage.reset_at.is_some());
    }

    #[test]
    fn codex_device_session_roundtrips_device_payload() {
        let payload = CodexDeviceSessionPayload {
            device_auth_id: "device-1".to_string(),
            user_code: "ABCD-EFGH".to_string(),
        };
        let encoded = serde_json::to_string(&payload).expect("encode");
        let decoded: CodexDeviceSessionPayload = serde_json::from_str(&encoded).expect("decode");

        assert_eq!(decoded.device_auth_id, "device-1");
        assert_eq!(decoded.user_code, "ABCD-EFGH");
    }

    #[test]
    fn codex_device_success_response_accepts_field_aliases() {
        let snake_case: CodexDeviceCodeSuccessResponse =
            serde_json::from_value(serde_json::json!({
                "authorization_code": "auth-code-1",
                "code_verifier": "verifier-1",
                "code_challenge": "challenge-1"
            }))
            .expect("snake case response");
        let aliases: CodexDeviceCodeSuccessResponse = serde_json::from_value(serde_json::json!({
            "code": "auth-code-2",
            "codeVerifier": "verifier-2",
            "codeChallenge": "challenge-2"
        }))
        .expect("alias response");

        assert_eq!(snake_case.authorization_code, "auth-code-1");
        assert_eq!(snake_case.code_verifier.as_deref(), Some("verifier-1"));
        assert_eq!(aliases.authorization_code, "auth-code-2");
        assert_eq!(aliases.code_verifier.as_deref(), Some("verifier-2"));
    }

    #[test]
    fn codex_device_expiry_uses_server_expires_in_or_default() {
        let now = Utc::now();

        assert_eq!(
            codex_device_expires_at(Some(60), now),
            now + chrono::Duration::seconds(60)
        );
        assert_eq!(
            codex_device_expires_at(Some(0), now),
            now + chrono::Duration::minutes(15)
        );
        assert_eq!(
            codex_device_expires_at(None, now),
            now + chrono::Duration::minutes(15)
        );
    }

    #[test]
    fn codex_token_response_ignores_non_positive_expires_in() {
        let payload: CodexTokenResponse = serde_json::from_value(serde_json::json!({
            "access_token": "access",
            "refresh_token": "refresh",
            "expires_in": 0
        }))
        .expect("token response");

        let expires_at = payload.expires_at.or_else(|| {
            payload
                .expires_in
                .filter(|seconds| *seconds > 0)
                .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds))
        });

        assert!(expires_at.is_none());
    }

    #[test]
    fn codex_device_poll_error_result_maps_device_flow_errors() {
        let pending = codex_device_poll_error_result(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"error":"authorization_pending"}"#,
        )
        .expect("pending");
        let slow_down = codex_device_poll_error_result(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"error":"slow_down","error_description":"wait longer"}"#,
        )
        .expect("slow down");
        let denied = codex_device_poll_error_result(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"error":"access_denied","message":"denied by user"}"#,
        )
        .expect("denied");
        let nested = codex_device_poll_error_result(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"error":{"code":"slow_down","message":"nested wait longer"}}"#,
        )
        .expect("nested error");
        let not_found =
            codex_device_poll_error_result(reqwest::StatusCode::NOT_FOUND, "").expect("not found");

        assert_eq!(pending.status, "pending");
        assert!(pending.message.is_none());
        assert_eq!(slow_down.status, "slow_down");
        assert_eq!(slow_down.message.as_deref(), Some("wait longer"));
        assert_eq!(denied.status, "denied");
        assert_eq!(denied.message.as_deref(), Some("denied by user"));
        assert_eq!(nested.status, "slow_down");
        assert_eq!(nested.message.as_deref(), Some("nested wait longer"));
        assert_eq!(not_found.status, "pending");
    }

    #[test]
    fn codex_account_id_uses_jwt_claims() {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"email":"dev@example.com"}"#);
        let token = format!("{header}.{payload}.");

        assert_eq!(
            codex_account_id_from_tokens(Some(&token), "").as_deref(),
            Some("codex-oauth-dev-example-com")
        );
    }

    #[test]
    fn codex_token_expiry_accepts_string_expires_in() {
        let parsed: CodexTokenResponse = serde_json::from_value(serde_json::json!({
            "access_token": "codex-access",
            "refresh_token": "codex-refresh",
            "expires_in": "3600"
        }))
        .expect("parse token");

        assert_eq!(parsed.expires_in, Some(3600));
    }

    #[test]
    fn token_refresh_uses_short_expiry_skew() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-06-08T12:00:00Z")
            .expect("datetime")
            .with_timezone(&chrono::Utc);

        assert!(token_needs_refresh(
            Some(now + chrono::Duration::seconds(30)),
            now
        ));
        assert!(!token_needs_refresh(
            Some(now + chrono::Duration::seconds(120)),
            now
        ));
        assert!(!token_needs_refresh(None, now));
    }
}
