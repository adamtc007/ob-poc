//! Document custom operations
//!
//! Operations for document cataloging and extraction that require
//! type lookups and external service integration.

use anyhow::Result;
use async_trait::async_trait;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Document cataloging with document type lookup (Idempotent)
///
/// Rationale: Requires lookup of document_type_id from document_types table
/// by type code, then insert into document_catalog with type-specific
/// attribute mappings from document_type_attributes.
///
/// Idempotency: Uses ON CONFLICT on (cbu_id, document_type_id, document_name)
/// to return existing document if already cataloged.
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
        let doc_id = Uuid::new_v4();

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
        Ok(ExecutionResult::Uuid(uuid::Uuid::new_v4()))
    }
}

/// Document extraction using AI/OCR
///
/// Rationale: Requires external AI service call for OCR/extraction,
/// then maps extracted values to attributes via document_type_attributes.
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
