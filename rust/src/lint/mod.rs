//! Lint Module - Schema validation for macro and verb definitions
//!
//! This module provides lint rules for validating YAML macro schemas,
//! ensuring they follow the operator vocabulary conventions and don't
//! leak implementation jargon to the UI layer.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use ob_poc::lint::{lint_macro_file, Severity};
//!
//! let yaml = r#"
//! structure.setup:
//!   kind: macro
//!   ui:
//!     label: "Set up Structure"
//!     ...
//! "#;
//!
//! let diagnostics = lint_macro_file(yaml, None);
//! for d in diagnostics {
//!     if d.severity == Severity::Error {
//!         eprintln!("{}: {} at {}", d.code, d.message, d.path);
//!     }
//! }
//! ```

mod diagnostic;
mod macro_lint;

pub use diagnostic::{Diagnostic, Severity};
pub use macro_lint::{lint_macro_file, PrimitiveRegistry};
