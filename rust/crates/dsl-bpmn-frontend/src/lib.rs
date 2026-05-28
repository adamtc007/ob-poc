//! dsl-bpmn-frontend: BPMN-Lite frontend for the unified DSL v0.1.
//!
//! This crate lowers typed `AtomBag` atoms (from `dsl-ast`) into the
//! bpmn-lite railway graph intermediate representation.
//!
//! # Public API
//!
//! - [`assemble`] — main entry point; converts an `AtomBag` into a
//!   [`RailwayGraph`] and accumulates diagnostics.
//!
//! # Crate layout
//!
//! - `railway` — typed graph types (`RailwayGraph`, `RailwayNode`, etc.)
//! - `assembly` — the assembly pass implementation

pub(crate) mod assembly;
pub(crate) mod railway;

pub use assembly::{
    assemble, DUPLICATE_NAME, GATEWAY_FAN_OUT_ERROR, INVALID_BOUNDARY_TARGET, UNREACHABLE_NODE,
    UNTERMINATED_PATH,
};
pub use railway::{
    BoundaryAttachmentEntry, GatewayKind, MergeClause, MergeOperator, NodeKind, ParallelJoinEntry,
    RailwayEdge, RailwayGateway, RailwayGraph, RailwayNode,
};
