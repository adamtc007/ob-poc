//! `GoalProposalTrace` — Phase 3.7.
//!
//! Each prompt round-trip now emits a typed `GoalProposalTrace` to a
//! pluggable sink. The trace captures the full agent view of one
//! drafting cycle (goal frame + draft + validation + audit
//! provenance) and is the replay-grade artefact V&S §13 references
//! ("audit and replay across BYOK models"). Phase 4 wires the
//! sink to the SemOS Semantic Traceability Kernel via
//! `sem_os_client`; the spike ships a logging stub.
//!
//! ## Spike vs. Phase 4
//!
//! - Spike: `LoggingTraceSink` writes a `tracing::info!` event per
//!   emission. Useful for live debug + paired with the JSONL audit
//!   sink (Phase 2.9) gives replay-grade history.
//! - Phase 4: `SemTraceKernelSink` posts to the substrate's
//!   Semantic Traceability Kernel through `sem_os_client::
//!   dispatch_tool` so traces become part of the substrate's
//!   audit chain.
//!
//! Trace shape is intentionally narrow: it embeds the goal frame
//! (already serialisable), the draft outcome (verb FQN + source),
//! the validation summary (passed + diagnostic count), and the
//! audit provenance (goal frame id, knowledge provider, hydrator
//! provider). Future Phase 4 widening is additive — never rename.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::goal_frame::GoalFrame;
use crate::planning::DraftSource;
use crate::repl_channel::ValidationOutcome;

/// Slim validation summary captured in the trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub passed: bool,
    pub diagnostic_count: usize,
}

impl From<&ValidationOutcome> for ValidationSummary {
    fn from(outcome: &ValidationOutcome) -> Self {
        Self {
            passed: outcome.passed(),
            diagnostic_count: outcome.diagnostics.len(),
        }
    }
}

/// One trace record. Emitted after every successful drafting cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalProposalTrace {
    /// Goal frame at the moment of emission (already carries
    /// frontier, blockers, approval, refused_drafts, …).
    pub goal_frame: GoalFrame,
    /// Verb FQN the planning loop drafted.
    pub draft_verb_fqn: String,
    /// Where the draft came from (LLM vs. deterministic fallback).
    pub draft_source: DraftSource,
    /// Result of the LSP-shaped validate round-trip.
    pub validation: ValidationSummary,
    /// Knowledge provider that answered the substrate query
    /// (`stub` / `phase-2-spike` / `sem_os_mcp@…` / `none`).
    pub knowledge_provider: String,
    /// Constellation hydrator provider (`stub` / `phase-3-spike` /
    /// `sem_os_mcp@…` / `none`).
    pub hydrator_provider: String,
    /// When the trace record was minted.
    pub emitted_at: DateTime<Utc>,
}

impl GoalProposalTrace {
    pub fn from_parts(
        goal_frame: GoalFrame,
        draft_verb_fqn: String,
        draft_source: DraftSource,
        validation: &ValidationOutcome,
        knowledge_provider: Option<&str>,
        hydrator_provider: Option<&str>,
    ) -> Self {
        Self {
            goal_frame,
            draft_verb_fqn,
            draft_source,
            validation: validation.into(),
            knowledge_provider: knowledge_provider.unwrap_or("none").to_string(),
            hydrator_provider: hydrator_provider.unwrap_or("none").to_string(),
            emitted_at: Utc::now(),
        }
    }
}

/// Async sink the prompt handler emits to after each draft cycle.
#[async_trait]
pub trait GoalProposalTraceSink: Send + Sync {
    async fn emit(&self, trace: GoalProposalTrace);

    /// Provider label for the startup banner / audit cross-check.
    fn provider_label(&self) -> &str {
        "unknown"
    }
}

/// Spike sink. `tracing::info!` per emission so live debug shows
/// the trace shape. Pair with `JsonlAuditSink` for durable replay.
#[derive(Debug, Default, Clone)]
pub struct LoggingTraceSink {
    label: String,
}

impl LoggingTraceSink {
    pub fn new() -> Self {
        Self {
            label: "logging".to_string(),
        }
    }

    pub fn with_label(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

#[async_trait]
impl GoalProposalTraceSink for LoggingTraceSink {
    async fn emit(&self, trace: GoalProposalTrace) {
        tracing::info!(
            target: "sage-acp.traces",
            goal_frame_id = %trace.goal_frame.id,
            pack_id = %trace.goal_frame.pack_id,
            pack_hash = %trace.goal_frame.pack_hash,
            verb_fqn = %trace.draft_verb_fqn,
            draft_source = ?trace.draft_source,
            validation_passed = trace.validation.passed,
            knowledge_provider = %trace.knowledge_provider,
            hydrator_provider = %trace.hydrator_provider,
            "goal_proposal_trace emitted"
        );
    }

    fn provider_label(&self) -> &str {
        &self.label
    }
}

/// Drop-everything sink. Used by tests and by operators who want
/// trace emission off.
#[derive(Debug, Default, Clone)]
pub struct NullTraceSink;

#[async_trait]
impl GoalProposalTraceSink for NullTraceSink {
    async fn emit(&self, _trace: GoalProposalTrace) {
        // No-op.
    }

    fn provider_label(&self) -> &str {
        "null"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal_frame::{GoalFrame, GoalFrameStatus};
    use crate::repl_channel::ValidationOutcome;
    use chrono::Utc;

    fn sample_frame() -> GoalFrame {
        let now = Utc::now();
        GoalFrame {
            id: "gf-test".to_string(),
            utterance: "set up".to_string(),
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
            approval: None,
            refused_drafts: Vec::new(),
        }
    }

    #[test]
    fn from_parts_threads_provider_labels() {
        let validation = ValidationOutcome {
            source: "(cbu.create)".to_string(),
            diagnostics: Vec::new(),
        };
        let trace = GoalProposalTrace::from_parts(
            sample_frame(),
            "cbu.create".to_string(),
            DraftSource::DeterministicFallback,
            &validation,
            Some("phase-2-spike"),
            Some("phase-3-spike"),
        );
        assert_eq!(trace.draft_verb_fqn, "cbu.create");
        assert_eq!(trace.draft_source, DraftSource::DeterministicFallback);
        assert!(trace.validation.passed);
        assert_eq!(trace.validation.diagnostic_count, 0);
        assert_eq!(trace.knowledge_provider, "phase-2-spike");
        assert_eq!(trace.hydrator_provider, "phase-3-spike");
    }

    #[test]
    fn from_parts_defaults_missing_providers_to_none() {
        let validation = ValidationOutcome {
            source: "(cbu.create)".to_string(),
            diagnostics: Vec::new(),
        };
        let trace = GoalProposalTrace::from_parts(
            sample_frame(),
            "cbu.create".to_string(),
            DraftSource::LlmTool,
            &validation,
            None,
            None,
        );
        assert_eq!(trace.knowledge_provider, "none");
        assert_eq!(trace.hydrator_provider, "none");
    }

    #[tokio::test]
    async fn logging_sink_label_round_trip() {
        let sink = LoggingTraceSink::with_label("phase-3-spike");
        assert_eq!(sink.provider_label(), "phase-3-spike");
        let validation = ValidationOutcome {
            source: "(cbu.create)".to_string(),
            diagnostics: Vec::new(),
        };
        let trace = GoalProposalTrace::from_parts(
            sample_frame(),
            "cbu.create".to_string(),
            DraftSource::DeterministicFallback,
            &validation,
            Some("stub"),
            Some("stub"),
        );
        // Should not panic.
        sink.emit(trace).await;
    }

    #[tokio::test]
    async fn null_sink_drops_traces() {
        let sink = NullTraceSink;
        let validation = ValidationOutcome {
            source: "(cbu.create)".to_string(),
            diagnostics: Vec::new(),
        };
        let trace = GoalProposalTrace::from_parts(
            sample_frame(),
            "cbu.create".to_string(),
            DraftSource::DeterministicFallback,
            &validation,
            None,
            None,
        );
        sink.emit(trace).await;
        assert_eq!(sink.provider_label(), "null");
    }
}
