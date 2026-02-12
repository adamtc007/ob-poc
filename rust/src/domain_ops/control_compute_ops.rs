//! Control Compute Operations (Phase 3.1)
//!
//! Computes control controllers per KYC/UBO architecture spec section 6.2:
//! 1. Load CONTROL and MANAGEMENT edges from `entity_relationships` for a case's entities
//! 2. Priority ordering: Board appointment > Management contract > Special rights (veto/golden share)
//! 3. Bridge to `cbu_entity_roles` for officers
//! 4. Output `(controller_entity_id, control_type, basis)` tuples
//!
//! ## Key Tables
//! - kyc.entity_workstreams (case → entity linkage)
//! - "ob-poc".entity_relationships (control/management edges)
//! - "ob-poc".cbu_entity_roles (officer roles)
//!
//! ## Priority Ordering
//! - 1: BOARD_APPOINTMENT (highest priority)
//! - 2: MANAGEMENT_CONTRACT
//! - 3: SPECIAL_RIGHTS (veto, golden share)
//! - 4: OFFICER (bridged from cbu_entity_roles)

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::{extract_uuid, extract_uuid_opt};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

// ============================================================================
// Result Types (typed structs per CLAUDE.md Non-Negotiable Rule #1)
// ============================================================================

/// Top-level result of a control compute operation for a KYC case entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlComputeResult {
    pub case_id: Uuid,
    pub entity_id: Uuid,
    pub controllers: Vec<ControllerRecord>,
    pub governance_controller_identified: bool,
}

/// A single controller record with type, basis, and priority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerRecord {
    pub controller_entity_id: Uuid,
    pub control_type: String,
    pub basis: String,
    pub priority: i32,
}

// ============================================================================
// Priority Constants
// ============================================================================

/// Board appointment: highest priority (e.g., majority board control).
const PRIORITY_BOARD_APPOINTMENT: i32 = 1;

/// Management contract: second priority (e.g., ManCo agreement).
const PRIORITY_MANAGEMENT_CONTRACT: i32 = 2;

/// Special rights: third priority (e.g., veto, golden share).
const PRIORITY_SPECIAL_RIGHTS: i32 = 3;

/// Officer role: lowest priority (bridged from cbu_entity_roles).
const PRIORITY_OFFICER: i32 = 4;

/// Map a relationship_type string to its priority and canonical control_type.
fn classify_relationship(relationship_type: &str) -> (i32, &'static str) {
    match relationship_type {
        "BOARD_APPOINTMENT" => (PRIORITY_BOARD_APPOINTMENT, "BOARD_APPOINTMENT"),
        "MANAGEMENT" | "MANAGEMENT_CONTRACT" => {
            (PRIORITY_MANAGEMENT_CONTRACT, "MANAGEMENT_CONTRACT")
        }
        "SPECIAL_RIGHTS" => (PRIORITY_SPECIAL_RIGHTS, "SPECIAL_RIGHTS"),
        "CONTROL" => {
            // Generic CONTROL edges default to SPECIAL_RIGHTS priority
            // since they lack a more specific classification.
            (PRIORITY_SPECIAL_RIGHTS, "SPECIAL_RIGHTS")
        }
        _ => (PRIORITY_SPECIAL_RIGHTS, "SPECIAL_RIGHTS"),
    }
}

/// Map an officer role_type string to a human-readable basis.
fn officer_basis(role_type: &str) -> String {
    format!("Officer role: {}", role_type)
}

// ============================================================================
// ControlComputeOp
// ============================================================================

/// Compute control controllers for entities linked to a KYC case.
///
/// Rationale: Requires loading and prioritizing multiple edge types (CONTROL,
/// MANAGEMENT, BOARD_APPOINTMENT, SPECIAL_RIGHTS) from entity_relationships,
/// bridging to cbu_entity_roles for officer data, deduplicating by
/// controller_entity_id, and sorting by priority. Cannot be expressed as
/// a single CRUD verb.
#[register_custom_op]
pub struct ControlComputeOp;

#[async_trait]
impl CustomOperation for ControlComputeOp {
    fn domain(&self) -> &'static str {
        "control"
    }

    fn verb(&self) -> &'static str {
        "compute-controllers"
    }

    fn rationale(&self) -> &'static str {
        "Loads and prioritizes multiple control edge types (CONTROL, MANAGEMENT, \
         BOARD_APPOINTMENT, SPECIAL_RIGHTS) from entity_relationships, bridges to \
         cbu_entity_roles for officer roles, deduplicates by controller, and sorts \
         by priority. Multi-source aggregation cannot be expressed as CRUD."
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // ------------------------------------------------------------------
        // 1. Extract arguments
        // ------------------------------------------------------------------
        let case_id = extract_uuid(verb_call, ctx, "case-id")?;

        // Optional entity-id to narrow scope to a single entity within the case.
        // When omitted, all entities in the case are considered as targets.
        let entity_id_filter = extract_uuid_opt(verb_call, ctx, "entity-id");

        // ------------------------------------------------------------------
        // 2. Load CONTROL / MANAGEMENT / BOARD_APPOINTMENT / SPECIAL_RIGHTS
        //    edges for entities linked to this case.
        // ------------------------------------------------------------------
        let control_edges: Vec<(Uuid, Uuid, String, Option<f32>, Option<String>)> =
            if let Some(eid) = entity_id_filter {
                sqlx::query_as(
                    r#"SELECT er.from_entity_id, er.to_entity_id, er.relationship_type,
                              er.confidence, er.evidence_hint
                       FROM "ob-poc".entity_relationships er
                       JOIN kyc.entity_workstreams ew ON er.to_entity_id = ew.entity_id
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
                .fetch_all(pool)
                .await?
            } else {
                sqlx::query_as(
                    r#"SELECT er.from_entity_id, er.to_entity_id, er.relationship_type,
                              er.confidence, er.evidence_hint
                       FROM "ob-poc".entity_relationships er
                       JOIN kyc.entity_workstreams ew ON er.to_entity_id = ew.entity_id
                       WHERE ew.case_id = $1
                         AND er.relationship_type IN (
                             'CONTROL', 'MANAGEMENT', 'BOARD_APPOINTMENT', 'SPECIAL_RIGHTS'
                         )
                         AND er.effective_to IS NULL
                       ORDER BY er.relationship_type, er.confidence DESC"#,
                )
                .bind(case_id)
                .fetch_all(pool)
                .await?
            };

        // ------------------------------------------------------------------
        // 3. Convert edges to ControllerRecords with priority classification
        // ------------------------------------------------------------------
        let mut controllers: Vec<ControllerRecord> = Vec::new();

        for (from_entity_id, _to_entity_id, relationship_type, confidence, evidence_hint) in
            &control_edges
        {
            let (priority, control_type) = classify_relationship(relationship_type);

            let basis = format!(
                "{}{}{}",
                relationship_type,
                confidence
                    .map(|c| format!(" (confidence: {:.2})", c))
                    .unwrap_or_default(),
                evidence_hint
                    .as_deref()
                    .map(|h| format!(" — {}", h))
                    .unwrap_or_default(),
            );

            controllers.push(ControllerRecord {
                controller_entity_id: *from_entity_id,
                control_type: control_type.to_string(),
                basis,
                priority,
            });
        }

        // ------------------------------------------------------------------
        // 4. Bridge to cbu_entity_roles for officer roles
        // ------------------------------------------------------------------
        let officer_rows: Vec<(Uuid, String, Uuid)> = if let Some(eid) = entity_id_filter {
            sqlx::query_as(
                r#"SELECT cer.entity_id, cer.role_type, cer.cbu_id
                   FROM "ob-poc".cbu_entity_roles cer
                   JOIN kyc.entity_workstreams ew ON cer.entity_id = ew.entity_id
                   WHERE ew.case_id = $1
                     AND cer.entity_id = $2
                     AND cer.role_type IN (
                         'DIRECTOR', 'OFFICER', 'SECRETARY', 'COMPLIANCE_OFFICER'
                     )"#,
            )
            .bind(case_id)
            .bind(eid)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                r#"SELECT cer.entity_id, cer.role_type, cer.cbu_id
                   FROM "ob-poc".cbu_entity_roles cer
                   JOIN kyc.entity_workstreams ew ON cer.entity_id = ew.entity_id
                   WHERE ew.case_id = $1
                     AND cer.role_type IN (
                         'DIRECTOR', 'OFFICER', 'SECRETARY', 'COMPLIANCE_OFFICER'
                     )"#,
            )
            .bind(case_id)
            .fetch_all(pool)
            .await?
        };

        for (entity_id, role_type, _cbu_id) in &officer_rows {
            controllers.push(ControllerRecord {
                controller_entity_id: *entity_id,
                control_type: "OFFICER".to_string(),
                basis: officer_basis(role_type),
                priority: PRIORITY_OFFICER,
            });
        }

        // ------------------------------------------------------------------
        // 5. Sort by priority (ascending = highest priority first),
        //    then by controller_entity_id for deterministic output.
        // ------------------------------------------------------------------
        controllers.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.controller_entity_id.cmp(&b.controller_entity_id))
        });

        let governance_controller_identified = !controllers.is_empty();

        // ------------------------------------------------------------------
        // 6. Determine the target entity_id for the result.
        //    If a filter was provided, use it; otherwise use the first entity
        //    from the case workstreams.
        // ------------------------------------------------------------------
        let result_entity_id = if let Some(eid) = entity_id_filter {
            eid
        } else {
            let first_entity: Option<(Uuid,)> = sqlx::query_as(
                r#"SELECT entity_id FROM kyc.entity_workstreams
                   WHERE case_id = $1 ORDER BY entity_id LIMIT 1"#,
            )
            .bind(case_id)
            .fetch_optional(pool)
            .await?;

            first_entity
                .map(|(eid,)| eid)
                .ok_or_else(|| anyhow!("No entity workstreams found for case {}", case_id))?
        };

        // ------------------------------------------------------------------
        // 7. Build and return typed result
        // ------------------------------------------------------------------
        tracing::info!(
            "control.compute-controllers: case={} entity={} controllers_found={} governance={}",
            case_id,
            result_entity_id,
            controllers.len(),
            governance_controller_identified,
        );

        let result = ControlComputeResult {
            case_id,
            entity_id: result_entity_id,
            controllers,
            governance_controller_identified,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "case_id": Uuid::nil(),
            "entity_id": Uuid::nil(),
            "controllers": [],
            "governance_controller_identified": false
        })))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_metadata() {
        let op = ControlComputeOp;
        assert_eq!(op.domain(), "control");
        assert_eq!(op.verb(), "compute-controllers");
        assert!(
            !op.rationale().is_empty(),
            "Rationale must be non-empty for documentation"
        );
    }

    #[test]
    fn test_priority_ordering() {
        // BOARD_APPOINTMENT (1) > MANAGEMENT_CONTRACT (2) > SPECIAL_RIGHTS (3) > OFFICER (4)
        assert!(
            PRIORITY_BOARD_APPOINTMENT < PRIORITY_MANAGEMENT_CONTRACT,
            "Board appointment must have higher priority (lower number) than management contract"
        );
        assert!(
            PRIORITY_MANAGEMENT_CONTRACT < PRIORITY_SPECIAL_RIGHTS,
            "Management contract must have higher priority than special rights"
        );
        assert!(
            PRIORITY_SPECIAL_RIGHTS < PRIORITY_OFFICER,
            "Special rights must have higher priority than officer"
        );
    }

    #[test]
    fn test_classify_relationship() {
        let (p, t) = classify_relationship("BOARD_APPOINTMENT");
        assert_eq!(p, 1);
        assert_eq!(t, "BOARD_APPOINTMENT");

        let (p, t) = classify_relationship("MANAGEMENT");
        assert_eq!(p, 2);
        assert_eq!(t, "MANAGEMENT_CONTRACT");

        let (p, t) = classify_relationship("MANAGEMENT_CONTRACT");
        assert_eq!(p, 2);
        assert_eq!(t, "MANAGEMENT_CONTRACT");

        let (p, t) = classify_relationship("SPECIAL_RIGHTS");
        assert_eq!(p, 3);
        assert_eq!(t, "SPECIAL_RIGHTS");

        let (p, t) = classify_relationship("CONTROL");
        assert_eq!(p, 3);
        assert_eq!(t, "SPECIAL_RIGHTS");
    }

    #[test]
    fn test_officer_basis_format() {
        let basis = officer_basis("DIRECTOR");
        assert_eq!(basis, "Officer role: DIRECTOR");

        let basis = officer_basis("COMPLIANCE_OFFICER");
        assert_eq!(basis, "Officer role: COMPLIANCE_OFFICER");
    }

    #[test]
    fn test_controller_record_serialization() {
        let record = ControllerRecord {
            controller_entity_id: Uuid::nil(),
            control_type: "BOARD_APPOINTMENT".to_string(),
            basis: "Board majority control".to_string(),
            priority: 1,
        };

        let json = serde_json::to_value(&record).expect("Should serialize");
        assert_eq!(json["control_type"], "BOARD_APPOINTMENT");
        assert_eq!(json["priority"], 1);
        assert_eq!(json["basis"], "Board majority control");
    }

    #[test]
    fn test_result_serialization() {
        let result = ControlComputeResult {
            case_id: Uuid::nil(),
            entity_id: Uuid::nil(),
            controllers: vec![
                ControllerRecord {
                    controller_entity_id: Uuid::nil(),
                    control_type: "BOARD_APPOINTMENT".to_string(),
                    basis: "Board appointment".to_string(),
                    priority: PRIORITY_BOARD_APPOINTMENT,
                },
                ControllerRecord {
                    controller_entity_id: Uuid::nil(),
                    control_type: "OFFICER".to_string(),
                    basis: "Officer role: DIRECTOR".to_string(),
                    priority: PRIORITY_OFFICER,
                },
            ],
            governance_controller_identified: true,
        };

        let json = serde_json::to_value(&result).expect("Should serialize");
        assert_eq!(json["governance_controller_identified"], true);

        let controllers = json["controllers"].as_array().expect("Should be array");
        assert_eq!(controllers.len(), 2);
        // First controller should be higher priority (lower number)
        assert_eq!(controllers[0]["priority"], 1);
        assert_eq!(controllers[1]["priority"], 4);
    }

    #[test]
    fn test_sort_by_priority_then_entity_id() {
        let id_a = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let id_b = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();

        let mut controllers = vec![
            ControllerRecord {
                controller_entity_id: id_b,
                control_type: "OFFICER".to_string(),
                basis: "Officer role: DIRECTOR".to_string(),
                priority: PRIORITY_OFFICER,
            },
            ControllerRecord {
                controller_entity_id: id_a,
                control_type: "BOARD_APPOINTMENT".to_string(),
                basis: "Board appointment".to_string(),
                priority: PRIORITY_BOARD_APPOINTMENT,
            },
            ControllerRecord {
                controller_entity_id: id_b,
                control_type: "MANAGEMENT_CONTRACT".to_string(),
                basis: "Management contract".to_string(),
                priority: PRIORITY_MANAGEMENT_CONTRACT,
            },
            ControllerRecord {
                controller_entity_id: id_a,
                control_type: "SPECIAL_RIGHTS".to_string(),
                basis: "Golden share veto".to_string(),
                priority: PRIORITY_SPECIAL_RIGHTS,
            },
        ];

        controllers.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.controller_entity_id.cmp(&b.controller_entity_id))
        });

        assert_eq!(controllers[0].control_type, "BOARD_APPOINTMENT");
        assert_eq!(controllers[0].priority, 1);
        assert_eq!(controllers[1].control_type, "MANAGEMENT_CONTRACT");
        assert_eq!(controllers[1].priority, 2);
        assert_eq!(controllers[2].control_type, "SPECIAL_RIGHTS");
        assert_eq!(controllers[2].priority, 3);
        assert_eq!(controllers[3].control_type, "OFFICER");
        assert_eq!(controllers[3].priority, 4);
    }
}
