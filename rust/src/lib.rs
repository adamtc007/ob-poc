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

// Ontology - Entity taxonomy and lifecycle management
pub mod ontology;

// REST API module (when server feature is enabled)
#[cfg(feature = "server")]
pub mod api;

// MCP server module (when mcp feature is enabled)
#[cfg(feature = "mcp")]
pub mod mcp;

// Agentic DSL generation module
pub mod agentic;

// Graph visualization module
#[cfg(feature = "database")]
pub mod graph;

// Navigation module - Nom-based parser for graph navigation commands
#[cfg(feature = "database")]
pub mod navigation;

// Session module - unified session context for REPL + Graph + Viewport
#[cfg(feature = "database")]
pub mod session;

// Workflow orchestration module
#[cfg(feature = "database")]
pub mod workflow;

// Trading profile document types and materialization
pub mod trading_profile;

// Template system for DSL generation
pub mod templates;

// Verification module - adversarial agent model for KYC
pub mod verification;

// Taxonomy module - generic taxonomy pattern for Product/Instrument domains
pub mod taxonomy;

// GLEIF integration - LEI data enrichment and corporate tree traversal
#[cfg(feature = "database")]
pub mod gleif;

// BODS integration - Beneficial Ownership Data Standard for UBO discovery
#[cfg(feature = "database")]
pub mod bods;

// Research macros - LLM + web search for structured discovery with human review
#[cfg(feature = "database")]
pub mod research;

// Database integration re-exports (when database feature is enabled)
#[cfg(feature = "database")]
pub use database::DictionaryDatabaseService;

// Core domain capabilities
pub use domains::{DomainHandler, DomainRegistry, DomainResult};

// Essential error types
pub use error::{DSLError, ParseError};

// DSL v2 types - unified S-expression DSL
pub use dsl_v2::{
    parse_program, parse_single_verb, Argument, AstNode, DslExecutor, ExecutionContext,
    ExecutionResult as DslV2ExecutionResult, Literal, Program, ReturnType, Span, Statement,
    VerbCall,
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
