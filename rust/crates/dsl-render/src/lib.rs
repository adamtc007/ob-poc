//! dsl-render: Read-only SVG renderer for bpmn-lite DSL railway graphs.
//!
//! Produces SVG strings from bpmn-lite DSL source or pre-assembled
//! `RailwayGraph` values. Intended for compliance-officer review,
//! documentation, and the Sage confirmation panel.
//!
//! # Quick start
//!
//! ```rust,ignore
//! let svg = dsl_render::render(r#"
//!   (node start :kind start-event)
//!   (node task :kind service-task)
//!   (node end :kind end-event)
//!   (flow start -> task)
//!   (flow task -> end)
//! "#).unwrap();
//! ```
//!
//! # Crate layout
//!
//! - `layout` — BFS topological layout (column depth from start node)
//! - `shapes` — SVG shape primitives, layout types
//! - `renderer` — main `render_dsl` / `render_graph` functions
//! - `style` — embedded CSS styles
#![deny(unreachable_pub)]

pub mod layout;
pub mod renderer;
pub mod shapes;
pub mod style;

pub use renderer::{render_dsl, render_graph, RenderOptions};

/// Render bpmn-lite DSL source to an SVG string using default options.
pub fn render(dsl_source: &str) -> anyhow::Result<String> {
    render_dsl(dsl_source, &RenderOptions::default())
}
