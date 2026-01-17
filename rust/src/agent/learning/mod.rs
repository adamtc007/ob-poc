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

pub mod decay;
pub mod drain;
pub mod embedder;
pub mod emitter;
pub mod inspector;
pub mod types;
pub mod warmup;

pub use decay::ConfidenceDecay;
pub use drain::{spawn_agent_drain_task, DrainConfig};
pub use embedder::{
    CachedEmbedder, Embedder, Embedding, NullEmbedder, OpenAIEmbedder, SharedEmbedder,
};
pub use emitter::{AgentEventEmitter, AgentEventReceiver, SharedAgentEmitter};
pub use inspector::{AgentLearningInspector, LearningCandidate, LearningStatus, LearningType};
pub use types::{
    AgentEvent, AgentEventPayload, CorrectionType, EntityCandidate, ExtractedIntent,
    ResolutionMethod, ResolvedEntity,
};
pub use warmup::{LearnedData, LearningWarmup, SharedLearnedData, WarmupStats};
