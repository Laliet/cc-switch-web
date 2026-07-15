#![cfg(feature = "web-server")]

use std::sync::{Arc, OnceLock};

use axum::{
    body::{to_bytes, Body},
    http::{header::AUTHORIZATION, HeaderValue, Method, Request, StatusCode},
};
use base64::Engine;
use cc_switch_lib::{web_api, AppState, MultiAppConfig};
use serial_test::serial;
use tokio::sync::Mutex;
use tower::ServiceExt;

#[path = "support.rs"]
mod support;
use support::{ensure_test_home, reset_test_fs};

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

fn make_app(password: &str, csrf: &str) -> axum::Router {
    make_app_with_state(password, csrf).0
}

fn async_test_mutex() -> &'static Mutex<()> {
    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_MUTEX.get_or_init(|| Mutex::new(()))
}

async fn dispatch(app: axum::Router, request: Request<Body>) -> axum::response::Response {
    app.oneshot(request).await.expect("router response")
}

async fn response_error_fields(res: axum::response::Response) -> (String, String) {
    let bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("response body");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("error json");
    let code = value
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let message = value
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    (code, message)
}

#[tokio::test]
#[serial]
async fn skills_list_rejects_claude_desktop_query() {
    let _guard = async_test_mutex().lock().await;
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/skills?app=claude-desktop")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .expect("build request");

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::NOT_IMPLEMENTED);
    let (code, error) = response_error_fields(res).await;
    assert_eq!(code, "skills_claude_desktop_unavailable");
    assert_unsupported_error(&error);
}

#[tokio::test]
#[serial]
async fn skills_install_rejects_claude_desktop_payload() {
    let _guard = async_test_mutex().lock().await;
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/skills/install")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .header("x-csrf-token", HeaderValue::from_static("csrf-token"))
        .header("content-type", HeaderValue::from_static("application/json"))
        .body(Body::from(
            serde_json::json!({
                "directory": "skills/demo",
                "app": "claude-desktop"
            })
            .to_string(),
        ))
        .expect("build request");

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::NOT_IMPLEMENTED);
    let (code, error) = response_error_fields(res).await;
    assert_eq!(code, "skills_claude_desktop_unavailable");
    assert_unsupported_error(&error);
}

fn assert_unsupported_error(error: &str) {
    assert!(
        error.contains("暂未支持")
            || error.contains("暂不支持")
            || error.contains("not supported yet")
            || error.contains("does not support"),
        "unexpected error message: {error}"
    );
}

#[tokio::test]
#[serial]
async fn config_get_dir_supports_opencode() {
    let _guard = async_test_mutex().lock().await;
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/config/opencode/dir")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .expect("build request");

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn mcp_get_config_maps_omo_to_opencode() {
    let _guard = async_test_mutex().lock().await;
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/mcp/config/omo")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .expect("build request");

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn providers_list_supports_opencode() {
    let _guard = async_test_mutex().lock().await;
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/providers/opencode")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .expect("build request");

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn prompts_list_rejects_upcoming_app() {
    let _guard = async_test_mutex().lock().await;
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/prompts/omo")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .expect("build request");

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::NOT_IMPLEMENTED);
    let (code, error) = response_error_fields(res).await;
    assert_eq!(code, "prompts_omo_unavailable");
    assert!(
        error.contains("暂未支持") || error.contains("not supported yet"),
        "unexpected error message: {error}"
    );
}

#[tokio::test]
#[serial]
async fn known_app_feature_boundaries_return_coded_501() {
    let _guard = async_test_mutex().lock().await;
    reset_test_fs();
    let _home = ensure_test_home();

    for (method, uri, csrf, expected_code) in [
        (
            Method::GET,
            "/api/mcp/config/openclaw",
            false,
            "mcp_openclaw_unavailable",
        ),
        (
            Method::GET,
            "/api/config/opencode/common-snippet",
            false,
            "config_snippet_opencode_unavailable",
        ),
        (
            Method::POST,
            "/api/providers/openclaw/provider-a/usage",
            true,
            "usage_openclaw_unavailable",
        ),
    ] {
        let app = make_app("password", "csrf-token");
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header(AUTHORIZATION, basic_auth_header("admin", "password"));
        if csrf {
            request = request.header("x-csrf-token", HeaderValue::from_static("csrf-token"));
        }
        let response = dispatch(app, request.body(Body::empty()).expect("build request")).await;
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED, "{uri}");
        let (code, error) = response_error_fields(response).await;
        assert_eq!(code, expected_code, "{uri}: {error}");
        assert!(error.contains("not supported yet"), "{uri}: {error}");
    }
}

#[tokio::test]
#[serial]
async fn unknown_app_remains_a_bad_request() {
    let _guard = async_test_mutex().lock().await;
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/prompts/not-an-app")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .expect("build request");
    let response = dispatch(app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let (code, _) = response_error_fields(response).await;
    assert_eq!(code, "bad_request");
}
