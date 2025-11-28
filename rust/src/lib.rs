//! OB-POC - DSL v2 System
//!
//! This crate provides a unified S-expression DSL system with data-driven execution.
//!
//! ## Architecture
//! All DSL operations flow through dsl_v2:
//! DSL Source -> Parser (Nom) -> AST -> DslExecutor -> Database
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ob_poc::dsl_v2::{parse_program, DslExecutor, ExecutionContext};
//!
//! let dsl = r#"(cbu.create :name "Test Fund" :jurisdiction "LU")"#;
//! let program = parse_program(dsl).unwrap();
//! // Execute with DslExecutor
//! ```

// Core error handling
pub mod error;

// Essential AST types
pub mod ast;

// Data dictionary
pub mod data_dictionary;

// Domain handlers for business logic
pub mod domains;

// Database integration (when enabled)
#[cfg(feature = "database")]
pub mod database;
#[cfg(feature = "database")]
pub mod models;

// Services for database integration
#[cfg(feature = "database")]
pub mod services;

// DSL v2 - Unified S-expression DSL with data-driven execution
pub mod dsl_v2;

// REST API module (when server feature is enabled)
#[cfg(feature = "server")]
pub mod api;

// Template system for structured DSL generation
#[cfg(feature = "server")]
pub mod templates;

// Database integration re-exports (when database feature is enabled)
#[cfg(feature = "database")]
pub use database::DictionaryDatabaseService;

// Core domain capabilities
pub use domains::{DomainHandler, DomainRegistry, DomainResult};

// Essential error types
pub use error::{DSLError, ParseError};

// Core AST types (EntityLabel, EdgeType for graph operations)
pub use ast::{EdgeType, EntityLabel};

// DSL v2 types - unified S-expression DSL
pub use dsl_v2::{
    domains, find_verb, get_table_mappings, parse_program, parse_single_verb, resolve_column,
    verb_count, verbs_for_domain, Argument, Behavior, ColumnMapping, DbType, DslExecutor,
    ExecutionContext, ExecutionResult as DslV2ExecutionResult, Key, Program, ReturnType, Span,
    Statement, TableMappings, Value, VerbCall, VerbDef,
};

// System info
pub use system_info as get_system_info;

/// System information module
pub mod system_info {
    /// Get system information
    pub fn get_system_info() -> String {
        format!(
            "OB-POC v{} - DSL v2 Architecture",
            env!("CARGO_PKG_VERSION")
        )
    }
}
