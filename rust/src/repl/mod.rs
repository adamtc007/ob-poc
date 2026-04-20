//! REPL Module
//!
//! Pack-Guided Runbook REPL (V2, vnext-repl feature)
//!
//! Clean redesign with explicit state machine, pack routing,
//! proposal engine, and durable execution:
//! - State machine in `ob_poc::sequencer` (relocated from `ob_poc::sequencer`
//!   in Phase 5b narrow-scope extraction — `ReplOrchestratorV2` struct unchanged;
//!   only the module path moves to match V&S §8.1)
//! - Pack selection + verb filtering in `session_v2`
//! - Intent matching via `intent_service` wrapping `IntentMatcher` trait
//! - Proposal generation via `proposal_engine`
//! - Runbook editing + execution via `runbook`

// ============================================================================
// Shared Intent Matching (used by V2)
// ============================================================================

pub mod intent_matcher;
pub mod types;

pub use intent_matcher::IntentMatcher;
pub use types::{
    ClientGroupOption, EntityCandidate, EntityMention, IntentMatchResult, IntentTierOption,
    MatchContext, MatchDebugInfo, MatchOutcome, ScopeCandidate, ScopeContext, UnresolvedRef,
    VerbCandidate,
};

// ============================================================================
// REPL v2 — Pack-Guided Runbook Architecture (vnext-repl feature)
// ============================================================================

pub mod runbook;

pub mod verb_config_index;

pub mod executor_bridge;

pub mod sentence_gen;

pub mod types_v2;

pub mod session_v2;

pub mod response_v2;

pub mod intent_service;

pub mod proposal_engine;

// Phase 5b — `orchestrator_v2` relocated to `ob_poc::sequencer` (§8.1 alignment).
// All consumers now reach it via `crate::sequencer` / `ob_poc::sequencer`.

#[cfg(feature = "database")]
pub mod session_repository;

pub mod bootstrap;

pub mod context_stack;

pub mod scoring;

#[cfg(test)]
pub mod entity_resolution;

pub mod deterministic_extraction;

pub mod decision_log;

pub mod preconditions;

pub mod session_trace;

#[cfg(feature = "database")]
pub mod trace_repository;

pub mod session_replay;
