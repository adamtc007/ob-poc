//! CRUD Executor - Bridge from CRUD IR to Domain Services
//!
//! Per Section 4.2 of the master architecture:
//! - CrudExecutor accepts CrudStatement + ExecutionContext
//! - Delegates to appropriate domain services
//! - Must NOT embed SQL; it orchestrates services
//! - Logs results via CrudService

use crate::cbu_model_dsl::ast::CbuModel;
use crate::database::{
    AttributeValuesService, CbuEntityRolesService, CbuService, DictionaryDatabaseService,
    DocumentService, EntityService, NewCbuFields, NewDocumentFields, NewEntityFields,
    NewProperPersonFields,
};
use crate::forth_engine::env::RuntimeEnv;
use crate::parser::ast::{
    CrudStatement, DataCreate, DataDelete, DataRead, DataUpdate, Literal, Value,
};
use anyhow::{anyhow, Result};
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
            _ => Err(anyhow!("Unsupported CRUD statement type")),
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

            // Execute the CRUD statement
            let result = self.execute(stmt).await?;

            // Update environment state after successful CBU operations
            if result.asset == "CBU" {
                // Set CBU ID in environment if created
                if let Some(id) = result.generated_id {
                    env.set_cbu_id(id);
                }

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
            CrudStatement::DataCreate(create) if create.asset == "CBU" => {
                // New CBU starts in initial state (handled by model)
                Some("Proposed".to_string())
            }
            CrudStatement::DataUpdate(update) if update.asset == "CBU" => {
                // Check for status/state field in update values
                self.get_string_value(&update.values, "status")
                    .or_else(|| self.get_string_value(&update.values, "state"))
            }
            _ => None,
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
                let person_name = self
                    .get_string_value(&create.values, "person-name")
                    .or_else(|| self.get_string_value(&create.values, "name"))
                    .unwrap_or_else(|| "Unknown Person".to_string());

                // Split name into first/last
                let parts: Vec<&str> = person_name.split_whitespace().collect();
                let (first_name, last_name) = if parts.len() >= 2 {
                    (parts[0].to_string(), parts[1..].join(" "))
                } else {
                    (person_name.clone(), String::new())
                };

                let fields = NewProperPersonFields {
                    first_name,
                    last_name,
                    middle_names: None,
                    date_of_birth: None,
                    nationality: self.get_string_value(&create.values, "nationality"),
                    residence_address: None,
                    id_document_type: None,
                    id_document_number: None,
                };

                let (entity_id, _proper_person_id) =
                    self.entity_service.create_proper_person(&fields).await?;

                info!("Created proper person: {} ({})", person_name, entity_id);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "PROPER_PERSON".to_string(),
                    rows_affected: 1,
                    generated_id: Some(entity_id),
                    data: None,
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
            Value::Literal(Literal::String(s)) => Some(s.clone()),
            Value::Identifier(s) => Some(s.clone()),
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
            Value::Literal(Literal::Number(n)) => Some(*n as i64),
            _ => None,
        })
    }

    /// Convert AST Value to JSON Value
    fn value_to_json(&self, value: &Value) -> JsonValue {
        match value {
            Value::Literal(Literal::String(s)) => JsonValue::String(s.clone()),
            Value::Literal(Literal::Number(n)) => serde_json::Number::from_f64(*n)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null),
            Value::Literal(Literal::Boolean(b)) => JsonValue::Bool(*b),
            Value::Identifier(s) => JsonValue::String(s.clone()),
            Value::Json(j) => j.clone(),
            _ => JsonValue::Null,
        }
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
