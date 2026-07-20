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
pub(crate) struct CrudResult {
    pub success: bool,
    pub id: Option<Uuid>,
    pub message: String,
    pub affected_rows: Option<i64>,
}

/// Document type info
#[derive(Debug, Clone)]
pub(crate) struct DocumentTypeInfo {
    pub type_id: Uuid,
    pub type_code: String,
    pub display_name: String,
    pub category: String,
    pub domain: Option<String>,
}

/// Attribute info (from attribute_registry)
#[derive(Debug, Clone)]
pub(crate) struct AttributeInfo {
    pub uuid: Uuid,
    pub id: String,
    pub display_name: String,
    pub value_type: String,
}

/// Mapping info
#[derive(Debug, Clone)]
pub(crate) struct MappingInfo {
    pub mapping_id: Uuid,
    pub document_type_id: Uuid,
    pub attribute_uuid: Uuid,
    pub extraction_method: String,
    pub is_required: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct DocumentAttributeCrudService {
    pool: PgPool,
}

impl DocumentAttributeCrudService {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // DOCUMENT TYPE CRUD
    // =========================================================================


    /// Get document type by code
    pub(crate) async fn get_document_type(
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



    // =========================================================================
    // ATTRIBUTE CRUD (using attribute_registry)
    // =========================================================================



    // =========================================================================
    // MAPPING CRUD (Link/Unlink)
    // =========================================================================





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
