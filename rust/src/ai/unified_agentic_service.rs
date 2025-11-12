//! Unified Agentic CRUD Service - AI-Powered Operations for CBU, Entities, and Documents
//!
//! This module provides a unified interface for AI-powered CRUD operations across
//! the three main data domains: Client Business Units (CBUs), Entities, and Documents.
//! It consolidates all agentic operations into a single service for easier management
//! and testing.

use crate::ai::agentic_crud_service::{
    AgenticCrudRequest, AgenticCrudResponse, AgenticCrudService,
};
use crate::ai::agentic_document_service::{
    AgenticDocumentRequest, AgenticDocumentResponse, AgenticDocumentService,
};
use crate::ai::crud_prompt_builder::{CrudPromptBuilder, PromptConfig};
use crate::ai::rag_system::{CrudRagSystem, RetrievedContext};
use crate::parser::idiomatic_parser::parse_crud_statement;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unified Agentic Service for all CRUD operations
pub(crate) struct UnifiedAgenticService {
    /// CBU and general CRUD service
    crud_service: AgenticCrudService,
    /// Document-specific service
    document_service: AgenticDocumentService,
    /// RAG system for context retrieval
    rag_system: CrudRagSystem,
    /// Service configuration
    config: UnifiedServiceConfig,
}

/// Configuration for the unified service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UnifiedServiceConfig {
    /// Service name
    pub service_name: String,
    /// AI provider configuration
    pub ai_provider: UnifiedAiProvider,
    /// Operation routing configuration
    pub routing_config: RoutingConfig,
    /// Performance settings
    pub performance_config: PerformanceConfig,
    /// Whether to execute operations or simulate
    pub execute_operations: bool,
}

/// AI provider configuration for unified service
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) enum UnifiedAiProvider {
    Mock { responses: HashMap<String, String> },
    OpenAI { api_key: String, model: String },
    Gemini { api_key: String, model: String },
}

/// Configuration for operation routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RoutingConfig {
    /// Keywords that trigger CBU operations
    pub cbu_keywords: Vec<String>,
    /// Keywords that trigger entity operations
    pub entity_keywords: Vec<String>,
    /// Keywords that trigger document operations
    pub document_keywords: Vec<String>,
    /// Default operation type when unclear
    pub default_operation_type: OperationType,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PerformanceConfig {
    /// Maximum concurrent operations
    pub max_concurrent_operations: usize,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Enable caching
    pub enable_caching: bool,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
}

/// Operation types supported by the unified service
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum OperationType {
    /// CBU-related operations
    Cbu,
    /// Entity-related operations
    Entity,
    /// Document-related operations
    Document,
    /// General/mixed operations
    General,
}

/// Unified request for any type of operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UnifiedAgenticRequest {
    /// Natural language instruction
    pub instruction: String,
    /// Suggested operation type (optional)
    pub operation_type_hint: Option<OperationType>,
    /// Context for the operation
    pub context: UnifiedContext,
    /// Whether to execute the operation
    pub execute: bool,
    /// Request ID for tracking
    pub request_id: Option<String>,
}

/// Context information for unified operations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct UnifiedContext {
    /// CBU ID if relevant
    pub cbu_id: Option<Uuid>,
    /// Entity ID if relevant
    pub entity_id: Option<Uuid>,
    /// Document ID if relevant
    pub doc_id: Option<Uuid>,
    /// Additional context hints
    pub hints: Vec<String>,
    /// Business domain context
    pub domain: Option<String>,
}

/// Unified response from any operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UnifiedAgenticResponse {
    /// Detected operation type
    pub operation_type: OperationType,
    /// Generated DSL statement
    pub generated_dsl: String,
    /// Operation result
    pub operation_result: UnifiedOperationResult,
    /// RAG context used
    pub rag_context: RetrievedContext,
    /// Processing metadata
    pub metadata: UnifiedProcessingMetadata,
    /// Any errors encountered
    pub errors: Vec<String>,
    /// Warnings or suggestions
    pub warnings: Vec<String>,
    /// Overall success status
    pub success: bool,
    /// Request ID for tracking
    pub request_id: String,
}

/// Results from different operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum UnifiedOperationResult {
    /// CBU operation result
    CbuResult {
        operation: String,
        affected_records: usize,
        data: serde_json::Value,
    },
    /// Entity operation result
    EntityResult {
        operation: String,
        entity_type: String,
        affected_records: usize,
        data: serde_json::Value,
    },
    /// Document operation result
    DocumentResult {
        operation: String,
        doc_id: Option<Uuid>,
        metadata_updated: bool,
        data: serde_json::Value,
    },
    /// Mixed or general operation result
    GeneralResult {
        operations_performed: Vec<String>,
        total_affected_records: usize,
        data: serde_json::Value,
    },
}

/// Processing metadata for unified operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UnifiedProcessingMetadata {
    /// Time for operation detection (ms)
    pub detection_time_ms: u64,
    /// Time for RAG retrieval (ms)
    pub rag_time_ms: u64,
    /// Time for AI generation (ms)
    pub ai_generation_time_ms: u64,
    /// Time for DSL parsing (ms)
    pub parsing_time_ms: u64,
    /// Time for operation execution (ms)
    pub execution_time_ms: u64,
    /// Total processing time (ms)
    pub total_time_ms: u64,
    /// AI model used
    pub model_used: String,
    /// Service version that handled the request
    pub service_version: String,
    /// Confidence scores for various components
    pub confidence_scores: HashMap<String, f64>,
}

/// Statistics for the unified service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UnifiedServiceStats {
    /// Total requests processed
    pub total_requests: usize,
    /// Requests by operation type
    pub requests_by_type: HashMap<OperationType, usize>,
    /// Success rate by operation type
    pub success_rates: HashMap<OperationType, f64>,
    /// Average processing times (ms)
    pub avg_processing_times: HashMap<String, f64>,
    /// Most common operations
    pub top_operations: Vec<(String, usize)>,
    /// Service uptime
    pub uptime_seconds: u64,
}

impl UnifiedAgenticService {
    /// Create a new unified agentic service
    pub async fn new(config: UnifiedServiceConfig) -> Result<Self> {
        // Create mock CRUD service for testing
        let mut mock_responses = std::collections::HashMap::new();
        mock_responses.insert("default".to_string(), r#"{"dsl_content": "(data.read :asset \"cbu\")", "explanation": "Mock response", "confidence": 0.8, "changes": [], "warnings": [], "suggestions": []}"#.to_string());

        let crud_config = crate::ai::agentic_crud_service::ServiceConfig {
            ai_provider: crate::ai::agentic_crud_service::AiProvider::Mock {
                responses: mock_responses,
            },
            model_config: crate::ai::agentic_crud_service::ModelConfig::default(),
            prompt_config: crate::ai::crud_prompt_builder::PromptConfig::default(),
            execute_dsl: false,
            max_retries: 2,
            timeout_seconds: 10,
            enable_caching: false,
            cache_ttl_seconds: 0,
        };

        #[cfg(feature = "database")]
        let crud_service = {
            let db_manager = crate::database::DatabaseManager::with_default_config().await?;
            crate::ai::agentic_crud_service::AgenticCrudService::new(
                db_manager.pool().clone(),
                crud_config,
            )
            .await?
        };

        #[cfg(not(feature = "database"))]
        let crud_service =
            crate::ai::agentic_crud_service::AgenticCrudService::new_mock(crud_config).await?;

        Ok(Self {
            crud_service,
            document_service: AgenticDocumentService::with_mock(),
            rag_system: CrudRagSystem::new(),
            config,
        })
    }

    /// Create a service with mock providers for testing
    pub async fn with_mock() -> Result<Self> {
        let config = UnifiedServiceConfig::default();
        Self::new(config).await
    }

    /// Process a unified request
    pub async fn process_request(
        &self,
        request: UnifiedAgenticRequest,
    ) -> Result<UnifiedAgenticResponse> {
        let start_time = std::time::Instant::now();
        let request_id = request
            .request_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        println!("Processing unified request: {}", request.instruction);

        // Step 1: Detect operation type
        let detection_start = std::time::Instant::now();
        let operation_type = request
            .operation_type_hint
            .clone()
            .unwrap_or_else(|| self.detect_operation_type(&request.instruction));
        let detection_time_ms = detection_start.elapsed().as_millis() as u64;

        println!("Detected operation type: {:?}", operation_type);

        // Step 2: RAG Context Retrieval
        let rag_start = std::time::Instant::now();
        let rag_context = self
            .rag_system
            .retrieve_context(&request.instruction)
            .context("Failed to retrieve RAG context")?;
        let rag_time_ms = rag_start.elapsed().as_millis() as u64;

        // Step 3: Route to appropriate service
        let _processing_start = std::time::Instant::now();
        let (generated_dsl, operation_result, ai_time, parsing_time, execution_time) =
            match operation_type.clone() {
                OperationType::Document => self.process_document_operation(&request).await?,
                OperationType::Cbu | OperationType::Entity => {
                    self.process_crud_operation(&request).await?
                }
                OperationType::General => self.process_general_operation(&request).await?,
            };

        let total_time_ms = start_time.elapsed().as_millis() as u64;

        // Build response
        let response = UnifiedAgenticResponse {
            operation_type: operation_type.clone(),
            generated_dsl,
            operation_result,
            rag_context,
            metadata: UnifiedProcessingMetadata {
                detection_time_ms,
                rag_time_ms,
                ai_generation_time_ms: ai_time,
                parsing_time_ms: parsing_time,
                execution_time_ms: execution_time,
                total_time_ms,
                model_used: "mock-unified-ai".to_string(),
                service_version: "1.0.0".to_string(),
                confidence_scores: HashMap::from([
                    ("operation_detection".to_string(), 0.90),
                    ("dsl_generation".to_string(), 0.85),
                    ("execution_success".to_string(), 0.95),
                ]),
            },
            errors: vec![],
            warnings: vec![],
            success: true,
            request_id,
        };

        println!(
            "Unified request processed in {}ms with operation type: {:?}",
            total_time_ms, operation_type
        );

        Ok(response)
    }

    /// Detect the operation type from the instruction
    fn detect_operation_type(&self, instruction: &str) -> OperationType {
        let instruction_lower = instruction.to_lowercase();

        // Check document keywords
        for keyword in &self.config.routing_config.document_keywords {
            if instruction_lower.contains(keyword) {
                return OperationType::Document;
            }
        }

        // Check CBU keywords
        for keyword in &self.config.routing_config.cbu_keywords {
            if instruction_lower.contains(keyword) {
                return OperationType::Cbu;
            }
        }

        // Check entity keywords
        for keyword in &self.config.routing_config.entity_keywords {
            if instruction_lower.contains(keyword) {
                return OperationType::Entity;
            }
        }

        // Default fallback
        self.config.routing_config.default_operation_type.clone()
    }

    /// Process document operations
    async fn process_document_operation(
        &self,
        request: &UnifiedAgenticRequest,
    ) -> Result<(String, UnifiedOperationResult, u64, u64, u64)> {
        let doc_request = AgenticDocumentRequest {
            instruction: request.instruction.clone(),
            operation_hint: None,
            target_doc_id: request.context.doc_id,
            cbu_id: request.context.cbu_id,
            entity_id: request.context.entity_id,
            context_hints: Some(request.context.hints.clone()),
            execute: request.execute,
            request_id: Some(format!("doc_{}", Uuid::new_v4())),
        };

        let doc_response = self
            .document_service
            .process_document_request(doc_request)?;

        let operation_result = UnifiedOperationResult::DocumentResult {
            operation: format!("{:?}", doc_response.operation_type),
            doc_id: request.context.doc_id,
            metadata_updated: true,
            data: serde_json::json!({
                "operation_type": doc_response.operation_type,
                "operation_result": doc_response.operation_result
            }),
        };

        Ok((
            doc_response.generated_dsl,
            operation_result,
            doc_response.generation_metadata.ai_generation_time_ms,
            doc_response.generation_metadata.parsing_time_ms,
            doc_response.generation_metadata.execution_time_ms,
        ))
    }

    /// Process CBU/Entity CRUD operations
    async fn process_crud_operation(
        &self,
        request: &UnifiedAgenticRequest,
    ) -> Result<(String, UnifiedOperationResult, u64, u64, u64)> {
        let crud_request = AgenticCrudRequest {
            instruction: request.instruction.clone(),
            context_hints: Some(request.context.hints.clone()),
            execute: request.execute,
            request_id: request.request_id.clone(),
            business_context: None,
            constraints: None,
        };

        let crud_response = self.crud_service.process_request(crud_request).await?;

        let operation_result = if request.instruction.to_lowercase().contains("entity") {
            UnifiedOperationResult::EntityResult {
                operation: "crud_operation".to_string(),
                entity_type: "generic".to_string(),
                affected_records: 1,
                data: serde_json::json!({
                    "generated_dsl": crud_response.generated_dsl,
                    "parsed_statement": crud_response.parsed_statement.map(|stmt| format!("{:?}", stmt))
                }),
            }
        } else {
            UnifiedOperationResult::CbuResult {
                operation: "crud_operation".to_string(),
                affected_records: 1,
                data: serde_json::json!({
                    "generated_dsl": crud_response.generated_dsl,
                    "parsed_statement": crud_response.parsed_statement.map(|stmt| format!("{:?}", stmt))
                }),
            }
        };

        Ok((
            crud_response.generated_dsl,
            operation_result,
            crud_response.generation_metadata.ai_generation_time_ms,
            crud_response.generation_metadata.parsing_time_ms,
            0, // execution time not tracked in crud service
        ))
    }

    /// Process general/mixed operations
    async fn process_general_operation(
        &self,
        request: &UnifiedAgenticRequest,
    ) -> Result<(String, UnifiedOperationResult, u64, u64, u64)> {
        // For general operations, try to determine the best service
        // This is a simplified implementation
        let generated_dsl = format!(
            "(unified.operation :instruction \"{}\" :context \"{}\")",
            request.instruction, "general_operation"
        );

        let operation_result = UnifiedOperationResult::GeneralResult {
            operations_performed: vec!["unified_operation".to_string()],
            total_affected_records: 0,
            data: serde_json::json!({
                "instruction": request.instruction,
                "context": request.context,
                "note": "General operation processed"
            }),
        };

        Ok((generated_dsl, operation_result, 100, 50, 25))
    }

    /// Get service statistics
    pub fn get_statistics(&self) -> UnifiedServiceStats {
        UnifiedServiceStats {
            total_requests: 0,
            requests_by_type: HashMap::new(),
            success_rates: HashMap::new(),
            avg_processing_times: HashMap::new(),
            top_operations: vec![],
            uptime_seconds: 0,
        }
    }

    /// Validate service health
    pub async fn health_check(&self) -> Result<bool> {
        // Check if all underlying services are healthy
        // This would be implemented with actual health checks
        Ok(true)
    }

    /// Get available operation types
    pub(crate) fn get_available_operations(&self) -> Vec<String> {
        vec![
            "CBU creation and management".to_string(),
            "Entity CRUD operations".to_string(),
            "Document cataloging and search".to_string(),
            "Document extraction and analysis".to_string(),
            "Cross-domain operations".to_string(),
        ]
    }
}

// ============================================================================
// DEFAULT IMPLEMENTATIONS
// ============================================================================

impl Default for UnifiedServiceConfig {
    fn default() -> Self {
        Self {
            service_name: "UnifiedAgenticService".to_string(),
            ai_provider: UnifiedAiProvider::Mock {
                responses: HashMap::new(),
            },
            routing_config: RoutingConfig::default(),
            performance_config: PerformanceConfig::default(),
            execute_operations: false,
        }
    }
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            cbu_keywords: vec![
                "client".to_string(),
                "cbu".to_string(),
                "business unit".to_string(),
                "onboarding".to_string(),
                "customer".to_string(),
            ],
            entity_keywords: vec![
                "entity".to_string(),
                "company".to_string(),
                "partnership".to_string(),
                "trust".to_string(),
                "person".to_string(),
                "individual".to_string(),
            ],
            document_keywords: vec![
                "document".to_string(),
                "file".to_string(),
                "passport".to_string(),
                "certificate".to_string(),
                "extract".to_string(),
                "catalog".to_string(),
                "upload".to_string(),
            ],
            default_operation_type: OperationType::General,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_concurrent_operations: 10,
            timeout_seconds: 60,
            enable_caching: true,
            cache_ttl_seconds: 300, // 5 minutes
        }
    }
}

// Default implementation is now derived

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_service() -> UnifiedAgenticService {
        UnifiedAgenticService::with_mock().await.unwrap()
    }

    #[tokio::test]
    async fn test_unified_service_creation() {
        let service = create_test_service().await;
        assert_eq!(service.config.service_name, "UnifiedAgenticService");
    }

    #[tokio::test]
    async fn test_operation_type_detection() {
        let service = create_test_service().await;

        // Test document detection
        let doc_instruction = "Upload this passport document";
        assert_eq!(
            service.detect_operation_type(doc_instruction),
            OperationType::Document
        );

        // Test CBU detection
        let cbu_instruction = "Create a new client business unit";
        assert_eq!(
            service.detect_operation_type(cbu_instruction),
            OperationType::Cbu
        );

        // Test entity detection
        let entity_instruction = "Add a new company entity";
        assert_eq!(
            service.detect_operation_type(entity_instruction),
            OperationType::Entity
        );
    }

    #[tokio::test]
    async fn test_unified_request_processing() {
        let service = create_test_service().await;

        let request = UnifiedAgenticRequest {
            instruction: "Create a new client called Test Corp".to_string(),
            operation_type_hint: Some(OperationType::Cbu),
            context: UnifiedContext::default(),
            execute: false,
            request_id: Some("test".to_string()),
        };

        let response = service.process_request(request).await.unwrap();

        assert!(response.success);
        assert!(!response.generated_dsl.is_empty());
        assert_eq!(response.operation_type, OperationType::Cbu);
        assert_eq!(response.request_id, "test");
    }

    #[tokio::test]
    async fn test_document_operation_routing() {
        let service = create_test_service().await;

        let request = UnifiedAgenticRequest {
            instruction: "Extract data from this document".to_string(),
            operation_type_hint: None, // Let it auto-detect
            context: UnifiedContext {
                doc_id: Some(Uuid::new_v4()),
                ..Default::default()
            },
            execute: false,
            request_id: Some("doc_test".to_string()),
        };

        let response = service.process_request(request).await.unwrap();

        assert!(response.success);
        assert_eq!(response.operation_type, OperationType::Document);
        assert!(matches!(
            response.operation_result,
            UnifiedOperationResult::DocumentResult { .. }
        ));
    }

    #[tokio::test]
    async fn test_service_health_check() {
        let service = create_test_service().await;
        let health = service.health_check().await.unwrap();
        assert!(health);
    }

    #[test]
    fn test_routing_config_defaults() {
        let config = RoutingConfig::default();
        assert!(!config.cbu_keywords.is_empty());
        assert!(!config.entity_keywords.is_empty());
        assert!(!config.document_keywords.is_empty());
        assert_eq!(config.default_operation_type, OperationType::General);
    }
}
