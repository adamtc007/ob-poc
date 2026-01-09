//! Spatial Index for fast viewport hit testing
//!
//! Uses R-tree (via `rstar`) for O(log n) lookups instead of O(n) linear scan.
//! Critical for smooth hover/click with 200-500+ node CBUs.

use rstar::{PointDistance, RTree, RTreeObject, AABB};

/// Spatial index entry for a rendered node
#[derive(Debug, Clone)]
pub struct SpatialNode {
    /// Unique identifier for the node
    pub id: String,
    /// Axis-aligned bounding box
    bounds: AABB<[f32; 2]>,
    /// Center position
    pub center: [f32; 2],
    /// Radius for circular nodes (or half-diagonal for rects)
    pub radius: f32,
    /// Node type for filtering (e.g., "entity", "cbu", "edge")
    pub node_type: Option<String>,
}

impl SpatialNode {
    /// Create a new spatial node entry
    pub fn new(id: impl Into<String>, center: [f32; 2], radius: f32) -> Self {
        let id = id.into();
        let bounds = AABB::from_corners(
            [center[0] - radius, center[1] - radius],
            [center[0] + radius, center[1] + radius],
        );
        Self {
            id,
            bounds,
            center,
            radius,
            node_type: None,
        }
    }

    /// Create with explicit bounds (for non-circular nodes)
    pub fn with_bounds(id: impl Into<String>, min: [f32; 2], max: [f32; 2]) -> Self {
        let id = id.into();
        let center = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
        let dx = (max[0] - min[0]) / 2.0;
        let dy = (max[1] - min[1]) / 2.0;
        let radius = (dx * dx + dy * dy).sqrt();
        let bounds = AABB::from_corners(min, max);
        Self {
            id,
            bounds,
            center,
            radius,
            node_type: None,
        }
    }

    /// Set the node type for filtering
    pub fn with_type(mut self, node_type: impl Into<String>) -> Self {
        self.node_type = Some(node_type.into());
        self
    }

    /// Get the bounding box
    pub fn bounds(&self) -> &AABB<[f32; 2]> {
        &self.bounds
    }
}

impl RTreeObject for SpatialNode {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.bounds
    }
}

impl PointDistance for SpatialNode {
    fn distance_2(&self, point: &[f32; 2]) -> f32 {
        // Distance squared from point to node center, accounting for radius
        let dx = point[0] - self.center[0];
        let dy = point[1] - self.center[1];
        let dist_to_center = (dx * dx + dy * dy).sqrt();
        let dist_to_edge = (dist_to_center - self.radius).max(0.0);
        dist_to_edge * dist_to_edge
    }

    fn contains_point(&self, point: &[f32; 2]) -> bool {
        let dx = point[0] - self.center[0];
        let dy = point[1] - self.center[1];
        let dist_sq = dx * dx + dy * dy;
        dist_sq <= self.radius * self.radius
    }
}

/// Spatial index for viewport hit testing
///
/// Provides O(log n) hit testing for hover, click, and rectangle selection.
#[derive(Clone)]
pub struct SpatialIndex {
    tree: RTree<SpatialNode>,
    /// Dirty flag - rebuild on next query if true
    dirty: bool,
    /// Number of nodes in the index
    count: usize,
}

impl std::fmt::Debug for SpatialIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpatialIndex")
            .field("count", &self.count)
            .field("dirty", &self.dirty)
            .finish_non_exhaustive()
    }
}

impl Default for SpatialIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl SpatialIndex {
    /// Create a new empty spatial index
    pub fn new() -> Self {
        Self {
            tree: RTree::new(),
            dirty: false,
            count: 0,
        }
    }

    /// Create a spatial index from an iterator of nodes
    pub fn from_nodes(nodes: impl Iterator<Item = SpatialNode>) -> Self {
        let nodes: Vec<_> = nodes.collect();
        let count = nodes.len();
        Self {
            tree: RTree::bulk_load(nodes),
            dirty: false,
            count,
        }
    }

    /// Rebuild index from current node positions
    /// Call when nodes are added/removed/repositioned
    pub fn rebuild(&mut self, nodes: impl Iterator<Item = SpatialNode>) {
        let nodes: Vec<_> = nodes.collect();
        self.count = nodes.len();
        self.tree = RTree::bulk_load(nodes);
        self.dirty = false;
    }

    /// Clear the index
    pub fn clear(&mut self) {
        self.tree = RTree::new();
        self.count = 0;
        self.dirty = false;
    }

    /// Find node at screen position (for hover/click)
    /// Returns closest node within threshold, or None
    pub fn hit_test(&self, point: [f32; 2], threshold: f32) -> Option<&SpatialNode> {
        let search_bounds = AABB::from_corners(
            [point[0] - threshold, point[1] - threshold],
            [point[0] + threshold, point[1] + threshold],
        );

        // Use locate_in_envelope_intersecting to find nodes whose bounds
        // intersect (not just contained within) the search area
        self.tree
            .locate_in_envelope_intersecting(&search_bounds)
            .min_by(|a, b| {
                let dist_a = distance_to_node(point, a);
                let dist_b = distance_to_node(point, b);
                dist_a
                    .partial_cmp(&dist_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .filter(|node| distance_to_node(point, node) <= threshold)
    }

    /// Find node by ID
    pub fn find_by_id(&self, id: &str) -> Option<&SpatialNode> {
        self.tree.iter().find(|n| n.id == id)
    }

    /// Find all nodes in rectangle (for drag selection)
    pub fn query_rect(&self, min: [f32; 2], max: [f32; 2]) -> Vec<&SpatialNode> {
        let bounds = AABB::from_corners(min, max);
        self.tree.locate_in_envelope(&bounds).collect()
    }

    /// Find all nodes intersecting a rectangle
    pub fn query_rect_intersecting(&self, min: [f32; 2], max: [f32; 2]) -> Vec<&SpatialNode> {
        let bounds = AABB::from_corners(min, max);
        self.tree.locate_in_envelope_intersecting(&bounds).collect()
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

    /// Find the N nearest nodes to a point
    pub fn nearest_n(&self, point: [f32; 2], n: usize) -> Vec<&SpatialNode> {
        self.tree.nearest_neighbor_iter(&point).take(n).collect()
    }

    /// Find the nearest node to a point
    pub fn nearest(&self, point: [f32; 2]) -> Option<&SpatialNode> {
        self.tree.nearest_neighbor(&point)
    }

    /// Mark index as needing rebuild
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Check if index needs rebuild
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Get number of nodes in the index
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Iterate over all nodes
    pub fn iter(&self) -> impl Iterator<Item = &SpatialNode> {
        self.tree.iter()
    }
}

/// Calculate distance from point to node center, accounting for radius
fn distance_to_node(point: [f32; 2], node: &SpatialNode) -> f32 {
    let dx = point[0] - node.center[0];
    let dy = point[1] - node.center[1];
    let dist_to_center = (dx * dx + dy * dy).sqrt();
    // Return distance to edge, not center
    (dist_to_center - node.radius).max(0.0)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_nodes(count: usize) -> Vec<SpatialNode> {
        (0..count)
            .map(|i| {
                let x = (i % 50) as f32 * 20.0;
                let y = (i / 50) as f32 * 20.0;
                SpatialNode::new(format!("node_{}", i), [x, y], 8.0)
            })
            .collect()
    }

    #[test]
    fn test_empty_index() {
        let index = SpatialIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert!(index.hit_test([0.0, 0.0], 10.0).is_none());
    }

    #[test]
    fn test_single_node() {
        let mut index = SpatialIndex::new();
        let node = SpatialNode::new("test", [100.0, 100.0], 10.0);
        index.rebuild(std::iter::once(node));

        assert_eq!(index.len(), 1);

        // Hit inside
        let hit = index.hit_test([100.0, 100.0], 5.0);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().id, "test");

        // Hit on edge
        let hit = index.hit_test([110.0, 100.0], 5.0);
        assert!(hit.is_some());

        // Miss
        let hit = index.hit_test([200.0, 200.0], 5.0);
        assert!(hit.is_none());
    }

    #[test]
    fn test_multiple_nodes() {
        let mut index = SpatialIndex::new();
        let nodes = vec![
            SpatialNode::new("a", [0.0, 0.0], 10.0),
            SpatialNode::new("b", [50.0, 0.0], 10.0),
            SpatialNode::new("c", [100.0, 0.0], 10.0),
        ];
        index.rebuild(nodes.into_iter());

        // Should find closest
        let hit = index.hit_test([48.0, 0.0], 15.0);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().id, "b");
    }

    #[test]
    fn test_rect_query() {
        let mut index = SpatialIndex::new();
        let nodes = vec![
            SpatialNode::new("a", [10.0, 10.0], 5.0),
            SpatialNode::new("b", [50.0, 10.0], 5.0),
            SpatialNode::new("c", [10.0, 50.0], 5.0),
            SpatialNode::new("d", [50.0, 50.0], 5.0),
        ];
        index.rebuild(nodes.into_iter());

        // Query should find nodes in rectangle
        let results = index.query_rect([0.0, 0.0], [30.0, 30.0]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "a");

        // Larger query
        let results = index.query_rect([0.0, 0.0], [60.0, 60.0]);
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_radius_query() {
        let mut index = SpatialIndex::new();
        let nodes = vec![
            SpatialNode::new("a", [0.0, 0.0], 5.0),
            SpatialNode::new("b", [20.0, 0.0], 5.0),
            SpatialNode::new("c", [100.0, 0.0], 5.0),
        ];
        index.rebuild(nodes.into_iter());

        // Should find nearby nodes
        let results = index.query_radius([10.0, 0.0], 20.0);
        assert_eq!(results.len(), 2); // a and b
    }

    #[test]
    fn test_nearest() {
        let mut index = SpatialIndex::new();
        let nodes = vec![
            SpatialNode::new("far", [100.0, 100.0], 5.0),
            SpatialNode::new("near", [10.0, 10.0], 5.0),
            SpatialNode::new("medium", [50.0, 50.0], 5.0),
        ];
        index.rebuild(nodes.into_iter());

        let nearest = index.nearest([0.0, 0.0]);
        assert!(nearest.is_some());
        assert_eq!(nearest.unwrap().id, "near");
    }

    #[test]
    fn test_nearest_n() {
        let mut index = SpatialIndex::new();
        let nodes = vec![
            SpatialNode::new("a", [10.0, 0.0], 5.0),
            SpatialNode::new("b", [20.0, 0.0], 5.0),
            SpatialNode::new("c", [30.0, 0.0], 5.0),
            SpatialNode::new("d", [100.0, 0.0], 5.0),
        ];
        index.rebuild(nodes.into_iter());

        let results = index.nearest_n([0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_find_by_id() {
        let mut index = SpatialIndex::new();
        let nodes = vec![
            SpatialNode::new("alpha", [0.0, 0.0], 5.0),
            SpatialNode::new("beta", [50.0, 0.0], 5.0),
        ];
        index.rebuild(nodes.into_iter());

        assert!(index.find_by_id("alpha").is_some());
        assert!(index.find_by_id("gamma").is_none());
    }

    #[test]
    fn test_dirty_flag() {
        let mut index = SpatialIndex::new();
        assert!(!index.is_dirty());

        index.mark_dirty();
        assert!(index.is_dirty());

        index.rebuild(std::iter::empty());
        assert!(!index.is_dirty());
    }

    #[test]
    fn test_node_with_type() {
        let node = SpatialNode::new("test", [0.0, 0.0], 10.0).with_type("entity");
        assert_eq!(node.node_type, Some("entity".to_string()));
    }

    #[test]
    fn test_with_bounds() {
        let node = SpatialNode::with_bounds("rect", [-10.0, -5.0], [10.0, 5.0]);
        assert_eq!(node.center, [0.0, 0.0]);
        assert!(node.radius > 0.0);
    }

    #[test]
    fn test_performance_500_nodes() {
        let mut index = SpatialIndex::new();
        let nodes = create_test_nodes(500);
        index.rebuild(nodes.into_iter());

        assert_eq!(index.len(), 500);

        // Hit test should be fast (O(log n))
        let hit = index.hit_test([500.0, 100.0], 15.0);
        assert!(hit.is_some());

        // Rect query
        let results = index.query_rect([0.0, 0.0], [200.0, 100.0]);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_iter() {
        let mut index = SpatialIndex::new();
        let nodes = vec![
            SpatialNode::new("a", [0.0, 0.0], 5.0),
            SpatialNode::new("b", [50.0, 0.0], 5.0),
        ];
        index.rebuild(nodes.into_iter());

        let ids: Vec<_> = index.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
    }
}
