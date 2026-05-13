//! Trading Profile Module
//!
//! This module provides:
//! - Types for trading profile documents (JSONB storage)
//! - Document-level operations for incremental construction
//! - AST builder for tree-based document construction (new architecture)
//! - AST database operations (load, save, apply_and_save)
//! - Import functionality for YAML seed files
//! - Entity resolution (LEI, BIC, NAME → UUID)
//! - Validation (SSI refs, booking rule refs, go-live readiness)
//!
//! ## Architecture
//!
//! The document IS the AST - a tree structure stored as JSONB. DSL verbs build
//! the AST incrementally using `ast_db::apply_and_save()`. No materialization
//! to operational tables is needed - the document directly serves the UI.

pub mod ast_builder;
#[cfg(feature = "database")]
pub mod ast_db;
pub mod document_ops;
pub mod resolve;
pub mod types;
pub mod validate;

pub use ast_builder::{apply_op, AstBuildError, AstBuildResult};
#[cfg(feature = "database")]
pub use ast_db::{apply_and_save, load_document, ApplyError, AstDbError};
pub use document_ops::*;
pub use resolve::{resolve_entity_ref, ResolveError};
pub use types::*;
pub use validate::{validate_csa_ssi_refs, validate_document, ValidationError};
