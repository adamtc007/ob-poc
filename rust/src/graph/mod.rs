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
//! - `filters`: Filter logic for visibility computation
//! - `viewport`: Viewport state (zoom, pan, visible/off-screen tracking)
//! - `view_model`: GraphViewModel - output of graph.* DSL verbs
//! - `query_engine`: GraphQueryEngine for executing graph.* verbs
//! - `config_driven_builder`: ConfigDrivenGraphBuilder for constructing graphs from DB config
//! - `layout_v2`: LayoutEngineV2 for computing node positions from DB config

#[cfg(feature = "database")]
pub mod config_driven_builder;
pub mod filters;
pub mod layout_v2;
#[cfg(feature = "database")]
pub mod query_engine;
pub mod types;
pub mod view_model;
pub mod viewport;

#[cfg(feature = "database")]
pub use config_driven_builder::{ConfigDrivenGraphBuilder, EdgeLayoutHints, NodeRenderingHints};
pub use filters::{FilterBuilder, GraphFilterOps};
pub use layout_v2::{EdgeLayoutConfig, LayoutConfigV2, LayoutEngineV2};
#[cfg(feature = "database")]
pub use query_engine::GraphQueryEngine;
pub use types::*;
pub use view_model::*;
pub use viewport::{OffScreenSummary, PanDirection, ViewportContext, ZoomName};
