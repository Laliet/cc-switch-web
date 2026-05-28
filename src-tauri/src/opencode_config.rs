use crate::config::{get_client_config_dir_info, get_client_config_dir_path, write_json_file};
use crate::error::AppError;
use crate::settings::get_opencode_override_dir;
use serde_json::{json, Map, Value};
use std::path::PathBuf;

const STANDARD_OMO_PLUGIN_PREFIXES: [&str; 2] = ["oh-my-openagent", "oh-my-opencode"];
const SLIM_OMO_PLUGIN_PREFIXES: [&str; 1] = ["oh-my-opencode-slim"];
pub const STANDARD_OMO_PLUGIN: &str = "oh-my-openagent@latest";
#[allow(dead_code)]
pub const LEGACY_OMO_PLUGIN: &str = "oh-my-opencode@latest";
pub const SLIM_OMO_PLUGIN: &str = "oh-my-opencode-slim@latest";

fn matches_plugin_prefix(plugin_name: &str, prefix: &str) -> bool {
    plugin_name == prefix
        || plugin_name
            .strip_prefix(prefix)
            .map(|suffix| suffix.starts_with('@'))
            .unwrap_or(false)
}

fn matches_any_plugin_prefix(plugin_name: &str, prefixes: &[&str]) -> bool {
    prefixes
        .iter()
        .any(|prefix| matches_plugin_prefix(plugin_name, prefix))
}

fn canonicalize_plugin_name(plugin_name: &str) -> String {
    if let Some(suffix) = plugin_name.strip_prefix("oh-my-opencode") {
        if suffix.is_empty() || suffix.starts_with('@') {
            return format!("oh-my-openagent{suffix}");
        }
    }
    plugin_name.to_string()
}

pub fn get_opencode_dir() -> PathBuf {
    get_client_config_dir_path(get_opencode_override_dir(), ".config/opencode")
        .unwrap_or_else(|_| PathBuf::from(".config").join("opencode"))
}

pub fn get_opencode_dir_info() -> Result<crate::config::ConfigDirInfo, AppError> {
    get_client_config_dir_info(get_opencode_override_dir(), ".config/opencode")
}

pub fn get_opencode_config_path() -> PathBuf {
    get_opencode_dir().join("opencode.json")
}

pub fn read_opencode_config() -> Result<Value, AppError> {
    let path = get_opencode_config_path();

    if !path.exists() {
        return Ok(json!({
            "$schema": "https://opencode.ai/config.json"
        }));
    }

    let content = std::fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    json5::from_str(&content).map_err(|e| {
        AppError::Config(format!(
            "Failed to parse OpenCode config: {}: {e}",
            path.display()
        ))
    })
}

pub fn write_opencode_config(config: &Value) -> Result<(), AppError> {
    let path = get_opencode_config_path();
    write_json_file(&path, config)?;
    Ok(())
}

#[allow(dead_code)]
pub fn get_providers() -> Result<Map<String, Value>, AppError> {
    let config = read_opencode_config()?;
    Ok(config
        .get("provider")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default())
}

#[allow(dead_code)]
pub fn set_provider(id: &str, config: Value) -> Result<(), AppError> {
    let mut full_config = read_opencode_config()?;

    if full_config.get("provider").is_none() {
        full_config["provider"] = json!({});
    }

    if let Some(providers) = full_config
        .get_mut("provider")
        .and_then(|value| value.as_object_mut())
    {
        providers.insert(id.to_string(), config);
    }

    write_opencode_config(&full_config)
}

#[allow(dead_code)]
pub fn remove_provider(id: &str) -> Result<(), AppError> {
    let mut config = read_opencode_config()?;

    if let Some(providers) = config
        .get_mut("provider")
        .and_then(|value| value.as_object_mut())
    {
        providers.remove(id);
    }

    write_opencode_config(&config)
}

pub fn add_plugin(plugin_name: &str) -> Result<(), AppError> {
    let mut config = read_opencode_config()?;
    let normalized_plugin_name = canonicalize_plugin_name(plugin_name);

    let plugins = config
        .get_mut("plugin")
        .and_then(|value| value.as_array_mut());

    match plugins {
        Some(arr) => {
            if matches_any_plugin_prefix(&normalized_plugin_name, &STANDARD_OMO_PLUGIN_PREFIXES)
                || matches_any_plugin_prefix(&normalized_plugin_name, &SLIM_OMO_PLUGIN_PREFIXES)
            {
                arr.retain(|value| {
                    value
                        .as_str()
                        .map(|plugin| {
                            !matches_any_plugin_prefix(plugin, &STANDARD_OMO_PLUGIN_PREFIXES)
                                && !matches_any_plugin_prefix(plugin, &SLIM_OMO_PLUGIN_PREFIXES)
                        })
                        .unwrap_or(true)
                });
            }

            let already_exists = arr
                .iter()
                .any(|value| value.as_str() == Some(normalized_plugin_name.as_str()));
            if !already_exists {
                arr.push(Value::String(normalized_plugin_name));
            }
        }
        None => {
            config["plugin"] = json!([normalized_plugin_name]);
        }
    }

    write_opencode_config(&config)
}

pub fn add_standard_omo_plugin() -> Result<(), AppError> {
    add_plugin(STANDARD_OMO_PLUGIN)
}

#[allow(dead_code)]
pub fn add_slim_omo_plugin() -> Result<(), AppError> {
    add_plugin(SLIM_OMO_PLUGIN)
}

#[allow(dead_code)]
pub fn remove_plugin_by_prefix(prefix: &str) -> Result<(), AppError> {
    remove_plugins_by_prefixes(&[prefix])
}

pub fn remove_plugins_by_prefixes(prefixes: &[&str]) -> Result<(), AppError> {
    let mut config = read_opencode_config()?;

    if let Some(arr) = config
        .get_mut("plugin")
        .and_then(|value| value.as_array_mut())
    {
        arr.retain(|value| {
            value
                .as_str()
                .map(|plugin| !matches_any_plugin_prefix(plugin, prefixes))
                .unwrap_or(true)
        });

        if arr.is_empty() {
            config.as_object_mut().map(|obj| obj.remove("plugin"));
        }
    }

    write_opencode_config(&config)
}

#[allow(dead_code)]
pub fn has_plugin(plugin_name: &str) -> Result<bool, AppError> {
    has_any_plugin(&[plugin_name])
}

#[allow(dead_code)]
pub fn has_any_plugin(plugin_names: &[&str]) -> Result<bool, AppError> {
    let config = read_opencode_config()?;
    Ok(config
        .get("plugin")
        .and_then(|value| value.as_array())
        .map(|plugins| {
            plugins.iter().any(|value| {
                value.as_str().is_some_and(|plugin| {
                    plugin_names
                        .iter()
                        .any(|name| plugin == canonicalize_plugin_name(name))
                })
            })
        })
        .unwrap_or(false))
}

pub fn has_standard_omo_plugin() -> Result<bool, AppError> {
    has_any_plugin_prefix(&STANDARD_OMO_PLUGIN_PREFIXES)
}

pub fn has_slim_omo_plugin() -> Result<bool, AppError> {
    has_any_plugin_prefix(&SLIM_OMO_PLUGIN_PREFIXES)
}

fn has_any_plugin_prefix(prefixes: &[&str]) -> Result<bool, AppError> {
    let config = read_opencode_config()?;
    Ok(config
        .get("plugin")
        .and_then(|value| value.as_array())
        .map(|plugins| {
            plugins.iter().any(|value| {
                value
                    .as_str()
                    .is_some_and(|plugin| matches_any_plugin_prefix(plugin, prefixes))
            })
        })
        .unwrap_or(false))
}

pub fn get_mcp_servers() -> Result<Map<String, Value>, AppError> {
    let config = read_opencode_config()?;
    Ok(config
        .get("mcp")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default())
}

pub fn set_mcp_server(id: &str, config: Value) -> Result<(), AppError> {
    let mut full_config = read_opencode_config()?;

    if full_config.get("mcp").is_none() {
        full_config["mcp"] = json!({});
    }

    if let Some(mcp) = full_config
        .get_mut("mcp")
        .and_then(|value| value.as_object_mut())
    {
        mcp.insert(id.to_string(), config);
    }

    write_opencode_config(&full_config)
}

pub fn remove_mcp_server(id: &str) -> Result<(), AppError> {
    let mut config = read_opencode_config()?;

    if let Some(mcp) = config
        .get_mut("mcp")
        .and_then(|value| value.as_object_mut())
    {
        mcp.remove(id);
    }

    write_opencode_config(&config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonicalizes_legacy_omo_plugin_name() {
        assert_eq!(
            canonicalize_plugin_name("oh-my-opencode@latest"),
            "oh-my-openagent@latest"
        );
        assert_eq!(
            canonicalize_plugin_name("oh-my-opencode"),
            "oh-my-openagent"
        );
        assert_eq!(
            canonicalize_plugin_name("oh-my-opencode-slim@latest"),
            "oh-my-opencode-slim@latest"
        );
    }

    #[test]
    fn plugin_prefix_matching_requires_exact_package_prefix() {
        assert!(matches_plugin_prefix(
            "oh-my-openagent@latest",
            "oh-my-openagent"
        ));
        assert!(matches_plugin_prefix("oh-my-openagent", "oh-my-openagent"));
        assert!(!matches_plugin_prefix(
            "oh-my-openagent-slim@latest",
            "oh-my-openagent"
        ));
    }

    #[test]
    fn read_opencode_config_accepts_json5_content_shape() {
        let parsed: Value = json5::from_str(
            r#"{
              // OpenCode accepts JSON5-style comments
              provider: {
                test: { npm: "@ai-sdk/openai-compatible" },
              },
            }"#,
        )
        .expect("json5 should parse");

        assert_eq!(
            parsed["provider"]["test"]["npm"],
            json!("@ai-sdk/openai-compatible")
        );
    }
}
