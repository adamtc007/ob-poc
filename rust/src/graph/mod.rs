//! Graph visualization module for CBU data
//!
//! This module provides an intermediate representation (IR) for CBU graph data
//! that can be serialized to JSON and consumed by visualization clients.
//!
//! The layout engine computes node positions server-side based on view mode.
//!
//! ## Module Structure
//!
//! - `types`: Core graph types (GraphNode, GraphEdge, EdgeType, etc.)
//! - `view_model`: GraphViewModel - output of graph.* DSL verbs
//! - `query_engine`: GraphQueryEngine for executing graph.* verbs
//! - `builder`: CbuGraphBuilder for constructing graphs from DB data
//! - `layout`: LayoutEngine for computing node positions

pub mod builder;
pub mod layout;
#[cfg(feature = "database")]
pub mod query_engine;
pub mod types;
pub mod view_model;

pub use builder::CbuGraphBuilder;
pub use layout::{LayoutConfig, LayoutEngine, Orientation, ViewMode};
#[cfg(feature = "database")]
pub use query_engine::GraphQueryEngine;
pub use types::*;
pub use view_model::*;
