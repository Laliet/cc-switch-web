//! OpenClaw JSON5 configuration support.
//!
//! OpenClaw keeps all providers in `models.providers` and selects a default
//! model separately under `agents.defaults.model`. This module deliberately
//! preserves unknown sections while writing atomically, because OpenClaw may
//! add fields that cc-switch-web does not know yet.

use crate::config::{atomic_write, get_app_config_dir, get_home_dir};
use crate::error::AppError;
use json_five::rt::parser::{
    from_str as rt_from_str, JSONKeyValuePair as RtJSONKeyValuePair,
    JSONObjectContext as RtJSONObjectContext, JSONText as RtJSONText, JSONValue as RtJSONValue,
    KeyValuePairContext as RtKeyValuePairContext,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_CONFIG_BYTES: u64 = 8 * 1024 * 1024;
const MAX_BACKUPS: usize = 10;
const DEFAULT_SOURCE: &str = "{\n  models: {\n    mode: 'merge',\n    providers: {},\n  },\n}\n";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawHealthWarning {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawWriteOutcome {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<OpenClawHealthWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawDefaultModel {
    pub primary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawModelEntry {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<OpenClawModelEntry>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawLiveModelSummary {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawLiveProviderSummary {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    pub models: Vec<OpenClawLiveModelSummary>,
    pub has_api_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawLiveStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<OpenClawDefaultModel>,
    pub providers: Vec<OpenClawLiveProviderSummary>,
    pub warnings: Vec<OpenClawHealthWarning>,
}

pub fn get_openclaw_dir() -> PathBuf {
    if let Ok(value) = std::env::var("CC_SWITCH_OPENCLAW_CONFIG_DIR") {
        let value = value.trim();
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    get_home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openclaw")
}

pub fn get_openclaw_config_path() -> PathBuf {
    get_openclaw_dir().join("openclaw.json")
}

pub fn get_openclaw_workspace_dir() -> PathBuf {
    if let Ok(value) = std::env::var("CC_SWITCH_OPENCLAW_WORKSPACE_DIR") {
        let value = value.trim();
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    get_openclaw_dir().join("workspace")
}

pub fn read_openclaw_config() -> Result<Value, AppError> {
    let path = get_openclaw_config_path();
    if !path.exists() {
        return Ok(default_config());
    }
    let metadata = fs::metadata(&path).map_err(|e| AppError::io(&path, e))?;
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err(AppError::InvalidInput(format!(
            "OpenClaw config exceeds {} MiB",
            MAX_CONFIG_BYTES / 1024 / 1024
        )));
    }
    let content = fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    json5::from_str(&content)
        .map_err(|e| AppError::Config(format!("Failed to parse OpenClaw JSON5: {e}")))
}

pub fn get_providers() -> Result<Map<String, Value>, AppError> {
    Ok(read_openclaw_config()?
        .get("models")
        .and_then(|value| value.get("providers"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default())
}

pub fn get_provider(id: &str) -> Result<Option<Value>, AppError> {
    Ok(get_providers()?.get(id).cloned())
}

pub fn get_live_provider_summaries() -> Result<Vec<OpenClawLiveProviderSummary>, AppError> {
    let mut providers = get_typed_providers()?
        .into_iter()
        .map(|(id, provider)| OpenClawLiveProviderSummary {
            id,
            base_url: provider.base_url.as_deref().and_then(sanitize_base_url),
            api: provider.api,
            models: provider
                .models
                .into_iter()
                .map(|model| OpenClawLiveModelSummary {
                    id: model.id,
                    name: model.name,
                    alias: model.alias,
                })
                .collect(),
            has_api_key: provider
                .api_key
                .as_deref()
                .is_some_and(|key| !key.trim().is_empty()),
        })
        .collect::<Vec<_>>();
    providers.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(providers)
}

pub fn get_live_provider_summary(
    id: &str,
) -> Result<Option<OpenClawLiveProviderSummary>, AppError> {
    let id = validate_id(id)?;
    Ok(get_live_provider_summaries()?
        .into_iter()
        .find(|provider| provider.id == id))
}

pub fn get_live_status() -> Result<OpenClawLiveStatus, AppError> {
    Ok(OpenClawLiveStatus {
        default_model: get_default_model()?,
        providers: get_live_provider_summaries()?,
        warnings: scan_openclaw_config_health()?,
    })
}

pub fn set_provider(id: &str, provider: Value) -> Result<OpenClawWriteOutcome, AppError> {
    let id = validate_id(id)?;
    let mut document = OpenClawDocument::load()?;
    let mut config = document.parsed_value()?;
    let root = ensure_object(&mut config);
    let models = root
        .entry("models")
        .or_insert_with(|| json!({"mode": "merge", "providers": {}}));
    let models_object = ensure_object(models);
    let providers = models_object
        .entry("providers")
        .or_insert_with(|| Value::Object(Map::new()));
    ensure_object(providers).insert(id, provider);
    document.set_root_section(
        "models",
        config.get("models").expect("models was initialized"),
    )?;
    document.save()
}

pub fn merge_providers(providers: Map<String, Value>) -> Result<OpenClawWriteOutcome, AppError> {
    for id in providers.keys() {
        validate_id(id)?;
    }
    let mut document = OpenClawDocument::load()?;
    let mut config = document.parsed_value()?;
    let root = ensure_object(&mut config);
    let models = root
        .entry("models")
        .or_insert_with(|| json!({"mode": "merge", "providers": {}}));
    let models_object = ensure_object(models);
    let live_providers = models_object
        .entry("providers")
        .or_insert_with(|| Value::Object(Map::new()));
    let live_providers = ensure_object(live_providers);
    for (id, provider) in providers {
        live_providers.insert(id, provider);
    }
    document.set_root_section("models", models)?;
    document.save()
}

pub fn remove_provider(id: &str) -> Result<OpenClawWriteOutcome, AppError> {
    let id = validate_id(id)?;
    let mut document = OpenClawDocument::load()?;
    let mut config = document.parsed_value()?;
    let removed = config
        .get_mut("models")
        .and_then(|models| models.get_mut("providers"))
        .and_then(Value::as_object_mut)
        .map(|providers| providers.remove(&id).is_some())
        .unwrap_or(false);
    if !removed {
        return Ok(OpenClawWriteOutcome {
            warnings: scan_health(&config),
            ..Default::default()
        });
    }
    document.set_root_section(
        "models",
        config.get("models").expect("models exists after removal"),
    )?;
    document.save()
}

pub fn get_default_model() -> Result<Option<OpenClawDefaultModel>, AppError> {
    let config = read_openclaw_config()?;
    let Some(value) = config
        .get("agents")
        .and_then(|agents| agents.get("defaults"))
        .and_then(|defaults| defaults.get("model"))
    else {
        return Ok(None);
    };
    if let Some(primary) = value.as_str() {
        return Ok(Some(OpenClawDefaultModel {
            primary: primary.to_string(),
            fallbacks: Vec::new(),
        }));
    }
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|e| AppError::Config(format!("Invalid OpenClaw default model: {e}")))
}

pub fn set_default_model(model: &OpenClawDefaultModel) -> Result<OpenClawWriteOutcome, AppError> {
    if model.primary.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "OpenClaw default model is required".to_string(),
        ));
    }
    let mut document = OpenClawDocument::load()?;
    let mut config = document.parsed_value()?;
    validate_default_model(&config, model)?;
    let root = ensure_object(&mut config);
    let agents = root
        .entry("agents")
        .or_insert_with(|| Value::Object(Map::new()));
    let defaults = ensure_object(agents)
        .entry("defaults")
        .or_insert_with(|| Value::Object(Map::new()));
    ensure_object(defaults).insert(
        "model".to_string(),
        serde_json::to_value(model).map_err(|e| AppError::JsonSerialize { source: e })?,
    );
    document.set_root_section(
        "agents",
        config.get("agents").expect("agents was initialized"),
    )?;
    document.save()
}

pub fn clear_default_model() -> Result<OpenClawWriteOutcome, AppError> {
    let mut document = OpenClawDocument::load()?;
    let mut config = document.parsed_value()?;
    let removed = config
        .get_mut("agents")
        .and_then(|agents| agents.get_mut("defaults"))
        .and_then(Value::as_object_mut)
        .map(|defaults| defaults.remove("model").is_some())
        .unwrap_or(false);
    if !removed {
        return Ok(OpenClawWriteOutcome {
            warnings: scan_health(&config),
            ..Default::default()
        });
    }
    document.set_root_section(
        "agents",
        config.get("agents").expect("agents exists after removal"),
    )?;
    document.save()
}

pub fn scan_openclaw_config_health() -> Result<Vec<OpenClawHealthWarning>, AppError> {
    let path = get_openclaw_config_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    match read_openclaw_config() {
        Ok(config) => Ok(scan_health(&config)),
        Err(_) => Ok(vec![OpenClawHealthWarning {
            code: "config_parse_failed".to_string(),
            message: "OpenClaw configuration could not be read".to_string(),
            path: Some("openclaw.json".to_string()),
        }]),
    }
}

pub fn get_typed_providers() -> Result<HashMap<String, OpenClawProviderConfig>, AppError> {
    let mut result = HashMap::new();
    for (id, value) in get_providers()? {
        if let Ok(config) = serde_json::from_value(value) {
            result.insert(id, config);
        }
    }
    Ok(result)
}

fn default_config() -> Value {
    json!({"models": {"mode": "merge", "providers": {}}})
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().expect("value normalized to object")
}

fn validate_id(id: &str) -> Result<String, AppError> {
    let trimmed = id.trim();
    if trimmed.is_empty()
        || trimmed.len() > 128
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains('/')
        || trimmed.contains('\\')
    {
        return Err(AppError::InvalidInput(
            "Invalid OpenClaw provider id".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn sanitize_base_url(raw: &str) -> Option<String> {
    let mut url = url::Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    url.set_username("").ok()?;
    url.set_password(None).ok()?;
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
}

fn validate_default_model(config: &Value, model: &OpenClawDefaultModel) -> Result<(), AppError> {
    for reference in
        std::iter::once(model.primary.as_str()).chain(model.fallbacks.iter().map(String::as_str))
    {
        if model_reference_exists(config, reference) {
            continue;
        }
        return Err(AppError::InvalidInput(format!(
            "OpenClaw model reference does not exist: {reference}"
        )));
    }
    Ok(())
}

fn model_reference_exists(config: &Value, reference: &str) -> bool {
    let Some((provider_id, model_id)) = reference.trim().split_once('/') else {
        return false;
    };
    if provider_id.is_empty() || model_id.is_empty() {
        return false;
    }
    config
        .get("models")
        .and_then(|models| models.get("providers"))
        .and_then(|providers| providers.get(provider_id))
        .and_then(|provider| provider.get("models"))
        .and_then(Value::as_array)
        .is_some_and(|models| {
            models.iter().any(|model| {
                model
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id == model_id)
            })
        })
}

struct OpenClawDocument {
    path: PathBuf,
    backup_dir: PathBuf,
    original_source: Option<String>,
    text: RtJSONText,
}

impl OpenClawDocument {
    fn load() -> Result<Self, AppError> {
        let path = get_openclaw_config_path();
        let backup_dir = get_app_config_dir()?.join("backups").join("openclaw");
        Self::load_from(path, backup_dir)
    }

    fn load_from(path: PathBuf, backup_dir: PathBuf) -> Result<Self, AppError> {
        let original_source = if path.exists() {
            Some(fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?)
        } else {
            None
        };
        let source = original_source
            .clone()
            .unwrap_or_else(|| DEFAULT_SOURCE.to_string());
        let text = rt_from_str(&source).map_err(|e| {
            AppError::Config(format!(
                "Failed to parse OpenClaw config as round-trip JSON5: {}",
                e.message
            ))
        })?;
        Ok(Self {
            path,
            backup_dir,
            original_source,
            text,
        })
    }

    fn set_root_section(&mut self, key: &str, value: &Value) -> Result<(), AppError> {
        let RtJSONValue::JSONObject {
            key_value_pairs,
            context,
        } = &mut self.text.value
        else {
            return Err(AppError::Config(
                "OpenClaw config root must be a JSON5 object".to_string(),
            ));
        };
        if key_value_pairs.is_empty()
            && context
                .as_ref()
                .map(|context| context.wsc.0.is_empty())
                .unwrap_or(true)
        {
            *context = Some(RtJSONObjectContext {
                wsc: ("\n  ".to_string(),),
            });
        }
        let leading_ws = context
            .as_ref()
            .map(|context| context.wsc.0.clone())
            .unwrap_or_default();
        let entry_separator_ws = derive_entry_separator(&leading_ws);
        let child_indent = extract_trailing_indent(&leading_ws);
        let next_value = value_to_rt_value(value, &child_indent)?;

        if let Some(existing) = key_value_pairs
            .iter_mut()
            .find(|pair| json5_key_name(&pair.key) == Some(key))
        {
            existing.value = next_value;
            return Ok(());
        }
        let pair = if let Some(last) = key_value_pairs.last_mut() {
            let last_context = ensure_kvp_context(last);
            let closing_ws = if let Some(after_comma) = last_context.wsc.3.clone() {
                last_context.wsc.3 = Some(entry_separator_ws.clone());
                after_comma
            } else {
                let closing_ws = std::mem::take(&mut last_context.wsc.2);
                last_context.wsc.3 = Some(entry_separator_ws);
                closing_ws
            };
            make_root_pair(key, next_value, closing_ws)
        } else {
            make_root_pair(
                key,
                next_value,
                derive_closing_ws_from_separator(&leading_ws),
            )
        };
        key_value_pairs.push(pair);
        Ok(())
    }

    fn parsed_value(&self) -> Result<Value, AppError> {
        json5::from_str(&self.text.to_string())
            .map_err(|error| AppError::Config(format!("Failed to parse OpenClaw JSON5: {error}")))
    }

    fn save(self) -> Result<OpenClawWriteOutcome, AppError> {
        let _guard = write_lock()
            .lock()
            .map_err(|_| AppError::Database("OpenClaw config lock poisoned".to_string()))?;
        let current_source = if self.path.exists() {
            Some(fs::read_to_string(&self.path).map_err(|e| AppError::io(&self.path, e))?)
        } else {
            None
        };
        if current_source != self.original_source {
            return Err(AppError::Config(
                "OpenClaw config changed on disk; reload and retry".to_string(),
            ));
        }
        let next_source = self.text.to_string();
        let parsed: Value = json5::from_str(&next_source).map_err(|e| {
            AppError::Config(format!("Failed to validate updated OpenClaw JSON5: {e}"))
        })?;
        if current_source.as_deref() == Some(next_source.as_str()) {
            return Ok(OpenClawWriteOutcome {
                backup_path: None,
                warnings: scan_health(&parsed),
            });
        }
        let backup_path = current_source
            .as_deref()
            .map(|source| create_backup(source, &self.backup_dir))
            .transpose()?
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            });
        atomic_write(&self.path, next_source.as_bytes())?;
        Ok(OpenClawWriteOutcome {
            backup_path,
            warnings: scan_health(&parsed),
        })
    }
}

fn create_backup(source: &str, backup_dir: &Path) -> Result<PathBuf, AppError> {
    fs::create_dir_all(backup_dir).map_err(|e| AppError::io(backup_dir, e))?;
    let stamp = chrono::Utc::now().timestamp_millis();
    let backup = backup_dir.join(format!("openclaw-{stamp}.json5"));
    atomic_write(&backup, source.as_bytes())?;
    cleanup_backups(backup_dir);
    Ok(backup)
}

fn ensure_kvp_context(pair: &mut RtJSONKeyValuePair) -> &mut RtKeyValuePairContext {
    pair.context.get_or_insert_with(|| RtKeyValuePairContext {
        wsc: (String::new(), " ".to_string(), String::new(), None),
    })
}

fn extract_trailing_indent(separator: &str) -> String {
    separator
        .rsplit_once('\n')
        .map(|(_, indent)| indent.to_string())
        .unwrap_or_default()
}

fn derive_entry_separator(leading: &str) -> String {
    if leading.contains('\n') {
        format!("\n{}", extract_trailing_indent(leading))
    } else {
        String::new()
    }
}

fn derive_closing_ws_from_separator(separator: &str) -> String {
    let Some((prefix, indent)) = separator.rsplit_once('\n') else {
        return String::new();
    };
    let reduced = indent
        .strip_suffix("  ")
        .or_else(|| indent.strip_suffix('\t'))
        .or_else(|| indent.strip_suffix(' '))
        .unwrap_or(indent);
    format!("{prefix}\n{reduced}")
}

fn value_to_rt_value(value: &Value, parent_indent: &str) -> Result<RtJSONValue, AppError> {
    let source = serde_json::to_string_pretty(value)
        .map_err(|e| AppError::Config(format!("Failed to serialize OpenClaw section: {e}")))?;
    let adjusted = reindent_block(&source, parent_indent);
    rt_from_str(&adjusted)
        .map(|text| text.value)
        .map_err(|e| AppError::Config(format!("Failed to parse generated JSON5: {}", e.message)))
}

fn reindent_block(source: &str, indent: &str) -> String {
    if indent.is_empty() || !source.contains('\n') {
        return source.to_string();
    }
    let mut lines = source.lines();
    let mut result = lines.next().unwrap_or_default().to_string();
    for line in lines {
        result.push('\n');
        result.push_str(indent);
        result.push_str(line);
    }
    result
}

fn make_root_pair(key: &str, value: RtJSONValue, closing_ws: String) -> RtJSONKeyValuePair {
    RtJSONKeyValuePair {
        key: if is_identifier_key(key) {
            RtJSONValue::Identifier(key.to_string())
        } else {
            RtJSONValue::DoubleQuotedString(key.to_string())
        },
        value,
        context: Some(RtKeyValuePairContext {
            wsc: (String::new(), " ".to_string(), closing_ws, None),
        }),
    }
}

fn is_identifier_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    matches!(first, 'a'..='z' | 'A'..='Z' | '_' | '$')
        && chars.all(|ch| matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$'))
}

fn json5_key_name(key: &RtJSONValue) -> Option<&str> {
    match key {
        RtJSONValue::Identifier(name)
        | RtJSONValue::DoubleQuotedString(name)
        | RtJSONValue::SingleQuotedString(name) => Some(name),
        _ => None,
    }
}

fn write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn cleanup_backups(dir: &Path) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    let mut entries = read_dir
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json5"))
        .collect::<Vec<_>>();
    if entries.len() <= MAX_BACKUPS {
        return;
    }
    entries.sort_by_key(|entry| entry.metadata().and_then(|meta| meta.modified()).ok());
    let remove_count = entries.len().saturating_sub(MAX_BACKUPS);
    for entry in entries.into_iter().take(remove_count) {
        let _ = fs::remove_file(entry.path());
    }
}

fn scan_health(config: &Value) -> Vec<OpenClawHealthWarning> {
    let mut warnings = Vec::new();
    let models = config.get("models").and_then(Value::as_object);
    if models.is_none() {
        warnings.push(OpenClawHealthWarning {
            code: "models_missing".to_string(),
            message: "OpenClaw models section is missing".to_string(),
            path: Some("models".to_string()),
        });
    } else if models
        .and_then(|models| models.get("mode"))
        .and_then(Value::as_str)
        .is_some_and(|mode| mode != "merge")
    {
        warnings.push(OpenClawHealthWarning {
            code: "models_mode_not_merge".to_string(),
            message: "OpenClaw models.mode should be 'merge' for additive providers".to_string(),
            path: Some("models.mode".to_string()),
        });
    }

    match models
        .and_then(|models| models.get("providers"))
        .and_then(Value::as_object)
    {
        Some(providers) => {
            for (provider_id, provider) in providers {
                if validate_id(provider_id).is_err() || !provider.is_object() {
                    warnings.push(OpenClawHealthWarning {
                        code: "provider_invalid".to_string(),
                        message: "OpenClaw provider configuration is invalid".to_string(),
                        path: Some("models.providers".to_string()),
                    });
                    continue;
                }
                let has_models = provider
                    .get("models")
                    .and_then(Value::as_array)
                    .is_some_and(|models| {
                        models.iter().any(|model| {
                            model
                                .get("id")
                                .and_then(Value::as_str)
                                .is_some_and(|id| !id.trim().is_empty())
                        })
                    });
                if !has_models {
                    warnings.push(OpenClawHealthWarning {
                        code: "provider_models_missing".to_string(),
                        message: "OpenClaw provider has no model IDs".to_string(),
                        path: Some("models.providers".to_string()),
                    });
                }
            }
        }
        None if models.is_some() => warnings.push(OpenClawHealthWarning {
            code: "providers_missing".to_string(),
            message: "OpenClaw models.providers section is missing".to_string(),
            path: Some("models.providers".to_string()),
        }),
        None => {}
    }

    if let Some(default) = config.pointer("/agents/defaults/model").and_then(|value| {
        value
            .as_str()
            .map(ToString::to_string)
            .or_else(|| value.get("primary")?.as_str().map(ToString::to_string))
    }) {
        if !model_reference_exists(config, &default) {
            warnings.push(OpenClawHealthWarning {
                code: "default_model_missing".to_string(),
                message: "OpenClaw default model does not exist".to_string(),
                path: Some("agents.defaults.model".to_string()),
            });
        }
    }
    if let Some(profile) = config
        .get("tools")
        .and_then(|tools| tools.get("profile"))
        .and_then(Value::as_str)
    {
        if !["minimal", "coding", "messaging", "full"].contains(&profile) {
            warnings.push(OpenClawHealthWarning {
                code: "invalid_tools_profile".to_string(),
                message: "OpenClaw tools.profile is unsupported".to_string(),
                path: Some("tools.profile".to_string()),
            });
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::{
        scan_health, validate_default_model, validate_id, OpenClawDefaultModel, OpenClawDocument,
    };
    use serde_json::json;
    use std::fs;

    #[test]
    fn validates_provider_ids() {
        assert!(validate_id("deepseek").is_ok());
        assert!(validate_id("../escape").is_err());
        assert!(validate_id("a/b").is_err());
    }

    #[test]
    fn reports_invalid_tools_profile() {
        let warnings = scan_health(&json!({"models": {}, "tools": {"profile": "unsafe"}}));
        assert!(warnings
            .iter()
            .any(|warning| warning.code == "invalid_tools_profile"));
    }

    #[test]
    fn round_trip_write_preserves_comments_and_unknown_sections() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("openclaw.json");
        let source = r#"{
  // this section is managed by OpenClaw
  channels: {
    telegram: { enabled: true }, // keep inline comment
  },
  models: {
    mode: 'merge',
    providers: { old: { models: [{ id: 'old-1' }] } },
  },
}
"#;
        fs::write(&path, source).expect("seed config");

        let mut document = OpenClawDocument::load_from(path.clone(), temp.path().join("backups"))
            .expect("load round-trip document");
        document
            .set_root_section(
                "models",
                &json!({
                    "mode": "merge",
                    "providers": {"new": {"models": [{"id": "new-1"}]}}
                }),
            )
            .expect("replace models section");
        let outcome = document.save().expect("save round-trip document");

        let backup_name = outcome.backup_path.expect("backup name");
        assert!(!backup_name.contains('/') && !backup_name.contains('\\'));
        let updated = fs::read_to_string(path).expect("read updated config");
        assert!(updated.contains("this section is managed by OpenClaw"));
        assert!(updated.contains("keep inline comment"));
        let parsed: serde_json::Value = json5::from_str(&updated).expect("parse updated JSON5");
        assert_eq!(
            parsed.pointer("/channels/telegram/enabled"),
            Some(&json!(true))
        );
        assert!(parsed.pointer("/models/providers/new").is_some());
        assert!(parsed.pointer("/models/providers/old").is_none());
    }

    #[test]
    fn health_warnings_do_not_echo_provider_or_model_values() {
        let warnings = scan_health(&json!({
            "models": {
                "providers": {
                    "secret-provider-token": {"models": []}
                }
            },
            "agents": {"defaults": {"model": "secret-provider-token/private-model"}}
        }));
        let rendered = serde_json::to_string(&warnings).expect("serialize warnings");
        assert!(!rendered.contains("secret-provider-token"));
        assert!(!rendered.contains("private-model"));
    }

    #[test]
    fn round_trip_write_rejects_concurrent_disk_changes() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("openclaw.json");
        fs::write(&path, "{ models: { providers: {} } }\n").expect("seed config");

        let mut document = OpenClawDocument::load_from(path.clone(), temp.path().join("backups"))
            .expect("load round-trip document");
        document
            .set_root_section("models", &json!({"mode": "merge", "providers": {}}))
            .expect("replace models section");
        let external = "{ models: { providers: {} }, external: true }\n";
        fs::write(&path, external).expect("simulate external edit");

        let error = document
            .save()
            .expect_err("concurrent edit must be rejected");
        assert!(error.to_string().contains("changed on disk"));
        assert_eq!(
            fs::read_to_string(path).expect("read external edit"),
            external
        );
    }

    #[test]
    fn default_model_must_reference_a_live_provider_model() {
        let config = json!({
            "models": {
                "providers": {
                    "alpha": {"models": [{"id": "alpha-1"}]}
                }
            }
        });
        assert!(validate_default_model(
            &config,
            &OpenClawDefaultModel {
                primary: "alpha/alpha-1".to_string(),
                fallbacks: Vec::new(),
            }
        )
        .is_ok());
        assert!(validate_default_model(
            &config,
            &OpenClawDefaultModel {
                primary: "alpha/missing".to_string(),
                fallbacks: Vec::new(),
            }
        )
        .is_err());
    }
}
