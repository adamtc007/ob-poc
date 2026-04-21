//! Trading-matrix domain verbs (3 plugin verbs) — SemOS-side
//! YAML-first re-implementation of the plugin subset of
//! `rust/config/verbs/trading-matrix.yaml`.
//!
//! - `investment-manager.find-for-trade` — complex scope matching
//!   (scope_all, scope_markets, scope_instrument_classes,
//!   scope_currencies, scope_isda_asset_classes) with NULL=any
//!   semantics + priority ordering.
//! - `pricing-config.find-for-instrument` — priority-based
//!   pricing-source lookup with fallback chain.
//! - `sla.list-open-breaches` — multi-table join across breaches,
//!   commitments, templates, metric types, and measurements.
//!
//! Slice #10 pattern applied: `sqlx::query!` macros rewritten as
//! runtime `sqlx::query_as`. NUMERIC columns (measured_value /
//! target_value) are cast to text in SQL so we avoid pulling
//! `rust_decimal` into `sem_os_postgres`.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── investment-manager.find-for-trade ─────────────────────────────────────────

pub struct FindImForTrade;

#[async_trait]
impl SemOsVerbOp for FindImForTrade {
    fn fqn(&self) -> &str {
        "investment-manager.find-for-trade"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let instrument_class = json_extract_string(args, "instrument-class")?;
        let market = json_extract_string_opt(args, "market");
        let currency = json_extract_string_opt(args, "currency");
        let isda_asset_class = json_extract_string_opt(args, "isda-asset-class");

        type Row = (
            Uuid,              // assignment_id
            Option<String>,    // manager_lei
            Option<String>,    // manager_name
            Option<String>,    // manager_role
            Option<i32>,       // priority
            Option<String>,    // instruction_method
            bool,              // scope_all
        );

        let row: Option<Row> = sqlx::query_as(
            r#"
            SELECT
                assignment_id,
                manager_lei,
                manager_name,
                manager_role,
                priority,
                instruction_method,
                scope_all
            FROM "ob-poc".cbu_im_assignments
            WHERE cbu_id = $1
              AND status = 'ACTIVE'
              AND (
                  scope_all = true
                  OR (
                      (scope_instrument_classes IS NULL OR $2 = ANY(scope_instrument_classes))
                      AND (scope_markets IS NULL OR $3 = ANY(scope_markets) OR $3 IS NULL)
                      AND (scope_currencies IS NULL OR $4 = ANY(scope_currencies) OR $4 IS NULL)
                      AND (scope_isda_asset_classes IS NULL OR $5 = ANY(scope_isda_asset_classes) OR $5 IS NULL)
                  )
              )
            ORDER BY
                scope_all ASC,
                priority ASC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .bind(&instrument_class)
        .bind(&market)
        .bind(&currency)
        .bind(&isda_asset_class)
        .fetch_optional(scope.executor())
        .await?;

        let result = match row {
            Some((assignment_id, lei, name, role, priority, method, scope_all)) => json!({
                "assignment_id": assignment_id.to_string(),
                "manager_lei": lei,
                "manager_name": name,
                "manager_role": role,
                "priority": priority,
                "instruction_method": method,
                "scope_all": scope_all,
            }),
            None => json!({
                "error": "no_matching_im",
                "message": format!(
                    "No IM assignment found for instrument_class={}, market={:?}, currency={:?}",
                    instrument_class, market, currency
                ),
            }),
        };
        Ok(VerbExecutionOutcome::Record(result))
    }
}

// ── pricing-config.find-for-instrument ────────────────────────────────────────

pub struct FindPricingForInstrument;

#[async_trait]
impl SemOsVerbOp for FindPricingForInstrument {
    fn fqn(&self) -> &str {
        "pricing-config.find-for-instrument"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let instrument_class = json_extract_string(args, "instrument-class")?;
        let market = json_extract_string_opt(args, "market");
        let currency = json_extract_string_opt(args, "currency");

        let class_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT class_id FROM "ob-poc".instrument_classes WHERE code = $1"#,
        )
        .bind(&instrument_class)
        .fetch_optional(scope.executor())
        .await?;

        let class_id = match class_id {
            Some(id) => id,
            None => {
                return Ok(VerbExecutionOutcome::Record(json!({
                    "error": "unknown_instrument_class",
                    "message": format!("Unknown instrument class: {}", instrument_class),
                })));
            }
        };

        let market_id: Option<Uuid> = if let Some(ref mic) = market {
            sqlx::query_scalar(r#"SELECT market_id FROM "ob-poc".markets WHERE mic = $1"#)
                .bind(mic)
                .fetch_optional(scope.executor())
                .await?
        } else {
            None
        };

        type Row = (
            Uuid,              // config_id
            Option<String>,    // source
            Option<String>,    // price_type
            Option<String>,    // fallback_source
            Option<i32>,       // max_age_hours
            Option<String>,    // tolerance_pct (cast ::text)
            Option<String>,    // stale_action
            Option<i32>,       // priority
        );

        let row: Option<Row> = sqlx::query_as(
            r#"
            SELECT
                config_id,
                source,
                price_type,
                fallback_source,
                max_age_hours,
                tolerance_pct::text,
                stale_action,
                priority
            FROM "ob-poc".cbu_pricing_config
            WHERE cbu_id = $1
              AND is_active = true
              AND instrument_class_id = $2
              AND (market_id IS NULL OR market_id = $3 OR $3 IS NULL)
              AND (currency IS NULL OR currency = $4 OR $4 IS NULL)
            ORDER BY priority ASC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .bind(class_id)
        .bind(market_id)
        .bind(&currency)
        .fetch_optional(scope.executor())
        .await?;

        let result = match row {
            Some((config_id, source, price_type, fallback, max_age, tolerance, stale, priority)) => {
                json!({
                    "config_id": config_id.to_string(),
                    "source": source,
                    "price_type": price_type,
                    "fallback_source": fallback,
                    "max_age_hours": max_age,
                    "tolerance_pct": tolerance,
                    "stale_action": stale,
                    "priority": priority,
                })
            }
            None => json!({
                "error": "no_pricing_config",
                "message": format!(
                    "No pricing config found for instrument_class={}, market={:?}, currency={:?}",
                    instrument_class, market, currency
                ),
            }),
        };
        Ok(VerbExecutionOutcome::Record(result))
    }
}

// ── sla.list-open-breaches ────────────────────────────────────────────────────

pub struct ListOpenSlaBreaches;

#[async_trait]
impl SemOsVerbOp for ListOpenSlaBreaches {
    fn fqn(&self) -> &str {
        "sla.list-open-breaches"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let severity = json_extract_string_opt(args, "severity");

        // NUMERIC columns cast to ::text so we don't need rust_decimal.
        // sqlx::FromRow can't derive tuples > 16 fields, so this struct owns
        // the 18-column breach snapshot directly.
        #[derive(FromRow)]
        struct BreachRow {
            breach_id: Uuid,
            breach_severity: Option<String>,
            breach_date: Option<chrono::NaiveDate>,
            root_cause_category: Option<String>,
            root_cause_description: Option<String>,
            remediation_status: Option<String>,
            remediation_plan: Option<String>,
            remediation_due_date: Option<chrono::NaiveDate>,
            escalated_to: Option<String>,
            commitment_id: Uuid,
            template_code: String,
            template_name: String,
            metric_code: String,
            metric_name: String,
            measured_value: Option<String>,
            target_value: Option<String>,
            period_start: Option<chrono::NaiveDate>,
            period_end: Option<chrono::NaiveDate>,
        }

        let rows: Vec<BreachRow> = sqlx::query_as(
            r#"
            SELECT
                b.breach_id,
                b.breach_severity,
                b.breach_date,
                b.root_cause_category,
                b.root_cause_description,
                b.remediation_status,
                b.remediation_plan,
                b.remediation_due_date,
                b.escalated_to,
                c.commitment_id,
                t.template_code,
                t.name as template_name,
                mt.metric_code,
                mt.name as metric_name,
                m.measured_value::text,
                COALESCE(c.override_target_value, t.target_value)::text as target_value,
                m.period_start,
                m.period_end
            FROM "ob-poc".sla_breaches b
            JOIN "ob-poc".cbu_sla_commitments c ON b.commitment_id = c.commitment_id
            JOIN "ob-poc".sla_templates t ON c.template_id = t.template_id
            JOIN "ob-poc".sla_metric_types mt ON t.metric_code = mt.metric_code
            JOIN "ob-poc".sla_measurements m ON b.measurement_id = m.measurement_id
            WHERE c.cbu_id = $1
              AND b.remediation_status IN ('OPEN', 'IN_PROGRESS', 'ESCALATED')
              AND ($2::text IS NULL OR b.breach_severity = $2)
            ORDER BY
                CASE b.breach_severity
                    WHEN 'CRITICAL' THEN 1
                    WHEN 'MAJOR' THEN 2
                    WHEN 'MINOR' THEN 3
                END,
                b.breach_date DESC
            "#,
        )
        .bind(cbu_id)
        .bind(&severity)
        .fetch_all(scope.executor())
        .await?;

        let breaches: Vec<Value> = rows
            .into_iter()
            .map(|r| {
                json!({
                    "breach_id": r.breach_id.to_string(),
                    "breach_severity": r.breach_severity,
                    "breach_date": r.breach_date.map(|d| d.to_string()),
                    "root_cause_category": r.root_cause_category,
                    "root_cause_description": r.root_cause_description,
                    "remediation_status": r.remediation_status,
                    "remediation_plan": r.remediation_plan,
                    "remediation_due_date": r.remediation_due_date.map(|d| d.to_string()),
                    "escalated_to": r.escalated_to,
                    "commitment_id": r.commitment_id.to_string(),
                    "template_code": r.template_code,
                    "template_name": r.template_name,
                    "metric_code": r.metric_code,
                    "metric_name": r.metric_name,
                    "measured_value": r.measured_value,
                    "target_value": r.target_value,
                    "period_start": r.period_start.map(|d| d.to_string()),
                    "period_end": r.period_end.map(|d| d.to_string()),
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(breaches))
    }
}
