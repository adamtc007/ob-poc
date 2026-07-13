//! Sage — intent understanding layer for the utterance→DSL pipeline.
//!
//! The Sage understands WHAT the user wants (intent) without ever resolving
//! HOW to do it (verb FQNs, DSL assembly). That is the Drafter's job.
//!
//! T11.1b (2026-07-12): the six deterministic/LLM drafting engines
//! (`arg_assembly`, `deterministic`, `drafter`, `llm_sage`, `verb_index`,
//! `verb_resolve`) live here. `constrained_match`/`valid_verb_set` stay in
//! `ob-poc` — both reach real capabilities directly
//! (`mcp::verb_search::HybridVerbSearcher`, `sem_os_runtime::
//! constellation_runtime`), named as T11.2 keyed-door targets in the
//! ownership ledger's T11.1b entry. `ob-poc::sage` re-exports this
//! module's public surface alongside its own local
//! `constrained_match`/`valid_verb_set` so every existing
//! `crate::sage::*` caller in `ob-poc` sees one unified path, unchanged.
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

// Phase 2A of the capability-crate restructure (2026-05-13) — the eight
// pure-type sage modules moved out of `ob-poc-boundary::sage::*` into
// `ob-poc-sage::*` at the top level. Sibling Sage engines in this crate
// (deterministic, llm_sage, verb_resolve, verb_index, arg_assembly)
// continue to reach them via `super::{outcome, plane, polarity, context,
// drafter_result, verb_resolve_types, disposition, pre_classify}` through
// these re-exports. See docs/todo/capability-crate-restructure-v1.md §2.2.
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
//
// Gated: session_context is only compiled in ob-poc-sage behind its own
// `database` feature, so this re-export must be gated identically or
// isolated builds of this crate fail unconditionally (2026-07-13 E5 fix).
#[cfg(feature = "database")]
pub use ob_poc_sage::session_context;
pub use ob_poc_sage::verb_resolve_types;

// Phase 1.4
pub mod arg_assembly;
pub mod deterministic;
pub mod drafter;
pub mod llm_sage;
pub mod verb_index;
pub mod verb_resolve;

/// Sage-classification / Coder-drafting turn stages — T11.2 Part A
/// (2026-07-13), relocated from `ob_poc::agent::orchestrator`.
pub mod stages;

// Re-export core types for convenience
pub use context::{RecentIntent, SageContext};
pub use deterministic::DeterministicSage;
pub use disposition::{DelegateIntent, PendingMutation, ServeIntent, UtteranceDisposition};
pub use drafter::{DraftResult, DrafterEngine};
pub use llm_sage::LlmSage;
pub use outcome::{OutcomeAction, OutcomeIntent, OutcomeStep, SageConfidence};
pub use plane::ObservationPlane;
pub use polarity::IntentPolarity;

// Phase 2.2 (2026-05-13): SageEngine trait relocated to
// `ob_poc_sage::engine`. Compat re-export keeps existing `crate::sage::
// SageEngine` callers (orchestrator, deterministic, llm_sage) working.
pub use ob_poc_sage::engine::SageEngine;
