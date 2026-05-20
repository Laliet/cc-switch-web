use crate::config::{get_client_config_dir_info, get_client_config_dir_path, write_json_file};
use crate::error::AppError;
use crate::settings::get_opencode_override_dir;
use serde_json::{json, Map, Value};
use std::path::PathBuf;

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
    serde_json::from_str(&content).map_err(|e| AppError::json(&path, e))
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

    let plugins = config
        .get_mut("plugin")
        .and_then(|value| value.as_array_mut());

    match plugins {
        Some(arr) => {
            let already_exists = arr.iter().any(|value| value.as_str() == Some(plugin_name));
            if !already_exists {
                arr.push(Value::String(plugin_name.to_string()));
            }
        }
        None => {
            config["plugin"] = json!([plugin_name]);
        }
    }

    write_opencode_config(&config)
}

pub fn remove_plugin_by_prefix(prefix: &str) -> Result<(), AppError> {
    let mut config = read_opencode_config()?;

    if let Some(arr) = config
        .get_mut("plugin")
        .and_then(|value| value.as_array_mut())
    {
        arr.retain(|value| {
            value
                .as_str()
                .map(|plugin| !plugin.starts_with(prefix))
                .unwrap_or(true)
        });

        if arr.is_empty() {
            config.as_object_mut().map(|obj| obj.remove("plugin"));
        }
    }

    write_opencode_config(&config)
}

pub fn has_plugin(plugin_name: &str) -> Result<bool, AppError> {
    let config = read_opencode_config()?;
    Ok(config
        .get("plugin")
        .and_then(|value| value.as_array())
        .map(|plugins| {
            plugins
                .iter()
                .any(|value| value.as_str() == Some(plugin_name))
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
