//! Agentic DSL Generation Module
//!
//! This module implements AI-powered DSL generation from natural language instructions.
//! It uses a lexicon-based tokenizer with formal grammar parsing for intent classification.
//!
//! ## Architecture
//!
//! ```text
//! User Request → Tokenizer → Nom Parser → IntentAst → DSL Generation → Validation
//! ```
//!
//! ### Lexicon Pipeline
//!
//! The pipeline uses configuration-driven tokenization and grammar parsing:
//!
//! 1. **Tokenization** (`lexicon/tokenizer.rs`):
//!    - Lexicon lookup for verbs, roles, instruments, prepositions
//!    - EntityGateway lookup for counterparties, CBUs, persons
//!    - Session context for coreference resolution
//!
//! 2. **Grammar Parsing** (`lexicon/intent_parser.rs`):
//!    - Nom combinators match token patterns to intent structures
//!    - Builds typed IntentAst nodes
//!    - Handles domain detection (OTC vs Exchange-Traded)
//!
//! 3. **DSL Generation** (`lexicon/pipeline.rs`):
//!    - IntentAst → DSL source code
//!    - Entity reference resolution to UUIDs
//!    - Symbol binding generation
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

// Core agentic modules (legacy custody pipeline)
pub mod context_builder;
pub mod feedback;
pub mod generator;
pub mod intent;
pub mod orchestrator;
pub mod patterns;
pub mod planner;
pub mod validator;

// Lexicon-based pipeline (primary)
pub mod lexicon;
pub mod lexicon_agent;

// Re-export LLM client types
pub use backend::AgentBackend;
pub use client_factory::{create_llm_client, create_llm_client_with_key, current_backend};
pub use llm_client::LlmClient;

// Re-export legacy custody types
pub use intent::{
    ClientIntent, CounterpartyIntent, InstrumentIntent, MarketIntent, OnboardingIntent,
};
pub use orchestrator::{AgentOrchestrator, GenerationResult, OrchestratorBuilder};
pub use patterns::OnboardingPattern;
pub use planner::{OnboardingPlan, RequirementPlanner};

// Re-export lexicon-based pipeline (primary)
pub use lexicon_agent::{
    AgentResponse, ClarificationRequest, ExecutionResult, LexiconAgentPipeline, ResponseType,
    SessionContext,
};
