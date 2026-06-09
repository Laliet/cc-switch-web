#![cfg(feature = "web-server")]

use std::sync::{Arc, OnceLock};

use axum::{
    body::{to_bytes, Body},
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderValue, Method, Request, StatusCode,
    },
};
use base64::Engine;
use cc_switch_lib::{web_api, AppState, ManagedAuthProvider, MultiAppConfig};
use serde_json::{json, Value};
use serial_test::serial;
use tokio::sync::Mutex;
use tower::ServiceExt;

#[path = "support.rs"]
mod support;
use support::{ensure_test_home, reset_test_fs};

fn async_test_mutex() -> &'static Mutex<()> {
    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_MUTEX.get_or_init(|| Mutex::new(()))
}

fn basic_auth_header(user: &str, password: &str) -> HeaderValue {
    let raw = format!("{user}:{password}");
    let encoded = base64::engine::general_purpose::STANDARD.encode(raw.as_bytes());
    HeaderValue::from_str(&format!("Basic {encoded}")).expect("basic auth header")
}

fn make_app_with_state(password: &str, csrf: &str) -> (axum::Router, Arc<AppState>) {
    std::env::set_var("WEB_CSRF_TOKEN", csrf);
    let state =
        Arc::new(AppState::new_for_tests(MultiAppConfig::default()).expect("test app state"));
    (
        web_api::create_router(state.clone(), password.to_string()),
        state,
    )
}

async fn dispatch(app: axum::Router, request: Request<Body>) -> axum::response::Response {
    app.oneshot(request).await.expect("router response")
}

fn auth_request(method: Method, uri: &str, body: Body) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method.clone())
        .uri(uri)
        .header(AUTHORIZATION, basic_auth_header("admin", "password"));
    if method != Method::GET {
        builder = builder.header("x-csrf-token", HeaderValue::from_static("csrf-token"));
    }
    builder.body(body).unwrap()
}

fn auth_json_request(method: Method, uri: &str, value: Value) -> Request<Body> {
    let mut request = auth_request(method, uri, Body::from(value.to_string()));
    request
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    request
}

async fn response_json(response: axum::response::Response) -> Value {
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("json response")
}

#[tokio::test]
#[serial]
async fn auth_center_web_routes_manage_accounts() {
    let _guard = async_test_mutex().lock().await;
    reset_test_fs();
    let _home = ensure_test_home();

    let (app, state) = make_app_with_state("password", "csrf-token");

    let first_payload = json!({
        "provider": "github_copilot",
        "id": "gh-1",
        "label": "GitHub One",
        "username": "octo",
        "makeDefault": false,
        "tokens": {
            "accessToken": "access-1",
            "refreshToken": "refresh-1",
            "tokenType": "Bearer"
        }
    });
    let response = dispatch(
        app.clone(),
        auth_json_request(Method::POST, "/api/auth/accounts", first_payload),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let account = response_json(response).await;
    assert_eq!(account["id"], "gh-1");
    assert_eq!(account["provider"], "github_copilot");
    assert_eq!(account["isDefault"], true);

    let raw_tokens = state
        .db
        .get_raw_managed_auth_tokens_for_tests(ManagedAuthProvider::GithubCopilot, "gh-1")
        .expect("raw tokens")
        .expect("stored tokens");
    assert!(raw_tokens.0.starts_with("ccs2:"));
    assert!(raw_tokens
        .1
        .as_deref()
        .is_some_and(|value| value.starts_with("ccs2:")));

    let second_payload = json!({
        "provider": "github_copilot",
        "id": "gh-2",
        "label": "GitHub Two",
        "makeDefault": true,
        "tokens": {
            "accessToken": "access-2",
            "tokenType": "Bearer"
        }
    });
    let response = dispatch(
        app.clone(),
        auth_json_request(Method::POST, "/api/auth/accounts", second_payload),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = dispatch(
        app.clone(),
        auth_request(
            Method::GET,
            "/api/auth/accounts?provider=github_copilot",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let accounts = response_json(response).await;
    assert_eq!(accounts.as_array().expect("accounts").len(), 2);
    assert_eq!(accounts[0]["id"], "gh-2");
    assert_eq!(accounts[0]["isDefault"], true);
    assert!(accounts[0].get("accessToken").is_none());

    let response = dispatch(
        app.clone(),
        auth_request(
            Method::POST,
            "/api/auth/accounts/github_copilot/gh-1/default",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, json!(true));

    let response = dispatch(
        app.clone(),
        auth_request(
            Method::POST,
            "/api/auth/accounts/github_copilot/gh-1/logout",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, json!(true));

    let response = dispatch(
        app.clone(),
        auth_request(
            Method::DELETE,
            "/api/auth/accounts/github_copilot/gh-2",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, json!(true));

    let response = dispatch(
        app,
        auth_request(
            Method::GET,
            "/api/auth/accounts?provider=github_copilot",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let accounts = response_json(response).await;
    assert_eq!(accounts.as_array().expect("accounts").len(), 1);
    assert_eq!(accounts[0]["id"], "gh-1");
    assert_eq!(accounts[0]["status"], "logged_out");
}

#[tokio::test]
#[serial]
async fn auth_center_web_query_routes_accept_slash_account_ids() {
    let _guard = async_test_mutex().lock().await;
    reset_test_fs();
    let _home = ensure_test_home();

    let (app, _state) = make_app_with_state("password", "csrf-token");

    let payload = json!({
        "provider": "github_copilot",
        "id": "gh/team/1",
        "label": "GitHub Team",
        "tokens": {
            "accessToken": "access-team",
            "tokenType": "Bearer"
        }
    });
    let response = dispatch(
        app.clone(),
        auth_json_request(Method::POST, "/api/auth/accounts", payload),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = dispatch(
        app.clone(),
        auth_request(
            Method::POST,
            "/api/auth/accounts/default?provider=github_copilot&accountId=gh%2Fteam%2F1",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, json!(true));

    let response = dispatch(
        app.clone(),
        auth_request(
            Method::POST,
            "/api/auth/accounts/logout?provider=github_copilot&accountId=gh%2Fteam%2F1",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, json!(true));

    let response = dispatch(
        app,
        auth_request(
            Method::DELETE,
            "/api/auth/accounts?provider=github_copilot&accountId=gh%2Fteam%2F1",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, json!(true));
}

#[tokio::test]
#[serial]
async fn auth_center_web_usage_reports_missing_codex_account() {
    let _guard = async_test_mutex().lock().await;
    reset_test_fs();
    let _home = ensure_test_home();

    let (app, _state) = make_app_with_state("password", "csrf-token");

    let response = dispatch(
        app,
        auth_request(
            Method::GET,
            "/api/auth/usage?provider=codex_oauth",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert!(body["error"]
        .as_str()
        .is_some_and(|message| message.contains("Missing codex_oauth managed account")));
}
