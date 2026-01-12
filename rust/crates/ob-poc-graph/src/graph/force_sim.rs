//! Force Simulation for Galaxy/Cluster Visualization
//!
//! Implements a simple force-directed layout that runs continuously.
//! Designed for 10-200 nodes (clusters or funds), not thousands.
//!
//! Features:
//! - Repulsion between all nodes (prevents overlap)
//! - Optional attraction to cluster centers
//! - Zoom-responsive compression (nodes collapse at low zoom)
//! - Boundary containment
//!
//! # Usage
//! ```ignore
//! let mut sim = ForceSimulation::new();
//! sim.add_node(ClusterNode { id: "LU".into(), label: "Luxembourg".into(), count: 177, .. });
//!
//! // Each frame:
//! sim.set_zoom(camera.zoom());
//! sim.tick(dt);
//! for node in sim.nodes() {
//!     draw_circle(node.position, node.display_radius());
//! }
//! ```

use egui::{Pos2, Vec2};
use std::collections::HashMap;

// =============================================================================
// CLUSTER NODE
// =============================================================================

/// A node in the force simulation (represents a cluster or individual fund)
#[derive(Debug, Clone)]
pub struct ClusterNode {
    /// Unique identifier
    pub id: String,

    /// Display label (e.g., "Luxembourg" or "LU (177)")
    pub label: String,

    /// Short label for compressed view (e.g., "LU")
    pub short_label: String,

    /// Number of items in this cluster (affects radius)
    pub count: usize,

    /// Current position (updated by simulation)
    pub position: Pos2,

    /// Current velocity
    velocity: Vec2,

    /// Base radius (before zoom scaling)
    pub base_radius: f32,

    /// Cluster this node belongs to (for cluster attraction)
    pub cluster_id: Option<String>,

    /// Parent node ID (for hierarchical attraction)
    pub parent_id: Option<String>,

    /// Color for rendering
    pub color: egui::Color32,

    /// Is this node being dragged?
    pub pinned: bool,
}

impl ClusterNode {
    /// Create a new cluster node
    pub fn new(id: impl Into<String>, label: impl Into<String>, count: usize) -> Self {
        let id = id.into();
        let label = label.into();
        let short_label = if label.len() > 4 {
            label.chars().take(2).collect::<String>().to_uppercase()
        } else {
            label.clone()
        };

        Self {
            id,
            label,
            short_label,
            count,
            position: Pos2::ZERO,
            velocity: Vec2::ZERO,
            base_radius: Self::radius_for_count(count),
            cluster_id: None,
            parent_id: None,
            color: egui::Color32::from_rgb(100, 149, 237), // Cornflower blue
            pinned: false,
        }
    }

    /// Calculate radius based on item count (square root scaling)
    pub fn radius_for_count(count: usize) -> f32 {
        let min_radius = 20.0;
        let scale = 3.0;
        min_radius + (count as f32).sqrt() * scale
    }

    /// Get display radius adjusted for zoom/compression
    pub fn display_radius(&self, compression: f32) -> f32 {
        let min_compressed = 8.0;
        let full = self.base_radius;
        min_compressed + (full - min_compressed) * (1.0 - compression)
    }

    /// Builder: set position
    pub fn with_position(mut self, pos: Pos2) -> Self {
        self.position = pos;
        self
    }

    /// Builder: set color
    pub fn with_color(mut self, color: egui::Color32) -> Self {
        self.color = color;
        self
    }

    /// Builder: set parent
    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }
}

// =============================================================================
// SIMULATION CONFIG
// =============================================================================

/// Configuration for the force simulation
#[derive(Debug, Clone)]
pub struct ForceConfig {
    /// Repulsion strength between nodes
    pub repulsion: f32,

    /// Attraction to center (keeps nodes from flying away)
    pub center_attraction: f32,

    /// Attraction to cluster center (nodes in same cluster attract)
    pub cluster_attraction: f32,

    /// Attraction to parent node (hierarchical layouts)
    pub parent_attraction: f32,

    /// Velocity damping (0.0 = no damping, 1.0 = instant stop)
    pub damping: f32,

    /// Minimum distance for force calculation (prevents explosion)
    pub min_distance: f32,

    /// Maximum velocity (prevents instability)
    pub max_velocity: f32,

    /// Boundary radius (soft containment)
    pub boundary_radius: f32,

    /// Boundary stiffness (how hard the boundary pushes back)
    pub boundary_stiffness: f32,
}

/// Reference viewport size for scaling calculations
const REFERENCE_VIEWPORT_WIDTH: f32 = 800.0;
const REFERENCE_VIEWPORT_HEIGHT: f32 = 600.0;
const REFERENCE_VIEWPORT_AREA: f32 = REFERENCE_VIEWPORT_WIDTH * REFERENCE_VIEWPORT_HEIGHT;

impl Default for ForceConfig {
    fn default() -> Self {
        Self {
            repulsion: 8000.0,
            center_attraction: 0.02,
            cluster_attraction: 0.05,
            parent_attraction: 0.03,
            damping: 0.9,
            min_distance: 50.0,
            max_velocity: 500.0,
            boundary_radius: 400.0,
            boundary_stiffness: 0.5,
        }
    }
}

impl ForceConfig {
    /// Scale boundary radius based on viewport dimensions.
    ///
    /// Larger viewport = larger boundary, allowing nodes more room to spread.
    /// Uses the smaller dimension to maintain reasonable aspect ratio.
    pub fn scale_to_viewport(&mut self, width: f32, height: f32) {
        let min_dim = width.min(height);
        // Use 80% of the smaller dimension as boundary, with a minimum
        let scaled_boundary = (min_dim * 0.4).max(200.0);
        self.boundary_radius = scaled_boundary;

        // Also scale repulsion slightly - larger viewport can handle more spread
        let area_ratio = (width * height) / REFERENCE_VIEWPORT_AREA;
        let repulsion_scale = area_ratio.sqrt().clamp(0.7, 1.5);
        self.repulsion = 8000.0 * repulsion_scale;
    }

    /// Create config scaled for a specific viewport size
    pub fn for_viewport(width: f32, height: f32) -> Self {
        let mut config = Self::default();
        config.scale_to_viewport(width, height);
        config
    }
}

impl ForceConfig {
    /// Config for galaxy view (few large clusters)
    pub fn galaxy() -> Self {
        Self {
            repulsion: 15000.0,
            center_attraction: 0.01,
            cluster_attraction: 0.08,
            parent_attraction: 0.02,
            damping: 0.85,
            min_distance: 80.0,
            max_velocity: 300.0,
            boundary_radius: 500.0,
            boundary_stiffness: 0.3,
        }
    }

    /// Config for solar system view (more nodes, tighter clustering)
    pub fn solar_system() -> Self {
        Self {
            repulsion: 5000.0,
            center_attraction: 0.03,
            cluster_attraction: 0.04,
            parent_attraction: 0.05,
            damping: 0.92,
            min_distance: 40.0,
            max_velocity: 400.0,
            boundary_radius: 600.0,
            boundary_stiffness: 0.4,
        }
    }
}

// =============================================================================
// FORCE SIMULATION
// =============================================================================

/// Force-directed layout simulation
///
/// Runs continuously, updating node positions each frame based on:
/// - Repulsion between nodes (inverse square law)
/// - Attraction to center
/// - Boundary containment
/// - Zoom-responsive compression
#[derive(Debug, Clone)]
pub struct ForceSimulation {
    /// Nodes in the simulation
    nodes: Vec<ClusterNode>,

    /// Quick lookup by ID
    node_index: HashMap<String, usize>,

    /// Simulation configuration
    pub config: ForceConfig,

    /// Current zoom level (affects compression)
    zoom: f32,

    /// Compression factor (0.0 = expanded, 1.0 = collapsed)
    /// Derived from zoom level
    compression: f32,

    /// Is simulation running?
    pub running: bool,

    /// Center point for the simulation
    pub center: Pos2,

    /// Has the simulation stabilized?
    stabilized: bool,

    /// Kinetic energy (for stability detection)
    energy: f32,
}

impl Default for ForceSimulation {
    fn default() -> Self {
        Self::new()
    }
}

impl ForceSimulation {
    /// Create a new empty simulation
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            node_index: HashMap::new(),
            config: ForceConfig::default(),
            zoom: 1.0,
            compression: 0.0,
            running: true,
            center: Pos2::ZERO,
            stabilized: false,
            energy: f32::MAX,
        }
    }

    /// Create with specific config
    pub fn with_config(config: ForceConfig) -> Self {
        Self {
            config,
            ..Self::new()
        }
    }

    // =========================================================================
    // NODE MANAGEMENT
    // =========================================================================

    /// Add a node to the simulation
    pub fn add_node(&mut self, mut node: ClusterNode) {
        // Assign random initial position if at origin
        if node.position == Pos2::ZERO {
            let angle = (self.nodes.len() as f32) * 2.39996; // Golden angle
            let radius = 100.0 + (self.nodes.len() as f32) * 20.0;
            node.position = self.center + Vec2::angled(angle) * radius;
        }

        let idx = self.nodes.len();
        self.node_index.insert(node.id.clone(), idx);
        self.nodes.push(node);
        self.stabilized = false;
    }

    /// Add multiple nodes
    pub fn add_nodes(&mut self, nodes: impl IntoIterator<Item = ClusterNode>) {
        for node in nodes {
            self.add_node(node);
        }
    }

    /// Clear all nodes
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.node_index.clear();
        self.stabilized = false;
    }

    /// Get node by ID
    pub fn get_node(&self, id: &str) -> Option<&ClusterNode> {
        self.node_index.get(id).map(|&idx| &self.nodes[idx])
    }

    /// Get mutable node by ID
    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut ClusterNode> {
        self.node_index
            .get(id)
            .copied()
            .map(|idx| &mut self.nodes[idx])
    }

    /// Iterate over all nodes
    pub fn nodes(&self) -> &[ClusterNode] {
        &self.nodes
    }

    /// Number of nodes
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Is empty?
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    // =========================================================================
    // ZOOM / COMPRESSION
    // =========================================================================

    /// Set current zoom level
    /// Compression increases as zoom decreases
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;

        // Compression curve: at zoom 1.0 = 0% compressed, at zoom 0.3 = 100% compressed
        self.compression = ((1.0 - zoom) / 0.7).clamp(0.0, 1.0);
    }

    /// Get current compression factor (0.0 = expanded, 1.0 = collapsed)
    pub fn compression(&self) -> f32 {
        self.compression
    }

    /// Get current zoom
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    // =========================================================================
    // SIMULATION
    // =========================================================================

    /// Run one simulation step
    ///
    /// Call this each frame with delta time in seconds
    pub fn tick(&mut self, dt: f32) {
        if !self.running || self.nodes.is_empty() {
            return;
        }

        let dt = dt.min(0.05); // Cap dt to prevent instability

        // Calculate forces
        let forces = self.calculate_forces();

        // Apply forces and update positions
        let mut total_energy = 0.0;

        for (i, node) in self.nodes.iter_mut().enumerate() {
            if node.pinned {
                node.velocity = Vec2::ZERO;
                continue;
            }

            // Apply force as acceleration (F = ma, assume m = 1)
            node.velocity += forces[i] * dt;

            // Apply damping
            node.velocity *= self.config.damping;

            // Clamp velocity
            let speed = node.velocity.length();
            if speed > self.config.max_velocity {
                node.velocity = node.velocity.normalized() * self.config.max_velocity;
            }

            // Update position
            node.position += node.velocity * dt;

            // Track energy for stability detection
            total_energy += speed * speed;
        }

        self.energy = total_energy;
        self.stabilized = total_energy < 1.0;
    }

    /// Calculate forces on all nodes
    fn calculate_forces(&self) -> Vec<Vec2> {
        let n = self.nodes.len();
        let mut forces = vec![Vec2::ZERO; n];

        // Repulsion between all pairs
        for i in 0..n {
            for j in (i + 1)..n {
                let delta = self.nodes[i].position - self.nodes[j].position;
                let dist = delta.length().max(self.config.min_distance);

                // Inverse square repulsion, scaled by node radii
                let combined_radius = self.nodes[i].base_radius + self.nodes[j].base_radius;
                let ideal_dist = combined_radius * 1.5;

                if dist < ideal_dist * 2.0 {
                    let force_mag = self.config.repulsion / (dist * dist);
                    let force = delta.normalized() * force_mag;

                    forces[i] += force;
                    forces[j] -= force;
                }
            }
        }

        // Center attraction and boundary containment
        for (i, node) in self.nodes.iter().enumerate() {
            // Attraction to center
            let to_center = self.center - node.position;
            forces[i] += to_center * self.config.center_attraction;

            // Soft boundary
            let dist_from_center = to_center.length();
            if dist_from_center > self.config.boundary_radius {
                let overshoot = dist_from_center - self.config.boundary_radius;
                let push = to_center.normalized() * overshoot * self.config.boundary_stiffness;
                forces[i] += push;
            }

            // Parent attraction (hierarchical layout)
            if let Some(ref parent_id) = node.parent_id {
                if let Some(&parent_idx) = self.node_index.get(parent_id) {
                    let to_parent = self.nodes[parent_idx].position - node.position;
                    forces[i] += to_parent * self.config.parent_attraction;
                }
            }
        }

        // Cluster attraction (nodes in same cluster attract to cluster centroid)
        if self.config.cluster_attraction > 0.0 {
            // Build cluster centroids
            let mut cluster_sums: HashMap<String, (Pos2, usize)> = HashMap::new();
            for node in &self.nodes {
                if let Some(ref cid) = node.cluster_id {
                    let entry = cluster_sums.entry(cid.clone()).or_insert((Pos2::ZERO, 0));
                    entry.0 += node.position.to_vec2();
                    entry.1 += 1;
                }
            }
            let cluster_centers: HashMap<String, Pos2> = cluster_sums
                .into_iter()
                .filter(|(_, (_, count))| *count > 1)
                .map(|(cid, (sum, count))| {
                    (cid, Pos2::new(sum.x / count as f32, sum.y / count as f32))
                })
                .collect();

            // Apply attraction to cluster center
            for (i, node) in self.nodes.iter().enumerate() {
                if let Some(ref cid) = node.cluster_id {
                    if let Some(&center) = cluster_centers.get(cid) {
                        let to_center = center - node.position;
                        forces[i] += to_center * self.config.cluster_attraction;
                    }
                }
            }
        }

        forces
    }

    /// Check if simulation has stabilized
    pub fn is_stable(&self) -> bool {
        self.stabilized
    }

    /// Get current kinetic energy
    pub fn energy(&self) -> f32 {
        self.energy
    }

    /// Kick the simulation (add random velocity to destabilize)
    pub fn kick(&mut self) {
        use std::f32::consts::PI;
        let n = self.nodes.len() as f32;
        for (i, node) in self.nodes.iter_mut().enumerate() {
            let angle = (i as f32) * 2.0 * PI / n;
            node.velocity += Vec2::angled(angle) * 50.0;
        }
        self.stabilized = false;
    }

    // =========================================================================
    // HIT TESTING
    // =========================================================================

    /// Find node at position (for click handling)
    pub fn node_at(&self, pos: Pos2) -> Option<&ClusterNode> {
        for node in self.nodes.iter().rev() {
            // Reverse for top-first
            let radius = node.display_radius(self.compression);
            if (pos - node.position).length() <= radius {
                return Some(node);
            }
        }
        None
    }

    /// Find node ID at position
    pub fn node_id_at(&self, pos: Pos2) -> Option<&str> {
        self.node_at(pos).map(|n| n.id.as_str())
    }

    // =========================================================================
    // PINNING (for drag)
    // =========================================================================

    /// Pin a node (stops it from moving)
    pub fn pin(&mut self, id: &str) {
        if let Some(node) = self.get_node_mut(id) {
            node.pinned = true;
            node.velocity = Vec2::ZERO;
        }
    }

    /// Unpin a node
    pub fn unpin(&mut self, id: &str) {
        if let Some(node) = self.get_node_mut(id) {
            node.pinned = false;
        }
    }

    /// Move a pinned node
    pub fn move_node(&mut self, id: &str, new_pos: Pos2) {
        if let Some(node) = self.get_node_mut(id) {
            node.position = new_pos;
        }
    }

    // =========================================================================
    // VIEWPORT SCALING
    // =========================================================================

    /// Update simulation bounds for new viewport size.
    ///
    /// Call this when the viewport is resized. Updates:
    /// - Boundary radius (containment area)
    /// - Repulsion strength (scaled for viewport area)
    /// - Center point (optional, if viewport center changed)
    pub fn set_viewport_size(&mut self, width: f32, height: f32) {
        self.config.scale_to_viewport(width, height);
        // Optionally update center to viewport center
        self.center = Pos2::new(width / 2.0, height / 2.0);
        // Destabilize to allow re-layout
        self.stabilized = false;
    }

    /// Update just the center point (e.g., when viewport pans)
    pub fn set_center(&mut self, center: Pos2) {
        self.center = center;
    }

    /// Get the current boundary radius
    pub fn boundary_radius(&self) -> f32 {
        self.config.boundary_radius
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_nodes() {
        let mut sim = ForceSimulation::new();
        sim.add_node(ClusterNode::new("LU", "Luxembourg", 177));
        sim.add_node(ClusterNode::new("IE", "Ireland", 150));

        assert_eq!(sim.len(), 2);
        assert!(sim.get_node("LU").is_some());
        assert!(sim.get_node("IE").is_some());
    }

    #[test]
    fn test_simulation_runs() {
        let mut sim = ForceSimulation::new();
        sim.add_node(ClusterNode::new("A", "Node A", 10).with_position(Pos2::new(0.0, 0.0)));
        sim.add_node(ClusterNode::new("B", "Node B", 10).with_position(Pos2::new(10.0, 0.0)));

        let initial_dist = (sim.nodes[0].position - sim.nodes[1].position).length();

        // Run simulation
        for _ in 0..100 {
            sim.tick(1.0 / 60.0);
        }

        let final_dist = (sim.nodes[0].position - sim.nodes[1].position).length();

        // Nodes should have repelled each other
        assert!(final_dist > initial_dist);
    }

    #[test]
    fn test_compression() {
        let mut sim = ForceSimulation::new();

        sim.set_zoom(1.0);
        assert_eq!(sim.compression(), 0.0);

        sim.set_zoom(0.3);
        assert!((sim.compression() - 1.0).abs() < 0.01);

        sim.set_zoom(0.65);
        assert!((sim.compression() - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_node_radius_scaling() {
        let small = ClusterNode::new("S", "Small", 10);
        let large = ClusterNode::new("L", "Large", 200);

        assert!(large.base_radius > small.base_radius);

        // Compressed radius should be smaller
        assert!(small.display_radius(1.0) < small.display_radius(0.0));
    }

    #[test]
    fn test_hit_testing() {
        let mut sim = ForceSimulation::new();
        sim.add_node(ClusterNode::new("A", "Node A", 50).with_position(Pos2::new(100.0, 100.0)));

        assert!(sim.node_at(Pos2::new(100.0, 100.0)).is_some());
        assert!(sim.node_at(Pos2::new(100.0, 110.0)).is_some()); // Within radius
        assert!(sim.node_at(Pos2::new(500.0, 500.0)).is_none()); // Far away
    }
}
