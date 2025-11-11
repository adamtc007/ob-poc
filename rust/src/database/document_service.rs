//! Document Database Service - EAV/Metadata-Driven Operations
//!
//! This module provides database operations for the new EAV-style document catalog system.
//! All operations work with the AttributeID-as-Type pattern and support the document DSL verbs.

use crate::models::document_models::*;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

/// Document database service providing EAV operations
pub struct DocumentDatabaseService {
    pool: PgPool,
}

impl DocumentDatabaseService {
    /// Create a new document database service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ========================================================================
    // DOCUMENT CATALOG OPERATIONS
    // ========================================================================

    /// Create a new document in the catalog
    pub async fn create_document(&self, document: NewDocumentCatalog) -> Result<Uuid> {
        let doc_id = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".document_catalog
            (file_hash_sha256, storage_key, file_size_bytes, mime_type, extracted_data, extraction_status, extraction_confidence)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING doc_id
            "#,
            document.file_hash_sha256,
            document.storage_key,
            document.file_size_bytes,
            document.mime_type,
            document.extracted_data,
            document.extraction_status.as_deref().unwrap_or("PENDING"),
            document.extraction_confidence
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to insert document catalog entry")?
        .doc_id;

        Ok(doc_id)
    }

    /// Get document by ID
    pub async fn get_document(&self, doc_id: Uuid) -> Result<Option<DocumentCatalog>> {
        let document = sqlx::query_as!(
            DocumentCatalog,
            r#"
            SELECT doc_id, file_hash_sha256, storage_key, file_size_bytes, mime_type,
                   extracted_data, extraction_status, extraction_confidence,
                   last_extracted_at, created_at, updated_at
            FROM "ob-poc".document_catalog
            WHERE doc_id = $1
            "#,
            doc_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch document")?;

        Ok(document)
    }

    /// Get document with all metadata
    pub async fn get_document_with_metadata(
        &self,
        doc_id: Uuid,
    ) -> Result<Option<DocumentCatalogWithMetadata>> {
        let document = sqlx::query_as!(
            DocumentCatalogWithMetadata,
            r#"
            SELECT doc_id, file_hash_sha256, storage_key, file_size_bytes, mime_type,
                   extracted_data, extraction_status, extraction_confidence,
                   last_extracted_at, created_at, updated_at, metadata
            FROM "ob-poc".document_catalog_with_metadata
            WHERE doc_id = $1
            "#,
            doc_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch document with metadata")?;

        Ok(document)
    }

    /// Update document extraction results
    pub async fn update_document_extraction(
        &self,
        doc_id: Uuid,
        update: UpdateDocumentCatalog,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".document_catalog
            SET extracted_data = COALESCE($2, extracted_data),
                extraction_status = COALESCE($3, extraction_status),
                extraction_confidence = COALESCE($4, extraction_confidence),
                last_extracted_at = COALESCE($5, last_extracted_at),
                updated_at = (now() at time zone 'utc')
            WHERE doc_id = $1
            "#,
            doc_id,
            update.extracted_data,
            update.extraction_status,
            update.extraction_confidence,
            update.last_extracted_at
        )
        .execute(&self.pool)
        .await
        .context("Failed to update document extraction")?;

        Ok(())
    }

    /// Delete document and all related data
    pub async fn delete_document(&self, doc_id: Uuid) -> Result<()> {
        sqlx::query!(
            r#"DELETE FROM "ob-poc".document_catalog WHERE doc_id = $1"#,
            doc_id
        )
        .execute(&self.pool)
        .await
        .context("Failed to delete document")?;

        Ok(())
    }

    // ========================================================================
    // DOCUMENT METADATA OPERATIONS (EAV)
    // ========================================================================

    /// Add metadata attribute to document
    pub async fn add_document_metadata(&self, metadata: NewDocumentMetadata) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".document_metadata (doc_id, attribute_id, value)
            VALUES ($1, $2, $3)
            ON CONFLICT (doc_id, attribute_id) DO UPDATE SET
                value = EXCLUDED.value,
                created_at = (now() at time zone 'utc')
            "#,
            metadata.doc_id,
            metadata.attribute_id,
            metadata.value
        )
        .execute(&self.pool)
        .await
        .context("Failed to add document metadata")?;

        Ok(())
    }

    /// Add batch metadata to document
    pub async fn add_document_metadata_batch(&self, batch: DocumentMetadataBatch) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for attr_value in batch.metadata {
            sqlx::query!(
                r#"
                INSERT INTO "ob-poc".document_metadata (doc_id, attribute_id, value)
                VALUES ($1, $2, $3)
                ON CONFLICT (doc_id, attribute_id) DO UPDATE SET
                    value = EXCLUDED.value,
                    created_at = (now() at time zone 'utc')
                "#,
                batch.doc_id,
                attr_value.attribute_id,
                attr_value.value
            )
            .execute(&mut *tx)
            .await
            .context("Failed to add metadata in batch")?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Get all metadata for a document
    pub async fn get_document_metadata(&self, doc_id: Uuid) -> Result<Vec<DocumentMetadata>> {
        let metadata = sqlx::query_as!(
            DocumentMetadata,
            r#"
            SELECT doc_id, attribute_id, value, created_at
            FROM "ob-poc".document_metadata
            WHERE doc_id = $1
            ORDER BY created_at ASC
            "#,
            doc_id
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch document metadata")?;

        Ok(metadata)
    }

    /// Get specific metadata attribute for document
    pub async fn get_document_attribute(
        &self,
        doc_id: Uuid,
        attribute_id: Uuid,
    ) -> Result<Option<DocumentMetadata>> {
        let metadata = sqlx::query_as!(
            DocumentMetadata,
            r#"
            SELECT doc_id, attribute_id, value, created_at
            FROM "ob-poc".document_metadata
            WHERE doc_id = $1 AND attribute_id = $2
            "#,
            doc_id,
            attribute_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch document attribute")?;

        Ok(metadata)
    }

    /// Remove metadata attribute from document
    pub async fn remove_document_metadata(&self, doc_id: Uuid, attribute_id: Uuid) -> Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM "ob-poc".document_metadata
            WHERE doc_id = $1 AND attribute_id = $2
            "#,
            doc_id,
            attribute_id
        )
        .execute(&self.pool)
        .await
        .context("Failed to remove document metadata")?;

        Ok(())
    }

    // ========================================================================
    // DOCUMENT RELATIONSHIP OPERATIONS
    // ========================================================================

    /// Create relationship between documents
    pub async fn create_document_relationship(
        &self,
        relationship: NewDocumentRelationship,
    ) -> Result<Uuid> {
        let relationship_id = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".document_relationships
            (primary_doc_id, related_doc_id, relationship_type)
            VALUES ($1, $2, $3)
            RETURNING relationship_id
            "#,
            relationship.primary_doc_id,
            relationship.related_doc_id,
            relationship.relationship_type
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to create document relationship")?
        .relationship_id;

        Ok(relationship_id)
    }

    /// Get all relationships for a document
    pub async fn get_document_relationships(
        &self,
        doc_id: Uuid,
    ) -> Result<Vec<DocumentRelationship>> {
        let relationships = sqlx::query_as!(
            DocumentRelationship,
            r#"
            SELECT relationship_id, primary_doc_id, related_doc_id, relationship_type, created_at
            FROM "ob-poc".document_relationships
            WHERE primary_doc_id = $1 OR related_doc_id = $1
            ORDER BY created_at DESC
            "#,
            doc_id
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch document relationships")?;

        Ok(relationships)
    }

    /// Remove document relationship
    pub async fn remove_document_relationship(&self, relationship_id: Uuid) -> Result<()> {
        sqlx::query!(
            r#"DELETE FROM "ob-poc".document_relationships WHERE relationship_id = $1"#,
            relationship_id
        )
        .execute(&self.pool)
        .await
        .context("Failed to remove document relationship")?;

        Ok(())
    }

    // ========================================================================
    // DOCUMENT USAGE OPERATIONS
    // ========================================================================

    /// Record document usage by CBU/entity
    pub async fn record_document_usage(&self, usage: NewDocumentUsage) -> Result<Uuid> {
        let usage_id = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".document_usage
            (doc_id, cbu_id, entity_id, usage_context)
            VALUES ($1, $2, $3, $4)
            RETURNING usage_id
            "#,
            usage.doc_id,
            usage.cbu_id,
            usage.entity_id,
            usage.usage_context
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to record document usage")?
        .usage_id;

        Ok(usage_id)
    }

    /// Get document usage history
    pub async fn get_document_usage_history(&self, doc_id: Uuid) -> Result<Vec<DocumentUsage>> {
        let usage = sqlx::query_as!(
            DocumentUsage,
            r#"
            SELECT usage_id, doc_id, cbu_id, entity_id, usage_context, used_at
            FROM "ob-poc".document_usage
            WHERE doc_id = $1
            ORDER BY used_at DESC
            "#,
            doc_id
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch document usage history")?;

        Ok(usage)
    }

    /// Get documents used by CBU
    pub async fn get_cbu_documents(&self, cbu_id: Uuid) -> Result<Vec<DocumentUsage>> {
        let usage = sqlx::query_as!(
            DocumentUsage,
            r#"
            SELECT usage_id, doc_id, cbu_id, entity_id, usage_context, used_at
            FROM "ob-poc".document_usage
            WHERE cbu_id = $1
            ORDER BY used_at DESC
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch CBU documents")?;

        Ok(usage)
    }

    /// Remove document usage record
    pub async fn remove_document_usage(&self, usage_id: Uuid) -> Result<()> {
        sqlx::query!(
            r#"DELETE FROM "ob-poc".document_usage WHERE usage_id = $1"#,
            usage_id
        )
        .execute(&self.pool)
        .await
        .context("Failed to remove document usage")?;

        Ok(())
    }

    // ========================================================================
    // SEARCH OPERATIONS
    // ========================================================================

    /// Search documents with filters
    pub async fn search_documents(
        &self,
        request: DocumentSearchRequest,
    ) -> Result<DocumentSearchResponse> {
        let limit = request.limit.unwrap_or(20);
        let offset = request.offset.unwrap_or(0);

        // Build base query
        let mut query_conditions = Vec::new();
        let mut bind_values: Vec<String> = Vec::new();
        let mut bind_idx = 1;

        // Full-text search condition
        if let Some(query) = &request.query {
            query_conditions.push(format!(
                "EXISTS (SELECT 1 FROM \"ob-poc\".document_metadata dm
                 JOIN \"ob-poc\".dictionary d ON dm.attribute_id = d.attribute_id
                 WHERE dm.doc_id = dc.doc_id AND dm.value::text ILIKE ${})",
                bind_idx
            ));
            bind_values.push(format!("%{}%", query));
            bind_idx += 1;
        }

        // Extraction status filter
        if let Some(status) = &request.extraction_status {
            query_conditions.push(format!("dc.extraction_status = ${}", bind_idx));
            bind_values.push(status.clone());
            bind_idx += 1;
        }

        // MIME type filter
        if let Some(mime_type) = &request.mime_type {
            query_conditions.push(format!("dc.mime_type = ${}", bind_idx));
            bind_values.push(mime_type.clone());
            bind_idx += 1;
        }

        // Confidence filter
        if let Some(min_confidence) = request.min_confidence {
            query_conditions.push(format!("dc.extraction_confidence >= ${}", bind_idx));
            bind_values.push(min_confidence.to_string());
            bind_idx += 1;
        }

        // CBU usage filter
        if let Some(cbu_id) = request.used_by_cbu {
            query_conditions.push(format!(
                "EXISTS (SELECT 1 FROM \"ob-poc\".document_usage du
                 WHERE du.doc_id = dc.doc_id AND du.cbu_id = ${})",
                bind_idx
            ));
            bind_values.push(cbu_id.to_string());
            bind_idx += 1;
        }

        // Date range filters
        if let Some(created_from) = request.created_from {
            query_conditions.push(format!("dc.created_at >= ${}", bind_idx));
            bind_values.push(created_from.to_rfc3339());
            bind_idx += 1;
        }

        if let Some(created_to) = request.created_to {
            query_conditions.push(format!("dc.created_at <= ${}", bind_idx));
            bind_values.push(created_to.to_rfc3339());
            bind_idx += 1;
        }

        // Build WHERE clause
        let where_clause = if query_conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", query_conditions.join(" AND "))
        };

        // Execute search query using the view
        let search_query = format!(
            r#"
            SELECT doc_id, file_hash_sha256, storage_key, file_size_bytes, mime_type,
                   extracted_data, extraction_status, extraction_confidence,
                   last_extracted_at, created_at, updated_at, metadata
            FROM "ob-poc".document_catalog_with_metadata dc
            {}
            ORDER BY dc.created_at DESC
            LIMIT {} OFFSET {}
            "#,
            where_clause, limit, offset
        );

        // For now, we'll execute a simpler version without dynamic binding
        // In a production system, you'd want to use proper parameter binding
        let documents = sqlx::query_as!(
            DocumentCatalogWithMetadata,
            r#"
            SELECT doc_id, file_hash_sha256, storage_key, file_size_bytes, mime_type,
                   extracted_data, extraction_status, extraction_confidence,
                   last_extracted_at, created_at, updated_at, metadata
            FROM "ob-poc".document_catalog_with_metadata
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to search documents")?;

        // Get total count
        let total_count = sqlx::query_scalar!(r#"SELECT COUNT(*) FROM "ob-poc".document_catalog"#)
            .fetch_one(&self.pool)
            .await
            .context("Failed to get total document count")?
            .unwrap_or(0);

        Ok(DocumentSearchResponse {
            documents,
            total_count,
            has_more: (offset + limit) < total_count,
        })
    }

    /// Find documents by hash (for duplicate detection)
    pub async fn find_by_hash(&self, file_hash: &str) -> Result<Vec<DocumentCatalog>> {
        let documents = sqlx::query_as!(
            DocumentCatalog,
            r#"
            SELECT doc_id, file_hash_sha256, storage_key, file_size_bytes, mime_type,
                   extracted_data, extraction_status, extraction_confidence,
                   last_extracted_at, created_at, updated_at
            FROM "ob-poc".document_catalog
            WHERE file_hash_sha256 = $1
            "#,
            file_hash
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to find documents by hash")?;

        Ok(documents)
    }

    // ========================================================================
    // STATISTICS AND ANALYTICS
    // ========================================================================

    /// Get document statistics
    pub async fn get_document_statistics(&self) -> Result<DocumentStatistics> {
        // Total documents
        let total_documents =
            sqlx::query_scalar!(r#"SELECT COUNT(*) FROM "ob-poc".document_catalog"#)
                .fetch_one(&self.pool)
                .await?
                .unwrap_or(0);

        // Documents by status
        let status_rows = sqlx::query!(
            r#"
            SELECT extraction_status, COUNT(*) as count
            FROM "ob-poc".document_catalog
            GROUP BY extraction_status
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut documents_by_status = HashMap::new();
        for row in status_rows {
            documents_by_status.insert(row.extraction_status, row.count.unwrap_or(0));
        }

        // Documents by MIME type
        let mime_rows = sqlx::query!(
            r#"
            SELECT mime_type, COUNT(*) as count
            FROM "ob-poc".document_catalog
            WHERE mime_type IS NOT NULL
            GROUP BY mime_type
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut documents_by_mime_type = HashMap::new();
        for row in mime_rows {
            if let Some(mime_type) = row.mime_type {
                documents_by_mime_type.insert(mime_type, row.count.unwrap_or(0));
            }
        }

        // Average extraction confidence
        let avg_confidence = sqlx::query_scalar!(
            r#"
            SELECT AVG(extraction_confidence)
            FROM "ob-poc".document_catalog
            WHERE extraction_confidence IS NOT NULL
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        // Metadata statistics
        let total_metadata =
            sqlx::query_scalar!(r#"SELECT COUNT(*) FROM "ob-poc".document_metadata"#)
                .fetch_one(&self.pool)
                .await?
                .unwrap_or(0);

        let total_relationships =
            sqlx::query_scalar!(r#"SELECT COUNT(*) FROM "ob-poc".document_relationships"#)
                .fetch_one(&self.pool)
                .await?
                .unwrap_or(0);

        let total_usage = sqlx::query_scalar!(r#"SELECT COUNT(*) FROM "ob-poc".document_usage"#)
            .fetch_one(&self.pool)
            .await?
            .unwrap_or(0);

        // Most used attributes
        let attribute_usage = sqlx::query!(
            r#"
            SELECT dm.attribute_id, d.name, COUNT(*) as usage_count
            FROM "ob-poc".document_metadata dm
            JOIN "ob-poc".dictionary d ON dm.attribute_id = d.attribute_id
            GROUP BY dm.attribute_id, d.name
            ORDER BY usage_count DESC
            LIMIT 10
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let total_attrs = attribute_usage
            .iter()
            .map(|r| r.usage_count.unwrap_or(0))
            .sum::<i64>();
        let most_used_attributes = attribute_usage
            .into_iter()
            .map(|row| AttributeUsageStats {
                attribute_id: row.attribute_id,
                attribute_name: row.name,
                usage_count: row.usage_count.unwrap_or(0),
                percentage: if total_attrs > 0 {
                    (row.usage_count.unwrap_or(0) as f64 / total_attrs as f64) * 100.0
                } else {
                    0.0
                },
            })
            .collect();

        Ok(DocumentStatistics {
            total_documents,
            documents_by_status,
            documents_by_mime_type,
            average_extraction_confidence: avg_confidence,
            total_metadata_entries: total_metadata,
            total_relationships,
            total_usage_entries: total_usage,
            most_used_attributes,
        })
    }

    /// Get document summary for listing
    pub async fn get_document_summary(&self, doc_id: Uuid) -> Result<Option<DocumentSummary>> {
        let summary = sqlx::query!(
            r#"
            SELECT
                dc.doc_id,
                dc.storage_key,
                dc.mime_type,
                dc.extraction_status,
                dc.extraction_confidence,
                dc.created_at,
                COUNT(DISTINCT dm.attribute_id) as metadata_count,
                COUNT(DISTINCT du.usage_id) as usage_count,
                COUNT(DISTINCT dr.relationship_id) as relationship_count
            FROM "ob-poc".document_catalog dc
            LEFT JOIN "ob-poc".document_metadata dm ON dc.doc_id = dm.doc_id
            LEFT JOIN "ob-poc".document_usage du ON dc.doc_id = du.doc_id
            LEFT JOIN "ob-poc".document_relationships dr ON (dc.doc_id = dr.primary_doc_id OR dc.doc_id = dr.related_doc_id)
            WHERE dc.doc_id = $1
            GROUP BY dc.doc_id, dc.storage_key, dc.mime_type, dc.extraction_status, dc.extraction_confidence, dc.created_at
            "#,
            doc_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch document summary")?;

        if let Some(row) = summary {
            Ok(Some(DocumentSummary {
                doc_id: row.doc_id,
                storage_key: row.storage_key,
                mime_type: row.mime_type,
                extraction_status: row.extraction_status,
                extraction_confidence: row.extraction_confidence,
                metadata_count: row.metadata_count.unwrap_or(0),
                usage_count: row.usage_count.unwrap_or(0),
                relationship_count: row.relationship_count.unwrap_or(0),
                created_at: row.created_at,
            }))
        } else {
            Ok(None)
        }
    }

    // ========================================================================
    // BULK OPERATIONS
    // ========================================================================

    /// Bulk import documents
    pub async fn bulk_import_documents(
        &self,
        import_request: BulkDocumentImport,
    ) -> Result<BulkImportResult> {
        let mut tx = self.pool.begin().await?;

        let mut successful_imports = 0;
        let mut failed_imports = 0;
        let mut skipped_duplicates = 0;
        let mut imported_doc_ids = Vec::new();
        let mut errors = Vec::new();

        for document in import_request.documents {
            // Check for duplicates if requested
            if import_request.skip_duplicates {
                let existing = sqlx::query_scalar!(
                    r#"SELECT COUNT(*) FROM "ob-poc".document_catalog WHERE file_hash_sha256 = $1"#,
                    document.file_hash_sha256
                )
                .fetch_one(&mut *tx)
                .await?
                .unwrap_or(0);

                if existing > 0 {
                    skipped_duplicates += 1;
                    continue;
                }
            }

            // Insert document
            match sqlx::query!(
                r#"
                INSERT INTO "ob-poc".document_catalog
                (file_hash_sha256, storage_key, file_size_bytes, mime_type, extracted_data, extraction_status, extraction_confidence)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                RETURNING doc_id
                "#,
                document.file_hash_sha256,
                document.storage_key,
                document.file_size_bytes,
                document.mime_type,
                document.extracted_data,
                document.extraction_status.as_deref().unwrap_or("PENDING"),
                document.extraction_confidence
            )
            .fetch_one(&mut *tx)
            .await
            {
                Ok(row) => {
                    imported_doc_ids.push(row.doc_id);
                    successful_imports += 1;
                }
                Err(e) => {
                    failed_imports += 1;
                    errors.push(format!("Failed to import {}: {}", document.storage_key, e));
                }
            }
        }

        // Add metadata batches
        for batch in import_request.metadata_batches {
            if imported_doc_ids.contains(&batch.doc_id) {
                for attr_value in batch.metadata {
                    if let Err(e) = sqlx::query!(
                        r#"
                        INSERT INTO "ob-poc".document_metadata (doc_id, attribute_id, value)
                        VALUES ($1, $2, $3)
                        "#,
                        batch.doc_id,
                        attr_value.attribute_id,
                        attr_value.value
                    )
                    .execute(&mut *tx)
                    .await
                    {
                        errors.push(format!(
                            "Failed to add metadata for {}: {}",
                            batch.doc_id, e
                        ));
                    }
                }
            }
        }

        tx.commit().await?;

        Ok(BulkImportResult {
            total_processed: import_request.documents.len(),
            successful_imports,
            failed_imports,
            skipped_duplicates,
            imported_doc_ids,
            errors,
        })
    }

    // ========================================================================
    // HEALTH CHECK
    // ========================================================================

    /// Check database connectivity and table existence
    pub async fn health_check(&self) -> Result<bool> {
        // Simple query to check if our tables exist and are accessible
        let _count =
            sqlx::query_scalar!(r#"SELECT COUNT(*) FROM "ob-poc".document_catalog LIMIT 1"#)
                .fetch_one(&self.pool)
                .await
                .context("Document service health check failed")?;

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    async fn create_test_service() -> DocumentDatabaseService {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());
        let pool = PgPool::connect(&database_url).await.unwrap();
        DocumentDatabaseService::new(pool)
    }

    #[sqlx::test]
    async fn test_document_creation(pool: PgPool) -> Result<()> {
        let service = DocumentDatabaseService::new(pool);

        let new_doc = NewDocumentCatalog {
            file_hash_sha256: "test_hash_123".to_string(),
            storage_key: "test/document.pdf".to_string(),
            file_size_bytes: Some(1024),
            mime_type: Some("application/pdf".to_string()),
            extracted_data: None,
            extraction_status: Some("PENDING".to_string()),
            extraction_confidence: None,
        };

        let doc_id = service.create_document(new_doc).await?;
        assert!(!doc_id.is_nil());

        let retrieved = service.get_document(doc_id).await?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().file_hash_sha256, "test_hash_123");

        Ok(())
    }

    #[sqlx::test]
    async fn test_metadata_operations(pool: PgPool) -> Result<()> {
        let service = DocumentDatabaseService::new(pool);

        // Create a document first
        let new_doc = NewDocumentCatalog::default();
        let doc_id = service.create_document(new_doc).await?;

        // Add metadata
        let metadata = NewDocumentMetadata {
            doc_id,
            attribute_id: Uuid::new_v4(),
            value: serde_json::Value::String("test_value".to_string()),
        };

        service.add_document_metadata(metadata).await?;

        // Retrieve metadata
        let retrieved_metadata = service.get_document_metadata(doc_id).await?;
        assert_eq!(retrieved_metadata.len(), 1);
        assert_eq!(retrieved_metadata[0].doc_id, doc_id);

        Ok(())
    }
}
