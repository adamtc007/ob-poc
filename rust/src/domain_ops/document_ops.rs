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
//! - `document.solicit-set` - Request multiple documents (single multi-result task)
//! - `document.upload-version` - Upload a new version of a document
//! - `document.verify` - QA approves a document version
//! - `document.reject` - QA rejects with standardized reason code
//! - `document.missing-for-entity` - List missing document requirements

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use chrono::NaiveDate;

#[cfg(feature = "database")]
use uuid::Uuid;

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
        use uuid::Uuid;

        // Extract arguments
        let doc_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "doc-type" || a.key == "document-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing doc-type argument"))?;

        // Look up document type ID
        let type_row = sqlx::query!(
            r#"SELECT type_id FROM "ob-poc".document_types WHERE type_code = $1"#,
            doc_type
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Unknown document type: {}", doc_type))?;

        let doc_type_id = type_row.type_id;

        // Get optional arguments
        let document_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "title" || a.key == "document-name")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Get CBU ID if provided (resolve reference if needed)
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

        // Get Entity ID if provided (resolve reference if needed)
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

        // Idempotent: Check for existing document with same cbu_id, document_type_id, and document_name
        let existing = sqlx::query!(
            r#"SELECT doc_id FROM "ob-poc".document_catalog
               WHERE cbu_id IS NOT DISTINCT FROM $1
               AND document_type_id = $2
               AND document_name IS NOT DISTINCT FROM $3
               LIMIT 1"#,
            cbu_id,
            doc_type_id,
            document_name
        )
        .fetch_optional(pool)
        .await?;

        if let Some(row) = existing {
            ctx.bind("document", row.doc_id);
            return Ok(ExecutionResult::Uuid(row.doc_id));
        }

        // Create new document
        let doc_id = Uuid::now_v7();

        sqlx::query!(
            r#"INSERT INTO "ob-poc".document_catalog
               (doc_id, document_type_id, cbu_id, entity_id, document_name, status)
               VALUES ($1, $2, $3, $4, $5, 'active')"#,
            doc_id,
            doc_type_id,
            cbu_id,
            entity_id,
            document_name
        )
        .execute(pool)
        .await?;

        // Bind to context for reference
        ctx.bind("document", doc_id);

        Ok(ExecutionResult::Uuid(doc_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(uuid::Uuid::now_v7()))
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
        use uuid::Uuid;

        // Get document ID (doc_id is the PK in actual schema)
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

        // Update extraction status
        sqlx::query!(
            r#"UPDATE "ob-poc".document_catalog SET extraction_status = 'IN_PROGRESS' WHERE doc_id = $1"#,
            doc_id
        )
        .execute(pool)
        .await?;

        // TODO: Call external extraction service
        // For now, just mark as pending extraction

        // In a real implementation, this would:
        // 1. Fetch the document file
        // 2. Call AI/OCR service
        // 3. Map extracted fields to attributes via document_type_attributes
        // 4. Store extracted values in attribute_values_typed
        // 5. Update extraction_status to 'completed'

        Ok(ExecutionResult::Void)
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
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

        // 1. Ensure requirement exists (upsert)
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

        // 2. Create pending task (need workflow_instance_id for FK)
        let task_id = Uuid::now_v7();

        // If no workflow_instance_id provided, we need to handle this
        // For now, require workflow_instance_id for task creation
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

            // 3. Link requirement to task
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

            ctx.bind("task", task_id);
        }

        ctx.bind("requirement", requirement_id);

        // Return record with both IDs
        let result = serde_json::json!({
            "task_id": task_id,
            "requirement_id": requirement_id
        });

        Ok(ExecutionResult::Record(result))
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
        "solicit-set"
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

        let task_id = Uuid::now_v7();
        let mut requirement_ids: Vec<Uuid> = Vec::with_capacity(doc_types.len());

        // 1. Create requirements for each doc type
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

        // 2. Create single pending task with expected_cargo_count = doc_types.len()
        if let Some(instance_id) = workflow_instance_id {
            let args = serde_json::json!({
                "subject_entity_id": subject_entity_id,
                "doc_types": doc_types,
                "requirement_ids": requirement_ids
            });

            sqlx::query(
                r#"
                INSERT INTO "ob-poc".workflow_pending_tasks
                    (task_id, instance_id, blocker_type, blocker_key, verb, args,
                     expected_cargo_count, status)
                VALUES ($1, $2, 'document_set', $3, 'document.solicit-set', $4, $5, 'pending')
                "#,
            )
            .bind(task_id)
            .bind(instance_id)
            .bind(doc_types.join(","))
            .bind(&args)
            .bind(doc_types.len() as i32)
            .execute(pool)
            .await?;

            // 3. Link all requirements to the task and update status
            for req_id in &requirement_ids {
                sqlx::query(
                    r#"
                    UPDATE "ob-poc".document_requirements
                    SET current_task_id = $2, status = 'requested', updated_at = now()
                    WHERE requirement_id = $1
                    "#,
                )
                .bind(req_id)
                .bind(task_id)
                .execute(pool)
                .await?;
            }

            ctx.bind("task", task_id);
        }

        let result = serde_json::json!({
            "task_id": task_id,
            "expected_count": doc_types.len(),
            "requirement_ids": requirement_ids
        });

        Ok(ExecutionResult::Record(result))
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
        use sqlx::Row;

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

        if blob_ref.is_none() && structured_data.is_none() {
            return Err(anyhow::anyhow!(
                "Either blob-ref or structured-data is required"
            ));
        }

        // Optional validity dates
        let valid_from: Option<NaiveDate> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "valid-from")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let valid_to: Option<NaiveDate> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "valid-to")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        // Verify document exists
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

        // Get next version number
        let version_row =
            sqlx::query(r#"SELECT "ob-poc".get_next_document_version($1) as version_no"#)
                .bind(document_id)
                .fetch_one(pool)
                .await?;

        let version_no: i32 = version_row.try_get("version_no").unwrap_or(1);

        let version_id = Uuid::now_v7();

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

        // Generate cargo_ref URI
        let cargo_ref = format!("version://ob-poc/{}", version_id);

        ctx.bind("version", version_id);

        let result = serde_json::json!({
            "version_id": version_id,
            "version_no": version_no,
            "cargo_ref": cargo_ref
        });

        Ok(ExecutionResult::Record(result))
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

        Ok(ExecutionResult::Affected(1))
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
        use sqlx::Row;

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

        // Validate rejection code exists
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

        Ok(ExecutionResult::Affected(1))
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
        use sqlx::Row;

        // Extract required argument
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

        let results: Vec<serde_json::Value> = rows
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
            .collect();

        Ok(ExecutionResult::RecordSet(results))
    }
}
