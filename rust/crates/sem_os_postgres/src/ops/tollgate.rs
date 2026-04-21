//! Tollgate decision-engine verbs (4 plugin verbs) — SemOS-side
//! YAML-first re-implementation of the plugin subset of
//! `rust/config/verbs/tollgate.yaml`.
//!
//! - `evaluate` — compute metrics (ownership / control / UBO /
//!   doc / screening / red-flag / allegation / refresh staleness),
//!   compare against configurable thresholds, emit PASS /
//!   PASS_WITH_WARNINGS / FAIL + score, persist the
//!   `tollgate_evaluations` row.
//! - `get-metrics` — projection of the same metrics bundle.
//! - `override` — record an override row for a failed evaluation.
//! - `get-decision-readiness` — aggregate across all evaluations
//!   for a case, surface blocking issues + recommended actions.
//!
//! Shared `compute_metrics` + `compute_doc_completeness_pct`
//! helpers take `&mut dyn TransactionScope` so every metric query
//! participates in the Sequencer txn. Document requirements
//! service (`GovernedDocumentRequirementsService`) still takes
//! `PgPool` — transitional `scope.pool().clone()`.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::document_requirements::GovernedDocumentRequirementsService;
use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Debug, serde::Serialize)]
struct TollgateMetrics {
    ownership_verified_pct: Option<Decimal>,
    control_verified_pct: Option<Decimal>,
    ubo_coverage_pct: Option<Decimal>,
    doc_completeness_pct: Option<Decimal>,
    screening_clear_pct: Option<Decimal>,
    red_flag_count: i64,
    allegation_unresolved_count: i64,
    days_since_last_refresh: Option<i64>,
}

async fn compute_metrics(
    scope: &mut dyn TransactionScope,
    cbu_id: Uuid,
    case_id: Uuid,
) -> Result<TollgateMetrics> {
    let ownership_stats: (i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*), COUNT(*) FILTER (WHERE status = 'proven')
        FROM "ob-poc".cbu_relationship_verification
        WHERE cbu_id = $1
        "#,
    )
    .bind(cbu_id)
    .fetch_one(scope.executor())
    .await
    .unwrap_or((0, 0));

    let ownership_verified_pct = if ownership_stats.0 > 0 {
        Some(Decimal::from(ownership_stats.1 * 100) / Decimal::from(ownership_stats.0))
    } else {
        None
    };
    let control_verified_pct = ownership_verified_pct;

    let (ubo_coverage_pct,): (Option<Decimal>,) = sqlx::query_as(
        r#"
        SELECT COALESCE(SUM(ownership_percentage), 0)
        FROM "ob-poc".kyc_ubo_registry
        WHERE cbu_id = $1 AND workflow_status = 'VERIFIED'
        "#,
    )
    .bind(cbu_id)
    .fetch_one(scope.executor())
    .await
    .unwrap_or((None,));

    let doc_completeness_pct = compute_doc_completeness_pct(scope, cbu_id, case_id).await?;

    let screening_stats: (i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*), COUNT(*) FILTER (WHERE status IN ('CLEAR', 'HIT_DISMISSED'))
        FROM "ob-poc".screenings s
        JOIN "ob-poc".entity_workstreams ew ON s.workstream_id = ew.workstream_id
        WHERE ew.case_id = $1
        "#,
    )
    .bind(case_id)
    .fetch_one(scope.executor())
    .await
    .unwrap_or((0, 0));
    let screening_clear_pct = if screening_stats.0 > 0 {
        Some(Decimal::from(screening_stats.1 * 100) / Decimal::from(screening_stats.0))
    } else {
        None
    };

    let (red_flag_count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM "ob-poc".red_flags
        WHERE case_id = $1 AND status IN ('OPEN', 'UNDER_REVIEW', 'BLOCKING')
        "#,
    )
    .bind(case_id)
    .fetch_one(scope.executor())
    .await
    .unwrap_or((0,));

    let (allegation_unresolved_count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM "ob-poc".client_allegations
        WHERE cbu_id = $1 AND verification_status = 'PENDING'
        "#,
    )
    .bind(cbu_id)
    .fetch_one(scope.executor())
    .await
    .unwrap_or((0,));

    let last_activity: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT EXTRACT(DAY FROM (now() - last_activity_at))::bigint
        FROM "ob-poc".cases
        WHERE case_id = $1
        "#,
    )
    .bind(case_id)
    .fetch_optional(scope.executor())
    .await?;

    Ok(TollgateMetrics {
        ownership_verified_pct,
        control_verified_pct,
        ubo_coverage_pct,
        doc_completeness_pct,
        screening_clear_pct,
        red_flag_count,
        allegation_unresolved_count,
        days_since_last_refresh: last_activity.map(|l| l.0),
    })
}

async fn compute_doc_completeness_pct(
    scope: &mut dyn TransactionScope,
    cbu_id: Uuid,
    case_id: Uuid,
) -> Result<Option<Decimal>> {
    let governed_service = GovernedDocumentRequirementsService::new(scope.pool().clone());

    let entity_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT entity_id
        FROM "ob-poc".cbu_entity_roles
        WHERE cbu_id = $1
        ORDER BY entity_id
        "#,
    )
    .bind(cbu_id)
    .fetch_all(scope.executor())
    .await
    .unwrap_or_default();

    let mut mandatory_total = 0usize;
    let mut mandatory_satisfied = 0usize;
    let mut matched_any_governed_profile = false;
    for entity_id in entity_ids {
        if let Some(matrix) = governed_service.compute_matrix_for_entity(entity_id).await? {
            matched_any_governed_profile = true;
            mandatory_total += matrix.mandatory_obligations;
            mandatory_satisfied += matrix.mandatory_satisfied_obligations;
        }
    }

    if matched_any_governed_profile {
        let pct = if mandatory_total == 0 {
            Decimal::from(100)
        } else {
            Decimal::from((mandatory_satisfied * 100) as i64) / Decimal::from(mandatory_total as i64)
        };
        return Ok(Some(pct));
    }

    let doc_stats: (i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*), COUNT(*) FILTER (WHERE status IN ('VERIFIED', 'WAIVED'))
        FROM "ob-poc".doc_requests dr
        JOIN "ob-poc".entity_workstreams ew ON dr.workstream_id = ew.workstream_id
        WHERE ew.case_id = $1
        "#,
    )
    .bind(case_id)
    .fetch_one(scope.executor())
    .await
    .unwrap_or((0, 0));

    Ok(Some(if doc_stats.0 > 0 {
        Decimal::from(doc_stats.1 * 100) / Decimal::from(doc_stats.0)
    } else {
        Decimal::from(100)
    }))
}

// ── tollgate.evaluate ─────────────────────────────────────────────────────────

pub struct Evaluate;

#[async_trait]
impl SemOsVerbOp for Evaluate {
    fn fqn(&self) -> &str {
        "tollgate.evaluate"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let case_id = json_extract_uuid(args, ctx, "case-id")?;
        let evaluation_type = json_extract_string(args, "evaluation-type")?;
        let evaluated_by = json_extract_string_opt(args, "evaluated-by");

        let case_info: Option<(Uuid,)> =
            sqlx::query_as(r#"SELECT cbu_id FROM "ob-poc".cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_optional(scope.executor())
                .await?;
        let (cbu_id,) = case_info.ok_or_else(|| anyhow!("Case not found"))?;
        let metrics = compute_metrics(scope, cbu_id, case_id).await?;

        #[derive(sqlx::FromRow)]
        struct ThresholdRow {
            threshold_name: String,
            metric_type: String,
            comparison: String,
            threshold_value: Option<Decimal>,
            is_blocking: bool,
            weight: Option<Decimal>,
        }

        let thresholds: Vec<ThresholdRow> = sqlx::query_as(
            r#"
            SELECT threshold_name, metric_type, comparison, threshold_value, is_blocking, weight
            FROM "ob-poc".tollgate_thresholds
            WHERE $1 = ANY(applies_to_case_types) OR applies_to_case_types IS NULL
            ORDER BY is_blocking DESC, threshold_name
            "#,
        )
        .bind(&evaluation_type)
        .fetch_all(scope.executor())
        .await?;

        let mut blocking_failures: Vec<Value> = Vec::new();
        let mut warnings: Vec<Value> = Vec::new();
        let mut score = Decimal::from(100);

        for threshold in &thresholds {
            let metric_value = match threshold.metric_type.as_str() {
                "OWNERSHIP_VERIFIED_PCT" => metrics.ownership_verified_pct,
                "CONTROL_VERIFIED_PCT" => metrics.control_verified_pct,
                "UBO_COVERAGE_PCT" => metrics.ubo_coverage_pct,
                "DOC_COMPLETENESS_PCT" => metrics.doc_completeness_pct,
                "SCREENING_CLEAR_PCT" => metrics.screening_clear_pct,
                "RED_FLAG_COUNT" => Some(Decimal::from(metrics.red_flag_count)),
                "ALLEGATION_UNRESOLVED_COUNT" => {
                    Some(Decimal::from(metrics.allegation_unresolved_count))
                }
                "DAYS_SINCE_REFRESH" => metrics.days_since_last_refresh.map(Decimal::from),
                _ => None,
            };

            if let (Some(value), Some(threshold_val)) = (metric_value, threshold.threshold_value) {
                let passed = match threshold.comparison.as_str() {
                    "GT" => value > threshold_val,
                    "GTE" => value >= threshold_val,
                    "LT" => value < threshold_val,
                    "LTE" => value <= threshold_val,
                    "EQ" => value == threshold_val,
                    "NEQ" => value != threshold_val,
                    _ => true,
                };

                if !passed {
                    let failure = json!({
                        "threshold": threshold.threshold_name,
                        "metric_type": threshold.metric_type,
                        "actual_value": value.to_string(),
                        "comparison": threshold.comparison,
                        "threshold_value": threshold_val.to_string(),
                        "is_blocking": threshold.is_blocking,
                    });
                    let weight = threshold
                        .weight
                        .unwrap_or_else(|| Decimal::from(1))
                        .to_string()
                        .parse::<f64>()
                        .unwrap_or(1.0);

                    if threshold.is_blocking {
                        blocking_failures.push(failure);
                        score -= Decimal::from((20.0 * weight) as i64);
                    } else {
                        warnings.push(failure);
                        score -= Decimal::from((5.0 * weight) as i64);
                    }
                }
            }
        }

        if score < Decimal::ZERO {
            score = Decimal::ZERO;
        }

        let overall_result = if !blocking_failures.is_empty() {
            "FAIL"
        } else if !warnings.is_empty() {
            "PASS_WITH_WARNINGS"
        } else {
            "PASS"
        };

        let threshold_results: Value = thresholds
            .iter()
            .map(|t| {
                (
                    t.threshold_name.clone(),
                    json!({
                        "metric_type": t.metric_type,
                        "comparison": t.comparison,
                        "threshold_value": t.threshold_value.map(|v| v.to_string()),
                        "is_blocking": t.is_blocking,
                    }),
                )
            })
            .collect::<serde_json::Map<String, Value>>()
            .into();

        let blocking_failure_texts: Vec<String> =
            blocking_failures.iter().map(|f| f.to_string()).collect();
        let warning_texts: Vec<String> = warnings.iter().map(|w| w.to_string()).collect();

        let passed = overall_result == "PASS" || overall_result == "PASS_WITH_WARNINGS";
        let evaluation_detail = json!({
            "overall_result": overall_result,
            "score": score.to_string(),
            "metrics": metrics,
            "threshold_results": threshold_results,
            "blocking_failures": blocking_failure_texts,
            "warnings": warning_texts,
            "evaluated_by": evaluated_by,
        });

        let (evaluation_id,): (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO "ob-poc".tollgate_evaluations (
                case_id, tollgate_id, passed, evaluation_detail, config_version
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING evaluation_id
            "#,
        )
        .bind(case_id)
        .bind(&evaluation_type)
        .bind(passed)
        .bind(&evaluation_detail)
        .bind("1.0")
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "evaluation_id": evaluation_id,
            "case_id": case_id,
            "evaluation_type": evaluation_type,
            "overall_result": overall_result,
            "score": score.to_string(),
            "metrics": metrics,
            "blocking_failures": blocking_failures,
            "warnings": warnings,
        })))
    }
}

// ── tollgate.get-metrics ──────────────────────────────────────────────────────

pub struct GetMetrics;

#[async_trait]
impl SemOsVerbOp for GetMetrics {
    fn fqn(&self) -> &str {
        "tollgate.get-metrics"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        let case_id: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT case_id FROM "ob-poc".cases
            WHERE cbu_id = $1 AND status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN')
            ORDER BY opened_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(scope.executor())
        .await?;
        let case_id = case_id.map(|c| c.0).unwrap_or(Uuid::nil());

        let metrics = compute_metrics(scope, cbu_id, case_id).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(metrics)?))
    }
}

// ── tollgate.override ─────────────────────────────────────────────────────────

pub struct Override;

#[async_trait]
impl SemOsVerbOp for Override {
    fn fqn(&self) -> &str {
        "tollgate.override"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let evaluation_id = json_extract_uuid(args, ctx, "evaluation-id")?;
        let override_reason = json_extract_string(args, "override-reason")?;
        let approved_by = json_extract_string(args, "approved-by")?;
        let approval_authority = json_extract_string(args, "approval-authority")?;
        let conditions = json_extract_string_opt(args, "conditions");
        let expiry_date = json_extract_string_opt(args, "expiry-date");

        let eval: Option<(String,)> = sqlx::query_as(
            r#"SELECT overall_result FROM "ob-poc".tollgate_evaluations WHERE id = $1"#,
        )
        .bind(evaluation_id)
        .fetch_optional(scope.executor())
        .await?;
        let (result,) = eval.ok_or_else(|| anyhow!("Evaluation not found"))?;
        if result == "PASS" {
            return Err(anyhow!("Cannot override a passing evaluation"));
        }

        let (override_id,): (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO "ob-poc".tollgate_overrides (
                evaluation_id, override_reason, approved_by, approval_authority, conditions, expiry_date
            )
            VALUES ($1, $2, $3, $4, $5, $6::date)
            RETURNING id
            "#,
        )
        .bind(evaluation_id)
        .bind(&override_reason)
        .bind(&approved_by)
        .bind(&approval_authority)
        .bind(conditions)
        .bind(expiry_date)
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(override_id))
    }
}

// ── tollgate.get-decision-readiness ───────────────────────────────────────────

pub struct GetDecisionReadiness;

#[async_trait]
impl SemOsVerbOp for GetDecisionReadiness {
    fn fqn(&self) -> &str {
        "tollgate.get-decision-readiness"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let case_id = json_extract_uuid(args, ctx, "case-id")?;

        let case_info: Option<(Uuid, String)> =
            sqlx::query_as(r#"SELECT cbu_id, status FROM "ob-poc".cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_optional(scope.executor())
                .await?;
        let (cbu_id, case_status) = case_info.ok_or_else(|| anyhow!("Case not found"))?;

        let evaluations: Vec<(String, String, Decimal)> = sqlx::query_as(
            r#"
            SELECT DISTINCT ON (tollgate_id)
                tollgate_id, passed::text, COALESCE((evaluation_detail->>'score')::numeric, 0)
            FROM "ob-poc".tollgate_evaluations
            WHERE case_id = $1
            ORDER BY tollgate_id, evaluated_at DESC
            "#,
        )
        .bind(case_id)
        .fetch_all(scope.executor())
        .await?;

        let mut blocking_issues: Vec<Value> = Vec::new();
        let mut completion_summary: HashMap<String, Value> = HashMap::new();

        for (eval_type, result, score) in &evaluations {
            completion_summary.insert(
                eval_type.clone(),
                json!({ "result": result, "score": score.to_string() }),
            );
            if result == "FAIL" {
                blocking_issues.push(json!({
                    "evaluation_type": eval_type,
                    "issue": format!("{} failed with score {}", eval_type, score),
                }));
            }
        }

        for req in ["DISCOVERY_COMPLETE", "EVIDENCE_COMPLETE", "VERIFICATION_COMPLETE"] {
            if !completion_summary.contains_key(req) {
                blocking_issues.push(json!({
                    "evaluation_type": req,
                    "issue": format!("{} evaluation not yet performed", req),
                }));
            }
        }

        let metrics = compute_metrics(scope, cbu_id, case_id).await?;
        let mut recommended_actions: Vec<String> = Vec::new();
        if metrics.ownership_verified_pct.unwrap_or_default() < Decimal::from(100) {
            recommended_actions.push("Complete ownership verification".to_string());
        }
        if metrics.doc_completeness_pct.unwrap_or_default() < Decimal::from(100) {
            recommended_actions.push("Collect outstanding documents".to_string());
        }
        if metrics.red_flag_count > 0 {
            recommended_actions.push(format!(
                "Resolve {} outstanding red flags",
                metrics.red_flag_count
            ));
        }
        if metrics.screening_clear_pct.unwrap_or_default() < Decimal::from(100) {
            recommended_actions.push("Review pending screening results".to_string());
        }

        let is_decision_ready = blocking_issues.is_empty() && case_status == "REVIEW";

        Ok(VerbExecutionOutcome::Record(json!({
            "case_id": case_id,
            "case_status": case_status,
            "is_decision_ready": is_decision_ready,
            "blocking_issues": blocking_issues,
            "completion_summary": completion_summary,
            "recommended_actions": recommended_actions,
        })))
    }
}
