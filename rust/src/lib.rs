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

// Lint configuration - enforce facade pattern
#![allow(missing_docs)] // TEMPORARY: Silence 1000+ missing docs warnings during pub API cleanup
#![deny(dead_code)]
#![deny(unused_imports)]
#![deny(unused_variables)]
// Facade pattern enforcement
#![deny(clippy::wildcard_imports)]
//! - nom-based parser combinators
//! - EBNF grammar support
//! - Modular, extensible architecture
//!
//! # Quick Start - DSL-First Approach
//!
//! ```rust
//! use ob_poc::dsl::{DslProcessor, DomainContext};
//!
//! // Process DSL with domain awareness - THE CORE CAPABILITY
//! let processor = DslProcessor::new();
//! let context = DomainContext::kyc();
//!
//! let dsl_code = r#"
//! (kyc.start :customer-id "CUST-001")
//! (kyc.verify :status "approved")
//! "#;
//!
//! let result = processor.process_dsl(dsl_code, context).await?;
//! assert!(result.success);
//! ```
//!
//! # Architecture - DSL-Centric Design
//!
//! This is a **DSL-first application** where everything serves DSL processing:
//!
//! - **[`dsl`]** - 🎯 **THE CORE**: Domain-aware DSL processing, validation, operations
//! - [`parser`] - Low-level parsing infrastructure (serves DSL)
//! - [`ai`] - AI clients and services (generate/work with DSL)
//! - [`agents`] - High-level AI automation (manipulate DSL)
//! - [`data_dictionary`] - Attribute definitions (define DSL vocabulary)
//! - [`database`] - Persistence layer (store DSL instances)

// Internal imports for functions no longer in public re-exports
use crate::error::DSLResult;
use crate::grammar::{load_default_grammar, GrammarEngine};
use crate::parser_ast::Program;
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

// ============================================================================
// PUBLIC MODULES - Facade Pattern
// ============================================================================

// Core parsing and AST - main external interface
pub mod ast;
pub mod parser;

// Error handling - external consumers need these
pub mod error;

// Data dictionary and vocabulary - domain experts need these
pub mod data_dictionary;
pub mod vocabulary;

// Database integration (when enabled)
#[cfg(feature = "database")]
pub mod database;
#[cfg(feature = "database")]
pub mod models;

// ============================================================================
// INTERNAL MODULES - Implementation Details (pub(crate))
// ============================================================================

// Internal error handling
#[cfg(feature = "database")]
pub(crate) mod error_enhanced;

// Grammar engine - used internally by parser
pub(crate) mod grammar;

// Parser AST types - used internally by parser
pub(crate) mod parser_ast;

// DSL Manager - V3.1 aligned with proper type consolidation - STRING FORMATTING FIXED
// Legacy DSL managers moved to deprecated/ - using services/document_service instead

// REST API server functionality removed - use SQL integration tests instead

// Mock REST API server for testing without database
// pub mod mock_rest_api; // Removed - dead code

// 🎯 DSL CORE - The primary capability of the entire application
pub mod dsl;

// AI integration - low-level AI clients and services (serves DSL)
pub mod ai;

// Agentic capabilities - high-level intelligent automation (serves DSL)
pub mod agents;

// Supporting DSL infrastructure
pub(crate) mod domains;

// Database services (when enabled)
#[cfg(feature = "database")]
pub(crate) mod execution;
#[cfg(feature = "database")]
pub(crate) mod services;

// DSL Manager - internal coordination (may be promoted to public later)
pub(crate) mod dsl_manager;

// Protobuf and domain visualizations removed - functionality replaced by SQL integration

// Deprecated modules moved to src/deprecated/ (not needed for Phase 1)
// - proto/ - gRPC protobuf modules (for future web services)
// - grpc/ - gRPC service implementations (for future web services)

// ============================================================================
// PRIMARY PUBLIC API - DSL-First Architecture
// DSL is the CORE CAPABILITY - everything else serves DSL processing
// ============================================================================

// 🎯 DSL CORE - The Heart of the Application (Primary Interface)
pub use dsl::{
    detect_domains, normalize_legacy_dsl, validate_dsl, CentralDslEditor, DomainContext,
    DomainRegistry, DslOperation, DslProcessor, DslResult, OperationBuilder, ProcessingResult,
    ValidationReport,
};

// Core parsing infrastructure (serves DSL)
pub use parser::{parse_normalize_and_validate, parse_program};

// Essential error types
pub use error::{DSLError, ParseError};

// Low-level AI integration - clients and core services
pub use ai::{
    openai::OpenAiClient, AiConfig, AiDslRequest, AiDslResponse, AiResponseType, AiResult,
    AiService,
};

// High-level agentic capabilities - intelligent automation facade
#[cfg(feature = "database")]
pub use agents::{
    AgenticDictionaryService, AiDslService, DictionaryServiceConfig, KycCaseRequest,
    UboAnalysisRequest,
};

// Main AST types for complex workflows (advanced users)
pub use ast::{Statement, Workflow};

// Database integration (when enabled)
#[cfg(feature = "database")]
pub use database::{DatabaseConfig, DatabaseManager};
#[cfg(feature = "database")]
// Models re-exports removed due to visibility issues
// pub use models::{...};

// System utilities
pub use system_info as get_system_info;

// REST API components removed - protobuf API is behind the service layer

// Legacy managers removed; consolidated manager is the single entry point

// UI components removed - agentic CRUD is the master interface

/// Version information
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const PKG_NAME: &str = env!("CARGO_PKG_NAME");

/// DSL system configuration
#[derive(Debug, Clone)]
pub(crate) struct DSLConfig {
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
pub(crate) struct DSLContext {
    config: DSLConfig,
    grammar_engine: GrammarEngine,
}

impl DSLContext {
    /// Create a new DSL context with default configuration
    pub fn new() -> Result<Self, DSLError> {
        Self::with_config(DSLConfig::default())
    }

    /// Create a new DSL context with custom configuration
    pub(crate) fn with_config(config: DSLConfig) -> Result<Self, DSLError> {
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
        let coordinator = ParsingCoordinator::new(registry);

        // Parse with domain awareness
        coordinator.parse_dsl(input).await.map_err(|e| {
            DSLError::Parse(ParseError::Internal {
                message: format!("{:?}", e),
            })
        })
    }

    /// Get reference to the grammar engine
    pub(crate) fn grammar_engine(&self) -> &GrammarEngine {
        &self.grammar_engine
    }

    /// Get the current configuration
    pub fn config(&self) -> &DSLConfig {
        &self.config
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
pub(crate) fn parse_dsl(input: &str) -> DSLResult<Program> {
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
pub(crate) struct SystemInfo {
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
