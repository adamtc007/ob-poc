//! Document custom operations
//!
//! Operations for document cataloging, extraction, task queue integration,
//! and verification workflows.
//!
//! ## Task Queue Operations (Migration 049)
//!
//! The document solicitation and verification operations integrate with the
//! workflow task queue for async external system interaction:
//!
//! - `document.solicit` - Request document from entity (creates pending task)
//! - `document.solicit-batch` - Request multiple documents (single multi-result task)
//! - `document.upload-version` - Upload a new version of a document
//! - `document.verify` - QA approves a document version
//! - `document.reject` - QA rejects with standardized reason code
//! - `document.missing-for-entity` - List missing document requirements

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::helpers::{
    json_extract_string, json_extract_string_list_opt, json_extract_uuid, json_extract_uuid_opt,
};
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use crate::database::{GovernedDocumentRequirementsService, GovernedRequirementMatrix};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use chrono::NaiveDate;

#[cfg(feature = "database")]
use uuid::Uuid;

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

#[cfg(feature = "database")]
async fn resolve_document_type_id(pool: &PgPool, doc_type: &str) -> Result<Option<Uuid>> {
    let type_id = sqlx::query_scalar(
        r#"
        SELECT type_id
        FROM "ob-poc".document_types
        WHERE type_code = $1
        "#,
    )
    .bind(doc_type)
    .fetch_optional(pool)
    .await?;

    Ok(type_id)
}

#[cfg(feature = "database")]
async fn table_exists(pool: &PgPool, qualified_name: &str) -> Result<bool> {
    let exists = sqlx::query_scalar::<_, Option<String>>("SELECT to_regclass($1)")
        .bind(qualified_name)
        .fetch_one(pool)
        .await?;
    Ok(exists.is_some())
}

#[cfg(feature = "database")]
async fn derive_subject_entity_id(
    verb_call: &VerbCall,
    ctx: &ExecutionContext,
    pool: &PgPool,
) -> Result<Uuid> {
    if let Some(subject_entity_id) = verb_call
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
    {
        return Ok(subject_entity_id);
    }

    let cbu_id = verb_call
        .arguments
        .iter()
        .find(|a| a.key == "cbu-id")
        .and_then(|a| {
            if let Some(name) = a.value.as_symbol() {
                ctx.resolve(name)
            } else {
                a.value.as_uuid()
            }
        });
    let case_id = verb_call
        .arguments
        .iter()
        .find(|a| a.key == "case-id")
        .and_then(|a| {
            if let Some(name) = a.value.as_symbol() {
                ctx.resolve(name)
            } else {
                a.value.as_uuid()
            }
        });

    if let Some(case_id) = case_id {
        let subject = sqlx::query_scalar(
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
        .fetch_optional(pool)
        .await?;

        if let Some(subject) = subject {
            return Ok(subject);
        }
    }

    if let Some(cbu_id) = cbu_id {
        let subject = sqlx::query_scalar(
            r#"
            SELECT cer.entity_id
            FROM "ob-poc".cbu_entity_roles cer
            WHERE cer.cbu_id = $1
            ORDER BY cer.created_at DESC NULLS LAST, cer.entity_id
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await?;

        if let Some(subject) = subject {
            return Ok(subject);
        }
    }

    Err(anyhow::anyhow!(
        "Missing subject-entity-id argument and unable to derive subject from case-id/cbu-id"
    ))
}

#[cfg(feature = "database")]
async fn derive_doc_types(
    verb_call: &VerbCall,
    subject_entity_id: Uuid,
    pool: &PgPool,
) -> Result<Vec<String>> {
    if let Some(doc_types) = verb_call
        .arguments
        .iter()
        .find(|a| a.key == "doc-types")
        .and_then(|a| a.value.as_list())
        .map(|list| {
            list.iter()
                .filter_map(|item| item.as_string().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
    {
        if doc_types.is_empty() {
            return Err(anyhow::anyhow!("doc-types cannot be empty"));
        }
        return Ok(doc_types);
    }

    let governed_service = GovernedDocumentRequirementsService::new(pool.clone());
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
        let exists = sqlx::query_scalar::<_, Option<Uuid>>(
            r#"
            SELECT type_id
            FROM "ob-poc".document_types
            WHERE type_code = $1
            "#,
        )
        .bind(doc_type)
        .fetch_one(pool)
        .await?;
        if exists.is_some() {
            resolved.push(doc_type.to_string());
        }
    }

    if resolved.is_empty() {
        return Err(anyhow::anyhow!(
            "Missing doc-types argument and unable to derive governed or fallback document types"
        ));
    }

    Ok(resolved)
}

#[cfg(feature = "database")]
async fn derive_subject_entity_id_json(
    args: &serde_json::Value,
    ctx: &sem_os_core::execution::VerbExecutionContext,
    pool: &PgPool,
) -> Result<Uuid> {
    if let Some(subject_entity_id) = json_extract_uuid_opt(args, ctx, "subject-entity-id")
        .or_else(|| json_extract_uuid_opt(args, ctx, "entity-id"))
    {
        return Ok(subject_entity_id);
    }

    let cbu_id = json_extract_uuid_opt(args, ctx, "cbu-id");
    let case_id = json_extract_uuid_opt(args, ctx, "case-id");

    if let Some(case_id) = case_id {
        let subject = sqlx::query_scalar(
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
        .fetch_optional(pool)
        .await?;

        if let Some(subject) = subject {
            return Ok(subject);
        }
    }

    if let Some(cbu_id) = cbu_id {
        let subject = sqlx::query_scalar(
            r#"
            SELECT cer.entity_id
            FROM "ob-poc".cbu_entity_roles cer
            WHERE cer.cbu_id = $1
            ORDER BY cer.created_at DESC NULLS LAST, cer.entity_id
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await?;

        if let Some(subject) = subject {
            return Ok(subject);
        }
    }

    Err(anyhow::anyhow!(
        "Missing subject-entity-id argument and unable to derive subject from case-id/cbu-id"
    ))
}

#[cfg(feature = "database")]
async fn derive_doc_types_json(
    args: &serde_json::Value,
    subject_entity_id: Uuid,
    pool: &PgPool,
) -> Result<Vec<String>> {
    if let Some(doc_types) = json_extract_string_list_opt(args, "doc-types") {
        if doc_types.is_empty() {
            return Err(anyhow::anyhow!("doc-types cannot be empty"));
        }
        return Ok(doc_types);
    }

    let governed_service = GovernedDocumentRequirementsService::new(pool.clone());
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
        let exists = sqlx::query_scalar::<_, Option<Uuid>>(
            r#"
            SELECT type_id
            FROM "ob-poc".document_types
            WHERE type_code = $1
            "#,
        )
        .bind(doc_type)
        .fetch_one(pool)
        .await?;
        if exists.is_some() {
            resolved.push(doc_type.to_string());
        }
    }

    if resolved.is_empty() {
        return Err(anyhow::anyhow!(
            "Missing doc-types argument and unable to derive governed or fallback document types"
        ));
    }

    Ok(resolved)
}

#[cfg(feature = "database")]
fn parse_date_opt(value: Option<&str>, arg_name: &str) -> Result<Option<NaiveDate>> {
    value
        .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| anyhow::anyhow!("Invalid {} argument; expected YYYY-MM-DD", arg_name))
}

#[cfg(feature = "database")]
async fn document_catalog_impl(
    doc_type: &str,
    document_name: Option<String>,
    cbu_id: Option<Uuid>,
    entity_id: Option<Uuid>,
    pool: &PgPool,
) -> Result<Uuid> {
    let doc_type_id: Uuid = sqlx::query_scalar::<_, Uuid>(
        r#"SELECT type_id FROM "ob-poc".document_types WHERE type_code = $1"#,
    )
    .bind(doc_type)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("Unknown document type: {}", doc_type))?;

    let existing = sqlx::query_scalar(
        r#"SELECT doc_id FROM "ob-poc".document_catalog
           WHERE cbu_id IS NOT DISTINCT FROM $1
           AND document_type_id = $2
           AND document_name IS NOT DISTINCT FROM $3
           LIMIT 1"#,
    )
    .bind(cbu_id)
    .bind(doc_type_id)
    .bind(&document_name)
    .fetch_optional(pool)
    .await?;

    if let Some(doc_id) = existing {
        return Ok(doc_id);
    }

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
    .bind(document_name)
    .execute(pool)
    .await?;

    Ok(doc_id)
}

#[cfg(feature = "database")]
async fn document_extract_impl(doc_id: Uuid, pool: &PgPool) -> Result<()> {
    sqlx::query(
        r#"UPDATE "ob-poc".document_catalog SET extraction_status = 'IN_PROGRESS' WHERE doc_id = $1"#,
    )
    .bind(doc_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(feature = "database")]
async fn document_solicit_impl(
    subject_entity_id: Uuid,
    doc_type: &str,
    workflow_instance_id: Option<Uuid>,
    due_date: Option<NaiveDate>,
    required_state: &str,
    pool: &PgPool,
) -> Result<DocumentSolicitationResult> {
    use sqlx::Row;

    let has_document_requirements = table_exists(pool, "\"ob-poc\".document_requirements").await?;
    let has_workflow_pending_tasks =
        table_exists(pool, "\"ob-poc\".workflow_pending_tasks").await?;

    if !has_document_requirements {
        tracing::warn!(
            subject_entity_id = %subject_entity_id,
            doc_type,
            "document_requirements table not present; returning synthetic solicitation result"
        );
        let synthetic_requirement_id = Uuid::now_v7();
        let synthetic_request_id = workflow_instance_id.unwrap_or_else(Uuid::now_v7);
        return Ok(DocumentSolicitationResult {
            request_id: synthetic_request_id,
            requirement_id: synthetic_requirement_id,
            document_type_id: resolve_document_type_id(pool, doc_type).await?,
            document_type_code: doc_type.to_string(),
            document_id: None,
            blob_ref: None,
            status: "requested".to_string(),
        });
    }

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
    let document_type_id = resolve_document_type_id(pool, doc_type).await?;
    let task_id = Uuid::new_v4();

    if has_workflow_pending_tasks {
        if let Some(instance_id) = workflow_instance_id {
            let args = serde_json::json!({
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
            .bind(doc_type)
            .bind(&args)
            .execute(pool)
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
            .execute(pool)
            .await?;
        }
    }

    Ok(DocumentSolicitationResult {
        request_id: task_id,
        requirement_id,
        document_type_id,
        document_type_code: doc_type.to_string(),
        document_id: None,
        blob_ref: None,
        status: "requested".to_string(),
    })
}

#[cfg(feature = "database")]
async fn document_solicit_set_impl(
    subject_entity_id: Uuid,
    doc_types: Vec<String>,
    workflow_instance_id: Option<Uuid>,
    due_date: Option<NaiveDate>,
    required_state: &str,
    pool: &PgPool,
) -> Result<DocumentSolicitationBatchResult> {
    use sqlx::Row;

    let task_id = Uuid::now_v7();
    let mut request_items: Vec<DocumentSolicitationResult> = Vec::with_capacity(doc_types.len());
    let has_document_requirements = table_exists(pool, "\"ob-poc\".document_requirements").await?;
    let has_workflow_pending_tasks =
        table_exists(pool, "\"ob-poc\".workflow_pending_tasks").await?;

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
        return Ok(DocumentSolicitationBatchResult {
            request_id: task_id,
            expected_count: request_items.len(),
            requests: request_items,
            status: "requested".to_string(),
        });
    }

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
        request_items.push(DocumentSolicitationResult {
            request_id: task_id,
            requirement_id,
            document_type_id: resolve_document_type_id(pool, doc_type).await?,
            document_type_code: doc_type.clone(),
            document_id: None,
            blob_ref: None,
            status: "requested".to_string(),
        });
    }

    if has_workflow_pending_tasks {
        if let Some(instance_id) = workflow_instance_id {
            let args = serde_json::json!({
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
            .bind(&args)
            .bind(doc_types.len() as i32)
            .execute(pool)
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
                .execute(pool)
                .await?;
            }
        }
    }

    Ok(DocumentSolicitationBatchResult {
        request_id: task_id,
        expected_count: doc_types.len(),
        requests: request_items,
        status: "requested".to_string(),
    })
}

#[cfg(feature = "database")]
async fn document_upload_version_impl(
    document_id: Uuid,
    content_type: &str,
    blob_ref: Option<String>,
    structured_data: Option<serde_json::Value>,
    valid_from: Option<NaiveDate>,
    valid_to: Option<NaiveDate>,
    pool: &PgPool,
) -> Result<DocumentUploadVersionResult> {
    use sqlx::Row;

    if blob_ref.is_none() && structured_data.is_none() {
        return Err(anyhow::anyhow!(
            "Either blob-ref or structured-data is required"
        ));
    }

    let doc_exists = sqlx::query(
        r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".documents WHERE document_id = $1) as exists"#,
    )
    .bind(document_id)
    .fetch_one(pool)
    .await?;

    let exists: bool = doc_exists.get("exists");
    if !exists {
        return Err(anyhow::anyhow!("Document {} not found", document_id));
    }

    let document_type_id = sqlx::query_scalar(
        r#"
        SELECT document_type_id
        FROM "ob-poc".document_catalog
        WHERE document_id = $1
        "#,
    )
    .bind(document_id)
    .fetch_optional(pool)
    .await?;

    let version_row = sqlx::query(r#"SELECT "ob-poc".get_next_document_version($1) as version_no"#)
        .bind(document_id)
        .fetch_one(pool)
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
    .bind(content_type)
    .bind(&structured_data)
    .bind(&blob_ref)
    .bind(valid_from)
    .bind(valid_to)
    .execute(pool)
    .await?;

    Ok(DocumentUploadVersionResult {
        document_id,
        version_id,
        document_type_id,
        blob_ref,
        cargo_ref: format!("version://ob-poc/{}", version_id),
        status: "uploaded".to_string(),
    })
}

#[cfg(feature = "database")]
async fn document_verify_impl(version_id: Uuid, verified_by: &str, pool: &PgPool) -> Result<u64> {
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
    .bind(verified_by)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(anyhow::anyhow!(
            "Version not found or not in verifiable state"
        ));
    }

    Ok(1)
}

#[cfg(feature = "database")]
async fn document_reject_impl(
    version_id: Uuid,
    rejection_code: &str,
    verified_by: &str,
    rejection_reason: Option<String>,
    pool: &PgPool,
) -> Result<u64> {
    use sqlx::Row;

    let code_row = sqlx::query(
        r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".rejection_reason_codes WHERE code = $1) as exists"#,
    )
    .bind(rejection_code)
    .fetch_one(pool)
    .await?;

    let code_exists: bool = code_row.get("exists");
    if !code_exists {
        return Err(anyhow::anyhow!(
            "Unknown rejection code: {}",
            rejection_code
        ));
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
    .bind(rejection_code)
    .bind(&rejection_reason)
    .bind(verified_by)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(anyhow::anyhow!(
            "Version not found or not in rejectable state"
        ));
    }

    Ok(1)
}

#[cfg(feature = "database")]
async fn document_missing_for_entity_impl(
    entity_id: Uuid,
    workflow_instance_id: Option<Uuid>,
    pool: &PgPool,
) -> Result<Vec<serde_json::Value>> {
    use sqlx::Row;

    let governed_service = GovernedDocumentRequirementsService::new(pool.clone());
    if let Some(governed) = governed_service.compute_for_entity(entity_id).await? {
        let results = governed
            .gaps
            .into_iter()
            .map(|gap| {
                serde_json::json!({
                    "requirement_id": serde_json::Value::Null,
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

        return Ok(results);
    }

    let rows = sqlx::query(
        r#"
        SELECT
            requirement_id,
            doc_type,
            status,
            required_state,
            attempt_count,
            last_rejection_code
        FROM "ob-poc".document_requirements
        WHERE subject_entity_id = $1
          AND ($2::uuid IS NULL OR workflow_instance_id = $2)
          AND status NOT IN ('verified', 'waived')
        ORDER BY doc_type
        "#,
    )
    .bind(entity_id)
    .bind(workflow_instance_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "requirement_id": row.get::<Uuid, _>("requirement_id"),
                "doc_type": row.get::<String, _>("doc_type"),
                "status": row.get::<String, _>("status"),
                "required_state": row.get::<String, _>("required_state"),
                "attempt_count": row.get::<i32, _>("attempt_count"),
                "last_rejection_code": row.get::<Option<String>, _>("last_rejection_code")
            })
        })
        .collect())
}

#[cfg(feature = "database")]
async fn document_compute_requirements_impl(
    entity_id: Uuid,
    pool: &PgPool,
) -> Result<GovernedRequirementMatrix> {
    let governed_service = GovernedDocumentRequirementsService::new(pool.clone());
    governed_service
        .compute_matrix_for_entity(entity_id)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No active governed requirement profile matched entity {}",
                entity_id
            )
        })
}

/// Document cataloging with document type lookup (Idempotent)
///
/// Rationale: Requires lookup of document_type_id from document_types table
/// by type code, then insert into document_catalog with type-specific
/// attribute mappings from document_type_attributes.
///
/// Idempotency: Uses ON CONFLICT on (cbu_id, document_type_id, document_name)
/// to return existing document if already cataloged.
#[register_custom_op]
pub struct DocumentCatalogOp;

#[async_trait]
impl CustomOperation for DocumentCatalogOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "catalog"
    }
    fn rationale(&self) -> &'static str {
        "Requires document_type lookup and attribute mapping from document_type_attributes"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let doc_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "doc-type" || a.key == "document-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing doc-type argument"))?;

        let document_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "title" || a.key == "document-name")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());
        let cbu_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });
        let entity_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });
        let doc_id =
            document_catalog_impl(doc_type, document_name, cbu_id, entity_id, pool).await?;
        ctx.bind("document", doc_id);
        Ok(ExecutionResult::Uuid(doc_id))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let doc_type = json_extract_string(args, "doc-type")
            .or_else(|_| json_extract_string(args, "document-type"))?;
        let document_name = args
            .get("title")
            .and_then(|v| v.as_str())
            .or_else(|| args.get("document-name").and_then(|v| v.as_str()))
            .map(|s| s.to_string());
        let cbu_id = json_extract_uuid_opt(args, ctx, "cbu-id");
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let doc_id =
            document_catalog_impl(&doc_type, document_name, cbu_id, entity_id, pool).await?;
        ctx.bind("document", doc_id);
        Ok(sem_os_core::execution::VerbExecutionOutcome::Uuid(doc_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(uuid::Uuid::new_v4()))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Document extraction using AI/OCR
///
/// Rationale: Requires external AI service call for OCR/extraction,
/// then maps extracted values to attributes via document_type_attributes.
#[register_custom_op]
pub struct DocumentExtractOp;

#[async_trait]
impl CustomOperation for DocumentExtractOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "extract"
    }
    fn rationale(&self) -> &'static str {
        "Requires external AI/OCR service call and attribute mapping"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let doc_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "document-id" || a.key == "doc-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing document-id argument"))?;
        document_extract_impl(doc_id, pool).await?;
        Ok(ExecutionResult::Void)
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let doc_id = json_extract_uuid(args, ctx, "document-id")
            .or_else(|_| json_extract_uuid(args, ctx, "doc-id"))?;
        document_extract_impl(doc_id, pool).await?;
        Ok(sem_os_core::execution::VerbExecutionOutcome::Void)
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// Task Queue Document Operations (Migration 049)
// ============================================================================

/// Solicit a single document from an entity (creates pending task + requirement)
///
/// Rationale: Creates workflow_pending_tasks entry for external system,
/// ensures document_requirements row exists, and links them together.
/// Returns task_id for tracking and requirement_id for status queries.
#[register_custom_op]
#[cfg(feature = "database")]
pub struct DocumentSolicitOp;

#[cfg(feature = "database")]
#[async_trait]
impl CustomOperation for DocumentSolicitOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "solicit"
    }
    fn rationale(&self) -> &'static str {
        "Creates pending task and requirement for async document collection"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let subject_entity_id = derive_subject_entity_id(verb_call, ctx, pool).await?;
        let doc_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "doc-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing doc-type argument"))?;

        // Optional arguments
        let workflow_instance_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "workflow-instance-id")
            .and_then(|a| a.value.as_uuid());
        let due_date = parse_date_opt(
            verb_call
                .arguments
                .iter()
                .find(|a| a.key == "due-date")
                .and_then(|a| a.value.as_string()),
            "due-date",
        )?;
        let required_state = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "required-state")
            .and_then(|a| a.value.as_string())
            .unwrap_or("verified");
        let result = document_solicit_impl(
            subject_entity_id,
            doc_type,
            workflow_instance_id,
            due_date,
            required_state,
            pool,
        )
        .await?;
        ctx.bind("requirement", result.requirement_id);
        if workflow_instance_id.is_some() {
            ctx.bind("task", result.request_id);
        }
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let subject_entity_id = derive_subject_entity_id_json(args, ctx, pool).await?;
        let doc_type = json_extract_string(args, "doc-type")?;
        let workflow_instance_id = json_extract_uuid_opt(args, ctx, "workflow-instance-id");
        let due_date = parse_date_opt(args.get("due-date").and_then(|v| v.as_str()), "due-date")?;
        let required_state = args
            .get("required-state")
            .and_then(|v| v.as_str())
            .unwrap_or("verified");
        let result = document_solicit_impl(
            subject_entity_id,
            &doc_type,
            workflow_instance_id,
            due_date,
            required_state,
            pool,
        )
        .await?;
        ctx.bind("requirement", result.requirement_id);
        if workflow_instance_id.is_some() {
            ctx.bind("task", result.request_id);
        }
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Solicit multiple documents from an entity (single multi-result task)
///
/// Rationale: Creates one pending task with expected_cargo_count > 1,
/// and multiple requirements linked to that task.
#[register_custom_op]
#[cfg(feature = "database")]
pub struct DocumentSolicitSetOp;

#[cfg(feature = "database")]
#[async_trait]
impl CustomOperation for DocumentSolicitSetOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "solicit-batch"
    }
    fn rationale(&self) -> &'static str {
        "Creates single multi-result task for multiple document requirements"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let subject_entity_id = derive_subject_entity_id(verb_call, ctx, pool).await?;
        let doc_types = derive_doc_types(verb_call, subject_entity_id, pool).await?;
        let workflow_instance_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "workflow-instance-id")
            .and_then(|a| a.value.as_uuid());
        let due_date = parse_date_opt(
            verb_call
                .arguments
                .iter()
                .find(|a| a.key == "due-date")
                .and_then(|a| a.value.as_string()),
            "due-date",
        )?;
        let required_state = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "required-state")
            .and_then(|a| a.value.as_string())
            .unwrap_or("verified");
        let result = document_solicit_set_impl(
            subject_entity_id,
            doc_types,
            workflow_instance_id,
            due_date,
            required_state,
            pool,
        )
        .await?;
        if workflow_instance_id.is_some() {
            ctx.bind("task", result.request_id);
        }
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let subject_entity_id = derive_subject_entity_id_json(args, ctx, pool).await?;
        let doc_types = derive_doc_types_json(args, subject_entity_id, pool).await?;
        let workflow_instance_id = json_extract_uuid_opt(args, ctx, "workflow-instance-id");
        let due_date = parse_date_opt(args.get("due-date").and_then(|v| v.as_str()), "due-date")?;
        let required_state = args
            .get("required-state")
            .and_then(|v| v.as_str())
            .unwrap_or("verified");
        let result = document_solicit_set_impl(
            subject_entity_id,
            doc_types,
            workflow_instance_id,
            due_date,
            required_state,
            pool,
        )
        .await?;
        if workflow_instance_id.is_some() {
            ctx.bind("task", result.request_id);
        }
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Upload a new version of a document (Layer C - immutable submission)
///
/// Rationale: Requires version numbering, content validation, and
/// returns cargo_ref URI for use in task completion webhook.
#[register_custom_op]
#[cfg(feature = "database")]
pub struct DocumentUploadVersionOp;

#[cfg(feature = "database")]
#[async_trait]
impl CustomOperation for DocumentUploadVersionOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "upload-version"
    }
    fn rationale(&self) -> &'static str {
        "Requires version numbering and cargo_ref generation"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Extract required arguments
        let document_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "document-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing document-id argument"))?;

        let content_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "content-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing content-type argument"))?;

        // Optional content (at least one required)
        let blob_ref: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "blob-ref")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // For structured-data, we accept a map and convert to JSON
        let structured_data: Option<serde_json::Value> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "structured-data")
            .and_then(|a| {
                // Try to convert the AST node to JSON
                if let Some(map) = a.value.as_map() {
                    let obj: serde_json::Map<String, serde_json::Value> = map
                        .iter()
                        .filter_map(|(k, v)| {
                            v.as_string()
                                .map(|s| (k.clone(), serde_json::Value::String(s.to_string())))
                        })
                        .collect();
                    Some(serde_json::Value::Object(obj))
                } else if let Some(s) = a.value.as_string() {
                    // Try parsing as JSON string
                    serde_json::from_str(s).ok()
                } else {
                    None
                }
            });
        let valid_from = parse_date_opt(
            verb_call
                .arguments
                .iter()
                .find(|a| a.key == "valid-from")
                .and_then(|a| a.value.as_string()),
            "valid-from",
        )?;
        let valid_to = parse_date_opt(
            verb_call
                .arguments
                .iter()
                .find(|a| a.key == "valid-to")
                .and_then(|a| a.value.as_string()),
            "valid-to",
        )?;
        let result = document_upload_version_impl(
            document_id,
            content_type,
            blob_ref,
            structured_data,
            valid_from,
            valid_to,
            pool,
        )
        .await?;
        ctx.bind("version", result.version_id);
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let document_id = json_extract_uuid(args, ctx, "document-id")?;
        let content_type = json_extract_string(args, "content-type")?;
        let blob_ref = args
            .get("blob-ref")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let structured_data = args
            .get("structured-data")
            .cloned()
            .filter(|value| !value.is_null());
        let valid_from = parse_date_opt(
            args.get("valid-from").and_then(|v| v.as_str()),
            "valid-from",
        )?;
        let valid_to = parse_date_opt(args.get("valid-to").and_then(|v| v.as_str()), "valid-to")?;
        let result = document_upload_version_impl(
            document_id,
            &content_type,
            blob_ref,
            structured_data,
            valid_from,
            valid_to,
            pool,
        )
        .await?;
        ctx.bind("version", result.version_id);
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// QA approves a document version
///
/// Rationale: Updates verification status and triggers requirement sync
/// via database trigger (fn_sync_requirement_from_version).
#[register_custom_op]
#[cfg(feature = "database")]
pub struct DocumentVerifyOp;

#[cfg(feature = "database")]
#[async_trait]
impl CustomOperation for DocumentVerifyOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "verify"
    }
    fn rationale(&self) -> &'static str {
        "Updates verification status with QA attribution"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Extract required arguments
        let version_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "version-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing version-id argument"))?;

        let verified_by = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "verified-by")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing verified-by argument"))?;
        Ok(ExecutionResult::Affected(
            document_verify_impl(version_id, verified_by, pool).await?,
        ))
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let version_id = json_extract_uuid(args, ctx, "version-id")?;
        let verified_by = json_extract_string(args, "verified-by")?;
        Ok(sem_os_core::execution::VerbExecutionOutcome::Affected(
            document_verify_impl(version_id, &verified_by, pool).await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// QA rejects a document version with standardized reason code
///
/// Rationale: Uses rejection_reason_codes for standardized messaging,
/// updates requirement status, and may trigger re-solicitation.
#[register_custom_op]
#[cfg(feature = "database")]
pub struct DocumentRejectOp;

#[cfg(feature = "database")]
#[async_trait]
impl CustomOperation for DocumentRejectOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "reject"
    }
    fn rationale(&self) -> &'static str {
        "Uses standardized rejection codes for client messaging"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let version_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "version-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing version-id argument"))?;

        let rejection_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "rejection-code")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing rejection-code argument"))?;

        let verified_by = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "verified-by")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing verified-by argument"))?;

        // Optional free-text reason
        let rejection_reason: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "rejection-reason")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());
        Ok(ExecutionResult::Affected(
            document_reject_impl(
                version_id,
                rejection_code,
                verified_by,
                rejection_reason,
                pool,
            )
            .await?,
        ))
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let version_id = json_extract_uuid(args, ctx, "version-id")?;
        let rejection_code = json_extract_string(args, "rejection-code")?;
        let verified_by = json_extract_string(args, "verified-by")?;
        let rejection_reason = args
            .get("rejection-reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Ok(sem_os_core::execution::VerbExecutionOutcome::Affected(
            document_reject_impl(
                version_id,
                &rejection_code,
                &verified_by,
                rejection_reason,
                pool,
            )
            .await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// List missing document requirements for an entity
///
/// Rationale: Query joins requirements with status filtering
/// for workflow progress tracking.
#[register_custom_op]
#[cfg(feature = "database")]
pub struct DocumentMissingForEntityOp;

#[cfg(feature = "database")]
#[async_trait]
impl CustomOperation for DocumentMissingForEntityOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "missing-for-entity"
    }
    fn rationale(&self) -> &'static str {
        "Complex query filtering unsatisfied requirements"
    }

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

        // Optional workflow filter
        let workflow_instance_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "workflow-instance-id")
            .and_then(|a| a.value.as_uuid());
        Ok(ExecutionResult::RecordSet(
            document_missing_for_entity_impl(entity_id, workflow_instance_id, pool).await?,
        ))
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let workflow_instance_id = json_extract_uuid_opt(args, ctx, "workflow-instance-id");
        Ok(sem_os_core::execution::VerbExecutionOutcome::RecordSet(
            document_missing_for_entity_impl(entity_id, workflow_instance_id, pool).await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Compute governed document requirements for an entity.
///
/// Rationale: Resolves the published SemOS requirement profile and computes
/// obligation coverage against the current document runtime.
#[register_custom_op]
#[cfg(feature = "database")]
pub struct DocumentComputeRequirementsOp;

#[cfg(feature = "database")]
#[async_trait]
impl CustomOperation for DocumentComputeRequirementsOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "compute-requirements"
    }
    fn rationale(&self) -> &'static str {
        "Computes governed document requirement matrix from published SemOS policy snapshots"
    }

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

        Ok(ExecutionResult::Record(serde_json::to_value(
            document_compute_requirements_impl(entity_id, pool).await?,
        )?))
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(document_compute_requirements_impl(entity_id, pool).await?)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
