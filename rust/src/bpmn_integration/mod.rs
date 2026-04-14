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

pub mod canonical;
pub mod client;
pub mod config;
pub mod correlation;
pub mod dispatcher;
pub mod event_bridge;
pub mod job_frames;
pub mod parked_tokens;
pub mod pending_dispatch_worker;
pub mod pending_dispatches;
pub mod request_state;
pub mod signal_relay;
pub mod types;
pub mod worker;
