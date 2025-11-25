//! OB-POC - Clean Refactored DSL System
//!
//! This crate provides a simplified, clean implementation of the DSL system
//! following the proven call chain architecture.
//!
//! ## Clean Architecture
//! All DSL operations flow through the Forth engine:
//! DSL Source -> Forth Parser -> Expr AST -> Compile -> VM Execute
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

// Data dictionary
pub mod data_dictionary;
// vocabulary module removed - vocabulary now lives in forth_engine::vocab_registry

// Domain handlers for business logic
pub mod domains;

// Database integration (when enabled)
#[cfg(feature = "database")]
pub mod database;
#[cfg(feature = "database")]
pub mod models;

// Refactored call chain components
pub mod db_state_manager;
pub mod dsl;
pub mod dsl_manager;
pub mod dsl_source;
pub mod dsl_visualizer;

// Services for database integration and universal DSL lifecycle
#[cfg(feature = "database")]
pub mod services;

// Forth-style DSL execution engine - THE single DSL processing path
pub mod forth_engine;

// CBU Model DSL - specification DSL for CBU business models
pub mod cbu_model_dsl;

// CBU CRUD Template - template generation and instantiation
#[cfg(feature = "database")]
pub mod cbu_crud_template;

// DSL Test Harness - integration testing for onboarding DSL pipeline
#[cfg(feature = "database")]
pub mod dsl_test_harness;

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

// Core domain capabilities
pub use domains::{DomainHandler, DomainRegistry, DomainResult};

// Vocabulary - now in forth_engine::vocab_registry
pub use forth_engine::vocab_registry::create_standard_runtime;

// Essential error types
pub use error::{DSLError, ParseError};

// Core AST types (EntityLabel, EdgeType for graph operations)
pub use ast::{EdgeType, EntityLabel};

// Forth engine types - the canonical DSL interface
pub use forth_engine::ast::{DslParser, DslSheet, Expr};
pub use forth_engine::parser_nom::NomDslParser;

// CBU Model DSL types
#[cfg(feature = "database")]
pub use cbu_model_dsl::CbuModelService;
pub use cbu_model_dsl::{CbuModel, CbuModelError};
pub use forth_engine::cbu_model_parser::CbuModelParser;

// CBU CRUD Template types
#[cfg(feature = "database")]
pub use cbu_crud_template::{CbuCrudTemplate, CbuCrudTemplateService, DslDocSource};

// System info
pub use system_info as get_system_info;

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
