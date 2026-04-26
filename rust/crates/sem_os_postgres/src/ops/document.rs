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
use std::collections::HashSet;
use uuid::Uuid;

use dsl_runtime::document_requirements::GovernedDocumentRequirementsService;
use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_list_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentSolicitationResult {
    request_id: Uuid,
    requirement_id: Uuid,
    document_type_id: Option<Uuid>,
    document_type_code: String,
    document_id: Option<Uuid>,
    blob_ref: Option<String>,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentSolicitationBatchResult {
    request_id: Uuid,
    expected_count: usize,
    requests: Vec<DocumentSolicitationResult>,
    status: String,
}

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

async fn resolve_document_type_id(
    scope: &mut dyn TransactionScope,
    doc_type: &str,
) -> Result<Option<Uuid>> {
    Ok(
        sqlx::query_scalar(r#"SELECT type_id FROM "ob-poc".document_types WHERE type_code = $1"#)
            .bind(doc_type)
            .fetch_optional(scope.executor())
            .await?,
    )
}

async fn table_exists(scope: &mut dyn TransactionScope, qualified_name: &str) -> Result<bool> {
    let exists: Option<String> = sqlx::query_scalar("SELECT to_regclass($1)::text")
        .bind(qualified_name)
        .fetch_one(scope.executor())
        .await?;
    Ok(exists.is_some())
}

async fn derive_subject_entity_id(
    args: &Value,
    ctx: &VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    if let Some(subject_entity_id) = json_extract_uuid_opt(args, ctx, "subject-entity-id")
        .or_else(|| json_extract_uuid_opt(args, ctx, "entity-id"))
    {
        return Ok(subject_entity_id);
    }

    let cbu_id = json_extract_uuid_opt(args, ctx, "cbu-id");
    let case_id = json_extract_uuid_opt(args, ctx, "case-id");

    if let Some(case_id) = case_id {
        let subject: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT cer.entity_id
            FROM "ob-poc".cases c
            JOIN "ob-poc".cbu_entity_roles cer ON cer.cbu_id = c.cbu_id
            WHERE c.case_id = $1
            ORDER BY cer.created_at DESC NULLS LAST, cer.entity_id
            LIMIT 1
            "#,
        )
        .bind(case_id)
        .fetch_optional(scope.executor())
        .await?;
        if let Some(subject) = subject {
            return Ok(subject);
        }
    }

    if let Some(cbu_id) = cbu_id {
        let subject: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT cer.entity_id
            FROM "ob-poc".cbu_entity_roles cer
            WHERE cer.cbu_id = $1
            ORDER BY cer.created_at DESC NULLS LAST, cer.entity_id
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(scope.executor())
        .await?;
        if let Some(subject) = subject {
            return Ok(subject);
        }
    }

    Err(anyhow!(
        "Missing subject-entity-id argument and unable to derive subject from case-id/cbu-id"
    ))
}

async fn derive_doc_types(
    args: &Value,
    subject_entity_id: Uuid,
    scope: &mut dyn TransactionScope,
) -> Result<Vec<String>> {
    if let Some(doc_types) = json_extract_string_list_opt(args, "doc-types") {
        if doc_types.is_empty() {
            return Err(anyhow!("doc-types cannot be empty"));
        }
        return Ok(doc_types);
    }

    let governed_service = GovernedDocumentRequirementsService::new(scope.pool().clone());
    match governed_service.compute_for_entity(subject_entity_id).await {
        Ok(Some(governed)) => {
            let mut seen = HashSet::new();
            let mut doc_types = Vec::new();
            for gap in governed.gaps {
                let candidate = gap
                    .document_type_fqn
                    .rsplit('.')
                    .next()
                    .unwrap_or(&gap.document_type_fqn)
                    .replace('-', "_")
                    .to_ascii_uppercase();
                if seen.insert(candidate.clone()) {
                    doc_types.push(candidate);
                }
            }
            if !doc_types.is_empty() {
                return Ok(doc_types);
            }
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(
                subject_entity_id = %subject_entity_id,
                error = %error,
                "Failed to compute governed document requirements; using fallback doc types"
            );
        }
    }

    let fallback_doc_types = ["ARTICLES_OF_INCORPORATION", "BANK_STATEMENT"];
    let mut resolved = Vec::new();
    for doc_type in fallback_doc_types {
        let exists: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT type_id FROM "ob-poc".document_types WHERE type_code = $1"#,
        )
        .bind(doc_type)
        .fetch_one(scope.executor())
        .await?;
        if exists.is_some() {
            resolved.push(doc_type.to_string());
        }
    }
    if resolved.is_empty() {
        return Err(anyhow!(
            "Missing doc-types argument and unable to derive governed or fallback document types"
        ));
    }
    Ok(resolved)
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
// solicit / solicit-batch
// ---------------------------------------------------------------------------

pub struct Solicit;

#[async_trait]
impl SemOsVerbOp for Solicit {
    fn fqn(&self) -> &str {
        "document.solicit"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject_entity_id = derive_subject_entity_id(args, ctx, scope).await?;
        let doc_type = json_extract_string(args, "doc-type")?;
        let workflow_instance_id = json_extract_uuid_opt(args, ctx, "workflow-instance-id");
        let due_date = parse_date_opt(args.get("due-date").and_then(|v| v.as_str()), "due-date")?;
        let required_state = args
            .get("required-state")
            .and_then(|v| v.as_str())
            .unwrap_or("verified");

        let has_document_requirements =
            table_exists(scope, "\"ob-poc\".document_requirements").await?;
        let has_workflow_pending_tasks =
            table_exists(scope, "\"ob-poc\".workflow_pending_tasks").await?;

        if !has_document_requirements {
            tracing::warn!(
                subject_entity_id = %subject_entity_id,
                doc_type = %doc_type,
                "document_requirements table not present; returning synthetic solicitation result"
            );
            let synthetic_requirement_id = Uuid::now_v7();
            let synthetic_request_id = workflow_instance_id.unwrap_or_else(Uuid::now_v7);
            let document_type_id = resolve_document_type_id(scope, &doc_type).await?;
            let result = DocumentSolicitationResult {
                request_id: synthetic_request_id,
                requirement_id: synthetic_requirement_id,
                document_type_id,
                document_type_code: doc_type.clone(),
                document_id: None,
                blob_ref: None,
                status: "requested".to_string(),
            };
            ctx.bind("requirement", result.requirement_id);
            return Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?));
        }

        let row = sqlx::query(
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
        .bind(&doc_type)
        .bind(required_state)
        .bind(due_date)
        .fetch_one(scope.executor())
        .await?;
        let requirement_id: Uuid = row.get("requirement_id");
        let document_type_id = resolve_document_type_id(scope, &doc_type).await?;
        let task_id = Uuid::new_v4();

        if has_workflow_pending_tasks {
            if let Some(instance_id) = workflow_instance_id {
                let task_args = json!({
                    "subject_entity_id": subject_entity_id,
                    "doc_type": doc_type,
                    "requirement_id": requirement_id
                });
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".workflow_pending_tasks
                        (task_id, instance_id, blocker_type, blocker_key, verb, args,
                         expected_cargo_count, status)
                    VALUES ($1, $2, 'document', $3, 'document.solicit', $4, 1, 'pending')
                    "#,
                )
                .bind(task_id)
                .bind(instance_id)
                .bind(&doc_type)
                .bind(&task_args)
                .execute(scope.executor())
                .await?;

                sqlx::query(
                    r#"
                    UPDATE "ob-poc".document_requirements
                    SET current_task_id = $2, status = 'requested', updated_at = now()
                    WHERE requirement_id = $1
                    "#,
                )
                .bind(requirement_id)
                .bind(task_id)
                .execute(scope.executor())
                .await?;
            }
        }

        let result = DocumentSolicitationResult {
            request_id: task_id,
            requirement_id,
            document_type_id,
            document_type_code: doc_type.clone(),
            document_id: None,
            blob_ref: None,
            status: "requested".to_string(),
        };
        ctx.bind("requirement", result.requirement_id);
        if workflow_instance_id.is_some() {
            ctx.bind("task", result.request_id);
        }
        // Phase C.3 rollout: new document requirement in REQUESTED state.
        // ON CONFLICT DO UPDATE means this can be an idempotent touch
        // (same requirement already exists), but the status transition
        // to 'requested' is genuine on either path — the emit captures
        // the "solicitation issued" event rather than "new row".
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            requirement_id,
            "requirement:requested",
            "document/requirement",
            &format!(
                "document.solicit — {} for entity {}",
                doc_type, subject_entity_id
            ),
        );
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

pub struct SolicitBatch;

#[async_trait]
impl SemOsVerbOp for SolicitBatch {
    fn fqn(&self) -> &str {
        "document.solicit-batch"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject_entity_id = derive_subject_entity_id(args, ctx, scope).await?;
        let doc_types = derive_doc_types(args, subject_entity_id, scope).await?;
        let workflow_instance_id = json_extract_uuid_opt(args, ctx, "workflow-instance-id");
        let due_date = parse_date_opt(args.get("due-date").and_then(|v| v.as_str()), "due-date")?;
        let required_state = args
            .get("required-state")
            .and_then(|v| v.as_str())
            .unwrap_or("verified");

        let task_id = Uuid::now_v7();
        let mut request_items: Vec<DocumentSolicitationResult> =
            Vec::with_capacity(doc_types.len());
        let has_document_requirements =
            table_exists(scope, "\"ob-poc\".document_requirements").await?;
        let has_workflow_pending_tasks =
            table_exists(scope, "\"ob-poc\".workflow_pending_tasks").await?;

        if !has_document_requirements {
            tracing::warn!(
                subject_entity_id = %subject_entity_id,
                doc_types = ?doc_types,
                "document_requirements table not present; returning synthetic solicitation batch result"
            );
            let request_items = doc_types
                .iter()
                .map(|doc_type| DocumentSolicitationResult {
                    request_id: task_id,
                    requirement_id: Uuid::now_v7(),
                    document_type_id: None,
                    document_type_code: doc_type.clone(),
                    document_id: None,
                    blob_ref: None,
                    status: "requested".to_string(),
                })
                .collect::<Vec<_>>();
            let result = DocumentSolicitationBatchResult {
                request_id: task_id,
                expected_count: request_items.len(),
                requests: request_items,
                status: "requested".to_string(),
            };
            if workflow_instance_id.is_some() {
                ctx.bind("task", result.request_id);
            }
            return Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?));
        }

        for doc_type in &doc_types {
            let row = sqlx::query(
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
            .fetch_one(scope.executor())
            .await?;
            let requirement_id: Uuid = row.get("requirement_id");
            let document_type_id = resolve_document_type_id(scope, doc_type).await?;
            request_items.push(DocumentSolicitationResult {
                request_id: task_id,
                requirement_id,
                document_type_id,
                document_type_code: doc_type.clone(),
                document_id: None,
                blob_ref: None,
                status: "requested".to_string(),
            });
        }

        if has_workflow_pending_tasks {
            if let Some(instance_id) = workflow_instance_id {
                let task_args = json!({
                    "subject_entity_id": subject_entity_id,
                    "doc_types": doc_types,
                    "requirement_ids": request_items
                        .iter()
                        .map(|item| item.requirement_id)
                        .collect::<Vec<_>>()
                });
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".workflow_pending_tasks
                        (task_id, instance_id, blocker_type, blocker_key, verb, args,
                         expected_cargo_count, status)
                    VALUES ($1, $2, 'document_set', $3, 'document.solicit-batch', $4, $5, 'pending')
                    "#,
                )
                .bind(task_id)
                .bind(instance_id)
                .bind(doc_types.join(","))
                .bind(&task_args)
                .bind(doc_types.len() as i32)
                .execute(scope.executor())
                .await?;

                for item in &request_items {
                    sqlx::query(
                        r#"
                        UPDATE "ob-poc".document_requirements
                        SET current_task_id = $2, status = 'requested', updated_at = now()
                        WHERE requirement_id = $1
                        "#,
                    )
                    .bind(item.requirement_id)
                    .bind(task_id)
                    .execute(scope.executor())
                    .await?;
                }
            }
        }

        let result = DocumentSolicitationBatchResult {
            request_id: task_id,
            expected_count: doc_types.len(),
            requests: request_items,
            status: "requested".to_string(),
        };
        if workflow_instance_id.is_some() {
            ctx.bind("task", result.request_id);
        }
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
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
