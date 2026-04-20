//! Economic exposure look-through plugin operations.
//!
//! Computes indirect ownership through fund chains with bounded recursion,
//! cycle detection, minimum percentage threshold, and role-profile-aware
//! stop conditions.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{json_extract_string_opt, json_extract_uuid};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

#[register_custom_op]
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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let root_entity_id = json_extract_uuid(args, ctx, "root-entity-id")?;

        let as_of_date_str = json_extract_string_opt(args, "as-of-date");
        let as_of_date = as_of_date_str
            .as_deref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let max_depth = args
            .get("max-depth")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(6);

        let min_pct_str = json_extract_string_opt(args, "min-pct");
        let min_pct = min_pct_str
            .as_deref()
            .and_then(|s| s.parse::<rust_decimal::Decimal>().ok())
            .unwrap_or_else(|| rust_decimal::Decimal::new(1, 4));

        let max_rows = args
            .get("max-rows")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(200);

        let stop_on_no_bo_data = args
            .get("stop-on-no-bo-data")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let stop_on_policy_none = args
            .get("stop-on-policy-none")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

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
            FROM "ob-poc".fn_compute_economic_exposure($1, $2, $3, $4, $5, $6, $7)
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

        let results: Vec<serde_json::Value> = rows.into_iter().map(|r| r.into()).collect();
        Ok(VerbExecutionOutcome::RecordSet(results))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;

        let as_of_date_str = json_extract_string_opt(args, "as-of-date");
        let as_of_date = as_of_date_str
            .as_deref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let threshold_pct_str = json_extract_string_opt(args, "threshold-pct");
        let threshold_pct = threshold_pct_str
            .as_deref()
            .and_then(|s| s.parse::<rust_decimal::Decimal>().ok())
            .unwrap_or_else(|| rust_decimal::Decimal::new(5, 0));

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
            FROM "ob-poc".fn_economic_exposure_summary($1, $2, $3)
            "#,
        )
        .bind(issuer_entity_id)
        .bind(as_of_date)
        .bind(threshold_pct)
        .fetch_all(pool)
        .await?;

        let results: Vec<serde_json::Value> = rows.into_iter().map(|r| r.into()).collect();
        Ok(VerbExecutionOutcome::RecordSet(results))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

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
