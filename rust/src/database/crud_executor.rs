//! CRUD Executor - Bridge from CRUD IR to Domain Services
//!
//! Per Section 4.2 of the master architecture:
//! - CrudExecutor accepts CrudStatement + ExecutionContext
//! - Delegates to appropriate domain services
//! - Must NOT embed SQL; it orchestrates services
//! - Logs results via CrudService

use crate::cbu_model_dsl::ast::CbuModel;
use crate::database::{
    AttributeValuesService, CbuEntityRolesService, CbuService, DecisionService,
    DictionaryDatabaseService, DocumentService, EntityService, InvestigationService,
    LifecycleResourceService, MonitoringService, MonitoringSetupFields, NewCbuFields,
    NewConditionFields, NewDecisionFields, NewDocumentFields, NewEntityFields,
    NewInvestigationFields, NewLifecycleResourceFields, NewLimitedCompanyFields,
    NewMonitoringEventFields, NewPartnershipFields, NewPepScreeningFields, NewProductFields,
    NewProperPersonFields, NewRiskAssessmentFields, NewRiskFlagFields, NewSanctionsScreeningFields,
    NewScheduledReviewFields, NewServiceFields, NewTrustFields, ProductService, RiskRatingFields,
    RiskService, ScreeningResolutionFields, ScreeningResultFields, ScreeningService,
    ServiceService,
};
use crate::forth_engine::env::RuntimeEnv;
use crate::forth_engine::value::{
    CrudStatement, DataCreate, DataDelete, DataRead, DataUpdate, DataUpsert, Value,
};
use anyhow::{anyhow, Context, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Execution context for CRUD operations
/// Contains model and source information for proper routing
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// CBU Model for validation and routing
    pub cbu_model: Option<CbuModel>,
    /// Model ID
    pub model_id: Option<String>,
    /// DSL instance ID for source tracking
    pub dsl_instance_id: Option<Uuid>,
    /// Document ID for source tracking
    pub document_id: Option<Uuid>,
    /// Current DSL version
    pub dsl_version: i32,
    /// Current chunks being processed
    pub chunks: Vec<String>,
}

/// Result of executing a CRUD statement
#[derive(Debug, Clone)]
pub struct CrudExecutionResult {
    /// Type of operation executed
    pub operation: String,
    /// Asset/table affected
    pub asset: String,
    /// Number of rows affected
    pub rows_affected: u64,
    /// Generated ID (for creates)
    pub generated_id: Option<Uuid>,
    /// Retrieved data (for reads)
    pub data: Option<JsonValue>,
}

/// Executor for CRUD statements - bridges IR to domain services
pub struct CrudExecutor {
    #[allow(dead_code)]
    pool: PgPool,
    cbu_service: CbuService,
    entity_service: EntityService,
    document_service: DocumentService,
    #[allow(dead_code)]
    cbu_entity_roles_service: CbuEntityRolesService,
    attribute_values_service: AttributeValuesService,
    dictionary_service: DictionaryDatabaseService,
    product_service: ProductService,
    service_service: ServiceService,
    lifecycle_resource_service: LifecycleResourceService,
    // KYC Investigation services
    investigation_service: InvestigationService,
    screening_service: ScreeningService,
    risk_service: RiskService,
    decision_service: DecisionService,
    monitoring_service: MonitoringService,
}

impl CrudExecutor {
    /// Create a new CRUD executor with all domain services
    pub fn new(pool: PgPool) -> Self {
        Self {
            cbu_service: CbuService::new(pool.clone()),
            entity_service: EntityService::new(pool.clone()),
            document_service: DocumentService::new(pool.clone()),
            cbu_entity_roles_service: CbuEntityRolesService::new(pool.clone()),
            attribute_values_service: AttributeValuesService::new(pool.clone()),
            dictionary_service: DictionaryDatabaseService::new(pool.clone()),
            product_service: ProductService::new(pool.clone()),
            service_service: ServiceService::new(pool.clone()),
            lifecycle_resource_service: LifecycleResourceService::new(pool.clone()),
            // KYC Investigation services
            investigation_service: InvestigationService::new(pool.clone()),
            screening_service: ScreeningService::new(pool.clone()),
            risk_service: RiskService::new(pool.clone()),
            decision_service: DecisionService::new(pool.clone()),
            monitoring_service: MonitoringService::new(pool.clone()),
            pool,
        }
    }

    /// Execute a single CRUD statement
    pub async fn execute(&self, stmt: &CrudStatement) -> Result<CrudExecutionResult> {
        match stmt {
            CrudStatement::DataCreate(create) => self.execute_create(create).await,
            CrudStatement::DataRead(read) => self.execute_read(read).await,
            CrudStatement::DataUpdate(update) => self.execute_update(update).await,
            CrudStatement::DataDelete(delete) => self.execute_delete(delete).await,
            CrudStatement::DataUpsert(upsert) => self.execute_upsert(upsert).await,
        }
    }

    /// Execute multiple CRUD statements
    pub async fn execute_all(&self, stmts: &[CrudStatement]) -> Result<Vec<CrudExecutionResult>> {
        let mut results = Vec::new();

        for stmt in stmts {
            let result = self.execute(stmt).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Execute multiple CRUD statements with RuntimeEnv for state validation
    ///
    /// This method validates state transitions against the CBU Model before execution
    /// and updates the environment state after successful operations.
    pub async fn execute_all_with_env(
        &self,
        stmts: &[CrudStatement],
        env: &mut RuntimeEnv,
    ) -> Result<Vec<CrudExecutionResult>> {
        let mut results = Vec::new();

        for stmt in stmts {
            // Validate state transition if we have a model and this affects CBU state
            if let Some(target_state) = self.extract_target_state(stmt) {
                if let Some(_model) = env.get_cbu_model() {
                    // Check if transition is valid
                    if !env.is_valid_transition(&target_state) {
                        let current = env.get_cbu_state().unwrap_or("unknown");
                        return Err(anyhow!(
                            "Invalid state transition from '{}' to '{}'. Check CBU Model definition.",
                            current, target_state
                        ));
                    }

                    // Check preconditions
                    let missing = env.check_transition_preconditions(&target_state);
                    if !missing.is_empty() {
                        return Err(anyhow!(
                            "Missing required attributes for transition to '{}': {}",
                            target_state,
                            missing.join(", ")
                        ));
                    }

                    debug!("State transition to '{}' validated", target_state);
                }
            }

            // Inject context IDs from env into statement values before execution
            let stmt_with_context = self.inject_context_ids(stmt, env);

            // Execute the CRUD statement
            let result = self.execute(&stmt_with_context).await?;

            // Capture result into RuntimeEnv if requested by the statement
            self.capture_result_if_needed(&stmt_with_context, &result, env);

            // Update environment state after successful CBU operations
            if result.asset == "CBU" {
                // Update state based on operation
                if let Some(new_state) = self.extract_target_state(stmt) {
                    env.set_cbu_state(new_state);
                    debug!("CBU state updated in environment");
                }
            }

            results.push(result);
        }

        Ok(results)
    }

    /// Extract target state from a CRUD statement if it's a state-changing operation
    fn extract_target_state(&self, stmt: &CrudStatement) -> Option<String> {
        match stmt {
            CrudStatement::DataCreate(_create) if _create.asset == "CBU" => {
                // CREATE sets initial state, not a transition
                None
            }
            CrudStatement::DataUpdate(update) if update.asset == "CBU" => {
                // Check for status/state field in update values
                self.get_string_value(&update.values, "status")
                    .or_else(|| self.get_string_value(&update.values, "state"))
            }
            _ => None,
        }
    }

    /// Inject context IDs from RuntimeEnv into statement values before execution
    ///
    /// This enables late binding: when `cbu.ensure` runs first and captures its ID,
    /// subsequent statements like `risk.assess-cbu` will have the cbu_id injected
    /// even though the word was parsed before the CBU existed.
    fn inject_context_ids(&self, stmt: &CrudStatement, env: &RuntimeEnv) -> CrudStatement {
        match stmt {
            CrudStatement::DataCreate(create) => {
                let mut values = create.values.clone();
                self.inject_context_values(&mut values, env);
                CrudStatement::DataCreate(DataCreate {
                    asset: create.asset.clone(),
                    values,
                    capture_result: create.capture_result.clone(),
                })
            }
            CrudStatement::DataUpsert(upsert) => {
                let mut values = upsert.values.clone();
                self.inject_context_values(&mut values, env);
                CrudStatement::DataUpsert(DataUpsert {
                    asset: upsert.asset.clone(),
                    values,
                    conflict_keys: upsert.conflict_keys.clone(),
                    capture_result: upsert.capture_result.clone(),
                })
            }
            CrudStatement::DataUpdate(update) => {
                let mut values = update.values.clone();
                let mut where_clause = update.where_clause.clone();
                self.inject_context_values(&mut values, env);
                self.inject_context_values(&mut where_clause, env);
                CrudStatement::DataUpdate(DataUpdate {
                    asset: update.asset.clone(),
                    values,
                    where_clause,
                })
            }
            // Read and Delete don't need context injection for now
            _ => stmt.clone(),
        }
    }

    /// Helper to inject context values into a HashMap
    fn inject_context_values(
        &self,
        values: &mut std::collections::HashMap<String, Value>,
        env: &RuntimeEnv,
    ) {
        // Inject cbu_id if not present
        if !values.contains_key("cbu-id") {
            if let Some(cbu_id) = &env.cbu_id {
                values.insert("cbu-id".to_string(), Value::Str(cbu_id.to_string()));
            }
        }
        // Inject entity_id if not present
        if !values.contains_key("entity-id") {
            if let Some(entity_id) = &env.entity_id {
                values.insert("entity-id".to_string(), Value::Str(entity_id.to_string()));
            }
        }
        // Inject investigation_id if not present
        if !values.contains_key("investigation-id") {
            if let Some(inv_id) = &env.investigation_id {
                values.insert(
                    "investigation-id".to_string(),
                    Value::Str(inv_id.to_string()),
                );
            }
        }
        // Inject decision_id if not present
        if !values.contains_key("decision-id") {
            if let Some(dec_id) = &env.decision_id {
                values.insert("decision-id".to_string(), Value::Str(dec_id.to_string()));
            }
        }
    }

    /// Capture execution result into RuntimeEnv if requested by the statement
    ///
    /// This enables context propagation: when a word like `cbu.ensure` creates a CBU,
    /// the returned UUID is captured into `env.cbu_id` so subsequent words can use it.
    fn capture_result_if_needed(
        &self,
        stmt: &CrudStatement,
        result: &CrudExecutionResult,
        env: &mut RuntimeEnv,
    ) {
        let capture_key = match stmt {
            CrudStatement::DataCreate(c) => c.capture_result.as_ref(),
            CrudStatement::DataUpsert(u) => u.capture_result.as_ref(),
            _ => None,
        };

        if let Some(key) = capture_key {
            if let Some(id) = result.generated_id {
                match key.as_str() {
                    "cbu_id" => {
                        env.set_cbu_id(id);
                        info!("Captured cbu_id into context: {}", id);
                    }
                    "entity_id" => {
                        env.set_entity_id(id);
                        info!("Captured entity_id into context: {}", id);
                    }
                    "investigation_id" => {
                        env.set_investigation_id(id);
                        info!("Captured investigation_id into context: {}", id);
                    }
                    "decision_id" => {
                        env.set_decision_id(id);
                        info!("Captured decision_id into context: {}", id);
                    }
                    _ => {
                        warn!("Unknown capture key: {}", key);
                    }
                }
            }
        }
    }

    /// Execute a CREATE CBU with model-aware value splitting
    ///
    /// Per Section 5.2 of the plan:
    /// - Split DSL values into core CBU fields vs attribute values
    /// - Use dictionary to confirm sink includes "CBU"
    /// - Include source metadata with DSL CRUD doc id + chunk info
    pub async fn execute_create_cbu_with_context(
        &self,
        create: &DataCreate,
        ctx: &ExecutionContext,
    ) -> Result<CrudExecutionResult> {
        // Split values into CBU core fields and attribute values
        let (cbu_fields, attr_values) = self.split_cbu_values(create, ctx)?;

        // Create CBU row
        let cbu_id = self.cbu_service.create_cbu(&cbu_fields).await?;

        // Insert attribute values with source tracking
        for (attr_id, value, chunk_name) in attr_values {
            let source = serde_json::json!({
                "type": "DSL.CRUD.CBU",
                "dsl_instance_id": ctx.dsl_instance_id.map(|id| id.to_string()),
                "document_id": ctx.document_id.map(|id| id.to_string()),
                "model_id": ctx.model_id,
                "chunk": chunk_name,
            });

            // Try to resolve attribute ID from dictionary name
            let attr_uuid = self
                .resolve_attribute_id(&attr_id)
                .await
                .unwrap_or_else(|_| Uuid::new_v4()); // Generate if not found

            self.attribute_values_service
                .upsert_for_cbu(
                    cbu_id,
                    ctx.dsl_version,
                    attr_uuid,
                    value,
                    "user-input",
                    Some(source),
                )
                .await?;
        }

        info!(
            "Created CBU with model context: {} ({})",
            cbu_fields.name, cbu_id
        );

        Ok(CrudExecutionResult {
            operation: "CREATE".to_string(),
            asset: "CBU".to_string(),
            rows_affected: 1,
            generated_id: Some(cbu_id),
            data: None,
        })
    }

    /// Split DSL values into CBU core fields and attribute values
    #[allow(clippy::type_complexity)]
    fn split_cbu_values(
        &self,
        create: &DataCreate,
        ctx: &ExecutionContext,
    ) -> Result<(NewCbuFields, Vec<(String, JsonValue, String)>)> {
        // Core CBU fields that go into the cbus table
        let core_fields = [
            "cbu-name",
            "name",
            "description",
            "nature-purpose",
            "client-type",
            "jurisdiction",
        ];

        let name = self
            .get_string_value(&create.values, "cbu-name")
            .or_else(|| self.get_string_value(&create.values, "name"))
            .unwrap_or_else(|| "Unknown".to_string());

        let client_type = self.get_string_value(&create.values, "client-type");

        let jurisdiction = self.get_string_value(&create.values, "jurisdiction");

        let nature_purpose = self.get_string_value(&create.values, "nature-purpose");

        let description = self.get_string_value(&create.values, "description");

        let cbu_fields = NewCbuFields {
            name,
            description,
            nature_purpose,
            client_type,
            jurisdiction,
        };

        // Remaining fields become attribute values
        let mut attr_values = Vec::new();

        for (key, value) in &create.values {
            // Skip core fields
            if core_fields.contains(&key.as_str()) {
                continue;
            }

            // Map DSL keyword to attribute ID
            let attr_id = map_dsl_keyword_to_attr(key);
            let json_value = self.value_to_json(value);

            // Determine which chunk this attribute belongs to
            let chunk_name = self.find_chunk_for_attribute(&attr_id, ctx);

            attr_values.push((attr_id, json_value, chunk_name));
        }

        Ok((cbu_fields, attr_values))
    }

    /// Find which chunk an attribute belongs to
    fn find_chunk_for_attribute(&self, attr_id: &str, ctx: &ExecutionContext) -> String {
        if let Some(model) = &ctx.cbu_model {
            for group in &model.attributes.groups {
                if group.contains(attr_id) {
                    return group.name.clone();
                }
            }
        }

        // Default to first current chunk or "unknown"
        ctx.chunks
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Resolve attribute dictionary name to UUID
    async fn resolve_attribute_id(&self, attr_name: &str) -> Result<Uuid> {
        // Use DictionaryDatabaseService to look up attribute
        let result = self.dictionary_service.get_by_name(attr_name).await?;

        result
            .map(|attr| attr.attribute_id)
            .ok_or_else(|| anyhow!("Attribute '{}' not found in dictionary", attr_name))
    }

    /// Execute a CREATE statement by delegating to appropriate service
    async fn execute_create(&self, create: &DataCreate) -> Result<CrudExecutionResult> {
        match create.asset.as_str() {
            "CBU" => {
                // Map DSL fields to canonical DB columns
                // DSL keywords: :cbu-name, :client-type, :jurisdiction, :nature-purpose, :description
                let name = self
                    .get_string_value(&create.values, "cbu-name")
                    .or_else(|| self.get_string_value(&create.values, "name"))
                    .unwrap_or_else(|| "Unknown".to_string());

                let client_type = self.get_string_value(&create.values, "client-type");

                let jurisdiction = self.get_string_value(&create.values, "jurisdiction");

                let nature_purpose = self.get_string_value(&create.values, "nature-purpose");

                let description = self.get_string_value(&create.values, "description");

                let fields = NewCbuFields {
                    name: name.clone(),
                    description,
                    nature_purpose,
                    client_type,
                    jurisdiction,
                };

                let cbu_id = self.cbu_service.create_cbu(&fields).await?;

                info!("Created CBU: {} ({})", name, cbu_id);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: 1,
                    generated_id: Some(cbu_id),
                    data: None,
                })
            }

            "ENTITY" | "CBU_ENTITY_RELATIONSHIP" => {
                // Map DSL fields to normalized entity model per Section 3.3
                let entity_type = self
                    .get_string_value(&create.values, "entity-type")
                    .or_else(|| self.get_string_value(&create.values, "role"))
                    .unwrap_or_else(|| "UNKNOWN".to_string());

                let name = self
                    .get_string_value(&create.values, "name")
                    .or_else(|| self.get_string_value(&create.values, "entity-id"))
                    .unwrap_or_else(|| format!("Entity-{}", Uuid::new_v4()));

                let fields = NewEntityFields {
                    entity_type,
                    name: name.clone(),
                    external_id: self.get_string_value(&create.values, "external-id"),
                };

                let entity_id = self.entity_service.create_entity(&fields).await?;

                info!("Created entity: {} ({})", name, entity_id);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: create.asset.clone(),
                    rows_affected: 1,
                    generated_id: Some(entity_id),
                    data: None,
                })
            }

            "PROPER_PERSON" | "CBU_PROPER_PERSON" => {
                // Map DSL fields to proper person model
                let first_name = self
                    .get_string_value(&create.values, "first-name")
                    .unwrap_or_else(|| {
                        // Fallback: try to split person-name or name
                        let person_name = self
                            .get_string_value(&create.values, "person-name")
                            .or_else(|| self.get_string_value(&create.values, "name"))
                            .unwrap_or_else(|| "Unknown".to_string());
                        let parts: Vec<&str> = person_name.split_whitespace().collect();
                        parts
                            .first()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "Unknown".to_string())
                    });

                let last_name = self
                    .get_string_value(&create.values, "last-name")
                    .unwrap_or_else(|| {
                        let person_name = self
                            .get_string_value(&create.values, "person-name")
                            .or_else(|| self.get_string_value(&create.values, "name"))
                            .unwrap_or_default();
                        let parts: Vec<&str> = person_name.split_whitespace().collect();
                        if parts.len() > 1 {
                            parts[1..].join(" ")
                        } else {
                            String::new()
                        }
                    });

                let fields = NewProperPersonFields {
                    first_name: first_name.clone(),
                    last_name: last_name.clone(),
                    middle_names: self.get_string_value(&create.values, "middle-name"),
                    date_of_birth: self.get_date_value(&create.values, "date-of-birth"),
                    nationality: self.get_string_value(&create.values, "nationality"),
                    residence_address: self.get_string_value(&create.values, "residence-address"),
                    id_document_type: self.get_string_value(&create.values, "id-document-type"),
                    id_document_number: self.get_string_value(&create.values, "id-document-number"),
                };

                let (entity_id, _proper_person_id) =
                    self.entity_service.create_proper_person(&fields).await?;

                info!(
                    "Created proper person: {} {} ({})",
                    first_name, last_name, entity_id
                );

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "PROPER_PERSON".to_string(),
                    rows_affected: 1,
                    generated_id: Some(entity_id),
                    data: None,
                })
            }

            "LIMITED_COMPANY" => {
                let name = self
                    .get_string_value(&create.values, "name")
                    .or_else(|| self.get_string_value(&create.values, "company-name"))
                    .unwrap_or_else(|| "Unknown Company".to_string());

                let fields = NewLimitedCompanyFields {
                    name: name.clone(),
                    jurisdiction: self.get_string_value(&create.values, "jurisdiction"),
                    registration_number: self
                        .get_string_value(&create.values, "registration-number")
                        .or_else(|| self.get_string_value(&create.values, "company-number")),
                    incorporation_date: self.get_date_value(&create.values, "incorporation-date"),
                    registered_address: self
                        .get_string_value(&create.values, "registered-office")
                        .or_else(|| self.get_string_value(&create.values, "registered-address")),
                    business_nature: self.get_string_value(&create.values, "business-nature"),
                };

                let limited_company_id =
                    self.entity_service.create_limited_company(&fields).await?;

                info!("Created limited company: {} ({})", name, limited_company_id);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "LIMITED_COMPANY".to_string(),
                    rows_affected: 1,
                    generated_id: Some(limited_company_id),
                    data: None,
                })
            }

            "PARTNERSHIP" => {
                let name = self
                    .get_string_value(&create.values, "name")
                    .or_else(|| self.get_string_value(&create.values, "partnership-name"))
                    .unwrap_or_else(|| "Unknown Partnership".to_string());

                let fields = NewPartnershipFields {
                    name: name.clone(),
                    jurisdiction: self.get_string_value(&create.values, "jurisdiction"),
                    partnership_type: self.get_string_value(&create.values, "partnership-type"),
                    formation_date: self.get_date_value(&create.values, "formation-date"),
                    principal_place_business: self
                        .get_string_value(&create.values, "principal-place-business"),
                    partnership_agreement_date: self
                        .get_date_value(&create.values, "agreement-date")
                        .or_else(|| {
                            self.get_date_value(&create.values, "partnership-agreement-date")
                        }),
                };

                let partnership_id = self.entity_service.create_partnership(&fields).await?;

                info!("Created partnership: {} ({})", name, partnership_id);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "PARTNERSHIP".to_string(),
                    rows_affected: 1,
                    generated_id: Some(partnership_id),
                    data: None,
                })
            }

            "TRUST" => {
                let name = self
                    .get_string_value(&create.values, "name")
                    .or_else(|| self.get_string_value(&create.values, "trust-name"))
                    .unwrap_or_else(|| "Unknown Trust".to_string());

                let jurisdiction = self
                    .get_string_value(&create.values, "jurisdiction")
                    .unwrap_or_else(|| "Unknown".to_string());

                let fields = NewTrustFields {
                    name: name.clone(),
                    jurisdiction,
                    trust_type: self.get_string_value(&create.values, "trust-type"),
                    establishment_date: self
                        .get_date_value(&create.values, "formation-date")
                        .or_else(|| self.get_date_value(&create.values, "establishment-date")),
                    trust_deed_date: self.get_date_value(&create.values, "trust-deed-date"),
                    trust_purpose: self.get_string_value(&create.values, "trust-purpose"),
                    governing_law: self.get_string_value(&create.values, "governing-law"),
                };

                let trust_id = self.entity_service.create_trust(&fields).await?;

                info!("Created trust: {} ({})", name, trust_id);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "TRUST".to_string(),
                    rows_affected: 1,
                    generated_id: Some(trust_id),
                    data: None,
                })
            }

            "CBU_ENTITY_ROLE" => {
                // THE CRITICAL FIX: Actually link entity to CBU via cbu_entity_roles
                let cbu_id = self
                    .get_uuid_value(&create.values, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for CBU_ENTITY_ROLE"))?;

                let entity_id = self
                    .get_uuid_value(&create.values, "entity-id")
                    .ok_or_else(|| anyhow!("entity-id required for CBU_ENTITY_ROLE"))?;

                let role = self
                    .get_string_value(&create.values, "role")
                    .ok_or_else(|| anyhow!("role required for CBU_ENTITY_ROLE"))?;

                let cbu_entity_role_id = self
                    .entity_service
                    .attach_entity_to_cbu(cbu_id, entity_id, &role)
                    .await?;

                info!(
                    "Attached entity {} to CBU {} with role '{}'",
                    entity_id, cbu_id, role
                );

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "CBU_ENTITY_ROLE".to_string(),
                    rows_affected: 1,
                    generated_id: Some(cbu_entity_role_id),
                    data: Some(serde_json::json!({
                        "cbu_id": cbu_id.to_string(),
                        "entity_id": entity_id.to_string(),
                        "role": role
                    })),
                })
            }

            "ATTRIBUTE" => {
                // Map DSL attribute set to attribute_values per Section 3.2
                let cbu_id_str = self
                    .get_string_value(&create.values, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for ATTRIBUTE create"))?;
                let cbu_id = Uuid::parse_str(&cbu_id_str)?;

                let attr_id_str = self
                    .get_string_value(&create.values, "attribute-id")
                    .ok_or_else(|| anyhow!("attribute-id required"))?;
                let attribute_id = Uuid::parse_str(&attr_id_str)?;

                let value = create
                    .values
                    .get("value")
                    .map(|v| self.value_to_json(v))
                    .unwrap_or(JsonValue::Null);

                let state = self
                    .get_string_value(&create.values, "state")
                    .unwrap_or_else(|| "proposed".to_string());

                let dsl_version = self
                    .get_int_value(&create.values, "dsl-version")
                    .unwrap_or(1) as i32;

                self.attribute_values_service
                    .upsert_for_cbu(cbu_id, dsl_version, attribute_id, value, &state, None)
                    .await?;

                info!(
                    "Set attribute {} for CBU {} (version {})",
                    attribute_id, cbu_id, dsl_version
                );

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "ATTRIBUTE".to_string(),
                    rows_affected: 1,
                    generated_id: None,
                    data: None,
                })
            }

            "DOCUMENT" => {
                // Map DSL fields to document catalog
                // DSL keywords: :doc-id, :doc-type, :document-code, :title, :cbu-id
                let document_code = self
                    .get_string_value(&create.values, "doc-id")
                    .or_else(|| self.get_string_value(&create.values, "document-code"))
                    .unwrap_or_else(|| format!("DOC-{}", Uuid::new_v4()));

                let doc_type_code = self
                    .get_string_value(&create.values, "doc-type")
                    .or_else(|| self.get_string_value(&create.values, "document-type"))
                    .unwrap_or_else(|| "GENERAL".to_string());

                // Look up document type ID
                let document_type_id = self
                    .document_service
                    .get_document_type_id_by_code(&doc_type_code)
                    .await?
                    .ok_or_else(|| anyhow!("Document type '{}' not found", doc_type_code))?;

                // Parse CBU ID if provided
                let cbu_id =
                    if let Some(cbu_id_str) = self.get_string_value(&create.values, "cbu-id") {
                        Some(Uuid::parse_str(&cbu_id_str)?)
                    } else {
                        None
                    };

                // Parse issuer ID if provided
                let issuer_id =
                    if let Some(issuer_str) = self.get_string_value(&create.values, "issuer-id") {
                        Some(Uuid::parse_str(&issuer_str)?)
                    } else {
                        None
                    };

                let fields = NewDocumentFields {
                    document_code: document_code.clone(),
                    document_type_id,
                    issuer_id,
                    title: self.get_string_value(&create.values, "title"),
                    description: self.get_string_value(&create.values, "description"),
                    file_hash: self.get_string_value(&create.values, "file-hash"),
                    file_path: self.get_string_value(&create.values, "file-path"),
                    mime_type: self.get_string_value(&create.values, "mime-type"),
                    confidentiality_level: self
                        .get_string_value(&create.values, "confidentiality-level"),
                    cbu_id,
                };

                let document_id = self.document_service.create_document(&fields).await?;

                info!("Created document: {} ({})", document_code, document_id);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "DOCUMENT".to_string(),
                    rows_affected: 1,
                    generated_id: Some(document_id),
                    data: None,
                })
            }

            "DOCUMENT_METADATA" => {
                // Extract attribute value from document and store in document_metadata
                // DSL keywords: :doc-id, :attr-id, :cbu-id, :method, :value
                let doc_id_str = self
                    .get_string_value(&create.values, "doc-id")
                    .ok_or_else(|| anyhow!("doc-id required for DOCUMENT_METADATA"))?;
                let doc_id = Uuid::parse_str(&doc_id_str)?;

                let attr_id_str = self
                    .get_string_value(&create.values, "attr-id")
                    .ok_or_else(|| anyhow!("attr-id required for DOCUMENT_METADATA"))?;
                let attribute_id = Uuid::parse_str(&attr_id_str)?;

                let extraction_method = self
                    .get_string_value(&create.values, "method")
                    .unwrap_or_else(|| "MANUAL".to_string());

                // Get value - for now use placeholder, actual extraction happens elsewhere
                let value = create
                    .values
                    .get("value")
                    .map(|v| self.value_to_json(v))
                    .unwrap_or(serde_json::json!(null));

                // Insert into document_metadata
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".document_metadata
                    (doc_id, attribute_id, value, extraction_method, extracted_at)
                    VALUES ($1, $2, $3, $4, NOW())
                    ON CONFLICT (doc_id, attribute_id)
                    DO UPDATE SET value = $3, extraction_method = $4, extracted_at = NOW()
                    "#,
                    doc_id,
                    attribute_id,
                    value,
                    extraction_method
                )
                .execute(&self.pool)
                .await
                .context("Failed to insert document metadata")?;

                info!(
                    "Extracted attribute {} from document {}",
                    attribute_id, doc_id
                );

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "DOCUMENT_METADATA".to_string(),
                    rows_affected: 1,
                    generated_id: None,
                    data: None,
                })
            }

            "Product" | "PRODUCT" => {
                let name = self
                    .get_string_value(&create.values, "name")
                    .or_else(|| self.get_string_value(&create.values, "product-name"))
                    .unwrap_or_else(|| "Unknown Product".to_string());
                let fields = NewProductFields {
                    name: name.clone(),
                    description: self.get_string_value(&create.values, "description"),
                    product_code: self.get_string_value(&create.values, "product-code"),
                    product_category: self.get_string_value(&create.values, "product-category"),
                    regulatory_framework: self
                        .get_string_value(&create.values, "regulatory-framework"),
                    min_asset_requirement: None,
                    is_active: Some(true),
                    metadata: None,
                };
                let product_id = self.product_service.create_product(&fields).await?;
                info!("Created product: {} ({})", name, product_id);
                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "Product".to_string(),
                    rows_affected: 1,
                    generated_id: Some(product_id),
                    data: None,
                })
            }

            "Service" | "SERVICE" => {
                let name = self
                    .get_string_value(&create.values, "name")
                    .or_else(|| self.get_string_value(&create.values, "service-name"))
                    .unwrap_or_else(|| "Unknown Service".to_string());
                let fields = NewServiceFields {
                    name: name.clone(),
                    description: self.get_string_value(&create.values, "description"),
                    service_code: self.get_string_value(&create.values, "service-code"),
                    service_category: self.get_string_value(&create.values, "service-category"),
                    sla_definition: None,
                    is_active: Some(true),
                };
                let service_id = self.service_service.create_service(&fields).await?;
                info!("Created service: {} ({})", name, service_id);
                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "Service".to_string(),
                    rows_affected: 1,
                    generated_id: Some(service_id),
                    data: None,
                })
            }

            "LifecycleResource" | "LIFECYCLE_RESOURCE" => {
                let name = self
                    .get_string_value(&create.values, "name")
                    .or_else(|| self.get_string_value(&create.values, "resource-name"))
                    .unwrap_or_else(|| "Unknown Resource".to_string());
                let owner = self
                    .get_string_value(&create.values, "owner")
                    .unwrap_or_else(|| "system".to_string());
                let fields = NewLifecycleResourceFields {
                    name: name.clone(),
                    description: self.get_string_value(&create.values, "description"),
                    owner,
                    dictionary_group: self.get_string_value(&create.values, "dictionary-group"),
                    resource_code: self.get_string_value(&create.values, "resource-code"),
                    resource_type: self.get_string_value(&create.values, "resource-type"),
                    vendor: self.get_string_value(&create.values, "vendor"),
                    version: self.get_string_value(&create.values, "version"),
                    api_endpoint: self.get_string_value(&create.values, "api-endpoint"),
                    api_version: self.get_string_value(&create.values, "api-version"),
                    authentication_method: self
                        .get_string_value(&create.values, "authentication-method"),
                    is_active: Some(true),
                };
                let resource_id = self
                    .lifecycle_resource_service
                    .create_lifecycle_resource(&fields)
                    .await?;
                info!("Created lifecycle resource: {} ({})", name, resource_id);
                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "LifecycleResource".to_string(),
                    rows_affected: 1,
                    generated_id: Some(resource_id),
                    data: None,
                })
            }

            // KYC Investigation assets
            "INVESTIGATION" | "KYC_INVESTIGATION" => {
                self.execute_create_investigation(&create.values).await
            }

            "SCREENING_PEP" => self.execute_create_screening("PEP", &create.values).await,

            "SCREENING_SANCTIONS" => {
                self.execute_create_screening("SANCTIONS", &create.values)
                    .await
            }

            "SCREENING_ADVERSE_MEDIA" => {
                self.execute_create_screening("ADVERSE_MEDIA", &create.values)
                    .await
            }

            "SCREENING_RESULT" => {
                let screening_id = self
                    .get_uuid_value(&create.values, "screening-id")
                    .ok_or_else(|| anyhow!("screening-id required for SCREENING_RESULT"))?;

                let result = self
                    .get_string_value(&create.values, "result")
                    .ok_or_else(|| anyhow!("result required for SCREENING_RESULT"))?;

                let fields = ScreeningResultFields {
                    screening_id,
                    result: result.clone(),
                    match_details: self.get_json_value(&create.values, "match-details"),
                    reviewed_by: self.get_string_value(&create.values, "reviewed-by"),
                };

                self.screening_service.record_result(&fields).await?;

                info!(
                    "Recorded screening result '{}' for {}",
                    result, screening_id
                );

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "SCREENING_RESULT".to_string(),
                    rows_affected: 1,
                    generated_id: Some(screening_id),
                    data: None,
                })
            }

            "SCREENING_RESOLUTION" => {
                let screening_id = self
                    .get_uuid_value(&create.values, "screening-id")
                    .ok_or_else(|| anyhow!("screening-id required for SCREENING_RESOLUTION"))?;

                let resolution = self
                    .get_string_value(&create.values, "resolution")
                    .ok_or_else(|| anyhow!("resolution required for SCREENING_RESOLUTION"))?;

                let fields = ScreeningResolutionFields {
                    screening_id,
                    resolution: resolution.clone(),
                    rationale: self.get_string_value(&create.values, "rationale"),
                    resolved_by: self.get_string_value(&create.values, "resolved-by"),
                };

                self.screening_service.resolve(&fields).await?;

                info!("Resolved screening {} as '{}'", screening_id, resolution);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "SCREENING_RESOLUTION".to_string(),
                    rows_affected: 1,
                    generated_id: Some(screening_id),
                    data: None,
                })
            }

            "RISK_ASSESSMENT_ENTITY" => {
                self.execute_create_risk_assessment("ENTITY", &create.values)
                    .await
            }

            "RISK_ASSESSMENT_CBU" => {
                self.execute_create_risk_assessment("CBU", &create.values)
                    .await
            }

            "RISK_RATING" => {
                let fields = RiskRatingFields {
                    cbu_id: self.get_uuid_value(&create.values, "cbu-id"),
                    entity_id: self.get_uuid_value(&create.values, "entity-id"),
                    investigation_id: self.get_uuid_value(&create.values, "investigation-id"),
                    rating: self
                        .get_string_value(&create.values, "rating")
                        .ok_or_else(|| anyhow!("rating required for RISK_RATING"))?,
                    factors: self.get_json_value(&create.values, "factors"),
                    rationale: self.get_string_value(&create.values, "rationale"),
                    assessed_by: self.get_string_value(&create.values, "assessed-by"),
                };

                let assessment_id = self.risk_service.set_rating(&fields).await?;

                info!(
                    "Set risk rating '{}' for assessment {}",
                    fields.rating, assessment_id
                );

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "RISK_RATING".to_string(),
                    rows_affected: 1,
                    generated_id: Some(assessment_id),
                    data: None,
                })
            }

            "RISK_FLAG" => {
                let fields = NewRiskFlagFields {
                    cbu_id: self.get_uuid_value(&create.values, "cbu-id"),
                    entity_id: self.get_uuid_value(&create.values, "entity-id"),
                    investigation_id: self.get_uuid_value(&create.values, "investigation-id"),
                    flag_type: self
                        .get_string_value(&create.values, "flag-type")
                        .ok_or_else(|| anyhow!("flag-type required for RISK_FLAG"))?,
                    description: self.get_string_value(&create.values, "description"),
                    flagged_by: self.get_string_value(&create.values, "flagged-by"),
                };

                let flag_id = self.risk_service.add_flag(&fields).await?;

                info!("Added {} flag {}", fields.flag_type, flag_id);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "RISK_FLAG".to_string(),
                    rows_affected: 1,
                    generated_id: Some(flag_id),
                    data: None,
                })
            }

            "DECISION" | "KYC_DECISION" => self.execute_create_decision(&create.values).await,

            "DECISION_CONDITION" => {
                let decision_id = self
                    .get_uuid_value(&create.values, "decision-id")
                    .ok_or_else(|| anyhow!("decision-id required for DECISION_CONDITION"))?;

                let fields = NewConditionFields {
                    decision_id,
                    condition_type: self
                        .get_string_value(&create.values, "condition-type")
                        .ok_or_else(|| anyhow!("condition-type required"))?,
                    description: self.get_string_value(&create.values, "description"),
                    frequency: self.get_string_value(&create.values, "frequency"),
                    due_date: self.get_date_value(&create.values, "due-date"),
                    threshold: self
                        .get_string_value(&create.values, "threshold")
                        .and_then(|s| s.parse().ok()),
                    currency: self.get_string_value(&create.values, "currency"),
                    assigned_to: self.get_string_value(&create.values, "assigned-to"),
                };

                let condition_id = self.decision_service.add_condition(&fields).await?;

                info!(
                    "Added condition {} to decision {}",
                    condition_id, decision_id
                );

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "DECISION_CONDITION".to_string(),
                    rows_affected: 1,
                    generated_id: Some(condition_id),
                    data: None,
                })
            }

            "MONITORING_SETUP" => self.execute_create_monitoring_setup(&create.values).await,

            "MONITORING_EVENT" => {
                let cbu_id = self
                    .get_uuid_value(&create.values, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for MONITORING_EVENT"))?;

                let fields = NewMonitoringEventFields {
                    cbu_id,
                    event_type: self
                        .get_string_value(&create.values, "event-type")
                        .ok_or_else(|| anyhow!("event-type required"))?,
                    description: self.get_string_value(&create.values, "description"),
                    severity: self.get_string_value(&create.values, "severity"),
                    requires_review: self
                        .get_string_value(&create.values, "requires-review")
                        .map(|s| s.to_lowercase() == "true"),
                };

                let event_id = self.monitoring_service.record_event(&fields).await?;

                info!("Recorded monitoring event {} for CBU {}", event_id, cbu_id);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "MONITORING_EVENT".to_string(),
                    rows_affected: 1,
                    generated_id: Some(event_id),
                    data: None,
                })
            }

            "SCHEDULED_REVIEW" => {
                let cbu_id = self
                    .get_uuid_value(&create.values, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for SCHEDULED_REVIEW"))?;

                let fields = NewScheduledReviewFields {
                    cbu_id,
                    review_type: self
                        .get_string_value(&create.values, "review-type")
                        .ok_or_else(|| anyhow!("review-type required"))?,
                    due_date: self
                        .get_date_value(&create.values, "due-date")
                        .ok_or_else(|| anyhow!("due-date required"))?,
                    assigned_to: self.get_string_value(&create.values, "assigned-to"),
                };

                let review_id = self.monitoring_service.schedule_review(&fields).await?;

                info!("Scheduled review {} for CBU {}", review_id, cbu_id);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "SCHEDULED_REVIEW".to_string(),
                    rows_affected: 1,
                    generated_id: Some(review_id),
                    data: None,
                })
            }

            _ => {
                warn!("Unknown asset type for CREATE: {}", create.asset);
                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: create.asset.clone(),
                    rows_affected: 0,
                    generated_id: None,
                    data: None,
                })
            }
        }
    }

    /// Execute a READ statement
    async fn execute_read(&self, read: &DataRead) -> Result<CrudExecutionResult> {
        match read.asset.as_str() {
            "CBU" => {
                let cbu_id = self.get_string_value(&read.where_clause, "cbu-id");

                let data = if let Some(id) = cbu_id {
                    let uuid = Uuid::parse_str(&id)?;
                    if let Some(cbu) = self.cbu_service.get_cbu_by_id(uuid).await? {
                        vec![serde_json::json!({
                            "cbu_id": cbu.cbu_id.to_string(),
                            "name": cbu.name,
                            "description": cbu.description,
                            "nature_purpose": cbu.nature_purpose
                        })]
                    } else {
                        vec![]
                    }
                } else {
                    let limit = read.limit.map(|l| l as i32);
                    let cbus = self.cbu_service.list_cbus(limit, None).await?;
                    cbus.into_iter()
                        .map(|cbu| {
                            serde_json::json!({
                                "cbu_id": cbu.cbu_id.to_string(),
                                "name": cbu.name,
                                "description": cbu.description,
                                "nature_purpose": cbu.nature_purpose
                            })
                        })
                        .collect()
                };

                Ok(CrudExecutionResult {
                    operation: "READ".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: data.len() as u64,
                    generated_id: None,
                    data: Some(JsonValue::Array(data)),
                })
            }

            "Product" | "PRODUCT" => {
                let product_id = self.get_string_value(&read.where_clause, "product-id");
                let data = if let Some(id) = product_id {
                    let uuid = Uuid::parse_str(&id)?;
                    if let Some(product) = self.product_service.get_product_by_id(uuid).await? {
                        vec![
                            serde_json::json!({"product_id": product.product_id.to_string(), "name": product.name, "description": product.description}),
                        ]
                    } else {
                        vec![]
                    }
                } else {
                    let products = self
                        .product_service
                        .list_products(read.limit.map(|l| l as i32), None)
                        .await?;
                    products.into_iter().map(|p| serde_json::json!({"product_id": p.product_id.to_string(), "name": p.name, "description": p.description})).collect()
                };
                Ok(CrudExecutionResult {
                    operation: "READ".to_string(),
                    asset: "Product".to_string(),
                    rows_affected: data.len() as u64,
                    generated_id: None,
                    data: Some(JsonValue::Array(data)),
                })
            }

            "Service" | "SERVICE" => {
                let service_id = self.get_string_value(&read.where_clause, "service-id");
                let data = if let Some(id) = service_id {
                    let uuid = Uuid::parse_str(&id)?;
                    if let Some(service) = self.service_service.get_service_by_id(uuid).await? {
                        vec![
                            serde_json::json!({"service_id": service.service_id.to_string(), "name": service.name, "description": service.description}),
                        ]
                    } else {
                        vec![]
                    }
                } else {
                    let services = self
                        .service_service
                        .list_services(read.limit.map(|l| l as i32), None)
                        .await?;
                    services.into_iter().map(|s| serde_json::json!({"service_id": s.service_id.to_string(), "name": s.name, "description": s.description})).collect()
                };
                Ok(CrudExecutionResult {
                    operation: "READ".to_string(),
                    asset: "Service".to_string(),
                    rows_affected: data.len() as u64,
                    generated_id: None,
                    data: Some(JsonValue::Array(data)),
                })
            }

            "LifecycleResource" | "LIFECYCLE_RESOURCE" => {
                let resource_id = self.get_string_value(&read.where_clause, "resource-id");
                let data = if let Some(id) = resource_id {
                    let uuid = Uuid::parse_str(&id)?;
                    if let Some(resource) = self
                        .lifecycle_resource_service
                        .get_lifecycle_resource_by_id(uuid)
                        .await?
                    {
                        vec![
                            serde_json::json!({"resource_id": resource.resource_id.to_string(), "name": resource.name, "description": resource.description, "owner": resource.owner}),
                        ]
                    } else {
                        vec![]
                    }
                } else {
                    let resources = self
                        .lifecycle_resource_service
                        .list_lifecycle_resources(read.limit.map(|l| l as i32), None)
                        .await?;
                    resources.into_iter().map(|r| serde_json::json!({"resource_id": r.resource_id.to_string(), "name": r.name, "description": r.description, "owner": r.owner})).collect()
                };
                Ok(CrudExecutionResult {
                    operation: "READ".to_string(),
                    asset: "LifecycleResource".to_string(),
                    rows_affected: data.len() as u64,
                    generated_id: None,
                    data: Some(JsonValue::Array(data)),
                })
            }

            "CBU_ENTITY_ROLE" => {
                // List entities attached to a CBU
                let cbu_id = self
                    .get_uuid_value(&read.where_clause, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for CBU_ENTITY_ROLE read"))?;

                let role = self.get_string_value(&read.where_clause, "role");

                let entities = self
                    .entity_service
                    .list_cbu_entities(cbu_id, role.as_deref())
                    .await?;

                let data: Vec<JsonValue> = entities
                    .into_iter()
                    .map(|e| {
                        serde_json::json!({
                            "cbu_entity_role_id": e.cbu_entity_role_id.to_string(),
                            "cbu_id": e.cbu_id.to_string(),
                            "entity_id": e.entity_id.to_string(),
                            "role_id": e.role_id.to_string(),
                        })
                    })
                    .collect();

                Ok(CrudExecutionResult {
                    operation: "READ".to_string(),
                    asset: "CBU_ENTITY_ROLE".to_string(),
                    rows_affected: data.len() as u64,
                    generated_id: None,
                    data: Some(JsonValue::Array(data)),
                })
            }

            _ => {
                warn!("Unknown asset type for READ: {}", read.asset);
                Ok(CrudExecutionResult {
                    operation: "READ".to_string(),
                    asset: read.asset.clone(),
                    rows_affected: 0,
                    generated_id: None,
                    data: Some(JsonValue::Array(vec![])),
                })
            }
        }
    }

    /// Execute an UPDATE statement
    async fn execute_update(&self, update: &DataUpdate) -> Result<CrudExecutionResult> {
        match update.asset.as_str() {
            "CBU" => {
                let cbu_id_str = self
                    .get_string_value(&update.where_clause, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for UPDATE"))?;
                let cbu_id = Uuid::parse_str(&cbu_id_str)?;

                let name = self.get_string_value(&update.values, "name");
                let description = self
                    .get_string_value(&update.values, "description")
                    .or_else(|| self.get_string_value(&update.values, "status"));
                let nature_purpose = self.get_string_value(&update.values, "nature-purpose");

                let updated = self
                    .cbu_service
                    .update_cbu(
                        cbu_id,
                        name.as_deref(),
                        description.as_deref(),
                        nature_purpose.as_deref(),
                    )
                    .await?;

                info!("Updated CBU: {}", cbu_id);

                Ok(CrudExecutionResult {
                    operation: "UPDATE".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: if updated { 1 } else { 0 },
                    generated_id: None,
                    data: None,
                })
            }

            "Product" | "PRODUCT" => {
                let product_id_str = self
                    .get_string_value(&update.where_clause, "product-id")
                    .ok_or_else(|| anyhow!("product-id required for UPDATE"))?;
                let product_id = Uuid::parse_str(&product_id_str)?;
                let name = self.get_string_value(&update.values, "name");
                let description = self.get_string_value(&update.values, "description");
                let updated = self
                    .product_service
                    .update_product(product_id, name.as_deref(), description.as_deref())
                    .await?;
                info!("Updated Product: {}", product_id);
                Ok(CrudExecutionResult {
                    operation: "UPDATE".to_string(),
                    asset: "Product".to_string(),
                    rows_affected: if updated { 1 } else { 0 },
                    generated_id: None,
                    data: None,
                })
            }

            "Service" | "SERVICE" => {
                let service_id_str = self
                    .get_string_value(&update.where_clause, "service-id")
                    .ok_or_else(|| anyhow!("service-id required for UPDATE"))?;
                let service_id = Uuid::parse_str(&service_id_str)?;
                let name = self.get_string_value(&update.values, "name");
                let description = self.get_string_value(&update.values, "description");
                let updated = self
                    .service_service
                    .update_service(service_id, name.as_deref(), description.as_deref())
                    .await?;
                info!("Updated Service: {}", service_id);
                Ok(CrudExecutionResult {
                    operation: "UPDATE".to_string(),
                    asset: "Service".to_string(),
                    rows_affected: if updated { 1 } else { 0 },
                    generated_id: None,
                    data: None,
                })
            }

            "LifecycleResource" | "LIFECYCLE_RESOURCE" => {
                let resource_id_str = self
                    .get_string_value(&update.where_clause, "resource-id")
                    .ok_or_else(|| anyhow!("resource-id required for UPDATE"))?;
                let resource_id = Uuid::parse_str(&resource_id_str)?;
                let name = self.get_string_value(&update.values, "name");
                let description = self.get_string_value(&update.values, "description");
                let owner = self.get_string_value(&update.values, "owner");
                let updated = self
                    .lifecycle_resource_service
                    .update_lifecycle_resource(
                        resource_id,
                        name.as_deref(),
                        description.as_deref(),
                        owner.as_deref(),
                    )
                    .await?;
                info!("Updated Lifecycle Resource: {}", resource_id);
                Ok(CrudExecutionResult {
                    operation: "UPDATE".to_string(),
                    asset: "LifecycleResource".to_string(),
                    rows_affected: if updated { 1 } else { 0 },
                    generated_id: None,
                    data: None,
                })
            }

            _ => {
                warn!("Unknown asset type for UPDATE: {}", update.asset);
                Ok(CrudExecutionResult {
                    operation: "UPDATE".to_string(),
                    asset: update.asset.clone(),
                    rows_affected: 0,
                    generated_id: None,
                    data: None,
                })
            }
        }
    }

    /// Execute a DELETE statement
    async fn execute_delete(&self, delete: &DataDelete) -> Result<CrudExecutionResult> {
        match delete.asset.as_str() {
            "CBU" => {
                let cbu_id_str = self
                    .get_string_value(&delete.where_clause, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for DELETE"))?;
                let cbu_id = Uuid::parse_str(&cbu_id_str)?;

                let deleted = self.cbu_service.delete_cbu(cbu_id).await?;

                info!("Deleted CBU: {}", cbu_id);

                Ok(CrudExecutionResult {
                    operation: "DELETE".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: if deleted { 1 } else { 0 },
                    generated_id: None,
                    data: None,
                })
            }

            "Product" | "PRODUCT" => {
                let product_id_str = self
                    .get_string_value(&delete.where_clause, "product-id")
                    .ok_or_else(|| anyhow!("product-id required for DELETE"))?;
                let product_id = Uuid::parse_str(&product_id_str)?;
                let deleted = self.product_service.delete_product(product_id).await?;
                info!("Deleted Product: {}", product_id);
                Ok(CrudExecutionResult {
                    operation: "DELETE".to_string(),
                    asset: "Product".to_string(),
                    rows_affected: if deleted { 1 } else { 0 },
                    generated_id: None,
                    data: None,
                })
            }

            "Service" | "SERVICE" => {
                let service_id_str = self
                    .get_string_value(&delete.where_clause, "service-id")
                    .ok_or_else(|| anyhow!("service-id required for DELETE"))?;
                let service_id = Uuid::parse_str(&service_id_str)?;
                let deleted = self.service_service.delete_service(service_id).await?;
                info!("Deleted Service: {}", service_id);
                Ok(CrudExecutionResult {
                    operation: "DELETE".to_string(),
                    asset: "Service".to_string(),
                    rows_affected: if deleted { 1 } else { 0 },
                    generated_id: None,
                    data: None,
                })
            }

            "LifecycleResource" | "LIFECYCLE_RESOURCE" => {
                let resource_id_str = self
                    .get_string_value(&delete.where_clause, "resource-id")
                    .ok_or_else(|| anyhow!("resource-id required for DELETE"))?;
                let resource_id = Uuid::parse_str(&resource_id_str)?;
                let deleted = self
                    .lifecycle_resource_service
                    .delete_lifecycle_resource(resource_id)
                    .await?;
                info!("Deleted Lifecycle Resource: {}", resource_id);
                Ok(CrudExecutionResult {
                    operation: "DELETE".to_string(),
                    asset: "LifecycleResource".to_string(),
                    rows_affected: if deleted { 1 } else { 0 },
                    generated_id: None,
                    data: None,
                })
            }

            "CBU_ENTITY_ROLE" => {
                // Detach entity from CBU
                let cbu_id = self
                    .get_uuid_value(&delete.where_clause, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for CBU_ENTITY_ROLE delete"))?;

                let entity_id = self
                    .get_uuid_value(&delete.where_clause, "entity-id")
                    .ok_or_else(|| anyhow!("entity-id required for CBU_ENTITY_ROLE delete"))?;

                let role = self.get_string_value(&delete.where_clause, "role");

                let rows_affected = self
                    .entity_service
                    .detach_entity_from_cbu(cbu_id, entity_id, role.as_deref())
                    .await?;

                info!(
                    "Detached entity {} from CBU {} (rows: {})",
                    entity_id, cbu_id, rows_affected
                );

                Ok(CrudExecutionResult {
                    operation: "DELETE".to_string(),
                    asset: "CBU_ENTITY_ROLE".to_string(),
                    rows_affected,
                    generated_id: None,
                    data: None,
                })
            }

            _ => {
                warn!("Unknown asset type for DELETE: {}", delete.asset);
                Ok(CrudExecutionResult {
                    operation: "DELETE".to_string(),
                    asset: delete.asset.clone(),
                    rows_affected: 0,
                    generated_id: None,
                    data: None,
                })
            }
        }
    }

    /// Helper to extract string value from HashMap
    fn get_string_value(
        &self,
        values: &std::collections::HashMap<String, Value>,
        key: &str,
    ) -> Option<String> {
        values.get(key).and_then(|v| match v {
            Value::Str(s) => Some(s.clone()),
            _ => None,
        })
    }

    /// Helper to extract integer value from HashMap
    fn get_int_value(
        &self,
        values: &std::collections::HashMap<String, Value>,
        key: &str,
    ) -> Option<i64> {
        values.get(key).and_then(|v| match v {
            Value::Float(n) => Some(*n as i64),
            _ => None,
        })
    }

    /// Helper to extract date value from HashMap (expects YYYY-MM-DD format)
    fn get_date_value(
        &self,
        values: &std::collections::HashMap<String, Value>,
        key: &str,
    ) -> Option<chrono::NaiveDate> {
        self.get_string_value(values, key)
            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
    }

    /// Helper to extract UUID value from HashMap
    fn get_uuid_value(
        &self,
        values: &std::collections::HashMap<String, Value>,
        key: &str,
    ) -> Option<Uuid> {
        self.get_string_value(values, key)
            .and_then(|s| Uuid::parse_str(&s).ok())
    }

    /// Convert AST Value to JSON Value
    fn value_to_json(&self, value: &Value) -> JsonValue {
        match value {
            Value::Str(s) => JsonValue::String(s.clone()),
            Value::Float(n) => serde_json::Number::from_f64(*n)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null),
            Value::Bool(b) => JsonValue::Bool(*b),
            Value::List(items) => {
                JsonValue::Array(items.iter().map(|v| self.value_to_json(v)).collect())
            }
            Value::Map(pairs) => {
                let map: serde_json::Map<String, JsonValue> = pairs
                    .iter()
                    .map(|(k, v)| (k.clone(), self.value_to_json(v)))
                    .collect();
                JsonValue::Object(map)
            }
            _ => JsonValue::Null,
        }
    }

    /// Helper to extract JSON value from HashMap
    fn get_json_value(
        &self,
        values: &std::collections::HashMap<String, Value>,
        key: &str,
    ) -> Option<JsonValue> {
        values.get(key).map(|v| self.value_to_json(v))
    }

    // =========================================================================
    // UPSERT Operations (Idempotent create-or-update)
    // =========================================================================

    /// Execute an UPSERT statement (idempotent create-or-update using natural keys)
    async fn execute_upsert(&self, upsert: &DataUpsert) -> Result<CrudExecutionResult> {
        match upsert.asset.as_str() {
            "CBU" => {
                // UPSERT CBU using natural key: (name) - names are globally unique per cbus_name_key constraint
                let name = self
                    .get_string_value(&upsert.values, "cbu-name")
                    .or_else(|| self.get_string_value(&upsert.values, "name"))
                    .ok_or_else(|| anyhow!("cbu-name required for CBU UPSERT"))?;

                let jurisdiction = self.get_string_value(&upsert.values, "jurisdiction");
                let nature_purpose = self.get_string_value(&upsert.values, "nature-purpose");
                let description = self.get_string_value(&upsert.values, "description");
                let client_type = self.get_string_value(&upsert.values, "client-type");

                let cbu_id = sqlx::query_scalar::<_, Uuid>(
                    r#"
                    INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, nature_purpose, description, client_type, created_at, updated_at)
                    VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, NOW(), NOW())
                    ON CONFLICT (name)
                    DO UPDATE SET
                        jurisdiction = COALESCE(EXCLUDED.jurisdiction, cbus.jurisdiction),
                        nature_purpose = COALESCE(EXCLUDED.nature_purpose, cbus.nature_purpose),
                        description = COALESCE(EXCLUDED.description, cbus.description),
                        client_type = COALESCE(EXCLUDED.client_type, cbus.client_type),
                        updated_at = NOW()
                    RETURNING cbu_id
                    "#,
                )
                .bind(&name)
                .bind(&jurisdiction)
                .bind(&nature_purpose)
                .bind(&description)
                .bind(&client_type)
                .fetch_one(&self.pool)
                .await
                .context("Failed to upsert CBU")?;

                info!("Upserted CBU: {} ({})", name, cbu_id);

                Ok(CrudExecutionResult {
                    operation: "UPSERT".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: 1,
                    generated_id: Some(cbu_id),
                    data: None,
                })
            }

            "LIMITED_COMPANY" => {
                // UPSERT Limited Company using natural key: company_number
                let name = self
                    .get_string_value(&upsert.values, "name")
                    .or_else(|| self.get_string_value(&upsert.values, "company-name"))
                    .ok_or_else(|| anyhow!("name required for LIMITED_COMPANY UPSERT"))?;

                let company_number = self
                    .get_string_value(&upsert.values, "company-number")
                    .or_else(|| self.get_string_value(&upsert.values, "registration-number"));

                let jurisdiction = self.get_string_value(&upsert.values, "jurisdiction");

                // First upsert the base entity
                let entity_id = sqlx::query_scalar::<_, Uuid>(
                    r#"
                    INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name, jurisdiction, created_at)
                    SELECT gen_random_uuid(), et.entity_type_id, $1, $2, NOW()
                    FROM "ob-poc".entity_types et WHERE et.type_code = 'LIMITED_COMPANY'
                    ON CONFLICT (name, jurisdiction) WHERE entity_type_id = (
                        SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = 'LIMITED_COMPANY'
                    )
                    DO UPDATE SET updated_at = NOW()
                    RETURNING entity_id
                    "#,
                )
                .bind(&name)
                .bind(&jurisdiction)
                .fetch_one(&self.pool)
                .await
                .context("Failed to upsert LIMITED_COMPANY entity")?;

                // Then upsert the extension table
                let incorporation_date = self.get_date_value(&upsert.values, "incorporation-date");
                let registered_address = self
                    .get_string_value(&upsert.values, "registered-office")
                    .or_else(|| self.get_string_value(&upsert.values, "registered-address"));

                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".entity_limited_companies
                        (company_id, entity_id, company_number, incorporation_date, registered_address)
                    VALUES (gen_random_uuid(), $1, $2, $3, $4)
                    ON CONFLICT (entity_id)
                    DO UPDATE SET
                        company_number = COALESCE(EXCLUDED.company_number, entity_limited_companies.company_number),
                        incorporation_date = COALESCE(EXCLUDED.incorporation_date, entity_limited_companies.incorporation_date),
                        registered_address = COALESCE(EXCLUDED.registered_address, entity_limited_companies.registered_address)
                    "#,
                )
                .bind(entity_id)
                .bind(&company_number)
                .bind(incorporation_date)
                .bind(&registered_address)
                .execute(&self.pool)
                .await
                .context("Failed to upsert LIMITED_COMPANY extension")?;

                info!("Upserted LIMITED_COMPANY: {} ({})", name, entity_id);

                Ok(CrudExecutionResult {
                    operation: "UPSERT".to_string(),
                    asset: "LIMITED_COMPANY".to_string(),
                    rows_affected: 1,
                    generated_id: Some(entity_id),
                    data: None,
                })
            }

            "PROPER_PERSON" => {
                // UPSERT Proper Person using natural key: tax_id OR (first_name, last_name, date_of_birth)
                let first_name = self
                    .get_string_value(&upsert.values, "first-name")
                    .unwrap_or_else(|| {
                        let person_name = self
                            .get_string_value(&upsert.values, "name")
                            .unwrap_or_default();
                        person_name
                            .split_whitespace()
                            .next()
                            .unwrap_or("Unknown")
                            .to_string()
                    });

                let last_name = self
                    .get_string_value(&upsert.values, "last-name")
                    .unwrap_or_else(|| {
                        let person_name = self
                            .get_string_value(&upsert.values, "name")
                            .unwrap_or_default();
                        let parts: Vec<&str> = person_name.split_whitespace().collect();
                        if parts.len() > 1 {
                            parts[1..].join(" ")
                        } else {
                            String::new()
                        }
                    });

                let full_name = format!("{} {}", first_name, last_name);
                let nationality = self.get_string_value(&upsert.values, "nationality");
                let tax_id = self.get_string_value(&upsert.values, "tax-id");

                // Upsert base entity
                let entity_id = sqlx::query_scalar::<_, Uuid>(
                    r#"
                    INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name, created_at)
                    SELECT gen_random_uuid(), et.entity_type_id, $1, NOW()
                    FROM "ob-poc".entity_types et WHERE et.type_code = 'PROPER_PERSON'
                    ON CONFLICT (name, jurisdiction) WHERE entity_type_id = (
                        SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = 'PROPER_PERSON'
                    )
                    DO UPDATE SET updated_at = NOW()
                    RETURNING entity_id
                    "#,
                )
                .bind(&full_name)
                .fetch_one(&self.pool)
                .await
                .context("Failed to upsert PROPER_PERSON entity")?;

                // Upsert extension
                let date_of_birth = self.get_date_value(&upsert.values, "date-of-birth");

                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".entity_proper_persons
                        (person_id, entity_id, first_name, last_name, date_of_birth, nationality, tax_id)
                    VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6)
                    ON CONFLICT (entity_id)
                    DO UPDATE SET
                        first_name = EXCLUDED.first_name,
                        last_name = EXCLUDED.last_name,
                        date_of_birth = COALESCE(EXCLUDED.date_of_birth, entity_proper_persons.date_of_birth),
                        nationality = COALESCE(EXCLUDED.nationality, entity_proper_persons.nationality),
                        tax_id = COALESCE(EXCLUDED.tax_id, entity_proper_persons.tax_id)
                    "#,
                )
                .bind(entity_id)
                .bind(&first_name)
                .bind(&last_name)
                .bind(date_of_birth)
                .bind(&nationality)
                .bind(&tax_id)
                .execute(&self.pool)
                .await
                .context("Failed to upsert PROPER_PERSON extension")?;

                info!("Upserted PROPER_PERSON: {} ({})", full_name, entity_id);

                Ok(CrudExecutionResult {
                    operation: "UPSERT".to_string(),
                    asset: "PROPER_PERSON".to_string(),
                    rows_affected: 1,
                    generated_id: Some(entity_id),
                    data: None,
                })
            }

            "CBU_ENTITY_ROLE" => {
                // UPSERT CBU-Entity-Role using natural key: (cbu_id, entity_id, role_id)
                let cbu_id = self
                    .get_uuid_value(&upsert.values, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for CBU_ENTITY_ROLE UPSERT"))?;

                let entity_id = self
                    .get_uuid_value(&upsert.values, "entity-id")
                    .ok_or_else(|| anyhow!("entity-id required for CBU_ENTITY_ROLE UPSERT"))?;

                let role = self
                    .get_string_value(&upsert.values, "role")
                    .ok_or_else(|| anyhow!("role required for CBU_ENTITY_ROLE UPSERT"))?;

                let ownership_percent = self
                    .get_string_value(&upsert.values, "ownership-percent")
                    .and_then(|s| s.parse::<f64>().ok());

                // This uses the existing UNIQUE constraint on (cbu_id, entity_id, role_id)
                let cbu_entity_role_id = self
                    .entity_service
                    .attach_entity_to_cbu(cbu_id, entity_id, &role)
                    .await?;

                info!(
                    "Upserted CBU_ENTITY_ROLE: CBU {} + Entity {} as {} ({})",
                    cbu_id, entity_id, role, cbu_entity_role_id
                );

                Ok(CrudExecutionResult {
                    operation: "UPSERT".to_string(),
                    asset: "CBU_ENTITY_ROLE".to_string(),
                    rows_affected: 1,
                    generated_id: Some(cbu_entity_role_id),
                    data: Some(serde_json::json!({
                        "cbu_id": cbu_id.to_string(),
                        "entity_id": entity_id.to_string(),
                        "role": role,
                        "ownership_percent": ownership_percent
                    })),
                })
            }

            "OWNERSHIP_EDGE" => {
                // UPSERT ownership edge using natural key: (from_entity_id, to_entity_id, relationship_type)
                let from_entity_id = self
                    .get_uuid_value(&upsert.values, "from-entity-id")
                    .ok_or_else(|| anyhow!("from-entity-id required for OWNERSHIP_EDGE UPSERT"))?;

                let to_entity_id = self
                    .get_uuid_value(&upsert.values, "to-entity-id")
                    .ok_or_else(|| anyhow!("to-entity-id required for OWNERSHIP_EDGE UPSERT"))?;

                let ownership_percent = self
                    .get_string_value(&upsert.values, "ownership-percent")
                    .and_then(|s| s.parse::<f64>().ok());

                let ownership_type = self
                    .get_string_value(&upsert.values, "ownership-type")
                    .unwrap_or_else(|| "DIRECT".to_string());

                let control_type = self
                    .get_string_value(&upsert.values, "control-type")
                    .unwrap_or_else(|| "SHAREHOLDING".to_string());

                let effective_date = self.get_date_value(&upsert.values, "effective-date");

                let connection_id = sqlx::query_scalar::<_, Uuid>(
                    r#"
                    INSERT INTO "ob-poc".entity_role_connections
                        (connection_id, source_entity_id, target_entity_id, relationship_type,
                         ownership_percentage, control_type, effective_date, created_at)
                    VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, NOW())
                    ON CONFLICT (source_entity_id, target_entity_id, relationship_type)
                    DO UPDATE SET
                        ownership_percentage = COALESCE(EXCLUDED.ownership_percentage, entity_role_connections.ownership_percentage),
                        control_type = COALESCE(EXCLUDED.control_type, entity_role_connections.control_type),
                        effective_date = COALESCE(EXCLUDED.effective_date, entity_role_connections.effective_date),
                        updated_at = NOW()
                    RETURNING connection_id
                    "#,
                )
                .bind(from_entity_id)
                .bind(to_entity_id)
                .bind(&ownership_type)
                .bind(ownership_percent)
                .bind(&control_type)
                .bind(effective_date)
                .fetch_one(&self.pool)
                .await
                .context("Failed to upsert OWNERSHIP_EDGE")?;

                info!(
                    "Upserted OWNERSHIP_EDGE: {} -> {} ({})",
                    from_entity_id, to_entity_id, connection_id
                );

                Ok(CrudExecutionResult {
                    operation: "UPSERT".to_string(),
                    asset: "OWNERSHIP_EDGE".to_string(),
                    rows_affected: 1,
                    generated_id: Some(connection_id),
                    data: None,
                })
            }

            _ => {
                warn!(
                    "Unknown asset type for UPSERT: {}, falling back to CREATE",
                    upsert.asset
                );
                // Fall back to create behavior
                let create = DataCreate {
                    asset: upsert.asset.clone(),
                    values: upsert.values.clone(),
                    capture_result: upsert.capture_result.clone(),
                };
                self.execute_create(&create).await
            }
        }
    }

    // =========================================================================
    // KYC Investigation Operations
    // =========================================================================

    /// Execute investigation CREATE
    async fn execute_create_investigation(
        &self,
        values: &std::collections::HashMap<String, Value>,
    ) -> Result<CrudExecutionResult> {
        let investigation_type = self
            .get_string_value(values, "investigation-type")
            .unwrap_or_else(|| "STANDARD".to_string());

        let cbu_id = self.get_uuid_value(values, "cbu-id");

        let fields = NewInvestigationFields {
            cbu_id,
            investigation_type: investigation_type.clone(),
            risk_rating: self.get_string_value(values, "risk-rating"),
            regulatory_framework: self.get_json_value(values, "regulatory-framework"),
            ubo_threshold: self
                .get_string_value(values, "ubo-threshold")
                .and_then(|s| s.parse().ok()),
            investigation_depth: self
                .get_string_value(values, "investigation-depth")
                .and_then(|s| s.parse().ok()),
            deadline: self.get_date_value(values, "deadline"),
        };

        let investigation_id = self
            .investigation_service
            .create_investigation(&fields)
            .await?;

        info!(
            "Created investigation {} type '{}'",
            investigation_id, investigation_type
        );

        Ok(CrudExecutionResult {
            operation: "CREATE".to_string(),
            asset: "INVESTIGATION".to_string(),
            rows_affected: 1,
            generated_id: Some(investigation_id),
            data: None,
        })
    }

    /// Execute screening CREATE (PEP, Sanctions, Adverse Media)
    async fn execute_create_screening(
        &self,
        screening_type: &str,
        values: &std::collections::HashMap<String, Value>,
    ) -> Result<CrudExecutionResult> {
        let entity_id = self
            .get_uuid_value(values, "entity-id")
            .ok_or_else(|| anyhow!("entity-id required for screening"))?;

        let investigation_id = self.get_uuid_value(values, "investigation-id");

        let screening_id = match screening_type {
            "PEP" => {
                let fields = NewPepScreeningFields {
                    investigation_id,
                    entity_id,
                    databases: self.get_json_value(values, "databases"),
                    include_rca: self
                        .get_string_value(values, "include-rca")
                        .map(|s| s.to_lowercase() == "true"),
                };
                self.screening_service.create_pep_screening(&fields).await?
            }
            "SANCTIONS" => {
                let fields = NewSanctionsScreeningFields {
                    investigation_id,
                    entity_id,
                    lists: self.get_json_value(values, "lists"),
                };
                self.screening_service
                    .create_sanctions_screening(&fields)
                    .await?
            }
            "ADVERSE_MEDIA" => {
                let fields = crate::database::NewAdverseMediaScreeningFields {
                    investigation_id,
                    entity_id,
                    search_depth: self.get_string_value(values, "depth"),
                    languages: self.get_json_value(values, "languages"),
                };
                self.screening_service
                    .create_adverse_media_screening(&fields)
                    .await?
            }
            _ => return Err(anyhow!("Unknown screening type: {}", screening_type)),
        };

        info!(
            "Created {} screening {} for entity {}",
            screening_type, screening_id, entity_id
        );

        Ok(CrudExecutionResult {
            operation: "CREATE".to_string(),
            asset: format!("SCREENING_{}", screening_type),
            rows_affected: 1,
            generated_id: Some(screening_id),
            data: None,
        })
    }

    /// Execute risk assessment CREATE
    async fn execute_create_risk_assessment(
        &self,
        assessment_type: &str,
        values: &std::collections::HashMap<String, Value>,
    ) -> Result<CrudExecutionResult> {
        let fields = NewRiskAssessmentFields {
            cbu_id: self.get_uuid_value(values, "cbu-id"),
            entity_id: self.get_uuid_value(values, "entity-id"),
            investigation_id: self.get_uuid_value(values, "investigation-id"),
            assessment_type: assessment_type.to_string(),
            methodology: self.get_string_value(values, "methodology"),
        };

        let assessment_id = match assessment_type {
            "ENTITY" => self.risk_service.assess_entity(&fields).await?,
            "CBU" => self.risk_service.assess_cbu(&fields).await?,
            _ => return Err(anyhow!("Unknown assessment type: {}", assessment_type)),
        };

        info!(
            "Created {} risk assessment {}",
            assessment_type, assessment_id
        );

        Ok(CrudExecutionResult {
            operation: "CREATE".to_string(),
            asset: format!("RISK_ASSESSMENT_{}", assessment_type),
            rows_affected: 1,
            generated_id: Some(assessment_id),
            data: None,
        })
    }

    /// Execute decision CREATE
    async fn execute_create_decision(
        &self,
        values: &std::collections::HashMap<String, Value>,
    ) -> Result<CrudExecutionResult> {
        let cbu_id = self
            .get_uuid_value(values, "cbu-id")
            .ok_or_else(|| anyhow!("cbu-id required for decision"))?;

        let decision = self
            .get_string_value(values, "decision")
            .ok_or_else(|| anyhow!("decision required"))?;

        let fields = NewDecisionFields {
            cbu_id,
            investigation_id: self.get_uuid_value(values, "investigation-id"),
            decision: decision.clone(),
            decision_authority: self.get_string_value(values, "decision-authority"),
            rationale: self.get_string_value(values, "rationale"),
            decided_by: self.get_string_value(values, "decided-by"),
        };

        let decision_id = self.decision_service.record_decision(&fields).await?;

        info!(
            "Recorded decision {} '{}' for CBU {}",
            decision_id, decision, cbu_id
        );

        Ok(CrudExecutionResult {
            operation: "CREATE".to_string(),
            asset: "DECISION".to_string(),
            rows_affected: 1,
            generated_id: Some(decision_id),
            data: None,
        })
    }

    /// Execute monitoring setup CREATE
    async fn execute_create_monitoring_setup(
        &self,
        values: &std::collections::HashMap<String, Value>,
    ) -> Result<CrudExecutionResult> {
        let cbu_id = self
            .get_uuid_value(values, "cbu-id")
            .ok_or_else(|| anyhow!("cbu-id required for monitoring setup"))?;

        let monitoring_level = self
            .get_string_value(values, "monitoring-level")
            .unwrap_or_else(|| "STANDARD".to_string());

        let fields = MonitoringSetupFields {
            cbu_id,
            monitoring_level: monitoring_level.clone(),
            components: self.get_json_value(values, "components"),
        };

        let setup_id = self.monitoring_service.setup_monitoring(&fields).await?;

        info!(
            "Setup {} monitoring for CBU {} ({})",
            monitoring_level, cbu_id, setup_id
        );

        Ok(CrudExecutionResult {
            operation: "CREATE".to_string(),
            asset: "MONITORING_SETUP".to_string(),
            rows_affected: 1,
            generated_id: Some(setup_id),
            data: None,
        })
    }
}

/// Map DSL keyword back to attribute ID
/// Inverse of the mapping used in template generation
fn map_dsl_keyword_to_attr(keyword: &str) -> String {
    let kw = keyword.trim_start_matches(':');
    match kw {
        "cbu-name" => "CBU.LEGAL_NAME".into(),
        "jurisdiction" => "CBU.JURISDICTION".into(),
        "nature-purpose" => "CBU.NATURE_PURPOSE".into(),
        "entity-type" => "CBU.ENTITY_TYPE".into(),
        "registered-address" => "CBU.REGISTERED_ADDRESS".into(),
        "primary-contact-email" => "CBU.PRIMARY_CONTACT_EMAIL".into(),
        "primary-contact-name" => "CBU.PRIMARY_CONTACT_NAME".into(),
        "primary-contact-phone" => "CBU.PRIMARY_CONTACT_PHONE".into(),
        "trading-name" => "CBU.TRADING_NAME".into(),
        "lei" => "CBU.LEI".into(),
        "client-type" => "CBU.ENTITY_TYPE".into(),
        "beneficial-owner-name" => "UBO.BENEFICIAL_OWNER_NAME".into(),
        "ownership-percentage" => "UBO.OWNERSHIP_PERCENTAGE".into(),
        "nationality" => "UBO.NATIONALITY".into(),
        "tax-residency" => "UBO.TAX_RESIDENCY".into(),
        _ => kw.replace('-', ".").to_uppercase(),
    }
}
