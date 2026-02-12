//! Tollgate Gate Evaluation Operations (Phase 2.6)
//!
//! Evaluates a specific named tollgate gate against the current case state.
//! Loads the gate definition from `ob_ref.tollgate_definitions`, computes
//! pass/fail against case data, and records the result in
//! `kyc.tollgate_evaluations`.
//!
//! ## Rationale
//! Gate evaluation requires custom code because:
//! - Each gate has different pass/fail criteria requiring bespoke SQL
//! - Threshold loading from reference data is dynamic (JSONB)
//! - Multi-table aggregation across workstreams, ownership graph, and sources

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::{extract_string, extract_uuid};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

// =============================================================================
// Result Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TollgateEvaluationResult {
    pub evaluation_id: Uuid,
    pub case_id: Uuid,
    pub gate_name: String,
    pub passed: bool,
    pub evaluation_detail: serde_json::Value,
    pub evaluated_at: String,
}

// =============================================================================
// Internal Types
// =============================================================================

/// Gate definition loaded from `ob_ref.tollgate_definitions`.
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
struct GateDefinition {
    tollgate_id: String,
    display_name: String,
    required_status: Option<String>,
    default_thresholds: serde_json::Value,
}

/// Result of evaluating a single gate's criteria.
#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GateCheckResult {
    criterion: String,
    passed: bool,
    actual_value: serde_json::Value,
    threshold_value: serde_json::Value,
    detail: String,
}

// =============================================================================
// Gate Evaluation Logic
// =============================================================================

/// Evaluate the SKELETON_READY gate.
///
/// Checks:
/// 1. ownership_coverage_pct >= threshold (default 70%)
/// 2. All workstream entities have at least one ownership edge
/// 3. Minimum sources consulted (if threshold defined)
#[cfg(feature = "database")]
async fn evaluate_skeleton_ready(
    pool: &PgPool,
    case_id: Uuid,
    thresholds: &serde_json::Value,
) -> Result<(bool, Vec<GateCheckResult>)> {
    let mut checks: Vec<GateCheckResult> = Vec::new();
    let mut all_passed = true;

    // ---------------------------------------------------------------
    // Check 1: Ownership coverage percentage
    // ---------------------------------------------------------------
    let ownership_threshold = thresholds
        .get("ownership_coverage_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(70.0);

    // Compute ownership coverage from workstream data
    let coverage_stats: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) AS total_entities,
            COUNT(*) FILTER (WHERE ownership_proved = TRUE) AS ownership_proved_count
        FROM kyc.entity_workstreams
        WHERE case_id = $1
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0, 0));

    let ownership_pct = if coverage_stats.0 > 0 {
        (coverage_stats.1 as f64 / coverage_stats.0 as f64) * 100.0
    } else {
        0.0
    };

    let ownership_passed = ownership_pct >= ownership_threshold;
    if !ownership_passed {
        all_passed = false;
    }

    checks.push(GateCheckResult {
        criterion: "ownership_coverage_pct".to_string(),
        passed: ownership_passed,
        actual_value: serde_json::json!(ownership_pct),
        threshold_value: serde_json::json!(ownership_threshold),
        detail: format!(
            "Ownership coverage {:.1}% (threshold: {:.1}%): {} of {} entities proved",
            ownership_pct, ownership_threshold, coverage_stats.1, coverage_stats.0
        ),
    });

    // ---------------------------------------------------------------
    // Check 2: All workstream entities have at least one ownership edge
    // ---------------------------------------------------------------
    let entities_without_edges: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM kyc.entity_workstreams ew
        WHERE ew.case_id = $1
          AND NOT EXISTS (
              SELECT 1
              FROM "ob-poc".cbu_ownership_graph og
              JOIN kyc.cases c ON c.case_id = $1
              WHERE og.cbu_id = c.cbu_id
                AND (og.from_entity_id = ew.entity_id OR og.to_entity_id = ew.entity_id)
          )
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    let edges_passed = entities_without_edges.0 == 0;
    if !edges_passed {
        all_passed = false;
    }

    checks.push(GateCheckResult {
        criterion: "all_entities_have_ownership_edge".to_string(),
        passed: edges_passed,
        actual_value: serde_json::json!(entities_without_edges.0),
        threshold_value: serde_json::json!(0),
        detail: format!(
            "{} workstream entities without ownership edges",
            entities_without_edges.0
        ),
    });

    // ---------------------------------------------------------------
    // Check 3: Minimum sources consulted (optional)
    // ---------------------------------------------------------------
    if let Some(min_sources) = thresholds
        .get("minimum_sources_consulted")
        .and_then(|v| v.as_i64())
    {
        // Count distinct determination runs as proxy for sources consulted
        let source_count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(DISTINCT run_id)
            FROM kyc.ubo_determination_runs
            WHERE case_id = $1
            "#,
        )
        .bind(case_id)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let sources_passed = source_count.0 >= min_sources;
        if !sources_passed {
            all_passed = false;
        }

        checks.push(GateCheckResult {
            criterion: "minimum_sources_consulted".to_string(),
            passed: sources_passed,
            actual_value: serde_json::json!(source_count.0),
            threshold_value: serde_json::json!(min_sources),
            detail: format!(
                "{} sources consulted (minimum: {})",
                source_count.0, min_sources
            ),
        });
    }

    Ok((all_passed, checks))
}

/// Evaluate the EVIDENCE_COMPLETE gate.
///
/// Checks:
/// 1. ownership_coverage_pct >= threshold (default 95%)
/// 2. identity_docs_verified_pct >= threshold (default 100%)
/// 3. screening_cleared_pct >= threshold (default 100%)
/// 4. No outstanding outreach plan items (outreach_plan_items_max)
#[cfg(feature = "database")]
async fn evaluate_evidence_complete(
    pool: &PgPool,
    case_id: Uuid,
    thresholds: &serde_json::Value,
) -> Result<(bool, Vec<GateCheckResult>)> {
    let mut checks: Vec<GateCheckResult> = Vec::new();
    let mut all_passed = true;

    // ---------------------------------------------------------------
    // Check 1: Ownership coverage
    // ---------------------------------------------------------------
    let ownership_threshold = thresholds
        .get("ownership_coverage_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(95.0);

    let ownership_stats: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE ownership_proved = TRUE) AS proved
        FROM kyc.entity_workstreams
        WHERE case_id = $1
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0, 0));

    let ownership_pct = if ownership_stats.0 > 0 {
        (ownership_stats.1 as f64 / ownership_stats.0 as f64) * 100.0
    } else {
        100.0
    };

    let ownership_passed = ownership_pct >= ownership_threshold;
    if !ownership_passed {
        all_passed = false;
    }

    checks.push(GateCheckResult {
        criterion: "ownership_coverage_pct".to_string(),
        passed: ownership_passed,
        actual_value: serde_json::json!(ownership_pct),
        threshold_value: serde_json::json!(ownership_threshold),
        detail: format!(
            "Ownership coverage {:.1}% (threshold: {:.1}%)",
            ownership_pct, ownership_threshold
        ),
    });

    // ---------------------------------------------------------------
    // Check 2: Identity docs verified percentage
    // ---------------------------------------------------------------
    let identity_threshold = thresholds
        .get("identity_docs_verified_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(100.0);

    let identity_stats: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE identity_verified = TRUE) AS verified
        FROM kyc.entity_workstreams
        WHERE case_id = $1
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0, 0));

    let identity_pct = if identity_stats.0 > 0 {
        (identity_stats.1 as f64 / identity_stats.0 as f64) * 100.0
    } else {
        100.0
    };

    let identity_passed = identity_pct >= identity_threshold;
    if !identity_passed {
        all_passed = false;
    }

    checks.push(GateCheckResult {
        criterion: "identity_docs_verified_pct".to_string(),
        passed: identity_passed,
        actual_value: serde_json::json!(identity_pct),
        threshold_value: serde_json::json!(identity_threshold),
        detail: format!(
            "Identity verification {:.1}% (threshold: {:.1}%)",
            identity_pct, identity_threshold
        ),
    });

    // ---------------------------------------------------------------
    // Check 3: Screening cleared percentage
    // ---------------------------------------------------------------
    let screening_threshold = thresholds
        .get("screening_cleared_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(100.0);

    let screening_stats: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE screening_cleared = TRUE) AS cleared
        FROM kyc.entity_workstreams
        WHERE case_id = $1
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0, 0));

    let screening_pct = if screening_stats.0 > 0 {
        (screening_stats.1 as f64 / screening_stats.0 as f64) * 100.0
    } else {
        100.0
    };

    let screening_passed = screening_pct >= screening_threshold;
    if !screening_passed {
        all_passed = false;
    }

    checks.push(GateCheckResult {
        criterion: "screening_cleared_pct".to_string(),
        passed: screening_passed,
        actual_value: serde_json::json!(screening_pct),
        threshold_value: serde_json::json!(screening_threshold),
        detail: format!(
            "Screening cleared {:.1}% (threshold: {:.1}%)",
            screening_pct, screening_threshold
        ),
    });

    // ---------------------------------------------------------------
    // Check 4: Outstanding outreach plan items
    // ---------------------------------------------------------------
    let max_outstanding = thresholds
        .get("outreach_plan_items_max")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let outstanding_items: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM kyc.outreach_items oi
        JOIN kyc.outreach_plans op ON op.plan_id = oi.plan_id
        WHERE op.case_id = $1
          AND oi.status = 'PENDING'
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    let outreach_passed = outstanding_items.0 <= max_outstanding;
    if !outreach_passed {
        all_passed = false;
    }

    checks.push(GateCheckResult {
        criterion: "outreach_plan_items_max".to_string(),
        passed: outreach_passed,
        actual_value: serde_json::json!(outstanding_items.0),
        threshold_value: serde_json::json!(max_outstanding),
        detail: format!(
            "{} outstanding outreach items (max: {})",
            outstanding_items.0, max_outstanding
        ),
    });

    Ok((all_passed, checks))
}

/// Evaluate the REVIEW_COMPLETE gate.
///
/// Checks:
/// 1. All workstreams closed (status IN CLOSED, COMPLETED)
/// 2. All UBOs approved (if threshold set)
/// 3. No open discrepancies (if threshold set)
#[cfg(feature = "database")]
async fn evaluate_review_complete(
    pool: &PgPool,
    case_id: Uuid,
    thresholds: &serde_json::Value,
) -> Result<(bool, Vec<GateCheckResult>)> {
    let mut checks: Vec<GateCheckResult> = Vec::new();
    let mut all_passed = true;

    // ---------------------------------------------------------------
    // Check 1: All workstreams closed
    // ---------------------------------------------------------------
    let check_workstreams = thresholds
        .get("all_workstreams_closed")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if check_workstreams {
        let open_workstreams: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM kyc.entity_workstreams
            WHERE case_id = $1
              AND status NOT IN ('CLOSED', 'COMPLETED')
            "#,
        )
        .bind(case_id)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let ws_passed = open_workstreams.0 == 0;
        if !ws_passed {
            all_passed = false;
        }

        checks.push(GateCheckResult {
            criterion: "all_workstreams_closed".to_string(),
            passed: ws_passed,
            actual_value: serde_json::json!(open_workstreams.0),
            threshold_value: serde_json::json!(0),
            detail: format!("{} workstreams still open", open_workstreams.0),
        });
    }

    // ---------------------------------------------------------------
    // Check 2: All UBOs approved
    // ---------------------------------------------------------------
    let check_ubos = thresholds
        .get("all_ubos_approved")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if check_ubos {
        // UBO workstreams that are flagged as is_ubo but not yet evidence_complete
        let unapproved_ubos: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM kyc.entity_workstreams
            WHERE case_id = $1
              AND is_ubo = TRUE
              AND (evidence_complete IS NULL OR evidence_complete = FALSE)
            "#,
        )
        .bind(case_id)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let ubos_passed = unapproved_ubos.0 == 0;
        if !ubos_passed {
            all_passed = false;
        }

        checks.push(GateCheckResult {
            criterion: "all_ubos_approved".to_string(),
            passed: ubos_passed,
            actual_value: serde_json::json!(unapproved_ubos.0),
            threshold_value: serde_json::json!(0),
            detail: format!("{} UBOs not yet approved", unapproved_ubos.0),
        });
    }

    // ---------------------------------------------------------------
    // Check 3: No open discrepancies
    // ---------------------------------------------------------------
    let check_discrepancies = thresholds
        .get("no_open_discrepancies")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if check_discrepancies {
        let open_findings: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM kyc.ownership_reconciliation_findings orf
            JOIN kyc.ownership_reconciliation_runs orr ON orr.reconciliation_run_id = orf.reconciliation_run_id
            WHERE orr.case_id = $1
              AND orf.status IN ('OPEN', 'UNDER_REVIEW')
            "#,
        )
        .bind(case_id)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let disc_passed = open_findings.0 == 0;
        if !disc_passed {
            all_passed = false;
        }

        checks.push(GateCheckResult {
            criterion: "no_open_discrepancies".to_string(),
            passed: disc_passed,
            actual_value: serde_json::json!(open_findings.0),
            threshold_value: serde_json::json!(0),
            detail: format!("{} open reconciliation discrepancies", open_findings.0),
        });
    }

    Ok((all_passed, checks))
}

// =============================================================================
// TollgateEvaluateGateOp
// =============================================================================

/// Evaluate a specific named tollgate gate against the current case state.
///
/// Loads gate definition from `ob_ref.tollgate_definitions`, dispatches to
/// gate-specific evaluation logic, and records the result in
/// `kyc.tollgate_evaluations`.
#[register_custom_op]
pub struct TollgateEvaluateGateOp;

#[async_trait]
impl CustomOperation for TollgateEvaluateGateOp {
    fn domain(&self) -> &'static str {
        "tollgate"
    }

    fn verb(&self) -> &'static str {
        "evaluate-gate"
    }

    fn rationale(&self) -> &'static str {
        "Each gate has distinct pass/fail criteria requiring bespoke SQL and cross-table aggregation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let case_id = extract_uuid(verb_call, ctx, "case-id")?;
        let gate_name = extract_string(verb_call, "gate-name")?;

        // Validate gate name
        let valid_gates = ["SKELETON_READY", "EVIDENCE_COMPLETE", "REVIEW_COMPLETE"];
        if !valid_gates.contains(&gate_name.as_str()) {
            return Err(anyhow!(
                "Invalid gate-name '{}'. Valid values: {}",
                gate_name,
                valid_gates.join(", ")
            ));
        }

        // ---------------------------------------------------------------
        // 1. Validate case exists and get status
        // ---------------------------------------------------------------
        let case_info: Option<(Uuid, String)> =
            sqlx::query_as(r#"SELECT case_id, status FROM kyc.cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_optional(pool)
                .await?;

        let (_case_id, case_status) =
            case_info.ok_or_else(|| anyhow!("Case not found: {}", case_id))?;

        // ---------------------------------------------------------------
        // 2. Load gate definition from reference data
        // ---------------------------------------------------------------
        #[derive(sqlx::FromRow)]
        struct GateRow {
            tollgate_id: String,
            display_name: String,
            required_status: Option<String>,
            default_thresholds: serde_json::Value,
        }

        let gate_row: GateRow = sqlx::query_as(
            r#"
            SELECT tollgate_id, display_name, required_status, default_thresholds
            FROM ob_ref.tollgate_definitions
            WHERE tollgate_id = $1
            "#,
        )
        .bind(&gate_name)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow!("Gate definition not found: {}", gate_name))?;

        let gate_def = GateDefinition {
            tollgate_id: gate_row.tollgate_id,
            display_name: gate_row.display_name,
            required_status: gate_row.required_status,
            default_thresholds: gate_row.default_thresholds,
        };

        // ---------------------------------------------------------------
        // 3. Check case status prerequisite (if defined)
        // ---------------------------------------------------------------
        if let Some(ref required_status) = gate_def.required_status {
            if case_status != *required_status {
                let detail = serde_json::json!({
                    "gate_name": gate_def.tollgate_id,
                    "display_name": gate_def.display_name,
                    "passed": false,
                    "reason": format!(
                        "Case status '{}' does not match required status '{}'",
                        case_status, required_status
                    ),
                    "checks": []
                });

                // Record the failed evaluation
                let eval_row: (Uuid, String) = sqlx::query_as(
                    r#"
                    INSERT INTO kyc.tollgate_evaluations (
                        case_id, tollgate_id, passed, evaluation_detail, config_version
                    )
                    VALUES ($1, $2, FALSE, $3, 'v1')
                    RETURNING evaluation_id, evaluated_at::text
                    "#,
                )
                .bind(case_id)
                .bind(&gate_def.tollgate_id)
                .bind(&detail)
                .fetch_one(pool)
                .await?;

                let result = TollgateEvaluationResult {
                    evaluation_id: eval_row.0,
                    case_id,
                    gate_name: gate_def.tollgate_id,
                    passed: false,
                    evaluation_detail: detail,
                    evaluated_at: eval_row.1,
                };

                if let Some(binding) = verb_call.binding.as_deref() {
                    ctx.bind(binding, eval_row.0);
                }

                return Ok(ExecutionResult::Record(serde_json::to_value(result)?));
            }
        }

        // ---------------------------------------------------------------
        // 4. Dispatch to gate-specific evaluation
        // ---------------------------------------------------------------
        let (passed, checks) = match gate_name.as_str() {
            "SKELETON_READY" => {
                evaluate_skeleton_ready(pool, case_id, &gate_def.default_thresholds).await?
            }
            "EVIDENCE_COMPLETE" => {
                evaluate_evidence_complete(pool, case_id, &gate_def.default_thresholds).await?
            }
            "REVIEW_COMPLETE" => {
                evaluate_review_complete(pool, case_id, &gate_def.default_thresholds).await?
            }
            _ => {
                return Err(anyhow!("No evaluation logic for gate: {}", gate_name));
            }
        };

        // ---------------------------------------------------------------
        // 5. Build evaluation detail
        // ---------------------------------------------------------------
        let gaps: Vec<serde_json::Value> = checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| {
                serde_json::json!({
                    "criterion": c.criterion,
                    "actual": c.actual_value,
                    "threshold": c.threshold_value,
                    "detail": c.detail
                })
            })
            .collect();

        let evaluation_detail = serde_json::json!({
            "gate_name": gate_def.tollgate_id,
            "display_name": gate_def.display_name,
            "passed": passed,
            "checks": checks,
            "thresholds_used": gate_def.default_thresholds,
            "case_status": case_status
        });

        let gaps_json = if gaps.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::json!(gaps)
        };

        // ---------------------------------------------------------------
        // 6. Record evaluation
        // ---------------------------------------------------------------
        let eval_row: (Uuid, String) = sqlx::query_as(
            r#"
            INSERT INTO kyc.tollgate_evaluations (
                case_id, tollgate_id, passed, evaluation_detail, gaps, config_version
            )
            VALUES ($1, $2, $3, $4, $5, 'v1')
            RETURNING evaluation_id, evaluated_at::text
            "#,
        )
        .bind(case_id)
        .bind(&gate_def.tollgate_id)
        .bind(passed)
        .bind(&evaluation_detail)
        .bind(&gaps_json)
        .fetch_one(pool)
        .await?;

        // ---------------------------------------------------------------
        // 7. Build result
        // ---------------------------------------------------------------
        let result = TollgateEvaluationResult {
            evaluation_id: eval_row.0,
            case_id,
            gate_name: gate_def.tollgate_id,
            passed,
            evaluation_detail,
            evaluated_at: eval_row.1,
        };

        if let Some(binding) = verb_call.binding.as_deref() {
            ctx.bind(binding, eval_row.0);
        }

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_metadata() {
        let op = TollgateEvaluateGateOp;
        assert_eq!(op.domain(), "tollgate");
        assert_eq!(op.verb(), "evaluate-gate");
    }

    #[test]
    fn test_valid_gate_names() {
        let valid = ["SKELETON_READY", "EVIDENCE_COMPLETE", "REVIEW_COMPLETE"];
        assert_eq!(valid.len(), 3);
        assert!(valid.contains(&"SKELETON_READY"));
        assert!(valid.contains(&"EVIDENCE_COMPLETE"));
        assert!(valid.contains(&"REVIEW_COMPLETE"));
        assert!(!valid.contains(&"INVALID_GATE"));
    }

    #[test]
    fn test_result_serialization() {
        let result = TollgateEvaluationResult {
            evaluation_id: Uuid::nil(),
            case_id: Uuid::nil(),
            gate_name: "SKELETON_READY".to_string(),
            passed: true,
            evaluation_detail: serde_json::json!({
                "gate_name": "SKELETON_READY",
                "passed": true,
                "checks": []
            }),
            evaluated_at: "2026-02-12T00:00:00Z".to_string(),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["gate_name"], "SKELETON_READY");
        assert_eq!(json["passed"], true);
    }

    #[test]
    fn test_gate_check_result_serialization() {
        let check = GateCheckResult {
            criterion: "ownership_coverage_pct".to_string(),
            passed: false,
            actual_value: serde_json::json!(65.0),
            threshold_value: serde_json::json!(70.0),
            detail: "Ownership coverage 65.0% (threshold: 70.0%)".to_string(),
        };

        let json = serde_json::to_value(&check).unwrap();
        assert_eq!(json["criterion"], "ownership_coverage_pct");
        assert_eq!(json["passed"], false);
    }
}
