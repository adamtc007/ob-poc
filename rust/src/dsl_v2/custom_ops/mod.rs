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

mod custody;
mod onboarding;
mod rfi;
mod threshold;
mod ubo_analysis;

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use super::ast::VerbCall;
use super::executor::{ExecutionContext, ExecutionResult};

pub use custody::{
    DeriveRequiredCoverageOp, LookupSsiForTradeOp, SetupSsiFromDocumentOp, SubcustodianLookupOp,
    ValidateBookingCoverageOp,
};
pub use onboarding::{
    OnboardingEnsureOp, OnboardingExecuteOp, OnboardingGetUrlsOp, OnboardingPlanOp,
    OnboardingShowPlanOp, OnboardingStatusOp,
};
pub use rfi::{RfiCheckCompletionOp, RfiGenerateOp, RfiListByCaseOp};
pub use threshold::{ThresholdCheckEntityOp, ThresholdDeriveOp, ThresholdEvaluateOp};
pub use ubo_analysis::{
    UboCheckCompletenessOp, UboCompareSnapshotOp, UboDiscoverOwnerOp, UboInferChainOp,
    UboSnapshotCbuOp, UboSupersedeOp, UboTraceChainsOp,
};

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
        registry.register(Arc::new(ScreeningAdverseMediaOp));

        // Resource instance operations
        registry.register(Arc::new(ResourceCreateOp));
        registry.register(Arc::new(ResourceSetAttrOp));
        registry.register(Arc::new(ResourceActivateOp));
        registry.register(Arc::new(ResourceSuspendOp));
        registry.register(Arc::new(ResourceDecommissionOp));
        registry.register(Arc::new(ResourceValidateAttrsOp));

        // CBU operations
        registry.register(Arc::new(CbuAddProductOp));
        registry.register(Arc::new(CbuShowOp));
        registry.register(Arc::new(CbuDecideOp));

        // NOTE: delivery.record, delivery.complete, delivery.fail are now CRUD verbs
        // defined in config/verbs/delivery.yaml - no plugin needed

        // Custody operations
        registry.register(Arc::new(SubcustodianLookupOp));
        registry.register(Arc::new(LookupSsiForTradeOp));
        registry.register(Arc::new(ValidateBookingCoverageOp));
        registry.register(Arc::new(DeriveRequiredCoverageOp));
        registry.register(Arc::new(SetupSsiFromDocumentOp));

        // Observation operations
        registry.register(Arc::new(ObservationFromDocumentOp));
        registry.register(Arc::new(ObservationGetCurrentOp));
        registry.register(Arc::new(ObservationReconcileOp));
        registry.register(Arc::new(ObservationVerifyAllegationsOp));

        // Document extraction to observations
        registry.register(Arc::new(DocumentExtractObservationsOp));

        // Threshold operations (Phase 2)
        registry.register(Arc::new(ThresholdDeriveOp));
        registry.register(Arc::new(ThresholdEvaluateOp));
        registry.register(Arc::new(ThresholdCheckEntityOp));

        // RFI operations (Phase 3) - works with existing kyc.doc_requests
        registry.register(Arc::new(RfiGenerateOp));
        registry.register(Arc::new(RfiCheckCompletionOp));
        registry.register(Arc::new(RfiListByCaseOp));

        // UBO Analysis operations (Phase 4)
        registry.register(Arc::new(UboDiscoverOwnerOp));
        registry.register(Arc::new(UboInferChainOp));
        registry.register(Arc::new(UboTraceChainsOp));
        registry.register(Arc::new(UboCheckCompletenessOp));
        registry.register(Arc::new(UboSupersedeOp));
        registry.register(Arc::new(UboSnapshotCbuOp));
        registry.register(Arc::new(UboCompareSnapshotOp));

        // Onboarding operations (Terraform-like resource provisioning with dependencies)
        registry.register(Arc::new(OnboardingPlanOp));
        registry.register(Arc::new(OnboardingShowPlanOp));
        registry.register(Arc::new(OnboardingExecuteOp));
        registry.register(Arc::new(OnboardingStatusOp));
        registry.register(Arc::new(OnboardingGetUrlsOp));
        registry.register(Arc::new(OnboardingEnsureOp));

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

/// Generic entity creation with type dispatch (Idempotent)
///
/// Rationale: Maps :type argument (natural-person, limited-company, etc.) to
/// the correct entity_type and extension table. This is a convenience op
/// for agent-generated DSL that uses a single verb with type parameter.
///
/// Idempotency: Checks for existing entity with same name in extension table
/// before creating. Returns existing entity_id if found.
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
            .find(|a| a.key == "type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing :type argument"))?;

        // Extract name
        let name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "name")
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

        // Idempotency: Check for existing entity with same name
        // For proper_persons, we split name and check first_name + last_name
        // For companies/partnerships/trusts, we check the name column directly
        let existing_entity_id: Option<Uuid> = match extension_table {
            "entity_proper_persons" => {
                let name_parts: Vec<&str> = name.split_whitespace().collect();
                let (first_name, last_name) = if name_parts.len() >= 2 {
                    (name_parts[0].to_string(), name_parts[1..].join(" "))
                } else {
                    (name.to_string(), "".to_string())
                };
                sqlx::query_scalar(
                    r#"SELECT entity_id FROM "ob-poc".entity_proper_persons
                       WHERE first_name = $1 AND last_name = $2
                       LIMIT 1"#,
                )
                .bind(&first_name)
                .bind(&last_name)
                .fetch_optional(pool)
                .await?
            }
            "entity_limited_companies" => {
                sqlx::query_scalar(
                    r#"SELECT entity_id FROM "ob-poc".entity_limited_companies
                       WHERE company_name = $1
                       LIMIT 1"#,
                )
                .bind(name)
                .fetch_optional(pool)
                .await?
            }
            "entity_partnerships" => {
                sqlx::query_scalar(
                    r#"SELECT entity_id FROM "ob-poc".entity_partnerships
                       WHERE partnership_name = $1
                       LIMIT 1"#,
                )
                .bind(name)
                .fetch_optional(pool)
                .await?
            }
            "entity_trusts" => {
                sqlx::query_scalar(
                    r#"SELECT entity_id FROM "ob-poc".entity_trusts
                       WHERE trust_name = $1
                       LIMIT 1"#,
                )
                .bind(name)
                .fetch_optional(pool)
                .await?
            }
            _ => None,
        };

        // If entity already exists, return existing ID
        if let Some(existing_id) = existing_entity_id {
            ctx.bind("entity", existing_id);
            return Ok(ExecutionResult::Uuid(existing_id));
        }

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

/// Document cataloging with document type lookup (Idempotent)
///
/// Rationale: Requires lookup of document_type_id from document_types table
/// by type code, then insert into document_catalog with type-specific
/// attribute mappings from document_type_attributes.
///
/// Idempotency: Uses ON CONFLICT on (cbu_id, document_type_id, document_name)
/// to return existing document if already cataloged.
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
            .find(|a| a.key == "doc-type" || a.key == "document-type")
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
            .find(|a| a.key == "title" || a.key == "document-name")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Get CBU ID if provided (resolve reference if needed)
        let cbu_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Get Entity ID if provided (resolve reference if needed)
        let entity_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Idempotent: Check for existing document with same cbu_id, document_type_id, and document_name
        let existing = sqlx::query!(
            r#"SELECT doc_id FROM "ob-poc".document_catalog
               WHERE cbu_id IS NOT DISTINCT FROM $1
               AND document_type_id = $2
               AND document_name IS NOT DISTINCT FROM $3
               LIMIT 1"#,
            cbu_id,
            doc_type_id,
            document_name
        )
        .fetch_optional(pool)
        .await?;

        if let Some(row) = existing {
            ctx.bind("document", row.doc_id);
            return Ok(ExecutionResult::Uuid(row.doc_id));
        }

        // Create new document
        let doc_id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO "ob-poc".document_catalog
               (doc_id, document_type_id, cbu_id, entity_id, document_name, status)
               VALUES ($1, $2, $3, $4, $5, 'active')"#,
            doc_id,
            doc_type_id,
            cbu_id,
            entity_id,
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
            .find(|a| a.key == "document-id" || a.key == "doc-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
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
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
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
            .find(|a| a.key == "threshold")
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

/// PEP (Politically Exposed Person) screening (Idempotent)
///
/// Rationale: Requires external PEP database API call and result processing.
/// Idempotency: Returns existing pending PEP screening for same entity if exists.
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
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Screenings now require a workstream_id (kyc.screenings table)
        // This verb should be called via case-screening.initiate which has workstream context
        // For backwards compatibility, check if there's an active workstream for this entity
        let workstream = sqlx::query!(
            r#"SELECT w.workstream_id FROM kyc.entity_workstreams w
               JOIN kyc.cases c ON c.case_id = w.case_id
               WHERE w.entity_id = $1 AND w.status NOT IN ('COMPLETE', 'BLOCKED')
               ORDER BY w.created_at DESC
               LIMIT 1"#,
            entity_id
        )
        .fetch_optional(pool)
        .await?;

        let workstream_id = match workstream {
            Some(row) => row.workstream_id,
            None => {
                // No active workstream - return error
                return Err(anyhow::anyhow!(
                    "No active workstream for entity. Use case-screening.initiate instead."
                ));
            }
        };

        // Check for existing pending screening
        let existing = sqlx::query!(
            r#"SELECT screening_id FROM kyc.screenings
               WHERE workstream_id = $1 AND screening_type = 'PEP' AND status = 'PENDING'
               LIMIT 1"#,
            workstream_id
        )
        .fetch_optional(pool)
        .await?;

        if let Some(row) = existing {
            ctx.bind("screening", row.screening_id);
            return Ok(ExecutionResult::Uuid(row.screening_id));
        }

        // Create screening record
        let screening_id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO kyc.screenings
               (screening_id, workstream_id, screening_type, status)
               VALUES ($1, $2, 'PEP', 'PENDING')"#,
            screening_id,
            workstream_id
        )
        .execute(pool)
        .await?;

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

/// Sanctions screening (Idempotent)
///
/// Rationale: Requires external sanctions database API call and result processing.
/// Idempotency: Returns existing pending sanctions screening for same entity if exists.
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
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Screenings now require a workstream_id (kyc.screenings table)
        // This verb should be called via case-screening.initiate which has workstream context
        // For backwards compatibility, check if there's an active workstream for this entity
        let workstream = sqlx::query!(
            r#"SELECT w.workstream_id FROM kyc.entity_workstreams w
               JOIN kyc.cases c ON c.case_id = w.case_id
               WHERE w.entity_id = $1 AND w.status NOT IN ('COMPLETE', 'BLOCKED')
               ORDER BY w.created_at DESC
               LIMIT 1"#,
            entity_id
        )
        .fetch_optional(pool)
        .await?;

        let workstream_id = match workstream {
            Some(row) => row.workstream_id,
            None => {
                // No active workstream - return error
                return Err(anyhow::anyhow!(
                    "No active workstream for entity. Use case-screening.initiate instead."
                ));
            }
        };

        // Check for existing pending screening
        let existing = sqlx::query!(
            r#"SELECT screening_id FROM kyc.screenings
               WHERE workstream_id = $1 AND screening_type = 'SANCTIONS' AND status = 'PENDING'
               LIMIT 1"#,
            workstream_id
        )
        .fetch_optional(pool)
        .await?;

        if let Some(row) = existing {
            ctx.bind("screening", row.screening_id);
            return Ok(ExecutionResult::Uuid(row.screening_id));
        }

        // Create screening record
        let screening_id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO kyc.screenings
               (screening_id, workstream_id, screening_type, status)
               VALUES ($1, $2, 'SANCTIONS', 'PENDING')"#,
            screening_id,
            workstream_id
        )
        .execute(pool)
        .await?;

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

/// Adverse media screening (Not Implemented)
///
/// Rationale: Requires external adverse media API call and result processing.
/// Status: Stub - returns error indicating not implemented.
pub struct ScreeningAdverseMediaOp;

#[async_trait]
impl CustomOperation for ScreeningAdverseMediaOp {
    fn domain(&self) -> &'static str {
        "screening"
    }
    fn verb(&self) -> &'static str {
        "adverse-media"
    }
    fn rationale(&self) -> &'static str {
        "Requires external adverse media screening service API call"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "screening.adverse-media is not yet implemented"
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "screening.adverse-media is not yet implemented"
        ))
    }
}

// ============================================================================
// Resource Instance Operations
// ============================================================================

/// Create a resource instance for a CBU (Idempotent)
///
/// Rationale: Requires lookup of resource_type_id from service_resource_types by code,
/// creates the instance record with proper FK relationships.
///
/// Idempotency: Uses ON CONFLICT on (instance_url) or (cbu_id, resource_type_id, instance_identifier)
/// to return existing instance if already created.
pub struct ResourceCreateOp;

#[async_trait]
impl CustomOperation for ResourceCreateOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "provision"
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
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
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
            .find(|a| a.key == "resource-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing resource-type argument"))?;

        // Get instance URL (optional - auto-generate if not provided)
        let instance_url = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instance-url")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Auto-generate URN: urn:ob-poc:{cbu_id}:{resource_type}:{uuid}
                format!(
                    "urn:ob-poc:{}:{}:{}",
                    cbu_id,
                    resource_type_code.to_lowercase().replace('_', "-"),
                    Uuid::new_v4()
                )
            });

        // Look up resource type ID
        let resource_type_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT resource_id FROM "ob-poc".service_resource_types WHERE resource_code = $1"#,
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
            .find(|a| a.key == "instance-id")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let instance_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instance-name")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Get product-id if provided
        let product_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "product-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Get service-id if provided, otherwise auto-derive from resource type capabilities
        let mut service_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "service-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Auto-derive service_id from service_resource_capabilities if not provided
        if service_id.is_none() {
            // Look up service(s) that support this resource type
            let services: Vec<Uuid> = sqlx::query_scalar(
                r#"SELECT service_id FROM "ob-poc".service_resource_capabilities
                   WHERE resource_id = $1 AND is_active = true
                   ORDER BY priority ASC
                   LIMIT 1"#,
            )
            .bind(resource_type_id)
            .fetch_all(pool)
            .await?;

            // Use the first (highest priority) service if available
            service_id = services.into_iter().next();
        }

        // Extract depends-on references (list of @symbol refs to other instances)
        let depends_on_refs: Vec<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "depends-on")
            .and_then(|a| a.value.as_list())
            .map(|list| {
                list.iter()
                    .filter_map(|node| {
                        if let Some(sym) = node.as_symbol() {
                            ctx.resolve(sym)
                        } else {
                            node.as_uuid()
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Idempotent: INSERT or return existing using instance_url as conflict key
        let instance_id = Uuid::new_v4();

        let row: (Uuid,) = sqlx::query_as(
            r#"WITH ins AS (
                INSERT INTO "ob-poc".cbu_resource_instances
                (instance_id, cbu_id, product_id, service_id, resource_type_id,
                 instance_url, instance_identifier, instance_name, status)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'PENDING')
                ON CONFLICT (instance_url) DO NOTHING
                RETURNING instance_id
            )
            SELECT instance_id FROM ins
            UNION ALL
            SELECT instance_id FROM "ob-poc".cbu_resource_instances
            WHERE instance_url = $6
            AND NOT EXISTS (SELECT 1 FROM ins)
            LIMIT 1"#,
        )
        .bind(instance_id)
        .bind(cbu_id)
        .bind(product_id)
        .bind(service_id)
        .bind(resource_type_id)
        .bind(instance_url)
        .bind(&instance_identifier)
        .bind(&instance_name)
        .fetch_one(pool)
        .await?;

        let result_id = row.0;

        // Record instance dependencies if any were specified
        for dep_instance_id in depends_on_refs {
            sqlx::query(
                r#"INSERT INTO "ob-poc".resource_instance_dependencies
                   (instance_id, depends_on_instance_id)
                   VALUES ($1, $2)
                   ON CONFLICT DO NOTHING"#,
            )
            .bind(result_id)
            .bind(dep_instance_id)
            .execute(pool)
            .await?;
        }

        ctx.bind("instance", result_id);

        Ok(ExecutionResult::Uuid(result_id))
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
        "service-resource"
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
            .find(|a| a.key == "instance-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
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
            .find(|a| a.key == "attr")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing attr argument"))?;

        // Get value (required)
        let value = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "value")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing value argument"))?;

        // Look up attribute ID from unified attribute_registry
        let attribute_id: Option<Uuid> =
            sqlx::query_scalar(r#"SELECT uuid FROM "ob-poc".attribute_registry WHERE name = $1"#)
                .bind(attr_name)
                .fetch_optional(pool)
                .await?;

        let attribute_id =
            attribute_id.ok_or_else(|| anyhow::anyhow!("Unknown attribute: {}", attr_name))?;

        // Get optional state
        let state = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "state")
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
        "service-resource"
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
            .find(|a| a.key == "instance-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
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
                let missing_uuids: Vec<Uuid> = missing.iter().map(|u| **u).collect();
                let missing_names: Vec<String> = sqlx::query_scalar(
                    r#"SELECT name FROM "ob-poc".attribute_registry WHERE uuid = ANY($1)"#,
                )
                .bind(missing_uuids)
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
        "service-resource"
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
            .find(|a| a.key == "instance-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
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
        "service-resource"
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
            .find(|a| a.key == "instance-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
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

/// Validate that all required attributes are set for a resource instance
pub struct ResourceValidateAttrsOp;

#[async_trait]
impl CustomOperation for ResourceValidateAttrsOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "validate-attrs"
    }
    fn rationale(&self) -> &'static str {
        "Validates required attributes against resource_attribute_requirements"
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
            .find(|a| a.key == "instance-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing instance-id argument"))?;

        // Get resource type for this instance
        let resource_type_id: Option<Option<Uuid>> = sqlx::query_scalar(
            r#"SELECT resource_type_id FROM "ob-poc".cbu_resource_instances WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .fetch_optional(pool)
        .await?;

        let resource_type_id = match resource_type_id.and_then(|r| r) {
            Some(id) => id,
            None => {
                // No resource type = nothing to validate
                return Ok(ExecutionResult::Record(serde_json::json!({
                    "valid": true,
                    "missing": [],
                    "message": "No resource type defined, skipping validation"
                })));
            }
        };

        // Get required attributes for this resource type
        let required_attrs: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT rar.attribute_id, ar.name
               FROM "ob-poc".resource_attribute_requirements rar
               JOIN "ob-poc".attribute_registry ar ON rar.attribute_id = ar.uuid
               WHERE rar.resource_id = $1 AND rar.is_mandatory = true
               ORDER BY rar.display_order"#,
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

        // Find missing required attributes
        let missing: Vec<String> = required_attrs
            .iter()
            .filter(|(id, _)| !set_attrs.contains(id))
            .map(|(_, name)| name.clone())
            .collect();

        let valid = missing.is_empty();
        let result = serde_json::json!({
            "valid": valid,
            "missing": missing,
            "message": if valid {
                "All required attributes are set".to_string()
            } else {
                format!("Missing {} required attribute(s)", missing.len())
            }
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "valid": true,
            "missing": [],
            "message": "Validation skipped (no database)"
        })))
    }
}

// ============================================================================
// CBU Product Assignment
// ============================================================================

/// Add a product to a CBU by creating service_delivery_map and cbu_resource_instances entries
///
/// This is a CRITICAL onboarding operation that:
/// 1. Validates CBU exists
/// 2. Looks up product by name and validates it exists
/// 3. Validates product has services defined
/// 4. Creates service_delivery_map entries for ALL services under that product
/// 5. Creates cbu_resource_instances for ALL resource types under each service
///    (via service_resource_capabilities join) - one per (CBU, resource_type)
///
/// NOTE: A CBU can have MULTIPLE products. This verb adds one product at a time.
/// The service_delivery_map is the source of truth for CBU->Product relationships.
/// cbus.product_id is NOT used (legacy field).
///
/// Idempotency: Safe to re-run - uses ON CONFLICT DO NOTHING for all entries
/// Transaction: All operations wrapped in a transaction for atomicity
pub struct CbuAddProductOp;

#[async_trait]
impl CustomOperation for CbuAddProductOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }
    fn verb(&self) -> &'static str {
        "add-product"
    }
    fn rationale(&self) -> &'static str {
        "Critical onboarding op: links CBU to product and creates service delivery entries"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // =====================================================================
        // Step 1: Extract and validate arguments
        // =====================================================================
        // cbu-id can be: @reference, UUID string, or CBU name string
        let cbu_id_arg = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .ok_or_else(|| anyhow::anyhow!("cbu.add-product: Missing required argument :cbu-id"))?;

        let product_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "product")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| {
                anyhow::anyhow!("cbu.add-product: Missing required argument :product")
            })?;

        // =====================================================================
        // Step 2: Resolve CBU - by reference, UUID, or name
        // =====================================================================
        let (cbu_id, cbu_name): (Uuid, String) =
            if let Some(ref_name) = cbu_id_arg.value.as_symbol() {
                // It's a @reference - resolve from context
                let resolved_id = ctx.resolve(ref_name).ok_or_else(|| {
                    anyhow::anyhow!("cbu.add-product: Unresolved reference @{}", ref_name)
                })?;
                let row = sqlx::query!(
                    r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
                    resolved_id
                )
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("cbu.add-product: CBU not found with id {}", resolved_id)
                })?;
                (row.cbu_id, row.name)
            } else if let Some(uuid_val) = cbu_id_arg.value.as_uuid() {
                // It's a UUID
                let row = sqlx::query!(
                    r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
                    uuid_val
                )
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("cbu.add-product: CBU not found with id {}", uuid_val)
                })?;
                (row.cbu_id, row.name)
            } else if let Some(str_val) = cbu_id_arg.value.as_string() {
                // It's a string - try as UUID first, then as name
                if let Ok(uuid_val) = Uuid::parse_str(str_val) {
                    let row = sqlx::query!(
                        r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
                        uuid_val
                    )
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| {
                        anyhow::anyhow!("cbu.add-product: CBU not found with id {}", uuid_val)
                    })?;
                    (row.cbu_id, row.name)
                } else {
                    // Look up by name (case-insensitive)
                    let row = sqlx::query!(
                        r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE LOWER(name) = LOWER($1)"#,
                        str_val
                    )
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                        "cbu.add-product: CBU '{}' not found. Use cbu.list to see available CBUs.",
                        str_val
                    )
                    })?;
                    (row.cbu_id, row.name)
                }
            } else {
                return Err(anyhow::anyhow!(
                    "cbu.add-product: :cbu-id must be a @reference, UUID, or CBU name string"
                ));
            };

        // Note: We don't touch cbus.product_id - service_delivery_map is source of truth

        // =====================================================================
        // Step 3: Validate product exists and get its ID (lookup by product_code)
        // =====================================================================
        let product_row = sqlx::query!(
            r#"SELECT product_id, name, product_code FROM "ob-poc".products WHERE product_code = $1"#,
            product_name
        )
        .fetch_optional(pool)
        .await?;

        let product = product_row.ok_or_else(|| {
            anyhow::anyhow!(
                "cbu.add-product: Product '{}' not found. Use product codes: CUSTODY, FUND_ACCOUNTING, TRANSFER_AGENCY, MIDDLE_OFFICE, COLLATERAL_MGMT, MARKETS_FX, ALTS",
                product_name
            )
        })?;

        let product_id = product.product_id;

        // =====================================================================
        // Step 4: Get all services for this product
        // =====================================================================
        let services = sqlx::query!(
            r#"SELECT ps.service_id, s.name as service_name
               FROM "ob-poc".product_services ps
               JOIN "ob-poc".services s ON ps.service_id = s.service_id
               WHERE ps.product_id = $1
               ORDER BY s.name"#,
            product_id
        )
        .fetch_all(pool)
        .await?;

        if services.is_empty() {
            return Err(anyhow::anyhow!(
                "cbu.add-product: Product '{}' has no services defined in product_services. \
                 Cannot add product without services.",
                product_name
            ));
        }

        // =====================================================================
        // Step 5: Execute in transaction
        // =====================================================================
        let mut tx = pool.begin().await?;

        // 5a: Create service_delivery_map entries for each service
        let mut delivery_created: i64 = 0;
        let mut delivery_skipped: i64 = 0;

        for svc in &services {
            let delivery_id = Uuid::new_v4();
            let result = sqlx::query(
                r#"INSERT INTO "ob-poc".service_delivery_map
                   (delivery_id, cbu_id, product_id, service_id, delivery_status)
                   VALUES ($1, $2, $3, $4, 'PENDING')
                   ON CONFLICT (cbu_id, product_id, service_id) DO NOTHING"#,
            )
            .bind(delivery_id)
            .bind(cbu_id)
            .bind(product_id)
            .bind(svc.service_id)
            .execute(&mut *tx)
            .await?;

            if result.rows_affected() > 0 {
                delivery_created += 1;
            } else {
                delivery_skipped += 1;
            }
        }

        // =====================================================================
        // Step 5b: Create cbu_resource_instances for each service's resource types
        // =====================================================================
        let mut resource_created: i64 = 0;
        let mut resource_skipped: i64 = 0;

        // Get all (service, resource_type) pairs for this product
        // Each service-resource combination gets its own instance
        // e.g., SWIFT Connection under Trade Settlement is separate from SWIFT Connection under Income Collection
        let service_resources = sqlx::query!(
            r#"SELECT src.service_id, src.resource_id, srt.resource_code, srt.name as resource_name
               FROM "ob-poc".service_resource_capabilities src
               JOIN "ob-poc".service_resource_types srt ON src.resource_id = srt.resource_id
               WHERE src.service_id IN (
                   SELECT service_id FROM "ob-poc".product_services WHERE product_id = $1
               )
               AND src.is_active = true
               ORDER BY src.service_id, srt.name"#,
            product_id
        )
        .fetch_all(&mut *tx)
        .await?;

        for sr in &service_resources {
            let instance_id = Uuid::new_v4();
            // Generate a unique instance URL using CBU name, resource code, and partial UUID
            let instance_url = format!(
                "urn:ob-poc:{}:{}:{}",
                cbu_name.to_lowercase().replace(' ', "-"),
                sr.resource_code.as_deref().unwrap_or("unknown"),
                &instance_id.to_string()[..8]
            );

            // Unique key is (cbu_id, product_id, service_id, resource_type_id)
            // One resource instance per service per resource type
            let result = sqlx::query(
                r#"INSERT INTO "ob-poc".cbu_resource_instances
                   (instance_id, cbu_id, product_id, service_id, resource_type_id,
                    instance_url, instance_name, status)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, 'PENDING')
                   ON CONFLICT (cbu_id, product_id, service_id, resource_type_id) DO NOTHING"#,
            )
            .bind(instance_id)
            .bind(cbu_id)
            .bind(product_id)
            .bind(sr.service_id)
            .bind(sr.resource_id)
            .bind(&instance_url)
            .bind(&sr.resource_name)
            .execute(&mut *tx)
            .await?;

            if result.rows_affected() > 0 {
                resource_created += 1;
            } else {
                resource_skipped += 1;
            }
        }

        // Commit transaction
        tx.commit().await?;

        // =====================================================================
        // Step 6: Log result for debugging
        // =====================================================================
        tracing::info!(
            cbu_id = %cbu_id,
            cbu_name = %cbu_name,
            product = %product_name,
            services_total = services.len(),
            delivery_entries_created = delivery_created,
            delivery_entries_skipped = delivery_skipped,
            resource_instances_created = resource_created,
            resource_instances_skipped = resource_skipped,
            "cbu.add-product completed"
        );

        // Return total entries created (deliveries + resources)
        Ok(ExecutionResult::Affected(
            (delivery_created + resource_created) as u64,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(0))
    }
}

// ============================================================================
// CBU Show Operation
// ============================================================================

/// Show full CBU structure including entities, roles, documents, screenings
///
/// Rationale: Requires multiple joins across CBU, entities, roles, documents,
/// screenings, and service deliveries to build a complete picture.
pub struct CbuShowOp;

#[async_trait]
impl CustomOperation for CbuShowOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }
    fn verb(&self) -> &'static str {
        "show"
    }
    fn rationale(&self) -> &'static str {
        "Requires aggregating data from multiple tables into a structured view"
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
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else if let Some(uuid_val) = a.value.as_uuid() {
                    Some(uuid_val)
                } else if let Some(str_val) = a.value.as_string() {
                    Uuid::parse_str(str_val).ok()
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid cbu-id argument"))?;

        // Get basic CBU info
        let cbu = sqlx::query!(
            r#"SELECT cbu_id, name, jurisdiction, client_type, cbu_category,
                      nature_purpose, description, created_at, updated_at
               FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        // Get entities with their roles
        let entities = sqlx::query!(
            r#"SELECT DISTINCT e.entity_id, e.name, et.type_code as entity_type,
                      COALESCE(lc.jurisdiction, pp.nationality, p.jurisdiction, t.jurisdiction) as jurisdiction
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
               LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
               LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
               LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
               WHERE cer.cbu_id = $1
               ORDER BY e.name"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        // Get roles per entity
        let roles = sqlx::query!(
            r#"SELECT cer.entity_id, r.name as role_name
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               WHERE cer.cbu_id = $1
               ORDER BY cer.entity_id, r.name"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        // Build entity list with roles
        let entity_list: Vec<serde_json::Value> = entities
            .iter()
            .map(|e| {
                let entity_roles: Vec<String> = roles
                    .iter()
                    .filter(|r| r.entity_id == e.entity_id)
                    .map(|r| r.role_name.clone())
                    .collect();
                serde_json::json!({
                    "entity_id": e.entity_id,
                    "name": e.name,
                    "entity_type": e.entity_type,
                    "jurisdiction": e.jurisdiction,
                    "roles": entity_roles
                })
            })
            .collect();

        // Get documents
        let documents = sqlx::query!(
            r#"SELECT dc.doc_id, dc.document_name, dt.type_code, dt.display_name, dc.status
               FROM "ob-poc".document_catalog dc
               LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
               WHERE dc.cbu_id = $1
               ORDER BY dt.type_code"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let doc_list: Vec<serde_json::Value> = documents
            .iter()
            .map(|d| {
                serde_json::json!({
                    "doc_id": d.doc_id,
                    "name": d.document_name,
                    "type_code": d.type_code,
                    "type_name": d.display_name,
                    "status": d.status
                })
            })
            .collect();

        // Get screenings (via KYC workstreams)
        let screenings = sqlx::query!(
            r#"SELECT s.screening_id, w.entity_id, e.name as entity_name,
                      s.screening_type, s.status, s.result_summary
               FROM kyc.screenings s
               JOIN kyc.entity_workstreams w ON w.workstream_id = s.workstream_id
               JOIN kyc.cases c ON c.case_id = w.case_id
               JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
               WHERE c.cbu_id = $1
               ORDER BY s.screening_type, e.name"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let screening_list: Vec<serde_json::Value> = screenings
            .iter()
            .map(|s| {
                serde_json::json!({
                    "screening_id": s.screening_id,
                    "entity_id": s.entity_id,
                    "entity_name": s.entity_name,
                    "screening_type": s.screening_type,
                    "status": s.status,
                    "result": s.result_summary
                })
            })
            .collect();

        // Get service deliveries
        let services = sqlx::query!(
            r#"SELECT sdm.delivery_id, p.name as product_name, p.product_code,
                      s.name as service_name, sdm.delivery_status
               FROM "ob-poc".service_delivery_map sdm
               JOIN "ob-poc".products p ON p.product_id = sdm.product_id
               JOIN "ob-poc".services s ON s.service_id = sdm.service_id
               WHERE sdm.cbu_id = $1
               ORDER BY p.name, s.name"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let service_list: Vec<serde_json::Value> = services
            .iter()
            .map(|s| {
                serde_json::json!({
                    "delivery_id": s.delivery_id,
                    "product": s.product_name,
                    "product_code": s.product_code,
                    "service": s.service_name,
                    "status": s.delivery_status
                })
            })
            .collect();

        // Get KYC cases
        let cases = sqlx::query!(
            r#"SELECT case_id, status, case_type, risk_rating, escalation_level,
                      opened_at, closed_at
               FROM kyc.cases
               WHERE cbu_id = $1
               ORDER BY opened_at DESC"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let case_list: Vec<serde_json::Value> = cases
            .iter()
            .map(|c| {
                serde_json::json!({
                    "case_id": c.case_id,
                    "status": c.status,
                    "case_type": c.case_type,
                    "risk_rating": c.risk_rating,
                    "escalation_level": c.escalation_level,
                    "opened_at": c.opened_at.to_rfc3339(),
                    "closed_at": c.closed_at.map(|t| t.to_rfc3339())
                })
            })
            .collect();

        // Build complete result
        let result = serde_json::json!({
            "cbu_id": cbu.cbu_id,
            "name": cbu.name,
            "jurisdiction": cbu.jurisdiction,
            "client_type": cbu.client_type,
            "category": cbu.cbu_category,
            "nature_purpose": cbu.nature_purpose,
            "description": cbu.description,
            "created_at": cbu.created_at.map(|t| t.to_rfc3339()),
            "updated_at": cbu.updated_at.map(|t| t.to_rfc3339()),
            "entities": entity_list,
            "documents": doc_list,
            "screenings": screening_list,
            "services": service_list,
            "kyc_cases": case_list,
            "summary": {
                "entity_count": entity_list.len(),
                "document_count": doc_list.len(),
                "screening_count": screening_list.len(),
                "service_count": service_list.len(),
                "case_count": case_list.len()
            }
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "error": "Database required for cbu.show"
        })))
    }
}

// ============================================================================
// CBU Decision Operation
// ============================================================================

/// Record KYC/AML decision for CBU collective state
///
/// Rationale: This is the decision point verb. Its execution in DSL history
/// IS the searchable snapshot boundary. Updates CBU status, case status,
/// and creates evaluation snapshot.
pub struct CbuDecideOp;

#[async_trait]
impl CustomOperation for CbuDecideOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }
    fn verb(&self) -> &'static str {
        "decide"
    }
    fn rationale(&self) -> &'static str {
        "Decision point for CBU collective state - searchable in DSL history"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Extract required args
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let decision = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "decision")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing decision argument"))?;

        let decided_by = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "decided-by")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing decided-by argument"))?;

        let rationale = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "rationale")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing rationale argument"))?;

        // Optional args
        let case_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "case-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        let conditions = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "conditions")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let escalation_reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "escalation-reason")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Validate: REFERRED requires escalation-reason
        if decision == "REFERRED" && escalation_reason.is_none() {
            return Err(anyhow::anyhow!(
                "escalation-reason is required when decision is REFERRED"
            ));
        }

        // Get current CBU
        let cbu = sqlx::query!(
            r#"SELECT name, status FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        // Map decision to new CBU status
        // Valid statuses: DISCOVERED, VALIDATION_PENDING, VALIDATED, UPDATE_PENDING_PROOF, VALIDATION_FAILED
        let new_cbu_status = match decision {
            "APPROVED" => "VALIDATED",
            "REJECTED" => "VALIDATION_FAILED",
            "REFERRED" => "VALIDATION_PENDING", // Stays pending, escalated for review
            _ => return Err(anyhow::anyhow!("Invalid decision: {}", decision)),
        };

        // Map decision to case status
        // Valid: INTAKE, DISCOVERY, ASSESSMENT, REVIEW, APPROVED, REJECTED, BLOCKED, WITHDRAWN, EXPIRED, REFER_TO_REGULATOR, DO_NOT_ONBOARD
        let new_case_status = match decision {
            "APPROVED" => "APPROVED",
            "REJECTED" => "REJECTED",
            "REFERRED" => "REVIEW", // Stays in REVIEW with escalation
            _ => "REVIEW",
        };

        // Find or validate case_id
        let case_id = match case_id {
            Some(id) => id,
            None => {
                // Find active case for this CBU
                let row = sqlx::query!(
                    r#"SELECT case_id FROM kyc.cases
                       WHERE cbu_id = $1 AND status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN', 'EXPIRED')
                       ORDER BY opened_at DESC LIMIT 1"#,
                    cbu_id
                )
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| anyhow::anyhow!("No active KYC case found for CBU"))?;
                row.case_id
            }
        };

        // Begin transaction
        let mut tx = pool.begin().await?;

        // 1. Update CBU status
        sqlx::query!(
            r#"UPDATE "ob-poc".cbus SET status = $1, updated_at = now() WHERE cbu_id = $2"#,
            new_cbu_status,
            cbu_id
        )
        .execute(&mut *tx)
        .await?;

        // 2. Update case status
        let should_close = matches!(decision, "APPROVED" | "REJECTED");
        if should_close {
            sqlx::query!(
                r#"UPDATE kyc.cases SET status = $1, closed_at = now(), last_activity_at = now() WHERE case_id = $2"#,
                new_case_status,
                case_id
            )
            .execute(&mut *tx)
            .await?;
        } else {
            // REFERRED - update escalation level
            sqlx::query!(
                r#"UPDATE kyc.cases SET escalation_level = 'SENIOR_COMPLIANCE', last_activity_at = now() WHERE case_id = $1"#,
                case_id
            )
            .execute(&mut *tx)
            .await?;
        }

        // 3. Create evaluation snapshot with decision
        let snapshot_id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO "ob-poc".case_evaluation_snapshots
               (snapshot_id, case_id, soft_count, escalate_count, hard_stop_count, total_score,
                recommended_action, evaluated_by, decision_made, decision_made_at, decision_made_by, decision_notes)
               VALUES ($1, $2, 0, 0, 0, 0, $3, $4, $3, now(), $4, $5)"#,
            snapshot_id,
            case_id,
            decision,
            decided_by,
            rationale
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        // Return decision record
        Ok(ExecutionResult::Record(serde_json::json!({
            "cbu_id": cbu_id,
            "cbu_name": cbu.name,
            "case_id": case_id,
            "snapshot_id": snapshot_id,
            "decision": decision,
            "previous_status": cbu.status,
            "new_status": new_cbu_status,
            "decided_by": decided_by,
            "rationale": rationale,
            "conditions": conditions,
            "escalation_reason": escalation_reason
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "error": "Database required for cbu.decide"
        })))
    }
}

// ============================================================================
// Observation Operations
// ============================================================================

/// Record an observation from a document
pub struct ObservationFromDocumentOp;

#[async_trait]
impl CustomOperation for ObservationFromDocumentOp {
    fn domain(&self) -> &'static str {
        "observation"
    }
    fn verb(&self) -> &'static str {
        "record-from-document"
    }
    fn rationale(&self) -> &'static str {
        "Requires document lookup, attribute lookup, and automatic confidence/authoritative flags"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get entity-id (required)
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Get document-id (required)
        let document_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "document-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing document-id argument"))?;

        // Get attribute name (required) and look up ID
        let attr_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attribute")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing attribute argument"))?;

        let attribute_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT uuid FROM "ob-poc".attribute_registry WHERE id = $1 OR name = $1"#,
        )
        .bind(attr_name)
        .fetch_optional(pool)
        .await?;

        let attribute_id =
            attribute_id.ok_or_else(|| anyhow::anyhow!("Unknown attribute: {}", attr_name))?;

        // Get value (required)
        let value = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "value")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing value argument"))?;

        // Get optional extraction method
        let extraction_method = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "extraction-method")
            .and_then(|a| a.value.as_string());

        // Get optional confidence (default 0.80 for document extractions)
        let confidence: f64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "confidence")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.80);

        // Look up if this document type is authoritative for this attribute
        let is_authoritative: bool = sqlx::query_scalar(
            r#"SELECT COALESCE(dal.is_authoritative, FALSE)
               FROM "ob-poc".document_catalog dc
               LEFT JOIN "ob-poc".document_attribute_links dal
                 ON dal.document_type_id = dc.document_type_id
                 AND dal.attribute_id = $2
                 AND dal.direction IN ('SOURCE', 'BOTH')
               WHERE dc.doc_id = $1
               LIMIT 1"#,
        )
        .bind(document_id)
        .bind(attribute_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or(false);

        // Insert observation
        let observation_id = Uuid::new_v4();

        sqlx::query(
            r#"INSERT INTO "ob-poc".attribute_observations
               (observation_id, entity_id, attribute_id, value_text, source_type,
                source_document_id, confidence, is_authoritative, extraction_method,
                observed_at, status)
               VALUES ($1, $2, $3, $4, 'DOCUMENT', $5, $6, $7, $8, NOW(), 'ACTIVE')"#,
        )
        .bind(observation_id)
        .bind(entity_id)
        .bind(attribute_id)
        .bind(value)
        .bind(document_id)
        .bind(confidence)
        .bind(is_authoritative)
        .bind(extraction_method)
        .execute(pool)
        .await?;

        ctx.bind("observation", observation_id);
        Ok(ExecutionResult::Uuid(observation_id))
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

/// Get current best observation for an attribute
pub struct ObservationGetCurrentOp;

#[async_trait]
impl CustomOperation for ObservationGetCurrentOp {
    fn domain(&self) -> &'static str {
        "observation"
    }
    fn verb(&self) -> &'static str {
        "get-current"
    }
    fn rationale(&self) -> &'static str {
        "Requires priority-based selection (authoritative > confidence > recency)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get entity-id (required)
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Get attribute name (required) and look up ID
        let attr_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attribute")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing attribute argument"))?;

        let attribute_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT uuid FROM "ob-poc".attribute_registry WHERE id = $1 OR name = $1"#,
        )
        .bind(attr_name)
        .fetch_optional(pool)
        .await?;

        let attribute_id =
            attribute_id.ok_or_else(|| anyhow::anyhow!("Unknown attribute: {}", attr_name))?;

        // Get current best observation from view
        let result: Option<(
            Uuid,
            Option<String>,
            Option<rust_decimal::Decimal>,
            Option<bool>,
            Option<chrono::NaiveDate>,
            String,
            Option<rust_decimal::Decimal>,
            bool,
        )> = sqlx::query_as(
            r#"SELECT observation_id, value_text, value_number, value_boolean, value_date,
                      source_type, confidence, is_authoritative
               FROM "ob-poc".v_attribute_current
               WHERE entity_id = $1 AND attribute_id = $2"#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .fetch_optional(pool)
        .await?;

        match result {
            Some((
                obs_id,
                value_text,
                value_number,
                value_boolean,
                value_date,
                source_type,
                confidence,
                is_authoritative,
            )) => Ok(ExecutionResult::Record(serde_json::json!({
                "observation_id": obs_id,
                "value_text": value_text,
                "value_number": value_number,
                "value_boolean": value_boolean,
                "value_date": value_date,
                "source_type": source_type,
                "confidence": confidence,
                "is_authoritative": is_authoritative
            }))),
            None => Ok(ExecutionResult::Record(serde_json::json!({
                "found": false,
                "message": "No active observation found for this attribute"
            }))),
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "found": false,
            "message": "No database available"
        })))
    }
}

/// Reconcile observations for an attribute and auto-create discrepancies
///
/// Rationale: Compares all active observations for an entity+attribute,
/// identifies conflicts, and optionally creates discrepancy records.
pub struct ObservationReconcileOp;

#[async_trait]
impl CustomOperation for ObservationReconcileOp {
    fn domain(&self) -> &'static str {
        "observation"
    }
    fn verb(&self) -> &'static str {
        "reconcile"
    }
    fn rationale(&self) -> &'static str {
        "Requires comparing multiple observations and detecting value conflicts"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get entity-id (required)
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Get attribute name
        let attr_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attribute")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing attribute argument"))?;

        // Lookup attribute ID
        let attribute_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT uuid FROM "ob-poc".attribute_registry WHERE id = $1 OR name = $1"#,
        )
        .bind(attr_name)
        .fetch_optional(pool)
        .await?;

        let attribute_id =
            attribute_id.ok_or_else(|| anyhow::anyhow!("Unknown attribute: {}", attr_name))?;

        // Optional case-id
        let case_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "case-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Auto-create discrepancies (default true)
        let auto_create = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "auto-create-discrepancies")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        // Get all active observations for this entity+attribute
        let observations: Vec<(
            Uuid,
            Option<String>,
            String,
            Option<rust_decimal::Decimal>,
            bool,
        )> = sqlx::query_as(
            r#"SELECT observation_id, value_text, source_type, confidence, is_authoritative
                   FROM "ob-poc".attribute_observations
                   WHERE entity_id = $1 AND attribute_id = $2 AND status = 'ACTIVE'
                   ORDER BY is_authoritative DESC, confidence DESC NULLS LAST, observed_at DESC"#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .fetch_all(pool)
        .await?;

        if observations.len() < 2 {
            return Ok(ExecutionResult::Record(serde_json::json!({
                "status": "no_conflict",
                "observation_count": observations.len(),
                "discrepancies_created": 0
            })));
        }

        // Compare values to find discrepancies
        let mut discrepancies_created = 0;
        let first = &observations[0];

        for other in observations.iter().skip(1) {
            // Compare values (simplified - just text comparison for now)
            if first.1 != other.1 && auto_create {
                // Create discrepancy record
                let discrepancy_id = Uuid::new_v4();
                sqlx::query!(
                    r#"INSERT INTO "ob-poc".observation_discrepancies
                       (discrepancy_id, entity_id, attribute_id, observation_1_id, observation_2_id,
                        discrepancy_type, severity, description, case_id, resolution_status)
                       VALUES ($1, $2, $3, $4, $5, 'VALUE_MISMATCH', 'MEDIUM',
                               'Different values observed for same attribute', $6, 'OPEN')"#,
                    discrepancy_id,
                    entity_id,
                    attribute_id,
                    first.0,
                    other.0,
                    case_id
                )
                .execute(pool)
                .await?;
                discrepancies_created += 1;
            }
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "status": if discrepancies_created > 0 { "conflicts_found" } else { "no_conflict" },
            "observation_count": observations.len(),
            "discrepancies_created": discrepancies_created
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "status": "no_database",
            "discrepancies_created": 0
        })))
    }
}

/// Batch verify pending allegations against observations
///
/// Rationale: Compares pending allegations with authoritative observations
/// and auto-updates verification status.
pub struct ObservationVerifyAllegationsOp;

#[async_trait]
impl CustomOperation for ObservationVerifyAllegationsOp {
    fn domain(&self) -> &'static str {
        "observation"
    }
    fn verb(&self) -> &'static str {
        "verify-allegations"
    }
    fn rationale(&self) -> &'static str {
        "Requires joining allegations with observations and comparing values"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get cbu-id (required)
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        // Get entity-id (required)
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Get pending allegations for this entity
        let allegations: Vec<(Uuid, Uuid, serde_json::Value, Option<String>)> = sqlx::query_as(
            r#"SELECT allegation_id, attribute_id, alleged_value, alleged_value_display
               FROM "ob-poc".client_allegations
               WHERE cbu_id = $1 AND entity_id = $2 AND verification_status = 'PENDING'"#,
        )
        .bind(cbu_id)
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        let mut verified = 0;
        let mut contradicted = 0;
        let mut no_observation = 0;

        for (allegation_id, attribute_id, alleged_value, alleged_display) in allegations {
            // Get current observation for this attribute
            let current: Option<(Uuid, Option<String>)> = sqlx::query_as(
                r#"SELECT observation_id, value_text
                   FROM "ob-poc".v_attribute_current
                   WHERE entity_id = $1 AND attribute_id = $2"#,
            )
            .bind(entity_id)
            .bind(attribute_id)
            .fetch_optional(pool)
            .await?;

            match current {
                Some((observation_id, obs_value_text)) => {
                    // Compare alleged value with observation
                    let alleged_str = alleged_display
                        .or_else(|| alleged_value.as_str().map(String::from))
                        .unwrap_or_default();

                    let matches = obs_value_text
                        .as_ref()
                        .map(|v| v.to_lowercase() == alleged_str.to_lowercase())
                        .unwrap_or(false);

                    if matches {
                        // Verify the allegation
                        sqlx::query!(
                            r#"UPDATE "ob-poc".client_allegations
                               SET verification_status = 'VERIFIED',
                                   verified_by_observation_id = $2,
                                   verified_at = NOW()
                               WHERE allegation_id = $1"#,
                            allegation_id,
                            observation_id
                        )
                        .execute(pool)
                        .await?;
                        verified += 1;
                    } else {
                        // Contradict the allegation
                        sqlx::query!(
                            r#"UPDATE "ob-poc".client_allegations
                               SET verification_status = 'CONTRADICTED',
                                   verified_by_observation_id = $2,
                                   verified_at = NOW(),
                                   verification_notes = 'Value does not match observation'
                               WHERE allegation_id = $1"#,
                            allegation_id,
                            observation_id
                        )
                        .execute(pool)
                        .await?;
                        contradicted += 1;
                    }
                }
                None => {
                    no_observation += 1;
                }
            }
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "verified": verified,
            "contradicted": contradicted,
            "no_observation": no_observation,
            "total_processed": verified + contradicted + no_observation
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "verified": 0,
            "contradicted": 0,
            "no_observation": 0
        })))
    }
}

/// Extract document data and create observations
///
/// Rationale: Uses document_attribute_links to determine what attributes
/// a document can provide, extracts values, and creates observations.
pub struct DocumentExtractObservationsOp;

#[async_trait]
impl CustomOperation for DocumentExtractObservationsOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "extract-to-observations"
    }
    fn rationale(&self) -> &'static str {
        "Requires joining document with attribute links and creating observations"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get document-id (required)
        let document_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "document-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing document-id argument"))?;

        // Get entity-id (required)
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Auto-verify allegations (default true)
        let auto_verify = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "auto-verify-allegations")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        // Get document type
        let doc_type: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT document_type_id FROM "ob-poc".document_catalog WHERE doc_id = $1"#,
        )
        .bind(document_id)
        .fetch_optional(pool)
        .await?;

        let doc_type_id =
            doc_type.ok_or_else(|| anyhow::anyhow!("Document not found: {}", document_id))?;

        // Get attributes this document can provide (SOURCE direction)
        let extractable: Vec<(Uuid, String, Option<String>, Option<sqlx::types::BigDecimal>, bool)> =
            sqlx::query_as(
                r#"SELECT dal.attribute_id, ar.id, dal.extraction_method,
                          dal.extraction_confidence_default, dal.is_authoritative
                   FROM "ob-poc".document_attribute_links dal
                   JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
                   WHERE dal.document_type_id = $1 AND dal.direction = 'SOURCE' AND dal.is_active = TRUE"#,
            )
            .bind(doc_type_id)
            .fetch_all(pool)
            .await?;

        // For now, we create placeholder observations - actual extraction would call AI/OCR
        let mut observations_created = 0;

        for (attribute_id, attr_name, extraction_method, confidence, is_authoritative) in
            &extractable
        {
            let observation_id = Uuid::new_v4();

            // Create observation with placeholder value (real impl would extract from document)
            sqlx::query!(
                r#"INSERT INTO "ob-poc".attribute_observations
                   (observation_id, entity_id, attribute_id, value_text, source_type,
                    source_document_id, extraction_method, confidence, is_authoritative, status)
                   VALUES ($1, $2, $3, $4, 'DOCUMENT', $5, $6, $7, $8, 'ACTIVE')
                   ON CONFLICT (entity_id, attribute_id, source_type, source_document_id)
                   WHERE status = 'ACTIVE'
                   DO NOTHING"#,
                observation_id,
                entity_id,
                attribute_id,
                format!("[Extracted from document - {}]", attr_name), // Placeholder
                document_id,
                extraction_method.as_deref().unwrap_or("MANUAL"),
                confidence.clone(),
                is_authoritative
            )
            .execute(pool)
            .await?;

            observations_created += 1;
        }

        // Auto-verify allegations if requested
        let mut allegations_verified = 0;
        if auto_verify && observations_created > 0 {
            // Get CBU for this entity's document
            let cbu_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT cbu_id FROM "ob-poc".document_catalog WHERE doc_id = $1"#,
            )
            .bind(document_id)
            .fetch_optional(pool)
            .await?;

            if let Some(cbu_id) = cbu_id {
                // Update matching pending allegations
                let result = sqlx::query!(
                    r#"UPDATE "ob-poc".client_allegations ca
                       SET verification_status = 'VERIFIED',
                           verified_at = NOW(),
                           verification_notes = 'Auto-verified by document extraction'
                       WHERE ca.cbu_id = $1 AND ca.entity_id = $2
                         AND ca.verification_status = 'PENDING'
                         AND EXISTS (
                           SELECT 1 FROM "ob-poc".attribute_observations ao
                           WHERE ao.entity_id = ca.entity_id
                             AND ao.attribute_id = ca.attribute_id
                             AND ao.source_document_id = $3
                             AND ao.status = 'ACTIVE'
                         )"#,
                    cbu_id,
                    entity_id,
                    document_id
                )
                .execute(pool)
                .await?;

                allegations_verified = result.rows_affected() as i32;
            }
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "observations_created": observations_created,
            "attributes_extractable": extractable.len(),
            "allegations_verified": allegations_verified
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "observations_created": 0,
            "attributes_extractable": 0,
            "allegations_verified": 0
        })))
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
        // Service resource instance operations
        assert!(registry.has("service-resource", "provision"));
        assert!(registry.has("service-resource", "set-attr"));
        assert!(registry.has("service-resource", "activate"));
        assert!(registry.has("service-resource", "suspend"));
        assert!(registry.has("service-resource", "decommission"));
        assert!(registry.has("service-resource", "validate-attrs"));
        // Delivery operations are now CRUD-based (delivery.yaml)
        // Custody operations
        assert!(registry.has("subcustodian", "lookup"));
        assert!(registry.has("cbu-custody", "lookup-ssi"));
        assert!(registry.has("cbu-custody", "validate-booking-coverage"));
        assert!(registry.has("cbu-custody", "derive-required-coverage"));
        // CBU operations
        assert!(registry.has("cbu", "add-product"));
        assert!(registry.has("cbu", "show"));
    }

    #[test]
    fn test_registry_list() {
        let registry = CustomOperationRegistry::new();
        let ops = registry.list();
        // 7 original (entity-create, doc-catalog, doc-extract, ubo-calculate, 3 screening)
        // + 6 resource + 4 custody + 4 observation + 1 doc-extract-observations
        // + 3 threshold + 3 rfi + 7 ubo-analysis + 3 cbu (add-product, show, decide) = 39
        assert_eq!(ops.len(), 39);
    }
}
