//! REST API module for agentic operations
//!
//! This module provides HTTP endpoints for the agentic DSL system,
//! allowing external clients to interact with the system via REST API.
//!
//! ## Intent-Based Architecture
//!
//! The API uses an intent-based pipeline:
//! 1. LLM extracts structured intents (JSON) from natural language
//! 2. Rust validates intents against verb registry
//! 3. Rust assembles valid s-expression DSL deterministically

#[cfg(feature = "server")]
pub mod attribute_routes;

#[cfg(feature = "server")]
pub mod agent_routes;

#[cfg(feature = "server")]
pub mod intent;

#[cfg(feature = "server")]
pub mod session;

#[cfg(feature = "server")]
pub mod intent_extractor;

#[cfg(feature = "server")]
pub mod dsl_assembler;

#[cfg(feature = "server")]
pub use attribute_routes::create_attribute_router;

#[cfg(feature = "server")]
pub use agent_routes::create_agent_router;

#[cfg(feature = "server")]
pub use session::{create_session_store, SessionStore};

#[cfg(feature = "server")]
pub use intent::{AssembledDsl, IntentSequence, VerbIntent};

#[cfg(feature = "server")]
pub use intent_extractor::IntentExtractor;

#[cfg(feature = "server")]
pub use dsl_assembler::DslAssembler;
