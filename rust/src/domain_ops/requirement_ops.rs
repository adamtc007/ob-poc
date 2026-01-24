//! Requirement custom operations
//!
//! Operations for document requirement management that integrate with
//! the workflow task queue (Migration 049).
//!
//! - `requirement.create-set` - Create multiple requirements in batch
//! - `requirement.unsatisfied` - List unsatisfied requirements for workflow

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use chrono::NaiveDate;

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use uuid::Uuid;

/// Create multiple document requirements for an entity
///
/// Rationale: Batch creation is more efficient than individual creates,
/// and all requirements share the same workflow context.
#[register_custom_op]
#[cfg(feature = "database")]
pub struct RequirementCreateSetOp;

#[cfg(feature = "database")]
#[async_trait]
impl CustomOperation for RequirementCreateSetOp {
    fn domain(&self) -> &'static str {
        "requirement"
    }
    fn verb(&self) -> &'static str {
        "create-set"
    }
    fn rationale(&self) -> &'static str {
        "Batch creation of requirements with shared workflow context"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use sqlx::Row;

        // Extract required arguments
        let subject_entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "subject-entity-id" || a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing subject-entity-id argument"))?;

        let doc_types: Vec<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "doc-types")
            .and_then(|a| a.value.as_list())
            .map(|list| {
                list.iter()
                    .filter_map(|item| item.as_string().map(|s| s.to_string()))
                    .collect()
            })
            .ok_or_else(|| anyhow::anyhow!("Missing doc-types argument"))?;

        if doc_types.is_empty() {
            return Err(anyhow::anyhow!("doc-types cannot be empty"));
        }

        // Optional arguments
        let workflow_instance_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "workflow-instance-id")
            .and_then(|a| a.value.as_uuid());

        let due_date: Option<NaiveDate> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "due-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let required_state = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "required-state")
            .and_then(|a| a.value.as_string())
            .unwrap_or("verified");

        let mut requirement_ids: Vec<Uuid> = Vec::with_capacity(doc_types.len());

        // Create requirements for each doc type
        for doc_type in &doc_types {
            let requirement_row = sqlx::query(
                r#"
                INSERT INTO "ob-poc".document_requirements
                    (subject_entity_id, workflow_instance_id, doc_type, required_state, due_date, status)
                VALUES ($1, $2, $3, $4, $5, 'missing')
                ON CONFLICT (workflow_instance_id, subject_entity_id, doc_type)
                DO UPDATE SET
                    due_date = COALESCE(EXCLUDED.due_date, document_requirements.due_date),
                    updated_at = now()
                RETURNING requirement_id
                "#,
            )
            .bind(subject_entity_id)
            .bind(workflow_instance_id)
            .bind(doc_type)
            .bind(required_state)
            .bind(due_date)
            .fetch_one(pool)
            .await?;

            let requirement_id: Uuid = requirement_row.get("requirement_id");
            requirement_ids.push(requirement_id);
        }

        let result = serde_json::json!({
            "requirement_ids": requirement_ids,
            "count": requirement_ids.len()
        });

        Ok(ExecutionResult::Record(result))
    }
}

/// List unsatisfied requirements for a workflow
///
/// Rationale: Complex query with joined fields for workflow progress display.
#[register_custom_op]
#[cfg(feature = "database")]
pub struct RequirementUnsatisfiedOp;

#[cfg(feature = "database")]
#[async_trait]
impl CustomOperation for RequirementUnsatisfiedOp {
    fn domain(&self) -> &'static str {
        "requirement"
    }
    fn verb(&self) -> &'static str {
        "unsatisfied"
    }
    fn rationale(&self) -> &'static str {
        "Complex query for workflow progress tracking"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use sqlx::Row;

        // Extract required argument
        let workflow_instance_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "workflow-instance-id")
            .and_then(|a| a.value.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("Missing workflow-instance-id argument"))?;

        let rows = sqlx::query(
            r#"
            SELECT
                requirement_id,
                doc_type,
                subject_entity_id,
                status,
                required_state,
                attempt_count,
                last_rejection_code,
                last_rejection_reason
            FROM "ob-poc".document_requirements
            WHERE workflow_instance_id = $1
              AND status NOT IN ('verified', 'waived')
            ORDER BY doc_type
            "#,
        )
        .bind(workflow_instance_id)
        .fetch_all(pool)
        .await?;

        let results: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "requirement_id": row.get::<Uuid, _>("requirement_id"),
                    "doc_type": row.get::<String, _>("doc_type"),
                    "subject_entity_id": row.get::<Uuid, _>("subject_entity_id"),
                    "status": row.get::<String, _>("status"),
                    "required_state": row.get::<String, _>("required_state"),
                    "attempt_count": row.get::<i32, _>("attempt_count"),
                    "last_rejection_code": row.get::<Option<String>, _>("last_rejection_code"),
                    "last_rejection_reason": row.get::<Option<String>, _>("last_rejection_reason")
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(results))
    }
}
