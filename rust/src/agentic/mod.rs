//! Agentic DSL Generation Module
//!
//! This module implements AI-powered DSL generation from natural language instructions.
//! It uses a pattern-based approach with deterministic requirement planning and
//! LLM API (Anthropic or OpenAI) for intent extraction and DSL generation.
//!
//! ## Architecture
//!
//! ```text
//! User Request → Intent Classification → Entity Extraction → DSL Generation → Validation
//! ```
//!
//! ### Phase 3 Agent Intelligence Pipeline
//!
//! The new pipeline (Phase 3) uses configuration-driven classification and extraction:
//!
//! 1. **Intent Classification** (`intent_classifier.rs`):
//!    - Pattern-based matching using trigger phrases from `intent_taxonomy.yaml`
//!    - Context-aware re-ranking based on conversation history
//!    - Compound intent detection for multi-action utterances
//!
//! 2. **Entity Extraction** (`entity_extractor.rs`):
//!    - Regex pattern matching for structured values (MICs, currencies, LEIs)
//!    - Lookup table matching for known entities
//!    - Category expansion (e.g., "European" → list of MIC codes)
//!    - Coreference resolution for pronouns and anaphora
//!
//! 3. **DSL Generation** (`dsl_generator.rs`):
//!    - Intent → Verb mapping using canonical verb from taxonomy
//!    - Entity → Parameter mapping using `parameter_mappings.yaml`
//!    - Default value inference from context and configuration
//!    - Symbol generation for entity captures
//!
//! ### Legacy Pipeline
//!
//! The original pipeline still exists for custody onboarding scenarios:
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

// Core agentic modules (legacy pipeline)
pub mod context_builder;
pub mod feedback;
pub mod generator;
pub mod intent;
pub mod orchestrator;
pub mod patterns;
pub mod planner;
pub mod validator;

// Phase 3: Configuration-driven agent intelligence
pub mod dsl_generator;
pub mod entity_extractor;
pub mod entity_types;
pub mod instrument_hierarchy;
pub mod intent_classifier;
pub mod market_regions;
pub mod pipeline;
pub mod taxonomy;

#[cfg(test)]
mod pipeline_tests;

// Re-export LLM client types
pub use backend::AgentBackend;
pub use client_factory::{create_llm_client, create_llm_client_with_key, current_backend};
pub use llm_client::LlmClient;

// Re-export legacy types
pub use intent::{
    ClientIntent, CounterpartyIntent, InstrumentIntent, MarketIntent, OnboardingIntent,
};
pub use orchestrator::{AgentOrchestrator, GenerationResult, OrchestratorBuilder};
pub use patterns::OnboardingPattern;
pub use planner::{OnboardingPlan, RequirementPlanner};

// Re-export Phase 3 types
pub use dsl_generator::{
    DslGenerator, DslStatement, GeneratedDsl, GenerationContext, ParameterMappingsConfig,
};
pub use entity_extractor::{EntityExtractor, ExtractedEntities, ExtractedEntity};
pub use entity_types::EntityTypesConfig;
pub use instrument_hierarchy::InstrumentHierarchyConfig;
pub use intent_classifier::{
    ClassificationResult, ClassifiedIntent, ConversationContext, ExecutionDecision,
    IntentClassifier,
};
pub use market_regions::MarketRegionsConfig;
pub use pipeline::{AgentPipeline, AgentResponse, PipelineError, ResponseType, SessionContext};
pub use taxonomy::IntentTaxonomy;
