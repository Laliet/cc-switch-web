#![cfg(feature = "web-server")]

use std::sync::{Arc, OnceLock};

use axum::{
    body::{to_bytes, Body},
    http::{header::AUTHORIZATION, HeaderValue, Method, Request, StatusCode},
};
use base64::Engine;
use cc_switch_lib::{database::StreamCheckLogRecord, web_api, AppState, MultiAppConfig};
use serde_json::{json, Value};
use serial_test::serial;
use tokio::sync::Mutex;
use tower::ServiceExt;

#[path = "support.rs"]
mod support;
use support::{ensure_test_home, reset_test_fs};

fn test_mutex() -> &'static Mutex<()> {
    static MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    MUTEX.get_or_init(|| Mutex::new(()))
}

fn auth_header() -> HeaderValue {
    let encoded = base64::engine::general_purpose::STANDARD.encode(b"admin:password");
    HeaderValue::from_str(&format!("Basic {encoded}")).expect("auth header")
}

fn make_app() -> (axum::Router, Arc<AppState>) {
    std::env::set_var("WEB_CSRF_TOKEN", "csrf-token");
    let state =
        Arc::new(AppState::new_for_tests(MultiAppConfig::default()).expect("test app state"));
    (
        web_api::create_router(state.clone(), "password".to_string()),
        state,
    )
}

fn request(method: Method, uri: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method.clone())
        .uri(uri)
        .header(AUTHORIZATION, auth_header());
    if method != Method::GET {
        builder = builder.header("x-csrf-token", "csrf-token");
    }
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    builder
        .body(
            body.map(|value| Body::from(value.to_string()))
                .unwrap_or_else(Body::empty),
        )
        .expect("request")
}

async fn dispatch(app: axum::Router, request: Request<Body>) -> axum::response::Response {
    app.oneshot(request).await.expect("router response")
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("json response")
}

fn provider(id: &str, model: &str) -> Value {
    json!({
        "id": id,
        "name": id,
        "settingsConfig": {
            "baseUrl": "https://api.example.com/v1",
            "apiKey": "secret-token",
            "api": "openai-completions",
            "models": [{ "id": model, "name": model }]
        },
        "category": "custom",
        "sortIndex": 0
    })
}

#[tokio::test]
#[serial]
async fn openclaw_web_provider_lifecycle_updates_live_config_and_default() {
    let _guard = test_mutex().lock().await;
    reset_test_fs();
    let home = ensure_test_home();
    let (app, state) = make_app();

    for (id, model) in [("provider-a", "model-a"), ("provider-b", "model-b")] {
        let response = dispatch(
            app.clone(),
            request(
                Method::POST,
                "/api/providers/openclaw",
                Some(provider(id, model)),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let response = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/providers/openclaw/provider-a/switch",
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let config_path = home.join(".openclaw").join("openclaw.json");
    let source = std::fs::read_to_string(&config_path).expect("live OpenClaw config");
    let live: Value = json5::from_str(&source).expect("valid JSON5 config");
    assert!(live["models"]["providers"]["provider-a"].is_object());
    assert!(live["models"]["providers"]["provider-b"].is_object());
    assert_eq!(
        live["agents"]["defaults"]["model"]["primary"],
        "provider-a/model-a"
    );

    let response = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/providers/openclaw/provider-b/switch",
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = dispatch(
        app.clone(),
        request(Method::DELETE, "/api/providers/openclaw/provider-b", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let status = dispatch(
        app.clone(),
        request(Method::GET, "/api/openclaw/status", None),
    )
    .await;
    assert_eq!(status.status(), StatusCode::OK);
    let status = json_body(status).await;
    assert_eq!(status["defaultModel"]["primary"], "provider-a/model-a");
    assert_eq!(status["providers"].as_array().map(Vec::len), Some(1));

    let stored = state.load_config().expect("stored config");
    let manager = stored.apps.get("openclaw").expect("OpenClaw manager");
    assert!(manager.providers.contains_key("provider-a"));
    assert!(!manager.providers.contains_key("provider-b"));
}

#[tokio::test]
#[serial]
async fn workspace_web_routes_enforce_etags_and_restore_backups() {
    let _guard = test_mutex().lock().await;
    reset_test_fs();
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let workspace_path = workspace.path().to_string_lossy().to_string();
    std::env::set_var("CC_SWITCH_OPENCLAW_WORKSPACE_DIR", &workspace_path);
    let (app, _) = make_app();

    let response = dispatch(
        app.clone(),
        request(Method::GET, "/api/workspace/memory/2026-07-13", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let error = json_body(response).await;
    assert_eq!(error["error"], "Request could not be processed");
    assert!(!error.to_string().contains(&workspace_path));

    let response = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/workspace/files/AGENTS.md",
            Some(json!({ "content": "first", "expectedEtag": null })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let first = json_body(response).await;
    let first_etag = first["etag"].as_str().expect("etag").to_string();

    let response = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/workspace/files/AGENTS.md",
            Some(json!({ "content": "stale", "expectedEtag": "wrong" })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error = json_body(response).await;
    assert_eq!(error["code"], "workspace_etag_conflict");

    let response = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/workspace/files/AGENTS.md",
            Some(json!({ "content": "second", "expectedEtag": first_etag })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let second = json_body(response).await;
    let second_etag = second["etag"].as_str().expect("second etag");
    assert!(second["backupId"].as_str().is_some());

    let response = dispatch(
        app.clone(),
        request(Method::GET, "/api/workspace/files/AGENTS.md/backups", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let backups = json_body(response).await;
    let backup_id = backups[0]["id"].as_str().expect("backup id");

    let response = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/workspace/files/AGENTS.md/restore",
            Some(json!({ "backupId": backup_id, "expectedEtag": second_etag })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = dispatch(
        app.clone(),
        request(Method::GET, "/api/workspace/files/AGENTS.md", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let content = json_body(response).await;
    assert_eq!(content["content"], "first");

    let response = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/workspace/memory/2026-02-30",
            Some(json!({ "content": "invalid", "expectedEtag": null })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/workspace/memory/2026-07-13",
            Some(json!({ "content": "daily first", "expectedEtag": null })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let memory_created = json_body(response).await;
    let memory_etag = memory_created["etag"]
        .as_str()
        .expect("memory etag")
        .to_string();

    let response = dispatch(
        app.clone(),
        request(Method::GET, "/api/workspace/memory/2026-07-13", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await["content"], "daily first");

    let response = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/workspace/memory/2026-07-13",
            Some(json!({ "content": "stale memory", "expectedEtag": "stale" })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let response = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/workspace/memory/2026-07-13",
            Some(json!({ "content": "daily second", "expectedEtag": memory_etag })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = dispatch(
        app.clone(),
        request(Method::GET, "/api/workspace/memory", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let memory_entries = json_body(response).await;
    assert_eq!(memory_entries[0]["date"], "2026-07-13");

    let sensitive_content = format!("workspace-secret-must-not-leak{}", "x".repeat(1024 * 1024));
    let response = dispatch(
        app,
        request(
            Method::PUT,
            "/api/workspace/files/SOUL.md",
            Some(json!({ "content": sensitive_content, "expectedEtag": null })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let error = json_body(response).await;
    let serialized = error.to_string();
    assert_eq!(error["code"], "workspace_file_too_large");
    assert!(!serialized.contains("workspace-secret-must-not-leak"));
    assert!(!serialized.contains(&workspace_path));

    std::env::remove_var("CC_SWITCH_OPENCLAW_WORKSPACE_DIR");
}

#[cfg(unix)]
#[tokio::test]
#[serial]
async fn workspace_web_routes_reject_file_memory_and_backup_symlinks() {
    use std::os::unix::fs::symlink;

    let _guard = test_mutex().lock().await;
    reset_test_fs();
    let home = ensure_test_home();
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let outside = tempfile::tempdir().expect("outside tempdir");
    let workspace_path = workspace.path().to_string_lossy().to_string();
    let outside_path = outside.path().to_string_lossy().to_string();
    std::env::set_var("CC_SWITCH_OPENCLAW_WORKSPACE_DIR", &workspace_path);
    let (app, _) = make_app();

    let outside_file = outside.path().join("outside.md");
    std::fs::write(&outside_file, "outside-secret").expect("outside file");
    symlink(&outside_file, workspace.path().join("SOUL.md")).expect("file symlink");
    let response = dispatch(
        app.clone(),
        request(Method::GET, "/api/workspace/files/SOUL.md", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error = json_body(response).await.to_string();
    assert!(!error.contains("outside-secret"));
    assert!(!error.contains(&outside_path));

    symlink(outside.path(), workspace.path().join("memory")).expect("memory symlink");
    let response = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/workspace/memory/2026-07-13",
            Some(json!({ "content": "must-not-escape", "expectedEtag": null })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!outside.path().join("2026-07-13.md").exists());

    let response = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/workspace/files/AGENTS.md",
            Some(json!({ "content": "first", "expectedEtag": null })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let first_etag = json_body(response).await["etag"]
        .as_str()
        .expect("first etag")
        .to_string();

    let backup_parent = home.join(".cc-switch/backups/workspace");
    std::fs::create_dir_all(&backup_parent).expect("backup parent");
    let outside_backups = outside.path().join("backups");
    std::fs::create_dir(&outside_backups).expect("outside backup dir");
    symlink(&outside_backups, backup_parent.join("AGENTS.md")).expect("backup dir symlink");
    let response = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/workspace/files/AGENTS.md",
            Some(json!({ "content": "second", "expectedEtag": first_etag })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error = json_body(response).await.to_string();
    assert!(!error.contains(&outside_path));
    assert!(std::fs::read_dir(&outside_backups)
        .expect("outside backup entries")
        .next()
        .is_none());

    std::fs::remove_file(backup_parent.join("AGENTS.md")).expect("remove backup symlink");
    let backup_dir = backup_parent.join("AGENTS.md");
    std::fs::create_dir(&backup_dir).expect("real backup dir");
    std::fs::write(backup_dir.join("not-a-backup.txt"), "ignored").expect("invalid backup entry");
    symlink(&outside_file, backup_dir.join("123-deadbeef.bak")).expect("backup file symlink");
    let response = dispatch(
        app.clone(),
        request(Method::GET, "/api/workspace/files/AGENTS.md/backups", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await.as_array().map(Vec::len), Some(0));

    let response = dispatch(
        app,
        request(
            Method::POST,
            "/api/workspace/files/AGENTS.md/restore",
            Some(json!({
                "backupId": "123-deadbeef.bak",
                "expectedEtag": first_etag
            })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        std::fs::read_to_string(outside_file).expect("outside file remains"),
        "outside-secret"
    );

    std::env::remove_var("CC_SWITCH_OPENCLAW_WORKSPACE_DIR");
}

#[tokio::test]
#[serial]
async fn stream_check_web_routes_filter_persisted_history() {
    let _guard = test_mutex().lock().await;
    reset_test_fs();
    let (app, state) = make_app();
    state
        .db
        .insert_stream_check_log(&StreamCheckLogRecord {
            id: 0,
            provider_id: "provider-a".to_string(),
            provider_name: "Provider A".to_string(),
            app_type: "claude".to_string(),
            status: "operational".to_string(),
            success: true,
            message: "ok".to_string(),
            response_time_ms: Some(120),
            http_status: Some(200),
            model_used: "claude-test".to_string(),
            retry_count: 0,
            error_category: None,
            tested_at: 1_000,
        })
        .expect("insert stream check log");

    let response = dispatch(
        app.clone(),
        request(
            Method::GET,
            "/api/stream-check/logs/latest?appType=claude",
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let latest = json_body(response).await;
    assert_eq!(latest.as_array().map(Vec::len), Some(1));
    assert_eq!(latest[0]["providerId"], "provider-a");

    let response = dispatch(
        app,
        request(Method::GET, "/api/stream-check/logs?appType=codex", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await.as_array().map(Vec::len), Some(0));
}

#[tokio::test]
#[serial]
async fn stream_check_standard_and_legacy_routes_are_both_registered() {
    let _guard = test_mutex().lock().await;
    reset_test_fs();
    let (app, _) = make_app();

    for (path, body) in [
        (
            "/api/stream-check/providers/missing-provider",
            Some(json!({ "appType": "not-an-app" })),
        ),
        (
            "/api/stream-check/all",
            Some(json!({ "appType": "not-an-app" })),
        ),
        (
            "/api/stream-check/providers/not-an-app/missing-provider",
            None,
        ),
        (
            "/api/stream-check/providers",
            Some(json!({ "appType": "not-an-app" })),
        ),
    ] {
        let response = dispatch(app.clone(), request(Method::POST, path, body)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "route {path}");
    }
}

#[tokio::test]
#[serial]
async fn stream_check_routes_return_coded_501_for_unsupported_apps() {
    let _guard = test_mutex().lock().await;
    reset_test_fs();
    let (app, _) = make_app();

    for (path, body) in [
        (
            "/api/stream-check/providers/provider-a",
            Some(json!({ "appType": "openclaw" })),
        ),
        ("/api/stream-check/all", Some(json!({ "appType": "omo" }))),
        ("/api/stream-check/providers/omo-slim/provider-a", None),
    ] {
        let response = dispatch(app.clone(), request(Method::POST, path, body)).await;
        assert_eq!(
            response.status(),
            StatusCode::NOT_IMPLEMENTED,
            "route {path}"
        );
        let error = json_body(response).await;
        assert!(
            error["code"]
                .as_str()
                .is_some_and(|code| code.ends_with("_unavailable")),
            "route {path} must return a capability error code: {error}"
        );
    }
}
