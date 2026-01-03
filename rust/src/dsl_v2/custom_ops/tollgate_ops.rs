//! Tollgate Decision Engine Operations
//!
//! Plugin handlers for KYC workflow gate evaluation.
//! Computes verification metrics and compares against configurable thresholds.
//!
//! ## Rationale
//! Tollgate operations require custom code because:
//! - Metric computation requires aggregation across multiple tables
//! - Threshold evaluation involves complex comparison logic
//! - Override recording requires audit trail management

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::get_required_uuid;
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

/// Evaluate tollgate for a KYC case
pub struct TollgateEvaluateOp;

#[async_trait]
impl CustomOperation for TollgateEvaluateOp {
    fn domain(&self) -> &'static str {
        "tollgate"
    }

    fn verb(&self) -> &'static str {
        "evaluate"
    }

    fn rationale(&self) -> &'static str {
        "Tollgate evaluation requires computing metrics across tables and comparing against thresholds"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let case_id = get_required_uuid(verb_call, "case-id")?;
        let evaluation_type = verb_call
            .get_arg("evaluation-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("evaluation-type is required"))?;
        let evaluated_by = verb_call
            .get_arg("evaluated-by")
            .and_then(|a| a.value.as_string());

        // Get case info
        let case_info: Option<(Uuid,)> =
            sqlx::query_as(r#"SELECT cbu_id FROM kyc.cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_optional(pool)
                .await?;

        let (cbu_id,) = case_info.ok_or_else(|| anyhow!("Case not found"))?;

        // Compute metrics
        let metrics = compute_metrics(pool, cbu_id, case_id).await?;

        // Get thresholds for this evaluation type
        // Uses kyc.tollgate_thresholds with comparison-based evaluation
        #[derive(sqlx::FromRow)]
        struct ThresholdRow {
            threshold_name: String,
            metric_type: String,
            comparison: String,
            threshold_value: Option<rust_decimal::Decimal>,
            is_blocking: bool,
            weight: Option<rust_decimal::Decimal>,
        }

        let thresholds: Vec<ThresholdRow> = sqlx::query_as(
            r#"
            SELECT threshold_name, metric_type, comparison, threshold_value, is_blocking, weight
            FROM kyc.tollgate_thresholds
            WHERE $1 = ANY(applies_to_case_types) OR applies_to_case_types IS NULL
            ORDER BY is_blocking DESC, threshold_name
            "#,
        )
        .bind(evaluation_type)
        .fetch_all(pool)
        .await?;

        // Evaluate against thresholds using comparison operators
        let mut blocking_failures: Vec<serde_json::Value> = Vec::new();
        let mut warnings: Vec<serde_json::Value> = Vec::new();
        let mut score = rust_decimal::Decimal::from(100);

        for threshold in &thresholds {
            let metric_value = match threshold.metric_type.as_str() {
                "OWNERSHIP_VERIFIED_PCT" => metrics.ownership_verified_pct,
                "CONTROL_VERIFIED_PCT" => metrics.control_verified_pct,
                "UBO_COVERAGE_PCT" => metrics.ubo_coverage_pct,
                "DOC_COMPLETENESS_PCT" => metrics.doc_completeness_pct,
                "SCREENING_CLEAR_PCT" => metrics.screening_clear_pct,
                "RED_FLAG_COUNT" => Some(rust_decimal::Decimal::from(metrics.red_flag_count)),
                "ALLEGATION_UNRESOLVED_COUNT" => Some(rust_decimal::Decimal::from(
                    metrics.allegation_unresolved_count,
                )),
                "DAYS_SINCE_REFRESH" => metrics
                    .days_since_last_refresh
                    .map(rust_decimal::Decimal::from),
                _ => None,
            };

            if let (Some(value), Some(threshold_val)) = (metric_value, threshold.threshold_value) {
                // Evaluate using comparison operator
                let passed = match threshold.comparison.as_str() {
                    "GT" => value > threshold_val,
                    "GTE" => value >= threshold_val,
                    "LT" => value < threshold_val,
                    "LTE" => value <= threshold_val,
                    "EQ" => value == threshold_val,
                    "NEQ" => value != threshold_val,
                    _ => true, // Unknown comparison, assume pass
                };

                if !passed {
                    let failure = json!({
                        "threshold": threshold.threshold_name,
                        "metric_type": threshold.metric_type,
                        "actual_value": value.to_string(),
                        "comparison": threshold.comparison,
                        "threshold_value": threshold_val.to_string(),
                        "is_blocking": threshold.is_blocking
                    });

                    let weight = threshold
                        .weight
                        .unwrap_or(rust_decimal::Decimal::from(1))
                        .to_string()
                        .parse::<f64>()
                        .unwrap_or(1.0);

                    if threshold.is_blocking {
                        blocking_failures.push(failure);
                        score -= rust_decimal::Decimal::from((20.0 * weight) as i64);
                    } else {
                        warnings.push(failure);
                        score -= rust_decimal::Decimal::from((5.0 * weight) as i64);
                    }
                }
            }
        }

        // Clamp score
        if score < rust_decimal::Decimal::ZERO {
            score = rust_decimal::Decimal::ZERO;
        }

        let overall_result = if !blocking_failures.is_empty() {
            "FAIL"
        } else if !warnings.is_empty() {
            "PASS_WITH_WARNINGS"
        } else {
            "PASS"
        };

        // Build threshold_results for JSONB column
        let threshold_results: serde_json::Value = thresholds
            .iter()
            .map(|t| {
                (
                    t.threshold_name.clone(),
                    json!({
                        "metric_type": t.metric_type,
                        "comparison": t.comparison,
                        "threshold_value": t.threshold_value.map(|v| v.to_string()),
                        "is_blocking": t.is_blocking
                    }),
                )
            })
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        // Convert blocking_failures and warnings to TEXT[] for database
        let blocking_failure_texts: Vec<String> =
            blocking_failures.iter().map(|f| f.to_string()).collect();
        let warning_texts: Vec<String> = warnings.iter().map(|w| w.to_string()).collect();

        // Record evaluation - schema requires cbu_id
        let evaluation_id: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO kyc.tollgate_evaluations (
                case_id, cbu_id, evaluation_type, overall_result, score,
                metrics, threshold_results, blocking_failures, warnings, evaluated_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
        )
        .bind(case_id)
        .bind(cbu_id)
        .bind(evaluation_type)
        .bind(overall_result)
        .bind(score)
        .bind(json!(&metrics))
        .bind(threshold_results)
        .bind(&blocking_failure_texts)
        .bind(&warning_texts)
        .bind(evaluated_by)
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Record(json!({
            "evaluation_id": evaluation_id.0,
            "case_id": case_id,
            "evaluation_type": evaluation_type,
            "overall_result": overall_result,
            "score": score.to_string(),
            "metrics": metrics,
            "blocking_failures": blocking_failures,
            "warnings": warnings
        })))
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

/// Get current metrics for a CBU
pub struct TollgateGetMetricsOp;

#[async_trait]
impl CustomOperation for TollgateGetMetricsOp {
    fn domain(&self) -> &'static str {
        "tollgate"
    }

    fn verb(&self) -> &'static str {
        "get-metrics"
    }

    fn rationale(&self) -> &'static str {
        "Metrics computation requires aggregation across ownership, control, documents, and screenings"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id")?;

        // Find active case for this CBU
        let case_id: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT case_id FROM kyc.cases
            WHERE cbu_id = $1 AND status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN')
            ORDER BY opened_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await?;

        let case_id = case_id.map(|c| c.0).unwrap_or(Uuid::nil());

        let metrics = compute_metrics(pool, cbu_id, case_id).await?;

        Ok(ExecutionResult::Record(serde_json::to_value(&metrics)?))
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

/// Record management override of tollgate failure
pub struct TollgateOverrideOp;

#[async_trait]
impl CustomOperation for TollgateOverrideOp {
    fn domain(&self) -> &'static str {
        "tollgate"
    }

    fn verb(&self) -> &'static str {
        "override"
    }

    fn rationale(&self) -> &'static str {
        "Override recording requires linking to evaluation and maintaining audit trail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let evaluation_id = get_required_uuid(verb_call, "evaluation-id")?;
        let override_reason = verb_call
            .get_arg("override-reason")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("override-reason is required"))?;
        let approved_by = verb_call
            .get_arg("approved-by")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("approved-by is required"))?;
        let approval_authority = verb_call
            .get_arg("approval-authority")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("approval-authority is required (ANALYST, SENIOR_ANALYST, TEAM_LEAD, COMPLIANCE_OFFICER, SENIOR_COMPLIANCE, MLRO, EXECUTIVE, BOARD)"))?;
        let conditions = verb_call
            .get_arg("conditions")
            .and_then(|a| a.value.as_string());
        let expiry_date = verb_call
            .get_arg("expiry-date")
            .and_then(|a| a.value.as_string());

        // Verify evaluation exists and is a failure
        let eval: Option<(String,)> =
            sqlx::query_as(r#"SELECT overall_result FROM kyc.tollgate_evaluations WHERE id = $1"#)
                .bind(evaluation_id)
                .fetch_optional(pool)
                .await?;

        let (result,) = eval.ok_or_else(|| anyhow!("Evaluation not found"))?;

        if result == "PASS" {
            return Err(anyhow!("Cannot override a passing evaluation"));
        }

        // Record override - schema requires approval_authority
        let override_id: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO kyc.tollgate_overrides (
                evaluation_id, override_reason, approved_by, approval_authority, conditions, expiry_date
            )
            VALUES ($1, $2, $3, $4, $5, $6::date)
            RETURNING id
            "#,
        )
        .bind(evaluation_id)
        .bind(override_reason)
        .bind(approved_by)
        .bind(approval_authority)
        .bind(conditions)
        .bind(expiry_date)
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Uuid(override_id.0))
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

/// Get decision readiness summary
pub struct TollgateDecisionReadinessOp;

#[async_trait]
impl CustomOperation for TollgateDecisionReadinessOp {
    fn domain(&self) -> &'static str {
        "tollgate"
    }

    fn verb(&self) -> &'static str {
        "get-decision-readiness"
    }

    fn rationale(&self) -> &'static str {
        "Decision readiness requires evaluating all tollgates and providing actionable summary"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let case_id = get_required_uuid(verb_call, "case-id")?;

        // Get case info
        let case_info: Option<(Uuid, String)> =
            sqlx::query_as(r#"SELECT cbu_id, status FROM kyc.cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_optional(pool)
                .await?;

        let (cbu_id, case_status) = case_info.ok_or_else(|| anyhow!("Case not found"))?;

        // Get latest evaluations for each type
        let evaluations: Vec<(String, String, rust_decimal::Decimal)> = sqlx::query_as(
            r#"
            SELECT DISTINCT ON (evaluation_type)
                evaluation_type, overall_result, score
            FROM kyc.tollgate_evaluations
            WHERE case_id = $1
            ORDER BY evaluation_type, evaluated_at DESC
            "#,
        )
        .bind(case_id)
        .fetch_all(pool)
        .await?;

        let mut blocking_issues: Vec<serde_json::Value> = Vec::new();
        let mut completion_summary: std::collections::HashMap<String, serde_json::Value> =
            std::collections::HashMap::new();

        for (eval_type, result, score) in &evaluations {
            completion_summary.insert(
                eval_type.clone(),
                json!({
                    "result": result,
                    "score": score.to_string()
                }),
            );

            if result == "FAIL" {
                blocking_issues.push(json!({
                    "evaluation_type": eval_type,
                    "issue": format!("{} failed with score {}", eval_type, score)
                }));
            }
        }

        // Check for missing mandatory evaluations
        let required_evaluations = [
            "DISCOVERY_COMPLETE",
            "EVIDENCE_COMPLETE",
            "VERIFICATION_COMPLETE",
        ];
        for req in required_evaluations {
            if !completion_summary.contains_key(req) {
                blocking_issues.push(json!({
                    "evaluation_type": req,
                    "issue": format!("{} evaluation not yet performed", req)
                }));
            }
        }

        // Get metrics for recommendations
        let metrics = compute_metrics(pool, cbu_id, case_id).await?;

        let mut recommended_actions: Vec<String> = Vec::new();

        if metrics.ownership_verified_pct.unwrap_or_default() < rust_decimal::Decimal::from(100) {
            recommended_actions.push("Complete ownership verification".to_string());
        }
        if metrics.doc_completeness_pct.unwrap_or_default() < rust_decimal::Decimal::from(100) {
            recommended_actions.push("Collect outstanding documents".to_string());
        }
        if metrics.red_flag_count > 0 {
            recommended_actions.push(format!(
                "Resolve {} outstanding red flags",
                metrics.red_flag_count
            ));
        }
        if metrics.screening_clear_pct.unwrap_or_default() < rust_decimal::Decimal::from(100) {
            recommended_actions.push("Review pending screening results".to_string());
        }

        let is_decision_ready = blocking_issues.is_empty() && case_status == "REVIEW";

        Ok(ExecutionResult::Record(json!({
            "case_id": case_id,
            "case_status": case_status,
            "is_decision_ready": is_decision_ready,
            "blocking_issues": blocking_issues,
            "completion_summary": completion_summary,
            "recommended_actions": recommended_actions
        })))
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

// Helper struct for metrics
#[derive(Debug, serde::Serialize)]
struct TollgateMetrics {
    ownership_verified_pct: Option<rust_decimal::Decimal>,
    control_verified_pct: Option<rust_decimal::Decimal>,
    ubo_coverage_pct: Option<rust_decimal::Decimal>,
    doc_completeness_pct: Option<rust_decimal::Decimal>,
    screening_clear_pct: Option<rust_decimal::Decimal>,
    red_flag_count: i64,
    allegation_unresolved_count: i64,
    days_since_last_refresh: Option<i64>,
}

#[cfg(feature = "database")]
async fn compute_metrics(pool: &PgPool, cbu_id: Uuid, case_id: Uuid) -> Result<TollgateMetrics> {
    // Ownership verified percentage
    let ownership_stats: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total,
            COUNT(*) FILTER (WHERE status = 'proven') as verified
        FROM "ob-poc".cbu_relationship_verification
        WHERE cbu_id = $1
        "#,
    )
    .bind(cbu_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0, 0));

    let ownership_verified_pct = if ownership_stats.0 > 0 {
        Some(
            rust_decimal::Decimal::from(ownership_stats.1 * 100)
                / rust_decimal::Decimal::from(ownership_stats.0),
        )
    } else {
        None
    };

    // Control verified - same as ownership for now
    let control_verified_pct = ownership_verified_pct;

    // UBO coverage - percentage of ownership covered by identified UBOs
    let ubo_stats: (Option<rust_decimal::Decimal>,) = sqlx::query_as(
        r#"
        SELECT COALESCE(SUM(ownership_percentage), 0)
        FROM "ob-poc".ubo_registry
        WHERE cbu_id = $1 AND workflow_status = 'VERIFIED'
        "#,
    )
    .bind(cbu_id)
    .fetch_one(pool)
    .await
    .unwrap_or((None,));

    let ubo_coverage_pct = ubo_stats.0;

    // Document completeness
    let doc_stats: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total,
            COUNT(*) FILTER (WHERE status IN ('VERIFIED', 'WAIVED')) as complete
        FROM kyc.doc_requests dr
        JOIN kyc.entity_workstreams ew ON dr.workstream_id = ew.workstream_id
        WHERE ew.case_id = $1
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0, 0));

    let doc_completeness_pct = if doc_stats.0 > 0 {
        Some(
            rust_decimal::Decimal::from(doc_stats.1 * 100)
                / rust_decimal::Decimal::from(doc_stats.0),
        )
    } else {
        Some(rust_decimal::Decimal::from(100)) // No docs required = 100%
    };

    // Screening clear percentage
    let screening_stats: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total,
            COUNT(*) FILTER (WHERE status IN ('CLEAR', 'HIT_DISMISSED')) as clear
        FROM kyc.screenings s
        JOIN kyc.entity_workstreams ew ON s.workstream_id = ew.workstream_id
        WHERE ew.case_id = $1
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0, 0));

    let screening_clear_pct = if screening_stats.0 > 0 {
        Some(
            rust_decimal::Decimal::from(screening_stats.1 * 100)
                / rust_decimal::Decimal::from(screening_stats.0),
        )
    } else {
        None
    };

    // Red flag count
    let red_flag_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM kyc.red_flags
        WHERE case_id = $1 AND status IN ('OPEN', 'UNDER_REVIEW', 'BLOCKING')
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    // Unresolved allegations
    let allegation_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM "ob-poc".client_allegations
        WHERE cbu_id = $1 AND verification_status = 'PENDING'
        "#,
    )
    .bind(cbu_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    // Days since last refresh (based on case last activity)
    let last_activity: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT EXTRACT(DAY FROM (now() - last_activity_at))::bigint
        FROM kyc.cases
        WHERE case_id = $1
        "#,
    )
    .bind(case_id)
    .fetch_optional(pool)
    .await?;

    Ok(TollgateMetrics {
        ownership_verified_pct,
        control_verified_pct,
        ubo_coverage_pct,
        doc_completeness_pct,
        screening_clear_pct,
        red_flag_count: red_flag_count.0,
        allegation_unresolved_count: allegation_count.0,
        days_since_last_refresh: last_activity.map(|l| l.0),
    })
}
