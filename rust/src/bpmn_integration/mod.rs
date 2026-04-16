//! BPMN-Lite Integration Module
//!
//! Bridges the external bpmn-lite gRPC service to ob-poc's REPL pipeline.
//! Provides workflow dispatch (Direct vs Orchestrated routing), job worker
//! (long-poll execution loop), and event bridge (signal translation).
//!
//! ## Architecture
//!
//! ```text
//! ob-poc REPL                        bpmn-lite gRPC service
//! ───────────                        ──────────────────────
//! WorkflowDispatcher ──StartProcess──► Process Instance
//!                                          │
//!                    ◄──ActivateJobs──     VM ticks
//! JobWorker ──────────► execute verb       │
//!                    ──CompleteJob───►     resume fiber
//!                                          │
//! EventBridge ◄──SubscribeEvents──── lifecycle events
//!     │
//!     └─► ParkedTokenStore + CorrelationStore + REPL signal
//! ```

pub(crate) mod canonical;
pub(crate) mod client;
pub(crate) mod config;
pub(crate) mod correlation;
pub(crate) mod dispatcher;
pub(crate) mod event_bridge;
pub(crate) mod job_frames;
pub(crate) mod parked_tokens;
pub(crate) mod pending_dispatch_worker;
pub(crate) mod pending_dispatches;
pub(crate) mod request_state;
pub(crate) mod signal_relay;
pub(crate) mod types;
pub(crate) mod worker;

pub use canonical::{canonical_json_with_hash, sha256_bytes, validate_payload_hash};
pub use client::{
    BpmnLifecycleEvent, BpmnLiteConnection, CompileDiagnostic, CompileResult, CompleteJobRequest,
    FiberSnapshot, JobActivation, OrchestratorFlag, ProcessInspection, StartProcessRequest,
    WaitSnapshot,
};
pub use config::{WorkflowConfig, WorkflowConfigIndex};
pub use correlation::CorrelationStore;
pub use dispatcher::WorkflowDispatcher;
pub use event_bridge::EventBridge;
pub use job_frames::JobFrameStore;
pub use parked_tokens::ParkedTokenStore;
pub use pending_dispatch_worker::PendingDispatchWorker;
pub use pending_dispatches::PendingDispatchStore;
pub use request_state::RequestStateStore;
pub use signal_relay::SignalRelay;
pub use types::{
    CorrelationRecord, CorrelationStatus, ExecutionRoute, JobFrame, JobFrameStatus, OutcomeEvent,
    ParkedToken, ParkedTokenStatus, PendingDispatch, PendingDispatchStatus, RequestStateRecord,
    RequestStatus, TaskBinding, WorkflowBinding,
};
pub use worker::JobWorker;
