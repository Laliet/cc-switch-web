#![cfg(feature = "web-server")]
#![allow(clippy::await_holding_lock)]

use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{
    body::{to_bytes, Body, Bytes},
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderMap, HeaderValue, Method, Request, StatusCode, Uri,
    },
    Json,
};
use base64::Engine;
use cc_switch_lib::{
    web_api, AppState, AppType, ClaudeDesktopMode, ClaudeDesktopModelRoute, MultiAppConfig,
    Provider, ProviderMeta, UniversalProvider,
};
use futures::StreamExt;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use serial_test::serial;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;

#[path = "support.rs"]
mod support;
use support::{ensure_test_home, reset_test_fs, test_mutex};

struct AccountHomeGuard {
    user: Option<OsString>,
    logname: Option<OsString>,
}

#[derive(Debug)]
struct CapturedUpstreamRequest {
    method: Method,
    path_and_query: String,
    headers: HeaderMap,
    body: Bytes,
}

#[derive(Clone)]
struct CaptureUpstreamState {
    sender: tokio::sync::mpsc::UnboundedSender<CapturedUpstreamRequest>,
    status: StatusCode,
    content_type: &'static str,
    response_body: Arc<str>,
}

#[derive(Clone)]
struct RectifierRetryUpstreamState {
    sender: tokio::sync::mpsc::UnboundedSender<CapturedUpstreamRequest>,
    requests: Arc<AtomicUsize>,
}

impl AccountHomeGuard {
    fn without_account_home() -> Self {
        let guard = Self {
            user: env::var_os("USER"),
            logname: env::var_os("LOGNAME"),
        };
        env::set_var("USER", "");
        env::set_var("LOGNAME", "");
        guard
    }
}

impl Drop for AccountHomeGuard {
    fn drop(&mut self) {
        restore_env("USER", self.user.take());
        restore_env("LOGNAME", self.logname.take());
    }
}

fn restore_env(key: &str, value: Option<OsString>) {
    if let Some(value) = value {
        env::set_var(key, value);
    } else {
        env::remove_var(key);
    }
}

fn basic_auth_header(user: &str, password: &str) -> HeaderValue {
    let raw = format!("{user}:{password}");
    let encoded = base64::engine::general_purpose::STANDARD.encode(raw.as_bytes());
    HeaderValue::from_str(&format!("Basic {encoded}")).expect("basic auth header")
}

fn make_app(password: &str, csrf: &str) -> axum::Router {
    make_app_with_gemini_provider(password, csrf, generic_gemini_provider())
}

fn make_app_with_gemini_provider(
    password: &str,
    csrf: &str,
    gemini_provider: Provider,
) -> axum::Router {
    env::set_var("WEB_CSRF_TOKEN", csrf);
    let mut config = MultiAppConfig::default();
    add_claude_provider(&mut config);
    add_gemini_provider(&mut config, gemini_provider);

    let state = Arc::new(AppState::new_for_tests(config).expect("test app state"));
    web_api::create_router(state, password.to_string())
}

fn make_app_with_claude_provider(
    password: &str,
    csrf: &str,
    claude_provider: Provider,
) -> axum::Router {
    make_app_with_claude_provider_and_state(password, csrf, claude_provider).0
}

fn make_app_with_claude_provider_and_state(
    password: &str,
    csrf: &str,
    claude_provider: Provider,
) -> (axum::Router, Arc<AppState>) {
    env::set_var("WEB_CSRF_TOKEN", csrf);
    let mut config = MultiAppConfig::default();
    add_claude_provider_value(&mut config, claude_provider);
    add_gemini_provider(&mut config, generic_gemini_provider());

    let state = Arc::new(AppState::new_for_tests(config).expect("test app state"));
    (
        web_api::create_router(state.clone(), password.to_string()),
        state,
    )
}

fn make_app_with_claude_desktop_provider_and_state(
    password: &str,
    csrf: &str,
    claude_desktop_provider: Provider,
) -> (axum::Router, Arc<AppState>) {
    env::set_var("WEB_CSRF_TOKEN", csrf);
    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::ClaudeDesktop);
    let manager = config
        .get_manager_mut(&AppType::ClaudeDesktop)
        .expect("claude desktop manager");
    manager.current = claude_desktop_provider.id.clone();
    manager
        .providers
        .insert(claude_desktop_provider.id.clone(), claude_desktop_provider);
    add_gemini_provider(&mut config, generic_gemini_provider());

    let state = Arc::new(AppState::new_for_tests(config).expect("test app state"));
    (
        web_api::create_router(state.clone(), password.to_string()),
        state,
    )
}

fn make_app_with_claude_desktop_providers(
    password: &str,
    csrf: &str,
    current_provider_id: &str,
    providers: Vec<Provider>,
) -> axum::Router {
    env::set_var("WEB_CSRF_TOKEN", csrf);
    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::ClaudeDesktop);
    let manager = config
        .get_manager_mut(&AppType::ClaudeDesktop)
        .expect("claude desktop manager");
    manager.current = current_provider_id.to_string();
    for provider in providers {
        manager.providers.insert(provider.id.clone(), provider);
    }
    add_gemini_provider(&mut config, generic_gemini_provider());

    let state = Arc::new(AppState::new_for_tests(config).expect("test app state"));
    web_api::create_router(state, password.to_string())
}

fn make_app_with_claude_failover(
    password: &str,
    csrf: &str,
    current_provider: Provider,
    backup_provider: Provider,
) -> axum::Router {
    env::set_var("WEB_CSRF_TOKEN", csrf);
    let mut config = MultiAppConfig::default();
    let manager = config
        .get_manager_mut(&AppType::Claude)
        .expect("claude manager");
    manager.current = current_provider.id.clone();
    manager.backup_current = Some(backup_provider.id.clone());
    manager
        .providers
        .insert(current_provider.id.clone(), current_provider);
    manager
        .providers
        .insert(backup_provider.id.clone(), backup_provider);
    add_gemini_provider(&mut config, generic_gemini_provider());

    let state = Arc::new(AppState::new_for_tests(config).expect("test app state"));
    web_api::create_router(state, password.to_string())
}

fn make_app_with_claude_providers(
    password: &str,
    csrf: &str,
    current_provider_id: &str,
    backup_provider_id: Option<&str>,
    providers: Vec<Provider>,
) -> axum::Router {
    env::set_var("WEB_CSRF_TOKEN", csrf);
    let mut config = MultiAppConfig::default();
    let manager = config
        .get_manager_mut(&AppType::Claude)
        .expect("claude manager");
    manager.current = current_provider_id.to_string();
    manager.backup_current = backup_provider_id.map(ToString::to_string);
    for provider in providers {
        manager.providers.insert(provider.id.clone(), provider);
    }
    add_gemini_provider(&mut config, generic_gemini_provider());

    let state = Arc::new(AppState::new_for_tests(config).expect("test app state"));
    web_api::create_router(state, password.to_string())
}

fn add_claude_provider(config: &mut MultiAppConfig) {
    add_claude_provider_value(
        config,
        Provider::with_id(
            "claude-test".to_string(),
            "Claude Test".to_string(),
            json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.anthropic.com",
                    "ANTHROPIC_AUTH_TOKEN": "test-token"
                }
            }),
            None,
        ),
    );
}

fn add_claude_provider_value(config: &mut MultiAppConfig, provider: Provider) {
    let manager = config
        .get_manager_mut(&AppType::Claude)
        .expect("claude manager");
    manager.current = provider.id.clone();
    manager.providers.insert(provider.id.clone(), provider);
}

fn add_gemini_provider(config: &mut MultiAppConfig, provider: Provider) {
    let manager = config
        .get_manager_mut(&AppType::Gemini)
        .expect("gemini manager");
    manager.current = provider.id.clone();
    manager.providers.insert(provider.id.clone(), provider);
}

fn generic_gemini_provider() -> Provider {
    Provider::with_id(
        "gemini-api-key".to_string(),
        "Gemini API Key".to_string(),
        json!({
            "env": {
                "GEMINI_API_KEY": "test-gemini-key",
                "GOOGLE_GEMINI_BASE_URL": "https://generativelanguage.googleapis.com"
            }
        }),
        None,
    )
}

fn google_oauth_gemini_provider() -> Provider {
    let mut provider = Provider::with_id(
        "gemini-google-oauth".to_string(),
        "Google Official".to_string(),
        json!({
            "env": {
                "GOOGLE_GEMINI_BASE_URL": "https://generativelanguage.googleapis.com"
            }
        }),
        None,
    );
    provider.meta = Some(ProviderMeta {
        partner_promotion_key: Some("google-official".to_string()),
        ..ProviderMeta::default()
    });
    provider
}

fn universal_provider_fixture() -> UniversalProvider {
    serde_json::from_value(json!({
        "id": "newapi",
        "name": "New API",
        "providerType": "newapi",
        "apps": {
            "claude": true,
            "codex": true,
            "gemini": true
        },
        "baseUrl": "https://gateway.example.com",
        "apiKey": "universal-token",
        "models": {
            "claude": {
                "model": "claude-sonnet-test",
                "haikuModel": "claude-haiku-test",
                "sonnetModel": "claude-sonnet-test",
                "opusModel": "claude-opus-test"
            },
            "codex": {
                "model": "gpt-test",
                "reasoningEffort": "medium"
            },
            "gemini": {
                "model": "gemini-test"
            }
        },
        "websiteUrl": "https://gateway.example.com/docs",
        "notes": "shared provider"
    }))
    .expect("universal provider fixture")
}

fn claude_provider_with_base_url(base_url: &str) -> Provider {
    claude_provider_with_id_base_url("claude-local", "Claude Local", base_url)
}

fn claude_provider_with_id_base_url(id: &str, name: &str, base_url: &str) -> Provider {
    Provider::with_id(
        id.to_string(),
        name.to_string(),
        json!({
            "env": {
                "ANTHROPIC_BASE_URL": base_url,
                "ANTHROPIC_AUTH_TOKEN": "test-token"
            }
        }),
        None,
    )
}

fn claude_desktop_proxy_provider_with_base_url(base_url: &str) -> Provider {
    let mut provider =
        claude_provider_with_id_base_url("claude-desktop-local", "Claude Desktop Local", base_url);
    provider.meta = Some(ProviderMeta {
        claude_desktop_mode: Some(ClaudeDesktopMode::Proxy),
        claude_desktop_model_routes: HashMap::from([(
            "claude-sonnet-4-6".to_string(),
            ClaudeDesktopModelRoute {
                model: "upstream-sonnet".to_string(),
                label_override: None,
                supports_1m: Some(false),
            },
        )]),
        ..ProviderMeta::default()
    });
    provider
}

fn claude_desktop_proxy_provider_without_auth(base_url: &str) -> Provider {
    let mut provider = Provider::with_id(
        "claude-desktop-local".to_string(),
        "Claude Desktop Local".to_string(),
        json!({
            "env": {
                "ANTHROPIC_BASE_URL": base_url
            }
        }),
        None,
    );
    provider.meta = Some(ProviderMeta {
        claude_desktop_mode: Some(ClaudeDesktopMode::Proxy),
        claude_desktop_model_routes: HashMap::from([(
            "claude-sonnet-4-6".to_string(),
            ClaudeDesktopModelRoute {
                model: "upstream-sonnet".to_string(),
                label_override: None,
                supports_1m: Some(false),
            },
        )]),
        ..ProviderMeta::default()
    });
    provider
}

fn codex_provider_with_base_url(base_url: &str) -> Provider {
    let mut provider = Provider::with_id(
        "codex-local".to_string(),
        "Codex Local".to_string(),
        json!({
            "auth": {
                "OPENAI_API_KEY": "codex-token"
            },
            "config": format!(
                "model_provider = \"local\"\n[model_providers.local]\nbase_url = \"{base_url}\""
            )
        }),
        None,
    );
    provider.meta = Some(ProviderMeta {
        provider_type: Some("codex_oauth".to_string()),
        prompt_cache_key: Some("codex-cache-key".to_string()),
        ..ProviderMeta::default()
    });
    provider
}

fn gemini_provider_with_base_url(base_url: &str) -> Provider {
    Provider::with_id(
        "gemini-local".to_string(),
        "Gemini Local".to_string(),
        json!({
            "env": {
                "GEMINI_API_KEY": "gemini-token",
                "GOOGLE_GEMINI_BASE_URL": base_url
            }
        }),
        None,
    )
}

fn claude_desktop_protocol_provider(
    base_url: &str,
    api_format: &str,
    upstream_model: &str,
) -> Provider {
    let settings_config = match api_format {
        "gemini_native" => json!({
            "env": {
                "ANTHROPIC_BASE_URL": base_url,
                "GEMINI_API_KEY": "gemini-desktop-token"
            }
        }),
        "openai_chat" | "openai_responses" => json!({
            "env": {
                "ANTHROPIC_BASE_URL": base_url,
                "OPENAI_API_KEY": "openai-desktop-token"
            }
        }),
        _ => json!({
            "env": {
                "ANTHROPIC_BASE_URL": base_url,
                "ANTHROPIC_AUTH_TOKEN": "anthropic-desktop-token"
            }
        }),
    };
    let mut provider = Provider::with_id(
        format!("claude-desktop-{api_format}"),
        format!("Claude Desktop {api_format}"),
        settings_config,
        None,
    );
    provider.meta = Some(ProviderMeta {
        claude_desktop_mode: Some(ClaudeDesktopMode::Proxy),
        claude_desktop_model_routes: HashMap::from([(
            "claude-sonnet-4-6".to_string(),
            ClaudeDesktopModelRoute {
                model: upstream_model.to_string(),
                label_override: None,
                supports_1m: Some(false),
            },
        )]),
        api_format: Some(api_format.to_string()),
        ..ProviderMeta::default()
    });
    provider
}

fn make_app_with_protocol_providers(
    password: &str,
    csrf: &str,
    upstream_base: &str,
) -> (axum::Router, Arc<AppState>) {
    env::set_var("WEB_CSRF_TOKEN", csrf);
    let mut config = MultiAppConfig::default();
    add_claude_provider_value(&mut config, claude_provider_with_base_url(upstream_base));
    let codex_provider = codex_provider_with_base_url(upstream_base);
    let codex_manager = config
        .get_manager_mut(&AppType::Codex)
        .expect("codex manager");
    codex_manager.current = codex_provider.id.clone();
    codex_manager
        .providers
        .insert(codex_provider.id.clone(), codex_provider);
    add_gemini_provider(&mut config, gemini_provider_with_base_url(upstream_base));

    let state = Arc::new(AppState::new_for_tests(config).expect("test app state"));
    (
        web_api::create_router(state.clone(), password.to_string()),
        state,
    )
}

fn free_tcp_port() -> u16 {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind free port");
    listener.local_addr().expect("local addr").port()
}

async fn spawn_json_upstream() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind upstream");
    let addr = listener.local_addr().expect("upstream addr");
    let app = axum::Router::new()
        .route(
            "/",
            axum::routing::any(|| async { Json(json!({ "ok": true })) }),
        )
        .route(
            "/*path",
            axum::routing::any(|| async { Json(json!({ "ok": true })) }),
        );
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), handle)
}

async fn spawn_authorization_capture_upstream() -> (
    String,
    tokio::task::JoinHandle<()>,
    tokio::sync::oneshot::Receiver<Option<String>>,
) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind upstream");
    let addr = listener.local_addr().expect("upstream addr");
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));
    let app = axum::Router::new()
        .route(
            "/",
            axum::routing::any({
                let tx = tx.clone();
                move |headers: axum::http::HeaderMap| {
                    let tx = tx.clone();
                    async move {
                        if let Some(tx) = tx.lock().await.take() {
                            let auth = headers
                                .get(AUTHORIZATION)
                                .and_then(|value| value.to_str().ok())
                                .map(ToString::to_string);
                            let _ = tx.send(auth);
                        }
                        Json(json!({ "ok": true }))
                    }
                }
            }),
        )
        .route(
            "/*path",
            axum::routing::any(move |headers: axum::http::HeaderMap| {
                let tx = tx.clone();
                async move {
                    if let Some(tx) = tx.lock().await.take() {
                        let auth = headers
                            .get(AUTHORIZATION)
                            .and_then(|value| value.to_str().ok())
                            .map(ToString::to_string);
                        let _ = tx.send(auth);
                    }
                    Json(json!({ "ok": true }))
                }
            }),
        );
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), handle, rx)
}

async fn capture_upstream_request(
    axum::extract::State(state): axum::extract::State<CaptureUpstreamState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());
    let _ = state.sender.send(CapturedUpstreamRequest {
        method,
        path_and_query,
        headers,
        body,
    });
    axum::response::Response::builder()
        .status(state.status)
        .header(CONTENT_TYPE, state.content_type)
        .body(Body::from(state.response_body.to_string()))
        .expect("capture upstream response")
}

async fn spawn_capture_upstream(
    status: StatusCode,
    content_type: &'static str,
    response_body: &'static str,
) -> (
    String,
    tokio::task::JoinHandle<()>,
    tokio::sync::mpsc::UnboundedReceiver<CapturedUpstreamRequest>,
) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind capture upstream");
    let addr = listener.local_addr().expect("capture upstream addr");
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let app = axum::Router::new()
        .fallback(capture_upstream_request)
        .with_state(CaptureUpstreamState {
            sender,
            status,
            content_type,
            response_body: Arc::from(response_body),
        });
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), handle, receiver)
}

async fn rectifier_retry_upstream(
    axum::extract::State(state): axum::extract::State<RectifierRetryUpstreamState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());
    let _ = state.sender.send(CapturedUpstreamRequest {
        method,
        path_and_query,
        headers,
        body,
    });
    let attempt = state.requests.fetch_add(1, Ordering::SeqCst);
    let (status, body) = if attempt == 0 {
        (
            StatusCode::BAD_REQUEST,
            r#"{"error":{"message":"Invalid signature in thinking block"}}"#,
        )
    } else {
        (
            StatusCode::OK,
            r#"{"id":"msg-retried","model":"claude-sonnet-4-6","content":[{"type":"text","text":"ok"}],"usage":{"input_tokens":9,"output_tokens":2}}"#,
        )
    };
    axum::response::Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .expect("rectifier upstream response")
}

async fn spawn_rectifier_retry_upstream() -> (
    String,
    Arc<AtomicUsize>,
    tokio::task::JoinHandle<()>,
    tokio::sync::mpsc::UnboundedReceiver<CapturedUpstreamRequest>,
) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind rectifier upstream");
    let address = listener.local_addr().expect("rectifier upstream address");
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let requests = Arc::new(AtomicUsize::new(0));
    let app = axum::Router::new()
        .fallback(rectifier_retry_upstream)
        .with_state(RectifierRetryUpstreamState {
            sender,
            requests: requests.clone(),
        });
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{address}"), requests, handle, receiver)
}

async fn next_captured_request(
    receiver: &mut tokio::sync::mpsc::UnboundedReceiver<CapturedUpstreamRequest>,
) -> CapturedUpstreamRequest {
    tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .expect("upstream request should arrive")
        .expect("capture upstream should remain available")
}

async fn spawn_text_upstream(
    status: StatusCode,
    content_type: &'static str,
    body: &'static str,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind upstream");
    let addr = listener.local_addr().expect("upstream addr");
    let body = body.to_string();
    let app = axum::Router::new()
        .route(
            "/",
            axum::routing::any({
                let body = body.clone();
                move || {
                    let body = body.clone();
                    async move {
                        axum::response::Response::builder()
                            .status(status)
                            .header(CONTENT_TYPE, content_type)
                            .body(Body::from(body))
                            .expect("response")
                    }
                }
            }),
        )
        .route(
            "/*path",
            axum::routing::any(move || {
                let body = body.clone();
                async move {
                    axum::response::Response::builder()
                        .status(status)
                        .header(CONTENT_TYPE, content_type)
                        .body(Body::from(body))
                        .expect("response")
                }
            }),
        );
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), handle)
}

async fn spawn_delayed_stream_upstream(
    first_delay: Duration,
    second_delay: Option<Duration>,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind upstream");
    let addr = listener.local_addr().expect("upstream addr");
    let handle = tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut request = vec![0u8; 16 * 1024];
        let _ = socket.read(&mut request).await;
        if socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            )
            .await
            .is_err()
        {
            return;
        }
        if socket.flush().await.is_err() {
            return;
        }

        tokio::time::sleep(first_delay).await;
        let first = b"data: first\n\n";
        if socket
            .write_all(format!("{:X}\r\n", first.len()).as_bytes())
            .await
            .is_err()
            || socket.write_all(first).await.is_err()
            || socket.write_all(b"\r\n").await.is_err()
            || socket.flush().await.is_err()
        {
            return;
        }

        if let Some(second_delay) = second_delay {
            tokio::time::sleep(second_delay).await;
            let second = b"data: second\n\n";
            if socket
                .write_all(format!("{:X}\r\n", second.len()).as_bytes())
                .await
                .is_err()
                || socket.write_all(second).await.is_err()
                || socket.write_all(b"\r\n").await.is_err()
                || socket.flush().await.is_err()
            {
                return;
            }
        }
        let _ = socket.write_all(b"0\r\n\r\n").await;
        let _ = socket.flush().await;
    });
    (format!("http://{addr}"), handle)
}

fn request(method: Method, uri: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method.clone())
        .uri(uri)
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .header("x-csrf-token", HeaderValue::from_static("csrf-token"));

    let body = if let Some(body) = body {
        builder = builder.header(CONTENT_TYPE, "application/json");
        Body::from(body.to_string())
    } else {
        Body::empty()
    };

    builder.body(body).expect("build request")
}

fn raw_json_request(method: Method, uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(AUTHORIZATION, basic_auth_header("admin", "password"))
        .header("x-csrf-token", HeaderValue::from_static("csrf-token"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("build raw json request")
}

async fn dispatch(app: axum::Router, request: Request<Body>) -> axum::response::Response {
    app.oneshot(request).await.expect("router response")
}

async fn json_body<T: DeserializeOwned>(res: axum::response::Response) -> T {
    let bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("json response")
}

async fn start_test_proxy(app: &axum::Router, proxy_port: u16) -> String {
    start_test_proxy_for_app(app, proxy_port, "claude").await
}

async fn start_test_proxy_for_app(app: &axum::Router, proxy_port: u16, bind_app: &str) -> String {
    let response = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": bind_app,
                    "enableLogging": true
                }
            })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let status: Value = json_body(response).await;
    status["listenUrl"]
        .as_str()
        .expect("proxy listen URL")
        .to_string()
}

async fn stop_test_proxy(app: &axum::Router) {
    let response = dispatch(app.clone(), request(Method::POST, "/api/proxy/stop", None)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

fn setup() -> AccountHomeGuard {
    let _home = ensure_test_home();
    let guard = AccountHomeGuard::without_account_home();
    reset_test_fs();
    guard
}

#[tokio::test]
#[serial]
async fn proxy_status_and_config_return_defaults() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app("password", "csrf-token");

    let res = dispatch(app.clone(), request(Method::GET, "/api/proxy/status", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    assert_eq!(status["running"], json!(false));
    assert!(status["listenUrl"].is_null());
    assert_eq!(status["takeover"]["claude"], json!(false));

    let res = dispatch(app, request(Method::GET, "/api/proxy/config", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let config: Value = json_body(res).await;
    assert_eq!(config["host"], json!("127.0.0.1"));
    assert_eq!(config["port"], json!(3456));
    assert_eq!(config["bindApp"], json!("claude"));
}

#[tokio::test]
#[serial]
async fn proxy_config_put_normalizes_and_persists_settings() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app("password", "csrf-token");

    let payload = json!({
        "settings": {
            "host": "127.0.0.1",
            "port": 4567,
            "upstreamProxy": "   ",
            "bindApp": "codex",
            "apps": {
                "claude": { "enabled": true, "maxRetries": 3 }
            }
        }
    });

    let res = dispatch(
        app.clone(),
        request(Method::PUT, "/api/proxy/config", Some(payload)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let config: Value = json_body(res).await;
    assert_eq!(config["host"], json!("127.0.0.1"));
    assert_eq!(config["port"], json!(4567));
    assert_eq!(config["bindApp"], json!("codex"));
    assert!(config.get("upstreamProxy").is_none());
    assert_eq!(config["liveTakeoverActive"], json!(true));
    assert_eq!(config["apps"]["claude"]["enabled"], json!(true));
    assert_eq!(config["apps"]["claude"]["maxRetries"], json!(3));
    assert_eq!(config["apps"]["codex"]["enabled"], json!(false));

    let res = dispatch(app, request(Method::GET, "/api/proxy/config", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let persisted: Value = json_body(res).await;
    assert_eq!(persisted["port"], json!(4567));
    assert!(persisted.get("upstreamProxy").is_none());
}

#[tokio::test]
#[serial]
async fn proxy_takeover_restore_and_recover_routes_update_flags() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app("password", "csrf-token");

    let res = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/proxy/takeover/claude",
            Some(json!({ "enabled": true })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let result: Value = json_body(res).await;
    assert_eq!(result["app"], json!("claude"));
    assert_eq!(result["enabled"], json!(true));
    assert_eq!(result["status"]["takeover"]["claude"], json!(true));

    let res = dispatch(
        app.clone(),
        request(Method::POST, "/api/proxy/restore", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    assert_eq!(status["takeover"]["claude"], json!(false));

    let res = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/proxy/takeover/codex",
            Some(json!({ "enabled": true })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = dispatch(
        app,
        request(Method::POST, "/api/proxy/recover-stale-takeover", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    assert_eq!(status["takeover"]["codex"], json!(false));
}

#[tokio::test]
#[serial]
async fn proxy_restore_updates_running_status_takeover_flags() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let proxy_port = free_tcp_port();
    let app = make_app("password", "csrf-token");

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude",
                    "apps": {
                        "claude": { "enabled": true }
                    }
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    assert_eq!(status["running"], json!(true));
    assert_eq!(status["takeover"]["claude"], json!(true));

    let res = dispatch(
        app.clone(),
        request(Method::POST, "/api/proxy/restore", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    assert_eq!(status["running"], json!(true));
    assert_eq!(status["takeover"]["claude"], json!(false));
    assert_eq!(status["activeTargets"], json!([]));

    let res = dispatch(app.clone(), request(Method::GET, "/api/proxy/status", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    assert_eq!(status["takeover"]["claude"], json!(false));
    assert_eq!(status["activeTargets"], json!([]));

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
}

#[tokio::test]
#[serial]
async fn proxy_routes_validate_test_payload_and_reject_unsupported_takeover_app() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app("password", "csrf-token");

    let valid_settings = json!({
        "settings": {
            "host": "127.0.0.1",
            "port": 3456,
            "bindApp": "claude"
        }
    });
    let res = dispatch(
        app.clone(),
        request(Method::POST, "/api/proxy/test", Some(valid_settings)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let result: Value = json_body(res).await;
    assert_eq!(result["success"], json!(true));
    assert_eq!(result["baseUrl"], json!("https://api.anthropic.com"));

    for unsupported in ["omo", "openclaw"] {
        let res = dispatch(
            app.clone(),
            request(
                Method::PUT,
                &format!("/api/proxy/takeover/{unsupported}"),
                Some(json!({ "enabled": true })),
            ),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::NOT_IMPLEMENTED,
            "takeover must reject {unsupported}"
        );
        let error: Value = json_body(res).await;
        assert!(
            error["code"]
                .as_str()
                .is_some_and(|code| code.ends_with("_unavailable")),
            "unsupported takeover should return a capability code: {error}"
        );
    }

    for unsupported in ["omo", "openclaw"] {
        let res = dispatch(
            app.clone(),
            request(
                Method::GET,
                &format!("/api/proxy/failover/{unsupported}"),
                None,
            ),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::NOT_IMPLEMENTED,
            "failover queue must reject {unsupported}"
        );
    }
}

#[tokio::test]
#[serial]
async fn proxy_settings_put_returns_boolean_and_takeover_get_is_status_alias() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app("password", "csrf-token");

    let payload = json!({
        "settings": {
            "host": " 127.0.0.1 ",
            "port": 4568,
            "upstreamProxy": " http://127.0.0.1:7890 ",
            "bindApp": "Claude",
            "apps": {
                "gemini": { "enabled": true }
            }
        }
    });

    let res = dispatch(
        app.clone(),
        request(Method::PUT, "/api/proxy/settings", Some(payload)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let saved: Value = json_body(res).await;
    assert_eq!(saved, json!(true));

    let res = dispatch(app.clone(), request(Method::GET, "/api/proxy/config", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let config: Value = json_body(res).await;
    assert_eq!(config["host"], json!("127.0.0.1"));
    assert_eq!(config["upstreamProxy"], json!("http://127.0.0.1:7890"));
    assert_eq!(config["bindApp"], json!("claude"));
    assert_eq!(config["apps"]["gemini"]["enabled"], json!(true));

    let res = dispatch(app, request(Method::GET, "/api/proxy/takeover", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    assert_eq!(status["running"], json!(false));
    assert_eq!(status["takeover"]["gemini"], json!(true));
    assert!(status.get("activeTargets").is_some());
}

#[tokio::test]
#[serial]
async fn proxy_failover_queue_routes_manage_ordered_provider_ids() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app_with_claude_providers(
        "password",
        "csrf-token",
        "claude-primary",
        None,
        vec![
            claude_provider_with_id_base_url("claude-primary", "Claude Primary", "http://p1"),
            claude_provider_with_id_base_url("claude-second", "Claude Second", "http://p2"),
            claude_provider_with_id_base_url("claude-third", "Claude Third", "http://p3"),
        ],
    );

    let res = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/proxy/failover/claude",
            Some(json!({ "providerIds": ["claude-third", "claude-second"] })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let queue: Value = json_body(res).await;
    assert_eq!(queue[0]["providerId"], json!("claude-third"));
    assert_eq!(queue[0]["providerName"], json!("Claude Third"));
    assert_eq!(queue[0]["position"], json!(0));
    assert_eq!(queue[1]["providerId"], json!("claude-second"));

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/failover/claude/claude-primary",
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let queue: Value = json_body(res).await;
    let ids: Vec<_> = queue
        .as_array()
        .expect("queue array")
        .iter()
        .map(|item| item["providerId"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec!["claude-third", "claude-second", "claude-primary"]);

    let res = dispatch(
        app.clone(),
        request(
            Method::DELETE,
            "/api/proxy/failover/claude/claude-second",
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let queue: Value = json_body(res).await;
    let ids: Vec<_> = queue
        .as_array()
        .expect("queue array")
        .iter()
        .map(|item| item["providerId"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec!["claude-third", "claude-primary"]);
    assert_eq!(queue[1]["position"], json!(1));

    let res = dispatch(
        app.clone(),
        request(Method::GET, "/api/proxy/failover/claude", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let queue: Value = json_body(res).await;
    assert_eq!(queue.as_array().expect("queue array").len(), 2);

    let res = dispatch(
        app.clone(),
        request(Method::PUT, "/api/proxy/failover/claude", Some(json!({}))),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let res = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/proxy/failover/claude",
            Some(json!({ "providerIds": ["missing-provider"] })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let res = dispatch(
        app,
        request(Method::DELETE, "/api/proxy/failover/claude", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let queue: Value = json_body(res).await;
    assert_eq!(queue, json!([]));
}

#[tokio::test]
#[serial]
async fn proxy_failover_queue_routes_accept_claude_desktop_providers() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let mut primary = claude_desktop_proxy_provider_with_base_url("http://desktop-primary");
    primary.id = "desktop-primary".to_string();
    primary.name = "Desktop Primary".to_string();
    let mut backup = claude_desktop_proxy_provider_with_base_url("http://desktop-backup");
    backup.id = "desktop-backup".to_string();
    backup.name = "Desktop Backup".to_string();
    let app = make_app_with_claude_desktop_providers(
        "password",
        "csrf-token",
        "desktop-primary",
        vec![primary, backup],
    );

    let res = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/proxy/failover/claude-desktop",
            Some(json!({ "providerIds": ["desktop-backup"] })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let queue: Value = json_body(res).await;
    assert_eq!(queue[0]["providerId"], json!("desktop-backup"));
    assert_eq!(queue[0]["providerName"], json!("Desktop Backup"));

    let res = dispatch(
        app,
        request(Method::GET, "/api/proxy/failover/claude-desktop", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let queue: Value = json_body(res).await;
    assert_eq!(queue.as_array().expect("queue array").len(), 1);
    assert_eq!(queue[0]["providerId"], json!("desktop-backup"));
}

#[tokio::test]
#[serial]
async fn proxy_model_pricing_routes_manage_custom_pricing() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app("password", "csrf-token");

    let res = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/proxy/pricing/models/custom-model",
            Some(json!({
                "modelId": "custom-model",
                "displayName": "Custom Model",
                "inputCostPerMillion": "2.5",
                "outputCostPerMillion": "7.5",
                "cacheReadCostPerMillion": "0.25",
                "cacheCreationCostPerMillion": "1.25"
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let saved: Value = json_body(res).await;
    assert_eq!(saved, json!(true));

    let res = dispatch(
        app.clone(),
        request(Method::GET, "/api/proxy/pricing/models", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let rows: Value = json_body(res).await;
    let custom = rows
        .as_array()
        .expect("pricing rows")
        .iter()
        .find(|row| row["modelId"] == json!("custom-model"))
        .expect("custom model row");
    assert_eq!(custom["displayName"], json!("Custom Model"));
    assert_eq!(custom["inputCostPerMillion"], json!("2.5"));

    let res = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/proxy/pricing/models/custom-model",
            Some(json!({
                "modelId": "other-model",
                "displayName": "Other Model",
                "inputCostPerMillion": "2.5",
                "outputCostPerMillion": "7.5",
                "cacheReadCostPerMillion": "0.25",
                "cacheCreationCostPerMillion": "1.25"
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let res = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/proxy/pricing/models/bad-model",
            Some(json!({
                "modelId": "bad-model",
                "displayName": "Bad Model",
                "inputCostPerMillion": "not-a-number",
                "outputCostPerMillion": "7.5",
                "cacheReadCostPerMillion": "0.25",
                "cacheCreationCostPerMillion": "1.25"
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let res = dispatch(
        app,
        request(
            Method::DELETE,
            "/api/proxy/pricing/models/custom-model",
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let deleted: Value = json_body(res).await;
    assert_eq!(deleted, json!(true));
}

#[tokio::test]
#[serial]
async fn universal_provider_web_api_roundtrips_syncs_and_deletes_generated_app_providers() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app("password", "csrf-token");
    let provider = serde_json::to_value(universal_provider_fixture()).expect("provider json");

    let res = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/providers/universal/newapi",
            Some(provider.clone()),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let saved: Value = json_body(res).await;
    assert_eq!(saved, json!(true));

    let res = dispatch(
        app.clone(),
        request(Method::GET, "/api/providers/universal", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let providers: Value = json_body(res).await;
    assert_eq!(providers["newapi"]["name"], json!("New API"));
    assert_eq!(providers["newapi"]["apps"]["claude"], json!(true));

    let res = dispatch(
        app.clone(),
        request(Method::POST, "/api/providers/universal/newapi/sync", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let synced: Value = json_body(res).await;
    assert_eq!(synced, json!(true));

    let res = dispatch(
        app.clone(),
        request(Method::GET, "/api/providers/claude", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let claude_providers: Value = json_body(res).await;
    assert_eq!(
        claude_providers["universal-claude-newapi"]["settingsConfig"]["env"]["ANTHROPIC_MODEL"],
        json!("claude-sonnet-test")
    );

    let res = dispatch(
        app.clone(),
        request(Method::GET, "/api/providers/codex", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let codex_providers: Value = json_body(res).await;
    assert!(
        codex_providers["universal-codex-newapi"]["settingsConfig"]["config"]
            .as_str()
            .unwrap_or_default()
            .contains("gpt-test")
    );

    let res = dispatch(
        app.clone(),
        request(Method::DELETE, "/api/providers/universal/newapi", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let deleted: Value = json_body(res).await;
    assert_eq!(deleted, json!(true));

    let res = dispatch(app, request(Method::GET, "/api/providers/claude", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let claude_providers: Value = json_body(res).await;
    assert!(claude_providers.get("universal-claude-newapi").is_none());
}

#[tokio::test]
#[serial]
async fn claude_desktop_web_api_exposes_status_routes_and_import() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let claude_provider = Provider::with_id(
        "claude-import-source".to_string(),
        "Claude Import Source".to_string(),
        json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.anthropic.com",
                "ANTHROPIC_AUTH_TOKEN": "test-token",
                "ANTHROPIC_MODEL": "claude-sonnet-4"
            }
        }),
        None,
    );
    let app = make_app_with_claude_provider("password", "csrf-token", claude_provider);

    let res = dispatch(
        app.clone(),
        request(
            Method::GET,
            "/api/providers/claude-desktop/default-routes",
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let routes: Value = json_body(res).await;
    assert_eq!(routes[0]["routeId"], json!("claude-sonnet-4-6"));

    let res = dispatch(
        app.clone(),
        request(Method::GET, "/api/providers/claude-desktop/status", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    assert!(status.get("supported").is_some());
    assert!(status.get("proxyRunning").is_some());
    assert!(status.get("desktopRunning").is_some());

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/providers/claude-desktop/import-from-claude",
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let imported: Value = json_body(res).await;
    assert_eq!(imported, json!(1));

    let res = dispatch(
        app,
        request(Method::GET, "/api/providers/claude-desktop", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let providers: Value = json_body(res).await;
    assert_eq!(
        providers["claude-import-source"]["meta"]["claudeDesktopMode"],
        json!("direct")
    );
}

#[tokio::test]
#[serial]
async fn universal_provider_web_api_rejects_path_body_id_mismatch() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app("password", "csrf-token");
    let mut provider = serde_json::to_value(universal_provider_fixture()).expect("provider json");
    provider["id"] = json!("other");

    let res = dispatch(
        app,
        request(
            Method::PUT,
            "/api/providers/universal/newapi",
            Some(provider),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn proxy_config_routes_reject_invalid_settings_boundaries() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app("password", "csrf-token");

    let cases = [
        (
            "invalid host",
            json!({
                "settings": {
                    "host": "localhost",
                    "port": 3456,
                    "bindApp": "claude"
                }
            }),
        ),
        (
            "invalid port",
            json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": 0,
                    "bindApp": "claude"
                }
            }),
        ),
        (
            "invalid upstream proxy",
            json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": 3456,
                    "upstreamProxy": "socks5://127.0.0.1:7890",
                    "bindApp": "claude"
                }
            }),
        ),
        (
            "unsupported bind app",
            json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": 3456,
                    "bindApp": "omo"
                }
            }),
        ),
        (
            "unsupported OpenClaw bind app",
            json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": 3456,
                    "bindApp": "openclaw"
                }
            }),
        ),
    ];

    for (name, payload) in cases {
        let res = dispatch(
            app.clone(),
            request(Method::PUT, "/api/proxy/config", Some(payload)),
        )
        .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{name}");
    }
}

#[tokio::test]
#[serial]
async fn proxy_start_is_idempotent_when_current_runtime_already_owns_port() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app("password", "csrf-token");
    let proxy_port = free_tcp_port();
    let payload = json!({
        "settings": {
            "host": "127.0.0.1",
            "port": proxy_port,
            "bindApp": "claude"
        }
    });

    let res = dispatch(
        app.clone(),
        request(Method::POST, "/api/proxy/start", Some(payload.clone())),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let first: Value = json_body(res).await;
    assert_eq!(first["running"], json!(true));

    let res = dispatch(
        app.clone(),
        request(Method::POST, "/api/proxy/start", Some(payload)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let second: Value = json_body(res).await;
    assert_eq!(second["running"], json!(true));
    assert_eq!(second["listenUrl"], first["listenUrl"]);

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
}

#[tokio::test]
#[serial]
async fn proxy_start_reports_clear_error_when_port_is_owned_by_another_process() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind occupied port");
    let occupied_port = listener.local_addr().expect("occupied addr").port();
    let app = make_app("password", "csrf-token");

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": occupied_port,
                    "bindApp": "claude"
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body: Value = json_body(res).await;
    let error = body["error"].as_str().unwrap_or_default();
    assert!(error.contains("已被占用") || error.contains("already in use"));
    assert!(!error.contains("Failed to bind proxy listener"));

    let res = dispatch(app, request(Method::GET, "/api/proxy/config", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let config: Value = json_body(res).await;
    assert_eq!(config["enabled"], json!(false));
    assert_eq!(config["liveTakeoverActive"], json!(false));
}

#[tokio::test]
#[serial]
async fn proxy_routes_return_json_rejections_for_malformed_or_missing_payloads() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app("password", "csrf-token");

    let res = dispatch(
        app.clone(),
        raw_json_request(Method::PUT, "/api/proxy/config", "{"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let res = dispatch(
        app,
        request(Method::PUT, "/api/proxy/config", Some(json!({}))),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
#[serial]
async fn proxy_recent_logs_are_empty_when_logging_is_disabled() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let app = make_app("password", "csrf-token");

    let res = dispatch(app, request(Method::GET, "/api/proxy/logs/recent", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let logs: Value = json_body(res).await;
    assert_eq!(logs, json!([]));
}

#[tokio::test]
#[serial]
async fn claude_desktop_gateway_requires_configured_bearer_token() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle) = spawn_json_upstream().await;
    let proxy_port = free_tcp_port();
    let (app, state) = make_app_with_claude_desktop_provider_and_state(
        "password",
        "csrf-token",
        claude_desktop_proxy_provider_with_base_url(&upstream_base),
    );
    state
        .db
        .set_setting("claude_desktop_gateway_token", "desktop-gateway-token")
        .expect("store gateway token");

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude",
                    "enableLogging": true
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    let listen_url = status["listenUrl"].as_str().expect("listen URL");
    let client = reqwest::Client::new();
    let models_url = format!("{listen_url}/claude-desktop/v1/models");

    let missing = client
        .get(&models_url)
        .send()
        .await
        .expect("missing token request");
    assert_eq!(missing.status(), reqwest::StatusCode::UNAUTHORIZED);

    let wrong = client
        .get(&models_url)
        .bearer_auth("wrong-token")
        .send()
        .await
        .expect("wrong token request");
    assert_eq!(wrong.status(), reqwest::StatusCode::UNAUTHORIZED);

    let ok = client
        .get(&models_url)
        .header(AUTHORIZATION, "bearer desktop-gateway-token")
        .send()
        .await
        .expect("authorized request");
    assert_eq!(ok.status(), reqwest::StatusCode::OK);
    let body: Value = ok.json().await.expect("model list body");
    assert_eq!(body["data"][0]["id"], json!("claude-sonnet-4-6"));

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn claude_desktop_gateway_bearer_token_is_not_forwarded_upstream() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle, upstream_auth) =
        spawn_authorization_capture_upstream().await;
    let proxy_port = free_tcp_port();
    let (app, state) = make_app_with_claude_desktop_provider_and_state(
        "password",
        "csrf-token",
        claude_desktop_proxy_provider_without_auth(&upstream_base),
    );
    state
        .db
        .set_setting("claude_desktop_gateway_token", "desktop-gateway-token")
        .expect("store gateway token");

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude"
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    let listen_url = status["listenUrl"].as_str().expect("listen URL");

    let proxy_response = reqwest::Client::new()
        .post(format!("{listen_url}/claude-desktop/v1/messages"))
        .bearer_auth("desktop-gateway-token")
        .json(&json!({
            "model": "claude-sonnet-4-6",
            "messages": []
        }))
        .send()
        .await
        .expect("proxy request");
    assert_eq!(proxy_response.status(), reqwest::StatusCode::OK);

    let forwarded_auth = tokio::time::timeout(Duration::from_secs(2), upstream_auth)
        .await
        .expect("upstream should receive request")
        .expect("upstream auth sender should complete");
    assert_eq!(forwarded_auth, None);

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_native_protocol_http_conformance_preserves_method_url_auth_and_schema() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle, mut captured) =
        spawn_capture_upstream(StatusCode::OK, "application/json", r#"{"ok":true}"#).await;
    let (app, state) = make_app_with_protocol_providers("password", "csrf-token", &upstream_base);
    let listen_url = start_test_proxy(&app, free_tcp_port()).await;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{listen_url}/v1/messages?trace=anthropic"))
        .json(&json!({
            "model": "claude-sonnet-4-6",
            "_private": "remove-me",
            "messages": [{ "role": "user", "content": "hello" }],
            "tools": [{
                "name": "lookup",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "_id": { "type": "string", "_private_note": "remove-me" }
                    }
                }
            }]
        }))
        .send()
        .await
        .expect("anthropic proxy request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let request = next_captured_request(&mut captured).await;
    assert_eq!(request.method, Method::POST);
    assert_eq!(request.path_and_query, "/v1/messages?trace=anthropic");
    assert_eq!(
        request
            .headers
            .get("x-api-key")
            .and_then(|value| value.to_str().ok()),
        Some("test-token")
    );
    let body: Value = serde_json::from_slice(&request.body).expect("anthropic upstream body");
    assert!(body.get("_private").is_none());
    assert!(body["tools"][0]["input_schema"]["properties"]
        .get("_id")
        .is_some());
    assert!(body["tools"][0]["input_schema"]["properties"]["_id"]
        .get("_private_note")
        .is_none());

    let response = client
        .post(format!("{listen_url}/v1/responses?trace=codex"))
        .json(&json!({
            "model": "gpt-5.1-codex",
            "metadata": { "session_id": "codex-session-1" },
            "input": "hello"
        }))
        .send()
        .await
        .expect("codex proxy request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let request = next_captured_request(&mut captured).await;
    assert_eq!(request.method, Method::POST);
    assert_eq!(request.path_and_query, "/v1/responses?trace=codex");
    assert_eq!(
        request
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some("Bearer codex-token")
    );
    assert_eq!(
        request
            .headers
            .get("openai-session-id")
            .and_then(|value| value.to_str().ok()),
        Some("codex-session-1")
    );
    assert_eq!(
        request
            .headers
            .get("openai-prompt-cache-key")
            .and_then(|value| value.to_str().ok()),
        Some("codex-cache-key")
    );
    let body: Value = serde_json::from_slice(&request.body).expect("codex upstream body");
    assert_eq!(body["model"], json!("gpt-5.1-codex"));

    let logs = state
        .db
        .recent_proxy_request_logs(10)
        .expect("proxy request logs");
    let codex_log = logs
        .iter()
        .find(|log| log.app_type == "codex")
        .expect("Codex request log");
    assert_eq!(codex_log.session_id.as_deref(), Some("codex-session-1"));

    let response = client
        .post(format!(
            "{listen_url}/v1beta/models/gemini-2.5-pro:generateContent?alt=json"
        ))
        .json(&json!({
            "contents": [{ "role": "user", "parts": [{ "text": "hello" }] }]
        }))
        .send()
        .await
        .expect("gemini proxy request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let request = next_captured_request(&mut captured).await;
    assert_eq!(request.method, Method::POST);
    assert_eq!(
        request.path_and_query,
        "/v1beta/models/gemini-2.5-pro:generateContent?alt=json"
    );
    assert_eq!(
        request
            .headers
            .get("x-goog-api-key")
            .and_then(|value| value.to_str().ok()),
        Some("gemini-token")
    );

    stop_test_proxy(&app).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_codex_models_method_and_session_cache_usage_are_correlated() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle, mut captured) = spawn_capture_upstream(
        StatusCode::OK,
        "application/json",
        r#"{"id":"resp-usage","model":"gpt-5.1-codex","usage":{"input_tokens":42,"output_tokens":7},"output":[]}"#,
    )
    .await;
    let (app, state) = make_app_with_protocol_providers("password", "csrf-token", &upstream_base);
    let listen_url = start_test_proxy_for_app(&app, free_tcp_port(), "codex").await;
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{listen_url}/v1/models?limit=20"))
        .send()
        .await
        .expect("models request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let request = next_captured_request(&mut captured).await;
    assert_eq!(request.method, Method::GET);
    assert_eq!(request.path_and_query, "/v1/models?limit=20");
    assert_eq!(
        request
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some("Bearer codex-token")
    );

    let response = client
        .post(format!("{listen_url}/v1/responses"))
        .json(&json!({
            "model": "gpt-5.1-codex",
            "metadata": { "session_id": "codex-session-usage" },
            "input": "hello"
        }))
        .send()
        .await
        .expect("Codex usage request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let request = next_captured_request(&mut captured).await;
    assert_eq!(request.method, Method::POST);
    assert_eq!(
        request
            .headers
            .get("openai-session-id")
            .and_then(|value| value.to_str().ok()),
        Some("codex-session-usage")
    );
    assert_eq!(
        request
            .headers
            .get("openai-prompt-cache-key")
            .and_then(|value| value.to_str().ok()),
        Some("codex-cache-key")
    );

    let logs = state
        .db
        .recent_proxy_request_logs(10)
        .expect("proxy request logs");
    let usage_log = logs
        .iter()
        .find(|log| log.session_id.as_deref() == Some("codex-session-usage"))
        .expect("session-linked usage log");
    assert_eq!(usage_log.provider_id, "codex-local");
    assert_eq!(usage_log.input_tokens, 42);
    assert_eq!(usage_log.output_tokens, 7);
    assert_eq!(usage_log.model, "gpt-5.1-codex");

    stop_test_proxy(&app).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_anthropic_signature_rectifier_retries_same_provider_over_http() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, requests, upstream_handle, mut captured) =
        spawn_rectifier_retry_upstream().await;
    let (app, _) = make_app_with_claude_provider_and_state(
        "password",
        "csrf-token",
        claude_provider_with_base_url(&upstream_base),
    );
    let listen_url = start_test_proxy(&app, free_tcp_port()).await;

    let response = reqwest::Client::new()
        .post(format!("{listen_url}/v1/messages"))
        .json(&json!({
            "model": "claude-sonnet-4-6",
            "thinking": { "type": "enabled" },
            "messages": [{
                "role": "assistant",
                "content": [
                    { "type": "thinking", "thinking": "private", "signature": "bad" },
                    { "type": "text", "text": "visible", "signature": "stray" }
                ]
            }]
        }))
        .send()
        .await
        .expect("rectified proxy request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(requests.load(Ordering::SeqCst), 2);

    let first = next_captured_request(&mut captured).await;
    let second = next_captured_request(&mut captured).await;
    assert_eq!(first.method, Method::POST);
    assert_eq!(second.method, Method::POST);
    let first_body: Value = serde_json::from_slice(&first.body).expect("first request body");
    let second_body: Value = serde_json::from_slice(&second.body).expect("retry request body");
    assert_eq!(first_body["messages"][0]["content"][0]["type"], "thinking");
    assert!(second_body.get("thinking").is_none());
    assert_eq!(
        second_body["messages"][0]["content"],
        json!([{ "type": "text", "text": "visible" }])
    );

    stop_test_proxy(&app).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_claude_desktop_openai_chat_http_conformance_maps_both_directions() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle, mut captured) = spawn_capture_upstream(
        StatusCode::OK,
        "application/json",
        r#"{
            "id":"chatcmpl-conformance",
            "model":"gpt-4.1",
            "choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"chat ok"}}],
            "usage":{"prompt_tokens":12,"completion_tokens":5}
        }"#,
    )
    .await;
    let (app, state) = make_app_with_claude_desktop_provider_and_state(
        "password",
        "csrf-token",
        claude_desktop_protocol_provider(&upstream_base, "openai_chat", "gpt-4.1"),
    );
    state
        .db
        .set_setting("claude_desktop_gateway_token", "desktop-gateway-token")
        .expect("store gateway token");
    let listen_url = start_test_proxy(&app, free_tcp_port()).await;

    let response = reqwest::Client::new()
        .post(format!(
            "{listen_url}/claude-desktop/v1/messages?trace=chat"
        ))
        .bearer_auth("desktop-gateway-token")
        .json(&json!({
            "model": "claude-sonnet-4-6",
            "system": "Be concise.",
            "messages": [{ "role": "user", "content": "hello" }],
            "tool_choice": { "type": "tool", "name": "lookup_price" },
            "tools": [{
                "name": "lookup_price",
                "input_schema": { "type": "object" }
            }]
        }))
        .send()
        .await
        .expect("Claude Desktop OpenAI Chat request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let response_body: Value = response.json().await.expect("Anthropic response body");
    assert_eq!(response_body["type"], json!("message"));
    assert_eq!(response_body["content"][0]["text"], json!("chat ok"));
    assert_eq!(response_body["usage"]["input_tokens"], json!(12));
    assert_eq!(response_body["usage"]["output_tokens"], json!(5));

    let request = next_captured_request(&mut captured).await;
    assert_eq!(request.method, Method::POST);
    assert_eq!(request.path_and_query, "/v1/chat/completions?trace=chat");
    assert_eq!(
        request
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some("Bearer openai-desktop-token")
    );
    let body: Value = serde_json::from_slice(&request.body).expect("OpenAI Chat body");
    assert_eq!(body["model"], json!("gpt-4.1"));
    assert_eq!(body["messages"][0]["role"], json!("system"));
    assert_eq!(
        body["tool_choice"],
        json!({ "type": "function", "function": { "name": "lookup_price" } })
    );
    assert_eq!(body["tools"][0]["function"]["name"], json!("lookup_price"));

    stop_test_proxy(&app).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_claude_desktop_openai_responses_http_conformance_maps_both_directions() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle, mut captured) = spawn_capture_upstream(
        StatusCode::OK,
        "application/json",
        r#"{
            "id":"resp-conformance",
            "model":"gpt-5.1",
            "status":"completed",
            "output":[{"type":"message","content":[{"type":"output_text","text":"responses ok"}]}],
            "usage":{"input_tokens":20,"output_tokens":4}
        }"#,
    )
    .await;
    let (app, state) = make_app_with_claude_desktop_provider_and_state(
        "password",
        "csrf-token",
        claude_desktop_protocol_provider(&upstream_base, "openai_responses", "gpt-5.1"),
    );
    state
        .db
        .set_setting("claude_desktop_gateway_token", "desktop-gateway-token")
        .expect("store gateway token");
    let listen_url = start_test_proxy(&app, free_tcp_port()).await;

    let response = reqwest::Client::new()
        .post(format!(
            "{listen_url}/claude-desktop/v1/messages?trace=responses"
        ))
        .bearer_auth("desktop-gateway-token")
        .json(&json!({
            "model": "claude-sonnet-4-6",
            "messages": [{ "role": "user", "content": "hello" }],
            "thinking": { "type": "enabled", "budget_tokens": 2000 },
            "tool_choice": { "type": "tool", "name": "lookup_price" },
            "tools": [{
                "name": "lookup_price",
                "input_schema": { "type": "object" }
            }]
        }))
        .send()
        .await
        .expect("Claude Desktop Responses request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let response_body: Value = response.json().await.expect("Anthropic response body");
    assert_eq!(response_body["content"][0]["text"], json!("responses ok"));
    assert_eq!(response_body["usage"]["input_tokens"], json!(20));
    assert_eq!(response_body["usage"]["output_tokens"], json!(4));

    let request = next_captured_request(&mut captured).await;
    assert_eq!(request.method, Method::POST);
    assert_eq!(request.path_and_query, "/v1/responses?trace=responses");
    let body: Value = serde_json::from_slice(&request.body).expect("Responses body");
    assert_eq!(body["model"], json!("gpt-5.1"));
    assert_eq!(body["input"][0]["role"], json!("user"));
    assert_eq!(body["reasoning"], json!({ "effort": "low" }));
    assert_eq!(
        body["tool_choice"],
        json!({ "type": "function", "name": "lookup_price" })
    );
    assert_eq!(body["tools"][0]["name"], json!("lookup_price"));

    stop_test_proxy(&app).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_claude_desktop_gemini_native_http_conformance_maps_both_directions() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle, mut captured) = spawn_capture_upstream(
        StatusCode::OK,
        "application/json",
        r#"{
            "responseId":"gemini-conformance",
            "modelVersion":"gemini-2.5-pro",
            "candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"gemini ok"}]}}],
            "usageMetadata":{"promptTokenCount":10,"totalTokenCount":13}
        }"#,
    )
    .await;
    let (app, state) = make_app_with_claude_desktop_provider_and_state(
        "password",
        "csrf-token",
        claude_desktop_protocol_provider(&upstream_base, "gemini_native", "gemini-2.5-pro"),
    );
    state
        .db
        .set_setting("claude_desktop_gateway_token", "desktop-gateway-token")
        .expect("store gateway token");
    let listen_url = start_test_proxy(&app, free_tcp_port()).await;

    let response = reqwest::Client::new()
        .post(format!(
            "{listen_url}/claude-desktop/v1/messages?trace=gemini"
        ))
        .bearer_auth("desktop-gateway-token")
        .json(&json!({
            "model": "claude-sonnet-4-6",
            "messages": [{ "role": "user", "content": "hello" }],
            "tool_choice": { "type": "tool", "name": "lookup_price" },
            "tools": [{
                "name": "lookup_price",
                "input_schema": { "type": "object" }
            }]
        }))
        .send()
        .await
        .expect("Claude Desktop Gemini request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let response_body: Value = response.json().await.expect("Anthropic response body");
    assert_eq!(response_body["content"][0]["text"], json!("gemini ok"));
    assert_eq!(response_body["usage"]["input_tokens"], json!(10));
    assert_eq!(response_body["usage"]["output_tokens"], json!(3));

    let request = next_captured_request(&mut captured).await;
    assert_eq!(request.method, Method::POST);
    assert_eq!(
        request.path_and_query,
        "/v1beta/models/gemini-2.5-pro:generateContent?trace=gemini"
    );
    assert_eq!(
        request
            .headers
            .get("x-goog-api-key")
            .and_then(|value| value.to_str().ok()),
        Some("gemini-desktop-token")
    );
    let body: Value = serde_json::from_slice(&request.body).expect("Gemini body");
    assert!(body.get("model").is_none());
    assert_eq!(body["contents"][0]["parts"][0]["text"], json!("hello"));
    assert_eq!(
        body["toolConfig"]["functionCallingConfig"],
        json!({ "mode": "ANY", "allowedFunctionNames": ["lookup_price"] })
    );

    stop_test_proxy(&app).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_vertex_full_endpoint_http_conformance_does_not_append_request_path() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle, mut captured) =
        spawn_capture_upstream(StatusCode::OK, "application/json", r#"{"ok":true}"#).await;
    let endpoint = format!(
        "{upstream_base}/v1/projects/p/locations/us/publishers/anthropic/models/claude:rawPredict?api-version=1"
    );
    let mut provider = claude_provider_with_base_url(&endpoint);
    provider.meta = Some(ProviderMeta {
        is_full_url: Some(true),
        ..ProviderMeta::default()
    });
    let app = make_app_with_claude_provider("password", "csrf-token", provider);
    let listen_url = start_test_proxy(&app, free_tcp_port()).await;

    let response = reqwest::Client::new()
        .post(format!(
            "{listen_url}/v1/messages?stream=false&trace=vertex"
        ))
        .json(&json!({
            "model": "claude-sonnet-4-6",
            "messages": [{ "role": "user", "content": "hello" }]
        }))
        .send()
        .await
        .expect("Vertex full endpoint request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let request = next_captured_request(&mut captured).await;
    assert_eq!(request.method, Method::POST);
    assert_eq!(
        request.path_and_query,
        "/v1/projects/p/locations/us/publishers/anthropic/models/claude:rawPredict?api-version=1&stream=false&trace=vertex"
    );

    stop_test_proxy(&app).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_recent_logs_capture_request_summary_with_sanitized_query() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle) = spawn_json_upstream().await;
    let proxy_port = free_tcp_port();
    let app = make_app_with_claude_provider(
        "password",
        "csrf-token",
        claude_provider_with_base_url(&upstream_base),
    );

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude",
                    "enableLogging": true
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    let listen_url = status["listenUrl"].as_str().expect("listen URL");

    let proxy_response = reqwest::Client::new()
        .post(format!(
            "{listen_url}/v1/messages?key=secret-token&safe=visible"
        ))
        .json(&json!({ "messages": [] }))
        .send()
        .await
        .expect("proxy request");
    assert_eq!(proxy_response.status(), reqwest::StatusCode::OK);

    let res = dispatch(
        app.clone(),
        request(Method::GET, "/api/proxy/logs/recent", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let logs: Value = json_body(res).await;
    let logs = logs.as_array().expect("logs array");
    assert_eq!(logs.len(), 1);
    let log = &logs[0];
    assert_eq!(log["app"], json!("claude"));
    assert_eq!(log["method"], json!("POST"));
    assert_eq!(log["status"], json!(200));
    assert_eq!(log["path"], json!("/v1/messages?key=***&safe=visible"));
    assert!(!log["path"]
        .as_str()
        .unwrap_or_default()
        .contains("secret-token"));
    assert!(log["durationMs"].as_u64().is_some());
    assert!(log["error"].is_null());

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_request_logs_persist_response_usage_and_cost_to_database() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle) = spawn_text_upstream(
        StatusCode::OK,
        "application/json",
        r#"{
            "id": "msg_test",
            "model": "claude-sonnet-4-20250514",
            "usage": {
                "input_tokens": 1000,
                "output_tokens": 500,
                "cache_read_input_tokens": 100,
                "cache_creation_input_tokens": 20
            }
        }"#,
    )
    .await;
    let proxy_port = free_tcp_port();
    let (app, state) = make_app_with_claude_provider_and_state(
        "password",
        "csrf-token",
        claude_provider_with_base_url(&upstream_base),
    );

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude",
                    "enableLogging": true
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    let listen_url = status["listenUrl"].as_str().expect("listen URL");

    let proxy_response = reqwest::Client::new()
        .post(format!("{listen_url}/v1/messages"))
        .json(&json!({
            "model": "claude-sonnet-4-20250514",
            "metadata": {
                "sessionId": "proxy-session-1"
            },
            "messages": []
        }))
        .send()
        .await
        .expect("proxy request");
    assert_eq!(proxy_response.status(), reqwest::StatusCode::OK);

    let logs = state
        .db
        .recent_proxy_request_logs(1)
        .expect("recent proxy request logs");
    assert_eq!(logs.len(), 1);
    let log = &logs[0];
    assert_eq!(log.app_type, "claude");
    assert_eq!(log.model, "claude-sonnet-4-20250514");
    assert_eq!(
        log.request_model.as_deref(),
        Some("claude-sonnet-4-20250514")
    );
    assert_eq!(log.session_id.as_deref(), Some("proxy-session-1"));
    assert_eq!(log.input_tokens, 1000);
    assert_eq!(log.output_tokens, 500);
    assert_eq!(log.cache_read_tokens, 100);
    assert_eq!(log.cache_creation_tokens, 20);
    assert_eq!(log.input_cost_usd, "0.003");
    assert_eq!(log.output_cost_usd, "0.0075");
    assert_eq!(log.cache_read_cost_usd, "0.00003");
    assert_eq!(log.cache_creation_cost_usd, "0.000075");
    assert_eq!(log.total_cost_usd, "0.010605");
    assert_eq!(log.status_code, 200);
    assert!(!log.is_streaming);
    assert_eq!(log.cost_multiplier, "1");

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_recent_logs_sanitize_upstream_error_query_values() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let upstream_port = free_tcp_port();
    let upstream_base = format!("http://127.0.0.1:{upstream_port}");
    let proxy_port = free_tcp_port();
    let app = make_app_with_claude_provider(
        "password",
        "csrf-token",
        claude_provider_with_base_url(&upstream_base),
    );

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude",
                    "enableLogging": true
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    let listen_url = status["listenUrl"].as_str().expect("listen URL");

    let proxy_response = reqwest::Client::new()
        .post(format!(
            "{listen_url}/v1/messages?key=secret-token&safe=visible"
        ))
        .json(&json!({ "messages": [] }))
        .send()
        .await
        .expect("proxy request");
    assert_eq!(proxy_response.status(), reqwest::StatusCode::BAD_GATEWAY);

    let res = dispatch(
        app.clone(),
        request(Method::GET, "/api/proxy/logs/recent", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let logs: Value = json_body(res).await;
    let logs = logs.as_array().expect("logs array");
    assert_eq!(logs.len(), 1);
    let log = &logs[0];
    assert_eq!(log["path"], json!("/v1/messages?key=***&safe=visible"));
    let error = log["error"].as_str().expect("error string");
    assert!(error.contains("Proxy upstream request failed"));
    assert!(!error.contains("secret-token"));
    assert!(!error.contains("key=secret-token"));

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
}

#[tokio::test]
#[serial]
async fn proxy_streaming_response_is_passed_through_without_conversion() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle) = spawn_text_upstream(
        StatusCode::OK,
        "text/event-stream",
        "data: one\n\ndata: two\n\n",
    )
    .await;
    let proxy_port = free_tcp_port();
    let app = make_app_with_claude_provider(
        "password",
        "csrf-token",
        claude_provider_with_base_url(&upstream_base),
    );

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude",
                    "streamingFirstByteTimeout": 1,
                    "streamingIdleTimeout": 1,
                    "apps": {
                        "claude": {
                            "streamingFirstByteTimeout": 1,
                            "streamingIdleTimeout": 1
                        }
                    }
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    let listen_url = status["listenUrl"].as_str().expect("listen URL");

    let proxy_response = reqwest::Client::new()
        .post(format!("{listen_url}/v1/messages"))
        .header("accept", "text/event-stream")
        .json(&json!({ "messages": [] }))
        .send()
        .await
        .expect("proxy request");
    assert_eq!(proxy_response.status(), reqwest::StatusCode::OK);
    let content_type = proxy_response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = proxy_response.text().await.expect("streaming body");
    assert!(content_type.contains("text/event-stream"));
    assert_eq!(body, "data: one\n\ndata: two\n\n");

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_streaming_request_logs_update_usage_after_stream_finishes() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle) = spawn_text_upstream(
        StatusCode::OK,
        "text/event-stream",
        "data: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-sonnet-4-20250514\",\"usage\":{\"input_tokens\":1000,\"cache_read_input_tokens\":100}}}\n\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":500}}\n\n",
    )
    .await;
    let proxy_port = free_tcp_port();
    let (app, state) = make_app_with_claude_provider_and_state(
        "password",
        "csrf-token",
        claude_provider_with_base_url(&upstream_base),
    );

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude",
                    "enableLogging": true,
                    "streamingFirstByteTimeout": 1,
                    "streamingIdleTimeout": 1,
                    "apps": {
                        "claude": {
                            "streamingFirstByteTimeout": 1,
                            "streamingIdleTimeout": 1
                        }
                    }
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    let listen_url = status["listenUrl"].as_str().expect("listen URL");

    let proxy_response = reqwest::Client::new()
        .post(format!("{listen_url}/v1/messages"))
        .header("accept", "text/event-stream")
        .json(&json!({
            "model": "claude-sonnet-4-20250514",
            "messages": []
        }))
        .send()
        .await
        .expect("proxy request");
    assert_eq!(proxy_response.status(), reqwest::StatusCode::OK);
    let body = proxy_response.text().await.expect("streaming body");
    assert!(body.contains("message_start"));
    assert!(body.contains("message_delta"));

    let logs = state
        .db
        .recent_proxy_request_logs(1)
        .expect("recent proxy request logs");
    assert_eq!(logs.len(), 1);
    let log = &logs[0];
    assert!(log.is_streaming);
    assert!(log.first_token_ms.is_some());
    assert_eq!(log.model, "claude-sonnet-4-20250514");
    assert_eq!(log.input_tokens, 1000);
    assert_eq!(log.output_tokens, 500);
    assert_eq!(log.cache_read_tokens, 100);
    assert_eq!(log.total_cost_usd, "0.01053");

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_streaming_first_byte_timeout_returns_bad_gateway() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle) =
        spawn_delayed_stream_upstream(Duration::from_secs(2), None).await;
    let proxy_port = free_tcp_port();
    let app = make_app_with_claude_provider(
        "password",
        "csrf-token",
        claude_provider_with_base_url(&upstream_base),
    );

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude",
                    "streamingFirstByteTimeout": 1,
                    "streamingIdleTimeout": 5,
                    "apps": {
                        "claude": {
                            "streamingFirstByteTimeout": 1,
                            "streamingIdleTimeout": 5
                        }
                    }
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    let listen_url = status["listenUrl"].as_str().expect("listen URL");

    let proxy_response = reqwest::Client::new()
        .post(format!("{listen_url}/v1/messages"))
        .header("accept", "text/event-stream")
        .json(&json!({ "messages": [] }))
        .send()
        .await
        .expect("proxy request");
    assert_eq!(proxy_response.status(), reqwest::StatusCode::BAD_GATEWAY);
    let body = proxy_response.text().await.expect("error body");
    assert!(
        body.contains("first byte"),
        "expected first-byte timeout error, got {body}"
    );

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_streaming_idle_timeout_terminates_body_after_first_chunk() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (upstream_base, upstream_handle) =
        spawn_delayed_stream_upstream(Duration::from_millis(10), Some(Duration::from_secs(2)))
            .await;
    let proxy_port = free_tcp_port();
    let app = make_app_with_claude_provider(
        "password",
        "csrf-token",
        claude_provider_with_base_url(&upstream_base),
    );

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude",
                    "streamingFirstByteTimeout": 1,
                    "streamingIdleTimeout": 1,
                    "apps": {
                        "claude": {
                            "streamingFirstByteTimeout": 1,
                            "streamingIdleTimeout": 1
                        }
                    }
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    let listen_url = status["listenUrl"].as_str().expect("listen URL");

    let proxy_response = reqwest::Client::new()
        .post(format!("{listen_url}/v1/messages"))
        .header("accept", "text/event-stream")
        .json(&json!({ "messages": [] }))
        .send()
        .await
        .expect("proxy request");
    assert_eq!(proxy_response.status(), reqwest::StatusCode::OK);

    let mut stream = proxy_response.bytes_stream();
    let first = stream
        .next()
        .await
        .expect("first chunk should arrive")
        .expect("first chunk should be ok");
    assert_eq!(first, Bytes::from_static(b"data: first\n\n"));

    match stream.next().await {
        Some(Err(_)) => {}
        Some(Ok(bytes)) => panic!("expected idle timeout error, got chunk {bytes:?}"),
        None => panic!("expected idle timeout error before stream end"),
    }

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
    upstream_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_failover_switches_backup_provider_to_current() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (primary_base, primary_handle) =
        spawn_text_upstream(StatusCode::INTERNAL_SERVER_ERROR, "application/json", "{}").await;
    let (backup_base, backup_handle) =
        spawn_text_upstream(StatusCode::OK, "application/json", r#"{"ok":true}"#).await;
    let proxy_port = free_tcp_port();
    let app = make_app_with_claude_failover(
        "password",
        "csrf-token",
        claude_provider_with_id_base_url("claude-primary", "Claude Primary", &primary_base),
        claude_provider_with_id_base_url("claude-backup", "Claude Backup", &backup_base),
    );

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude",
                    "apps": {
                        "claude": {
                            "enabled": true,
                            "autoFailoverEnabled": true,
                            "maxRetries": 1,
                            "streamingFirstByteTimeout": 1,
                            "streamingIdleTimeout": 5
                        }
                    }
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    let listen_url = status["listenUrl"].as_str().expect("listen URL");

    let proxy_response = reqwest::Client::new()
        .post(format!("{listen_url}/v1/messages"))
        .json(&json!({ "messages": [] }))
        .send()
        .await
        .expect("proxy request");
    let proxy_status = proxy_response.status();
    let proxy_body = proxy_response.text().await.expect("proxy response body");
    assert_eq!(
        proxy_status,
        reqwest::StatusCode::OK,
        "unexpected proxy response: {proxy_body}"
    );
    assert_eq!(proxy_body, r#"{"ok":true}"#);

    let res = dispatch(
        app.clone(),
        request(Method::GET, "/api/providers/claude/current", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let current: Value = json_body(res).await;
    assert_eq!(current, json!("claude-backup"));

    let res = dispatch(app.clone(), request(Method::GET, "/api/proxy/status", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    assert_eq!(status["failoverCount"], json!(1));
    assert_eq!(status["lastFailoverFrom"], json!("Claude Primary"));
    assert_eq!(status["lastFailoverTo"], json!("Claude Backup"));

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
    primary_handle.abort();
    backup_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_failover_uses_db_queue_before_backup_current() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (primary_base, primary_handle) =
        spawn_text_upstream(StatusCode::INTERNAL_SERVER_ERROR, "application/json", "{}").await;
    let (queued_base, queued_handle) =
        spawn_text_upstream(StatusCode::OK, "application/json", r#"{"source":"queued"}"#).await;
    let (backup_base, backup_handle) =
        spawn_text_upstream(StatusCode::OK, "application/json", r#"{"source":"backup"}"#).await;
    let proxy_port = free_tcp_port();
    let app = make_app_with_claude_providers(
        "password",
        "csrf-token",
        "claude-primary",
        Some("claude-backup"),
        vec![
            claude_provider_with_id_base_url("claude-primary", "Claude Primary", &primary_base),
            claude_provider_with_id_base_url("claude-queued", "Claude Queued", &queued_base),
            claude_provider_with_id_base_url("claude-backup", "Claude Backup", &backup_base),
        ],
    );

    let res = dispatch(
        app.clone(),
        request(
            Method::PUT,
            "/api/proxy/failover/claude",
            Some(json!({ "providerIds": ["claude-queued"] })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude",
                    "apps": {
                        "claude": {
                            "enabled": true,
                            "autoFailoverEnabled": true,
                            "maxRetries": 1
                        }
                    }
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    let listen_url = status["listenUrl"].as_str().expect("listen URL");

    let proxy_response = reqwest::Client::new()
        .post(format!("{listen_url}/v1/messages"))
        .json(&json!({ "messages": [] }))
        .send()
        .await
        .expect("proxy request");
    assert_eq!(proxy_response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        proxy_response.text().await.expect("proxy response body"),
        r#"{"source":"queued"}"#
    );

    let res = dispatch(
        app.clone(),
        request(Method::GET, "/api/providers/claude/current", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let current: Value = json_body(res).await;
    assert_eq!(current, json!("claude-queued"));

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
    primary_handle.abort();
    queued_handle.abort();
    backup_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_streaming_first_byte_timeout_fails_over_before_sending_body() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();
    let (primary_base, primary_handle) =
        spawn_delayed_stream_upstream(Duration::from_secs(2), None).await;
    let (backup_base, backup_handle) =
        spawn_text_upstream(StatusCode::OK, "text/event-stream", "data: backup\n\n").await;
    let proxy_port = free_tcp_port();
    let app = make_app_with_claude_failover(
        "password",
        "csrf-token",
        claude_provider_with_id_base_url("claude-primary", "Claude Primary", &primary_base),
        claude_provider_with_id_base_url("claude-backup", "Claude Backup", &backup_base),
    );

    let res = dispatch(
        app.clone(),
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": proxy_port,
                    "bindApp": "claude",
                    "streamingFirstByteTimeout": 1,
                    "streamingIdleTimeout": 5,
                    "apps": {
                        "claude": {
                            "enabled": true,
                            "autoFailoverEnabled": true,
                            "maxRetries": 1,
                            "streamingFirstByteTimeout": 1,
                            "streamingIdleTimeout": 5
                        }
                    }
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    let listen_url = status["listenUrl"].as_str().expect("listen URL");

    let proxy_response = reqwest::Client::new()
        .post(format!("{listen_url}/v1/messages"))
        .header("accept", "text/event-stream")
        .json(&json!({ "messages": [] }))
        .send()
        .await
        .expect("proxy request");
    assert_eq!(proxy_response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        proxy_response.text().await.expect("proxy body"),
        "data: backup\n\n"
    );

    let res = dispatch(
        app.clone(),
        request(Method::GET, "/api/providers/claude/current", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let current: Value = json_body(res).await;
    assert_eq!(current, json!("claude-backup"));

    let res = dispatch(app.clone(), request(Method::GET, "/api/proxy/status", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let status: Value = json_body(res).await;
    assert_eq!(status["failoverCount"], json!(1));
    assert_eq!(status["lastFailoverFrom"], json!("Claude Primary"));
    assert_eq!(status["lastFailoverTo"], json!("Claude Backup"));

    let _ = dispatch(app, request(Method::POST, "/api/proxy/stop", None)).await;
    primary_handle.abort();
    backup_handle.abort();
}

#[tokio::test]
#[serial]
async fn proxy_gemini_takeover_rejects_oauth_provider_and_allows_api_key_provider() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    let _account_guard = setup();

    let oauth_app =
        make_app_with_gemini_provider("password", "csrf-token", google_oauth_gemini_provider());
    let res = dispatch(
        oauth_app.clone(),
        request(
            Method::PUT,
            "/api/proxy/takeover/gemini",
            Some(json!({ "enabled": true })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body: Value = json_body(res).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Gemini OAuth"),
        "unexpected error body: {body}"
    );

    let res = dispatch(
        oauth_app,
        request(
            Method::POST,
            "/api/proxy/start",
            Some(json!({
                "settings": {
                    "host": "127.0.0.1",
                    "port": 4569,
                    "bindApp": "gemini",
                    "apps": {
                        "gemini": { "enabled": true }
                    }
                }
            })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body: Value = json_body(res).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Gemini OAuth"),
        "unexpected error body: {body}"
    );

    let api_key_app =
        make_app_with_gemini_provider("password", "csrf-token", generic_gemini_provider());
    let res = dispatch(
        api_key_app,
        request(
            Method::PUT,
            "/api/proxy/takeover/gemini",
            Some(json!({ "enabled": true })),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let result: Value = json_body(res).await;
    assert_eq!(result["app"], json!("gemini"));
    assert_eq!(result["enabled"], json!(true));
    assert_eq!(result["status"]["takeover"]["gemini"], json!(true));
}
