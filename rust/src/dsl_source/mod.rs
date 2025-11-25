//! DSL Source Library - Generation, editing, and manipulation of DSL source text
//!
//! This is the "write side" of the DSL system. For parsing and execution,
//! see the `forth_engine` module.
//!
//! # Architecture
//!
//! ```text
//! DSL Source Library              Forth Engine Library
//! (Generation/Editing)     →      (Parsing/Execution)
//!
//! NL/Templates/Builder → DSL Text → NOM Parser → AST → Runtime → CRUD
//! ```
//!
//! # Modules
//!
//! - `generator` - Programmatic DSL generation (builders, templates, domains)
//! - `agentic` - LLM-powered generation with RAG context
//! - `orchestrator` - Single entry point: prompt → generate → validate → execute
//! - `editor` - DSL manipulation (transform, merge, format)
//! - `validation` - Pre-execution validation pipeline
//! - `context` - Context providers for generation

// Core builder API
pub mod generator;
pub use generator::{DslBuilder, DslTemplate};

// Agentic (LLM-powered) generation
pub mod agentic;
pub use agentic::{AgenticGenerator, GeneratedDsl, RagContext, RagContextProvider};

// Orchestrator - single entry point for agentic DSL generation
pub mod orchestrator;
pub use orchestrator::{AgenticOrchestrator, AgenticResult, OrchestratorConfig};

// DSL editing and transformation
pub mod editor;
pub use editor::DslFormatter;

// Validation pipeline
pub mod validation;
pub use validation::{ValidationError, ValidationPipeline, ValidationResult, ValidationStage};

// Context providers
pub mod context;
pub use context::{AttributeProvider, VocabularyProvider};

// Attribute value sources
pub mod sources;
pub use sources::{AttributeSource, DocumentSource, ExtractionDsl, SourceContext, SourceError};
