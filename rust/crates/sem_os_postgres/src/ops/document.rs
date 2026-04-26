//! Document verbs (9 plugin verbs) — YAML-first re-implementation of
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
//! - `upload-version` — insert a `document_versions` row with a
//!   monotonic version number via `fn_get_next_document_version`
//! - `verify` — QA approval (`verification_status = 'verified'`)
//! - `reject` — QA rejection with standardized reason code against
//!   `rejection_reason_codes`
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

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use dsl_runtime::document_requirements::GovernedDocumentRequirementsService;
use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentUploadVersionResult {
    document_id: Uuid,
    version_id: Uuid,
    document_type_id: Option<Uuid>,
    blob_ref: Option<String>,
    cargo_ref: String,
    status: String,
}

fn parse_date_opt(value: Option<&str>, arg_name: &str) -> Result<Option<NaiveDate>> {
    value
        .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| anyhow!("Invalid {} argument; expected YYYY-MM-DD", arg_name))
}

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

pub struct UploadVersion;

#[async_trait]
impl SemOsVerbOp for UploadVersion {
    fn fqn(&self) -> &str {
        "document.upload-version"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let document_id = json_extract_uuid(args, ctx, "document-id")?;
        let content_type = json_extract_string(args, "content-type")?;
        let blob_ref = args
            .get("blob-ref")
            .and_then(|v| v.as_str())
            .map(String::from);
        let structured_data = args
            .get("structured-data")
            .cloned()
            .filter(|value| !value.is_null());
        let valid_from = parse_date_opt(
            args.get("valid-from").and_then(|v| v.as_str()),
            "valid-from",
        )?;
        let valid_to = parse_date_opt(args.get("valid-to").and_then(|v| v.as_str()), "valid-to")?;

        if blob_ref.is_none() && structured_data.is_none() {
            return Err(anyhow!("Either blob-ref or structured-data is required"));
        }

        let doc_row = sqlx::query(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".documents WHERE document_id = $1) as exists"#,
        )
        .bind(document_id)
        .fetch_one(scope.executor())
        .await?;
        let exists: bool = doc_row.get("exists");
        if !exists {
            return Err(anyhow!("Document {} not found", document_id));
        }

        let document_type_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT document_type_id FROM "ob-poc".document_catalog WHERE document_id = $1"#,
        )
        .bind(document_id)
        .fetch_optional(scope.executor())
        .await?;

        let version_row =
            sqlx::query(r#"SELECT "ob-poc".get_next_document_version($1) as version_no"#)
                .bind(document_id)
                .fetch_one(scope.executor())
                .await?;
        let version_no: i32 = version_row.try_get("version_no").unwrap_or(1);
        let version_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_versions
                (version_id, document_id, version_no, content_type,
                 structured_data, blob_ref, valid_from, valid_to)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(version_id)
        .bind(document_id)
        .bind(version_no)
        .bind(&content_type)
        .bind(&structured_data)
        .bind(&blob_ref)
        .bind(valid_from)
        .bind(valid_to)
        .execute(scope.executor())
        .await?;

        let result = DocumentUploadVersionResult {
            document_id,
            version_id,
            document_type_id,
            blob_ref,
            cargo_ref: format!("version://ob-poc/{}", version_id),
            status: "uploaded".to_string(),
        };
        ctx.bind("version", result.version_id);
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

pub struct Verify;

#[async_trait]
impl SemOsVerbOp for Verify {
    fn fqn(&self) -> &str {
        "document.verify"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let version_id = json_extract_uuid(args, ctx, "version-id")?;
        let verified_by = json_extract_string(args, "verified-by")?;

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".document_versions
            SET verification_status = 'verified',
                verified_by = $2,
                verified_at = now()
            WHERE version_id = $1
              AND verification_status IN ('pending', 'in_qa')
            "#,
        )
        .bind(version_id)
        .bind(&verified_by)
        .execute(scope.executor())
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Version not found or not in verifiable state"));
        }
        // Phase C.3 rollout: document version → verified.
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            version_id,
            "document-version:verified",
            "document/verification",
            &format!(
                "document.verify — version {} verified by {}",
                version_id, verified_by
            ),
        );
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct Reject;

#[async_trait]
impl SemOsVerbOp for Reject {
    fn fqn(&self) -> &str {
        "document.reject"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let version_id = json_extract_uuid(args, ctx, "version-id")?;
        let rejection_code = json_extract_string(args, "rejection-code")?;
        let verified_by = json_extract_string(args, "verified-by")?;
        let rejection_reason = args
            .get("rejection-reason")
            .and_then(|v| v.as_str())
            .map(String::from);

        let code_row = sqlx::query(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".rejection_reason_codes WHERE code = $1) as exists"#,
        )
        .bind(&rejection_code)
        .fetch_one(scope.executor())
        .await?;
        let code_exists: bool = code_row.get("exists");
        if !code_exists {
            return Err(anyhow!("Unknown rejection code: {}", rejection_code));
        }

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".document_versions
            SET verification_status = 'rejected',
                rejection_code = $2,
                rejection_reason = $3,
                verified_by = $4,
                verified_at = now()
            WHERE version_id = $1
              AND verification_status IN ('pending', 'in_qa')
            "#,
        )
        .bind(version_id)
        .bind(&rejection_code)
        .bind(&rejection_reason)
        .bind(&verified_by)
        .execute(scope.executor())
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Version not found or not in rejectable state"));
        }
        // Phase C.3 rollout: document version → rejected.
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            version_id,
            "document-version:rejected",
            "document/verification",
            &format!(
                "document.reject — version {} rejected ({}) by {}",
                version_id, rejection_code, verified_by
            ),
        );
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// ---------------------------------------------------------------------------
// missing-for-entity / compute-requirements
// ---------------------------------------------------------------------------

pub struct MissingForEntity;

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
