//! Requirement custom operations
//!
//! Operations for document requirement management that integrate with
//! the workflow task queue (Migration 049).
//!
//! - `requirement.create-set` - Create multiple requirements in batch
//! - `requirement.list-outstanding` - List unsatisfied requirements for workflow

use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;
use uuid::Uuid;

use crate::custom_op::CustomOperation;
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

/// Create multiple document requirements for an entity
///
/// Rationale: Batch creation is more efficient than individual creates,
/// and all requirements share the same workflow context.
#[register_custom_op]
pub struct RequirementCreateSetOp;

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use crate::domain_ops::helpers::{
            json_extract_string_list, json_extract_string_opt, json_extract_uuid,
            json_extract_uuid_opt,
        };
        use sqlx::Row;

        let subject_entity_id = json_extract_uuid(args, ctx, "subject-entity-id")
            .or_else(|_| json_extract_uuid(args, ctx, "entity-id"))?;

        let doc_types = json_extract_string_list(args, "doc-types")?;

        if doc_types.is_empty() {
            return Err(anyhow::anyhow!("doc-types cannot be empty"));
        }

        let workflow_instance_id: Option<Uuid> =
            json_extract_uuid_opt(args, ctx, "workflow-instance-id");

        let due_date: Option<NaiveDate> = json_extract_string_opt(args, "due-date")
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

        let required_state = json_extract_string_opt(args, "required-state")
            .unwrap_or_else(|| "verified".to_string());

        let mut requirement_ids: Vec<Uuid> = Vec::with_capacity(doc_types.len());

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
            .bind(&required_state)
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

        Ok(VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// List unsatisfied requirements for a workflow
///
/// Rationale: Complex query with joined fields for workflow progress display.
#[register_custom_op]
pub struct RequirementUnsatisfiedOp;

#[async_trait]
impl CustomOperation for RequirementUnsatisfiedOp {
    fn domain(&self) -> &'static str {
        "requirement"
    }
    fn verb(&self) -> &'static str {
        "list-outstanding"
    }
    fn rationale(&self) -> &'static str {
        "Complex query for workflow progress tracking"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use crate::domain_ops::helpers::json_extract_uuid;
        use sqlx::Row;

        let workflow_instance_id = json_extract_uuid(args, ctx, "workflow-instance-id")?;

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

        Ok(VerbExecutionOutcome::RecordSet(results))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
