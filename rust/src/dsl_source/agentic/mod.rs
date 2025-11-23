//! Agentic DSL Generation - LLM-powered generation with RAG context

pub mod rag_context;
pub mod llm_generator;
pub mod providers;

pub use rag_context::{RagContext, RagContextProvider, VocabEntry, DslExample, AttributeDefinition};
pub use llm_generator::{LlmDslGenerator, GeneratedDsl, GeneratorConfig};
pub use providers::{MultiProviderLlm, LlmProvider, ProviderConfig, LlmResponse};

/// Convenience type alias
pub type AgenticGenerator = LlmDslGenerator;
