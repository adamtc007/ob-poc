//! Research Macro System
//!
//! Bridges fuzzy LLM discovery → human review → deterministic GLEIF DSL verbs.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
//! │ Research Macro  │ ──► │  Human Review   │ ──► │  GLEIF Verbs    │
//! │ (LLM + search)  │     │  (approve/edit) │     │  (deterministic)│
//! └─────────────────┘     └─────────────────┘     └─────────────────┘
//!       fuzzy                   gate                   100% reliable
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use ob_poc::research::{ResearchMacroRegistry, ResearchExecutor, ClaudeResearchClient};
//! use std::collections::HashMap;
//!
//! // Load macros from config
//! let registry = ResearchMacroRegistry::load_from_dir("config/macros/research".as_ref()).unwrap();
//!
//! // Create executor with LLM client
//! let client = ClaudeResearchClient::from_env().unwrap();
//! let executor = ResearchExecutor::new(registry, client);
//!
//! // Execute a research macro
//! let params = HashMap::from([
//!     ("client_name".to_string(), serde_json::json!("Allianz")),
//! ]);
//! // let result = executor.execute("client-discovery", params).await?;
//! ```
//!
//! # Modules
//!
//! - `definition`: Types for macro definitions loaded from YAML
//! - `registry`: Macro registry with YAML loading and search
//! - `executor`: Research execution with LLM and validation
//! - `llm_client`: LLM client trait with tool use support
//! - `error`: Error types

pub mod agent_controller;
pub mod definition;
pub mod error;
pub mod executor;
pub mod llm_client;
pub mod registry;

// Re-exports for convenience
pub use agent_controller::{
    AgentController, AgentEvent, CheckpointResponse, ConfidenceConfig, StrategyResult,
};
pub use definition::{MacroParamDef, ResearchMacroDef, ResearchOutput, ReviewRequirement};
pub use error::{ResearchError, Result};
pub use executor::{ApprovedResearch, ResearchExecutor, ResearchResult, SearchQuality};
pub use llm_client::{
    ClaudeResearchClient, LlmResponse, ResearchLlmClient, ResearchSource, ToolCall, ToolDef,
};
pub use registry::ResearchMacroRegistry;
