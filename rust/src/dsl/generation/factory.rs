//! Generation Factory Module
//!
//! This module provides factory methods for creating DSL generators,
//! implementing the factory pattern for generation method selection.

use super::{
    ai::{AiDslGenerationService, AiGenerator, AiGeneratorConfig},
    template::{TemplateGenerator, TemplateGeneratorConfig},
    traits::{
        DslGenerator, GenerationError, GenerationOperationType, GenerationResult, GeneratorFactory,
        GeneratorType,
    },
};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Generation method selection strategy
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationMethod {
    /// Use template-based generation (Direct Method)
    Template,
    /// Use AI-based generation (Agent Method)
    Ai,
    /// Use hybrid approach with fallback
    Hybrid,
    /// Automatically select best method
    Auto,
}

/// Factory for creating DSL generators
pub struct GenerationFactory {
    /// Templates directory path
    templates_dir: PathBuf,

    /// Template generator configuration
    template_config: TemplateGeneratorConfig,

    /// AI generator configuration
    ai_config: AiGeneratorConfig,

    /// AI service for AI generation
    ai_service: Option<Arc<dyn AiDslGenerationService + Send + Sync>>,

    /// Default generation method
    default_method: GenerationMethod,
}

/// Hybrid generator that combines template and AI generation
pub struct HybridGenerator {
    template_generator: TemplateGenerator,
    ai_generator: AiGenerator,
    fallback_strategy: FallbackStrategy,
}

/// Fallback strategy for hybrid generation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackStrategy {
    /// Try template first, fallback to AI
    TemplateThenAi,
    /// Try AI first, fallback to template
    AiThenTemplate,
    /// Use the fastest available method
    Fastest,
    /// Use the most accurate method
    MostAccurate,
}

/// Generator selection metadata
#[derive(Debug, Clone)]
pub struct GeneratorSelection {
    /// Selected generator type
    pub generator_type: GeneratorType,

    /// Reason for selection
    pub selection_reason: String,

    /// Confidence in selection (0.0 to 1.0)
    pub selection_confidence: f64,

    /// Alternative generators considered
    pub alternatives: Vec<GeneratorType>,
}

impl GenerationFactory {
    /// Create a new generation factory
    pub fn new(templates_dir: PathBuf) -> Self {
        Self {
            templates_dir,
            template_config: TemplateGeneratorConfig::default(),
            ai_config: AiGeneratorConfig::default(),
            ai_service: None,
            default_method: GenerationMethod::Template,
        }
    }

    /// Create factory with AI service
    pub fn with_ai_service(
        templates_dir: PathBuf,
        ai_service: Arc<dyn AiDslGenerationService + Send + Sync>,
    ) -> Self {
        Self {
            templates_dir,
            template_config: TemplateGeneratorConfig::default(),
            ai_config: AiGeneratorConfig::default(),
            ai_service: Some(ai_service),
            default_method: GenerationMethod::Hybrid,
        }
    }

    /// Set template configuration
    pub fn with_template_config(mut self, config: TemplateGeneratorConfig) -> Self {
        self.template_config = config;
        self
    }

    /// Set AI configuration
    pub fn with_ai_config(mut self, config: AiGeneratorConfig) -> Self {
        self.ai_config = config;
        self
    }

    /// Set default generation method
    pub fn with_default_method(mut self, method: GenerationMethod) -> Self {
        self.default_method = method;
        self
    }

    /// Create a template generator
    pub fn create_template_generator(&self) -> GenerationResult<TemplateGenerator> {
        info!(
            "Creating template generator with templates dir: {:?}",
            self.templates_dir
        );
        Ok(TemplateGenerator::with_config(
            self.templates_dir.clone(),
            self.template_config.clone(),
        ))
    }

    /// Create an AI generator
    pub fn create_ai_generator(&self) -> GenerationResult<AiGenerator> {
        let ai_service =
            self.ai_service
                .as_ref()
                .ok_or_else(|| GenerationError::ConfigurationError {
                    message: "AI service not configured".to_string(),
                })?;

        info!(
            "Creating AI generator with model: {}",
            self.ai_config.default_model
        );
        Ok(AiGenerator::with_config(
            ai_service.clone(),
            self.ai_config.clone(),
        ))
    }

    /// Create a hybrid generator
    pub fn create_hybrid_generator(&self) -> GenerationResult<HybridGenerator> {
        let template_generator = self.create_template_generator()?;
        let ai_generator = self.create_ai_generator()?;

        info!("Creating hybrid generator with template-first fallback strategy");
        Ok(HybridGenerator::new(
            template_generator,
            ai_generator,
            FallbackStrategy::TemplateThenAi,
        ))
    }

    /// Create generator by type
    pub fn create_generator(
        &self,
        generator_type: GeneratorType,
    ) -> GenerationResult<Box<dyn DslGenerator>> {
        match generator_type {
            GeneratorType::Template => {
                let generator = self.create_template_generator()?;
                Ok(Box::new(generator))
            }
            GeneratorType::Ai => {
                let generator = self.create_ai_generator()?;
                Ok(Box::new(generator))
            }
            GeneratorType::Hybrid => {
                let generator = self.create_hybrid_generator()?;
                Ok(Box::new(generator))
            }
        }
    }

    /// Select best generator for operation type
    pub async fn select_generator(
        &self,
        operation_type: &GenerationOperationType,
    ) -> GenerationResult<GeneratorSelection> {
        match self.default_method {
            GenerationMethod::Template => Ok(GeneratorSelection {
                generator_type: GeneratorType::Template,
                selection_reason: "Default method is template".to_string(),
                selection_confidence: 1.0,
                alternatives: vec![GeneratorType::Ai, GeneratorType::Hybrid],
            }),
            GenerationMethod::Ai => {
                if self.ai_service.is_some() {
                    Ok(GeneratorSelection {
                        generator_type: GeneratorType::Ai,
                        selection_reason: "Default method is AI and service is available"
                            .to_string(),
                        selection_confidence: 1.0,
                        alternatives: vec![GeneratorType::Template, GeneratorType::Hybrid],
                    })
                } else {
                    warn!(
                        "AI method selected but no AI service available, falling back to template"
                    );
                    Ok(GeneratorSelection {
                        generator_type: GeneratorType::Template,
                        selection_reason: "AI service not available, fallback to template"
                            .to_string(),
                        selection_confidence: 0.7,
                        alternatives: vec![GeneratorType::Ai],
                    })
                }
            }
            GenerationMethod::Hybrid => {
                if self.ai_service.is_some() {
                    Ok(GeneratorSelection {
                        generator_type: GeneratorType::Hybrid,
                        selection_reason: "Hybrid method with both template and AI available"
                            .to_string(),
                        selection_confidence: 1.0,
                        alternatives: vec![GeneratorType::Template, GeneratorType::Ai],
                    })
                } else {
                    Ok(GeneratorSelection {
                        generator_type: GeneratorType::Template,
                        selection_reason: "Hybrid requested but AI unavailable, using template"
                            .to_string(),
                        selection_confidence: 0.8,
                        alternatives: vec![GeneratorType::Hybrid],
                    })
                }
            }
            GenerationMethod::Auto => self.auto_select_generator(operation_type).await,
        }
    }

    /// Automatically select the best generator
    async fn auto_select_generator(
        &self,
        operation_type: &GenerationOperationType,
    ) -> GenerationResult<GeneratorSelection> {
        let mut selection_confidence = 0.0;
        let mut selection_reason = String::new();
        let mut alternatives = vec![];

        // Check template availability
        let template_available = self.templates_dir.exists() && self.templates_dir.is_dir();

        // Check AI availability
        let ai_available = if let Some(ai_service) = &self.ai_service {
            ai_service.health_check().await
        } else {
            false
        };

        debug!(
            "Auto selection: template_available={}, ai_available={}",
            template_available, ai_available
        );

        // Selection logic based on operation type and availability
        let selected_type = match operation_type {
            // Standard operations work well with templates
            GenerationOperationType::CreateCbu
            | GenerationOperationType::RegisterEntity
            | GenerationOperationType::CalculateUbo => {
                if template_available {
                    alternatives.push(GeneratorType::Ai);
                    if ai_available {
                        alternatives.push(GeneratorType::Hybrid);
                    }
                    selection_confidence = 0.9;
                    selection_reason = "Standard operation with template available".to_string();
                    GeneratorType::Template
                } else if ai_available {
                    alternatives.push(GeneratorType::Template);
                    selection_confidence = 0.7;
                    selection_reason = "Template not available, using AI".to_string();
                    GeneratorType::Ai
                } else {
                    selection_confidence = 0.3;
                    selection_reason = "No generators available, attempting template".to_string();
                    GeneratorType::Template
                }
            }

            // Complex operations benefit from AI
            GenerationOperationType::KycWorkflow
            | GenerationOperationType::ComplianceCheck
            | GenerationOperationType::IsdaTrade => {
                if ai_available && template_available {
                    alternatives.extend(vec![GeneratorType::Template, GeneratorType::Ai]);
                    selection_confidence = 0.9;
                    selection_reason = "Complex operation, using hybrid approach".to_string();
                    GeneratorType::Hybrid
                } else if ai_available {
                    alternatives.push(GeneratorType::Template);
                    selection_confidence = 0.8;
                    selection_reason = "Complex operation, AI preferred".to_string();
                    GeneratorType::Ai
                } else if template_available {
                    alternatives.push(GeneratorType::Ai);
                    selection_confidence = 0.6;
                    selection_reason = "Complex operation but only template available".to_string();
                    GeneratorType::Template
                } else {
                    selection_confidence = 0.3;
                    selection_reason = "No generators available for complex operation".to_string();
                    GeneratorType::Template
                }
            }

            // Custom operations benefit from AI
            GenerationOperationType::Custom { .. } => {
                if ai_available {
                    alternatives.push(GeneratorType::Template);
                    if template_available {
                        alternatives.push(GeneratorType::Hybrid);
                    }
                    selection_confidence = 0.8;
                    selection_reason = "Custom operation, AI can handle flexibility".to_string();
                    GeneratorType::Ai
                } else if template_available {
                    alternatives.push(GeneratorType::Ai);
                    selection_confidence = 0.5;
                    selection_reason = "Custom operation but only template available".to_string();
                    GeneratorType::Template
                } else {
                    selection_confidence = 0.2;
                    selection_reason = "Custom operation with no good options".to_string();
                    GeneratorType::Template
                }
            }

            // Document and update operations are straightforward
            GenerationOperationType::DocumentCatalog | GenerationOperationType::UpdateDsl => {
                if template_available {
                    alternatives.push(GeneratorType::Ai);
                    selection_confidence = 0.8;
                    selection_reason = "Document/update operation with template".to_string();
                    GeneratorType::Template
                } else if ai_available {
                    alternatives.push(GeneratorType::Template);
                    selection_confidence = 0.7;
                    selection_reason = "Document/update operation with AI fallback".to_string();
                    GeneratorType::Ai
                } else {
                    selection_confidence = 0.4;
                    selection_reason = "Document/update operation, no ideal generator".to_string();
                    GeneratorType::Template
                }
            }
        };

        info!(
            "Auto-selected generator: {:?} for operation: {} (confidence: {:.2})",
            selected_type, operation_type, selection_confidence
        );

        Ok(GeneratorSelection {
            generator_type: selected_type,
            selection_reason,
            selection_confidence,
            alternatives,
        })
    }

    /// Create generator based on auto-selection
    pub async fn create_auto_generator(
        &self,
        operation_type: &GenerationOperationType,
    ) -> GenerationResult<(Box<dyn DslGenerator>, GeneratorSelection)> {
        let selection = self.select_generator(operation_type).await?;
        let generator = self.create_generator(selection.generator_type.clone())?;
        Ok((generator, selection))
    }

    /// Check health of all available generators
    pub async fn health_check(&self) -> GenerationResult<FactoryHealthStatus> {
        let mut template_healthy = false;
        let mut ai_healthy = false;
        let mut errors = vec![];

        // Check template generator
        match self.create_template_generator() {
            Ok(template_gen) => {
                template_healthy = template_gen.health_check().await.unwrap_or(false);
            }
            Err(e) => {
                errors.push(format!("Template generator creation failed: {}", e));
            }
        }

        // Check AI generator
        if self.ai_service.is_some() {
            match self.create_ai_generator() {
                Ok(ai_gen) => {
                    ai_healthy = ai_gen.health_check().await.unwrap_or(false);
                }
                Err(e) => {
                    errors.push(format!("AI generator creation failed: {}", e));
                }
            }
        } else {
            errors.push("AI service not configured".to_string());
        }

        Ok(FactoryHealthStatus {
            template_healthy,
            ai_healthy,
            hybrid_healthy: template_healthy && ai_healthy,
            errors,
            checked_at: chrono::Utc::now(),
        })
    }
}

/// Factory health status
#[derive(Debug, Clone)]
pub struct FactoryHealthStatus {
    pub template_healthy: bool,
    pub ai_healthy: bool,
    pub hybrid_healthy: bool,
    pub errors: Vec<String>,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

impl HybridGenerator {
    /// Create a new hybrid generator
    pub fn new(
        template_generator: TemplateGenerator,
        ai_generator: AiGenerator,
        fallback_strategy: FallbackStrategy,
    ) -> Self {
        Self {
            template_generator,
            ai_generator,
            fallback_strategy,
        }
    }

    /// Set fallback strategy
    pub fn with_fallback_strategy(mut self, strategy: FallbackStrategy) -> Self {
        self.fallback_strategy = strategy;
        self
    }
}

#[async_trait::async_trait]
impl DslGenerator for HybridGenerator {
    async fn generate_dsl(
        &self,
        request: super::traits::GenerationRequest,
    ) -> GenerationResult<super::traits::GenerationResponse> {
        let start_time = std::time::Instant::now();

        info!(
            "Starting hybrid DSL generation with strategy: {:?}",
            self.fallback_strategy
        );

        let (primary_generator, fallback_generator): (&dyn DslGenerator, &dyn DslGenerator) =
            match self.fallback_strategy {
                FallbackStrategy::TemplateThenAi => (&self.template_generator, &self.ai_generator),
                FallbackStrategy::AiThenTemplate => (&self.ai_generator, &self.template_generator),
                FallbackStrategy::Fastest => {
                    // For simplicity, assume template is faster
                    (&self.template_generator, &self.ai_generator)
                }
                FallbackStrategy::MostAccurate => {
                    // For simplicity, assume AI is more accurate
                    (&self.ai_generator, &self.template_generator)
                }
            };

        // Try primary generator first
        match primary_generator.generate_dsl(request.clone()).await {
            Ok(mut response) => {
                let processing_time = start_time.elapsed().as_millis() as u64;
                response.processing_time_ms = processing_time;

                // Add hybrid metadata
                response
                    .metadata
                    .insert("hybrid_primary_used".to_string(), "true".to_string());
                response.metadata.insert(
                    "fallback_strategy".to_string(),
                    format!("{:?}", self.fallback_strategy),
                );

                info!(
                    "Hybrid generation succeeded with primary generator in {}ms",
                    processing_time
                );
                Ok(response)
            }
            Err(primary_error) => {
                warn!(
                    "Primary generator failed: {}, trying fallback",
                    primary_error
                );

                // Try fallback generator
                match fallback_generator.generate_dsl(request).await {
                    Ok(mut response) => {
                        let processing_time = start_time.elapsed().as_millis() as u64;
                        response.processing_time_ms = processing_time;

                        // Add hybrid metadata
                        response
                            .metadata
                            .insert("hybrid_fallback_used".to_string(), "true".to_string());
                        response
                            .metadata
                            .insert("primary_error".to_string(), primary_error.to_string());
                        response.metadata.insert(
                            "fallback_strategy".to_string(),
                            format!("{:?}", self.fallback_strategy),
                        );

                        info!(
                            "Hybrid generation succeeded with fallback generator in {}ms",
                            processing_time
                        );
                        Ok(response)
                    }
                    Err(fallback_error) => {
                        let error_msg = format!(
                            "Hybrid generation failed - Primary: {}, Fallback: {}",
                            primary_error, fallback_error
                        );

                        Err(GenerationError::AiGenerationFailed { reason: error_msg })
                    }
                }
            }
        }
    }

    fn can_handle(&self, operation_type: &GenerationOperationType) -> bool {
        // Hybrid can handle anything either generator can handle
        self.template_generator.can_handle(operation_type)
            || self.ai_generator.can_handle(operation_type)
    }

    fn metadata(&self) -> super::traits::GeneratorMetadata {
        let template_meta = self.template_generator.metadata();
        let ai_meta = self.ai_generator.metadata();

        // Combine metadata from both generators
        let mut supported_operations = template_meta.supported_operations;
        supported_operations.extend(ai_meta.supported_operations);
        supported_operations.sort_by_key(|a| a.to_string());
        supported_operations.dedup();

        let mut capabilities = template_meta.capabilities;
        capabilities.extend(ai_meta.capabilities);
        capabilities.insert("hybrid".to_string(), "true".to_string());
        capabilities.insert(
            "fallback_strategy".to_string(),
            format!("{:?}", self.fallback_strategy),
        );

        super::traits::GeneratorMetadata {
            name: "HybridGenerator".to_string(),
            version: "1.0.0".to_string(),
            supported_operations,
            average_confidence: (template_meta.average_confidence + ai_meta.average_confidence)
                / 2.0,
            average_processing_time_ms: template_meta
                .average_processing_time_ms
                .max(ai_meta.average_processing_time_ms),
            available: template_meta.available || ai_meta.available,
            capabilities,
        }
    }

    async fn health_check(&self) -> GenerationResult<bool> {
        let template_health = self
            .template_generator
            .health_check()
            .await
            .unwrap_or(false);
        let ai_health = self.ai_generator.health_check().await.unwrap_or(false);

        // Hybrid is healthy if at least one generator is healthy
        Ok(template_health || ai_health)
    }

    fn validate_request(&self, request: &super::traits::GenerationRequest) -> GenerationResult<()> {
        // Validate request against both generators
        let template_result = self.template_generator.validate_request(request);
        let ai_result = self.ai_generator.validate_request(request);

        // If either generator can handle the request, it's valid
        match (template_result, ai_result) {
            (Ok(()), _) | (_, Ok(())) => Ok(()),
            (Err(template_error), Err(ai_error)) => Err(GenerationError::ValidationError {
                errors: vec![
                    format!("Template validation: {}", template_error),
                    format!("AI validation: {}", ai_error),
                ],
            }),
        }
    }
}

#[async_trait::async_trait]
impl GeneratorFactory for GenerationFactory {
    async fn create_generator(
        &self,
        generator_type: GeneratorType,
    ) -> GenerationResult<Box<dyn DslGenerator>> {
        self.create_generator(generator_type)
    }

    fn available_generators(&self) -> Vec<GeneratorType> {
        let mut available = vec![GeneratorType::Template];

        if self.ai_service.is_some() {
            available.push(GeneratorType::Ai);
            available.push(GeneratorType::Hybrid);
        }

        available
    }

    fn recommend_generator(
        &self,
        operation_type: &GenerationOperationType,
    ) -> Option<GeneratorType> {
        // Simple recommendation logic
        match operation_type {
            GenerationOperationType::CreateCbu
            | GenerationOperationType::RegisterEntity
            | GenerationOperationType::CalculateUbo => Some(GeneratorType::Template),

            GenerationOperationType::KycWorkflow
            | GenerationOperationType::ComplianceCheck
            | GenerationOperationType::IsdaTrade
            | GenerationOperationType::Custom { .. } => {
                if self.ai_service.is_some() {
                    Some(GeneratorType::Hybrid)
                } else {
                    Some(GeneratorType::Template)
                }
            }

            GenerationOperationType::DocumentCatalog | GenerationOperationType::UpdateDsl => {
                Some(GeneratorType::Template)
            }
        }
    }
}

impl std::fmt::Display for GenerationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Template => write!(f, "template"),
            Self::Ai => write!(f, "ai"),
            Self::Hybrid => write!(f, "hybrid"),
            Self::Auto => write!(f, "auto"),
        }
    }
}

impl std::fmt::Display for FallbackStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TemplateThenAi => write!(f, "template_then_ai"),
            Self::AiThenTemplate => write!(f, "ai_then_template"),
            Self::Fastest => write!(f, "fastest"),
            Self::MostAccurate => write!(f, "most_accurate"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::generation::ai::{AiServiceMetadata, AiServiceStatus, TokenUsage};
    use crate::dsl::generation::{AiGenerationError, AiGenerationRequest, AiGenerationResponse};
    use std::sync::Arc;
    use tempfile::tempdir;

    // Mock AI service for testing
    #[derive(Debug, Clone)]
    struct MockAiService {
        healthy: bool,
    }

    impl MockAiService {
        fn new(healthy: bool) -> Self {
            Self { healthy }
        }
    }

    #[async_trait::async_trait]
    impl AiDslGenerationService for MockAiService {
        async fn generate_dsl_ai(
            &self,
            _request: &AiGenerationRequest,
        ) -> Result<AiGenerationResponse, AiGenerationError> {
            if self.healthy {
                Ok(AiGenerationResponse {
                    dsl_content: "(test.operation)".to_string(),
                    model_used: "mock".to_string(),
                    confidence_score: 0.8,
                    token_usage: TokenUsage {
                        prompt_tokens: 10,
                        completion_tokens: 5,
                        total_tokens: 15,
                    },
                    metadata: std::collections::HashMap::new(),
                })
            } else {
                Err(AiGenerationError::ServiceError {
                    message: "Mock failure".to_string(),
                })
            }
        }

        fn available_models(&self) -> Vec<String> {
            vec!["mock".to_string()]
        }

        async fn health_check(&self) -> bool {
            self.healthy
        }

        fn service_metadata(&self) -> AiServiceMetadata {
            AiServiceMetadata {
                name: "Mock".to_string(),
                version: "1.0".to_string(),
                models: vec!["mock".to_string()],
                capabilities: std::collections::HashMap::new(),
                status: if self.healthy {
                    AiServiceStatus::Active
                } else {
                    AiServiceStatus::Inactive
                },
            }
        }
    }

    #[test]
    fn test_factory_creation() {
        let temp_dir = tempdir().unwrap();
        let factory = GenerationFactory::new(temp_dir.path().to_path_buf());

        assert_eq!(factory.default_method, GenerationMethod::Template);
        assert!(factory.ai_service.is_none());
    }

    #[test]
    fn test_factory_with_ai_service() {
        let temp_dir = tempdir().unwrap();
        let ai_service = Arc::new(MockAiService::new(true));
        let factory = GenerationFactory::with_ai_service(temp_dir.path().to_path_buf(), ai_service);

        assert_eq!(factory.default_method, GenerationMethod::Hybrid);
        assert!(factory.ai_service.is_some());
    }

    #[tokio::test]
    async fn test_template_generator_creation() {
        let temp_dir = tempdir().unwrap();
        let factory = GenerationFactory::new(temp_dir.path().to_path_buf());

        let generator = factory.create_template_generator().unwrap();
        assert!(generator.can_handle(&GenerationOperationType::CreateCbu));
    }

    #[tokio::test]
    async fn test_ai_generator_creation() {
        let temp_dir = tempdir().unwrap();
        let ai_service = Arc::new(MockAiService::new(true));
        let factory = GenerationFactory::with_ai_service(temp_dir.path().to_path_buf(), ai_service);

        let generator = factory.create_ai_generator().unwrap();
        assert!(generator.can_handle(&GenerationOperationType::CreateCbu));
    }

    #[tokio::test]
    async fn test_auto_selection() {
        let temp_dir = tempdir().unwrap();
        let ai_service = Arc::new(MockAiService::new(true));
        let factory = GenerationFactory::with_ai_service(temp_dir.path().to_path_buf(), ai_service)
            .with_default_method(GenerationMethod::Auto);

        let selection = factory
            .select_generator(&GenerationOperationType::CreateCbu)
            .await
            .unwrap();
        assert_eq!(selection.generator_type, GeneratorType::Template);
        assert!(selection.selection_confidence > 0.0);
    }

    #[tokio::test]
    async fn test_health_check() {
        let temp_dir = tempdir().unwrap();
        let ai_service = Arc::new(MockAiService::new(true));
        let factory = GenerationFactory::with_ai_service(temp_dir.path().to_path_buf(), ai_service);

        let health = factory.health_check().await.unwrap();
        assert!(health.template_healthy);
        assert!(health.ai_healthy);
        assert!(health.hybrid_healthy);
    }
}
