use futures::StreamExt;
use reqwest::{redirect::Policy, Client};
use rquickjs::{Context, Function, Runtime};
use serde_json::Value;
use std::{
    collections::HashMap,
    env,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::{Duration, Instant},
};
use tokio::net::lookup_host;
use url::{Host, Url};

use crate::error::AppError;

const JS_MEMORY_LIMIT_BYTES: usize = 32 * 1024 * 1024; // 32MB 上限，防止脚本占用过大内存
const JS_MAX_STACK_SIZE: usize = 512 * 1024; // 512KB 调用栈

/// 执行用量查询脚本
pub async fn execute_usage_script(
    script_code: &str,
    api_key: &str,
    base_url: &str,
    timeout_secs: u64,
    access_token: Option<&str>,
    user_id: Option<&str>,
) -> Result<Value, AppError> {
    // 1. 替换变量
    let mut replaced = script_code
        .replace("{{apiKey}}", api_key)
        .replace("{{baseUrl}}", base_url);

    // 替换 accessToken 和 userId
    if let Some(token) = access_token {
        replaced = replaced.replace("{{accessToken}}", token);
    }
    if let Some(uid) = user_id {
        replaced = replaced.replace("{{userId}}", uid);
    }

    let script_source = replaced; // 复用同一份字符串，避免重复 clone

    // 2. 在独立作用域中提取 request 配置（确保 Runtime/Context 在 await 前释放）
    let request_config = {
        let runtime = build_sandboxed_runtime(timeout_secs)?;
        let context = Context::full(&runtime).map_err(|e| {
            AppError::localized(
                "usage_script.context_create_failed",
                format!("创建 JS 上下文失败: {e}"),
                format!("Failed to create JS context: {e}"),
            )
        })?;

        context.with(|ctx| {
            // 执行用户代码，获取配置对象
            let config: rquickjs::Object = ctx.eval(script_source.as_str()).map_err(|e| {
                AppError::localized(
                    "usage_script.config_parse_failed",
                    format!("解析配置失败: {e}"),
                    format!("Failed to parse config: {e}"),
                )
            })?;

            // 提取 request 配置
            let request: rquickjs::Object = config.get("request").map_err(|e| {
                AppError::localized(
                    "usage_script.request_missing",
                    format!("缺少 request 配置: {e}"),
                    format!("Missing request config: {e}"),
                )
            })?;

            // 将 request 转换为 JSON 字符串
            let request_json: String = ctx
                .json_stringify(request)
                .map_err(|e| {
                    AppError::localized(
                        "usage_script.request_serialize_failed",
                        format!("序列化 request 失败: {e}"),
                        format!("Failed to serialize request: {e}"),
                    )
                })?
                .ok_or_else(|| {
                    AppError::localized(
                        "usage_script.serialize_none",
                        "序列化返回 None",
                        "Serialization returned None",
                    )
                })?
                .get()
                .map_err(|e| {
                    AppError::localized(
                        "usage_script.get_string_failed",
                        format!("获取字符串失败: {e}"),
                        format!("Failed to get string: {e}"),
                    )
                })?;

            Ok::<_, AppError>(request_json)
        })?
    }; // Runtime 和 Context 在这里被 drop

    // 3. 解析 request 配置
    let request: RequestConfig = serde_json::from_str(&request_config).map_err(|e| {
        AppError::localized(
            "usage_script.request_format_invalid",
            format!("request 配置格式错误: {e}"),
            format!("Invalid request config format: {e}"),
        )
    })?;

    // 4. 发送 HTTP 请求
    let response_data = send_http_request(&request, timeout_secs).await?;

    // 5. 在独立作用域中执行 extractor（确保 Runtime/Context 在函数结束前释放）
    let result: Value = {
        let runtime = build_sandboxed_runtime(timeout_secs)?;
        let context = Context::full(&runtime).map_err(|e| {
            AppError::localized(
                "usage_script.context_create_failed",
                format!("创建 JS 上下文失败: {e}"),
                format!("Failed to create JS context: {e}"),
            )
        })?;

        context.with(|ctx| {
            // 重新 eval 获取配置对象
            let config: rquickjs::Object = ctx.eval(script_source.as_str()).map_err(|e| {
                AppError::localized(
                    "usage_script.config_reparse_failed",
                    format!("重新解析配置失败: {e}"),
                    format!("Failed to re-parse config: {e}"),
                )
            })?;

            // 提取 extractor 函数
            let extractor: Function = config.get("extractor").map_err(|e| {
                AppError::localized(
                    "usage_script.extractor_missing",
                    format!("缺少 extractor 函数: {e}"),
                    format!("Missing extractor function: {e}"),
                )
            })?;

            // 将响应数据转换为 JS 值
            let response_js: rquickjs::Value =
                ctx.json_parse(response_data.as_str()).map_err(|e| {
                    AppError::localized(
                        "usage_script.response_parse_failed",
                        format!("解析响应 JSON 失败: {e}"),
                        format!("Failed to parse response JSON: {e}"),
                    )
                })?;

            // 调用 extractor(response)
            let result_js: rquickjs::Value = extractor.call((response_js,)).map_err(|e| {
                AppError::localized(
                    "usage_script.extractor_exec_failed",
                    format!("执行 extractor 失败: {e}"),
                    format!("Failed to execute extractor: {e}"),
                )
            })?;

            // 转换为 JSON 字符串
            let result_json: String = ctx
                .json_stringify(result_js)
                .map_err(|e| {
                    AppError::localized(
                        "usage_script.result_serialize_failed",
                        format!("序列化结果失败: {e}"),
                        format!("Failed to serialize result: {e}"),
                    )
                })?
                .ok_or_else(|| {
                    AppError::localized(
                        "usage_script.serialize_none",
                        "序列化返回 None",
                        "Serialization returned None",
                    )
                })?
                .get()
                .map_err(|e| {
                    AppError::localized(
                        "usage_script.get_string_failed",
                        format!("获取字符串失败: {e}"),
                        format!("Failed to get string: {e}"),
                    )
                })?;

            // 解析为 serde_json::Value
            serde_json::from_str(&result_json).map_err(|e| {
                AppError::localized(
                    "usage_script.json_parse_failed",
                    format!("JSON 解析失败: {e}"),
                    format!("JSON parse failed: {e}"),
                )
            })
        })?
    }; // Runtime 和 Context 在这里被 drop

    // 6. 验证返回值格式
    validate_result(&result)?;

    Ok(result)
}

/// 请求配置结构
#[derive(Debug, serde::Deserialize)]
struct RequestConfig {
    url: String,
    method: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: Option<String>,
}

/// 发送 HTTP 请求
async fn send_http_request(config: &RequestConfig, timeout_secs: u64) -> Result<String, AppError> {
    let url = validate_request_url(&config.url).await?;

    let max_body_bytes = parse_env_usize("USAGE_SCRIPT_MAX_BODY_BYTES", 65_536);
    if let Some(body) = &config.body {
        if body.len() > max_body_bytes {
            return Err(AppError::localized(
                "usage_script.request_body_too_large",
                format!("请求体过大，最大允许 {max_body_bytes} 字节"),
                format!("Request body too large; max {max_body_bytes} bytes allowed"),
            ));
        }
    }

    let max_header_count = parse_env_usize("USAGE_SCRIPT_MAX_HEADER_COUNT", 32);
    if config.headers.len() > max_header_count {
        return Err(AppError::localized(
            "usage_script.header_count_exceeded",
            format!(
                "请求头数量超过限制: {} / {}",
                config.headers.len(),
                max_header_count
            ),
            format!(
                "Request header count exceeds limit: {} / {}",
                config.headers.len(),
                max_header_count
            ),
        ));
    }

    for name in config.headers.keys() {
        let normalized = name.trim().to_ascii_lowercase();
        if is_forbidden_header_name(&normalized) {
            return Err(AppError::localized(
                "usage_script.forbidden_header",
                format!("不允许设置请求头: {name}"),
                format!("Forbidden header name: {name}"),
            ));
        }
    }

    let allow_redirects = env_flag("USAGE_SCRIPT_ALLOW_REDIRECTS");
    let redirect_policy = if allow_redirects {
        Policy::limited(5)
    } else {
        Policy::none()
    };

    // 约束超时范围，防止异常配置导致长时间阻塞
    let timeout = timeout_secs.clamp(2, 30);
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout))
        .redirect(redirect_policy)
        .build()
        .map_err(|e| {
            AppError::localized(
                "usage_script.client_create_failed",
                format!("创建客户端失败: {e}"),
                format!("Failed to create client: {e}"),
            )
        })?;

    // 严格校验 HTTP 方法，非法值不回退为 GET
    let method: reqwest::Method = config.method.parse().map_err(|_| {
        AppError::localized(
            "usage_script.invalid_http_method",
            format!("不支持的 HTTP 方法: {}", config.method),
            format!("Unsupported HTTP method: {}", config.method),
        )
    })?;

    let mut req = client.request(method.clone(), url);

    // 添加请求头
    for (k, v) in &config.headers {
        req = req.header(k, v);
    }

    // 添加请求体
    if let Some(body) = &config.body {
        req = req.body(body.clone());
    }

    // 发送请求
    let resp = req.send().await.map_err(|e| {
        let err_str = e.to_string();
        let err_lower = err_str.to_lowercase();
        let invalid_url = err_lower.contains("invalid url") || err_lower.contains("relative url");

        let (msg_zh, msg_en) = if invalid_url {
            (
                "URL 格式无效，请检查脚本中的 request.url 配置",
                "Invalid URL format; please check request.url in your script",
            )
        } else if e.is_connect() {
            if err_lower.contains("connection refused") {
                (
                    "无法连接到目标服务器（连接被拒绝）",
                    "Unable to connect to the server (connection refused)",
                )
            } else if err_lower.contains("dns") {
                (
                    "DNS 解析失败，请检查域名是否正确",
                    "DNS resolution failed; please verify the domain name",
                )
            } else {
                ("无法连接到目标服务器", "Unable to connect to the server")
            }
        } else if e.is_timeout() {
            (
                "请求超时，目标服务器响应过慢",
                "Request timed out; the server took too long to respond",
            )
        } else if e.is_request() {
            (
                "请求构建失败，请检查 URL 和 HTTP 方法配置",
                "Request build failed; please check the URL and HTTP method",
            )
        } else {
            ("请求失败", "Request failed")
        };

        // reqwest errors may include the complete request URL (including query
        // credentials) or proxy details. Keep the public error category only.
        AppError::localized("usage_script.request_failed", msg_zh, msg_en)
    })?;

    let status = resp.status();
    let max_response_bytes = parse_env_usize("USAGE_SCRIPT_MAX_RESPONSE_BYTES", 1_048_576);
    let text = read_response_body(resp, max_response_bytes).await?;

    if !status.is_success() {
        let include_body = env_flag("USAGE_SCRIPT_INCLUDE_BODY");

        let preview = if include_body {
            let preview = text.chars().take(200).collect::<String>();
            let suffix = if text.chars().count() > 200 {
                "..."
            } else {
                ""
            };
            format!("{}{suffix}", redact_response_preview(&preview))
        } else {
            "<response body omitted>".to_string()
        };

        return Err(AppError::localized(
            "usage_script.http_error",
            format!("HTTP {status} : {preview}"),
            format!("HTTP {status} : {preview}"),
        ));
    }

    Ok(text)
}

fn redact_response_preview(value: &str) -> String {
    let mut json = match serde_json::from_str::<Value>(value) {
        Ok(json) => json,
        Err(_) => return "<response body redacted>".to_string(),
    };
    redact_json_credentials(&mut json);
    serde_json::to_string(&json).unwrap_or_else(|_| "<response body redacted>".to_string())
}

fn redact_json_credentials(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                let normalized = key
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_ascii_lowercase();
                if normalized.contains("authorization")
                    || normalized.contains("apikey")
                    || normalized.contains("accesstoken")
                    || normalized.contains("refreshtoken")
                    || normalized.contains("secret")
                    || normalized == "token"
                    || normalized == "password"
                {
                    *child = Value::String("[redacted]".to_string());
                } else {
                    redact_json_credentials(child);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_json_credentials(item);
            }
        }
        _ => {}
    }
}

async fn read_response_body(resp: reqwest::Response, max_bytes: usize) -> Result<String, AppError> {
    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();
    let mut total = 0usize;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| {
            AppError::localized(
                "usage_script.read_response_failed",
                "读取响应失败",
                "Failed to read response",
            )
        })?;
        total = total.saturating_add(chunk.len());
        if total > max_bytes {
            return Err(AppError::localized(
                "usage_script.response_too_large",
                format!("响应体过大，最大允许 {max_bytes} 字节"),
                format!("Response body too large; max {max_bytes} bytes allowed"),
            ));
        }
        buf.extend_from_slice(&chunk);
    }

    Ok(String::from_utf8_lossy(&buf).to_string())
}

fn parse_env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(default)
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

fn is_forbidden_header_name(name: &str) -> bool {
    matches!(
        name,
        "host"
            | "content-length"
            | "transfer-encoding"
            | "connection"
            | "proxy-authorization"
            | "proxy-authenticate"
            | "proxy-connection"
    )
}

#[derive(Clone, Copy)]
enum EgressPolicy {
    Strict,
    Trusted,
}

async fn validate_request_url(raw_url: &str) -> Result<Url, AppError> {
    let url = Url::parse(raw_url).map_err(|e| {
        AppError::localized(
            "usage_script.url_invalid",
            format!("URL 格式无效: {e}"),
            format!("Invalid URL format: {e}"),
        )
    })?;

    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(AppError::localized(
            "usage_script.url_scheme_not_allowed",
            format!("URL 仅支持 http/https，当前: {scheme}"),
            format!("Only http/https URLs are allowed; got {scheme}"),
        ));
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::localized(
            "usage_script.url_userinfo_not_allowed",
            "URL 不允许包含用户名或密码",
            "URL must not include username or password",
        ));
    }

    let host = url.host().ok_or_else(|| {
        AppError::localized(
            "usage_script.url_host_missing",
            "URL 缺少主机名",
            "URL is missing a host",
        )
    })?;
    let host_str = url.host_str().unwrap_or_default();

    let allowed_hosts = parse_allowed_hosts();
    if let Some(allowed) = allowed_hosts {
        let normalized_host = host_str.trim_end_matches('.').to_ascii_lowercase();
        if !allowed.iter().any(|entry| entry == &normalized_host) {
            return Err(AppError::localized(
                "usage_script.url_host_not_allowed",
                format!("主机名不在允许列表中: {host_str}"),
                format!("Host is not in allowlist: {host_str}"),
            ));
        }
    }

    let policy = parse_egress_policy();
    match host {
        Host::Ipv4(ip) => ensure_ip_allowed(IpAddr::V4(ip), policy)?,
        Host::Ipv6(ip) => ensure_ip_allowed(IpAddr::V6(ip), policy)?,
        Host::Domain(domain) => {
            let port = url.port_or_known_default().unwrap_or(80);
            let ips = resolve_host_ips(domain, port).await?;
            if ips.iter().any(|ip| is_disallowed_ip(*ip, policy)) {
                return Err(AppError::localized(
                    "usage_script.url_blocked",
                    "目标地址被策略阻止",
                    "Target address is blocked by policy",
                ));
            }
        }
    }

    Ok(url)
}

fn parse_egress_policy() -> EgressPolicy {
    let raw = env::var("USAGE_SCRIPT_EGRESS_POLICY").unwrap_or_default();
    match raw.trim().to_ascii_lowercase().as_str() {
        "strict" => EgressPolicy::Strict,
        "trusted" => EgressPolicy::Trusted,
        _ => EgressPolicy::Trusted,
    }
}

fn parse_allowed_hosts() -> Option<Vec<String>> {
    let value = env::var("USAGE_SCRIPT_ALLOWED_HOSTS").ok()?;
    let entries = value
        .split(',')
        .map(|entry| entry.trim().trim_end_matches('.').to_ascii_lowercase())
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();
    if entries.is_empty() {
        None
    } else {
        Some(entries)
    }
}

async fn resolve_host_ips(host: &str, port: u16) -> Result<Vec<IpAddr>, AppError> {
    let resolved = lookup_host((host, port)).await.map_err(|_| {
        AppError::localized(
            "usage_script.dns_lookup_failed",
            "DNS 解析失败",
            "DNS lookup failed",
        )
    })?;

    let ips = resolved.map(|addr| addr.ip()).collect::<Vec<_>>();
    if ips.is_empty() {
        return Err(AppError::localized(
            "usage_script.dns_lookup_failed",
            "DNS 解析失败: 未解析到地址",
            "DNS lookup failed: no addresses resolved",
        ));
    }

    Ok(ips)
}

fn ensure_ip_allowed(ip: IpAddr, policy: EgressPolicy) -> Result<(), AppError> {
    if is_disallowed_ip(ip, policy) {
        return Err(AppError::localized(
            "usage_script.url_blocked",
            "目标地址被策略阻止",
            "Target address is blocked by policy",
        ));
    }
    Ok(())
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

fn ipv4_is_multicast(ip: Ipv4Addr) -> bool {
    (ip.octets()[0] & 0xf0) == 224
}

fn ipv4_is_broadcast(ip: Ipv4Addr) -> bool {
    ip.octets() == [255, 255, 255, 255]
}

fn ipv4_is_unspecified(ip: Ipv4Addr) -> bool {
    ip.octets() == [0, 0, 0, 0]
}

fn ipv6_is_loopback(ip: Ipv6Addr) -> bool {
    ip.segments() == [0, 0, 0, 0, 0, 0, 0, 1]
}

fn ipv6_is_unspecified(ip: Ipv6Addr) -> bool {
    ip.segments() == [0, 0, 0, 0, 0, 0, 0, 0]
}

fn ipv6_is_multicast(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xff00) == 0xff00
}

fn ipv6_is_unique_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn ipv6_is_link_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn is_disallowed_ip(ip: IpAddr, policy: EgressPolicy) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let blocked = ipv4_is_link_local(v4)
                || ipv4_is_unspecified(v4)
                || ipv4_is_multicast(v4)
                || ipv4_is_broadcast(v4);
            if matches!(policy, EgressPolicy::Strict) {
                blocked || ipv4_is_loopback(v4) || ipv4_is_private(v4)
            } else {
                blocked
            }
        }
        IpAddr::V6(v6) => {
            let blocked =
                ipv6_is_link_local(v6) || ipv6_is_unspecified(v6) || ipv6_is_multicast(v6);
            if matches!(policy, EgressPolicy::Strict) {
                blocked || ipv6_is_loopback(v6) || ipv6_is_unique_local(v6)
            } else {
                blocked
            }
        }
    }
}

fn build_sandboxed_runtime(timeout_secs: u64) -> Result<Runtime, AppError> {
    let runtime = Runtime::new().map_err(|e| {
        AppError::localized(
            "usage_script.runtime_create_failed",
            format!("创建 JS 运行时失败: {e}"),
            format!("Failed to create JS runtime: {e}"),
        )
    })?;

    // 设置内存、栈与 CPU 限制，避免脚本拖垮宿主
    runtime.set_memory_limit(JS_MEMORY_LIMIT_BYTES);
    runtime.set_max_stack_size(JS_MAX_STACK_SIZE);

    let bounded = timeout_secs.clamp(2, 30);
    let max_ms = bounded * 1_000;
    let deadline = Instant::now() + Duration::from_millis(max_ms);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));

    Ok(runtime)
}

/// 验证脚本返回值（支持单对象或数组）
fn validate_result(result: &Value) -> Result<(), AppError> {
    // 如果是数组，验证每个元素
    if let Some(arr) = result.as_array() {
        if arr.is_empty() {
            return Err(AppError::localized(
                "usage_script.empty_array",
                "脚本返回的数组不能为空",
                "Script returned empty array",
            ));
        }
        for (idx, item) in arr.iter().enumerate() {
            validate_single_usage(item).map_err(|e| {
                AppError::localized(
                    "usage_script.array_validation_failed",
                    format!("数组索引[{idx}]验证失败: {e}"),
                    format!("Validation failed at index [{idx}]: {e}"),
                )
            })?;
        }
        return Ok(());
    }

    // 如果是单对象，直接验证（向后兼容）
    validate_single_usage(result)
}

/// 验证单个用量数据对象
fn validate_single_usage(result: &Value) -> Result<(), AppError> {
    let obj = result.as_object().ok_or_else(|| {
        AppError::localized(
            "usage_script.must_return_object",
            "脚本必须返回对象或对象数组",
            "Script must return object or array of objects",
        )
    })?;

    // 所有字段均为可选，只进行类型检查
    if obj.contains_key("isValid")
        && !result["isValid"].is_null()
        && !result["isValid"].is_boolean()
    {
        return Err(AppError::localized(
            "usage_script.isvalid_type_error",
            "isValid 必须是布尔值或 null",
            "isValid must be boolean or null",
        ));
    }
    if obj.contains_key("invalidMessage")
        && !result["invalidMessage"].is_null()
        && !result["invalidMessage"].is_string()
    {
        return Err(AppError::localized(
            "usage_script.invalidmessage_type_error",
            "invalidMessage 必须是字符串或 null",
            "invalidMessage must be string or null",
        ));
    }
    if obj.contains_key("remaining")
        && !result["remaining"].is_null()
        && !result["remaining"].is_number()
    {
        return Err(AppError::localized(
            "usage_script.remaining_type_error",
            "remaining 必须是数字或 null",
            "remaining must be number or null",
        ));
    }
    if obj.contains_key("unit") && !result["unit"].is_null() && !result["unit"].is_string() {
        return Err(AppError::localized(
            "usage_script.unit_type_error",
            "unit 必须是字符串或 null",
            "unit must be string or null",
        ));
    }
    if obj.contains_key("total") && !result["total"].is_null() && !result["total"].is_number() {
        return Err(AppError::localized(
            "usage_script.total_type_error",
            "total 必须是数字或 null",
            "total must be number or null",
        ));
    }
    if obj.contains_key("used") && !result["used"].is_null() && !result["used"].is_number() {
        return Err(AppError::localized(
            "usage_script.used_type_error",
            "used 必须是数字或 null",
            "used must be number or null",
        ));
    }
    if obj.contains_key("planName")
        && !result["planName"].is_null()
        && !result["planName"].is_string()
    {
        return Err(AppError::localized(
            "usage_script.planname_type_error",
            "planName 必须是字符串或 null",
            "planName must be string or null",
        ));
    }
    if obj.contains_key("extra") && !result["extra"].is_null() && !result["extra"].is_string() {
        return Err(AppError::localized(
            "usage_script.extra_type_error",
            "extra 必须是字符串或 null",
            "extra must be string or null",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::redact_response_preview;

    #[test]
    fn response_preview_redacts_nested_credentials() {
        let preview = redact_response_preview(
            r#"{"message":"bad","authorization":"Bearer secret","nested":{"api_key":"key-secret","token":"token-secret"}}"#,
        );
        assert!(!preview.contains("secret"));
        assert!(!preview.contains("key-secret"));
        assert!(preview.contains("[redacted]"));
    }

    #[test]
    fn non_json_response_preview_is_omitted() {
        assert_eq!(
            redact_response_preview("Bearer secret-token upstream failure"),
            "<response body redacted>"
        );
    }
}
