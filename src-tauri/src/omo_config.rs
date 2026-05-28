use crate::config::write_json_file;
use crate::error::AppError;
use crate::opencode_config::get_opencode_dir;
use serde_json::Value;
use std::path::PathBuf;

const OMO_CONFIG_JSONC: &str = "oh-my-opencode.jsonc";
const OMO_CONFIG_JSON: &str = "oh-my-opencode.json";
const OPENAGENT_CONFIG_JSONC: &str = "oh-my-openagent.jsonc";
const OPENAGENT_CONFIG_JSON: &str = "oh-my-openagent.json";
const OMO_SLIM_CONFIG_JSONC: &str = "oh-my-opencode-slim.jsonc";
const OMO_SLIM_CONFIG_JSON: &str = "oh-my-opencode-slim.json";

pub fn get_omo_dir() -> PathBuf {
    get_opencode_dir()
}

pub fn get_omo_config_path() -> PathBuf {
    get_omo_dir().join(OPENAGENT_CONFIG_JSONC)
}

pub fn resolve_omo_config_path() -> PathBuf {
    for file_name in [
        OPENAGENT_CONFIG_JSONC,
        OPENAGENT_CONFIG_JSON,
        OMO_CONFIG_JSONC,
        OMO_CONFIG_JSON,
    ] {
        let path = get_omo_dir().join(file_name);
        if path.exists() {
            return path;
        }
    }

    get_omo_config_path()
}

pub fn get_omo_slim_config_path() -> PathBuf {
    get_omo_dir().join(OMO_SLIM_CONFIG_JSONC)
}

pub fn resolve_omo_slim_config_path() -> PathBuf {
    for file_name in [OMO_SLIM_CONFIG_JSONC, OMO_SLIM_CONFIG_JSON] {
        let path = get_omo_dir().join(file_name);
        if path.exists() {
            return path;
        }
    }

    get_omo_slim_config_path()
}

pub fn read_omo_config() -> Result<Value, AppError> {
    let path = resolve_omo_config_path();
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }

    let content = std::fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    let cleaned = strip_jsonc_comments(&content);
    serde_json::from_str(&cleaned).map_err(|e| AppError::json(&path, e))
}

pub fn write_omo_config(config: &Value) -> Result<(), AppError> {
    let path = get_omo_config_path();
    write_json_file(&path, config)?;
    Ok(())
}

pub fn read_omo_slim_config() -> Result<Value, AppError> {
    let path = resolve_omo_slim_config_path();
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }

    let content = std::fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    let cleaned = strip_jsonc_comments(&content);
    serde_json::from_str(&cleaned).map_err(|e| AppError::json(&path, e))
}

pub fn write_omo_slim_config(config: &Value) -> Result<(), AppError> {
    let path = get_omo_slim_config_path();
    write_json_file(&path, config)?;
    Ok(())
}

fn strip_jsonc_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;

    while let Some(&ch) = chars.peek() {
        if in_string {
            result.push(ch);
            chars.next();
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            result.push(ch);
            chars.next();
            continue;
        }

        if ch == '/' {
            chars.next();
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    while let Some(&next) = chars.peek() {
                        if next == '\n' {
                            break;
                        }
                        chars.next();
                    }
                }
                Some('*') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '*' && matches!(chars.peek(), Some('/')) {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => result.push('/'),
            }
            continue;
        }

        result.push(ch);
        chars.next();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_jsonc_comments_preserves_string_contents() {
        let input = r#"{
  // comment
  "name": "oh-my-opencode",
  "url": "https://example.com//keep"
}"#;

        let cleaned = strip_jsonc_comments(input);
        let parsed: Value = serde_json::from_str(&cleaned).expect("json should parse");
        assert_eq!(parsed["name"], "oh-my-opencode");
        assert_eq!(parsed["url"], "https://example.com//keep");
    }

    #[test]
    fn resolve_omo_config_path_returns_supported_filename() {
        let path = resolve_omo_config_path();
        let file_name = path.file_name().and_then(|value| value.to_str());
        assert!(matches!(
            file_name,
            Some(
                OPENAGENT_CONFIG_JSONC | OPENAGENT_CONFIG_JSON | OMO_CONFIG_JSONC | OMO_CONFIG_JSON
            )
        ));
    }

    #[test]
    fn default_write_paths_use_openagent_and_slim_names() {
        assert_eq!(
            get_omo_config_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some(OPENAGENT_CONFIG_JSONC)
        );
        assert_eq!(
            get_omo_slim_config_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some(OMO_SLIM_CONFIG_JSONC)
        );
    }
}
