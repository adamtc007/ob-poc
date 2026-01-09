# TODO: Spatial Index & SDF Services (Pre-UAT Required)

> **Priority:** HIGH - Required before Allianz UAT
> **Rationale:** This viewport is THE product UI. First impression with 200-500 entity CBUs must feel premium, not laggy.
> **Status:** Base build requirement, not deferred optimization

## Why This Cannot Wait

1. **Allianz is the first UAT client** - complex investment manager with large ownership structures
2. **This is the ONLY user interface** - no backend screens to fall back on
3. **Performance issues = failed demo** - you don't recover from "it feels slow"
4. **Perceived quality matters** - smooth hover, organic clusters, instant response = professional tool

---

## Part 1: Spatial Index (R-tree)

### User Experience Goal

| Scenario | Without R-tree | With R-tree |
|----------|---------------|-------------|
| Mouse hover over 500 nodes | Laggy, inconsistent highlight | Instant, <1ms |
| Click to select in dense view | Mis-clicks, frustration | Precise, forgiving |
| Drag selection rectangle | Stutters while dragging | Smooth 60fps |

### Implementation

**Location:** `rust/crates/ob-poc-ui/src/viewport/spatial.rs`

**Recommended crate:** `rstar` - mature R-tree for Rust

```rust
use rstar::{RTree, RTreeObject, AABB};

/// Spatial index entry for a rendered node
#[derive(Debug, Clone)]
pub struct SpatialNode {
    pub id: String,
    pub bounds: AABB<[f32; 2]>,
    pub center: [f32; 2],
    pub radius: f32,  // For circular nodes
}

impl RTreeObject for SpatialNode {
    type Envelope = AABB<[f32; 2]>;
    
    fn envelope(&self) -> Self::Envelope {
        self.bounds
    }
}

/// Spatial index for viewport hit testing
pub struct SpatialIndex {
    tree: RTree<SpatialNode>,
    /// Dirty flag - rebuild on next query if true
    dirty: bool,
}

impl SpatialIndex {
    pub fn new() -> Self {
        Self {
            tree: RTree::new(),
            dirty: false,
        }
    }
    
    /// Rebuild index from current node positions
    /// Call when nodes are added/removed/repositioned
    pub fn rebuild(&mut self, nodes: impl Iterator<Item = SpatialNode>) {
        self.tree = RTree::bulk_load(nodes.collect());
        self.dirty = false;
    }
    
    /// Find node at screen position (for hover/click)
    /// Returns closest node within threshold, or None
    pub fn hit_test(&self, point: [f32; 2], threshold: f32) -> Option<&SpatialNode> {
        // Query nearby nodes
        let search_bounds = AABB::from_corners(
            [point[0] - threshold, point[1] - threshold],
            [point[0] + threshold, point[1] + threshold],
        );
        
        self.tree
            .locate_in_envelope(&search_bounds)
            .min_by(|a, b| {
                let dist_a = distance_to_node(point, a);
                let dist_b = distance_to_node(point, b);
                dist_a.partial_cmp(&dist_b).unwrap()
            })
            .filter(|node| distance_to_node(point, node) <= threshold)
    }
    
    /// Find all nodes in rectangle (for drag selection)
    pub fn query_rect(&self, min: [f32; 2], max: [f32; 2]) -> Vec<&SpatialNode> {
        let bounds = AABB::from_corners(min, max);
        self.tree.locate_in_envelope(&bounds).collect()
    }
    
    /// Find all nodes within radius of point
    pub fn query_radius(&self, center: [f32; 2], radius: f32) -> Vec<&SpatialNode> {
        let bounds = AABB::from_corners(
            [center[0] - radius, center[1] - radius],
            [center[0] + radius, center[1] + radius],
        );
        
        self.tree
            .locate_in_envelope(&bounds)
            .filter(|node| distance_to_node(center, node) <= radius)
            .collect()
    }
    
    /// Mark index as needing rebuild
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}

fn distance_to_node(point: [f32; 2], node: &SpatialNode) -> f32 {
    let dx = point[0] - node.center[0];
    let dy = point[1] - node.center[1];
    (dx * dx + dy * dy).sqrt() - node.radius
}
```

### Integration Points

1. **On node add/remove/layout change:** `spatial_index.mark_dirty()`
2. **Before hit test if dirty:** `spatial_index.rebuild(current_nodes())`
3. **On mouse move:** `spatial_index.hit_test(cursor_pos, hover_threshold)`
4. **On click:** `spatial_index.hit_test(cursor_pos, click_threshold)`
5. **On drag select:** `spatial_index.query_rect(selection_bounds)`

### Performance Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Rebuild (500 nodes) | <10ms | Bulk load is O(n log n) |
| Hit test | <0.5ms | O(log n) |
| Rect query | <1ms | Depends on result count |

---

## Part 2: SDF Library

### User Experience Goal

| Feature | What User Sees |
|---------|---------------|
| Confidence halos | Uncertain entities have soft, fuzzy edges - "this might not be right" |
| Cluster blobs | Related entities feel grouped, organic shapes not rigid boxes |
| Generous hit zones | Clicking "near" an entity works - forgiving, not frustrating |
| Edge proximity | Visual feedback when cursor approaches entity boundary |

### Implementation

**Location:** `rust/crates/sdf/` - new crate, callable service

```rust
//! Signed Distance Field library for viewport rendering
//!
//! Provides distance field primitives and operations for:
//! - Confidence zone halos (fuzzy membership visualization)
//! - Cluster blob shapes (organic grouping)
//! - Hit testing with smooth boundaries
//! - Edge proximity detection

/// 2D point
pub type Point = [f32; 2];

// =============================================================================
// PRIMITIVES
// =============================================================================

/// Distance to circle edge (negative = inside)
#[inline]
pub fn circle(point: Point, center: Point, radius: f32) -> f32 {
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];
    (dx * dx + dy * dy).sqrt() - radius
}

/// Distance to axis-aligned rectangle edge
#[inline]
pub fn rect(point: Point, center: Point, half_size: Point) -> f32 {
    let dx = (point[0] - center[0]).abs() - half_size[0];
    let dy = (point[1] - center[1]).abs() - half_size[1];
    
    let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
    let inside = dx.max(dy).min(0.0);
    
    outside + inside
}

/// Distance to rounded rectangle
#[inline]
pub fn rounded_rect(point: Point, center: Point, half_size: Point, radius: f32) -> f32 {
    let inner_half = [
        (half_size[0] - radius).max(0.0),
        (half_size[1] - radius).max(0.0),
    ];
    rect(point, center, inner_half) - radius
}

/// Distance to line segment
pub fn line_segment(point: Point, a: Point, b: Point, thickness: f32) -> f32 {
    let pa = [point[0] - a[0], point[1] - a[1]];
    let ba = [b[0] - a[0], b[1] - a[1]];
    
    let h = ((pa[0] * ba[0] + pa[1] * ba[1]) / (ba[0] * ba[0] + ba[1] * ba[1]))
        .clamp(0.0, 1.0);
    
    let dx = pa[0] - ba[0] * h;
    let dy = pa[1] - ba[1] * h;
    
    (dx * dx + dy * dy).sqrt() - thickness
}

// =============================================================================
// OPERATIONS
// =============================================================================

/// Union of two shapes (minimum distance)
#[inline]
pub fn union(d1: f32, d2: f32) -> f32 {
    d1.min(d2)
}

/// Intersection of two shapes (maximum distance)
#[inline]
pub fn intersection(d1: f32, d2: f32) -> f32 {
    d1.max(d2)
}

/// Subtraction (d1 minus d2)
#[inline]
pub fn subtraction(d1: f32, d2: f32) -> f32 {
    d1.max(-d2)
}

/// Smooth union - blends shapes together organically
/// k controls smoothness (0.1 = very smooth, 1.0 = sharp)
#[inline]
pub fn smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
    lerp(d2, d1, h) - k * h * (1.0 - h)
}

/// Smooth intersection
#[inline]
pub fn smooth_intersection(d1: f32, d2: f32, k: f32) -> f32 {
    let h = (0.5 - 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
    lerp(d2, d1, h) + k * h * (1.0 - h)
}

/// Offset/dilate a shape (positive = grow, negative = shrink)
#[inline]
pub fn offset(d: f32, amount: f32) -> f32 {
    d - amount
}

/// Round the edges of a shape
#[inline]
pub fn round(d: f32, radius: f32) -> f32 {
    d - radius
}

// =============================================================================
// CLUSTER BLOB
// =============================================================================

/// Generate a blob shape that encompasses multiple circles
/// Uses smooth union to create organic, cloud-like boundary
pub fn cluster_blob(point: Point, circles: &[(Point, f32)], smoothness: f32) -> f32 {
    if circles.is_empty() {
        return f32::MAX;
    }
    
    let mut d = circle(point, circles[0].0, circles[0].1);
    
    for &(center, radius) in &circles[1..] {
        let d2 = circle(point, center, radius);
        d = smooth_union(d, d2, smoothness);
    }
    
    d
}

// =============================================================================
// CONFIDENCE HALO
// =============================================================================

/// Confidence zone rendering data
#[derive(Debug, Clone, Copy)]
pub struct ConfidenceHalo {
    pub center: Point,
    pub core_radius: f32,
    pub confidence: f32,  // 0.0 - 1.0
}

impl ConfidenceHalo {
    /// Get the alpha/opacity at a given point based on distance
    /// Core (high confidence) = solid
    /// Shell (medium) = semi-transparent
    /// Penumbra (low) = fading edge
    pub fn alpha_at(&self, point: Point) -> f32 {
        let d = circle(point, self.center, self.core_radius);
        
        if d < 0.0 {
            // Inside core
            self.confidence
        } else {
            // Outside - fade based on confidence
            let falloff = 1.0 - self.confidence; // Lower confidence = wider halo
            let halo_width = self.core_radius * falloff * 0.5;
            
            if d < halo_width {
                // In the halo
                let t = d / halo_width;
                self.confidence * (1.0 - t * t) // Quadratic falloff
            } else {
                0.0
            }
        }
    }
    
    /// Get the outer radius including halo
    pub fn outer_radius(&self) -> f32 {
        let falloff = 1.0 - self.confidence;
        self.core_radius * (1.0 + falloff * 0.5)
    }
}

// =============================================================================
// HIT TESTING
// =============================================================================

/// SDF-based hit testing with gradient (tells you direction to nearest edge)
pub struct HitResult {
    pub distance: f32,
    pub inside: bool,
    /// Normalized direction to nearest edge (for hover effects)
    pub gradient: Point,
}

/// Compute hit result with gradient for a circle
pub fn hit_test_circle(point: Point, center: Point, radius: f32) -> HitResult {
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];
    let dist_to_center = (dx * dx + dy * dy).sqrt();
    
    let distance = dist_to_center - radius;
    let inside = distance < 0.0;
    
    // Gradient points away from center (toward edge if inside, away if outside)
    let gradient = if dist_to_center > 0.001 {
        [dx / dist_to_center, dy / dist_to_center]
    } else {
        [1.0, 0.0] // Arbitrary direction at center
    };
    
    HitResult {
        distance,
        inside,
        gradient,
    }
}

// =============================================================================
// HELPERS
// =============================================================================

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_circle_distance() {
        // Center of circle
        assert!((circle([0.0, 0.0], [0.0, 0.0], 10.0) - (-10.0)).abs() < 0.001);
        // On edge
        assert!((circle([10.0, 0.0], [0.0, 0.0], 10.0)).abs() < 0.001);
        // Outside
        assert!((circle([15.0, 0.0], [0.0, 0.0], 10.0) - 5.0).abs() < 0.001);
    }
    
    #[test]
    fn test_smooth_union() {
        let c1 = circle([0.0, 0.0], [-5.0, 0.0], 8.0);
        let c2 = circle([0.0, 0.0], [5.0, 0.0], 8.0);
        
        // Smooth union should be less than regular union at the junction
        let regular = union(c1, c2);
        let smooth = smooth_union(c1, c2, 4.0);
        
        assert!(smooth <= regular);
    }
    
    #[test]
    fn test_cluster_blob() {
        let circles = vec![
            ([-10.0, 0.0], 8.0),
            ([10.0, 0.0], 8.0),
            ([0.0, 10.0], 6.0),
        ];
        
        // Point at center should be inside
        let d = cluster_blob([0.0, 3.0], &circles, 5.0);
        assert!(d < 0.0);
        
        // Point far away should be outside
        let d = cluster_blob([100.0, 100.0], &circles, 5.0);
        assert!(d > 0.0);
    }
    
    #[test]
    fn test_confidence_halo() {
        let halo = ConfidenceHalo {
            center: [0.0, 0.0],
            core_radius: 20.0,
            confidence: 0.7,
        };
        
        // Inside core - full confidence alpha
        assert!((halo.alpha_at([0.0, 0.0]) - 0.7).abs() < 0.001);
        
        // Outside halo - zero
        assert!((halo.alpha_at([100.0, 0.0])).abs() < 0.001);
    }
}
```

### Integration with Rendering

```rust
// In ob-poc-ui renderer

/// Render confidence halo for an entity
fn render_confidence_halo(
    painter: &egui::Painter,
    entity: &EntityNode,
    halo: &ConfidenceHalo,
) {
    // Sample points around the halo and render gradient
    let steps = 32;
    let outer = halo.outer_radius();
    
    for i in 0..steps {
        let angle = (i as f32 / steps as f32) * std::f32::consts::TAU;
        let x = halo.center[0] + outer * angle.cos();
        let y = halo.center[1] + outer * angle.sin();
        
        let alpha = halo.alpha_at([x, y]);
        if alpha > 0.01 {
            // Render point with alpha
        }
    }
}

/// Render cluster blob for a group of entities
fn render_cluster_blob(
    painter: &egui::Painter,
    members: &[EntityNode],
    smoothness: f32,
    color: egui::Color32,
) {
    let circles: Vec<_> = members
        .iter()
        .map(|e| ([e.pos.x, e.pos.y], e.radius))
        .collect();
    
    // Render blob boundary by sampling
    // (or generate mesh from SDF for better performance)
}
```

### Cargo.toml for SDF crate

```toml
[package]
name = "sdf"
version = "0.1.0"
edition = "2021"

[dependencies]
# No dependencies - pure math

[dev-dependencies]
# None needed
```

---

## Dependencies to Add

```toml
# In rust/crates/ob-poc-ui/Cargo.toml
[dependencies]
rstar = "0.11"  # R-tree spatial index

# In rust/Cargo.toml workspace
[workspace]
members = [
    # ... existing
    "crates/sdf",
]
```

---

## Acceptance Criteria

### Spatial Index
- [ ] R-tree implemented with `rstar`
- [ ] Hit testing returns correct node in <1ms for 500 nodes
- [ ] Rectangle selection works smoothly
- [ ] Index rebuilds on layout change
- [ ] Hover feels instant, no perceptible lag

### SDF Library
- [ ] Circle, rect, rounded_rect primitives
- [ ] smooth_union for cluster blobs
- [ ] ConfidenceHalo with alpha falloff
- [ ] Cluster blob rendering for entity groups
- [ ] Unit tests pass

### Integration
- [ ] Spatial index used for all hit testing
- [ ] Confidence halos render for entities with < 0.95 confidence
- [ ] Cluster blobs render for CBU member groups
- [ ] No performance regression on 500 node viewport

---

## Testing with Allianz-Scale Data

Before UAT, test with:
- [ ] 200+ entity CBU
- [ ] Complex ownership chains (5+ levels)
- [ ] Mixed confidence scores (Core/Shell/Penumbra visible)
- [ ] Full instrument matrix expanded
- [ ] Rapid mouse movement across dense areas
- [ ] Drag selection across entire viewport

**The viewport must feel premium. No lag. No jank. First impression is everything.**
