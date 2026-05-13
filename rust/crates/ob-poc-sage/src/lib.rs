//! ob-poc-sage — Sage intent understanding.
//!
//! ## Capability claim
//!
//! Turns raw user utterances into structured intent: (`plane`, `polarity`,
//! `action`, `domain_concept`, `subject`). The Sage decides WHAT the user
//! wants without ever resolving HOW to do it (verb FQNs, DSL assembly,
//! database state) — that's the Coder's and sequencer's job.
//!
//! ## Anti-charter
//!
//! - NOT DSL assembly (`arg_assembly` may move here later but only the
//!   pure shape, not the `mcp::intent_pipeline`-coupled glue).
//! - NOT verb scoring tied to live SemReg state (that's `valid_verb_set`,
//!   which stays in ob-poc).
//! - NOT LLM client wiring (`llm_sage` stays in ob-poc).
//!
//! ## Public surface
//!
//! Eight pure-type modules (Phase 2A, 2026-05-13):
//! - `plane` — `ObservationPlane`.
//! - `polarity` — `IntentPolarity` + clue-word lists.
//! - `context` — `SageContext`, `RecentIntent`.
//! - `outcome` — `OutcomeIntent`, `OutcomeAction`, `OutcomeStep`,
//!   `EntityRef`, `Clarification`, `SageConfidence`, `UtteranceHints`,
//!   `SageExplain`, `DrafterHandoff`.
//! - `drafter_result` — `DraftResolution`, `DraftFailureKind`,
//!   `DraftDiagnostics`, `DraftFilterDiagnostics`, `DraftResult`.
//! - `verb_resolve_types` — `ScoredVerbCandidate`, `FilterDiagnostics`
//!   + `From<FilterDiagnostics> for DraftFilterDiagnostics`.
//! - `disposition` — `UtteranceDisposition`, `ServeIntent`,
//!   `DelegateIntent`, `PendingMutation`.
//! - `pre_classify` — `pre_classify()` + `SagePreClassification`.
//!
//! One DB-coupled module (Phase 2B, pending):
//! - `session_context` — `SageSession`, `EntityState`, sqlx::PgPool
//!   helpers like `load_entity_states_for_group`.
//!
//! ## Dependency discipline
//!
//! Depends only on `ob-poc-types` and primitives (`chrono`, `serde`,
//! `uuid`, `anyhow`, `thiserror`, `async-trait`). `sqlx` is optional and
//! gated behind the `database` feature (used by `session_context`). Does
//! NOT depend on `dsl-core`, `dsl-runtime`, `sem_os_*`,
//! `ob-poc-boundary`, or any execution-tier surface in ob-poc.

pub mod context;
pub mod disposition;
pub mod drafter_result;
// Phase 2.2 (2026-05-13): engine traits — SageEngine (moved here from
// ob-poc), ValidVerbSetEngine (new). The traits live here so
// ob-poc-agent can reach engines that live in ob-poc without depending
// on ob-poc. Locked decision 6.
pub mod engine;
pub mod outcome;
pub mod plane;
pub mod polarity;
pub mod pre_classify;
#[cfg(feature = "database")]
pub mod session_context;
// Phase 2.2 (2026-05-13): pure-DTO types for the valid verb set
// (relocated from ob-poc::sage::valid_verb_set). The computation
// reaches sem_os_core constellation_runtime and stays in ob-poc as
// the ValidVerbSetEngine impl.
pub mod valid_verb_set;
pub mod verb_resolve_types;
