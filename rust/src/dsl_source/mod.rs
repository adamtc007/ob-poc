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
//! NL/Templates/Builder → DSL Text → NOM Parser → AST → VM → CRUD
//! ```
//!
//! # Modules
//!
//! - `generator` - Programmatic DSL generation (builders, templates, domains)
//! - `agentic` - LLM-powered generation with RAG context
//! - `editor` - DSL manipulation (transform, merge, format)
//! - `validation` - Pre-execution validation pipeline
//! - `context` - Context providers for generation

// Core builder API
pub mod generator;
pub use generator::{DslBuilder, DslTemplate};

// Agentic (LLM-powered) generation
pub mod agentic;
pub use agentic::{AgenticGenerator, RagContext, RagContextProvider, GeneratedDsl};

// DSL editing and transformation
pub mod editor;
pub use editor::{DslFormatter};

// Validation pipeline
pub mod validation;
pub use validation::{ValidationPipeline, ValidationResult, ValidationStage, ValidationError};

// Context providers
pub mod context;
pub use context::{VocabularyProvider, AttributeProvider};

// Attribute value sources
pub mod sources;
pub use sources::{DocumentSource, ExtractionDsl, AttributeSource, SourceContext, SourceError};
