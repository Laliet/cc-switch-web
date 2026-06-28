use serde_json::Value;
use std::collections::HashSet;

pub fn filter_private_params_with_whitelist(body: Value, whitelist: &[String]) -> Value {
    let whitelist_set: HashSet<&str> = whitelist.iter().map(String::as_str).collect();
    filter_recursive_with_whitelist(body, &mut Vec::new(), &mut Vec::new(), &whitelist_set)
}

fn filter_recursive_with_whitelist(
    value: Value,
    path: &mut Vec<String>,
    removed_keys: &mut Vec<String>,
    whitelist: &HashSet<&str>,
) -> Value {
    match value {
        Value::Object(map) => {
            let is_schema_name_map = path.last().is_some_and(|key| matches_schema_name_map(key));
            let filtered = map
                .into_iter()
                .filter_map(|(key, value)| {
                    if key.starts_with('_')
                        && !whitelist.contains(key.as_str())
                        && !is_schema_name_map
                    {
                        removed_keys.push(key);
                        None
                    } else {
                        path.push(key.clone());
                        let value =
                            filter_recursive_with_whitelist(value, path, removed_keys, whitelist);
                        path.pop();
                        Some((key, value))
                    }
                })
                .collect();

            if !removed_keys.is_empty() {
                log::debug!("[BodyFilter] filtered private params: {removed_keys:?}");
                removed_keys.clear();
            }

            Value::Object(filtered)
        }
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| filter_recursive_with_whitelist(value, path, removed_keys, whitelist))
                .collect(),
        ),
        other => other,
    }
}

fn matches_schema_name_map(key: &str) -> bool {
    matches!(
        key,
        "properties" | "patternProperties" | "definitions" | "$defs"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn body_filter_removes_private_params_recursively() {
        let output = filter_private_params_with_whitelist(
            json!({
                "model": "claude-3",
                "_internal_id": "abc123",
                "messages": [{
                    "role": "user",
                    "content": "hello",
                    "_session_token": "secret"
                }],
                "metadata": {
                    "user_id": "user-1",
                    "_tracking_id": "track-1"
                }
            }),
            &[],
        );

        assert!(output.get("_internal_id").is_none());
        assert!(output["messages"][0].get("_session_token").is_none());
        assert!(output["metadata"].get("_tracking_id").is_none());
        assert_eq!(output["metadata"]["user_id"], "user-1");
    }

    #[test]
    fn body_filter_preserves_json_schema_property_names_with_underscore() {
        let output = filter_private_params_with_whitelist(
            json!({
                "tools": [{
                    "name": "lookup",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "_id": { "type": "string", "_internal_note": "remove" },
                            "_meta": { "type": "object" }
                        },
                        "_private_schema_note": "remove"
                    }
                }]
            }),
            &[],
        );
        let schema = &output["tools"][0]["input_schema"];

        assert!(schema["properties"].get("_id").is_some());
        assert!(schema["properties"].get("_meta").is_some());
        assert!(schema["properties"]["_id"].get("_internal_note").is_none());
        assert!(schema.get("_private_schema_note").is_none());
    }
}
