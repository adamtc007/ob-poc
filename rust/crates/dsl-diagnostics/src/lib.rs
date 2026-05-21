//! dsl-diagnostics: Source-attributed diagnostics for the unified DSL v0.1.
//!
//! Provides `Span`, `Diagnostic`, `DiagnosticSeverity`, `DiagnosticBag`, and
//! well-known diagnostic code constants used across the parsing and lowering
//! pipeline.

pub mod diagnostic;

pub use diagnostic::{
    Diagnostic, DiagnosticBag, DiagnosticSeverity, Span,
    // Well-known diagnostic codes
    DEPRECATED_PACK_VERSION,
    INVALID_PARAMETER_NAME,
    MERGE_CONFLICT,
    MISSING_REQUIRED_SLOT,
    RETIRED_PACK_VERSION,
    UNDECLARED_MERGE,
    UNKNOWN_ATOM_KIND,
    UNKNOWN_DECLARATIVE_KIND,
    UNKNOWN_LOOP_VARIABLE,
    UNKNOWN_PACK_REFERENCE,
    UNKNOWN_TEMPLATE_PARAMETER,
    UNRESOLVED_INSERTION_MARKER,
    UNRESOLVED_NAME_REF,
};
