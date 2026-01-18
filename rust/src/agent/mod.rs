//! Agent Module
//!
//! Contains the agent learning infrastructure for continuous improvement,
//! and the ESPER navigation command registry.

pub mod esper;
pub mod learning;

pub use esper::{
    EsperCommandRegistry, EsperConfig, EsperMatch, EsperWarmup, MatchSource, MatchType,
};
pub use learning::{
    spawn_agent_drain_task, AgentEvent, AgentEventEmitter, AgentEventPayload,
    AgentLearningInspector, CorrectionType, DrainConfig, LearnedData, LearningCandidate,
    LearningStatus, LearningType, LearningWarmup, SharedAgentEmitter, SharedLearnedData,
    WarmupStats,
};
