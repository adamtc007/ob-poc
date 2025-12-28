//! Workflow Templates for DSL Generation
//!
//! Templates capture domain lifecycle patterns - chained verb sequences that
//! accomplish business goals. They serve as prompt enhancement for agent DSL
//! generation.
//!
//! Key concepts:
//! - Templates expand to DSL TEXT - agent sees exactly what will run
//! - Two-phase entity resolution: NL names → UUIDs in dsl_generate, then DSL pipeline
//! - Existing entity = simple verb, new entity = template
//!
//! # Example
//!
//! ```yaml
//! template: onboard-director
//! metadata:
//!   name: Onboard Director
//!   summary: Add a natural person as director with full KYC setup
//! params:
//!   cbu_id:
//!     type: cbu_ref
//!     source: session
//!   name:
//!     type: string
//!     required: true
//!     prompt: "Director's full legal name"
//! body: |
//!   (let [person (entity.create-proper-person :name "$name" ...)]
//!     (cbu.assign-role :cbu "$cbu_id" :entity person :role DIRECTOR)
//!     ...)
//! ```

mod definition;
mod error;
mod expander;
mod registry;

pub use definition::{
    EntityDependencySummary, EntityParamInfo, OutputDefinition, ParamCardinality, ParamDefinition,
    PrimaryEntity, PrimaryEntityType, TemplateDefinition, TemplateMetadata, WorkflowContext,
};
pub use error::TemplateError;
pub use expander::{ExpansionContext, ExpansionResult, MissingParam, TemplateExpander};
pub use registry::TemplateRegistry;
