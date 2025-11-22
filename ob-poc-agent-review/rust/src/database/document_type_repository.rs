//! Document Type Repository
//!
//! Database access layer for document types and their attribute mappings.

use crate::models::document_type_models::{
    DocumentAttributeMapping, DocumentType, ExtractedAttribute, ExtractionMethod, TypedDocument,
};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

/// Repository for document type operations
#[derive(Clone)]
pub struct DocumentTypeRepository {
    pool: Arc<PgPool>,
}

impl DocumentTypeRepository {
    /// Create a new document type repository
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Get document type by code
    pub async fn get_by_code(&self, type_code: &str) -> Result<Option<DocumentType>, sqlx::Error> {
        sqlx::query_as::<_, DocumentType>(
            r#"
            SELECT type_id, type_code, display_name, category, domain, description
            FROM "ob-poc".document_types
            WHERE type_code = $1
            "#,
        )
        .bind(type_code)
        .fetch_optional(self.pool.as_ref())
        .await
    }

    /// Get document type by ID
    pub async fn get_by_id(&self, type_id: Uuid) -> Result<Option<DocumentType>, sqlx::Error> {
        sqlx::query_as::<_, DocumentType>(
            r#"
            SELECT type_id, type_code, display_name, category, domain, description
            FROM "ob-poc".document_types
            WHERE type_id = $1
            "#,
        )
        .bind(type_id)
        .fetch_optional(self.pool.as_ref())
        .await
    }

    /// Get all document types
    pub async fn get_all(&self) -> Result<Vec<DocumentType>, sqlx::Error> {
        sqlx::query_as::<_, DocumentType>(
            r#"
            SELECT type_id, type_code, display_name, category, domain, description
            FROM "ob-poc".document_types
            ORDER BY type_code
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await
    }

    /// Get attribute mappings for a document type
    pub async fn get_mappings(
        &self,
        type_id: Uuid,
    ) -> Result<Vec<DocumentAttributeMapping>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                mapping_id,
                document_type_id,
                attribute_uuid,
                extraction_method,
                field_location,
                field_name,
                confidence_threshold,
                is_required,
                validation_pattern
            FROM "ob-poc".document_attribute_mappings
            WHERE document_type_id = $1
            ORDER BY is_required DESC, confidence_threshold DESC
            "#,
        )
        .bind(type_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        let mut mappings = Vec::new();
        for row in rows {
            let extraction_method_str: String = row.get("extraction_method");
            let extraction_method =
                extraction_method_str
                    .parse::<ExtractionMethod>()
                    .map_err(|e| {
                        sqlx::Error::Decode(Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            e,
                        )))
                    })?;

            mappings.push(DocumentAttributeMapping {
                mapping_id: row.get("mapping_id"),
                document_type_id: row.get("document_type_id"),
                attribute_uuid: row.get("attribute_uuid"),
                extraction_method,
                field_location: row.get("field_location"),
                field_name: row.get("field_name"),
                confidence_threshold: row
                    .get::<sqlx::types::BigDecimal, _>("confidence_threshold")
                    .to_string()
                    .parse()
                    .unwrap_or(0.80),
                is_required: row.get("is_required"),
                validation_pattern: row.get("validation_pattern"),
            });
        }

        Ok(mappings)
    }

    /// Get typed document (document type + mappings)
    pub async fn get_typed_document(
        &self,
        document_id: Uuid,
    ) -> Result<Option<TypedDocument>, sqlx::Error> {
        // Get document type from document_catalog
        let doc_type_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT document_type_id
            FROM "ob-poc".document_catalog
            WHERE document_id = $1
            "#,
        )
        .bind(document_id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        let Some(type_id) = doc_type_id else {
            return Ok(None);
        };

        // Get document type
        let Some(doc_type) = self.get_by_id(type_id).await? else {
            return Ok(None);
        };

        // Get mappings
        let mappings = self.get_mappings(type_id).await?;

        Ok(Some(TypedDocument {
            document_id,
            document_type: doc_type,
            extractable_attributes: mappings,
        }))
    }

    /// Get required attributes for a document type
    pub async fn get_required_attributes(&self, type_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
        sqlx::query_scalar(
            r#"
            SELECT attribute_uuid
            FROM "ob-poc".document_attribute_mappings
            WHERE document_type_id = $1 AND is_required = true
            ORDER BY confidence_threshold DESC
            "#,
        )
        .bind(type_id)
        .fetch_all(self.pool.as_ref())
        .await
    }

    /// Check if attribute can be extracted from document type
    pub async fn can_extract_attribute(
        &self,
        type_id: Uuid,
        attribute_uuid: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".document_attribute_mappings
            WHERE document_type_id = $1 AND attribute_uuid = $2
            "#,
        )
        .bind(type_id)
        .bind(attribute_uuid)
        .fetch_one(self.pool.as_ref())
        .await?;

        Ok(count > 0)
    }

    /// Get extraction method for an attribute from document type
    pub async fn get_extraction_method(
        &self,
        type_id: Uuid,
        attribute_uuid: Uuid,
    ) -> Result<Option<ExtractionMethod>, sqlx::Error> {
        let method_str: Option<String> = sqlx::query_scalar(
            r#"
            SELECT extraction_method
            FROM "ob-poc".document_attribute_mappings
            WHERE document_type_id = $1 AND attribute_uuid = $2
            "#,
        )
        .bind(type_id)
        .bind(attribute_uuid)
        .fetch_optional(self.pool.as_ref())
        .await?;

        match method_str {
            Some(s) => Ok(Some(s.parse().map_err(|e: String| {
                sqlx::Error::Decode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                )))
            })?)),
            None => Ok(None),
        }
    }

    /// Store extracted attribute value
    pub async fn store_extracted_value(
        &self,
        document_id: Uuid,
        entity_id: Uuid,
        extracted: &ExtractedAttribute,
    ) -> Result<(), sqlx::Error> {
        // START TRANSACTION
        let mut tx = self.pool.begin().await?;

        // Store in document_metadata (using correct column name: doc_id and attribute_id)
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_metadata
            (doc_id, attribute_id, value, extraction_confidence, extraction_method, extracted_at, extraction_metadata)
            VALUES ($1, $2, $3, $4, $5, NOW(), $6)
            ON CONFLICT (doc_id, attribute_id)
            DO UPDATE SET
                value = EXCLUDED.value,
                extraction_confidence = EXCLUDED.extraction_confidence,
                extraction_method = EXCLUDED.extraction_method,
                extracted_at = EXCLUDED.extracted_at,
                extraction_metadata = EXCLUDED.extraction_metadata
            "#,
        )
        .bind(document_id)
        .bind(extracted.attribute_uuid)
        .bind(&extracted.value)
        .bind(extracted.confidence)
        .bind(extracted.extraction_method.to_string())
        .bind(sqlx::types::Json(&extracted.metadata))
        .execute(&mut *tx)
        .await?;

        // Store in attribute_values_typed
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".attribute_values_typed
            (entity_id, attribute_uuid, value_json)
            VALUES ($1, $2, $3)
            ON CONFLICT (entity_id, attribute_uuid)
            DO UPDATE SET
                value_json = EXCLUDED.value_json,
                updated_at = NOW()
            "#,
        )
        .bind(entity_id)
        .bind(extracted.attribute_uuid)
        .bind(&extracted.value)
        .execute(&mut *tx)
        .await?;

        // COMMIT TRANSACTION
        tx.commit().await?;
        Ok(())
    }

    /// Get all extracted values for a document
    pub async fn get_extracted_values(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<ExtractedAttribute>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                attribute_uuid,
                extracted_value as value,
                extraction_confidence as confidence,
                extraction_method,
                extraction_metadata as metadata
            FROM "ob-poc".document_metadata
            WHERE document_id = $1
            "#,
        )
        .bind(document_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        let mut extracted = Vec::new();
        for row in rows {
            let method_str: String = row.get("extraction_method");
            let method = method_str.parse::<ExtractionMethod>().map_err(|e| {
                sqlx::Error::Decode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                )))
            })?;

            extracted.push(ExtractedAttribute {
                attribute_uuid: row.get("attribute_uuid"),
                value: row.get("value"),
                confidence: row
                    .get::<Option<sqlx::types::BigDecimal>, _>("confidence")
                    .map(|bd| bd.to_string().parse().unwrap_or(0.0))
                    .unwrap_or(0.0),
                extraction_method: method,
                metadata: row
                    .get::<Option<sqlx::types::Json<_>>, _>("metadata")
                    .map(|j| j.0),
            });
        }

        Ok(extracted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running PostgreSQL database with the schema
    // Run with: cargo test --features database document_type_repository -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_get_document_type_by_code() {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql:///data_designer?user=adamtc007".to_string());
        let pool = Arc::new(PgPool::connect(&database_url).await.unwrap());
        let repo = DocumentTypeRepository::new(pool);

        let doc_type = repo.get_by_code("PASSPORT").await.unwrap();
        assert!(doc_type.is_some());

        let passport = doc_type.unwrap();
        assert_eq!(passport.type_code, "PASSPORT");
        assert_eq!(passport.category, "IDENTITY");
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_mappings() {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql:///data_designer?user=adamtc007".to_string());
        let pool = Arc::new(PgPool::connect(&database_url).await.unwrap());
        let repo = DocumentTypeRepository::new(pool);

        let doc_type = repo.get_by_code("PASSPORT").await.unwrap().unwrap();
        let mappings = repo.get_mappings(doc_type.type_id).await.unwrap();

        assert!(!mappings.is_empty());
        assert!(mappings
            .iter()
            .any(|m| m.extraction_method == ExtractionMethod::MRZ));
    }
}
