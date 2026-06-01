use crate::{
    app_config::MultiAppConfig, database::SCHEMA_VERSION, error::AppError, services::ConfigService,
    settings::WebDavSettings, store::AppState,
};
use chrono::Utc;
use futures::StreamExt;
use reqwest::{Client, Method, RequestBuilder, StatusCode, Url};
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;

const SNAPSHOT_KIND: &str = "cc-switch-web-snapshot";
const SNAPSHOT_FILE_EXT: &str = "json";
const MAX_SNAPSHOT_BYTES: usize = 10 * 1024 * 1024;
const MAX_ERROR_BODY_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavCompatibilityCheck {
    pub name: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavSnapshotPreview {
    pub exists: bool,
    pub remote_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
    pub artifact_list: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<i32>,
    pub compatible: bool,
    pub checks: Vec<WebDavCompatibilityCheck>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavSyncResult {
    pub success: bool,
    pub message: String,
    pub remote_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<WebDavSnapshotPreview>,
}

#[derive(Debug, Clone)]
struct RemoteTarget {
    snapshot_url: Url,
    collection_urls: Vec<Url>,
}

#[derive(Debug)]
struct ParsedSnapshot {
    config: MultiAppConfig,
    artifact_list: Vec<String>,
    schema_version: Option<i32>,
}

pub async fn upload_snapshot(
    state: &AppState,
    settings: &WebDavSettings,
) -> Result<WebDavSyncResult, AppError> {
    let settings = normalized_settings(settings)?;
    let target = remote_target(&settings)?;
    let client = webdav_client()?;
    ensure_remote_collections(&client, &settings, &target.collection_urls).await?;

    let config = state.load_config()?;
    let artifact_list = artifact_list(&config);
    let payload = json!({
        "kind": SNAPSHOT_KIND,
        "appVersion": env!("CARGO_PKG_VERSION"),
        "schemaVersion": SCHEMA_VERSION,
        "createdAt": Utc::now().to_rfc3339(),
        "artifactList": artifact_list,
        "config": config,
    });
    let bytes =
        serde_json::to_vec_pretty(&payload).map_err(|e| AppError::JsonSerialize { source: e })?;

    let response = with_auth(
        client
            .put(target.snapshot_url.clone())
            .header("content-type", "application/json")
            .body(bytes),
        &settings,
    )
    .send()
    .await
    .map_err(reqwest_error)?;
    if !response.status().is_success() {
        return Err(status_error("WebDAV upload failed", response).await);
    }

    let preview = preview_snapshot_with_client(&client, &settings, target.snapshot_url.clone())
        .await?
        .unwrap_or_else(|| missing_preview(target.snapshot_url.as_str()));
    Ok(WebDavSyncResult {
        success: true,
        message: "Snapshot uploaded".to_string(),
        remote_path: target.snapshot_url.to_string(),
        backup_id: None,
        preview: Some(preview),
    })
}

pub async fn preview_snapshot(
    settings: &WebDavSettings,
) -> Result<WebDavSnapshotPreview, AppError> {
    let settings = normalized_settings(settings)?;
    let target = remote_target(&settings)?;
    let client = webdav_client()?;
    Ok(
        preview_snapshot_with_client(&client, &settings, target.snapshot_url.clone())
            .await?
            .unwrap_or_else(|| missing_preview(target.snapshot_url.as_str())),
    )
}

pub async fn download_snapshot(
    state: &AppState,
    settings: &WebDavSettings,
) -> Result<WebDavSyncResult, AppError> {
    let settings = normalized_settings(settings)?;
    let target = remote_target(&settings)?;
    let client = webdav_client()?;
    let Some(downloaded) =
        download_snapshot_bytes(&client, &settings, target.snapshot_url.clone()).await?
    else {
        return Err(AppError::InvalidInput(
            "Remote WebDAV snapshot not found".into(),
        ));
    };
    let parsed = parse_snapshot_value(&downloaded.value)?;
    let preview = build_preview(
        target.snapshot_url.as_str(),
        Some(downloaded.bytes_len as u64),
        downloaded.modified_at,
        &parsed,
    );
    if !preview.compatible {
        return Err(AppError::InvalidInput(
            "Remote WebDAV snapshot is not compatible with this version".into(),
        ));
    }

    let backup_id = ConfigService::apply_import_config(parsed.config, state)?;
    Ok(WebDavSyncResult {
        success: true,
        message: "Snapshot downloaded".to_string(),
        remote_path: target.snapshot_url.to_string(),
        backup_id: Some(backup_id),
        preview: Some(preview),
    })
}

fn webdav_client() -> Result<Client, AppError> {
    Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(reqwest_error)
}

fn normalized_settings(settings: &WebDavSettings) -> Result<WebDavSettings, AppError> {
    let mut next = settings.clone();
    next.base_url = next.base_url.trim().trim_end_matches('/').to_string();
    next.username = next.username.trim().to_string();
    next.remote_dir = next.remote_dir.trim().trim_matches('/').to_string();
    next.profile = next.profile.trim().to_string();
    if next.base_url.is_empty() {
        return Err(AppError::InvalidInput("WebDAV base URL is required".into()));
    }
    if next.profile.is_empty() {
        return Err(AppError::InvalidInput("WebDAV profile is required".into()));
    }
    Ok(next)
}

fn remote_target(settings: &WebDavSettings) -> Result<RemoteTarget, AppError> {
    let base = Url::parse(&settings.base_url)
        .map_err(|e| AppError::InvalidInput(format!("Invalid WebDAV base URL: {e}")))?;
    if !matches!(base.scheme(), "http" | "https") {
        return Err(AppError::InvalidInput(
            "WebDAV base URL must use http or https".into(),
        ));
    }
    if !base.username().is_empty() || base.password().is_some() {
        return Err(AppError::InvalidInput(
            "WebDAV credentials must be configured separately".into(),
        ));
    }

    let base_segments = base
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let remote_segments = split_relative_segments(&settings.remote_dir)?;
    let file_name = format!(
        "{}.{}",
        sanitize_profile(&settings.profile)?,
        SNAPSHOT_FILE_EXT
    );

    let mut collection_urls = Vec::new();
    for index in 1..=remote_segments.len() {
        collection_urls.push(build_url(
            base.clone(),
            &base_segments,
            &remote_segments[..index],
            None,
        )?);
    }
    let snapshot_url = build_url(base, &base_segments, &remote_segments, Some(&file_name))?;

    Ok(RemoteTarget {
        snapshot_url,
        collection_urls,
    })
}

fn split_relative_segments(path: &str) -> Result<Vec<String>, AppError> {
    if path.is_empty() {
        return Ok(Vec::new());
    }
    path.split('/')
        .filter(|segment| !segment.trim().is_empty())
        .map(|segment| {
            let trimmed = segment.trim();
            if trimmed == "." || trimmed == ".." || trimmed.contains('\\') {
                return Err(AppError::InvalidInput(
                    "WebDAV remote directory must be a relative path".into(),
                ));
            }
            Ok(trimmed.to_string())
        })
        .collect()
}

fn sanitize_profile(profile: &str) -> Result<String, AppError> {
    let sanitized = profile
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('.')
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        return Err(AppError::InvalidInput("Invalid WebDAV profile".into()));
    }
    Ok(sanitized)
}

fn build_url(
    mut base: Url,
    base_segments: &[String],
    remote_segments: &[String],
    file_name: Option<&str>,
) -> Result<Url, AppError> {
    base.set_path("/");
    {
        let mut segments = base
            .path_segments_mut()
            .map_err(|_| AppError::InvalidInput("Invalid WebDAV base URL".into()))?;
        for segment in base_segments.iter().chain(remote_segments.iter()) {
            segments.push(segment);
        }
        match file_name {
            Some(file_name) => {
                segments.push(file_name);
            }
            None => {
                segments.push("");
            }
        }
    }
    Ok(base)
}

async fn ensure_remote_collections(
    client: &Client,
    settings: &WebDavSettings,
    urls: &[Url],
) -> Result<(), AppError> {
    let mkcol = Method::from_bytes(b"MKCOL")
        .map_err(|e| AppError::Config(format!("Invalid WebDAV MKCOL method: {e}")))?;
    for url in urls {
        let response = with_auth(client.request(mkcol.clone(), url.clone()), settings)
            .send()
            .await
            .map_err(reqwest_error)?;
        let status = response.status();
        if status.is_success()
            || matches!(
                status,
                StatusCode::METHOD_NOT_ALLOWED | StatusCode::CONFLICT | StatusCode::OK
            )
        {
            continue;
        }
        return Err(status_error("WebDAV MKCOL failed", response).await);
    }
    Ok(())
}

struct DownloadedSnapshot {
    value: Value,
    bytes_len: usize,
    modified_at: Option<String>,
}

async fn preview_snapshot_with_client(
    client: &Client,
    settings: &WebDavSettings,
    url: Url,
) -> Result<Option<WebDavSnapshotPreview>, AppError> {
    let Some(downloaded) = download_snapshot_bytes(client, settings, url.clone()).await? else {
        return Ok(None);
    };
    let parsed = parse_snapshot_value(&downloaded.value)?;
    Ok(Some(build_preview(
        url.as_str(),
        Some(downloaded.bytes_len as u64),
        downloaded.modified_at,
        &parsed,
    )))
}

async fn download_snapshot_bytes(
    client: &Client,
    settings: &WebDavSettings,
    url: Url,
) -> Result<Option<DownloadedSnapshot>, AppError> {
    let response = with_auth(client.get(url), settings)
        .send()
        .await
        .map_err(reqwest_error)?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(status_error("WebDAV download failed", response).await);
    }
    let modified_at = response
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = read_limited_response_body(response, MAX_SNAPSHOT_BYTES, "WebDAV snapshot").await?;
    let value = serde_json::from_slice::<Value>(&bytes)
        .map_err(|e| AppError::Config(format!("Invalid WebDAV snapshot JSON: {e}")))?;
    Ok(Some(DownloadedSnapshot {
        value,
        bytes_len: bytes.len(),
        modified_at,
    }))
}

fn parse_snapshot_value(value: &Value) -> Result<ParsedSnapshot, AppError> {
    let is_envelope = value
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == SNAPSHOT_KIND);
    let config_value = if is_envelope {
        value
            .get("config")
            .cloned()
            .ok_or_else(|| AppError::Config("WebDAV snapshot is missing config".into()))?
    } else {
        value.clone()
    };
    MultiAppConfig::ensure_not_v1_value(&config_value)?;
    let has_skills_in_config = config_value
        .as_object()
        .is_some_and(|map| map.contains_key("skills"));
    let mut config: MultiAppConfig = serde_json::from_value(config_value)
        .map_err(|e| AppError::Config(format!("Invalid WebDAV config snapshot: {e}")))?;
    let _ = config.normalize_after_load(has_skills_in_config)?;
    let artifact_list = if is_envelope {
        value
            .get("artifactList")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty())
            .unwrap_or_else(|| artifact_list(&config))
    } else {
        artifact_list(&config)
    };
    let schema_version = value
        .get("schemaVersion")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok());

    Ok(ParsedSnapshot {
        config,
        artifact_list,
        schema_version,
    })
}

fn build_preview(
    remote_path: &str,
    size_bytes: Option<u64>,
    modified_at: Option<String>,
    parsed: &ParsedSnapshot,
) -> WebDavSnapshotPreview {
    let mut checks = Vec::new();
    checks.push(WebDavCompatibilityCheck {
        name: "configVersion".to_string(),
        ok: parsed.config.version == 2,
        message: format!("config version {}", parsed.config.version),
    });
    checks.push(WebDavCompatibilityCheck {
        name: "databaseSchema".to_string(),
        ok: parsed
            .schema_version
            .map(|schema_version| schema_version <= SCHEMA_VERSION)
            .unwrap_or(true),
        message: parsed
            .schema_version
            .map(|schema_version| format!("schema {schema_version}, supported {SCHEMA_VERSION}"))
            .unwrap_or_else(|| "schema not declared".to_string()),
    });
    checks.push(WebDavCompatibilityCheck {
        name: "artifacts".to_string(),
        ok: !parsed.artifact_list.is_empty(),
        message: parsed.artifact_list.join(", "),
    });
    let compatible = checks.iter().all(|check| check.ok);

    WebDavSnapshotPreview {
        exists: true,
        remote_path: remote_path.to_string(),
        size_bytes,
        modified_at,
        artifact_list: parsed.artifact_list.clone(),
        config_version: Some(parsed.config.version),
        schema_version: parsed.schema_version,
        compatible,
        checks,
    }
}

fn missing_preview(remote_path: &str) -> WebDavSnapshotPreview {
    WebDavSnapshotPreview {
        exists: false,
        remote_path: remote_path.to_string(),
        size_bytes: None,
        modified_at: None,
        artifact_list: Vec::new(),
        config_version: None,
        schema_version: None,
        compatible: false,
        checks: vec![WebDavCompatibilityCheck {
            name: "exists".to_string(),
            ok: false,
            message: "remote snapshot not found".to_string(),
        }],
    }
}

fn artifact_list(config: &MultiAppConfig) -> Vec<String> {
    let mut artifacts = Vec::new();
    let provider_count: usize = config
        .apps
        .values()
        .map(|manager| manager.providers.len())
        .sum();
    if provider_count > 0 {
        artifacts.push(format!("providers:{provider_count}"));
    }
    let mcp_count = config
        .mcp
        .servers
        .as_ref()
        .map(|servers| servers.len())
        .unwrap_or(0)
        + config.mcp.claude.servers.len()
        + config.mcp.codex.servers.len()
        + config.mcp.gemini.servers.len()
        + config.mcp.opencode.servers.len();
    if mcp_count > 0 {
        artifacts.push(format!("mcp:{mcp_count}"));
    }
    let prompt_count: usize = config.prompts.claude.prompts.len()
        + config.prompts.codex.prompts.len()
        + config.prompts.gemini.prompts.len()
        + config.prompts.opencode.prompts.len();
    if prompt_count > 0 {
        artifacts.push(format!("prompts:{prompt_count}"));
    }
    let skill_count = config.skills.repos.len() + config.skills.skills.len();
    if skill_count > 0 {
        artifacts.push(format!("skills:{skill_count}"));
    }
    if config.common_config_snippets.claude.is_some()
        || config.common_config_snippets.codex.is_some()
        || config.common_config_snippets.gemini.is_some()
    {
        artifacts.push("commonConfigSnippets".to_string());
    }
    if artifacts.is_empty() {
        artifacts.push("emptyConfig".to_string());
    }
    artifacts
}

fn with_auth(builder: RequestBuilder, settings: &WebDavSettings) -> RequestBuilder {
    if settings.username.is_empty() {
        return builder;
    }
    builder.basic_auth(&settings.username, Some(&settings.password))
}

fn reqwest_error(err: reqwest::Error) -> AppError {
    AppError::Message(format!("WebDAV request failed: {err}"))
}

async fn read_limited_response_body(
    response: reqwest::Response,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, AppError> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(AppError::InvalidInput(format!(
            "{label} exceeds the {max_bytes} byte limit"
        )));
    }

    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(reqwest_error)?;
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(AppError::InvalidInput(format!(
                "{label} exceeds the {max_bytes} byte limit"
            )));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn response_body_preview(response: reqwest::Response, max_bytes: usize) -> String {
    let mut bytes = Vec::new();
    let mut truncated = response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64);
    let mut stream = response.bytes_stream();
    while bytes.len() < max_bytes {
        let Some(chunk) = stream.next().await else {
            break;
        };
        let Ok(chunk) = chunk else {
            break;
        };
        let remaining = max_bytes.saturating_sub(bytes.len());
        if chunk.len() > remaining {
            bytes.extend_from_slice(&chunk[..remaining]);
            truncated = true;
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut body = String::from_utf8_lossy(&bytes).to_string();
    if truncated {
        body.push_str("...");
    }
    body
}

async fn status_error(context: &str, response: reqwest::Response) -> AppError {
    let status = response.status();
    let body = response_body_preview(response, MAX_ERROR_BODY_BYTES).await;
    let detail = body.trim();
    if detail.is_empty() {
        AppError::Message(format!("{context}: HTTP {status}"))
    } else {
        AppError::Message(format!("{context}: HTTP {status}: {detail}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{Provider, ProviderManager};
    use serde_json::json;

    #[test]
    fn remote_target_builds_profile_snapshot_url_and_collection_urls() {
        let settings = WebDavSettings {
            enabled: true,
            base_url: "https://dav.example.com/remote.php/dav/files/me/".to_string(),
            username: "me".to_string(),
            password: "secret".to_string(),
            remote_dir: "/cc-switch-web/prod/".to_string(),
            profile: "main profile".to_string(),
        };

        let target = remote_target(&settings).expect("remote target");

        assert_eq!(
            target.snapshot_url.as_str(),
            "https://dav.example.com/remote.php/dav/files/me/cc-switch-web/prod/main-profile.json"
        );
        assert_eq!(target.collection_urls.len(), 2);
        assert_eq!(
            target.collection_urls[0].as_str(),
            "https://dav.example.com/remote.php/dav/files/me/cc-switch-web/"
        );
    }

    #[test]
    fn remote_target_rejects_parent_segments() {
        let settings = WebDavSettings {
            base_url: "https://dav.example.com".to_string(),
            remote_dir: "../bad".to_string(),
            profile: "default".to_string(),
            ..WebDavSettings::default()
        };

        let err = remote_target(&settings).unwrap_err();

        assert!(err.to_string().contains("relative path"));
    }

    #[tokio::test]
    async fn read_limited_response_body_rejects_oversized_body() {
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

        let response = reqwest::get(format!("http://{addr}/snapshot.json"))
            .await
            .expect("fetch response");
        let err = read_limited_response_body(response, 8, "WebDAV snapshot")
            .await
            .unwrap_err();

        assert!(err.to_string().contains("exceeds"));
        server.await.expect("server join");
    }

    #[test]
    fn snapshot_preview_reports_artifacts_and_compatibility() {
        let mut config = MultiAppConfig::default();
        let mut manager = ProviderManager::default();
        manager.providers.insert(
            "p1".to_string(),
            Provider::with_id(
                "p1".to_string(),
                "Provider".to_string(),
                json!({ "env": {} }),
                None,
            ),
        );
        config.apps.insert("claude".to_string(), manager);

        let value = json!({
            "kind": SNAPSHOT_KIND,
            "schemaVersion": SCHEMA_VERSION,
            "artifactList": ["providers:1"],
            "config": config,
        });
        let parsed = parse_snapshot_value(&value).expect("snapshot parse");
        let preview = build_preview(
            "https://dav.example.com/default.json",
            Some(100),
            None,
            &parsed,
        );

        assert!(preview.exists);
        assert!(preview.compatible);
        assert_eq!(preview.artifact_list, vec!["providers:1"]);
        assert_eq!(preview.config_version, Some(2));
    }
}
