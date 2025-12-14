//! Workflow Orchestration Layer
//!
//! Provides stateful workflow tracking for KYC, UBO, and onboarding processes.
//! Workflows are defined in YAML and executed by a state machine engine.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                           WORKFLOW ENGINE                               │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
//! │  │  Workflow   │  │   State     │  │ Transition  │  │  Blocker    │    │
//! │  │ Definition  │  │  Tracker    │  │   Guard     │  │  Resolver   │    │
//! │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘    │
//! └─────────────────────────────────────────────────────────────────────────┘
//!                                     │
//!                                     ▼
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                           DSL EXECUTION                                 │
//! │              Workflow emits DSL → Executor runs → Results fed back      │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Concepts
//!
//! - **WorkflowDefinition**: YAML-defined state machine with states, transitions, guards
//! - **WorkflowInstance**: Running instance of a workflow for a specific subject
//! - **Guard**: Condition that must be met before a transition can occur
//! - **Blocker**: Actionable item preventing workflow advancement (with resolution DSL)
//!
//! # Example Usage
//!
//! ```ignore
//! let engine = WorkflowEngine::new(pool, definitions);
//!
//! // Start a new workflow
//! let instance = engine.start_workflow("kyc_onboarding", "cbu", cbu_id, None).await?;
//!
//! // Get status with blockers
//! let status = engine.get_status(instance.instance_id).await?;
//! println!("Current state: {}", status.current_state);
//! println!("Blockers: {:?}", status.blockers);
//!
//! // Try to advance (evaluates guards, auto-transitions if possible)
//! let instance = engine.try_advance(instance.instance_id).await?;
//! ```

mod definition;
mod engine;
mod guards;
mod repository;
mod requirements;
mod state;

pub use definition::{
    ActionDef, RequirementDef, StateDef, TransitionDef, TriggerDef, WorkflowDefinition,
    WorkflowLoader,
};
pub use engine::{
    AvailableAction, AvailableTransition, GuardStatus, WorkflowEngine, WorkflowStatus,
};
pub use guards::{GuardEvaluator, GuardResult};
pub use repository::WorkflowRepository;
pub use requirements::RequirementEvaluator;
pub use state::{Blocker, BlockerType, StateTransition, WorkflowInstance};

/// Workflow-related errors
#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error("Unknown workflow: {0}")]
    UnknownWorkflow(String),

    #[error("No initial state defined for workflow")]
    NoInitialState,

    #[error("Invalid transition from '{from}' to '{to}'")]
    InvalidTransition { from: String, to: String },

    #[error("Guard '{guard}' failed: {blockers:?}")]
    GuardFailed {
        guard: String,
        blockers: Vec<Blocker>,
    },

    #[error("Workflow instance not found: {0}")]
    InstanceNotFound(uuid::Uuid),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("JSON serialization error: {0}")]
    JsonSerialize(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
