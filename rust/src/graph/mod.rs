//! Graph visualization module for CBU data
//!
//! This module provides an intermediate representation (IR) for CBU graph data
//! that can be serialized to JSON and consumed by visualization clients.

pub mod builder;
pub mod types;

pub use builder::CbuGraphBuilder;
pub use types::*;
