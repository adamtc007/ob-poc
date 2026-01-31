//! Chamber types - navigable regions within a world.
//!
//! A chamber is a self-contained navigable region containing entities.
//! Chambers use Structure-of-Arrays (SoA) layout for cache-friendly
//! iteration and O(1) navigation via precomputed indices.

use crate::{DoorSnapshot, GridSnapshot, Rect, Vec2, NONE_IDX};
use serde::{Deserialize, Serialize};

// =============================================================================
// CHAMBER KIND
// =============================================================================

/// Type of chamber layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum ChamberKind {
    /// Grid layout (uniform spacing).
    #[default]
    Grid = 0,

    /// Tree layout (hierarchical).
    Tree = 1,

    /// Force-directed layout (organic clustering).
    Force = 2,

    /// Ring/orbital layout (entities around a center).
    Orbital = 3,

    /// Matrix layout (2D tabular).
    Matrix = 4,
}

// =============================================================================
// CAMERA PRESET
// =============================================================================

/// Default camera position for a chamber.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct CameraPreset {
    /// Default camera center position.
    pub center: Vec2,

    /// Default zoom level (1.0 = 100%).
    pub zoom: f32,
}

impl CameraPreset {
    pub fn new(center: Vec2, zoom: f32) -> Self {
        Self { center, zoom }
    }

    /// Create a preset that fits the given bounds.
    pub fn fit_bounds(bounds: Rect, viewport_size: Vec2) -> Self {
        let center = bounds.center();

        // Calculate zoom to fit bounds in viewport with padding
        let padding = 0.9; // 90% of viewport
        let zoom_x = (viewport_size.x * padding) / bounds.width().max(1.0);
        let zoom_y = (viewport_size.y * padding) / bounds.height().max(1.0);
        let zoom = zoom_x.min(zoom_y).clamp(0.1, 10.0);

        Self { center, zoom }
    }
}

// =============================================================================
// CHAMBER SNAPSHOT
// =============================================================================

/// A chamber containing navigable entities.
///
/// # Structure-of-Arrays Layout
///
/// Entity data is stored in parallel arrays for cache efficiency:
///
/// ```text
/// entity_ids:    [id0,   id1,   id2,   ...]
/// kind_ids:      [k0,    k1,    k2,    ...]
/// x:             [x0,    x1,    x2,    ...]
/// y:             [y0,    y1,    y2,    ...]
/// label_ids:     [l0,    l1,    l2,    ...]
/// detail_refs:   [d0,    d1,    d2,    ...]
/// first_child:   [fc0,   fc1,   fc2,   ...]
/// next_sibling:  [ns0,   ns1,   ns2,   ...]
/// prev_sibling:  [ps0,   ps1,   ps2,   ...]
/// ```
///
/// All arrays MUST have the same length N.
///
/// # Navigation Indices
///
/// The `first_child`, `next_sibling`, `prev_sibling` arrays enable O(1)
/// tree navigation. Values are indices into the entity arrays.
/// `NONE_IDX` (u32::MAX) indicates no link.
///
/// # Invariants
///
/// - All parallel arrays have the same length
/// - Navigation indices are either NONE_IDX or < array length
/// - All entity_ids are non-zero (NONE_ID is reserved)
/// - All label_ids are valid indices into world.string_table
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChamberSnapshot {
    /// Unique chamber ID within the world.
    pub id: u32,

    /// Layout type.
    pub kind: ChamberKind,

    /// Bounding box of all entities.
    pub bounds: Rect,

    /// Default camera position.
    pub default_camera: CameraPreset,

    // =========================================================================
    // SoA ENTITY DATA - ALL ARRAYS MUST HAVE SAME LENGTH
    // =========================================================================
    /// Entity IDs (non-zero, maps to external UUID).
    pub entity_ids: Vec<u64>,

    /// Entity kind IDs (index into policy's kind_schema).
    pub kind_ids: Vec<u16>,

    /// X positions in world space.
    pub x: Vec<f32>,

    /// Y positions in world space.
    pub y: Vec<f32>,

    /// Label string IDs (index into world.string_table).
    pub label_ids: Vec<u32>,

    /// Detail reference IDs (NONE_ID if no detail).
    /// Used to fetch additional data on demand.
    pub detail_refs: Vec<u64>,

    // =========================================================================
    // NAVIGATION INDICES - Use NONE_IDX for no link
    // =========================================================================
    /// Index of first child entity (for tree navigation).
    pub first_child: Vec<u32>,

    /// Index of next sibling entity.
    pub next_sibling: Vec<u32>,

    /// Index of previous sibling entity.
    pub prev_sibling: Vec<u32>,

    // =========================================================================
    // CROSS-CHAMBER NAVIGATION
    // =========================================================================
    /// Doors to other chambers.
    pub doors: Vec<DoorSnapshot>,

    /// Spatial index for viewport queries.
    pub grid: GridSnapshot,
}

impl ChamberSnapshot {
    /// Number of entities in this chamber.
    #[inline]
    pub fn entity_count(&self) -> usize {
        self.entity_ids.len()
    }

    /// Check if the chamber is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entity_ids.is_empty()
    }

    /// Get entity position by index.
    #[inline]
    pub fn position(&self, idx: usize) -> Option<Vec2> {
        if idx < self.entity_ids.len() {
            Some(Vec2::new(self.x[idx], self.y[idx]))
        } else {
            None
        }
    }

    /// Get entity ID by index.
    #[inline]
    pub fn entity_id(&self, idx: usize) -> Option<u64> {
        self.entity_ids.get(idx).copied()
    }

    /// Find entity index by ID.
    pub fn find_entity(&self, id: u64) -> Option<usize> {
        self.entity_ids.iter().position(|&eid| eid == id)
    }

    /// Get children of an entity (via navigation indices).
    ///
    /// Returns iterator of child indices.
    pub fn children(&self, parent_idx: usize) -> impl Iterator<Item = usize> + '_ {
        ChildrenIter {
            chamber: self,
            next_idx: self.first_child.get(parent_idx).copied(),
        }
    }

    /// Get siblings of an entity (excluding self).
    pub fn siblings(&self, entity_idx: usize) -> impl Iterator<Item = usize> + '_ {
        SiblingsIter {
            chamber: self,
            start_idx: entity_idx,
            current_idx: Some(entity_idx),
            went_forward: false,
        }
    }

    /// Get next sibling index (O(1)).
    #[inline]
    pub fn next_sibling_idx(&self, idx: usize) -> Option<usize> {
        self.next_sibling.get(idx).and_then(|&ns| {
            if ns == NONE_IDX {
                None
            } else {
                Some(ns as usize)
            }
        })
    }

    /// Get previous sibling index (O(1)).
    #[inline]
    pub fn prev_sibling_idx(&self, idx: usize) -> Option<usize> {
        self.prev_sibling.get(idx).and_then(|&ps| {
            if ps == NONE_IDX {
                None
            } else {
                Some(ps as usize)
            }
        })
    }

    /// Get first child index (O(1)).
    #[inline]
    pub fn first_child_idx(&self, idx: usize) -> Option<usize> {
        self.first_child.get(idx).and_then(|&fc| {
            if fc == NONE_IDX {
                None
            } else {
                Some(fc as usize)
            }
        })
    }

    /// Find door attached to an entity.
    pub fn door_for_entity(&self, entity_idx: u32) -> Option<&DoorSnapshot> {
        self.doors.iter().find(|d| d.from_entity_idx == entity_idx)
    }

    /// Find all doors attached to an entity.
    pub fn doors_for_entity(&self, entity_idx: u32) -> impl Iterator<Item = &DoorSnapshot> {
        self.doors
            .iter()
            .filter(move |d| d.from_entity_idx == entity_idx)
    }
}

// =============================================================================
// NAVIGATION ITERATORS
// =============================================================================

/// Iterator over children of an entity.
struct ChildrenIter<'a> {
    chamber: &'a ChamberSnapshot,
    next_idx: Option<u32>,
}

impl<'a> Iterator for ChildrenIter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let idx = self.next_idx?;
        if idx == NONE_IDX {
            return None;
        }

        let result = idx as usize;

        // Move to next sibling
        self.next_idx = self
            .chamber
            .next_sibling
            .get(result)
            .copied()
            .filter(|&ns| ns != NONE_IDX);

        Some(result)
    }
}

/// Iterator over siblings of an entity.
struct SiblingsIter<'a> {
    chamber: &'a ChamberSnapshot,
    start_idx: usize,
    current_idx: Option<usize>,
    went_forward: bool,
}

impl<'a> Iterator for SiblingsIter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let current = self.current_idx?;

            if !self.went_forward {
                // Go forward via next_sibling
                if let Some(next) = self.chamber.next_sibling_idx(current) {
                    self.current_idx = Some(next);
                    return Some(next);
                }

                // Switch to backward from start
                self.went_forward = true;
                self.current_idx = self.chamber.prev_sibling_idx(self.start_idx);
                continue;
            } else {
                // Go backward via prev_sibling
                let result = current;
                self.current_idx = self.chamber.prev_sibling_idx(current);
                return Some(result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_chamber() -> ChamberSnapshot {
        // Tree structure:
        //       0 (root)
        //      / \
        //     1   2
        //    /
        //   3

        ChamberSnapshot {
            id: 1,
            kind: ChamberKind::Tree,
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            default_camera: CameraPreset::default(),
            entity_ids: vec![100, 101, 102, 103],
            kind_ids: vec![1, 1, 1, 1],
            x: vec![50.0, 25.0, 75.0, 12.5],
            y: vec![10.0, 30.0, 30.0, 50.0],
            label_ids: vec![0, 1, 2, 3],
            detail_refs: vec![100, 101, 102, 103],
            // Navigation: 0's children are 1,2; 1's child is 3
            first_child: vec![1, 3, NONE_IDX, NONE_IDX],
            next_sibling: vec![NONE_IDX, 2, NONE_IDX, NONE_IDX],
            prev_sibling: vec![NONE_IDX, NONE_IDX, 1, NONE_IDX],
            doors: vec![],
            grid: GridSnapshot::default(),
        }
    }

    #[test]
    fn entity_count() {
        let chamber = make_test_chamber();
        assert_eq!(chamber.entity_count(), 4);
        assert!(!chamber.is_empty());
    }

    #[test]
    fn position_lookup() {
        let chamber = make_test_chamber();
        assert_eq!(chamber.position(0), Some(Vec2::new(50.0, 10.0)));
        assert_eq!(chamber.position(1), Some(Vec2::new(25.0, 30.0)));
        assert_eq!(chamber.position(99), None);
    }

    #[test]
    fn find_entity() {
        let chamber = make_test_chamber();
        assert_eq!(chamber.find_entity(100), Some(0));
        assert_eq!(chamber.find_entity(102), Some(2));
        assert_eq!(chamber.find_entity(999), None);
    }

    #[test]
    fn children_iteration() {
        let chamber = make_test_chamber();

        // Root (0) has children 1, 2
        let children: Vec<_> = chamber.children(0).collect();
        assert_eq!(children, vec![1, 2]);

        // Entity 1 has child 3
        let children: Vec<_> = chamber.children(1).collect();
        assert_eq!(children, vec![3]);

        // Entity 2 has no children
        let children: Vec<_> = chamber.children(2).collect();
        assert!(children.is_empty());
    }

    #[test]
    fn sibling_navigation() {
        let chamber = make_test_chamber();

        // Entity 1's next sibling is 2
        assert_eq!(chamber.next_sibling_idx(1), Some(2));
        assert_eq!(chamber.next_sibling_idx(2), None);

        // Entity 2's prev sibling is 1
        assert_eq!(chamber.prev_sibling_idx(2), Some(1));
        assert_eq!(chamber.prev_sibling_idx(1), None);
    }

    #[test]
    fn first_child_navigation() {
        let chamber = make_test_chamber();

        assert_eq!(chamber.first_child_idx(0), Some(1));
        assert_eq!(chamber.first_child_idx(1), Some(3));
        assert_eq!(chamber.first_child_idx(2), None);
        assert_eq!(chamber.first_child_idx(3), None);
    }

    #[test]
    fn camera_preset_fit_bounds() {
        let bounds = Rect::new(0.0, 0.0, 1000.0, 500.0);
        let viewport = Vec2::new(800.0, 600.0);

        let preset = CameraPreset::fit_bounds(bounds, viewport);

        assert_eq!(preset.center, Vec2::new(500.0, 250.0));
        // Zoom should fit width (limiting factor)
        assert!(preset.zoom > 0.0);
        assert!(preset.zoom < 1.0); // bounds wider than viewport
    }
}
