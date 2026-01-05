//! Trading Profile Module
//!
//! This module provides:
//! - Types for trading profile documents (JSONB storage)
//! - Document-level operations for incremental construction
//! - Import functionality for YAML seed files
//! - Materialization to operational tables (cbu_ssi, ssi_booking_rules, etc.)
//! - Sync between document and operational tables
//! - Entity resolution (LEI, BIC, NAME → UUID)
//! - Validation (SSI refs, booking rule refs, go-live readiness)

pub mod document_ops;
pub mod resolve;
pub mod types;
pub mod validate;

pub use document_ops::*;
pub use resolve::{resolve_entity_ref, ResolveError};
pub use types::*;
pub use validate::{validate_csa_ssi_refs, validate_document, ValidationError};
