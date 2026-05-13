//! Verb-resolution result DTOs — relocated from `src/sage/verb_resolve.rs`
//! for Phase 3 slice 2bb.
//!
//! `ScoredVerbCandidate` is the per-verb output of the structured scorer:
//! the verb FQN, total score, and the action/param sub-scores that fed it.
//! `FilterDiagnostics` records the candidate counts after each
//! deterministic filter stage (base, domain, phase, subject-kind, final).
//!
//! Both are pure DTOs with no execution-tier dependencies. The
//! `StructuredVerbScorer` that produces them stays in ob-poc because it
//! needs `VerbMetadataIndex` (dsl_core verb config).
//!
//! The `From<FilterDiagnostics> for CoderFilterDiagnostics` impl moved
//! here alongside the type — once both sides of the conversion live in
//! envelope, the orphan rule requires the impl to live with them too.

use super::coder_result::CoderFilterDiagnostics;

/// Ranked candidate returned by the structured scorer.
#[derive(Debug, Clone)]
pub struct ScoredVerbCandidate {
    pub fqn: String,
    pub score: f32,
    pub action_score: f32,
    pub param_overlap_score: f32,
}

/// Candidate counts after each deterministic filter stage.
#[derive(Debug, Clone, Copy, Default)]
pub struct FilterDiagnostics {
    pub base_candidates: usize,
    pub domain_candidates: usize,
    pub phase_candidates: usize,
    pub subject_kind_candidates: usize,
    pub final_candidates: usize,
}

impl From<FilterDiagnostics> for CoderFilterDiagnostics {
    fn from(value: FilterDiagnostics) -> Self {
        Self {
            base_candidates: value.base_candidates,
            domain_candidates: value.domain_candidates,
            phase_candidates: value.phase_candidates,
            subject_kind_candidates: value.subject_kind_candidates,
            final_candidates: value.final_candidates,
        }
    }
}
