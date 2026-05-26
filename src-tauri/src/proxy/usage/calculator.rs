use super::parser::TokenUsage;
use crate::database::ModelPricing;
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct CostBreakdown {
    pub input_cost: Decimal,
    pub output_cost: Decimal,
    pub cache_read_cost: Decimal,
    pub cache_creation_cost: Decimal,
    pub total_cost: Decimal,
}

pub struct CostCalculator;

impl CostCalculator {
    pub fn try_calculate_for_app(
        app_type: &str,
        usage: &TokenUsage,
        pricing: Option<&ModelPricing>,
        cost_multiplier: Decimal,
    ) -> Option<CostBreakdown> {
        pricing.map(|pricing| {
            let input_includes_cache_read = matches!(app_type, "codex" | "gemini" | "opencode");
            Self::calculate_with_cache_semantics(
                usage,
                pricing,
                cost_multiplier,
                input_includes_cache_read,
            )
        })
    }

    fn calculate_with_cache_semantics(
        usage: &TokenUsage,
        pricing: &ModelPricing,
        cost_multiplier: Decimal,
        input_includes_cache_read: bool,
    ) -> CostBreakdown {
        let million = Decimal::from(1_000_000);
        let billable_input_tokens = if input_includes_cache_read {
            usage.input_tokens.saturating_sub(usage.cache_read_tokens)
        } else {
            usage.input_tokens
        };
        let input_cost =
            Decimal::from(billable_input_tokens) * pricing.input_cost_per_million / million;
        let output_cost =
            Decimal::from(usage.output_tokens) * pricing.output_cost_per_million / million;
        let cache_read_cost =
            Decimal::from(usage.cache_read_tokens) * pricing.cache_read_cost_per_million / million;
        let cache_creation_cost = Decimal::from(usage.cache_creation_tokens)
            * pricing.cache_creation_cost_per_million
            / million;
        let total_cost =
            (input_cost + output_cost + cache_read_cost + cache_creation_cost) * cost_multiplier;

        CostBreakdown {
            input_cost,
            output_cost,
            cache_read_cost,
            cache_creation_cost,
            total_cost,
        }
    }
}
