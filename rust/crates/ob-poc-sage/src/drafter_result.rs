//! Coder result DTOs — relocated from `src/sage/drafter.rs` for Phase 3 slice 2aa.
//!
//! These five types form the Coder's output surface: the resolution verdict,
//! failure taxonomy, scorer-filter counts, and end-to-end result envelope.
//! They are referenced by `sage::disposition::PendingMutation` and consumed by
//! the agent orchestrator + dispatcher. The `DrafterEngine` itself stays in
//! `ob_poc::sage::drafter` because it pulls execution-tier deps (dsl_core
//! verb config, mcp::intent_pipeline, scorer state).
//!
//! Slice 2bb (2026-05-13) update: `FilterDiagnostics` itself moved to the
//! boundary tier (see `verb_resolve_types`), so the
//! `From<FilterDiagnostics> for DraftFilterDiagnostics` impl that used to
//! live at the engine site now lives alongside the types.

use serde::{Deserialize, Serialize};

/// Resolution state for the Coder output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DraftResolution {
    Confident,
    Proposed,
    NeedsInput,
}

/// Explicit failure reason when deterministic Coder resolution cannot proceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DraftFailureKind {
    NoCandidateAfterFilters,
    DomainConflict,
    PhaseConflict,
    SubjectKindConflict,
    ActionConflict,
    BelowThreshold,
    PolicyConflict,
}

/// Diagnostics explaining how Coder resolution succeeded or failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftDiagnostics {
    pub failure_kind: Option<DraftFailureKind>,
    pub filter_diagnostics: DraftFilterDiagnostics,
    pub top_candidate: Option<String>,
    pub top_score: Option<f32>,
    pub threshold: Option<f32>,
}

/// Serializable copy of scorer filter counts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DraftFilterDiagnostics {
    pub base_candidates: usize,
    pub domain_candidates: usize,
    pub phase_candidates: usize,
    pub subject_kind_candidates: usize,
    pub final_candidates: usize,
}

/// End-to-end Coder output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftResult {
    pub verb_fqn: String,
    pub dsl: String,
    pub resolution: DraftResolution,
    pub missing_args: Vec<String>,
    pub unresolved_refs: Vec<String>,
    pub diagnostics: Option<DraftDiagnostics>,
}
