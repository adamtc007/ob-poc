//! Economic-exposure verbs (2 plugin verbs) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/economic-exposure.yaml`.
//!
//! - `economic-exposure.compute` — bounded-recursion ownership
//!   look-through with min-pct / max-depth / max-rows caps and
//!   role-profile-aware stop conditions.
//! - `economic-exposure.summary` — aggregates direct holdings with
//!   look-through computation per investor.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{json_extract_string_opt, json_extract_uuid};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(sqlx::FromRow)]
struct ExposureRow {
    root_entity_id: Uuid,
    leaf_entity_id: Uuid,
    leaf_name: Option<String>,
    cumulative_pct: rust_decimal::Decimal,
    depth: i32,
    path_entities: Vec<Uuid>,
    path_names: Option<Vec<String>>,
    stopped_reason: Option<String>,
}

impl From<ExposureRow> for Value {
    fn from(row: ExposureRow) -> Self {
        json!({
            "root_entity_id": row.root_entity_id,
            "leaf_entity_id": row.leaf_entity_id,
            "leaf_name": row.leaf_name,
            "cumulative_pct": row.cumulative_pct.to_string(),
            "depth": row.depth,
            "path_entities": row.path_entities,
            "path_names": row.path_names,
            "stopped_reason": row.stopped_reason,
        })
    }
}

#[derive(sqlx::FromRow)]
struct ExposureSummaryRow {
    investor_entity_id: Uuid,
    investor_name: Option<String>,
    direct_pct: rust_decimal::Decimal,
    lookthrough_pct: rust_decimal::Decimal,
    is_above_threshold: bool,
    role_type: Option<String>,
    depth: i32,
    stop_reason: Option<String>,
}

impl From<ExposureSummaryRow> for Value {
    fn from(row: ExposureSummaryRow) -> Self {
        json!({
            "investor_entity_id": row.investor_entity_id,
            "investor_name": row.investor_name,
            "direct_pct": row.direct_pct.to_string(),
            "lookthrough_pct": row.lookthrough_pct.to_string(),
            "is_above_threshold": row.is_above_threshold,
            "role_type": row.role_type,
            "depth": row.depth,
            "stop_reason": row.stop_reason,
        })
    }
}

// ── economic-exposure.compute ─────────────────────────────────────────────────

pub struct Compute;

#[async_trait]
impl SemOsVerbOp for Compute {
    fn fqn(&self) -> &str {
        "economic-exposure.compute"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let root_entity_id = json_extract_uuid(args, ctx, "root-entity-id")?;

        let as_of_date = json_extract_string_opt(args, "as-of-date")
            .as_deref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let max_depth = args
            .get("max-depth")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(6);
        let min_pct = json_extract_string_opt(args, "min-pct")
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

        let rows: Vec<ExposureRow> = sqlx::query_as(
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
        .fetch_all(scope.executor())
        .await?;

        let results: Vec<Value> = rows.into_iter().map(Value::from).collect();
        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

// ── economic-exposure.summary ─────────────────────────────────────────────────

pub struct Summary;

#[async_trait]
impl SemOsVerbOp for Summary {
    fn fqn(&self) -> &str {
        "economic-exposure.summary"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let as_of_date = json_extract_string_opt(args, "as-of-date")
            .as_deref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let threshold_pct = json_extract_string_opt(args, "threshold-pct")
            .as_deref()
            .and_then(|s| s.parse::<rust_decimal::Decimal>().ok())
            .unwrap_or_else(|| rust_decimal::Decimal::new(5, 0));

        let rows: Vec<ExposureSummaryRow> = sqlx::query_as(
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
        .fetch_all(scope.executor())
        .await?;

        let results: Vec<Value> = rows.into_iter().map(Value::from).collect();
        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}
