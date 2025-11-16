//! Taxonomy module for Product-Service-Resource management
//!
//! This module provides DSL operations and management for the complete
//! product-service-resource taxonomy system.

pub mod manager;
pub mod operations;

pub use manager::TaxonomyDslManager;
pub use operations::{DslOperation, DslResult};
