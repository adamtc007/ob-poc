//! CBU Model DSL Module
//!
//! This module implements the CBU Model DSL (`DSL.CBU`) - a specification DSL
//! that documents the business model of a CBU including:
//! - Attribute sets (pick-lists, required/optional, grouped)
//! - Stable states and valid transitions
//! - Role/entity requirements
//!
//! The CBU Model DSL is stored as a document type (`DSL.CBU.MODEL`) and is used
//! by the execution layer (Forth + CRUD) for validation and scaffolding.
//!
//! ## Architecture
//!
//! ```text
//! DSL.CBU spec  --> parsed to CbuModel (Rust struct) --> stored as document
//!                         |
//!                         v
//! Execution (Forth + CRUD) loads relevant CbuModel
//!                         |
//!                         v
//! Validates attribute usage, state transitions, role requirements
//!                         |
//!                         v
//! CrudExecutor + domain services perform actual DB operations
//! ```

pub mod ast;
pub mod ebnf;
pub mod parser;

#[cfg(feature = "database")]
pub mod service;

// Re-exports
pub use ast::{
    CbuAttributeGroup, CbuAttributesSpec, CbuModel, CbuRoleSpec, CbuState, CbuStateMachine,
    CbuTransition,
};
pub use ebnf::CBU_MODEL_EBNF;
pub use parser::{CbuModelError, CbuModelParser};

#[cfg(feature = "database")]
pub use service::CbuModelService;
