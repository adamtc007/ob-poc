//! Traceability domain for utterance-level execution traces.

pub mod payloads;
#[cfg(feature = "database")]
pub mod phase2;
#[cfg(feature = "database")]
pub mod phase3;
#[cfg(feature = "database")]
pub mod phase4;
#[cfg(feature = "database")]
pub mod phase5;
pub mod types;

pub mod replay;
#[cfg(feature = "database")]
pub mod repository;

pub use payloads::{
    build_final_trace_payload, build_phase2_unavailable_payload, build_phase_trace_payload,
    build_trace_scaffold_payload,
};
#[cfg(feature = "database")]
pub use payloads::{build_phase2_trace_payload, compute_phase2_situation_signature_hash};
#[cfg(feature = "database")]
pub use phase2::{Phase2Artifacts, Phase2Evaluation, Phase2Service};
#[cfg(feature = "database")]
pub use phase3::{
    build_phase3_payload, build_phase3_unavailable_payload, enforce_phase2_evaluation_subset,
    enforce_phase2_legal_subset, evaluate_phase3_against_phase2, Phase3Evaluation,
    Phase3SubsetResult,
};
#[cfg(feature = "database")]
pub use phase4::{
    build_phase4_payload, build_phase4_unavailable_payload,
    enforce_phase4_resolution_within_evaluation, enforce_phase4_resolution_within_phase2,
    evaluate_phase4_within_phase2, fallback_reason_code_for_trace, Phase4Evaluation,
};
#[cfg(feature = "database")]
pub use phase5::build_phase5_unavailable_payload;
#[cfg(feature = "database")]
pub use phase5::{build_phase5_agent_payload, evaluate_phase5_agent, Phase5Evaluation};
#[cfg(feature = "database")]
pub use phase5::{
    build_phase5_repl_payload, build_repl_execution_shape_kind, evaluate_phase5_repl,
};
pub use types::{
    NewUtteranceTrace, SurfaceVersions, TraceKind, TraceOutcome, UtteranceTraceRecord,
};

#[cfg(feature = "database")]
pub use replay::compare_session_traces;
#[cfg(feature = "database")]
pub use replay::compare_trace_ids;
pub use replay::{
    compare_trace_records, compare_trace_sequences, compute_replay_narrowing_diff,
    derive_replay_verdict, NarrowingDrift, ReplayNarrowingDiff, ReplayVerdict,
    TraceReplayComparison,
};
#[cfg(feature = "database")]
pub use repository::UtteranceTraceRepository;
