//! Graph visualization module for CBU and UBO data
//!
//! This module provides an intermediate representation (IR) for entity graph data
//! that can be serialized to JSON and consumed by visualization clients.
//!
//! The layout engine computes node positions server-side based on view mode.
//!
//! ## Module Structure
//!
//! - `types`: Core graph types (EntityGraph, GraphNode, typed edges, etc.)
//! - `config_driven_builder`: ConfigDrivenGraphBuilder for constructing graphs from DB config
//! - `layout_v2`: LayoutEngineV2 for computing node positions from DB config

#[cfg(feature = "database")]
pub mod config_driven_builder;
#[cfg(feature = "database")]
pub mod deal_graph_builder;
pub mod layout_v2;
pub mod types;

#[cfg(feature = "database")]
pub(crate) use config_driven_builder::ConfigDrivenGraphBuilder;
#[cfg(feature = "database")]
pub(crate) use deal_graph_builder::DealGraphBuilder;
pub(crate) use layout_v2::LayoutEngineV2;
pub use types::{CbuSummary, EdgeType, GraphNode, NodeType};
pub(crate) use types::{GraphScope, RoleCategory};
