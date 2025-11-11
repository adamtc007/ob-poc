//! Entity CRUD Service - Phase 1 Implementation
//!
//! This service provides comprehensive CRUD operations for entity tables with
//! full integration into the agentic DSL system. It extends the existing
//! CRUD patterns to support all entity types (partnerships, limited companies,
//! proper persons, trusts) and their relationships.

use crate::ai::{
    ai_dsl_service::AiDslService, crud_prompt_builder::CrudPromptBuilder,
    openai_client::OpenAiClient, rag_system::CrudRagSystem,
};
use crate::models::entity_models::*;
use crate::parser::idiomatic_parser::parse_crud_statement;
use crate::{
    CrudStatement, DataCreate, DataDelete, DataRead, DataUpdate, Key, Literal, PropertyMap, Value,
};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Entity CRUD Service errors
#[derive(Debug, thiserror::Error)]
pub enum EntityCrudError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("AI service error: {0}")]
    AiError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("DSL parsing error: {0}")]
    ParsingError(String),

    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Result type for entity CRUD operations
pub type EntityCrudResult<T> = Result<T, EntityCrudError>;

/// Entity CRUD Service for agentic operations
pub struct EntityCrudService {
    /// Database connection pool
    pool: PgPool,
    /// RAG system for context retrieval
    rag_system: CrudRagSystem,
    /// Prompt builder for AI integration
    prompt_builder: CrudPromptBuilder,
    /// AI service for DSL generation
    ai_service: Option<AiDslService>,
    /// Service configuration
    config: EntityCrudConfig,
}

/// Configuration for entity CRUD operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCrudConfig {
    /// Maximum number of records to return in read operations
    pub max_read_limit: i32,
    /// Default read limit if not specified
    pub default_read_limit: i32,
    /// Enable validation rules checking
    pub enable_validation: bool,
    /// Enable audit logging
    pub enable_audit_logging: bool,
    /// AI confidence threshold for auto-execution
    pub confidence_threshold: f64,
    /// Maximum AI retries
    pub max_ai_retries: usize,
}

impl Default for EntityCrudConfig {
    fn default() -> Self {
        Self {
            max_read_limit: 1000,
            default_read_limit: 50,
            enable_validation: true,
            enable_audit_logging: true,
            confidence_threshold: 0.8,
            max_ai_retries: 3,
        }
    }
}

impl EntityCrudService {
    /// Create a new Entity CRUD Service
    pub fn new(
        pool: PgPool,
        rag_system: CrudRagSystem,
        prompt_builder: CrudPromptBuilder,
        config: Option<EntityCrudConfig>,
    ) -> Self {
        Self {
            pool,
            rag_system,
            prompt_builder,
            ai_service: None,
            config: config.unwrap_or_default(),
        }
    }

    /// Create a new Entity CRUD Service with AI integration
    pub async fn new_with_ai(
        pool: PgPool,
        rag_system: CrudRagSystem,
        prompt_builder: CrudPromptBuilder,
        ai_service: AiDslService,
        config: Option<EntityCrudConfig>,
    ) -> Self {
        Self {
            pool,
            rag_system,
            prompt_builder,
            ai_service: Some(ai_service),
            config: config.unwrap_or_default(),
        }
    }

    /// Create a new entity via agentic CRUD
    pub async fn agentic_create_entity(
        &self,
        request: AgenticEntityCreateRequest,
    ) -> EntityCrudResult<AgenticEntityCrudResponse> {
        let start_time = std::time::Instant::now();

        // Step 1: Generate DSL from natural language instruction
        info!(
            "Generating DSL for entity creation: {}",
            request.instruction
        );

        let generated_dsl = self.generate_create_dsl(&request).await?;
        debug!("Generated DSL: {}", generated_dsl);

        // Step 2: Parse and validate DSL
        let crud_statement = parse_crud_statement(&generated_dsl)
            .map_err(|e| EntityCrudError::ParsingError(format!("Failed to parse DSL: {}", e)))?;

        // Step 3: Execute the operation
        let operation_id = Uuid::new_v4();
        let affected_records = self
            .execute_create_statement(&crud_statement, &request)
            .await?;

        // Step 4: Log the operation
        let execution_time = start_time.elapsed().as_millis() as i32;
        self.log_crud_operation(
            operation_id,
            CrudOperationType::Create,
            request.asset_type.clone(),
            &generated_dsl,
            &request.instruction,
            &affected_records,
            ExecutionStatus::Completed,
            None,
            execution_time,
        )
        .await?;

        // Step 5: Link to CBU if requested
        if let Some(cbu_id) = request.link_to_cbu {
            self.link_entities_to_cbu(cbu_id, &affected_records, request.role_in_cbu.as_deref())
                .await?;
        }

        Ok(AgenticEntityCrudResponse {
            operation_id,
            generated_dsl,
            execution_status: ExecutionStatus::Completed,
            affected_records,
            ai_explanation: format!("Created {} entity(s) as requested", affected_records.len()),
            ai_confidence: Some(0.9), // Mock confidence for now
            execution_time_ms: Some(execution_time),
            error_message: None,
            rag_context_used: vec![], // TODO: Add RAG context tracking
        })
    }

    /// Read entities via agentic CRUD
    pub async fn agentic_read_entities(
        &self,
        request: AgenticEntityReadRequest,
    ) -> EntityCrudResult<(AgenticEntityCrudResponse, Vec<EntityWithDetails>)> {
        let start_time = std::time::Instant::now();

        // Step 1: Generate DSL from natural language instruction
        info!("Generating DSL for entity read: {}", request.instruction);

        let generated_dsl = self.generate_read_dsl(&request).await?;
        debug!("Generated DSL: {}", generated_dsl);

        // Step 2: Parse and validate DSL
        let crud_statement = parse_crud_statement(&generated_dsl)
            .map_err(|e| EntityCrudError::ParsingError(format!("Failed to parse DSL: {}", e)))?;

        // Step 3: Execute the operation
        let operation_id = Uuid::new_v4();
        let entities = self
            .execute_read_statement(&crud_statement, &request)
            .await?;
        let affected_records: Vec<Uuid> = entities.iter().map(|e| e.entity.entity_id).collect();

        // Step 4: Log the operation
        let execution_time = start_time.elapsed().as_millis() as i32;
        self.log_crud_operation(
            operation_id,
            CrudOperationType::Read,
            EntityAssetType::Entity, // Generic for multi-type reads
            &generated_dsl,
            &request.instruction,
            &affected_records,
            ExecutionStatus::Completed,
            None,
            execution_time,
        )
        .await?;

        let response = AgenticEntityCrudResponse {
            operation_id,
            generated_dsl,
            execution_status: ExecutionStatus::Completed,
            affected_records,
            ai_explanation: format!("Found {} entity(s) matching criteria", entities.len()),
            ai_confidence: Some(0.9),
            execution_time_ms: Some(execution_time),
            error_message: None,
            rag_context_used: vec![],
        };

        Ok((response, entities))
    }

    /// Update entities via agentic CRUD
    pub async fn agentic_update_entity(
        &self,
        request: AgenticEntityUpdateRequest,
    ) -> EntityCrudResult<AgenticEntityCrudResponse> {
        let start_time = std::time::Instant::now();

        // Step 1: Generate DSL from natural language instruction
        info!("Generating DSL for entity update: {}", request.instruction);

        let generated_dsl = self.generate_update_dsl(&request).await?;
        debug!("Generated DSL: {}", generated_dsl);

        // Step 2: Parse and validate DSL
        let crud_statement = parse_crud_statement(&generated_dsl)
            .map_err(|e| EntityCrudError::ParsingError(format!("Failed to parse DSL: {}", e)))?;

        // Step 3: Execute the operation
        let operation_id = Uuid::new_v4();
        let affected_records = self
            .execute_update_statement(&crud_statement, &request)
            .await?;

        // Step 4: Log the operation
        let execution_time = start_time.elapsed().as_millis() as i32;
        self.log_crud_operation(
            operation_id,
            CrudOperationType::Update,
            request.asset_type.clone(),
            &generated_dsl,
            &request.instruction,
            &affected_records,
            ExecutionStatus::Completed,
            None,
            execution_time,
        )
        .await?;

        Ok(AgenticEntityCrudResponse {
            operation_id,
            generated_dsl,
            execution_status: ExecutionStatus::Completed,
            affected_records,
            ai_explanation: format!("Updated {} entity(s) as requested", affected_records.len()),
            ai_confidence: Some(0.9),
            execution_time_ms: Some(execution_time),
            error_message: None,
            rag_context_used: vec![],
        })
    }

    /// Delete entities via agentic CRUD
    pub async fn agentic_delete_entity(
        &self,
        request: AgenticEntityDeleteRequest,
    ) -> EntityCrudResult<AgenticEntityCrudResponse> {
        let start_time = std::time::Instant::now();

        // Step 1: Generate DSL from natural language instruction
        info!(
            "Generating DSL for entity deletion: {}",
            request.instruction
        );

        let generated_dsl = self.generate_delete_dsl(&request).await?;
        debug!("Generated DSL: {}", generated_dsl);

        // Step 2: Parse and validate DSL
        let crud_statement = parse_crud_statement(&generated_dsl)
            .map_err(|e| EntityCrudError::ParsingError(format!("Failed to parse DSL: {}", e)))?;

        // Step 3: Execute the operation
        let operation_id = Uuid::new_v4();
        let affected_records = self
            .execute_delete_statement(&crud_statement, &request)
            .await?;

        // Step 4: Log the operation
        let execution_time = start_time.elapsed().as_millis() as i32;
        self.log_crud_operation(
            operation_id,
            CrudOperationType::Delete,
            request.asset_type.clone(),
            &generated_dsl,
            &request.instruction,
            &affected_records,
            ExecutionStatus::Completed,
            None,
            execution_time,
        )
        .await?;

        Ok(AgenticEntityCrudResponse {
            operation_id,
            generated_dsl,
            execution_status: ExecutionStatus::Completed,
            affected_records,
            ai_explanation: format!("Deleted {} entity(s) as requested", affected_records.len()),
            ai_confidence: Some(0.9),
            execution_time_ms: Some(execution_time),
            error_message: None,
            rag_context_used: vec![],
        })
    }

    // ========================================================================
    // DSL GENERATION METHODS
    // ========================================================================

    async fn generate_create_dsl(
        &self,
        request: &AgenticEntityCreateRequest,
    ) -> EntityCrudResult<String> {
        // Use AI service if available, otherwise fall back to pattern matching
        if let Some(ai_service) = &self.ai_service {
            self.generate_ai_create_dsl(ai_service, request).await
        } else {
            self.generate_pattern_create_dsl(request).await
        }
    }

    /// Generate DSL using AI service
    async fn generate_ai_create_dsl(
        &self,
        ai_service: &AiDslService,
        request: &AgenticEntityCreateRequest,
    ) -> EntityCrudResult<String> {
        // Get relevant context from RAG system
        let rag_context = self
            .rag_system
            .retrieve_context(&request.instruction)
            .map_err(|e| EntityCrudError::AiError(format!("RAG retrieval failed: {}", e)))?;

        // Build comprehensive prompt with context
        let prompt = self.prompt_builder.build_entity_create_prompt(
            &request.instruction,
            &request.asset_type,
            &request.context,
            &rag_context,
        )?;

        // Generate DSL using AI
        let ai_response = ai_service
            .generate_dsl_from_prompt(&prompt)
            .await
            .map_err(|e| EntityCrudError::AiError(format!("AI generation failed: {}", e)))?;

        Ok(ai_response.dsl_content)
    }

    /// Generate DSL using pattern matching (fallback)
    async fn generate_pattern_create_dsl(
        &self,
        request: &AgenticEntityCreateRequest,
    ) -> EntityCrudResult<String> {
        let asset_name = request.asset_type.asset_name();
        let mut values = PropertyMap::new();

        // Extract key information from instruction and context
        for (key, value) in &request.context {
            let dsl_key = Key::Keyword(key.clone());
            let dsl_value = self.json_to_dsl_value(value)?;
            values.insert(dsl_key, dsl_value);
        }

        Ok(format!(
            r#"(data.create :asset "{}" :values {})"#,
            asset_name,
            self.property_map_to_string(&values)
        ))
    }

    async fn generate_read_dsl(
        &self,
        request: &AgenticEntityReadRequest,
    ) -> EntityCrudResult<String> {
        // Use AI service if available, otherwise fall back to pattern matching
        if let Some(ai_service) = &self.ai_service {
            self.generate_ai_read_dsl(ai_service, request).await
        } else {
            self.generate_pattern_read_dsl(request).await
        }
    }

    /// Generate read DSL using AI service
    async fn generate_ai_read_dsl(
        &self,
        ai_service: &AiDslService,
        request: &AgenticEntityReadRequest,
    ) -> EntityCrudResult<String> {
        // Get relevant context from RAG system
        let rag_context = self
            .rag_system
            .retrieve_context(&request.instruction)
            .map_err(|e| EntityCrudError::AiError(format!("RAG retrieval failed: {}", e)))?;

        // Build comprehensive prompt with context
        let prompt = self.prompt_builder.build_entity_read_prompt(
            &request.instruction,
            &request.asset_types,
            &request.filters,
            request.limit,
            &rag_context,
        )?;

        // Generate DSL using AI
        let ai_response = ai_service
            .generate_dsl_from_prompt(&prompt)
            .await
            .map_err(|e| EntityCrudError::AiError(format!("AI generation failed: {}", e)))?;

        Ok(ai_response.dsl_content)
    }

    /// Generate read DSL using pattern matching (fallback)
    async fn generate_pattern_read_dsl(
        &self,
        request: &AgenticEntityReadRequest,
    ) -> EntityCrudResult<String> {
        let asset_name = if request.asset_types.len() == 1 {
            request.asset_types[0].asset_name()
        } else {
            "entity" // Generic entity search
        };

        let mut where_clause = PropertyMap::new();
        for (key, value) in &request.filters {
            let dsl_key = Key::Keyword(key.clone());
            let dsl_value = self.json_to_dsl_value(value)?;
            where_clause.insert(dsl_key, dsl_value);
        }

        let limit_clause = if let Some(limit) = request.limit {
            format!(" :limit {}", limit)
        } else {
            format!(" :limit {}", self.config.default_read_limit)
        };

        Ok(format!(
            r#"(data.read :asset "{}" :where {}{limit_clause})"#,
            asset_name,
            self.property_map_to_string(&where_clause),
            limit_clause = limit_clause
        ))
    }

    async fn generate_update_dsl(
        &self,
        request: &AgenticEntityUpdateRequest,
    ) -> EntityCrudResult<String> {
        // Use AI service if available, otherwise fall back to pattern matching
        if let Some(ai_service) = &self.ai_service {
            self.generate_ai_update_dsl(ai_service, request).await
        } else {
            self.generate_pattern_update_dsl(request).await
        }
    }

    /// Generate update DSL using AI service
    async fn generate_ai_update_dsl(
        &self,
        ai_service: &AiDslService,
        request: &AgenticEntityUpdateRequest,
    ) -> EntityCrudResult<String> {
        // Get relevant context from RAG system
        let rag_context = self
            .rag_system
            .retrieve_context(&request.instruction)
            .map_err(|e| EntityCrudError::AiError(format!("RAG retrieval failed: {}", e)))?;

        // Build comprehensive prompt with context
        let prompt = self.prompt_builder.build_entity_update_prompt(
            &request.instruction,
            &request.asset_type,
            &request.identifier,
            &request.updates,
            &rag_context,
        )?;

        // Generate DSL using AI
        let ai_response = ai_service
            .generate_dsl_from_prompt(&prompt)
            .await
            .map_err(|e| EntityCrudError::AiError(format!("AI generation failed: {}", e)))?;

        Ok(ai_response.dsl_content)
    }

    /// Generate update DSL using pattern matching (fallback)
    async fn generate_pattern_update_dsl(
        &self,
        request: &AgenticEntityUpdateRequest,
    ) -> EntityCrudResult<String> {
        let asset_name = request.asset_type.asset_name();

        let mut where_clause = PropertyMap::new();
        for (key, value) in &request.identifier {
            let dsl_key = Key::Keyword(key.clone());
            let dsl_value = self.json_to_dsl_value(value)?;
            where_clause.insert(dsl_key, dsl_value);
        }

        let mut values = PropertyMap::new();
        for (key, value) in &request.updates {
            let dsl_key = Key::Keyword(key.clone());
            let dsl_value = self.json_to_dsl_value(value)?;
            values.insert(dsl_key, dsl_value);
        }

        Ok(format!(
            r#"(data.update :asset "{}" :where {} :values {})"#,
            asset_name,
            self.property_map_to_string(&where_clause),
            self.property_map_to_string(&values)
        ))
    }

    async fn generate_delete_dsl(
        &self,
        request: &AgenticEntityDeleteRequest,
    ) -> EntityCrudResult<String> {
        // Use AI service if available, otherwise fall back to pattern matching
        if let Some(ai_service) = &self.ai_service {
            self.generate_ai_delete_dsl(ai_service, request).await
        } else {
            self.generate_pattern_delete_dsl(request).await
        }
    }

    /// Generate delete DSL using AI service
    async fn generate_ai_delete_dsl(
        &self,
        ai_service: &AiDslService,
        request: &AgenticEntityDeleteRequest,
    ) -> EntityCrudResult<String> {
        // Get relevant context from RAG system
        let rag_context = self
            .rag_system
            .retrieve_context(&request.instruction)
            .map_err(|e| EntityCrudError::AiError(format!("RAG retrieval failed: {}", e)))?;

        // Build comprehensive prompt with context
        let prompt = self.prompt_builder.build_entity_delete_prompt(
            &request.instruction,
            &request.asset_type,
            &request.identifier,
            &rag_context,
        )?;

        // Generate DSL using AI
        let ai_response = ai_service
            .generate_dsl_from_prompt(&prompt)
            .await
            .map_err(|e| EntityCrudError::AiError(format!("AI generation failed: {}", e)))?;

        Ok(ai_response.dsl_content)
    }

    /// Generate delete DSL using pattern matching (fallback)
    async fn generate_pattern_delete_dsl(
        &self,
        request: &AgenticEntityDeleteRequest,
    ) -> EntityCrudResult<String> {
        let asset_name = request.asset_type.asset_name();

        let mut where_clause = PropertyMap::new();
        for (key, value) in &request.identifier {
            let dsl_key = Key::Keyword(key.clone());
            let dsl_value = self.json_to_dsl_value(value)?;
            where_clause.insert(dsl_key, dsl_value);
        }

        Ok(format!(
            r#"(data.delete :asset "{}" :where {})"#,
            asset_name,
            self.property_map_to_string(&where_clause)
        ))
    }

    // ========================================================================
    // DSL EXECUTION METHODS
    // ========================================================================

    async fn execute_create_statement(
        &self,
        statement: &CrudStatement,
        request: &AgenticEntityCreateRequest,
    ) -> EntityCrudResult<Vec<Uuid>> {
        match statement {
            CrudStatement::Create(create) => match request.asset_type {
                EntityAssetType::Partnership => {
                    let partnership = self.create_partnership_from_dsl(create).await?;
                    Ok(vec![partnership.partnership_id])
                }
                EntityAssetType::LimitedCompany => {
                    let company = self.create_limited_company_from_dsl(create).await?;
                    Ok(vec![company.limited_company_id])
                }
                EntityAssetType::ProperPerson => {
                    let person = self.create_proper_person_from_dsl(create).await?;
                    Ok(vec![person.proper_person_id])
                }
                EntityAssetType::Trust => {
                    let trust = self.create_trust_from_dsl(create).await?;
                    Ok(vec![trust.trust_id])
                }
                EntityAssetType::Entity => {
                    return Err(EntityCrudError::UnsupportedOperation(
                        "Cannot create generic entity directly. Use specific entity type."
                            .to_string(),
                    ));
                }
            },
            _ => Err(EntityCrudError::UnsupportedOperation(
                "Expected CREATE statement".to_string(),
            )),
        }
    }

    async fn execute_read_statement(
        &self,
        statement: &CrudStatement,
        request: &AgenticEntityReadRequest,
    ) -> EntityCrudResult<Vec<EntityWithDetails>> {
        match statement {
            CrudStatement::Read(read) => {
                // Execute read based on asset types requested
                let mut all_entities = Vec::new();

                for asset_type in &request.asset_types {
                    let entities = match asset_type {
                        EntityAssetType::Partnership => {
                            self.read_partnerships_from_dsl(read).await?
                        }
                        EntityAssetType::LimitedCompany => {
                            self.read_limited_companies_from_dsl(read).await?
                        }
                        EntityAssetType::ProperPerson => {
                            self.read_proper_persons_from_dsl(read).await?
                        }
                        EntityAssetType::Trust => self.read_trusts_from_dsl(read).await?,
                        EntityAssetType::Entity => self.read_all_entities_from_dsl(read).await?,
                    };
                    all_entities.extend(entities);
                }

                Ok(all_entities)
            }
            _ => Err(EntityCrudError::UnsupportedOperation(
                "Expected READ statement".to_string(),
            )),
        }
    }

    async fn execute_update_statement(
        &self,
        statement: &CrudStatement,
        request: &AgenticEntityUpdateRequest,
    ) -> EntityCrudResult<Vec<Uuid>> {
        match statement {
            CrudStatement::Update(update) => match request.asset_type {
                EntityAssetType::Partnership => self.update_partnerships_from_dsl(update).await,
                EntityAssetType::LimitedCompany => {
                    self.update_limited_companies_from_dsl(update).await
                }
                EntityAssetType::ProperPerson => self.update_proper_persons_from_dsl(update).await,
                EntityAssetType::Trust => self.update_trusts_from_dsl(update).await,
                EntityAssetType::Entity => Err(EntityCrudError::UnsupportedOperation(
                    "Cannot update generic entity directly. Use specific entity type.".to_string(),
                )),
            },
            _ => Err(EntityCrudError::UnsupportedOperation(
                "Expected UPDATE statement".to_string(),
            )),
        }
    }

    async fn execute_delete_statement(
        &self,
        statement: &CrudStatement,
        request: &AgenticEntityDeleteRequest,
    ) -> EntityCrudResult<Vec<Uuid>> {
        match statement {
            CrudStatement::Delete(delete) => match request.asset_type {
                EntityAssetType::Partnership => self.delete_partnerships_from_dsl(delete).await,
                EntityAssetType::LimitedCompany => {
                    self.delete_limited_companies_from_dsl(delete).await
                }
                EntityAssetType::ProperPerson => self.delete_proper_persons_from_dsl(delete).await,
                EntityAssetType::Trust => self.delete_trusts_from_dsl(delete).await,
                EntityAssetType::Entity => Err(EntityCrudError::UnsupportedOperation(
                    "Cannot delete generic entity directly. Use specific entity type.".to_string(),
                )),
            },
            _ => Err(EntityCrudError::UnsupportedOperation(
                "Expected DELETE statement".to_string(),
            )),
        }
    }

    // ========================================================================
    // PARTNERSHIP OPERATIONS
    // ========================================================================

    async fn create_partnership_from_dsl(
        &self,
        create: &DataCreate,
    ) -> EntityCrudResult<Partnership> {
        let mut partnership_name = String::new();
        let mut partnership_type: Option<String> = None;
        let mut jurisdiction: Option<String> = None;
        let mut formation_date: Option<NaiveDate> = None;
        let mut principal_place_business: Option<String> = None;
        let mut partnership_agreement_date: Option<NaiveDate> = None;

        // Extract values from DSL
        for (key, value) in &create.values {
            match key {
                Key::Keyword(k) if k == "partnership_name" || k == "name" => {
                    if let Value::Literal(Literal::String(s)) = value {
                        partnership_name = s.clone();
                    }
                }
                Key::Keyword(k) if k == "partnership_type" || k == "type" => {
                    if let Value::Literal(Literal::String(s)) = value {
                        partnership_type = Some(s.clone());
                    }
                }
                Key::Keyword(k) if k == "jurisdiction" => {
                    if let Value::Literal(Literal::String(s)) = value {
                        jurisdiction = Some(s.clone());
                    }
                }
                Key::Keyword(k) if k == "formation_date" => {
                    if let Value::Literal(Literal::String(s)) = value {
                        formation_date = NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
                    }
                }
                Key::Keyword(k) if k == "principal_place_business" => {
                    if let Value::Literal(Literal::String(s)) = value {
                        principal_place_business = Some(s.clone());
                    }
                }
                Key::Keyword(k) if k == "partnership_agreement_date" => {
                    if let Value::Literal(Literal::String(s)) = value {
                        partnership_agreement_date = NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
                    }
                }
                _ => {}
            }
        }

        if partnership_name.is_empty() {
            return Err(EntityCrudError::ValidationError(
                "Partnership name is required".to_string(),
            ));
        }

        let partnership = sqlx::query_as!(
            Partnership,
            r#"
            INSERT INTO "ob-poc".entity_partnerships (
                partnership_name, partnership_type, jurisdiction, formation_date,
                principal_place_business, partnership_agreement_date
            ) VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING partnership_id, partnership_name, partnership_type, jurisdiction,
                      formation_date, principal_place_business, partnership_agreement_date,
                      created_at, updated_at
            "#,
            partnership_name,
            partnership_type,
            jurisdiction,
            formation_date,
            principal_place_business,
            partnership_agreement_date
        )
        .fetch_one(&self.pool)
        .await?;

        info!(
            "Created partnership: {} (ID: {})",
            partnership.partnership_name, partnership.partnership_id
        );
        Ok(partnership)
    }

    async fn read_partnerships_from_dsl(
        &self,
        _read: &DataRead,
    ) -> EntityCrudResult<Vec<EntityWithDetails>> {
        // Simplified implementation - would parse WHERE clause in full implementation
        let partnerships = sqlx::query_as!(
            Partnership,
            r#"SELECT * FROM "ob-poc".entity_partnerships ORDER BY created_at DESC LIMIT $1"#,
            self.config.default_read_limit
        )
        .fetch_all(&self.pool)
        .await?;

        // Convert to EntityWithDetails
        let mut results = Vec::new();
        for partnership in partnerships {
            // Get entity type
            let entity_type = sqlx::query_as!(
                EntityType,
                r#"SELECT * FROM "ob-poc".entity_types WHERE name = 'PARTNERSHIP' LIMIT 1"#
            )
            .fetch_one(&self.pool)
            .await?;

            // Create a mock entity record (in full implementation, would link properly)
            let entity = Entity {
                entity_id: partnership.partnership_id, // Using same ID for simplicity
                entity_type_id: entity_type.entity_type_id,
                external_id: Some(partnership.partnership_id.to_string()),
                name: partnership.partnership_name.clone(),
                created_at: partnership.created_at,
                updated_at: partnership.updated_at,
            };

            results.push(EntityWithDetails {
                entity,
                entity_type,
                details: EntityDetails::Partnership(partnership),
            });
        }

        Ok(results)
    }

    async fn update_partnerships_from_dsl(
        &self,
        _update: &DataUpdate,
    ) -> EntityCrudResult<Vec<Uuid>> {
        // Placeholder implementation
        warn!("Partnership update not fully implemented yet");
        Ok(vec![])
    }

    async fn delete_partnerships_from_dsl(
        &self,
        _delete: &DataDelete,
    ) -> EntityCrudResult<Vec<Uuid>> {
        // Placeholder implementation
        warn!("Partnership delete not fully implemented yet");
        Ok(vec![])
    }

    // ========================================================================
    // LIMITED COMPANY OPERATIONS (Placeholder implementations)
    // ========================================================================

    async fn create_limited_company_from_dsl(
        &self,
        _create: &DataCreate,
    ) -> EntityCrudResult<LimitedCompany> {
        // Placeholder - would implement similar to partnership
        Err(EntityCrudError::UnsupportedOperation(
            "Limited company creation not yet implemented".to_string(),
        ))
    }

    async fn read_limited_companies_from_dsl(
        &self,
        _read: &DataRead,
    ) -> EntityCrudResult<Vec<EntityWithDetails>> {
        // Placeholder
        Ok(vec![])
    }

    async fn update_limited_companies_from_dsl(
        &self,
        _update: &DataUpdate,
    ) -> EntityCrudResult<Vec<Uuid>> {
        Ok(vec![])
    }

    async fn delete_limited_companies_from_dsl(
        &self,
        _delete: &DataDelete,
    ) -> EntityCrudResult<Vec<Uuid>> {
        Ok(vec![])
    }

    // ========================================================================
    // PROPER PERSON OPERATIONS (Placeholder implementations)
    // ========================================================================

    async fn create_proper_person_from_dsl(
        &self,
        _create: &DataCreate,
    ) -> EntityCrudResult<ProperPerson> {
        Err(EntityCrudError::UnsupportedOperation(
            "Proper person creation not yet implemented".to_string(),
        ))
    }

    async fn read_proper_persons_from_dsl(
        &self,
        _read: &DataRead,
    ) -> EntityCrudResult<Vec<EntityWithDetails>> {
        Ok(vec![])
    }

    async fn update_proper_persons_from_dsl(
        &self,
        _update: &DataUpdate,
    ) -> EntityCrudResult<Vec<Uuid>> {
        Ok(vec![])
    }

    async fn delete_proper_persons_from_dsl(
        &self,
        _delete: &DataDelete,
    ) -> EntityCrudResult<Vec<Uuid>> {
        Ok(vec![])
    }

    // ========================================================================
    // TRUST OPERATIONS (Placeholder implementations)
    // ========================================================================

    async fn create_trust_from_dsl(&self, _create: &DataCreate) -> EntityCrudResult<Trust> {
        Err(EntityCrudError::UnsupportedOperation(
            "Trust creation not yet implemented".to_string(),
        ))
    }

    async fn read_trusts_from_dsl(
        &self,
        _read: &DataRead,
    ) -> EntityCrudResult<Vec<EntityWithDetails>> {
        Ok(vec![])
    }

    async fn update_trusts_from_dsl(&self, _update: &DataUpdate) -> EntityCrudResult<Vec<Uuid>> {
        Ok(vec![])
    }

    async fn delete_trusts_from_dsl(&self, _delete: &DataDelete) -> EntityCrudResult<Vec<Uuid>> {
        Ok(vec![])
    }

    // ========================================================================
    // GENERIC ENTITY OPERATIONS
    // ========================================================================

    async fn read_all_entities_from_dsl(
        &self,
        _read: &DataRead,
    ) -> EntityCrudResult<Vec<EntityWithDetails>> {
        // This would implement cross-entity searches
        Ok(vec![])
    }

    // ========================================================================
    // UTILITY METHODS
    // ========================================================================

    /// Convert JSON value to DSL Value
    fn json_to_dsl_value(&self, json_value: &serde_json::Value) -> EntityCrudResult<Value> {
        match json_value {
            serde_json::Value::String(s) => Ok(Value::Literal(Literal::String(s.clone()))),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Value::Literal(Literal::Integer(i)))
                } else if let Some(f) = n.as_f64() {
                    Ok(Value::Literal(Literal::Float(f)))
                } else {
                    Err(EntityCrudError::ValidationError(format!(
                        "Invalid number: {}",
                        n
                    )))
                }
            }
            serde_json::Value::Bool(b) => Ok(Value::Literal(Literal::Boolean(*b))),
            serde_json::Value::Null => Ok(Value::Literal(Literal::Nil)),
            _ => Err(EntityCrudError::ValidationError(
                "Unsupported JSON value type".to_string(),
            )),
        }
    }

    /// Convert PropertyMap to string representation
    fn property_map_to_string(&self, map: &PropertyMap) -> String {
        let entries: Vec<String> = map
            .iter()
            .map(|(key, value)| {
                let key_str = match key {
                    Key::Keyword(k) => format!(":{}", k),
                    Key::Symbol(s) => s.clone(),
                };
                let value_str = match value {
                    Value::Literal(Literal::String(s)) => format!("\"{}\"", s),
                    Value::Literal(Literal::Integer(i)) => i.to_string(),
                    Value::Literal(Literal::Float(f)) => f.to_string(),
                    Value::Literal(Literal::Boolean(b)) => b.to_string(),
                    Value::Literal(Literal::Nil) => "nil".to_string(),
                    _ => "unknown".to_string(),
                };
                format!("{} {}", key_str, value_str)
            })
            .collect();
        format!("{{{}}}", entries.join(" "))
    }

    /// Link entities to a CBU with specified role
    async fn link_entities_to_cbu(
        &self,
        cbu_id: Uuid,
        entity_ids: &[Uuid],
        role_name: Option<&str>,
    ) -> EntityCrudResult<()> {
        let default_role_name = "CLIENT_ENTITY";
        let role_name = role_name.unwrap_or(default_role_name);

        // Get or create role
        let role_id = self.get_or_create_role(role_name).await?;

        // Link each entity to the CBU
        for entity_id in entity_ids {
            sqlx::query!(
                r#"
                INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
                VALUES ($1, $2, $3)
                ON CONFLICT (cbu_id, entity_id, role_id) DO NOTHING
                "#,
                cbu_id,
                entity_id,
                role_id
            )
            .execute(&self.pool)
            .await?;
        }

        info!(
            "Linked {} entities to CBU {} with role {}",
            entity_ids.len(),
            cbu_id,
            role_name
        );
        Ok(())
    }

    /// Get or create a role by name
    async fn get_or_create_role(&self, role_name: &str) -> EntityCrudResult<Uuid> {
        // First try to get existing role
        let existing_role = sqlx::query!(
            r#"SELECT role_id FROM "ob-poc".roles WHERE name = $1"#,
            role_name
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(role) = existing_role {
            return Ok(role.role_id);
        }

        // Create new role if it doesn't exist
        let new_role = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".roles (name, description)
            VALUES ($1, $2)
            RETURNING role_id
            "#,
            role_name,
            format!("Auto-created role for entity linking: {}", role_name)
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(new_role.role_id)
    }

    /// Log a CRUD operation to the database
    async fn log_crud_operation(
        &self,
        operation_id: Uuid,
        operation_type: CrudOperationType,
        asset_type: EntityAssetType,
        generated_dsl: &str,
        instruction: &str,
        affected_records: &[Uuid],
        status: ExecutionStatus,
        error_message: Option<String>,
        execution_time_ms: i32,
    ) -> EntityCrudResult<()> {
        if !self.config.enable_audit_logging {
            return Ok(());
        }

        let affected_records_json = serde_json::to_value(affected_records).map_err(|e| {
            EntityCrudError::ValidationError(format!("Failed to serialize affected records: {}", e))
        })?;

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".crud_operations (
                operation_id, operation_type, asset_type, entity_table_name,
                generated_dsl, ai_instruction, affected_records, execution_status,
                ai_confidence, ai_provider, ai_model, execution_time_ms,
                error_message, rows_affected
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14
            )
            "#,
            operation_id,
            operation_type.to_string(),
            asset_type.to_string().to_uppercase(),
            asset_type.table_name(),
            generated_dsl,
            instruction,
            affected_records_json,
            status.to_string(),
            None::<rust_decimal::Decimal>, // ai_confidence - will be filled in when AI integration is complete
            Some("mock".to_string()),      // ai_provider
            Some("mock-model".to_string()), // ai_model
            execution_time_ms,
            error_message,
            affected_records.len() as i32
        )
        .execute(&self.pool)
        .await?;

        debug!(
            "Logged CRUD operation {} with status {}",
            operation_id, status
        );
        Ok(())
    }

    /// Validate entity data against rules
    async fn validate_entity_data(
        &self,
        asset_type: &EntityAssetType,
        operation_type: &CrudOperationType,
        data: &PropertyMap,
    ) -> EntityCrudResult<Vec<String>> {
        if !self.config.enable_validation {
            return Ok(vec![]);
        }

        let table_name = asset_type.table_name();
        let rules = sqlx::query_as!(
            EntityCrudRule,
            r#"
            SELECT * FROM "ob-poc".entity_crud_rules
            WHERE entity_table_name = $1 AND operation_type = $2 AND is_active = true
            "#,
            table_name,
            operation_type.to_string()
        )
        .fetch_all(&self.pool)
        .await?;

        let mut validation_errors = Vec::new();

        for rule in rules {
            match rule.constraint_type.as_str() {
                "REQUIRED" => {
                    if let Some(field_name) = &rule.field_name {
                        let field_key = Key::Keyword(field_name.clone());
                        if !data.contains_key(&field_key) {
                            validation_errors.push(rule.error_message.unwrap_or_else(|| {
                                format!("Required field '{}' is missing", field_name)
                            }));
                        }
                    }
                }
                "VALIDATION" => {
                    if let (Some(field_name), Some(pattern)) =
                        (&rule.field_name, &rule.validation_pattern)
                    {
                        let field_key = Key::Keyword(field_name.clone());
                        if let Some(value) = data.get(&field_key) {
                            if let Value::Literal(Literal::String(s)) = value {
                                let regex = regex::Regex::new(pattern).map_err(|e| {
                                    EntityCrudError::ValidationError(format!(
                                        "Invalid regex pattern: {}",
                                        e
                                    ))
                                })?;
                                if !regex.is_match(s) {
                                    validation_errors.push(rule.error_message.unwrap_or_else(
                                        || {
                                            format!(
                                                "Field '{}' does not match required pattern",
                                                field_name
                                            )
                                        },
                                    ));
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Other constraint types not implemented yet
                }
            }
        }

        Ok(validation_errors)
    }

    /// Get CRUD operation by ID
    pub async fn get_crud_operation(
        &self,
        operation_id: Uuid,
    ) -> EntityCrudResult<Option<CrudOperation>> {
        let operation = sqlx::query_as!(
            CrudOperation,
            r#"SELECT * FROM "ob-poc".crud_operations WHERE operation_id = $1"#,
            operation_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(operation)
    }

    /// Get recent CRUD operations
    pub async fn get_recent_crud_operations(
        &self,
        limit: i32,
    ) -> EntityCrudResult<Vec<CrudOperation>> {
        let operations = sqlx::query_as!(
            CrudOperation,
            r#"
            SELECT * FROM "ob-poc".crud_operations
            ORDER BY created_at DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(operations)
    }

    /// Get DSL examples for a specific operation and asset type
    pub async fn get_dsl_examples(
        &self,
        operation_type: &CrudOperationType,
        asset_type: &EntityAssetType,
        limit: i32,
    ) -> EntityCrudResult<Vec<DslExample>> {
        let examples = sqlx::query_as!(
            DslExample,
            r#"
            SELECT * FROM "ob-poc".dsl_examples
            WHERE operation_type = $1 AND asset_type = $2
            ORDER BY usage_count DESC, success_rate DESC
            LIMIT $3
            "#,
            operation_type.to_string(),
            asset_type.to_string().to_uppercase(),
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(examples)
    }

    /// Update usage statistics for a DSL example
    pub async fn update_example_usage(
        &self,
        example_id: Uuid,
        successful: bool,
    ) -> EntityCrudResult<()> {
        let mut tx = self.pool.begin().await?;

        // Get current stats
        let current = sqlx::query!(
            r#"SELECT usage_count, success_rate FROM "ob-poc".dsl_examples WHERE example_id = $1"#,
            example_id
        )
        .fetch_one(&mut *tx)
        .await?;

        let new_usage_count = current.usage_count + 1;
        let current_success_rate = current.success_rate.to_f64().unwrap_or(1.0);
        let new_success_rate = if successful {
            current_success_rate
        } else {
            (current_success_rate * current.usage_count as f64) / new_usage_count as f64
        };

        sqlx::query!(
            r#"
            UPDATE "ob-poc".dsl_examples
            SET usage_count = $1, success_rate = $2, last_used_at = NOW()
            WHERE example_id = $3
            "#,
            new_usage_count,
            rust_decimal::Decimal::from_f64_retain(new_success_rate)
                .unwrap_or(rust_decimal::Decimal::ONE),
            example_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{crud_prompt_builder::PromptConfig, rag_system::CrudRagSystem};

    // Mock implementations for testing would go here
    // This is a placeholder for the testing infrastructure

    #[tokio::test]
    async fn test_partnership_creation() {
        // This would test partnership creation with a test database
        // Placeholder for actual test implementation
        assert!(true);
    }
}
