//! LSP-shaped client surface to the REPL validator.
//!
//! Phase 2.7 introduced a single-shot `validate(source)` method.
//! Phase 4.4 extends the surface to the full LSP-style runbook
//! lifecycle:
//!
//! - `open_runbook(uri, source)` — `textDocument/didOpen`
//!   equivalent. Registers a document URI with the channel.
//! - `change_runbook(uri, new_source)` —
//!   `textDocument/didChange` equivalent. Replaces the source
//!   while preserving the URI.
//! - `close_runbook(uri)` — `textDocument/didClose` equivalent.
//! - `validate_only(uri)` — custom method per V&S §7.2. Returns
//!   diagnostics for the current source bound to `uri`.
//! - `validate_and_execute(uri)` — custom method per V&S §7.2.
//!   The spike refuses with `ExecutionRefused::ApprovalRequired`
//!   because mutation flows through the workbook approval +
//!   compiled-runbook gate in `ob-poc-boundary`, not through this
//!   channel. Future hardening retains this refusal — execution
//!   never bypasses the gate.
//!
//! The original `validate(source)` method is retained for callers
//! that don't need persistent document state (the Phase 2.6 prompt
//! handler still uses it).

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;

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

/// Errors produced by the runbook-document lifecycle methods.
#[derive(Debug, Error)]
pub enum RunbookChannelError {
    #[error("runbook '{0}' not open on the channel")]
    NotOpen(String),
    #[error("runbook '{0}' already open on the channel")]
    AlreadyOpen(String),
    #[error("transport failure: {0}")]
    Transport(String),
}

impl From<serde_json::Error> for RunbookChannelError {
    fn from(error: serde_json::Error) -> Self {
        Self::Transport(format!("serde_json: {error}"))
    }
}

/// Why `validate_and_execute` refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionRefusalReason {
    /// Execution requires the workbook approval + compiled-runbook
    /// gate in `ob-poc-boundary`. The channel itself does not run
    /// mutations — V&S §6.4 / §7.2.
    ApprovalRequired,
    /// The runbook didn't pass validation; refuse before any
    /// approval step.
    ValidationFailed,
}

/// Outcome of `validate_and_execute`. The spike's only success
/// shape is `Refused` (no execution happens here); future hardening
/// keeps the refusal as the canonical path and adds a
/// `Validated { approval_token, … }` variant for hand-off to the
/// boundary's approval gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome_kind", rename_all = "snake_case")]
pub enum ValidateAndExecuteOutcome {
    Refused {
        reason: ExecutionRefusalReason,
        validation: ValidationOutcome,
        detail: String,
    },
}

/// Async client surface to the REPL validator.
///
/// Phase 4.4 widens this from the single-shot `validate(source)`
/// (still supported for backwards compatibility) to the full LSP-
/// shaped runbook lifecycle. Two impls today:
///
/// - [`LocalParseChannel`] — single-shot parse, no document state.
///   Used by the Phase 2 prompt handler.
/// - [`LocalRunbookChannel`] — full lifecycle with per-URI state.
///   Production-shape spike. Subprocess-based `dsl-lsp` client
///   uses the same trait surface when the transport swap lands.
#[async_trait]
pub trait ReplChannelClient: Send + Sync {
    /// Validate a complete runbook source (single-shot).
    async fn validate(&self, source: &str) -> ValidationOutcome;

    /// Open a runbook document bound to `uri` with the initial
    /// `source`. Default impl no-ops so legacy clients
    /// (`LocalParseChannel`) don't have to implement the document
    /// lifecycle.
    async fn open_runbook(&self, _uri: &str, _source: &str) -> Result<(), RunbookChannelError> {
        Ok(())
    }

    /// Update the document source bound to `uri`.
    async fn change_runbook(
        &self,
        _uri: &str,
        _new_source: &str,
    ) -> Result<(), RunbookChannelError> {
        Err(RunbookChannelError::Transport(
            "change_runbook unsupported on this channel".to_string(),
        ))
    }

    /// Drop the document bound to `uri`.
    async fn close_runbook(&self, _uri: &str) -> Result<(), RunbookChannelError> {
        Ok(())
    }

    /// Validate the document bound to `uri`. Returns the standard
    /// outcome shape.
    async fn validate_only(&self, _uri: &str) -> Result<ValidationOutcome, RunbookChannelError> {
        Err(RunbookChannelError::Transport(
            "validate_only unsupported on this channel".to_string(),
        ))
    }

    /// Spike refusal — execution never runs through the channel.
    /// V&S §6.4 / §7.2 reserves mutation for the boundary's
    /// workbook approval + compiled-runbook gate.
    async fn validate_and_execute(
        &self,
        _uri: &str,
    ) -> Result<ValidateAndExecuteOutcome, RunbookChannelError> {
        Err(RunbookChannelError::Transport(
            "validate_and_execute unsupported on this channel".to_string(),
        ))
    }
}

/// In-process single-shot channel: parse-only validation via
/// `dsl_core`. Document lifecycle methods are not implemented;
/// callers needing per-URI state use [`LocalRunbookChannel`].
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
        parse_into_outcome(source)
    }
}

/// In-process full-lifecycle channel: tracks document state per
/// URI and supports the LSP-shaped open / change / validate /
/// close / validate-and-execute methods.
#[derive(Default)]
pub struct LocalRunbookChannel {
    documents: Mutex<std::collections::HashMap<String, String>>,
}

impl LocalRunbookChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Returns a clone of the current source for `uri`, if open.
    pub async fn snapshot(&self, uri: &str) -> Option<String> {
        self.documents.lock().await.get(uri).cloned()
    }
}

#[async_trait]
impl ReplChannelClient for LocalRunbookChannel {
    async fn validate(&self, source: &str) -> ValidationOutcome {
        parse_into_outcome(source)
    }

    async fn open_runbook(&self, uri: &str, source: &str) -> Result<(), RunbookChannelError> {
        let mut guard = self.documents.lock().await;
        if guard.contains_key(uri) {
            return Err(RunbookChannelError::AlreadyOpen(uri.to_string()));
        }
        guard.insert(uri.to_string(), source.to_string());
        Ok(())
    }

    async fn change_runbook(&self, uri: &str, new_source: &str) -> Result<(), RunbookChannelError> {
        let mut guard = self.documents.lock().await;
        if !guard.contains_key(uri) {
            return Err(RunbookChannelError::NotOpen(uri.to_string()));
        }
        guard.insert(uri.to_string(), new_source.to_string());
        Ok(())
    }

    async fn close_runbook(&self, uri: &str) -> Result<(), RunbookChannelError> {
        let mut guard = self.documents.lock().await;
        if guard.remove(uri).is_none() {
            return Err(RunbookChannelError::NotOpen(uri.to_string()));
        }
        Ok(())
    }

    async fn validate_only(&self, uri: &str) -> Result<ValidationOutcome, RunbookChannelError> {
        let source = {
            let guard = self.documents.lock().await;
            guard
                .get(uri)
                .cloned()
                .ok_or_else(|| RunbookChannelError::NotOpen(uri.to_string()))?
        };
        Ok(parse_into_outcome(&source))
    }

    async fn validate_and_execute(
        &self,
        uri: &str,
    ) -> Result<ValidateAndExecuteOutcome, RunbookChannelError> {
        let validation = self.validate_only(uri).await?;
        if !validation.passed() {
            return Ok(ValidateAndExecuteOutcome::Refused {
                reason: ExecutionRefusalReason::ValidationFailed,
                validation,
                detail: format!("runbook '{uri}' failed validation; cannot proceed to execution"),
            });
        }
        // Spike always refuses with ApprovalRequired. Execution
        // flows through workbook approval + compiled-runbook gate
        // in ob-poc-boundary; the agent does not bypass.
        Ok(ValidateAndExecuteOutcome::Refused {
            reason: ExecutionRefusalReason::ApprovalRequired,
            validation,
            detail: "execution requires the workbook approval + compiled-runbook gate \
                     (V&S §6.4 / §7.2); the runbook channel does not run mutations"
                .to_string(),
        })
    }
}

fn parse_into_outcome(source: &str) -> ValidationOutcome {
    match dsl_core::parse_program(source) {
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
        let outcome = channel
            .validate(&minimal_source_for_verb("cbu.create"))
            .await;
        assert!(outcome.passed(), "diagnostics: {:?}", outcome.diagnostics);
        assert!(outcome.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn parse_fails_for_garbage_source() {
        let channel = LocalParseChannel::new();
        let outcome = channel.validate("not a valid dsl program").await;
        assert!(!outcome.passed());
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(
            outcome.diagnostics[0].severity,
            DraftDiagnosticSeverity::Error
        );
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
        assert!(dsl_core::parse_program(&source).is_ok());
    }

    // Phase 4.4 — LocalRunbookChannel lifecycle tests.

    #[tokio::test]
    async fn runbook_channel_open_then_validate() {
        let channel = LocalRunbookChannel::new();
        channel
            .open_runbook("runbook://session/1/draft", "(cbu.create)")
            .await
            .unwrap();
        let outcome = channel
            .validate_only("runbook://session/1/draft")
            .await
            .unwrap();
        assert!(outcome.passed());
    }

    #[tokio::test]
    async fn runbook_channel_change_updates_source() {
        let channel = LocalRunbookChannel::new();
        channel.open_runbook("uri", "(cbu.create)").await.unwrap();
        channel.change_runbook("uri", "garbage").await.unwrap();
        let outcome = channel.validate_only("uri").await.unwrap();
        assert!(!outcome.passed());
        assert_eq!(channel.snapshot("uri").await.unwrap(), "garbage");
    }

    #[tokio::test]
    async fn runbook_channel_open_twice_errors() {
        let channel = LocalRunbookChannel::new();
        channel.open_runbook("uri", "(cbu.create)").await.unwrap();
        let err = channel
            .open_runbook("uri", "(cbu.attach-product)")
            .await
            .expect_err("must reject double-open");
        assert!(matches!(err, RunbookChannelError::AlreadyOpen(_)));
    }

    #[tokio::test]
    async fn runbook_channel_validate_unopened_errors() {
        let channel = LocalRunbookChannel::new();
        let err = channel.validate_only("uri").await.expect_err("must reject");
        assert!(matches!(err, RunbookChannelError::NotOpen(_)));
    }

    #[tokio::test]
    async fn runbook_channel_close_then_validate_errors() {
        let channel = LocalRunbookChannel::new();
        channel.open_runbook("uri", "(cbu.create)").await.unwrap();
        channel.close_runbook("uri").await.unwrap();
        let err = channel.validate_only("uri").await.expect_err("must reject");
        assert!(matches!(err, RunbookChannelError::NotOpen(_)));
    }

    #[tokio::test]
    async fn validate_and_execute_refuses_passing_runbook_with_approval_required() {
        let channel = LocalRunbookChannel::new();
        channel.open_runbook("uri", "(cbu.create)").await.unwrap();
        let outcome = channel.validate_and_execute("uri").await.unwrap();
        match outcome {
            ValidateAndExecuteOutcome::Refused {
                reason, validation, ..
            } => {
                assert_eq!(reason, ExecutionRefusalReason::ApprovalRequired);
                assert!(validation.passed());
            }
        }
    }

    #[tokio::test]
    async fn validate_and_execute_refuses_failing_runbook_with_validation_failed() {
        let channel = LocalRunbookChannel::new();
        channel.open_runbook("uri", "garbage").await.unwrap();
        let outcome = channel.validate_and_execute("uri").await.unwrap();
        match outcome {
            ValidateAndExecuteOutcome::Refused {
                reason, validation, ..
            } => {
                assert_eq!(reason, ExecutionRefusalReason::ValidationFailed);
                assert!(!validation.passed());
            }
        }
    }
}
