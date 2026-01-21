//! Economic exposure look-through plugin operations.
//!
//! Computes indirect ownership through fund chains with:
//! - Bounded recursion (configurable depth limit)
//! - Cycle detection
//! - Minimum percentage threshold
//! - Role profile-aware stop conditions

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract a required UUID argument from verb call
#[cfg(feature = "database")]
fn get_required_uuid(
    verb_call: &VerbCall,
    key: &str,
    ctx: &ExecutionContext,
) -> Result<uuid::Uuid> {
    use uuid::Uuid;

    let arg = verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))?;

    // Try as symbol reference first
    if let Some(ref_name) = arg.value.as_symbol() {
        let resolved = ctx
            .resolve(ref_name)
            .ok_or_else(|| anyhow::anyhow!("Unresolved reference @{}", ref_name))?;
        return Ok(resolved);
    }

    // Try as UUID directly
    if let Some(uuid_val) = arg.value.as_uuid() {
        return Ok(uuid_val);
    }

    // Try as string (may be UUID string)
    if let Some(str_val) = arg.value.as_string() {
        return Uuid::parse_str(str_val)
            .map_err(|e| anyhow::anyhow!("Invalid UUID for :{}: {}", key, e));
    }

    Err(anyhow::anyhow!(":{} must be a UUID or @reference", key))
}

/// Extract an optional date argument from verb call
#[cfg(feature = "database")]
fn get_optional_date(verb_call: &VerbCall, key: &str) -> Option<chrono::NaiveDate> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string())
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
}

/// Extract an optional integer argument from verb call
#[cfg(feature = "database")]
fn get_optional_int(verb_call: &VerbCall, key: &str) -> Option<i32> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_integer().map(|i| i as i32))
}

/// Extract an optional decimal argument from verb call
#[cfg(feature = "database")]
fn get_optional_decimal(verb_call: &VerbCall, key: &str) -> Option<rust_decimal::Decimal> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| {
            a.value
                .as_decimal()
                .or_else(|| a.value.as_integer().map(rust_decimal::Decimal::from))
        })
}

/// Extract an optional boolean argument from verb call
#[cfg(feature = "database")]
fn get_optional_bool(verb_call: &VerbCall, key: &str) -> Option<bool> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_boolean())
}

// ============================================================================
// EconomicExposureComputeOp - Bounded look-through computation
// ============================================================================

/// Compute economic exposure from a root entity through ownership chains.
/// Uses bounded recursion with configurable depth, min percentage, and stop conditions.
pub struct EconomicExposureComputeOp;

#[async_trait]
impl CustomOperation for EconomicExposureComputeOp {
    fn domain(&self) -> &'static str {
        "economic-exposure"
    }

    fn verb(&self) -> &'static str {
        "compute"
    }

    fn rationale(&self) -> &'static str {
        "Complex recursive SQL function with multiple parameters and configurable stop conditions"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Required arguments
        let root_entity_id = get_required_uuid(verb_call, "root-entity-id", ctx)?;

        // Optional arguments with defaults
        let as_of_date = get_optional_date(verb_call, "as-of-date")
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let max_depth = get_optional_int(verb_call, "max-depth").unwrap_or(6);
        let min_pct = get_optional_decimal(verb_call, "min-pct")
            .unwrap_or_else(|| rust_decimal::Decimal::new(1, 4)); // 0.0001
        let max_rows = get_optional_int(verb_call, "max-rows").unwrap_or(200);
        let stop_on_no_bo_data = get_optional_bool(verb_call, "stop-on-no-bo-data").unwrap_or(true);
        let stop_on_policy_none =
            get_optional_bool(verb_call, "stop-on-policy-none").unwrap_or(true);

        // Call the SQL function
        let rows = sqlx::query_as::<_, ExposureRow>(
            r#"
            SELECT
                root_entity_id,
                leaf_entity_id,
                leaf_name,
                cumulative_pct,
                depth,
                path_entities,
                path_names,
                stopped_reason
            FROM kyc.fn_compute_economic_exposure($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(root_entity_id)
        .bind(as_of_date)
        .bind(max_depth)
        .bind(min_pct)
        .bind(max_rows)
        .bind(stop_on_no_bo_data)
        .bind(stop_on_policy_none)
        .fetch_all(pool)
        .await?;

        // Convert to JSON array
        let results: Vec<serde_json::Value> = rows.into_iter().map(|r| r.into()).collect();

        Ok(ExecutionResult::RecordSet(results))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "economic-exposure.compute requires database feature"
        ))
    }
}

// ============================================================================
// EconomicExposureSummaryOp - Aggregated exposure summary for an issuer
// ============================================================================

/// Get aggregated economic exposure summary for an issuer.
/// Shows both direct and look-through percentages with threshold filtering.
pub struct EconomicExposureSummaryOp;

#[async_trait]
impl CustomOperation for EconomicExposureSummaryOp {
    fn domain(&self) -> &'static str {
        "economic-exposure"
    }

    fn verb(&self) -> &'static str {
        "summary"
    }

    fn rationale(&self) -> &'static str {
        "Complex aggregation query combining direct holdings with look-through computation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Required arguments
        let issuer_entity_id = get_required_uuid(verb_call, "issuer-entity-id", ctx)?;

        // Optional arguments with defaults
        let as_of_date = get_optional_date(verb_call, "as-of-date")
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let threshold_pct = get_optional_decimal(verb_call, "threshold-pct")
            .unwrap_or_else(|| rust_decimal::Decimal::new(5, 0)); // 5.0

        // Call the SQL function
        let rows = sqlx::query_as::<_, ExposureSummaryRow>(
            r#"
            SELECT
                investor_entity_id,
                investor_name,
                direct_pct,
                lookthrough_pct,
                is_above_threshold,
                role_type,
                depth,
                stop_reason
            FROM kyc.fn_economic_exposure_summary($1, $2, $3)
            "#,
        )
        .bind(issuer_entity_id)
        .bind(as_of_date)
        .bind(threshold_pct)
        .fetch_all(pool)
        .await?;

        // Convert to JSON array
        let results: Vec<serde_json::Value> = rows.into_iter().map(|r| r.into()).collect();

        Ok(ExecutionResult::RecordSet(results))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "economic-exposure.summary requires database feature"
        ))
    }
}

// ============================================================================
// Row types for query results
// ============================================================================

#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct ExposureRow {
    root_entity_id: uuid::Uuid,
    leaf_entity_id: uuid::Uuid,
    leaf_name: Option<String>,
    cumulative_pct: rust_decimal::Decimal,
    depth: i32,
    path_entities: Vec<uuid::Uuid>,
    path_names: Option<Vec<String>>,
    stopped_reason: Option<String>,
}

#[cfg(feature = "database")]
impl From<ExposureRow> for serde_json::Value {
    fn from(row: ExposureRow) -> Self {
        serde_json::json!({
            "root_entity_id": row.root_entity_id,
            "leaf_entity_id": row.leaf_entity_id,
            "leaf_name": row.leaf_name,
            "cumulative_pct": row.cumulative_pct.to_string(),
            "depth": row.depth,
            "path_entities": row.path_entities,
            "path_names": row.path_names,
            "stopped_reason": row.stopped_reason
        })
    }
}

#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct ExposureSummaryRow {
    investor_entity_id: uuid::Uuid,
    investor_name: Option<String>,
    direct_pct: rust_decimal::Decimal,
    lookthrough_pct: rust_decimal::Decimal,
    is_above_threshold: bool,
    role_type: Option<String>,
    depth: i32,
    stop_reason: Option<String>,
}

#[cfg(feature = "database")]
impl From<ExposureSummaryRow> for serde_json::Value {
    fn from(row: ExposureSummaryRow) -> Self {
        serde_json::json!({
            "investor_entity_id": row.investor_entity_id,
            "investor_name": row.investor_name,
            "direct_pct": row.direct_pct.to_string(),
            "lookthrough_pct": row.lookthrough_pct.to_string(),
            "is_above_threshold": row.is_above_threshold,
            "role_type": row.role_type,
            "depth": row.depth,
            "stop_reason": row.stop_reason
        })
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Register economic exposure operations with the registry
pub fn register_economic_exposure_ops(registry: &mut crate::domain_ops::CustomOperationRegistry) {
    registry.register(Arc::new(EconomicExposureComputeOp));
    registry.register(Arc::new(EconomicExposureSummaryOp));
}
