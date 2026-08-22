//! Document verbs (5 plugin verbs) — YAML-first re-implementation of
//! `document.*` from `rust/config/verbs/document.yaml`.
//!
//! Ops:
//! - `catalog` — idempotent upsert into `document_catalog`, keyed on
//!   (cbu_id, document_type_id, document_name)
//! - `extract` — flip `extraction_status = 'IN_PROGRESS'` (async
//!   OCR/AI mopping-up lives in the workflow tier)
//! - `solicit` — create `document_requirements` row + matching
//!   `workflow_pending_tasks` entry for the external-system relay
//! - `solicit-batch` — one multi-result task, N requirements, sharing
//!   the same `task_id`
//! - `missing-for-entity` — governed requirements first, fall back to
//!   the raw `document_requirements` table
//! - `compute-requirements` — full governed matrix (snapshot set id +
//!   obligation coverage)
//!
//! `GovernedDocumentRequirementsService` still takes `PgPool` by value;
//! until it's scoped, solicit-batch / missing-for-entity /
//! compute-requirements use the transitional `scope.pool().clone()`
//! pattern — the reads are outside the ambient txn but the verb
//! contract (read-and-compute) doesn't require strong isolation.
//!
//! `upload-version` / `verify` / `reject` (plus the crud `start-qa`,
//! `list-versions`, `get`) were removed 2026-08-20
//! (EOP-PLAN-MANDATE-FIX-001 F4, no_verb_calls_absent_function /
//! no_orphan_child_table): the whole "Layer C" document-version-and-QA
//! workflow was built against `"ob-poc".document_versions` and
//! `"ob-poc".v_documents_with_status`, neither of which exists in the
//! live schema, plus a call to the never-created
//! `"ob-poc".get_next_document_version()`. Every real invocation of any
//! of the 6 verbs failed. Full concept removal, not a regression.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use dsl_runtime::GovernedDocumentRequirementsService;
use dsl_runtime::TransactionScope;
use dsl_runtime::{json_extract_string, json_extract_uuid, json_extract_uuid_opt};
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ---------------------------------------------------------------------------
// catalog / extract
// ---------------------------------------------------------------------------

pub struct Catalog;

#[async_trait]
impl SemOsVerbOp for Catalog {
    fn fqn(&self) -> &str {
        "document.catalog"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let doc_type = json_extract_string(args, "doc-type")
            .or_else(|_| json_extract_string(args, "document-type"))?;
        let document_name = args
            .get("title")
            .and_then(|v| v.as_str())
            .or_else(|| args.get("document-name").and_then(|v| v.as_str()))
            .map(|s| s.to_string());
        let cbu_id = json_extract_uuid_opt(args, ctx, "cbu-id");
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");

        let doc_type_id: Uuid = sqlx::query_scalar(
            r#"SELECT type_id FROM "ob-poc".document_types WHERE type_code = $1"#,
        )
        .bind(&doc_type)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Unknown document type: {}", doc_type))?;

        let existing: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT doc_id FROM "ob-poc".document_catalog
               WHERE cbu_id IS NOT DISTINCT FROM $1
               AND document_type_id = $2
               AND document_name IS NOT DISTINCT FROM $3
               LIMIT 1"#,
        )
        .bind(cbu_id)
        .bind(doc_type_id)
        .bind(&document_name)
        .fetch_optional(scope.executor())
        .await?;

        let doc_id = if let Some(doc_id) = existing {
            doc_id
        } else {
            let doc_id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO "ob-poc".document_catalog
                   (doc_id, document_type_id, cbu_id, entity_id, document_name, status)
                   VALUES ($1, $2, $3, $4, $5, 'active')"#,
            )
            .bind(doc_id)
            .bind(doc_type_id)
            .bind(cbu_id)
            .bind(entity_id)
            .bind(&document_name)
            .execute(scope.executor())
            .await?;
            doc_id
        };

        ctx.bind("document", doc_id);
        Ok(VerbExecutionOutcome::Uuid(doc_id))
    }
}

pub struct Extract;

#[async_trait]
impl SemOsVerbOp for Extract {
    fn fqn(&self) -> &str {
        "document.extract"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let doc_id = json_extract_uuid(args, ctx, "document-id")
            .or_else(|_| json_extract_uuid(args, ctx, "doc-id"))?;
        sqlx::query(
            r#"UPDATE "ob-poc".document_catalog SET extraction_status = 'IN_PROGRESS' WHERE doc_id = $1"#,
        )
        .bind(doc_id)
        .execute(scope.executor())
        .await?;
        Ok(VerbExecutionOutcome::Void)
    }
}

// ---------------------------------------------------------------------------
// upload-version / verify / reject
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// missing-for-entity / compute-requirements
// ---------------------------------------------------------------------------

pub struct MissingForEntity;

/// Wrapper to support list-missing as an alias for missing-for-entity.
pub struct ListMissing;

#[async_trait]
impl SemOsVerbOp for ListMissing {
    fn fqn(&self) -> &str {
        "document.list-missing"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        MissingForEntity.execute(args, ctx, scope).await
    }
}

#[async_trait]
impl SemOsVerbOp for MissingForEntity {
    fn fqn(&self) -> &str {
        "document.missing-for-entity"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let workflow_instance_id = json_extract_uuid_opt(args, ctx, "workflow-instance-id");

        let governed_service = GovernedDocumentRequirementsService::new(scope.pool().clone());
        if let Some(governed) = governed_service.compute_for_entity(entity_id).await? {
            let results: Vec<Value> = governed
                .gaps
                .into_iter()
                .map(|gap| {
                    json!({
                        "requirement_id": Value::Null,
                        "doc_type": gap.document_type_fqn,
                        "status": gap.status,
                        "required_state": gap.required_state,
                        "attempt_count": 0,
                        "last_rejection_code": gap.last_rejection_code,
                        "requirement_profile_fqn": governed.requirement_profile_fqn,
                        "obligation_fqn": gap.obligation_fqn,
                        "obligation_category": gap.obligation_category,
                        "strategy_fqn": gap.strategy_fqn,
                        "strategy_priority": gap.strategy_priority,
                        "matched_document_id": gap.matched_document_id,
                        "matched_version_id": gap.matched_version_id,
                        "snapshot_set_id": governed.snapshot_set_id
                    })
                })
                .collect();
            return Ok(VerbExecutionOutcome::RecordSet(results));
        }

        let rows = sqlx::query(
            r#"
            SELECT
                requirement_id, doc_type, status, required_state,
                attempt_count, last_rejection_code
            FROM "ob-poc".document_requirements
            WHERE subject_entity_id = $1
              AND ($2::uuid IS NULL OR workflow_instance_id = $2)
              AND status NOT IN ('verified', 'waived')
            ORDER BY doc_type
            "#,
        )
        .bind(entity_id)
        .bind(workflow_instance_id)
        .fetch_all(scope.executor())
        .await?;

        let results: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "requirement_id": row.get::<Uuid, _>("requirement_id"),
                    "doc_type": row.get::<String, _>("doc_type"),
                    "status": row.get::<String, _>("status"),
                    "required_state": row.get::<String, _>("required_state"),
                    "attempt_count": row.get::<i32, _>("attempt_count"),
                    "last_rejection_code": row.get::<Option<String>, _>("last_rejection_code")
                })
            })
            .collect();
        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

pub struct ComputeRequirements;

#[async_trait]
impl SemOsVerbOp for ComputeRequirements {
    fn fqn(&self) -> &str {
        "document.compute-requirements"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let governed_service = GovernedDocumentRequirementsService::new(scope.pool().clone());
        let matrix = governed_service
            .compute_matrix_for_entity(entity_id)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "No active governed requirement profile matched entity {}",
                    entity_id
                )
            })?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(matrix)?))
    }
}
