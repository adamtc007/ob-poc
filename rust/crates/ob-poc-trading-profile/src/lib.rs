//! ob-poc-trading-profile — Trading Profile AST and resolver shapes.
//!
//! Relocated from `ob_poc_domain::trading_profile` by ob-poc-domain split
//! v1 Slice B4 (2026-05-14). Largest single business capability in the
//! workspace at ~5,600 LOC.
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
#![deny(unreachable_pub)]

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
pub use resolve::{resolve_entity_ref, ResolveError};
// Allowlist re-export of types consumed externally (mostly through
// `crate::trading_profile::*` paths in `src/domain_ops/trading_profile.rs`).
// Other types are still reachable as `crate::trading_profile::types::*`.
pub use types::{
    BookingRule, EntityRefType, InvestmentManagerMandate, IsdaAgreementConfig,
    MaterializationResult, StandingInstruction, TradingProfileDocument, TradingProfileImport,
    Universe,
};
pub use validate::{validate_csa_ssi_refs, validate_document, ValidationError};
// `document_ops::*` glob removed — consumers reach helpers via the
// `crate::trading_profile::document_ops::*` path directly.

#[cfg(test)]
mod integration_tests;
