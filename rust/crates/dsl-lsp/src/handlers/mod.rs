//! LSP request handlers.
//!
//! Only `diagnostics::analyze_document` is reachable through the library
//! target (`lib.rs`). The remaining handlers — code_actions, completion,
//! goto_definition, hover, playbook, rename, signature, symbols — are
//! consumed by the binary-only `server::DslLanguageServer` (in `main.rs`).
//! The `#[allow(dead_code)]` on those modules reflects that the lib build
//! genuinely doesn't reach them; the bin build does. Splitting this crate
//! into `dsl-lsp-core` (lib, analyser surface) and `dsl-lsp-bin` (server)
//! would let us drop the allow, but the duplication isn't worth it today.

#[allow(dead_code)]
pub(crate) mod code_actions;
#[allow(dead_code)]
pub(crate) mod completion;
pub mod diagnostics;
#[allow(dead_code)]
pub(crate) mod goto_definition;
#[allow(dead_code)]
pub(crate) mod hover;
#[allow(dead_code)]
pub(crate) mod playbook;
#[allow(dead_code)]
pub(crate) mod rename;
#[allow(dead_code)]
pub(crate) mod signature;
#[allow(dead_code)]
pub(crate) mod symbols;
