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
//! ```rust,no_run
//! use ob_poc::dsl_manager::CleanDslManager;
//!
//! let mut manager = CleanDslManager::new();
//! let dsl_content = r#"(case.create :case-id "TEST-001" :case-type "ONBOARDING")"#;
//! let result = manager.process_dsl_request(dsl_content.to_string());
//! assert!(result.success);
//! ```

// Core error handling
pub mod error;

// Essential AST types
pub mod ast;

// Core parser with full DSL capabilities (includes AST types in parser::ast)
pub mod parser;

// Data dictionary and vocabulary management
pub mod data_dictionary;
pub mod vocabulary;

// Macro system for reducing boilerplate
#[macro_use]
pub mod macros;

// Domain handlers for business logic
pub mod domains;

// Execution engine for DSL operations
// #[cfg(feature = "database")]
// pub mod execution;

// Database integration (when enabled)
#[cfg(feature = "database")]
pub mod database;
#[cfg(feature = "database")]
pub mod models;

// Taxonomy system for Product-Service-Resource management
#[cfg(feature = "database")]
pub mod taxonomy;

// Refactored call chain components
pub mod db_state_manager;
pub mod dsl;
pub mod dsl_manager;
pub mod dsl_visualizer;

// Services for database integration and universal DSL lifecycle
#[cfg(feature = "database")]
pub mod services;

// Forth-style DSL execution engine
pub mod forth_engine;

// CBU Model DSL - specification DSL for CBU business models
pub mod cbu_model_dsl;

// CBU CRUD Template - template generation and instantiation
#[cfg(feature = "database")]
pub mod cbu_crud_template;

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

// Services module (when database feature is enabled)
// Note: Most types are already re-exported from dsl_manager above

// Core parsing and execution capabilities
pub use domains::{DomainHandler, DomainRegistry, DomainResult};
pub use parser::{PropertyMap, Value};

// Vocabulary
pub use vocabulary::vocab_registry::VocabularyRegistry;

// Essential error types
pub use error::{DSLError, ParseError};

// Core AST types (EntityLabel, EdgeType for graph operations)
pub use ast::{EdgeType, EntityLabel};

// CBU Model DSL types
#[cfg(feature = "database")]
pub use cbu_model_dsl::CbuModelService;
pub use cbu_model_dsl::{CbuModel, CbuModelError, CbuModelParser};

// CBU CRUD Template types
#[cfg(feature = "database")]
pub use cbu_crud_template::{CbuCrudTemplate, CbuCrudTemplateService, DslDocSource};

// System info
pub use system_info as get_system_info;

/// Parse DSL program using full parser capabilities
pub use parser::{
    execute_dsl, parse_normalize_and_validate, parse_program,
    ExecutionResult as ParserExecutionResult,
};

/// Parse DSL with full normalization and validation
pub fn parse_dsl(input: &str) -> Result<parser::Program, ParseError> {
    parse_program(input).map_err(|e| ParseError::Internal {
        message: format!("Parse error: {:?}", e),
    })
}

/// Execute DSL program
pub fn execute_dsl_program(program: &parser::Program) -> Result<ParserExecutionResult, ParseError> {
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
