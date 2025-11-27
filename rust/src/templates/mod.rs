//! Template System for Structured DSL Generation
//!
//! Templates define form structures with typed slots.
//! EntityRef slots use the EntitySearchService for typeahead.

pub mod registry;
pub mod renderer;
pub mod slot_types;

pub use registry::TemplateRegistry;
pub use renderer::{RenderError, TemplateRenderer};
pub use slot_types::{EnumOption, FormTemplate, RefScope, SlotDefinition, SlotType};
