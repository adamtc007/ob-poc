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

#![allow(dead_code)]

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

// Public submodules required by external integration tests or crates
pub mod runbook;
pub mod verb_config_index;
pub mod executor_bridge;
pub mod types_v2;
pub mod intent_service;
pub mod decision_log;

#[cfg(feature = "database")]
pub mod session_repository;

// strictly internal submodules
pub(crate) mod sentence_gen;
pub(crate) mod session_v2;
pub(crate) mod response_v2;
pub(crate) mod proposal_engine;
pub(crate) mod bootstrap;
pub(crate) mod context_stack;
pub(crate) mod scoring;

#[cfg(test)]
pub(crate) mod entity_resolution;

pub(crate) mod deterministic_extraction;
pub(crate) mod preconditions;

// Phase 3 slice 2c.2b (2026-05-12): relocated to ob-poc-boundary.
pub use ob_poc_boundary::session_trace;

#[cfg(feature = "database")]
pub(crate) mod trace_repository;

pub(crate) mod session_replay;
