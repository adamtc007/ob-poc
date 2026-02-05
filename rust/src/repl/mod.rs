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
//! ## 2. REPL State Machine (New)
//!
//! Clean redesign with explicit state machine:
//! 1. Command Ledger as single source of truth
//! 2. All state derived from ledger entries
//! 3. Pure IntentMatcher service (no side effects)
//! 4. Clear state transitions (Idle → IntentMatching → Clarifying/DslReady → Executing)
//!
//! The new state machine (`orchestrator`, `intent_matcher`, `session`, `types`, `response`)
//! will eventually replace the staged runbook subsystem.

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
// REPL State Machine Subsystem (New)
// ============================================================================

pub mod intent_matcher;
pub mod orchestrator;
mod response;
pub mod session;
pub mod types;

// Re-export new state machine types
pub use types::{
    ClarifyingKind, ClarifyingState, ClientGroupOption, EntityCandidate, EntityMention,
    EntryStatus, IntentMatchResult, IntentTierOption, LedgerEntry, LedgerExecutionResult,
    MatchContext, MatchDebugInfo, MatchOutcome, ReplCommand, ReplState, ScopeCandidate,
    ScopeContext, UnresolvedRef, UserInput, VerbCandidate,
};

pub use intent_matcher::{EntityLinkingService, HybridIntentMatcher, IntentMatcher, LlmClient};
pub use orchestrator::{ClientGroupProvider, DslExecutor, ReplOrchestrator};
pub use response::{ReplResponse, ReplResponseKind};
pub use session::{ChatMessage, DerivedState, MessageRole, ReplSession};
