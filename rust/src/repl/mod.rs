//! REPL Module
//!
//! This module contains two subsystems:
//!
//! ## 1. Staged Runbook REPL (Legacy)
//!
//! Anti-hallucination execution model for DSL commands:
//! 1. Commands are staged (no side effects)
//! 2. Entity references resolved to UUIDs via DB search
//! 3. DAG analysis determines execution order
//! 4. Execution only happens on explicit user confirmation
//!
//! ## 2. Pack-Guided Runbook REPL (V2, vnext-repl feature)
//!
//! Clean redesign with explicit state machine, pack routing,
//! proposal engine, and durable execution:
//! - State machine in `orchestrator_v2`
//! - Pack selection + verb filtering in `session_v2`
//! - Intent matching via `intent_service` wrapping `IntentMatcher` trait
//! - Proposal generation via `proposal_engine`
//! - Runbook editing + execution via `runbook`

// ============================================================================
// Staged Runbook Subsystem (Legacy)
// ============================================================================

pub mod dag_analyzer;
pub mod events;
pub mod resolver;
pub mod staged_runbook;

#[cfg(feature = "database")]
pub mod repository;

#[cfg(feature = "database")]
pub mod service;

// Re-exports for staged runbook
pub use dag_analyzer::{DagAnalyzer, DagError, DependencyEdge, ReorderDiff, ReorderMove};
pub use events::{
    BlockingCommand, CommandResult, EntityFootprintEntry, LearnedTag, PickerCandidate,
    RunbookEvent, RunbookSummary, StagedCommandSummary,
};
pub use resolver::{EntityArgResolver, EntityMatch, MatchType, ResolutionResult};
pub use staged_runbook::{
    ResolutionSource, ResolutionStatus, ResolvedEntity, RunbookStatus, StagedCommand, StagedRunbook,
};

#[cfg(feature = "database")]
pub use repository::StagedRunbookRepository;

#[cfg(feature = "database")]
pub use service::{PickError, PickResult, RunError, RunbookService, StageError, StageResult};

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

#[cfg(feature = "vnext-repl")]
pub mod runbook;

#[cfg(feature = "vnext-repl")]
pub mod verb_config_index;

#[cfg(feature = "vnext-repl")]
pub mod executor_bridge;

#[cfg(feature = "vnext-repl")]
pub mod sentence_gen;

#[cfg(feature = "vnext-repl")]
pub mod types_v2;

#[cfg(feature = "vnext-repl")]
pub mod session_v2;

#[cfg(feature = "vnext-repl")]
pub mod response_v2;

#[cfg(feature = "vnext-repl")]
pub mod intent_service;

#[cfg(feature = "vnext-repl")]
pub mod proposal_engine;

#[cfg(feature = "vnext-repl")]
pub mod orchestrator_v2;

#[cfg(feature = "vnext-repl")]
#[cfg(feature = "database")]
pub mod session_repository;

#[cfg(feature = "vnext-repl")]
pub mod bootstrap;

#[cfg(feature = "vnext-repl")]
pub mod context_stack;

#[cfg(feature = "vnext-repl")]
pub mod scoring;

#[cfg(feature = "vnext-repl")]
pub mod entity_resolution;

#[cfg(feature = "vnext-repl")]
pub mod deterministic_extraction;

#[cfg(feature = "vnext-repl")]
pub mod decision_log;
