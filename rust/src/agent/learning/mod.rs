//! Agent Learning Infrastructure
//!
//! Continuous improvement system that learns from user interactions:
//! - Entity aliases (user terms → canonical names)
//! - Lexicon tokens (vocabulary expansion)
//! - Invocation phrases (natural language → verb mapping)
//!
//! # Architecture
//!
//! Two independent paths write to `"ob-poc".events` /
//! `"ob-poc".learning_candidates` and are read back by this module; the
//! original fire-and-forget `AgentEvent` emitter/drain-task/decay pipeline
//! that this doc used to describe was never wired to a producer and was
//! removed (dead-code remediation, 2026-07-30):
//!
//! - **[`background`]**: periodic post-startup task — feedback analysis,
//!   auto-apply, promotion pipeline, embedding-coverage checks — via
//!   `ob_semantic_matcher::{FeedbackService, PatternLearner, PromotionService}`.
//! - **[`inspector::AgentLearningInspector`]**: on-demand analysis (MCP
//!   tools) — detects correction-based learning candidates, threshold
//!   auto-apply, manual approve/reject.
//!
//! [`warmup::LearningWarmup`] loads applied learnings into memory at
//! startup (and on manual reload) for fast lookup; [`embedder`] provides the
//! local Candle (BGE-small-en-v1.5) embedding service used for semantic
//! phrase matching.

pub mod background;
pub mod embedder;
pub mod inspector;
pub mod warmup;

pub use embedder::{CandleEmbedder, Embedder, Embedding};
pub(crate) use inspector::{AgentLearningInspector, LearningCandidate, LearningStatus, LearningType};
pub use warmup::{LearningWarmup};
pub(crate) use warmup::{LearnedData, SharedLearnedData, WarmupStats};

pub use background::{create_learning_status, spawn_learning_task, trigger_learning_cycle, LearningConfig};
