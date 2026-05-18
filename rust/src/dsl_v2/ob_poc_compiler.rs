//! ob-poc verb compiler re-export.
//!
//! The implementation lives in the `ob-poc-compiler` crate so that
//! `dsl-lsp` and `ob-agentic` can depend on it without pulling in
//! the full ob-poc binary. This module re-exports everything for
//! callers that already have access to `dsl_v2`.
pub use ob_poc_compiler::*;
