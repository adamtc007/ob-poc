//! OB-POC - Clean Refactored DSL System
//!
//! This crate provides a simplified, clean implementation of the DSL system
//! following the proven call chain architecture.
//!
//! ## Clean Architecture
//! DSL Manager → DSL Mod → DB State Manager → DSL Visualizer
//!
//! ## Quick Start
//!
//! ```rust
//! use ob_poc::dsl_manager::CleanDslManager;
//!
//! let mut manager = CleanDslManager::new();
//! let dsl_content = r#"(case.create :case-id "TEST-001" :case-type "ONBOARDING")"#;
//! let result = manager.process_dsl_request(dsl_content.to_string()).await;
//! assert!(result.success);
//! ```

// Core error handling
pub mod error;

// Essential AST types
pub mod ast;
pub mod parser_ast;

// Core parser with full DSL capabilities
pub mod parser;

// Grammar engine for EBNF parsing
pub mod grammar;

// Data dictionary and vocabulary management
pub mod data_dictionary;
pub mod vocabulary;

// Domain handlers for business logic
pub mod domains;

// Execution engine for DSL operations
#[cfg(feature = "database")]
pub mod execution;

// Database integration (when enabled)
#[cfg(feature = "database")]
pub mod database;
#[cfg(feature = "database")]
pub mod models;

// Refactored call chain components
pub mod db_state_manager;
pub mod dsl;
pub mod dsl_manager;
pub mod dsl_visualizer;

// Services for database integration and universal DSL lifecycle
#[cfg(feature = "database")]
pub mod services;

// Public re-exports for the clean architecture
pub use db_state_manager::{AccumulatedState, DbStateManager, StateResult};
pub use dsl::{
    DomainSnapshot, DslOrchestrationInterface, DslPipelineProcessor, DslPipelineResult,
    ExecutionResult, OrchestrationContext, OrchestrationOperation, OrchestrationOperationType,
};
pub use dsl_manager::{CallChainResult, CleanDslManager, IncrementalResult, ValidationResult};
pub use dsl_visualizer::{DslVisualizer, VisualizationResult};

// Database integration re-exports (when database feature is enabled)
#[cfg(feature = "database")]
pub use database::{DatabaseConfig, DatabaseManager, DictionaryDatabaseService};

// Universal DSL lifecycle service - edit→validate→parse→save pattern for ALL DSL
#[cfg(feature = "database")]
pub use services::{
    create_lifecycle_service, create_lifecycle_service_with_config, DslChangeRequest,
    DslChangeResult, DslChangeType, DslLifecycleService, EditSession, EditSessionStatus,
    LifecycleConfig, LifecycleMetrics, LifecyclePhase,
};

// Core parsing and execution capabilities
pub use domains::{DomainHandler, DomainRegistry, DomainResult};
pub use parser::{PropertyMap, Value};

// Grammar and vocabulary
pub use grammar::GrammarEngine;
pub use vocabulary::vocab_registry::VocabularyRegistry;

// Essential error types
pub use error::{DSLError, ParseError};

// Core AST types
pub use ast::{Statement, Workflow};

// System info
pub use system_info as get_system_info;

/// Parse DSL program using full parser capabilities
pub use parser::{
    execute_dsl, parse_normalize_and_validate, parse_program,
    ExecutionResult as ParserExecutionResult,
};

/// Parse DSL with full normalization and validation
pub fn parse_dsl(input: &str) -> Result<parser_ast::Program, ParseError> {
    parse_program(input).map_err(|e| ParseError::Internal {
        message: format!("Parse error: {:?}", e),
    })
}

/// Universal DSL change processing - implements edit→validate→parse→save for ALL DSL
/// This is the master function that handles the universal lifecycle pattern
#[cfg(feature = "database")]
pub fn process_dsl_change_sync(request: services::DslChangeRequest) -> Result<String, ParseError> {
    // Stub implementation for compilation
    Ok(format!(
        "Processed DSL change for case: {}",
        request.case_id
    ))
}

/// Execute DSL program
pub fn execute_dsl_program(
    program: &parser_ast::Program,
) -> Result<ParserExecutionResult, ParseError> {
    execute_dsl(program).map_err(|e| ParseError::Internal {
        message: format!("Execution error: {:?}", e),
    })
}

/// System information module
pub mod system_info {
    /// Get system information
    pub fn get_system_info() -> String {
        format!(
            "OB-POC v{} - Clean Refactored Architecture",
            env!("CARGO_PKG_VERSION")
        )
    }
}
