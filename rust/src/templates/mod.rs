//! Workflow Templates for DSL Generation
//!
//! This module re-exports types from the `ob-templates` crate and provides
//! integration with the main crate's session types.
//!
//! Templates capture domain lifecycle patterns - chained verb sequences that
//! accomplish business goals. They serve as prompt enhancement for agent DSL
//! generation.
//!
//! Key concepts:
//! - Templates expand to DSL TEXT - agent sees exactly what will run
//! - Two-phase entity resolution: NL names → UUIDs in dsl_generate, then DSL pipeline
//! - Existing entity = simple verb, new entity = template

// Re-export everything from ob-templates crate
pub use ob_templates::{
    EntityDependencySummary, EntityParamInfo, ExpansionContext, ExpansionResult, MissingParam,
    OutputDefinition, ParamCardinality, ParamDefinition, PrimaryEntity, PrimaryEntityType,
    TemplateDefinition, TemplateError, TemplateExpander, TemplateMetadata, TemplateRegistry,
    WorkflowContext,
};

// Keep harness module in main crate (has heavy dsl_v2 dependencies)
#[cfg(test)]
pub mod harness;

// Extension trait for ExpansionContext integration with main crate session types
mod context_ext;
pub use context_ext::ExpansionContextExt;
