//! Door types for cross-chamber navigation.
//!
//! Doors are one-way portals between chambers. They enable hierarchical
//! navigation (drill into details) and lateral navigation (related entities).

use crate::Vec2;
use serde::{Deserialize, Serialize};

/// Door kind determines visual representation and navigation behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum DoorKind {
    /// Drill down into child entities (ownership, composition).
    Descend = 0,

    /// Navigate to related entities (references, associations).
    #[default]
    Related = 1,

    /// Navigate to parent/container (inverse of Descend).
    Ascend = 2,

    /// Jump to a specific entity in another chamber.
    Jump = 3,

    /// External link (opens outside the navigation system).
    External = 4,
}

/// A door connecting entities across chambers.
///
/// Doors enable navigation between chambers. Each door has:
/// - A source entity in the current chamber
/// - A target chamber (and optionally target entity)
/// - Visual properties (position, label)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoorSnapshot {
    /// Unique door ID within the world.
    pub id: u32,

    /// Index of the entity this door is attached to (in current chamber).
    /// This is a chamber-local index into the entity arrays.
    pub from_entity_idx: u32,

    /// Target chamber ID (stable, not an index).
    pub target_chamber_id: u32,

    /// Target entity ID in the target chamber (0 = no specific target).
    /// When non-zero, the camera will focus on this entity after transition.
    pub target_entity_id: u64,

    /// Door type for rendering and behavior.
    pub door_kind: DoorKind,

    /// Label string ID (index into world's string_table).
    pub label_id: u32,

    /// Position relative to the attached entity (for rendering).
    pub position: Vec2,
}

impl DoorSnapshot {
    /// Check if this door targets a specific entity (vs just a chamber).
    #[inline]
    pub fn has_target_entity(&self) -> bool {
        self.target_entity_id != crate::NONE_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn door_kind_default() {
        assert_eq!(DoorKind::default(), DoorKind::Related);
    }

    #[test]
    fn door_has_target_entity() {
        let door_with_target = DoorSnapshot {
            id: 1,
            from_entity_idx: 0,
            target_chamber_id: 2,
            target_entity_id: 12345,
            door_kind: DoorKind::Descend,
            label_id: 0,
            position: Vec2::ZERO,
        };

        let door_without_target = DoorSnapshot {
            id: 2,
            from_entity_idx: 0,
            target_chamber_id: 2,
            target_entity_id: crate::NONE_ID,
            door_kind: DoorKind::Related,
            label_id: 0,
            position: Vec2::ZERO,
        };

        assert!(door_with_target.has_target_entity());
        assert!(!door_without_target.has_target_entity());
    }
}
