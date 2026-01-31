//! Core snapshot types.
//!
//! These types form the wire format contract between server (compiler) and
//! client (renderer). Changes here require schema version bump.

use crate::ChamberSnapshot;
use serde::{Deserialize, Serialize};

// =============================================================================
// GEOMETRY PRIMITIVES
// =============================================================================

/// 2D vector for positions and deltas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    #[inline]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }

    #[inline]
    pub fn distance(self, other: Self) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Axis-aligned bounding rectangle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub const ZERO: Rect = Rect {
        min: Vec2::ZERO,
        max: Vec2::ZERO,
    };

    #[inline]
    pub fn new(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            min: Vec2::new(min_x, min_y),
            max: Vec2::new(max_x, max_y),
        }
    }

    #[inline]
    pub fn from_center_size(center: Vec2, width: f32, height: f32) -> Self {
        let half_w = width / 2.0;
        let half_h = height / 2.0;
        Self {
            min: Vec2::new(center.x - half_w, center.y - half_h),
            max: Vec2::new(center.x + half_w, center.y + half_h),
        }
    }

    #[inline]
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    #[inline]
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    #[inline]
    pub fn center(&self) -> Vec2 {
        Vec2::new(
            (self.min.x + self.max.x) / 2.0,
            (self.min.y + self.max.y) / 2.0,
        )
    }

    #[inline]
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    #[inline]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }

    /// Expand rect to include a point.
    #[inline]
    pub fn expand_to_include(&mut self, point: Vec2) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
    }
}

// =============================================================================
// SNAPSHOT ENVELOPE
// =============================================================================

/// Snapshot metadata for cache invalidation and versioning.
///
/// The envelope is checked first when loading a snapshot to determine
/// if recompilation is needed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotEnvelope {
    /// Schema version for forward compatibility.
    /// Snapshots with different schema versions are incompatible.
    pub schema_version: u32,

    /// Hash of source graph data.
    /// If source changes, snapshot must be recompiled.
    pub source_hash: u64,

    /// Hash of render policy.
    /// If policy changes, snapshot must be recompiled.
    pub policy_hash: u64,

    /// Unix timestamp when snapshot was created.
    pub created_at: u64,

    /// CBU ID this snapshot represents (as u64 for wire efficiency).
    /// Client maps back to UUID via lookup table.
    pub cbu_id: u64,
}

// =============================================================================
// WORLD SNAPSHOT
// =============================================================================

/// Complete navigation world - the top-level serialized structure.
///
/// A world contains:
/// - Envelope for cache validation
/// - String table for label deduplication
/// - Chambers (navigable regions)
///
/// # String Table
///
/// All labels are stored in the string table and referenced by index.
/// This deduplication typically reduces snapshot size by 30-50%.
///
/// # Chambers
///
/// Each chamber is a self-contained navigable region. Doors connect
/// chambers to enable cross-chamber navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    /// Metadata for cache invalidation.
    pub envelope: SnapshotEnvelope,

    /// Deduplicated string table.
    /// Chamber `label_ids` index into this table.
    pub string_table: Vec<String>,

    /// Navigable chambers.
    /// Index 0 is typically the root chamber.
    pub chambers: Vec<ChamberSnapshot>,
}

impl WorldSnapshot {
    /// Create an empty world (for testing).
    pub fn empty(cbu_id: u64) -> Self {
        Self {
            envelope: SnapshotEnvelope {
                schema_version: crate::SCHEMA_VERSION,
                source_hash: 0,
                policy_hash: 0,
                created_at: 0,
                cbu_id,
            },
            string_table: Vec::new(),
            chambers: Vec::new(),
        }
    }

    /// Get a chamber by ID.
    pub fn chamber(&self, id: u32) -> Option<&ChamberSnapshot> {
        self.chambers.iter().find(|c| c.id == id)
    }

    /// Get a chamber by index (faster when you know the index).
    pub fn chamber_at(&self, index: usize) -> Option<&ChamberSnapshot> {
        self.chambers.get(index)
    }

    /// Look up a string from the string table.
    #[inline]
    pub fn string(&self, id: u32) -> Option<&str> {
        self.string_table.get(id as usize).map(|s| s.as_str())
    }

    /// Total entity count across all chambers.
    pub fn total_entities(&self) -> usize {
        self.chambers.iter().map(|c| c.entity_count()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec2_lerp() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(10.0, 20.0);

        let mid = a.lerp(b, 0.5);
        assert!((mid.x - 5.0).abs() < 0.001);
        assert!((mid.y - 10.0).abs() < 0.001);
    }

    #[test]
    fn rect_contains() {
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);

        assert!(rect.contains(Vec2::new(50.0, 50.0)));
        assert!(rect.contains(Vec2::new(0.0, 0.0)));
        assert!(rect.contains(Vec2::new(100.0, 100.0)));
        assert!(!rect.contains(Vec2::new(-1.0, 50.0)));
        assert!(!rect.contains(Vec2::new(101.0, 50.0)));
    }

    #[test]
    fn rect_intersects() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 150.0, 150.0);
        let c = Rect::new(200.0, 200.0, 300.0, 300.0);

        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn world_snapshot_empty() {
        let world = WorldSnapshot::empty(42);
        assert_eq!(world.envelope.cbu_id, 42);
        assert_eq!(world.total_entities(), 0);
    }
}
