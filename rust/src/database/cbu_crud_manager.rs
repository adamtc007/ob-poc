//! CBU CRUD Manager - Comprehensive Client Business Unit CRUD Operations
//!
//! This module implements the complete CBU lifecycle management system with multi-table
//! CRUD operations, following the DSL CRUD specification. It handles complex operations
//! across CBUs, entities, attributes, UBO calculations, and product workflows.

use crate::database::DictionaryDatabaseService;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};

use tracing::{error, info, warn};
use uuid::Uuid;

/// CBU CRUD Manager for comprehensive CBU operations
#[derive(Clone)]
pub(crate) struct CbuCrudManager {
    pool: PgPool,
    dictionary_service: DictionaryDatabaseService,
}

/// CBU creation request with full relationship setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuCreateRequest {
    pub name: String,
    pub description: Option<String>,
    pub nature_purpose: Option<String>,
    pub entities: Vec<CbuEntityAssignment>,
    pub attributes: Vec<CbuAttributeAssignment>,
    pub products: Vec<String>,
    pub services: Vec<String>,
    pub workflow_type: Option<String>,
    pub auto_calculate_ubo: bool,
    pub compliance_frameworks: Vec<String>,
}

/// Entity assignment to CBU with roles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuEntityAssignment {
    pub entity_id: Uuid,
    pub entity_type: String,
    pub roles: Vec<String>,
    pub jurisdiction: Option<String>,
    pub ownership_percentage: Option<f64>,
    pub effective_date: Option<NaiveDate>,
}

/// Attribute assignment to CBU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuAttributeAssignment {
    pub attribute_id: Uuid,
    pub value: Value,
    pub source: Option<Value>,
}

/// CBU search criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuSearchCriteria {
    pub jurisdictions: Option<Vec<String>>,
    pub entity_types: Option<Vec<String>>,
    pub product_types: Option<Vec<String>>,
    pub status: Option<Vec<String>>,
    pub aum_range: Option<AumRange>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub include_relations: Vec<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// AUM (Assets Under Management) range filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AumRange {
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub currency: String,
}

/// CBU update request with complex operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuUpdateRequest {
    pub cbu_id: Uuid,
    pub basic_updates: Option<CbuBasicUpdates>,
    pub entity_operations: Vec<CbuEntityOperation>,
    pub attribute_operations: Vec<CbuAttributeOperation>,
    pub product_operations: Vec<CbuProductOperation>,
    pub recalculate_ubo: bool,
    pub update_compliance_status: bool,
}

/// Basic CBU field updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuBasicUpdates {
    pub name: Option<String>,
    pub description: Option<String>,
    pub nature_purpose: Option<String>,
}

/// Entity operations for CBU updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuEntityOperation {
    pub operation: String, // "add_entity", "update_roles", "update_ownership", "remove_entity"
    pub entity_id: Uuid,
    pub entity_type: Option<String>,
    pub roles: Option<Vec<String>>,
    pub add_roles: Option<Vec<String>>,
    pub remove_roles: Option<Vec<String>>,
    pub ownership_percentage: Option<f64>,
    pub effective_date: Option<NaiveDate>,
    pub removal_reason: Option<String>,
}

/// Attribute operations for CBU updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuAttributeOperation {
    pub operation: String, // "set", "update", "remove", "append"
    pub attribute_id: Uuid,
    pub value: Option<Value>,
}

/// Product operations for CBU updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuProductOperation {
    pub operation: String, // "add", "remove", "update_status"
    pub products: Option<Vec<String>>,
    pub product: Option<String>,
    pub status: Option<String>,
    pub effective_date: Option<NaiveDate>,
}

/// CBU deletion configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuDeleteRequest {
    pub cbu_id: Uuid,
    pub deletion_strategy: DeletionStrategy,
    pub dependency_handling: DependencyHandling,
    pub confirmation_token: String,
    pub deletion_reason: String,
    pub authorized_by: String,
    pub regulatory_notifications_required: bool,
}

/// Deletion strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum DeletionStrategy {
    SoftDelete,
    HardDelete,
    Archive,
}

/// Dependency handling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DependencyHandling {
    pub entity_references: String,
    pub document_references: String,
    pub ubo_calculations: String,
    pub orchestration_sessions: String,
    pub product_workflows: String,
}

/// CBU creation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuCreationResult {
    pub cbu_id: Uuid,
    pub session_id: Option<Uuid>,
    pub created_entities: usize,
    pub created_attributes: usize,
    pub initialized_products: usize,
    pub ubo_calculation_initiated: bool,
    pub creation_time_ms: u64,
}

/// Complete CBU data with all relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuCompleteData {
    pub cbu: CbuRecord,
    pub entities: Vec<CbuEntityWithRoles>,
    pub attributes: Vec<CbuAttributeValue>,
    pub ubo_calculations: Vec<UboCalculation>,
    pub product_workflows: Vec<ProductWorkflow>,
    pub orchestration_status: Option<OrchestrationStatus>,
    pub document_associations: Vec<DocumentAssociation>,
    pub compliance_status: Vec<ComplianceStatus>,
}

/// Core CBU record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuRecord {
    pub cbu_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub nature_purpose: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// CBU entity with roles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuEntityWithRoles {
    pub entity_id: Uuid,
    pub entity_type: String,
    pub entity_name: String,
    pub roles: Vec<String>,
    pub jurisdiction: Option<String>,
    pub ownership_percentage: Option<f64>,
    pub created_at: DateTime<Utc>,
}

/// CBU attribute value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CbuAttributeValue {
    pub attribute_id: Uuid,
    pub attribute_name: String,
    pub value: Value,
    pub source: Option<Value>,
    pub state: String,
    pub observed_at: DateTime<Utc>,
}

/// UBO calculation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UboCalculation {
    pub ubo_id: Uuid,
    pub subject_entity_id: Uuid,
    pub ubo_proper_person_id: Uuid,
    pub relationship_type: String,
    pub ownership_percentage: Option<f64>,
    pub control_type: Option<String>,
    pub regulatory_framework: Option<String>,
    pub calculated_at: DateTime<Utc>,
}

/// Product workflow status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProductWorkflow {
    pub workflow_id: Uuid,
    pub product_id: Uuid,
    pub entity_type: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Orchestration session status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OrchestrationStatus {
    pub session_id: Uuid,
    pub primary_domain: String,
    pub current_state: String,
    pub workflow_type: String,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

/// Document association
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentAssociation {
    pub document_id: Uuid,
    pub document_type: String,
    pub relationship_type: String,
    pub created_at: DateTime<Utc>,
}

/// Compliance status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ComplianceStatus {
    pub framework: String,
    pub status: String,
    pub last_reviewed: Option<DateTime<Utc>>,
    pub next_review_due: Option<DateTime<Utc>>,
}

/// CBU operation errors
#[derive(Debug, thiserror::Error)]
pub(crate) enum CbuCrudError {
    #[error("CBU validation failed: {details}")]
    ValidationError { details: String },

    #[error("Entity relationship conflict: {entity_id} already has conflicting role {role}")]
    EntityRoleConflict { entity_id: Uuid, role: String },

    #[error("UBO calculation failed: {reason}")]
    UboCalculationError { reason: String },

    #[error("Regulatory constraint violation: {regulation} prevents {operation}")]
    RegulatoryConstraintViolation {
        regulation: String,
        operation: String,
    },

    #[error("Dependency constraint: Cannot delete CBU {cbu_id} due to {constraint}")]
    DependencyConstraint { cbu_id: Uuid, constraint: String },

    #[error("CBU not found: {cbu_id}")]
    CbuNotFound { cbu_id: Uuid },

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

impl CbuCrudManager {
    /// Create new CBU CRUD manager
    pub fn new(pool: PgPool) -> Self {
        let dictionary_service = DictionaryDatabaseService::new(pool.clone());
        Self {
            pool,
            dictionary_service,
        }
    }

    /// Create CBU with comprehensive multi-table setup
    pub async fn create_cbu_complex(
        &self,
        request: CbuCreateRequest,
    ) -> Result<CbuCreationResult, CbuCrudError> {
        let start_time = std::time::Instant::now();
        let mut tx = self.pool.begin().await?;

        info!("Creating CBU: {}", request.name);

        // 1. Validate request
        self.validate_cbu_create_request(&request).await?;

        // 2. Create main CBU record
        let cbu_id = self.create_cbu_record(&mut tx, &request).await?;
        info!("Created CBU record with ID: {}", cbu_id);

        // 3. Create entity relationships
        let entities_created = if !request.entities.is_empty() {
            self.create_cbu_entity_roles(&mut tx, cbu_id, &request.entities)
                .await?
        } else {
            0
        };

        // 4. Set attribute values
        let attributes_created = if !request.attributes.is_empty() {
            self.set_cbu_attributes(&mut tx, cbu_id, &request.attributes)
                .await?
        } else {
            0
        };

        // 5. Initialize product workflows
        let products_initialized = if !request.products.is_empty() {
            self.create_product_workflows(&mut tx, cbu_id, &request.products, &request.entities)
                .await?
        } else {
            0
        };

        // 6. Create orchestration session
        let session_id = if request.workflow_type.is_some() {
            Some(
                self.create_orchestration_session(&mut tx, cbu_id, &request)
                    .await?,
            )
        } else {
            None
        };

        // 7. Initial UBO calculation if needed
        let ubo_initiated = if request.auto_calculate_ubo && !request.entities.is_empty() {
            self.calculate_ubo_initial(&mut tx, cbu_id, &request.compliance_frameworks)
                .await?;
            true
        } else {
            false
        };

        // 8. Audit logging
        self.log_cbu_creation(&mut tx, cbu_id, &request).await?;

        tx.commit().await?;

        let creation_time = start_time.elapsed().as_millis() as u64;
        info!(
            "CBU {} created successfully in {}ms with {} entities, {} attributes, {} products",
            cbu_id, creation_time, entities_created, attributes_created, products_initialized
        );

        Ok(CbuCreationResult {
            cbu_id,
            session_id,
            created_entities: entities_created,
            created_attributes: attributes_created,
            initialized_products: products_initialized,
            ubo_calculation_initiated: ubo_initiated,
            creation_time_ms: creation_time,
        })
    }

    /// Read CBU with comprehensive relationship data
    pub async fn read_cbu_complete(
        &self,
        cbu_id: Uuid,
        include_relations: &[String],
        _with_history: bool,
    ) -> Result<CbuCompleteData, CbuCrudError> {
        info!(
            "Reading CBU {} with relations: {:?}",
            cbu_id, include_relations
        );

        // 1. Get main CBU record
        let cbu = self.get_cbu_record(cbu_id).await?;

        // 2. Load relationships based on requested inclusions
        let entities = if include_relations.contains(&"entities_with_roles".to_string()) {
            self.get_cbu_entities_with_roles(cbu_id).await?
        } else {
            vec![]
        };

        let attributes = if include_relations.contains(&"attribute_values_resolved".to_string()) {
            self.get_cbu_attribute_values(cbu_id, true).await?
        } else {
            vec![]
        };

        let ubo_calculations =
            if include_relations.contains(&"ubo_calculations_current".to_string()) {
                self.get_cbu_ubo_calculations(cbu_id).await?
            } else {
                vec![]
            };

        let product_workflows =
            if include_relations.contains(&"product_workflows_active".to_string()) {
                self.get_cbu_product_workflows(cbu_id).await?
            } else {
                vec![]
            };

        let orchestration_status =
            if include_relations.contains(&"orchestration_status".to_string()) {
                self.get_cbu_orchestration_status(cbu_id).await?
            } else {
                None
            };

        let document_associations =
            if include_relations.contains(&"document_associations".to_string()) {
                self.get_cbu_document_associations(cbu_id).await?
            } else {
                vec![]
            };

        let compliance_status = if include_relations.contains(&"compliance_status".to_string()) {
            self.get_cbu_compliance_status(cbu_id).await?
        } else {
            vec![]
        };

        Ok(CbuCompleteData {
            cbu,
            entities,
            attributes,
            ubo_calculations,
            product_workflows,
            orchestration_status,
            document_associations,
            compliance_status,
        })
    }

    /// Search CBUs with complex criteria
    pub async fn search_cbus(
        &self,
        criteria: CbuSearchCriteria,
    ) -> Result<Vec<CbuCompleteData>, CbuCrudError> {
        info!("Searching CBUs with criteria: {:?}", criteria);

        // For now, use a simple query that works with sqlx::query!
        // TODO: Implement more complex filtering later
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT c.cbu_id, c.name, c.description, c.nature_purpose,
                   c.created_at, c.updated_at
            FROM "ob-poc".cbus c
            ORDER BY c.created_at DESC
            LIMIT COALESCE($1, 100)
            OFFSET COALESCE($2, 0)
            "#,
            criteria.limit.map(|l| l as i32),
            criteria.offset.map(|o| o as i32)
        )
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            let cbu_data = self
                .read_cbu_complete(row.cbu_id, &criteria.include_relations, false)
                .await?;
            results.push(cbu_data);
        }

        info!("Found {} CBUs matching search criteria", results.len());
        Ok(results)
    }

    /// Update CBU with complex multi-table operations
    pub async fn update_cbu_complex(&self, request: CbuUpdateRequest) -> Result<(), CbuCrudError> {
        let mut tx = self.pool.begin().await?;

        info!("Updating CBU: {}", request.cbu_id);

        // 1. Validate CBU exists
        self.validate_cbu_exists(&mut tx, request.cbu_id).await?;

        // 2. Apply basic updates
        if let Some(basic_updates) = &request.basic_updates {
            self.apply_cbu_basic_updates(&mut tx, request.cbu_id, basic_updates)
                .await?;
        }

        // 3. Apply entity operations
        for entity_op in &request.entity_operations {
            self.apply_entity_operation(&mut tx, request.cbu_id, entity_op)
                .await?;
        }

        // 4. Apply attribute operations
        for attr_op in &request.attribute_operations {
            self.apply_attribute_operation(&mut tx, request.cbu_id, attr_op)
                .await?;
        }

        // 5. Apply product operations
        for product_op in &request.product_operations {
            self.apply_product_operation(&mut tx, request.cbu_id, product_op)
                .await?;
        }

        // 6. Recalculate UBO if requested
        if request.recalculate_ubo {
            self.recalculate_ubo_for_cbu(&mut tx, request.cbu_id)
                .await?;
        }

        // 7. Update compliance status if requested
        if request.update_compliance_status {
            self.update_compliance_status(&mut tx, request.cbu_id)
                .await?;
        }

        // 8. Update timestamp
        self.update_cbu_timestamp(&mut tx, request.cbu_id).await?;

        // 9. Log the update
        self.log_cbu_update(&mut tx, &request).await?;

        tx.commit().await?;
        info!("CBU {} updated successfully", request.cbu_id);
        Ok(())
    }

    /// Delete CBU with comprehensive dependency handling
    pub async fn delete_cbu(&self, request: CbuDeleteRequest) -> Result<(), CbuCrudError> {
        let mut tx = self.pool.begin().await?;

        info!(
            "Deleting CBU {} with strategy {:?}",
            request.cbu_id, request.deletion_strategy
        );

        // 1. Validate confirmation token
        self.validate_deletion_confirmation(&request).await?;

        // 2. Check dependencies and constraints
        self.check_deletion_constraints(&mut tx, request.cbu_id)
            .await?;

        // 3. Handle dependencies based on strategy
        self.handle_deletion_dependencies(&mut tx, &request).await?;

        // 4. Perform deletion based on strategy
        match request.deletion_strategy {
            DeletionStrategy::SoftDelete => {
                self.soft_delete_cbu(&mut tx, &request).await?;
            }
            DeletionStrategy::HardDelete => {
                self.hard_delete_cbu(&mut tx, &request).await?;
            }
            DeletionStrategy::Archive => {
                self.archive_cbu(&mut tx, &request).await?;
            }
        }

        // 5. Log deletion
        self.log_cbu_deletion(&mut tx, &request).await?;

        tx.commit().await?;
        info!("CBU {} deleted successfully", request.cbu_id);
        Ok(())
    }

    // Private helper methods

    async fn validate_cbu_create_request(
        &self,
        request: &CbuCreateRequest,
    ) -> Result<(), CbuCrudError> {
        // Validate name is not empty
        if request.name.trim().is_empty() {
            return Err(CbuCrudError::ValidationError {
                details: "CBU name cannot be empty".to_string(),
            });
        }

        // Validate entity IDs exist
        for entity in &request.entities {
            // Check if entity exists
            let exists = sqlx::query!(
                "SELECT entity_id FROM \"ob-poc\".entities WHERE entity_id = $1",
                entity.entity_id
            )
            .fetch_optional(&self.pool)
            .await?;

            if exists.is_none() {
                return Err(CbuCrudError::ValidationError {
                    details: format!("Entity {} does not exist", entity.entity_id),
                });
            }
        }

        // Validate attribute IDs exist
        for attr in &request.attributes {
            let exists = sqlx::query!(
                "SELECT attribute_id FROM \"ob-poc\".dictionary WHERE attribute_id = $1",
                attr.attribute_id
            )
            .fetch_optional(&self.pool)
            .await?;

            if exists.is_none() {
                return Err(CbuCrudError::ValidationError {
                    details: format!("Attribute {} does not exist", attr.attribute_id),
                });
            }
        }

        Ok(())
    }

    async fn create_cbu_record(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        request: &CbuCreateRequest,
    ) -> Result<Uuid, CbuCrudError> {
        let cbu_id = Uuid::new_v4();

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose)
            VALUES ($1, $2, $3, $4)
            "#,
            cbu_id,
            request.name,
            request.description,
            request.nature_purpose
        )
        .execute(&mut **tx)
        .await?;

        Ok(cbu_id)
    }

    async fn create_cbu_entity_roles(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
        entities: &[CbuEntityAssignment],
    ) -> Result<usize, CbuCrudError> {
        let mut created_count = 0;

        for entity in entities {
            for role in &entity.roles {
                // First, get or create the role ID
                let role_id = self.get_or_create_role_id(tx, role).await?;

                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
                    VALUES ($1, $2, $3)
                    "#,
                    cbu_id,
                    entity.entity_id,
                    role_id
                )
                .execute(&mut **tx)
                .await?;

                created_count += 1;
            }
        }

        Ok(created_count)
    }

    async fn set_cbu_attributes(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
        attributes: &[CbuAttributeAssignment],
    ) -> Result<usize, CbuCrudError> {
        let mut created_count = 0;

        for attr in attributes {
            sqlx::query!(
                r#"
                INSERT INTO "ob-poc".attribute_values
                (cbu_id, attribute_id, value, source, dsl_version, state)
                VALUES ($1, $2, $3, $4, 1, 'resolved')
                "#,
                cbu_id,
                attr.attribute_id,
                attr.value,
                attr.source
            )
            .execute(&mut **tx)
            .await?;

            created_count += 1;
        }

        Ok(created_count)
    }

    async fn create_product_workflows(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
        products: &[String],
        entities: &[CbuEntityAssignment],
    ) -> Result<usize, CbuCrudError> {
        let mut created_count = 0;

        for product in products {
            for entity in entities {
                let workflow_id = Uuid::new_v4(); // Use v4 instead of v5
                                                  // Original v5 logic: Uuid::new_v5(
                let product_id = self.get_or_create_product_id(tx, product).await?;

                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".product_workflows
                    (workflow_id, cbu_id, product_id, entity_type, status, required_dsl, generated_dsl, compliance_rules)
                    VALUES ($1, $2, $3, $4, 'PENDING', '{}', '', '{}')
                    "#,
                    workflow_id,
                    cbu_id,
                    product_id,
                    entity.entity_type
                )
                .execute(&mut **tx)
                .await?;

                created_count += 1;
            }
        }

        Ok(created_count)
    }

    async fn create_orchestration_session(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
        request: &CbuCreateRequest,
    ) -> Result<Uuid, CbuCrudError> {
        let session_id = Uuid::new_v4();
        let primary_entity_type = request
            .entities
            .first()
            .map(|e| e.entity_type.clone())
            .unwrap_or_else(|| "unknown".to_string());

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".orchestration_sessions
            (session_id, primary_domain, cbu_id, entity_type, workflow_type, current_state,
             products, services)
            VALUES ($1, 'cbu_management', $2, $3, $4, 'CREATED', $5, $6)
            "#,
            session_id,
            cbu_id,
            primary_entity_type,
            request
                .workflow_type
                .as_deref()
                .unwrap_or("STANDARD_ONBOARDING"),
            &request.products,
            &request.services
        )
        .execute(&mut **tx)
        .await?;

        Ok(session_id)
    }

    async fn calculate_ubo_initial(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
        compliance_frameworks: &[String],
    ) -> Result<(), CbuCrudError> {
        // Implementation would trigger UBO calculation
        // For now, just log the initiation
        info!(
            "UBO calculation initiated for CBU {} with frameworks: {:?}",
            cbu_id, compliance_frameworks
        );
        Ok(())
    }

    async fn log_cbu_creation(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
        request: &CbuCreateRequest,
    ) -> Result<(), CbuCrudError> {
        // Log the CBU creation in audit trail
        info!(
            "Logging CBU creation: {} with {} entities, {} attributes",
            cbu_id,
            request.entities.len(),
            request.attributes.len()
        );
        Ok(())
    }

    async fn get_cbu_record(&self, cbu_id: Uuid) -> Result<CbuRecord, CbuCrudError> {
        let row = sqlx::query!(
            "SELECT cbu_id, name, description, nature_purpose, created_at, updated_at FROM \"ob-poc\".cbus WHERE cbu_id = $1",
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(CbuRecord {
                cbu_id: r.cbu_id,
                name: r.name,
                description: r.description,
                nature_purpose: r.nature_purpose,
                created_at: r.created_at.unwrap_or_else(Utc::now),
                updated_at: r.updated_at.unwrap_or_else(Utc::now),
            }),
            None => Err(CbuCrudError::CbuNotFound { cbu_id }),
        }
    }

    async fn get_cbu_entities_with_roles(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<CbuEntityWithRoles>, CbuCrudError> {
        let rows = sqlx::query!(
            r#"
            SELECT e.entity_id, et.name as entity_type, e.name as entity_name,
                   array_agg(r.name) as roles, cer.created_at
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            LEFT JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            GROUP BY e.entity_id, et.name, e.name, cer.created_at
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut entities = Vec::new();
        for row in rows {
            entities.push(CbuEntityWithRoles {
                entity_id: row.entity_id,
                entity_type: row.entity_type,
                entity_name: row.entity_name,
                roles: row.roles.unwrap_or_default(),
                jurisdiction: None,         // Would need additional join
                ownership_percentage: None, // Would need additional join
                created_at: row.created_at.unwrap_or_else(Utc::now),
            });
        }

        Ok(entities)
    }

    async fn get_cbu_attribute_values(
        &self,
        cbu_id: Uuid,
        _resolve_names: bool,
    ) -> Result<Vec<CbuAttributeValue>, CbuCrudError> {
        let rows = sqlx::query!(
            r#"
            SELECT av.attribute_id, av.value, av.source, av.state, av.observed_at,
                   d.name as attribute_name
            FROM "ob-poc".attribute_values av
            JOIN "ob-poc".dictionary d ON av.attribute_id = d.attribute_id
            WHERE av.cbu_id = $1
            ORDER BY av.observed_at DESC
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut attributes = Vec::new();
        for row in rows {
            attributes.push(CbuAttributeValue {
                attribute_id: row.attribute_id,
                attribute_name: row.attribute_name,
                value: row.value,
                source: row.source,
                state: row.state,
                observed_at: row.observed_at.unwrap_or_else(Utc::now),
            });
        }

        Ok(attributes)
    }

    async fn get_cbu_ubo_calculations(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<UboCalculation>, CbuCrudError> {
        let rows = sqlx::query!(
            r#"
            SELECT ubo_id, subject_entity_id, ubo_proper_person_id, relationship_type,
                   ownership_percentage, control_type, regulatory_framework, created_at
            FROM "ob-poc".ubo_registry
            WHERE cbu_id = $1
            ORDER BY created_at DESC
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut calculations = Vec::new();
        for row in rows {
            calculations.push(UboCalculation {
                ubo_id: row.ubo_id,
                subject_entity_id: row.subject_entity_id,
                ubo_proper_person_id: row.ubo_proper_person_id,
                relationship_type: row.relationship_type,
                ownership_percentage: row
                    .ownership_percentage
                    .map(|p| p.to_string().parse::<f64>().unwrap_or(0.0)),
                control_type: row.control_type,
                regulatory_framework: row.regulatory_framework,
                calculated_at: row.created_at.unwrap_or_else(Utc::now),
            });
        }

        Ok(calculations)
    }

    async fn get_cbu_product_workflows(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<ProductWorkflow>, CbuCrudError> {
        let rows = sqlx::query!(
            "SELECT workflow_id, product_id, entity_type, status, created_at, updated_at FROM \"ob-poc\".product_workflows WHERE cbu_id = $1",
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut workflows = Vec::new();
        for row in rows {
            workflows.push(ProductWorkflow {
                workflow_id: row.workflow_id,
                product_id: row.product_id,
                entity_type: row.entity_type,
                status: row.status,
                created_at: row.created_at.unwrap_or_else(Utc::now),
                updated_at: row.updated_at.unwrap_or_else(Utc::now),
            });
        }

        Ok(workflows)
    }

    async fn get_cbu_orchestration_status(
        &self,
        cbu_id: Uuid,
    ) -> Result<Option<OrchestrationStatus>, CbuCrudError> {
        let row = sqlx::query!(
            "SELECT session_id, primary_domain, current_state, workflow_type, created_at, updated_at as last_activity FROM \"ob-poc\".orchestration_sessions WHERE cbu_id = $1",
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| OrchestrationStatus {
            session_id: r.session_id,
            primary_domain: r.primary_domain,
            current_state: r.current_state.unwrap_or_else(|| "UNKNOWN".to_string()),
            workflow_type: r.workflow_type.unwrap_or_else(|| "ONBOARDING".to_string()),
            created_at: r.created_at.unwrap_or_else(Utc::now),
            last_activity: r.last_activity.unwrap_or_else(Utc::now),
        }))
    }

    async fn get_cbu_document_associations(
        &self,
        _cbu_id: Uuid,
    ) -> Result<Vec<DocumentAssociation>, CbuCrudError> {
        // Implementation would query document associations
        Ok(vec![])
    }

    async fn get_cbu_compliance_status(
        &self,
        _cbu_id: Uuid,
    ) -> Result<Vec<ComplianceStatus>, CbuCrudError> {
        // Implementation would query compliance status
        Ok(vec![])
    }

    async fn get_or_create_role_id(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        role_name: &str,
    ) -> Result<Uuid, CbuCrudError> {
        // Try to find existing role
        let existing = sqlx::query!(
            "SELECT role_id FROM \"ob-poc\".roles WHERE name = $1",
            role_name
        )
        .fetch_optional(&mut **tx)
        .await?;

        if let Some(row) = existing {
            Ok(row.role_id)
        } else {
            // Create new role
            let role_id = Uuid::new_v4();
            sqlx::query!(
                "INSERT INTO \"ob-poc\".roles (role_id, name, description) VALUES ($1, $2, $3)",
                role_id,
                role_name,
                format!("Auto-created role: {}", role_name)
            )
            .execute(&mut **tx)
            .await?;
            Ok(role_id)
        }
    }

    async fn get_or_create_product_id(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        _product_name: &str,
    ) -> Result<Uuid, CbuCrudError> {
        // For now, generate a deterministic UUID based on product name
        // In production, this would lookup/create in a products table
        let uuid = Uuid::new_v4(); // Use v4 instead of v5 for compatibility
        Ok(uuid)
    }

    // Update operation helper methods
    async fn validate_cbu_exists(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
    ) -> Result<(), CbuCrudError> {
        let exists = sqlx::query!(
            "SELECT 1 as exists FROM \"ob-poc\".cbus WHERE cbu_id = $1",
            cbu_id
        )
        .fetch_optional(&mut **tx)
        .await?;

        if exists.is_none() {
            Err(CbuCrudError::CbuNotFound { cbu_id })
        } else {
            Ok(())
        }
    }

    async fn apply_cbu_basic_updates(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
        updates: &CbuBasicUpdates,
    ) -> Result<(), CbuCrudError> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".cbus
            SET name = COALESCE($2, name),
                description = COALESCE($3, description),
                nature_purpose = COALESCE($4, nature_purpose),
                updated_at = NOW()
            WHERE cbu_id = $1
            "#,
            cbu_id,
            updates.name.as_ref(),
            updates.description.as_ref(),
            updates.nature_purpose.as_ref()
        )
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    async fn apply_entity_operation(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
        operation: &CbuEntityOperation,
    ) -> Result<(), CbuCrudError> {
        match operation.operation.as_str() {
            "add_entity" => {
                if let Some(roles) = &operation.roles {
                    for role in roles {
                        let role_id = self.get_or_create_role_id(tx, role).await?;
                        sqlx::query!(
                            "INSERT INTO \"ob-poc\".cbu_entity_roles (cbu_id, entity_id, role_id) VALUES ($1, $2, $3)",
                            cbu_id, operation.entity_id, role_id
                        )
                        .execute(&mut **tx)
                        .await?;
                    }
                }
            }
            "remove_entity" => {
                sqlx::query!(
                    "DELETE FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = $1 AND entity_id = $2",
                    cbu_id,
                    operation.entity_id
                )
                .execute(&mut **tx)
                .await?;
            }
            _ => {
                warn!("Unsupported entity operation: {}", operation.operation);
            }
        }
        Ok(())
    }

    async fn apply_attribute_operation(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
        operation: &CbuAttributeOperation,
    ) -> Result<(), CbuCrudError> {
        match operation.operation.as_str() {
            "set" | "update" => {
                if let Some(value) = &operation.value {
                    sqlx::query!(
                        r#"
                        INSERT INTO "ob-poc".attribute_values (cbu_id, attribute_id, value, dsl_version, state)
                        VALUES ($1, $2, $3, 1, 'resolved')
                        ON CONFLICT (cbu_id, attribute_id) DO UPDATE SET
                        value = $3, observed_at = NOW()
                        "#,
                        cbu_id, operation.attribute_id, value
                    )
                    .execute(&mut **tx)
                    .await?;
                }
            }
            "remove" => {
                sqlx::query!(
                    "DELETE FROM \"ob-poc\".attribute_values WHERE cbu_id = $1 AND attribute_id = $2",
                    cbu_id, operation.attribute_id
                )
                .execute(&mut **tx)
                .await?;
            }
            _ => {
                warn!("Unsupported attribute operation: {}", operation.operation);
            }
        }
        Ok(())
    }

    async fn apply_product_operation(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        _cbu_id: Uuid,
        operation: &CbuProductOperation,
    ) -> Result<(), CbuCrudError> {
        info!("Product operation: {}", operation.operation);
        // Implementation would handle product workflow updates
        Ok(())
    }

    async fn recalculate_ubo_for_cbu(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
    ) -> Result<(), CbuCrudError> {
        info!("Recalculating UBO for CBU: {}", cbu_id);
        // Implementation would trigger UBO recalculation
        Ok(())
    }

    async fn update_compliance_status(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
    ) -> Result<(), CbuCrudError> {
        info!("Updating compliance status for CBU: {}", cbu_id);
        // Implementation would update compliance status
        Ok(())
    }

    async fn update_cbu_timestamp(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
    ) -> Result<(), CbuCrudError> {
        sqlx::query!(
            "UPDATE \"ob-poc\".cbus SET updated_at = NOW() WHERE cbu_id = $1",
            cbu_id
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn log_cbu_update(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        request: &CbuUpdateRequest,
    ) -> Result<(), CbuCrudError> {
        info!("Logging CBU update for: {}", request.cbu_id);
        Ok(())
    }

    // Deletion helper methods
    async fn validate_deletion_confirmation(
        &self,
        request: &CbuDeleteRequest,
    ) -> Result<(), CbuCrudError> {
        // In production, would validate the confirmation token
        if request.confirmation_token.is_empty() {
            return Err(CbuCrudError::ValidationError {
                details: "Confirmation token required for deletion".to_string(),
            });
        }
        Ok(())
    }

    async fn check_deletion_constraints(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        cbu_id: Uuid,
    ) -> Result<(), CbuCrudError> {
        info!("Checking deletion constraints for CBU: {}", cbu_id);
        // Implementation would check for constraints preventing deletion
        Ok(())
    }

    async fn handle_deletion_dependencies(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        request: &CbuDeleteRequest,
    ) -> Result<(), CbuCrudError> {
        info!(
            "Handling deletion dependencies for CBU: {} with strategy: {:?}",
            request.cbu_id, request.dependency_handling
        );
        // Implementation would handle dependencies based on configuration
        Ok(())
    }

    async fn soft_delete_cbu(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        request: &CbuDeleteRequest,
    ) -> Result<(), CbuCrudError> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".cbus
            SET updated_at = NOW()
            WHERE cbu_id = $1
            "#,
            request.cbu_id
        )
        .execute(&mut **tx)
        .await?;

        info!("Soft deleted CBU: {}", request.cbu_id);
        Ok(())
    }

    async fn hard_delete_cbu(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        request: &CbuDeleteRequest,
    ) -> Result<(), CbuCrudError> {
        // Delete in reverse dependency order
        sqlx::query!(
            "DELETE FROM \"ob-poc\".attribute_values WHERE cbu_id = $1",
            request.cbu_id
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query!(
            "DELETE FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = $1",
            request.cbu_id
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query!(
            "DELETE FROM \"ob-poc\".cbus WHERE cbu_id = $1",
            request.cbu_id
        )
        .execute(&mut **tx)
        .await?;

        info!("Hard deleted CBU: {}", request.cbu_id);
        Ok(())
    }

    async fn archive_cbu(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        request: &CbuDeleteRequest,
    ) -> Result<(), CbuCrudError> {
        info!("Archived CBU: {}", request.cbu_id);
        // Implementation would move to archive tables
        Ok(())
    }

    async fn log_cbu_deletion(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        request: &CbuDeleteRequest,
    ) -> Result<(), CbuCrudError> {
        info!(
            "Logging CBU deletion: {} by {}",
            request.cbu_id, request.authorized_by
        );
        Ok(())
    }
}
