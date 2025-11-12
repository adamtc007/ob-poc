//! Agentic Dictionary Service - AI-Powered Dictionary Management
//!
//! This module provides AI-powered CRUD operations for the dictionary/attributes system.
//! It follows the established agentic pattern and integrates with the existing AI infrastructure.

use crate::ai::crud_prompt_builder::CrudPromptBuilder;
use crate::ai::rag_system::{CrudRagSystem, RetrievedContext};
use crate::ai::{AiConfig, AiDslRequest, AiResponseType, AiService};
#[cfg(feature = "database")]
use crate::database::DictionaryDatabaseService;
use crate::dsl_manager::{DslContext, DslManager, DslManagerFactory, DslProcessingOptions};
#[cfg(feature = "database")]
use crate::models::{
    AgenticAttributeCreateRequest, AgenticAttributeCrudResponse, AgenticAttributeDeleteRequest,
    AgenticAttributeDiscoverRequest, AgenticAttributeReadRequest, AgenticAttributeSearchRequest,
    AgenticAttributeUpdateRequest, AgenticAttributeValidateRequest, AttributeAssetType,
    AttributeDiscoveryRequest, AttributeOperationType, AttributeSearchCriteria,
    AttributeValidationRequest, DictionaryAttribute, DictionaryExecutionStatus,
    NewDictionaryAttribute, UpdateDictionaryAttribute,
};
// Simplified parsing for now - in full implementation would use proper DSL parsing
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Agentic Dictionary Service for AI-powered attribute management - now delegates to DSL Manager
#[cfg(feature = "database")]
pub struct AgenticDictionaryService {
    /// DSL Manager - Central gateway for ALL DSL operations
    dsl_manager: Arc<DslManager>,
    /// Database service for dictionary operations (kept for backwards compatibility)
    db_service: DictionaryDatabaseService,
    /// Service configuration
    config: DictionaryServiceConfig,
    /// Operation cache for performance
    operation_cache: Arc<RwLock<HashMap<String, CachedDictionaryOperation>>>,
}

/// Configuration for the Dictionary Service
#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryServiceConfig {
    /// Whether to execute generated DSL
    pub execute_dsl: bool,
    /// Maximum retries for AI generation
    pub max_retries: usize,
    /// Timeout for operations (seconds)
    pub timeout_seconds: u64,
    /// Enable caching
    pub enable_caching: bool,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// AI model configuration
    pub ai_temperature: f64,
    /// Max tokens for AI responses
    pub max_tokens: Option<u32>,
}

#[cfg(feature = "database")]
impl Default for DictionaryServiceConfig {
    fn default() -> Self {
        Self {
            execute_dsl: true,
            max_retries: 3,
            timeout_seconds: 30,
            enable_caching: true,
            cache_ttl_seconds: 300, // 5 minutes
            ai_temperature: 0.1,    // Low temperature for consistent DSL generation
            max_tokens: Some(1000),
        }
    }
}

/// Cached dictionary operation
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
struct CachedDictionaryOperation {
    response: AgenticAttributeCrudResponse,
    timestamp: chrono::DateTime<chrono::Utc>,
    ttl_seconds: u64,
}

#[cfg(feature = "database")]
impl CachedDictionaryOperation {
    fn is_expired(&self) -> bool {
        let now = chrono::Utc::now();
        let age = now.timestamp() - self.timestamp.timestamp();
        age > self.ttl_seconds as i64
    }
}

/// Dictionary operation metadata
#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryOperationMetadata {
    pub operation_start_time: chrono::DateTime<chrono::Utc>,
    pub ai_generation_time_ms: u64,
    pub parsing_time_ms: u64,
    pub database_time_ms: Option<u64>,
    pub total_time_ms: u64,
    pub retries_needed: usize,
    pub cache_hit: bool,
    pub ai_model_used: String,
    pub ai_confidence: f64,
}

#[cfg(feature = "database")]
impl AgenticDictionaryService {
    /// Create a new agentic dictionary service - now with DSL Manager integration
    pub fn new(
        db_service: DictionaryDatabaseService,
        ai_client: Arc<dyn AiService + Send + Sync>,
        config: Option<DictionaryServiceConfig>,
    ) -> Self {
        let config = config.unwrap_or_default();

        // Create DSL Manager with AI client
        let mut dsl_manager = DslManagerFactory::new();
        dsl_manager.set_ai_service(ai_client.clone());

        Self {
            dsl_manager: Arc::new(dsl_manager),
            db_service,
            config,
            operation_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create attribute via AI-generated DSL - NOW DELEGATES TO DSL MANAGER
    pub async fn create_agentic(
        &self,
        request: AgenticAttributeCreateRequest,
    ) -> Result<AgenticAttributeCrudResponse> {
        let operation_start = chrono::Utc::now();
        let operation_id = Uuid::new_v4();

        info!(
            "Delegating agentic attribute create operation to DSL Manager: {}",
            operation_id
        );

        // Check cache if enabled
        if self.config.enable_caching {
            let cache_key = self.generate_cache_key("create", &request.instruction);
            if let Some(cached) = self.get_cached_operation(&cache_key).await {
                info!("Cache hit for create operation");
                return Ok(cached);
            }
        }

        // Convert to DSL Manager request
        let dsl_request = crate::AgenticCrudRequest {
            instruction: request.instruction.clone(),
            asset_type: Some("dictionary".to_string()),
            operation_type: Some("Create".to_string()),
            execute_dsl: self.config.execute_dsl,
            context_hints: vec![
                format!("asset_type: {:?}", request.asset_type),
                format!("context: {:?}", request.context),
            ],
            metadata: HashMap::new(),
        };

        let context = DslContext {
            request_id: operation_id.to_string(),
            user_id: "dictionary_service".to_string(),
            domain: "dictionary".to_string(),
            options: DslProcessingOptions::default(),
            audit_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert(
                    "operation_type".to_string(),
                    "dictionary_create".to_string(),
                );
                metadata.insert("instruction".to_string(), request.instruction.clone());
                metadata
            },
        };

        // Delegate to DSL Manager
        let dsl_result = self
            .dsl_manager
            .process_agentic_crud_request(dsl_request, context)
            .await
            .map_err(|e| anyhow!("DSL Manager error: {}", e))?;

        // Convert DSL Manager result to Dictionary service response format
        let (execution_status, affected_records, database_time, error_message) =
            if dsl_result.success {
                (
                    DictionaryExecutionStatus::Completed,
                    vec![operation_id], // Simplified - would extract actual IDs from result
                    Some(dsl_result.metrics.execution_time_ms),
                    None,
                )
            } else {
                let error_msg = if !dsl_result.errors.is_empty() {
                    Some(dsl_result.errors[0].to_string())
                } else {
                    Some("DSL processing failed".to_string())
                };
                (DictionaryExecutionStatus::Failed, vec![], None, error_msg)
            };

        let total_time = chrono::Utc::now().timestamp_millis() as u64
            - operation_start.timestamp_millis() as u64;

        let response = AgenticAttributeCrudResponse {
            operation_id,
            generated_dsl: format!("{:?}", dsl_result.ast.unwrap_or_default()),
            execution_status,
            affected_records,
            ai_explanation: "DSL Manager generated response".to_string(),
            ai_confidence: Some(0.8), // Default confidence from DSL Manager
            execution_time_ms: database_time.map(|t| t as i32),
            error_message,
            rag_context_used: vec![], // DSL Manager handles RAG internally
            operation_type: AttributeOperationType::Create,
            results: None,
        };

        // Cache the result if enabled
        if self.config.enable_caching {
            let cache_key = self.generate_cache_key("create", &request.instruction);
            self.cache_operation(&cache_key, &response).await;
        }

        info!(
            "Completed agentic attribute create operation: {} in {}ms",
            operation_id, total_time
        );

        Ok(response)
    }

    /// Read attributes via AI-generated DSL
    pub async fn read_agentic(
        &self,
        request: AgenticAttributeReadRequest,
    ) -> Result<AgenticAttributeCrudResponse> {
        let operation_start = chrono::Utc::now();
        let operation_id = Uuid::new_v4();

        info!(
            "Starting agentic attribute read operation: {}",
            operation_id
        );

        // Generate RAG context for attribute reading
        let rag_context = vec![
            "Dictionary attribute reading operation".to_string(),
            "Standard attribute retrieval patterns".to_string(),
        ];

        // Build AI prompt for attribute reading
        let prompt_text = format!(
            "Read attributes with the following instruction: {}\nContext: {:?}",
            request.instruction, rag_context
        );

        // Generate DSL via AI
        let ai_start = std::time::Instant::now();
        let ai_response = self.generate_dsl_with_retries(&prompt_text).await?;
        let _ai_generation_time = ai_start.elapsed().as_millis() as u64;

        // Parse the generated DSL (simplified)
        let parse_start = std::time::Instant::now();
        let _parsing_time = parse_start.elapsed().as_millis() as u64;

        // Execute if configured to do so
        let (execution_status, affected_records, database_time, error_message, results) =
            if self.config.execute_dsl {
                match self
                    .execute_read_from_instruction(&request.instruction)
                    .await
                {
                    Ok((attributes, exec_time)) => {
                        let ids = attributes.iter().map(|a| a.attribute_id).collect();
                        let results_json = serde_json::to_value(&attributes).unwrap_or_default();
                        (
                            DictionaryExecutionStatus::Completed,
                            ids,
                            Some(exec_time),
                            None,
                            Some(results_json),
                        )
                    }
                    Err(e) => (
                        DictionaryExecutionStatus::Failed,
                        vec![],
                        None,
                        Some(e.to_string()),
                        None,
                    ),
                }
            } else {
                (DictionaryExecutionStatus::Pending, vec![], None, None, None)
            };

        let total_time = chrono::Utc::now().timestamp_millis() as u64
            - operation_start.timestamp_millis() as u64;

        let response = AgenticAttributeCrudResponse {
            operation_id,
            generated_dsl: ai_response.generated_dsl,
            execution_status,
            affected_records,
            ai_explanation: ai_response.explanation,
            ai_confidence: Some(ai_response.confidence),
            execution_time_ms: database_time.map(|t| t as i32),
            error_message,
            rag_context_used: rag_context,
            operation_type: AttributeOperationType::Read,
            results,
        };

        info!(
            "Completed agentic attribute read operation: {} in {}ms",
            operation_id, total_time
        );

        Ok(response)
    }

    /// Update attributes via AI-generated DSL
    pub async fn update_agentic(
        &self,
        request: AgenticAttributeUpdateRequest,
    ) -> Result<AgenticAttributeCrudResponse> {
        let operation_start = chrono::Utc::now();
        let operation_id = Uuid::new_v4();

        info!(
            "Starting agentic attribute update operation: {}",
            operation_id
        );

        // Generate RAG context for attribute updating
        let rag_context = vec![
            "Dictionary attribute updating operation".to_string(),
            "Standard attribute modification patterns".to_string(),
        ];

        // Build AI prompt for attribute updating
        let prompt_text = format!(
            "Update attributes with the following instruction: {}\nContext: {:?}",
            request.instruction, rag_context
        );

        // Generate DSL via AI
        let ai_start = std::time::Instant::now();
        let ai_response = self.generate_dsl_with_retries(&prompt_text).await?;
        let _ai_generation_time = ai_start.elapsed().as_millis() as u64;

        // Parse the generated DSL (simplified)
        let parse_start = std::time::Instant::now();
        let _parsing_time = parse_start.elapsed().as_millis() as u64;

        // Execute if configured to do so
        let (execution_status, affected_records, database_time, error_message) =
            if self.config.execute_dsl {
                // For now, return pending status - full implementation would parse and execute
                (DictionaryExecutionStatus::Pending, vec![], None, None)
            } else {
                (DictionaryExecutionStatus::Pending, vec![], None, None)
            };

        let total_time = chrono::Utc::now().timestamp_millis() as u64
            - operation_start.timestamp_millis() as u64;

        let response = AgenticAttributeCrudResponse {
            operation_id,
            generated_dsl: ai_response.generated_dsl,
            execution_status,
            affected_records,
            ai_explanation: ai_response.explanation,
            ai_confidence: Some(ai_response.confidence),
            execution_time_ms: database_time.map(|t: u64| t as i32),
            error_message,
            rag_context_used: rag_context,
            operation_type: AttributeOperationType::Update,
            results: None,
        };

        info!(
            "Completed agentic attribute update operation: {} in {}ms",
            operation_id, total_time
        );

        Ok(response)
    }

    /// Delete attributes via AI-generated DSL
    pub async fn delete_agentic(
        &self,
        request: AgenticAttributeDeleteRequest,
    ) -> Result<AgenticAttributeCrudResponse> {
        let operation_start = chrono::Utc::now();
        let operation_id = Uuid::new_v4();

        info!(
            "Starting agentic attribute delete operation: {}",
            operation_id
        );

        // Generate RAG context for attribute deletion
        let rag_context = vec![
            "Dictionary attribute deletion operation".to_string(),
            "Standard attribute removal patterns".to_string(),
        ];

        // Build AI prompt for attribute deletion
        let prompt_text = format!(
            "Delete attributes with the following instruction: {}\nContext: {:?}",
            request.instruction, rag_context
        );

        // Generate DSL via AI
        let ai_start = std::time::Instant::now();
        let ai_response = self.generate_dsl_with_retries(&prompt_text).await?;
        let _ai_generation_time = ai_start.elapsed().as_millis() as u64;

        // Parse the generated DSL (simplified)
        let parse_start = std::time::Instant::now();
        let _parsing_time = parse_start.elapsed().as_millis() as u64;

        // Execute if configured to do so
        let (execution_status, affected_records, database_time, error_message) =
            if self.config.execute_dsl {
                // For now, return pending status - full implementation would parse and execute
                (DictionaryExecutionStatus::Pending, vec![], None, None)
            } else {
                (DictionaryExecutionStatus::Pending, vec![], None, None)
            };

        let total_time = chrono::Utc::now().timestamp_millis() as u64
            - operation_start.timestamp_millis() as u64;

        let response = AgenticAttributeCrudResponse {
            operation_id,
            generated_dsl: ai_response.generated_dsl,
            execution_status,
            affected_records,
            ai_explanation: ai_response.explanation,
            ai_confidence: Some(ai_response.confidence),
            execution_time_ms: database_time.map(|t: u64| t as i32),
            error_message,
            rag_context_used: rag_context,
            operation_type: AttributeOperationType::Delete,
            results: None,
        };

        info!(
            "Completed agentic attribute delete operation: {} in {}ms",
            operation_id, total_time
        );

        Ok(response)
    }

    /// Search attributes via AI-generated DSL
    pub async fn search_agentic(
        &self,
        request: AgenticAttributeSearchRequest,
    ) -> Result<AgenticAttributeCrudResponse> {
        let _operation_start = chrono::Utc::now();
        let operation_id = Uuid::new_v4();

        info!(
            "Starting agentic attribute search operation: {}",
            operation_id
        );

        // For search, we can use direct database operations or AI-generated DSL
        let database_start = std::time::Instant::now();
        let search_results = self
            .db_service
            .search_attributes(&request.search_criteria)
            .await?;
        let database_time = database_start.elapsed().as_millis() as u64;

        // Generate a simple DSL representation of the search
        let generated_dsl = self.generate_search_dsl(&request.search_criteria);

        let response = AgenticAttributeCrudResponse {
            operation_id,
            generated_dsl,
            execution_status: DictionaryExecutionStatus::Completed,
            affected_records: search_results.iter().map(|a| a.attribute_id).collect(),
            ai_explanation: format!(
                "Found {} attributes matching the search criteria",
                search_results.len()
            ),
            ai_confidence: Some(1.0), // High confidence for direct database search
            execution_time_ms: Some(database_time as i32),
            error_message: None,
            rag_context_used: vec![],
            operation_type: AttributeOperationType::Search,
            results: Some(serde_json::to_value(search_results)?),
        };

        info!(
            "Completed agentic attribute search operation: {} in {}ms",
            operation_id, database_time
        );

        Ok(response)
    }

    /// Validate attribute via AI-generated DSL
    pub async fn validate_agentic(
        &self,
        request: AgenticAttributeValidateRequest,
    ) -> Result<AgenticAttributeCrudResponse> {
        let _operation_start = chrono::Utc::now();
        let operation_id = Uuid::new_v4();

        info!(
            "Starting agentic attribute validate operation: {}",
            operation_id
        );

        // Perform validation using database service
        let database_start = std::time::Instant::now();
        let validation_result = self
            .db_service
            .validate_attribute_value(&request.validation_request)
            .await?;
        let database_time = database_start.elapsed().as_millis() as u64;

        // Generate a simple DSL representation of the validation
        let generated_dsl = format!(
            "(attribute.validate :attribute-id {} :value {})",
            request.validation_request.attribute_id,
            serde_json::to_string(&request.validation_request.value)?
        );

        let response = AgenticAttributeCrudResponse {
            operation_id,
            generated_dsl,
            execution_status: DictionaryExecutionStatus::Completed,
            affected_records: vec![request.validation_request.attribute_id],
            ai_explanation: format!(
                "Validation result: {}",
                if validation_result.is_valid {
                    "Valid"
                } else {
                    "Invalid"
                }
            ),
            ai_confidence: Some(1.0), // High confidence for direct validation
            execution_time_ms: Some(database_time as i32),
            error_message: None,
            rag_context_used: vec![],
            operation_type: AttributeOperationType::Validate,
            results: Some(serde_json::to_value(validation_result)?),
        };

        info!(
            "Completed agentic attribute validate operation: {} in {}ms",
            operation_id, database_time
        );

        Ok(response)
    }

    /// Discover attributes via AI-generated DSL
    pub async fn discover_agentic(
        &self,
        request: AgenticAttributeDiscoverRequest,
    ) -> Result<AgenticAttributeCrudResponse> {
        let _operation_start = chrono::Utc::now();
        let operation_id = Uuid::new_v4();

        info!(
            "Starting agentic attribute discover operation: {}",
            operation_id
        );

        // Perform semantic search using database service
        let database_start = std::time::Instant::now();
        let discovered_attributes = self
            .db_service
            .semantic_search(
                &request.discovery_request.semantic_query,
                request.discovery_request.limit,
            )
            .await?;
        let database_time = database_start.elapsed().as_millis() as u64;

        // Generate a simple DSL representation of the discovery
        let generated_dsl = format!(
            "(attribute.discover :semantic-query \"{}\")",
            request.discovery_request.semantic_query
        );

        let response = AgenticAttributeCrudResponse {
            operation_id,
            generated_dsl,
            execution_status: DictionaryExecutionStatus::Completed,
            affected_records: discovered_attributes
                .iter()
                .map(|a| a.attribute.attribute_id)
                .collect(),
            ai_explanation: format!(
                "Discovered {} relevant attributes",
                discovered_attributes.len()
            ),
            ai_confidence: Some(0.8), // Good confidence for semantic search
            execution_time_ms: Some(database_time as i32),
            error_message: None,
            rag_context_used: vec![],
            operation_type: AttributeOperationType::Discover,
            results: Some(serde_json::to_value(discovered_attributes)?),
        };

        info!(
            "Completed agentic attribute discover operation: {} in {}ms",
            operation_id, database_time
        );

        Ok(response)
    }

    // Private helper methods

    /// Generate DSL with retry logic
    async fn generate_dsl_with_retries(&self, prompt: &str) -> Result<AiDslResponse> {
        let mut retries = 0;
        let mut last_error = None;

        while retries <= self.config.max_retries {
            match self.generate_dsl_once(prompt).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    warn!("DSL generation attempt {} failed: {}", retries + 1, e);
                    last_error = Some(e);
                    retries += 1;
                    if retries <= self.config.max_retries {
                        // Brief delay before retry
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("DSL generation failed after all retries")))
    }

    /// Generate DSL once via AI
    async fn generate_dsl_once(&self, prompt: &str) -> Result<AiDslResponse> {
        let ai_request = AiDslRequest {
            instruction: prompt.to_string(),
            context: Some(HashMap::new()),
            response_type: AiResponseType::DslGeneration,
            temperature: Some(self.config.ai_temperature),
            max_tokens: self.config.max_tokens,
        };

        // Convert AiDslRequest to AgenticCrudRequest
        let crud_request = crate::AgenticCrudRequest {
            instruction: ai_request.instruction.clone(),
            asset_type: Some("dictionary".to_string()),
            operation_type: Some("Create".to_string()),
            execute_dsl: true,
            context_hints: vec![
                "Dictionary AI DSL generation".to_string(),
                format!("Context: {:?}", ai_request.context),
            ],
            metadata: std::collections::HashMap::new(),
        };

        let context = crate::dsl_manager::DslContext {
            request_id: uuid::Uuid::new_v4().to_string(),
            user_id: "agentic_dictionary_service".to_string(),
            domain: "dictionary".to_string(),
            options: crate::dsl_manager::DslProcessingOptions::default(),
            audit_metadata: std::collections::HashMap::new(),
        };
        let ai_response = self
            .dsl_manager
            .process_agentic_crud_request(crud_request, context)
            .await?;

        let generated_dsl = if let Some(result) = &ai_response.execution_result {
            result
                .data
                .get("generated_dsl")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "(dictionary.create)".to_string())
        } else {
            "(dictionary.create)".to_string()
        };

        Ok(AiDslResponse {
            generated_dsl,
            explanation: "Generated dictionary DSL via agentic CRUD".to_string(),
            confidence: 0.8,
        })
    }

    /// Execute create operation from instruction (simplified)
    async fn execute_create_from_instruction(&self, instruction: &str) -> Result<(Uuid, u64)> {
        let start_time = std::time::Instant::now();

        // For now, create a basic attribute based on instruction parsing
        // In full implementation, this would parse the DSL properly
        let new_attribute = NewDictionaryAttribute {
            name: format!("generated.{}", chrono::Utc::now().timestamp()),
            long_description: Some(instruction.to_string()),
            group_id: Some("generated".to_string()),
            mask: Some("string".to_string()),
            domain: Some("AI".to_string()),
            vector: None,
            source: None,
            sink: None,
        };

        let created_attribute = self.db_service.create_attribute(new_attribute).await?;
        let execution_time = start_time.elapsed().as_millis() as u64;
        Ok((created_attribute.attribute_id, execution_time))
    }

    /// Execute read operation from instruction (simplified)
    async fn execute_read_from_instruction(
        &self,
        instruction: &str,
    ) -> Result<(Vec<DictionaryAttribute>, u64)> {
        let start_time = std::time::Instant::now();

        // Basic search based on instruction
        let criteria = AttributeSearchCriteria {
            name_pattern: Some(instruction.to_lowercase()),
            group_id: None,
            domain: None,
            mask: None,
            semantic_query: None,
            limit: Some(10),
            offset: Some(0),
        };

        let attributes = self.db_service.search_attributes(&criteria).await?;
        let execution_time = start_time.elapsed().as_millis() as u64;
        Ok((attributes, execution_time))
    }

    /// Generate search DSL representation
    fn generate_search_dsl(&self, criteria: &AttributeSearchCriteria) -> String {
        let mut dsl_parts = vec!["(attribute.search".to_string()];

        if let Some(name) = &criteria.name_pattern {
            dsl_parts.push(format!(":name \"{}\"", name));
        }

        if let Some(group_id) = &criteria.group_id {
            dsl_parts.push(format!(":group-id \"{}\"", group_id));
        }

        if let Some(domain) = &criteria.domain {
            dsl_parts.push(format!(":domain \"{}\"", domain));
        }

        if let Some(mask) = &criteria.mask {
            dsl_parts.push(format!(":mask \"{}\"", mask));
        }

        if let Some(limit) = criteria.limit {
            dsl_parts.push(format!(":limit {}", limit));
        }

        dsl_parts.push(")".to_string());
        dsl_parts.join(" ")
    }

    /// Generate cache key for operations
    fn generate_cache_key(&self, operation: &str, instruction: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        operation.hash(&mut hasher);
        instruction.hash(&mut hasher);
        format!("attr_{}_{}", operation, hasher.finish())
    }

    /// Get cached operation if available and not expired
    async fn get_cached_operation(&self, cache_key: &str) -> Option<AgenticAttributeCrudResponse> {
        let cache = self.operation_cache.read().await;
        if let Some(cached_op) = cache.get(cache_key) {
            if !cached_op.is_expired() {
                return Some(cached_op.response.clone());
            }
        }
        None
    }

    /// Cache an operation result
    async fn cache_operation(&self, cache_key: &str, response: &AgenticAttributeCrudResponse) {
        let cached_op = CachedDictionaryOperation {
            response: response.clone(),
            timestamp: chrono::Utc::now(),
            ttl_seconds: self.config.cache_ttl_seconds,
        };

        let mut cache = self.operation_cache.write().await;
        cache.insert(cache_key.to_string(), cached_op);

        // Simple cleanup: remove expired entries if cache gets too large
        if cache.len() > 1000 {
            cache.retain(|_, v| !v.is_expired());
        }
    }

    /// Clear operation cache
    pub async fn clear_cache(&self) {
        let mut cache = self.operation_cache.write().await;
        cache.clear();
        info!("Dictionary operation cache cleared");
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> CacheStats {
        let cache = self.operation_cache.read().await;
        let total_entries = cache.len();
        let expired_entries = cache.values().filter(|v| v.is_expired()).count();

        CacheStats {
            total_entries,
            active_entries: total_entries - expired_entries,
            expired_entries,
        }
    }
}

/// AI DSL Response structure
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
struct AiDslResponse {
    generated_dsl: String,
    explanation: String,
    confidence: f64,
}

/// Cache statistics
#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub active_entries: usize,
    pub expired_entries: usize,
}
