//! Tollgate gate-evaluation verb (1 plugin verb) — YAML-first
//! re-implementation of `tollgate.check-gate` from
//! `rust/config/verbs/tollgate.yaml`.
//!
//! Dispatches to per-gate evaluation (SKELETON_READY /
//! EVIDENCE_COMPLETE / REVIEW_COMPLETE), records the result in
//! `tollgate_evaluations`, and returns pass/fail with detailed
//! check breakdown.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{json_extract_string, json_extract_uuid};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GateCheckResult {
    criterion: String,
    passed: bool,
    actual_value: Value,
    threshold_value: Value,
    detail: String,
}

async fn evaluate_skeleton_ready(
    scope: &mut dyn TransactionScope,
    case_id: Uuid,
    thresholds: &Value,
) -> Result<(bool, Vec<GateCheckResult>)> {
    let mut checks = Vec::new();
    let mut all_passed = true;

    let ownership_threshold = thresholds
        .get("ownership_coverage_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(70.0);
    let (total, proved): (i64, i64) = sqlx::query_as(
        r#"SELECT COUNT(*), COUNT(*) FILTER (WHERE ownership_proved = TRUE)
           FROM "ob-poc".entity_workstreams WHERE case_id = $1"#,
    )
    .bind(case_id)
    .fetch_one(scope.executor())
    .await
    .unwrap_or((0, 0));
    let ownership_pct = if total > 0 {
        (proved as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    let passed = ownership_pct >= ownership_threshold;
    if !passed {
        all_passed = false;
    }
    checks.push(GateCheckResult {
        criterion: "ownership_coverage_pct".into(),
        passed,
        actual_value: json!(ownership_pct),
        threshold_value: json!(ownership_threshold),
        detail: format!(
            "Ownership coverage {:.1}% (threshold: {:.1}%): {} of {} entities proved",
            ownership_pct, ownership_threshold, proved, total
        ),
    });

    let (entities_without_edges,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*)
           FROM "ob-poc".entity_workstreams ew
           WHERE ew.case_id = $1
             AND NOT EXISTS (
                 SELECT 1 FROM "ob-poc".cbu_ownership_graph og
                 JOIN "ob-poc".cases c ON c.case_id = $1
                 WHERE og.cbu_id = c.cbu_id
                   AND (og.from_entity_id = ew.entity_id OR og.to_entity_id = ew.entity_id)
             )"#,
    )
    .bind(case_id)
    .fetch_one(scope.executor())
    .await
    .unwrap_or((0,));
    let passed = entities_without_edges == 0;
    if !passed {
        all_passed = false;
    }
    checks.push(GateCheckResult {
        criterion: "all_entities_have_ownership_edge".into(),
        passed,
        actual_value: json!(entities_without_edges),
        threshold_value: json!(0),
        detail: format!(
            "{} workstream entities without ownership edges",
            entities_without_edges
        ),
    });

    if let Some(min_sources) = thresholds
        .get("minimum_sources_consulted")
        .and_then(|v| v.as_i64())
    {
        let (source_count,): (i64,) = sqlx::query_as(
            r#"SELECT COUNT(DISTINCT run_id)
               FROM "ob-poc".ubo_determination_runs WHERE case_id = $1"#,
        )
        .bind(case_id)
        .fetch_one(scope.executor())
        .await
        .unwrap_or((0,));
        let passed = source_count >= min_sources;
        if !passed {
            all_passed = false;
        }
        checks.push(GateCheckResult {
            criterion: "minimum_sources_consulted".into(),
            passed,
            actual_value: json!(source_count),
            threshold_value: json!(min_sources),
            detail: format!(
                "{} sources consulted (minimum: {})",
                source_count, min_sources
            ),
        });
    }

    Ok((all_passed, checks))
}

async fn check_pct(
    scope: &mut dyn TransactionScope,
    case_id: Uuid,
    sql: &str,
    thresholds: &Value,
    key: &str,
    default_threshold: f64,
    label: &str,
) -> Result<(bool, GateCheckResult)> {
    let threshold = thresholds
        .get(key)
        .and_then(|v| v.as_f64())
        .unwrap_or(default_threshold);
    let (total, count): (i64, i64) = sqlx::query_as(sql)
        .bind(case_id)
        .fetch_one(scope.executor())
        .await
        .unwrap_or((0, 0));
    let pct = if total > 0 {
        (count as f64 / total as f64) * 100.0
    } else {
        100.0
    };
    let passed = pct >= threshold;
    Ok((
        passed,
        GateCheckResult {
            criterion: key.into(),
            passed,
            actual_value: json!(pct),
            threshold_value: json!(threshold),
            detail: format!("{} {:.1}% (threshold: {:.1}%)", label, pct, threshold),
        },
    ))
}

async fn evaluate_evidence_complete(
    scope: &mut dyn TransactionScope,
    case_id: Uuid,
    thresholds: &Value,
) -> Result<(bool, Vec<GateCheckResult>)> {
    let mut checks = Vec::new();
    let mut all_passed = true;

    let (passed, check) = check_pct(
        scope,
        case_id,
        r#"SELECT COUNT(*), COUNT(*) FILTER (WHERE ownership_proved = TRUE)
           FROM "ob-poc".entity_workstreams WHERE case_id = $1"#,
        thresholds,
        "ownership_coverage_pct",
        95.0,
        "Ownership coverage",
    )
    .await?;
    if !passed {
        all_passed = false;
    }
    checks.push(check);

    let (passed, check) = check_pct(
        scope,
        case_id,
        r#"SELECT COUNT(*), COUNT(*) FILTER (WHERE identity_verified = TRUE)
           FROM "ob-poc".entity_workstreams WHERE case_id = $1"#,
        thresholds,
        "identity_docs_verified_pct",
        100.0,
        "Identity verification",
    )
    .await?;
    if !passed {
        all_passed = false;
    }
    checks.push(check);

    let (passed, check) = check_pct(
        scope,
        case_id,
        r#"SELECT COUNT(*), COUNT(*) FILTER (WHERE screening_cleared = TRUE)
           FROM "ob-poc".entity_workstreams WHERE case_id = $1"#,
        thresholds,
        "screening_cleared_pct",
        100.0,
        "Screening cleared",
    )
    .await?;
    if !passed {
        all_passed = false;
    }
    checks.push(check);

    let max_outstanding = thresholds
        .get("outreach_plan_items_max")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let (outstanding,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*)
           FROM "ob-poc".outreach_items oi
           JOIN "ob-poc".outreach_plans op ON op.plan_id = oi.plan_id
           WHERE op.case_id = $1 AND oi.status = 'PENDING'"#,
    )
    .bind(case_id)
    .fetch_one(scope.executor())
    .await
    .unwrap_or((0,));
    let passed = outstanding <= max_outstanding;
    if !passed {
        all_passed = false;
    }
    checks.push(GateCheckResult {
        criterion: "outreach_plan_items_max".into(),
        passed,
        actual_value: json!(outstanding),
        threshold_value: json!(max_outstanding),
        detail: format!(
            "{} outstanding outreach items (max: {})",
            outstanding, max_outstanding
        ),
    });

    Ok((all_passed, checks))
}

async fn evaluate_review_complete(
    scope: &mut dyn TransactionScope,
    case_id: Uuid,
    thresholds: &Value,
) -> Result<(bool, Vec<GateCheckResult>)> {
    let mut checks = Vec::new();
    let mut all_passed = true;

    if thresholds
        .get("all_workstreams_closed")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
    {
        let (open,): (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".entity_workstreams
               WHERE case_id = $1 AND status NOT IN ('CLOSED', 'COMPLETED')"#,
        )
        .bind(case_id)
        .fetch_one(scope.executor())
        .await
        .unwrap_or((0,));
        let passed = open == 0;
        if !passed {
            all_passed = false;
        }
        checks.push(GateCheckResult {
            criterion: "all_workstreams_closed".into(),
            passed,
            actual_value: json!(open),
            threshold_value: json!(0),
            detail: format!("{} workstreams still open", open),
        });
    }

    if thresholds
        .get("all_ubos_approved")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
    {
        let (unapproved,): (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".entity_workstreams
               WHERE case_id = $1 AND is_ubo = TRUE
                 AND (evidence_complete IS NULL OR evidence_complete = FALSE)"#,
        )
        .bind(case_id)
        .fetch_one(scope.executor())
        .await
        .unwrap_or((0,));
        let passed = unapproved == 0;
        if !passed {
            all_passed = false;
        }
        checks.push(GateCheckResult {
            criterion: "all_ubos_approved".into(),
            passed,
            actual_value: json!(unapproved),
            threshold_value: json!(0),
            detail: format!("{} UBOs not yet approved", unapproved),
        });
    }

    if thresholds
        .get("no_open_discrepancies")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
    {
        let (open_findings,): (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*)
               FROM "ob-poc".ownership_reconciliation_findings orf
               JOIN "ob-poc".ownership_reconciliation_runs orr ON orr.reconciliation_run_id = orf.reconciliation_run_id
               WHERE orr.case_id = $1 AND orf.status IN ('OPEN', 'UNDER_REVIEW')"#,
        )
        .bind(case_id)
        .fetch_one(scope.executor())
        .await
        .unwrap_or((0,));
        let passed = open_findings == 0;
        if !passed {
            all_passed = false;
        }
        checks.push(GateCheckResult {
            criterion: "no_open_discrepancies".into(),
            passed,
            actual_value: json!(open_findings),
            threshold_value: json!(0),
            detail: format!("{} open reconciliation discrepancies", open_findings),
        });
    }

    Ok((all_passed, checks))
}

pub struct CheckGate;

#[async_trait]
impl SemOsVerbOp for CheckGate {
    fn fqn(&self) -> &str {
        "tollgate.check-gate"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let case_id = json_extract_uuid(args, ctx, "case-id")?;
        let gate_name = json_extract_string(args, "gate-name")?;

        let valid_gates = ["SKELETON_READY", "EVIDENCE_COMPLETE", "REVIEW_COMPLETE"];
        if !valid_gates.contains(&gate_name.as_str()) {
            return Err(anyhow!(
                "Invalid gate-name '{}'. Valid values: {}",
                gate_name,
                valid_gates.join(", ")
            ));
        }

        let case_info: Option<(Uuid, String)> =
            sqlx::query_as(r#"SELECT case_id, status FROM "ob-poc".cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_optional(scope.executor())
                .await?;
        let (_, case_status) = case_info.ok_or_else(|| anyhow!("Case not found: {}", case_id))?;

        #[derive(sqlx::FromRow)]
        struct GateRow {
            tollgate_id: String,
            display_name: String,
            required_status: Option<String>,
            default_thresholds: Value,
        }

        let gate_row: GateRow = sqlx::query_as(
            r#"SELECT tollgate_id, display_name, required_status, default_thresholds
               FROM "ob-poc".tollgate_definitions WHERE tollgate_id = $1"#,
        )
        .bind(&gate_name)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Gate definition not found: {}", gate_name))?;

        if let Some(ref required_status) = gate_row.required_status {
            if case_status != *required_status {
                let detail = json!({
                    "gate_name": gate_row.tollgate_id,
                    "display_name": gate_row.display_name,
                    "passed": false,
                    "reason": format!(
                        "Case status '{}' does not match required status '{}'",
                        case_status, required_status
                    ),
                    "checks": []
                });
                let (eval_id, evaluated_at): (Uuid, String) = sqlx::query_as(
                    r#"INSERT INTO "ob-poc".tollgate_evaluations (
                        case_id, tollgate_id, passed, evaluation_detail, config_version
                    ) VALUES ($1, $2, FALSE, $3, 'v1')
                    RETURNING evaluation_id, evaluated_at::text"#,
                )
                .bind(case_id)
                .bind(&gate_row.tollgate_id)
                .bind(&detail)
                .fetch_one(scope.executor())
                .await?;

                return Ok(VerbExecutionOutcome::Record(json!({
                    "evaluation_id": eval_id,
                    "case_id": case_id,
                    "gate_name": gate_row.tollgate_id,
                    "passed": false,
                    "evaluation_detail": detail,
                    "evaluated_at": evaluated_at,
                })));
            }
        }

        let (passed, checks) = match gate_name.as_str() {
            "SKELETON_READY" => {
                evaluate_skeleton_ready(scope, case_id, &gate_row.default_thresholds).await?
            }
            "EVIDENCE_COMPLETE" => {
                evaluate_evidence_complete(scope, case_id, &gate_row.default_thresholds).await?
            }
            "REVIEW_COMPLETE" => {
                evaluate_review_complete(scope, case_id, &gate_row.default_thresholds).await?
            }
            _ => return Err(anyhow!("No evaluation logic for gate: {}", gate_name)),
        };

        let gaps: Vec<Value> = checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| {
                json!({
                    "criterion": c.criterion,
                    "actual": c.actual_value,
                    "threshold": c.threshold_value,
                    "detail": c.detail,
                })
            })
            .collect();

        let evaluation_detail = json!({
            "gate_name": gate_row.tollgate_id,
            "display_name": gate_row.display_name,
            "passed": passed,
            "checks": checks,
            "thresholds_used": gate_row.default_thresholds,
            "case_status": case_status,
        });
        let gaps_json = if gaps.is_empty() {
            Value::Null
        } else {
            json!(gaps)
        };

        let (eval_id, evaluated_at): (Uuid, String) = sqlx::query_as(
            r#"INSERT INTO "ob-poc".tollgate_evaluations (
                case_id, tollgate_id, passed, evaluation_detail, gaps, config_version
            ) VALUES ($1, $2, $3, $4, $5, 'v1')
            RETURNING evaluation_id, evaluated_at::text"#,
        )
        .bind(case_id)
        .bind(&gate_row.tollgate_id)
        .bind(passed)
        .bind(&evaluation_detail)
        .bind(&gaps_json)
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "evaluation_id": eval_id,
            "case_id": case_id,
            "gate_name": gate_row.tollgate_id,
            "passed": passed,
            "evaluation_detail": evaluation_detail,
            "evaluated_at": evaluated_at,
        })))
    }
}
