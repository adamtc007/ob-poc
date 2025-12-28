//! Agentic DSL Generation Module
//!
//! This module implements AI-powered DSL generation from natural language instructions.
//! It uses a lexicon-based tokenizer with formal grammar parsing for intent classification.
//!
//! Most functionality is in the `ob-agentic` crate (no DB dependencies).
//! The orchestrator module stays here as it requires the DB executor.

// Re-export everything from ob-agentic crate
pub use ob_agentic::anthropic_client;
pub use ob_agentic::backend;
pub use ob_agentic::client_factory;
pub use ob_agentic::context_builder;
pub use ob_agentic::feedback;
pub use ob_agentic::generator;
pub use ob_agentic::intent;
pub use ob_agentic::lexicon;
pub use ob_agentic::llm_client;
pub use ob_agentic::openai_client;
pub use ob_agentic::patterns;
pub use ob_agentic::planner;
pub use ob_agentic::validator;

// Orchestrator stays local (has DB dependencies)
pub mod orchestrator;

// Lexicon agent requires gateway feature in ob-agentic
#[cfg(feature = "database")]
pub mod lexicon_agent {
    pub use ob_agentic::lexicon_agent::*;
}

// Re-export LLM client types
pub use client_factory::{create_llm_client_with_key, current_backend};
pub use ob_agentic::create_llm_client;
pub use ob_agentic::AgentBackend;
pub use ob_agentic::LlmClient;

// Re-export intent types
pub use intent::{ClientIntent, CounterpartyIntent, InstrumentIntent, MarketIntent};
pub use ob_agentic::intent::{
    ClarificationRequest as IntentClarificationRequest, IntentResult, OnboardingIntent,
};

// Re-export orchestrator
pub use orchestrator::{AgentOrchestrator, GenerationResult, OrchestratorBuilder};

// Re-export patterns and planner
pub use ob_agentic::patterns::OnboardingPattern;
pub use ob_agentic::planner::{OnboardingPlan, RequirementPlanner};
