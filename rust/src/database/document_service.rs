//! Document Service - CRUD operations for document management
//!
//! This module provides database operations for document types, catalog,
//! metadata, and relationships following the document dictionary pattern.

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

/// Document catalog entry
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentCatalogEntry {
    pub doc_id: Uuid,
    pub entity_id: String,
    pub document_type: String,
    pub issuer: Option<String>,
    pub title: Option<String>,
    pub file_hash: Option<String>,
    pub storage_key: Option<String>,
    pub mime_type: Option<String>,
    pub confidentiality_level: Option<String>,
    pub status: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Document metadata (attribute values extracted from documents)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub doc_id: Uuid,
    pub attribute_id: Uuid,
    pub value: String,
}

/// Document relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRelationship {
    pub primary_doc_id: Uuid,
    pub related_doc_id: Uuid,
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

    /// Create a document catalog entry
    pub async fn create_document_catalog_entry(
        &self,
        entity_id: &str,
        document_type: &str,
        issuer: Option<&str>,
        title: Option<&str>,
        file_hash: Option<&str>,
        storage_key: Option<&str>,
        mime_type: Option<&str>,
        confidentiality_level: Option<&str>,
    ) -> Result<Uuid> {
        let doc_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_catalog (
                doc_id, entity_id, document_type, issuer, title,
                file_hash, storage_key, mime_type, confidentiality_level,
                status, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'ACTIVE', NOW(), NOW())
            "#,
        )
        .bind(doc_id)
        .bind(entity_id)
        .bind(document_type)
        .bind(issuer)
        .bind(title)
        .bind(file_hash)
        .bind(storage_key)
        .bind(mime_type)
        .bind(confidentiality_level)
        .execute(&self.pool)
        .await
        .context("Failed to create document catalog entry")?;

        info!(
            "Created document catalog entry {} for entity {} (type: {})",
            doc_id, entity_id, document_type
        );

        Ok(doc_id)
    }

    /// Get document catalog entry by ID
    pub async fn get_document_by_id(&self, doc_id: Uuid) -> Result<Option<DocumentCatalogEntry>> {
        let result = sqlx::query_as::<_, DocumentCatalogEntry>(
            r#"
            SELECT doc_id, entity_id, document_type, issuer, title,
                   file_hash, storage_key, mime_type, confidentiality_level,
                   status, created_at, updated_at
            FROM "ob-poc".document_catalog
            WHERE doc_id = $1
            "#,
        )
        .bind(doc_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get document by ID")?;

        Ok(result)
    }

    /// Get documents for an entity
    pub async fn get_documents_for_entity(
        &self,
        entity_id: &str,
    ) -> Result<Vec<DocumentCatalogEntry>> {
        let results = sqlx::query_as::<_, DocumentCatalogEntry>(
            r#"
            SELECT doc_id, entity_id, document_type, issuer, title,
                   file_hash, storage_key, mime_type, confidentiality_level,
                   status, created_at, updated_at
            FROM "ob-poc".document_catalog
            WHERE entity_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get documents for entity")?;

        Ok(results)
    }

    /// Set document metadata (attribute extracted from document)
    pub async fn set_document_metadata(
        &self,
        doc_id: Uuid,
        attribute_id: Uuid,
        value: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_metadata (doc_id, attribute_id, value, created_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            ON CONFLICT (doc_id, attribute_id)
            DO UPDATE SET value = $3, updated_at = NOW()
            "#,
        )
        .bind(doc_id)
        .bind(attribute_id)
        .bind(value)
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
        let rows = sqlx::query_as::<_, (Uuid, String)>(
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

        Ok(rows)
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
                primary_doc_id, related_doc_id, relationship_type, created_at
            )
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (primary_doc_id, related_doc_id, relationship_type)
            DO NOTHING
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
}
