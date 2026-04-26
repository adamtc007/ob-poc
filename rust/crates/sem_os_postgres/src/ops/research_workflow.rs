//! Research workflow verbs (4 plugin verbs) — YAML-first
//! re-implementation of `research.workflow.*` from
//! `rust/config/verbs/research-workflow.yaml`.
//!
//! Ops:
//! - `research.workflow.confirm-decision` — update decision with
//!   user's selection, record verified_by
//! - `research.workflow.reject-decision` — mark decision REJECTED with
//!   reason appended to selection_reasoning
//! - `research.workflow.audit-trail` — multi-table aggregation across
//!   decisions / actions / corrections / anomalies / import_runs with
//!   per-section include flags
//! - `research.workflow.supersession-trail` — full supersession chain
//!   for a case (import runs, corrections, stale UBO determination
//!   runs, reset outreach plans, stale tollgate evaluations)

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_string, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ---------------------------------------------------------------------------
// Public result types (consumed by serde in the op bodies only)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupersessionTrailResult {
    pub case_id: Uuid,
    pub import_runs: Vec<ImportRunEntry>,
    pub corrections: Vec<CorrectionEntry>,
    pub stale_determination_runs: Vec<StaleDeterminationEntry>,
    pub reset_outreach_plans: Vec<ResetOutreachEntry>,
    pub stale_tollgate_evaluations: Vec<StaleTollgateEntry>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleDeterminationEntry {
    pub run_id: Uuid,
    pub subject_entity_id: Uuid,
    pub as_of: String,
    pub candidates_found: i32,
    pub coverage_snapshot_present: bool,
    pub computed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetOutreachEntry {
    pub plan_id: Uuid,
    pub status: String,
    pub total_items: i32,
    pub determination_run_id: Option<Uuid>,
    pub generated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleTollgateEntry {
    pub evaluation_id: Uuid,
    pub tollgate_id: String,
    pub passed: bool,
    pub stale_reason: Option<String>,
    pub evaluated_at: Option<String>,
}

// ---------------------------------------------------------------------------
// confirm-decision
// ---------------------------------------------------------------------------

pub struct ConfirmDecision;

#[async_trait]
impl SemOsVerbOp for ConfirmDecision {
    fn fqn(&self) -> &str {
        "research.workflow.confirm-decision"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let decision_id = json_extract_uuid(args, ctx, "decision-id")?;
        let selected_key = json_extract_string(args, "selected-key")?;
        let selected_key_type = json_extract_string(args, "selected-key-type")?;
        let verified_by: Option<Uuid> = ctx.principal.actor_id.parse().ok();

        let (decision_id_out, target_entity_id, source_provider): (Uuid, Uuid, String) =
            sqlx::query_as(
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
            .fetch_one(scope.executor())
            .await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "decision_id": decision_id_out,
            "target_entity_id": target_entity_id,
            "source_provider": source_provider,
            "selected_key": selected_key,
            "selected_key_type": selected_key_type,
            "status": "confirmed"
        })))
    }
}

// ---------------------------------------------------------------------------
// reject-decision
// ---------------------------------------------------------------------------

pub struct RejectDecision;

#[async_trait]
impl SemOsVerbOp for RejectDecision {
    fn fqn(&self) -> &str {
        "research.workflow.reject-decision"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let decision_id = json_extract_uuid(args, ctx, "decision-id")?;
        let rejection_reason = json_extract_string(args, "rejection-reason")?;
        let verified_by: Option<Uuid> = ctx.principal.actor_id.parse().ok();

        let (decision_id_out, target_entity_id): (Uuid, Uuid) = sqlx::query_as(
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
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "decision_id": decision_id_out,
            "target_entity_id": target_entity_id,
            "status": "rejected",
            "reason": rejection_reason
        })))
    }
}

// ---------------------------------------------------------------------------
// audit-trail
// ---------------------------------------------------------------------------

pub struct AuditTrail;

#[async_trait]
impl SemOsVerbOp for AuditTrail {
    fn fqn(&self) -> &str {
        "research.workflow.audit-trail"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let include_decisions = json_extract_bool_opt(args, "include-decisions").unwrap_or(true);
        let include_actions = json_extract_bool_opt(args, "include-actions").unwrap_or(true);
        let include_corrections =
            json_extract_bool_opt(args, "include-corrections").unwrap_or(true);
        let include_anomalies = json_extract_bool_opt(args, "include-anomalies").unwrap_or(true);
        let include_import_runs =
            json_extract_bool_opt(args, "include-import-runs").unwrap_or(true);

        let mut result = json!({ "entity_id": entity_id });

        if include_decisions {
            type Row = (
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
            );
            let decisions: Vec<Row> = sqlx::query_as(
                r#"
                SELECT decision_id, search_query, source_provider, candidates_count,
                       selected_key, selected_key_type, selection_confidence,
                       decision_type, auto_selected, created_at
                FROM "ob-poc".research_decisions
                WHERE target_entity_id = $1
                ORDER BY created_at DESC
                "#,
            )
            .bind(entity_id)
            .fetch_all(scope.executor())
            .await?;
            result["decisions"] = json!(decisions
                .iter()
                .map(|d| json!({
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
                }))
                .collect::<Vec<_>>());
        }

        if include_actions {
            type Row = (
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
            );
            let actions: Vec<Row> = sqlx::query_as(
                r#"
                SELECT action_id, decision_id, action_type, source_provider, source_key,
                       verb_domain, verb_name, success,
                       entities_created, entities_updated, relationships_created,
                       error_message, duration_ms, executed_at
                FROM "ob-poc".research_actions
                WHERE target_entity_id = $1
                ORDER BY executed_at DESC
                "#,
            )
            .bind(entity_id)
            .fetch_all(scope.executor())
            .await?;
            result["actions"] = json!(actions.iter().map(|a| json!({
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
            })).collect::<Vec<_>>());
        }

        if include_corrections {
            type Row = (
                Uuid,
                Uuid,
                String,
                Option<String>,
                Option<String>,
                String,
                Option<chrono::DateTime<chrono::Utc>>,
            );
            let corrections: Vec<Row> = sqlx::query_as(
                r#"
                SELECT c.correction_id, c.original_decision_id,
                       c.correction_type, c.wrong_key, c.correct_key,
                       c.correction_reason, c.corrected_at
                FROM "ob-poc".research_corrections c
                JOIN "ob-poc".research_decisions d ON d.decision_id = c.original_decision_id
                WHERE d.target_entity_id = $1
                ORDER BY c.corrected_at DESC
                "#,
            )
            .bind(entity_id)
            .fetch_all(scope.executor())
            .await?;
            result["corrections"] = json!(corrections
                .iter()
                .map(|c| json!({
                    "correction_id": c.0,
                    "original_decision_id": c.1,
                    "correction_type": c.2,
                    "wrong_key": c.3,
                    "correct_key": c.4,
                    "reason": c.5,
                    "corrected_at": c.6
                }))
                .collect::<Vec<_>>());
        }

        if include_anomalies {
            type Row = (
                Uuid,
                Option<Uuid>,
                String,
                String,
                String,
                String,
                Option<String>,
                chrono::DateTime<chrono::Utc>,
            );
            let anomalies: Vec<Row> = sqlx::query_as(
                r#"
                SELECT anomaly_id, action_id, rule_code, severity, description,
                       status, resolution, detected_at
                FROM "ob-poc".research_anomalies
                WHERE entity_id = $1
                ORDER BY detected_at DESC
                "#,
            )
            .bind(entity_id)
            .fetch_all(scope.executor())
            .await?;
            result["anomalies"] = json!(anomalies
                .iter()
                .map(|a| json!({
                    "anomaly_id": a.0,
                    "action_id": a.1,
                    "rule_code": a.2,
                    "severity": a.3,
                    "description": a.4,
                    "status": a.5,
                    "resolution": a.6,
                    "detected_at": a.7
                }))
                .collect::<Vec<_>>());
        }

        if include_import_runs {
            type Row = (
                Uuid,
                String,
                String,
                String,
                Option<i32>,
                Option<Uuid>,
                Option<String>,
                Option<chrono::DateTime<chrono::Utc>>,
            );
            let import_runs: Vec<Row> = sqlx::query_as(
                r#"SELECT run_id, run_kind, source, status, edges_created,
                          superseded_by, superseded_reason, imported_at
                   FROM "ob-poc".graph_import_runs
                   WHERE scope_root_entity_id = $1
                   ORDER BY imported_at DESC"#,
            )
            .bind(entity_id)
            .fetch_all(scope.executor())
            .await?;
            result["import_runs"] = json!(import_runs
                .iter()
                .map(|r| json!({
                    "run_id": r.0,
                    "run_kind": r.1,
                    "source": r.2,
                    "status": r.3,
                    "edges_created": r.4,
                    "superseded_by": r.5,
                    "superseded_reason": r.6,
                    "imported_at": r.7
                }))
                .collect::<Vec<_>>());
        }

        Ok(VerbExecutionOutcome::Record(result))
    }
}

// ---------------------------------------------------------------------------
// supersession-trail
// ---------------------------------------------------------------------------

pub struct SupersessionTrail;

#[async_trait]
impl SemOsVerbOp for SupersessionTrail {
    fn fqn(&self) -> &str {
        "research.workflow.supersession-trail"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let case_id = json_extract_uuid(args, ctx, "case-id")?;

        type ImportRow = (
            Uuid,
            String,
            String,
            String,
            Option<i32>,
            Option<Uuid>,
            Option<String>,
            Option<chrono::DateTime<chrono::Utc>>,
        );
        let import_run_rows: Vec<ImportRow> = sqlx::query_as(
            r#"SELECT g.run_id, g.run_kind, g.source, g.status,
                      g.edges_created, g.superseded_by, g.superseded_reason,
                      g.imported_at
               FROM "ob-poc".graph_import_runs g
               JOIN "ob-poc".case_import_runs cir ON cir.run_id = g.run_id
               WHERE cir.case_id = $1
               ORDER BY g.imported_at ASC"#,
        )
        .bind(case_id)
        .fetch_all(scope.executor())
        .await?;

        let import_runs: Vec<ImportRunEntry> = import_run_rows
            .into_iter()
            .map(|r| ImportRunEntry {
                run_id: r.0,
                run_kind: r.1,
                source: r.2,
                status: r.3,
                edges_created: r.4,
                superseded_by: r.5,
                superseded_reason: r.6,
                imported_at: r.7.map(|t| t.to_rfc3339()),
            })
            .collect();

        type CorrRow = (
            Uuid,
            Uuid,
            String,
            Option<String>,
            Option<String>,
            String,
            Option<chrono::DateTime<chrono::Utc>>,
        );
        let correction_rows: Vec<CorrRow> = sqlx::query_as(
            r#"SELECT c.correction_id, c.original_decision_id,
                      c.correction_type, c.wrong_key, c.correct_key,
                      c.correction_reason, c.corrected_at
               FROM "ob-poc".research_corrections c
               JOIN "ob-poc".case_import_runs cir ON cir.decision_id = c.original_decision_id
               WHERE cir.case_id = $1
               ORDER BY c.corrected_at DESC"#,
        )
        .bind(case_id)
        .fetch_all(scope.executor())
        .await?;

        let corrections: Vec<CorrectionEntry> = correction_rows
            .into_iter()
            .map(|r| CorrectionEntry {
                correction_id: r.0,
                original_decision_id: r.1,
                correction_type: r.2,
                wrong_key: r.3,
                correct_key: r.4,
                correction_reason: r.5,
                corrected_at: r.6.map(|t| t.to_rfc3339()),
            })
            .collect();

        type StaleRow = (
            Uuid,
            Uuid,
            chrono::NaiveDate,
            i32,
            bool,
            Option<chrono::DateTime<chrono::Utc>>,
        );
        let stale_det_rows: Vec<StaleRow> = sqlx::query_as(
            r#"SELECT run_id, subject_entity_id, as_of, candidates_found,
                      (coverage_snapshot IS NOT NULL) AS coverage_present,
                      computed_at
               FROM "ob-poc".ubo_determination_runs
               WHERE case_id = $1 AND coverage_snapshot IS NULL
               ORDER BY computed_at DESC"#,
        )
        .bind(case_id)
        .fetch_all(scope.executor())
        .await?;

        let stale_determination_runs: Vec<StaleDeterminationEntry> = stale_det_rows
            .into_iter()
            .map(|r| StaleDeterminationEntry {
                run_id: r.0,
                subject_entity_id: r.1,
                as_of: r.2.to_string(),
                candidates_found: r.3,
                coverage_snapshot_present: r.4,
                computed_at: r.5.map(|t| t.to_rfc3339()),
            })
            .collect();

        type OutreachRow = (
            Uuid,
            String,
            i32,
            Option<Uuid>,
            Option<chrono::DateTime<chrono::Utc>>,
        );
        let outreach_rows: Vec<OutreachRow> = sqlx::query_as(
            r#"SELECT plan_id, status, total_items,
                      determination_run_id, generated_at
               FROM "ob-poc".outreach_plans
               WHERE case_id = $1 AND status = 'DRAFT'
               ORDER BY generated_at DESC"#,
        )
        .bind(case_id)
        .fetch_all(scope.executor())
        .await?;

        let reset_outreach_plans: Vec<ResetOutreachEntry> = outreach_rows
            .into_iter()
            .map(|r| ResetOutreachEntry {
                plan_id: r.0,
                status: r.1,
                total_items: r.2,
                determination_run_id: r.3,
                generated_at: r.4.map(|t| t.to_rfc3339()),
            })
            .collect();

        type TgRow = (
            Uuid,
            String,
            bool,
            Option<Value>,
            Option<chrono::DateTime<chrono::Utc>>,
        );
        let tollgate_rows: Vec<TgRow> = sqlx::query_as(
            r#"SELECT evaluation_id, tollgate_id, passed,
                      evaluation_detail, evaluated_at
               FROM "ob-poc".tollgate_evaluations
               WHERE case_id = $1
                 AND passed = false
                 AND config_version = 'supersession'
               ORDER BY evaluated_at DESC"#,
        )
        .bind(case_id)
        .fetch_all(scope.executor())
        .await?;

        let stale_tollgate_evaluations: Vec<StaleTollgateEntry> = tollgate_rows
            .into_iter()
            .map(
                |(evaluation_id, tollgate_id, passed, evaluation_detail, evaluated_at)| {
                    let stale_reason = evaluation_detail
                        .as_ref()
                        .and_then(|d| d.get("stale_reason"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    StaleTollgateEntry {
                        evaluation_id,
                        tollgate_id,
                        passed,
                        stale_reason,
                        evaluated_at: evaluated_at.map(|t| t.to_rfc3339()),
                    }
                },
            )
            .collect();

        let result = SupersessionTrailResult {
            case_id,
            import_runs,
            corrections,
            stale_determination_runs,
            reset_outreach_plans,
            stale_tollgate_evaluations,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}
