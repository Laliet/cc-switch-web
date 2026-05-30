//! SQL helpers shared by usage aggregation queries.

const CACHE_INCLUSIVE_APP_TYPES: &[&str] = &["codex", "gemini", "opencode"];

pub fn fresh_input_sql(alias: &str) -> String {
    let prefix = if alias.is_empty() {
        String::new()
    } else {
        format!("{alias}.")
    };
    let app_type_list = CACHE_INCLUSIVE_APP_TYPES
        .iter()
        .map(|app| format!("'{app}'"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "CASE WHEN {prefix}app_type IN ({app_type_list}) AND {prefix}input_tokens >= {prefix}cache_read_tokens \
              THEN ({prefix}input_tokens - {prefix}cache_read_tokens) \
              ELSE {prefix}input_tokens END"
    )
}
