//! Inspector Projection - Deterministic projection schema for Inspector UI.
//!
//! This crate defines the wire format for Inspector visualizations:
//! - `NodeId` - Stable path-based identifiers
//! - `RefValue` - `$ref` linking for node relationships
//! - `Node` - Core node structure with 20 kinds
//! - `InspectorProjection` - Top-level envelope
//! - `RenderPolicy` - LOD, depth limits, filters
//! - Validation - Referential integrity, cycle detection
//!
//! # Architecture
//!
//! The projection is a flat map of nodes connected by `$ref` links:
//!
//! ```text
//! InspectorProjection
//! ├── snapshot (metadata)
//! ├── render_policy (LOD, filters)
//! ├── ui_hints (display options)
//! ├── root: { chamber -> $ref }
//! └── nodes: { NodeId -> Node }
//!     └── Node.branches: { name -> $ref }
//! ```
//!
//! This design enables:
//! - O(1) node lookup by ID
//! - Deterministic serialization (BTreeMap)
//! - Easy validation (all refs must resolve)
//! - Cycle detection via DFS
//!
//! # Example
//!
//! ```
//! use inspector_projection::{NodeId, RefValue, InspectorProjection};
//!
//! // Load from YAML
//! let yaml = r#"
//! snapshot:
//!   schema_version: 1
//!   source_hash: "abc123"
//!   policy_hash: "def456"
//!   created_at: "2026-02-04T00:00:00Z"
//! render_policy:
//!   lod: 2
//!   max_depth: 3
//!   max_items_per_list: 50
//! root:
//!   cbu:
//!     $ref: "cbu:test"
//! nodes:
//!   "cbu:test":
//!     id: "cbu:test"
//!     kind: CBU
//!     label_short: "Test CBU"
//! "#;
//!
//! let projection: InspectorProjection = serde_yaml::from_str(yaml).unwrap();
//! assert!(projection.nodes.contains_key(&NodeId::new("cbu:test").unwrap()));
//! ```

mod error;
pub mod generator;
mod model;
mod node_id;
mod policy;
mod ref_value;
mod validate;

// Re-exports
pub use error::ValidationError;
pub use generator::{CbuGenerator, MatrixGenerator, ProjectionGenerator};
pub use model::{
    InspectorProjection, Node, NodeKind, NodeSummary, PagingInfo, PagingList, Provenance,
    RefOrList, SnapshotMeta, UiHints,
};
pub use node_id::{NodeId, NodeIdError};
pub use policy::{PruneFilter, RenderPolicy, ShowFilter};
pub use ref_value::RefValue;
pub use validate::{validate, ValidationResult};
