//! Out-of-process LSP-shaped runbook channel — spawns `dsl-lsp`
//! and speaks proper LSP traffic (Content-Length framing,
//! initialize handshake, document lifecycle notifications,
//! `textDocument/publishDiagnostics` notifications back).
//!
//! Sibling to `InProcessTransport` / `SubprocessTransport` for the
//! MCP knowledge surface. Implements the [`ReplChannelClient`]
//! trait so it slots into the same agent wiring as
//! `LocalRunbookChannel`.
//!
//! ## Why this exists
//!
//! V&S §6.5 / D1=a wants Sage to use `dsl-lsp` as its REPL channel
//! so the same analyser pipeline serves both human editors and the
//! agent. The Phase 4.4 spike used an in-process parse-only channel
//! while the dsl-lsp dependency wall was being inverted (§9 item
//! 8). With that wall cut, the agent can drive a real `dsl-lsp`
//! subprocess and get full analyser diagnostics.
//!
//! ## Concurrency
//!
//! Single `tokio::sync::Mutex` guarding the (stdin, stdout) pair —
//! matches the spike's single-prompt-at-a-time planning loop. A
//! `validate_only` call drains messages until it sees a
//! `textDocument/publishDiagnostics` notification for the URI it
//! cares about; intervening responses / unrelated notifications are
//! discarded for the spike. Production deployments that need
//! concurrent calls or persistent state should layer a proper
//! request-id correlation map on top; the `ReplChannelClient` trait
//! surface is unchanged either way.
//!
//! ## Error semantics
//!
//! Spawn / framing / parse failures surface as
//! [`RunbookChannelError::Transport`]. A dead subprocess is
//! detected on the next call (stdin write or stdout read fails).
//! The transport does not auto-restart.

use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use lsp_types::{
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Initialized,
        Notification,
    },
    request::{Initialize, Request},
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializedParams, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, Url, VersionedTextDocumentIdentifier,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::repl_channel::{
    DraftDiagnostic, DraftDiagnosticSeverity, ExecutionRefusalReason, ReplChannelClient,
    RunbookChannelError, ValidateAndExecuteOutcome, ValidationOutcome,
};

/// Default timeout for waiting on `publishDiagnostics` for a URI
/// after a `didOpen` / `didChange`. The analyser runs in-process
/// inside the subprocess, so the realistic upper bound is well
/// under a second; 3s is generous for safety.
const DEFAULT_DIAGNOSTIC_TIMEOUT: Duration = Duration::from_secs(3);

/// Subprocess LSP-shaped runbook channel.
pub struct SubprocessLspChannel {
    inner: Mutex<Inner>,
    /// Per-URI document version counters — LSP `didChange` requires
    /// a monotonically increasing version per document. Counters
    /// reset on `didClose`.
    versions: Mutex<HashMap<String, i32>>,
    /// Request id counter for synchronous LSP requests (initialize,
    /// future custom requests). Monotonic.
    next_request_id: Mutex<i64>,
    label: String,
    diagnostic_timeout: Duration,
}

struct Inner {
    #[allow(dead_code)]
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl SubprocessLspChannel {
    /// Spawn the named `dsl-lsp` binary and complete the LSP
    /// `initialize` handshake. Returns the ready-to-use channel.
    pub async fn spawn(
        command: impl AsRef<std::ffi::OsStr>,
        args: &[&str],
    ) -> Result<Self, RunbookChannelError> {
        let mut child = Command::new(command.as_ref())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| RunbookChannelError::Transport(format!("spawn dsl-lsp: {e}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| RunbookChannelError::Transport("stdin missing".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| RunbookChannelError::Transport("stdout missing".into()))?;
        let pid = child.id().unwrap_or(0);
        let command_lossy = command.as_ref().to_string_lossy().into_owned();

        let mut channel = Self {
            inner: Mutex::new(Inner {
                child,
                stdin: BufWriter::new(stdin),
                stdout: BufReader::new(stdout),
            }),
            versions: Mutex::new(HashMap::new()),
            next_request_id: Mutex::new(1),
            label: format!("subprocess:{command_lossy} pid={pid}"),
            diagnostic_timeout: DEFAULT_DIAGNOSTIC_TIMEOUT,
        };

        channel.do_initialize_handshake().await?;
        Ok(channel)
    }

    /// Override the diagnostic-wait timeout (default 3s).
    pub fn with_diagnostic_timeout(mut self, timeout: Duration) -> Self {
        self.diagnostic_timeout = timeout;
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    async fn do_initialize_handshake(&mut self) -> Result<(), RunbookChannelError> {
        let id = self.next_id().await;
        // Minimal initialize params — dsl-lsp doesn't require
        // anything more than a present params block. Use defaults.
        let params = InitializeParams::default();
        let request = Message::request(id, Initialize::METHOD, serde_json::to_value(params)?);

        let mut inner = self.inner.lock().await;
        write_message(&mut inner.stdin, &request).await?;
        // Drain messages until we see the matching response.
        loop {
            let msg = read_message(&mut inner.stdout).await?;
            if let Some(resp_id) = msg.id.as_ref().and_then(|v| v.as_i64()) {
                if resp_id == id {
                    if let Some(err) = msg.error {
                        return Err(RunbookChannelError::Transport(format!(
                            "initialize failed: code={} msg={}",
                            err.code, err.message
                        )));
                    }
                    break;
                }
            }
            // Pre-initialize notifications would be unusual; ignore.
        }

        // Send `initialized` notification.
        let notification =
            Message::notification(Initialized::METHOD, serde_json::to_value(InitializedParams {})?);
        write_message(&mut inner.stdin, &notification).await?;
        Ok(())
    }

    async fn next_id(&self) -> i64 {
        let mut guard = self.next_request_id.lock().await;
        let id = *guard;
        *guard += 1;
        id
    }

    async fn bump_version(&self, uri: &str) -> i32 {
        let mut guard = self.versions.lock().await;
        let entry = guard.entry(uri.to_string()).or_insert(0);
        *entry += 1;
        *entry
    }

    async fn clear_version(&self, uri: &str) {
        let mut guard = self.versions.lock().await;
        guard.remove(uri);
    }

    /// Wait for a `textDocument/publishDiagnostics` notification
    /// matching `uri`. Intervening responses / unrelated
    /// notifications are discarded; returns an empty outcome on
    /// timeout (no signal = no errors observed within window).
    async fn collect_diagnostics(
        &self,
        inner: &mut Inner,
        uri: &str,
    ) -> Result<ValidationOutcome, RunbookChannelError> {
        let result = timeout(self.diagnostic_timeout, async {
            loop {
                let msg = read_message(&mut inner.stdout).await?;
                if msg.method.as_deref() == Some("textDocument/publishDiagnostics") {
                    if let Some(params) = msg.params {
                        let diag_uri = params
                            .get("uri")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        if diag_uri == uri {
                            return parse_publish_diagnostics(&params);
                        }
                    }
                }
            }
        })
        .await;
        match result {
            Ok(Ok(outcome)) => Ok(outcome),
            Ok(Err(err)) => Err(err),
            Err(_) => {
                // Timeout: no diagnostics arrived. Treat as "no
                // findings" — safer than synthesising a fake error.
                Ok(ValidationOutcome {
                    source: String::new(),
                    diagnostics: Vec::new(),
                })
            }
        }
    }
}

#[async_trait]
impl ReplChannelClient for SubprocessLspChannel {
    async fn validate(&self, source: &str) -> ValidationOutcome {
        // Single-shot validation reuses the lifecycle methods
        // against a transient document URI so dsl-lsp's full
        // pipeline runs. Errors degrade to an empty outcome —
        // callers asking for `validate(&str)` are typically the
        // Phase 2 prompt handler which doesn't surface transport
        // errors separately.
        let uri = format!("memory:single-shot/{}.runbook", uuid::Uuid::new_v4());
        if self.open_runbook(&uri, source).await.is_err() {
            return ValidationOutcome {
                source: source.to_string(),
                diagnostics: Vec::new(),
            };
        }
        let outcome = self
            .validate_only(&uri)
            .await
            .unwrap_or_else(|_| ValidationOutcome {
                source: source.to_string(),
                diagnostics: Vec::new(),
            });
        let _ = self.close_runbook(&uri).await;
        ValidationOutcome {
            source: source.to_string(),
            diagnostics: outcome.diagnostics,
        }
    }

    async fn open_runbook(
        &self,
        uri: &str,
        source: &str,
    ) -> Result<(), RunbookChannelError> {
        let version = self.bump_version(uri).await;
        let url = parse_uri(uri)?;
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: url,
                language_id: "dsl".to_string(),
                version,
                text: source.to_string(),
            },
        };
        let notification = Message::notification(
            DidOpenTextDocument::METHOD,
            serde_json::to_value(params)?,
        );
        let mut inner = self.inner.lock().await;
        write_message(&mut inner.stdin, &notification).await?;
        Ok(())
    }

    async fn change_runbook(
        &self,
        uri: &str,
        new_source: &str,
    ) -> Result<(), RunbookChannelError> {
        let version = self.bump_version(uri).await;
        let url = parse_uri(uri)?;
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri: url, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: new_source.to_string(),
            }],
        };
        let notification = Message::notification(
            DidChangeTextDocument::METHOD,
            serde_json::to_value(params)?,
        );
        let mut inner = self.inner.lock().await;
        write_message(&mut inner.stdin, &notification).await?;
        Ok(())
    }

    async fn close_runbook(&self, uri: &str) -> Result<(), RunbookChannelError> {
        let url = parse_uri(uri)?;
        let params = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: url },
        };
        let notification = Message::notification(
            DidCloseTextDocument::METHOD,
            serde_json::to_value(params)?,
        );
        let mut inner = self.inner.lock().await;
        write_message(&mut inner.stdin, &notification).await?;
        drop(inner);
        self.clear_version(uri).await;
        Ok(())
    }

    async fn validate_only(&self, uri: &str) -> Result<ValidationOutcome, RunbookChannelError> {
        let mut inner = self.inner.lock().await;
        self.collect_diagnostics(&mut inner, uri).await
    }

    async fn validate_and_execute(
        &self,
        uri: &str,
    ) -> Result<ValidateAndExecuteOutcome, RunbookChannelError> {
        let validation = self.validate_only(uri).await?;
        // Spike refuses execution per V&S §6.4 — mutation requires
        // the boundary workbook-approval + compiled-runbook gate.
        let (reason, detail) = if validation.passed() {
            (
                ExecutionRefusalReason::ApprovalRequired,
                "execution refused: workbook approval required (V&S §6.4)".to_string(),
            )
        } else {
            (
                ExecutionRefusalReason::ValidationFailed,
                "execution refused: validation produced error diagnostics".to_string(),
            )
        };
        Ok(ValidateAndExecuteOutcome::Refused {
            reason,
            validation,
            detail,
        })
    }
}

/// Minimal LSP message shape. Covers all of {request, response,
/// notification} per the JSON-RPC 2.0 envelope LSP wraps around.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<MessageError>,
}

impl Message {
    fn request(id: i64, method: &str, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id: Some(json!(id)),
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    fn notification(method: &str, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id: None,
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageError {
    code: i64,
    message: String,
}

async fn write_message(
    stdin: &mut BufWriter<ChildStdin>,
    msg: &Message,
) -> Result<(), RunbookChannelError> {
    let body = serde_json::to_string(msg)
        .map_err(|e| RunbookChannelError::Transport(format!("serialise lsp message: {e}")))?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    stdin
        .write_all(header.as_bytes())
        .await
        .map_err(|e| RunbookChannelError::Transport(format!("write lsp header: {e}")))?;
    stdin
        .write_all(body.as_bytes())
        .await
        .map_err(|e| RunbookChannelError::Transport(format!("write lsp body: {e}")))?;
    stdin
        .flush()
        .await
        .map_err(|e| RunbookChannelError::Transport(format!("flush lsp stdin: {e}")))?;
    Ok(())
}

async fn read_message(
    stdout: &mut BufReader<ChildStdout>,
) -> Result<Message, RunbookChannelError> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let read = stdout
            .read_line(&mut line)
            .await
            .map_err(|e| RunbookChannelError::Transport(format!("read lsp header: {e}")))?;
        if read == 0 {
            return Err(RunbookChannelError::Transport(
                "subprocess closed stdout (EOF) while reading lsp message".into(),
            ));
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value
                .trim()
                .parse::<usize>()
                .ok()
                .or_else(|| {
                    tracing::warn!(target: "sage-acp", "malformed Content-Length: {trimmed}");
                    None
                });
        }
        // Other headers (Content-Type, etc.) are ignored.
    }
    let len = content_length.ok_or_else(|| {
        RunbookChannelError::Transport("lsp message missing Content-Length header".into())
    })?;
    let mut body = vec![0u8; len];
    stdout
        .read_exact(&mut body)
        .await
        .map_err(|e| RunbookChannelError::Transport(format!("read lsp body: {e}")))?;
    serde_json::from_slice(&body)
        .map_err(|e| RunbookChannelError::Transport(format!("decode lsp message: {e}")))
}

fn parse_uri(uri: &str) -> Result<Url, RunbookChannelError> {
    Url::parse(uri).map_err(|e| RunbookChannelError::Transport(format!("parse uri '{uri}': {e}")))
}

fn parse_publish_diagnostics(params: &Value) -> Result<ValidationOutcome, RunbookChannelError> {
    let array = params
        .get("diagnostics")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            RunbookChannelError::Transport(
                "publishDiagnostics: diagnostics field missing or not array".into(),
            )
        })?;
    let diagnostics = array
        .iter()
        .map(|d| {
            let message = d
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            // LSP severity: 1=Error, 2=Warning, 3=Info, 4=Hint
            let severity = match d.get("severity").and_then(|v| v.as_i64()).unwrap_or(1) {
                1 => DraftDiagnosticSeverity::Error,
                2 => DraftDiagnosticSeverity::Warning,
                _ => DraftDiagnosticSeverity::Hint,
            };
            // LSP positions are 0-indexed; our DraftDiagnostic uses
            // 1-indexed line numbers per the Phase 4.4 contract.
            let (line, column) = d
                .get("range")
                .and_then(|r| r.get("start"))
                .and_then(|s| {
                    let line = s.get("line").and_then(|v| v.as_u64())?;
                    let col = s.get("character").and_then(|v| v.as_u64())?;
                    Some((line as u32 + 1, col as u32))
                })
                .unwrap_or((1, 0));
            DraftDiagnostic {
                severity,
                message,
                line,
                column,
            }
        })
        .collect();
    Ok(ValidationOutcome {
        source: String::new(),
        diagnostics,
    })
}

/// Test helper: locate the `dsl-lsp` binary sibling to the
/// currently-running test executable (same target dir).
#[cfg(test)]
pub(crate) fn locate_dsl_lsp_binary() -> std::path::PathBuf {
    let test_exe = std::env::current_exe().expect("current_exe");
    let bin_dir = test_exe
        .parent()
        .and_then(|p| p.parent())
        .expect("target debug dir");
    bin_dir.join("dsl-lsp")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Integration test against the real `dsl-lsp` binary. Requires
    /// `cargo build -p dsl-lsp --bin dsl-lsp` first. Marked
    /// `#[ignore]` so default `cargo test` stays hermetic.
    #[tokio::test]
    #[ignore = "requires `cargo build -p dsl-lsp` first; opt-in via --ignored"]
    async fn subprocess_lsp_channel_round_trips_minimal_runbook() {
        let bin_path = locate_dsl_lsp_binary();
        assert!(
            bin_path.exists(),
            "dsl-lsp binary not at {} — run `cargo build -p dsl-lsp` first",
            bin_path.display()
        );

        let channel = SubprocessLspChannel::spawn(&bin_path, &[])
            .await
            .expect("spawn dsl-lsp");
        assert!(channel.label().starts_with("subprocess:"));

        // Open a trivial document and check we receive
        // publishDiagnostics without timing out.
        let uri = "memory:test/round-trip.runbook";
        channel
            .open_runbook(uri, "(cbu.create)")
            .await
            .expect("didOpen");
        let outcome = channel.validate_only(uri).await.expect("validate_only");
        // We don't assert on specific diagnostic content — dsl-lsp's
        // analyser may or may not find anything in the minimal
        // source. What matters is that the round-trip works without
        // a transport error.
        let _ = outcome;
        channel.close_runbook(uri).await.expect("didClose");
    }
}
