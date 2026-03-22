//! Agent Module
//!
//! Contains the agent learning infrastructure for continuous improvement.
//! Navigation commands go through the unified intent pipeline (view.* and session.* verbs).

pub mod composite_state;
pub mod composite_state_loader;
pub mod harness;
pub mod learning;
pub mod orchestrator;
pub mod sem_os_context_envelope;
pub mod telemetry;
pub mod verb_surface;

pub use learning::{
    spawn_agent_drain_task, AgentEvent, AgentEventEmitter, AgentEventPayload,
    AgentLearningInspector, CorrectionType, DrainConfig, LearnedData, LearningCandidate,
    LearningStatus, LearningType, LearningWarmup, SharedAgentEmitter, SharedLearnedData,
    WarmupStats,
};
