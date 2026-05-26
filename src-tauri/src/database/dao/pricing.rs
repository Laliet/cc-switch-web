use super::super::{lock_conn, Database};
use crate::{error::AppError, settings::ProxyAppSettings};
use rusqlite::{params, OptionalExtension};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub const PRICING_SOURCE_REQUEST: &str = "request";
pub const PRICING_SOURCE_RESPONSE: &str = "response";

#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input_cost_per_million: Decimal,
    pub output_cost_per_million: Decimal,
    pub cache_read_cost_per_million: Decimal,
    pub cache_creation_cost_per_million: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelPricingRecord {
    pub model_id: String,
    pub display_name: String,
    pub input_cost_per_million: String,
    pub output_cost_per_million: String,
    pub cache_read_cost_per_million: String,
    pub cache_creation_cost_per_million: String,
}

impl ModelPricing {
    pub fn from_strings(
        input: &str,
        output: &str,
        cache_read: &str,
        cache_creation: &str,
    ) -> Result<Self, rust_decimal::Error> {
        Ok(Self {
            input_cost_per_million: Decimal::from_str(input)?,
            output_cost_per_million: Decimal::from_str(output)?,
            cache_read_cost_per_million: Decimal::from_str(cache_read)?,
            cache_creation_cost_per_million: Decimal::from_str(cache_creation)?,
        })
    }
}

impl Database {
    pub fn list_model_pricing(&self) -> Result<Vec<ModelPricingRecord>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn
            .prepare(
                "SELECT model_id, display_name, input_cost_per_million,
                        output_cost_per_million, cache_read_cost_per_million,
                        cache_creation_cost_per_million
                 FROM model_pricing
                 ORDER BY model_id ASC",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ModelPricingRecord {
                    model_id: row.get(0)?,
                    display_name: row.get(1)?,
                    input_cost_per_million: row.get(2)?,
                    output_cost_per_million: row.get(3)?,
                    cache_read_cost_per_million: row.get(4)?,
                    cache_creation_cost_per_million: row.get(5)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(records)
    }

    pub fn upsert_model_pricing(&self, record: &ModelPricingRecord) -> Result<(), AppError> {
        validate_model_pricing_record(record)?;
        let conn = lock_conn!(self.conn);
        conn.execute(
            "INSERT OR REPLACE INTO model_pricing (
                model_id, display_name, input_cost_per_million, output_cost_per_million,
                cache_read_cost_per_million, cache_creation_cost_per_million
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                record.model_id,
                record.display_name,
                record.input_cost_per_million,
                record.output_cost_per_million,
                record.cache_read_cost_per_million,
                record.cache_creation_cost_per_million,
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn delete_model_pricing(&self, model_id: &str) -> Result<bool, AppError> {
        let conn = lock_conn!(self.conn);
        let affected = conn
            .execute(
                "DELETE FROM model_pricing WHERE model_id = ?1",
                params![model_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(affected > 0)
    }

    pub fn get_model_pricing(&self, model_id: &str) -> Result<Option<ModelPricing>, AppError> {
        let conn = lock_conn!(self.conn);
        for candidate in model_pricing_candidates(model_id) {
            if let Some(pricing) = conn
                .query_row(
                    "SELECT input_cost_per_million, output_cost_per_million,
                            cache_read_cost_per_million, cache_creation_cost_per_million
                     FROM model_pricing
                     WHERE model_id = ?1",
                    params![candidate],
                    |row| {
                        let input: String = row.get(0)?;
                        let output: String = row.get(1)?;
                        let cache_read: String = row.get(2)?;
                        let cache_creation: String = row.get(3)?;
                        Ok((input, output, cache_read, cache_creation))
                    },
                )
                .optional()
                .map_err(|e| AppError::Database(e.to_string()))?
            {
                return ModelPricing::from_strings(&pricing.0, &pricing.1, &pricing.2, &pricing.3)
                    .map(Some)
                    .map_err(|e| {
                        AppError::Database(format!("Failed to parse model pricing: {e}"))
                    });
            }
        }
        Ok(None)
    }

    pub fn get_proxy_pricing_config(&self, app_type: &str) -> Result<(String, String), AppError> {
        let config = self.get_proxy_config()?;
        let app_config: ProxyAppSettings = match app_type {
            "claude" => config.apps.claude,
            "codex" => config.apps.codex,
            "gemini" => config.apps.gemini,
            "opencode" => config.apps.opencode,
            _ => ProxyAppSettings::default(),
        };
        Ok((
            app_config.default_cost_multiplier,
            app_config.pricing_model_source,
        ))
    }
}

fn validate_model_pricing_record(record: &ModelPricingRecord) -> Result<(), AppError> {
    if record.model_id.trim().is_empty() {
        return Err(AppError::InvalidInput("model_id is required".to_string()));
    }
    if record.display_name.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "display_name is required".to_string(),
        ));
    }
    ModelPricing::from_strings(
        &record.input_cost_per_million,
        &record.output_cost_per_million,
        &record.cache_read_cost_per_million,
        &record.cache_creation_cost_per_million,
    )
    .map_err(|e| AppError::InvalidInput(format!("Invalid model pricing value: {e}")))?;
    Ok(())
}

fn model_pricing_candidates(model_id: &str) -> Vec<String> {
    let cleaned = clean_model_id_for_pricing(model_id);
    if cleaned.is_empty() || matches!(cleaned.as_str(), "unknown" | "null" | "none") {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    push_unique(&mut candidates, cleaned.clone());
    if let Some((_, rest)) = cleaned.rsplit_once('/') {
        push_unique(&mut candidates, rest.to_string());
    }
    if let Some((base, _)) = cleaned.split_once(':') {
        push_unique(&mut candidates, base.to_string());
    }
    if let Some(stripped) = strip_model_date_suffix(&cleaned) {
        push_unique(&mut candidates, stripped);
    }
    if let Some(stripped) = strip_reasoning_effort_suffix(&cleaned) {
        push_unique(&mut candidates, stripped);
    }
    if let Some(pos) = cleaned.rfind("claude-") {
        push_unique(&mut candidates, cleaned[pos..].to_string());
    }
    candidates
}

fn clean_model_id_for_pricing(model_id: &str) -> String {
    model_id
        .trim()
        .replace('@', "-")
        .to_ascii_lowercase()
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn push_unique(candidates: &mut Vec<String>, candidate: String) {
    if !candidate.is_empty() && !candidates.iter().any(|existing| existing == &candidate) {
        candidates.push(candidate);
    }
}

fn strip_model_date_suffix(model_id: &str) -> Option<String> {
    let (base, suffix) = model_id.rsplit_once('-')?;
    (!base.is_empty() && suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_digit()))
        .then(|| base.to_string())
}

fn strip_reasoning_effort_suffix(model_id: &str) -> Option<String> {
    for suffix in ["-minimal", "-low", "-medium", "-high", "-xhigh"] {
        if let Some(stripped) = model_id.strip_suffix(suffix) {
            return Some(stripped.to_string());
        }
    }
    None
}
