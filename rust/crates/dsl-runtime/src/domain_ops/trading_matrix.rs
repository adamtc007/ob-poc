//! Trading Matrix Custom Operations
//!
//! Plugin handlers for Investment Manager, Pricing Config, and SLA domains.
//! These operations require custom code because they involve:
//! - Complex scope matching logic (find IM for trade)
//! - Priority-based lookups (find pricing source)
//! - Multi-table joins (list open SLA breaches)

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::{FromRow, PgPool};

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{json_extract_string, json_extract_string_opt, json_extract_uuid};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

/// Row type for SLA breach query results
#[derive(FromRow)]
struct SlaBreachRow {
    breach_id: uuid::Uuid,
    breach_severity: Option<String>,
    breach_date: Option<chrono::NaiveDate>,
    root_cause_category: Option<String>,
    root_cause_description: Option<String>,
    remediation_status: Option<String>,
    remediation_plan: Option<String>,
    remediation_due_date: Option<chrono::NaiveDate>,
    escalated_to: Option<String>,
    commitment_id: uuid::Uuid,
    template_code: String,
    template_name: String,
    metric_code: String,
    metric_name: String,
    measured_value: Option<rust_decimal::Decimal>,
    target_value: Option<rust_decimal::Decimal>,
    period_start: Option<chrono::NaiveDate>,
    period_end: Option<chrono::NaiveDate>,
}

// ============================================================================
// Investment Manager Operations
// ============================================================================

/// Find IM assignment that covers given trade characteristics
///
/// Rationale: Complex scope matching - must check scope_all, scope_markets,
/// scope_instrument_classes, scope_currencies, scope_isda_asset_classes with
/// NULL = any semantics, priority ordering, and return instruction method.
#[register_custom_op]
pub struct FindImForTradeOp;

#[async_trait]
impl CustomOperation for FindImForTradeOp {
    fn domain(&self) -> &'static str {
        "investment-manager"
    }
    fn verb(&self) -> &'static str {
        "find-for-trade"
    }
    fn rationale(&self) -> &'static str {
        "Complex scope matching with priority ordering and NULL=any semantics"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use serde_json::json;

        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let instrument_class = json_extract_string(args, "instrument-class")?;
        let market = json_extract_string_opt(args, "market");
        let currency = json_extract_string_opt(args, "currency");
        let isda_asset_class = json_extract_string_opt(args, "isda-asset-class");

        let row = sqlx::query!(
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
            cbu_id,
            instrument_class,
            market,
            currency,
            isda_asset_class
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(VerbExecutionOutcome::Record(json!({
                "assignment_id": r.assignment_id.to_string(),
                "manager_lei": r.manager_lei,
                "manager_name": r.manager_name,
                "manager_role": r.manager_role,
                "priority": r.priority,
                "instruction_method": r.instruction_method,
                "scope_all": r.scope_all
            }))),
            None => Ok(VerbExecutionOutcome::Record(json!({
                "error": "no_matching_im",
                "message": format!(
                    "No IM assignment found for instrument_class={}, market={:?}, currency={:?}",
                    instrument_class, market, currency
                )
            }))),
        }
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// Pricing Config Operations
// ============================================================================

/// Find pricing source for given instrument characteristics
///
/// Rationale: Priority-based lookup with fallback chain and NULL=any semantics
/// for market and currency fields.
#[register_custom_op]
pub struct FindPricingForInstrumentOp;

#[async_trait]
impl CustomOperation for FindPricingForInstrumentOp {
    fn domain(&self) -> &'static str {
        "pricing-config"
    }
    fn verb(&self) -> &'static str {
        "find-for-instrument"
    }
    fn rationale(&self) -> &'static str {
        "Priority-based pricing source lookup with fallback chain"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use serde_json::json;

        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let instrument_class = json_extract_string(args, "instrument-class")?;
        let market = json_extract_string_opt(args, "market");
        let currency = json_extract_string_opt(args, "currency");

        let class_id: Option<uuid::Uuid> = sqlx::query_scalar!(
            r#"SELECT class_id FROM "ob-poc".instrument_classes WHERE code = $1"#,
            instrument_class
        )
        .fetch_optional(pool)
        .await?;

        let class_id = match class_id {
            Some(id) => id,
            None => {
                return Ok(VerbExecutionOutcome::Record(json!({
                    "error": "unknown_instrument_class",
                    "message": format!("Unknown instrument class: {}", instrument_class)
                })));
            }
        };

        let market_id: Option<uuid::Uuid> = if let Some(ref mic) = market {
            sqlx::query_scalar!(
                r#"SELECT market_id FROM "ob-poc".markets WHERE mic = $1"#,
                mic
            )
            .fetch_optional(pool)
            .await?
        } else {
            None
        };

        let row = sqlx::query!(
            r#"
            SELECT
                config_id,
                source,
                price_type,
                fallback_source,
                max_age_hours,
                tolerance_pct,
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
            cbu_id,
            class_id,
            market_id,
            currency
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(VerbExecutionOutcome::Record(json!({
                "config_id": r.config_id.to_string(),
                "source": r.source,
                "price_type": r.price_type,
                "fallback_source": r.fallback_source,
                "max_age_hours": r.max_age_hours,
                "tolerance_pct": r.tolerance_pct,
                "stale_action": r.stale_action,
                "priority": r.priority
            }))),
            None => Ok(VerbExecutionOutcome::Record(json!({
                "error": "no_pricing_config",
                "message": format!(
                    "No pricing config found for instrument_class={}, market={:?}, currency={:?}",
                    instrument_class, market, currency
                )
            }))),
        }
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// SLA Operations
// ============================================================================

/// List open SLA breaches for CBU
///
/// Rationale: Multi-table join across commitments, measurements, breaches
/// with template details, filtering by status and optional severity.
#[register_custom_op]
pub struct ListOpenSlaBreachesOp;

#[async_trait]
impl CustomOperation for ListOpenSlaBreachesOp {
    fn domain(&self) -> &'static str {
        "sla"
    }
    fn verb(&self) -> &'static str {
        "list-open-breaches"
    }
    fn rationale(&self) -> &'static str {
        "Multi-table join with template details and severity filtering"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use serde_json::json;

        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let severity = json_extract_string_opt(args, "severity");

        let rows: Vec<SlaBreachRow> = sqlx::query_as(
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
                m.measured_value,
                COALESCE(c.override_target_value, t.target_value) as target_value,
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
        .fetch_all(pool)
        .await?;

        let breaches: Vec<serde_json::Value> = rows
            .iter()
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
                    "period_end": r.period_end.map(|d| d.to_string())
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(breaches))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_im_for_trade_op_metadata() {
        let op = FindImForTradeOp;
        assert_eq!(op.domain(), "investment-manager");
        assert_eq!(op.verb(), "find-for-trade");
    }

    #[test]
    fn test_find_pricing_for_instrument_op_metadata() {
        let op = FindPricingForInstrumentOp;
        assert_eq!(op.domain(), "pricing-config");
        assert_eq!(op.verb(), "find-for-instrument");
    }

    #[test]
    fn test_list_open_sla_breaches_op_metadata() {
        let op = ListOpenSlaBreachesOp;
        assert_eq!(op.domain(), "sla");
        assert_eq!(op.verb(), "list-open-breaches");
    }
}
