//! AI-based DSL Generator
//!
//! This module implements DSL generation using AI services like OpenAI and Gemini.
//! It integrates with the existing AI services and provides fallback mechanisms.

use super::traits::{
    DslGenerator, GenerationError, GenerationMethod, GenerationOperationType, GenerationRequest,
    GenerationResponse, GenerationResult, GeneratorMetadata,
};
// Note: AI service types defined locally since services module requires database feature
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};

/// AI-based DSL generator
pub struct AiGenerator {
    /// AI service for DSL generation
    ai_service: Arc<dyn AiDslGenerationService + Send + Sync>,

    /// Generator configuration
    config: AiGeneratorConfig,

    /// RAG (Retrieval-Augmented Generation) context provider
    rag_provider: Option<Arc<dyn RagContextProvider + Send + Sync>>,
}

/// Configuration for AI generator
#[derive(Debug, Clone)]
pub struct AiGeneratorConfig {
    /// Default AI model to use
    pub default_model: String,

    /// Maximum tokens for AI generation
    pub max_tokens: u32,

    /// Temperature for AI generation (creativity)
    pub temperature: f32,

    /// Enable RAG context
    pub enable_rag: bool,

    /// Maximum context length for RAG
    pub max_rag_context_length: usize,

    /// Timeout for AI requests
    pub ai_timeout_seconds: u64,

    /// Number of retry attempts
    pub max_retries: u32,

    /// Enable confidence scoring
    pub enable_confidence_scoring: bool,
}

/// AI service trait for DSL generation
#[async_trait]
pub trait AiDslGenerationService: Send + Sync {
    /// Generate DSL using AI
    async fn generate_dsl_ai(
        &self,
        request: &AiGenerationRequest,
    ) -> Result<AiGenerationResponse, AiGenerationError>;

    /// Get available models
    fn available_models(&self) -> Vec<String>;

    /// Check service health
    async fn health_check(&self) -> bool;

    /// Get service metadata
    fn service_metadata(&self) -> AiServiceMetadata;
}

/// RAG context provider trait
#[async_trait]
pub trait RagContextProvider: Send + Sync {
    /// Get relevant context for DSL generation
    async fn get_context(
        &self,
        operation_type: &GenerationOperationType,
        query: &str,
    ) -> Result<RagContext, RagError>;

    /// Update context database
    async fn update_context(&self, context: &RagContext) -> Result<(), RagError>;
}

/// AI generation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiGenerationRequest {
    /// Operation type to generate DSL for
    pub operation_type: GenerationOperationType,

    /// User instruction or prompt
    pub instruction: String,

    /// Context data
    pub context_data: HashMap<String, String>,

    /// RAG context (if available)
    pub rag_context: Option<RagContext>,

    /// AI model to use
    pub model: Option<String>,

    /// Generation parameters
    pub parameters: AiGenerationParameters,
}

/// AI generation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiGenerationParameters {
    /// Maximum tokens to generate
    pub max_tokens: u32,

    /// Temperature (creativity) setting
    pub temperature: f32,

    /// Top-p nucleus sampling
    pub top_p: Option<f32>,

    /// Frequency penalty
    pub frequency_penalty: Option<f32>,

    /// Presence penalty
    pub presence_penalty: Option<f32>,
}

/// AI generation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiGenerationResponse {
    /// Generated DSL content
    pub dsl_content: String,

    /// Model used for generation
    pub model_used: String,

    /// Confidence score (0.0 to 1.0)
    pub confidence_score: f64,

    /// Token usage statistics
    pub token_usage: TokenUsage,

    /// Generation metadata
    pub metadata: HashMap<String, String>,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Tokens in the prompt
    pub prompt_tokens: u32,

    /// Tokens in the completion
    pub completion_tokens: u32,

    /// Total tokens used
    pub total_tokens: u32,
}

/// RAG context for enhanced generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagContext {
    /// Retrieved documents or examples
    pub documents: Vec<RagDocument>,

    /// Contextual metadata
    pub metadata: HashMap<String, String>,

    /// Relevance score
    pub relevance_score: f64,
}

/// RAG document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagDocument {
    /// Document ID
    pub id: String,

    /// Document content
    pub content: String,

    /// Document type
    pub doc_type: String,

    /// Relevance score
    pub score: f64,

    /// Document metadata
    pub metadata: HashMap<String, String>,
}

/// AI service metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiServiceMetadata {
    /// Service name
    pub name: String,

    /// Service version
    pub version: String,

    /// Available models
    pub models: Vec<String>,

    /// Service capabilities
    pub capabilities: HashMap<String, String>,

    /// Service status
    pub status: AiServiceStatus,
}

/// AI service status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AiServiceStatus {
    Active,
    Degraded,
    Inactive,
    Unknown,
}

/// AI generation errors
#[derive(Debug, thiserror::Error)]
pub enum AiGenerationError {
    #[error("AI service error: {message}")]
    ServiceError { message: String },

    #[error("Invalid model: {model}")]
    InvalidModel { model: String },

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Timeout occurred")]
    Timeout,

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Content filtering triggered: {reason}")]
    ContentFiltered { reason: String },

    #[error("Insufficient quota")]
    InsufficientQuota,

    #[error("Network error: {message}")]
    NetworkError { message: String },
}

/// RAG errors
#[derive(Debug, thiserror::Error)]
pub enum RagError {
    #[error("Context retrieval failed: {message}")]
    RetrievalFailed { message: String },

    #[error("Context database error: {message}")]
    DatabaseError { message: String },

    #[error("Embedding generation failed")]
    EmbeddingFailed,
}

impl Default for AiGeneratorConfig {
    fn default() -> Self {
        Self {
            default_model: "gpt-3.5-turbo".to_string(),
            max_tokens: 2048,
            temperature: 0.7,
            enable_rag: false,
            max_rag_context_length: 4000,
            ai_timeout_seconds: 30,
            max_retries: 3,
            enable_confidence_scoring: true,
        }
    }
}

impl Default for AiGenerationParameters {
    fn default() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.7,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
        }
    }
}

impl AiGenerator {
    /// Create a new AI generator with service
    pub fn new(ai_service: Arc<dyn AiDslGenerationService + Send + Sync>) -> Self {
        Self {
            ai_service,
            config: AiGeneratorConfig::default(),
            rag_provider: None,
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        ai_service: Arc<dyn AiDslGenerationService + Send + Sync>,
        config: AiGeneratorConfig,
    ) -> Self {
        Self {
            ai_service,
            config,
            rag_provider: None,
        }
    }

    /// Create with RAG provider
    pub fn with_rag(
        ai_service: Arc<dyn AiDslGenerationService + Send + Sync>,
        config: AiGeneratorConfig,
        rag_provider: Arc<dyn RagContextProvider + Send + Sync>,
    ) -> Self {
        Self {
            ai_service,
            config,
            rag_provider: Some(rag_provider),
        }
    }

    /// Build RAG-enhanced prompt
    async fn build_rag_prompt(&self, request: &GenerationRequest) -> GenerationResult<String> {
        let mut prompt = self.build_base_prompt(request);

        if self.config.enable_rag {
            if let Some(rag_provider) = &self.rag_provider {
                let instruction = request
                    .context
                    .instruction
                    .clone()
                    .unwrap_or_else(|| format!("Generate DSL for {}", request.operation_type));

                match rag_provider
                    .get_context(&request.operation_type, &instruction)
                    .await
                {
                    Ok(rag_context) => {
                        prompt = self.enhance_prompt_with_rag(prompt, &rag_context);
                        info!(
                            "Enhanced prompt with RAG context (relevance: {:.2})",
                            rag_context.relevance_score
                        );
                    }
                    Err(e) => {
                        warn!("Failed to get RAG context: {}", e);
                        // Continue without RAG context
                    }
                }
            }
        }

        Ok(prompt)
    }

    /// Build base prompt for DSL generation
    fn build_base_prompt(&self, request: &GenerationRequest) -> String {
        let operation_type = &request.operation_type;
        let context = &request.context;

        let mut prompt = format!(
            r#"You are an expert DSL (Domain Specific Language) generator for the OB-POC (Ultimate Beneficial Ownership Proof of Concept) system.

Generate a DSL document for the following operation:
Operation Type: {}

"#,
            operation_type
        );

        // Add user instruction if available
        if let Some(instruction) = &context.instruction {
            prompt.push_str(&format!("User Instruction: {}\n\n", instruction));
        }

        // Add context data
        if !context.entity_data.is_empty() {
            prompt.push_str("Context Data:\n");
            for (key, value) in &context.entity_data {
                prompt.push_str(&format!("- {}: {}\n", key, value));
            }
            prompt.push('\n');
        }

        // Add CBU and case information
        if let Some(cbu_id) = &context.cbu_id {
            prompt.push_str(&format!("CBU ID: {}\n", cbu_id));
        }

        if let Some(case_id) = &context.case_id {
            prompt.push_str(&format!("Case ID: {}\n", case_id));
        }

        if let Some(domain) = &context.domain {
            prompt.push_str(&format!("Domain: {}\n", domain));
        }

        // Add DSL format guidelines
        prompt.push_str(
            r#"
Please generate a valid DSL document following these guidelines:

1. Use S-expression syntax with parentheses
2. Include appropriate domain-specific verbs (e.g., case.create, entity.register, ubo.calculate)
3. Use proper AttributeID references where applicable
4. Include necessary metadata and timestamps
5. Follow the established DSL V3.1 grammar
6. Ensure the generated DSL is executable and valid

DSL Examples:
- (case.create (case.id "uuid") (entity.name "Company") (status "ACTIVE"))
- (entity.register (entity.id "uuid") (entity.type "CORPORATION") (jurisdiction "US"))
- (ubo.calculate (entity.id "uuid") (threshold 0.25) (method "DIRECT_OWNERSHIP"))

Generate only the DSL content, without explanations or markdown formatting.
"#,
        );

        prompt
    }

    /// Enhance prompt with RAG context
    fn enhance_prompt_with_rag(&self, mut prompt: String, rag_context: &RagContext) -> String {
        if !rag_context.documents.is_empty() {
            prompt.push_str("\nRelevant Examples and Context:\n");

            for (i, doc) in rag_context.documents.iter().take(3).enumerate() {
                prompt.push_str(&format!(
                    "\nExample {} (relevance: {:.2}):\n{}\n",
                    i + 1,
                    doc.score,
                    doc.content
                ));
            }

            prompt.push_str("\nUse these examples as guidance for generating the DSL.\n");
        }

        // Trim to max context length
        if prompt.len() > self.config.max_rag_context_length {
            let truncate_pos = self.config.max_rag_context_length;
            prompt.truncate(truncate_pos);
            prompt.push_str("\n[Context truncated to fit limits]");
        }

        prompt
    }

    /// Convert generation request to AI request
    async fn to_ai_request(
        &self,
        request: &GenerationRequest,
    ) -> GenerationResult<AiGenerationRequest> {
        let prompt = self.build_rag_prompt(request).await?;

        let mut context_data = HashMap::new();
        context_data.insert("prompt".to_string(), prompt);

        // Add all context data
        for (key, value) in &request.context.entity_data {
            context_data.insert(key.clone(), value.clone());
        }

        if let Some(cbu_id) = &request.context.cbu_id {
            context_data.insert("cbu_id".to_string(), cbu_id.clone());
        }

        if let Some(case_id) = &request.context.case_id {
            context_data.insert("case_id".to_string(), case_id.clone());
        }

        let parameters = AiGenerationParameters {
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            top_p: Some(0.9),
            frequency_penalty: Some(0.1),
            presence_penalty: Some(0.1),
        };

        Ok(AiGenerationRequest {
            operation_type: request.operation_type.clone(),
            instruction: request
                .context
                .instruction
                .clone()
                .unwrap_or_else(|| format!("Generate DSL for {}", request.operation_type)),
            context_data,
            rag_context: None, // RAG context is already embedded in the prompt
            model: Some(self.config.default_model.clone()),
            parameters,
        })
    }

    /// Validate generated DSL content
    fn validate_generated_dsl(&self, dsl_content: &str) -> Vec<String> {
        let mut errors = vec![];

        // Basic syntax checks
        if dsl_content.trim().is_empty() {
            errors.push("Generated DSL is empty".to_string());
            return errors;
        }

        // Check for balanced parentheses
        let open_parens = dsl_content.matches('(').count();
        let close_parens = dsl_content.matches(')').count();

        if open_parens != close_parens {
            errors.push(format!(
                "Unbalanced parentheses: {} open, {} close",
                open_parens, close_parens
            ));
        }

        // Check for at least one DSL verb
        let common_verbs = [
            "case.create",
            "case.update",
            "entity.register",
            "ubo.calculate",
            "kyc.start",
            "compliance.screen",
            "document.catalog",
            "isda.establish_master",
        ];

        let has_verb = common_verbs.iter().any(|verb| dsl_content.contains(verb));

        if !has_verb {
            errors.push("Generated DSL does not contain recognizable verbs".to_string());
        }

        // Check for common DSL patterns
        if !dsl_content.contains('"') && !dsl_content.contains("'") {
            warn!("Generated DSL may be missing string literals");
        }

        errors
    }

    /// Calculate confidence score for generated DSL
    fn calculate_confidence_score(
        &self,
        dsl_content: &str,
        ai_response: &AiGenerationResponse,
        validation_errors: &[String],
    ) -> f64 {
        if !self.config.enable_confidence_scoring {
            return ai_response.confidence_score;
        }

        let mut confidence = ai_response.confidence_score;

        // Reduce confidence for validation errors
        let error_penalty = validation_errors.len() as f64 * 0.2;
        confidence -= error_penalty;

        // Increase confidence for well-formed DSL
        let length_bonus = (dsl_content.len() as f64 / 1000.0).min(0.1);
        confidence += length_bonus;

        // Check for complexity indicators
        let complexity_indicators = [
            "entity.register",
            "ubo.calculate",
            "workflow",
            "BEGIN",
            "END",
            "set-attribute",
        ];

        let complexity_score = complexity_indicators
            .iter()
            .map(|indicator| {
                if dsl_content.contains(indicator) {
                    0.05
                } else {
                    0.0
                }
            })
            .sum::<f64>();

        confidence += complexity_score;

        // Clamp to valid range
        confidence.max(0.0).min(1.0)
    }
}

#[async_trait]
impl DslGenerator for AiGenerator {
    async fn generate_dsl(
        &self,
        request: GenerationRequest,
    ) -> GenerationResult<GenerationResponse> {
        let start_time = std::time::Instant::now();

        info!(
            "Starting AI-based DSL generation for operation: {}",
            request.operation_type
        );

        // Convert to AI request
        let ai_request = self.to_ai_request(&request).await?;

        // Generate DSL with timeout and retries
        let mut last_error = None;

        for attempt in 1..=self.config.max_retries {
            debug!(
                "AI generation attempt {} of {}",
                attempt, self.config.max_retries
            );

            let ai_future = self.ai_service.generate_dsl_ai(&ai_request);
            let timeout_duration = Duration::from_secs(self.config.ai_timeout_seconds);

            match timeout(timeout_duration, ai_future).await {
                Ok(Ok(ai_response)) => {
                    let validation_errors = self.validate_generated_dsl(&ai_response.dsl_content);

                    let confidence_score = self.calculate_confidence_score(
                        &ai_response.dsl_content,
                        &ai_response,
                        &validation_errors,
                    );

                    let processing_time = start_time.elapsed().as_millis() as u64;

                    info!(
                        "AI-based DSL generation completed in {}ms with confidence {:.2}",
                        processing_time, confidence_score
                    );

                    return Ok(GenerationResponse {
                        success: validation_errors.is_empty(),
                        dsl_content: ai_response.dsl_content,
                        method_used: GenerationMethod::Ai {
                            model: ai_response.model_used.clone(),
                        },
                        processing_time_ms: processing_time,
                        confidence_score: Some(confidence_score),
                        errors: validation_errors,
                        warnings: vec![],
                        debug_info: if request.options.include_debug_info {
                            Some(super::traits::GenerationDebugInfo {
                                template_id: None,
                                ai_model: Some(ai_response.model_used),
                                resolved_variables: ai_request.context_data,
                                generation_steps: vec![
                                    "Build RAG prompt".to_string(),
                                    "Generate with AI".to_string(),
                                    "Validate output".to_string(),
                                    "Calculate confidence".to_string(),
                                ],
                                performance_metrics: {
                                    let mut metrics = HashMap::new();
                                    metrics.insert(
                                        "processing_time_ms".to_string(),
                                        processing_time as f64,
                                    );
                                    metrics.insert(
                                        "prompt_tokens".to_string(),
                                        ai_response.token_usage.prompt_tokens as f64,
                                    );
                                    metrics.insert(
                                        "completion_tokens".to_string(),
                                        ai_response.token_usage.completion_tokens as f64,
                                    );
                                    metrics.insert(
                                        "total_tokens".to_string(),
                                        ai_response.token_usage.total_tokens as f64,
                                    );
                                    metrics.insert("attempts".to_string(), attempt as f64);
                                    metrics
                                },
                            })
                        } else {
                            None
                        },
                        metadata: ai_response.metadata,
                    });
                }
                Ok(Err(ai_error)) => {
                    error!("AI generation attempt {} failed: {}", attempt, ai_error);
                    last_error = Some(ai_error);

                    // Don't retry certain errors
                    match &last_error {
                        Some(AiGenerationError::AuthenticationFailed)
                        | Some(AiGenerationError::InsufficientQuota) => break,
                        _ => continue,
                    }
                }
                Err(_timeout_error) => {
                    error!("AI generation attempt {} timed out", attempt);
                    last_error = Some(AiGenerationError::Timeout);
                }
            }
        }

        // All attempts failed
        let error_message = match last_error {
            Some(error) => format!(
                "AI generation failed after {} attempts: {}",
                self.config.max_retries, error
            ),
            None => format!(
                "AI generation failed after {} attempts: unknown error",
                self.config.max_retries
            ),
        };

        Err(GenerationError::AiGenerationFailed {
            reason: error_message,
        })
    }

    fn can_handle(&self, operation_type: &GenerationOperationType) -> bool {
        // AI generator can handle any operation type
        match operation_type {
            GenerationOperationType::CreateCbu
            | GenerationOperationType::RegisterEntity
            | GenerationOperationType::CalculateUbo
            | GenerationOperationType::UpdateDsl
            | GenerationOperationType::KycWorkflow
            | GenerationOperationType::ComplianceCheck
            | GenerationOperationType::DocumentCatalog
            | GenerationOperationType::IsdaTrade
            | GenerationOperationType::Custom { .. } => true,
        }
    }

    fn metadata(&self) -> GeneratorMetadata {
        let service_metadata = self.ai_service.service_metadata();

        GeneratorMetadata {
            name: "AiGenerator".to_string(),
            version: "1.0.0".to_string(),
            supported_operations: vec![
                GenerationOperationType::CreateCbu,
                GenerationOperationType::RegisterEntity,
                GenerationOperationType::CalculateUbo,
                GenerationOperationType::UpdateDsl,
                GenerationOperationType::KycWorkflow,
                GenerationOperationType::ComplianceCheck,
                GenerationOperationType::DocumentCatalog,
                GenerationOperationType::IsdaTrade,
            ],
            average_confidence: 0.8, // AI generators are less predictable than templates
            average_processing_time_ms: 2000, // Slower than templates
            available: service_metadata.status == AiServiceStatus::Active,
            capabilities: {
                let mut caps = HashMap::new();
                caps.insert("flexible".to_string(), "true".to_string());
                caps.insert("context_aware".to_string(), "true".to_string());
                caps.insert("natural_language".to_string(), "true".to_string());
                caps.insert("ai_powered".to_string(), "true".to_string());
                caps.insert("models".to_string(), service_metadata.models.join(","));
                caps
            },
        }
    }

    async fn health_check(&self) -> GenerationResult<bool> {
        Ok(self.ai_service.health_check().await)
    }

    fn validate_request(&self, request: &GenerationRequest) -> GenerationResult<()> {
        let mut errors = vec![];

        if request.options.timeout_ms == 0 {
            errors.push("Timeout must be greater than 0".to_string());
        }

        if request.context.instruction.is_none() && request.context.entity_data.is_empty() {
            errors.push(
                "Either instruction or entity data must be provided for AI generation".to_string(),
            );
        }

        if !errors.is_empty() {
            return Err(GenerationError::ValidationError { errors });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Mock AI service for testing
    #[derive(Debug, Clone)]
    struct MockAiService {
        should_fail: bool,
        response_content: String,
    }

    impl MockAiService {
        fn new() -> Self {
            Self {
                should_fail: false,
                response_content: "(test.operation (result \"success\"))".to_string(),
            }
        }

        fn with_failure() -> Self {
            Self {
                should_fail: true,
                response_content: String::new(),
            }
        }
    }

    #[async_trait]
    impl AiDslGenerationService for MockAiService {
        async fn generate_dsl_ai(
            &self,
            _request: &AiGenerationRequest,
        ) -> Result<AiGenerationResponse, AiGenerationError> {
            if self.should_fail {
                return Err(AiGenerationError::ServiceError {
                    message: "Mock failure".to_string(),
                });
            }

            Ok(AiGenerationResponse {
                dsl_content: self.response_content.clone(),
                model_used: "mock-model".to_string(),
                confidence_score: 0.9,
                token_usage: TokenUsage {
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                },
                metadata: HashMap::new(),
            })
        }

        fn available_models(&self) -> Vec<String> {
            vec!["mock-model".to_string()]
        }

        async fn health_check(&self) -> bool {
            !self.should_fail
        }

        fn service_metadata(&self) -> AiServiceMetadata {
            AiServiceMetadata {
                name: "MockAiService".to_string(),
                version: "1.0.0".to_string(),
                models: vec!["mock-model".to_string()],
                capabilities: HashMap::new(),
                status: if self.should_fail {
                    AiServiceStatus::Inactive
                } else {
                    AiServiceStatus::Active
                },
            }
        }
    }

    #[tokio::test]
    async fn test_ai_generator_creation() {
        let mock_service = Arc::new(MockAiService::new());
        let generator = AiGenerator::new(mock_service);

        assert_eq!(generator.config.default_model, "gpt-3.5-turbo");
        assert_eq!(generator.config.max_tokens, 2048);
    }

    #[tokio::test]
    async fn test_ai_dsl_generation() {
        let mock_service = Arc::new(MockAiService::new());
        let generator = AiGenerator::new(mock_service);

        let mut context = super::super::traits::GenerationContext::default();
        context.instruction = Some("Create a test operation".to_string());

        let request = GenerationRequest::new(GenerationOperationType::CreateCbu, context);

        let response = generator.generate_dsl(request).await.unwrap();

        assert!(response.success);
        assert!(response.dsl_content.contains("test.operation"));
        assert!(response.confidence_score.is_some());
        assert!(response.confidence_score.unwrap() > 0.0);
    }

    #[tokio::test]
    async fn test_ai_generation_failure() {
        let mock_service = Arc::new(MockAiService::with_failure());
        let generator = AiGenerator::new(mock_service);

        let context = super::super::traits::GenerationContext::default();
        let request = GenerationRequest::new(GenerationOperationType::CreateCbu, context);

        let result = generator.generate_dsl(request).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            GenerationError::AiGenerationFailed { .. } => {
                // Expected error type
            }
            other => panic!("Unexpected error type: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_prompt_building() {
        let mock_service = Arc::new(MockAiService::new());
        let generator = AiGenerator::new(mock_service);

        let mut context = super::super::traits::GenerationContext::default();
        context.instruction = Some("Test instruction".to_string());
        context.cbu_id = Some("test-cbu-id".to_string());

        let request = GenerationRequest::new(GenerationOperationType::RegisterEntity, context);

        let prompt = generator.build_base_prompt(&request);

        assert!(prompt.contains("RegisterEntity"));
        assert!(prompt.contains("Test instruction"));
        assert!(prompt.contains("test-cbu-id"));
        assert!(prompt.contains("S-expression syntax"));
    }

    #[tokio::test]
    async fn test_dsl_validation() {
        let mock_service = Arc::new(MockAiService::new());
        let generator = AiGenerator::new(mock_service);

        // Valid DSL
        let valid_dsl = "(case.create (case.id \"123\") (status \"ACTIVE\"))";
        let errors = generator.validate_generated_dsl(valid_dsl);
        assert!(errors.is_empty());

        // Invalid DSL - unbalanced parentheses
        let invalid_dsl = "(case.create (case.id \"123\" (status \"ACTIVE\")";
        let errors = generator.validate_generated_dsl(invalid_dsl);
        assert!(!errors.is_empty());

        // Empty DSL
        let empty_dsl = "";
        let errors = generator.validate_generated_dsl(empty_dsl);
        assert!(!errors.is_empty());
    }

    #[tokio::test]
    async fn test_health_check() {
        let healthy_service = Arc::new(MockAiService::new());
        let healthy_generator = AiGenerator::new(healthy_service);

        let health = healthy_generator.health_check().await.unwrap();
        assert!(health);

        let unhealthy_service = Arc::new(MockAiService::with_failure());
        let unhealthy_generator = AiGenerator::new(unhealthy_service);

        let health = unhealthy_generator.health_check().await.unwrap();
        assert!(!health);
    }
}
