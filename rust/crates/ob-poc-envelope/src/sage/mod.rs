//! Sage pure-type vocabulary relocated from ob-poc's `src/sage/` for
//! Phase 3 slice 2v (2026-05-13).
//!
//! The four leaf modules here form the Sage's observable types — what
//! the orchestrator hands the Sage and what the Sage hands back to the
//! Coder — with zero internal-crate dependencies beyond serde/uuid.
//!
//! - `plane` — `ObservationPlane` (Instance / Structure / Registry).
//! - `polarity` — `IntentPolarity` (Read / Write / Ambiguous) + clue-word lists.
//! - `outcome` — `OutcomeIntent`, `OutcomeAction`, `OutcomeStep`, `EntityRef`,
//!   `Clarification`, `SageConfidence`, `UtteranceHints`, `SageExplain`,
//!   `CoderHandoff`.
//! - `context` — `SageContext`, `RecentIntent`.
//!
//! The Sage engine implementations (`pre_classify`, `deterministic`,
//! `llm_sage`, `coder`, `verb_resolve`, `verb_index`, `valid_verb_set`,
//! `session_context`, `clash_matrix`, `arg_assembly`, `constrained_match`,
//! `disposition`) stay in ob-poc because they pull in execution-tier
//! crates (database, dsl_v2, lookup, agent::sem_os_context_envelope).

// Phase 3 slice 2aa (2026-05-13): `coder_result` (CoderResolution +
// CoderFailureKind + CoderDiagnostics + CoderFilterDiagnostics + CoderResult)
// + `disposition` (UtteranceDisposition + ServeIntent + DelegateIntent +
// PendingMutation) relocated. `CoderEngine` itself stays in src/sage/coder.rs.
pub mod coder_result;
pub mod context;
pub mod disposition;
pub mod outcome;
pub mod plane;
pub mod polarity;
// Phase 3 slice 2cc (2026-05-13): full pre_classify module relocated —
// `SagePreClassification` DTO + `pre_classify()` deterministic
// classifier + helpers. Zero crate-internal deps beyond the already-
// relocated context/plane/polarity siblings, so the entire 775 LOC
// engine fits in the boundary tier as-is.
pub mod pre_classify;
// Phase 3 slice 2bb (2026-05-13): ScoredVerbCandidate + FilterDiagnostics +
// the From<FilterDiagnostics> for CoderFilterDiagnostics impl relocated
// here. Once both sides of the conversion are in envelope the orphan rule
// requires the impl to follow.
pub mod verb_resolve_types;
