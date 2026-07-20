//! Agent Learning Infrastructure
//!
//! Continuous improvement system that learns from user interactions:
//! - Entity aliases (user terms → canonical names)
//! - Lexicon tokens (vocabulary expansion)
//! - Invocation phrases (natural language → verb mapping)
//!
//! # Architecture
//!
//! ```text
//! User Chat → AgentEvent (fire-and-forget) → DB (async drain)
//!                                                   ↓
//!                                            Learning Analysis
//!                                                   ↓
//!                              ┌─────────────────────────────────────┐
//!                              │         Trigger Points              │
//!                              ├─────────────────────────────────────┤
//!                              │ 1. Startup: Apply pending learnings │
//!                              │ 2. Threshold: 3+ occurrences        │
//!                              │ 3. Immediate: User corrections      │
//!                              │ 4. On-demand: MCP tools             │
//!                              └─────────────────────────────────────┘
//! ```
//!
//! # Two Feedback Loops
//!
//! This system complements the DSL execution feedback loop:
//!
//! - **Loop 1 (DSL)**: "Did the DSL execute correctly?" → Fix verbs/handlers
//! - **Loop 2 (Agent)**: "Did we understand the user?" → Learn from corrections
//!
//! Both loops use fire-and-forget emission (< 1μs overhead) with background
//! database persistence.

pub mod background;
pub mod decay;
pub mod drain;
pub mod embedder;
pub mod emitter;
pub mod inspector;
pub mod types;
pub mod warmup;

pub(crate) use decay::ConfidenceDecay;
pub(crate) use drain::{spawn_agent_drain_task, DrainConfig};
pub use embedder::{CandleEmbedder, Embedder, Embedding};
pub(crate) use embedder::{CachedEmbedder, NullEmbedder, SharedEmbedder, EMBEDDING_DIMENSION};
pub(crate) use emitter::{AgentEventEmitter, AgentEventReceiver, SharedAgentEmitter};
pub(crate) use inspector::{AgentLearningInspector, LearningCandidate, LearningStatus, LearningType};
pub(crate) use types::{
    AgentEvent, AgentEventPayload, CorrectionType, EntityCandidate, ExtractedIntent,
    ResolutionMethod, ResolvedEntity,
};
pub use warmup::{LearningWarmup};
pub(crate) use warmup::{LearnedData, SharedLearnedData, WarmupStats};

pub use background::{create_learning_status, spawn_learning_task, trigger_learning_cycle, LearningConfig};
pub(crate) use background::{LearningCycleResult, LearningStatus as BackgroundLearningStatus, SharedLearningStatus};
