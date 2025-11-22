//! Document Service - CRUD operations for document management
//!
//! This module provides database operations for document types, catalog,
//! metadata, and relationships following the document dictionary pattern.
//!
//! Schema aligned with "ob-poc".document_catalog table structure.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// Document type definition from the dictionary
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentType {
    pub type_id: Uuid,
    pub type_code: String,
    pub display_name: String,
    pub category: String,
    pub required_attributes: Option<JsonValue>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Document catalog entry - aligned with actual DB schema in data_designer
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentCatalogEntry {
    pub doc_id: Uuid,
    pub document_name: Option<String>,
    pub document_type_id: Option<Uuid>,
    pub document_type_code: Option<String>,
    pub cbu_id: Option<Uuid>,
    pub file_hash_sha256: Option<String>,
    pub storage_key: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub mime_type: Option<String>,
    pub status: Option<String>,
    pub metadata: Option<JsonValue>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

// Compatibility alias for code using document_id
impl DocumentCatalogEntry {
    pub fn document_id(&self) -> Uuid {
        self.doc_id
    }

    pub fn document_code(&self) -> String {
        self.document_name.clone().unwrap_or_default()
    }
}

/// Fields for creating a new document catalog entry
#[derive(Debug, Clone)]
pub struct NewDocumentFields {
    pub document_code: String,
    pub document_type_id: Uuid,
    pub issuer_id: Option<Uuid>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub file_hash: Option<String>,
    pub file_path: Option<String>,
    pub mime_type: Option<String>,
    pub confidentiality_level: Option<String>,
    pub cbu_id: Option<Uuid>,
}

/// Document metadata (attribute values extracted from documents)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub document_id: Uuid,
    pub attribute_id: Uuid,
    pub value: String,
}

/// Document relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRelationship {
    pub source_document_id: Uuid,
    pub target_document_id: Uuid,
    pub relationship_type: String,
}

/// Service for document operations
#[derive(Clone, Debug)]
pub struct DocumentService {
    pool: PgPool,
}

impl DocumentService {
    /// Create a new document service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get document type by code
    pub async fn get_document_type_by_code(&self, type_code: &str) -> Result<Option<DocumentType>> {
        let result = sqlx::query_as::<_, DocumentType>(
            r#"
            SELECT type_id, type_code, display_name, category, required_attributes, created_at, updated_at
            FROM "ob-poc".document_types
            WHERE type_code = $1
            "#,
        )
        .bind(type_code)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get document type by code")?;

        Ok(result)
    }

    /// Get document type ID by code
    pub async fn get_document_type_id_by_code(&self, type_code: &str) -> Result<Option<Uuid>> {
        let result = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT type_id
            FROM "ob-poc".document_types
            WHERE type_code = $1
            "#,
        )
        .bind(type_code)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get document type ID by code")?;

        Ok(result)
    }

    /// Create a document catalog entry with NewDocumentFields
    pub async fn create_document(&self, fields: &NewDocumentFields) -> Result<Uuid> {
        let doc_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_catalog (
                doc_id, document_name, document_type_id, cbu_id,
                file_hash_sha256, mime_type, status, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, 'active', NOW(), NOW())
            "#,
        )
        .bind(doc_id)
        .bind(&fields.document_code) // maps to document_name
        .bind(fields.document_type_id)
        .bind(fields.cbu_id)
        .bind(&fields.file_hash)
        .bind(&fields.mime_type)
        .execute(&self.pool)
        .await
        .context("Failed to create document catalog entry")?;

        info!(
            "Created document catalog entry {} with name {} (type_id: {})",
            doc_id, fields.document_code, fields.document_type_id
        );

        Ok(doc_id)
    }

    /// Get document catalog entry by ID
    pub async fn get_document_by_id(
        &self,
        document_id: Uuid,
    ) -> Result<Option<DocumentCatalogEntry>> {
        let result = sqlx::query_as::<_, DocumentCatalogEntry>(
            r#"
            SELECT doc_id, document_name, document_type_id, document_type_code,
                   cbu_id, file_hash_sha256, storage_key, file_size_bytes,
                   mime_type, status, metadata, created_at, updated_at
            FROM "ob-poc".document_catalog
            WHERE doc_id = $1
            "#,
        )
        .bind(document_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get document by ID")?;

        Ok(result)
    }

    /// Get documents for a CBU
    pub async fn get_documents_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<DocumentCatalogEntry>> {
        let results = sqlx::query_as::<_, DocumentCatalogEntry>(
            r#"
            SELECT doc_id, document_name, document_type_id, document_type_code,
                   cbu_id, file_hash_sha256, storage_key, file_size_bytes,
                   mime_type, status, metadata, created_at, updated_at
            FROM "ob-poc".document_catalog
            WHERE cbu_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get documents for CBU")?;

        Ok(results)
    }

    /// Get documents by document name pattern
    pub async fn get_documents_by_code(
        &self,
        document_name: &str,
    ) -> Result<Vec<DocumentCatalogEntry>> {
        let results = sqlx::query_as::<_, DocumentCatalogEntry>(
            r#"
            SELECT doc_id, document_name, document_type_id, document_type_code,
                   cbu_id, file_hash_sha256, storage_key, file_size_bytes,
                   mime_type, status, metadata, created_at, updated_at
            FROM "ob-poc".document_catalog
            WHERE document_name = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(document_name)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get documents by name")?;

        Ok(results)
    }

    /// Set document metadata (attribute extracted from document)
    pub async fn set_document_metadata(
        &self,
        doc_id: Uuid,
        attribute_id: Uuid,
        value: &str,
    ) -> Result<()> {
        // Convert string value to JSON
        let json_value = serde_json::json!(value);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_metadata (doc_id, attribute_id, value, created_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (doc_id, attribute_id)
            DO UPDATE SET value = $3
            "#,
        )
        .bind(doc_id)
        .bind(attribute_id)
        .bind(&json_value)
        .execute(&self.pool)
        .await
        .context("Failed to set document metadata")?;

        info!(
            "Set metadata for doc {} attribute {} = '{}'",
            doc_id, attribute_id, value
        );

        Ok(())
    }

    /// Get all metadata for a document
    pub async fn get_document_metadata(&self, doc_id: Uuid) -> Result<Vec<(Uuid, String)>> {
        let rows = sqlx::query_as::<_, (Uuid, JsonValue)>(
            r#"
            SELECT attribute_id, value
            FROM "ob-poc".document_metadata
            WHERE doc_id = $1
            "#,
        )
        .bind(doc_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get document metadata")?;

        // Convert JSON values to strings
        let result: Vec<(Uuid, String)> = rows
            .into_iter()
            .map(|(id, val)| {
                let s = match val {
                    JsonValue::String(s) => s,
                    other => other.to_string(),
                };
                (id, s)
            })
            .collect();

        Ok(result)
    }

    /// Link two documents with a relationship
    pub async fn link_documents(
        &self,
        primary_doc_id: Uuid,
        related_doc_id: Uuid,
        relationship_type: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_relationships (
                primary_doc_id, related_doc_id, relationship_type
            )
            VALUES ($1, $2, $3)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(primary_doc_id)
        .bind(related_doc_id)
        .bind(relationship_type)
        .execute(&self.pool)
        .await
        .context("Failed to link documents")?;

        info!(
            "Linked documents {} -> {} ({})",
            primary_doc_id, related_doc_id, relationship_type
        );

        Ok(())
    }

    /// Get required attributes for a document type
    pub async fn get_required_attributes_for_doc_type(
        &self,
        type_code: &str,
    ) -> Result<Vec<String>> {
        let doc_type = self.get_document_type_by_code(type_code).await?;

        match doc_type {
            Some(dt) => {
                if let Some(attrs) = dt.required_attributes {
                    if let Some(arr) = attrs.as_array() {
                        let result: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        return Ok(result);
                    }
                }
                Ok(vec![])
            }
            None => Ok(vec![]),
        }
    }

    /// Update document status
    pub async fn update_document_status(&self, doc_id: Uuid, status: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".document_catalog
            SET status = $1, updated_at = NOW()
            WHERE doc_id = $2
            "#,
        )
        .bind(status)
        .bind(doc_id)
        .execute(&self.pool)
        .await
        .context("Failed to update document status")?;

        Ok(result.rows_affected() > 0)
    }

    /// Get source attributes that a document type produces
    pub async fn get_source_attributes_for_doc_type(&self, doc_type: &str) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT attribute_id
            FROM "ob-poc".dictionary
            WHERE source IS NOT NULL
              AND source::text ILIKE $1
            "#,
        )
        .bind(format!("%{}%", doc_type))
        .fetch_all(&self.pool)
        .await
        .context("Failed to get source attributes for doc type")?;

        Ok(rows)
    }

    /// Ensure a document type exists, creating it if necessary
    pub async fn ensure_document_type(
        &self,
        type_code: &str,
        display_name: &str,
        category: &str,
        description: &str,
    ) -> Result<Uuid> {
        // Try to get existing type first
        if let Some(existing) = self.get_document_type_by_code(type_code).await? {
            return Ok(existing.type_id);
        }

        // Create new type
        let type_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, description)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(type_id)
        .bind(type_code)
        .bind(display_name)
        .bind(category)
        .bind(description)
        .execute(&self.pool)
        .await
        .context("Failed to ensure document type")?;

        Ok(type_id)
    }

    /// Link a document to a CBU
    pub async fn link_document_to_cbu(&self, doc_id: Uuid, cbu_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".document_catalog
            SET cbu_id = $1, updated_at = NOW()
            WHERE doc_id = $2
            "#,
        )
        .bind(cbu_id)
        .bind(doc_id)
        .execute(&self.pool)
        .await
        .context("Failed to link document to CBU")?;

        Ok(result.rows_affected() > 0)
    }

    /// Create a document catalog entry with full metadata
    /// Used by cbu_crud_template service for template/instance tracking
    pub async fn create_document_with_metadata(
        &self,
        doc_id: Uuid,
        type_code: &str,
        name: &str,
        metadata: serde_json::Value,
    ) -> Result<Uuid> {
        // Look up document type ID
        let document_type_id = self
            .get_document_type_id_by_code(type_code)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Document type '{}' not found", type_code))?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_catalog (
                doc_id, document_name, document_type_id,
                metadata, status
            )
            VALUES ($1, $2, $3, $4, 'active')
            "#,
        )
        .bind(doc_id)
        .bind(name)
        .bind(document_type_id)
        .bind(metadata)
        .execute(&self.pool)
        .await
        .context("Failed to create document with metadata")?;

        Ok(doc_id)
    }

    /// Get document catalog entry by ID (returns name and metadata)
    /// Used by cbu_crud_template service
    pub async fn get_document_catalog_entry(
        &self,
        doc_id: Uuid,
    ) -> Result<Option<(String, serde_json::Value)>> {
        let row = sqlx::query_as::<_, (Option<String>, Option<serde_json::Value>)>(
            r#"
            SELECT document_name, metadata
            FROM "ob-poc".document_catalog
            WHERE doc_id = $1
            "#,
        )
        .bind(doc_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get document catalog entry")?;

        Ok(row.map(|(name, meta)| {
            (
                name.unwrap_or_default(),
                meta.unwrap_or_else(|| serde_json::json!({})),
            )
        }))
    }

    /// Find template document by template_id in metadata
    /// Used by cbu_crud_template service
    pub async fn find_template_by_id(
        &self,
        template_id: &str,
    ) -> Result<Option<(Uuid, serde_json::Value)>> {
        // Look up the document type ID for templates
        let type_id = self
            .get_document_type_id_by_code("DSL.CRUD.CBU.TEMPLATE")
            .await?;

        if type_id.is_none() {
            return Ok(None);
        }

        let row = sqlx::query_as::<_, (Uuid, Option<serde_json::Value>)>(
            r#"
            SELECT doc_id, metadata
            FROM "ob-poc".document_catalog
            WHERE document_type_id = $1
            AND metadata->>'template_id' = $2
            AND status != 'deleted'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(type_id.unwrap())
        .bind(template_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to find template by ID")?;

        Ok(row.map(|(id, meta)| (id, meta.unwrap_or_else(|| serde_json::json!({})))))
    }
}
