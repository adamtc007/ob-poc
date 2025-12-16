//! OB-POC Graph Widget
//!
//! This crate contains ONLY the graph widget - no API, no app shell.
//! The widget is used by ob-poc-ui which owns the API and app lifecycle.

pub mod graph;

pub use graph::{CbuGraphData, CbuGraphWidget, GraphEdgeData, GraphNodeData, ViewMode};
