//! Document-Attribute Mapping CRUD Service
//!
//! Manages the sparse matrix of document types → attributes
//!
//! Tables (actual schema):
//! - document_types: type_id, type_code, display_name, category, domain, description
//! - document_attribute_mappings: mapping_id, document_type_id (FK), attribute_uuid (FK)
//! - attribute_registry: uuid, id, display_name, value_type, category, etc.

use sqlx::PgPool;
use uuid::Uuid;

/// Result of a CRUD operation
#[derive(Debug, Clone)]
pub struct CrudResult {
    pub success: bool,
    pub id: Option<Uuid>,
    pub message: String,
    pub affected_rows: Option<i64>,
}

/// Document type info
#[derive(Debug, Clone)]
pub struct DocumentTypeInfo {
    pub type_id: Uuid,
    pub type_code: String,
    pub display_name: String,
    pub category: String,
    pub domain: Option<String>,
}

/// Attribute info (from attribute_registry)
#[derive(Debug, Clone)]
pub struct AttributeInfo {
    pub uuid: Uuid,
    pub id: String,
    pub display_name: String,
    pub value_type: String,
}

/// Mapping info
#[derive(Debug, Clone)]
pub struct MappingInfo {
    pub mapping_id: Uuid,
    pub document_type_id: Uuid,
    pub attribute_uuid: Uuid,
    pub extraction_method: String,
    pub is_required: bool,
}

#[derive(Debug, Clone)]
pub struct DocumentAttributeCrudService {
    pool: PgPool,
}

impl DocumentAttributeCrudService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // DOCUMENT TYPE CRUD
    // =========================================================================

    /// Create a new document type
    pub async fn create_document_type(
        &self,
        type_code: &str,
        display_name: &str,
        category: &str,
        domain: Option<&str>,
        description: Option<&str>,
    ) -> Result<CrudResult, String> {
        let type_id = Uuid::new_v4();

        let result = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".document_types (
                type_id, type_code, display_name, category, domain, description
            ) VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (type_code) DO NOTHING
            RETURNING type_id
            "#,
            type_id,
            type_code,
            display_name,
            category,
            domain,
            description
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to create document type: {}", e))?;

        match result {
            Some(row) => Ok(CrudResult {
                success: true,
                id: Some(row.type_id),
                message: format!("Created document type '{}'", type_code),
                affected_rows: Some(1),
            }),
            None => {
                // Already exists - get existing ID
                let existing = self.get_document_type(type_code).await?;
                Ok(CrudResult {
                    success: true,
                    id: existing.map(|e| e.type_id),
                    message: format!("Document type '{}' already exists", type_code),
                    affected_rows: Some(0),
                })
            }
        }
    }

    /// Get document type by code
    pub async fn get_document_type(
        &self,
        type_code: &str,
    ) -> Result<Option<DocumentTypeInfo>, String> {
        let row = sqlx::query!(
            r#"
            SELECT type_id, type_code, display_name, category, domain
            FROM "ob-poc".document_types
            WHERE type_code = $1
            "#,
            type_code
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to get document type: {}", e))?;

        Ok(row.map(|r| DocumentTypeInfo {
            type_id: r.type_id,
            type_code: r.type_code,
            display_name: r.display_name,
            category: r.category,
            domain: r.domain,
        }))
    }

    /// Get document type by ID
    pub async fn get_document_type_by_id(
        &self,
        type_id: Uuid,
    ) -> Result<Option<DocumentTypeInfo>, String> {
        let row = sqlx::query!(
            r#"
            SELECT type_id, type_code, display_name, category, domain
            FROM "ob-poc".document_types
            WHERE type_id = $1
            "#,
            type_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to get document type: {}", e))?;

        Ok(row.map(|r| DocumentTypeInfo {
            type_id: r.type_id,
            type_code: r.type_code,
            display_name: r.display_name,
            category: r.category,
            domain: r.domain,
        }))
    }

    /// Delete document type (cascade deletes mappings via FK)
    pub async fn delete_document_type(&self, type_code: &str) -> Result<CrudResult, String> {
        let result = sqlx::query!(
            r#"DELETE FROM "ob-poc".document_types WHERE type_code = $1"#,
            type_code
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to delete document type: {}", e))?;

        Ok(CrudResult {
            success: result.rows_affected() > 0,
            id: None,
            message: format!(
                "Deleted document type '{}' ({} rows)",
                type_code,
                result.rows_affected()
            ),
            affected_rows: Some(result.rows_affected() as i64),
        })
    }

    // =========================================================================
    // ATTRIBUTE CRUD (using attribute_registry)
    // =========================================================================

    /// Get attribute by code/id
    pub async fn get_attribute_by_code(
        &self,
        attribute_id: &str,
    ) -> Result<Option<AttributeInfo>, String> {
        let row = sqlx::query!(
            r#"
            SELECT uuid, id, display_name, value_type
            FROM "ob-poc".attribute_registry
            WHERE id = $1
            "#,
            attribute_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to get attribute: {}", e))?;

        Ok(row.map(|r| AttributeInfo {
            uuid: r.uuid,
            id: r.id,
            display_name: r.display_name,
            value_type: r.value_type,
        }))
    }

    /// Get attribute by UUID
    pub async fn get_attribute_by_uuid(
        &self,
        attribute_uuid: Uuid,
    ) -> Result<Option<AttributeInfo>, String> {
        let row = sqlx::query!(
            r#"
            SELECT uuid, id, display_name, value_type
            FROM "ob-poc".attribute_registry
            WHERE uuid = $1
            "#,
            attribute_uuid
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to get attribute: {}", e))?;

        Ok(row.map(|r| AttributeInfo {
            uuid: r.uuid,
            id: r.id,
            display_name: r.display_name,
            value_type: r.value_type,
        }))
    }

    // =========================================================================
    // MAPPING CRUD (Link/Unlink)
    // =========================================================================

    /// Link an attribute to a document type
    /// Creates entry in document_attribute_mappings
    pub async fn link_attribute_to_document_type(
        &self,
        document_type_id: Uuid,
        attribute_uuid: Uuid,
        extraction_method: &str,
        is_required: Option<bool>,
        field_name: Option<&str>,
    ) -> Result<CrudResult, String> {
        let mapping_id = Uuid::new_v4();
        let required = is_required.unwrap_or(false);

        let result = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".document_attribute_mappings (
                mapping_id, document_type_id, attribute_uuid,
                extraction_method, is_required, field_name
            ) VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (document_type_id, attribute_uuid) DO NOTHING
            RETURNING mapping_id
            "#,
            mapping_id,
            document_type_id,
            attribute_uuid,
            extraction_method,
            required,
            field_name
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to link attribute: {}", e))?;

        match result {
            Some(row) => Ok(CrudResult {
                success: true,
                id: Some(row.mapping_id),
                message: "Linked attribute to document type".to_string(),
                affected_rows: Some(1),
            }),
            None => Ok(CrudResult {
                success: false,
                id: None,
                message: "Mapping already exists".to_string(),
                affected_rows: Some(0),
            }),
        }
    }

    /// Unlink an attribute from a document type
    /// Removes entry from document_attribute_mappings
    pub async fn unlink_attribute_from_document_type(
        &self,
        document_type_id: Uuid,
        attribute_uuid: Uuid,
    ) -> Result<CrudResult, String> {
        let result = sqlx::query!(
            r#"
            DELETE FROM "ob-poc".document_attribute_mappings
            WHERE document_type_id = $1 AND attribute_uuid = $2
            "#,
            document_type_id,
            attribute_uuid
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to unlink attribute: {}", e))?;

        Ok(CrudResult {
            success: result.rows_affected() > 0,
            id: None,
            message: format!(
                "Unlinked attribute from document type ({} rows)",
                result.rows_affected()
            ),
            affected_rows: Some(result.rows_affected() as i64),
        })
    }

    /// Get all mappings for a document type
    pub async fn get_mappings_for_document_type(
        &self,
        document_type_id: Uuid,
    ) -> Result<Vec<MappingInfo>, String> {
        let rows = sqlx::query!(
            r#"
            SELECT
                mapping_id,
                document_type_id,
                attribute_uuid,
                extraction_method,
                is_required
            FROM "ob-poc".document_attribute_mappings
            WHERE document_type_id = $1
            "#,
            document_type_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to get mappings: {}", e))?;

        Ok(rows
            .into_iter()
            .map(|r| MappingInfo {
                mapping_id: r.mapping_id,
                document_type_id: r.document_type_id,
                attribute_uuid: r.attribute_uuid,
                extraction_method: r.extraction_method,
                is_required: r.is_required.unwrap_or(false),
            })
            .collect())
    }

    /// Get all document types that have a specific attribute
    pub async fn get_document_types_for_attribute(
        &self,
        attribute_uuid: Uuid,
    ) -> Result<Vec<Uuid>, String> {
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT document_type_id
            FROM "ob-poc".document_attribute_mappings
            WHERE attribute_uuid = $1
            "#,
            attribute_uuid
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to get document types: {}", e))?;

        Ok(rows.into_iter().map(|r| r.document_type_id).collect())
    }

    /// Check if a mapping exists
    pub async fn mapping_exists(
        &self,
        document_type_id: Uuid,
        attribute_uuid: Uuid,
    ) -> Result<bool, String> {
        let row = sqlx::query!(
            r#"
            SELECT mapping_id FROM "ob-poc".document_attribute_mappings
            WHERE document_type_id = $1 AND attribute_uuid = $2
            "#,
            document_type_id,
            attribute_uuid
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to check mapping: {}", e))?;

        Ok(row.is_some())
    }
}

// =========================================================================
// TESTS
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crud_result() {
        let result = CrudResult {
            success: true,
            id: Some(Uuid::new_v4()),
            message: "Test".to_string(),
            affected_rows: Some(1),
        };
        assert!(result.success);
    }
}
