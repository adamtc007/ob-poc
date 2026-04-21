//! Requirement verbs — SemOS-side YAML-first re-implementation.
//!
//! Two plugin verbs backing document-requirement management
//! (Migration 049 workflow task queue):
//! - `requirement.create-set` — batch create/upsert of requirements.
//! - `requirement.list-outstanding` — list unsatisfied requirements
//!   for workflow progress display.
//!
//! Writes + reads all route through `scope.executor()` so the batch
//! insert in `create-set` commits atomically inside the Sequencer's txn.

use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::Row;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string_list, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

pub struct CreateSet;

#[async_trait]
impl SemOsVerbOp for CreateSet {
    fn fqn(&self) -> &str {
        "requirement.create-set"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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

        let required_state =
            json_extract_string_opt(args, "required-state").unwrap_or_else(|| "verified".to_string());

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
            .fetch_one(scope.executor())
            .await?;

            let requirement_id: Uuid = requirement_row.get("requirement_id");
            requirement_ids.push(requirement_id);
        }

        let count = requirement_ids.len();
        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "requirement_ids": requirement_ids,
            "count": count,
        })))
    }
}

pub struct ListOutstanding;

#[async_trait]
impl SemOsVerbOp for ListOutstanding {
    fn fqn(&self) -> &str {
        "requirement.list-outstanding"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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
        .fetch_all(scope.executor())
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
                    "last_rejection_reason": row.get::<Option<String>, _>("last_rejection_reason"),
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}
