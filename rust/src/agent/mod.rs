//! Agent Module
//!
//! Contains the agent learning infrastructure for continuous improvement.
//! Navigation commands go through the unified intent pipeline (view.* and session.* verbs).

pub(crate) mod agent_turn_context;
pub(crate) mod capability_provenance;
pub mod composite_state;
pub mod composite_state_loader;
pub mod constellation_verb_index;
pub(crate) mod control_plane_audit;
pub(crate) mod control_plane_envelope_store;
pub(crate) mod control_plane_floor;
pub(crate) mod control_plane_metrics;
// G5 (EOP-PLAN-CONTROLPLANE-GRADUATION-001 §3 item 4): widened from
// `pub(crate)` to `pub` -- `ob-poc-web`'s `bus_runtime.rs` (Path D's
// adapter, a different crate) needs `ShadowDecisionRow`/
// `build_shadow_decision_row`/`insert_shadow_decision` to persist Path D's
// shadow-gate evaluation rows. Every other item in this module stays at
// its existing `pub(crate)` visibility -- widening the module path alone
// does not widen any individual item that wasn't already marked `pub`
// (disclosed in the G5 session doc's blind-review summary as a real
// pub-surface change, not silently absorbed).
pub mod control_plane_shadow;
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

pub use learning::{LearningWarmup};
pub(crate) use learning::{spawn_agent_drain_task, AgentEvent, AgentEventEmitter, AgentEventPayload, AgentLearningInspector, CorrectionType, DrainConfig, LearnedData, LearningCandidate, LearningStatus, LearningType, SharedAgentEmitter, SharedLearnedData, WarmupStats};
