//! Agentic CRUD Service - AI-Powered CRUD Operations
//!
//! This module provides the main orchestrator service for AI-powered CRUD operations.
//! It combines RAG context retrieval, AI prompt generation, and DSL parsing/execution
//! to enable natural language CRUD operations on the database.

use crate::ai::crud_prompt_builder::{CrudPromptBuilder, GeneratedPrompt, PromptConfig};
use crate::ai::rag_system::{CrudRagSystem, RetrievedContext};
use crate::parser::idiomatic_parser::parse_crud_statement;
use crate::{CrudStatement, Key, Literal, PropertyMap, Value};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Agentic CRUD Service that orchestrates AI-powered CRUD operations
pub struct AgenticCrudService {
    /// RAG system for context retrieval
    rag_system: CrudRagSystem,
    /// Prompt builder for AI interaction
    prompt_builder: CrudPromptBuilder,
    /// Configuration
    config: ServiceConfig,
}

/// Configuration for the Agentic CRUD Service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// AI provider to use
    pub ai_provider: AiProvider,
    /// AI model configuration
    pub model_config: ModelConfig,
    /// Prompt configuration
    pub prompt_config: PromptConfig,
    /// Whether to execute DSL or just generate it
    pub execute_dsl: bool,
    /// Maximum retries for AI generation
    pub max_retries: usize,
    /// Timeout for AI requests (seconds)
    pub timeout_seconds: u64,
}

/// AI provider options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AiProvider {
    Mock { responses: HashMap<String, String> },
}

/// AI model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Temperature for AI generation (0.0-1.0)
    pub temperature: f64,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Top-p value for nucleus sampling
    pub top_p: Option<f64>,
}

/// Request for agentic CRUD operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticCrudRequest {
    /// Natural language instruction
    pub instruction: String,
    /// Optional context hints
    pub context_hints: Option<Vec<String>>,
    /// Whether to execute the generated DSL
    pub execute: bool,
    /// Request ID for tracking
    pub request_id: Option<String>,
}

/// Response from agentic CRUD operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticCrudResponse {
    /// Generated DSL statement
    pub generated_dsl: String,
    /// Parsed CRUD statement (if successful)
    pub parsed_statement: Option<CrudStatement>,
    /// RAG context used
    pub rag_context: RetrievedContext,
    /// AI generation metadata
    pub generation_metadata: GenerationMetadata,
    /// Any errors encountered
    pub errors: Vec<String>,
    /// Overall success status
    pub success: bool,
    /// Request ID for tracking
    pub request_id: String,
}

/// Metadata about AI generation process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationMetadata {
    /// Time taken for RAG retrieval (ms)
    pub rag_time_ms: u64,
    /// Time taken for AI generation (ms)
    pub ai_generation_time_ms: u64,
    /// Time taken for parsing (ms)
    pub parsing_time_ms: u64,
    /// Number of AI retries needed
    pub retries: usize,
    /// AI model used
    pub model_used: String,
    /// Prompt metadata
    pub prompt_metadata: crate::ai::crud_prompt_builder::PromptMetadata,
}

/// Mock AI client for demo purposes
pub struct MockAiClient {
    responses: HashMap<String, String>,
}

impl Default for MockAiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MockAiClient {
    pub fn new() -> Self {
        let mut responses = HashMap::new();

        // Add default responses for common patterns
        responses.insert(
            "create".to_string(),
            r#"(data.create :asset "cbu" :values {:name "New Client"})"#.to_string(),
        );
        responses.insert(
            "read".to_string(),
            r#"(data.read :asset "cbu" :select ["name"])"#.to_string(),
        );
        responses.insert(
            "update".to_string(),
            r#"(data.update :asset "cbu" :where {:name "Test"} :values {:description "Updated"})"#
                .to_string(),
        );
        responses.insert(
            "delete".to_string(),
            r#"(data.delete :asset "cbu" :where {:name "Test"})"#.to_string(),
        );

        Self { responses }
    }

    pub fn generate_dsl(&self, instruction: &str) -> String {
        let instruction_lower = instruction.to_lowercase();

        if instruction_lower.contains("create") || instruction_lower.contains("add") {
            self.responses.get("create").unwrap().clone()
        } else if instruction_lower.contains("update") || instruction_lower.contains("modify") {
            self.responses.get("update").unwrap().clone()
        } else if instruction_lower.contains("delete") || instruction_lower.contains("remove") {
            self.responses.get("delete").unwrap().clone()
        } else {
            self.responses.get("read").unwrap().clone()
        }
    }
}

impl AgenticCrudService {
    /// Creates a new Agentic CRUD Service with mock AI client
    pub fn new(config: ServiceConfig) -> Self {
        Self {
            rag_system: CrudRagSystem::new(),
            prompt_builder: CrudPromptBuilder::new(),
            config,
        }
    }

    /// Creates a service with mock AI client for demonstration
    pub fn with_mock() -> Self {
        let config = ServiceConfig::default();
        Self::new(config)
    }

    /// Processes a natural language CRUD request
    pub fn process_request(&self, request: AgenticCrudRequest) -> Result<AgenticCrudResponse> {
        let start_time = std::time::Instant::now();
        let request_id = request
            .request_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        println!("Processing agentic CRUD request: {}", request.instruction);

        // Step 1: RAG Context Retrieval
        let rag_start = std::time::Instant::now();
        let rag_context = self
            .rag_system
            .retrieve_context(&request.instruction)
            .context("Failed to retrieve RAG context")?;
        let rag_time_ms = rag_start.elapsed().as_millis() as u64;

        println!(
            "RAG context retrieved with confidence: {:.2}",
            rag_context.confidence_score
        );

        // Step 2: Generate AI Prompt
        let prompt = self
            .prompt_builder
            .generate_prompt(
                &rag_context,
                &request.instruction,
                &self.config.prompt_config,
            )
            .context("Failed to generate AI prompt")?;

        println!(
            "Generated prompt with {} tokens",
            prompt.metadata.total_length
        );

        // Step 3: Mock AI Generation
        let ai_start = std::time::Instant::now();
        let mock_client = MockAiClient::new();
        let generated_dsl = mock_client.generate_dsl(&request.instruction);
        let ai_generation_time_ms = ai_start.elapsed().as_millis() as u64;

        println!("AI generated DSL: {}", generated_dsl);

        // Step 4: Parse DSL
        let parsing_start = std::time::Instant::now();
        let parsing_result = parse_crud_statement(&generated_dsl);
        let parsing_time_ms = parsing_start.elapsed().as_millis() as u64;

        let mut errors = Vec::new();
        let mut parsed_statement = None;

        match parsing_result {
            Ok(statement) => {
                println!("DSL parsed successfully: {:?}", statement);
                parsed_statement = Some(statement.clone());
            }
            Err(e) => {
                println!("DSL parsing failed: {}", e);
                errors.push(format!("Parsing failed: {}", e));
            }
        }

        let success = errors.is_empty() && parsed_statement.is_some();

        let response = AgenticCrudResponse {
            generated_dsl,
            parsed_statement,
            rag_context,
            generation_metadata: GenerationMetadata {
                rag_time_ms,
                ai_generation_time_ms,
                parsing_time_ms,
                retries: 0,
                model_used: "mock-ai".to_string(),
                prompt_metadata: prompt.metadata,
            },
            errors,
            success,
            request_id,
        };

        println!(
            "Request processed in {}ms with success: {}",
            start_time.elapsed().as_millis(),
            success
        );

        Ok(response)
    }

    /// Cleans AI response to extract DSL
    fn clean_ai_response(&self, response: &str) -> String {
        // Remove common AI response formatting
        let cleaned = response
            .trim()
            .replace("```lisp", "")
            .replace("```", "")
            .replace("DSL:", "")
            .replace("Result:", "")
            .trim()
            .to_string();

        // Find the first valid DSL statement (starts with parenthesis)
        if let Some(start) = cleaned.find('(') {
            if let Some(end) = cleaned.rfind(')') {
                return cleaned[start..=end].to_string();
            }
        }

        cleaned
    }

    /// Gets available asset types
    pub fn get_available_assets(&self) -> Vec<String> {
        self.rag_system.get_available_assets()
    }

    /// Gets examples for a specific category
    pub fn get_examples_by_category(
        &self,
        category: &str,
    ) -> Vec<crate::ai::rag_system::CrudExample> {
        self.rag_system
            .get_all_examples()
            .iter()
            .filter(|example| {
                example
                    .category
                    .to_lowercase()
                    .contains(&category.to_lowercase())
            })
            .cloned()
            .collect()
    }

    /// Tests the service configuration without making database changes
    pub fn test_configuration(&self, test_instruction: &str) -> Result<AgenticCrudResponse> {
        let test_request = AgenticCrudRequest {
            instruction: test_instruction.to_string(),
            context_hints: None,
            execute: false, // Don't execute in test mode
            request_id: Some("test".to_string()),
        };

        self.process_request(test_request)
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            temperature: 0.1,      // Low temperature for consistent DSL generation
            max_tokens: Some(200), // DSL statements are typically short
            top_p: Some(0.9),
        }
    }
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            ai_provider: AiProvider::Mock {
                responses: HashMap::new(),
            },
            model_config: ModelConfig::default(),
            prompt_config: PromptConfig::default(),
            execute_dsl: false,
            max_retries: 3,
            timeout_seconds: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> AgenticCrudService {
        AgenticCrudService::with_mock()
    }

    #[test]
    fn test_service_creation() {
        let service = create_test_service();
        assert!(!service.get_available_assets().is_empty());
    }

    #[test]
    fn test_request_processing() {
        let service = create_test_service();

        let request = AgenticCrudRequest {
            instruction: "Create a new client called Test Corp".to_string(),
            context_hints: None,
            execute: false,
            request_id: Some("test".to_string()),
        };

        let response = service.process_request(request).unwrap();

        assert!(response.success);
        assert!(!response.generated_dsl.is_empty());
        assert!(response.parsed_statement.is_some());
        assert_eq!(response.request_id, "test");
    }

    #[test]
    fn test_ai_response_cleaning() {
        let service = create_test_service();

        // Test various AI response formats
        let responses = vec![
            "```lisp\n(data.create :asset \"cbu\" :values {:name \"Test\"})\n```",
            "DSL: (data.create :asset \"cbu\" :values {:name \"Test\"})",
            "Here's the DSL:\n(data.create :asset \"cbu\" :values {:name \"Test\"})\n\nThis creates...",
            "(data.create :asset \"cbu\" :values {:name \"Test\"})",
        ];

        for response in responses {
            let cleaned = service.clean_ai_response(response);
            assert_eq!(
                cleaned,
                r#"(data.create :asset "cbu" :values {:name "Test"})"#
            );
        }
    }

    #[test]
    fn test_examples_retrieval() {
        let service = create_test_service();

        let cbu_examples = service.get_examples_by_category("cbu");
        assert!(!cbu_examples.is_empty());

        let creation_examples = service.get_examples_by_category("creation");
        assert!(!creation_examples.is_empty());
    }

    #[test]
    fn test_configuration_test() {
        let service = create_test_service();

        let response = service.test_configuration("Create a test client").unwrap();

        assert!(response.success);
        assert!(!response.generated_dsl.is_empty());
    }
}
