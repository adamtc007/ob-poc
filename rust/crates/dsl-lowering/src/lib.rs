//! dsl-lowering: Lowering passes from assembled IR to executable forms.
//!
//! Provides two lowering targets:
//! - `bpmn` — lowers a `RailwayGraph` to a `JourneySpec` (bpmn-lite)
//! - SemOS lowering (Tranche 6+, not yet implemented)
#![deny(unreachable_pub)]

mod bpmn;

pub use bpmn::{
    lower, JourneyBoundaryAttachment, JourneyEdge, JourneyMergeClause, JourneyNode,
    JourneyParallelJoin, JourneySpec,
};
