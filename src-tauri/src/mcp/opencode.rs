use serde_json::{json, Map, Value};
use std::collections::HashMap;

use crate::app_config::{McpApps, McpServer, MultiAppConfig};
use crate::error::AppError;
use crate::opencode_config;

use super::validation::validate_server_spec;

fn should_sync_opencode_mcp() -> bool {
    opencode_config::get_opencode_dir().exists()
}

pub fn convert_to_opencode_format(spec: &Value) -> Result<Value, AppError> {
    let obj = spec
        .as_object()
        .ok_or_else(|| AppError::McpValidation("MCP spec must be a JSON object".into()))?;

    let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("stdio");
    let mut result = serde_json::Map::new();

    match typ {
        "stdio" => {
            result.insert("type".into(), json!("local"));

            let cmd = obj.get("command").and_then(|v| v.as_str()).unwrap_or("");
            let mut command_arr = vec![json!(cmd)];
            if let Some(args) = obj.get("args").and_then(|v| v.as_array()) {
                for arg in args {
                    command_arr.push(arg.clone());
                }
            }
            result.insert("command".into(), Value::Array(command_arr));

            if let Some(env) = obj.get("env") {
                if env.is_object() && !env.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                    result.insert("environment".into(), env.clone());
                }
            }

            result.insert("enabled".into(), json!(true));
        }
        "sse" | "http" => {
            result.insert("type".into(), json!("remote"));
            if let Some(url) = obj.get("url") {
                result.insert("url".into(), url.clone());
            }
            if let Some(headers) = obj.get("headers") {
                if headers.is_object() && !headers.as_object().map(|o| o.is_empty()).unwrap_or(true)
                {
                    result.insert("headers".into(), headers.clone());
                }
            }
            result.insert("enabled".into(), json!(true));
        }
        _ => return Err(AppError::McpValidation(format!("Unknown MCP type: {typ}"))),
    }

    Ok(Value::Object(result))
}

pub fn convert_from_opencode_format(spec: &Value) -> Result<Value, AppError> {
    let obj = spec
        .as_object()
        .ok_or_else(|| AppError::McpValidation("OpenCode MCP spec must be a JSON object".into()))?;

    let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("local");
    let mut result = serde_json::Map::new();

    match typ {
        "local" | "stdio" => {
            result.insert("type".into(), json!("stdio"));
            if let Some(cmd_arr) = obj.get("command").and_then(|v| v.as_array()) {
                if !cmd_arr.is_empty() {
                    if let Some(cmd) = cmd_arr.first().and_then(|v| v.as_str()) {
                        result.insert("command".into(), json!(cmd));
                    }
                    if cmd_arr.len() > 1 {
                        result.insert("args".into(), Value::Array(cmd_arr[1..].to_vec()));
                    }
                }
            } else if let Some(cmd) = obj.get("command").and_then(|v| v.as_str()) {
                result.insert("command".into(), json!(cmd));
                if let Some(args) = obj.get("args").and_then(|v| v.as_array()) {
                    result.insert("args".into(), Value::Array(args.clone()));
                }
            }
            if let Some(env) = obj.get("environment").or_else(|| obj.get("env")) {
                if env.is_object() && !env.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                    result.insert("env".into(), env.clone());
                }
            }
        }
        "remote" | "http" | "sse" => {
            let target_type = if typ == "http" { "http" } else { "sse" };
            result.insert("type".into(), json!(target_type));
            if let Some(url) = obj.get("url") {
                result.insert("url".into(), url.clone());
            }
            if let Some(headers) = obj.get("headers") {
                if headers.is_object() && !headers.as_object().map(|o| o.is_empty()).unwrap_or(true)
                {
                    result.insert("headers".into(), headers.clone());
                }
            }
        }
        _ => {
            return Err(AppError::McpValidation(format!(
                "Unknown OpenCode MCP type: {typ}"
            )))
        }
    }

    Ok(Value::Object(result))
}

pub fn sync_single_server_to_opencode(
    _config: &MultiAppConfig,
    id: &str,
    server_spec: &Value,
) -> Result<(), AppError> {
    if !should_sync_opencode_mcp() {
        return Ok(());
    }

    let opencode_spec = convert_to_opencode_format(server_spec)?;
    opencode_config::set_mcp_server(id, opencode_spec)
}

pub fn remove_server_from_opencode(id: &str) -> Result<(), AppError> {
    if !should_sync_opencode_mcp() {
        return Ok(());
    }
    opencode_config::remove_mcp_server(id)
}

pub fn import_mcp_map(
    config: &mut MultiAppConfig,
    mcp_map: Map<String, Value>,
) -> Result<usize, AppError> {
    if mcp_map.is_empty() {
        return Ok(0);
    }

    let servers = config.mcp.servers.get_or_insert_with(HashMap::new);
    let mut changed = 0;

    for (id, spec) in mcp_map {
        let unified_spec = match convert_from_opencode_format(&spec) {
            Ok(spec) => spec,
            Err(error) => {
                log::warn!("Skip invalid OpenCode MCP server '{id}': {error}");
                continue;
            }
        };

        if let Err(error) = validate_server_spec(&unified_spec) {
            log::warn!("Skip invalid MCP server '{id}' after conversion: {error}");
            continue;
        }

        if let Some(existing) = servers.get_mut(&id) {
            if !existing.apps.opencode {
                existing.apps.opencode = true;
                changed += 1;
            }
        } else {
            servers.insert(
                id.clone(),
                McpServer {
                    id: id.clone(),
                    name: id.clone(),
                    server: unified_spec,
                    apps: McpApps {
                        claude: false,
                        codex: false,
                        gemini: false,
                        opencode: true,
                    },
                    description: None,
                    homepage: None,
                    docs: None,
                    tags: Vec::new(),
                },
            );
            changed += 1;
        }
    }

    Ok(changed)
}

pub fn import_from_opencode(config: &mut MultiAppConfig) -> Result<usize, AppError> {
    import_mcp_map(config, opencode_config::get_mcp_servers()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn import_accepts_local_command_array() {
        let mut config = MultiAppConfig::default();
        let changed = import_mcp_map(
            &mut config,
            Map::from_iter([(
                "local-array".to_string(),
                json!({
                    "type": "local",
                    "command": ["node", "server.js", "--stdio"],
                    "environment": {
                        "API_KEY": "test"
                    }
                }),
            )]),
        )
        .expect("local command array should import");

        assert_eq!(changed, 1);
        let entry = config
            .mcp
            .servers
            .as_ref()
            .unwrap()
            .get("local-array")
            .expect("server imported");
        assert!(entry.apps.opencode);
        assert_eq!(
            entry.server,
            json!({
                "type": "stdio",
                "command": "node",
                "args": ["server.js", "--stdio"],
                "env": {
                    "API_KEY": "test"
                }
            })
        );
    }

    #[test]
    fn import_accepts_stdio_command_string_with_args() {
        let mut config = MultiAppConfig::default();
        let changed = import_mcp_map(
            &mut config,
            Map::from_iter([(
                "stdio-string".to_string(),
                json!({
                    "type": "stdio",
                    "command": "uvx",
                    "args": ["mcp-server-fetch"],
                    "env": {
                        "DEBUG": "1"
                    }
                }),
            )]),
        )
        .expect("stdio command string should import");

        assert_eq!(changed, 1);
        let entry = config
            .mcp
            .servers
            .as_ref()
            .unwrap()
            .get("stdio-string")
            .expect("server imported");
        assert!(entry.apps.opencode);
        assert_eq!(
            entry.server,
            json!({
                "type": "stdio",
                "command": "uvx",
                "args": ["mcp-server-fetch"],
                "env": {
                    "DEBUG": "1"
                }
            })
        );
    }

    #[test]
    fn import_accepts_remote_http_and_sse_configs() {
        let mut config = MultiAppConfig::default();
        let changed = import_mcp_map(
            &mut config,
            Map::from_iter([
                (
                    "remote-default".to_string(),
                    json!({
                        "type": "remote",
                        "url": "https://mcp.example.com/sse",
                        "headers": {
                            "Authorization": "Bearer token"
                        }
                    }),
                ),
                (
                    "http-explicit".to_string(),
                    json!({
                        "type": "http",
                        "url": "https://mcp.example.com/http"
                    }),
                ),
                (
                    "sse-explicit".to_string(),
                    json!({
                        "type": "sse",
                        "url": "https://mcp.example.com/events"
                    }),
                ),
            ]),
        )
        .expect("remote/http/sse configs should import");

        assert_eq!(changed, 3);
        let servers = config.mcp.servers.as_ref().unwrap();
        assert_eq!(
            servers.get("remote-default").unwrap().server,
            json!({
                "type": "sse",
                "url": "https://mcp.example.com/sse",
                "headers": {
                    "Authorization": "Bearer token"
                }
            })
        );
        assert_eq!(
            servers.get("http-explicit").unwrap().server,
            json!({
                "type": "http",
                "url": "https://mcp.example.com/http"
            })
        );
        assert_eq!(
            servers.get("sse-explicit").unwrap().server,
            json!({
                "type": "sse",
                "url": "https://mcp.example.com/events"
            })
        );
        assert!(servers.values().all(|entry| entry.apps.opencode));
    }
}
