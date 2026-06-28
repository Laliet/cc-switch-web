use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
enum GeminiFunctionParameters {
    Schema(Value),
    JsonSchema(Value),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnthropicToolSchemaHint {
    expected_keys: Vec<String>,
    required_keys: Vec<String>,
}

pub type AnthropicToolSchemaHints = HashMap<String, AnthropicToolSchemaHint>;

pub fn build_gemini_function_declaration(
    name: &str,
    description: Option<&str>,
    input_schema: Value,
) -> Value {
    let mut declaration = Map::new();
    declaration.insert("name".to_string(), json!(name));
    declaration.insert("description".to_string(), json!(description.unwrap_or("")));

    match build_gemini_function_parameters(input_schema) {
        GeminiFunctionParameters::Schema(schema) => {
            declaration.insert("parameters".to_string(), schema);
        }
        GeminiFunctionParameters::JsonSchema(schema) => {
            declaration.insert("parametersJsonSchema".to_string(), schema);
        }
    }

    Value::Object(declaration)
}

fn build_gemini_function_parameters(input_schema: Value) -> GeminiFunctionParameters {
    let schema = ensure_object_schema(normalize_json_schema(input_schema));

    if requires_parameters_json_schema(&schema) {
        GeminiFunctionParameters::JsonSchema(schema)
    } else {
        GeminiFunctionParameters::Schema(to_gemini_schema(schema))
    }
}

fn ensure_object_schema(schema: Value) -> Value {
    match schema {
        Value::Object(mut obj) => {
            obj.entry("type".to_string())
                .or_insert_with(|| json!("object"));
            if obj.get("type").and_then(Value::as_str) == Some("object") {
                obj.entry("properties".to_string())
                    .or_insert_with(|| json!({}));
            }
            Value::Object(obj)
        }
        other => other,
    }
}

fn normalize_json_schema(schema: Value) -> Value {
    match schema {
        Value::Object(mut obj) => {
            obj.remove("$schema");
            obj.remove("$id");

            if let Some(properties) = obj.get_mut("properties").and_then(Value::as_object_mut) {
                for value in properties.values_mut() {
                    *value = normalize_json_schema(value.clone());
                }
            }

            if let Some(items) = obj.get_mut("items") {
                *items = normalize_json_schema(items.clone());
            }

            for key in ["anyOf", "oneOf", "allOf", "prefixItems"] {
                if let Some(values) = obj.get_mut(key).and_then(Value::as_array_mut) {
                    for value in values.iter_mut() {
                        *value = normalize_json_schema(value.clone());
                    }
                }
            }

            for key in ["not", "if", "then", "else", "additionalProperties"] {
                if let Some(value) = obj.get_mut(key) {
                    *value = normalize_json_schema(value.clone());
                }
            }

            Value::Object(obj)
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().map(normalize_json_schema).collect())
        }
        other => other,
    }
}

fn requires_parameters_json_schema(schema: &Value) -> bool {
    match schema {
        Value::Object(obj) => object_requires_parameters_json_schema(obj),
        Value::Array(values) => values.iter().any(requires_parameters_json_schema),
        _ => false,
    }
}

fn object_requires_parameters_json_schema(obj: &Map<String, Value>) -> bool {
    for (key, value) in obj {
        match key.as_str() {
            "type" => {
                if value.is_array() {
                    return true;
                }
            }
            "format" | "title" | "description" | "nullable" | "enum" | "maxItems" | "minItems"
            | "required" | "minProperties" | "maxProperties" | "minLength" | "maxLength"
            | "pattern" | "example" | "propertyOrdering" | "default" | "minimum" | "maximum" => {}
            "properties" => {
                let Some(properties) = value.as_object() else {
                    return true;
                };
                if properties.values().any(requires_parameters_json_schema) {
                    return true;
                }
            }
            "items" => {
                if !value.is_object() || requires_parameters_json_schema(value) {
                    return true;
                }
            }
            "anyOf" => {
                let Some(values) = value.as_array() else {
                    return true;
                };
                if values.iter().any(requires_parameters_json_schema) {
                    return true;
                }
            }
            "$ref"
            | "$defs"
            | "definitions"
            | "additionalProperties"
            | "unevaluatedProperties"
            | "patternProperties"
            | "oneOf"
            | "allOf"
            | "const"
            | "not"
            | "if"
            | "then"
            | "else"
            | "dependentRequired"
            | "dependentSchemas"
            | "contains"
            | "minContains"
            | "maxContains"
            | "prefixItems"
            | "exclusiveMinimum"
            | "exclusiveMaximum"
            | "multipleOf"
            | "examples" => return true,
            _ => return true,
        }
    }

    false
}

fn to_gemini_schema(schema: Value) -> Value {
    match schema {
        Value::Object(obj) => {
            let mut result = Map::new();

            for (key, value) in obj {
                match key.as_str() {
                    "type" | "format" | "title" | "description" | "nullable" | "enum"
                    | "maxItems" | "minItems" | "required" | "minProperties" | "maxProperties"
                    | "minLength" | "maxLength" | "pattern" | "example" | "propertyOrdering"
                    | "default" | "minimum" | "maximum" => {
                        result.insert(key, value);
                    }
                    "properties" => {
                        if let Some(properties) = value.as_object() {
                            let converted = properties
                                .iter()
                                .map(|(name, property_schema)| {
                                    (name.clone(), to_gemini_schema(property_schema.clone()))
                                })
                                .collect();
                            result.insert("properties".to_string(), Value::Object(converted));
                        }
                    }
                    "items" if value.is_object() => {
                        result.insert("items".to_string(), to_gemini_schema(value));
                    }
                    "anyOf" => {
                        if let Some(values) = value.as_array() {
                            result.insert(
                                "anyOf".to_string(),
                                Value::Array(
                                    values
                                        .iter()
                                        .map(|value| to_gemini_schema(value.clone()))
                                        .collect(),
                                ),
                            );
                        }
                    }
                    _ => {}
                }
            }

            Value::Object(result)
        }
        other => other,
    }
}

pub fn extract_anthropic_tool_schema_hints(body: &Value) -> AnthropicToolSchemaHints {
    body.get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| {
            let name = tool.get("name").and_then(Value::as_str)?;
            let input_schema = tool.get("input_schema").and_then(Value::as_object)?;
            let properties = input_schema.get("properties").and_then(Value::as_object)?;
            if properties.is_empty() {
                return None;
            }

            let expected_keys = properties.keys().cloned().collect::<Vec<_>>();
            let required_keys = input_schema
                .get("required")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(ToString::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            Some((
                name.to_string(),
                AnthropicToolSchemaHint {
                    expected_keys,
                    required_keys,
                },
            ))
        })
        .collect()
}

pub fn rectify_gemini_tool_call_parts(
    parts: &mut [Value],
    tool_schema_hints: Option<&AnthropicToolSchemaHints>,
) {
    for part in parts {
        let Some(function_call) = part.get_mut("functionCall").and_then(Value::as_object_mut)
        else {
            continue;
        };
        let Some(name) = function_call
            .get("name")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        else {
            continue;
        };
        let Some(args) = function_call.get_mut("args") else {
            continue;
        };

        if rectify_gemini_tool_call_args(&name, args, tool_schema_hints) {
            log::info!("[Claude/Gemini] Rectified tool args for `{name}`");
        }
    }
}

pub fn rectify_gemini_tool_call_args(
    tool_name: &str,
    args: &mut Value,
    tool_schema_hints: Option<&AnthropicToolSchemaHints>,
) -> bool {
    let Some(tool_schema_hints) = tool_schema_hints else {
        return false;
    };
    let Some(hint) = tool_schema_hints.get(tool_name) else {
        return false;
    };
    let Some(args_object) = args.as_object_mut() else {
        return false;
    };
    if args_object.is_empty() || hint.expected_keys.is_empty() {
        return false;
    }
    let mut changed = false;

    if hint.expected_keys.iter().any(|key| key == "skill") && !args_object.contains_key("skill") {
        if let Some(value) = args_object.remove("name") {
            args_object.insert("skill".to_string(), value);
            changed = true;
        }
    }

    let expects_parameters_key = hint.expected_keys.iter().any(|key| key == "parameters");
    if !expects_parameters_key {
        let extracted_parameters = args_object
            .get("parameters")
            .and_then(Value::as_object)
            .map(|parameters_object| {
                hint.expected_keys
                    .iter()
                    .filter_map(|expected_key| {
                        if args_object.contains_key(expected_key) {
                            return None;
                        }
                        let value = parameters_object.get(expected_key)?;
                        let normalized_value = match value {
                            Value::Array(values) if values.len() == 1 => values[0].clone(),
                            _ => value.clone(),
                        };
                        Some((expected_key.clone(), normalized_value))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if !extracted_parameters.is_empty() {
            for (expected_key, normalized_value) in extracted_parameters {
                args_object.insert(expected_key, normalized_value);
            }
            args_object.remove("parameters");
            changed = true;
        }
    }

    if hint
        .required_keys
        .iter()
        .all(|key| args_object.contains_key(key.as_str()))
    {
        return changed;
    }

    let expected_key_set = hint
        .expected_keys
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let unexpected_keys = args_object
        .keys()
        .filter(|key| !expected_key_set.contains(key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if unexpected_keys.len() != 1 {
        return false;
    }

    let target_key = hint
        .required_keys
        .iter()
        .find(|key| !args_object.contains_key(key.as_str()))
        .cloned()
        .or_else(|| {
            if hint.expected_keys.len() == 1 && args_object.len() == 1 {
                hint.expected_keys.first().cloned()
            } else {
                None
            }
        });
    let Some(target_key) = target_key else {
        return false;
    };
    if args_object.contains_key(&target_key) {
        return false;
    }

    let source_key = &unexpected_keys[0];
    let Some(value) = args_object.remove(source_key) else {
        return false;
    };
    args_object.insert(target_key, value);
    true
}
