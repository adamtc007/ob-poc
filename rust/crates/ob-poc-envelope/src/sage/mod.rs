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

pub mod context;
pub mod outcome;
pub mod plane;
pub mod polarity;
