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
//! use ob_poc::parser::parse_program;
//!
//! // Parse a simple DSL program
//! let dsl_code = r#"
//! (kyc.start :customer-id "CUST-001")
//! (kyc.verify :status "approved")
//! "#;
//!
//! let program = parse_program(dsl_code).unwrap();
//! assert_eq!(program.len(), 2);
//! ```
//!
//! # Architecture
//!
//! The system is organized into several key modules:
//!
//! - [`ast`] - Abstract Syntax Tree definitions with strong typing
#![allow(dead_code, unused_imports)] // Temporarily allow dead code and unused imports during refactoring

// New v3.0 AST definitions (temporary, to be moved to ast.rs once accessible)
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Literal {
    String(String),
    Number(f64),
    Boolean(bool),
    Date(String), // ISO 8601 string, e.g., "2025-11-10T10:30:00Z"
    Uuid(String), // UUID string, e.g., "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    pub parts: Vec<String>, // e.g., ["customer", "id"] for :customer.id
}

impl Key {
    pub fn new(s: &str) -> Self {
        Self {
            parts: s.split('.').map(|p| p.to_string()).collect(),
        }
    }

    pub fn as_str(&self) -> String {
        self.parts.join(".")
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Value {
    Literal(Literal),
    Identifier(String), // For unquoted symbols
    List(Vec<Value>),
    Map(HashMap<Key, Value>),
    AttrRef(String), // UUID string, e.g., "@attr{uuid-001}"
}

// Helper for property maps, for consistency with old PropertyMap, but uses new Key/Value
pub type PropertyMap = HashMap<Key, Value>;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct VerbForm {
    pub verb: String,
    pub pairs: PropertyMap, // Using PropertyMap for key-value pairs
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Form {
    Verb(VerbForm),
    Comment(String),
}

// Program is now a sequence of forms (workflow replaced by a specific verb form)
pub type Program = Vec<Form>;
// - [`parser`] - nom-based parser with proper error handling
// - [`grammar`] - EBNF grammar engine for extensible syntax
// - [`error`] - Comprehensive error types with context
// - [`vocabulary`] - Domain vocabulary management
//
// # Error Handling
//
// All operations return strongly-typed `Result` types with detailed error information:
//
// ```rust
// use ob_poc::{parse_dsl, DSLError};
//
// match parse_dsl("invalid syntax") {
//     Ok(program) => assert!(false, "Parsing 'invalid syntax' should fail, but got {:?}", program),
//     Err(DSLError::Parse(parse_err)) => assert!(true, "Caught expected parse error: {}", parse_err),
//     Err(DSLError::Grammar(grammar_err)) => assert!(false, "Caught unexpected grammar error: {}", grammar_err),
//     Err(other) => assert!(false, "Caught unexpected error: {}", other),
// }
// # Ok::<(), ob_poc::DSLError>(())
// ```

// Core modules
pub mod ast;
pub mod data_dictionary;
pub mod error;
#[cfg(feature = "database")]
pub mod error_enhanced;
pub mod grammar;
pub mod parser;
pub mod vocabulary;

// Database and models for domain architecture
#[cfg(feature = "database")]
pub mod database;
// Models for database entities
#[cfg(feature = "database")]
pub mod models;

// DSL Manager - V3.1 aligned with proper type consolidation - STRING FORMATTING FIXED
#[cfg(feature = "database")]
pub mod dsl_manager;
#[cfg(feature = "database")]
pub mod dsl_manager_enhanced;

// REST API server for visualizer
#[cfg(feature = "rest-api")]
pub mod rest_api;

// Mock REST API server for testing without database
#[cfg(any(feature = "rest-api", feature = "mock-api"))]
pub mod mock_rest_api;

// Central DSL system with domain context switching
pub mod dsl;

// Domain implementations for centralized DSL editing
pub mod domains;

// New modules for gRPC server and services - DISABLED UNTIL DSL+DB COMPLETE
// #[cfg(feature = "database")]
// pub mod execution;
// #[cfg(feature = "database")]
// pub mod grpc_server;

// DSL Visualizer module (egui-based desktop application)
#[cfg(feature = "visualizer")]
pub mod visualizer;

// UI module for egui components and state management
#[cfg(feature = "visualizer")]
pub mod ui;

// AI agents for DSL generation, transformation, and validation (v3.1 compatible)
pub mod agents;

// AI integration module for external AI services (Gemini, OpenAI, etc.)
pub mod ai;
// #[cfg(feature = "database")]
// pub mod services;

// Generated protobuf modules - DISABLED UNTIL DSL+DB COMPLETE
// #[cfg(feature = "database")]
// pub mod proto;

// Domain-specific visualization features (Phase 3) removed during consolidation

// Deprecated modules moved to src/deprecated/ (not needed for Phase 1)
// - proto/ - gRPC protobuf modules (for future web services)
// - grpc/ - gRPC service implementations (for future web services)

// Re-export key types and functions for public API
// pub use ast::{Program, PropertyMap, Statement, Value, Workflow}; // Old AST re-exports commented out
pub use error::{
    DSLError, DSLResult, ErrorCollector, ErrorSeverity, GrammarError, ParseError, RuntimeError,
    SourceLocation, ValidationError, VocabularyError,
};
pub use grammar::{load_default_grammar, EBNFGrammar, EBNFParser, GrammarEngine};
pub use parser::parse_program;

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
// pub use services::{DslRetrievalServiceImpl, DslTransformServiceImpl};\n
// Re-export database and models
#[cfg(feature = "database")]
pub use database::{
    DatabaseConfig, DatabaseManager, DslDomainRepository, DslInstanceRepository,
    PgDslInstanceRepository,
};
#[cfg(feature = "database")]
pub use models::{
    CompilationStatus, DslDomain, DslVersion, ExecutionPhase, ExecutionStatus, ParsedAst,
};

// Re-export REST API components
#[cfg(feature = "rest-api")]
pub use rest_api::{RestApiConfig, RestApiServer};

// Re-export Mock REST API components
#[cfg(any(feature = "rest-api", feature = "mock-api"))]
pub use mock_rest_api::{MockRestApiConfig, MockRestApiServer};

// Legacy managers removed; consolidated manager is the single entry point

// Re-export UI components (for egui visualizer)
#[cfg(feature = "visualizer")]
pub use ui::{
    create_global_state_manager, AstNodeData, CbuData, DslContext, DslEntry, GlobalDslState,
    GlobalDslStateManager, OperationStatus, RefreshData, RefreshRequest, ViewportId,
    ViewportManager, ViewportRefreshError, ViewportRefreshHandler,
};

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

    /// Parse DSL code with this context's configuration (legacy method)
    pub fn parse(&self, input: &str) -> DSLResult<Program> {
        let program = parse_program(input).map_err(|e| DSLError::Parse(e.into()))?;
        // Validation will occur at a later stage as the AST structure has changed.
        Ok(program)
    }

    /// Parse DSL code with domain-aware routing (new v3.0 method)
    pub async fn parse_with_domains(
        &self,
        input: &str,
    ) -> DSLResult<crate::dsl::parsing_coordinator::ParseResult> {
        use crate::domains::{KycDomainHandler, OnboardingDomainHandler};
        use crate::dsl::domain_registry::DomainRegistry;
        use crate::dsl::parsing_coordinator::ParsingCoordinator;

        // Create domain registry with all available domains
        let mut registry = DomainRegistry::new();

        // Register all available domains
        if registry
            .register_domain(Box::new(KycDomainHandler::new()))
            .is_err()
        {
            log::warn!("Failed to register KYC domain");
        }
        if registry
            .register_domain(Box::new(OnboardingDomainHandler::new()))
            .is_err()
        {
            log::warn!("Failed to register Onboarding domain");
        }

        // Create parsing coordinator
        let mut coordinator = ParsingCoordinator::new(registry);

        // Parse with domain awareness
        coordinator.parse_dsl(input).await.map_err(|e| {
            DSLError::Parse(ParseError::Internal {
                message: format!("{:?}", e),
            })
        })
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

/// Convenience function for parsing DSL with default settings (legacy)
///
/// This is equivalent to:
/// ```rust
/// use ob_poc::{parse_dsl, Program, DSLError};
/// let dsl_code = "(entity :id \"test\" :label \"Company\")";
/// let program = parse_dsl(dsl_code)?;
/// assert_eq!(program.len(), 1);
/// # Ok::<(), ob_poc::DSLError>(())
/// ```
#[doc(hidden)]
pub fn parse_dsl(input: &str) -> DSLResult<Program> {
    parse_program(input).map_err(|e| DSLError::Parse(e.into()))
}

/// Domain-aware DSL parsing with v3.0 support
///
/// This function automatically detects the domain from DSL verbs and routes
/// to appropriate domain handlers for enhanced validation and processing.
///
/// ```rust
/// use ob_poc::{parse_dsl_v3, DSLError};
///
/// async fn example() -> Result<(), DSLError> {
///     let dsl_code = "(kyc.verify :customer-id \"person-001\" :method \"enhanced_dd\")";
///     let result = parse_dsl_v3(dsl_code).await?;
///     assert_eq!(result.program.len(), 1);
///     assert_eq!(result.handler_used, Some("kyc".to_string()));
///     Ok(())
/// }
/// ```
pub async fn parse_dsl_v3(input: &str) -> DSLResult<crate::dsl::parsing_coordinator::ParseResult> {
    let context = DSLContext::default();
    context.parse_with_domains(input).await
}

/// Comprehensive test for v3.0 domain-aware DSL parsing
#[cfg(test)]
pub async fn test_v3_dsl_comprehensive() -> DSLResult<()> {
    use crate::dsl::parsing_coordinator::ParseResult;

    // Test 1: KYC domain detection and parsing
    let kyc_dsl = r#"
        ;; KYC verification example
        (kyc.verify
          :customer-id "person-john-smith"
          :method "enhanced_due_diligence"
          :verified-at "2025-11-10T10:00:00Z")

        (kyc.screen_sanctions
          :target "person-john-smith"
          :databases ["OFAC", "UN", "EU"]
          :status "CLEAR")
    "#;

    let result: ParseResult = parse_dsl_v3(kyc_dsl).await?;
    assert_eq!(result.program.len(), 2); // 2 forms (excluding comments)
    assert_eq!(result.handler_used, Some("kyc".to_string()));

    // Test 2: Onboarding domain with case management
    let onboarding_dsl = r#"
        (case.create
          :id "CBU-ZENITH-001"
          :nature-purpose "Hedge fund investor onboarding")

        (case.update
          :id "CBU-ZENITH-001"
          :add-products ["CUSTODY", "FUND_ACCOUNTING"])
    "#;

    let result = parse_dsl_v3(onboarding_dsl).await?;
    assert_eq!(result.program.len(), 2);
    assert_eq!(result.handler_used, Some("onboarding".to_string()));

    // Test 3: UBO domain with entity and edge definitions
    let ubo_dsl = r#"
        (entity
          :id "company-zenith-spv-001"
          :label "Company"
          :props {
            :legal-name "Zenith Capital Partners LP"
            :registration-number "KY-123456"
            :jurisdiction "KY"
          })

        (edge
          :from "person-john-smith"
          :to "company-zenith-spv-001"
          :type "HAS_OWNERSHIP"
          :props {
            :percent 45.0
            :share-class "Class A Units"
          }
          :evidence ["doc-cayman-registry-001"])

        (ubo.outcome
          :target "company-zenith-spv-001"
          :at "2025-11-10T10:30:00Z"
          :threshold 25.0
          :ubos [{
            :entity "person-john-smith"
            :effective-percent 45.0
            :prongs {:ownership 45.0, :voting 45.0}
          }])
    "#;

    let result = parse_dsl_v3(ubo_dsl).await?;
    assert_eq!(result.program.len(), 3);
    assert_eq!(result.handler_used, Some("ubo".to_string()));

    // Test 4: Mixed domain detection (should pick most frequent)
    let mixed_dsl = r#"
        (kyc.verify :customer-id "test" :method "basic")
        (case.create :id "test-case")
        (kyc.assess_risk :target "test" :score 50.0)
    "#;

    let result = parse_dsl_v3(mixed_dsl).await?;
    assert_eq!(result.program.len(), 3);
    assert_eq!(result.handler_used, Some("kyc".to_string())); // KYC wins (2 vs 1 verbs)

    // Test 5: Complex nested structures
    let complex_dsl = r#"
        (define-kyc-investigation
          :id "zenith-capital-ubo-discovery"
          :target-entity "company-zenith-spv-001"
          :jurisdiction "KY"
          :ubo-threshold 25.0

          (entity
            :id "company-zenith-spv-001"
            :label "Company"
            :props {
              :legal-name "Zenith Capital Partners LP"
              :entity-type "Limited Partnership"
            })

          (kyc.collect_document
            :target "company-zenith-spv-001"
            :doc-type "certificate_incorporation"
            :status "VERIFIED"))
    "#;

    let result = parse_dsl_v3(complex_dsl).await?;
    assert!(result.program.len() >= 1);
    assert_eq!(result.handler_used, Some("kyc".to_string()));

    println!("✅ All v3.0 DSL domain-aware parsing tests passed!");
    println!("   - KYC domain: ✓");
    println!("   - Onboarding domain: ✓");
    println!("   - UBO domain: ✓");
    println!("   - Mixed domain detection: ✓");
    println!("   - Complex nested structures: ✓");

    Ok(())
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

    // Old AST validation is removed. Semantic validation will be done by domain handlers.
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
