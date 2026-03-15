#![cfg(feature = "web-server")]

use std::sync::{Arc, RwLock};

use axum::{
    body::Body,
    http::{header::AUTHORIZATION, header::CONTENT_TYPE, HeaderValue, Method, Request, StatusCode},
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

#[tokio::test]
#[serial]
async fn test_basic_auth_valid() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/config/app/path")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .unwrap();

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn test_basic_auth_invalid_password() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/config/app/path")
        .header(AUTHORIZATION, basic_auth_header("admin", "wrong"))
        .body(Body::empty())
        .unwrap();

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn test_basic_auth_invalid_user() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/config/app/path")
        .header(AUTHORIZATION, basic_auth_header("not-admin", "password"))
        .body(Body::empty())
        .unwrap();

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn test_basic_auth_missing() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/config/app/path")
        .body(Body::empty())
        .unwrap();

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn test_csrf_required_for_post() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/tray/update")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .unwrap();

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[serial]
async fn test_csrf_not_required_for_get() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/config/app/path")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .unwrap();

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn test_csrf_invalid_token() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/tray/update")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .header("x-csrf-token", HeaderValue::from_static("wrong-token"))
        .body(Body::empty())
        .unwrap();

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[serial]
async fn test_security_headers_present() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/tray/update")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .header("x-csrf-token", HeaderValue::from_static("csrf-token"))
        .body(Body::empty())
        .unwrap();

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::OK);

    let headers = res.headers();
    assert_eq!(
        headers
            .get("strict-transport-security")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
        "max-age=31536000; includeSubDomains"
    );
    assert_eq!(
        headers
            .get("x-frame-options")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
        "DENY"
    );
    assert_eq!(
        headers
            .get("x-content-type-options")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
        "nosniff"
    );
    assert_eq!(
        headers
            .get("referrer-policy")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
        "no-referrer"
    );
}

#[tokio::test]
#[serial]
async fn test_update_credentials_persists_and_rotates_auth() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::PUT)
        .uri("/api/system/credentials")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .header("x-csrf-token", HeaderValue::from_static("csrf-token"))
        .header("content-type", HeaderValue::from_static("application/json"))
        .body(Body::from(
            serde_json::json!({
                "username": "new-user",
                "password": "new-pass"
            })
            .to_string(),
        ))
        .unwrap();

    let res = dispatch(app.clone(), req).await;
    assert_eq!(res.status(), StatusCode::OK);

    let req_old = Request::builder()
        .method(Method::GET)
        .uri("/api/config/app/path")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .unwrap();
    let res_old = dispatch(app.clone(), req_old).await;
    assert_eq!(res_old.status(), StatusCode::UNAUTHORIZED);

    let req_new = Request::builder()
        .method(Method::GET)
        .uri("/api/config/app/path")
        .header(AUTHORIZATION, basic_auth_header("new-user", "new-pass"))
        .body(Body::empty())
        .unwrap();
    let res_new = dispatch(app, req_new).await;
    assert_eq!(res_new.status(), StatusCode::OK);

    let username_path = home.join(".cc-switch").join("web_username");
    let password_path = home.join(".cc-switch").join("web_password");
    let username = std::fs::read_to_string(username_path).expect("read username");
    let password = std::fs::read_to_string(password_path).expect("read password");
    assert_eq!(username.trim(), "new-user");
    assert_eq!(password.trim(), "new-pass");
}

#[tokio::test]
#[serial]
async fn test_update_credentials_rotates_auth() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");

    let payload = serde_json::json!({
        "username": "new-admin",
        "password": "new-secret"
    })
    .to_string();
    let update_req = Request::builder()
        .method(Method::PUT)
        .uri("/api/system/credentials")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .header("x-csrf-token", HeaderValue::from_static("csrf-token"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(payload))
        .unwrap();

    let update_res = dispatch(app.clone(), update_req).await;
    assert_eq!(update_res.status(), StatusCode::OK);

    let old_req = Request::builder()
        .method(Method::GET)
        .uri("/api/config/app/path")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .unwrap();
    let old_res = dispatch(app.clone(), old_req).await;
    assert_eq!(old_res.status(), StatusCode::UNAUTHORIZED);

    let new_req = Request::builder()
        .method(Method::GET)
        .uri("/api/config/app/path")
        .header(AUTHORIZATION, basic_auth_header("new-admin", "new-secret"))
        .body(Body::empty())
        .unwrap();
    let new_res = dispatch(app, new_req).await;
    assert_eq!(new_res.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn test_update_credentials_rolls_back_persisted_username_on_failure() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let cc_switch_dir = home.join(".cc-switch");
    std::fs::create_dir_all(&cc_switch_dir).expect("create .cc-switch dir");
    std::fs::write(cc_switch_dir.join("web_username"), "admin").expect("seed username");
    std::fs::create_dir_all(cc_switch_dir.join("web_password"))
        .expect("make password path a directory");

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::PUT)
        .uri("/api/system/credentials")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .header("x-csrf-token", HeaderValue::from_static("csrf-token"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "broken-user",
                "password": "broken-pass"
            })
            .to_string(),
        ))
        .unwrap();

    let res = dispatch(app.clone(), req).await;
    assert_eq!(
        res.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "failing password persistence should return server error"
    );

    let persisted_username =
        std::fs::read_to_string(cc_switch_dir.join("web_username")).expect("read username");
    assert_eq!(
        persisted_username.trim(),
        "admin",
        "username file should roll back when password persistence fails"
    );

    let req_old = Request::builder()
        .method(Method::GET)
        .uri("/api/config/app/path")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .body(Body::empty())
        .unwrap();
    let res_old = dispatch(app, req_old).await;
    assert_eq!(
        res_old.status(),
        StatusCode::OK,
        "in-memory auth should remain usable after failed update"
    );
}

#[tokio::test]
#[serial]
async fn test_update_credentials_rejects_short_password() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let _home = ensure_test_home();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::PUT)
        .uri("/api/system/credentials")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .header("x-csrf-token", HeaderValue::from_static("csrf-token"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "new-user",
                "password": "short"
            })
            .to_string(),
        ))
        .unwrap();

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn test_update_credentials_rejects_empty_username() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::PUT)
        .uri("/api/system/credentials")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .header("x-csrf-token", HeaderValue::from_static("csrf-token"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "   ",
                "password": "secret123"
            })
            .to_string(),
        ))
        .unwrap();

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn test_update_credentials_rejects_invalid_username() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::PUT)
        .uri("/api/system/credentials")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .header("x-csrf-token", HeaderValue::from_static("csrf-token"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "bad:user",
                "password": "secret123"
            })
            .to_string(),
        ))
        .unwrap();

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn test_update_credentials_rejects_empty_password() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();

    let app = make_app("password", "csrf-token");

    let req = Request::builder()
        .method(Method::PUT)
        .uri("/api/system/credentials")
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .header("x-csrf-token", HeaderValue::from_static("csrf-token"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "new-user",
                "password": "   "
            })
            .to_string(),
        ))
        .unwrap();

    let res = dispatch(app, req).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
