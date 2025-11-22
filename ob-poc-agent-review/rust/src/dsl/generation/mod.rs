//! DSL Generation Domain Library
//!
//! This module provides a comprehensive DSL generation system that supports
//! both template-based and AI-based generation methods, implementing the
//! Generation Method Substitution pattern for Phase 1.5.
//!
//! ## Architecture
//!
//! The generation system follows a clean architecture with multiple layers:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Generation Factory                           │
//! │  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐   │
//! │  │ Template        │ │ AI Generator    │ │ Hybrid          │   │
//! │  │ Generator       │ │                 │ │ Generator       │   │
//! │  └─────────────────┘ └─────────────────┘ └─────────────────┘   │
//! └─────────────────────────────────────────────────────────────────┘
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Common Traits & Types                       │
//! │  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐   │
//! │  │ DslGenerator    │ │ GenerationError │ │ GenerationContext│   │
//! │  │ Trait           │ │                 │ │ Builder         │   │
//! │  └─────────────────┘ └─────────────────┘ └─────────────────┘   │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Generation Methods
//!
//! ### Template Generation (Direct Method)
//! - Uses pre-defined templates with variable substitution
//! - Fast, deterministic, and reliable
//! - Best for standard operations with known patterns
//! - Templates are loaded from the filesystem and cached
//!
//! ### AI Generation (Agent Method)
//! - Uses AI services (OpenAI, Gemini) for dynamic generation
//! - Flexible, context-aware, and handles novel scenarios
//! - Best for complex or custom operations
//! - Supports RAG (Retrieval-Augmented Generation) for enhanced context
//!
//! ### Hybrid Generation
//! - Combines both template and AI generation with fallback strategies
//! - Provides the best of both worlds with fault tolerance
//! - Configurable fallback strategies (template-first, AI-first, etc.)
//!
//! ## Usage Examples
//!
//! ### Basic Template Generation
//!
//! ```rust
//! use crate::dsl::generation::{
//!     GenerationFactory, GenerationMethod, GenerationOperationType,
//!     GenerationContextBuilder, GenerationRequest
//! };
//!
//! let factory = GenerationFactory::new(templates_dir)
//!     .with_default_method(GenerationMethod::Template);
//!
//! let generator = factory.create_template_generator()?;
//!
//! let context = GenerationContextBuilder::new()
//!     .cbu_id("test-cbu-123")
//!     .template_variable("entity_name", "ACME Corp")
//!     .build();
//!
//! let request = GenerationRequest::new(
//!     GenerationOperationType::CreateCbu,
//!     context
//! );
//!
//! let response = generator.generate_dsl(request).await?;
//! println!("Generated DSL: {}", response.dsl_content);
//! ```
//!
//! ### AI-Powered Generation
//!
//! ```rust
//! let factory = GenerationFactory::with_ai_service(templates_dir, ai_service)
//!     .with_default_method(GenerationMethod::Ai);
//!
//! let generator = factory.create_ai_generator()?;
//!
//! let context = GenerationContextBuilder::new()
//!     .instruction("Create onboarding for UK hedge fund")
//!     .entity_data("fund_type", "HEDGE_FUND")
//!     .entity_data("jurisdiction", "GB")
//!     .build();
//!
//! let request = GenerationRequest::new(
//!     GenerationOperationType::KycWorkflow,
//!     context
//! );
//!
//! let response = generator.generate_dsl(request).await?;
//! ```
//!
//! ### Auto-Selection with Factory
//!
//! ```rust
//! let factory = GenerationFactory::with_ai_service(templates_dir, ai_service)
//!     .with_default_method(GenerationMethod::Auto);
//!
//! let (generator, selection) = factory.create_auto_generator(&operation_type).await?;
//! println!("Selected generator: {:?}", selection.generator_type);
//!
//! let response = generator.generate_dsl(request).await?;
//! ```
//!
//! ## Error Handling
//!
//! All generation methods return `GenerationResult<T>` which provides comprehensive
//! error information including:
//! - Template not found errors
//! - AI generation failures
//! - Validation errors
//! - Configuration issues
//!
//! ## Performance Characteristics
//!
//! | Method   | Speed | Flexibility | Determinism | Offline |
//! |----------|-------|-------------|-------------|---------|
//! | Template | +++   | +           | +++         | +++     |
//! | AI       | +     | +++         | +           | -       |
//! | Hybrid   | ++    | +++         | ++          | ++      |
//!
//! ## Integration with DSL Manager
//!
//! The generation system integrates seamlessly with the DSL Manager through
//! the orchestration interface, allowing for:
//! - Unified error handling
//! - Consistent logging and metrics
//! - Database integration
//! - Visualization support

pub mod ai;
pub mod context;
pub mod equivalence_tests;
pub mod factory;
pub mod template;
pub mod traits;

// Public re-exports for external API
pub use traits::{
    DslGenerator, GenerationContext, GenerationDebugInfo, GenerationError, GenerationMethod,
    GenerationMethodPreference, GenerationOperationType, GenerationOptions, GenerationRequest,
    GenerationResponse, GenerationResult, GeneratorFactory, GeneratorMetadata, GeneratorType,
};

pub use template::{
    Template, TemplateGenerator, TemplateGeneratorConfig, TemplateRequirements, TemplateVariable,
};

pub use ai::{
    AiDslGenerationService, AiGenerationError, AiGenerationRequest, AiGenerationResponse,
    AiGenerator, AiGeneratorConfig, RagContext, RagContextProvider,
};

pub use context::{
    ContextEnhancementResult, ContextResolver, ContextResolverFactory, ContextValidationRule,
    GenerationContextBuilder,
};

pub use factory::{
    FactoryHealthStatus, FallbackStrategy, GenerationFactory,
    GenerationMethod as FactoryGenerationMethod, GeneratorSelection, HybridGenerator,
};

pub use equivalence_tests::{
    assert_equivalent_dsl, test_template_agent_equivalence, EquivalenceTestConfig,
    EquivalenceTestResult, GenerationComparison,
};

/// Version information for the generation system
pub const GENERATION_VERSION: &str = "1.0.0";

/// Supported DSL version
pub const SUPPORTED_DSL_VERSION: &str = "3.1";

/// Default timeout for generation operations (in milliseconds)
pub const DEFAULT_GENERATION_TIMEOUT_MS: u64 = 30_000;

/// Maximum retry attempts for failed generations
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, GenerationError>;

/// Convenience function to create a generation factory with templates only
pub fn create_template_factory(templates_dir: std::path::PathBuf) -> GenerationFactory {
    GenerationFactory::new(templates_dir).with_default_method(factory::GenerationMethod::Template)
}

/// Convenience function to create a generation factory with AI service
pub fn create_ai_factory(
    templates_dir: std::path::PathBuf,
    ai_service: std::sync::Arc<dyn AiDslGenerationService + Send + Sync>,
) -> GenerationFactory {
    GenerationFactory::with_ai_service(templates_dir, ai_service)
        .with_default_method(factory::GenerationMethod::Hybrid)
}

/// Convenience function to create a generation factory with auto-selection
pub fn create_auto_factory(
    templates_dir: std::path::PathBuf,
    ai_service: Option<std::sync::Arc<dyn AiDslGenerationService + Send + Sync>>,
) -> GenerationFactory {
    match ai_service {
        Some(service) => GenerationFactory::with_ai_service(templates_dir, service)
            .with_default_method(factory::GenerationMethod::Auto),
        None => GenerationFactory::new(templates_dir)
            .with_default_method(factory::GenerationMethod::Template),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_module_exports() {
        // Test that all major types are accessible
        let _template_config = TemplateGeneratorConfig::default();
        let _ai_config = AiGeneratorConfig::default();
        let _context = GenerationContextBuilder::new().build();
        let _options = GenerationOptions::default();
    }

    #[test]
    fn test_convenience_functions() {
        let temp_dir = tempdir().unwrap();
        let templates_dir = temp_dir.path().to_path_buf();

        let template_factory = create_template_factory(templates_dir.clone());
        assert!(template_factory
            .available_generators()
            .contains(&GeneratorType::Template));

        let auto_factory = create_auto_factory(templates_dir, None);
        assert!(auto_factory
            .available_generators()
            .contains(&GeneratorType::Template));
    }

    #[test]
    fn test_constants() {
        assert_eq!(GENERATION_VERSION, "1.0.0");
        assert_eq!(SUPPORTED_DSL_VERSION, "3.1");
        assert!(DEFAULT_GENERATION_TIMEOUT_MS > 0);
        assert!(DEFAULT_MAX_RETRIES > 0);
    }
}
