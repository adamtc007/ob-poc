//! Models module for DSL domain architecture
//!
//! This module contains all the data structures and types used to represent
//! DSL domains, versions, AST storage, and execution tracking in the database.

pub mod domain_models;

// Re-export commonly used types for convenience
pub use domain_models::{
    CompilationStatus, DomainStatistics, DslDomain, DslExecutionLog, DslExecutionSummary,
    DslLatestVersion, DslVersion, ExecutionPhase, ExecutionStatus, NewDslVersion, NewParsedAst,
    ParsedAst, VersionHistoryEntry,
};
