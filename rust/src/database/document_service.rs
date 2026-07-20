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
pub(crate) struct DocumentType {
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
pub(crate) struct DocumentCatalogEntry {
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
pub(crate) struct NewDocumentFields {
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



/// Service for document operations
#[derive(Clone, Debug)]
pub(crate) struct DocumentService {
    pool: PgPool,
}

impl DocumentService {
    /// Create a new document service
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get document type by code
    pub(crate) async fn get_document_type_by_code(&self, type_code: &str) -> Result<Option<DocumentType>> {
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
    pub(crate) async fn get_document_type_id_by_code(&self, type_code: &str) -> Result<Option<Uuid>> {
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
    pub(crate) async fn create_document(&self, fields: &NewDocumentFields) -> Result<Uuid> {
        let document_id = Uuid::new_v4();

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









    /// Get source attributes that a document type produces
    pub(crate) async fn get_source_attributes_for_doc_type(&self, doc_type: &str) -> Result<Vec<Uuid>> {
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
