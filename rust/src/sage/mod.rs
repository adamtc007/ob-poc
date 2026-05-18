//! Sage — intent understanding layer for the utterance→DSL pipeline.
//!
//! The Sage understands WHAT the user wants (intent) without ever resolving
//! HOW to do it (verb FQNs, DSL assembly). That is the Drafter's job.
//!
//! ## Architecture
//!
//! ```text
//! User utterance (raw text)
//!      │
//!      ▼  Stage 1.5 — BEFORE entity linking (E-SAGE-1)
//! ┌─────────────────────────────────────────────────────┐
//! │  SageEngine::classify(utterance, SageContext)        │
//! │  ┌───────────────────────────────────────────────┐  │
//! │  │ pre_classify() — deterministic, no LLM         │  │
//! │  │   1. ObservationPlane from session context    │  │
//! │  │   2. IntentPolarity from clue words           │  │
//! │  │   3. Domain hints from keyword scan            │  │
//! │  └───────────────────────────────────────────────┘  │
//! │  → OutcomeIntent (plane, polarity, domain, action)  │
//! └─────────────────────────────────────────────────────┘
//!      │
//!      ▼  Stage 3 — entity linking runs here (after Sage)
//! ```
//!
//! ## Invariants
//!
//! | ID | Invariant |
//! |----|-----------|
//! | E-SAGE-1 | Sage fires BEFORE entity linking (raw utterance, no UUID resolution) |
//! | E-SAGE-2 | Sage never sees verb FQNs (SageContext has no verb/fqn fields) |
//! | E-SAGE-3 | Drafter never interprets NL (takes OutcomeIntent, not &str) |
//! | E-SAGE-4 | Shadow mode has zero production impact |
//! | E-SAGE-5 | `cargo check -p ob-poc` passes after every sub-phase |
//! | E-SAGE-6 | data_management_rewrite() unchanged until Sage accuracy exceeds it |

pub mod constrained_match;
// Phase 2A of the capability-crate restructure (2026-05-13) — the eight
// pure-type sage modules moved out of `ob-poc-boundary::sage::*` into
// `ob-poc-sage::*` at the top level. Sibling Sage engines in this crate
// (deterministic, llm_sage, coder, verb_resolve, verb_index,
// arg_assembly, clash_matrix, valid_verb_set) continue to reach them via
// `super::{outcome, plane, polarity, context, drafter_result,
// verb_resolve_types, disposition, pre_classify}` through these
// re-exports. See docs/todo/capability-crate-restructure-v1.md §2.2.
pub use ob_poc_sage::context;
pub use ob_poc_sage::disposition;
pub use ob_poc_sage::drafter_result;
pub use ob_poc_sage::outcome;
pub use ob_poc_sage::plane;
pub use ob_poc_sage::polarity;
pub use ob_poc_sage::pre_classify;
// Phase 2B (2026-05-13) — session_context with its sqlx::PgPool loaders
// joined the other eight sage modules in ob-poc-sage. The
// ob-poc-boundary::sage submodule is gone.
pub use ob_poc_sage::session_context;
pub use ob_poc_sage::verb_resolve_types;
pub mod valid_verb_set;

// Phase 1.4
pub mod arg_assembly;
pub mod clash_matrix;
pub mod deterministic;
pub mod drafter;
pub mod llm_sage;
pub mod verb_index;
pub mod verb_resolve;

// Re-export core types for convenience
pub use arg_assembly::assemble_args_from_step;
pub use clash_matrix::{build_clash_matrix, render_clash_reports, ClashRow};
pub use context::{RecentIntent, SageContext};
pub use deterministic::DeterministicSage;
pub use disposition::{DelegateIntent, PendingMutation, ServeIntent, UtteranceDisposition};
pub use drafter::{DraftResolution, DraftResult, DrafterEngine};
pub use llm_sage::LlmSage;
pub use outcome::{
    Clarification, DrafterHandoff, EntityRef, OutcomeAction, OutcomeIntent, OutcomeStep,
    SageConfidence, SageExplain, UtteranceHints,
};
pub use plane::ObservationPlane;
pub use polarity::IntentPolarity;
pub use pre_classify::SagePreClassification;
pub use verb_index::{runtime_registry_parity, VerbMeta, VerbMetadataIndex};
pub use verb_resolve::{ScoredVerbCandidate, StructuredVerbScorer};

// Phase 2.2 (2026-05-13): SageEngine trait relocated to
// `ob_poc_sage::engine`. Compat re-export keeps existing `crate::sage::
// SageEngine` callers (orchestrator, deterministic, llm_sage) working.
pub use ob_poc_sage::engine::SageEngine;
