//! Agent Module
//!
//! Contains the agent learning infrastructure for continuous improvement.
//! Navigation commands go through the unified intent pipeline (view.* and session.* verbs).

pub mod learning;
pub mod orchestrator;
pub mod telemetry;

pub use learning::{
    spawn_agent_drain_task, AgentEvent, AgentEventEmitter, AgentEventPayload,
    AgentLearningInspector, CorrectionType, DrainConfig, LearnedData, LearningCandidate,
    LearningStatus, LearningType, LearningWarmup, SharedAgentEmitter, SharedLearnedData,
    WarmupStats,
};
