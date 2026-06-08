use crate::{
    database::{lock_conn, Database, ModelPricing, ModelPricingRecord},
    error::AppError,
    services::sql_helpers::fresh_input_sql,
};
use chrono::{Datelike, Local, TimeZone};
use rusqlite::{params, Connection, OptionalExtension, ToSql};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};

const SESSION_PROXY_DEDUP_WINDOW_MILLIS: i64 = 10 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub total_requests: u64,
    pub total_cost: String,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub success_rate: f32,
    pub real_total_tokens: u64,
    pub cache_hit_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummaryByApp {
    pub app_type: String,
    pub summary: UsageSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyStats {
    pub date: String,
    pub request_count: u64,
    pub total_cost: String,
    pub total_tokens: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStats {
    pub provider_id: String,
    pub provider_name: String,
    pub app_type: String,
    pub request_count: u64,
    pub total_tokens: u64,
    pub total_cost: String,
    pub success_rate: f32,
    pub avg_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStats {
    pub model: String,
    pub request_count: u64,
    pub total_tokens: u64,
    pub total_cost: String,
    pub avg_cost_per_request: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFilters {
    pub app_type: Option<String>,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub model: Option<String>,
    pub status_code: Option<u16>,
    pub start_date: Option<i64>,
    pub end_date: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStatsFilters {
    pub app_type: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
}

impl UsageStatsFilters {
    fn from_app_type(app_type: Option<&str>) -> Self {
        Self {
            app_type: app_type.map(str::to_string),
            provider_id: None,
            model: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedLogs {
    pub data: Vec<RequestLogDetail>,
    pub total: u32,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogDetail {
    pub request_id: String,
    pub provider_id: String,
    pub provider_name: Option<String>,
    pub app_type: String,
    pub model: String,
    pub request_model: Option<String>,
    pub cost_multiplier: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    pub input_cost_usd: String,
    pub output_cost_usd: String,
    pub cache_read_cost_usd: String,
    pub cache_creation_cost_usd: String,
    pub total_cost_usd: String,
    pub is_streaming: bool,
    pub latency_ms: u64,
    pub first_token_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub status_code: u16,
    pub error_message: Option<String>,
    pub session_id: Option<String>,
    pub provider_type: Option<String>,
    pub created_at: i64,
    pub data_source: Option<String>,
    pub is_unpriced: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSourceSummary {
    pub data_source: String,
    pub request_count: u64,
    pub total_cost_usd: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageDataExtent {
    pub first_seen_at: Option<i64>,
    pub last_seen_at: Option<i64>,
    pub request_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderLimitStatus {
    pub provider_id: String,
    pub app_type: String,
    pub daily_usage: String,
    pub daily_limit: Option<String>,
    pub daily_exceeded: bool,
    pub monthly_usage: String,
    pub monthly_limit: Option<String>,
    pub monthly_exceeded: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSyncResult {
    pub imported: u64,
    pub skipped: u64,
    pub files_scanned: u64,
    pub errors: Vec<String>,
}

fn derive_real_total_and_hit_rate(
    fresh_input: u64,
    output: u64,
    cache_creation: u64,
    cache_read: u64,
) -> (u64, f64) {
    let real_total = fresh_input + output + cache_creation + cache_read;
    let cacheable_input = fresh_input + cache_creation + cache_read;
    let hit_rate = if cacheable_input > 0 {
        cache_read as f64 / cacheable_input as f64
    } else {
        0.0
    };
    (real_total, hit_rate)
}

fn validate_usage_app(app_type: Option<&str>) -> Result<(), AppError> {
    if let Some(app_type) = app_type {
        match app_type {
            "claude" | "claude-desktop" | "codex" | "gemini" | "opencode" => Ok(()),
            _ => Err(AppError::InvalidInput(format!(
                "Unsupported usage app type: {app_type}"
            ))),
        }
    } else {
        Ok(())
    }
}

fn provider_name_expr(log_alias: &str, provider_alias: &str) -> String {
    format!("COALESCE({provider_alias}.name, {log_alias}.provider_id)")
}

fn row_to_request_log_detail(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestLogDetail> {
    let input_tokens = row.get::<_, i64>(7)?.max(0) as u32;
    let output_tokens = row.get::<_, i64>(8)?.max(0) as u32;
    let cache_read_tokens = row.get::<_, i64>(9)?.max(0) as u32;
    let cache_creation_tokens = row.get::<_, i64>(10)?.max(0) as u32;
    let total_cost_usd: String = row.get(15)?;
    let status_code = row.get::<_, i64>(20)?.max(0) as u16;
    let cost_multiplier = row
        .get::<_, Option<String>>(6)?
        .unwrap_or_else(|| "1".to_string());
    let has_tokens =
        input_tokens > 0 || output_tokens > 0 || cache_read_tokens > 0 || cache_creation_tokens > 0;
    let total_cost = total_cost_usd.parse::<f64>().unwrap_or(0.0);
    let multiplier = cost_multiplier.parse::<f64>().unwrap_or(1.0);

    Ok(RequestLogDetail {
        request_id: row.get(0)?,
        provider_id: row.get(1)?,
        provider_name: row.get(2)?,
        app_type: row.get(3)?,
        model: row.get(4)?,
        request_model: row.get(5)?,
        cost_multiplier,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        input_cost_usd: row.get(11)?,
        output_cost_usd: row.get(12)?,
        cache_read_cost_usd: row.get(13)?,
        cache_creation_cost_usd: row.get(14)?,
        total_cost_usd,
        is_streaming: row.get::<_, i64>(16)? != 0,
        latency_ms: row.get::<_, i64>(17)?.max(0) as u64,
        first_token_ms: row
            .get::<_, Option<i64>>(18)?
            .map(|value| value.max(0) as u64),
        duration_ms: row
            .get::<_, Option<i64>>(19)?
            .map(|value| value.max(0) as u64),
        status_code,
        error_message: row.get(21)?,
        session_id: row.get(22)?,
        provider_type: row.get(23)?,
        created_at: row.get(24)?,
        data_source: row.get(25)?,
        is_unpriced: (200..300).contains(&status_code)
            && has_tokens
            && multiplier != 0.0
            && total_cost == 0.0,
    })
}

fn make_summary(
    total_requests: i64,
    total_cost: f64,
    total_input_tokens: i64,
    total_output_tokens: i64,
    total_cache_creation_tokens: i64,
    total_cache_read_tokens: i64,
    success_count: i64,
) -> UsageSummary {
    let success_rate = if total_requests > 0 {
        (success_count as f32 / total_requests as f32) * 100.0
    } else {
        0.0
    };
    let (real_total_tokens, cache_hit_rate) = derive_real_total_and_hit_rate(
        total_input_tokens.max(0) as u64,
        total_output_tokens.max(0) as u64,
        total_cache_creation_tokens.max(0) as u64,
        total_cache_read_tokens.max(0) as u64,
    );

    UsageSummary {
        total_requests: total_requests.max(0) as u64,
        total_cost: format!("{total_cost:.6}"),
        total_input_tokens: total_input_tokens.max(0) as u64,
        total_output_tokens: total_output_tokens.max(0) as u64,
        total_cache_creation_tokens: total_cache_creation_tokens.max(0) as u64,
        total_cache_read_tokens: total_cache_read_tokens.max(0) as u64,
        success_rate,
        real_total_tokens,
        cache_hit_rate,
    }
}

fn push_common_log_filters(
    conditions: &mut Vec<String>,
    params_vec: &mut Vec<Box<dyn ToSql>>,
    alias: &str,
    start_date: Option<i64>,
    end_date: Option<i64>,
    filters: UsageStatsFiltersRef<'_>,
) {
    if let Some(start) = start_date {
        conditions.push(format!("{alias}.created_at >= ?"));
        params_vec.push(Box::new(start));
    }
    if let Some(end) = end_date {
        conditions.push(format!("{alias}.created_at <= ?"));
        params_vec.push(Box::new(end));
    }
    if let Some(app) = filters.app_type {
        conditions.push(format!("{alias}.app_type = ?"));
        params_vec.push(Box::new(app.to_string()));
    }
    if let Some(provider_id) = filters.provider_id {
        conditions.push(format!("{alias}.provider_id = ?"));
        params_vec.push(Box::new(provider_id.to_string()));
    }
    if let Some(model) = filters.model {
        conditions.push(format!("{alias}.model = ?"));
        params_vec.push(Box::new(model.to_string()));
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct UsageStatsFiltersRef<'a> {
    app_type: Option<&'a str>,
    provider_id: Option<&'a str>,
    model: Option<&'a str>,
}

impl<'a> UsageStatsFiltersRef<'a> {
    fn from_parts(
        app_type: Option<&'a str>,
        provider_id: Option<&'a str>,
        model: Option<&'a str>,
    ) -> Self {
        Self {
            app_type: trim_optional_filter(app_type),
            provider_id: trim_optional_filter(provider_id),
            model: trim_optional_filter(model),
        }
    }
}

impl<'a> From<&'a UsageStatsFilters> for UsageStatsFiltersRef<'a> {
    fn from(filters: &'a UsageStatsFilters) -> Self {
        Self::from_parts(
            filters.app_type.as_deref(),
            filters.provider_id.as_deref(),
            filters.model.as_deref(),
        )
    }
}

fn trim_optional_filter(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn rollup_date_from_millis(ts: i64) -> Result<String, AppError> {
    Local
        .timestamp_millis_opt(ts)
        .single()
        .ok_or_else(|| AppError::Database(format!("Invalid timestamp: {ts}")))
        .map(|dt| dt.format("%Y-%m-%d").to_string())
}

fn push_rollup_filters(
    conditions: &mut Vec<String>,
    params_vec: &mut Vec<Box<dyn ToSql>>,
    alias: &str,
    start_date: Option<i64>,
    end_date: Option<i64>,
    filters: UsageStatsFiltersRef<'_>,
) -> Result<(), AppError> {
    if let Some(start) = start_date {
        conditions.push(format!("{alias}.date >= ?"));
        params_vec.push(Box::new(rollup_date_from_millis(start)?));
    }
    if let Some(end) = end_date {
        conditions.push(format!("{alias}.date <= ?"));
        params_vec.push(Box::new(rollup_date_from_millis(end)?));
    }
    if let Some(app) = filters.app_type {
        conditions.push(format!("{alias}.app_type = ?"));
        params_vec.push(Box::new(app.to_string()));
    }
    if let Some(provider_id) = filters.provider_id {
        conditions.push(format!("{alias}.provider_id = ?"));
        params_vec.push(Box::new(provider_id.to_string()));
    }
    if let Some(model) = filters.model {
        conditions.push(format!("{alias}.model = ?"));
        params_vec.push(Box::new(model.to_string()));
    }
    Ok(())
}

fn where_clause(conditions: &[String]) -> String {
    if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    }
}

fn params_refs(params: &[Box<dyn ToSql>]) -> Vec<&dyn ToSql> {
    params.iter().map(|param| param.as_ref()).collect()
}

fn parse_multiplier(value: &str) -> Decimal {
    Decimal::from_str(value).unwrap_or(Decimal::ONE)
}

fn calculate_cost_strings(
    app_type: &str,
    log: &RequestLogDetail,
    pricing: &ModelPricing,
) -> [String; 5] {
    let million = Decimal::from(1_000_000);
    let input_tokens = if matches!(app_type, "codex" | "gemini" | "opencode")
        && log.input_tokens >= log.cache_read_tokens
    {
        log.input_tokens - log.cache_read_tokens
    } else {
        log.input_tokens
    };
    let input_cost = Decimal::from(input_tokens) * pricing.input_cost_per_million / million;
    let output_cost = Decimal::from(log.output_tokens) * pricing.output_cost_per_million / million;
    let cache_read_cost =
        Decimal::from(log.cache_read_tokens) * pricing.cache_read_cost_per_million / million;
    let cache_creation_cost = Decimal::from(log.cache_creation_tokens)
        * pricing.cache_creation_cost_per_million
        / million;
    let total_cost = (input_cost + output_cost + cache_read_cost + cache_creation_cost)
        * parse_multiplier(&log.cost_multiplier);
    [
        input_cost.to_string(),
        output_cost.to_string(),
        cache_read_cost.to_string(),
        cache_creation_cost.to_string(),
        total_cost.to_string(),
    ]
}

#[derive(Debug, Clone, Copy)]
struct SessionUsageForCost<'a> {
    app_type: &'a str,
    model: &'a str,
    input_tokens: u32,
    output_tokens: u32,
    cache_read_tokens: u32,
    cache_creation_tokens: u32,
}

fn calculate_session_cost_strings(
    usage: &SessionUsageForCost<'_>,
    pricing: &ModelPricing,
) -> [String; 5] {
    let log = RequestLogDetail {
        request_id: String::new(),
        provider_id: String::new(),
        provider_name: None,
        app_type: usage.app_type.to_string(),
        model: usage.model.to_string(),
        request_model: Some(usage.model.to_string()),
        cost_multiplier: "1.0".to_string(),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        cache_creation_tokens: usage.cache_creation_tokens,
        input_cost_usd: "0".to_string(),
        output_cost_usd: "0".to_string(),
        cache_read_cost_usd: "0".to_string(),
        cache_creation_cost_usd: "0".to_string(),
        total_cost_usd: "0".to_string(),
        is_streaming: true,
        latency_ms: 0,
        first_token_ms: None,
        duration_ms: None,
        status_code: 200,
        error_message: None,
        session_id: None,
        provider_type: None,
        created_at: 0,
        data_source: None,
        is_unpriced: false,
    };
    calculate_cost_strings(usage.app_type, &log, pricing)
}

fn parse_rfc3339_millis(timestamp: Option<&str>) -> i64 {
    timestamp
        .and_then(|ts| {
            chrono::DateTime::parse_from_rfc3339(ts)
                .ok()
                .map(|dt| dt.timestamp_millis())
        })
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
                .unwrap_or(0)
        })
}

fn metadata_modified_nanos(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn get_sync_state(conn: &Connection, file_path: &str) -> Result<(i64, i64), AppError> {
    conn.query_row(
        "SELECT last_modified, last_line_offset FROM session_log_sync WHERE file_path = ?1",
        params![file_path],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map(|row| row.unwrap_or((0, 0)))
    .map_err(|err| AppError::Database(err.to_string()))
}

fn update_sync_state(
    conn: &Connection,
    file_path: &str,
    last_modified: i64,
    last_offset: i64,
) -> Result<(), AppError> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    conn.execute(
        "INSERT OR REPLACE INTO session_log_sync (
            file_path, last_modified, last_line_offset, last_synced_at
         ) VALUES (?1, ?2, ?3, ?4)",
        params![file_path, last_modified, last_offset, now],
    )
    .map_err(|err| AppError::Database(err.to_string()))?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct DedupKey<'a> {
    app_type: &'a str,
    model: &'a str,
    input_tokens: u32,
    output_tokens: u32,
    cache_read_tokens: u32,
    cache_creation_tokens: u32,
    created_at: i64,
}

fn should_skip_session_insert(
    conn: &Connection,
    request_id: &str,
    key: &DedupKey<'_>,
) -> Result<bool, AppError> {
    let exists = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM proxy_request_logs WHERE request_id = ?1)",
            params![request_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|err| AppError::Database(err.to_string()))?;
    if exists {
        return Ok(true);
    }

    let allow_missing_cache_creation =
        matches!(key.app_type, "codex" | "gemini") && key.cache_creation_tokens == 0;
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM proxy_request_logs l
            WHERE COALESCE(l.data_source, 'proxy') = 'proxy'
              AND l.app_type = ?1
              AND l.status_code >= 200 AND l.status_code < 300
              AND l.input_tokens = ?3
              AND l.output_tokens = ?4
              AND l.cache_read_tokens = ?5
              AND (l.cache_creation_tokens = ?6 OR ?9 = 1)
              AND l.created_at BETWEEN ?7 - ?8 AND ?7 + ?8
              AND (
                    LOWER(l.model) = LOWER(?2)
                    OR LOWER(l.model) = 'unknown'
                    OR LOWER(?2) = 'unknown'
              )
        )",
        params![
            key.app_type,
            key.model,
            key.input_tokens as i64,
            key.output_tokens as i64,
            key.cache_read_tokens as i64,
            key.cache_creation_tokens as i64,
            key.created_at,
            SESSION_PROXY_DEDUP_WINDOW_MILLIS,
            allow_missing_cache_creation as i64,
        ],
        |row| row.get::<_, bool>(0),
    )
    .map_err(|err| AppError::Database(err.to_string()))
}

fn collect_jsonl_recursive(dir: &Path, files: &mut Vec<PathBuf>, depth: u32, max_depth: u32) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && depth < max_depth {
            collect_jsonl_recursive(&path, files, depth + 1, max_depth);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }
}

fn pricing_for_model_on_conn(
    conn: &Connection,
    model: &str,
) -> Result<Option<ModelPricing>, AppError> {
    if let Some(pricing) = conn
        .query_row(
            "SELECT input_cost_per_million, output_cost_per_million,
                    cache_read_cost_per_million, cache_creation_cost_per_million
             FROM model_pricing
             WHERE LOWER(model_id) = LOWER(?1)",
            params![model],
            |row| {
                let input: String = row.get(0)?;
                let output: String = row.get(1)?;
                let cache_read: String = row.get(2)?;
                let cache_creation: String = row.get(3)?;
                Ok((input, output, cache_read, cache_creation))
            },
        )
        .optional()
        .map_err(|err| AppError::Database(err.to_string()))?
    {
        return ModelPricing::from_strings(&pricing.0, &pricing.1, &pricing.2, &pricing.3)
            .map(Some)
            .map_err(|err| AppError::Database(format!("Failed to parse model pricing: {err}")));
    }

    let mut stmt = conn
        .prepare(
            "SELECT model_id, input_cost_per_million, output_cost_per_million,
                    cache_read_cost_per_million, cache_creation_cost_per_million
             FROM model_pricing
             ORDER BY LENGTH(model_id) DESC",
        )
        .map_err(|err| AppError::Database(err.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|err| AppError::Database(err.to_string()))?;
    for row in rows {
        let (model_id, input, output, cache_read, cache_creation) =
            row.map_err(|err| AppError::Database(err.to_string()))?;
        if model_matches_pricing_key(model, &model_id) {
            return ModelPricing::from_strings(&input, &output, &cache_read, &cache_creation)
                .map(Some)
                .map_err(|err| {
                    AppError::Database(format!("Failed to parse model pricing: {err}"))
                });
        }
    }
    Ok(None)
}

#[derive(Debug, Clone)]
struct SessionLogEntry {
    request_id: String,
    provider_id: &'static str,
    app_type: &'static str,
    model: String,
    input_tokens: u32,
    output_tokens: u32,
    cache_read_tokens: u32,
    cache_creation_tokens: u32,
    session_id: Option<String>,
    provider_type: &'static str,
    data_source: &'static str,
    created_at: i64,
}

fn insert_session_log_entry(conn: &Connection, entry: &SessionLogEntry) -> Result<bool, AppError> {
    let dedup_key = DedupKey {
        app_type: entry.app_type,
        model: &entry.model,
        input_tokens: entry.input_tokens,
        output_tokens: entry.output_tokens,
        cache_read_tokens: entry.cache_read_tokens,
        cache_creation_tokens: entry.cache_creation_tokens,
        created_at: entry.created_at,
    };
    if should_skip_session_insert(conn, &entry.request_id, &dedup_key)? {
        return Ok(false);
    }

    let costs = match pricing_for_model_on_conn(conn, &entry.model)? {
        Some(pricing) => calculate_session_cost_strings(
            &SessionUsageForCost {
                app_type: entry.app_type,
                model: &entry.model,
                input_tokens: entry.input_tokens,
                output_tokens: entry.output_tokens,
                cache_read_tokens: entry.cache_read_tokens,
                cache_creation_tokens: entry.cache_creation_tokens,
            },
            &pricing,
        ),
        None => [
            "0".to_string(),
            "0".to_string(),
            "0".to_string(),
            "0".to_string(),
            "0".to_string(),
        ],
    };

    conn.execute(
        "INSERT OR IGNORE INTO proxy_request_logs (
            request_id, provider_id, app_type, model, request_model,
            input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
            input_cost_usd, output_cost_usd, cache_read_cost_usd,
            cache_creation_cost_usd, total_cost_usd,
            latency_ms, first_token_ms, duration_ms, status_code, error_message,
            session_id, provider_type, is_streaming, cost_multiplier, created_at, data_source
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
            ?21, ?22, ?23, ?24, ?25
         )",
        params![
            entry.request_id,
            entry.provider_id,
            entry.app_type,
            entry.model,
            entry.model,
            entry.input_tokens as i64,
            entry.output_tokens as i64,
            entry.cache_read_tokens as i64,
            entry.cache_creation_tokens as i64,
            costs[0],
            costs[1],
            costs[2],
            costs[3],
            costs[4],
            0i64,
            Option::<i64>::None,
            Option::<i64>::None,
            200i64,
            Option::<String>::None,
            entry.session_id,
            entry.provider_type,
            1i64,
            "1.0",
            entry.created_at,
            entry.data_source,
        ],
    )
    .map_err(|err| AppError::Database(err.to_string()))?;
    Ok(conn.changes() > 0)
}

#[derive(Debug)]
struct ParsedClaudeUsage {
    message_id: String,
    model: String,
    input_tokens: u32,
    output_tokens: u32,
    cache_read_tokens: u32,
    cache_creation_tokens: u32,
    stop_reason: Option<String>,
    timestamp: Option<String>,
    session_id: Option<String>,
}

fn collect_claude_session_files(projects_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = match fs::read_dir(projects_dir) {
        Ok(entries) => entries,
        Err(_) => return files,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Ok(sub_entries) = fs::read_dir(path) {
            for sub_entry in sub_entries.flatten() {
                let sub_path = sub_entry.path();
                if sub_path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                    files.push(sub_path);
                }
            }
        }
    }
    files
}

fn sync_single_claude_file(conn: &Connection, file_path: &Path) -> Result<(u64, u64), AppError> {
    let file_path_str = file_path.to_string_lossy().to_string();
    let metadata = fs::metadata(file_path).map_err(|err| AppError::io(file_path, err))?;
    let file_modified = metadata_modified_nanos(&metadata);
    let (last_modified, last_offset) = get_sync_state(conn, &file_path_str)?;
    if file_modified <= last_modified {
        return Ok((0, 0));
    }

    let file = fs::File::open(file_path).map_err(|err| AppError::io(file_path, err))?;
    let reader = BufReader::new(file);
    let mut line_offset = 0i64;
    let mut messages: HashMap<String, ParsedClaudeUsage> = HashMap::new();
    let mut current_session_id: Option<String> = None;

    for line_result in reader.lines() {
        line_offset += 1;
        let line = match line_result {
            Ok(line) => line,
            Err(_) => continue,
        };
        if line.trim().is_empty() || !line.contains("\"assistant\"") {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if current_session_id.is_none() {
            current_session_id = value
                .get("sessionId")
                .and_then(|value| value.as_str())
                .map(ToString::to_string);
        }
        if line_offset <= last_offset {
            continue;
        }
        if value.get("type").and_then(|value| value.as_str()) != Some("assistant") {
            continue;
        }
        let Some(message) = value.get("message") else {
            continue;
        };
        let Some(message_id) = message.get("id").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(usage) = message.get("usage") else {
            continue;
        };
        let parsed = ParsedClaudeUsage {
            message_id: message_id.to_string(),
            model: message
                .get("model")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .to_string(),
            input_tokens: usage
                .get("input_tokens")
                .and_then(|value| value.as_u64())
                .unwrap_or(0) as u32,
            output_tokens: usage
                .get("output_tokens")
                .and_then(|value| value.as_u64())
                .unwrap_or(0) as u32,
            cache_read_tokens: usage
                .get("cache_read_input_tokens")
                .and_then(|value| value.as_u64())
                .unwrap_or(0) as u32,
            cache_creation_tokens: usage
                .get("cache_creation_input_tokens")
                .and_then(|value| value.as_u64())
                .unwrap_or(0) as u32,
            stop_reason: message
                .get("stop_reason")
                .and_then(|value| value.as_str())
                .map(ToString::to_string),
            timestamp: value
                .get("timestamp")
                .and_then(|value| value.as_str())
                .map(ToString::to_string),
            session_id: current_session_id.clone(),
        };

        let should_replace = match messages.get(message_id) {
            None => true,
            Some(existing) if parsed.stop_reason.is_some() && existing.stop_reason.is_none() => {
                true
            }
            Some(existing) if parsed.stop_reason.is_some() == existing.stop_reason.is_some() => {
                parsed.output_tokens > existing.output_tokens
            }
            Some(_) => false,
        };
        if should_replace {
            messages.insert(message_id.to_string(), parsed);
        }
    }

    let mut imported = 0u64;
    let mut skipped = 0u64;
    for msg in messages.values() {
        if msg.stop_reason.is_none() || msg.output_tokens == 0 {
            continue;
        }
        let entry = SessionLogEntry {
            request_id: format!("session:{}", msg.message_id),
            provider_id: "_session",
            app_type: "claude",
            model: msg.model.clone(),
            input_tokens: msg.input_tokens,
            output_tokens: msg.output_tokens,
            cache_read_tokens: msg.cache_read_tokens,
            cache_creation_tokens: msg.cache_creation_tokens,
            session_id: msg.session_id.clone(),
            provider_type: "session_log",
            data_source: "session_log",
            created_at: parse_rfc3339_millis(msg.timestamp.as_deref()),
        };
        match insert_session_log_entry(conn, &entry) {
            Ok(true) => imported += 1,
            Ok(false) => skipped += 1,
            Err(_) => skipped += 1,
        }
    }

    update_sync_state(conn, &file_path_str, file_modified, line_offset)?;
    Ok((imported, skipped))
}

#[derive(Debug, Clone, Default)]
struct CumulativeTokens {
    input: u64,
    cached_input: u64,
    output: u64,
}

#[derive(Debug)]
struct DeltaTokens {
    input: u32,
    cached_input: u32,
    output: u32,
}

impl DeltaTokens {
    fn is_zero(&self) -> bool {
        self.input == 0 && self.cached_input == 0 && self.output == 0
    }
}

fn normalize_codex_model(raw: &str) -> String {
    let mut name = raw.to_ascii_lowercase();
    if let Some(pos) = name.rfind('/') {
        name = name[pos + 1..].to_string();
    }
    if let Some(base) = strip_ascii_date_suffix(&name, "-0000-00-00") {
        name = base.to_string();
    }
    if let Some((base, suffix)) = name.rsplit_once('-') {
        if suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_digit()) {
            name = base.to_string();
        }
    }
    name
}

fn strip_ascii_date_suffix<'a>(value: &'a str, pattern: &str) -> Option<&'a str> {
    let value_bytes = value.as_bytes();
    let pattern_bytes = pattern.as_bytes();
    if value_bytes.len() < pattern_bytes.len() {
        return None;
    }
    let start = value_bytes.len() - pattern_bytes.len();
    let suffix = &value_bytes[start..];
    let matches = suffix
        .iter()
        .zip(pattern_bytes)
        .all(|(value, pattern)| match pattern {
            b'0' => value.is_ascii_digit(),
            marker => value == marker,
        });
    if matches && value.is_char_boundary(start) {
        Some(&value[..start])
    } else {
        None
    }
}

fn parse_cumulative_tokens(total_usage: &serde_json::Value) -> Option<CumulativeTokens> {
    total_usage.is_object().then(|| CumulativeTokens {
        input: total_usage
            .get("input_tokens")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        cached_input: total_usage
            .get("cached_input_tokens")
            .or_else(|| total_usage.get("cache_read_input_tokens"))
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        output: total_usage
            .get("output_tokens")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
    })
}

fn compute_delta(prev: &Option<CumulativeTokens>, current: &CumulativeTokens) -> DeltaTokens {
    match prev {
        None => DeltaTokens {
            input: current.input as u32,
            cached_input: current.cached_input as u32,
            output: current.output as u32,
        },
        Some(prev) => DeltaTokens {
            input: current.input.saturating_sub(prev.input) as u32,
            cached_input: current.cached_input.saturating_sub(prev.cached_input) as u32,
            output: current.output.saturating_sub(prev.output) as u32,
        },
    }
}

fn collect_codex_session_files(codex_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let sessions_dir = codex_dir.join("sessions");
    if sessions_dir.is_dir() {
        collect_jsonl_recursive(&sessions_dir, &mut files, 0, 3);
    }
    let archived_dir = codex_dir.join("archived_sessions");
    if let Ok(entries) = fs::read_dir(archived_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }
    }
    files
}

fn sync_single_codex_file(conn: &Connection, file_path: &Path) -> Result<(u64, u64), AppError> {
    let file_path_str = file_path.to_string_lossy().to_string();
    let metadata = fs::metadata(file_path).map_err(|err| AppError::io(file_path, err))?;
    let file_modified = metadata_modified_nanos(&metadata);
    let (last_modified, last_offset) = get_sync_state(conn, &file_path_str)?;
    if file_modified <= last_modified {
        return Ok((0, 0));
    }

    let file = fs::File::open(file_path).map_err(|err| AppError::io(file_path, err))?;
    let reader = BufReader::new(file);
    let mut session_id: Option<String> = None;
    let mut current_model = "unknown".to_string();
    let mut prev_total: Option<CumulativeTokens> = None;
    let mut event_index = 0u32;
    let mut line_offset = 0i64;
    let mut imported = 0u64;
    let mut skipped = 0u64;

    for line_result in reader.lines() {
        line_offset += 1;
        let line = match line_result {
            Ok(line) => line,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        let is_event_msg = line.contains("\"event_msg\"");
        let is_turn_context = line.contains("\"turn_context\"");
        let is_session_meta = line.contains("\"session_meta\"");
        if !is_event_msg && !is_turn_context && !is_session_meta {
            continue;
        }
        if is_event_msg && !line.contains("\"token_count\"") {
            continue;
        }

        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        match value.get("type").and_then(|value| value.as_str()) {
            Some("session_meta") if session_id.is_none() => {
                session_id = value
                    .get("payload")
                    .and_then(|payload| {
                        payload
                            .get("session_id")
                            .or_else(|| payload.get("sessionId"))
                            .or_else(|| payload.get("id"))
                    })
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string);
            }
            Some("turn_context") => {
                if let Some(model) = value
                    .get("payload")
                    .and_then(|payload| {
                        payload
                            .get("model")
                            .or_else(|| payload.get("info").and_then(|info| info.get("model")))
                    })
                    .and_then(|value| value.as_str())
                {
                    current_model = normalize_codex_model(model);
                }
            }
            Some("event_msg") => {
                let Some(payload) = value.get("payload") else {
                    continue;
                };
                if payload.get("type").and_then(|value| value.as_str()) != Some("token_count") {
                    continue;
                }
                let Some(info) = payload.get("info").filter(|value| !value.is_null()) else {
                    continue;
                };
                if let Some(model) = info
                    .get("model")
                    .or_else(|| info.get("model_name"))
                    .or_else(|| payload.get("model"))
                    .and_then(|value| value.as_str())
                {
                    current_model = normalize_codex_model(model);
                }
                let (cumulative, is_total) = if let Some(total) = info.get("total_token_usage") {
                    (parse_cumulative_tokens(total), true)
                } else if let Some(last) = info.get("last_token_usage") {
                    (parse_cumulative_tokens(last), false)
                } else {
                    continue;
                };
                let Some(cumulative) = cumulative else {
                    continue;
                };
                let delta = if is_total {
                    let delta = compute_delta(&prev_total, &cumulative);
                    prev_total = Some(cumulative);
                    delta
                } else {
                    DeltaTokens {
                        input: cumulative.input as u32,
                        cached_input: cumulative.cached_input as u32,
                        output: cumulative.output as u32,
                    }
                };
                let delta = DeltaTokens {
                    cached_input: delta.cached_input.min(delta.input),
                    ..delta
                };
                if delta.is_zero() {
                    continue;
                }
                event_index += 1;
                if line_offset <= last_offset {
                    continue;
                }
                let session = session_id.as_deref().unwrap_or("unknown");
                let entry = SessionLogEntry {
                    request_id: format!("codex_session:{session}:{event_index}"),
                    provider_id: "_codex_session",
                    app_type: "codex",
                    model: current_model.clone(),
                    input_tokens: delta.input,
                    output_tokens: delta.output,
                    cache_read_tokens: delta.cached_input,
                    cache_creation_tokens: 0,
                    session_id: session_id.clone(),
                    provider_type: "codex_session",
                    data_source: "codex_session",
                    created_at: parse_rfc3339_millis(
                        value.get("timestamp").and_then(|value| value.as_str()),
                    ),
                };
                match insert_session_log_entry(conn, &entry) {
                    Ok(true) => imported += 1,
                    Ok(false) => skipped += 1,
                    Err(_) => skipped += 1,
                }
            }
            _ => {}
        }
    }

    update_sync_state(conn, &file_path_str, file_modified, line_offset)?;
    Ok((imported, skipped))
}

#[derive(Debug)]
struct GeminiTokens {
    input: u32,
    output: u32,
    cached: u32,
    thoughts: u32,
}

fn parse_gemini_tokens(tokens: &serde_json::Value) -> GeminiTokens {
    GeminiTokens {
        input: tokens
            .get("input")
            .and_then(|value| value.as_u64())
            .unwrap_or(0) as u32,
        output: tokens
            .get("output")
            .and_then(|value| value.as_u64())
            .unwrap_or(0) as u32,
        cached: tokens
            .get("cached")
            .and_then(|value| value.as_u64())
            .unwrap_or(0) as u32,
        thoughts: tokens
            .get("thoughts")
            .and_then(|value| value.as_u64())
            .unwrap_or(0) as u32,
    }
}

fn collect_gemini_session_files(gemini_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let tmp_dir = gemini_dir.join("tmp");
    let Ok(project_dirs) = fs::read_dir(tmp_dir) else {
        return files;
    };
    for entry in project_dirs.flatten() {
        let chats_dir = entry.path().join("chats");
        let Ok(chat_files) = fs::read_dir(chats_dir) else {
            continue;
        };
        for file_entry in chat_files.flatten() {
            let path = file_entry.path();
            let is_session = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("session-") && name.ends_with(".json"))
                .unwrap_or(false);
            if is_session {
                files.push(path);
            }
        }
    }
    files
}

fn sync_single_gemini_file(conn: &Connection, file_path: &Path) -> Result<(u64, u64), AppError> {
    let file_path_str = file_path.to_string_lossy().to_string();
    let metadata = fs::metadata(file_path).map_err(|err| AppError::io(file_path, err))?;
    let file_modified = metadata_modified_nanos(&metadata);
    let (last_modified, _) = get_sync_state(conn, &file_path_str)?;
    if file_modified <= last_modified {
        return Ok((0, 0));
    }

    let content = fs::read_to_string(file_path).map_err(|err| AppError::io(file_path, err))?;
    let value: serde_json::Value =
        serde_json::from_str(&content).map_err(|err| AppError::json(file_path, err))?;
    let session_id = value
        .get("sessionId")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    let Some(messages) = value.get("messages").and_then(|value| value.as_array()) else {
        update_sync_state(conn, &file_path_str, file_modified, 0)?;
        return Ok((0, 0));
    };

    let mut imported = 0u64;
    let mut skipped = 0u64;
    let mut message_count = 0i64;
    for msg in messages {
        if msg.get("type").and_then(|value| value.as_str()) != Some("gemini") {
            continue;
        }
        let Some(tokens_obj) = msg.get("tokens").filter(|value| value.is_object()) else {
            continue;
        };
        let tokens = parse_gemini_tokens(tokens_obj);
        if tokens.input == 0 && tokens.output == 0 && tokens.cached == 0 && tokens.thoughts == 0 {
            continue;
        }
        message_count += 1;
        let message_id = msg
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let model = msg
            .get("model")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let session = session_id.as_deref().unwrap_or("unknown");
        let entry = SessionLogEntry {
            request_id: format!("gemini_session:{session}:{message_id}"),
            provider_id: "_gemini_session",
            app_type: "gemini",
            model: model.to_string(),
            input_tokens: tokens.input,
            output_tokens: tokens.output + tokens.thoughts,
            cache_read_tokens: tokens.cached,
            cache_creation_tokens: 0,
            session_id: session_id.clone(),
            provider_type: "gemini_session",
            data_source: "gemini_session",
            created_at: parse_rfc3339_millis(msg.get("timestamp").and_then(|value| value.as_str())),
        };
        match insert_session_log_entry(conn, &entry) {
            Ok(true) => imported += 1,
            Ok(false) => skipped += 1,
            Err(_) => skipped += 1,
        }
    }

    update_sync_state(conn, &file_path_str, file_modified, message_count)?;
    Ok((imported, skipped))
}

fn model_matches_pricing_key(model: &str, pricing_key: &str) -> bool {
    let model = model.trim().to_ascii_lowercase();
    let pricing_key = pricing_key.trim().to_ascii_lowercase();
    if model.is_empty() || pricing_key.is_empty() {
        return false;
    }
    let mut candidates = vec![model.as_str()];
    if let Some((_, tail)) = model.rsplit_once('/') {
        candidates.push(tail);
    }
    if let Some((head, _)) = model.split_once(':') {
        candidates.push(head);
    }
    if let Some((_, tail)) = model.rsplit_once('/') {
        if let Some((tail_head, _)) = tail.split_once(':') {
            candidates.push(tail_head);
        }
    }

    candidates
        .into_iter()
        .any(|candidate| candidate_matches_pricing_key(candidate, &pricing_key))
}

fn candidate_matches_pricing_key(candidate: &str, pricing_key: &str) -> bool {
    candidate == pricing_key
        || candidate
            .strip_prefix(pricing_key)
            .and_then(|suffix| suffix.strip_prefix('-'))
            .is_some_and(|suffix| !suffix.is_empty())
}

impl Database {
    pub fn get_usage_summary(
        &self,
        start_date: Option<i64>,
        end_date: Option<i64>,
        app_type: Option<&str>,
    ) -> Result<UsageSummary, AppError> {
        self.get_usage_summary_with_filters(
            start_date,
            end_date,
            &UsageStatsFilters::from_app_type(app_type),
        )
    }

    pub fn get_usage_summary_with_filters(
        &self,
        start_date: Option<i64>,
        end_date: Option<i64>,
        filters: &UsageStatsFilters,
    ) -> Result<UsageSummary, AppError> {
        let filters = UsageStatsFiltersRef::from(filters);
        validate_usage_app(filters.app_type)?;
        let conn = lock_conn!(self.conn);

        let mut log_conditions = Vec::new();
        let mut log_params: Vec<Box<dyn ToSql>> = Vec::new();
        push_common_log_filters(
            &mut log_conditions,
            &mut log_params,
            "l",
            start_date,
            end_date,
            filters,
        );
        let log_where = where_clause(&log_conditions);

        let mut rollup_conditions = Vec::new();
        let mut rollup_params: Vec<Box<dyn ToSql>> = Vec::new();
        push_rollup_filters(
            &mut rollup_conditions,
            &mut rollup_params,
            "r",
            start_date,
            end_date,
            filters,
        )?;
        let rollup_where = where_clause(&rollup_conditions);

        let fresh_log = fresh_input_sql("l");
        let fresh_rollup = fresh_input_sql("r");
        let sql = format!(
            "SELECT
                COALESCE(d.total_requests, 0) + COALESCE(r.total_requests, 0),
                COALESCE(d.total_cost, 0) + COALESCE(r.total_cost, 0),
                COALESCE(d.total_input_tokens, 0) + COALESCE(r.total_input_tokens, 0),
                COALESCE(d.total_output_tokens, 0) + COALESCE(r.total_output_tokens, 0),
                COALESCE(d.total_cache_creation_tokens, 0) + COALESCE(r.total_cache_creation_tokens, 0),
                COALESCE(d.total_cache_read_tokens, 0) + COALESCE(r.total_cache_read_tokens, 0),
                COALESCE(d.success_count, 0) + COALESCE(r.success_count, 0)
             FROM
                (SELECT COUNT(*) AS total_requests,
                    COALESCE(SUM(CAST(l.total_cost_usd AS REAL)), 0) AS total_cost,
                    COALESCE(SUM({fresh_log}), 0) AS total_input_tokens,
                    COALESCE(SUM(l.output_tokens), 0) AS total_output_tokens,
                    COALESCE(SUM(l.cache_creation_tokens), 0) AS total_cache_creation_tokens,
                    COALESCE(SUM(l.cache_read_tokens), 0) AS total_cache_read_tokens,
                    COALESCE(SUM(CASE WHEN l.status_code >= 200 AND l.status_code < 300 THEN 1 ELSE 0 END), 0) AS success_count
                 FROM proxy_request_logs l {log_where}) d,
                (SELECT COALESCE(SUM(r.request_count), 0) AS total_requests,
                    COALESCE(SUM(CAST(r.total_cost_usd AS REAL)), 0) AS total_cost,
                    COALESCE(SUM({fresh_rollup}), 0) AS total_input_tokens,
                    COALESCE(SUM(r.output_tokens), 0) AS total_output_tokens,
                    COALESCE(SUM(r.cache_creation_tokens), 0) AS total_cache_creation_tokens,
                    COALESCE(SUM(r.cache_read_tokens), 0) AS total_cache_read_tokens,
                    COALESCE(SUM(r.success_count), 0) AS success_count
                 FROM usage_daily_rollups r {rollup_where}) r"
        );

        let mut all_params = log_params;
        all_params.extend(rollup_params);
        let refs = params_refs(&all_params);

        conn.query_row(&sql, refs.as_slice(), |row| {
            Ok(make_summary(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })
        .map_err(|err| AppError::Database(err.to_string()))
    }

    pub fn get_usage_summary_by_app(
        &self,
        start_date: Option<i64>,
        end_date: Option<i64>,
    ) -> Result<Vec<UsageSummaryByApp>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut log_conditions = Vec::new();
        let mut log_params: Vec<Box<dyn ToSql>> = Vec::new();
        push_common_log_filters(
            &mut log_conditions,
            &mut log_params,
            "l",
            start_date,
            end_date,
            UsageStatsFiltersRef::default(),
        );
        let log_where = where_clause(&log_conditions);

        let mut rollup_conditions = Vec::new();
        let mut rollup_params: Vec<Box<dyn ToSql>> = Vec::new();
        push_rollup_filters(
            &mut rollup_conditions,
            &mut rollup_params,
            "r",
            start_date,
            end_date,
            UsageStatsFiltersRef::default(),
        )?;
        let rollup_where = where_clause(&rollup_conditions);

        let fresh_log = fresh_input_sql("l");
        let fresh_rollup = fresh_input_sql("r");
        let sql = format!(
            "SELECT app_type, SUM(total_requests), SUM(total_cost), SUM(total_input_tokens),
                    SUM(total_output_tokens), SUM(total_cache_creation_tokens),
                    SUM(total_cache_read_tokens), SUM(success_count)
             FROM (
                SELECT l.app_type AS app_type,
                    COUNT(*) AS total_requests,
                    COALESCE(SUM(CAST(l.total_cost_usd AS REAL)), 0) AS total_cost,
                    COALESCE(SUM({fresh_log}), 0) AS total_input_tokens,
                    COALESCE(SUM(l.output_tokens), 0) AS total_output_tokens,
                    COALESCE(SUM(l.cache_creation_tokens), 0) AS total_cache_creation_tokens,
                    COALESCE(SUM(l.cache_read_tokens), 0) AS total_cache_read_tokens,
                    COALESCE(SUM(CASE WHEN l.status_code >= 200 AND l.status_code < 300 THEN 1 ELSE 0 END), 0) AS success_count
                 FROM proxy_request_logs l {log_where}
                 GROUP BY l.app_type
                UNION ALL
                SELECT r.app_type,
                    COALESCE(SUM(r.request_count), 0),
                    COALESCE(SUM(CAST(r.total_cost_usd AS REAL)), 0),
                    COALESCE(SUM({fresh_rollup}), 0),
                    COALESCE(SUM(r.output_tokens), 0),
                    COALESCE(SUM(r.cache_creation_tokens), 0),
                    COALESCE(SUM(r.cache_read_tokens), 0),
                    COALESCE(SUM(r.success_count), 0)
                 FROM usage_daily_rollups r {rollup_where}
                 GROUP BY r.app_type
             )
             GROUP BY app_type"
        );

        let mut all_params = log_params;
        all_params.extend(rollup_params);
        let refs = params_refs(&all_params);
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|err| AppError::Database(err.to_string()))?;
        let rows = stmt
            .query_map(refs.as_slice(), |row| {
                Ok(UsageSummaryByApp {
                    app_type: row.get(0)?,
                    summary: make_summary(
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ),
                })
            })
            .map_err(|err| AppError::Database(err.to_string()))?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row.map_err(|err| AppError::Database(err.to_string()))?);
        }
        summaries.sort_by(|a, b| {
            b.summary
                .real_total_tokens
                .cmp(&a.summary.real_total_tokens)
        });
        Ok(summaries)
    }

    pub fn get_daily_trends(
        &self,
        start_date: Option<i64>,
        end_date: Option<i64>,
        app_type: Option<&str>,
    ) -> Result<Vec<DailyStats>, AppError> {
        self.get_daily_trends_with_filters(
            start_date,
            end_date,
            &UsageStatsFilters::from_app_type(app_type),
        )
    }

    pub fn get_daily_trends_with_filters(
        &self,
        start_date: Option<i64>,
        end_date: Option<i64>,
        filters: &UsageStatsFilters,
    ) -> Result<Vec<DailyStats>, AppError> {
        let filters = UsageStatsFiltersRef::from(filters);
        validate_usage_app(filters.app_type)?;
        let conn = lock_conn!(self.conn);
        let end = end_date.unwrap_or_else(|| Local::now().timestamp_millis());
        let start = start_date.unwrap_or(end - 30 * 24 * 60 * 60 * 1000);
        let duration = end.saturating_sub(start);
        let hourly = duration <= 36 * 60 * 60 * 1000;
        let bucket_expr = if hourly {
            "strftime('%Y-%m-%dT%H:00:00', l.created_at / 1000, 'unixepoch', 'localtime')"
        } else {
            "date(l.created_at / 1000, 'unixepoch', 'localtime')"
        };

        let mut conditions = Vec::new();
        let mut query_params: Vec<Box<dyn ToSql>> = Vec::new();
        push_common_log_filters(
            &mut conditions,
            &mut query_params,
            "l",
            Some(start),
            Some(end),
            filters,
        );
        let log_where = where_clause(&conditions);
        let fresh_log = fresh_input_sql("l");
        let sql = format!(
            "SELECT {bucket_expr} AS bucket,
                    COUNT(*),
                    COALESCE(SUM(CAST(l.total_cost_usd AS REAL)), 0),
                    COALESCE(SUM({fresh_log} + l.output_tokens), 0),
                    COALESCE(SUM({fresh_log}), 0),
                    COALESCE(SUM(l.output_tokens), 0),
                    COALESCE(SUM(l.cache_creation_tokens), 0),
                    COALESCE(SUM(l.cache_read_tokens), 0)
             FROM proxy_request_logs l
             {log_where}
             GROUP BY bucket
             ORDER BY bucket ASC"
        );
        let refs = params_refs(&query_params);
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|err| AppError::Database(err.to_string()))?;
        let rows = stmt
            .query_map(refs.as_slice(), |row| {
                Ok(DailyStats {
                    date: row.get(0)?,
                    request_count: row.get::<_, i64>(1)?.max(0) as u64,
                    total_cost: format!("{:.6}", row.get::<_, f64>(2)?),
                    total_tokens: row.get::<_, i64>(3)?.max(0) as u64,
                    total_input_tokens: row.get::<_, i64>(4)?.max(0) as u64,
                    total_output_tokens: row.get::<_, i64>(5)?.max(0) as u64,
                    total_cache_creation_tokens: row.get::<_, i64>(6)?.max(0) as u64,
                    total_cache_read_tokens: row.get::<_, i64>(7)?.max(0) as u64,
                })
            })
            .map_err(|err| AppError::Database(err.to_string()))?;

        let mut stats = Vec::new();
        for row in rows {
            stats.push(row.map_err(|err| AppError::Database(err.to_string()))?);
        }

        if !hourly || stats.is_empty() {
            let mut rollup_conditions = Vec::new();
            let mut rollup_params: Vec<Box<dyn ToSql>> = Vec::new();
            push_rollup_filters(
                &mut rollup_conditions,
                &mut rollup_params,
                "r",
                Some(start),
                Some(end),
                filters,
            )?;
            let rollup_where = where_clause(&rollup_conditions);
            let fresh_rollup = fresh_input_sql("r");
            let rollup_sql = format!(
                "SELECT r.date,
                        COALESCE(SUM(r.request_count), 0),
                        COALESCE(SUM(CAST(r.total_cost_usd AS REAL)), 0),
                        COALESCE(SUM({fresh_rollup} + r.output_tokens), 0),
                        COALESCE(SUM({fresh_rollup}), 0),
                        COALESCE(SUM(r.output_tokens), 0),
                        COALESCE(SUM(r.cache_creation_tokens), 0),
                        COALESCE(SUM(r.cache_read_tokens), 0)
                 FROM usage_daily_rollups r
                 {rollup_where}
                 GROUP BY r.date
                 ORDER BY r.date ASC"
            );
            let refs = params_refs(&rollup_params);
            let mut stmt = conn
                .prepare(&rollup_sql)
                .map_err(|err| AppError::Database(err.to_string()))?;
            let rows = stmt
                .query_map(refs.as_slice(), |row| {
                    Ok(DailyStats {
                        date: row.get(0)?,
                        request_count: row.get::<_, i64>(1)?.max(0) as u64,
                        total_cost: format!("{:.6}", row.get::<_, f64>(2)?),
                        total_tokens: row.get::<_, i64>(3)?.max(0) as u64,
                        total_input_tokens: row.get::<_, i64>(4)?.max(0) as u64,
                        total_output_tokens: row.get::<_, i64>(5)?.max(0) as u64,
                        total_cache_creation_tokens: row.get::<_, i64>(6)?.max(0) as u64,
                        total_cache_read_tokens: row.get::<_, i64>(7)?.max(0) as u64,
                    })
                })
                .map_err(|err| AppError::Database(err.to_string()))?;
            let mut by_date: HashMap<String, DailyStats> = stats
                .into_iter()
                .map(|stat| (stat.date.clone(), stat))
                .collect();
            for row in rows {
                let stat = row.map_err(|err| AppError::Database(err.to_string()))?;
                by_date
                    .entry(stat.date.clone())
                    .and_modify(|existing| {
                        existing.request_count += stat.request_count;
                        existing.total_tokens += stat.total_tokens;
                        existing.total_input_tokens += stat.total_input_tokens;
                        existing.total_output_tokens += stat.total_output_tokens;
                        existing.total_cache_creation_tokens += stat.total_cache_creation_tokens;
                        existing.total_cache_read_tokens += stat.total_cache_read_tokens;
                        let cost = existing.total_cost.parse::<f64>().unwrap_or(0.0)
                            + stat.total_cost.parse::<f64>().unwrap_or(0.0);
                        existing.total_cost = format!("{cost:.6}");
                    })
                    .or_insert(stat);
            }
            stats = by_date.into_values().collect();
            stats.sort_by(|a, b| a.date.cmp(&b.date));
        }

        Ok(stats)
    }

    pub fn get_provider_stats(
        &self,
        start_date: Option<i64>,
        end_date: Option<i64>,
        app_type: Option<&str>,
    ) -> Result<Vec<ProviderStats>, AppError> {
        self.get_provider_stats_with_filters(
            start_date,
            end_date,
            &UsageStatsFilters::from_app_type(app_type),
        )
    }

    pub fn get_provider_stats_with_filters(
        &self,
        start_date: Option<i64>,
        end_date: Option<i64>,
        filters: &UsageStatsFilters,
    ) -> Result<Vec<ProviderStats>, AppError> {
        let filters = UsageStatsFiltersRef::from(filters);
        validate_usage_app(filters.app_type)?;
        let conn = lock_conn!(self.conn);
        let mut log_conditions = Vec::new();
        let mut log_params: Vec<Box<dyn ToSql>> = Vec::new();
        push_common_log_filters(
            &mut log_conditions,
            &mut log_params,
            "l",
            start_date,
            end_date,
            filters,
        );
        let log_where = where_clause(&log_conditions);

        let mut rollup_conditions = Vec::new();
        let mut rollup_params: Vec<Box<dyn ToSql>> = Vec::new();
        push_rollup_filters(
            &mut rollup_conditions,
            &mut rollup_params,
            "r",
            start_date,
            end_date,
            filters,
        )?;
        let rollup_where = where_clause(&rollup_conditions);

        let fresh_log = fresh_input_sql("l");
        let fresh_rollup = fresh_input_sql("r");
        let pname = provider_name_expr("l", "p");
        let rpname = provider_name_expr("r", "p");
        let sql = format!(
            "SELECT provider_id, app_type, COALESCE(MAX(provider_name), provider_id),
                    SUM(request_count), SUM(total_tokens), SUM(total_cost),
                    SUM(success_count),
                    CASE WHEN SUM(request_count) > 0
                         THEN SUM(latency_total) * 1.0 / SUM(request_count)
                         ELSE 0
                    END
             FROM (
                SELECT l.provider_id, l.app_type, {pname} AS provider_name,
                    COUNT(*) AS request_count,
                    COALESCE(SUM({fresh_log} + l.output_tokens), 0) AS total_tokens,
                    COALESCE(SUM(CAST(l.total_cost_usd AS REAL)), 0) AS total_cost,
                    COALESCE(SUM(CASE WHEN l.status_code >= 200 AND l.status_code < 300 THEN 1 ELSE 0 END), 0) AS success_count,
                    COALESCE(SUM(l.latency_ms), 0) AS latency_total
                 FROM proxy_request_logs l
                 LEFT JOIN providers p ON l.provider_id = p.id AND l.app_type = p.app_type
                 {log_where}
                 GROUP BY l.provider_id, l.app_type
                UNION ALL
                SELECT r.provider_id, r.app_type, {rpname} AS provider_name,
                    COALESCE(SUM(r.request_count), 0) AS request_count,
                    COALESCE(SUM({fresh_rollup} + r.output_tokens), 0) AS total_tokens,
                    COALESCE(SUM(CAST(r.total_cost_usd AS REAL)), 0) AS total_cost,
                    COALESCE(SUM(r.success_count), 0) AS success_count,
                    COALESCE(SUM(r.avg_latency_ms * r.request_count), 0) AS latency_total
                 FROM usage_daily_rollups r
                 LEFT JOIN providers p ON r.provider_id = p.id AND r.app_type = p.app_type
                 {rollup_where}
                 GROUP BY r.provider_id, r.app_type
             )
             GROUP BY provider_id, app_type
             ORDER BY 6 DESC, 4 DESC"
        );
        let mut all_params = log_params;
        all_params.extend(rollup_params);
        let refs = params_refs(&all_params);
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|err| AppError::Database(err.to_string()))?;
        let rows = stmt
            .query_map(refs.as_slice(), |row| {
                let request_count = row.get::<_, i64>(3)?.max(0);
                let success_count = row.get::<_, i64>(6)?.max(0);
                let success_rate = if request_count > 0 {
                    (success_count as f32 / request_count as f32) * 100.0
                } else {
                    0.0
                };
                Ok(ProviderStats {
                    provider_id: row.get(0)?,
                    app_type: row.get(1)?,
                    provider_name: row.get(2)?,
                    request_count: request_count as u64,
                    total_tokens: row.get::<_, i64>(4)?.max(0) as u64,
                    total_cost: format!("{:.6}", row.get::<_, f64>(5)?),
                    success_rate,
                    avg_latency_ms: row.get::<_, f64>(7)?.max(0.0) as u64,
                })
            })
            .map_err(|err| AppError::Database(err.to_string()))?;
        let mut stats = Vec::new();
        for row in rows {
            stats.push(row.map_err(|err| AppError::Database(err.to_string()))?);
        }
        Ok(stats)
    }

    pub fn get_model_stats(
        &self,
        start_date: Option<i64>,
        end_date: Option<i64>,
        app_type: Option<&str>,
    ) -> Result<Vec<ModelStats>, AppError> {
        self.get_model_stats_with_filters(
            start_date,
            end_date,
            &UsageStatsFilters::from_app_type(app_type),
        )
    }

    pub fn get_model_stats_with_filters(
        &self,
        start_date: Option<i64>,
        end_date: Option<i64>,
        filters: &UsageStatsFilters,
    ) -> Result<Vec<ModelStats>, AppError> {
        let filters = UsageStatsFiltersRef::from(filters);
        validate_usage_app(filters.app_type)?;
        let conn = lock_conn!(self.conn);
        let mut log_conditions = Vec::new();
        let mut log_params: Vec<Box<dyn ToSql>> = Vec::new();
        push_common_log_filters(
            &mut log_conditions,
            &mut log_params,
            "l",
            start_date,
            end_date,
            filters,
        );
        let log_where = where_clause(&log_conditions);

        let mut rollup_conditions = Vec::new();
        let mut rollup_params: Vec<Box<dyn ToSql>> = Vec::new();
        push_rollup_filters(
            &mut rollup_conditions,
            &mut rollup_params,
            "r",
            start_date,
            end_date,
            filters,
        )?;
        let rollup_where = where_clause(&rollup_conditions);

        let fresh_log = fresh_input_sql("l");
        let fresh_rollup = fresh_input_sql("r");
        let sql = format!(
            "SELECT model, SUM(request_count), SUM(total_tokens), SUM(total_cost)
             FROM (
                SELECT l.model,
                    COUNT(*) AS request_count,
                    COALESCE(SUM({fresh_log} + l.output_tokens), 0) AS total_tokens,
                    COALESCE(SUM(CAST(l.total_cost_usd AS REAL)), 0) AS total_cost
                 FROM proxy_request_logs l
                 {log_where}
                 GROUP BY l.model
                UNION ALL
                SELECT r.model,
                    COALESCE(SUM(r.request_count), 0) AS request_count,
                    COALESCE(SUM({fresh_rollup} + r.output_tokens), 0) AS total_tokens,
                    COALESCE(SUM(CAST(r.total_cost_usd AS REAL)), 0) AS total_cost
                 FROM usage_daily_rollups r
                 {rollup_where}
                 GROUP BY r.model
             )
             GROUP BY model
             ORDER BY 4 DESC, 2 DESC"
        );
        let mut all_params = log_params;
        all_params.extend(rollup_params);
        let refs = params_refs(&all_params);
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|err| AppError::Database(err.to_string()))?;
        let rows = stmt
            .query_map(refs.as_slice(), |row| {
                let request_count = row.get::<_, i64>(1)?.max(0);
                let total_cost = row.get::<_, f64>(3)?;
                let avg_cost = if request_count > 0 {
                    total_cost / request_count as f64
                } else {
                    0.0
                };
                Ok(ModelStats {
                    model: row.get(0)?,
                    request_count: request_count as u64,
                    total_tokens: row.get::<_, i64>(2)?.max(0) as u64,
                    total_cost: format!("{total_cost:.6}"),
                    avg_cost_per_request: format!("{avg_cost:.6}"),
                })
            })
            .map_err(|err| AppError::Database(err.to_string()))?;
        let mut stats = Vec::new();
        for row in rows {
            stats.push(row.map_err(|err| AppError::Database(err.to_string()))?);
        }
        Ok(stats)
    }

    pub fn get_request_logs(
        &self,
        filters: &LogFilters,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedLogs, AppError> {
        validate_usage_app(filters.app_type.as_deref())?;
        let conn = lock_conn!(self.conn);
        let page_size = page_size.clamp(1, 200);
        let mut conditions = Vec::new();
        let mut query_params: Vec<Box<dyn ToSql>> = Vec::new();
        push_common_log_filters(
            &mut conditions,
            &mut query_params,
            "l",
            filters.start_date,
            filters.end_date,
            UsageStatsFiltersRef::from_parts(
                filters.app_type.as_deref(),
                filters.provider_id.as_deref(),
                None,
            ),
        );
        if let Some(provider_name) = filters
            .provider_name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            conditions.push(format!("{} LIKE ?", provider_name_expr("l", "p")));
            query_params.push(Box::new(format!("%{provider_name}%")));
        }
        if let Some(model) = filters
            .model
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            conditions.push("l.model LIKE ?".to_string());
            query_params.push(Box::new(format!("%{model}%")));
        }
        if let Some(status_code) = filters.status_code {
            conditions.push("l.status_code = ?".to_string());
            query_params.push(Box::new(i64::from(status_code)));
        }
        let log_where = where_clause(&conditions);
        let count_sql = format!(
            "SELECT COUNT(*)
             FROM proxy_request_logs l
             LEFT JOIN providers p ON l.provider_id = p.id AND l.app_type = p.app_type
             {log_where}"
        );
        let refs = params_refs(&query_params);
        let total = conn
            .query_row(&count_sql, refs.as_slice(), |row| row.get::<_, i64>(0))
            .map_err(|err| AppError::Database(err.to_string()))?
            .max(0) as u32;

        let offset = i64::from(page.saturating_mul(page_size));
        query_params.push(Box::new(i64::from(page_size)));
        query_params.push(Box::new(offset));
        let refs = params_refs(&query_params);
        let pname = provider_name_expr("l", "p");
        let sql = format!(
            "SELECT l.request_id, l.provider_id, {pname} AS provider_name, l.app_type, l.model,
                    l.request_model, l.cost_multiplier,
                    l.input_tokens, l.output_tokens, l.cache_read_tokens, l.cache_creation_tokens,
                    l.input_cost_usd, l.output_cost_usd, l.cache_read_cost_usd,
                    l.cache_creation_cost_usd, l.total_cost_usd,
                    l.is_streaming, l.latency_ms, l.first_token_ms, l.duration_ms,
                    l.status_code, l.error_message, l.session_id, l.provider_type,
                    l.created_at, l.data_source
             FROM proxy_request_logs l
             LEFT JOIN providers p ON l.provider_id = p.id AND l.app_type = p.app_type
             {log_where}
             ORDER BY l.created_at DESC
             LIMIT ? OFFSET ?"
        );
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|err| AppError::Database(err.to_string()))?;
        let rows = stmt
            .query_map(refs.as_slice(), row_to_request_log_detail)
            .map_err(|err| AppError::Database(err.to_string()))?;
        let mut logs = Vec::new();
        for row in rows {
            logs.push(row.map_err(|err| AppError::Database(err.to_string()))?);
        }
        Ok(PaginatedLogs {
            data: logs,
            total,
            page,
            page_size,
        })
    }

    pub fn get_request_detail(
        &self,
        request_id: &str,
    ) -> Result<Option<RequestLogDetail>, AppError> {
        let conn = lock_conn!(self.conn);
        let pname = provider_name_expr("l", "p");
        let sql = format!(
            "SELECT l.request_id, l.provider_id, {pname} AS provider_name, l.app_type, l.model,
                    l.request_model, l.cost_multiplier,
                    l.input_tokens, l.output_tokens, l.cache_read_tokens, l.cache_creation_tokens,
                    l.input_cost_usd, l.output_cost_usd, l.cache_read_cost_usd,
                    l.cache_creation_cost_usd, l.total_cost_usd,
                    l.is_streaming, l.latency_ms, l.first_token_ms, l.duration_ms,
                    l.status_code, l.error_message, l.session_id, l.provider_type,
                    l.created_at, l.data_source
             FROM proxy_request_logs l
             LEFT JOIN providers p ON l.provider_id = p.id AND l.app_type = p.app_type
             WHERE l.request_id = ?1"
        );
        conn.query_row(&sql, params![request_id], row_to_request_log_detail)
            .optional()
            .map_err(|err| AppError::Database(err.to_string()))
    }

    pub fn get_usage_data_sources(&self) -> Result<Vec<DataSourceSummary>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn
            .prepare(
                "SELECT COALESCE(data_source, 'proxy'), COUNT(*),
                        COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0)
                 FROM proxy_request_logs
                 GROUP BY COALESCE(data_source, 'proxy')
                 ORDER BY COUNT(*) DESC",
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(DataSourceSummary {
                    data_source: row.get(0)?,
                    request_count: row.get::<_, i64>(1)?.max(0) as u64,
                    total_cost_usd: format!("{:.6}", row.get::<_, f64>(2)?),
                })
            })
            .map_err(|err| AppError::Database(err.to_string()))?;
        let mut data = Vec::new();
        for row in rows {
            data.push(row.map_err(|err| AppError::Database(err.to_string()))?);
        }
        Ok(data)
    }

    pub fn get_usage_data_extent(
        &self,
        app_type: Option<&str>,
    ) -> Result<UsageDataExtent, AppError> {
        let conn = lock_conn!(self.conn);
        let mut sql =
            "SELECT MIN(created_at), MAX(created_at), COUNT(*) FROM proxy_request_logs".to_string();
        let mut params_vec: Vec<Box<dyn ToSql>> = Vec::new();
        if let Some(app_type) = app_type.filter(|value| !value.is_empty()) {
            sql.push_str(" WHERE app_type = ?");
            params_vec.push(Box::new(app_type.to_string()));
        }
        let params_refs = params_vec
            .iter()
            .map(|value| value.as_ref() as &dyn ToSql)
            .collect::<Vec<_>>();

        conn.query_row(&sql, params_refs.as_slice(), |row| {
            Ok(UsageDataExtent {
                first_seen_at: row.get(0)?,
                last_seen_at: row.get(1)?,
                request_count: row.get::<_, i64>(2)?.max(0) as u64,
            })
        })
        .map_err(|err| AppError::Database(err.to_string()))
    }

    pub fn check_provider_limits(
        &self,
        provider_id: &str,
        app_type: &str,
    ) -> Result<ProviderLimitStatus, AppError> {
        validate_usage_app(Some(app_type))?;
        let now = Local::now();
        let day_start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| AppError::Database("invalid local day start".to_string()))?;
        let month_start = now
            .date_naive()
            .with_day(1)
            .and_then(|day| day.and_hms_opt(0, 0, 0))
            .ok_or_else(|| AppError::Database("invalid local month start".to_string()))?;
        let day_start_ms = Local
            .from_local_datetime(&day_start)
            .single()
            .ok_or_else(|| AppError::Database("invalid local day timestamp".to_string()))?
            .timestamp_millis();
        let month_start_ms = Local
            .from_local_datetime(&month_start)
            .single()
            .ok_or_else(|| AppError::Database("invalid local month timestamp".to_string()))?
            .timestamp_millis();

        let conn = lock_conn!(self.conn);
        let daily: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0)
                 FROM proxy_request_logs
                 WHERE provider_id = ?1 AND app_type = ?2 AND created_at >= ?3",
                params![provider_id, app_type, day_start_ms],
                |row| row.get(0),
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
        let monthly: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0)
                 FROM proxy_request_logs
                 WHERE provider_id = ?1 AND app_type = ?2 AND created_at >= ?3",
                params![provider_id, app_type, month_start_ms],
                |row| row.get(0),
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
        Ok(ProviderLimitStatus {
            provider_id: provider_id.to_string(),
            app_type: app_type.to_string(),
            daily_usage: format!("{daily:.6}"),
            daily_limit: None,
            daily_exceeded: false,
            monthly_usage: format!("{monthly:.6}"),
            monthly_limit: None,
            monthly_exceeded: false,
        })
    }

    pub fn sync_session_usage(&self) -> Result<SessionSyncResult, AppError> {
        let mut result = SessionSyncResult {
            imported: 0,
            skipped: 0,
            files_scanned: 0,
            errors: Vec::new(),
        };

        let mut claude_files = Vec::new();
        match crate::config::get_claude_config_dir() {
            Ok(dir) => {
                let projects_dir = dir.join("projects");
                if projects_dir.is_dir() {
                    claude_files = collect_claude_session_files(&projects_dir);
                }
            }
            Err(err) => result.errors.push(format!("Claude config dir: {err}")),
        }

        let mut codex_files = Vec::new();
        match crate::codex_config::get_codex_config_dir() {
            Ok(dir) => codex_files = collect_codex_session_files(&dir),
            Err(err) => result.errors.push(format!("Codex config dir: {err}")),
        }

        let mut gemini_files = Vec::new();
        match crate::gemini_config::get_gemini_dir() {
            Ok(dir) => gemini_files = collect_gemini_session_files(&dir),
            Err(err) => result.errors.push(format!("Gemini config dir: {err}")),
        }

        result.files_scanned = (claude_files.len() + codex_files.len() + gemini_files.len()) as u64;

        let conn = lock_conn!(self.conn);
        for file in claude_files {
            match sync_single_claude_file(&conn, &file) {
                Ok((imported, skipped)) => {
                    result.imported += imported;
                    result.skipped += skipped;
                }
                Err(err) => result.errors.push(format!("{}: {err}", file.display())),
            }
        }
        for file in codex_files {
            match sync_single_codex_file(&conn, &file) {
                Ok((imported, skipped)) => {
                    result.imported += imported;
                    result.skipped += skipped;
                }
                Err(err) => result.errors.push(format!("{}: {err}", file.display())),
            }
        }
        for file in gemini_files {
            match sync_single_gemini_file(&conn, &file) {
                Ok((imported, skipped)) => {
                    result.imported += imported;
                    result.skipped += skipped;
                }
                Err(err) => result.errors.push(format!("{}: {err}", file.display())),
            }
        }

        Ok(result)
    }

    pub fn backfill_missing_usage_costs_for_model(&self, model_id: &str) -> Result<u64, AppError> {
        let Some(pricing) = self.get_model_pricing(model_id)? else {
            return Ok(0);
        };
        let conn = lock_conn!(self.conn);
        let mut stmt = conn
            .prepare(
                "SELECT l.request_id, l.provider_id, l.provider_id AS provider_name, l.app_type, l.model,
                        l.request_model, l.cost_multiplier,
                        l.input_tokens, l.output_tokens, l.cache_read_tokens, l.cache_creation_tokens,
                        l.input_cost_usd, l.output_cost_usd, l.cache_read_cost_usd,
                        l.cache_creation_cost_usd, l.total_cost_usd,
                        l.is_streaming, l.latency_ms, l.first_token_ms, l.duration_ms,
                        l.status_code, l.error_message, l.session_id, l.provider_type,
                        l.created_at, l.data_source
                 FROM proxy_request_logs l
                 WHERE l.status_code >= 200 AND l.status_code < 300
                   AND CAST(l.total_cost_usd AS REAL) = 0
                   AND (
                        LOWER(l.model) = LOWER(?1)
                        OR LOWER(l.model) LIKE LOWER(?1 || '-%')
                        OR LOWER(l.model) LIKE LOWER('%/' || ?1)
                        OR LOWER(l.model) LIKE LOWER('%/' || ?1 || ':%')
                        OR LOWER(COALESCE(l.request_model, '')) = LOWER(?1)
                        OR LOWER(COALESCE(l.request_model, '')) LIKE LOWER(?1 || '-%')
                        OR LOWER(COALESCE(l.request_model, '')) LIKE LOWER('%/' || ?1)
                        OR LOWER(COALESCE(l.request_model, '')) LIKE LOWER('%/' || ?1 || ':%')
                   )",
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
        let rows = stmt
            .query_map(params![model_id], row_to_request_log_detail)
            .map_err(|err| AppError::Database(err.to_string()))?;
        let mut logs = Vec::new();
        for row in rows {
            logs.push(row.map_err(|err| AppError::Database(err.to_string()))?);
        }
        drop(stmt);

        let mut updated = 0;
        for log in logs {
            if !model_matches_pricing_key(&log.model, model_id)
                && !log
                    .request_model
                    .as_deref()
                    .map(|request_model| model_matches_pricing_key(request_model, model_id))
                    .unwrap_or(false)
            {
                continue;
            }
            let [input, output, cache_read, cache_creation, total] =
                calculate_cost_strings(&log.app_type, &log, &pricing);
            conn.execute(
                "UPDATE proxy_request_logs
                 SET input_cost_usd = ?1,
                     output_cost_usd = ?2,
                     cache_read_cost_usd = ?3,
                     cache_creation_cost_usd = ?4,
                     total_cost_usd = ?5
                 WHERE request_id = ?6",
                params![
                    input,
                    output,
                    cache_read,
                    cache_creation,
                    total,
                    log.request_id
                ],
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
            updated += 1;
        }
        Ok(updated)
    }

    pub fn update_model_pricing_and_backfill(
        &self,
        record: &ModelPricingRecord,
    ) -> Result<u64, AppError> {
        self.upsert_model_pricing(record)?;
        self.backfill_missing_usage_costs_for_model(&record.model_id)
    }

    pub fn pricing_for_model(&self, model_id: &str) -> Result<Option<ModelPricing>, AppError> {
        self.get_model_pricing(model_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serial_test::serial;
    use tempfile::tempdir;

    struct EnvRestore {
        values: Vec<(&'static str, Option<String>)>,
    }

    impl EnvRestore {
        fn set_home(home: &Path) -> Self {
            let keys = ["HOME", "CC_SWITCH_ACCOUNT_HOME", "USERPROFILE"];
            let values = keys
                .iter()
                .map(|key| (*key, std::env::var(key).ok()))
                .collect::<Vec<_>>();
            std::env::set_var("HOME", home);
            std::env::set_var("CC_SWITCH_ACCOUNT_HOME", home);
            #[cfg(windows)]
            std::env::set_var("USERPROFILE", home);
            Self { values }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
            let _ = crate::update_settings(crate::AppSettings::default());
        }
    }

    #[test]
    fn usage_summary_and_logs_read_proxy_records() -> Result<(), AppError> {
        let db = Database::memory()?;
        let now = Utc::now().timestamp_millis();
        {
            let conn = lock_conn!(db.conn);
            conn.execute(
                "INSERT INTO proxy_request_logs (
                    request_id, provider_id, app_type, model,
                    input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
                    input_cost_usd, output_cost_usd, cache_read_cost_usd,
                    cache_creation_cost_usd, total_cost_usd, latency_ms,
                    status_code, created_at
                 ) VALUES (
                    'usage-test-1', 'p1', 'claude', 'claude-sonnet-4-20250514',
                    100, 40, 25, 10,
                    '0.000300', '0.000600', '0.000008',
                    '0.000038', '0.000946', 123,
                    200, ?1
                 )",
                params![now],
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
        }

        let summary = db.get_usage_summary(Some(now - 1000), Some(now + 1000), Some("claude"))?;
        assert_eq!(summary.total_requests, 1);
        assert_eq!(summary.total_input_tokens, 100);
        assert_eq!(summary.total_output_tokens, 40);
        assert_eq!(summary.total_cache_read_tokens, 25);
        assert_eq!(summary.total_cache_creation_tokens, 10);
        assert_eq!(summary.real_total_tokens, 175);

        let logs = db.get_request_logs(
            &LogFilters {
                app_type: Some("claude".to_string()),
                ..LogFilters::default()
            },
            0,
            20,
        )?;
        assert_eq!(logs.total, 1);
        assert_eq!(logs.data[0].request_id, "usage-test-1");
        assert_eq!(logs.data[0].latency_ms, 123);
        Ok(())
    }

    #[test]
    fn codex_summary_subtracts_cache_read_from_input() -> Result<(), AppError> {
        let db = Database::memory()?;
        let now = Utc::now().timestamp_millis();
        {
            let conn = lock_conn!(db.conn);
            conn.execute(
                "INSERT INTO proxy_request_logs (
                    request_id, provider_id, app_type, model,
                    input_tokens, output_tokens, cache_read_tokens,
                    total_cost_usd, latency_ms, status_code, created_at
                 ) VALUES (
                    'usage-codex-cache', 'p1', 'codex', 'gpt-5-mini',
                    1000, 50, 600,
                    '0.001', 50, 200, ?1
                 )",
                params![now],
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
        }

        let summary = db.get_usage_summary(None, None, Some("codex"))?;
        assert_eq!(summary.total_input_tokens, 400);
        assert_eq!(summary.total_cache_read_tokens, 600);
        assert_eq!(summary.real_total_tokens, 1050);
        Ok(())
    }

    #[test]
    fn usage_data_extent_reports_latest_data_by_app() -> Result<(), AppError> {
        let db = Database::memory()?;
        let claude_at = 1_760_000_000_000i64;
        let codex_first = 1_770_000_000_000i64;
        let codex_last = 1_780_000_000_000i64;
        {
            let conn = lock_conn!(db.conn);
            conn.execute(
                "INSERT INTO proxy_request_logs (
                    request_id, provider_id, app_type, model,
                    input_tokens, output_tokens, total_cost_usd,
                    latency_ms, status_code, created_at
                 ) VALUES
                    ('usage-extent-claude', 'p1', 'claude', 'claude-sonnet-4', 1, 1, '0', 1, 200, ?1),
                    ('usage-extent-codex-1', 'p1', 'codex', 'gpt-5-mini', 1, 1, '0', 1, 200, ?2),
                    ('usage-extent-codex-2', 'p1', 'codex', 'gpt-5-mini', 1, 1, '0', 1, 200, ?3)",
                params![claude_at, codex_first, codex_last],
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
        }

        let all = db.get_usage_data_extent(None)?;
        assert_eq!(all.first_seen_at, Some(claude_at));
        assert_eq!(all.last_seen_at, Some(codex_last));
        assert_eq!(all.request_count, 3);

        let codex = db.get_usage_data_extent(Some("codex"))?;
        assert_eq!(codex.first_seen_at, Some(codex_first));
        assert_eq!(codex.last_seen_at, Some(codex_last));
        assert_eq!(codex.request_count, 2);

        let gemini = db.get_usage_data_extent(Some("gemini"))?;
        assert_eq!(gemini.first_seen_at, None);
        assert_eq!(gemini.last_seen_at, None);
        assert_eq!(gemini.request_count, 0);
        Ok(())
    }

    #[test]
    fn codex_model_normalization_handles_multibyte_names() {
        assert_eq!(
            normalize_codex_model("provider/模型始-gpt-5-2025-12-27"),
            "模型始-gpt-5"
        );
        assert_eq!(normalize_codex_model("供应商/始模型-20251227"), "始模型");
        assert_eq!(normalize_codex_model("始-gpt-5"), "始-gpt-5");
    }

    #[test]
    fn pricing_update_backfills_zero_cost_logs() -> Result<(), AppError> {
        let db = Database::memory()?;
        let now = Utc::now().timestamp_millis();
        {
            let conn = lock_conn!(db.conn);
            conn.execute(
                "INSERT INTO proxy_request_logs (
                    request_id, provider_id, app_type, model,
                    input_tokens, output_tokens, total_cost_usd,
                    latency_ms, status_code, created_at
                 ) VALUES (
                    'usage-backfill-1', 'p1', 'claude', 'custom-priced-model',
                    1000000, 1000000, '0',
                    50, 200, ?1
                 )",
                params![now],
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
            conn.execute(
                "INSERT INTO proxy_request_logs (
                    request_id, provider_id, app_type, model,
                    input_tokens, output_tokens, total_cost_usd,
                    latency_ms, status_code, created_at
                 ) VALUES (
                    'usage-backfill-2', 'p1', 'claude', 'provider/custom-priced-model:extra',
                    1000000, 1000000, '0',
                    50, 200, ?1
                 )",
                params![now],
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
        }

        let updated = db.update_model_pricing_and_backfill(&ModelPricingRecord {
            model_id: "custom-priced-model".to_string(),
            display_name: "Custom Priced".to_string(),
            input_cost_per_million: "1".to_string(),
            output_cost_per_million: "2".to_string(),
            cache_read_cost_per_million: "0".to_string(),
            cache_creation_cost_per_million: "0".to_string(),
        })?;
        assert_eq!(updated, 2);

        let detail = db
            .get_request_detail("usage-backfill-1")?
            .expect("request detail");
        assert_eq!(detail.total_cost_usd, "3.0");
        assert!(!detail.is_unpriced);
        let namespaced_detail = db
            .get_request_detail("usage-backfill-2")?
            .expect("namespaced request detail");
        assert_eq!(namespaced_detail.total_cost_usd, "3.0");
        assert!(!namespaced_detail.is_unpriced);
        Ok(())
    }

    #[test]
    fn model_pricing_match_requires_model_boundary() {
        assert!(model_matches_pricing_key(
            "claude-sonnet-4-20250514",
            "claude-sonnet-4"
        ));
        assert!(model_matches_pricing_key(
            "provider/custom-model:extra",
            "custom-model"
        ));
        assert!(model_matches_pricing_key("gpt-4o-mini", "gpt-4o"));

        assert!(!model_matches_pricing_key("gpt-4o", "gpt-4"));
        assert!(!model_matches_pricing_key("gpt-4.1", "gpt-4"));
        assert!(!model_matches_pricing_key("provider/gpt-4o:extra", "gpt-4"));
    }

    #[test]
    fn provider_and_model_stats_include_rollups() -> Result<(), AppError> {
        let db = Database::memory()?;
        {
            let conn = lock_conn!(db.conn);
            conn.execute(
                "INSERT INTO usage_daily_rollups (
                    date, app_type, provider_id, model, request_count, success_count,
                    input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
                    total_cost_usd, avg_latency_ms
                 ) VALUES (
                    '2026-05-01', 'claude', 'archived-provider', 'claude-sonnet-4-20250514',
                    3, 2, 300, 90, 30, 15, '0.123456', 250
                 )",
                [],
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
        }

        let providers = db.get_provider_stats(None, None, Some("claude"))?;
        let provider = providers
            .iter()
            .find(|stat| stat.provider_id == "archived-provider")
            .expect("rollup provider stat");
        assert_eq!(provider.request_count, 3);
        assert_eq!(provider.total_tokens, 390);
        assert_eq!(provider.total_cost, "0.123456");
        assert!((provider.success_rate - 66.66667).abs() < 0.01);
        assert_eq!(provider.avg_latency_ms, 250);

        let models = db.get_model_stats(None, None, Some("claude"))?;
        let model = models
            .iter()
            .find(|stat| stat.model == "claude-sonnet-4-20250514")
            .expect("rollup model stat");
        assert_eq!(model.request_count, 3);
        assert_eq!(model.total_tokens, 390);
        assert_eq!(model.total_cost, "0.123456");
        assert_eq!(model.avg_cost_per_request, "0.041152");
        Ok(())
    }

    #[test]
    fn usage_stats_filters_provider_and_model_across_logs_and_rollups() -> Result<(), AppError> {
        let db = Database::memory()?;
        let now = Utc::now().timestamp_millis();
        let rollup_date = rollup_date_from_millis(now)?;
        {
            let conn = lock_conn!(db.conn);
            conn.execute(
                "INSERT INTO proxy_request_logs (
                    request_id, provider_id, app_type, model,
                    input_tokens, output_tokens, total_cost_usd,
                    latency_ms, status_code, created_at
                 ) VALUES
                    ('usage-filter-log-1', 'p1', 'claude', 'claude-sonnet-4', 100, 30, '0.010000', 100, 200, ?1),
                    ('usage-filter-log-2', 'p2', 'claude', 'claude-sonnet-4', 500, 50, '0.050000', 100, 200, ?1),
                    ('usage-filter-log-3', 'p1', 'claude', 'claude-opus-4', 700, 70, '0.070000', 100, 200, ?1)",
                params![now],
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
            conn.execute(
                "INSERT INTO usage_daily_rollups (
                    date, app_type, provider_id, model, request_count, success_count,
                    input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
                    total_cost_usd, avg_latency_ms
                 ) VALUES
                    (?1, 'claude', 'p1', 'claude-sonnet-4', 2, 2, 200, 60, 0, 0, '0.020000', 120),
                    (?1, 'claude', 'p2', 'claude-sonnet-4', 3, 3, 300, 90, 0, 0, '0.030000', 120),
                    (?1, 'claude', 'p1', 'claude-opus-4', 4, 4, 400, 120, 0, 0, '0.040000', 120)",
                params![rollup_date],
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
        }

        let filters = UsageStatsFilters {
            app_type: Some("claude".to_string()),
            provider_id: Some("p1".to_string()),
            model: Some("claude-sonnet-4".to_string()),
        };
        let summary =
            db.get_usage_summary_with_filters(Some(now - 1_000), Some(now + 1_000), &filters)?;
        assert_eq!(summary.total_requests, 3);
        assert_eq!(summary.total_input_tokens, 300);
        assert_eq!(summary.total_output_tokens, 90);
        assert_eq!(summary.total_cost, "0.030000");

        let providers =
            db.get_provider_stats_with_filters(Some(now - 1_000), Some(now + 1_000), &filters)?;
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].provider_id, "p1");
        assert_eq!(providers[0].request_count, 3);

        let models =
            db.get_model_stats_with_filters(Some(now - 1_000), Some(now + 1_000), &filters)?;
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model, "claude-sonnet-4");
        assert_eq!(models[0].request_count, 3);
        Ok(())
    }

    #[test]
    fn daily_trends_include_rollups_when_log_range_is_empty() -> Result<(), AppError> {
        let db = Database::memory()?;
        let now = Utc::now().timestamp_millis();
        let rollup_date = rollup_date_from_millis(now)?;
        {
            let conn = lock_conn!(db.conn);
            conn.execute(
                "INSERT INTO usage_daily_rollups (
                    date, app_type, provider_id, model, request_count, success_count,
                    input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
                    total_cost_usd, avg_latency_ms
                 ) VALUES (
                    ?1, 'claude', 'archived-provider', 'claude-sonnet-4-20250514',
                    2, 2, 200, 80, 20, 10, '0.222222', 200
                 )",
                params![rollup_date],
            )
            .map_err(|err| AppError::Database(err.to_string()))?;
        }

        let trends = db.get_daily_trends(Some(now - 1_000), Some(now + 1_000), Some("claude"))?;

        assert_eq!(trends.len(), 1);
        assert_eq!(trends[0].date, rollup_date);
        assert_eq!(trends[0].request_count, 2);
        assert_eq!(trends[0].total_cost, "0.222222");
        assert_eq!(trends[0].total_input_tokens, 200);
        assert_eq!(trends[0].total_output_tokens, 80);
        Ok(())
    }

    #[test]
    #[serial]
    fn sync_session_usage_imports_claude_jsonl() -> Result<(), AppError> {
        let temp = tempdir().map_err(|err| AppError::Config(err.to_string()))?;
        let _env = EnvRestore::set_home(temp.path());
        crate::update_settings(crate::AppSettings::default())?;

        let session_dir = temp.path().join(".claude/projects/example");
        fs::create_dir_all(&session_dir).map_err(|err| AppError::io(&session_dir, err))?;
        let session_file = session_dir.join("session.jsonl");
        fs::write(
            &session_file,
            r#"{"type":"assistant","message":{"id":"msg_usage_1","model":"claude-sonnet-4-20250514","usage":{"input_tokens":1000000,"output_tokens":1000000,"cache_read_input_tokens":0,"cache_creation_input_tokens":0},"stop_reason":"end_turn"},"timestamp":"2026-05-01T00:00:00Z","sessionId":"session-1"}"#,
        )
        .map_err(|err| AppError::io(&session_file, err))?;

        let db = Database::memory()?;
        let result = db.sync_session_usage()?;
        assert_eq!(result.files_scanned, 1);
        assert_eq!(result.imported, 1);
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        let detail = db
            .get_request_detail("session:msg_usage_1")?
            .expect("imported session log");
        assert_eq!(detail.provider_id, "_session");
        assert_eq!(detail.app_type, "claude");
        assert_eq!(detail.data_source.as_deref(), Some("session_log"));
        assert_eq!(detail.created_at, 1_777_593_600_000);
        assert_eq!(detail.total_cost_usd, "18.0");

        let second = db.sync_session_usage()?;
        assert_eq!(second.imported, 0);
        assert_eq!(second.files_scanned, 1);
        Ok(())
    }
}
