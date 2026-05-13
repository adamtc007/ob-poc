//! Audit emission for the Sage ACP runtime.
//!
//! Phase 2.9 of the Sage ACP capability plan. Every prompt that
//! traverses the planning loop produces one [`AuditRecord`] that
//! captures the inputs, the draft, the validation outcome, and
//! the goal-frame metadata. The spike emits records as JSON lines
//! (`.jsonl`) to a local file; Phase 5.3 wires the OTLP exporter
//! alongside the local sink per V&S §6.9 / §13.
//!
//! ## Replay-grade audit
//!
//! Each record carries:
//! - `goal_frame_id`, `created_at` — correlate against the editor
//!   session and the agent's own trace.
//! - `pack_id`, `pack_hash` — the manifest the agent saw is replay-
//!   verifiable byte-for-byte (matches V&S §6.5 audit invariant).
//! - `verb_fqn`, `draft_source` — the constrained-composition pick.
//! - `validation_passed`, `validation_diagnostic_count` — proof the
//!   draft cleared the LSP-shaped channel before the response.
//! - `knowledge_provider` — which knowledge transport answered the
//!   substrate query (Phase 4 distinguishes between the spike stub
//!   and the real `sem_os_mcp` client).
//!
//! ## Reliability
//!
//! `JsonlAuditSink::emit` is best-effort: on IO failure it logs at
//! warn level and continues. Audit is for post-hoc analysis, not for
//! gating execution. Phase 5.3 adds OTLP push as the durable
//! companion sink.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::planning::{DraftSource, PlanningOutcome};
use crate::repl_channel::ValidationOutcome;

/// One audited prompt round-trip. JSON-serialised as a single line
/// to the local sink. The shape is stable across spike and Phase 5
/// — the OTLP exporter wraps the same record in an OTLP event
/// envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Goal frame correlation id (`gf-<uuid>`).
    pub goal_frame_id: String,
    /// When the record was emitted.
    pub emitted_at: DateTime<Utc>,
    /// When the goal frame itself was constructed (planning round-
    /// trip start).
    pub created_at: DateTime<Utc>,
    /// Utterance text, length-truncated. Phase 4 redaction policy
    /// hashes / classifies before persisting.
    pub utterance: String,
    /// Pack the session anchored against.
    pub pack_id: String,
    /// SHA-256 of pack manifest YAML (replay invariant).
    pub pack_hash: String,
    /// Workspace tag (matches the serde-rename used elsewhere in
    /// the system — e.g. `cbu`, `onboarding_request`).
    pub workspace: String,
    /// Optional intent summary from the goal frame. `None` in Phase
    /// 2; Phase 3.4 fills it once the motivation prompt template
    /// lands.
    pub intent_summary: Option<String>,
    /// Verb FQN the planning loop drafted.
    pub verb_fqn: String,
    /// Identifier for which call site produced the draft.
    pub draft_source: String,
    /// Whether the LSP-shaped channel accepted the draft.
    pub validation_passed: bool,
    /// Diagnostic count from the channel.
    pub validation_diagnostic_count: usize,
    /// Knowledge provider label (`stub`, `phase-2-spike`,
    /// `sem_os_mcp@…`, etc.) or `none` when no client was wired.
    pub knowledge_provider: String,
}

impl AuditRecord {
    /// Build a record from the planning loop's outputs.
    pub fn from_outcome(
        outcome: &PlanningOutcome,
        validation: &ValidationOutcome,
        knowledge_provider: Option<&str>,
    ) -> Self {
        let draft_source = match outcome.source {
            DraftSource::LlmTool => "llm_tool",
            DraftSource::DeterministicFallback => "deterministic_fallback",
        }
        .to_string();
        Self {
            goal_frame_id: outcome.goal_frame.id.clone(),
            emitted_at: Utc::now(),
            created_at: outcome.goal_frame.created_at,
            utterance: outcome.goal_frame.utterance.clone(),
            pack_id: outcome.goal_frame.pack_id.clone(),
            pack_hash: outcome.goal_frame.pack_hash.clone(),
            workspace: outcome.goal_frame.workspace.clone(),
            intent_summary: outcome.goal_frame.intent_summary.clone(),
            verb_fqn: outcome.verb_fqn.clone(),
            draft_source,
            validation_passed: validation.passed(),
            validation_diagnostic_count: validation.diagnostics.len(),
            knowledge_provider: knowledge_provider.unwrap_or("none").to_string(),
        }
    }
}

/// Where audit records go. The spike emits to JSONL; Phase 5 adds
/// `OtlpAuditSink` and a `MultiSink` that fans out to both.
#[async_trait]
pub trait AuditSink: Send + Sync {
    async fn emit(&self, record: AuditRecord);
}

/// Best-effort JSONL sink. Appends one JSON object per line.
///
/// The mutex serialises writes so concurrent `emit` calls cannot
/// interleave bytes within a record. The Phase 2 binary handles one
/// prompt at a time so this is the simplest correct shape; Phase 4
/// can swap for an MPSC-backed writer task if concurrent emission
/// becomes a bottleneck.
pub struct JsonlAuditSink {
    path: PathBuf,
    write_lock: Mutex<()>,
}

impl JsonlAuditSink {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            write_lock: Mutex::new(()),
        }
    }

    /// The file the sink writes to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    async fn append_line(&self, line: String) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        Ok(())
    }
}

#[async_trait]
impl AuditSink for JsonlAuditSink {
    async fn emit(&self, record: AuditRecord) {
        let Ok(line) = serde_json::to_string(&record) else {
            tracing::warn!(
                target: "sage-acp",
                "failed to serialise audit record; dropping"
            );
            return;
        };
        let _guard = self.write_lock.lock().await;
        if let Err(error) = self.append_line(line).await {
            tracing::warn!(
                target: "sage-acp",
                %error,
                path = %self.path.display(),
                "audit emission failed"
            );
        }
    }
}

/// Discards records. Useful when running without a writable sink
/// (e.g. tests, CI smoke). The binary integrator picks this when
/// `OBPOC_SAGE_AUDIT` is set to `none`.
#[derive(Debug, Default, Clone)]
pub struct NullAuditSink;

#[async_trait]
impl AuditSink for NullAuditSink {
    async fn emit(&self, _record: AuditRecord) {
        // Drop on the floor.
    }
}

/// Default audit-file path discovery for the binary integrator.
/// Resolution order:
/// 1. `OBPOC_SAGE_AUDIT` env (explicit path or the literal `none` to
///    disable).
/// 2. `$XDG_STATE_HOME/sage-acp/audit.jsonl` if `XDG_STATE_HOME` set.
/// 3. `$HOME/.cache/sage-acp/audit.jsonl`.
/// 4. `./sage-acp-audit.jsonl` as the last-resort fallback.
pub fn default_audit_path() -> AuditPath {
    if let Ok(value) = std::env::var("OBPOC_SAGE_AUDIT") {
        if value == "none" {
            return AuditPath::Disabled;
        }
        return AuditPath::File(PathBuf::from(value));
    }
    if let Ok(xdg_state) = std::env::var("XDG_STATE_HOME") {
        return AuditPath::File(PathBuf::from(xdg_state).join("sage-acp").join("audit.jsonl"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return AuditPath::File(
            PathBuf::from(home)
                .join(".cache")
                .join("sage-acp")
                .join("audit.jsonl"),
        );
    }
    AuditPath::File(PathBuf::from("sage-acp-audit.jsonl"))
}

/// Outcome of [`default_audit_path`]. Distinguishes "audit disabled
/// by operator" from "audit enabled at <path>".
#[derive(Debug, Clone)]
pub enum AuditPath {
    File(PathBuf),
    Disabled,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal_frame::{GoalFrame, GoalFrameStatus};
    use crate::repl_channel::DraftDiagnostic;
    use chrono::Utc;
    use tempfile::TempDir;

    fn sample_outcome() -> PlanningOutcome {
        let now = Utc::now();
        PlanningOutcome {
            goal_frame: GoalFrame {
                id: "gf-test-123".to_string(),
                utterance: "set up a book".to_string(),
                pack_id: "book-setup".to_string(),
                pack_hash: "deadbeef".to_string(),
                workspace: "cbu".to_string(),
                intent_summary: None,
                created_at: now,
                updated_at: now,
                status: GoalFrameStatus::Proposed,
                constellation: None,
                frontier: None,
                blockers: None,
            },
            verb_fqn: "cbu.create".to_string(),
            source: DraftSource::DeterministicFallback,
        }
    }

    fn empty_validation() -> ValidationOutcome {
        ValidationOutcome {
            source: "(cbu.create)".to_string(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn record_from_outcome_captures_inputs() {
        let outcome = sample_outcome();
        let validation = empty_validation();
        let record = AuditRecord::from_outcome(&outcome, &validation, Some("phase-2-spike"));
        assert_eq!(record.goal_frame_id, "gf-test-123");
        assert_eq!(record.pack_id, "book-setup");
        assert_eq!(record.pack_hash, "deadbeef");
        assert_eq!(record.verb_fqn, "cbu.create");
        assert_eq!(record.draft_source, "deterministic_fallback");
        assert!(record.validation_passed);
        assert_eq!(record.validation_diagnostic_count, 0);
        assert_eq!(record.knowledge_provider, "phase-2-spike");
    }

    #[test]
    fn record_marks_failed_validation_and_diagnostic_count() {
        let outcome = sample_outcome();
        let validation = ValidationOutcome {
            source: "garbage".to_string(),
            diagnostics: vec![DraftDiagnostic {
                severity: crate::repl_channel::DraftDiagnosticSeverity::Error,
                message: "parse fail".to_string(),
                line: 1,
                column: 0,
            }],
        };
        let record = AuditRecord::from_outcome(&outcome, &validation, None);
        assert!(!record.validation_passed);
        assert_eq!(record.validation_diagnostic_count, 1);
        assert_eq!(record.knowledge_provider, "none");
    }

    #[tokio::test]
    async fn jsonl_sink_appends_one_line_per_record() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("audit.jsonl");
        let sink = JsonlAuditSink::new(&path);
        let validation = empty_validation();
        sink.emit(AuditRecord::from_outcome(
            &sample_outcome(),
            &validation,
            Some("stub"),
        ))
        .await;
        sink.emit(AuditRecord::from_outcome(
            &sample_outcome(),
            &validation,
            Some("stub"),
        ))
        .await;
        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let line_count = contents.lines().count();
        assert_eq!(line_count, 2);
        for line in contents.lines() {
            let parsed: AuditRecord = serde_json::from_str(line).unwrap();
            assert_eq!(parsed.pack_id, "book-setup");
        }
    }

    #[tokio::test]
    async fn null_sink_drops_records() {
        let sink = NullAuditSink;
        let validation = empty_validation();
        // Should not panic / error.
        sink.emit(AuditRecord::from_outcome(
            &sample_outcome(),
            &validation,
            None,
        ))
        .await;
    }

    // Both env-var branches exercised in a single test so cargo's
    // parallel test runner cannot race on the shared env state.
    #[test]
    fn default_audit_path_branches_on_env() {
        let original = std::env::var("OBPOC_SAGE_AUDIT").ok();

        std::env::set_var("OBPOC_SAGE_AUDIT", "none");
        match default_audit_path() {
            AuditPath::Disabled => {}
            AuditPath::File(p) => panic!("expected Disabled, got {}", p.display()),
        }

        std::env::set_var("OBPOC_SAGE_AUDIT", "/tmp/sage-audit-test.jsonl");
        match default_audit_path() {
            AuditPath::File(p) => assert_eq!(p, PathBuf::from("/tmp/sage-audit-test.jsonl")),
            AuditPath::Disabled => panic!("expected File"),
        }

        match original {
            Some(value) => std::env::set_var("OBPOC_SAGE_AUDIT", value),
            None => std::env::remove_var("OBPOC_SAGE_AUDIT"),
        }
    }
}
