//! Graph visualization module for CBU data
//!
//! This module provides an intermediate representation (IR) for CBU graph data
//! that can be serialized to JSON and consumed by visualization clients.
//!
//! The layout engine computes node positions server-side based on view mode.

pub mod builder;
pub mod layout;
pub mod types;

pub use builder::CbuGraphBuilder;
pub use layout::{LayoutConfig, LayoutEngine, ViewMode};
pub use types::*;
