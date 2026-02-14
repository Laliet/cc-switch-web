#![cfg(feature = "web-server")]

use std::sync::{Arc, RwLock};

use axum::{
    body::{to_bytes, Body},
    http::{header::AUTHORIZATION, HeaderValue, Method, Request, StatusCode},
};
use base64::Engine;
use cc_switch_lib::{web_api, AppState, MultiAppConfig};
use serial_test::serial;
use tower::ServiceExt;

#[path = "support.rs"]
mod support;
use support::{ensure_test_home, reset_test_fs, test_mutex};

fn basic_auth_header(user: &str, password: &str) -> HeaderValue {
    let raw = format!("{user}:{password}");
    let encoded = base64::engine::general_purpose::STANDARD.encode(raw.as_bytes());
    HeaderValue::from_str(&format!("Basic {encoded}")).expect("basic auth header")
}

fn make_app(password: &str, csrf: &str) -> axum::Router {
    std::env::set_var("WEB_CSRF_TOKEN", csrf);
    let state = Arc::new(AppState {
        config: RwLock::new(MultiAppConfig::default()),
    });
    web_api::create_router(state, password.to_string())
}

async fn dispatch(app: axum::Router, request: Request<Body>) -> axum::response::Response {
    app.oneshot(request).await.expect("router response")
}

async fn response_error_message(res: axum::response::Response) -> String {
    let bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("response body");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("error json");
    value
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
#[serial]
async fn skills_list_rejects_upcoming_app_query() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/skills?app=opencode")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .expect("build request");

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let error = response_error_message(res).await;
    assert!(
        error.contains("暂未支持") || error.contains("not supported yet"),
        "unexpected error message: {error}"
    );
}

#[tokio::test]
#[serial]
async fn skills_install_rejects_upcoming_app_payload() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
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
                "app": "omo"
            })
            .to_string(),
        ))
        .expect("build request");

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let error = response_error_message(res).await;
    assert!(
        error.contains("暂未支持") || error.contains("not supported yet"),
        "unexpected error message: {error}"
    );
}

#[tokio::test]
#[serial]
async fn config_get_dir_rejects_upcoming_app() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
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
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let error = response_error_message(res).await;
    assert!(
        error.contains("暂未支持") || error.contains("not supported yet"),
        "unexpected error message: {error}"
    );
}

#[tokio::test]
#[serial]
async fn mcp_get_config_rejects_upcoming_app() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
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
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let error = response_error_message(res).await;
    assert!(
        error.contains("暂未支持") || error.contains("not supported yet"),
        "unexpected error message: {error}"
    );
}

#[tokio::test]
#[serial]
async fn providers_list_rejects_upcoming_app() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
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
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let error = response_error_message(res).await;
    assert!(
        error.contains("暂未支持") || error.contains("not supported yet"),
        "unexpected error message: {error}"
    );
}

#[tokio::test]
#[serial]
async fn prompts_list_rejects_upcoming_app() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
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
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let error = response_error_message(res).await;
    assert!(
        error.contains("暂未支持") || error.contains("not supported yet"),
        "unexpected error message: {error}"
    );
}
