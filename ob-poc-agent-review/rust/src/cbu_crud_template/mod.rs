//! CBU CRUD Template Module
//!
//! Generates and manages CRUD templates from CBU Model DSL specifications.
//! Templates are parametrized "recipes" for creating/updating CBU instances.

mod service;

pub use service::{
    CbuCrudTemplate, CbuCrudTemplateService, DslDocSource, TemplateGenerationResult,
};
