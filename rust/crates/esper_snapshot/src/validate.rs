//! Snapshot validation.
//!
//! All snapshots MUST be validated before use. Validation enforces
//! the SoA invariants that the navigation engine depends on.

use crate::{
    ChamberSnapshot, SnapshotError, WorldSnapshot, MAX_ENTITIES_PER_CHAMBER, NONE_ID, NONE_IDX,
    SCHEMA_VERSION,
};

/// Trait for validatable types.
pub trait Validate {
    /// Validate the object, returning an error if invalid.
    fn validate(&self) -> Result<(), SnapshotError>;

    /// Validate in debug builds only (for performance).
    #[cfg(debug_assertions)]
    fn debug_validate(&self) -> Result<(), SnapshotError> {
        self.validate()
    }

    #[cfg(not(debug_assertions))]
    fn debug_validate(&self) -> Result<(), SnapshotError> {
        Ok(())
    }
}

impl Validate for WorldSnapshot {
    fn validate(&self) -> Result<(), SnapshotError> {
        // Check schema version
        if self.envelope.schema_version != SCHEMA_VERSION {
            return Err(SnapshotError::SchemaVersionMismatch {
                expected: SCHEMA_VERSION,
                actual: self.envelope.schema_version,
            });
        }

        // Validate each chamber
        for chamber in &self.chambers {
            chamber.validate_with_string_table(self.string_table.len())?;
        }

        // Validate door references
        let chamber_ids: std::collections::HashSet<u32> =
            self.chambers.iter().map(|c| c.id).collect();

        for chamber in &self.chambers {
            for door in &chamber.doors {
                if !chamber_ids.contains(&door.target_chamber_id) {
                    return Err(SnapshotError::InvalidDoorTarget {
                        door_id: door.id,
                        chamber_id: door.target_chamber_id,
                    });
                }
            }
        }

        Ok(())
    }
}

impl Validate for ChamberSnapshot {
    fn validate(&self) -> Result<(), SnapshotError> {
        self.validate_with_string_table(usize::MAX)
    }
}

impl ChamberSnapshot {
    /// Validate with known string table size.
    pub fn validate_with_string_table(&self, string_table_len: usize) -> Result<(), SnapshotError> {
        let n = self.entity_ids.len();

        // Check entity count limit
        if n > MAX_ENTITIES_PER_CHAMBER {
            return Err(SnapshotError::TooManyEntities {
                count: n,
                max: MAX_ENTITIES_PER_CHAMBER,
            });
        }

        // Check all parallel arrays have same length
        self.check_array_len(n, self.kind_ids.len(), "kind_ids")?;
        self.check_array_len(n, self.x.len(), "x")?;
        self.check_array_len(n, self.y.len(), "y")?;
        self.check_array_len(n, self.label_ids.len(), "label_ids")?;
        self.check_array_len(n, self.detail_refs.len(), "detail_refs")?;
        self.check_array_len(n, self.first_child.len(), "first_child")?;
        self.check_array_len(n, self.next_sibling.len(), "next_sibling")?;
        self.check_array_len(n, self.prev_sibling.len(), "prev_sibling")?;

        // Validate entity IDs (must be non-zero)
        for (idx, &id) in self.entity_ids.iter().enumerate() {
            if id == NONE_ID {
                return Err(SnapshotError::InvalidEntityId { index: idx });
            }
        }

        // Validate navigation indices
        self.check_nav_indices(n, &self.first_child, "first_child")?;
        self.check_nav_indices(n, &self.next_sibling, "next_sibling")?;
        self.check_nav_indices(n, &self.prev_sibling, "prev_sibling")?;

        // Validate string table references
        for &label_id in self.label_ids.iter() {
            if label_id as usize >= string_table_len && string_table_len != usize::MAX {
                return Err(SnapshotError::InvalidStringId {
                    index: label_id,
                    size: string_table_len,
                });
            }
            // Also check for obviously invalid values when no string table provided
            if string_table_len == usize::MAX && label_id == u32::MAX {
                return Err(SnapshotError::InvalidStringId {
                    index: label_id,
                    size: 0,
                });
            }
        }

        // Validate door entity references
        for door in &self.doors {
            if door.from_entity_idx as usize >= n {
                return Err(SnapshotError::IndexOutOfBounds {
                    index: door.from_entity_idx,
                    max: n,
                    field: "door.from_entity_idx",
                });
            }
        }

        // Validate grid
        self.validate_grid()?;

        Ok(())
    }

    fn check_array_len(
        &self,
        expected: usize,
        actual: usize,
        field: &'static str,
    ) -> Result<(), SnapshotError> {
        if actual != expected {
            Err(SnapshotError::ArrayLengthMismatch {
                expected,
                actual,
                field,
            })
        } else {
            Ok(())
        }
    }

    fn check_nav_indices(
        &self,
        n: usize,
        indices: &[u32],
        field: &'static str,
    ) -> Result<(), SnapshotError> {
        for &idx in indices.iter() {
            if idx != NONE_IDX && idx as usize >= n {
                return Err(SnapshotError::IndexOutOfBounds {
                    index: idx,
                    max: n,
                    field,
                });
            }
        }
        Ok(())
    }

    fn validate_grid(&self) -> Result<(), SnapshotError> {
        if self.grid.is_empty() {
            return Ok(());
        }

        let expected_cells = (self.grid.dims.0 * self.grid.dims.1) as usize;
        if self.grid.cell_ranges.len() != expected_cells {
            return Err(SnapshotError::ArrayLengthMismatch {
                expected: expected_cells,
                actual: self.grid.cell_ranges.len(),
                field: "grid.cell_ranges",
            });
        }

        // Validate cell ranges
        let indices_len = self.grid.cell_entity_indices.len();
        for &(start, count) in self.grid.cell_ranges.iter() {
            let end = start as usize + count as usize;
            if end > indices_len {
                return Err(SnapshotError::InvalidGridCellRange {
                    start,
                    count,
                    len: indices_len,
                });
            }
        }

        // Validate entity indices in grid
        let n = self.entity_ids.len();
        for &idx in &self.grid.cell_entity_indices {
            if idx as usize >= n {
                return Err(SnapshotError::IndexOutOfBounds {
                    index: idx,
                    max: n,
                    field: "grid.cell_entity_indices",
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rect, Vec2};

    fn make_valid_chamber() -> ChamberSnapshot {
        ChamberSnapshot {
            id: 1,
            kind: crate::ChamberKind::Grid,
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            default_camera: crate::CameraPreset::default(),
            entity_ids: vec![1, 2, 3],
            kind_ids: vec![0, 0, 0],
            x: vec![10.0, 50.0, 90.0],
            y: vec![10.0, 50.0, 90.0],
            label_ids: vec![0, 1, 2],
            detail_refs: vec![1, 2, 3],
            first_child: vec![NONE_IDX, NONE_IDX, NONE_IDX],
            next_sibling: vec![1, 2, NONE_IDX],
            prev_sibling: vec![NONE_IDX, 0, 1],
            doors: vec![],
            grid: crate::GridSnapshot::default(),
        }
    }

    fn make_valid_world() -> WorldSnapshot {
        WorldSnapshot {
            envelope: crate::SnapshotEnvelope {
                schema_version: SCHEMA_VERSION,
                source_hash: 123,
                policy_hash: 456,
                created_at: 0,
                cbu_id: 1,
            },
            string_table: vec![
                "Entity 1".to_string(),
                "Entity 2".to_string(),
                "Entity 3".to_string(),
            ],
            chambers: vec![make_valid_chamber()],
        }
    }

    #[test]
    fn valid_world_passes() {
        let world = make_valid_world();
        assert!(world.validate().is_ok());
    }

    #[test]
    fn valid_chamber_passes() {
        let chamber = make_valid_chamber();
        assert!(chamber.validate().is_ok());
    }

    #[test]
    fn array_length_mismatch_fails() {
        let mut chamber = make_valid_chamber();
        chamber.kind_ids.push(0); // Now 4 elements vs 3

        let result = chamber.validate();
        assert!(matches!(
            result,
            Err(SnapshotError::ArrayLengthMismatch {
                field: "kind_ids",
                ..
            })
        ));
    }

    #[test]
    fn zero_entity_id_fails() {
        let mut chamber = make_valid_chamber();
        chamber.entity_ids[1] = NONE_ID; // Zero is invalid

        let result = chamber.validate();
        assert!(matches!(
            result,
            Err(SnapshotError::InvalidEntityId { index: 1 })
        ));
    }

    #[test]
    fn out_of_bounds_nav_index_fails() {
        let mut chamber = make_valid_chamber();
        chamber.next_sibling[2] = 99; // Out of bounds

        let result = chamber.validate();
        assert!(matches!(
            result,
            Err(SnapshotError::IndexOutOfBounds {
                field: "next_sibling",
                ..
            })
        ));
    }

    #[test]
    fn invalid_string_id_fails() {
        let mut world = make_valid_world();
        world.chambers[0].label_ids[0] = 999; // Out of bounds

        let result = world.validate();
        assert!(matches!(
            result,
            Err(SnapshotError::InvalidStringId { index: 999, .. })
        ));
    }

    #[test]
    fn invalid_door_target_fails() {
        let mut world = make_valid_world();
        world.chambers[0].doors.push(crate::DoorSnapshot {
            id: 1,
            from_entity_idx: 0,
            target_chamber_id: 999, // Non-existent chamber
            target_entity_id: 0,
            door_kind: crate::DoorKind::Related,
            label_id: 0,
            position: Vec2::ZERO,
        });

        let result = world.validate();
        assert!(matches!(
            result,
            Err(SnapshotError::InvalidDoorTarget {
                door_id: 1,
                chamber_id: 999
            })
        ));
    }

    #[test]
    fn schema_version_mismatch_fails() {
        let mut world = make_valid_world();
        world.envelope.schema_version = 999;

        let result = world.validate();
        assert!(matches!(
            result,
            Err(SnapshotError::SchemaVersionMismatch {
                expected: SCHEMA_VERSION,
                actual: 999
            })
        ));
    }
}
