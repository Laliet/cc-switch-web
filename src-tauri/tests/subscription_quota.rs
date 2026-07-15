#![cfg(all(feature = "web-server", feature = "test-hooks"))]

use std::{
    ffi::OsString,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{header::AUTHORIZATION, HeaderMap, HeaderValue, Method, Request, StatusCode},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use cc_switch_lib::{web_api, AppState, MultiAppConfig, SubscriptionProvider, SubscriptionService};
use serde_json::{json, Value};
use serial_test::serial;
use tower::ServiceExt;

#[derive(Clone, Default)]
struct MockQuotaState {
    calls: Arc<AtomicUsize>,
    authorization: Arc<Mutex<Vec<String>>>,
}

struct EnvGuard(Vec<(&'static str, Option<OsString>)>);

impl EnvGuard {
    fn set(values: &[(&'static str, &str)]) -> Self {
        let previous = values
            .iter()
            .map(|(key, value)| {
                let old = std::env::var_os(key);
                std::env::set_var(key, value);
                (*key, old)
            })
            .collect();
        Self(previous)
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.0.drain(..) {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
        SubscriptionService::clear_cache_for_tests();
    }
}

async fn quota_response(State(state): State<MockQuotaState>, headers: HeaderMap) -> Json<Value> {
    let call = state.calls.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some(value) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        state
            .authorization
            .lock()
            .expect("authorization lock")
            .push(value.to_string());
    }
    Json(json!({
        "plan": "pro",
        "five_hour": {
            "utilization": call as f64 * 10.0,
            "reset_at": "2026-07-14T00:00:00Z"
        }
    }))
}

async fn refresh_failure(State(state): State<MockQuotaState>) -> StatusCode {
    state.calls.fetch_add(1, Ordering::SeqCst);
    StatusCode::UNAUTHORIZED
}

async fn spawn_quota_server() -> (String, MockQuotaState, tokio::task::JoinHandle<()>) {
    let state = MockQuotaState::default();
    let app = Router::new()
        .route("/quota", get(quota_response))
        .route("/refresh-fail", post(refresh_failure))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind quota mock");
    let address = listener.local_addr().expect("quota mock address");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{address}"), state, handle)
}

fn make_app() -> (Router, Arc<AppState>) {
    std::env::set_var("WEB_CSRF_TOKEN", "csrf-token");
    let state = Arc::new(AppState::new_for_tests(MultiAppConfig::default()).expect("test state"));
    (
        web_api::create_router(state.clone(), "password".to_string()),
        state,
    )
}

fn auth_header() -> HeaderValue {
    let encoded = base64::engine::general_purpose::STANDARD.encode(b"admin:password");
    HeaderValue::from_str(&format!("Basic {encoded}")).expect("auth header")
}

async fn get_json(app: Router, path: &str) -> Value {
    let request = Request::builder()
        .method(Method::GET)
        .uri(path)
        .header(AUTHORIZATION, auth_header())
        .body(Body::empty())
        .expect("request");
    let response = app.oneshot(request).await.expect("quota response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("quota response body");
    serde_json::from_slice(&bytes).expect("quota response JSON")
}

fn write_credentials(value: Value) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().expect("credential file");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&value).expect("credential JSON"),
    )
    .expect("write credential file");
    file
}

#[tokio::test]
#[serial]
async fn web_quota_cache_force_refresh_and_response_redaction() {
    let (base_url, mock, handle) = spawn_quota_server().await;
    let secret = "claude-quota-secret-never-return";
    let credentials = write_credentials(json!({
        "accessToken": secret,
        "expiresAt": "2099-01-01T00:00:00Z",
        "accountId": "claude-account",
        "email": "user@example.com"
    }));
    let credential_path = credentials.path().to_string_lossy().to_string();
    let _env = EnvGuard::set(&[
        ("CC_SWITCH_SUBSCRIPTION_CREDENTIALS", &credential_path),
        (
            "CC_SWITCH_TEST_CLAUDE_USAGE_URL",
            &format!("{base_url}/quota"),
        ),
    ]);
    SubscriptionService::clear_cache_for_tests();
    let (app, _) = make_app();

    let first = get_json(app.clone(), "/api/subscriptions/quota?provider=claude").await;
    let cached = get_json(app.clone(), "/api/subscriptions/quota?provider=claude").await;
    assert_eq!(mock.calls.load(Ordering::SeqCst), 1);
    assert_eq!(first["fetchedAt"], cached["fetchedAt"]);
    assert_eq!(first["status"], "available");
    assert_eq!(first["accountId"], "claude-account");

    let refreshed = get_json(app, "/api/subscriptions/quota?provider=claude&force=true").await;
    assert_eq!(mock.calls.load(Ordering::SeqCst), 2);
    assert_eq!(refreshed["status"], "available");

    for response in [&first, &cached, &refreshed] {
        let serialized = response.to_string();
        assert!(!serialized.contains(secret));
        assert!(!serialized.contains(&credential_path));
    }
    assert_eq!(
        mock.authorization
            .lock()
            .expect("authorization lock")
            .as_slice(),
        [format!("Bearer {secret}"), format!("Bearer {secret}")]
    );
    handle.abort();
}

#[tokio::test]
#[serial]
async fn codex_cli_credentials_are_discovered_without_exposing_tokens() {
    let (base_url, mock, handle) = spawn_quota_server().await;
    let secret = "codex-quota-secret-never-return";
    let credentials = write_credentials(json!({
        "auth_mode": "chatgpt",
        "expires_at": "2099-01-01T00:00:00Z",
        "email": "codex@example.com",
        "tokens": {
            "access_token": secret,
            "account_id": "codex-account"
        }
    }));
    let credential_path = credentials.path().to_string_lossy().to_string();
    let _env = EnvGuard::set(&[
        ("CC_SWITCH_SUBSCRIPTION_CREDENTIALS", &credential_path),
        (
            "CC_SWITCH_TEST_CODEX_USAGE_URL",
            &format!("{base_url}/quota"),
        ),
    ]);
    SubscriptionService::clear_cache_for_tests();
    let (_, state) = make_app();

    let quota = SubscriptionService::query(&state, SubscriptionProvider::Codex, None, true)
        .await
        .expect("Codex quota");
    let serialized = serde_json::to_string(&quota).expect("serialize quota");
    assert_eq!(quota.status, "available");
    assert_eq!(quota.account_id.as_deref(), Some("codex-account"));
    assert_eq!(mock.calls.load(Ordering::SeqCst), 1);
    assert!(!serialized.contains(secret));
    assert!(!serialized.contains(&credential_path));
    handle.abort();
}

#[tokio::test]
#[serial]
async fn expired_and_refresh_failed_credentials_return_sanitized_statuses() {
    let (base_url, mock, handle) = spawn_quota_server().await;
    let expired_secret = "expired-secret-never-return";
    let credentials = write_credentials(json!({
        "accessToken": expired_secret,
        "expiresAt": "2020-01-01T00:00:00Z"
    }));
    let credential_path = credentials.path().to_string_lossy().to_string();
    let token_url = format!("{base_url}/refresh-fail");
    let _env = EnvGuard::set(&[
        ("CC_SWITCH_SUBSCRIPTION_CREDENTIALS", &credential_path),
        ("CC_SWITCH_TEST_GEMINI_TOKEN_URL", &token_url),
    ]);
    SubscriptionService::clear_cache_for_tests();
    let (app, _) = make_app();

    let claude = get_json(
        app.clone(),
        "/api/subscriptions/quota?provider=claude&force=true",
    )
    .await;
    assert_eq!(claude["status"], "unavailable");
    assert!(claude["error"]
        .as_str()
        .unwrap_or_default()
        .contains("expired"));

    std::fs::write(
        credentials.path(),
        serde_json::to_vec(&json!({
            "accessToken": expired_secret,
            "refreshToken": "refresh-secret-never-return",
            "expiresAt": "2020-01-01T00:00:00Z"
        }))
        .expect("Gemini credential JSON"),
    )
    .expect("replace credential file");
    SubscriptionService::clear_cache_for_tests();
    let gemini = get_json(app, "/api/subscriptions/quota?provider=gemini&force=true").await;
    assert_eq!(gemini["status"], "unavailable");
    assert!(gemini["error"]
        .as_str()
        .unwrap_or_default()
        .contains("refresh failed"));
    assert_eq!(mock.calls.load(Ordering::SeqCst), 1);

    for response in [claude, gemini] {
        let serialized = response.to_string();
        assert!(!serialized.contains(expired_secret));
        assert!(!serialized.contains("refresh-secret-never-return"));
        assert!(!serialized.contains(&credential_path));
    }
    handle.abort();
}
