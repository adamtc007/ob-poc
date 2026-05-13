//! LSP-shaped client surface to the REPL validator.
//!
//! Phase 2.7 of the Sage ACP capability plan. The Sage runtime sends
//! a draft (verb FQN → minimal DSL source) through this channel
//! before the draft reaches the editor as a `goalProposalTrace`. The
//! response is a structured diagnostics envelope shaped like LSP's
//! `publishDiagnostics`.
//!
//! ## Spike scope (Phase 2.7)
//!
//! - One method exercised: parse-level `validate`. Builds a minimal
//!   `(domain.verb)` source from the proposed FQN and parses it via
//!   `dsl_core::parser::parse_program`. Returns parse errors as
//!   diagnostics; otherwise emits an empty diagnostics envelope.
//! - In-process. Phase 4 swaps `LocalParseChannel` for an
//!   out-of-process JSON-RPC client speaking actual LSP traffic to
//!   `dsl-lsp`.
//! - No semantic validation, no DAG ordering, no preconditions —
//!   those land alongside the registry-loaded analyser channel
//!   (Phase 4) and the validator/executor wiring (V&S §6).
//!
//! The trait deliberately mirrors the LSP method names — `validate`
//! corresponds to a `runbook/validate` request and the published
//! diagnostics envelope mirrors `textDocument/publishDiagnostics`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Severity tier used in the diagnostics envelope. Mirrors LSP's
/// `DiagnosticSeverity` (without `Information`) and `dsl_runtime`'s
/// `Severity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftDiagnosticSeverity {
    Error,
    Warning,
    Hint,
}

/// One diagnostic in the validate response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftDiagnostic {
    pub severity: DraftDiagnosticSeverity,
    pub message: String,
    /// 1-based line.
    pub line: u32,
    /// 0-based column.
    pub column: u32,
}

/// Outcome of a single `validate` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationOutcome {
    /// Source the channel validated (echoed back so the audit
    /// emitter has a single envelope it can hash).
    pub source: String,
    /// All diagnostics produced. Empty when validation passes
    /// without warnings.
    pub diagnostics: Vec<DraftDiagnostic>,
}

impl ValidationOutcome {
    pub fn passed(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|d| d.severity == DraftDiagnosticSeverity::Error)
    }
}

/// Async client surface to the REPL validator.
///
/// Two impls planned:
/// - [`LocalParseChannel`] — in-process parse via `dsl_core`. Used
///   by the Phase 2.7 spike.
/// - `LspChannelClient` (Phase 4) — out-of-process JSON-RPC client
///   speaking actual LSP traffic to `dsl-lsp`, gating drafts on
///   real semantic validation.
#[async_trait]
pub trait ReplChannelClient: Send + Sync {
    /// Validate a complete runbook source. The shape mirrors LSP's
    /// `runbook/validate` custom method.
    async fn validate(&self, source: &str) -> ValidationOutcome;
}

/// In-process spike channel: parse-only validation via `dsl_core`.
#[derive(Debug, Default, Clone)]
pub struct LocalParseChannel;

impl LocalParseChannel {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ReplChannelClient for LocalParseChannel {
    async fn validate(&self, source: &str) -> ValidationOutcome {
        match dsl_core::parser::parse_program(source) {
            Ok(_) => ValidationOutcome {
                source: source.to_string(),
                diagnostics: Vec::new(),
            },
            Err(message) => ValidationOutcome {
                source: source.to_string(),
                diagnostics: vec![DraftDiagnostic {
                    severity: DraftDiagnosticSeverity::Error,
                    message,
                    line: 1,
                    column: 0,
                }],
            },
        }
    }
}

/// Build the minimal DSL source the spike submits for a draft verb
/// FQN. Phase 4 replaces this with the full runbook envelope (header
/// sexp + body sexp + state context) per V&S §6.5.
pub fn minimal_source_for_verb(verb_fqn: &str) -> String {
    format!("({verb_fqn})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parse_passes_for_valid_no_arg_verb() {
        let channel = LocalParseChannel::new();
        let outcome = channel.validate(&minimal_source_for_verb("cbu.create")).await;
        assert!(outcome.passed(), "diagnostics: {:?}", outcome.diagnostics);
        assert!(outcome.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn parse_fails_for_garbage_source() {
        let channel = LocalParseChannel::new();
        let outcome = channel.validate("not a valid dsl program").await;
        assert!(!outcome.passed());
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(outcome.diagnostics[0].severity, DraftDiagnosticSeverity::Error);
    }

    #[tokio::test]
    async fn parse_fails_for_unbalanced_parens() {
        let channel = LocalParseChannel::new();
        let outcome = channel.validate("(cbu.create").await;
        assert!(!outcome.passed());
    }

    #[test]
    fn minimal_source_shape_matches_dsl_syntax() {
        let source = minimal_source_for_verb("cbu.create");
        assert_eq!(source, "(cbu.create)");
        // Confirm it parses (regression guard if the DSL grammar ever
        // requires a trailing space or arg block).
        assert!(dsl_core::parser::parse_program(&source).is_ok());
    }
}
