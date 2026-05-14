//! DSL Language Server library facade.
//!
//! Mirrors `main.rs`'s module set so that:
//!   - the integration tests in `tests/` can `use dsl_lsp::analyze_document`,
//!   - the `pub mod` declarations let dead-code analysis see every handler
//!     through the lib build (every handler is reachable through the server's
//!     `LanguageServer` impl, but only when something constructs the server).
//!
//! Nothing outside this crate consumes the library beyond `analyze_document`;
//! the actual LSP server runs as the `dsl-lsp` binary in `main.rs`.

pub mod analysis;
pub mod encoding;
pub mod entity_client;
pub mod handlers;
pub mod server;

pub use handlers::diagnostics::analyze_document;
