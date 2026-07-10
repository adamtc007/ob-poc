//! Traceability domain for utterance-level execution traces.
//!
//! Phase 3 slice 2h (2026-05-12): pure DTO contracts (`types`) moved to
//! `ob_poc_boundary::traceability::types`; phase builders, replay, and
//! the Postgres repository remain here.

pub mod payloads;
#[cfg(feature = "database")]
pub mod phase2;
#[cfg(feature = "database")]
pub mod phase3;
#[cfg(feature = "database")]
pub mod phase4;
#[cfg(feature = "database")]
pub mod phase5;
pub use ob_poc_boundary::traceability::types;

pub mod replay;
#[cfg(feature = "database")]
pub mod repository;

#[cfg(feature = "database")]
pub use payloads::{build_phase2_trace_payload, compute_phase2_situation_signature_hash};
pub use payloads::{
    build_phase2_unavailable_payload, build_phase_trace_payload, build_trace_scaffold_payload,
};
#[cfg(feature = "database")]
pub use phase2::{Phase2Evaluation, Phase2Service};
#[cfg(feature = "database")]
pub use phase3::{
    build_phase3_unavailable_payload, evaluate_phase3_against_phase2, Phase3Evaluation,
};
#[cfg(feature = "database")]
pub use phase4::{
    build_phase4_unavailable_payload, evaluate_phase4_within_phase2, Phase4Evaluation,
};
#[cfg(feature = "database")]
pub use phase5::build_phase5_unavailable_payload;
#[cfg(feature = "database")]
pub use phase5::evaluate_phase5_repl;
pub use types::{
    NewUtteranceTrace, SurfaceVersions, TraceKind, TraceOutcome, UtteranceTraceRecord,
};

#[cfg(feature = "database")]
pub use repository::UtteranceTraceRepository;
