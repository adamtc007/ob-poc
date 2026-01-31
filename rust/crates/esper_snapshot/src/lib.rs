//! ESPER Navigation Snapshot Format
//!
//! This crate defines the wire format for navigation snapshots - the compiled
//! representation of graph data that both server (compiler) and client (egui)
//! consume.
//!
//! # Architecture
//!
//! The snapshot uses Structure-of-Arrays (SoA) layout for cache-friendly
//! iteration and O(1) navigation via precomputed indices.
//!
//! # Sentinel Constants
//!
//! SoA arrays use sentinel values instead of `Option<T>` to avoid branching:
//! - `NONE_IDX` (u32::MAX) - No navigation link
//! - `NONE_ID` (0) - No entity ID / no detail reference

mod chamber;
mod door;
mod error;
pub mod grid;
mod types;
mod validate;

pub use chamber::{CameraPreset, ChamberKind, ChamberSnapshot};
pub use door::{DoorKind, DoorSnapshot};
pub use error::SnapshotError;
pub use grid::GridSnapshot;
pub use types::{Rect, SnapshotEnvelope, Vec2, WorldSnapshot};
pub use validate::Validate;

// =============================================================================
// SENTINEL CONSTANTS
// =============================================================================

/// Navigation index sentinel: no link exists.
///
/// Used in `first_child`, `next_sibling`, `prev_sibling` arrays to indicate
/// that no navigation link exists at this position.
///
/// # Example
///
/// ```
/// use esper_snapshot::NONE_IDX;
///
/// let next_sibling = vec![1, 2, NONE_IDX]; // Entity 2 has no next sibling
/// assert_eq!(next_sibling[2], u32::MAX);
/// ```
pub const NONE_IDX: u32 = u32::MAX;

/// Entity ID sentinel: no entity / no detail reference.
///
/// Used in `detail_refs` array to indicate that no detail data exists
/// for this entity (e.g., it's a placeholder or aggregate node).
///
/// # Important
///
/// All entity IDs in `entity_ids` arrays MUST be non-zero. Zero is reserved
/// as the sentinel value.
pub const NONE_ID: u64 = 0;

/// Maximum context stack depth (prevents runaway navigation).
pub const MAX_CONTEXT_DEPTH: usize = 32;

/// Maximum entities per chamber (sanity check).
pub const MAX_ENTITIES_PER_CHAMBER: usize = 1_000_000;

/// Current schema version for forward compatibility.
pub const SCHEMA_VERSION: u32 = 1;

// =============================================================================
// SERIALIZATION
// =============================================================================

impl WorldSnapshot {
    /// Serialize to bytes using bincode.
    ///
    /// Bincode produces compact binary output (~10x smaller than JSON)
    /// and is deterministic for cache key computation.
    pub fn serialize(&self) -> Result<Vec<u8>, SnapshotError> {
        bincode::serialize(self).map_err(|e| SnapshotError::SerializationFailed(e.to_string()))
    }

    /// Deserialize from bytes.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, SnapshotError> {
        bincode::deserialize(bytes).map_err(|e| SnapshotError::DeserializationFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinel_constants_are_correct() {
        assert_eq!(NONE_IDX, u32::MAX);
        assert_eq!(NONE_ID, 0);
    }

    #[test]
    fn schema_version_is_set() {
        assert_eq!(SCHEMA_VERSION, 1);
    }

    #[test]
    fn round_trip_serialization() {
        let world = WorldSnapshot {
            envelope: SnapshotEnvelope {
                schema_version: SCHEMA_VERSION,
                source_hash: 12345,
                policy_hash: 67890,
                created_at: 1706745600,
                cbu_id: 100,
            },
            string_table: vec!["Entity A".to_string(), "Entity B".to_string()],
            chambers: vec![],
        };

        let bytes = world.serialize().expect("serialize");
        let restored = WorldSnapshot::deserialize(&bytes).expect("deserialize");

        assert_eq!(restored.envelope.schema_version, SCHEMA_VERSION);
        assert_eq!(restored.envelope.source_hash, 12345);
        assert_eq!(restored.string_table.len(), 2);
    }
}
