//! Agent Module
//!
//! Contains the agent learning infrastructure for continuous improvement.
//! Navigation commands go through the unified intent pipeline (view.* and session.* verbs).

pub(crate) mod agent_turn_context;
pub(crate) mod capability_provenance;
pub mod composite_state;
pub mod composite_state_loader;
pub mod constellation_verb_index;
pub(crate) mod control_plane_envelope_store;
pub(crate) mod control_plane_floor;
pub(crate) mod control_plane_metrics;
pub(crate) mod control_plane_shadow;
pub(crate) mod control_plane_write_attestation_store;
pub mod harness;
pub mod learning;
pub(crate) mod legality_grant;
pub mod narration_engine;
pub mod onboarding_state_view;
pub mod orchestrator;
pub mod sem_os_context_envelope;
pub mod telemetry;
pub mod verb_surface;
pub mod workspace_mode_tags;

pub use learning::{
    spawn_agent_drain_task, AgentEvent, AgentEventEmitter, AgentEventPayload,
    AgentLearningInspector, CorrectionType, DrainConfig, LearnedData, LearningCandidate,
    LearningStatus, LearningType, LearningWarmup, SharedAgentEmitter, SharedLearnedData,
    WarmupStats,
};
