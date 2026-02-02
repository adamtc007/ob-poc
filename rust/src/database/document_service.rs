//! Document Service - CRUD operations for document management
//!
//! This module provides database operations for document types, catalog,
//! metadata, and relationships following the document dictionary pattern.
//!
//! Schema aligned with "ob-poc".document_catalog table structure.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
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
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Document catalog entry - aligned with actual DB schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentCatalogEntry {
    pub document_id: Uuid,
    pub document_code: String,
    pub document_type_id: Uuid,
    pub issuer_id: Option<Uuid>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub issue_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub verification_status: Option<String>,
    pub file_path: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub file_hash: Option<String>,
    pub mime_type: Option<String>,
    pub confidentiality_level: Option<String>,
    pub cbu_id: Option<Uuid>,
    pub extracted_attributes: Option<JsonValue>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
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

    /// Create a document catalog entry
    pub async fn create_document(&self, fields: &NewDocumentFields) -> Result<Uuid> {
        let document_id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_catalog (
                document_id, document_code, document_type_id, issuer_id,
                title, description, file_hash, file_path, mime_type,
                confidentiality_level, cbu_id, verification_status,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'pending', NOW(), NOW())
            "#,
        )
        .bind(document_id)
        .bind(&fields.document_code)
        .bind(fields.document_type_id)
        .bind(fields.issuer_id)
        .bind(&fields.title)
        .bind(&fields.description)
        .bind(&fields.file_hash)
        .bind(&fields.file_path)
        .bind(&fields.mime_type)
        .bind(&fields.confidentiality_level)
        .bind(fields.cbu_id)
        .execute(&self.pool)
        .await
        .context("Failed to create document catalog entry")?;

        info!(
            "Created document catalog entry {} with code {} (type_id: {})",
            document_id, fields.document_code, fields.document_type_id
        );

        Ok(document_id)
    }

    /// Get document catalog entry by ID
    pub async fn get_document_by_id(
        &self,
        document_id: Uuid,
    ) -> Result<Option<DocumentCatalogEntry>> {
        let result = sqlx::query_as::<_, DocumentCatalogEntry>(
            r#"
            SELECT document_id, document_code, document_type_id, issuer_id,
                   title, description, language, issue_date, expiry_date,
                   verification_status, file_path, file_size_bytes, file_hash,
                   mime_type, confidentiality_level, cbu_id, extracted_attributes,
                   created_at, updated_at
            FROM "ob-poc".document_catalog
            WHERE document_id = $1
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
            SELECT document_id, document_code, document_type_id, issuer_id,
                   title, description, language, issue_date, expiry_date,
                   verification_status, file_path, file_size_bytes, file_hash,
                   mime_type, confidentiality_level, cbu_id, extracted_attributes,
                   created_at, updated_at
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

    /// Get documents by document code
    pub async fn get_documents_by_code(
        &self,
        document_code: &str,
    ) -> Result<Vec<DocumentCatalogEntry>> {
        let results = sqlx::query_as::<_, DocumentCatalogEntry>(
            r#"
            SELECT document_id, document_code, document_type_id, issuer_id,
                   title, description, language, issue_date, expiry_date,
                   verification_status, file_path, file_size_bytes, file_hash,
                   mime_type, confidentiality_level, cbu_id, extracted_attributes,
                   created_at, updated_at
            FROM "ob-poc".document_catalog
            WHERE document_code = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(document_code)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get documents by code")?;

        Ok(results)
    }

    /// Set document metadata (attribute extracted from document)
    pub async fn set_document_metadata(
        &self,
        document_id: Uuid,
        attribute_id: Uuid,
        value: &str,
    ) -> Result<()> {
        let json_value = serde_json::json!(value);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_metadata (document_id, attribute_id, value, created_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (document_id, attribute_id)
            DO UPDATE SET value = $3
            "#,
        )
        .bind(document_id)
        .bind(attribute_id)
        .bind(&json_value)
        .execute(&self.pool)
        .await
        .context("Failed to set document metadata")?;

        info!(
            "Set metadata for doc {} attribute {} = '{}'",
            document_id, attribute_id, value
        );

        Ok(())
    }

    /// Get all metadata for a document
    pub async fn get_document_metadata(&self, document_id: Uuid) -> Result<Vec<(Uuid, String)>> {
        let rows = sqlx::query_as::<_, (Uuid, JsonValue)>(
            r#"
            SELECT attribute_id, value
            FROM "ob-poc".document_metadata
            WHERE document_id = $1
            "#,
        )
        .bind(document_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get document metadata")?;

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
        source_document_id: Uuid,
        target_document_id: Uuid,
        relationship_type: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_relationships (
                source_document_id, target_document_id, relationship_type
            )
            VALUES ($1, $2, $3)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(source_document_id)
        .bind(target_document_id)
        .bind(relationship_type)
        .execute(&self.pool)
        .await
        .context("Failed to link documents")?;

        info!(
            "Linked documents {} -> {} ({})",
            source_document_id, target_document_id, relationship_type
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

    /// Update document verification status
    pub async fn update_document_status(&self, document_id: Uuid, status: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".document_catalog
            SET verification_status = $1, updated_at = NOW()
            WHERE document_id = $2
            "#,
        )
        .bind(status)
        .bind(document_id)
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
        if let Some(existing) = self.get_document_type_by_code(type_code).await? {
            return Ok(existing.type_id);
        }

        let type_id = Uuid::now_v7();
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
    pub async fn link_document_to_cbu(&self, document_id: Uuid, cbu_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".document_catalog
            SET cbu_id = $1, updated_at = NOW()
            WHERE document_id = $2
            "#,
        )
        .bind(cbu_id)
        .bind(document_id)
        .execute(&self.pool)
        .await
        .context("Failed to link document to CBU")?;

        Ok(result.rows_affected() > 0)
    }

    /// Create a document catalog entry with extracted attributes metadata
    pub async fn create_document_with_metadata(
        &self,
        document_id: Uuid,
        type_code: &str,
        document_code: &str,
        extracted_attributes: serde_json::Value,
    ) -> Result<Uuid> {
        let document_type_id = self
            .get_document_type_id_by_code(type_code)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Document type '{}' not found", type_code))?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_catalog (
                document_id, document_code, document_type_id,
                extracted_attributes, verification_status
            )
            VALUES ($1, $2, $3, $4, 'pending')
            "#,
        )
        .bind(document_id)
        .bind(document_code)
        .bind(document_type_id)
        .bind(extracted_attributes)
        .execute(&self.pool)
        .await
        .context("Failed to create document with metadata")?;

        Ok(document_id)
    }

    /// Get document catalog entry by ID (returns code and extracted_attributes)
    pub async fn get_document_catalog_entry(
        &self,
        document_id: Uuid,
    ) -> Result<Option<(String, serde_json::Value)>> {
        let row = sqlx::query_as::<_, (String, Option<serde_json::Value>)>(
            r#"
            SELECT document_code, extracted_attributes
            FROM "ob-poc".document_catalog
            WHERE document_id = $1
            "#,
        )
        .bind(document_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get document catalog entry")?;

        Ok(row.map(|(code, attrs)| (code, attrs.unwrap_or_else(|| serde_json::json!({})))))
    }

    /// Find template document by template_id in extracted_attributes
    pub async fn find_template_by_id(
        &self,
        template_id: &str,
    ) -> Result<Option<(Uuid, serde_json::Value)>> {
        let type_id = self
            .get_document_type_id_by_code("DSL.CRUD.CBU.TEMPLATE")
            .await?;

        if type_id.is_none() {
            return Ok(None);
        }

        let row = sqlx::query_as::<_, (Uuid, Option<serde_json::Value>)>(
            r#"
            SELECT document_id, extracted_attributes
            FROM "ob-poc".document_catalog
            WHERE document_type_id = $1
            AND extracted_attributes->>'template_id' = $2
            AND verification_status != 'deleted'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(type_id.unwrap())
        .bind(template_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to find template by ID")?;

        Ok(row.map(|(id, attrs)| (id, attrs.unwrap_or_else(|| serde_json::json!({})))))
    }
}
