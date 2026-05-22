//! `bpmn-runtime` — Journey-persisted hydrate/dehydrate runtime for the
//! unified DSL v0.1 (Tranche 6).
//!
//! # Architecture
//!
//! This is **not** a conventional in-memory long-running process engine. Every
//! event triggers:
//!
//! 1. Load state from store
//! 2. Process one transition
//! 3. Write state back to store
//! 4. Done
//!
//! No in-memory fibers or long-lived threads per instance. The design follows
//! §6 of `docs/design/v0.1/session2-compiler-and-runtime.md`.
//!
//! # Quick start
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use bpmn_runtime::{RuntimeEngine, InMemoryJourneyStore, ScriptedAdaptor, VerbRegistry};
//!
//! let store = Arc::new(InMemoryJourneyStore::new());
//! let spec  = Arc::new(dsl_lowering::lower(&graph, "my-process"));
//! let verbs = Arc::new(VerbRegistry::new());
//! let sw    = Arc::new(ScriptedAdaptor::new());
//!
//! let engine = RuntimeEngine::new(store, spec, verbs, sw);
//! let id = engine.start_instance(serde_json::json!({})).await?;
//! println!("status: {:?}", engine.get_instance_status(id).await?);
//! ```

pub mod event_loop;
pub mod metrics;
pub mod processor;
pub mod retention;
pub mod store;
pub mod switch;
pub mod types;
pub mod verb;

#[cfg(feature = "postgres")]
pub mod store_postgres;

// Re-export the most commonly used items.
pub use event_loop::RuntimeEngine;
pub use metrics::{MetricsSnapshot, RuntimeMetrics};
pub use retention::RetentionPolicy;
pub use store::{InMemoryJourneyStore, JourneyLogEntry, JourneyStore, PendingWaitInfo};
pub use switch::{
    EdgeInfo, ScriptedAdaptor, SwitchAdaptor, SwitchError, SwitchReply, SwitchRequest,
};
pub use types::{
    ActiveToken, EventEnvelope, EventId, EventKind, InstanceId, InstanceStatus, TokenId,
    WorkflowInstance, WriteLogEntry,
};
pub use verb::{VerbContext, VerbError, VerbHandler, VerbOutput, VerbRegistry};

#[cfg(feature = "postgres")]
pub use store_postgres::postgres::PostgresJourneyStore;
