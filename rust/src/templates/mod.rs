//! Template System for Structured DSL Generation
//!
//! Templates define form structures with typed slots.
//! EntityRef slots use the EntitySearchService for typeahead.
//!
//! ## Onboarding Templates
//!
//! Multi-statement templates for product onboarding workflows:
//! - Global Custody
//! - Fund Accounting
//! - Middle Office IBOR

pub mod onboarding;
pub mod registry;
pub mod renderer;
pub mod slot_types;

pub use onboarding::{OnboardingRenderer, OnboardingTemplate, OnboardingTemplateRegistry};
pub use registry::TemplateRegistry;
pub use renderer::{RenderError, TemplateRenderer};
pub use slot_types::{EnumOption, FormTemplate, RefScope, SlotDefinition, SlotType};
