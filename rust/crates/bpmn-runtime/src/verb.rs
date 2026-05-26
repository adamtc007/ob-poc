//! Verb handler interface for the bpmn-lite runtime.
//!
//! Verb handlers are registered in a [`VerbRegistry`] before starting the
//! engine. When the runtime reaches a task node that has a `verb_ref`, it
//! looks up the handler and invokes it. If no handler is registered the
//! token is left at the node and a `pending_wait` row is created — the
//! caller must deliver a [`crate::types::EventKind::VerbCompletion`] event
//! to resume execution.

use crate::types::{InstanceId, TokenId};
use std::collections::{BTreeMap, HashMap};

/// Context provided to a verb handler during invocation.
pub struct VerbContext {
    /// @-slot bindings resolved at compile time + instance context.
    pub at_slots: BTreeMap<String, serde_json::Value>,
    /// Input arguments from the process data.
    pub inputs: BTreeMap<String, serde_json::Value>,
    /// Output collector — verb writes its outputs here.
    pub outputs: BTreeMap<String, serde_json::Value>,
    /// Pending effects emitted by the verb.
    pub effects: Vec<VerbEffect>,
    /// Current token ID.
    pub token_id: TokenId,
    /// Current instance ID.
    pub instance_id: InstanceId,
}

/// A side-effect that a verb can request from the runtime.
#[derive(Debug, Clone)]
pub enum VerbEffect {
    WriteData {
        location: String,
        value: serde_json::Value,
    },
    ScheduleTimer {
        duration_seconds: u64,
    },
    SendMessage {
        target: String,
        payload: serde_json::Value,
    },
    RaiseError {
        code: String,
        message: String,
    },
    RequestHumanTask {
        role: String,
        form_data: serde_json::Value,
    },
}

/// The output produced by a successful verb invocation.
#[derive(Debug, Clone)]
pub struct VerbOutput {
    /// Key-value pairs to write into instance data.
    pub data: BTreeMap<String, serde_json::Value>,
    /// Side-effects to enqueue.
    pub effects: Vec<VerbEffect>,
}

/// Errors that a verb can return.
#[derive(Debug, thiserror::Error)]
pub enum VerbError {
    #[error("verb error {code}: {message}")]
    Domain { code: String, message: String },
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Implement this trait for each verb that the runtime should execute inline.
#[async_trait::async_trait]
pub trait VerbHandler: Send + Sync {
    /// The fully-qualified verb name this handler services (e.g. `"cbu.create"`).
    fn verb_ref(&self) -> &str;
    async fn invoke(&self, ctx: VerbContext) -> Result<VerbOutput, VerbError>;
}

/// Registry of all verb handlers, keyed by their `verb_ref`.
pub struct VerbRegistry {
    handlers: HashMap<String, Box<dyn VerbHandler>>,
}

impl VerbRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler. Overwrites any previous registration for the same verb.
    pub fn register(&mut self, handler: Box<dyn VerbHandler>) {
        self.handlers
            .insert(handler.verb_ref().to_string(), handler);
    }

    /// Look up a handler by verb FQN. Returns `None` when no handler is registered.
    pub fn get(&self, verb_ref: &str) -> Option<&dyn VerbHandler> {
        self.handlers.get(verb_ref).map(|h| h.as_ref())
    }
}

impl Default for VerbRegistry {
    fn default() -> Self {
        Self::new()
    }
}
