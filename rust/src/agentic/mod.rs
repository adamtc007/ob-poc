//! Agentic DSL Generation Module
//!
//! This module implements AI-powered DSL generation from natural language instructions.
//! It uses a pattern-based approach with deterministic requirement planning and
//! Claude API for intent extraction and DSL generation.
//!
//! ## Architecture
//!
//! ```text
//! User Request → Intent Extraction → Pattern Classification → Requirement Planning → DSL Generation → Validation
//! ```
//!
//! - **Intent Extraction**: Claude extracts structured intent from natural language
//! - **Pattern Classification**: Deterministic classification (SimpleEquity, MultiMarket, WithOtc)
//! - **Requirement Planning**: Deterministic Rust code expands intent into complete requirements
//! - **DSL Generation**: Claude generates DSL with full schemas in context
//! - **Validation**: Parse + CSG lint with retry loop

pub mod feedback;
pub mod generator;
pub mod intent;
pub mod orchestrator;
pub mod patterns;
pub mod planner;
pub mod validator;

// Re-export main types
pub use intent::{
    ClientIntent, CounterpartyIntent, InstrumentIntent, MarketIntent, OnboardingIntent,
};
pub use orchestrator::{AgentOrchestrator, GenerationResult, OrchestratorBuilder};
pub use patterns::OnboardingPattern;
pub use planner::{OnboardingPlan, RequirementPlanner};
