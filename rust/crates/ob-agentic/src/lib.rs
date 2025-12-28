//! LLM-powered agent for DSL generation
//!
//! This crate provides AI-powered DSL generation from natural language instructions.
//! It has no database dependencies - orchestration with DB execution stays in ob-poc.
//!
//! ## Architecture
//!
//! ```text
//! User Request → Tokenizer → Nom Parser → IntentAst → DSL Generation
//! ```
//!
//! ## Backend Selection
//!
//! Set `AGENT_BACKEND` environment variable:
//! - `anthropic` (default): Anthropic Claude API
//! - `openai`: OpenAI API

// LLM client abstraction
pub mod anthropic_client;
pub mod backend;
pub mod client_factory;
pub mod llm_client;
pub mod openai_client;

// Core agentic modules
pub mod context_builder;
pub mod feedback;
pub mod generator;
pub mod intent;
pub mod patterns;
pub mod planner;
pub mod validator;

// Lexicon-based pipeline
pub mod lexicon;

#[cfg(feature = "gateway")]
pub mod lexicon_agent;

// Re-exports for convenience
pub use backend::AgentBackend;
pub use client_factory::create_llm_client;
pub use intent::{ClarificationRequest, IntentResult, OnboardingIntent};
pub use lexicon::IntentAst;
pub use llm_client::LlmClient;
