//! Snapshot error types.

use thiserror::Error;

/// Errors that can occur when working with snapshots.
#[derive(Debug, Error)]
pub enum SnapshotError {
    /// SoA array lengths don't match.
    #[error("Array length mismatch: expected {expected}, got {actual} for {field}")]
    ArrayLengthMismatch {
        expected: usize,
        actual: usize,
        field: &'static str,
    },

    /// Navigation index out of bounds.
    #[error("Navigation index {index} out of bounds (max {max}) in {field}")]
    IndexOutOfBounds {
        index: u32,
        max: usize,
        field: &'static str,
    },

    /// Entity ID is zero (reserved sentinel).
    #[error("Entity ID at index {index} is zero (reserved sentinel)")]
    InvalidEntityId { index: usize },

    /// String table index out of bounds.
    #[error("String table index {index} out of bounds (table size {size})")]
    InvalidStringId { index: u32, size: usize },

    /// Door references non-existent chamber.
    #[error("Door {door_id} references non-existent chamber {chamber_id}")]
    InvalidDoorTarget { door_id: u32, chamber_id: u32 },

    /// Grid cell range out of bounds.
    #[error("Grid cell range ({start}, {count}) exceeds entity indices length {len}")]
    InvalidGridCellRange { start: u32, count: u32, len: usize },

    /// Chamber count exceeds limit.
    #[error("Entity count {count} exceeds maximum {max}")]
    TooManyEntities { count: usize, max: usize },

    /// Serialization failed.
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),

    /// Deserialization failed.
    #[error("Deserialization failed: {0}")]
    DeserializationFailed(String),

    /// Schema version mismatch.
    #[error("Schema version mismatch: expected {expected}, got {actual}")]
    SchemaVersionMismatch { expected: u32, actual: u32 },
}
