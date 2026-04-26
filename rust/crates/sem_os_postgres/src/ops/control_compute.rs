//! Control-compute verb (1 plugin verb) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/control.yaml` (verb `compute-controllers`).
//!
//! Loads CONTROL / MANAGEMENT / BOARD_APPOINTMENT / SPECIAL_RIGHTS
//! edges from `entity_relationships` for a case's entities, bridges
//! to `cbu_entity_roles` for officer roles, and returns
//! controllers sorted by priority.
//!
//! Priority ordering:
//! 1. BOARD_APPOINTMENT — highest
//! 2. MANAGEMENT_CONTRACT
//! 3. SPECIAL_RIGHTS (includes generic `CONTROL` edges)
//! 4. OFFICER — bridged from `cbu_entity_roles`

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{json_extract_uuid, json_extract_uuid_opt};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

const PRIORITY_BOARD_APPOINTMENT: i32 = 1;
const PRIORITY_MANAGEMENT_CONTRACT: i32 = 2;
const PRIORITY_SPECIAL_RIGHTS: i32 = 3;
const PRIORITY_OFFICER: i32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ControllerRecord {
    controller_entity_id: Uuid,
    control_type: String,
    basis: String,
    priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ControlComputeResult {
    case_id: Uuid,
    entity_id: Uuid,
    controllers: Vec<ControllerRecord>,
    governance_controller_identified: bool,
}

fn classify_relationship(relationship_type: &str) -> (i32, &'static str) {
    match relationship_type {
        "BOARD_APPOINTMENT" => (PRIORITY_BOARD_APPOINTMENT, "BOARD_APPOINTMENT"),
        "MANAGEMENT" | "MANAGEMENT_CONTRACT" => {
            (PRIORITY_MANAGEMENT_CONTRACT, "MANAGEMENT_CONTRACT")
        }
        "SPECIAL_RIGHTS" | "CONTROL" => (PRIORITY_SPECIAL_RIGHTS, "SPECIAL_RIGHTS"),
        _ => (PRIORITY_SPECIAL_RIGHTS, "SPECIAL_RIGHTS"),
    }
}

// ── control.compute-controllers ───────────────────────────────────────────────

pub struct ComputeControllers;

#[async_trait]
impl SemOsVerbOp for ComputeControllers {
    fn fqn(&self) -> &str {
        "control.compute-controllers"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let case_id = json_extract_uuid(args, ctx, "case-id")?;
        let entity_id_filter = json_extract_uuid_opt(args, ctx, "entity-id");

        type ControlEdgeRow = (Uuid, Uuid, String, Option<f32>, Option<String>);
        let control_edges: Vec<ControlEdgeRow> = if let Some(eid) = entity_id_filter {
            sqlx::query_as(
                r#"SELECT er.from_entity_id, er.to_entity_id, er.relationship_type,
                          er.confidence, er.evidence_hint
                   FROM "ob-poc".entity_relationships er
                   JOIN "ob-poc".entity_workstreams ew ON er.to_entity_id = ew.entity_id
                   WHERE ew.case_id = $1
                     AND er.to_entity_id = $2
                     AND er.relationship_type IN (
                         'CONTROL', 'MANAGEMENT', 'BOARD_APPOINTMENT', 'SPECIAL_RIGHTS'
                     )
                     AND er.effective_to IS NULL
                   ORDER BY er.relationship_type, er.confidence DESC"#,
            )
            .bind(case_id)
            .bind(eid)
            .fetch_all(scope.executor())
            .await?
        } else {
            sqlx::query_as(
                r#"SELECT er.from_entity_id, er.to_entity_id, er.relationship_type,
                          er.confidence, er.evidence_hint
                   FROM "ob-poc".entity_relationships er
                   JOIN "ob-poc".entity_workstreams ew ON er.to_entity_id = ew.entity_id
                   WHERE ew.case_id = $1
                     AND er.relationship_type IN (
                         'CONTROL', 'MANAGEMENT', 'BOARD_APPOINTMENT', 'SPECIAL_RIGHTS'
                     )
                     AND er.effective_to IS NULL
                   ORDER BY er.relationship_type, er.confidence DESC"#,
            )
            .bind(case_id)
            .fetch_all(scope.executor())
            .await?
        };

        let mut controllers: Vec<ControllerRecord> = Vec::new();
        for (from_entity_id, _, relationship_type, confidence, evidence_hint) in &control_edges {
            let (priority, control_type) = classify_relationship(relationship_type);
            let basis = format!(
                "{}{}{}",
                relationship_type,
                confidence
                    .map(|c| format!(" (confidence: {:.2})", c))
                    .unwrap_or_default(),
                evidence_hint
                    .as_deref()
                    .map(|h| format!(" -- {}", h))
                    .unwrap_or_default(),
            );
            controllers.push(ControllerRecord {
                controller_entity_id: *from_entity_id,
                control_type: control_type.to_string(),
                basis,
                priority,
            });
        }

        type OfficerRow = (Uuid, String, Uuid);
        let officer_rows: Vec<OfficerRow> = if let Some(eid) = entity_id_filter {
            sqlx::query_as(
                r#"SELECT cer.entity_id, cer.role_type, cer.cbu_id
                   FROM "ob-poc".cbu_entity_roles cer
                   JOIN "ob-poc".entity_workstreams ew ON cer.entity_id = ew.entity_id
                   WHERE ew.case_id = $1
                     AND cer.entity_id = $2
                     AND cer.role_type IN (
                         'DIRECTOR', 'OFFICER', 'SECRETARY', 'COMPLIANCE_OFFICER'
                     )"#,
            )
            .bind(case_id)
            .bind(eid)
            .fetch_all(scope.executor())
            .await?
        } else {
            sqlx::query_as(
                r#"SELECT cer.entity_id, cer.role_type, cer.cbu_id
                   FROM "ob-poc".cbu_entity_roles cer
                   JOIN "ob-poc".entity_workstreams ew ON cer.entity_id = ew.entity_id
                   WHERE ew.case_id = $1
                     AND cer.role_type IN (
                         'DIRECTOR', 'OFFICER', 'SECRETARY', 'COMPLIANCE_OFFICER'
                     )"#,
            )
            .bind(case_id)
            .fetch_all(scope.executor())
            .await?
        };

        for (entity_id, role_type, _cbu_id) in &officer_rows {
            controllers.push(ControllerRecord {
                controller_entity_id: *entity_id,
                control_type: "OFFICER".to_string(),
                basis: format!("Officer role: {}", role_type),
                priority: PRIORITY_OFFICER,
            });
        }

        controllers.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.controller_entity_id.cmp(&b.controller_entity_id))
        });

        let governance_controller_identified = !controllers.is_empty();

        let result_entity_id = if let Some(eid) = entity_id_filter {
            eid
        } else {
            let first_entity: Option<(Uuid,)> = sqlx::query_as(
                r#"SELECT entity_id FROM "ob-poc".entity_workstreams
                   WHERE case_id = $1 ORDER BY entity_id LIMIT 1"#,
            )
            .bind(case_id)
            .fetch_optional(scope.executor())
            .await?;
            first_entity
                .map(|(eid,)| eid)
                .ok_or_else(|| anyhow!("No entity workstreams found for case {}", case_id))?
        };

        let result = ControlComputeResult {
            case_id,
            entity_id: result_entity_id,
            controllers,
            governance_controller_identified,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}
