//! CBU Model DSL Module
//!
//! This module implements the CBU Model DSL - a specification DSL
//! that documents the business model of a CBU including:
//! - Attribute sets (pick-lists, required/optional, grouped)
//! - Stable states and valid transitions
//! - Role/entity requirements
//!
//! The CBU Model DSL is parsed by the Forth engine and stored as a document.

pub mod ast;
pub mod ebnf;

#[cfg(feature = "database")]
pub mod service;

// Re-exports
pub use ast::{
    CbuAttributeGroup, CbuAttributesSpec, CbuModel, CbuModelError, CbuRoleSpec, CbuState,
    CbuStateMachine, CbuTransition,
};
pub use ebnf::CBU_MODEL_EBNF;

#[cfg(feature = "database")]
pub use service::CbuModelService;
