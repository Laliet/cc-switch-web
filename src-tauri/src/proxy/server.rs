use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::ErrorKind,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
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
    gemini_shadow::{GeminiShadowStore, GeminiToolCallMeta},
    live,
    service::ensure_gemini_takeover_supported,
    types::{
        OptimizerConfig, ProxyActiveTarget, ProxyProviderHealth, ProxyRecentLog, ProxyStats,
        ProxyStatus, ProxyTakeoverStatus, ProxyTestResult, PROXY_BODY_LIMIT_BYTES,
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
const PROXY_CLIENT_TIMEOUT_SECS: u64 = 600;
const PROXY_CLIENT_CONNECT_TIMEOUT_SECS: u64 = 30;
const PROXY_CLIENT_POOL_MAX_IDLE_PER_HOST: usize = 10;
const PROXY_CLIENT_TCP_KEEPALIVE_SECS: u64 = 60;
struct ProxyRuntime {
    handle: Mutex<Option<ProxyHandle>>,
    settings: Arc<RwLock<ProxySettings>>,
    stats: Arc<RwLock<ProxyStats>>,
    recent_logs: Arc<RwLock<VecDeque<ProxyRecentLog>>>,
    health: Arc<RwLock<HashMap<String, ProviderRuntimeHealth>>>,
    gemini_shadow: Arc<GeminiShadowStore>,
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
    gemini_shadow: Arc<GeminiShadowStore>,
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
                gemini_shadow: Arc::new(GeminiShadowStore::default()),
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
    parse_proxy_host_ip(host)?;
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
    for (app_name, app) in [
        ("claude", &settings.apps.claude),
        ("codex", &settings.apps.codex),
        ("gemini", &settings.apps.gemini),
        ("opencode", &settings.apps.opencode),
    ] {
        let valid = app.max_retries <= 10
            && (1..=120).contains(&app.streaming_first_byte_timeout)
            && (1..=600).contains(&app.streaming_idle_timeout)
            && (60..=1200).contains(&app.non_streaming_timeout)
            && (1..=20).contains(&app.circuit_failure_threshold)
            && (1..=10).contains(&app.circuit_recovery_threshold)
            && (1..=300).contains(&app.circuit_recovery_wait_seconds)
            && (1.0..=100.0).contains(&app.circuit_error_rate_threshold)
            && (5..=100).contains(&app.circuit_min_requests);
        if !valid {
            return Err(AppError::InvalidInput(format!(
                "Invalid per-app proxy settings for {app_name}"
            )));
        }
    }
    Ok(())
}

fn parse_proxy_host_ip(host: &str) -> Result<IpAddr, AppError> {
    let host = host.trim();
    if host.is_empty() {
        return Err(AppError::InvalidInput("Proxy host is required".into()));
    }

    let normalized = if let Some(inner) = host.strip_prefix('[') {
        inner
            .strip_suffix(']')
            .ok_or_else(|| AppError::InvalidInput("Proxy host must be an IP address".into()))?
    } else {
        if host.ends_with(']') {
            return Err(AppError::InvalidInput(
                "Proxy host must be an IP address".into(),
            ));
        }
        host
    };

    normalized
        .parse()
        .map_err(|_| AppError::InvalidInput("Proxy host must be an IP address".into()))
}

fn parse_proxy_listen_addr(host: &str, port: u16) -> Result<SocketAddr, AppError> {
    Ok(SocketAddr::new(parse_proxy_host_ip(host)?, port))
}

fn proxy_connect_ip_for_listen_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        ip => ip,
    }
}

fn format_http_origin(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(ip) => format!("http://{ip}:{port}"),
        IpAddr::V6(ip) => format!("http://[{ip}]:{port}"),
    }
}

fn listen_url_for_client(actual_addr: SocketAddr) -> String {
    format_http_origin(
        proxy_connect_ip_for_listen_ip(actual_addr.ip()),
        actual_addr.port(),
    )
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
        .timeout(Duration::from_secs(PROXY_CLIENT_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(PROXY_CLIENT_CONNECT_TIMEOUT_SECS))
        .pool_max_idle_per_host(PROXY_CLIENT_POOL_MAX_IDLE_PER_HOST)
        .tcp_keepalive(Duration::from_secs(PROXY_CLIENT_TCP_KEEPALIVE_SECS))
        .no_gzip()
        .no_brotli()
        .no_deflate();

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
    gemini_tool_schema_hints: Option<super::gemini_schema::AnthropicToolSchemaHints>,
}

struct UpstreamProviderResponse {
    response: reqwest::Response,
    gemini_tool_schema_hints: Option<super::gemini_schema::AnthropicToolSchemaHints>,
}

enum RectifierResponseDecision {
    Passthrough(reqwest::Response),
    Retry(Bytes),
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

#[derive(Debug, Clone)]
struct CopilotHeaderPlan {
    initiator: &'static str,
    is_subagent: bool,
    request_id: Option<String>,
    interaction_id: Option<String>,
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
    let settings = effective_proxy_settings_for_app(&settings, &app);
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
        let response_api_format = if matches!(app, AppType::ClaudeDesktop) {
            crate::claude_desktop_config::proxy_api_format(&upstream.provider)
        } else {
            None
        };
        let usage_context = build_stream_usage_context(
            &state.app_state,
            usage_app_type,
            &upstream.provider.id,
            &model,
            &request_id,
        );
        let stream_result = match response_api_format {
            Some("gemini_native") => {
                build_gemini_native_streaming_response(
                    upstream.response,
                    &settings,
                    usage_app_type,
                    request_started_at,
                    usage_context,
                    state.gemini_shadow.clone(),
                    upstream.provider.id.clone(),
                    session_id.clone(),
                    upstream.gemini_tool_schema_hints.clone(),
                )
                .await
            }
            Some("openai_responses") => {
                build_openai_responses_streaming_response(
                    upstream.response,
                    &settings,
                    usage_app_type,
                    request_started_at,
                    usage_context,
                )
                .await
            }
            Some("openai_chat") => {
                build_openai_chat_streaming_response(
                    upstream.response,
                    &settings,
                    usage_app_type,
                    request_started_at,
                    usage_context,
                )
                .await
            }
            _ => {
                build_streaming_response(
                    upstream.response,
                    &settings,
                    usage_app_type,
                    request_started_at,
                    usage_context,
                )
                .await
            }
        };
        match stream_result {
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
        let response_api_format = if matches!(app, AppType::ClaudeDesktop) {
            crate::claude_desktop_config::proxy_api_format(&upstream.provider)
        } else {
            None
        };
        build_buffered_response(
            upstream.response,
            total_timeout,
            request_started_at,
            usage_app_type,
            response_api_format,
            state.gemini_shadow.clone(),
            upstream.provider.id.clone(),
            session_id.clone(),
            upstream.gemini_tool_schema_hints.as_ref(),
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
    let candidates =
        failover_provider_candidates(&state.app_state, app, provider, failover_enabled)?;
    let max_attempts = if failover_enabled {
        usize::from(app_settings.max_retries).saturating_add(1)
    } else {
        1
    };
    let mut last_failover_response: Option<UpstreamResponse> = None;
    let mut last_error: Option<AppError> = None;
    let mut skipped_circuit_open = 0usize;

    for candidate in candidates.into_iter().take(max_attempts) {
        if failover_enabled
            && !provider_circuit_allows_request(settings, &state.health, app, &candidate.id).await
        {
            skipped_circuit_open = skipped_circuit_open.saturating_add(1);
            continue;
        }

        let attempt = send_upstream_provider(
            state,
            settings,
            app,
            &candidate,
            routed_uri,
            &method,
            &request_headers,
            &body_bytes,
            total_timeout,
        )
        .await;

        match attempt {
            Ok(response) if failover_enabled && is_failover_status(response.response.status()) => {
                record_provider_failure(
                    state,
                    settings,
                    &state.health,
                    app,
                    &candidate.id,
                    app_settings.max_retries,
                    Some(&format!("Upstream returned {}", response.response.status())),
                )
                .await;
                last_failover_response = Some(UpstreamResponse {
                    provider: candidate,
                    response: response.response,
                    gemini_tool_schema_hints: response.gemini_tool_schema_hints,
                });
            }
            Ok(response) => {
                record_provider_success(state, settings, &state.health, app, &candidate.id).await;
                if candidate.id != provider.id {
                    switch_to_failover_provider(state, app, provider, &candidate).await?;
                }
                return Ok(UpstreamResponse {
                    provider: candidate,
                    response: response.response,
                    gemini_tool_schema_hints: response.gemini_tool_schema_hints,
                });
            }
            Err(UpstreamAttemptError::Send(err)) if failover_enabled => {
                let error = err.to_string();
                record_provider_failure(
                    state,
                    settings,
                    &state.health,
                    app,
                    &candidate.id,
                    app_settings.max_retries,
                    Some(&error),
                )
                .await;
                last_error = Some(err);
            }
            Err(UpstreamAttemptError::Local(err)) if failover_enabled => {
                record_provider_failure(
                    state,
                    settings,
                    &state.health,
                    app,
                    &candidate.id,
                    app_settings.max_retries,
                    Some(&err.to_string()),
                )
                .await;
                last_error = Some(err);
            }
            Err(err) => return Err(err.into_app_error()),
        }
    }

    if let Some(response) = last_failover_response {
        return Ok(response);
    }
    if let Some(err) = last_error {
        return Err(err);
    }
    Err(AppError::Config(format!(
        "No available failover providers for {}{}",
        app.as_str(),
        if skipped_circuit_open > 0 {
            "; all attempted candidates are circuit-open"
        } else {
            ""
        }
    )))
}

fn failover_provider_candidates(
    state: &AppState,
    app: &AppType,
    current_provider: &Provider,
    failover_enabled: bool,
) -> Result<Vec<Provider>, AppError> {
    if !failover_enabled {
        return Ok(vec![current_provider.clone()]);
    }

    let guard = state.load_config()?;
    let Some(manager) = guard.get_manager(app) else {
        return Ok(vec![current_provider.clone()]);
    };
    let mut providers = Vec::new();
    for item in state.db.list_failover_queue(app.as_str())? {
        let provider_id = item.provider_id.trim();
        if provider_id.is_empty() {
            continue;
        }
        if providers
            .iter()
            .any(|provider: &Provider| provider.id == provider_id)
        {
            continue;
        }
        if let Some(provider) = manager.providers.get(provider_id) {
            providers.push(provider.clone());
        }
    }

    if providers.is_empty() {
        providers.push(current_provider.clone());
    }
    Ok(providers)
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

    let candidates = failover_provider_candidates(&state.app_state, app, failed_provider, true)?;
    for backup in candidates
        .into_iter()
        .filter(|provider| provider.id != failed_provider.id)
        .take(usize::from(app_settings.max_retries))
    {
        if !provider_circuit_allows_request(settings, &state.health, app, &backup.id).await {
            continue;
        }

        let backup_result = send_upstream_provider(
            state,
            settings,
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
            Ok(response) if !is_failover_status(response.response.status()) => response,
            Ok(response) => {
                record_provider_failure(
                    state,
                    settings,
                    &state.health,
                    app,
                    &backup.id,
                    app_settings.max_retries,
                    Some(&format!(
                        "Failover upstream returned {}",
                        response.response.status()
                    )),
                )
                .await;
                continue;
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
                continue;
            }
            Err(UpstreamAttemptError::Local(_)) => continue,
        };

        let should_stream =
            request_accepts_stream || is_streaming_response(&backup_response.response);
        let usage_app_type = usage_app_type_for_provider(app, &backup);
        let response = if should_stream {
            let response_api_format = if matches!(app, AppType::ClaudeDesktop) {
                crate::claude_desktop_config::proxy_api_format(&backup)
            } else {
                None
            };
            let backup_model = extract_request_model(app, routed_uri, body_bytes);
            let usage_context = build_stream_usage_context(
                &state.app_state,
                usage_app_type,
                &backup.id,
                &backup_model,
                request_id,
            );
            let stream_result = match response_api_format {
                Some("gemini_native") => {
                    build_gemini_native_streaming_response(
                        backup_response.response,
                        settings,
                        usage_app_type,
                        request_started_at,
                        usage_context,
                        state.gemini_shadow.clone(),
                        backup.id.clone(),
                        extract_request_session_id(body_bytes),
                        backup_response.gemini_tool_schema_hints.clone(),
                    )
                    .await
                }
                Some("openai_responses") => {
                    build_openai_responses_streaming_response(
                        backup_response.response,
                        settings,
                        usage_app_type,
                        request_started_at,
                        usage_context,
                    )
                    .await
                }
                Some("openai_chat") => {
                    build_openai_chat_streaming_response(
                        backup_response.response,
                        settings,
                        usage_app_type,
                        request_started_at,
                        usage_context,
                    )
                    .await
                }
                _ => {
                    build_streaming_response(
                        backup_response.response,
                        settings,
                        usage_app_type,
                        request_started_at,
                        usage_context,
                    )
                    .await
                }
            };
            match stream_result {
                Ok(response) => response,
                Err(StreamingResponseError::FirstByte(_)) => {
                    record_provider_failure(
                        state,
                        settings,
                        &state.health,
                        app,
                        &backup.id,
                        app_settings.max_retries,
                        Some(
                            "Failover streaming response did not produce a first byte before timeout",
                        ),
                    )
                    .await;
                    continue;
                }
                Err(err) => return Err(err.into_app_error()),
            }
        } else {
            let response_api_format = if matches!(app, AppType::ClaudeDesktop) {
                crate::claude_desktop_config::proxy_api_format(&backup)
            } else {
                None
            };
            build_buffered_response(
                backup_response.response,
                total_timeout,
                request_started_at,
                usage_app_type,
                response_api_format,
                state.gemini_shadow.clone(),
                backup.id.clone(),
                extract_request_session_id(body_bytes),
                backup_response.gemini_tool_schema_hints.as_ref(),
            )
            .await?
        };

        record_provider_success(state, settings, &state.health, app, &backup.id).await;
        switch_to_failover_provider(state, app, failed_provider, &backup).await?;
        return Ok(Some(response));
    }

    Ok(None)
}

#[allow(clippy::too_many_arguments)]
async fn send_upstream_provider(
    state: &ProxyHandlerState,
    settings: &ProxySettings,
    app: &AppType,
    provider: &Provider,
    routed_uri: &Uri,
    method: &reqwest::Method,
    request_headers: &reqwest::header::HeaderMap,
    body_bytes: &Bytes,
    total_timeout: Duration,
) -> Result<UpstreamProviderResponse, UpstreamAttemptError> {
    let adapter = adapter_for(app);
    let mut claude_desktop_gemini_model: Option<String> = None;
    let mut claude_desktop_gemini_stream = false;
    let mut copilot_header_plan: Option<CopilotHeaderPlan> = None;
    let mut gemini_tool_schema_hints = None;
    let body_bytes = if matches!(app, AppType::ClaudeDesktop) && routed_uri.path() == "/v1/messages"
    {
        let body: serde_json::Value = serde_json::from_slice(body_bytes).map_err(|e| {
            UpstreamAttemptError::Local(AppError::InvalidInput(format!(
                "Invalid Claude Desktop request body: {e}"
            )))
        })?;
        if crate::claude_desktop_config::proxy_api_format(provider) == Some("gemini_native") {
            let hints = super::gemini_schema::extract_anthropic_tool_schema_hints(&body);
            if !hints.is_empty() {
                gemini_tool_schema_hints = Some(hints);
            }
            let route = crate::claude_desktop_config::resolve_proxy_request_route(&body, provider)
                .map_err(UpstreamAttemptError::Local)?;
            claude_desktop_gemini_model = Some(route.upstream_model);
            claude_desktop_gemini_stream =
                body.get("stream").and_then(serde_json::Value::as_bool) == Some(true);
        }
        let session_id = extract_session_id_from_value(&body);
        copilot_header_plan = build_copilot_header_plan(
            provider,
            &body,
            request_headers.contains_key("anthropic-beta"),
            session_id.as_deref(),
        );
        let mapped =
            if crate::claude_desktop_config::proxy_api_format(provider) == Some("gemini_native") {
                crate::claude_desktop_config::map_proxy_request_model_with_gemini_shadow(
                    body,
                    provider,
                    &state.gemini_shadow,
                    session_id.as_deref(),
                )
            } else {
                crate::claude_desktop_config::map_proxy_request_model(body, provider)
            }
            .map_err(UpstreamAttemptError::Local)?;
        Bytes::from(mapped.to_string())
    } else {
        body_bytes.clone()
    };
    let body_bytes = apply_bedrock_optimizer(settings, app, provider, routed_uri, body_bytes);
    let mut body_bytes = prepare_upstream_request_body(body_bytes);
    let base_url = adapter
        .extract_base_url(provider)
        .map_err(UpstreamAttemptError::Local)?;
    let upstream_uri = upstream_uri_for_provider(
        app,
        provider,
        routed_uri,
        claude_desktop_gemini_model.as_deref(),
        claude_desktop_gemini_stream,
    )
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
    if let Some(plan) = copilot_header_plan.as_ref() {
        apply_copilot_header_plan(&mut headers, plan);
    }

    let mut signature_rectifier_retried = false;
    let mut budget_rectifier_retried = false;
    let response = loop {
        let response = timeout_app_error(
            total_timeout,
            state
                .client
                .request(method.clone(), url.clone())
                .headers(headers.clone())
                .body(body_bytes.clone())
                .send(),
            "Proxy upstream request timed out",
        )
        .await
        .map_err(UpstreamAttemptError::Send)?
        .map_err(|e| UpstreamAttemptError::Send(upstream_request_error(e)))?;

        match maybe_rectify_anthropic_upstream_response(
            response,
            settings,
            app,
            provider,
            routed_uri,
            &body_bytes,
            total_timeout,
            &mut signature_rectifier_retried,
            &mut budget_rectifier_retried,
        )
        .await
        .map_err(UpstreamAttemptError::Send)?
        {
            RectifierResponseDecision::Passthrough(response) => break response,
            RectifierResponseDecision::Retry(rectified_body) => {
                body_bytes = prepare_upstream_request_body(rectified_body);
            }
        }
    };

    Ok(UpstreamProviderResponse {
        response,
        gemini_tool_schema_hints,
    })
}

fn prepare_upstream_request_body(body_bytes: Bytes) -> Bytes {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body_bytes) else {
        return body_bytes;
    };
    let value = crate::json_canonical::canonicalize_value(
        super::body_filter::filter_private_params_with_whitelist(value, &[]),
    );
    Bytes::from(value.to_string())
}

fn apply_bedrock_optimizer(
    settings: &ProxySettings,
    app: &AppType,
    provider: &Provider,
    routed_uri: &Uri,
    body_bytes: Bytes,
) -> Bytes {
    if !supports_bedrock_optimizer(settings, app, provider, routed_uri) {
        return body_bytes;
    }

    let Ok(mut body) = serde_json::from_slice::<serde_json::Value>(&body_bytes) else {
        return body_bytes;
    };
    let config = optimizer_config_from_settings(settings);
    if config.thinking_optimizer {
        super::thinking_optimizer::optimize(&mut body, &config);
    }
    if config.cache_injection {
        super::cache_injector::inject(&mut body, &config);
    }
    Bytes::from(body.to_string())
}

fn optimizer_config_from_settings(settings: &ProxySettings) -> OptimizerConfig {
    OptimizerConfig {
        enabled: settings.optimizer_enabled,
        thinking_optimizer: settings.optimizer_thinking,
        cache_injection: settings.optimizer_cache_injection,
        cache_ttl: normalize_optimizer_cache_ttl(&settings.optimizer_cache_ttl),
    }
}

fn normalize_optimizer_cache_ttl(value: &str) -> String {
    match value.trim() {
        "5m" => "5m".to_string(),
        _ => "1h".to_string(),
    }
}

fn supports_bedrock_optimizer(
    settings: &ProxySettings,
    app: &AppType,
    provider: &Provider,
    routed_uri: &Uri,
) -> bool {
    settings.optimizer_enabled
        && is_bedrock_provider(provider)
        && supports_anthropic_rectifier(app, provider, routed_uri)
}

fn is_bedrock_provider(provider: &Provider) -> bool {
    provider
        .settings_config
        .get("env")
        .and_then(|env| env.get("CLAUDE_CODE_USE_BEDROCK"))
        .and_then(serde_json::Value::as_str)
        .map(|value| value == "1")
        .unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
async fn maybe_rectify_anthropic_upstream_response(
    response: reqwest::Response,
    settings: &ProxySettings,
    app: &AppType,
    provider: &Provider,
    routed_uri: &Uri,
    current_body: &Bytes,
    total_timeout: Duration,
    signature_rectifier_retried: &mut bool,
    budget_rectifier_retried: &mut bool,
) -> Result<RectifierResponseDecision, AppError> {
    if !supports_anthropic_rectifier(app, provider, routed_uri)
        || !is_rectifier_response_status(response.status())
    {
        return Ok(RectifierResponseDecision::Passthrough(response));
    }

    let status = response.status();
    let version = response.version();
    let headers = response.headers().clone();
    let bytes = timeout_app_error(
        total_timeout,
        read_limited_upstream_body(response, PROXY_RESPONSE_LIMIT_BYTES),
        "Proxy upstream rectifier error body timed out",
    )
    .await??;
    let error_message = extract_upstream_error_message(&bytes);

    if !*signature_rectifier_retried
        && super::thinking_rectifier::should_rectify_thinking_signature(
            error_message.as_deref(),
            settings.rectify_thinking_signature,
        )
    {
        if let Ok(mut body) = serde_json::from_slice::<serde_json::Value>(current_body) {
            let rectified = super::thinking_rectifier::rectify_anthropic_request(&mut body);
            if rectified.applied {
                *signature_rectifier_retried = true;
                log::info!(
                    "[Proxy] thinking signature rectifier retry: removed {} thinking, {} redacted_thinking, {} signature fields",
                    rectified.removed_thinking_blocks,
                    rectified.removed_redacted_thinking_blocks,
                    rectified.removed_signature_fields
                );
                return Ok(RectifierResponseDecision::Retry(Bytes::from(
                    body.to_string(),
                )));
            }
            log::warn!(
                "[Proxy] thinking signature rectifier matched but request body had nothing to rectify"
            );
        }
    }

    if !*budget_rectifier_retried
        && super::thinking_budget_rectifier::should_rectify_thinking_budget(
            error_message.as_deref(),
            settings.rectify_thinking_budget,
        )
    {
        if let Ok(mut body) = serde_json::from_slice::<serde_json::Value>(current_body) {
            let rectified = super::thinking_budget_rectifier::rectify_thinking_budget(&mut body);
            if rectified.applied {
                *budget_rectifier_retried = true;
                log::info!(
                    "[Proxy] thinking budget rectifier retry: before={:?}, after={:?}",
                    rectified.before,
                    rectified.after
                );
                return Ok(RectifierResponseDecision::Retry(Bytes::from(
                    body.to_string(),
                )));
            }
            log::warn!(
                "[Proxy] thinking budget rectifier matched but request body had nothing to rectify"
            );
        }
    }

    Ok(RectifierResponseDecision::Passthrough(
        rebuild_reqwest_response(status, version, &headers, bytes)?,
    ))
}

fn supports_anthropic_rectifier(app: &AppType, provider: &Provider, routed_uri: &Uri) -> bool {
    if routed_uri.path() != "/v1/messages" {
        return false;
    }
    match app {
        AppType::Claude => true,
        AppType::ClaudeDesktop => {
            matches!(
                crate::claude_desktop_config::proxy_api_format(provider),
                None | Some("anthropic")
            )
        }
        _ => false,
    }
}

fn is_rectifier_response_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 400 | 422)
}

fn extract_upstream_error_message(bytes: &Bytes) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    value
        .pointer("/error/message")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("message").and_then(serde_json::Value::as_str))
        .or_else(|| value.get("error").and_then(serde_json::Value::as_str))
        .map(ToString::to_string)
        .or_else(|| {
            value
                .get("error")
                .filter(|error| error.is_object() || error.is_array())
                .map(ToString::to_string)
        })
}

fn rebuild_reqwest_response(
    status: reqwest::StatusCode,
    version: reqwest::Version,
    headers: &reqwest::header::HeaderMap,
    bytes: Bytes,
) -> Result<reqwest::Response, AppError> {
    let mut builder = axum::http::Response::builder()
        .status(status)
        .version(version);
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    let response = builder
        .body(bytes)
        .map_err(|err| AppError::Config(format!("Failed to rebuild upstream response: {err}")))?;
    Ok(reqwest::Response::from(response))
}

fn upstream_uri_for_provider(
    app: &AppType,
    provider: &Provider,
    routed_uri: &Uri,
    claude_desktop_gemini_model: Option<&str>,
    claude_desktop_gemini_stream: bool,
) -> Result<Uri, AppError> {
    if !matches!(app, AppType::ClaudeDesktop) || routed_uri.path() != "/v1/messages" {
        return Ok(routed_uri.clone());
    }

    let target_path = match crate::claude_desktop_config::proxy_api_format(provider) {
        Some("openai_chat") => Some("/v1/chat/completions"),
        Some("openai_responses") => Some("/v1/responses"),
        Some("anthropic") | None => None,
        Some("gemini_native") => {
            return gemini_native_uri_for_model(
                claude_desktop_gemini_model,
                claude_desktop_gemini_stream,
                routed_uri.query(),
            )
        }
        Some(_) => None,
    };
    let Some(target_path) = target_path else {
        return Ok(routed_uri.clone());
    };

    replace_uri_path(routed_uri, target_path)
}

fn gemini_native_uri_for_model(
    model: Option<&str>,
    stream: bool,
    original_query: Option<&str>,
) -> Result<Uri, AppError> {
    let model = model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::InvalidInput("Gemini Native route is missing upstream model".to_string())
        })?;
    let model = model.strip_prefix("models/").unwrap_or(model);
    let method = if stream {
        "streamGenerateContent"
    } else {
        "generateContent"
    };
    let mut query_parts = original_query
        .into_iter()
        .flat_map(|query| query.split('&'))
        .filter(|part| !part.is_empty())
        .filter(|part| !part.starts_with("stream="))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if stream
        && !query_parts
            .iter()
            .any(|part| part == "alt=sse" || part.starts_with("alt="))
    {
        query_parts.push("alt=sse".to_string());
    }
    let query = if query_parts.is_empty() {
        String::new()
    } else {
        format!("?{}", query_parts.join("&"))
    };
    format!("/v1beta/models/{model}:{method}{query}")
        .parse()
        .map_err(|e| AppError::InvalidInput(format!("Invalid Gemini Native URI: {e}")))
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

fn effective_proxy_settings_for_app(settings: &ProxySettings, app: &AppType) -> ProxySettings {
    let app_settings = proxy_app_settings(settings, app);
    let mut effective = settings.clone();
    effective.streaming_first_byte_timeout = app_settings.streaming_first_byte_timeout;
    effective.streaming_idle_timeout = app_settings.streaming_idle_timeout;
    effective.non_streaming_timeout = app_settings.non_streaming_timeout;
    effective.circuit_failure_threshold = app_settings.circuit_failure_threshold;
    effective.circuit_recovery_threshold = app_settings.circuit_recovery_threshold;
    effective.circuit_recovery_wait_seconds = app_settings.circuit_recovery_wait_seconds;
    effective.circuit_error_rate_threshold = app_settings.circuit_error_rate_threshold;
    effective
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
    if matches!(app, AppType::ClaudeDesktop)
        && matches!(
            crate::claude_desktop_config::proxy_api_format(provider),
            Some("gemini_native")
        )
    {
        return "gemini";
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

fn build_copilot_header_plan(
    provider: &Provider,
    body: &serde_json::Value,
    has_anthropic_beta: bool,
    session_id: Option<&str>,
) -> Option<CopilotHeaderPlan> {
    if provider.meta.as_ref()?.provider_type() != Some(ProviderType::GithubCopilot) {
        return None;
    }

    let classification =
        super::copilot_optimizer::classify_request(body, has_anthropic_beta, true, true);
    let request_id = session_id.and_then(|session_id| {
        super::copilot_optimizer::deterministic_request_id(body, session_id)
    });
    let interaction_id =
        session_id.and_then(super::copilot_optimizer::deterministic_interaction_id);

    Some(CopilotHeaderPlan {
        initiator: classification.initiator,
        is_subagent: classification.is_subagent,
        request_id,
        interaction_id,
    })
}

fn apply_copilot_header_plan(headers: &mut reqwest::header::HeaderMap, plan: &CopilotHeaderPlan) {
    insert_header_if_valid(headers, "x-initiator", plan.initiator);
    if plan.is_subagent {
        insert_header_if_valid(headers, "x-interaction-type", "conversation-subagent");
    }
    if let Some(request_id) = plan.request_id.as_deref() {
        insert_header_if_valid(headers, "x-request-id", request_id);
        insert_header_if_valid(headers, "x-agent-task-id", request_id);
    }
    if let Some(interaction_id) = plan.interaction_id.as_deref() {
        insert_header_if_valid(headers, "x-interaction-id", interaction_id);
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

fn is_failover_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn provider_health_key(app: &AppType, provider_id: &str) -> String {
    format!("{}:{provider_id}", app.as_str())
}

fn provider_circuit_state_label(state: ProviderCircuitState) -> &'static str {
    match state {
        ProviderCircuitState::Healthy => "healthy",
        ProviderCircuitState::Open => "open",
        ProviderCircuitState::HalfOpen => "half_open",
    }
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
    let min_requests = proxy_app_settings(settings, app)
        .circuit_min_requests
        .max(1);
    if entry.failure_count >= threshold
        || (entry.window_requests >= min_requests
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

#[allow(clippy::too_many_arguments)]
async fn build_buffered_response(
    upstream: reqwest::Response,
    total_timeout: Duration,
    request_started_at: Instant,
    app_type: &str,
    response_api_format: Option<&'static str>,
    gemini_shadow: Arc<GeminiShadowStore>,
    provider_id: String,
    session_id: Option<String>,
    gemini_tool_schema_hints: Option<&super::gemini_schema::AnthropicToolSchemaHints>,
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
    let mut bytes = timeout_app_error(
        remaining_timeout(total_timeout, request_started_at),
        read_limited_upstream_body(upstream, PROXY_RESPONSE_LIMIT_BYTES),
        "Proxy upstream response body timed out",
    )
    .await??;
    if status.is_success() {
        let mapped = match response_api_format {
            Some("gemini_native") => {
                let body = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| {
                    AppError::Config(format!("Failed to parse Gemini Native response: {e}"))
                })?;
                Some(
                    crate::claude_desktop_config::map_gemini_native_response_to_anthropic_with_shadow_and_hints(
                        body,
                        Some(&gemini_shadow),
                        Some(&provider_id),
                        session_id.as_deref(),
                        gemini_tool_schema_hints,
                    )?,
                )
            }
            Some("openai_chat") => {
                let body = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| {
                    AppError::Config(format!("Failed to parse OpenAI Chat response: {e}"))
                })?;
                Some(crate::claude_desktop_config::map_openai_chat_response_to_anthropic(body)?)
            }
            Some("openai_responses") => {
                let body = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| {
                    AppError::Config(format!("Failed to parse OpenAI Responses response: {e}"))
                })?;
                Some(
                    crate::claude_desktop_config::map_openai_responses_response_to_anthropic(body)?,
                )
            }
            _ => None,
        };
        if let Some(mapped) = mapped {
            bytes = Bytes::from(mapped.to_string());
        }
    }
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

#[allow(clippy::too_many_arguments)]
async fn build_gemini_native_streaming_response(
    upstream: reqwest::Response,
    settings: &ProxySettings,
    app_type: &str,
    request_started_at: Instant,
    usage_context: StreamUsageContext,
    gemini_shadow: Arc<GeminiShadowStore>,
    provider_id: String,
    session_id: Option<String>,
    gemini_tool_schema_hints: Option<super::gemini_schema::AnthropicToolSchemaHints>,
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
    builder = builder.header(header::CONTENT_TYPE, "text/event-stream");

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
    let mut converter = GeminiNativeSseConverter::with_tool_schema_hints(gemini_tool_schema_hints);
    let (first, first_events) = converter
        .push_bytes(&first)
        .map_err(|err| StreamingResponseError::Other(AppError::Config(err)))?;
    let usage = TokenUsage::from_stream_events(app_type, &first_events);
    let events = first_events;

    let rest = stream::unfold(
        (
            upstream_stream,
            events,
            usage_context,
            first_token_ms,
            converter,
            gemini_shadow,
            provider_id,
            session_id,
            false,
        ),
        move |(
            mut stream,
            mut events,
            usage_context,
            first_token_ms,
            mut converter,
            gemini_shadow,
            provider_id,
            session_id,
            finished,
        )| {
            let idle_timeout = idle_timeout;
            async move {
                if finished {
                    return None;
                }
                match tokio::time::timeout(idle_timeout, stream.next()).await {
                    Ok(Some(Ok(bytes))) => match converter.push_bytes(&bytes) {
                        Ok((converted, parsed)) => {
                            events.extend(parsed);
                            Some((
                                Ok(converted),
                                (
                                    stream,
                                    events,
                                    usage_context,
                                    first_token_ms,
                                    converter,
                                    gemini_shadow,
                                    provider_id,
                                    session_id,
                                    false,
                                ),
                            ))
                        }
                        Err(err) => Some((
                            Err(std::io::Error::other(err)),
                            (
                                stream,
                                events,
                                usage_context,
                                first_token_ms,
                                converter,
                                gemini_shadow,
                                provider_id,
                                session_id,
                                true,
                            ),
                        )),
                    },
                    Ok(Some(Err(err))) => Some((
                        Err(std::io::Error::other(err)),
                        (
                            stream,
                            events,
                            usage_context,
                            first_token_ms,
                            converter,
                            gemini_shadow,
                            provider_id,
                            session_id,
                            true,
                        ),
                    )),
                    Ok(None) => {
                        let final_bytes = converter.finish();
                        if let (Some(session_id), Some((assistant_content, tool_calls))) =
                            (session_id.as_deref(), converter.shadow_assistant_turn())
                        {
                            gemini_shadow.record_assistant_turn(
                                provider_id.as_str(),
                                session_id,
                                assistant_content,
                                tool_calls,
                            );
                        }
                        persist_stream_usage_update(&usage_context, &events, first_token_ms);
                        if final_bytes.is_empty() {
                            None
                        } else {
                            Some((
                                Ok(final_bytes),
                                (
                                    stream,
                                    events,
                                    usage_context,
                                    first_token_ms,
                                    converter,
                                    gemini_shadow,
                                    provider_id,
                                    session_id,
                                    true,
                                ),
                            ))
                        }
                    }
                    Err(_) => Some((
                        Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "Proxy streaming idle timeout",
                        )),
                        (
                            stream,
                            events,
                            usage_context,
                            first_token_ms,
                            converter,
                            gemini_shadow,
                            provider_id,
                            session_id,
                            true,
                        ),
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

async fn build_openai_chat_streaming_response(
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
    builder = builder.header(header::CONTENT_TYPE, "text/event-stream");

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
    let mut converter = OpenAIChatSseConverter::default();
    let (first, first_events) = converter
        .push_bytes(&first)
        .map_err(|err| StreamingResponseError::Other(AppError::Config(err)))?;
    let usage = TokenUsage::from_stream_events(app_type, &first_events);
    let events = first_events;

    let rest = stream::unfold(
        (
            upstream_stream,
            events,
            usage_context,
            first_token_ms,
            converter,
            false,
        ),
        move |(mut stream, mut events, usage_context, first_token_ms, mut converter, finished)| {
            let idle_timeout = idle_timeout;
            async move {
                if finished {
                    return None;
                }
                match tokio::time::timeout(idle_timeout, stream.next()).await {
                    Ok(Some(Ok(bytes))) => match converter.push_bytes(&bytes) {
                        Ok((converted, parsed)) => {
                            events.extend(parsed);
                            Some((
                                Ok(converted),
                                (
                                    stream,
                                    events,
                                    usage_context,
                                    first_token_ms,
                                    converter,
                                    false,
                                ),
                            ))
                        }
                        Err(err) => Some((
                            Err(std::io::Error::other(err)),
                            (
                                stream,
                                events,
                                usage_context,
                                first_token_ms,
                                converter,
                                true,
                            ),
                        )),
                    },
                    Ok(Some(Err(err))) => Some((
                        Err(std::io::Error::other(err)),
                        (
                            stream,
                            events,
                            usage_context,
                            first_token_ms,
                            converter,
                            true,
                        ),
                    )),
                    Ok(None) => {
                        let final_bytes = converter.finish();
                        persist_stream_usage_update(&usage_context, &events, first_token_ms);
                        if final_bytes.is_empty() {
                            None
                        } else {
                            Some((
                                Ok(final_bytes),
                                (
                                    stream,
                                    events,
                                    usage_context,
                                    first_token_ms,
                                    converter,
                                    true,
                                ),
                            ))
                        }
                    }
                    Err(_) => Some((
                        Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "Proxy streaming idle timeout",
                        )),
                        (
                            stream,
                            events,
                            usage_context,
                            first_token_ms,
                            converter,
                            true,
                        ),
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

async fn build_openai_responses_streaming_response(
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
    builder = builder.header(header::CONTENT_TYPE, "text/event-stream");

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
    let mut converter = OpenAIResponsesSseConverter::default();
    let (first, first_events) = converter
        .push_bytes(&first)
        .map_err(|err| StreamingResponseError::Other(AppError::Config(err)))?;
    let usage = TokenUsage::from_stream_events(app_type, &first_events);
    let events = first_events;

    let rest = stream::unfold(
        (
            upstream_stream,
            events,
            usage_context,
            first_token_ms,
            converter,
            false,
        ),
        move |(mut stream, mut events, usage_context, first_token_ms, mut converter, finished)| {
            let idle_timeout = idle_timeout;
            async move {
                if finished {
                    return None;
                }
                match tokio::time::timeout(idle_timeout, stream.next()).await {
                    Ok(Some(Ok(bytes))) => match converter.push_bytes(&bytes) {
                        Ok((converted, parsed)) => {
                            events.extend(parsed);
                            Some((
                                Ok(converted),
                                (
                                    stream,
                                    events,
                                    usage_context,
                                    first_token_ms,
                                    converter,
                                    false,
                                ),
                            ))
                        }
                        Err(err) => Some((
                            Err(std::io::Error::other(err)),
                            (
                                stream,
                                events,
                                usage_context,
                                first_token_ms,
                                converter,
                                true,
                            ),
                        )),
                    },
                    Ok(Some(Err(err))) => Some((
                        Err(std::io::Error::other(err)),
                        (
                            stream,
                            events,
                            usage_context,
                            first_token_ms,
                            converter,
                            true,
                        ),
                    )),
                    Ok(None) => {
                        let final_bytes = converter.finish();
                        persist_stream_usage_update(&usage_context, &events, first_token_ms);
                        if final_bytes.is_empty() {
                            None
                        } else {
                            Some((
                                Ok(final_bytes),
                                (
                                    stream,
                                    events,
                                    usage_context,
                                    first_token_ms,
                                    converter,
                                    true,
                                ),
                            ))
                        }
                    }
                    Err(_) => Some((
                        Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "Proxy streaming idle timeout",
                        )),
                        (
                            stream,
                            events,
                            usage_context,
                            first_token_ms,
                            converter,
                            true,
                        ),
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

#[derive(Default)]
struct OpenAIChatSseConverter {
    buffer: String,
    utf8_remainder: Vec<u8>,
    started: bool,
    completed: bool,
    response_id: Option<String>,
    model: Option<String>,
    next_content_index: u64,
    current_block_kind: Option<&'static str>,
    current_block_index: Option<u64>,
    tool_blocks_by_index: HashMap<usize, OpenAIChatToolBlock>,
    open_tool_indices: HashSet<u64>,
    latest_usage: Option<serde_json::Value>,
    pending_message_delta: Option<(Option<String>, Option<serde_json::Value>)>,
    has_emitted_message_delta: bool,
}

#[derive(Default)]
struct OpenAIChatToolBlock {
    anthropic_index: u64,
    id: String,
    name: String,
    started: bool,
    pending_args: String,
    consecutive_whitespace: usize,
    aborted: bool,
}

const OPENAI_CHAT_INFINITE_WHITESPACE_THRESHOLD: usize = 20;

impl OpenAIChatSseConverter {
    fn push_bytes(&mut self, bytes: &Bytes) -> Result<(Bytes, Vec<serde_json::Value>), String> {
        append_utf8_safe(&mut self.buffer, &mut self.utf8_remainder, bytes);
        let mut output = Vec::new();
        let mut parsed_events = Vec::new();

        while let Some(block_end) = self.buffer.find("\n\n") {
            let block = self.buffer[..block_end].to_string();
            self.buffer.drain(..block_end + 2);
            let Some(data) = sse_block_data(&block) else {
                continue;
            };
            if data == "[DONE]" {
                output.extend(self.finish_done());
                continue;
            }
            let event = serde_json::from_str::<serde_json::Value>(&data)
                .map_err(|err| format!("Failed to parse OpenAI Chat SSE event: {err}"))?;
            parsed_events.push(event.clone());
            output.extend(self.process_event(&event));
        }

        Ok((Bytes::from(output), parsed_events))
    }

    fn process_event(&mut self, event: &serde_json::Value) -> Vec<u8> {
        self.capture_meta(event);
        let mut output = Vec::new();

        if let Some(usage) = event.get("usage").filter(|usage| !usage.is_null()) {
            let usage = crate::claude_desktop_config::openai_chat_usage_to_anthropic(Some(usage));
            self.latest_usage = Some(usage.clone());
            if let Some((_, pending_usage)) = self.pending_message_delta.as_mut() {
                *pending_usage = Some(usage);
            }
        }

        let Some(choice) = event
            .get("choices")
            .and_then(serde_json::Value::as_array)
            .and_then(|choices| choices.first())
        else {
            return output;
        };

        output.extend(self.ensure_message_start(event.get("usage")));

        let delta = choice.get("delta").unwrap_or(&serde_json::Value::Null);
        if let Some(reasoning) = delta
            .get("reasoning")
            .or_else(|| delta.get("reasoning_content"))
            .and_then(serde_json::Value::as_str)
        {
            output.extend(self.non_tool_delta("thinking", reasoning));
        }
        if let Some(content) = delta.get("content").and_then(serde_json::Value::as_str) {
            output.extend(self.non_tool_delta("text", content));
        }
        if let Some(tool_calls) = delta
            .get("tool_calls")
            .and_then(serde_json::Value::as_array)
        {
            output.extend(self.close_current_non_tool_block());
            for tool_call in tool_calls {
                output.extend(self.tool_call_delta(tool_call));
            }
        }
        if let Some(finish_reason) = choice
            .get("finish_reason")
            .and_then(serde_json::Value::as_str)
        {
            output.extend(self.handle_finish_reason(finish_reason));
        }

        output
    }

    fn capture_meta(&mut self, event: &serde_json::Value) {
        if self.response_id.is_none() {
            self.response_id = event
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string);
        }
        if self.model.is_none() {
            self.model = event
                .get("model")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string);
        }
    }

    fn ensure_message_start(&mut self, usage: Option<&serde_json::Value>) -> Vec<u8> {
        if self.started {
            return Vec::new();
        }
        self.started = true;
        let usage = crate::claude_desktop_config::openai_chat_usage_to_anthropic(usage);
        let event = serde_json::json!({
            "type": "message_start",
            "message": {
                "id": self.response_id.as_deref().unwrap_or(""),
                "type": "message",
                "role": "assistant",
                "model": self.model.as_deref().unwrap_or(""),
                "usage": usage
            }
        });
        encode_sse_event("message_start", &event)
    }

    fn non_tool_delta(&mut self, kind: &'static str, delta: &str) -> Vec<u8> {
        if delta.is_empty() {
            return Vec::new();
        }
        let mut output = Vec::new();
        if self.current_block_kind != Some(kind) {
            output.extend(self.close_current_non_tool_block());
            let index = self.next_content_index();
            self.current_block_kind = Some(kind);
            self.current_block_index = Some(index);
            let content_block = if kind == "thinking" {
                serde_json::json!({ "type": "thinking", "thinking": "" })
            } else {
                serde_json::json!({ "type": "text", "text": "" })
            };
            let event = serde_json::json!({
                "type": "content_block_start",
                "index": index,
                "content_block": content_block
            });
            output.extend(encode_sse_event("content_block_start", &event));
        }
        let index = self.current_block_index.unwrap_or(0);
        let delta_value = if kind == "thinking" {
            serde_json::json!({ "type": "thinking_delta", "thinking": delta })
        } else {
            serde_json::json!({ "type": "text_delta", "text": delta })
        };
        let event = serde_json::json!({
            "type": "content_block_delta",
            "index": index,
            "delta": delta_value
        });
        output.extend(encode_sse_event("content_block_delta", &event));
        output
    }

    fn close_current_non_tool_block(&mut self) -> Vec<u8> {
        let Some(index) = self.current_block_index.take() else {
            self.current_block_kind = None;
            return Vec::new();
        };
        self.current_block_kind = None;
        let event = serde_json::json!({
            "type": "content_block_stop",
            "index": index
        });
        encode_sse_event("content_block_stop", &event)
    }

    fn tool_call_delta(&mut self, tool_call: &serde_json::Value) -> Vec<u8> {
        let tool_index = tool_call
            .get("index")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;

        let mut next_index = self.next_content_index;
        let state = self
            .tool_blocks_by_index
            .entry(tool_index)
            .or_insert_with(|| {
                let index = next_index;
                next_index = next_index.saturating_add(1);
                OpenAIChatToolBlock {
                    anthropic_index: index,
                    ..OpenAIChatToolBlock::default()
                }
            });
        self.next_content_index = next_index;

        if state.aborted {
            return Vec::new();
        }
        if let Some(id) = tool_call.get("id").and_then(serde_json::Value::as_str) {
            state.id = id.to_string();
        }
        if let Some(name) = tool_call
            .get("function")
            .and_then(|function| function.get("name"))
            .and_then(serde_json::Value::as_str)
        {
            state.name = name.to_string();
        }

        let args_delta = tool_call
            .get("function")
            .and_then(|function| function.get("arguments"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);

        let should_start = !state.started && !state.id.is_empty() && !state.name.is_empty();
        if should_start {
            state.started = true;
        }

        let pending_after_start = if should_start && !state.pending_args.is_empty() {
            Some(std::mem::take(&mut state.pending_args))
        } else {
            None
        };

        let immediate_delta = args_delta.and_then(|args| {
            for ch in args.chars() {
                if ch.is_whitespace() {
                    state.consecutive_whitespace = state.consecutive_whitespace.saturating_add(1);
                } else {
                    state.consecutive_whitespace = 0;
                }
            }
            if state.consecutive_whitespace >= OPENAI_CHAT_INFINITE_WHITESPACE_THRESHOLD {
                state.aborted = true;
                return None;
            }
            if state.started {
                Some(args)
            } else {
                state.pending_args.push_str(&args);
                None
            }
        });

        let anthropic_index = state.anthropic_index;
        let id = state.id.clone();
        let name = state.name.clone();
        let mut output = Vec::new();
        if should_start {
            let event = serde_json::json!({
                "type": "content_block_start",
                "index": anthropic_index,
                "content_block": {
                    "type": "tool_use",
                    "id": id,
                    "name": name
                }
            });
            output.extend(encode_sse_event("content_block_start", &event));
            self.open_tool_indices.insert(anthropic_index);
        }
        for args in pending_after_start.into_iter().chain(immediate_delta) {
            let event = serde_json::json!({
                "type": "content_block_delta",
                "index": anthropic_index,
                "delta": { "type": "input_json_delta", "partial_json": args }
            });
            output.extend(encode_sse_event("content_block_delta", &event));
        }
        output
    }

    fn handle_finish_reason(&mut self, finish_reason: &str) -> Vec<u8> {
        let stop_reason = openai_chat_stop_reason(Some(finish_reason));
        let usage = self.latest_usage.clone();
        if self.has_emitted_message_delta {
            if let (Some((_, pending_usage)), Some(usage)) =
                (self.pending_message_delta.as_mut(), usage)
            {
                *pending_usage = Some(usage);
            }
            return Vec::new();
        }
        self.has_emitted_message_delta = true;

        let mut output = self.close_current_non_tool_block();
        output.extend(self.late_start_pending_tools());
        output.extend(self.close_open_tools());
        self.pending_message_delta = Some((stop_reason, usage));
        output
    }

    fn late_start_pending_tools(&mut self) -> Vec<u8> {
        let mut late_starts = Vec::new();
        for (tool_index, state) in self.tool_blocks_by_index.iter_mut() {
            if state.started || state.aborted {
                continue;
            }
            let has_payload =
                !state.pending_args.is_empty() || !state.id.is_empty() || !state.name.is_empty();
            if !has_payload {
                continue;
            }
            let id = if state.id.is_empty() {
                format!("tool_call_{tool_index}")
            } else {
                state.id.clone()
            };
            let name = if state.name.is_empty() {
                "unknown_tool".to_string()
            } else {
                state.name.clone()
            };
            state.started = true;
            late_starts.push((
                state.anthropic_index,
                id,
                name,
                std::mem::take(&mut state.pending_args),
            ));
        }
        late_starts.sort_unstable_by_key(|(index, _, _, _)| *index);

        let mut output = Vec::new();
        for (index, id, name, pending) in late_starts {
            let event = serde_json::json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {
                    "type": "tool_use",
                    "id": id,
                    "name": name
                }
            });
            output.extend(encode_sse_event("content_block_start", &event));
            self.open_tool_indices.insert(index);
            if !pending.is_empty() {
                let event = serde_json::json!({
                    "type": "content_block_delta",
                    "index": index,
                    "delta": { "type": "input_json_delta", "partial_json": pending }
                });
                output.extend(encode_sse_event("content_block_delta", &event));
            }
        }
        output
    }

    fn close_open_tools(&mut self) -> Vec<u8> {
        let mut output = Vec::new();
        let mut indices: Vec<_> = self.open_tool_indices.iter().copied().collect();
        indices.sort_unstable();
        for index in indices {
            let event = serde_json::json!({
                "type": "content_block_stop",
                "index": index
            });
            output.extend(encode_sse_event("content_block_stop", &event));
            self.open_tool_indices.remove(&index);
        }
        output
    }

    fn finish_done(&mut self) -> Vec<u8> {
        if self.completed {
            return Vec::new();
        }
        self.completed = true;
        let mut output = Vec::new();
        if let Some((stop_reason, usage)) = self.pending_message_delta.take() {
            let usage = usage.unwrap_or_else(|| {
                serde_json::json!({
                    "input_tokens": 0,
                    "output_tokens": 0
                })
            });
            let event = serde_json::json!({
                "type": "message_delta",
                "delta": {
                    "stop_reason": stop_reason,
                    "stop_sequence": serde_json::Value::Null
                },
                "usage": usage
            });
            output.extend(encode_sse_event("message_delta", &event));
        }
        if self.started {
            let event = serde_json::json!({ "type": "message_stop" });
            output.extend(encode_sse_event("message_stop", &event));
        }
        output
    }

    fn next_content_index(&mut self) -> u64 {
        let index = self.next_content_index;
        self.next_content_index = self.next_content_index.saturating_add(1);
        index
    }

    fn finish(&mut self) -> Bytes {
        if self.completed {
            return Bytes::new();
        }
        if self.pending_message_delta.is_none() {
            return Bytes::new();
        }
        Bytes::from(self.finish_done())
    }
}

fn openai_chat_stop_reason(reason: Option<&str>) -> Option<String> {
    reason.map(|reason| {
        match reason {
            "tool_calls" | "function_call" => "tool_use",
            "stop" => "end_turn",
            "length" => "max_tokens",
            "content_filter" => "end_turn",
            other => {
                log::warn!("[Claude/OpenAI] Unknown finish_reason in streaming: {other}");
                "end_turn"
            }
        }
        .to_string()
    })
}

#[derive(Default)]
struct OpenAIResponsesSseConverter {
    buffer: String,
    utf8_remainder: Vec<u8>,
    started: bool,
    completed: bool,
    response_id: Option<String>,
    model: Option<String>,
    has_tool_use: bool,
    next_content_index: u64,
    current_text_index: Option<u64>,
    current_thinking_index: Option<u64>,
    open_indices: HashSet<u64>,
    tool_index_by_item_id: HashMap<String, u64>,
    tool_name_by_index: HashMap<u64, String>,
    tool_args_by_index: HashMap<u64, String>,
    last_tool_index: Option<u64>,
}

impl OpenAIResponsesSseConverter {
    fn push_bytes(&mut self, bytes: &Bytes) -> Result<(Bytes, Vec<serde_json::Value>), String> {
        append_utf8_safe(&mut self.buffer, &mut self.utf8_remainder, bytes);
        let mut output = Vec::new();
        let mut parsed_events = Vec::new();

        while let Some(block_end) = self.buffer.find("\n\n") {
            let block = self.buffer[..block_end].to_string();
            self.buffer.drain(..block_end + 2);
            let Some(data) = sse_block_data(&block) else {
                continue;
            };
            if data == "[DONE]" {
                continue;
            }
            let event = serde_json::from_str::<serde_json::Value>(&data)
                .map_err(|err| format!("Failed to parse OpenAI Responses SSE event: {err}"))?;
            let event_name = sse_block_event(&block)
                .or_else(|| {
                    event
                        .get("type")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default();
            parsed_events.push(event.clone());
            output.extend(self.process_event(&event_name, &event));
        }

        Ok((Bytes::from(output), parsed_events))
    }

    fn process_event(&mut self, event_name: &str, event: &serde_json::Value) -> Vec<u8> {
        let mut output = Vec::new();
        match event_name {
            "response.created" => {
                let response = response_object_from_event(event);
                self.capture_response_meta(response);
                output.extend(self.ensure_message_start(response.get("usage")));
            }
            "response.content_part.added" => {
                if let Some(part_type) = event
                    .get("part")
                    .and_then(|part| part.get("type"))
                    .and_then(serde_json::Value::as_str)
                {
                    if matches!(part_type, "output_text" | "refusal") {
                        output.extend(self.ensure_message_start(None));
                        output.extend(self.ensure_text_block());
                    }
                }
            }
            "response.output_text.delta" | "response.refusal.delta" => {
                if let Some(delta) = event.get("delta").and_then(serde_json::Value::as_str) {
                    output.extend(self.ensure_message_start(None));
                    output.extend(self.text_delta(delta));
                }
            }
            "response.output_text.done" | "response.refusal.done" => {
                output.extend(self.close_text_block());
            }
            "response.output_item.added" => {
                if let Some(item) = event.get("item") {
                    if item.get("type").and_then(serde_json::Value::as_str) == Some("function_call")
                    {
                        output.extend(self.close_text_block());
                        output.extend(self.close_thinking_block());
                        output.extend(self.ensure_message_start(None));
                        output.extend(self.start_tool_block(event, item));
                    }
                }
            }
            "response.function_call_arguments.delta" => {
                if let Some(delta) = event.get("delta").and_then(serde_json::Value::as_str) {
                    output.extend(self.ensure_message_start(None));
                    output.extend(self.tool_arguments_delta(event, delta));
                }
            }
            "response.function_call_arguments.done" => {
                output.extend(self.finish_tool_arguments(event));
            }
            "response.reasoning.delta" => {
                if let Some(delta) = event
                    .get("delta")
                    .or_else(|| event.get("text"))
                    .and_then(serde_json::Value::as_str)
                {
                    output.extend(self.close_text_block());
                    output.extend(self.ensure_message_start(None));
                    output.extend(self.thinking_delta(delta));
                }
            }
            "response.reasoning.done" => {
                output.extend(self.close_thinking_block());
            }
            "response.completed" => {
                let response = response_object_from_event(event);
                self.capture_response_meta(response);
                output.extend(self.finish_with_response(response));
            }
            _ => {}
        }
        output
    }

    fn capture_response_meta(&mut self, response: &serde_json::Value) {
        if self.response_id.is_none() {
            self.response_id = response
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string);
        }
        if self.model.is_none() {
            self.model = response
                .get("model")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string);
        }
    }

    fn ensure_message_start(&mut self, usage: Option<&serde_json::Value>) -> Vec<u8> {
        if self.started {
            return Vec::new();
        }
        self.started = true;
        let usage = crate::claude_desktop_config::openai_responses_usage_to_anthropic(usage);
        let event = serde_json::json!({
            "type": "message_start",
            "message": {
                "id": self.response_id.as_deref().unwrap_or(""),
                "type": "message",
                "role": "assistant",
                "model": self.model.as_deref().unwrap_or(""),
                "usage": usage
            }
        });
        encode_sse_event("message_start", &event)
    }

    fn ensure_text_block(&mut self) -> Vec<u8> {
        if let Some(index) = self.current_text_index {
            if self.open_indices.contains(&index) {
                return Vec::new();
            }
        }
        let index = self
            .current_text_index
            .unwrap_or_else(|| self.next_content_index());
        self.current_text_index = Some(index);
        self.open_indices.insert(index);
        let event = serde_json::json!({
            "type": "content_block_start",
            "index": index,
            "content_block": { "type": "text", "text": "" }
        });
        encode_sse_event("content_block_start", &event)
    }

    fn text_delta(&mut self, delta: &str) -> Vec<u8> {
        if delta.is_empty() {
            return Vec::new();
        }
        let mut output = self.ensure_text_block();
        let index = self.current_text_index.unwrap_or(0);
        let event = serde_json::json!({
            "type": "content_block_delta",
            "index": index,
            "delta": { "type": "text_delta", "text": delta }
        });
        output.extend(encode_sse_event("content_block_delta", &event));
        output
    }

    fn close_text_block(&mut self) -> Vec<u8> {
        let Some(index) = self.current_text_index.take() else {
            return Vec::new();
        };
        if !self.open_indices.remove(&index) {
            return Vec::new();
        }
        let event = serde_json::json!({
            "type": "content_block_stop",
            "index": index
        });
        encode_sse_event("content_block_stop", &event)
    }

    fn thinking_delta(&mut self, delta: &str) -> Vec<u8> {
        if delta.is_empty() {
            return Vec::new();
        }
        let mut output = Vec::new();
        let index = match self.current_thinking_index {
            Some(index) => index,
            None => {
                let index = self.next_content_index();
                self.current_thinking_index = Some(index);
                self.open_indices.insert(index);
                let event = serde_json::json!({
                    "type": "content_block_start",
                    "index": index,
                    "content_block": { "type": "thinking", "thinking": "" }
                });
                output.extend(encode_sse_event("content_block_start", &event));
                index
            }
        };
        let event = serde_json::json!({
            "type": "content_block_delta",
            "index": index,
            "delta": { "type": "thinking_delta", "thinking": delta }
        });
        output.extend(encode_sse_event("content_block_delta", &event));
        output
    }

    fn close_thinking_block(&mut self) -> Vec<u8> {
        let Some(index) = self.current_thinking_index.take() else {
            return Vec::new();
        };
        if !self.open_indices.remove(&index) {
            return Vec::new();
        }
        let event = serde_json::json!({
            "type": "content_block_stop",
            "index": index
        });
        encode_sse_event("content_block_stop", &event)
    }

    fn start_tool_block(&mut self, event: &serde_json::Value, item: &serde_json::Value) -> Vec<u8> {
        self.has_tool_use = true;
        let index = self.next_content_index();
        let item_id = item
            .get("id")
            .and_then(serde_json::Value::as_str)
            .or_else(|| event.get("item_id").and_then(serde_json::Value::as_str));
        if let Some(item_id) = item_id {
            self.tool_index_by_item_id
                .insert(item_id.to_string(), index);
        }
        let name = item
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        self.tool_name_by_index.insert(index, name.to_string());
        self.tool_args_by_index.insert(index, String::new());
        self.last_tool_index = Some(index);
        self.open_indices.insert(index);
        let event = serde_json::json!({
            "type": "content_block_start",
            "index": index,
            "content_block": {
                "type": "tool_use",
                "id": item.get("call_id").and_then(serde_json::Value::as_str).unwrap_or(""),
                "name": name
            }
        });
        encode_sse_event("content_block_start", &event)
    }

    fn tool_arguments_delta(&mut self, event: &serde_json::Value, delta: &str) -> Vec<u8> {
        let Some(index) = self.tool_index_for_event(event) else {
            return Vec::new();
        };
        let name = self
            .tool_name_by_index
            .get(&index)
            .map(String::as_str)
            .unwrap_or("");
        if name == "Read" {
            self.tool_args_by_index
                .entry(index)
                .or_default()
                .push_str(delta);
            return Vec::new();
        }
        let event = serde_json::json!({
            "type": "content_block_delta",
            "index": index,
            "delta": { "type": "input_json_delta", "partial_json": delta }
        });
        encode_sse_event("content_block_delta", &event)
    }

    fn finish_tool_arguments(&mut self, event: &serde_json::Value) -> Vec<u8> {
        let Some(index) = self.tool_index_for_event(event) else {
            return Vec::new();
        };
        let mut output = Vec::new();
        if self.tool_name_by_index.get(&index).map(String::as_str) == Some("Read") {
            let raw = event
                .get("arguments")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    self.tool_args_by_index
                        .get(&index)
                        .cloned()
                        .unwrap_or_default()
                });
            let sanitized = sanitize_read_arguments_json(&raw);
            if !sanitized.is_empty() {
                let event = serde_json::json!({
                    "type": "content_block_delta",
                    "index": index,
                    "delta": {
                        "type": "input_json_delta",
                        "partial_json": sanitized
                    }
                });
                output.extend(encode_sse_event("content_block_delta", &event));
            }
        }
        if self.open_indices.remove(&index) {
            let event = serde_json::json!({
                "type": "content_block_stop",
                "index": index
            });
            output.extend(encode_sse_event("content_block_stop", &event));
        }
        if let Some(item_id) = event.get("item_id").and_then(serde_json::Value::as_str) {
            self.tool_index_by_item_id.remove(item_id);
        }
        self.tool_name_by_index.remove(&index);
        self.tool_args_by_index.remove(&index);
        output
    }

    fn tool_index_for_event(&self, event: &serde_json::Value) -> Option<u64> {
        event
            .get("item_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|item_id| self.tool_index_by_item_id.get(item_id).copied())
            .or(self.last_tool_index)
    }

    fn finish_with_response(&mut self, response: &serde_json::Value) -> Vec<u8> {
        if self.completed {
            return Vec::new();
        }
        self.completed = true;
        let mut output = self.ensure_message_start(response.get("usage"));
        output.extend(self.close_text_block());
        output.extend(self.close_thinking_block());
        output.extend(self.close_remaining_blocks());
        let stop_reason = crate::claude_desktop_config::map_responses_stop_reason(
            response.get("status").and_then(serde_json::Value::as_str),
            self.has_tool_use,
            response
                .pointer("/incomplete_details/reason")
                .and_then(serde_json::Value::as_str),
        );
        let usage = crate::claude_desktop_config::openai_responses_usage_to_anthropic(
            response.get("usage"),
        );
        let message_delta = serde_json::json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": stop_reason,
                "stop_sequence": serde_json::Value::Null
            },
            "usage": usage
        });
        output.extend(encode_sse_event("message_delta", &message_delta));
        let message_stop = serde_json::json!({ "type": "message_stop" });
        output.extend(encode_sse_event("message_stop", &message_stop));
        output
    }

    fn close_remaining_blocks(&mut self) -> Vec<u8> {
        let mut output = Vec::new();
        let mut indices: Vec<_> = self.open_indices.iter().copied().collect();
        indices.sort_unstable();
        for index in indices {
            let event = serde_json::json!({
                "type": "content_block_stop",
                "index": index
            });
            output.extend(encode_sse_event("content_block_stop", &event));
            self.open_indices.remove(&index);
        }
        output
    }

    fn next_content_index(&mut self) -> u64 {
        let index = self.next_content_index;
        self.next_content_index = self.next_content_index.saturating_add(1);
        index
    }

    fn finish(&mut self) -> Bytes {
        if self.completed {
            return Bytes::new();
        }
        let response = serde_json::json!({
            "id": self.response_id.as_deref().unwrap_or(""),
            "model": self.model.as_deref().unwrap_or(""),
            "status": "completed",
            "usage": serde_json::Value::Null
        });
        Bytes::from(self.finish_with_response(&response))
    }
}

fn response_object_from_event(event: &serde_json::Value) -> &serde_json::Value {
    event.get("response").unwrap_or(event)
}

fn sanitize_read_arguments_json(raw: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return raw.to_string();
    };
    if let Some(object) = value.as_object_mut() {
        if matches!(object.get("pages"), Some(serde_json::Value::String(value)) if value.is_empty())
        {
            object.remove("pages");
        }
    }
    serde_json::to_string(&value).unwrap_or_else(|_| raw.to_string())
}

#[derive(Default)]
struct GeminiNativeSseConverter {
    buffer: String,
    utf8_remainder: Vec<u8>,
    started: bool,
    text_block_index: Option<u64>,
    next_content_index: u64,
    emitted_text: String,
    text_thought_signature: Option<String>,
    tool_calls: Vec<GeminiToolCallMeta>,
    latest_usage: Option<serde_json::Value>,
    latest_finish_reason: Option<String>,
    response_id: Option<String>,
    model: Option<String>,
    tool_schema_hints: Option<super::gemini_schema::AnthropicToolSchemaHints>,
}

impl GeminiNativeSseConverter {
    fn with_tool_schema_hints(
        tool_schema_hints: Option<super::gemini_schema::AnthropicToolSchemaHints>,
    ) -> Self {
        Self {
            tool_schema_hints,
            ..Self::default()
        }
    }

    fn push_bytes(&mut self, bytes: &Bytes) -> Result<(Bytes, Vec<serde_json::Value>), String> {
        append_utf8_safe(&mut self.buffer, &mut self.utf8_remainder, bytes);
        let mut output = Vec::new();
        let mut parsed_events = Vec::new();

        while let Some(block_end) = self.buffer.find("\n\n") {
            let block = self.buffer[..block_end].to_string();
            self.buffer.drain(..block_end + 2);
            let Some(data) = sse_block_data(&block) else {
                continue;
            };
            if data == "[DONE]" {
                continue;
            }
            let event = serde_json::from_str::<serde_json::Value>(&data)
                .map_err(|err| format!("Failed to parse Gemini Native SSE event: {err}"))?;
            parsed_events.push(event.clone());
            output.extend(self.process_event(&event));
        }

        Ok((Bytes::from(output), parsed_events))
    }

    fn process_event(&mut self, event: &serde_json::Value) -> Vec<u8> {
        if self.response_id.is_none() {
            self.response_id = event
                .get("responseId")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string);
        }
        if self.model.is_none() {
            self.model = event
                .get("modelVersion")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string);
        }
        if let Some(usage) = event.get("usageMetadata") {
            self.latest_usage = Some(usage.clone());
        }
        if let Some(candidate) = event
            .get("candidates")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
        {
            if let Some(reason) = candidate
                .get("finishReason")
                .and_then(serde_json::Value::as_str)
            {
                self.latest_finish_reason = Some(reason.to_string());
            }
        }

        let mut output = Vec::new();
        if !self.started {
            output.extend(self.message_start());
            self.started = true;
        }
        if let Some(block_reason) = event
            .get("promptFeedback")
            .and_then(|value| value.get("blockReason"))
            .and_then(serde_json::Value::as_str)
        {
            let text = format!("Request blocked by Gemini safety filters: {block_reason}");
            output.extend(self.text_delta(&text));
            self.latest_finish_reason = Some("SAFETY".to_string());
            return output;
        }

        let Some(parts) = event
            .get("candidates")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .and_then(|candidate| candidate.get("content"))
            .and_then(|content| content.get("parts"))
            .and_then(serde_json::Value::as_array)
        else {
            return output;
        };

        let visible_text = parts
            .iter()
            .filter(|part| part.get("thought").and_then(serde_json::Value::as_bool) != Some(true))
            .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
            .collect::<String>();
        if visible_text.len() > self.emitted_text.len()
            && visible_text.starts_with(&self.emitted_text)
        {
            let delta = visible_text[self.emitted_text.len()..].to_string();
            self.emitted_text = visible_text;
            output.extend(self.text_delta(&delta));
        } else if visible_text != self.emitted_text {
            self.emitted_text = visible_text.clone();
            output.extend(self.text_delta(&visible_text));
        }

        if let Some(signature) = extract_gemini_text_thought_signature(parts) {
            self.text_thought_signature = Some(signature);
        }

        let mut incoming: Vec<GeminiToolCallMeta> = parts
            .iter()
            .enumerate()
            .filter_map(|(index, part)| {
                let function_call = part.get("functionCall")?;
                let id = function_call
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
                    .unwrap_or_else(|| format!("gemini_synth_{index}"));
                Some(GeminiToolCallMeta {
                    id: Some(id),
                    name: function_call
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    args: function_call
                        .get("args")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({})),
                    thought_signature: part
                        .get("thoughtSignature")
                        .or_else(|| part.get("thought_signature"))
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string),
                })
            })
            .collect();
        for tool_call in &mut incoming {
            super::gemini_schema::rectify_gemini_tool_call_args(
                &tool_call.name,
                &mut tool_call.args,
                self.tool_schema_hints.as_ref(),
            );
        }
        merge_gemini_tool_call_snapshots(&mut self.tool_calls, incoming);

        output
    }

    fn message_start(&self) -> Vec<u8> {
        let event = serde_json::json!({
            "type": "message_start",
            "message": {
                "id": self.response_id.as_deref().unwrap_or(""),
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": self.model.as_deref().unwrap_or(""),
                "stop_reason": serde_json::Value::Null,
                "stop_sequence": serde_json::Value::Null,
                "usage": { "input_tokens": 0, "output_tokens": 0 }
            }
        });
        encode_sse_event("message_start", &event)
    }

    fn text_delta(&mut self, delta: &str) -> Vec<u8> {
        if delta.is_empty() {
            return Vec::new();
        }
        let mut output = Vec::new();
        let index = match self.text_block_index {
            Some(index) => index,
            None => {
                let index = self.next_content_index;
                self.next_content_index = self.next_content_index.saturating_add(1);
                self.text_block_index = Some(index);
                let event = serde_json::json!({
                    "type": "content_block_start",
                    "index": index,
                    "content_block": { "type": "text", "text": "" }
                });
                output.extend(encode_sse_event("content_block_start", &event));
                index
            }
        };
        let event = serde_json::json!({
            "type": "content_block_delta",
            "index": index,
            "delta": { "type": "text_delta", "text": delta }
        });
        output.extend(encode_sse_event("content_block_delta", &event));
        output
    }

    fn finish(&mut self) -> Bytes {
        let mut output = Vec::new();
        if !self.started {
            output.extend(self.message_start());
            self.started = true;
        }
        if let Some(index) = self.text_block_index.take() {
            let event = serde_json::json!({
                "type": "content_block_stop",
                "index": index
            });
            output.extend(encode_sse_event("content_block_stop", &event));
        }
        for tool_call in &self.tool_calls {
            let index = self.next_content_index;
            self.next_content_index = self.next_content_index.saturating_add(1);
            let start = serde_json::json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {
                    "type": "tool_use",
                    "id": tool_call.id.as_deref().unwrap_or(""),
                    "name": tool_call.name.as_str()
                }
            });
            output.extend(encode_sse_event("content_block_start", &start));
            let delta = serde_json::json!({
                "type": "content_block_delta",
                "index": index,
                "delta": {
                    "type": "input_json_delta",
                    "partial_json": serde_json::to_string(
                        &tool_call.args
                    ).unwrap_or_else(|_| "{}".to_string())
                }
            });
            output.extend(encode_sse_event("content_block_delta", &delta));
            let stop = serde_json::json!({
                "type": "content_block_stop",
                "index": index
            });
            output.extend(encode_sse_event("content_block_stop", &stop));
        }
        let usage =
            crate::claude_desktop_config::gemini_usage_to_anthropic(self.latest_usage.as_ref());
        let stop_reason = crate::claude_desktop_config::gemini_finish_reason_to_anthropic(
            self.latest_finish_reason.as_deref(),
            !self.tool_calls.is_empty(),
        );
        let message_delta = serde_json::json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": stop_reason,
                "stop_sequence": serde_json::Value::Null
            },
            "usage": usage
        });
        output.extend(encode_sse_event("message_delta", &message_delta));
        let message_stop = serde_json::json!({ "type": "message_stop" });
        output.extend(encode_sse_event("message_stop", &message_stop));
        Bytes::from(output)
    }

    fn shadow_assistant_turn(&self) -> Option<(serde_json::Value, Vec<GeminiToolCallMeta>)> {
        let mut parts = Vec::new();
        if !self.emitted_text.is_empty() || self.text_thought_signature.is_some() {
            let mut text_part = serde_json::json!({ "text": self.emitted_text });
            if let Some(signature) = &self.text_thought_signature {
                text_part["thoughtSignature"] = serde_json::json!(signature);
            }
            parts.push(text_part);
        }
        for tool_call in &self.tool_calls {
            let mut part = serde_json::json!({
                "functionCall": {
                    "id": tool_call.id.as_deref().unwrap_or(""),
                    "name": tool_call.name.as_str(),
                    "args": &tool_call.args
                }
            });
            if let Some(signature) = &tool_call.thought_signature {
                part["thoughtSignature"] = serde_json::json!(signature);
            }
            parts.push(part);
        }
        (!parts.is_empty()).then_some((
            serde_json::json!({ "parts": parts }),
            self.tool_calls.clone(),
        ))
    }
}

fn extract_gemini_text_thought_signature(parts: &[serde_json::Value]) -> Option<String> {
    parts
        .iter()
        .filter(|part| part.get("text").is_some() && part.get("functionCall").is_none())
        .filter_map(|part| {
            part.get("thoughtSignature")
                .or_else(|| part.get("thought_signature"))
                .and_then(serde_json::Value::as_str)
        })
        .next_back()
        .map(ToString::to_string)
}

fn merge_gemini_tool_call_snapshots(
    snapshots: &mut Vec<GeminiToolCallMeta>,
    incoming: Vec<GeminiToolCallMeta>,
) {
    for (position, mut tool_call) in incoming.into_iter().enumerate() {
        let existing_index = tool_call
            .id
            .as_deref()
            .and_then(|incoming_id| {
                snapshots
                    .iter()
                    .position(|existing| existing.id.as_deref() == Some(incoming_id))
            })
            .or_else(|| snapshots.get(position).map(|_| position));

        if let Some(index) = existing_index {
            if tool_call.thought_signature.is_none() {
                tool_call
                    .thought_signature
                    .clone_from(&snapshots[index].thought_signature);
            }
            snapshots[index] = tool_call;
        } else {
            snapshots.push(tool_call);
        }
    }
}

fn sse_block_data(block: &str) -> Option<String> {
    let data = block
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    (!data.is_empty()).then_some(data)
}

fn append_utf8_safe(buffer: &mut String, remainder: &mut Vec<u8>, new_bytes: &[u8]) {
    let combined;
    let input = if remainder.is_empty() {
        new_bytes
    } else if remainder.len() > 3 {
        buffer.push_str(&String::from_utf8_lossy(remainder));
        remainder.clear();
        new_bytes
    } else {
        combined = {
            let mut bytes = std::mem::take(remainder);
            bytes.extend_from_slice(new_bytes);
            bytes
        };
        combined.as_slice()
    };

    let mut pos = 0;
    loop {
        match std::str::from_utf8(&input[pos..]) {
            Ok(text) => {
                buffer.push_str(text);
                return;
            }
            Err(err) => {
                let valid_up_to = pos + err.valid_up_to();
                if valid_up_to > pos {
                    buffer.push_str(std::str::from_utf8(&input[pos..valid_up_to]).unwrap_or(""));
                }
                if let Some(invalid_len) = err.error_len() {
                    buffer.push('\u{FFFD}');
                    pos = valid_up_to + invalid_len;
                } else {
                    *remainder = input[valid_up_to..].to_vec();
                    return;
                }
            }
        }
    }
}

fn sse_block_event(block: &str) -> Option<String> {
    block.lines().find_map(|line| {
        line.strip_prefix("event:")
            .map(str::trim)
            .filter(|event| !event.is_empty())
            .map(str::to_string)
    })
}

fn encode_sse_event(event_name: &str, value: &serde_json::Value) -> Vec<u8> {
    format!("event: {event_name}\ndata: {value}\n\n").into_bytes()
}

pub async fn start_proxy(
    state: Arc<AppState>,
    settings: ProxySettings,
) -> Result<ProxyStatus, AppError> {
    validate_settings(&settings)?;
    let client = build_client(&settings)?;
    let addr = parse_proxy_listen_addr(&settings.host, settings.port)?;

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
    let listen_url = listen_url_for_client(actual_addr);

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
        gemini_shadow: rt.gemini_shadow.clone(),
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
    runtime.optimizer_enabled = saved.optimizer_enabled;
    runtime.optimizer_thinking = saved.optimizer_thinking;
    runtime.optimizer_cache_injection = saved.optimizer_cache_injection;
    runtime.optimizer_cache_ttl = saved.optimizer_cache_ttl;
    let takeover = (
        runtime.apps.claude.enabled,
        runtime.apps.codex.enabled,
        runtime.apps.gemini.enabled,
        runtime.apps.opencode.enabled,
    );
    runtime.apps = saved.apps;

    if include_takeover {
        runtime.live_takeover_active = saved.live_takeover_active;
    } else {
        runtime.apps.claude.enabled = takeover.0;
        runtime.apps.codex.enabled = takeover.1;
        runtime.apps.gemini.enabled = takeover.2;
        runtime.apps.opencode.enabled = takeover.3;
    }

    runtime
}

pub async fn status() -> ProxyStatus {
    status_with_state(None).await
}

async fn status_with_state(state: Option<&Arc<AppState>>) -> ProxyStatus {
    let rt = runtime();
    let stats = rt.stats.read().await.clone();
    let provider_health = provider_health_status(&rt.health).await;
    let guard = rt.handle.lock().await;
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
                provider_health,
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
            provider_health,
        },
    }
}

pub async fn status_for_state(state: &Arc<AppState>) -> ProxyStatus {
    status_with_state(Some(state)).await
}

pub async fn reset_provider_circuit(app: &AppType, provider_id: &str) {
    let rt = runtime();
    rt.health
        .write()
        .await
        .remove(&provider_health_key(app, provider_id));
}

async fn provider_health_status(
    health: &Arc<RwLock<HashMap<String, ProviderRuntimeHealth>>>,
) -> Vec<ProxyProviderHealth> {
    let guard = health.read().await;
    let mut items: Vec<_> = guard
        .iter()
        .filter_map(|(key, entry)| {
            let (app_type, provider_id) = key.split_once(':')?;
            Some(ProxyProviderHealth {
                app_type: app_type.to_string(),
                provider_id: provider_id.to_string(),
                state: provider_circuit_state_label(entry.state).to_string(),
                failure_count: entry.failure_count,
                recovery_success_count: entry.recovery_success_count,
                window_requests: entry.window_requests,
                window_failures: entry.window_failures,
                last_failure_seconds_ago: entry
                    .last_failure_at
                    .map(|value| value.elapsed().as_secs()),
                opened_seconds_ago: entry.opened_at.map(|value| value.elapsed().as_secs()),
            })
        })
        .collect();
    items.sort_by(|a, b| {
        a.app_type
            .cmp(&b.app_type)
            .then_with(|| a.provider_id.cmp(&b.provider_id))
    });
    items
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
    extract_session_id_from_value(&value)
}

fn extract_session_id_from_value(value: &serde_json::Value) -> Option<String> {
    json_string_at_any_path(
        value,
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
        apply_bedrock_optimizer, apply_copilot_header_plan, build_client,
        build_copilot_header_plan, effective_proxy_settings_for_app, extract_request_model,
        extract_request_session_id, inject_codex_oauth_headers, listen_url_for_client,
        maybe_rectify_anthropic_upstream_response, merge_runtime_settings, parse_proxy_listen_addr,
        parse_sse_events_from_bytes, prepare_upstream_request_body, proxy_error_status,
        read_limited_upstream_body, takeover_apps, test_settings, upstream_uri_for_provider,
        usage_app_type_for_provider, AppError, AppType, GeminiNativeSseConverter,
        OpenAIChatSseConverter, OpenAIResponsesSseConverter, ProxySettings,
        RectifierResponseDecision, PROXY_CLIENT_CONNECT_TIMEOUT_SECS,
        PROXY_CLIENT_POOL_MAX_IDLE_PER_HOST, PROXY_CLIENT_TCP_KEEPALIVE_SECS,
        PROXY_CLIENT_TIMEOUT_SECS,
    };
    use crate::{
        app_config::MultiAppConfig,
        database::CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID,
        provider::{Provider, ProviderAuthBinding, ProviderManager, ProviderMeta},
        store::AppState,
    };
    use axum::{
        body::Bytes,
        http::{header, StatusCode, Uri},
    };
    use serde_json::json;
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::time::Duration;

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

    fn bedrock_provider() -> Provider {
        Provider::with_id(
            "bedrock".to_string(),
            "Bedrock".to_string(),
            json!({ "env": { "CLAUDE_CODE_USE_BEDROCK": "1" } }),
            None,
        )
    }

    fn plain_provider() -> Provider {
        Provider::with_id(
            "plain".to_string(),
            "Plain".to_string(),
            json!({ "env": {} }),
            None,
        )
    }

    #[test]
    fn upstream_request_body_filters_private_params_and_preserves_schema_names() {
        let prepared = prepare_upstream_request_body(Bytes::from(
            json!({
                "z": 1,
                "model": "claude-sonnet-4-6",
                "_internal_id": "remove",
                "messages": [{
                    "role": "user",
                    "content": "hello",
                    "_session_token": "remove"
                }],
                "tools": [{
                    "name": "lookup",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "_id": { "type": "string", "_note": "remove" },
                            "normal": { "type": "string" }
                        },
                        "_schema_note": "remove"
                    }
                }]
            })
            .to_string(),
        ));
        let prepared: serde_json::Value = serde_json::from_slice(&prepared).expect("json");

        assert!(prepared.get("_internal_id").is_none());
        assert!(prepared["messages"][0].get("_session_token").is_none());
        assert!(prepared["tools"][0]["input_schema"]
            .get("_schema_note")
            .is_none());
        assert!(prepared["tools"][0]["input_schema"]["properties"]
            .get("_id")
            .is_some());
        assert!(prepared["tools"][0]["input_schema"]["properties"]["_id"]
            .get("_note")
            .is_none());
    }

    #[test]
    fn bedrock_optimizer_is_disabled_by_default() {
        let settings = ProxySettings::default();
        let body = Bytes::from(
            json!({
                "model": "anthropic.claude-sonnet-4-6-20250514-v1:0",
                "tools": [{ "name": "lookup" }],
                "messages": [{ "role": "user", "content": "hello" }]
            })
            .to_string(),
        );

        let optimized = apply_bedrock_optimizer(
            &settings,
            &AppType::Claude,
            &bedrock_provider(),
            &"/v1/messages".parse::<Uri>().unwrap(),
            body.clone(),
        );

        assert_eq!(optimized, body);
    }

    #[test]
    fn bedrock_optimizer_applies_only_to_bedrock_anthropic_requests() {
        let settings = ProxySettings {
            optimizer_enabled: true,
            ..ProxySettings::default()
        };
        let body = Bytes::from(
            json!({
                "model": "anthropic.claude-sonnet-4-6-20250514-v1:0",
                "tools": [{ "name": "lookup" }],
                "system": "system prompt",
                "messages": [
                    { "role": "assistant", "content": [{ "type": "text", "text": "previous" }] },
                    { "role": "user", "content": "hello" }
                ]
            })
            .to_string(),
        );

        let optimized = apply_bedrock_optimizer(
            &settings,
            &AppType::Claude,
            &bedrock_provider(),
            &"/v1/messages".parse::<Uri>().unwrap(),
            body,
        );
        let optimized: serde_json::Value = serde_json::from_slice(&optimized).expect("json");

        assert_eq!(optimized["thinking"]["type"], "adaptive");
        assert_eq!(optimized["output_config"]["effort"], "max");
        assert!(optimized["anthropic_beta"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "context-1m-2025-08-07"));
        assert!(optimized["tools"][0].get("cache_control").is_some());
        assert!(optimized["system"][0].get("cache_control").is_some());
        assert!(optimized["messages"][0]["content"][0]
            .get("cache_control")
            .is_some());
    }

    #[test]
    fn bedrock_optimizer_skips_non_bedrock_provider() {
        let settings = ProxySettings {
            optimizer_enabled: true,
            ..ProxySettings::default()
        };
        let body = Bytes::from(
            json!({
                "model": "anthropic.claude-sonnet-4-6-20250514-v1:0",
                "tools": [{ "name": "lookup" }],
                "messages": [{ "role": "user", "content": "hello" }]
            })
            .to_string(),
        );

        let optimized = apply_bedrock_optimizer(
            &settings,
            &AppType::Claude,
            &plain_provider(),
            &"/v1/messages".parse::<Uri>().unwrap(),
            body.clone(),
        );

        assert_eq!(optimized, body);
    }

    #[tokio::test]
    async fn anthropic_rectifier_retries_signature_error_and_strips_thinking() {
        let provider = Provider {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            settings_config: json!({}),
            website_url: None,
            category: None,
            created_at: None,
            sort_index: None,
            notes: None,
            meta: None,
        };
        let response = reqwest::Response::from(
            axum::http::Response::builder()
                .status(400)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Bytes::from_static(
                    br#"{"error":{"message":"Invalid signature in thinking block"}}"#,
                ))
                .expect("response"),
        );
        let body = Bytes::from(
            json!({
                "model": "claude-sonnet-4-6",
                "thinking": { "type": "enabled" },
                "messages": [{
                    "role": "assistant",
                    "content": [
                        { "type": "thinking", "thinking": "secret", "signature": "bad" },
                        { "type": "text", "text": "visible", "signature": "stray" },
                        { "type": "tool_use", "id": "toolu_1", "name": "lookup", "input": {} }
                    ]
                }]
            })
            .to_string(),
        );
        let mut signature_retried = false;
        let mut budget_retried = false;

        let decision = maybe_rectify_anthropic_upstream_response(
            response,
            &ProxySettings::default(),
            &AppType::Claude,
            &provider,
            &"/v1/messages".parse().expect("uri"),
            &body,
            Duration::from_secs(5),
            &mut signature_retried,
            &mut budget_retried,
        )
        .await
        .expect("rectifier decision");

        let RectifierResponseDecision::Retry(rectified) = decision else {
            panic!("expected retry");
        };
        let rectified: serde_json::Value =
            serde_json::from_slice(&rectified).expect("rectified json");
        assert!(signature_retried);
        assert!(!budget_retried);
        assert!(rectified.get("thinking").is_none());
        assert_eq!(
            rectified["messages"][0]["content"],
            json!([
                { "type": "text", "text": "visible" },
                { "type": "tool_use", "id": "toolu_1", "name": "lookup", "input": {} }
            ])
        );
    }

    #[tokio::test]
    async fn anthropic_rectifier_retries_budget_error_with_upstream_defaults() {
        let provider = Provider {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            settings_config: json!({}),
            website_url: None,
            category: None,
            created_at: None,
            sort_index: None,
            notes: None,
            meta: None,
        };
        let response = reqwest::Response::from(
            axum::http::Response::builder()
                .status(400)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Bytes::from_static(
                    br#"{"error":{"message":"thinking.budget_tokens: Input should be greater than or equal to 1024"}}"#,
                ))
                .expect("response"),
        );
        let body = Bytes::from(
            json!({
                "model": "claude-sonnet-4-6",
                "max_tokens": 1024,
                "thinking": { "type": "enabled", "budget_tokens": 512 },
                "messages": [{ "role": "user", "content": "hello" }]
            })
            .to_string(),
        );
        let mut signature_retried = false;
        let mut budget_retried = false;

        let decision = maybe_rectify_anthropic_upstream_response(
            response,
            &ProxySettings::default(),
            &AppType::Claude,
            &provider,
            &"/v1/messages".parse().expect("uri"),
            &body,
            Duration::from_secs(5),
            &mut signature_retried,
            &mut budget_retried,
        )
        .await
        .expect("rectifier decision");

        let RectifierResponseDecision::Retry(rectified) = decision else {
            panic!("expected retry");
        };
        let rectified: serde_json::Value =
            serde_json::from_slice(&rectified).expect("rectified json");
        assert!(!signature_retried);
        assert!(budget_retried);
        assert_eq!(rectified["thinking"]["type"], "enabled");
        assert_eq!(rectified["thinking"]["budget_tokens"], 32_000);
        assert_eq!(rectified["max_tokens"], 64_000);
    }

    #[test]
    fn proxy_listen_addr_accepts_bare_and_bracketed_ipv6() {
        assert_eq!(
            parse_proxy_listen_addr("::1", 3456).expect("bare ipv6"),
            "[::1]:3456".parse::<SocketAddr>().expect("socket addr")
        );
        assert_eq!(
            parse_proxy_listen_addr("[::1]", 3456).expect("bracketed ipv6"),
            "[::1]:3456".parse::<SocketAddr>().expect("socket addr")
        );
    }

    #[test]
    fn proxy_listen_url_uses_connectable_localhost_for_wildcards() {
        assert_eq!(
            listen_url_for_client("0.0.0.0:3456".parse().expect("ipv4 wildcard")),
            "http://127.0.0.1:3456"
        );
        assert_eq!(
            listen_url_for_client("[::]:3456".parse().expect("ipv6 wildcard")),
            "http://[::1]:3456"
        );
        assert_eq!(
            listen_url_for_client("[::1]:3456".parse().expect("ipv6 localhost")),
            "http://[::1]:3456"
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
    fn copilot_header_plan_marks_tool_result_turn_as_agent() {
        let provider = Provider {
            id: "github-copilot".to_string(),
            name: "GitHub Copilot".to_string(),
            settings_config: json!({}),
            website_url: None,
            category: None,
            created_at: None,
            sort_index: None,
            notes: None,
            meta: Some(ProviderMeta {
                provider_type: Some("github_copilot".to_string()),
                ..ProviderMeta::default()
            }),
        };
        let body = json!({
            "session_id": "session-1",
            "messages": [
                {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": "toolu_1",
                        "name": "Read",
                        "input": {}
                    }]
                },
                {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": "toolu_1",
                        "content": "ok"
                    }]
                }
            ]
        });

        let plan =
            build_copilot_header_plan(&provider, &body, true, Some("session-1")).expect("plan");
        let mut headers = reqwest::header::HeaderMap::new();
        apply_copilot_header_plan(&mut headers, &plan);

        assert_eq!(
            headers
                .get("x-initiator")
                .and_then(|value| value.to_str().ok()),
            Some("agent")
        );
        assert!(headers.get("x-request-id").is_some());
        assert_eq!(
            headers
                .get("x-agent-task-id")
                .and_then(|value| value.to_str().ok()),
            headers
                .get("x-request-id")
                .and_then(|value| value.to_str().ok())
        );
        assert!(headers.get("x-interaction-id").is_some());
    }

    #[test]
    fn copilot_header_plan_marks_subagent_interaction_type() {
        let provider = Provider {
            id: "github-copilot".to_string(),
            name: "GitHub Copilot".to_string(),
            settings_config: json!({}),
            website_url: None,
            category: None,
            created_at: None,
            sort_index: None,
            notes: None,
            meta: Some(ProviderMeta {
                provider_type: Some("github_copilot".to_string()),
                ..ProviderMeta::default()
            }),
        };
        let body = json!({
            "metadata": { "session_id": "session-1" },
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": "<system-reminder>{\"__SUBAGENT_MARKER__\":{\"session_id\":\"session-1\"}}</system-reminder>"
                }]
            }]
        });

        let plan =
            build_copilot_header_plan(&provider, &body, false, Some("session-1")).expect("plan");
        let mut headers = reqwest::header::HeaderMap::new();
        apply_copilot_header_plan(&mut headers, &plan);

        assert_eq!(
            headers
                .get("x-initiator")
                .and_then(|value| value.to_str().ok()),
            Some("agent")
        );
        assert_eq!(
            headers
                .get("x-interaction-type")
                .and_then(|value| value.to_str().ok()),
            Some("conversation-subagent")
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
            upstream_uri_for_provider(&AppType::ClaudeDesktop, &provider, &uri, None, false)
                .expect("chat uri");
        assert_eq!(
            chat_uri.path_and_query().map(|value| value.as_str()),
            Some("/v1/chat/completions?stream=true")
        );

        provider.meta.as_mut().expect("meta").api_format = Some("openai-responses".to_string());
        let responses_uri =
            upstream_uri_for_provider(&AppType::ClaudeDesktop, &provider, &uri, None, false)
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
            upstream_uri_for_provider(&AppType::ClaudeDesktop, &provider, &uri, None, false)
                .expect("uri");
        assert_eq!(
            upstream.path_and_query().map(|value| value.as_str()),
            Some("/v1/messages?stream=true")
        );
    }

    #[test]
    fn claude_desktop_gemini_native_routes_messages_to_generate_content() {
        let provider = Provider {
            id: "desktop-gemini".to_string(),
            name: "Desktop Gemini".to_string(),
            settings_config: json!({}),
            website_url: None,
            category: None,
            created_at: None,
            sort_index: None,
            notes: None,
            meta: Some(ProviderMeta {
                api_format: Some("gemini_native".to_string()),
                ..ProviderMeta::default()
            }),
        };
        let uri = "/v1/messages?stream=true&trace=1"
            .parse()
            .expect("valid uri");

        let upstream = upstream_uri_for_provider(
            &AppType::ClaudeDesktop,
            &provider,
            &uri,
            Some("models/gemini-2.5-pro"),
            true,
        )
        .expect("uri");

        assert_eq!(
            upstream.path_and_query().map(|value| value.as_str()),
            Some("/v1beta/models/gemini-2.5-pro:streamGenerateContent?trace=1&alt=sse")
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

    #[test]
    fn claude_desktop_gemini_native_uses_gemini_usage_parser() {
        let provider = Provider {
            id: "desktop-gemini".to_string(),
            name: "Desktop Gemini".to_string(),
            settings_config: json!({}),
            website_url: None,
            category: None,
            created_at: None,
            sort_index: None,
            notes: None,
            meta: Some(ProviderMeta {
                api_format: Some("gemini_native".to_string()),
                ..ProviderMeta::default()
            }),
        };

        assert_eq!(
            usage_app_type_for_provider(&AppType::ClaudeDesktop, &provider),
            "gemini"
        );
    }

    #[test]
    fn gemini_native_sse_converter_emits_anthropic_events_and_final_usage() {
        let mut converter = GeminiNativeSseConverter::default();
        let (first, first_events) = converter
            .push_bytes(&Bytes::from_static(br#"data: {"responseId":"resp-1","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"text":"Hel"}]}}]}"#))
            .expect("first chunk");
        assert!(first_events.len() == 0);
        assert!(first.is_empty());

        let (second, second_events) = converter
            .push_bytes(&Bytes::from_static(
                b"\n\ndata: {\"responseId\":\"resp-1\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"finishReason\":\"STOP\",\"content\":{\"parts\":[{\"text\":\"Hello\"},{\"functionCall\":{\"name\":\"lookup_price\",\"args\":{\"symbol\":\"AAPL\"}}}]}}],\"usageMetadata\":{\"promptTokenCount\":10,\"totalTokenCount\":14,\"cachedContentTokenCount\":3}}\n\n",
            ))
            .expect("second chunk");
        assert_eq!(second_events.len(), 2);

        let mut output = Vec::new();
        output.extend_from_slice(&first);
        output.extend_from_slice(&second);
        output.extend_from_slice(&converter.finish());
        let events = parse_sse_events_from_bytes(&Bytes::from(output));

        assert!(events.iter().any(
            |event| event.get("type").and_then(|value| value.as_str()) == Some("message_start")
        ));
        assert!(events.iter().any(|event| {
            event.get("type").and_then(|value| value.as_str()) == Some("content_block_delta")
                && event
                    .pointer("/delta/text")
                    .and_then(|value| value.as_str())
                    == Some("Hel")
        }));
        assert!(events.iter().any(|event| {
            event.get("type").and_then(|value| value.as_str()) == Some("content_block_delta")
                && event
                    .pointer("/delta/text")
                    .and_then(|value| value.as_str())
                    == Some("lo")
        }));
        assert!(events.iter().any(|event| {
            event
                .pointer("/content_block/type")
                .and_then(|value| value.as_str())
                == Some("tool_use")
                && event
                    .pointer("/content_block/id")
                    .and_then(|value| value.as_str())
                    == Some("gemini_synth_1")
        }));
        let message_delta = events
            .iter()
            .find(|event| {
                event.get("type").and_then(|value| value.as_str()) == Some("message_delta")
            })
            .expect("message_delta");
        assert_eq!(
            message_delta.pointer("/delta/stop_reason"),
            Some(&json!("tool_use"))
        );
        assert_eq!(
            message_delta.pointer("/usage/input_tokens"),
            Some(&json!(10))
        );
        assert_eq!(
            message_delta.pointer("/usage/output_tokens"),
            Some(&json!(4))
        );
        assert_eq!(
            message_delta.pointer("/usage/cache_read_input_tokens"),
            Some(&json!(3))
        );
    }

    #[test]
    fn openai_responses_sse_converter_emits_anthropic_events_and_final_usage() {
        let mut converter = OpenAIResponsesSseConverter::default();
        let input = concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-1\",\"model\":\"gpt-5.1-codex\",\"usage\":{\"input_tokens\":12,\"output_tokens\":0,\"input_tokens_details\":{\"cached_tokens\":4}}}}\n\n",
            "event: response.reasoning.delta\n",
            "data: {\"type\":\"response.reasoning.delta\",\"delta\":\"Need lookup.\"}\n\n",
            "event: response.reasoning.done\n",
            "data: {\"type\":\"response.reasoning.done\"}\n\n",
            "event: response.content_part.added\n",
            "data: {\"type\":\"response.content_part.added\",\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n",
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Checking.\"}\n\n",
            "event: response.output_text.done\n",
            "data: {\"type\":\"response.output_text.done\"}\n\n",
            "event: response.output_item.added\n",
            "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"fc_1\",\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"lookup_price\"}}\n\n",
            "event: response.function_call_arguments.delta\n",
            "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc_1\",\"delta\":\"{\\\"symbol\\\":\\\"AAPL\\\"}\"}\n\n",
            "event: response.function_call_arguments.done\n",
            "data: {\"type\":\"response.function_call_arguments.done\",\"item_id\":\"fc_1\"}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":12,\"output_tokens\":6,\"input_tokens_details\":{\"cached_tokens\":4},\"cache_creation_input_tokens\":2}}}\n\n"
        );

        let (converted, parsed) = converter
            .push_bytes(&Bytes::from(input.as_bytes().to_vec()))
            .expect("converted");
        assert!(converter.finish().is_empty());
        assert_eq!(parsed.len(), 10);

        let events = parse_sse_events_from_bytes(&converted);
        assert!(events.iter().any(|event| {
            event.get("type").and_then(|value| value.as_str()) == Some("message_start")
                && event
                    .pointer("/message/id")
                    .and_then(|value| value.as_str())
                    == Some("resp-1")
                && event
                    .pointer("/message/model")
                    .and_then(|value| value.as_str())
                    == Some("gpt-5.1-codex")
        }));
        assert!(events.iter().any(|event| {
            event
                .pointer("/content_block/type")
                .and_then(|value| value.as_str())
                == Some("thinking")
        }));
        assert!(events.iter().any(|event| {
            event
                .pointer("/delta/thinking")
                .and_then(|value| value.as_str())
                == Some("Need lookup.")
        }));
        assert!(events.iter().any(|event| {
            event
                .pointer("/delta/text")
                .and_then(|value| value.as_str())
                == Some("Checking.")
        }));
        assert!(events.iter().any(|event| {
            event
                .pointer("/content_block/type")
                .and_then(|value| value.as_str())
                == Some("tool_use")
                && event
                    .pointer("/content_block/id")
                    .and_then(|value| value.as_str())
                    == Some("call_1")
        }));
        assert!(events.iter().any(|event| {
            event
                .pointer("/delta/partial_json")
                .and_then(|value| value.as_str())
                == Some("{\"symbol\":\"AAPL\"}")
        }));
        let message_delta = events
            .iter()
            .find(|event| {
                event.get("type").and_then(|value| value.as_str()) == Some("message_delta")
            })
            .expect("message_delta");
        assert_eq!(
            message_delta.pointer("/delta/stop_reason"),
            Some(&json!("tool_use"))
        );
        assert_eq!(
            message_delta.pointer("/usage/input_tokens"),
            Some(&json!(12))
        );
        assert_eq!(
            message_delta.pointer("/usage/output_tokens"),
            Some(&json!(6))
        );
        assert_eq!(
            message_delta.pointer("/usage/cache_read_input_tokens"),
            Some(&json!(4))
        );
        assert_eq!(
            message_delta.pointer("/usage/cache_creation_input_tokens"),
            Some(&json!(2))
        );
    }

    #[test]
    fn openai_responses_sse_converter_sanitizes_read_tool_arguments() {
        let mut converter = OpenAIResponsesSseConverter::default();
        let input = concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-read\",\"model\":\"gpt-5.1-codex\"}}\n\n",
            "event: response.output_item.added\n",
            "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"fc_read\",\"type\":\"function_call\",\"call_id\":\"call_read\",\"name\":\"Read\"}}\n\n",
            "event: response.function_call_arguments.delta\n",
            "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc_read\",\"delta\":\"{\\\"file_path\\\":\\\"README.md\\\",\\\"pages\\\":\\\"\\\"}\"}\n\n",
            "event: response.function_call_arguments.done\n",
            "data: {\"type\":\"response.function_call_arguments.done\",\"item_id\":\"fc_read\"}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n"
        );

        let (converted, _) = converter
            .push_bytes(&Bytes::from(input.as_bytes().to_vec()))
            .expect("converted");
        let output = String::from_utf8(converted.to_vec()).expect("utf8");

        assert!(output.contains("\"name\":\"Read\""));
        assert!(output.contains("\"partial_json\":\"{\\\"file_path\\\":\\\"README.md\\\"}\""));
        assert!(!output.contains("\\\"pages\\\":\\\"\\\""));
    }

    #[test]
    fn openai_chat_sse_converter_emits_text_reasoning_and_final_usage() {
        let mut converter = OpenAIChatSseConverter::default();
        let input = concat!(
            "data: {\"id\":\"chatcmpl-1\",\"model\":\"gpt-4.1\",\"choices\":[{\"delta\":{\"reasoning\":\"Think.\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"model\":\"gpt-4.1\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"model\":\"gpt-4.1\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":8,\"completion_tokens\":3,\"prompt_tokens_details\":{\"cached_tokens\":2}}}\n\n",
            "data: [DONE]\n\n"
        );

        let (converted, parsed) = converter
            .push_bytes(&Bytes::from(input.as_bytes().to_vec()))
            .expect("converted");
        assert!(converter.finish().is_empty());
        assert_eq!(parsed.len(), 3);
        let events = parse_sse_events_from_bytes(&converted);

        assert!(events.iter().any(|event| {
            event.get("type").and_then(|value| value.as_str()) == Some("message_start")
                && event
                    .pointer("/message/id")
                    .and_then(|value| value.as_str())
                    == Some("chatcmpl-1")
        }));
        assert!(events.iter().any(|event| {
            event
                .pointer("/content_block/type")
                .and_then(|value| value.as_str())
                == Some("thinking")
        }));
        assert!(events.iter().any(|event| {
            event
                .pointer("/delta/thinking")
                .and_then(|value| value.as_str())
                == Some("Think.")
        }));
        assert!(events.iter().any(|event| {
            event
                .pointer("/delta/text")
                .and_then(|value| value.as_str())
                == Some("Hello")
        }));
        let message_delta = events
            .iter()
            .find(|event| {
                event.get("type").and_then(|value| value.as_str()) == Some("message_delta")
            })
            .expect("message_delta");
        assert_eq!(
            message_delta.pointer("/delta/stop_reason"),
            Some(&json!("end_turn"))
        );
        assert_eq!(
            message_delta.pointer("/usage/input_tokens"),
            Some(&json!(8))
        );
        assert_eq!(
            message_delta.pointer("/usage/output_tokens"),
            Some(&json!(3))
        );
        assert_eq!(
            message_delta.pointer("/usage/cache_read_input_tokens"),
            Some(&json!(2))
        );
        assert_eq!(
            events.last().and_then(|event| event.get("type")),
            Some(&json!("message_stop"))
        );
    }

    #[test]
    fn openai_chat_sse_converter_delays_tool_start_until_id_and_name() {
        let mut converter = OpenAIChatSseConverter::default();
        let input = concat!(
            "data: {\"id\":\"chatcmpl-2\",\"model\":\"gpt-4.1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"a\\\":\"}}]}}]}\n\n",
            "data: {\"id\":\"chatcmpl-2\",\"model\":\"gpt-4.1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"lookup\"}}]}}]}\n\n",
            "data: {\"id\":\"chatcmpl-2\",\"model\":\"gpt-4.1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"1}\"}}]}}]}\n\n",
            "data: {\"id\":\"chatcmpl-2\",\"model\":\"gpt-4.1\",\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"cache_creation_input_tokens\":1}}\n\n",
            "data: [DONE]\n\n"
        );

        let (converted, _) = converter
            .push_bytes(&Bytes::from(input.as_bytes().to_vec()))
            .expect("converted");
        let events = parse_sse_events_from_bytes(&converted);
        let tool_starts: Vec<_> = events
            .iter()
            .filter(|event| {
                event
                    .pointer("/content_block/type")
                    .and_then(|value| value.as_str())
                    == Some("tool_use")
            })
            .collect();
        assert_eq!(tool_starts.len(), 1);
        assert_eq!(
            tool_starts[0].pointer("/content_block/id"),
            Some(&json!("call_1"))
        );
        assert_eq!(
            tool_starts[0].pointer("/content_block/name"),
            Some(&json!("lookup"))
        );
        let deltas: Vec<_> = events
            .iter()
            .filter_map(|event| {
                event
                    .pointer("/delta/partial_json")
                    .and_then(|value| value.as_str())
            })
            .collect();
        assert_eq!(deltas, vec!["{\"a\":", "1}"]);
        let message_delta = events
            .iter()
            .find(|event| {
                event.get("type").and_then(|value| value.as_str()) == Some("message_delta")
            })
            .expect("message_delta");
        assert_eq!(
            message_delta.pointer("/delta/stop_reason"),
            Some(&json!("tool_use"))
        );
        assert_eq!(
            message_delta.pointer("/usage/input_tokens"),
            Some(&json!(10))
        );
        assert_eq!(
            message_delta.pointer("/usage/output_tokens"),
            Some(&json!(5))
        );
        assert_eq!(
            message_delta.pointer("/usage/cache_creation_input_tokens"),
            Some(&json!(1))
        );
    }

    #[test]
    fn openai_chat_sse_converter_handles_utf8_split_across_chunks() {
        let mut converter = OpenAIChatSseConverter::default();
        let input = concat!(
            "data: {\"id\":\"chatcmpl-utf8\",\"model\":\"gpt-4.1\",\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-utf8\",\"model\":\"gpt-4.1\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2}}\n\n",
            "data: [DONE]\n\n"
        );
        let (left, right) = split_inside_utf8_char(input, "你");

        let (first, first_events) = converter
            .push_bytes(&Bytes::from(left.to_vec()))
            .expect("first chunk");
        let (second, second_events) = converter
            .push_bytes(&Bytes::from(right.to_vec()))
            .expect("second chunk");

        let mut output = Vec::new();
        output.extend_from_slice(&first);
        output.extend_from_slice(&second);
        let events = parse_sse_events_from_bytes(&Bytes::from(output));

        assert_eq!(first_events.len(), 0);
        assert_eq!(second_events.len(), 2);
        assert!(events.iter().any(|event| {
            event
                .pointer("/delta/text")
                .and_then(|value| value.as_str())
                == Some("你好")
        }));
        assert!(!String::from_utf8_lossy(&second).contains('\u{FFFD}'));
    }

    #[test]
    fn openai_responses_sse_converter_handles_utf8_split_across_chunks() {
        let mut converter = OpenAIResponsesSseConverter::default();
        let input = concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-utf8\",\"model\":\"gpt-5.1-codex\"}}\n\n",
            "event: response.content_part.added\n",
            "data: {\"type\":\"response.content_part.added\",\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n",
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"你好\"}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":5,\"output_tokens\":2}}}\n\n"
        );
        let (left, right) = split_inside_utf8_char(input, "你");

        let (first, _) = converter
            .push_bytes(&Bytes::from(left.to_vec()))
            .expect("first chunk");
        let (second, _) = converter
            .push_bytes(&Bytes::from(right.to_vec()))
            .expect("second chunk");

        let mut output = Vec::new();
        output.extend_from_slice(&first);
        output.extend_from_slice(&second);
        let events = parse_sse_events_from_bytes(&Bytes::from(output));

        assert!(events.iter().any(|event| {
            event
                .pointer("/delta/text")
                .and_then(|value| value.as_str())
                == Some("你好")
        }));
        assert!(!String::from_utf8_lossy(&second).contains('\u{FFFD}'));
    }

    #[test]
    fn gemini_native_sse_converter_handles_utf8_split_across_chunks() {
        let mut converter = GeminiNativeSseConverter::default();
        let input = concat!(
            "data: {\"responseId\":\"gemini-utf8\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"finishReason\":\"STOP\",\"content\":{\"parts\":[{\"text\":\"你好\"}]}}],\"usageMetadata\":{\"promptTokenCount\":5,\"totalTokenCount\":7}}\n\n"
        );
        let (left, right) = split_inside_utf8_char(input, "你");

        let (first, first_events) = converter
            .push_bytes(&Bytes::from(left.to_vec()))
            .expect("first chunk");
        let (second, second_events) = converter
            .push_bytes(&Bytes::from(right.to_vec()))
            .expect("second chunk");
        let final_bytes = converter.finish();

        let mut output = Vec::new();
        output.extend_from_slice(&first);
        output.extend_from_slice(&second);
        output.extend_from_slice(&final_bytes);
        let events = parse_sse_events_from_bytes(&Bytes::from(output));

        assert_eq!(first_events.len(), 0);
        assert_eq!(second_events.len(), 1);
        assert!(events.iter().any(|event| {
            event
                .pointer("/delta/text")
                .and_then(|value| value.as_str())
                == Some("你好")
        }));
        assert!(!String::from_utf8_lossy(&second).contains('\u{FFFD}'));
    }

    fn split_inside_utf8_char<'a>(input: &'a str, needle: &str) -> (&'a [u8], &'a [u8]) {
        let bytes = input.as_bytes();
        let needle = needle.as_bytes();
        let start = bytes
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("needle");
        let split = start + 1;
        (&bytes[..split], &bytes[split..])
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
    fn proxy_client_runtime_defaults_match_upstream_reliability_profile() {
        assert_eq!(PROXY_CLIENT_TIMEOUT_SECS, 600);
        assert_eq!(PROXY_CLIENT_CONNECT_TIMEOUT_SECS, 30);
        assert_eq!(PROXY_CLIENT_POOL_MAX_IDLE_PER_HOST, 10);
        assert_eq!(PROXY_CLIENT_TCP_KEEPALIVE_SECS, 60);

        let settings = ProxySettings::default();
        build_client(&settings).expect("default proxy client builds");
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
        saved.apps.claude.streaming_idle_timeout = 44;
        saved.apps.claude.circuit_min_requests = 17;

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
        assert_eq!(merged.apps.claude.streaming_idle_timeout, 44);
        assert_eq!(merged.apps.claude.circuit_min_requests, 17);
    }

    #[test]
    fn effective_proxy_settings_use_selected_app_timeouts_and_circuit_values() {
        let mut settings = ProxySettings::default();
        settings.streaming_first_byte_timeout = 99;
        settings.apps.codex.streaming_first_byte_timeout = 21;
        settings.apps.codex.streaming_idle_timeout = 45;
        settings.apps.codex.non_streaming_timeout = 321;
        settings.apps.codex.circuit_failure_threshold = 8;
        settings.apps.codex.circuit_recovery_threshold = 4;
        settings.apps.codex.circuit_recovery_wait_seconds = 76;
        settings.apps.codex.circuit_error_rate_threshold = 64.0;

        let effective = effective_proxy_settings_for_app(&settings, &AppType::Codex);

        assert_eq!(effective.streaming_first_byte_timeout, 21);
        assert_eq!(effective.streaming_idle_timeout, 45);
        assert_eq!(effective.non_streaming_timeout, 321);
        assert_eq!(effective.circuit_failure_threshold, 8);
        assert_eq!(effective.circuit_recovery_threshold, 4);
        assert_eq!(effective.circuit_recovery_wait_seconds, 76);
        assert_eq!(effective.circuit_error_rate_threshold, 64.0);
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
