//! DSL Core Module - Clean Refactored Architecture
//!
//! This module contains the core DSL processing capabilities following the
//! proven call chain architecture from the independent implementation blueprint.
//!
//! ## Clean Architecture
//! - **Pipeline Processor**: Clean 4-step DSL processing pipeline
//! - **Domain Snapshot**: Domain-aware state management
//! - **Processing Metrics**: Performance tracking
//!
//! ## 4-Step Processing Pipeline
//! 1. **DSL Change** - Validate operation input
//! 2. **AST Parse/Validate** - Parse DSL and validate syntax/semantics
//! 3. **DSL Domain Snapshot Save** - Save domain state snapshot
//! 4. **AST Dual Commit** - Commit both DSL state and parsed AST

// Core pipeline processor
pub mod pipeline_processor;

// Orchestration interface for DSL Manager to DSL Mod communication
pub mod orchestration_interface;

// DSL operations definitions
pub mod operations;

// DSL Generation Domain Library (Phase 1.5)
pub mod generation;

// Re-export the pipeline processor components
pub use pipeline_processor::{
    DomainSnapshot, DslPipelineProcessor, DslPipelineResult, PipelineConfig, ProcessingMetrics,
    StepResult,
};

// Re-export orchestration interface components
pub use orchestration_interface::{
    DslOrchestrationInterface, ExecutionResult, HealthStatus, OrchestrationContext,
    OrchestrationOperation, OrchestrationOperationType, OrchestrationResult, ParseResult,
    ProcessingOptions, SessionInfo, TransformationResult, TransformationType, ValidationReport,
};

// Re-export operations components
pub use operations::{DslOperationType, ExecutableDslOperation, OperationContext, OperationResult};

// Re-export generation components
pub use generation::{
    AiGenerator, DslGenerator, GenerationFactory, GenerationRequest, GenerationResponse,
    GenerationResult, GeneratorType, HybridGenerator, TemplateGenerator,
};
