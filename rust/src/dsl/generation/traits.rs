//! DSL Generation Traits
//!
//! This module defines the core traits and types for DSL generation,
//! supporting both template-based and AI-based generation methods.

use crate::dsl::orchestration_interface::OrchestrationContext;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Result type for DSL generation operations
pub type GenerationResult<T> = Result<T, GenerationError>;

/// DSL generation error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum GenerationError {
    #[error("Template not found: {template_id}")]
    TemplateNotFound { template_id: String },

    #[error("AI generation failed: {reason}")]
    AiGenerationFailed { reason: String },

    #[error("Template rendering failed: {message}")]
    TemplateRenderError { message: String },

    #[error("Validation failed: {errors:?}")]
    ValidationError { errors: Vec<String> },

    #[error("Missing required parameter: {parameter}")]
    MissingParameter { parameter: String },

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error("I/O error: {message}")]
    IoError { message: String },

    #[error("Service unavailable: {service}")]
    ServiceUnavailable { service: String },
}

/// DSL generation request containing all necessary context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    /// Unique identifier for this generation request
    pub request_id: Uuid,

    /// Type of DSL operation to generate
    pub operation_type: GenerationOperationType,

    /// Template variables and context data
    pub context: GenerationContext,

    /// Generation method preference
    pub method_preference: Option<GenerationMethodPreference>,

    /// Processing options
    pub options: GenerationOptions,

    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Types of DSL operations that can be generated
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GenerationOperationType {
    CreateCbu,
    RegisterEntity,
    CalculateUbo,
    UpdateDsl,
    KycWorkflow,
    ComplianceCheck,
    DocumentCatalog,
    IsdaTrade,
    Custom { operation_name: String },
}

/// Context data for DSL generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationContext {
    /// Client Business Unit ID
    pub cbu_id: Option<String>,

    /// Case ID for ongoing operations
    pub case_id: Option<String>,

    /// Entity information
    pub entity_data: HashMap<String, String>,

    /// Business domain context
    pub domain: Option<String>,

    /// User instruction (for AI generation)
    pub instruction: Option<String>,

    /// Template variables (for template generation)
    pub template_variables: HashMap<String, String>,

    /// Orchestration context
    pub orchestration_context: Option<OrchestrationContext>,
}

/// Generation method preference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GenerationMethodPreference {
    /// Prefer template-based generation
    Template,
    /// Prefer AI-based generation
    Ai,
    /// Use the fastest available method
    Fastest,
    /// Use the most accurate method
    MostAccurate,
}

/// Options for DSL generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationOptions {
    /// Enable strict validation of generated DSL
    pub strict_validation: bool,

    /// Include debug information in output
    pub include_debug_info: bool,

    /// Maximum generation time in milliseconds
    pub timeout_ms: u64,

    /// Enable caching of generation results
    pub enable_caching: bool,

    /// Format output for readability
    pub pretty_format: bool,

    /// Include confidence scores (for AI generation)
    pub include_confidence: bool,
}

/// Result of DSL generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResponse {
    /// Whether generation was successful
    pub success: bool,

    /// Generated DSL content
    pub dsl_content: String,

    /// Method used for generation
    pub method_used: GenerationMethod,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,

    /// Confidence score (0.0 to 1.0, if available)
    pub confidence_score: Option<f64>,

    /// Any errors encountered
    pub errors: Vec<String>,

    /// Any warnings generated
    pub warnings: Vec<String>,

    /// Debug information (if requested)
    pub debug_info: Option<GenerationDebugInfo>,

    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Debug information for generation process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationDebugInfo {
    /// Template used (if template generation)
    pub template_id: Option<String>,

    /// AI model used (if AI generation)
    pub ai_model: Option<String>,

    /// Variables resolved during generation
    pub resolved_variables: HashMap<String, String>,

    /// Generation steps taken
    pub generation_steps: Vec<String>,

    /// Performance metrics
    pub performance_metrics: HashMap<String, f64>,
}

/// Generation method actually used
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GenerationMethod {
    Template {
        template_id: String,
    },
    Ai {
        model: String,
    },
    Fallback {
        primary: Box<GenerationMethod>,
        fallback: Box<GenerationMethod>,
    },
}

/// Generator capabilities metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorMetadata {
    /// Name of the generator
    pub name: String,

    /// Version of the generator
    pub version: String,

    /// Supported operation types
    pub supported_operations: Vec<GenerationOperationType>,

    /// Average confidence score
    pub average_confidence: f64,

    /// Average processing time in milliseconds
    pub average_processing_time_ms: u64,

    /// Whether the generator is currently available
    pub available: bool,

    /// Additional capabilities
    pub capabilities: HashMap<String, String>,
}

/// Core trait for DSL generators
#[async_trait]
pub trait DslGenerator: Send + Sync {
    /// Generate DSL content from a request
    async fn generate_dsl(
        &self,
        request: GenerationRequest,
    ) -> GenerationResult<GenerationResponse>;

    /// Check if this generator can handle a specific operation type
    fn can_handle(&self, operation_type: &GenerationOperationType) -> bool;

    /// Get metadata about this generator's capabilities
    fn metadata(&self) -> GeneratorMetadata;

    /// Health check for the generator
    async fn health_check(&self) -> GenerationResult<bool>;

    /// Validate a generation request before processing
    fn validate_request(&self, request: &GenerationRequest) -> GenerationResult<()>;
}

/// Factory trait for creating generators
#[async_trait]
pub trait GeneratorFactory: Send + Sync {
    /// Create a new generator instance
    async fn create_generator(
        &self,
        generator_type: GeneratorType,
    ) -> GenerationResult<Box<dyn DslGenerator>>;

    /// List available generator types
    fn available_generators(&self) -> Vec<GeneratorType>;

    /// Get the recommended generator for an operation type
    fn recommend_generator(
        &self,
        operation_type: &GenerationOperationType,
    ) -> Option<GeneratorType>;
}

/// Types of generators available
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GeneratorType {
    Template,
    Ai,
    Hybrid,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            strict_validation: true,
            include_debug_info: false,
            timeout_ms: 30_000, // 30 seconds
            enable_caching: true,
            pretty_format: true,
            include_confidence: false,
        }
    }
}

impl Default for GenerationContext {
    fn default() -> Self {
        Self {
            cbu_id: None,
            case_id: None,
            entity_data: HashMap::new(),
            domain: None,
            instruction: None,
            template_variables: HashMap::new(),
            orchestration_context: None,
        }
    }
}

impl GenerationRequest {
    /// Create a new generation request
    pub fn new(operation_type: GenerationOperationType, context: GenerationContext) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            operation_type,
            context,
            method_preference: None,
            options: GenerationOptions::default(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the request
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Set method preference
    pub fn with_method_preference(mut self, preference: GenerationMethodPreference) -> Self {
        self.method_preference = Some(preference);
        self
    }

    /// Set options
    pub fn with_options(mut self, options: GenerationOptions) -> Self {
        self.options = options;
        self
    }
}

impl std::fmt::Display for GenerationOperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateCbu => write!(f, "create_cbu"),
            Self::RegisterEntity => write!(f, "register_entity"),
            Self::CalculateUbo => write!(f, "calculate_ubo"),
            Self::UpdateDsl => write!(f, "update_dsl"),
            Self::KycWorkflow => write!(f, "kyc_workflow"),
            Self::ComplianceCheck => write!(f, "compliance_check"),
            Self::DocumentCatalog => write!(f, "document_catalog"),
            Self::IsdaTrade => write!(f, "isda_trade"),
            Self::Custom { operation_name } => write!(f, "custom_{}", operation_name),
        }
    }
}

impl std::fmt::Display for GenerationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Template { template_id } => write!(f, "template({})", template_id),
            Self::Ai { model } => write!(f, "ai({})", model),
            Self::Fallback { primary, fallback } => {
                write!(f, "fallback({} -> {})", primary, fallback)
            }
        }
    }
}

impl std::fmt::Display for GeneratorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Template => write!(f, "template"),
            Self::Ai => write!(f, "ai"),
            Self::Hybrid => write!(f, "hybrid"),
        }
    }
}
