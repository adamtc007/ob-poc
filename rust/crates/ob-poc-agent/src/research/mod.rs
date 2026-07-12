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
//! - `sources`: Pluggable source loaders (GLEIF, Companies House, SEC EDGAR)

pub mod definition;
pub mod error;
pub mod executor;
pub mod llm_client;
pub mod registry;
pub mod sources;

// Re-exports for convenience
pub use definition::ReviewRequirement;
pub use executor::{ApprovedResearch, ResearchExecutor, ResearchResult};
pub use llm_client::ClaudeResearchClient;
pub use registry::ResearchMacroRegistry;

// Source loader re-exports
