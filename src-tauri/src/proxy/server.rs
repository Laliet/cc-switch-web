use std::{
    collections::{HashMap, VecDeque},
    io::ErrorKind,
    net::{IpAddr, SocketAddr},
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use axum::{
    body::{to_bytes, Body, Bytes},
    extract::State,
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::Response,
    routing::any,
    Router,
};
use futures::{stream, StreamExt};
use reqwest::Client;
use tokio::{
    net::TcpListener,
    sync::{oneshot, Mutex, RwLock},
    task::JoinHandle,
};

use crate::{
    app_config::AppType,
    error::AppError,
    provider::Provider,
    services::provider::ProviderService,
    settings::{self, ProxyAppSettings, ProxySettings},
    store::AppState,
};

use super::{
    adapters::{adapter_for, insert_auth_headers},
    live,
    service::ensure_gemini_takeover_supported,
    types::{
        ProxyActiveTarget, ProxyRecentLog, ProxyStats, ProxyStatus, ProxyTakeoverStatus,
        ProxyTestResult, PROXY_BODY_LIMIT_BYTES,
    },
};

const PROXY_RECENT_LOG_LIMIT: usize = 100;
const PROXY_LOG_VALUE_LIMIT: usize = 256;
const PROXY_LOG_PATH_LIMIT: usize = 2048;
const PROXY_CIRCUIT_OPEN_DURATION: Duration = Duration::from_secs(60);

struct ProxyRuntime {
    handle: Mutex<Option<ProxyHandle>>,
    stats: Arc<RwLock<ProxyStats>>,
    recent_logs: Arc<RwLock<VecDeque<ProxyRecentLog>>>,
    health: Arc<RwLock<HashMap<String, ProviderRuntimeHealth>>>,
}

struct ProxyHandle {
    shutdown: oneshot::Sender<()>,
    join: JoinHandle<()>,
    listen_url: String,
    address: String,
    port: u16,
    settings: ProxySettings,
}

#[derive(Clone)]
struct ProxyHandlerState {
    app_state: Arc<AppState>,
    client: Client,
    settings: ProxySettings,
    stats: Arc<RwLock<ProxyStats>>,
    recent_logs: Arc<RwLock<VecDeque<ProxyRecentLog>>>,
    health: Arc<RwLock<HashMap<String, ProviderRuntimeHealth>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderCircuitState {
    Healthy,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
struct ProviderRuntimeHealth {
    state: ProviderCircuitState,
    failure_count: u64,
    last_failure_at: Option<Instant>,
    opened_at: Option<Instant>,
}

impl Default for ProviderRuntimeHealth {
    fn default() -> Self {
        Self {
            state: ProviderCircuitState::Healthy,
            failure_count: 0,
            last_failure_at: None,
            opened_at: None,
        }
    }
}

static RUNTIME: OnceLock<Arc<ProxyRuntime>> = OnceLock::new();

fn runtime() -> Arc<ProxyRuntime> {
    RUNTIME
        .get_or_init(|| {
            Arc::new(ProxyRuntime {
                handle: Mutex::new(None),
                stats: Arc::new(RwLock::new(ProxyStats::default())),
                recent_logs: Arc::new(RwLock::new(VecDeque::new())),
                health: Arc::new(RwLock::new(HashMap::new())),
            })
        })
        .clone()
}

pub fn parse_proxy_app(value: &str) -> Result<AppType, AppError> {
    let app = AppType::parse_supported(value)?;
    if matches!(app, AppType::Omo) {
        return Err(AppError::localized(
            "proxy.omo.unsupported",
            "代理暂不支持 OMO，请选择 OpenCode。",
            "Proxy does not support OMO yet; choose OpenCode.",
        ));
    }
    Ok(app)
}

fn takeover_apps(settings: &ProxySettings) -> Vec<AppType> {
    let mut apps = Vec::new();
    if settings.apps.claude.enabled {
        apps.push(AppType::Claude);
    }
    if settings.apps.codex.enabled {
        apps.push(AppType::Codex);
    }
    if settings.apps.gemini.enabled {
        apps.push(AppType::Gemini);
    }
    if settings.apps.opencode.enabled {
        apps.push(AppType::Opencode);
    }
    apps
}

pub(crate) fn validate_settings(settings: &ProxySettings) -> Result<(), AppError> {
    let host = settings.host.trim();
    if host.is_empty() {
        return Err(AppError::InvalidInput("Proxy host is required".into()));
    }
    let _ip: IpAddr = host
        .parse()
        .map_err(|_| AppError::InvalidInput("Proxy host must be an IP address".into()))?;
    if settings.port == 0 {
        return Err(AppError::InvalidInput("Proxy port is required".into()));
    }
    if let Some(upstream) = settings.upstream_proxy.as_deref() {
        let upstream = upstream.trim();
        if !upstream.is_empty()
            && !(upstream.starts_with("http://") || upstream.starts_with("https://"))
        {
            return Err(AppError::InvalidInput(
                "Upstream proxy must start with http:// or https://".into(),
            ));
        }
    }
    parse_proxy_app(&settings.bind_app)?;
    Ok(())
}

fn bind_listener_error(addr: SocketAddr, err: std::io::Error) -> AppError {
    if err.kind() == ErrorKind::AddrInUse {
        return AppError::localized(
            "proxy.port.in_use",
            format!(
                "代理端口 {} 已被占用。代理可能已在另一个 cc-switch-web 实例中运行，或有其他进程正在使用该端口；请先停止旧实例，或换一个端口。",
                addr.port()
            ),
            format!(
                "Proxy port {} is already in use. The proxy may already be running in another cc-switch-web instance, or another process is using the port; stop the old instance first or choose another port.",
                addr.port()
            ),
        );
    }

    AppError::Config(format!("Failed to bind proxy listener on {addr}: {err}"))
}

fn build_client(settings: &ProxySettings) -> Result<Client, AppError> {
    let mut builder = Client::builder()
        .user_agent("cc-switch-local-proxy")
        .connect_timeout(Duration::from_secs(15));

    if let Some(upstream) = settings.upstream_proxy.as_deref() {
        let upstream = upstream.trim();
        if !upstream.is_empty() {
            let proxy = reqwest::Proxy::all(upstream)
                .map_err(|e| AppError::Config(format!("Invalid upstream proxy: {e}")))?;
            builder = builder.proxy(proxy);
        }
    }

    builder
        .build()
        .map_err(|e| AppError::Config(format!("Failed to build proxy client: {e}")))
}

pub fn current_provider(state: &AppState, app: &AppType) -> Result<Provider, AppError> {
    let guard = state.config.read().map_err(AppError::from)?;
    let manager = guard.get_manager(app).ok_or_else(|| {
        AppError::localized(
            "proxy.provider_app_missing",
            format!("应用 '{}' 尚未配置供应商。", app.as_str()),
            format!("No providers configured for app '{}'.", app.as_str()),
        )
    })?;
    let current = manager.current.trim();
    if current.is_empty() {
        return Err(AppError::localized(
            "proxy.current_provider_missing",
            format!("应用 '{}' 尚未选择当前供应商。", app.as_str()),
            format!("No current provider selected for app '{}'.", app.as_str()),
        ));
    }
    manager.providers.get(current).cloned().ok_or_else(|| {
        AppError::localized(
            "proxy.current_provider_not_found",
            format!("当前供应商 '{}' 不存在。", current),
            format!("Current provider '{}' was not found.", current),
        )
    })
}

fn should_skip_request_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "host"
            | "connection"
            | "proxy-connection"
            | "proxy-authorization"
            | "proxy-authenticate"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
    )
}

fn should_skip_response_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection" | "proxy-connection" | "transfer-encoding" | "upgrade" | "content-length"
    )
}

fn route_app(settings: &ProxySettings, uri: &Uri) -> Result<(AppType, Uri), AppError> {
    let path = uri.path();
    if path == "/v1/messages" || path.starts_with("/v1/messages/") {
        return Ok((AppType::Claude, uri.clone()));
    }
    if path.starts_with("/claude/") {
        return Ok((AppType::Claude, strip_prefix(uri, "/claude")?));
    }
    if path == "/v1/chat/completions"
        || path == "/v1/responses"
        || path == "/chat/completions"
        || path == "/responses"
        || path.starts_with("/v1/chat/completions/")
        || path.starts_with("/v1/responses/")
    {
        return Ok((AppType::Codex, uri.clone()));
    }
    if path.starts_with("/v1beta/") || path == "/v1beta" {
        return Ok((AppType::Gemini, uri.clone()));
    }
    if path.starts_with("/gemini/") {
        return Ok((AppType::Gemini, strip_prefix(uri, "/gemini")?));
    }
    parse_proxy_app(&settings.bind_app).map(|app| (app, uri.clone()))
}

fn strip_prefix(uri: &Uri, prefix: &str) -> Result<Uri, AppError> {
    let path = uri.path();
    let stripped = path
        .strip_prefix(prefix)
        .filter(|value| !value.is_empty())
        .unwrap_or("/");
    let path_and_query = match uri.query() {
        Some(query) => format!("{stripped}?{query}"),
        None => stripped.to_string(),
    };
    Uri::builder()
        .path_and_query(path_and_query)
        .build()
        .map_err(|e| AppError::InvalidInput(format!("Invalid proxy request URI: {e}")))
}

fn accepts_event_stream(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false)
}

fn is_streaming_response(response: &reqwest::Response) -> bool {
    let content_type_streaming = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false);
    let transfer_chunked = response
        .headers()
        .get(reqwest::header::TRANSFER_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);
    content_type_streaming || transfer_chunked
}

async fn timeout_app_error<T>(
    duration: Duration,
    future: impl std::future::Future<Output = T>,
    message: &'static str,
) -> Result<T, AppError> {
    tokio::time::timeout(duration, future)
        .await
        .map_err(|_| AppError::Config(message.to_string()))
}

fn remaining_timeout(total: Duration, started_at: Instant) -> Duration {
    total
        .checked_sub(started_at.elapsed())
        .unwrap_or_else(|| Duration::from_millis(1))
}

async fn proxy_handler(
    State(state): State<ProxyHandlerState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let started_at = Instant::now();
    let method_for_log = method.as_str().to_string();
    let fallback_path = sanitize_uri_for_log(&uri);
    {
        let mut stats = state.stats.write().await;
        stats.active_connections += 1;
        stats.total_requests += 1;
        stats.last_request_at = Some(chrono::Utc::now());
    }

    let result = proxy_request(state.clone(), method, uri, headers, body).await;
    let status = result.as_ref().ok().map(|result| result.response.status());
    let success = status
        .as_ref()
        .map(|status| status.is_success())
        .unwrap_or(false);
    let error = result
        .as_ref()
        .err()
        .map(|err| sanitize_error_for_log(&err.to_string()));
    {
        let mut stats = state.stats.write().await;
        stats.active_connections = stats.active_connections.saturating_sub(1);
        if success {
            stats.success_requests += 1;
        } else {
            stats.failed_requests += 1;
        }
        if let Some(error) = &error {
            stats.last_error = Some(error.clone());
        }
    }
    if state.settings.enable_logging {
        let (app, path) = result
            .as_ref()
            .map(|result| (result.app.clone(), result.path.clone()))
            .unwrap_or_else(|_| ("unknown".to_string(), fallback_path));
        push_recent_log(
            &state.recent_logs,
            ProxyRecentLog {
                at: chrono::Utc::now().to_rfc3339(),
                app,
                method: method_for_log,
                path,
                status: status.map(|status| status.as_u16()),
                duration_ms: started_at
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
                error,
            },
        )
        .await;
    }

    match result {
        Ok(result) => result.response,
        Err(err) => Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({ "error": err.to_string() }).to_string(),
            ))
            .unwrap_or_else(|_| Response::new(Body::empty())),
    }
}

struct ProxyRequestResult {
    response: Response,
    app: String,
    path: String,
}

enum UpstreamAttemptError {
    Local(AppError),
    Send(AppError),
}

struct UpstreamResponse {
    provider: Provider,
    response: reqwest::Response,
}

enum StreamingResponseError {
    FirstByte(AppError),
    Other(AppError),
}

impl StreamingResponseError {
    fn into_app_error(self) -> AppError {
        match self {
            Self::FirstByte(err) | Self::Other(err) => err,
        }
    }
}

impl UpstreamAttemptError {
    fn into_app_error(self) -> AppError {
        match self {
            Self::Local(err) | Self::Send(err) => err,
        }
    }
}

async fn proxy_request(
    state: ProxyHandlerState,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Result<ProxyRequestResult, AppError> {
    let (app, routed_uri) = route_app(&state.settings, &uri)?;
    let log_app = app.as_str().to_string();
    let log_path = sanitize_uri_for_log(&routed_uri);
    let request_accepts_stream = accepts_event_stream(&headers);
    let provider = current_provider(&state.app_state, &app)?;
    let body_bytes = to_bytes(body, PROXY_BODY_LIMIT_BYTES)
        .await
        .map_err(|e| AppError::Config(format!("Failed to read proxy request body: {e}")))?;

    let reqwest_method = reqwest::Method::from_bytes(method.as_str().as_bytes())
        .map_err(|e| AppError::InvalidInput(format!("Unsupported method: {e}")))?;
    let mut request_headers = reqwest::header::HeaderMap::new();
    for (name, value) in headers.iter() {
        if should_skip_request_header(name) {
            continue;
        }
        if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()) {
            if let Ok(header_value) = reqwest::header::HeaderValue::from_bytes(value.as_bytes()) {
                request_headers.insert(header_name, header_value);
            }
        }
    }

    let request_started_at = Instant::now();
    let total_timeout = Duration::from_secs(state.settings.non_streaming_timeout.max(1));
    let upstream = send_with_failover(
        &state,
        &app,
        &provider,
        &routed_uri,
        reqwest_method.clone(),
        request_headers.clone(),
        body_bytes.clone(),
        total_timeout,
    )
    .await?;

    let response = if request_accepts_stream || is_streaming_response(&upstream.response) {
        match build_streaming_response(upstream.response, &state.settings).await {
            Ok(response) => response,
            Err(err @ StreamingResponseError::FirstByte(_)) => {
                if upstream.provider.id != provider.id {
                    return Err(err.into_app_error());
                }
                if let Some(response) = retry_streaming_first_byte_failover(
                    &state,
                    &app,
                    &provider,
                    &routed_uri,
                    &reqwest_method,
                    &request_headers,
                    &body_bytes,
                    total_timeout,
                    request_started_at,
                    request_accepts_stream,
                )
                .await?
                {
                    response
                } else {
                    return Err(err.into_app_error());
                }
            }
            Err(err) => return Err(err.into_app_error()),
        }
    } else {
        build_buffered_response(upstream.response, total_timeout, request_started_at).await?
    };
    Ok(ProxyRequestResult {
        response,
        app: log_app,
        path: log_path,
    })
}

#[allow(clippy::too_many_arguments)]
async fn send_with_failover(
    state: &ProxyHandlerState,
    app: &AppType,
    provider: &Provider,
    routed_uri: &Uri,
    method: reqwest::Method,
    request_headers: reqwest::header::HeaderMap,
    body_bytes: Bytes,
    total_timeout: Duration,
) -> Result<UpstreamResponse, AppError> {
    let app_settings = proxy_app_settings(&state.settings, app);
    let failover_enabled = app_settings.auto_failover_enabled && app_settings.max_retries > 0;
    let backup = if failover_enabled {
        backup_provider(&state.app_state, app, &provider.id)?
    } else {
        None
    };
    let current_circuit_allows =
        provider_circuit_allows_request(&state.health, app, &provider.id).await;

    if !current_circuit_allows {
        if let Some(backup) = backup.as_ref() {
            if provider_circuit_allows_request(&state.health, app, &backup.id).await {
                let backup_result = send_upstream_provider(
                    state,
                    app,
                    backup,
                    routed_uri,
                    &method,
                    &request_headers,
                    &body_bytes,
                    total_timeout,
                )
                .await;
                if let Ok(response) = backup_result {
                    if !is_failover_status(response.status()) {
                        record_provider_success(&state.health, app, &backup.id).await;
                        switch_to_failover_provider(state, app, provider, backup).await?;
                        return Ok(UpstreamResponse {
                            provider: backup.clone(),
                            response,
                        });
                    }
                    return Ok(UpstreamResponse {
                        provider: backup.clone(),
                        response,
                    });
                }
            }
        }
    }

    let current_result = send_upstream_provider(
        state,
        app,
        provider,
        routed_uri,
        &method,
        &request_headers,
        &body_bytes,
        total_timeout,
    )
    .await;

    match current_result {
        Ok(response) => {
            if failover_enabled && is_failover_status(response.status()) {
                record_provider_failure(&state.health, app, &provider.id, app_settings.max_retries)
                    .await;
                if let Some(backup) = backup.as_ref() {
                    if provider_circuit_allows_request(&state.health, app, &backup.id).await {
                        let backup_result = send_upstream_provider(
                            state,
                            app,
                            backup,
                            routed_uri,
                            &method,
                            &request_headers,
                            &body_bytes,
                            total_timeout,
                        )
                        .await;
                        match backup_result {
                            Ok(backup_response)
                                if !is_failover_status(backup_response.status()) =>
                            {
                                record_provider_success(&state.health, app, &backup.id).await;
                                switch_to_failover_provider(state, app, provider, backup).await?;
                                return Ok(UpstreamResponse {
                                    provider: backup.clone(),
                                    response: backup_response,
                                });
                            }
                            Ok(_backup_response) => {
                                record_provider_failure(
                                    &state.health,
                                    app,
                                    &backup.id,
                                    app_settings.max_retries,
                                )
                                .await;
                                return Ok(UpstreamResponse {
                                    provider: provider.clone(),
                                    response,
                                });
                            }
                            Err(UpstreamAttemptError::Send(_)) => {
                                record_provider_failure(
                                    &state.health,
                                    app,
                                    &backup.id,
                                    app_settings.max_retries,
                                )
                                .await;
                                return Ok(UpstreamResponse {
                                    provider: provider.clone(),
                                    response,
                                });
                            }
                            Err(UpstreamAttemptError::Local(_)) => {
                                return Ok(UpstreamResponse {
                                    provider: provider.clone(),
                                    response,
                                });
                            }
                        }
                    }
                }
            } else {
                record_provider_success(&state.health, app, &provider.id).await;
            }
            Ok(UpstreamResponse {
                provider: provider.clone(),
                response,
            })
        }
        Err(UpstreamAttemptError::Send(err)) => {
            if failover_enabled {
                record_provider_failure(&state.health, app, &provider.id, app_settings.max_retries)
                    .await;
                if let Some(backup) = backup.as_ref() {
                    if provider_circuit_allows_request(&state.health, app, &backup.id).await {
                        let backup_result = send_upstream_provider(
                            state,
                            app,
                            backup,
                            routed_uri,
                            &method,
                            &request_headers,
                            &body_bytes,
                            total_timeout,
                        )
                        .await;
                        match backup_result {
                            Ok(backup_response)
                                if !is_failover_status(backup_response.status()) =>
                            {
                                record_provider_success(&state.health, app, &backup.id).await;
                                switch_to_failover_provider(state, app, provider, backup).await?;
                                return Ok(UpstreamResponse {
                                    provider: backup.clone(),
                                    response: backup_response,
                                });
                            }
                            Ok(_) | Err(UpstreamAttemptError::Send(_)) => {
                                record_provider_failure(
                                    &state.health,
                                    app,
                                    &backup.id,
                                    app_settings.max_retries,
                                )
                                .await;
                            }
                            Err(UpstreamAttemptError::Local(_)) => {}
                        }
                    }
                }
            }
            Err(err)
        }
        Err(err @ UpstreamAttemptError::Local(_)) => Err(err.into_app_error()),
    }
}

#[allow(clippy::too_many_arguments)]
async fn retry_streaming_first_byte_failover(
    state: &ProxyHandlerState,
    app: &AppType,
    failed_provider: &Provider,
    routed_uri: &Uri,
    method: &reqwest::Method,
    request_headers: &reqwest::header::HeaderMap,
    body_bytes: &Bytes,
    total_timeout: Duration,
    request_started_at: Instant,
    request_accepts_stream: bool,
) -> Result<Option<Response>, AppError> {
    let app_settings = proxy_app_settings(&state.settings, app);
    if !app_settings.auto_failover_enabled || app_settings.max_retries == 0 {
        return Ok(None);
    }

    record_provider_failure(
        &state.health,
        app,
        &failed_provider.id,
        app_settings.max_retries,
    )
    .await;

    let Some(backup) = backup_provider(&state.app_state, app, &failed_provider.id)? else {
        return Ok(None);
    };
    if !provider_circuit_allows_request(&state.health, app, &backup.id).await {
        return Ok(None);
    }

    let backup_result = send_upstream_provider(
        state,
        app,
        &backup,
        routed_uri,
        method,
        request_headers,
        body_bytes,
        total_timeout,
    )
    .await;

    let backup_response = match backup_result {
        Ok(response) if !is_failover_status(response.status()) => response,
        Ok(_) | Err(UpstreamAttemptError::Send(_)) => {
            record_provider_failure(&state.health, app, &backup.id, app_settings.max_retries).await;
            return Ok(None);
        }
        Err(UpstreamAttemptError::Local(_)) => return Ok(None),
    };

    let should_stream = request_accepts_stream || is_streaming_response(&backup_response);
    let response = if should_stream {
        match build_streaming_response(backup_response, &state.settings).await {
            Ok(response) => response,
            Err(StreamingResponseError::FirstByte(_)) => {
                record_provider_failure(&state.health, app, &backup.id, app_settings.max_retries)
                    .await;
                return Ok(None);
            }
            Err(err) => return Err(err.into_app_error()),
        }
    } else {
        build_buffered_response(backup_response, total_timeout, request_started_at).await?
    };

    record_provider_success(&state.health, app, &backup.id).await;
    switch_to_failover_provider(state, app, failed_provider, &backup).await?;
    Ok(Some(response))
}

#[allow(clippy::too_many_arguments)]
async fn send_upstream_provider(
    state: &ProxyHandlerState,
    app: &AppType,
    provider: &Provider,
    routed_uri: &Uri,
    method: &reqwest::Method,
    request_headers: &reqwest::header::HeaderMap,
    body_bytes: &Bytes,
    total_timeout: Duration,
) -> Result<reqwest::Response, UpstreamAttemptError> {
    let adapter = adapter_for(app);
    let base_url = adapter
        .extract_base_url(provider)
        .map_err(UpstreamAttemptError::Local)?;
    let url = adapter
        .build_url(&base_url, routed_uri)
        .map_err(UpstreamAttemptError::Local)?;
    let mut headers = request_headers.clone();
    let auth = adapter
        .extract_auth(provider)
        .map_err(UpstreamAttemptError::Local)?;
    if let Some(auth) = auth {
        insert_auth_headers(&mut headers, adapter, &auth);
    }

    timeout_app_error(
        total_timeout,
        state
            .client
            .request(method.clone(), url)
            .headers(headers)
            .body(body_bytes.clone())
            .send(),
        "Proxy upstream request timed out",
    )
    .await
    .map_err(UpstreamAttemptError::Send)?
    .map_err(|e| UpstreamAttemptError::Send(upstream_request_error(e)))
}

fn proxy_app_settings(settings: &ProxySettings, app: &AppType) -> ProxyAppSettings {
    match app {
        AppType::Claude => settings.apps.claude.clone(),
        AppType::Codex => settings.apps.codex.clone(),
        AppType::Gemini => settings.apps.gemini.clone(),
        AppType::Opencode => settings.apps.opencode.clone(),
        AppType::Omo => ProxyAppSettings::default(),
    }
}

fn backup_provider(
    state: &AppState,
    app: &AppType,
    current_provider_id: &str,
) -> Result<Option<Provider>, AppError> {
    let guard = state.config.read().map_err(AppError::from)?;
    let Some(manager) = guard.get_manager(app) else {
        return Ok(None);
    };
    let Some(backup_id) = manager.backup_current.as_deref() else {
        return Ok(None);
    };
    if backup_id.trim().is_empty() || backup_id == current_provider_id {
        return Ok(None);
    }
    Ok(manager.providers.get(backup_id).cloned())
}

fn is_failover_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn provider_health_key(app: &AppType, provider_id: &str) -> String {
    format!("{}:{provider_id}", app.as_str())
}

async fn provider_circuit_allows_request(
    health: &Arc<RwLock<HashMap<String, ProviderRuntimeHealth>>>,
    app: &AppType,
    provider_id: &str,
) -> bool {
    let key = provider_health_key(app, provider_id);
    let mut guard = health.write().await;
    let Some(entry) = guard.get_mut(&key) else {
        return true;
    };
    match entry.state {
        ProviderCircuitState::Healthy | ProviderCircuitState::HalfOpen => true,
        ProviderCircuitState::Open => {
            let Some(opened_at) = entry.opened_at else {
                entry.state = ProviderCircuitState::HalfOpen;
                return true;
            };
            if opened_at.elapsed() >= PROXY_CIRCUIT_OPEN_DURATION {
                entry.state = ProviderCircuitState::HalfOpen;
                true
            } else {
                false
            }
        }
    }
}

async fn record_provider_success(
    health: &Arc<RwLock<HashMap<String, ProviderRuntimeHealth>>>,
    app: &AppType,
    provider_id: &str,
) {
    let key = provider_health_key(app, provider_id);
    health
        .write()
        .await
        .insert(key, ProviderRuntimeHealth::default());
}

async fn record_provider_failure(
    health: &Arc<RwLock<HashMap<String, ProviderRuntimeHealth>>>,
    app: &AppType,
    provider_id: &str,
    max_retries: u8,
) {
    let key = provider_health_key(app, provider_id);
    let threshold = u64::from(max_retries).saturating_add(1).max(2);
    let mut guard = health.write().await;
    let entry = guard.entry(key).or_default();
    entry.failure_count = entry.failure_count.saturating_add(1);
    entry.last_failure_at = Some(Instant::now());
    if entry.failure_count >= threshold || entry.state == ProviderCircuitState::HalfOpen {
        entry.state = ProviderCircuitState::Open;
        entry.opened_at = Some(Instant::now());
    }
}

async fn switch_to_failover_provider(
    state: &ProxyHandlerState,
    app: &AppType,
    from: &Provider,
    to: &Provider,
) -> Result<(), AppError> {
    ProviderService::switch(&state.app_state, app.clone(), &to.id)?;
    let mut stats = state.stats.write().await;
    stats.failover_count = stats.failover_count.saturating_add(1);
    stats.last_failover_at = Some(chrono::Utc::now());
    stats.last_failover_from = Some(from.name.clone());
    stats.last_failover_to = Some(to.name.clone());
    Ok(())
}

async fn build_buffered_response(
    upstream: reqwest::Response,
    total_timeout: Duration,
    request_started_at: Instant,
) -> Result<Response, AppError> {
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = Response::builder().status(status);
    for (name, value) in upstream.headers() {
        if should_skip_response_header(name) {
            continue;
        }
        if let Ok(header_name) = HeaderName::from_bytes(name.as_str().as_bytes()) {
            if let Ok(header_value) = HeaderValue::from_bytes(value.as_bytes()) {
                builder = builder.header(header_name, header_value);
            }
        }
    }
    let bytes = timeout_app_error(
        remaining_timeout(total_timeout, request_started_at),
        upstream.bytes(),
        "Proxy upstream response body timed out",
    )
    .await?
    .map_err(|e| AppError::Config(format!("Failed to read upstream response: {e}")))?;
    builder
        .body(Body::from(bytes))
        .map_err(|e| AppError::Config(format!("Failed to build proxy response: {e}")))
}

async fn build_streaming_response(
    upstream: reqwest::Response,
    settings: &ProxySettings,
) -> Result<Response, StreamingResponseError> {
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = Response::builder().status(status);
    for (name, value) in upstream.headers() {
        if should_skip_response_header(name) {
            continue;
        }
        if let Ok(header_name) = HeaderName::from_bytes(name.as_str().as_bytes()) {
            if let Ok(header_value) = HeaderValue::from_bytes(value.as_bytes()) {
                builder = builder.header(header_name, header_value);
            }
        }
    }

    let first_byte_timeout = Duration::from_secs(settings.streaming_first_byte_timeout.max(1));
    let idle_timeout = Duration::from_secs(settings.streaming_idle_timeout.max(1));
    let mut upstream_stream = upstream.bytes_stream();
    let first = timeout_app_error(
        first_byte_timeout,
        upstream_stream.next(),
        "Proxy streaming first byte timed out",
    )
    .await
    .map_err(StreamingResponseError::FirstByte)?;

    let Some(first) = first else {
        return builder.body(Body::empty()).map_err(|e| {
            StreamingResponseError::Other(AppError::Config(format!(
                "Failed to build proxy response: {e}"
            )))
        });
    };
    let first = first.map_err(|e| {
        StreamingResponseError::FirstByte(AppError::Config(format!(
            "Failed to read first upstream streaming chunk: {e}"
        )))
    })?;

    let rest = stream::unfold(upstream_stream, move |mut stream| {
        let idle_timeout = idle_timeout;
        async move {
            match tokio::time::timeout(idle_timeout, stream.next()).await {
                Ok(Some(Ok(bytes))) => Some((Ok(bytes), stream)),
                Ok(Some(Err(err))) => Some((
                    Err(std::io::Error::new(std::io::ErrorKind::Other, err)),
                    stream,
                )),
                Ok(None) => None,
                Err(_) => Some((
                    Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Proxy streaming idle timeout",
                    )),
                    stream,
                )),
            }
        }
    });
    let body_stream = stream::once(async move { Ok::<Bytes, std::io::Error>(first) }).chain(rest);

    builder.body(Body::from_stream(body_stream)).map_err(|e| {
        StreamingResponseError::Other(AppError::Config(format!(
            "Failed to build proxy response: {e}"
        )))
    })
}

pub async fn start_proxy(
    state: Arc<AppState>,
    settings: ProxySettings,
) -> Result<ProxyStatus, AppError> {
    validate_settings(&settings)?;
    let client = build_client(&settings)?;
    let addr: SocketAddr = format!("{}:{}", settings.host.trim(), settings.port)
        .parse()
        .map_err(|e| AppError::InvalidInput(format!("Invalid proxy listen address: {e}")))?;

    let rt = runtime();
    let already_running_here = {
        let guard = rt.handle.lock().await;
        guard.as_ref().is_some_and(|handle| {
            handle.address == addr.ip().to_string() && handle.port == addr.port()
        })
    };
    if already_running_here {
        return Ok(status_with_state(Some(&state)).await);
    }
    stop_proxy().await?;

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| bind_listener_error(addr, e))?;
    let actual_addr = listener
        .local_addr()
        .map_err(|e| AppError::Config(format!("Failed to read proxy listener address: {e}")))?;
    let listen_url = format!("http://{actual_addr}");

    let mut applied_takeovers = Vec::new();
    for app in takeover_apps(&settings) {
        let result = (|| {
            live::sync_current_provider_from_live(&state, &app)?;
            let provider = current_provider(&state, &app)?;
            if matches!(app, AppType::Gemini) {
                ensure_gemini_takeover_supported(&provider)?;
            }
            live::apply_takeover(&app, &provider, &listen_url)
        })();
        if let Err(err) = result {
            for applied_app in applied_takeovers.iter().rev() {
                let _ = live::restore_takeover(applied_app);
            }
            return Err(err);
        }
        applied_takeovers.push(app);
    }

    let handler_state = ProxyHandlerState {
        app_state: state.clone(),
        client,
        settings: settings.clone(),
        stats: rt.stats.clone(),
        recent_logs: rt.recent_logs.clone(),
        health: rt.health.clone(),
    };
    let app_router = Router::new()
        .route("/", any(proxy_handler))
        .route("/*path", any(proxy_handler))
        .with_state(handler_state);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let stats = rt.stats.clone();
    let join = tokio::spawn(async move {
        let result = axum::serve(listener, app_router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
        if let Err(err) = result {
            stats.write().await.last_error = Some(err.to_string());
        }
    });

    *rt.stats.write().await = ProxyStats {
        started_at: Some(Instant::now()),
        ..ProxyStats::default()
    };
    rt.recent_logs.write().await.clear();
    rt.health.write().await.clear();
    *rt.handle.lock().await = Some(ProxyHandle {
        shutdown: shutdown_tx,
        join,
        listen_url: listen_url.clone(),
        address: actual_addr.ip().to_string(),
        port: actual_addr.port(),
        settings,
    });

    Ok(status_with_state(Some(&state)).await)
}

pub async fn stop_proxy() -> Result<ProxyStatus, AppError> {
    let rt = runtime();
    if let Some(handle) = rt.handle.lock().await.take() {
        let _ = handle.shutdown.send(());
        let _ = tokio::time::timeout(Duration::from_secs(3), handle.join).await;
    }
    rt.recent_logs.write().await.clear();
    rt.health.write().await.clear();
    Ok(status().await)
}

pub async fn recent_logs() -> Vec<ProxyRecentLog> {
    if !settings::get_settings().proxy.enable_logging {
        return Vec::new();
    }
    runtime().recent_logs.read().await.iter().cloned().collect()
}

pub async fn clear_recent_logs() {
    runtime().recent_logs.write().await.clear();
}

pub async fn update_runtime_settings(settings: ProxySettings) {
    let rt = runtime();
    let mut guard = rt.handle.lock().await;
    if let Some(handle) = guard.as_mut() {
        if !handle.join.is_finished() {
            handle.settings = settings;
        }
    }
}

pub async fn status() -> ProxyStatus {
    status_with_state(None).await
}

async fn status_with_state(state: Option<&Arc<AppState>>) -> ProxyStatus {
    let rt = runtime();
    let guard = rt.handle.lock().await;
    let stats = rt.stats.read().await.clone();
    let settings = settings::get_settings().proxy;
    match guard.as_ref() {
        Some(handle) if !handle.join.is_finished() => {
            let active_targets = state
                .map(|state| active_targets(state, &handle.settings))
                .unwrap_or_default();
            ProxyStatus {
                running: true,
                address: handle.address.clone(),
                port: handle.port,
                listen_url: Some(handle.listen_url.clone()),
                active_connections: stats.active_connections,
                total_requests: stats.total_requests,
                success_requests: stats.success_requests,
                failed_requests: stats.failed_requests,
                success_rate: stats.success_rate(),
                uptime_seconds: stats.uptime().as_secs(),
                active_targets,
                takeover: takeover_status(&handle.settings),
                bind_app: handle.settings.bind_app.clone(),
                last_request_at: stats.last_request_at.map(|value| value.to_rfc3339()),
                last_error: stats.last_error,
                failover_count: stats.failover_count,
                last_failover_at: stats.last_failover_at.map(|value| value.to_rfc3339()),
                last_failover_from: stats.last_failover_from,
                last_failover_to: stats.last_failover_to,
            }
        }
        _ => ProxyStatus {
            running: false,
            address: settings.host.clone(),
            port: settings.port,
            listen_url: None,
            active_connections: 0,
            total_requests: stats.total_requests,
            success_requests: stats.success_requests,
            failed_requests: stats.failed_requests,
            success_rate: stats.success_rate(),
            uptime_seconds: 0,
            active_targets: Vec::new(),
            takeover: takeover_status(&settings),
            bind_app: settings.bind_app,
            last_request_at: stats.last_request_at.map(|value| value.to_rfc3339()),
            last_error: stats.last_error,
            failover_count: stats.failover_count,
            last_failover_at: stats.last_failover_at.map(|value| value.to_rfc3339()),
            last_failover_from: stats.last_failover_from,
            last_failover_to: stats.last_failover_to,
        },
    }
}

pub async fn status_for_state(state: &Arc<AppState>) -> ProxyStatus {
    status_with_state(Some(state)).await
}

fn active_targets(state: &AppState, settings: &ProxySettings) -> Vec<ProxyActiveTarget> {
    takeover_apps(settings)
        .into_iter()
        .filter_map(|app| {
            let provider = current_provider(state, &app).ok()?;
            Some(ProxyActiveTarget {
                app_type: app.as_str().to_string(),
                provider_id: provider.id,
                provider_name: provider.name,
            })
        })
        .collect()
}

fn takeover_status(settings: &ProxySettings) -> ProxyTakeoverStatus {
    ProxyTakeoverStatus {
        claude: settings.apps.claude.enabled,
        codex: settings.apps.codex.enabled,
        gemini: settings.apps.gemini.enabled,
        opencode: settings.apps.opencode.enabled,
        omo: false,
    }
}

pub async fn test_settings(
    state: Arc<AppState>,
    settings: ProxySettings,
) -> Result<ProxyTestResult, AppError> {
    validate_settings(&settings)?;
    let app = parse_proxy_app(&settings.bind_app)?;
    let provider = current_provider(&state, &app)?;
    let adapter = adapter_for(&app);
    let base_url = adapter.extract_base_url(&provider)?;
    let _ = adapter.extract_auth(&provider)?;
    let _ = adapter.build_url(&base_url, &"/".parse::<Uri>().expect("valid root uri"))?;
    let _ = build_client(&settings)?;
    Ok(ProxyTestResult {
        success: true,
        message: "Proxy settings are valid.".to_string(),
        base_url: Some(base_url),
    })
}

pub async fn start_from_saved_settings(state: Arc<AppState>) {
    let settings = settings::get_settings().proxy;
    if settings.enabled && settings.auto_start {
        if let Err(err) = start_proxy(state, settings).await {
            runtime().stats.write().await.last_error = Some(err.to_string());
            log::warn!("Failed to auto-start local proxy: {}", err);
        }
    }
}

async fn push_recent_log(logs: &Arc<RwLock<VecDeque<ProxyRecentLog>>>, log: ProxyRecentLog) {
    let mut guard = logs.write().await;
    while guard.len() >= PROXY_RECENT_LOG_LIMIT {
        guard.pop_front();
    }
    guard.push_back(log);
}

fn sanitize_uri_for_log(uri: &Uri) -> String {
    let mut path = truncate_for_log(uri.path(), PROXY_LOG_PATH_LIMIT);
    let Some(query) = uri.query() else {
        return path;
    };
    if query.is_empty() {
        return path;
    }

    let mut sanitized = String::new();
    for (index, part) in query.split('&').enumerate() {
        if index > 0 {
            sanitized.push('&');
        }
        let (raw_key, raw_value) = part.split_once('=').unwrap_or((part, ""));
        sanitized.push_str(raw_key);
        if !raw_value.is_empty() || part.contains('=') {
            sanitized.push('=');
            if is_sensitive_query_key(raw_key) {
                sanitized.push_str("***");
            } else {
                sanitized.push_str(&truncate_for_log(raw_value, PROXY_LOG_VALUE_LIMIT));
            }
        } else if is_sensitive_query_key(raw_key) {
            sanitized.push_str("=***");
        }
    }

    path.push('?');
    path.push_str(&truncate_for_log(&sanitized, PROXY_LOG_PATH_LIMIT));
    truncate_for_log(&path, PROXY_LOG_PATH_LIMIT)
}

fn sanitize_error_for_log(error: &str) -> String {
    let mut sanitized = String::with_capacity(error.len());
    let mut remainder = error;

    while let Some((prefix, scheme)) = find_next_url(remainder) {
        sanitized.push_str(prefix);
        let url_start = prefix.len();
        let tail = &remainder[url_start..];
        let url_end = tail
            .find(|ch: char| ch.is_whitespace() || matches!(ch, ')' | '"' | '\'' | '<' | '>'))
            .unwrap_or(tail.len());
        let (url, rest) = tail.split_at(url_end);
        sanitized.push_str(&sanitize_url_for_log(url, scheme));
        remainder = rest;
    }

    sanitized.push_str(remainder);
    truncate_for_log(&sanitized, PROXY_LOG_PATH_LIMIT)
}

fn find_next_url(value: &str) -> Option<(&str, &str)> {
    let http = value.find("http://");
    let https = value.find("https://");
    match (http, https) {
        (Some(http), Some(https)) if http < https => Some((&value[..http], "http://")),
        (Some(_), Some(https)) => Some((&value[..https], "https://")),
        (Some(http), None) => Some((&value[..http], "http://")),
        (None, Some(https)) => Some((&value[..https], "https://")),
        (None, None) => None,
    }
}

fn sanitize_url_for_log(url: &str, scheme: &str) -> String {
    let without_scheme = url.strip_prefix(scheme).unwrap_or(url);
    let (authority, path_and_query) = without_scheme
        .split_once('/')
        .map(|(authority, path)| (authority, format!("/{path}")))
        .unwrap_or((without_scheme, "/".to_string()));

    match path_and_query.parse::<Uri>() {
        Ok(uri) => format!("{scheme}{authority}{}", sanitize_uri_for_log(&uri)),
        Err(_) => format!("{scheme}{authority}/***"),
    }
}

fn upstream_request_error(error: reqwest::Error) -> AppError {
    let reason = if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connection failed"
    } else if error.is_request() {
        "request build failed"
    } else if error.is_body() {
        "request body failed"
    } else {
        "request failed"
    };
    AppError::Config(format!("Proxy upstream request failed: {reason}"))
}

fn is_sensitive_query_key(key: &str) -> bool {
    let normalized = key.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "key"
            | "api_key"
            | "apikey"
            | "access_token"
            | "token"
            | "auth"
            | "authorization"
            | "client_secret"
            | "refresh_token"
            | "id_token"
    )
}

fn truncate_for_log(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_string();
    }
    let mut end = limit.saturating_sub(3);
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::{takeover_apps, AppType, ProxySettings};

    #[test]
    fn takeover_apps_does_not_duplicate_claude() {
        let mut settings = ProxySettings::default();
        settings.apps.claude.enabled = true;

        assert_eq!(takeover_apps(&settings), vec![AppType::Claude]);
    }
}
