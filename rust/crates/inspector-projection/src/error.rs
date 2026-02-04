//! Validation error types.

use crate::node_id::NodeId;
use thiserror::Error;

/// Validation errors for Inspector projections.
#[derive(Debug, Clone, Error)]
pub enum ValidationError {
    /// A `$ref` target does not exist in the nodes map.
    #[error("Dangling reference: {source_node} -> {target} (target not found in nodes)")]
    DanglingRef {
        /// Node containing the broken reference.
        source_node: NodeId,
        /// Target that doesn't exist.
        target: NodeId,
    },

    /// A root reference points to a missing node.
    #[error("Missing root node: chamber '{chamber}' references '{target}' which doesn't exist")]
    MissingRoot {
        /// Chamber name.
        chamber: String,
        /// Target NodeId that doesn't exist.
        target: NodeId,
    },

    /// A cycle was detected in the node graph.
    #[error("Cycle detected in node graph: {}", format_cycle(path))]
    CycleDetected {
        /// Path of nodes forming the cycle.
        path: Vec<NodeId>,
    },

    /// Node ID in the map doesn't match the node's id field.
    #[error("Node ID mismatch: map key '{key}' != node.id '{node_id}'")]
    IdMismatch {
        /// Map key.
        key: NodeId,
        /// Node's id field.
        node_id: NodeId,
    },

    /// Required provenance is missing.
    #[error("Missing provenance: node '{node_id}' (kind: {kind:?}) requires provenance")]
    MissingProvenance {
        /// Node missing provenance.
        node_id: NodeId,
        /// Node's kind.
        kind: crate::model::NodeKind,
    },

    /// Provenance sources array is empty.
    #[error("Empty provenance sources: node '{node_id}' has provenance but sources is empty")]
    EmptyProvenanceSources {
        /// Node with empty sources.
        node_id: NodeId,
    },

    /// Provenance asserted_at is missing.
    #[error("Missing asserted_at: node '{node_id}' has provenance but asserted_at is empty")]
    MissingAssertedAt {
        /// Node missing asserted_at.
        node_id: NodeId,
    },

    /// Confidence value is out of range.
    #[error("Invalid confidence: node '{node_id}' has confidence {value} (must be 0.0-1.0)")]
    InvalidConfidence {
        /// Node with invalid confidence.
        node_id: NodeId,
        /// Invalid value.
        value: f64,
    },

    /// Schema version is not supported.
    #[error("Unsupported schema version: {version} (max supported: {max_supported})")]
    UnsupportedSchemaVersion {
        /// Version in the projection.
        version: u32,
        /// Maximum supported version.
        max_supported: u32,
    },
}

fn format_cycle(path: &[NodeId]) -> String {
    path.iter()
        .map(|id| id.as_str())
        .collect::<Vec<_>>()
        .join(" -> ")
}

impl ValidationError {
    /// Get an error code for this error type.
    pub fn code(&self) -> &'static str {
        match self {
            Self::DanglingRef { .. } => "DANGLING_REF",
            Self::MissingRoot { .. } => "MISSING_ROOT",
            Self::CycleDetected { .. } => "CYCLE_DETECTED",
            Self::IdMismatch { .. } => "ID_MISMATCH",
            Self::MissingProvenance { .. } => "MISSING_PROVENANCE",
            Self::EmptyProvenanceSources { .. } => "EMPTY_PROVENANCE_SOURCES",
            Self::MissingAssertedAt { .. } => "MISSING_ASSERTED_AT",
            Self::InvalidConfidence { .. } => "INVALID_CONFIDENCE",
            Self::UnsupportedSchemaVersion { .. } => "UNSUPPORTED_SCHEMA_VERSION",
        }
    }

    /// Check if this is a blocking error (vs warning).
    pub fn is_blocking(&self) -> bool {
        !matches!(self, Self::CycleDetected { .. })
    }
}
