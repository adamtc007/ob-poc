//! Server-side snapshot compiler for ESPER navigation.
//!
//! This crate compiles graph data (CBU, entities, relationships) into
//! compact `WorldSnapshot` structures optimized for client-side navigation.
//!
//! # Architecture
//!
//! ```text
//! Graph Data (DB) ──► GraphInput ──► SnapshotCompiler ──► WorldSnapshot
//!                                          │
//!                                          ├── Layout algorithms
//!                                          ├── String table dedup
//!                                          ├── Navigation index build
//!                                          └── Grid spatial index
//! ```
//!
//! # Key Concepts
//!
//! - **GraphInput**: Abstract interface for graph data sources
//! - **LayoutEngine**: Computes positions for entities
//! - **SnapshotCompiler**: Orchestrates the compilation pipeline
//! - **CacheKey**: Content-addressed cache invalidation
//!
//! # Example
//!
//! ```ignore
//! use esper_compiler::{SnapshotCompiler, MemoryGraphInput};
//!
//! let input = MemoryGraphInput::new()
//!     .add_entity(EntityInput { id: 1, name: "Acme Corp", kind: 1, .. })
//!     .add_edge(1, 2, EdgeKind::Parent);
//!
//! let compiler = SnapshotCompiler::new();
//! let snapshot = compiler.compile(&input, CompilerConfig::default())?;
//! ```

mod cache;
mod compiler;
mod config;
mod error;
mod input;
mod layout;
mod string_table;

pub use cache::{CacheKey, SnapshotCache};
pub use compiler::SnapshotCompiler;
pub use config::{ChamberConfig, CompilerConfig, LayoutConfig};
pub use error::CompilerError;
pub use input::{EdgeInput, EdgeKind, EntityInput, GraphInput, MemoryGraphInput};
pub use layout::{LayoutAlgorithm, LayoutEngine, Position};
pub use string_table::StringTableBuilder;

/// Maximum entities per chamber before splitting.
pub const MAX_ENTITIES_PER_CHAMBER: usize = 10_000;

/// Default grid cell size for spatial indexing.
pub const DEFAULT_GRID_CELL_SIZE: f32 = 50.0;

/// Default viewport padding.
pub const DEFAULT_VIEWPORT_PADDING: f32 = 100.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_reasonable() {
        assert!(MAX_ENTITIES_PER_CHAMBER > 0);
        assert!(DEFAULT_GRID_CELL_SIZE > 0.0);
        assert!(DEFAULT_VIEWPORT_PADDING >= 0.0);
    }
}
