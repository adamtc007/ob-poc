//! Document Catalog Source
//!
//! Implements the AttributeSource trait for resolving attribute values
//! from uploaded documents via extraction services.

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::domains::attributes::execution_context::ExecutionContext;
// Note: ObPocError removed - not available in current error module
use super::extraction_service::ExtractionService;

/// Result type for source operations
pub type SourceResult<T> = Result<T, SourceError>;

/// Source-specific errors
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("No document found for CBU {0} with attribute {1}")]
    NoDocumentFound(Uuid, Uuid),

    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Trait for attribute sources - provides attribute values from various sources
#[async_trait]
pub trait AttributeSource: Send + Sync {
    /// Get attribute value from this source
    async fn get_value(
        &self,
        attribute_id: &Uuid,
        context: &ExecutionContext,
    ) -> SourceResult<Option<serde_json::Value>>;

    /// Priority of this source (higher = try first)
    fn priority(&self) -> i32;

    /// Name of this source for logging
    fn source_name(&self) -> &'static str;
}

/// Document-based attribute source
pub struct DocumentCatalogSource {
    pool: PgPool,
    extraction_service: Arc<dyn ExtractionService>,
}

impl DocumentCatalogSource {
    pub fn new(pool: PgPool, extraction_service: Arc<dyn ExtractionService>) -> Self {
        Self {
            pool,
            extraction_service,
        }
    }

    /// Find best document for an attribute
    async fn find_best_document(
        &self,
        cbu_id: &Uuid,
        attr_id: &Uuid,
    ) -> SourceResult<Option<Uuid>> {
        // First, check if we already have extracted metadata
        let existing_doc = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT doc_id
            FROM "ob-poc".document_metadata dm
            JOIN "ob-poc".document_usage du ON dm.doc_id = du.doc_id
            WHERE du.cbu_id = $1
            AND dm.attribute_id = $2
            ORDER BY dm.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .bind(attr_id)
        .fetch_optional(&self.pool)
        .await?;

        if existing_doc.is_some() {
            return Ok(existing_doc);
        }

        // If no existing extraction, find suitable document by catalog
        let doc_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT dc.doc_id
            FROM "ob-poc".document_catalog dc
            JOIN "ob-poc".document_usage du ON dc.doc_id = du.doc_id
            WHERE du.cbu_id = $1
            AND dc.extraction_status IN ('PENDING', 'COMPLETED')
            ORDER BY dc.last_extracted_at DESC NULLS LAST, dc.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(doc_id)
    }

    /// Extract attribute value from document
    async fn extract_from_document(
        &self,
        doc_id: &Uuid,
        attr_id: &Uuid,
    ) -> SourceResult<serde_json::Value> {
        // Check if already extracted
        if let Some(existing) = self.get_existing_extraction(doc_id, attr_id).await? {
            return Ok(existing);
        }

        // Perform extraction
        let value = self
            .extraction_service
            .extract(doc_id, attr_id)
            .await
            .map_err(|e| SourceError::ExtractionFailed(e.to_string()))?;

        // Store in metadata
        self.store_extraction(doc_id, attr_id, &value).await?;

        Ok(value)
    }

    /// Get existing extraction from document_metadata
    async fn get_existing_extraction(
        &self,
        doc_id: &Uuid,
        attr_id: &Uuid,
    ) -> SourceResult<Option<serde_json::Value>> {
        let value = sqlx::query_scalar::<_, serde_json::Value>(
            r#"
            SELECT value
            FROM "ob-poc".document_metadata
            WHERE doc_id = $1 AND attribute_id = $2
            "#,
        )
        .bind(doc_id)
        .bind(attr_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(value)
    }

    /// Store extraction result in document_metadata
    async fn store_extraction(
        &self,
        doc_id: &Uuid,
        attr_id: &Uuid,
        value: &serde_json::Value,
    ) -> SourceResult<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_metadata
            (doc_id, attribute_id, value)
            VALUES ($1, $2, $3)
            ON CONFLICT (doc_id, attribute_id)
            DO UPDATE SET
                value = EXCLUDED.value,
                created_at = NOW()
            "#,
        )
        .bind(doc_id)
        .bind(attr_id)
        .bind(value)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Log extraction attempt
    async fn log_extraction(
        &self,
        cbu_id: &Uuid,
        doc_id: &Uuid,
        attr_id: &Uuid,
        success: bool,
        error_message: Option<&str>,
        processing_time_ms: i32,
        extracted_value: Option<&serde_json::Value>,
    ) -> SourceResult<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".attribute_extraction_log
            (cbu_id, document_id, attribute_id, extraction_method, success,
             extracted_value, error_message, processing_time_ms)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(cbu_id)
        .bind(doc_id)
        .bind(attr_id)
        .bind(self.extraction_service.method_name())
        .bind(success)
        .bind(extracted_value)
        .bind(error_message)
        .bind(processing_time_ms)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl AttributeSource for DocumentCatalogSource {
    async fn get_value(
        &self,
        attribute_id: &Uuid,
        context: &ExecutionContext,
    ) -> SourceResult<Option<serde_json::Value>> {
        // Note: We need to extract CBU ID from context
        // For now, we'll create a placeholder CBU ID
        // In real implementation, context should have cbu_id field
        let cbu_id = Uuid::nil(); // TODO: Get from context when ExecutionContext has cbu_id

        let start = Instant::now();

        // Find best document for this attribute
        let doc_id = match self.find_best_document(&cbu_id, attribute_id).await? {
            Some(id) => id,
            None => {
                // Log failed attempt
                self.log_extraction(
                    &cbu_id,
                    &Uuid::nil(),
                    attribute_id,
                    false,
                    Some("No suitable document found"),
                    start.elapsed().as_millis() as i32,
                    None,
                )
                .await?;
                return Ok(None);
            }
        };

        // Extract value from document
        match self.extract_from_document(&doc_id, attribute_id).await {
            Ok(value) => {
                // Log successful extraction
                self.log_extraction(
                    &cbu_id,
                    &doc_id,
                    attribute_id,
                    true,
                    None,
                    start.elapsed().as_millis() as i32,
                    Some(&value),
                )
                .await?;

                Ok(Some(value))
            }
            Err(e) => {
                // Log failed extraction
                self.log_extraction(
                    &cbu_id,
                    &doc_id,
                    attribute_id,
                    false,
                    Some(&e.to_string()),
                    start.elapsed().as_millis() as i32,
                    None,
                )
                .await?;

                Err(e)
            }
        }
    }

    fn priority(&self) -> i32 {
        100 // High priority for document sources
    }

    fn source_name(&self) -> &'static str {
        "document_catalog"
    }
}

/// Form data source (placeholder for future implementation)
pub struct FormDataSource {
    #[allow(dead_code)]
    pool: PgPool,
}

impl FormDataSource {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AttributeSource for FormDataSource {
    async fn get_value(
        &self,
        _attribute_id: &Uuid,
        _context: &ExecutionContext,
    ) -> SourceResult<Option<serde_json::Value>> {
        // TODO: Implement form data lookup
        Ok(None)
    }

    fn priority(&self) -> i32 {
        50 // Medium priority
    }

    fn source_name(&self) -> &'static str {
        "form_data"
    }
}

/// API data source (placeholder for future implementation)
pub struct ApiDataSource;

impl ApiDataSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ApiDataSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttributeSource for ApiDataSource {
    async fn get_value(
        &self,
        _attribute_id: &Uuid,
        _context: &ExecutionContext,
    ) -> SourceResult<Option<serde_json::Value>> {
        // TODO: Implement third-party API calls
        Ok(None)
    }

    fn priority(&self) -> i32 {
        10 // Low priority - fallback only
    }

    fn source_name(&self) -> &'static str {
        "third_party_api"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::extraction_service::MockExtractionService;

    #[test]
    fn test_source_priorities() {
        let doc_source = DocumentCatalogSource {
            pool: PgPool::connect_lazy("postgresql://localhost/test").unwrap(),
            extraction_service: Arc::new(MockExtractionService::new()),
        };
        let form_source = FormDataSource {
            pool: PgPool::connect_lazy("postgresql://localhost/test").unwrap(),
        };
        let api_source = ApiDataSource::new();

        assert!(doc_source.priority() > form_source.priority());
        assert!(form_source.priority() > api_source.priority());
    }
}
