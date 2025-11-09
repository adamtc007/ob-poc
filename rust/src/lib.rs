//! UBO Proof of Concept - Idiomatic Rust DSL System
//!
//! This crate provides a comprehensive, type-safe domain-specific language (DSL)
//! for modeling and analyzing Ultimate Beneficial Ownership structures in
//! financial institutions' KYC workflows.
//!
//! The system is built with idiomatic Rust patterns, featuring:
//! - Strong typing with compile-time guarantees
//! - Zero-copy parsing where possible
//! - Comprehensive error handling with detailed context
//! - nom-based parser combinators
//! - EBNF grammar support
//! - Modular, extensible architecture
//!
//! # Quick Start
//!
//! ```rust
//! use ob_poc::{parse_dsl, GrammarEngine};
//!
//! // Parse a simple DSL program
//! let dsl_code = r#"
//!     (workflow "onboard-customer"
//!         (declare-entity "customer1" "person")
//!         (obtain-document "passport" "government")
//!         (create-edge "customer1" "passport" "evidenced-by"))
//! "#;
//!
//! let program = parse_dsl(dsl_code)?;
//! println!("Parsed {} workflows", program.workflows.len());
//! # Ok::<(), ob_poc::DSLError>(())
//! ```
//!
//! # Architecture
//!
//! The system is organized into several key modules:
//!
//! - [`ast`] - Abstract Syntax Tree definitions with strong typing
//! - [`parser`] - nom-based parser with proper error handling
//! - [`grammar`] - EBNF grammar engine for extensible syntax
//! - [`error`] - Comprehensive error types with context
//! - [`vocabulary`] - Domain vocabulary management
//!
//! # Error Handling
//!
//! All operations return strongly-typed `Result` types with detailed error information:
//!
//! ```rust
//! use ob_poc::{parse_dsl, DSLError};
//!
//! match parse_dsl("invalid syntax") {
//!     Ok(program) => println!("Success: {:?}", program),
//!     Err(DSLError::Parse(parse_err)) => println!("Parse error: {}", parse_err),
//!     Err(DSLError::Grammar(grammar_err)) => println!("Grammar error: {}", grammar_err),
//!     Err(other) => println!("Other error: {}", other),
//! }
//! ```

// Core modules
pub mod ast;
pub mod data_dictionary;
pub mod error;
pub mod grammar;
pub mod parser;
pub mod vocabulary;

// Database and models for domain architecture - temporarily disabled for Phase 1
// pub mod database;
// pub mod models;

// DSL Manager - core create/edit functionality - temporarily disabled for Phase 1
// pub mod dsl_manager;

// Central DSL system with domain context switching
pub mod dsl;

// Domain implementations for centralized DSL editing
pub mod domains;

// New modules for gRPC server and services - temporarily disabled for Phase 1
// pub mod grpc_server;
pub mod execution;
// pub mod services; // Temporarily disabled to focus on execution

// Generated protobuf modules - temporarily disabled for Phase 1
// pub mod proto;

// Domain-specific visualization features (Phase 3) removed during consolidation

// Deprecated modules moved to src/deprecated/ (not needed for Phase 1)
// - proto/ - gRPC protobuf modules (for future web services)
// - grpc/ - gRPC service implementations (for future web services)

// Re-export key types and functions for public API
pub use ast::{Program, PropertyMap, Statement, Value, Workflow};
pub use error::{
    DSLError, DSLResult, ErrorCollector, ErrorSeverity, GrammarError, ParseError, RuntimeError,
    SourceLocation, ValidationError, VocabularyError,
};
pub use grammar::{load_default_grammar, EBNFGrammar, EBNFParser, GrammarEngine};
pub use parser::{
    parse_program, parse_value_standalone as parse_value,
    parse_workflow_standalone as parse_workflow, validate_ast,
};

// Re-export consolidated DSL manager (main interface) - temporarily disabled for Phase 1
// pub use dsl_manager::{
//     ASTVisualization, CbuInfo, DomainVisualizationOptions, DslError as DslManagerError,
//     DslInstance, DslInstanceVersion, DslManager, DslResult, DslStorageKeys, DslTemplate,
//     InstanceStatus, KycCaseCreationResult, OnboardingRequestCreationResult,
//     OnboardingSessionRecord, OperationType, TemplateType, VisualEdge, VisualNode,
//     VisualizationOptions, VisualizationStatistics,
// };

// Re-export central DSL system components
pub use dsl::{
    central_editor::{CentralDslEditor, EditorConfig, EditorStats},
    domain_context::{DomainContext, OperationMetadata, OperationPriority, StateRequirements},
    domain_registry::{DomainHandler, DomainRegistry, DslVocabulary, StateTransition},
    operations::{DslOperation, OperationBuilder},
    DslEditError, DslEditResult,
};

// Re-export domain implementations
pub use domains::{
    available_domains, register_all_domains, KycDomainHandler, OnboardingDomainHandler,
    UboDomainHandler,
};

// Re-export gRPC server components - temporarily disabled for Phase 1
// pub use grpc_server::{
//     start_grpc_services, start_retrieval_service, start_transform_service, GrpcServerConfig,
//     GrpcServerManager,
// };
// pub use services::{DslRetrievalServiceImpl, DslTransformServiceImpl};

// Re-export database and models - temporarily disabled for Phase 1
// pub use database::{
//     DatabaseConfig, DatabaseManager, DslDomainRepository, DslInstanceRepository,
//     PgDslInstanceRepository,
// };
// pub use models::{
//     CompilationStatus, DslDomain, DslVersion, ExecutionPhase, ExecutionStatus, ParsedAst,
// };

// Legacy managers removed; consolidated manager is the single entry point

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");

/// DSL system configuration
#[derive(Debug, Clone)]
pub struct DSLConfig {
    /// Enable strict validation
    pub strict_validation: bool,
    /// Maximum parsing depth to prevent stack overflow
    pub max_depth: usize,
    /// Enable debug mode for verbose error messages
    pub debug_mode: bool,
    /// Custom grammar file path
    pub grammar_file: Option<String>,
}

impl Default for DSLConfig {
    fn default() -> Self {
        Self {
            strict_validation: true,
            max_depth: 1000,
            debug_mode: false,
            grammar_file: None,
        }
    }
}

/// Main DSL system context
#[derive(Debug)]
pub struct DSLContext {
    config: DSLConfig,
    grammar_engine: GrammarEngine,
}

impl DSLContext {
    /// Create a new DSL context with default configuration
    pub fn new() -> Result<Self, DSLError> {
        Self::with_config(DSLConfig::default())
    }

    /// Create a new DSL context with custom configuration
    pub fn with_config(config: DSLConfig) -> Result<Self, DSLError> {
        let grammar_engine = if let Some(ref grammar_file) = config.grammar_file {
            let grammar_source = std::fs::read_to_string(grammar_file).map_err(DSLError::Io)?;
            let mut engine = GrammarEngine::new();
            engine.load_grammar("custom", &grammar_source)?;
            engine
        } else {
            load_default_grammar()?
        };

        Ok(Self {
            config,
            grammar_engine,
        })
    }

    /// Parse DSL code with this context's configuration
    pub fn parse(&self, input: &str) -> DSLResult<Program> {
        let program = parse_program(input).map_err(|e| DSLError::Parse(e.into()))?;

        if self.config.strict_validation {
            validate_ast(&program).map_err(|errors| {
                DSLError::Validation(ValidationError::WorkflowError {
                    message: format!("Validation failed with {} errors", errors.len()),
                })
            })?;
        }

        Ok(program)
    }

    /// Get reference to the grammar engine
    pub fn grammar_engine(&self) -> &GrammarEngine {
        &self.grammar_engine
    }

    /// Get mutable reference to the grammar engine
    pub fn grammar_engine_mut(&mut self) -> &mut GrammarEngine {
        &mut self.grammar_engine
    }

    /// Get the current configuration
    pub fn config(&self) -> &DSLConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: DSLConfig) {
        self.config = config;
    }
}

impl Default for DSLContext {
    fn default() -> Self {
        // Use a panic-free fallback for default construction
        match Self::new() {
            Ok(context) => context,
            Err(_) => {
                // Create minimal fallback context
                Self {
                    config: DSLConfig::default(),
                    grammar_engine: GrammarEngine::new(),
                }
            }
        }
    }
}

/// Convenience function for parsing DSL with default settings
///
/// This is equivalent to:
/// ```rust
/// let context = DSLContext::new()?;
/// context.parse(input)
/// ```
pub fn parse_dsl(input: &str) -> DSLResult<Program> {
    parse_program(input).map_err(|e| DSLError::Parse(e.into()))
}

/// Parse and validate DSL with comprehensive error reporting
pub fn parse_and_validate(input: &str) -> Result<Program, ErrorCollector> {
    let mut collector = ErrorCollector::new();

    let program = match parse_dsl(input) {
        Ok(program) => program,
        Err(error) => {
            collector.add_simple_error(error, SourceLocation::unknown(), ErrorSeverity::Fatal);
            return Err(collector);
        }
    };

    // Validate the parsed program
    if let Err(errors) = validate_ast(&program) {
        for error in errors {
            collector.add_simple_error(error, SourceLocation::unknown(), ErrorSeverity::Error);
        }
    }

    if collector.has_errors() {
        Err(collector)
    } else {
        Ok(program)
    }
}

/// System information and diagnostics
pub fn system_info() -> SystemInfo {
    SystemInfo {
        version: VERSION.to_string(),
        package_name: PKG_NAME.to_string(),
        rust_version: option_env!("RUSTC_VERSION")
            .unwrap_or("unknown")
            .to_string(),
        build_date: option_env!("BUILD_DATE").unwrap_or("unknown").to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub version: String,
    pub package_name: String,
    pub rust_version: String,
    pub build_date: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_info() {
        let info = system_info();
        assert_eq!(info.package_name, "ob-poc");
        assert!(!info.version.is_empty());
    }

    #[test]
    fn test_error_types() {
        // Test that our error types work properly
        let parse_error = DSLError::Parse(ParseError::Incomplete);
        assert!(matches!(parse_error, DSLError::Parse(_)));

        let grammar_error = DSLError::Grammar(GrammarError::RuleNotFound {
            rule: "test".to_string(),
        });
        assert!(matches!(grammar_error, DSLError::Grammar(_)));
    }
}
