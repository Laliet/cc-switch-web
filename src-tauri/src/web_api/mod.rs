use std::{
    env, fs,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::{Path as StdPath, PathBuf},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Extension, Path},
    http::{
        header::{
            self, ACCEPT, AUTHORIZATION, CONTENT_TYPE, STRICT_TRANSPORT_SECURITY, WWW_AUTHENTICATE,
        },
        HeaderMap, HeaderValue, Method, Request, StatusCode,
    },
    middleware,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use base64::Engine;
use mime_guess::mime;
use rust_embed::RustEmbed;
use tokio::sync::Mutex;
use tower::limit::GlobalConcurrencyLimitLayer;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    validate_request::ValidateRequestHeaderLayer,
};
use url::Url;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::{
    config::{atomic_write, get_home_dir},
    error::AppError,
    store::AppState,
};

pub mod handlers;
pub mod routes;

/// Shared application state for the web server.
pub type SharedState = Arc<AppState>;

#[derive(Debug, Clone)]
pub struct WebAuthCredentials {
    pub username: String,
    pub password: String,
}

impl Default for WebAuthCredentials {
    fn default() -> Self {
        Self {
            username: DEFAULT_WEB_USERNAME.to_string(),
            password: String::new(),
        }
    }
}

pub type SharedWebAuth = Arc<RwLock<WebAuthCredentials>>;

#[derive(RustEmbed)]
#[folder = "../dist-web"]
struct WebAssets;

#[derive(Clone)]
struct WebTokens {
    csrf_token: String,
}

const DEFAULT_API_PREFIX: &str = "/api";
const DEFAULT_WEB_BODY_LIMIT_BYTES: usize = 2_097_152;
const DEFAULT_WEB_GLOBAL_CONCURRENCY: usize = 32;
const DEFAULT_WEB_USERNAME: &str = "admin";
const DEFAULT_WEB_PASSWORD_LEN: usize = 24;

/// Serve embedded static assets with index.html fallback for SPA routes.
async fn serve_static(
    path: Option<Path<String>>,
    headers: HeaderMap,
    tokens: Arc<WebTokens>,
    api_base: Arc<String>,
) -> impl IntoResponse {
    let requested_path = path.map(|Path(p)| p).unwrap_or_default();
    let requested_path = requested_path.trim_start_matches('/');
    let target_path = if requested_path.is_empty() {
        "index.html"
    } else {
        requested_path
    };

    // Try the requested file first; fall back to index.html for SPA routes.
    let (asset, served_path) = match WebAssets::get(target_path) {
        Some(content) => (content, target_path),
        None => {
            let has_extension = StdPath::new(target_path)
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| !ext.is_empty())
                .unwrap_or(false);
            let accepts_html = headers
                .get(ACCEPT)
                .and_then(|v| v.to_str().ok())
                .map(|value| value.to_ascii_lowercase().contains("text/html"))
                .unwrap_or(false);
            if !has_extension || accepts_html {
                match WebAssets::get("index.html") {
                    Some(content) => (content, "index.html"),
                    None => return StatusCode::NOT_FOUND.into_response(),
                }
            } else {
                return StatusCode::NOT_FOUND.into_response();
            }
        }
    };

    let mime = mime_guess::from_path(served_path).first_or(mime::APPLICATION_OCTET_STREAM);
    let mut content = asset.data.into_owned();

    if served_path == "index.html" {
        if let Ok(mut html) = String::from_utf8(content.clone()) {
            let csrf_token_json = serde_json::to_string(&tokens.csrf_token)
                .unwrap_or_else(|_| "\"\"".to_string())
                .replace('<', "\\u003c");
            let api_base_json = serde_json::to_string(api_base.as_str())
                .unwrap_or_else(|_| "\"/api\"".to_string())
                .replace('<', "\\u003c");
            let injection = format!(
                r#"<script>
window.__CC_SWITCH_API_BASE__ = {api_base};
window.__CC_SWITCH_TOKENS__ = {{
  csrfToken: {csrf}
}};
</script>"#,
                csrf = csrf_token_json,
                api_base = api_base_json
            );
            if let Some(pos) = html.find("</head>") {
                html.insert_str(pos, &injection);
            } else {
                html.push_str(&injection);
            }
            content = html.into_bytes();
        }
    }

    let body = Body::from(content);

    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );

    response
}

fn cors_layer() -> Option<CorsLayer> {
    // Production-safe CORS defaults. Enable explicitly via env when cross-origin access is needed.
    let allow_origins = env::var("CORS_ALLOW_ORIGINS").ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let allow_lan = env_truthy("ALLOW_LAN_CORS") || env_truthy("CC_SWITCH_LAN_CORS");
    let allow_credentials = env_truthy("CORS_ALLOW_CREDENTIALS");

    let mut layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::HEAD,
            Method::POST,
            Method::PUT,
            Method::DELETE,
        ])
        .allow_headers([
            ACCEPT,
            AUTHORIZATION,
            CONTENT_TYPE,
            header::HeaderName::from_static("x-csrf-token"),
        ]);

    let origins = match allow_origins.as_deref() {
        Some("*") => {
            // 显式禁止生产中的通配符，防止意外放开
            log::warn!("CORS_ALLOW_ORIGINS='*' 已被忽略，请使用逗号分隔的白名单");
            Vec::new()
        }
        Some(list) => list
            .split(',')
            .filter_map(|entry| {
                let trimmed = entry.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    HeaderValue::from_str(trimmed).ok()
                }
            })
            .collect(),
        None => Vec::new(),
    };

    if origins.is_empty() && !allow_lan {
        // No CORS allow-list provided -> rely on same-origin; do not loosen automatically.
        return None;
    }

    layer = layer.allow_origin(AllowOrigin::predicate(move |origin, _| {
        let is_listed = origins.iter().any(|allowed| allowed == origin);
        is_listed || (allow_lan && is_private_origin(origin))
    }));

    if allow_lan {
        layer = layer.allow_private_network(true);
    }

    if allow_credentials {
        layer = layer.allow_credentials(true);
    }

    Some(layer)
}

fn env_truthy(name: &str) -> bool {
    env::var(name).is_ok_and(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
}

fn is_private_origin(origin: &HeaderValue) -> bool {
    let Ok(origin_str) = origin.to_str() else {
        return false;
    };
    let Ok(url) = Url::parse(origin_str) else {
        return false;
    };
    match url.scheme() {
        "http" | "https" => {}
        _ => return false,
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let Ok(ip) = host.parse::<IpAddr>() else {
        return false;
    };
    is_private_ip(ip)
}

fn ipv4_is_private(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 10 || (a == 172 && (b & 0xf0) == 16) || (a == 192 && b == 168)
}

fn ipv4_is_loopback(ip: Ipv4Addr) -> bool {
    ip.octets()[0] == 127
}

fn ipv4_is_link_local(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 169 && b == 254
}

fn ipv6_is_loopback(ip: Ipv6Addr) -> bool {
    ip.segments() == [0, 0, 0, 0, 0, 0, 0, 1]
}

fn ipv6_is_unique_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn ipv6_is_link_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => ipv4_is_private(v4) || ipv4_is_loopback(v4) || ipv4_is_link_local(v4),
        IpAddr::V6(v6) => {
            ipv6_is_unique_local(v6) || ipv6_is_loopback(v6) || ipv6_is_link_local(v6)
        }
    }
}

fn normalize_api_prefix(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains("://") {
        log::warn!("WEB_API_PREFIX expects a path like /api, got {}", trimmed);
        return None;
    }

    let mut prefix = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };

    while prefix.ends_with('/') {
        prefix.pop();
    }

    if prefix.is_empty() || prefix == "/" {
        None
    } else {
        Some(prefix)
    }
}

fn web_api_prefix() -> String {
    let configured = env::var("WEB_API_PREFIX")
        .ok()
        .or_else(|| env::var("API_PREFIX").ok())
        .and_then(|value| normalize_api_prefix(&value));

    configured.unwrap_or_else(|| DEFAULT_API_PREFIX.to_string())
}

fn parse_env_usize(name: &str) -> Option<usize> {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse().ok())
}

fn parse_env_u64(name: &str) -> Option<u64> {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse().ok())
}

struct RateLimitState {
    window_start: Instant,
    count: u64,
}

async fn rate_limit_middleware(
    state: Arc<Mutex<RateLimitState>>,
    max: u64,
    window: Duration,
    req: Request<Body>,
    next: middleware::Next,
) -> Response {
    let mut guard = state.lock().await;
    if guard.window_start.elapsed() >= window {
        guard.window_start = Instant::now();
        guard.count = 0;
    }
    if guard.count >= max {
        let body = serde_json::json!({
            "error": "Rate limit exceeded.",
            "code": "RATE_LIMITED"
        });
        return Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap_or_else(|_| Response::new(Body::empty()));
    }
    guard.count += 1;
    drop(guard);

    next.run(req).await
}

pub fn web_password_path() -> Option<PathBuf> {
    get_home_dir().map(|home| home.join(".cc-switch").join("web_password"))
}

pub fn web_username_path() -> Option<PathBuf> {
    get_home_dir().map(|home| home.join(".cc-switch").join("web_username"))
}

pub fn load_or_generate_web_password() -> Result<(String, PathBuf), AppError> {
    let path = web_password_path().ok_or_else(|| {
        AppError::Config("Unable to locate home directory for web password".into())
    })?;

    if let Ok(content) = fs::read_to_string(&path) {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            if let Err(err) = enforce_permissions(&path) {
                log::warn!("Failed to enforce web password permissions: {}", err);
            }
            return Ok((trimmed.to_string(), path));
        }
    }

    let password = generate_password(DEFAULT_WEB_PASSWORD_LEN);
    let persisted = persist_web_password(&password)?;
    Ok((password, persisted))
}

pub fn persist_web_password(password: &str) -> Result<PathBuf, AppError> {
    let path = web_password_path().ok_or_else(|| {
        AppError::Config("Unable to locate home directory for web password".into())
    })?;
    atomic_write(&path, password.as_bytes())?;
    enforce_permissions(&path).map_err(|e| AppError::io(&path, e))?;
    Ok(path)
}

pub fn persist_web_username(username: &str) -> Result<PathBuf, AppError> {
    let path = web_username_path().ok_or_else(|| {
        AppError::Config("Unable to locate home directory for web username".into())
    })?;
    atomic_write(&path, username.as_bytes())?;
    enforce_permissions(&path).map_err(|e| AppError::io(&path, e))?;
    Ok(path)
}

pub fn persist_web_credentials(username: &str, password: &str) -> Result<(), AppError> {
    let username_path = web_username_path().ok_or_else(|| {
        AppError::Config("Unable to locate home directory for web username".into())
    })?;
    let password_path = web_password_path().ok_or_else(|| {
        AppError::Config("Unable to locate home directory for web password".into())
    })?;

    let previous_username = fs::read_to_string(&username_path)
        .ok()
        .map(|content| content.trim().to_string())
        .filter(|value| !value.is_empty());
    let previous_password = fs::read_to_string(&password_path)
        .ok()
        .map(|content| content.trim().to_string())
        .filter(|value| !value.is_empty());

    persist_web_username(username)?;

    if let Err(err) = persist_web_password(password) {
        match previous_username {
            Some(ref original_username) => {
                let _ = persist_web_username(original_username);
            }
            None if username_path.exists() => {
                let _ = fs::remove_file(&username_path);
            }
            None => {}
        }

        match previous_password {
            Some(ref original_password) => {
                let _ = persist_web_password(original_password);
            }
            None if password_path.exists() => {
                let _ = fs::remove_file(&password_path);
            }
            None => {}
        }

        return Err(err);
    }

    Ok(())
}

pub fn load_web_username() -> String {
    if let Some(path) = web_username_path() {
        if let Ok(content) = fs::read_to_string(&path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                if let Err(err) = enforce_permissions(&path) {
                    log::warn!("Failed to enforce web username permissions: {}", err);
                }
                return trimmed.to_string();
            }
        }
    }
    DEFAULT_WEB_USERNAME.to_string()
}

pub fn load_or_generate_web_credentials() -> Result<(SharedWebAuth, PathBuf), AppError> {
    let (password, password_path) = load_or_generate_web_password()?;
    let username = load_web_username();
    Ok((build_shared_web_auth(username, password), password_path))
}

pub fn build_shared_web_auth(username: String, password: String) -> SharedWebAuth {
    Arc::new(RwLock::new(WebAuthCredentials { username, password }))
}

/// Construct the axum router with all API routes and middleware.
pub fn create_router(state: SharedState, password: String) -> Router {
    create_router_with_credentials(state, load_web_username(), password)
}

/// Construct the axum router with all API routes and middleware.
pub fn create_router_with_credentials(
    state: SharedState,
    username: String,
    password: String,
) -> Router {
    let auth_state = build_shared_web_auth(username, password);
    create_router_with_auth_state(state, auth_state)
}

pub fn create_router_with_auth_state(state: SharedState, auth_state: SharedWebAuth) -> Router {
    let tokens = Arc::new(load_or_generate_tokens());
    let csrf_token = Some(Arc::new(tokens.csrf_token.clone()));
    let api_prefix = web_api_prefix();
    let api_prefix_arc = Arc::new(api_prefix.clone());

    let hsts_enabled = env::var("ENABLE_HSTS")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(true);

    let auth_validator = AuthValidator::new(auth_state.clone(), Some(tokens.csrf_token.clone()));

    let body_limit = parse_env_usize("WEB_MAX_BODY_BYTES").unwrap_or(DEFAULT_WEB_BODY_LIMIT_BYTES);
    let global_concurrency =
        parse_env_usize("WEB_GLOBAL_CONCURRENCY").unwrap_or(DEFAULT_WEB_GLOBAL_CONCURRENCY);
    let rate_limit_num = parse_env_u64("WEB_RATE_LIMIT_NUM").filter(|value| *value > 0);
    let rate_limit_window = parse_env_u64("WEB_RATE_LIMIT_WINDOW_SECS").filter(|value| *value > 0);

    let mut router = routes::create_router(state)
        .fallback(api_not_found)
        .layer(Extension(csrf_token))
        .layer(Extension(auth_state))
        .layer(ValidateRequestHeaderLayer::custom(auth_validator.clone()));

    if body_limit > 0 {
        router = router.layer(DefaultBodyLimit::max(body_limit));
    }

    // Only apply CORS when a valid allow-list is configured; default to same-origin.
    let router = if let Some(cors) = cors_layer() {
        router.layer(cors)
    } else {
        router
    };

    let static_router = Router::new()
        .route(
            "/",
            get({
                let tokens = tokens.clone();
                let api_base = api_prefix_arc.clone();
                move |path, headers| serve_static(path, headers, tokens.clone(), api_base.clone())
            }),
        )
        .route(
            "/*path",
            get({
                let tokens = tokens.clone();
                let api_base = api_prefix_arc.clone();
                move |path, headers| serve_static(path, headers, tokens.clone(), api_base.clone())
            }),
        )
        .layer(ValidateRequestHeaderLayer::custom(auth_validator));

    let mut root = Router::new()
        .nest(api_prefix.as_str(), router)
        .merge(static_router)
        .layer(middleware::from_fn(move |req, next| {
            add_hsts_header(hsts_enabled, req, next)
        }));

    if global_concurrency > 0 {
        root = root.layer(GlobalConcurrencyLimitLayer::new(global_concurrency));
    }
    if let (Some(num), Some(window)) = (rate_limit_num, rate_limit_window) {
        let state = Arc::new(Mutex::new(RateLimitState {
            window_start: Instant::now(),
            count: 0,
        }));
        let window = Duration::from_secs(window);
        root = root.layer(middleware::from_fn({
            let state = state.clone();
            move |req, next| rate_limit_middleware(state.clone(), num, window, req, next)
        }));
    }

    root
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

#[derive(Clone)]
struct AuthValidator {
    credentials: SharedWebAuth,
    csrf_token: Option<Arc<String>>,
}

impl AuthValidator {
    fn new(credentials: SharedWebAuth, csrf_token: Option<String>) -> Self {
        Self {
            credentials,
            csrf_token: csrf_token.map(Arc::new),
        }
    }

    fn is_authorized(&self, auth_value: &str) -> bool {
        if let Some(raw) = auth_value.strip_prefix("Basic ") {
            if let Ok(decoded) =
                base64::engine::general_purpose::STANDARD.decode(raw.trim().as_bytes())
            {
                if let Ok(s) = String::from_utf8(decoded) {
                    if let Some((user, pass)) = s.split_once(':') {
                        if let Ok(guard) = self.credentials.read() {
                            return user == guard.username.as_str()
                                && pass == guard.password.as_str();
                        }
                    }
                }
            }
        }

        false
    }

    fn unauthorized() -> Response {
        let body = serde_json::json!({
            "error": "Authentication required. Please provide valid credentials.",
            "code": "UNAUTHORIZED"
        });
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(
                WWW_AUTHENTICATE,
                HeaderValue::from_static(r#"Basic realm="cc-switch", charset="UTF-8""#),
            )
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap_or_else(|_| Response::new(Body::empty()))
    }

    fn forbidden_csrf() -> Response {
        let body = serde_json::json!({
            "error": "CSRF token invalid or missing. Please refresh the page and try again.",
            "code": "CSRF_VALIDATION_FAILED"
        });
        Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap_or_else(|_| Response::new(Body::empty()))
    }
}

impl tower_http::validate_request::ValidateRequest<Body> for AuthValidator {
    type ResponseBody = Body;

    fn validate(
        &mut self,
        request: &mut Request<Body>,
    ) -> Result<(), Response<Self::ResponseBody>> {
        let Some(auth_header) = request
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
        else {
            return Err(Self::unauthorized());
        };

        if !self.is_authorized(auth_header) {
            return Err(Self::unauthorized());
        }

        if let Some(csrf) = &self.csrf_token {
            if request.method() != Method::GET && request.method() != Method::HEAD {
                let token = request
                    .headers()
                    .get("x-csrf-token")
                    .and_then(|v| v.to_str().ok());
                if token != Some(csrf.as_str()) {
                    return Err(Self::forbidden_csrf());
                }
            }
        }

        Ok(())
    }
}

async fn add_hsts_header(
    hsts_enabled: bool,
    req: Request<Body>,
    next: middleware::Next,
) -> Response {
    let mut res = next.run(req).await;
    if hsts_enabled {
        let value = HeaderValue::from_static("max-age=31536000; includeSubDomains");
        res.headers_mut()
            .entry(STRICT_TRANSPORT_SECURITY)
            .or_insert(value);
    }

    res.headers_mut()
        .entry(header::HeaderName::from_static("x-frame-options"))
        .or_insert(HeaderValue::from_static("DENY"));
    res.headers_mut()
        .entry(header::HeaderName::from_static("x-content-type-options"))
        .or_insert(HeaderValue::from_static("nosniff"));
    res.headers_mut()
        .entry(header::HeaderName::from_static("referrer-policy"))
        .or_insert(HeaderValue::from_static("no-referrer"));

    res
}

fn token_store_path() -> Option<PathBuf> {
    get_home_dir().map(|home| home.join(".cc-switch").join("web_env"))
}

#[cfg(unix)]
fn enforce_permissions(path: &StdPath) -> std::io::Result<()> {
    fs::set_permissions(path, PermissionsExt::from_mode(0o600))
}

#[cfg(windows)]
fn enforce_permissions(path: &StdPath) -> std::io::Result<()> {
    use std::process::Command;

    let path_str = path.to_string_lossy();
    let output = Command::new("icacls")
        .args([&*path_str, "/inheritance:r", "/grant:r", "*S-1-3-4:F"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!("Failed to set Windows file permissions: {}", stderr);
    }

    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
fn enforce_permissions(_path: &StdPath) -> std::io::Result<()> {
    Ok(())
}

fn load_or_generate_tokens() -> WebTokens {
    let env_csrf = env::var("WEB_CSRF_TOKEN").ok().and_then(|val| {
        let trimmed = val.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    if let Some(csrf) = env_csrf {
        return WebTokens { csrf_token: csrf };
    }

    if let Some(path) = token_store_path() {
        if let Ok(content) = fs::read_to_string(&path) {
            let mut csrf = None;
            for line in content.lines() {
                if let Some(val) = line.strip_prefix("WEB_CSRF_TOKEN=") {
                    let trimmed = val.trim();
                    if !trimmed.is_empty() {
                        csrf = Some(trimmed.to_string());
                    }
                }
            }
            if let Some(csrf_val) = csrf {
                let _ = enforce_permissions(&path);
                return WebTokens {
                    csrf_token: csrf_val,
                };
            }
        }

        let csrf = generate_token(16);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let write_result = atomic_write(&path, format!("WEB_CSRF_TOKEN={csrf}\n").as_bytes());
        if write_result.is_ok() {
            if let Err(err) = enforce_permissions(&path) {
                log::warn!("Failed to enforce web token file permissions: {}", err);
            }
        }
        log::info!("WEB_CSRF_TOKEN 已生成并写入 {}", path.display());
        WebTokens { csrf_token: csrf }
    } else {
        WebTokens {
            csrf_token: generate_token(16),
        }
    }
}

fn generate_token(len: usize) -> String {
    use rand::{distributions::Alphanumeric, thread_rng, Rng};
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn generate_password(length: usize) -> String {
    use rand::{seq::SliceRandom, thread_rng};

    const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const DIGITS: &[u8] = b"0123456789";
    const ALL: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

    let mut rng = thread_rng();
    let mut chars = Vec::with_capacity(length);

    let mut push_from = |pool: &[u8]| {
        if let Some(ch) = pool.choose(&mut rng) {
            chars.push(*ch as char);
        }
    };

    push_from(LOWER);
    push_from(UPPER);
    push_from(DIGITS);

    while chars.len() < length {
        if let Some(ch) = ALL.choose(&mut rng) {
            chars.push(*ch as char);
        }
    }

    chars.shuffle(&mut rng);
    chars.into_iter().collect()
}
