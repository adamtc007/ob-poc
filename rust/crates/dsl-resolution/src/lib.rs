//! dsl-resolution: Name resolution and cross-atom reference binding.
//!
//! This crate resolves symbolic references in a typed `AtomBag`: `@slot-ref`
//! cross-atom bindings, `pack/atom` qualified name lookups, template parameter
//! substitution and splice expansion, and insertion marker placement.
//!
//! # Scope
//!
//! - Consumes: `dsl_ast::AtomBag`, `dsl_diagnostics::DiagnosticBag`
//! - Produces: validated packs indexed in `PackRegistry` + provenance summary
//!
//! # Entry points
//!
//! - [`resolve`] — resolution pass; validates and indexes decision-pack atoms.
//! - [`validate_bpmn`] — full pipeline (parse → bag → resolve → assemble).
#![deny(unreachable_pub)]

pub mod pack_registry;
pub mod resolve;
pub mod validator;

pub use pack_registry::{DecisionPack, PackParam, PackRegistry};
pub use resolve::resolve;
pub use validator::{validate_bpmn, DiagnosticSummary, ProvenanceSummary, ValidateResponse};
