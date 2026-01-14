//! Node aggregation for galaxy view
//!
//! When there are too many CBUs to display, aggregate them into:
//! - Jurisdiction galaxies (LU(47), DE(23), US(89))
//! - ManCo/segment regions within jurisdictions
//!
//! Cap guarantee: never render more than MAX_VISIBLE_NODES nodes.

use std::collections::HashMap;

use egui::{Color32, Pos2, Vec2};

use super::types::{EntityType, LayoutGraph, LayoutNode, NodeStyle, PrimaryRole};
use super::viewport_fit::{ViewLevel, MAX_VISIBLE_NODES};

// =============================================================================
// AGGREGATED NODE TYPES
// =============================================================================

/// A galaxy node representing a jurisdiction or segment
#[derive(Debug, Clone)]
pub struct GalaxyNode {
    /// Unique ID (e.g., "galaxy-LU", "galaxy-DE")
    pub id: String,

    /// Display label (e.g., "LU", "DE", "US")
    pub label: String,

    /// Count of CBUs in this galaxy
    pub count: usize,

    /// Child CBU IDs contained in this galaxy
    pub children: Vec<String>,

    /// Computed position for rendering
    pub position: Pos2,

    /// Computed radius based on count
    pub radius: f32,

    /// Color for rendering
    pub color: Color32,
}

impl GalaxyNode {
    /// Create a new galaxy node
    pub fn new(jurisdiction: &str, children: Vec<String>) -> Self {
        let count = children.len();
        Self {
            id: format!("galaxy-{}", jurisdiction),
            label: jurisdiction.to_string(),
            count,
            children,
            position: Pos2::ZERO,
            radius: Self::radius_for_count(count),
            color: Self::color_for_jurisdiction(jurisdiction),
        }
    }

    /// Calculate radius based on count (square root scaling)
    fn radius_for_count(count: usize) -> f32 {
        let min_radius = 30.0;
        let scale = 5.0;
        min_radius + (count as f32).sqrt() * scale
    }

    /// Get color based on jurisdiction
    fn color_for_jurisdiction(jur: &str) -> Color32 {
        match jur {
            "LU" => Color32::from_rgb(100, 149, 237), // Cornflower blue
            "DE" => Color32::from_rgb(255, 193, 7),   // Amber
            "US" => Color32::from_rgb(76, 175, 80),   // Green
            "IE" => Color32::from_rgb(156, 39, 176),  // Purple
            "UK" | "GB" => Color32::from_rgb(244, 67, 54), // Red
            "FR" => Color32::from_rgb(33, 150, 243),  // Blue
            "NL" => Color32::from_rgb(255, 152, 0),   // Orange
            "CH" => Color32::from_rgb(233, 30, 99),   // Pink
            _ => Color32::from_rgb(158, 158, 158),    // Gray
        }
    }

    /// Convert to a LayoutNode for rendering
    pub fn to_layout_node(&self) -> LayoutNode {
        let size = Vec2::splat(self.radius * 2.0);
        LayoutNode {
            id: self.id.clone(),
            entity_type: EntityType::Unknown, // Aggregated node
            primary_role: PrimaryRole::Unknown,
            all_roles: vec![],
            label: self.label.clone(),
            sublabel: Some(format!("{} CBUs", self.count)),
            jurisdiction: Some(self.label.clone()),
            status: None,
            base_position: self.position,
            offset: Vec2::ZERO,
            position: self.position,
            base_size: size,
            size_override: None,
            size,
            in_focus: true,
            is_cbu_root: false,
            style: NodeStyle {
                fill_color: self.color,
                border_color: self.color.gamma_multiply(1.3),
                text_color: Color32::WHITE,
                border_width: 3.0,
            },
            importance: 1.0,
            hierarchy_depth: 0,
            kyc_completion: None,
            verification_summary: None,
            needs_attention: false,
            entity_category: Some("CLUSTER".to_string()),
            person_state: None,
            is_container: true,
            contains_type: Some("cbu".to_string()),
            child_count: Some(self.count as i64),
            browse_nickname: None,
            parent_key: None,
            container_parent_id: None,
            control_confidence: None,
            control_explanation: None,
            control_data_gaps: None,
            control_rule: None,
            cluster_id: None,
            parent_id: None,
        }
    }
}

/// A region node representing a ManCo or segment within a jurisdiction
#[derive(Debug, Clone)]
pub struct RegionNode {
    /// Unique ID (e.g., "region-LU-Allianz")
    pub id: String,

    /// Display label (e.g., "Allianz", "DWS")
    pub label: String,

    /// Parent jurisdiction
    pub jurisdiction: String,

    /// Count of CBUs in this region
    pub count: usize,

    /// Child CBU IDs
    pub children: Vec<String>,

    /// Position
    pub position: Pos2,

    /// Radius
    pub radius: f32,

    /// Color
    pub color: Color32,
}

impl RegionNode {
    /// Create a new region node
    pub fn new(label: &str, jurisdiction: &str, children: Vec<String>) -> Self {
        let count = children.len();
        Self {
            id: format!("region-{}-{}", jurisdiction, label.replace(' ', "_")),
            label: label.to_string(),
            jurisdiction: jurisdiction.to_string(),
            count,
            children,
            position: Pos2::ZERO,
            radius: Self::radius_for_count(count),
            color: GalaxyNode::color_for_jurisdiction(jurisdiction).gamma_multiply(0.8),
        }
    }

    fn radius_for_count(count: usize) -> f32 {
        let min_radius = 25.0;
        let scale = 4.0;
        min_radius + (count as f32).sqrt() * scale
    }

    /// Convert to a LayoutNode for rendering
    pub fn to_layout_node(&self) -> LayoutNode {
        let size = Vec2::splat(self.radius * 2.0);
        LayoutNode {
            id: self.id.clone(),
            entity_type: EntityType::Unknown, // Aggregated node
            primary_role: PrimaryRole::Unknown,
            all_roles: vec![],
            label: self.label.clone(),
            sublabel: Some(format!("{} CBUs", self.count)),
            jurisdiction: Some(self.jurisdiction.clone()),
            status: None,
            base_position: self.position,
            offset: Vec2::ZERO,
            position: self.position,
            base_size: size,
            size_override: None,
            size,
            in_focus: true,
            is_cbu_root: false,
            style: NodeStyle {
                fill_color: self.color,
                border_color: self.color.gamma_multiply(1.3),
                text_color: Color32::WHITE,
                border_width: 2.0,
            },
            importance: 0.9,
            hierarchy_depth: 1,
            kyc_completion: None,
            verification_summary: None,
            needs_attention: false,
            entity_category: Some("CLUSTER".to_string()),
            person_state: None,
            is_container: true,
            contains_type: Some("cbu".to_string()),
            child_count: Some(self.count as i64),
            browse_nickname: None,
            parent_key: None,
            container_parent_id: None,
            control_confidence: None,
            control_explanation: None,
            control_data_gaps: None,
            control_rule: None,
            cluster_id: None,
            parent_id: None,
        }
    }
}

// =============================================================================
// AGGREGATION FUNCTIONS
// =============================================================================

/// Aggregate CBUs into jurisdiction galaxies
///
/// Groups all CBUs by jurisdiction and returns GalaxyNode for each jurisdiction.
pub fn aggregate_to_galaxies(graph: &LayoutGraph) -> Vec<GalaxyNode> {
    let mut by_jurisdiction: HashMap<String, Vec<String>> = HashMap::new();

    for (id, node) in &graph.nodes {
        // Only aggregate CBU root nodes
        if node.is_cbu_root {
            let jur = node
                .jurisdiction
                .clone()
                .unwrap_or_else(|| "XX".to_string());
            by_jurisdiction.entry(jur).or_default().push(id.clone());
        }
    }

    let mut galaxies: Vec<GalaxyNode> = by_jurisdiction
        .into_iter()
        .map(|(jur, children)| GalaxyNode::new(&jur, children))
        .collect();

    // Sort by count descending
    galaxies.sort_by(|a, b| b.count.cmp(&a.count));

    // Arrange in a circle
    arrange_in_circle(&mut galaxies);

    galaxies
}

/// Aggregate CBUs within a jurisdiction into ManCo/segment regions
///
/// Groups CBUs by their ManCo or segment (extracted from name or metadata).
pub fn aggregate_to_regions(graph: &LayoutGraph, jurisdiction: Option<&str>) -> Vec<RegionNode> {
    let mut by_segment: HashMap<(String, String), Vec<String>> = HashMap::new();

    for (id, node) in &graph.nodes {
        if !node.is_cbu_root {
            continue;
        }

        let jur = node
            .jurisdiction
            .clone()
            .unwrap_or_else(|| "XX".to_string());

        // Filter by jurisdiction if specified
        if let Some(filter_jur) = jurisdiction {
            if jur != filter_jur {
                continue;
            }
        }

        // Extract segment from label (first word, or "Other" if short)
        let segment = extract_segment(&node.label);
        by_segment
            .entry((jur, segment))
            .or_default()
            .push(id.clone());
    }

    let mut regions: Vec<RegionNode> = by_segment
        .into_iter()
        .map(|((jur, segment), children)| RegionNode::new(&segment, &jur, children))
        .collect();

    // Sort by count descending
    regions.sort_by(|a, b| b.count.cmp(&a.count));

    // Arrange in a grid or circle
    arrange_regions(&mut regions);

    regions
}

/// Extract segment/ManCo name from CBU label
fn extract_segment(label: &str) -> String {
    // Try to extract the first word as the ManCo/segment name
    // e.g., "Allianz Global Investors SICAV" -> "Allianz"
    let words: Vec<&str> = label.split_whitespace().collect();

    if words.is_empty() {
        return "Other".to_string();
    }

    // Use first word if it's a reasonable segment name
    let first = words[0];
    if first.len() >= 3 && first.chars().next().map_or(false, |c| c.is_uppercase()) {
        first.to_string()
    } else {
        "Other".to_string()
    }
}

/// Arrange galaxy nodes in a circle
fn arrange_in_circle(galaxies: &mut [GalaxyNode]) {
    let n = galaxies.len();
    if n == 0 {
        return;
    }

    let radius = 200.0 + (n as f32 * 30.0);

    for (i, galaxy) in galaxies.iter_mut().enumerate() {
        let angle = (i as f32 / n as f32) * std::f32::consts::TAU;
        galaxy.position = Pos2::new(angle.cos() * radius, angle.sin() * radius);
    }
}

/// Arrange region nodes in a grid-like pattern
fn arrange_regions(regions: &mut [RegionNode]) {
    let n = regions.len();
    if n == 0 {
        return;
    }

    let cols = ((n as f32).sqrt().ceil() as usize).max(1);
    let spacing = 150.0;
    let start_x = -((cols - 1) as f32 * spacing) / 2.0;
    let start_y = -(((n / cols) as f32) * spacing) / 2.0;

    for (i, region) in regions.iter_mut().enumerate() {
        let col = i % cols;
        let row = i / cols;
        region.position = Pos2::new(
            start_x + col as f32 * spacing,
            start_y + row as f32 * spacing,
        );
    }
}

/// Create an aggregated layout graph based on view level
///
/// Returns a new LayoutGraph with aggregated nodes replacing individual CBUs
/// when the view level requires it.
pub fn create_aggregated_graph(
    source_graph: &LayoutGraph,
    view_level: ViewLevel,
    focused_jurisdiction: Option<&str>,
) -> LayoutGraph {
    match view_level {
        ViewLevel::Galaxy => {
            // Aggregate to jurisdiction galaxies
            let galaxies = aggregate_to_galaxies(source_graph);
            let mut graph = LayoutGraph::default();

            for galaxy in galaxies {
                graph
                    .nodes
                    .insert(galaxy.id.clone(), galaxy.to_layout_node());
            }

            // Compute bounds
            graph.bounds = compute_bounds(&graph);
            graph
        }

        ViewLevel::Region => {
            // Aggregate to ManCo regions
            let regions = aggregate_to_regions(source_graph, focused_jurisdiction);
            let mut graph = LayoutGraph::default();

            for region in regions {
                graph
                    .nodes
                    .insert(region.id.clone(), region.to_layout_node());
            }

            graph.bounds = compute_bounds(&graph);
            graph
        }

        ViewLevel::Cluster | ViewLevel::Solar => {
            // No aggregation - use source graph
            // But cap at MAX_VISIBLE_NODES if needed
            if source_graph.nodes.len() <= MAX_VISIBLE_NODES {
                source_graph.clone()
            } else {
                // Take first MAX_VISIBLE_NODES by importance
                let mut nodes: Vec<_> = source_graph.nodes.iter().collect();
                nodes.sort_by(|a, b| {
                    b.1.importance
                        .partial_cmp(&a.1.importance)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let mut graph = LayoutGraph::default();
                for (id, node) in nodes.into_iter().take(MAX_VISIBLE_NODES) {
                    graph.nodes.insert(id.clone(), node.clone());
                }

                // Keep edges that connect visible nodes
                for edge in &source_graph.edges {
                    if graph.nodes.contains_key(&edge.source_id)
                        && graph.nodes.contains_key(&edge.target_id)
                    {
                        graph.edges.push(edge.clone());
                    }
                }

                graph.bounds = compute_bounds(&graph);
                graph
            }
        }
    }
}

/// Compute bounding box of all nodes
fn compute_bounds(graph: &LayoutGraph) -> egui::Rect {
    if graph.nodes.is_empty() {
        return egui::Rect::from_center_size(Pos2::ZERO, Vec2::new(100.0, 100.0));
    }

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for node in graph.nodes.values() {
        let half_size = node.size / 2.0;
        min_x = min_x.min(node.position.x - half_size.x);
        min_y = min_y.min(node.position.y - half_size.y);
        max_x = max_x.max(node.position.x + half_size.x);
        max_y = max_y.max(node.position.y + half_size.y);
    }

    egui::Rect::from_min_max(
        Pos2::new(min_x - 50.0, min_y - 50.0),
        Pos2::new(max_x + 50.0, max_y + 50.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_segment() {
        assert_eq!(extract_segment("Allianz Global Investors"), "Allianz");
        assert_eq!(extract_segment("DWS Fund"), "DWS");
        assert_eq!(extract_segment("a small name"), "Other");
        assert_eq!(extract_segment(""), "Other");
    }

    #[test]
    fn test_galaxy_node_radius() {
        let small = GalaxyNode::new("LU", vec!["a".to_string()]);
        let large = GalaxyNode::new("DE", (0..100).map(|i| format!("cbu-{}", i)).collect());

        assert!(large.radius > small.radius);
    }
}
