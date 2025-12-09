//! Agentic DSL Generation Module
//!
//! This module implements AI-powered DSL generation from natural language instructions.
//! It uses a pattern-based approach with deterministic requirement planning and
//! LLM API (Anthropic or OpenAI) for intent extraction and DSL generation.
//!
//! ## Architecture
//!
//! ```text
//! User Request → Intent Extraction → Pattern Classification → Requirement Planning → DSL Generation → Validation
//! ```
//!
//! - **Intent Extraction**: LLM extracts structured intent from natural language
//! - **Pattern Classification**: Deterministic classification (SimpleEquity, MultiMarket, WithOtc)
//! - **Requirement Planning**: Deterministic Rust code expands intent into complete requirements
//! - **DSL Generation**: LLM generates DSL with full schemas in context
//! - **Validation**: Parse + CSG lint with retry loop
//!
//! ## Backend Selection
//!
//! Set `AGENT_BACKEND` environment variable to switch providers:
//! - `anthropic` or `claude` (default): Use Anthropic Claude API
//! - `openai` or `gpt`: Use OpenAI API

// LLM client abstraction
pub mod anthropic_client;
pub mod backend;
pub mod client_factory;
pub mod llm_client;
pub mod openai_client;

// Core agentic modules
pub mod feedback;
pub mod generator;
pub mod intent;
pub mod orchestrator;
pub mod patterns;
pub mod planner;
pub mod validator;

// Re-export LLM client types
pub use backend::AgentBackend;
pub use client_factory::{create_llm_client, create_llm_client_with_key, current_backend};
pub use llm_client::LlmClient;

// Re-export main types
pub use intent::{
    ClientIntent, CounterpartyIntent, InstrumentIntent, MarketIntent, OnboardingIntent,
};
pub use orchestrator::{AgentOrchestrator, GenerationResult, OrchestratorBuilder};
pub use patterns::OnboardingPattern;
pub use planner::{OnboardingPlan, RequirementPlanner};
