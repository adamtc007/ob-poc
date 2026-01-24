//! Research Workflow Operations
//!
//! Plugin handlers for research workflow management:
//! - Decision confirmation/rejection
//! - Audit trail queries
//!
//! Most operations use CRUD via YAML definitions. These handlers
//! implement complex logic that can't be expressed in CRUD.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde_json::json;
use uuid::Uuid;

use super::{CustomOperation, ExecutionResult};
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::ExecutionContext;

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// CONFIRM DECISION
// =============================================================================

/// Confirm an ambiguous decision with user's selection
///
/// Rationale: Updates decision record and may trigger follow-up import
#[register_custom_op]
pub struct WorkflowConfirmDecisionOp;

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
        // Extract arguments
        let decision_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "decision-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing decision-id argument"))?;

        let selected_key = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "selected-key")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing selected-key argument"))?;

        let selected_key_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "selected-key-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing selected-key-type argument"))?;

        // Try to parse audit_user as UUID, otherwise use None
        let verified_by: Option<Uuid> = ctx.audit_user.as_ref().and_then(|s| s.parse().ok());

        // Update the decision
        let result = sqlx::query!(
            r#"
            UPDATE kyc.research_decisions
            SET
                selected_key = $2,
                selected_key_type = $3,
                decision_type = 'USER_CONFIRMED',
                verified_by = $4,
                verified_at = NOW()
            WHERE decision_id = $1
            RETURNING decision_id, target_entity_id, source_provider
            "#,
            decision_id,
            selected_key,
            selected_key_type,
            verified_by
        )
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Record(json!({
            "decision_id": result.decision_id,
            "target_entity_id": result.target_entity_id,
            "source_provider": result.source_provider,
            "selected_key": selected_key,
            "selected_key_type": selected_key_type,
            "status": "confirmed"
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"status": "confirmed"})))
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
        let decision_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "decision-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing decision-id argument"))?;

        let rejection_reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "rejection-reason")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing rejection-reason argument"))?;

        // Try to parse audit_user as UUID, otherwise use None
        let verified_by: Option<Uuid> = ctx.audit_user.as_ref().and_then(|s| s.parse().ok());

        // Update the decision
        let result = sqlx::query!(
            r#"
            UPDATE kyc.research_decisions
            SET
                decision_type = 'REJECTED',
                selection_reasoning = selection_reasoning || ' | REJECTED: ' || $2,
                verified_by = $3,
                verified_at = NOW()
            WHERE decision_id = $1
            RETURNING decision_id, target_entity_id
            "#,
            decision_id,
            rejection_reason,
            verified_by
        )
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Record(json!({
            "decision_id": result.decision_id,
            "target_entity_id": result.target_entity_id,
            "status": "rejected",
            "reason": rejection_reason
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"status": "rejected"})))
    }
}

// =============================================================================
// AUDIT TRAIL
// =============================================================================

/// Get complete research audit trail for an entity
///
/// Rationale: Aggregates decisions, actions, corrections, and anomalies
#[register_custom_op]
pub struct WorkflowAuditTrailOp;

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
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        let include_decisions = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "include-decisions")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        let include_actions = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "include-actions")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        let include_corrections = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "include-corrections")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        let include_anomalies = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "include-anomalies")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        let mut result = json!({
            "entity_id": entity_id
        });

        // Fetch decisions
        if include_decisions {
            let decisions = sqlx::query!(
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
                FROM kyc.research_decisions
                WHERE target_entity_id = $1
                ORDER BY created_at DESC
                "#,
                entity_id
            )
            .fetch_all(pool)
            .await?;

            result["decisions"] = json!(decisions
                .iter()
                .map(|d| {
                    json!({
                        "decision_id": d.decision_id,
                        "search_query": d.search_query,
                        "source_provider": d.source_provider,
                        "candidates_count": d.candidates_count,
                        "selected_key": d.selected_key,
                        "selected_key_type": d.selected_key_type,
                        "confidence": d.selection_confidence,
                        "decision_type": d.decision_type,
                        "auto_selected": d.auto_selected,
                        "created_at": d.created_at
                    })
                })
                .collect::<Vec<_>>());
        }

        // Fetch actions
        if include_actions {
            let actions = sqlx::query!(
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
                FROM kyc.research_actions
                WHERE target_entity_id = $1
                ORDER BY executed_at DESC
                "#,
                entity_id
            )
            .fetch_all(pool)
            .await?;

            result["actions"] = json!(actions
                .iter()
                .map(|a| {
                    json!({
                        "action_id": a.action_id,
                        "decision_id": a.decision_id,
                        "action_type": a.action_type,
                        "source_provider": a.source_provider,
                        "source_key": a.source_key,
                        "verb": format!("{}:{}", a.verb_domain, a.verb_name),
                        "success": a.success,
                        "entities_created": a.entities_created,
                        "entities_updated": a.entities_updated,
                        "relationships_created": a.relationships_created,
                        "error_message": a.error_message,
                        "duration_ms": a.duration_ms,
                        "executed_at": a.executed_at
                    })
                })
                .collect::<Vec<_>>());
        }

        // Fetch corrections
        if include_corrections {
            let corrections = sqlx::query!(
                r#"
                SELECT
                    c.correction_id,
                    c.original_decision_id,
                    c.correction_type,
                    c.wrong_key,
                    c.correct_key,
                    c.correction_reason,
                    c.corrected_at
                FROM kyc.research_corrections c
                JOIN kyc.research_decisions d ON d.decision_id = c.original_decision_id
                WHERE d.target_entity_id = $1
                ORDER BY c.corrected_at DESC
                "#,
                entity_id
            )
            .fetch_all(pool)
            .await?;

            result["corrections"] = json!(corrections
                .iter()
                .map(|c| {
                    json!({
                        "correction_id": c.correction_id,
                        "original_decision_id": c.original_decision_id,
                        "correction_type": c.correction_type,
                        "wrong_key": c.wrong_key,
                        "correct_key": c.correct_key,
                        "reason": c.correction_reason,
                        "corrected_at": c.corrected_at
                    })
                })
                .collect::<Vec<_>>());
        }

        // Fetch anomalies
        if include_anomalies {
            let anomalies = sqlx::query!(
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
                FROM kyc.research_anomalies
                WHERE entity_id = $1
                ORDER BY detected_at DESC
                "#,
                entity_id
            )
            .fetch_all(pool)
            .await?;

            result["anomalies"] = json!(anomalies
                .iter()
                .map(|a| {
                    json!({
                        "anomaly_id": a.anomaly_id,
                        "action_id": a.action_id,
                        "rule_code": a.rule_code,
                        "severity": a.severity,
                        "description": a.description,
                        "status": a.status,
                        "resolution": a.resolution,
                        "detected_at": a.detected_at
                    })
                })
                .collect::<Vec<_>>());
        }

        Ok(ExecutionResult::Record(result))
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
            "anomalies": []
        })))
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
    }
}
