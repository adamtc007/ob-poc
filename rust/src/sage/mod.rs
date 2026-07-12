//! Sage module — slim remnant after T11.1b (2026-07-12, agent-tier
//! extraction).
//!
//! `arg_assembly`/`deterministic`/`drafter`/`llm_sage`/`verb_index`/
//! `verb_resolve` moved to `ob-poc-agent::sage` — re-exported here so
//! every existing `crate::sage::*` caller continues to resolve unchanged.
//!
//! `constrained_match`/`valid_verb_set` stay here — both reach real
//! capabilities directly (`mcp::verb_search::HybridVerbSearcher`,
//! `sem_os_runtime::constellation_runtime`), named as T11.2 keyed-door
//! targets in the ownership ledger's T11.1b entry.
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
// `ob-poc-sage::*` at the top level. `valid_verb_set` (still here) and the
// six engines now in `ob-poc-agent::sage` continue to reach them via this
// crate's or that crate's own `pub use ob_poc_sage::*` re-export
// respectively. See docs/todo/capability-crate-restructure-v1.md §2.2.
//
// T11.1b (2026-07-12): `drafter_result`/`verb_resolve_types` dropped from
// this crate's re-export list — their only consumers (`drafter.rs`/
// `verb_resolve.rs`) moved to `ob-poc-agent::sage`, which re-exports them
// independently; `constrained_match.rs`/`valid_verb_set.rs` (still here)
// never used them.
pub use ob_poc_sage::context;
pub use ob_poc_sage::disposition;
pub use ob_poc_sage::outcome;
pub use ob_poc_sage::plane;
pub use ob_poc_sage::polarity;
pub use ob_poc_sage::pre_classify;
// Phase 2B (2026-05-13) — session_context with its sqlx::PgPool loaders
// joined the other eight sage modules in ob-poc-sage. The
// ob-poc-boundary::sage submodule is gone. Deliberately NOT re-exported
// from ob-poc-agent::sage (MCA-001's AB4 finding — a capability-shaped
// re-export, T11.3's read-lens remedy, not carried into the agent-tier
// crate) — stays here, orchestrator.rs's only consumer is in ob-poc too.
pub use ob_poc_sage::session_context;
pub mod valid_verb_set;

// T11.1b (2026-07-12): re-export the six moved engines' flattened public
// surface so every existing `crate::sage::{DeterministicSage,DrafterEngine,
// LlmSage,...}` caller continues to resolve unchanged. `verb_index`/
// `verb_resolve` omitted as bare module paths — nothing in `ob-poc`
// references them by that path (only via the flattened re-exports below,
// which don't need them); `ob_poc_agent::sage::{verb_index,verb_resolve}`
// remains reachable directly for any future caller that does.
pub use ob_poc_agent::sage::{arg_assembly, deterministic, drafter, llm_sage};

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
