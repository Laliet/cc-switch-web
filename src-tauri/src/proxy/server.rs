use std::{
    collections::{HashMap, VecDeque},
    io::ErrorKind,
    net::{IpAddr, SocketAddr},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
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
use rust_decimal::Decimal;
use std::str::FromStr;
use tokio::{
    net::TcpListener,
    sync::{oneshot, Mutex, RwLock},
    task::JoinHandle,
};

use crate::{
    app_config::AppType,
    database::{ProxyRequestLogRecord, ProxyRequestUsageUpdate},
    error::AppError,
    provider::{Provider, ProviderType},
    services::provider::ProviderService,
    settings::{self, ProxyAppSettings, ProxySettings},
    store::AppState,
};

use super::{
    adapters::{
        adapter_for, full_endpoint_url, insert_auth_headers, provider_type,
        resolve_auth_for_provider,
    },
    live,
    service::ensure_gemini_takeover_supported,
    types::{
        ProxyActiveTarget, ProxyRecentLog, ProxyStats, ProxyStatus, ProxyTakeoverStatus,
        ProxyTestResult, PROXY_BODY_LIMIT_BYTES,
    },
    usage::{
        calculator::{CostBreakdown, CostCalculator},
        parser::TokenUsage,
    },
};

const PROXY_RECENT_LOG_LIMIT: usize = 100;
const PROXY_LOG_VALUE_LIMIT: usize = 256;
const PROXY_LOG_PATH_LIMIT: usize = 2048;
const PROXY_RESPONSE_LIMIT_BYTES: usize = PROXY_BODY_LIMIT_BYTES;
struct ProxyRuntime {
    handle: Mutex<Option<ProxyHandle>>,
    settings: Arc<RwLock<ProxySettings>>,
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
    settings: Arc<RwLock<ProxySettings>>,
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
    recovery_success_count: u64,
    window_requests: u64,
    window_failures: u64,
    last_failure_at: Option<Instant>,
    opened_at: Option<Instant>,
}

impl Default for ProviderRuntimeHealth {
    fn default() -> Self {
        Self {
            state: ProviderCircuitState::Healthy,
            failure_count: 0,
            recovery_success_count: 0,
            window_requests: 0,
            window_failures: 0,
            last_failure_at: None,
            opened_at: None,
        }
    }
}

static RUNTIME: OnceLock<Arc<ProxyRuntime>> = OnceLock::new();
static REQUEST_LOG_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn runtime() -> Arc<ProxyRuntime> {
    RUNTIME
        .get_or_init(|| {
            Arc::new(ProxyRuntime {
                handle: Mutex::new(None),
                settings: Arc::new(RwLock::new(ProxySettings::default())),
                stats: Arc::new(RwLock::new(ProxyStats::default())),
                recent_logs: Arc::new(RwLock::new(VecDeque::new())),
                health: Arc::new(RwLock::new(HashMap::new())),
            })
        })
        .clone()
}

pub fn parse_proxy_app(value: &str) -> Result<AppType, AppError> {
    let app = AppType::parse_supported(value)?;
    if matches!(app, AppType::Omo | AppType::OmoSlim) {
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
        if !(upstream.is_empty()
            || upstream.starts_with("http://")
            || upstream.starts_with("https://"))
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
    let guard = state.load_config()?;
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
    if path == "/claude-desktop/v1/models" || path == "/claude-desktop/v1/models/" {
        return Ok((
            AppType::ClaudeDesktop,
            strip_prefix(uri, "/claude-desktop")?,
        ));
    }
    if path == "/claude-desktop/v1/messages" || path.starts_with("/claude-desktop/v1/messages/") {
        return Ok((
            AppType::ClaudeDesktop,
            strip_prefix(uri, "/claude-desktop")?,
        ));
    }
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

    let request_id = next_proxy_request_id();
    let result = proxy_request(
        state.clone(),
        method,
        uri,
        headers,
        body,
        request_id.clone(),
    )
    .await;
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
    let log_settings = state.settings.read().await.clone();
    if log_settings.enable_logging {
        let duration_ms = started_at
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        let (app, path) = result
            .as_ref()
            .map(|result| (result.app.clone(), result.path.clone()))
            .unwrap_or_else(|_| ("unknown".to_string(), fallback_path));
        let provider_id = result
            .as_ref()
            .map(|result| result.provider_id.clone())
            .unwrap_or_default();
        let provider_type = result
            .as_ref()
            .ok()
            .and_then(|result| result.provider_type.clone());
        let model = result
            .as_ref()
            .map(|result| result.model.clone())
            .unwrap_or_default();
        let usage = result
            .as_ref()
            .map(|result| result.usage.clone())
            .unwrap_or_default();
        let session_id = result
            .as_ref()
            .ok()
            .and_then(|result| result.session_id.clone());
        push_recent_log(
            &state.recent_logs,
            ProxyRecentLog {
                at: chrono::Utc::now().to_rfc3339(),
                app: app.clone(),
                method: method_for_log,
                path,
                status: status.map(|status| status.as_u16()),
                duration_ms,
                error: error.clone(),
            },
        )
        .await;
        persist_proxy_request_log(ProxyRequestLogInput {
            state: &state,
            app_type: app,
            provider_id,
            provider_type,
            model,
            usage_capture: usage,
            session_id,
            request_id,
            status: status.map(|status| status.as_u16()),
            duration_ms,
            error: error.as_deref(),
        });
    }

    match result {
        Ok(result) => result.response,
        Err(err) => Response::builder()
            .status(proxy_error_status(&err))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({ "error": err.to_string() }).to_string(),
            ))
            .unwrap_or_else(|_| Response::new(Body::empty())),
    }
}

fn proxy_error_status(err: &AppError) -> StatusCode {
    match err {
        AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
        AppError::InvalidInput(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::BAD_GATEWAY,
    }
}

struct ProxyRequestResult {
    response: Response,
    app: String,
    path: String,
    provider_id: String,
    provider_type: Option<String>,
    model: String,
    usage: ProxyUsageCapture,
    session_id: Option<String>,
}

enum UpstreamAttemptError {
    Local(AppError),
    Send(AppError),
}

struct UpstreamResponse {
    provider: Provider,
    response: reqwest::Response,
}

#[derive(Debug, Clone, Default)]
struct ProxyUsageCapture {
    usage: Option<TokenUsage>,
    usage_app_type: Option<String>,
    first_token_ms: Option<u64>,
    is_streaming: bool,
}

#[derive(Clone)]
struct StreamUsageContext {
    app_state: Arc<AppState>,
    app_type: String,
    provider_id: String,
    request_model: String,
    request_id: String,
    cost_multiplier: Decimal,
    pricing_source: String,
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
    request_id: String,
) -> Result<ProxyRequestResult, AppError> {
    let settings = state.settings.read().await.clone();
    let (app, routed_uri) = route_app(&settings, &uri)?;
    let log_app = app.as_str().to_string();
    let log_path = sanitize_uri_for_log(&routed_uri);
    let request_accepts_stream = accepts_event_stream(&headers);
    if matches!(app, AppType::ClaudeDesktop) {
        validate_claude_desktop_gateway_request(&state, &headers)?;
    }
    let provider = current_provider(&state.app_state, &app)?;
    let body_bytes = to_bytes(body, PROXY_BODY_LIMIT_BYTES)
        .await
        .map_err(|e| AppError::Config(format!("Failed to read proxy request body: {e}")))?;
    if matches!(app, AppType::ClaudeDesktop) && routed_uri.path() == "/v1/models" {
        let payload = crate::claude_desktop_config::model_list_response(&provider)?;
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload.to_string()))
            .map_err(|e| AppError::Config(format!("Failed to build model response: {e}")))?;
        return Ok(ProxyRequestResult {
            response,
            app: log_app,
            path: log_path,
            provider_type: provider_type(&provider),
            provider_id: provider.id,
            model: String::new(),
            usage: ProxyUsageCapture::default(),
            session_id: None,
        });
    }
    let model = extract_request_model(&app, &routed_uri, &body_bytes);
    let session_id = extract_request_session_id(&body_bytes);

    let reqwest_method = reqwest::Method::from_bytes(method.as_str().as_bytes())
        .map_err(|e| AppError::InvalidInput(format!("Unsupported method: {e}")))?;
    let mut request_headers = reqwest::header::HeaderMap::new();
    for (name, value) in headers.iter() {
        if should_skip_request_header(name) {
            continue;
        }
        if matches!(app, AppType::ClaudeDesktop) && name == header::AUTHORIZATION {
            continue;
        }
        if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()) {
            if let Ok(header_value) = reqwest::header::HeaderValue::from_bytes(value.as_bytes()) {
                request_headers.insert(header_name, header_value);
            }
        }
    }

    let request_started_at = Instant::now();
    let total_timeout = Duration::from_secs(settings.non_streaming_timeout.max(1));
    let upstream = send_with_failover(
        &state,
        &settings,
        &app,
        &provider,
        &routed_uri,
        reqwest_method.clone(),
        request_headers.clone(),
        body_bytes.clone(),
        total_timeout,
    )
    .await?;
    let usage_app_type = usage_app_type_for_provider(&app, &upstream.provider);

    let (response, usage) = if request_accepts_stream || is_streaming_response(&upstream.response) {
        match build_streaming_response(
            upstream.response,
            &settings,
            usage_app_type,
            request_started_at,
            build_stream_usage_context(
                &state.app_state,
                usage_app_type,
                &upstream.provider.id,
                &model,
                &request_id,
            ),
        )
        .await
        {
            Ok((response, usage)) => (response, usage),
            Err(err @ StreamingResponseError::FirstByte(_)) => {
                if upstream.provider.id != provider.id {
                    return Err(err.into_app_error());
                }
                if let Some(response) = retry_streaming_first_byte_failover(
                    &state,
                    &settings,
                    &app,
                    &provider,
                    &routed_uri,
                    &reqwest_method,
                    &request_headers,
                    &body_bytes,
                    total_timeout,
                    request_started_at,
                    request_accepts_stream,
                    &request_id,
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
        build_buffered_response(
            upstream.response,
            total_timeout,
            request_started_at,
            usage_app_type,
        )
        .await?
    };
    Ok(ProxyRequestResult {
        response,
        app: log_app,
        path: log_path,
        provider_type: provider_type(&upstream.provider),
        provider_id: upstream.provider.id,
        model,
        usage,
        session_id,
    })
}

#[allow(clippy::too_many_arguments)]
async fn send_with_failover(
    state: &ProxyHandlerState,
    settings: &ProxySettings,
    app: &AppType,
    provider: &Provider,
    routed_uri: &Uri,
    method: reqwest::Method,
    request_headers: reqwest::header::HeaderMap,
    body_bytes: Bytes,
    total_timeout: Duration,
) -> Result<UpstreamResponse, AppError> {
    let app_settings = proxy_app_settings(settings, app);
    let failover_enabled = app_settings.auto_failover_enabled && app_settings.max_retries > 0;
    let backup = if failover_enabled {
        backup_provider(&state.app_state, app, &provider.id)?
    } else {
        None
    };
    let current_circuit_allows =
        provider_circuit_allows_request(settings, &state.health, app, &provider.id).await;

    if !current_circuit_allows {
        if let Some(backup) = backup.as_ref() {
            if provider_circuit_allows_request(settings, &state.health, app, &backup.id).await {
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
                        record_provider_success(state, settings, &state.health, app, &backup.id)
                            .await;
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
                record_provider_failure(
                    state,
                    settings,
                    &state.health,
                    app,
                    &provider.id,
                    app_settings.max_retries,
                    Some(&format!("Upstream returned {}", response.status())),
                )
                .await;
                if let Some(backup) = backup.as_ref() {
                    if provider_circuit_allows_request(settings, &state.health, app, &backup.id)
                        .await
                    {
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
                        let original_response = response;
                        match backup_result {
                            Ok(backup_response)
                                if !is_failover_status(backup_response.status()) =>
                            {
                                record_provider_success(
                                    state,
                                    settings,
                                    &state.health,
                                    app,
                                    &backup.id,
                                )
                                .await;
                                switch_to_failover_provider(state, app, provider, backup).await?;
                                return Ok(UpstreamResponse {
                                    provider: backup.clone(),
                                    response: backup_response,
                                });
                            }
                            Ok(_backup_response) => {
                                record_provider_failure(
                                    state,
                                    settings,
                                    &state.health,
                                    app,
                                    &backup.id,
                                    app_settings.max_retries,
                                    Some(&format!(
                                        "Failover upstream returned {}",
                                        _backup_response.status()
                                    )),
                                )
                                .await;
                                return Ok(UpstreamResponse {
                                    provider: provider.clone(),
                                    response: original_response,
                                });
                            }
                            Err(UpstreamAttemptError::Send(err)) => {
                                let err = err.to_string();
                                record_provider_failure(
                                    state,
                                    settings,
                                    &state.health,
                                    app,
                                    &backup.id,
                                    app_settings.max_retries,
                                    Some(&err),
                                )
                                .await;
                                return Ok(UpstreamResponse {
                                    provider: provider.clone(),
                                    response: original_response,
                                });
                            }
                            Err(UpstreamAttemptError::Local(_)) => {
                                return Ok(UpstreamResponse {
                                    provider: provider.clone(),
                                    response: original_response,
                                });
                            }
                        }
                    }
                }
            } else {
                record_provider_success(state, settings, &state.health, app, &provider.id).await;
            }
            Ok(UpstreamResponse {
                provider: provider.clone(),
                response,
            })
        }
        Err(UpstreamAttemptError::Send(err)) => {
            if failover_enabled {
                let error = err.to_string();
                record_provider_failure(
                    state,
                    settings,
                    &state.health,
                    app,
                    &provider.id,
                    app_settings.max_retries,
                    Some(&error),
                )
                .await;
                if let Some(backup) = backup.as_ref() {
                    if provider_circuit_allows_request(settings, &state.health, app, &backup.id)
                        .await
                    {
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
                                record_provider_success(
                                    state,
                                    settings,
                                    &state.health,
                                    app,
                                    &backup.id,
                                )
                                .await;
                                switch_to_failover_provider(state, app, provider, backup).await?;
                                return Ok(UpstreamResponse {
                                    provider: backup.clone(),
                                    response: backup_response,
                                });
                            }
                            Ok(backup_response) => {
                                record_provider_failure(
                                    state,
                                    settings,
                                    &state.health,
                                    app,
                                    &backup.id,
                                    app_settings.max_retries,
                                    Some(&format!(
                                        "Failover upstream returned {}",
                                        backup_response.status()
                                    )),
                                )
                                .await;
                            }
                            Err(UpstreamAttemptError::Send(err)) => {
                                let err = err.to_string();
                                record_provider_failure(
                                    state,
                                    settings,
                                    &state.health,
                                    app,
                                    &backup.id,
                                    app_settings.max_retries,
                                    Some(&err),
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
    settings: &ProxySettings,
    app: &AppType,
    failed_provider: &Provider,
    routed_uri: &Uri,
    method: &reqwest::Method,
    request_headers: &reqwest::header::HeaderMap,
    body_bytes: &Bytes,
    total_timeout: Duration,
    request_started_at: Instant,
    request_accepts_stream: bool,
    request_id: &str,
) -> Result<Option<(Response, ProxyUsageCapture)>, AppError> {
    let app_settings = proxy_app_settings(settings, app);
    if !app_settings.auto_failover_enabled || app_settings.max_retries == 0 {
        return Ok(None);
    }

    record_provider_failure(
        state,
        settings,
        &state.health,
        app,
        &failed_provider.id,
        app_settings.max_retries,
        Some("Streaming response did not produce a first byte before timeout"),
    )
    .await;

    let Some(backup) = backup_provider(&state.app_state, app, &failed_provider.id)? else {
        return Ok(None);
    };
    if !provider_circuit_allows_request(settings, &state.health, app, &backup.id).await {
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
        Ok(response) => {
            record_provider_failure(
                state,
                settings,
                &state.health,
                app,
                &backup.id,
                app_settings.max_retries,
                Some(&format!("Failover upstream returned {}", response.status())),
            )
            .await;
            return Ok(None);
        }
        Err(UpstreamAttemptError::Send(err)) => {
            let err = err.to_string();
            record_provider_failure(
                state,
                settings,
                &state.health,
                app,
                &backup.id,
                app_settings.max_retries,
                Some(&err),
            )
            .await;
            return Ok(None);
        }
        Err(UpstreamAttemptError::Local(_)) => return Ok(None),
    };

    let should_stream = request_accepts_stream || is_streaming_response(&backup_response);
    let usage_app_type = usage_app_type_for_provider(app, &backup);
    let response = if should_stream {
        match build_streaming_response(
            backup_response,
            settings,
            usage_app_type,
            request_started_at,
            build_stream_usage_context(
                &state.app_state,
                usage_app_type,
                &backup.id,
                &extract_request_model(app, routed_uri, body_bytes),
                request_id,
            ),
        )
        .await
        {
            Ok(response) => response,
            Err(StreamingResponseError::FirstByte(_)) => {
                record_provider_failure(
                    state,
                    settings,
                    &state.health,
                    app,
                    &backup.id,
                    app_settings.max_retries,
                    Some("Failover streaming response did not produce a first byte before timeout"),
                )
                .await;
                return Ok(None);
            }
            Err(err) => return Err(err.into_app_error()),
        }
    } else {
        build_buffered_response(
            backup_response,
            total_timeout,
            request_started_at,
            usage_app_type,
        )
        .await?
    };

    record_provider_success(state, settings, &state.health, app, &backup.id).await;
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
    let body_bytes = if matches!(app, AppType::ClaudeDesktop) && routed_uri.path() == "/v1/messages"
    {
        let body: serde_json::Value = serde_json::from_slice(body_bytes).map_err(|e| {
            UpstreamAttemptError::Local(AppError::InvalidInput(format!(
                "Invalid Claude Desktop request body: {e}"
            )))
        })?;
        let mapped = crate::claude_desktop_config::map_proxy_request_model(body, provider)
            .map_err(UpstreamAttemptError::Local)?;
        Bytes::from(mapped.to_string())
    } else {
        body_bytes.clone()
    };
    let base_url = adapter
        .extract_base_url(provider)
        .map_err(UpstreamAttemptError::Local)?;
    let upstream_uri = upstream_uri_for_provider(app, provider, routed_uri)
        .map_err(UpstreamAttemptError::Local)?;
    let url = if provider
        .meta
        .as_ref()
        .and_then(|meta| meta.is_full_url)
        .unwrap_or(false)
    {
        full_endpoint_url(&base_url, &upstream_uri).map_err(UpstreamAttemptError::Local)?
    } else {
        adapter
            .build_url(&base_url, &upstream_uri)
            .map_err(UpstreamAttemptError::Local)?
    };
    let mut headers = request_headers.clone();
    let auth = resolve_auth_for_provider(&state.app_state, app, provider, adapter)
        .await
        .map_err(UpstreamAttemptError::Local)?;
    if let Some(auth) = auth {
        insert_auth_headers(&mut headers, adapter, &auth);
    }
    inject_codex_oauth_headers(&mut headers, provider, body_bytes.as_ref());

    timeout_app_error(
        total_timeout,
        state
            .client
            .request(method.clone(), url)
            .headers(headers)
            .body(body_bytes)
            .send(),
        "Proxy upstream request timed out",
    )
    .await
    .map_err(UpstreamAttemptError::Send)?
    .map_err(|e| UpstreamAttemptError::Send(upstream_request_error(e)))
}

fn upstream_uri_for_provider(
    app: &AppType,
    provider: &Provider,
    routed_uri: &Uri,
) -> Result<Uri, AppError> {
    if !matches!(app, AppType::ClaudeDesktop) || routed_uri.path() != "/v1/messages" {
        return Ok(routed_uri.clone());
    }

    let target_path = match crate::claude_desktop_config::proxy_api_format(provider) {
        Some("openai_chat") => Some("/v1/chat/completions"),
        Some("openai_responses") => Some("/v1/responses"),
        Some("anthropic") | None => None,
        Some("gemini_native") => None,
        Some(_) => None,
    };
    let Some(target_path) = target_path else {
        return Ok(routed_uri.clone());
    };

    replace_uri_path(routed_uri, target_path)
}

fn replace_uri_path(uri: &Uri, path: &str) -> Result<Uri, AppError> {
    let path_and_query = match uri.query() {
        Some(query) => format!("{path}?{query}"),
        None => path.to_string(),
    };
    Uri::builder()
        .path_and_query(path_and_query)
        .build()
        .map_err(|e| AppError::InvalidInput(format!("Invalid proxy request URI: {e}")))
}

fn proxy_app_settings(settings: &ProxySettings, app: &AppType) -> ProxyAppSettings {
    match app {
        AppType::Claude => settings.apps.claude.clone(),
        AppType::Codex => settings.apps.codex.clone(),
        AppType::Gemini => settings.apps.gemini.clone(),
        AppType::Opencode => settings.apps.opencode.clone(),
        AppType::ClaudeDesktop => settings.apps.claude.clone(),
        AppType::Omo | AppType::OmoSlim => ProxyAppSettings::default(),
    }
}

fn usage_app_type_for_provider<'a>(app: &'a AppType, provider: &Provider) -> &'a str {
    if matches!(app, AppType::ClaudeDesktop)
        && provider.meta.as_ref().is_some_and(|meta| {
            matches!(
                meta.provider_type(),
                Some(ProviderType::GithubCopilot | ProviderType::CodexOauth)
            ) || matches!(
                crate::claude_desktop_config::proxy_api_format(provider),
                Some("openai_chat" | "openai_responses")
            )
        })
    {
        return "codex";
    }
    app.as_str()
}

fn inject_codex_oauth_headers(
    headers: &mut reqwest::header::HeaderMap,
    provider: &Provider,
    body: &[u8],
) {
    let Some(meta) = provider.meta.as_ref() else {
        return;
    };
    if meta.provider_type() != Some(ProviderType::CodexOauth) {
        return;
    }

    let session_id = extract_session_id_from_slice(body);
    if let Some(session_id) = session_id.as_deref() {
        insert_header_if_valid(headers, "openai-session-id", session_id);
        insert_header_if_valid(headers, "x-openai-session-id", session_id);
    }
    if session_id.is_some() {
        if let Some(cache_key) = meta
            .prompt_cache_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            insert_header_if_valid(headers, "openai-prompt-cache-key", cache_key);
            insert_header_if_valid(headers, "x-openai-prompt-cache-key", cache_key);
        }
    }
    if meta.codex_fast_mode.unwrap_or(false) {
        insert_header_if_valid(headers, "openai-fast-mode", "true");
        insert_header_if_valid(headers, "x-codex-fast-mode", "true");
    }
}

fn insert_header_if_valid(
    headers: &mut reqwest::header::HeaderMap,
    name: &'static str,
    value: &str,
) {
    if let Ok(value) = reqwest::header::HeaderValue::from_str(value) {
        headers.insert(reqwest::header::HeaderName::from_static(name), value);
    }
}

fn backup_provider(
    state: &AppState,
    app: &AppType,
    current_provider_id: &str,
) -> Result<Option<Provider>, AppError> {
    let guard = state.load_config()?;
    let Some(manager) = guard.get_manager(app) else {
        return Ok(None);
    };
    let queue = state.db.list_failover_queue(app.as_str())?;
    for item in queue {
        let provider_id = item.provider_id.trim();
        if provider_id.is_empty() || provider_id == current_provider_id {
            continue;
        }
        if let Some(provider) = manager.providers.get(provider_id) {
            return Ok(Some(provider.clone()));
        }
    }
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
    settings: &ProxySettings,
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
            let wait = Duration::from_secs(settings.circuit_recovery_wait_seconds.max(1));
            if opened_at.elapsed() >= wait {
                entry.state = ProviderCircuitState::HalfOpen;
                true
            } else {
                false
            }
        }
    }
}

async fn record_provider_success(
    state: &ProxyHandlerState,
    settings: &ProxySettings,
    health: &Arc<RwLock<HashMap<String, ProviderRuntimeHealth>>>,
    app: &AppType,
    provider_id: &str,
) {
    let key = provider_health_key(app, provider_id);
    let mut guard = health.write().await;
    let entry = guard.entry(key).or_default();
    entry.window_requests = entry.window_requests.saturating_add(1);
    entry.failure_count = 0;
    entry.last_failure_at = None;
    match entry.state {
        ProviderCircuitState::HalfOpen => {
            entry.recovery_success_count = entry.recovery_success_count.saturating_add(1);
            if entry.recovery_success_count >= settings.circuit_recovery_threshold.max(1) {
                *entry = ProviderRuntimeHealth::default();
            }
        }
        ProviderCircuitState::Open => {}
        ProviderCircuitState::Healthy => {
            entry.recovery_success_count = 0;
            entry.opened_at = None;
        }
    }
    drop(guard);
    if let Err(err) = state
        .app_state
        .db
        .record_provider_success(app.as_str(), provider_id)
    {
        log::warn!("Failed to persist provider health success: {err}");
    }
}

async fn record_provider_failure(
    state: &ProxyHandlerState,
    settings: &ProxySettings,
    health: &Arc<RwLock<HashMap<String, ProviderRuntimeHealth>>>,
    app: &AppType,
    provider_id: &str,
    max_retries: u8,
    error: Option<&str>,
) {
    let key = provider_health_key(app, provider_id);
    let threshold = settings
        .circuit_failure_threshold
        .max(u64::from(max_retries).saturating_add(1))
        .max(1);
    let mut guard = health.write().await;
    let entry = guard.entry(key).or_default();
    entry.window_requests = entry.window_requests.saturating_add(1);
    entry.window_failures = entry.window_failures.saturating_add(1);
    entry.failure_count = entry.failure_count.saturating_add(1);
    entry.recovery_success_count = 0;
    entry.last_failure_at = Some(Instant::now());
    let error_rate = if entry.window_requests == 0 {
        0.0
    } else {
        entry.window_failures as f64 / entry.window_requests as f64 * 100.0
    };
    if entry.failure_count >= threshold
        || (entry.window_requests >= threshold
            && error_rate >= settings.circuit_error_rate_threshold)
        || entry.state == ProviderCircuitState::HalfOpen
    {
        entry.state = ProviderCircuitState::Open;
        entry.opened_at = Some(Instant::now());
    }
    let unhealthy = matches!(entry.state, ProviderCircuitState::Open);
    if let Err(err) =
        state
            .app_state
            .db
            .record_provider_failure(app.as_str(), provider_id, error, unhealthy)
    {
        log::warn!("Failed to persist provider health failure: {err}");
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
    app_type: &str,
) -> Result<(Response, ProxyUsageCapture), AppError> {
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
        read_limited_upstream_body(upstream, PROXY_RESPONSE_LIMIT_BYTES),
        "Proxy upstream response body timed out",
    )
    .await??;
    let usage = parse_json_usage(app_type, &bytes);
    let response = builder
        .body(Body::from(bytes))
        .map_err(|e| AppError::Config(format!("Failed to build proxy response: {e}")))?;
    Ok((
        response,
        ProxyUsageCapture {
            usage,
            usage_app_type: Some(app_type.to_string()),
            first_token_ms: None,
            is_streaming: false,
        },
    ))
}

async fn read_limited_upstream_body(
    upstream: reqwest::Response,
    max_bytes: usize,
) -> Result<Bytes, AppError> {
    if upstream
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(AppError::Config(format!(
            "Proxy upstream response exceeds the {max_bytes} byte limit"
        )));
    }

    let mut buffer = Vec::new();
    let mut stream = upstream.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|e| AppError::Config(format!("Failed to read upstream response: {e}")))?;
        if buffer.len().saturating_add(chunk.len()) > max_bytes {
            return Err(AppError::Config(format!(
                "Proxy upstream response exceeds the {max_bytes} byte limit"
            )));
        }
        buffer.extend_from_slice(&chunk);
    }
    Ok(Bytes::from(buffer))
}

fn validate_claude_desktop_gateway_request(
    state: &ProxyHandlerState,
    headers: &HeaderMap,
) -> Result<(), AppError> {
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    crate::claude_desktop_config::validate_gateway_bearer_token(&state.app_state.db, authorization)
}

async fn build_streaming_response(
    upstream: reqwest::Response,
    settings: &ProxySettings,
    app_type: &str,
    request_started_at: Instant,
    usage_context: StreamUsageContext,
) -> Result<(Response, ProxyUsageCapture), StreamingResponseError> {
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
        let response = builder.body(Body::empty()).map_err(|e| {
            StreamingResponseError::Other(AppError::Config(format!(
                "Failed to build proxy response: {e}"
            )))
        })?;
        return Ok((
            response,
            ProxyUsageCapture {
                usage: None,
                usage_app_type: Some(app_type.to_string()),
                first_token_ms: None,
                is_streaming: true,
            },
        ));
    };
    let first = first.map_err(|e| {
        StreamingResponseError::FirstByte(AppError::Config(format!(
            "Failed to read first upstream streaming chunk: {e}"
        )))
    })?;

    let first_token_ms = Some(
        request_started_at
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
    );
    let first_events = parse_sse_events_from_bytes(&first);
    let usage = TokenUsage::from_stream_events(app_type, &first_events);
    let events = first_events;

    let rest = stream::unfold(
        (upstream_stream, events, usage_context, first_token_ms),
        move |(mut stream, mut events, usage_context, first_token_ms)| {
            let idle_timeout = idle_timeout;
            async move {
                match tokio::time::timeout(idle_timeout, stream.next()).await {
                    Ok(Some(Ok(bytes))) => {
                        events.extend(parse_sse_events_from_bytes(&bytes));
                        Some((Ok(bytes), (stream, events, usage_context, first_token_ms)))
                    }
                    Ok(Some(Err(err))) => Some((
                        Err(std::io::Error::other(err)),
                        (stream, events, usage_context, first_token_ms),
                    )),
                    Ok(None) => {
                        persist_stream_usage_update(&usage_context, &events, first_token_ms);
                        None
                    }
                    Err(_) => Some((
                        Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "Proxy streaming idle timeout",
                        )),
                        (stream, events, usage_context, first_token_ms),
                    )),
                }
            }
        },
    );
    let body_stream = stream::once(async move { Ok::<Bytes, std::io::Error>(first) }).chain(rest);

    let response = builder.body(Body::from_stream(body_stream)).map_err(|e| {
        StreamingResponseError::Other(AppError::Config(format!(
            "Failed to build proxy response: {e}"
        )))
    })?;
    Ok((
        response,
        ProxyUsageCapture {
            usage,
            usage_app_type: Some(app_type.to_string()),
            first_token_ms,
            is_streaming: true,
        },
    ))
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

    *rt.settings.write().await = settings.clone();

    let handler_state = ProxyHandlerState {
        app_state: state.clone(),
        client,
        settings: rt.settings.clone(),
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

pub async fn recent_logs_for_state(state: &AppState) -> Vec<ProxyRecentLog> {
    let enable_logging = state
        .db
        .get_proxy_config()
        .map(|config| config.enable_logging)
        .unwrap_or_else(|err| {
            log::warn!("Failed to read proxy config from database: {err}");
            settings::get_settings().proxy.enable_logging
        });
    if !enable_logging {
        return Vec::new();
    }
    runtime().recent_logs.read().await.iter().cloned().collect()
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
    update_runtime_settings_with(settings, false).await;
}

pub async fn update_runtime_takeover_settings(settings: ProxySettings) {
    update_runtime_settings_with(settings, true).await;
}

async fn update_runtime_settings_with(settings: ProxySettings, include_takeover: bool) {
    let rt = runtime();
    let mut guard = rt.handle.lock().await;
    if let Some(handle) = guard.as_mut() {
        if !handle.join.is_finished() {
            let runtime_settings =
                merge_runtime_settings(&handle.settings, settings, include_takeover);
            handle.settings = runtime_settings.clone();
            *rt.settings.write().await = runtime_settings;
        }
    }
}

fn merge_runtime_settings(
    current: &ProxySettings,
    saved: ProxySettings,
    include_takeover: bool,
) -> ProxySettings {
    let mut runtime = current.clone();
    runtime.enable_logging = saved.enable_logging;
    runtime.bind_app = saved.bind_app;
    runtime.streaming_first_byte_timeout = saved.streaming_first_byte_timeout;
    runtime.streaming_idle_timeout = saved.streaming_idle_timeout;
    runtime.non_streaming_timeout = saved.non_streaming_timeout;
    runtime.circuit_failure_threshold = saved.circuit_failure_threshold;
    runtime.circuit_recovery_threshold = saved.circuit_recovery_threshold;
    runtime.circuit_recovery_wait_seconds = saved.circuit_recovery_wait_seconds;
    runtime.circuit_error_rate_threshold = saved.circuit_error_rate_threshold;
    runtime.rectify_thinking_signature = saved.rectify_thinking_signature;
    runtime.rectify_thinking_budget = saved.rectify_thinking_budget;
    runtime.apps.claude.auto_failover_enabled = saved.apps.claude.auto_failover_enabled;
    runtime.apps.claude.max_retries = saved.apps.claude.max_retries;
    runtime.apps.claude.default_cost_multiplier = saved.apps.claude.default_cost_multiplier;
    runtime.apps.claude.pricing_model_source = saved.apps.claude.pricing_model_source;
    runtime.apps.codex.auto_failover_enabled = saved.apps.codex.auto_failover_enabled;
    runtime.apps.codex.max_retries = saved.apps.codex.max_retries;
    runtime.apps.codex.default_cost_multiplier = saved.apps.codex.default_cost_multiplier;
    runtime.apps.codex.pricing_model_source = saved.apps.codex.pricing_model_source;
    runtime.apps.gemini.auto_failover_enabled = saved.apps.gemini.auto_failover_enabled;
    runtime.apps.gemini.max_retries = saved.apps.gemini.max_retries;
    runtime.apps.gemini.default_cost_multiplier = saved.apps.gemini.default_cost_multiplier;
    runtime.apps.gemini.pricing_model_source = saved.apps.gemini.pricing_model_source;
    runtime.apps.opencode.auto_failover_enabled = saved.apps.opencode.auto_failover_enabled;
    runtime.apps.opencode.max_retries = saved.apps.opencode.max_retries;
    runtime.apps.opencode.default_cost_multiplier = saved.apps.opencode.default_cost_multiplier;
    runtime.apps.opencode.pricing_model_source = saved.apps.opencode.pricing_model_source;

    if include_takeover {
        runtime.live_takeover_active = saved.live_takeover_active;
        runtime.apps.claude.enabled = saved.apps.claude.enabled;
        runtime.apps.codex.enabled = saved.apps.codex.enabled;
        runtime.apps.gemini.enabled = saved.apps.gemini.enabled;
        runtime.apps.opencode.enabled = saved.apps.opencode.enabled;
    }

    runtime
}

pub async fn status() -> ProxyStatus {
    status_with_state(None).await
}

async fn status_with_state(state: Option<&Arc<AppState>>) -> ProxyStatus {
    let rt = runtime();
    let guard = rt.handle.lock().await;
    let stats = rt.stats.read().await.clone();
    let settings = state
        .and_then(|state| state.db.get_proxy_config().ok())
        .unwrap_or_else(|| settings::get_settings().proxy);
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
    if matches!(app, AppType::ClaudeDesktop) {
        crate::claude_desktop_config::validate_provider(&provider)?;
        let base_url = match crate::claude_desktop_config::provider_mode(&provider) {
            crate::provider::ClaudeDesktopMode::Direct => {
                crate::claude_desktop_config::direct_gateway_credentials(&provider)
                    .map(|credentials| credentials.base_url)
                    .ok()
            }
            crate::provider::ClaudeDesktopMode::Proxy => {
                crate::claude_desktop_config::proxy_gateway_base_url_from_db(&state.db).ok()
            }
        };
        let _ = build_client(&settings)?;
        return Ok(ProxyTestResult {
            success: true,
            message: "Proxy settings are valid.".to_string(),
            base_url,
        });
    }
    let adapter = adapter_for(&app);
    let base_url = adapter.extract_base_url(&provider)?;
    let _ = resolve_auth_for_provider(&state, &app, &provider, adapter).await?;
    let _ = adapter.build_url(&base_url, &"/".parse::<Uri>().expect("valid root uri"))?;
    let _ = build_client(&settings)?;
    Ok(ProxyTestResult {
        success: true,
        message: "Proxy settings are valid.".to_string(),
        base_url: Some(base_url),
    })
}

pub async fn start_from_saved_settings(state: Arc<AppState>) {
    let settings = state
        .db
        .get_proxy_config()
        .unwrap_or_else(|_| settings::get_settings().proxy);
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

fn build_stream_usage_context(
    state: &Arc<AppState>,
    app_type: &str,
    provider_id: &str,
    request_model: &str,
    request_id: &str,
) -> StreamUsageContext {
    let (cost_multiplier, pricing_source) =
        resolve_proxy_pricing_config(state, app_type, provider_id);
    StreamUsageContext {
        app_state: state.clone(),
        app_type: app_type.to_string(),
        provider_id: provider_id.to_string(),
        request_model: request_model.to_string(),
        request_id: request_id.to_string(),
        cost_multiplier,
        pricing_source,
    }
}

fn persist_stream_usage_update(
    context: &StreamUsageContext,
    events: &[serde_json::Value],
    first_token_ms: Option<u64>,
) {
    let Some(usage) = TokenUsage::from_stream_events(&context.app_type, events) else {
        return;
    };
    let model_for_log = usage
        .model
        .clone()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| context.request_model.clone());
    let pricing_model = if context.pricing_source == crate::database::PRICING_SOURCE_REQUEST {
        context.request_model.as_str()
    } else {
        model_for_log.as_str()
    };
    let costs = context
        .app_state
        .db
        .get_model_pricing(pricing_model)
        .ok()
        .flatten()
        .and_then(|pricing| {
            CostCalculator::try_calculate_for_app(
                &context.app_type,
                &usage,
                Some(&pricing),
                context.cost_multiplier,
            )
        });
    let (input_cost, output_cost, cache_read_cost, cache_creation_cost, total_cost) =
        cost_strings(costs.as_ref());
    let update = ProxyRequestUsageUpdate {
        request_id: context.request_id.clone(),
        model: model_for_log,
        input_tokens: i64::from(usage.input_tokens),
        output_tokens: i64::from(usage.output_tokens),
        cache_read_tokens: i64::from(usage.cache_read_tokens),
        cache_creation_tokens: i64::from(usage.cache_creation_tokens),
        input_cost_usd: input_cost,
        output_cost_usd: output_cost,
        cache_read_cost_usd: cache_read_cost,
        cache_creation_cost_usd: cache_creation_cost,
        total_cost_usd: total_cost,
        first_token_ms: first_token_ms.map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
        duration_ms: None,
    };
    if let Err(err) = context.app_state.db.update_proxy_request_log_usage(&update) {
        log::warn!(
            "Failed to update streaming proxy usage log for {} provider {}: {err}",
            context.app_type,
            context.provider_id
        );
    }
}

struct ProxyRequestLogInput<'a> {
    state: &'a ProxyHandlerState,
    app_type: String,
    provider_id: String,
    provider_type: Option<String>,
    model: String,
    usage_capture: ProxyUsageCapture,
    session_id: Option<String>,
    request_id: String,
    status: Option<u16>,
    duration_ms: u64,
    error: Option<&'a str>,
}

fn persist_proxy_request_log(input: ProxyRequestLogInput<'_>) {
    let ProxyRequestLogInput {
        state,
        app_type,
        provider_id,
        provider_type,
        model,
        usage_capture,
        session_id,
        request_id,
        status,
        duration_ms,
        error,
    } = input;
    let app_type_ref = app_type.as_str();
    let usage_app_type = usage_capture
        .usage_app_type
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(app_type_ref);
    let request_model = Some(model.clone()).filter(|value| !value.is_empty());
    let resolved_usage = usage_capture.usage;
    let response_model = resolved_usage
        .as_ref()
        .and_then(|usage| usage.model.clone())
        .filter(|value| !value.is_empty());
    let model_for_log = response_model.clone().unwrap_or_else(|| model.clone());
    let (input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens) =
        resolved_usage.as_ref().map_or((0, 0, 0, 0), |usage| {
            (
                i64::from(usage.input_tokens),
                i64::from(usage.output_tokens),
                i64::from(usage.cache_read_tokens),
                i64::from(usage.cache_creation_tokens),
            )
        });
    let (cost_multiplier, pricing_source) =
        resolve_proxy_pricing_config(&state.app_state, app_type_ref, &provider_id);
    let pricing_model = if pricing_source == crate::database::PRICING_SOURCE_REQUEST {
        request_model.as_deref().unwrap_or(&model_for_log)
    } else {
        &model_for_log
    };
    let costs = resolved_usage.as_ref().and_then(|usage| {
        let pricing = state
            .app_state
            .db
            .get_model_pricing(pricing_model)
            .ok()
            .flatten();
        CostCalculator::try_calculate_for_app(
            usage_app_type,
            usage,
            pricing.as_ref(),
            cost_multiplier,
        )
    });
    let (input_cost, output_cost, cache_read_cost, cache_creation_cost, total_cost) =
        cost_strings(costs.as_ref());
    let record = ProxyRequestLogRecord {
        request_id,
        provider_id,
        app_type,
        model: model_for_log,
        request_model,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        input_cost_usd: input_cost,
        output_cost_usd: output_cost,
        cache_read_cost_usd: cache_read_cost,
        cache_creation_cost_usd: cache_creation_cost,
        total_cost_usd: total_cost,
        latency_ms: i64::try_from(duration_ms).unwrap_or(i64::MAX),
        first_token_ms: usage_capture
            .first_token_ms
            .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
        duration_ms: Some(i64::try_from(duration_ms).unwrap_or(i64::MAX)),
        status_code: status.map(i64::from).unwrap_or(0),
        error_message: error.map(ToString::to_string),
        session_id,
        provider_type,
        is_streaming: usage_capture.is_streaming,
        cost_multiplier: cost_multiplier.to_string(),
        created_at: chrono::Utc::now().timestamp_millis(),
        data_source: "proxy".to_string(),
    };
    if let Err(err) = state.app_state.db.insert_proxy_request_log(&record) {
        log::warn!("Failed to persist proxy request log: {err}");
    }
}

fn resolve_proxy_pricing_config(
    state: &AppState,
    app_type: &str,
    provider_id: &str,
) -> (Decimal, String) {
    let (default_multiplier, default_source) = state
        .db
        .get_proxy_pricing_config(app_type)
        .unwrap_or_else(|_| {
            (
                "1".to_string(),
                crate::database::PRICING_SOURCE_RESPONSE.to_string(),
            )
        });
    let mut multiplier = default_multiplier;
    let mut source = default_source;
    if let Ok(config) = state.load_config() {
        if let Ok(app) = AppType::parse_supported(app_type) {
            if let Some(provider) = config
                .get_manager(&app)
                .and_then(|manager| manager.providers.get(provider_id))
            {
                if let Some(meta) = provider.meta.as_ref() {
                    if let Some(value) = meta.cost_multiplier.as_ref() {
                        multiplier = value.clone();
                    }
                    if let Some(value) = meta.pricing_model_source.as_ref() {
                        source = value.clone();
                    }
                }
            }
        }
    }
    let multiplier = Decimal::from_str(&multiplier).unwrap_or_else(|_| Decimal::from(1));
    if source != crate::database::PRICING_SOURCE_REQUEST
        && source != crate::database::PRICING_SOURCE_RESPONSE
    {
        source = crate::database::PRICING_SOURCE_RESPONSE.to_string();
    }
    (multiplier, source)
}

fn cost_strings(cost: Option<&CostBreakdown>) -> (String, String, String, String, String) {
    match cost {
        Some(cost) => (
            cost.input_cost.to_string(),
            cost.output_cost.to_string(),
            cost.cache_read_cost.to_string(),
            cost.cache_creation_cost.to_string(),
            cost.total_cost.to_string(),
        ),
        None => (
            "0".to_string(),
            "0".to_string(),
            "0".to_string(),
            "0".to_string(),
            "0".to_string(),
        ),
    }
}

fn next_proxy_request_id() -> String {
    let sequence = REQUEST_LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("proxy-{}-{sequence}", chrono::Utc::now().timestamp_millis())
}

fn parse_json_usage(app_type: &str, bytes: &Bytes) -> Option<TokenUsage> {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|value| TokenUsage::from_response(app_type, &value))
}

fn parse_sse_events_from_bytes(bytes: &Bytes) -> Vec<serde_json::Value> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Vec::new();
    };
    let mut events = Vec::new();
    for block in text.split("\n\n") {
        for line in block.lines() {
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
                events.push(value);
            }
        }
    }
    events
}

fn extract_request_model(app: &AppType, uri: &Uri, body: &Bytes) -> String {
    if matches!(app, AppType::Gemini) {
        if let Some(model) = extract_gemini_model_from_uri(uri) {
            return model;
        }
    }

    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| json_string_at_any_path(&value, &[&["model"], &["request", "model"]]))
        .unwrap_or_default()
}

fn extract_request_session_id(body: &Bytes) -> Option<String> {
    extract_session_id_from_slice(body.as_ref())
}

fn extract_session_id_from_slice(body: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    json_string_at_any_path(
        &value,
        &[
            &["session_id"],
            &["sessionId"],
            &["conversation_id"],
            &["conversationId"],
            &["metadata", "session_id"],
            &["metadata", "sessionId"],
        ],
    )
    .filter(|value| !value.trim().is_empty())
}

fn json_string_at_any_path(value: &serde_json::Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        let mut current = value;
        for key in *path {
            current = current.get(*key)?;
        }
        current.as_str().map(ToString::to_string)
    })
}

fn extract_gemini_model_from_uri(uri: &Uri) -> Option<String> {
    let mut segments = uri.path().trim_start_matches('/').split('/');
    while let Some(segment) = segments.next() {
        if segment == "models" {
            let model_segment = segments.next()?;
            let model = model_segment
                .split_once(':')
                .map(|(model, _)| model)
                .unwrap_or(model_segment)
                .trim();
            if !model.is_empty() {
                return Some(model.to_string());
            }
        }
    }
    None
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
    use super::{
        extract_request_model, extract_request_session_id, inject_codex_oauth_headers,
        merge_runtime_settings, proxy_error_status, read_limited_upstream_body, takeover_apps,
        test_settings, upstream_uri_for_provider, usage_app_type_for_provider, AppError, AppType,
        ProxySettings,
    };
    use crate::{
        app_config::MultiAppConfig,
        database::CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID,
        provider::{Provider, ProviderAuthBinding, ProviderManager, ProviderMeta},
        store::AppState,
    };
    use axum::{body::Bytes, http::StatusCode};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn takeover_apps_does_not_duplicate_claude() {
        let mut settings = ProxySettings::default();
        settings.apps.claude.enabled = true;

        assert_eq!(takeover_apps(&settings), vec![AppType::Claude]);
    }

    #[test]
    fn proxy_error_status_maps_client_errors() {
        assert_eq!(
            proxy_error_status(&AppError::Unauthorized("missing token".into())),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            proxy_error_status(&AppError::InvalidInput("bad request".into())),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            proxy_error_status(&AppError::Config("upstream failed".into())),
            StatusCode::BAD_GATEWAY
        );
    }

    #[test]
    fn extract_request_model_prefers_gemini_uri_model() {
        let uri = "/v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
            .parse()
            .expect("valid uri");
        let body = Bytes::from_static(br#"{"model":"body-model"}"#);

        assert_eq!(
            extract_request_model(&AppType::Gemini, &uri, &body),
            "gemini-2.5-pro"
        );
    }

    #[test]
    fn extract_request_model_supports_nested_responses_model() {
        let uri = "/v1/responses".parse().expect("valid uri");
        let body = Bytes::from_static(br#"{"request":{"model":"gpt-5.1-codex"}}"#);

        assert_eq!(
            extract_request_model(&AppType::Codex, &uri, &body),
            "gpt-5.1-codex"
        );
    }

    #[test]
    fn extract_request_session_id_supports_metadata_and_camel_case() {
        let metadata = Bytes::from_static(br#"{"metadata":{"sessionId":"session-meta"}}"#);
        assert_eq!(
            extract_request_session_id(&metadata).as_deref(),
            Some("session-meta")
        );

        let top_level = Bytes::from_static(br#"{"conversation_id":"conversation-1"}"#);
        assert_eq!(
            extract_request_session_id(&top_level).as_deref(),
            Some("conversation-1")
        );
    }

    #[test]
    fn codex_oauth_headers_include_session_cache_and_fast_mode() {
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
                prompt_cache_key: Some("cache-key".to_string()),
                codex_fast_mode: Some(true),
                auth_binding: Some(ProviderAuthBinding {
                    mode: "managed".to_string(),
                    provider_type: Some("codex_oauth".to_string()),
                    account_id: None,
                    use_default: Some(true),
                }),
                ..ProviderMeta::default()
            }),
        };
        let mut headers = reqwest::header::HeaderMap::new();
        inject_codex_oauth_headers(
            &mut headers,
            &provider,
            br#"{"metadata":{"sessionId":"session-1"}}"#,
        );

        assert_eq!(
            headers
                .get("openai-session-id")
                .and_then(|value| value.to_str().ok()),
            Some("session-1")
        );
        assert_eq!(
            headers
                .get("openai-prompt-cache-key")
                .and_then(|value| value.to_str().ok()),
            Some("cache-key")
        );
        assert_eq!(
            headers
                .get("openai-fast-mode")
                .and_then(|value| value.to_str().ok()),
            Some("true")
        );
    }

    #[test]
    fn codex_oauth_headers_skip_cache_key_without_session_identity() {
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
                prompt_cache_key: Some("cache-key".to_string()),
                codex_fast_mode: Some(true),
                auth_binding: Some(ProviderAuthBinding {
                    mode: "managed".to_string(),
                    provider_type: Some("codex_oauth".to_string()),
                    account_id: None,
                    use_default: Some(true),
                }),
                ..ProviderMeta::default()
            }),
        };
        let mut headers = reqwest::header::HeaderMap::new();
        inject_codex_oauth_headers(&mut headers, &provider, br#"{"input":"hello"}"#);

        assert!(headers.get("openai-session-id").is_none());
        assert!(headers.get("openai-prompt-cache-key").is_none());
        assert_eq!(
            headers
                .get("openai-fast-mode")
                .and_then(|value| value.to_str().ok()),
            Some("true")
        );
    }

    #[test]
    fn claude_desktop_openai_formats_route_messages_to_openai_endpoints() {
        let mut provider = Provider {
            id: "desktop-openai".to_string(),
            name: "Desktop OpenAI".to_string(),
            settings_config: json!({}),
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
        let uri = "/v1/messages?stream=true".parse().expect("valid uri");

        let chat_uri =
            upstream_uri_for_provider(&AppType::ClaudeDesktop, &provider, &uri).expect("chat uri");
        assert_eq!(
            chat_uri.path_and_query().map(|value| value.as_str()),
            Some("/v1/chat/completions?stream=true")
        );

        provider.meta.as_mut().expect("meta").api_format = Some("openai-responses".to_string());
        let responses_uri = upstream_uri_for_provider(&AppType::ClaudeDesktop, &provider, &uri)
            .expect("responses uri");
        assert_eq!(
            responses_uri.path_and_query().map(|value| value.as_str()),
            Some("/v1/responses?stream=true")
        );
    }

    #[test]
    fn claude_desktop_anthropic_format_keeps_messages_endpoint() {
        let provider = Provider {
            id: "desktop-anthropic".to_string(),
            name: "Desktop Anthropic".to_string(),
            settings_config: json!({}),
            website_url: None,
            category: None,
            created_at: None,
            sort_index: None,
            notes: None,
            meta: Some(ProviderMeta {
                api_format: Some("anthropic".to_string()),
                ..ProviderMeta::default()
            }),
        };
        let uri = "/v1/messages?stream=true".parse().expect("valid uri");

        let upstream =
            upstream_uri_for_provider(&AppType::ClaudeDesktop, &provider, &uri).expect("uri");
        assert_eq!(
            upstream.path_and_query().map(|value| value.as_str()),
            Some("/v1/messages?stream=true")
        );
    }

    #[test]
    fn claude_desktop_openai_formats_use_codex_usage_parser() {
        let provider = Provider {
            id: "desktop-codex".to_string(),
            name: "Desktop Codex".to_string(),
            settings_config: json!({}),
            website_url: None,
            category: None,
            created_at: None,
            sort_index: None,
            notes: None,
            meta: Some(ProviderMeta {
                provider_type: Some("codex_oauth".to_string()),
                api_format: Some("openai_responses".to_string()),
                ..ProviderMeta::default()
            }),
        };

        assert_eq!(
            usage_app_type_for_provider(&AppType::ClaudeDesktop, &provider),
            "codex"
        );
    }

    #[tokio::test]
    async fn read_limited_upstream_body_rejects_oversized_body() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("test server addr");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept test client");
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buffer = [0u8; 1024];
            let _ = socket.read(&mut buffer).await;
            let body = b"0123456789abcdef";
            let response = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n", body.len());
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write headers");
            socket.write_all(body).await.expect("write body");
        });

        let response = reqwest::get(format!("http://{addr}/upstream"))
            .await
            .expect("fetch response");
        let err = read_limited_upstream_body(response, 8).await.unwrap_err();

        assert!(err.to_string().contains("exceeds"));
        server.await.expect("server join");
    }

    #[tokio::test]
    async fn test_settings_accepts_claude_desktop_official_provider() {
        let provider = Provider::with_id(
            CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID.to_string(),
            "Claude Desktop Official".to_string(),
            json!({"env": {}}),
            Some("https://claude.ai/download".to_string()),
        );
        let mut config = MultiAppConfig::default();
        config.apps.insert(
            AppType::ClaudeDesktop.as_str().to_string(),
            ProviderManager {
                providers: HashMap::from([(provider.id.clone(), provider)]),
                current: CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID.to_string(),
                backup_current: None,
            },
        );
        let state = Arc::new(AppState::new_for_tests(config).expect("test app state"));
        let settings = ProxySettings {
            bind_app: AppType::ClaudeDesktop.as_str().to_string(),
            ..ProxySettings::default()
        };

        let result = test_settings(state, settings).await.expect("proxy test");

        assert!(result.success);
    }

    #[test]
    fn merge_runtime_settings_preserves_listener_client_and_takeover_fields_for_plain_save() {
        let mut current = ProxySettings {
            host: "127.0.0.1".to_string(),
            port: 3456,
            upstream_proxy: Some("http://127.0.0.1:8080".to_string()),
            auto_start: true,
            live_takeover_active: true,
            ..ProxySettings::default()
        };
        current.apps.claude.enabled = true;
        current.apps.claude.auto_failover_enabled = false;
        current.apps.claude.max_retries = 0;

        let mut saved = current.clone();
        saved.host = "0.0.0.0".to_string();
        saved.port = 4567;
        saved.upstream_proxy = Some("http://127.0.0.1:9090".to_string());
        saved.auto_start = false;
        saved.live_takeover_active = false;
        saved.apps.claude.enabled = false;
        saved.enable_logging = true;
        saved.streaming_idle_timeout = 30;
        saved.apps.claude.auto_failover_enabled = true;
        saved.apps.claude.max_retries = 2;

        let merged = merge_runtime_settings(&current, saved, false);

        assert_eq!(merged.host, "127.0.0.1");
        assert_eq!(merged.port, 3456);
        assert_eq!(
            merged.upstream_proxy.as_deref(),
            Some("http://127.0.0.1:8080")
        );
        assert!(merged.auto_start);
        assert!(merged.live_takeover_active);
        assert!(merged.apps.claude.enabled);
        assert!(merged.enable_logging);
        assert_eq!(merged.streaming_idle_timeout, 30);
        assert!(merged.apps.claude.auto_failover_enabled);
        assert_eq!(merged.apps.claude.max_retries, 2);
    }

    #[test]
    fn merge_runtime_settings_can_include_applied_takeover_fields() {
        let mut current = ProxySettings {
            live_takeover_active: true,
            ..ProxySettings::default()
        };
        current.apps.claude.enabled = true;

        let mut saved = current.clone();
        saved.live_takeover_active = false;
        saved.apps.claude.enabled = false;

        let merged = merge_runtime_settings(&current, saved, true);

        assert!(!merged.live_takeover_active);
        assert!(!merged.apps.claude.enabled);
    }
}
