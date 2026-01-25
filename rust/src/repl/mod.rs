//! Staged Runbook REPL
//!
//! Anti-hallucination execution model for DSL commands:
//! 1. Commands are staged (no side effects)
//! 2. Entity references resolved to UUIDs via DB search
//! 3. DAG analysis determines execution order
//! 4. Execution only happens on explicit user confirmation
//!
//! # Architecture
//!
//! ```text
//! User prompt
//!     │
//!     ▼
//! Intent Classification (Candle)
//!     │
//!     ├── StageCommand (default)
//!     │       │
//!     │       ▼
//!     │   runbook_stage tool
//!     │       │
//!     │       ▼
//!     │   Parse DSL → Resolve entities → Stage command
//!     │       │
//!     │       ▼
//!     │   CommandStaged / ResolutionAmbiguous / StageFailed event
//!     │
//!     ├── RunRunbook
//!     │       │
//!     │       ▼
//!     │   runbook_run tool
//!     │       │
//!     │       ▼
//!     │   Validate readiness → DAG sort → Execute in order
//!     │
//!     └── EditRunbook / ShowRunbook / AbortRunbook
//!             │
//!             ▼
//!         Corresponding tool
//! ```
//!
//! # Non-Negotiable Invariants
//!
//! 1. **No side-effects unless user explicitly says** `run/execute/commit`
//! 2. **No invented UUIDs** - All UUIDs from DB resolution or picker validation
//! 3. **No ambiguous refs** - Ambiguous → picker required before execution
//! 4. **DAG reordering transparent** - Diff shown to user
//! 5. **Agent must call tools** - Never answer with invented actions
//! 6. **Picker entity_ids from events only** - Cannot fabricate UUIDs

pub mod dag_analyzer;
pub mod events;
pub mod resolver;
pub mod staged_runbook;

#[cfg(feature = "database")]
pub mod repository;

#[cfg(feature = "database")]
pub mod service;

// Re-exports for convenience
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
