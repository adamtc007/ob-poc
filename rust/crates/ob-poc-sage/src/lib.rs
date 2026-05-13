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
//! ## Public surface contract
//!
//! Consumers should reach for:
//! - `ObservationPlane`, `IntentPolarity`, `OutcomeAction`, `OutcomeIntent`,
//!   `OutcomeStep`, `EntityRef`, `Clarification`, `SageConfidence`,
//!   `UtteranceHints`, `SageExplain`, `CoderHandoff` — the Sage's output
//!   vocabulary.
//! - `SageContext`, `RecentIntent` — session context handed INTO the Sage.
//! - `pre_classify()`, `SagePreClassification` — deterministic
//!   classification with no LLM call.
//! - `CoderResult`, `CoderResolution`, `CoderDiagnostics`,
//!   `CoderFailureKind`, `CoderFilterDiagnostics` — Coder's output handed
//!   BACK to the orchestrator.
//! - `UtteranceDisposition`, `ServeIntent`, `DelegateIntent`,
//!   `PendingMutation` — Sage-primary routing decision.
//! - `ScoredVerbCandidate`, `FilterDiagnostics` — verb-scorer DTOs.
//!
//! ## Dependency discipline
//!
//! Must depend only on `ob-poc-types` and primitives (`chrono`, `serde`,
//! `uuid`, `anyhow`, `thiserror`, `async-trait`). Optionally `sqlx` for
//! `session_context`'s DB-loader helpers, gated behind the `database`
//! feature. Must NOT depend on `dsl-core`, `dsl-runtime`, `sem_os_*`,
//! `ob-poc-boundary`, or any execution-tier surface in ob-poc.
//!
//! ## Migration status (2026-05-13)
//!
//! This crate is the destination for Phase 2 of the capability-crate
//! restructure (`docs/todo/capability-crate-restructure-v1.md`). Phase 2
//! moves nine sage modules out of `ob-poc-boundary::sage::*` into this
//! crate's top level. Until Phase 2 lands, this crate is intentionally
//! empty — adding modules ahead of Phase 2 risks re-introducing the
//! drift the restructure exists to fix.

// Empty — Phase 2 fills this in.
