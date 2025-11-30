//! Custom Operations (Tier 2)
//!
//! This module contains operations that cannot be expressed as data-driven
//! verb definitions. Each custom operation must have a clear rationale for
//! why it requires custom code.
//!
//! ## When to use Custom Operations
//!
//! - External API calls (screening services, AI extraction)
//! - Complex business logic (UBO calculation, graph traversal)
//! - Operations requiring multiple database transactions
//! - Operations with side effects (file I/O, notifications)
//!
//! ## Guidelines
//!
//! 1. Exhaust all options for data-driven verbs first
//! 2. Document WHY this operation requires custom code
//! 3. Keep operations focused and single-purpose
//! 4. Ensure operations are testable in isolation

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use super::ast::VerbCall;
use super::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Trait for custom operations that cannot be expressed as data-driven verbs
#[async_trait]
pub trait CustomOperation: Send + Sync {
    /// Domain this operation belongs to
    fn domain(&self) -> &'static str;

    /// Verb name for this operation
    fn verb(&self) -> &'static str;

    /// Why this operation requires custom code (documentation)
    fn rationale(&self) -> &'static str;

    /// Execute the custom operation
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult>;

    /// Execute without database (for testing)
    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult>;
}

/// Registry for custom operations
pub struct CustomOperationRegistry {
    operations: HashMap<(String, String), Arc<dyn CustomOperation>>,
}

impl CustomOperationRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            operations: HashMap::new(),
        };

        // Register built-in custom operations
        registry.register(Arc::new(EntityCreateOp));
        registry.register(Arc::new(DocumentCatalogOp));
        registry.register(Arc::new(DocumentExtractOp));
        registry.register(Arc::new(UboCalculateOp));
        registry.register(Arc::new(ScreeningPepOp));
        registry.register(Arc::new(ScreeningSanctionsOp));

        // Resource instance operations
        registry.register(Arc::new(ResourceCreateOp));
        registry.register(Arc::new(ResourceSetAttrOp));
        registry.register(Arc::new(ResourceActivateOp));
        registry.register(Arc::new(ResourceSuspendOp));
        registry.register(Arc::new(ResourceDecommissionOp));

        // Service delivery operations
        registry.register(Arc::new(DeliveryRecordOp));
        registry.register(Arc::new(DeliveryCompleteOp));
        registry.register(Arc::new(DeliveryFailOp));

        registry
    }

    /// Register a custom operation
    pub fn register(&mut self, op: Arc<dyn CustomOperation>) {
        let key = (op.domain().to_string(), op.verb().to_string());
        self.operations.insert(key, op);
    }

    /// Get a custom operation by domain and verb
    pub fn get(&self, domain: &str, verb: &str) -> Option<Arc<dyn CustomOperation>> {
        let key = (domain.to_string(), verb.to_string());
        self.operations.get(&key).cloned()
    }

    /// Check if an operation exists
    pub fn has(&self, domain: &str, verb: &str) -> bool {
        let key = (domain.to_string(), verb.to_string());
        self.operations.contains_key(&key)
    }

    /// List all registered custom operations
    pub fn list(&self) -> Vec<(&str, &str, &str)> {
        self.operations
            .values()
            .map(|op| (op.domain(), op.verb(), op.rationale()))
            .collect()
    }
}

impl Default for CustomOperationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Built-in Custom Operations
// ============================================================================

/// Generic entity creation with type dispatch
///
/// Rationale: Maps :type argument (natural-person, limited-company, etc.) to
/// the correct entity_type and extension table. This is a convenience op
/// for agent-generated DSL that uses a single verb with type parameter.
pub struct EntityCreateOp;

#[async_trait]
impl CustomOperation for EntityCreateOp {
    fn domain(&self) -> &'static str {
        "entity"
    }
    fn verb(&self) -> &'static str {
        "create"
    }
    fn rationale(&self) -> &'static str {
        "Requires mapping :type to entity_type and selecting correct extension table"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Extract entity type
        let entity_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("type"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing :type argument"))?;

        // Extract name
        let name = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("name"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing :name argument"))?;

        // Map type string to entity_type_name and extension table
        let (entity_type_name, extension_table) = match entity_type {
            "natural-person" => ("PROPER_PERSON_NATURAL", "entity_proper_persons"),
            "limited-company" => ("LIMITED_COMPANY_PRIVATE", "entity_limited_companies"),
            "partnership" => ("PARTNERSHIP_LIMITED", "entity_partnerships"),
            "trust" => ("TRUST_DISCRETIONARY", "entity_trusts"),
            _ => return Err(anyhow::anyhow!("Unknown entity type: {}", entity_type)),
        };

        // Look up entity type ID
        let type_row = sqlx::query!(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE name = $1"#,
            entity_type_name
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Entity type not found: {}", entity_type_name))?;

        let entity_type_id = type_row.entity_type_id;
        let entity_id = Uuid::new_v4();

        // Insert into base entities table
        sqlx::query!(
            r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, created_at, updated_at)
               VALUES ($1, $2, NOW(), NOW())"#,
            entity_id,
            entity_type_id
        )
        .execute(pool)
        .await?;

        // Insert into extension table based on type
        match extension_table {
            "entity_proper_persons" => {
                // Split name into first/last for proper_persons
                let name_parts: Vec<&str> = name.split_whitespace().collect();
                let (first_name, last_name) = if name_parts.len() >= 2 {
                    (name_parts[0].to_string(), name_parts[1..].join(" "))
                } else {
                    (name.to_string(), "".to_string())
                };

                sqlx::query!(
                    r#"INSERT INTO "ob-poc".entity_proper_persons (entity_id, first_name, last_name)
                       VALUES ($1, $2, $3)"#,
                    entity_id,
                    first_name,
                    last_name
                )
                .execute(pool)
                .await?;
            }
            "entity_limited_companies" => {
                sqlx::query!(
                    r#"INSERT INTO "ob-poc".entity_limited_companies (entity_id, company_name)
                       VALUES ($1, $2)"#,
                    entity_id,
                    name
                )
                .execute(pool)
                .await?;
            }
            "entity_partnerships" => {
                sqlx::query!(
                    r#"INSERT INTO "ob-poc".entity_partnerships (entity_id, partnership_name)
                       VALUES ($1, $2)"#,
                    entity_id,
                    name
                )
                .execute(pool)
                .await?;
            }
            "entity_trusts" => {
                sqlx::query!(
                    r#"INSERT INTO "ob-poc".entity_trusts (entity_id, trust_name)
                       VALUES ($1, $2)"#,
                    entity_id,
                    name
                )
                .execute(pool)
                .await?;
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unknown extension table: {}",
                    extension_table
                ))
            }
        }

        // Bind to context
        ctx.bind("entity", entity_id);

        Ok(ExecutionResult::Uuid(entity_id))
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

/// Document cataloging with document type lookup
///
/// Rationale: Requires lookup of document_type_id from document_types table
/// by type code, then insert into document_catalog with type-specific
/// attribute mappings from document_type_attributes.
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
            .find(|a| a.key.matches("doc-type") || a.key.matches("document-type"))
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
            .find(|a| a.key.matches("title") || a.key.matches("document-name"))
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Get CBU ID if provided (resolve reference if needed)
        let cbu_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Create document - doc_id is the PK in actual schema
        let doc_id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO "ob-poc".document_catalog
               (doc_id, document_type_id, cbu_id, document_name, status)
               VALUES ($1, $2, $3, $4, 'active')"#,
            doc_id,
            doc_type_id,
            cbu_id,
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
            .find(|a| a.key.matches("document-id") || a.key.matches("doc-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
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

/// UBO (Ultimate Beneficial Owner) calculation
///
/// Rationale: Requires recursive graph traversal through ownership chains
/// to identify beneficial owners above the specified threshold.
pub struct UboCalculateOp;

#[async_trait]
impl CustomOperation for UboCalculateOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "calculate"
    }
    fn rationale(&self) -> &'static str {
        "Requires recursive graph traversal through ownership hierarchy"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        use uuid::Uuid;

        // Get CBU ID
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        // Get threshold (default 25%)
        let threshold: f64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("threshold"))
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(25.0))
            .unwrap_or(25.0);

        // First get the primary entity for this CBU
        let cbu_entity = sqlx::query!(
            r#"
            SELECT e.entity_id
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
            JOIN "ob-poc".roles r ON r.role_id = cer.role_id
            WHERE cer.cbu_id = $1
            AND r.name IN ('Primary Entity', 'Main Entity', 'Client')
            LIMIT 1
            "#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?;

        let target_entity_id = match cbu_entity {
            Some(row) => row.entity_id,
            None => {
                // No primary entity found, return empty result
                return Ok(ExecutionResult::RecordSet(vec![]));
            }
        };

        // Query ownership structure using recursive CTE through ownership_relationships
        let ubos = sqlx::query!(
            r#"
            WITH RECURSIVE ownership_chain AS (
                -- Base case: direct owners of the target entity
                SELECT
                    orel.owner_entity_id as entity_id,
                    orel.ownership_percent,
                    ARRAY[orel.owner_entity_id] as path,
                    1 as depth
                FROM "ob-poc".ownership_relationships orel
                WHERE orel.owned_entity_id = $1
                AND orel.ownership_type IN ('DIRECT', 'BENEFICIAL')
                AND (orel.effective_to IS NULL OR orel.effective_to > CURRENT_DATE)

                UNION ALL

                -- Recursive case: owners of owners
                SELECT
                    orel2.owner_entity_id as entity_id,
                    (oc.ownership_percent * orel2.ownership_percent / 100)::numeric(5,2) as ownership_percent,
                    oc.path || orel2.owner_entity_id,
                    oc.depth + 1
                FROM ownership_chain oc
                JOIN "ob-poc".ownership_relationships orel2 ON orel2.owned_entity_id = oc.entity_id
                WHERE oc.depth < 10
                AND NOT orel2.owner_entity_id = ANY(oc.path)
                AND (orel2.effective_to IS NULL OR orel2.effective_to > CURRENT_DATE)
            )
            SELECT
                entity_id,
                SUM(ownership_percent) as total_ownership
            FROM ownership_chain
            GROUP BY entity_id
            HAVING SUM(ownership_percent) >= $2
            ORDER BY total_ownership DESC
            "#,
            target_entity_id,
            sqlx::types::BigDecimal::try_from(threshold).ok()
        )
        .fetch_all(pool)
        .await?;

        // Build result
        let ubo_list: Vec<serde_json::Value> = ubos
            .iter()
            .map(|row| {
                json!({
                    "entity_id": row.entity_id,
                    "ownership_percent": row.total_ownership
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(ubo_list))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}

/// PEP (Politically Exposed Person) screening
///
/// Rationale: Requires external PEP database API call and result processing.
pub struct ScreeningPepOp;

#[async_trait]
impl CustomOperation for ScreeningPepOp {
    fn domain(&self) -> &'static str {
        "screening"
    }
    fn verb(&self) -> &'static str {
        "pep"
    }
    fn rationale(&self) -> &'static str {
        "Requires external PEP screening service API call"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get entity ID
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("entity-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Create screening record
        let screening_id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO "ob-poc".screenings
               (screening_id, screening_type, entity_id, status, result)
               VALUES ($1, 'PEP', $2, 'PENDING', 'PENDING')"#,
            screening_id,
            entity_id
        )
        .execute(pool)
        .await?;

        // TODO: Call external PEP screening API
        // For now, just create the pending screening record

        // In a real implementation, this would:
        // 1. Fetch entity details (name, DOB, nationality)
        // 2. Call PEP screening API
        // 3. Process and store results
        // 4. Update screening status

        ctx.bind("screening", screening_id);

        Ok(ExecutionResult::Uuid(screening_id))
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

/// Sanctions screening
///
/// Rationale: Requires external sanctions database API call and result processing.
pub struct ScreeningSanctionsOp;

#[async_trait]
impl CustomOperation for ScreeningSanctionsOp {
    fn domain(&self) -> &'static str {
        "screening"
    }
    fn verb(&self) -> &'static str {
        "sanctions"
    }
    fn rationale(&self) -> &'static str {
        "Requires external sanctions screening service API call"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get entity ID
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("entity-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Create screening record
        let screening_id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO "ob-poc".screenings
               (screening_id, screening_type, entity_id, status, result)
               VALUES ($1, 'SANCTIONS', $2, 'PENDING', 'PENDING')"#,
            screening_id,
            entity_id
        )
        .execute(pool)
        .await?;

        // TODO: Call external sanctions screening API

        ctx.bind("screening", screening_id);

        Ok(ExecutionResult::Uuid(screening_id))
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

// ============================================================================
// Resource Instance Operations
// ============================================================================

/// Create a resource instance for a CBU
///
/// Rationale: Requires lookup of resource_type_id from prod_resources by code,
/// creates the instance record with proper FK relationships.
pub struct ResourceCreateOp;

#[async_trait]
impl CustomOperation for ResourceCreateOp {
    fn domain(&self) -> &'static str {
        "resource"
    }
    fn verb(&self) -> &'static str {
        "create"
    }
    fn rationale(&self) -> &'static str {
        "Requires resource_type lookup by code and CBU/product/service FK resolution"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get CBU ID (required)
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        // Get resource type code (required)
        let resource_type_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("resource-type"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing resource-type argument"))?;

        // Get instance URL (required)
        let instance_url = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("instance-url"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing instance-url argument"))?;

        // Look up resource type ID
        let resource_type_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT resource_id FROM "ob-poc".prod_resources WHERE resource_code = $1"#,
        )
        .bind(resource_type_code)
        .fetch_optional(pool)
        .await?;

        let resource_type_id = resource_type_id
            .ok_or_else(|| anyhow::anyhow!("Unknown resource type: {}", resource_type_code))?;

        // Get optional arguments
        let instance_identifier = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("instance-id"))
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let instance_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("instance-name"))
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Get product-id if provided
        let product_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("product-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Get service-id if provided
        let service_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("service-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Create instance
        let instance_id = Uuid::new_v4();

        sqlx::query(
            r#"INSERT INTO "ob-poc".cbu_resource_instances
               (instance_id, cbu_id, product_id, service_id, resource_type_id,
                instance_url, instance_identifier, instance_name, status)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'PENDING')"#,
        )
        .bind(instance_id)
        .bind(cbu_id)
        .bind(product_id)
        .bind(service_id)
        .bind(resource_type_id)
        .bind(instance_url)
        .bind(&instance_identifier)
        .bind(&instance_name)
        .execute(pool)
        .await?;

        ctx.bind("instance", instance_id);

        Ok(ExecutionResult::Uuid(instance_id))
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

/// Set an attribute on a resource instance
///
/// Rationale: Requires lookup of attribute_id from dictionary by name,
/// then upsert into resource_instance_attributes with typed value.
pub struct ResourceSetAttrOp;

#[async_trait]
impl CustomOperation for ResourceSetAttrOp {
    fn domain(&self) -> &'static str {
        "resource"
    }
    fn verb(&self) -> &'static str {
        "set-attr"
    }
    fn rationale(&self) -> &'static str {
        "Requires attribute lookup by name and typed value storage"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get instance ID (required)
        let instance_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("instance-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing instance-id argument"))?;

        // Get attribute name (required)
        let attr_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("attr"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing attr argument"))?;

        // Get value (required)
        let value = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("value"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing value argument"))?;

        // Look up attribute ID
        let attribute_id: Option<Uuid> =
            sqlx::query_scalar(r#"SELECT attribute_id FROM "ob-poc".dictionary WHERE name = $1"#)
                .bind(attr_name)
                .fetch_optional(pool)
                .await?;

        let attribute_id =
            attribute_id.ok_or_else(|| anyhow::anyhow!("Unknown attribute: {}", attr_name))?;

        // Get optional state
        let state = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("state"))
            .and_then(|a| a.value.as_string())
            .unwrap_or("proposed");

        // Upsert attribute value (storing as text for simplicity)
        let value_id = Uuid::new_v4();

        sqlx::query(
            r#"INSERT INTO "ob-poc".resource_instance_attributes
               (value_id, instance_id, attribute_id, value_text, state, observed_at)
               VALUES ($1, $2, $3, $4, $5, NOW())
               ON CONFLICT (instance_id, attribute_id) DO UPDATE SET
                   value_text = EXCLUDED.value_text,
                   state = EXCLUDED.state,
                   observed_at = NOW()"#,
        )
        .bind(value_id)
        .bind(instance_id)
        .bind(attribute_id)
        .bind(value)
        .bind(state)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Uuid(value_id))
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

/// Activate a resource instance
///
/// Rationale: Validates required attributes are set before activation.
pub struct ResourceActivateOp;

#[async_trait]
impl CustomOperation for ResourceActivateOp {
    fn domain(&self) -> &'static str {
        "resource"
    }
    fn verb(&self) -> &'static str {
        "activate"
    }
    fn rationale(&self) -> &'static str {
        "Validates required attributes before status transition"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get instance ID
        let instance_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("instance-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing instance-id argument"))?;

        // Get resource type for this instance
        let instance_row: Option<(Option<Uuid>,)> = sqlx::query_as(
            r#"SELECT resource_type_id FROM "ob-poc".cbu_resource_instances WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .fetch_optional(pool)
        .await?;

        let instance_row =
            instance_row.ok_or_else(|| anyhow::anyhow!("Instance not found: {}", instance_id))?;

        // If resource type is set, validate required attributes
        if let Some(resource_type_id) = instance_row.0 {
            // Get required attributes for this resource type
            let required_attrs: Vec<Uuid> = sqlx::query_scalar(
                r#"SELECT attribute_id FROM "ob-poc".resource_attribute_requirements
                   WHERE resource_id = $1 AND is_mandatory = true"#,
            )
            .bind(resource_type_id)
            .fetch_all(pool)
            .await?;

            // Get set attributes for this instance
            let set_attrs: Vec<Uuid> = sqlx::query_scalar(
                r#"SELECT attribute_id FROM "ob-poc".resource_instance_attributes
                   WHERE instance_id = $1"#,
            )
            .bind(instance_id)
            .fetch_all(pool)
            .await?;

            // Check for missing required attributes
            let missing: Vec<_> = required_attrs
                .iter()
                .filter(|a| !set_attrs.contains(a))
                .collect();

            if !missing.is_empty() {
                // Look up attribute names for error message
                let missing_names: Vec<String> = sqlx::query_scalar(
                    r#"SELECT name FROM "ob-poc".dictionary WHERE attribute_id = ANY($1)"#,
                )
                .bind(&missing.iter().map(|u| **u).collect::<Vec<_>>())
                .fetch_all(pool)
                .await?;

                return Err(anyhow::anyhow!(
                    "Cannot activate: missing required attributes: {}",
                    missing_names.join(", ")
                ));
            }
        }

        // Update status to ACTIVE
        sqlx::query(
            r#"UPDATE "ob-poc".cbu_resource_instances
               SET status = 'ACTIVE', activated_at = NOW(), updated_at = NOW()
               WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .execute(pool)
        .await?;

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

/// Suspend a resource instance
pub struct ResourceSuspendOp;

#[async_trait]
impl CustomOperation for ResourceSuspendOp {
    fn domain(&self) -> &'static str {
        "resource"
    }
    fn verb(&self) -> &'static str {
        "suspend"
    }
    fn rationale(&self) -> &'static str {
        "Status transition with optional reason logging"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let instance_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("instance-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing instance-id argument"))?;

        sqlx::query(
            r#"UPDATE "ob-poc".cbu_resource_instances
               SET status = 'SUSPENDED', updated_at = NOW()
               WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .execute(pool)
        .await?;

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

/// Decommission a resource instance
pub struct ResourceDecommissionOp;

#[async_trait]
impl CustomOperation for ResourceDecommissionOp {
    fn domain(&self) -> &'static str {
        "resource"
    }
    fn verb(&self) -> &'static str {
        "decommission"
    }
    fn rationale(&self) -> &'static str {
        "Terminal status transition with timestamp"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let instance_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("instance-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing instance-id argument"))?;

        sqlx::query(
            r#"UPDATE "ob-poc".cbu_resource_instances
               SET status = 'DECOMMISSIONED', decommissioned_at = NOW(), updated_at = NOW()
               WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .execute(pool)
        .await?;

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

// ============================================================================
// Service Delivery Operations
// ============================================================================

/// Record a service delivery for a CBU
pub struct DeliveryRecordOp;

#[async_trait]
impl CustomOperation for DeliveryRecordOp {
    fn domain(&self) -> &'static str {
        "delivery"
    }
    fn verb(&self) -> &'static str {
        "record"
    }
    fn rationale(&self) -> &'static str {
        "Requires product/service code lookup and upsert logic"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get CBU ID
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        // Get product code and look up ID
        let product_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("product"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing product argument"))?;

        let product_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT product_id FROM "ob-poc".products WHERE product_code = $1"#,
        )
        .bind(product_code)
        .fetch_optional(pool)
        .await?;

        let product_id =
            product_id.ok_or_else(|| anyhow::anyhow!("Unknown product: {}", product_code))?;

        // Get service code and look up ID
        let service_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("service"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing service argument"))?;

        let service_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT service_id FROM "ob-poc".services WHERE service_code = $1"#,
        )
        .bind(service_code)
        .fetch_optional(pool)
        .await?;

        let service_id =
            service_id.ok_or_else(|| anyhow::anyhow!("Unknown service: {}", service_code))?;

        // Get optional instance-id
        let instance_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("instance-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Create delivery record
        let delivery_id = Uuid::new_v4();

        sqlx::query(
            r#"INSERT INTO "ob-poc".service_delivery_map
               (delivery_id, cbu_id, product_id, service_id, instance_id, delivery_status)
               VALUES ($1, $2, $3, $4, $5, 'PENDING')
               ON CONFLICT (cbu_id, product_id, service_id) DO UPDATE SET
                   instance_id = EXCLUDED.instance_id,
                   updated_at = NOW()"#,
        )
        .bind(delivery_id)
        .bind(cbu_id)
        .bind(product_id)
        .bind(service_id)
        .bind(instance_id)
        .execute(pool)
        .await?;

        ctx.bind("delivery", delivery_id);

        Ok(ExecutionResult::Uuid(delivery_id))
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

/// Mark service delivery as complete
pub struct DeliveryCompleteOp;

#[async_trait]
impl CustomOperation for DeliveryCompleteOp {
    fn domain(&self) -> &'static str {
        "delivery"
    }
    fn verb(&self) -> &'static str {
        "complete"
    }
    fn rationale(&self) -> &'static str {
        "Updates delivery status with timestamp"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let product_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("product"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing product argument"))?;

        let service_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("service"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing service argument"))?;

        // Look up product and service IDs
        let product_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT product_id FROM "ob-poc".products WHERE product_code = $1"#,
        )
        .bind(product_code)
        .fetch_optional(pool)
        .await?;

        let product_id =
            product_id.ok_or_else(|| anyhow::anyhow!("Unknown product: {}", product_code))?;

        let service_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT service_id FROM "ob-poc".services WHERE service_code = $1"#,
        )
        .bind(service_code)
        .fetch_optional(pool)
        .await?;

        let service_id =
            service_id.ok_or_else(|| anyhow::anyhow!("Unknown service: {}", service_code))?;

        sqlx::query(
            r#"UPDATE "ob-poc".service_delivery_map
               SET delivery_status = 'DELIVERED', delivered_at = NOW(), updated_at = NOW()
               WHERE cbu_id = $1 AND product_id = $2 AND service_id = $3"#,
        )
        .bind(cbu_id)
        .bind(product_id)
        .bind(service_id)
        .execute(pool)
        .await?;

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

/// Mark service delivery as failed
pub struct DeliveryFailOp;

#[async_trait]
impl CustomOperation for DeliveryFailOp {
    fn domain(&self) -> &'static str {
        "delivery"
    }
    fn verb(&self) -> &'static str {
        "fail"
    }
    fn rationale(&self) -> &'static str {
        "Updates delivery status with failure reason"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let product_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("product"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing product argument"))?;

        let service_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("service"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing service argument"))?;

        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("reason"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing reason argument"))?;

        // Look up product and service IDs
        let product_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT product_id FROM "ob-poc".products WHERE product_code = $1"#,
        )
        .bind(product_code)
        .fetch_optional(pool)
        .await?;

        let product_id =
            product_id.ok_or_else(|| anyhow::anyhow!("Unknown product: {}", product_code))?;

        let service_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT service_id FROM "ob-poc".services WHERE service_code = $1"#,
        )
        .bind(service_code)
        .fetch_optional(pool)
        .await?;

        let service_id =
            service_id.ok_or_else(|| anyhow::anyhow!("Unknown service: {}", service_code))?;

        sqlx::query(
            r#"UPDATE "ob-poc".service_delivery_map
               SET delivery_status = 'FAILED', failed_at = NOW(), failure_reason = $4, updated_at = NOW()
               WHERE cbu_id = $1 AND product_id = $2 AND service_id = $3"#,
        )
        .bind(cbu_id)
        .bind(product_id)
        .bind(service_id)
        .bind(reason)
        .execute(pool)
        .await?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = CustomOperationRegistry::new();
        assert!(registry.has("document", "catalog"));
        assert!(registry.has("document", "extract"));
        assert!(registry.has("ubo", "calculate"));
        assert!(registry.has("screening", "pep"));
        assert!(registry.has("screening", "sanctions"));
        // New resource operations
        assert!(registry.has("resource", "create"));
        assert!(registry.has("resource", "set-attr"));
        assert!(registry.has("resource", "activate"));
        assert!(registry.has("resource", "suspend"));
        assert!(registry.has("resource", "decommission"));
        // New delivery operations
        assert!(registry.has("delivery", "record"));
        assert!(registry.has("delivery", "complete"));
        assert!(registry.has("delivery", "fail"));
    }

    #[test]
    fn test_registry_list() {
        let registry = CustomOperationRegistry::new();
        let ops = registry.list();
        assert_eq!(ops.len(), 14); // 6 original + 5 resource + 3 delivery
    }
}
