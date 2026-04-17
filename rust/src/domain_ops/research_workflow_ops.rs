//! Research Workflow Operations
//!
//! Plugin handlers for research workflow management:
//! - Decision confirmation/rejection
//! - Audit trail queries (including import run supersession history)
//! - Supersession trail (full import run chain with downstream impacts)
//!
//! Most operations use CRUD via YAML definitions. These handlers
//! implement complex logic that can't be expressed in CRUD.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::helpers::{
    extract_bool_opt, extract_uuid, json_extract_bool_opt, json_extract_string, json_extract_uuid,
};
use super::{CustomOperation, ExecutionResult};
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::ExecutionContext;

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// Types for supersession-trail
// =============================================================================

/// Full supersession trail for a case, showing the chain of import runs,
/// which runs superseded which, and downstream impacts (corrections,
/// stale determination runs, reset outreach plans, stale tollgates).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupersessionTrailResult {
    pub case_id: Uuid,
    pub import_runs: Vec<ImportRunEntry>,
    pub corrections: Vec<CorrectionEntry>,
    pub stale_determination_runs: Vec<StaleDeterminationEntry>,
    pub reset_outreach_plans: Vec<ResetOutreachEntry>,
    pub stale_tollgate_evaluations: Vec<StaleTollgateEntry>,
}

/// A single import run in the supersession chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRunEntry {
    pub run_id: Uuid,
    pub run_kind: String,
    pub source: String,
    pub status: String,
    pub edges_created: Option<i32>,
    pub superseded_by: Option<Uuid>,
    pub superseded_reason: Option<String>,
    pub imported_at: Option<String>,
}

/// A correction logged as part of the supersession cascade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionEntry {
    pub correction_id: Uuid,
    pub original_decision_id: Uuid,
    pub correction_type: String,
    pub wrong_key: Option<String>,
    pub correct_key: Option<String>,
    pub correction_reason: String,
    pub corrected_at: Option<String>,
}

/// A UBO determination run that was marked stale by supersession.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleDeterminationEntry {
    pub run_id: Uuid,
    pub subject_entity_id: Uuid,
    pub as_of: String,
    pub candidates_found: i32,
    pub coverage_snapshot_present: bool,
    pub computed_at: Option<String>,
}

/// An outreach plan that was reset to DRAFT by supersession.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetOutreachEntry {
    pub plan_id: Uuid,
    pub status: String,
    pub total_items: i32,
    pub determination_run_id: Option<Uuid>,
    pub generated_at: Option<String>,
}

/// A tollgate evaluation inserted as stale after supersession.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleTollgateEntry {
    pub evaluation_id: Uuid,
    pub tollgate_id: String,
    pub passed: bool,
    pub stale_reason: Option<String>,
    pub evaluated_at: Option<String>,
}

// =============================================================================
// CONFIRM DECISION
// =============================================================================

/// Confirm an ambiguous decision with user's selection
///
/// Rationale: Updates decision record and may trigger follow-up import
#[register_custom_op]
pub struct WorkflowConfirmDecisionOp;

#[cfg(feature = "database")]
async fn workflow_confirm_decision_impl(
    decision_id: Uuid,
    selected_key: String,
    selected_key_type: String,
    verified_by: Option<Uuid>,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    let result: (Uuid, Uuid, String) = sqlx::query_as(
        r#"
        UPDATE "ob-poc".research_decisions
        SET
            selected_key = $2,
            selected_key_type = $3,
            decision_type = 'USER_CONFIRMED',
            verified_by = $4,
            verified_at = NOW()
        WHERE decision_id = $1
        RETURNING decision_id, target_entity_id, source_provider
        "#,
    )
    .bind(decision_id)
    .bind(&selected_key)
    .bind(&selected_key_type)
    .bind(verified_by)
    .fetch_one(pool)
    .await?;

    Ok(json!({
        "decision_id": result.0,
        "target_entity_id": result.1,
        "source_provider": result.2,
        "selected_key": selected_key,
        "selected_key_type": selected_key_type,
        "status": "confirmed"
    }))
}

#[async_trait]
impl CustomOperation for WorkflowConfirmDecisionOp {
    fn domain(&self) -> &'static str {
        "research.workflow"
    }

    fn verb(&self) -> &'static str {
        "confirm-decision"
    }

    fn rationale(&self) -> &'static str {
        "Updates decision status, records user selection, and links verification"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let decision_id = extract_uuid(verb_call, ctx, "decision-id")?;
        let selected_key = json_extract_string(
            &super::sem_os_helpers::extract_args_as_json(verb_call, ctx),
            "selected-key",
        )?;
        let selected_key_type = json_extract_string(
            &super::sem_os_helpers::extract_args_as_json(verb_call, ctx),
            "selected-key-type",
        )?;
        let verified_by: Option<Uuid> = ctx.audit_user.as_ref().and_then(|s| s.parse().ok());
        Ok(ExecutionResult::Record(
            workflow_confirm_decision_impl(
                decision_id,
                selected_key,
                selected_key_type,
                verified_by,
                pool,
            )
            .await?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"status": "confirmed"})))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let decision_id = json_extract_uuid(args, ctx, "decision-id")?;
        let selected_key = json_extract_string(args, "selected-key")?;
        let selected_key_type = json_extract_string(args, "selected-key-type")?;
        let verified_by: Option<Uuid> = ctx.principal.actor_id.parse().ok();
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            workflow_confirm_decision_impl(
                decision_id,
                selected_key,
                selected_key_type,
                verified_by,
                pool,
            )
            .await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// REJECT DECISION
// =============================================================================

/// Reject a suggested decision
///
/// Rationale: Marks decision as rejected with reason for audit
#[register_custom_op]
pub struct WorkflowRejectDecisionOp;

#[cfg(feature = "database")]
async fn workflow_reject_decision_impl(
    decision_id: Uuid,
    rejection_reason: String,
    verified_by: Option<Uuid>,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    let result: (Uuid, Uuid) = sqlx::query_as(
        r#"
        UPDATE "ob-poc".research_decisions
        SET
            decision_type = 'REJECTED',
            selection_reasoning = selection_reasoning || ' | REJECTED: ' || $2,
            verified_by = $3,
            verified_at = NOW()
        WHERE decision_id = $1
        RETURNING decision_id, target_entity_id
        "#,
    )
    .bind(decision_id)
    .bind(&rejection_reason)
    .bind(verified_by)
    .fetch_one(pool)
    .await?;

    Ok(json!({
        "decision_id": result.0,
        "target_entity_id": result.1,
        "status": "rejected",
        "reason": rejection_reason
    }))
}

#[async_trait]
impl CustomOperation for WorkflowRejectDecisionOp {
    fn domain(&self) -> &'static str {
        "research.workflow"
    }

    fn verb(&self) -> &'static str {
        "reject-decision"
    }

    fn rationale(&self) -> &'static str {
        "Marks decision as rejected and records rejection reason"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let decision_id = extract_uuid(verb_call, ctx, "decision-id")?;
        let rejection_reason = json_extract_string(
            &super::sem_os_helpers::extract_args_as_json(verb_call, ctx),
            "rejection-reason",
        )?;
        let verified_by: Option<Uuid> = ctx.audit_user.as_ref().and_then(|s| s.parse().ok());
        Ok(ExecutionResult::Record(
            workflow_reject_decision_impl(decision_id, rejection_reason, verified_by, pool).await?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"status": "rejected"})))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let decision_id = json_extract_uuid(args, ctx, "decision-id")?;
        let rejection_reason = json_extract_string(args, "rejection-reason")?;
        let verified_by: Option<Uuid> = ctx.principal.actor_id.parse().ok();
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            workflow_reject_decision_impl(decision_id, rejection_reason, verified_by, pool).await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// AUDIT TRAIL
// =============================================================================

/// Get complete research audit trail for an entity
///
/// Rationale: Aggregates decisions, actions, corrections, anomalies, and
/// import run history across multiple tables for compliance reporting.
#[register_custom_op]
pub struct WorkflowAuditTrailOp;

#[cfg(feature = "database")]
async fn workflow_audit_trail_impl(
    entity_id: Uuid,
    include_decisions: bool,
    include_actions: bool,
    include_corrections: bool,
    include_anomalies: bool,
    include_import_runs: bool,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    let mut result = json!({
        "entity_id": entity_id
    });

    if include_decisions {
        let decisions: Vec<(
            Uuid,
            String,
            String,
            i32,
            Option<String>,
            Option<String>,
            Option<rust_decimal::Decimal>,
            String,
            bool,
            chrono::DateTime<chrono::Utc>,
        )> = sqlx::query_as(
            r#"
            SELECT
                decision_id,
                search_query,
                source_provider,
                candidates_count,
                selected_key,
                selected_key_type,
                selection_confidence,
                decision_type,
                auto_selected,
                created_at
            FROM "ob-poc".research_decisions
            WHERE target_entity_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        result["decisions"] = json!(
            decisions
                .iter()
                .map(|d| {
                    json!({
                        "decision_id": d.0,
                        "search_query": d.1,
                        "source_provider": d.2,
                        "candidates_count": d.3,
                        "selected_key": d.4,
                        "selected_key_type": d.5,
                        "confidence": d.6,
                        "decision_type": d.7,
                        "auto_selected": d.8,
                        "created_at": d.9
                    })
                })
                .collect::<Vec<_>>()
        );
    }

    if include_actions {
        let actions: Vec<(
            Uuid,
            Option<Uuid>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            bool,
            i32,
            i32,
            i32,
            Option<String>,
            Option<i32>,
            chrono::DateTime<chrono::Utc>,
        )> = sqlx::query_as(
            r#"
            SELECT
                action_id,
                decision_id,
                action_type,
                source_provider,
                source_key,
                verb_domain,
                verb_name,
                success,
                entities_created,
                entities_updated,
                relationships_created,
                error_message,
                duration_ms,
                executed_at
            FROM "ob-poc".research_actions
            WHERE target_entity_id = $1
            ORDER BY executed_at DESC
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        result["actions"] = json!(actions
            .iter()
            .map(|a| {
                json!({
                    "action_id": a.0,
                    "decision_id": a.1,
                    "action_type": a.2,
                    "source_provider": a.3,
                    "source_key": a.4,
                    "verb": format!("{}:{}", a.5.clone().unwrap_or_default(), a.6.clone().unwrap_or_default()),
                    "success": a.7,
                    "entities_created": a.8,
                    "entities_updated": a.9,
                    "relationships_created": a.10,
                    "error_message": a.11,
                    "duration_ms": a.12,
                    "executed_at": a.13
                })
            })
            .collect::<Vec<_>>());
    }

    if include_corrections {
        let corrections: Vec<(
            Uuid,
            Uuid,
            String,
            Option<String>,
            Option<String>,
            String,
            Option<chrono::DateTime<chrono::Utc>>,
        )> = sqlx::query_as(
            r#"
            SELECT
                c.correction_id,
                c.original_decision_id,
                c.correction_type,
                c.wrong_key,
                c.correct_key,
                c.correction_reason,
                c.corrected_at
            FROM "ob-poc".research_corrections c
            JOIN "ob-poc".research_decisions d ON d.decision_id = c.original_decision_id
            WHERE d.target_entity_id = $1
            ORDER BY c.corrected_at DESC
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        result["corrections"] = json!(
            corrections
                .iter()
                .map(|c| {
                    json!({
                        "correction_id": c.0,
                        "original_decision_id": c.1,
                        "correction_type": c.2,
                        "wrong_key": c.3,
                        "correct_key": c.4,
                        "reason": c.5,
                        "corrected_at": c.6
                    })
                })
                .collect::<Vec<_>>()
        );
    }

    if include_anomalies {
        let anomalies: Vec<(
            Uuid,
            Option<Uuid>,
            String,
            String,
            String,
            String,
            Option<String>,
            chrono::DateTime<chrono::Utc>,
        )> = sqlx::query_as(
            r#"
            SELECT
                anomaly_id,
                action_id,
                rule_code,
                severity,
                description,
                status,
                resolution,
                detected_at
            FROM "ob-poc".research_anomalies
            WHERE entity_id = $1
            ORDER BY detected_at DESC
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        result["anomalies"] = json!(
            anomalies
                .iter()
                .map(|a| {
                    json!({
                        "anomaly_id": a.0,
                        "action_id": a.1,
                        "rule_code": a.2,
                        "severity": a.3,
                        "description": a.4,
                        "status": a.5,
                        "resolution": a.6,
                        "detected_at": a.7
                    })
                })
                .collect::<Vec<_>>()
        );
    }

    if include_import_runs {
        let import_runs: Vec<(
            Uuid,
            String,
            String,
            String,
            Option<i32>,
            Option<Uuid>,
            Option<String>,
            Option<chrono::DateTime<chrono::Utc>>,
        )> = sqlx::query_as(
            r#"SELECT run_id, run_kind, source, status, edges_created,
                      superseded_by, superseded_reason, imported_at
               FROM "ob-poc".graph_import_runs
               WHERE scope_root_entity_id = $1
               ORDER BY imported_at DESC"#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        result["import_runs"] = json!(
            import_runs
                .iter()
                .map(
                    |(
                        run_id,
                        run_kind,
                        source,
                        status,
                        edges_created,
                        superseded_by,
                        superseded_reason,
                        imported_at,
                    )| {
                        json!({
                            "run_id": run_id,
                            "run_kind": run_kind,
                            "source": source,
                            "status": status,
                            "edges_created": edges_created,
                            "superseded_by": superseded_by,
                            "superseded_reason": superseded_reason,
                            "imported_at": imported_at
                        })
                    }
                )
                .collect::<Vec<_>>()
        );
    }

    Ok(result)
}

#[async_trait]
impl CustomOperation for WorkflowAuditTrailOp {
    fn domain(&self) -> &'static str {
        "research.workflow"
    }

    fn verb(&self) -> &'static str {
        "audit-trail"
    }

    fn rationale(&self) -> &'static str {
        "Aggregates research history across multiple tables for compliance reporting"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;
        let args = super::sem_os_helpers::extract_args_as_json(verb_call, ctx);
        let include_decisions = json_extract_bool_opt(&args, "include-decisions").unwrap_or(true);
        let include_actions = json_extract_bool_opt(&args, "include-actions").unwrap_or(true);
        let include_corrections =
            json_extract_bool_opt(&args, "include-corrections").unwrap_or(true);
        let include_anomalies = json_extract_bool_opt(&args, "include-anomalies").unwrap_or(true);
        let include_import_runs =
            extract_bool_opt(verb_call, "include-import-runs").unwrap_or(true);

        Ok(ExecutionResult::Record(
            workflow_audit_trail_impl(
                entity_id,
                include_decisions,
                include_actions,
                include_corrections,
                include_anomalies,
                include_import_runs,
                pool,
            )
            .await?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({
            "decisions": [],
            "actions": [],
            "corrections": [],
            "anomalies": [],
            "import_runs": []
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let include_decisions = json_extract_bool_opt(args, "include-decisions").unwrap_or(true);
        let include_actions = json_extract_bool_opt(args, "include-actions").unwrap_or(true);
        let include_corrections =
            json_extract_bool_opt(args, "include-corrections").unwrap_or(true);
        let include_anomalies = json_extract_bool_opt(args, "include-anomalies").unwrap_or(true);
        let include_import_runs =
            json_extract_bool_opt(args, "include-import-runs").unwrap_or(true);

        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            workflow_audit_trail_impl(
                entity_id,
                include_decisions,
                include_actions,
                include_corrections,
                include_anomalies,
                include_import_runs,
                pool,
            )
            .await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// SUPERSESSION TRAIL
// =============================================================================

/// Get the full supersession chain for a KYC case, showing import runs,
/// corrections, stale determination runs, reset outreach plans, and
/// stale tollgate evaluations.
///
/// Rationale: Multi-table aggregation across the entire supersession cascade
/// that cannot be expressed as a single CRUD query.
#[register_custom_op]
pub struct WorkflowSupersessionTrailOp;

#[cfg(feature = "database")]
async fn workflow_supersession_trail_impl(
    case_id: Uuid,
    pool: &PgPool,
) -> Result<SupersessionTrailResult> {
    let import_run_rows: Vec<(
        Uuid,
        String,
        String,
        String,
        Option<i32>,
        Option<Uuid>,
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = sqlx::query_as(
        r#"SELECT g.run_id, g.run_kind, g.source, g.status,
                  g.edges_created, g.superseded_by, g.superseded_reason,
                  g.imported_at
           FROM "ob-poc".graph_import_runs g
           JOIN "ob-poc".case_import_runs cir ON cir.run_id = g.run_id
           WHERE cir.case_id = $1
           ORDER BY g.imported_at ASC"#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    let import_runs: Vec<ImportRunEntry> = import_run_rows
        .iter()
        .map(
            |(
                run_id,
                run_kind,
                source,
                status,
                edges_created,
                superseded_by,
                superseded_reason,
                imported_at,
            )| ImportRunEntry {
                run_id: *run_id,
                run_kind: run_kind.clone(),
                source: source.clone(),
                status: status.clone(),
                edges_created: *edges_created,
                superseded_by: *superseded_by,
                superseded_reason: superseded_reason.clone(),
                imported_at: imported_at.map(|t| t.to_rfc3339()),
            },
        )
        .collect();

    let correction_rows: Vec<(
        Uuid,
        Uuid,
        String,
        Option<String>,
        Option<String>,
        String,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = sqlx::query_as(
        r#"SELECT c.correction_id, c.original_decision_id,
                  c.correction_type, c.wrong_key, c.correct_key,
                  c.correction_reason, c.corrected_at
           FROM "ob-poc".research_corrections c
           JOIN "ob-poc".case_import_runs cir ON cir.decision_id = c.original_decision_id
           WHERE cir.case_id = $1
           ORDER BY c.corrected_at DESC"#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    let corrections: Vec<CorrectionEntry> = correction_rows
        .iter()
        .map(
            |(
                correction_id,
                original_decision_id,
                correction_type,
                wrong_key,
                correct_key,
                correction_reason,
                corrected_at,
            )| CorrectionEntry {
                correction_id: *correction_id,
                original_decision_id: *original_decision_id,
                correction_type: correction_type.clone(),
                wrong_key: wrong_key.clone(),
                correct_key: correct_key.clone(),
                correction_reason: correction_reason.clone(),
                corrected_at: corrected_at.map(|t| t.to_rfc3339()),
            },
        )
        .collect();

    let stale_det_rows: Vec<(
        Uuid,
        Uuid,
        chrono::NaiveDate,
        i32,
        bool,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = sqlx::query_as(
        r#"SELECT run_id, subject_entity_id, as_of, candidates_found,
                  (coverage_snapshot IS NOT NULL) AS coverage_present,
                  computed_at
           FROM "ob-poc".ubo_determination_runs
           WHERE case_id = $1 AND coverage_snapshot IS NULL
           ORDER BY computed_at DESC"#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    let stale_determination_runs: Vec<StaleDeterminationEntry> = stale_det_rows
        .iter()
        .map(
            |(
                run_id,
                subject_entity_id,
                as_of,
                candidates_found,
                coverage_present,
                computed_at,
            )| StaleDeterminationEntry {
                run_id: *run_id,
                subject_entity_id: *subject_entity_id,
                as_of: as_of.to_string(),
                candidates_found: *candidates_found,
                coverage_snapshot_present: *coverage_present,
                computed_at: computed_at.map(|t| t.to_rfc3339()),
            },
        )
        .collect();

    let outreach_rows: Vec<(
        Uuid,
        String,
        i32,
        Option<Uuid>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = sqlx::query_as(
        r#"SELECT plan_id, status, total_items,
                  determination_run_id, generated_at
           FROM "ob-poc".outreach_plans
           WHERE case_id = $1 AND status = 'DRAFT'
           ORDER BY generated_at DESC"#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    let reset_outreach_plans: Vec<ResetOutreachEntry> = outreach_rows
        .iter()
        .map(
            |(plan_id, status, total_items, determination_run_id, generated_at)| {
                ResetOutreachEntry {
                    plan_id: *plan_id,
                    status: status.clone(),
                    total_items: *total_items,
                    determination_run_id: *determination_run_id,
                    generated_at: generated_at.map(|t| t.to_rfc3339()),
                }
            },
        )
        .collect();

    let tollgate_rows: Vec<(
        Uuid,
        String,
        bool,
        Option<serde_json::Value>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = sqlx::query_as(
        r#"SELECT evaluation_id, tollgate_id, passed,
                  evaluation_detail, evaluated_at
           FROM "ob-poc".tollgate_evaluations
           WHERE case_id = $1
             AND passed = false
             AND config_version = 'supersession'
           ORDER BY evaluated_at DESC"#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    let stale_tollgate_evaluations: Vec<StaleTollgateEntry> = tollgate_rows
        .iter()
        .map(
            |(evaluation_id, tollgate_id, passed, evaluation_detail, evaluated_at)| {
                let stale_reason = evaluation_detail
                    .as_ref()
                    .and_then(|d| d.get("stale_reason"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                StaleTollgateEntry {
                    evaluation_id: *evaluation_id,
                    tollgate_id: tollgate_id.clone(),
                    passed: *passed,
                    stale_reason,
                    evaluated_at: evaluated_at.map(|t| t.to_rfc3339()),
                }
            },
        )
        .collect();

    Ok(SupersessionTrailResult {
        case_id,
        import_runs,
        corrections,
        stale_determination_runs,
        reset_outreach_plans,
        stale_tollgate_evaluations,
    })
}

#[async_trait]
impl CustomOperation for WorkflowSupersessionTrailOp {
    fn domain(&self) -> &'static str {
        "research.workflow"
    }

    fn verb(&self) -> &'static str {
        "supersession-trail"
    }

    fn rationale(&self) -> &'static str {
        "Aggregates the full supersession chain for a case: import runs, corrections, stale UBO runs, reset outreach, stale tollgates"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let case_id = extract_uuid(verb_call, ctx, "case-id")?;
        Ok(ExecutionResult::Record(serde_json::to_value(
            workflow_supersession_trail_impl(case_id, pool).await?,
        )?))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let case_id = json_extract_uuid(args, ctx, "case-id")?;
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(workflow_supersession_trail_impl(case_id, pool).await?)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_metadata() {
        let confirm = WorkflowConfirmDecisionOp;
        assert_eq!(confirm.domain(), "research.workflow");
        assert_eq!(confirm.verb(), "confirm-decision");

        let reject = WorkflowRejectDecisionOp;
        assert_eq!(reject.domain(), "research.workflow");
        assert_eq!(reject.verb(), "reject-decision");

        let audit = WorkflowAuditTrailOp;
        assert_eq!(audit.domain(), "research.workflow");
        assert_eq!(audit.verb(), "audit-trail");

        let supersession = WorkflowSupersessionTrailOp;
        assert_eq!(supersession.domain(), "research.workflow");
        assert_eq!(supersession.verb(), "supersession-trail");
    }
}
