//! Agent Module
//!
//! Contains the agent learning infrastructure for continuous improvement.

pub mod learning;

pub use learning::{
    spawn_agent_drain_task, AgentEvent, AgentEventEmitter, AgentEventPayload,
    AgentLearningInspector, CorrectionType, DrainConfig, LearnedData, LearningCandidate,
    LearningStatus, LearningType, LearningWarmup, SharedAgentEmitter, SharedLearnedData,
    WarmupStats,
};
