//! Board Control View - Ownership tree layout and rendering
//!
//! This module provides:
//! - Hierarchical tree layout for ownership chains (flows upward to UBOs)
//! - Control edge rendering with BODS interest labels
//! - PSC category badges on person nodes
//! - Path highlighting for the winning control rule
//! - Evidence panel rendering
//! - Breadcrumb navigation back to source CBU

use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::types::{EdgeType, EntityType, LayoutEdge, LayoutGraph, LayoutNode, NodeStyle};

// =============================================================================
// CONTROL SPHERE DATA (from API)
// =============================================================================

/// Control sphere response from GET /api/control-sphere/{entity_id}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlSphereData {
    /// Anchor entity ID
    pub anchor_entity_id: Uuid,
    /// Anchor entity name
    pub anchor_entity_name: String,
    /// Nodes in the control sphere (ownership chain)
    pub nodes: Vec<ControlSphereNode>,
    /// Edges in the control sphere
    pub edges: Vec<ControlSphereEdge>,
    /// Depth of the tree
    pub depth: u8,
}

/// A node in the control sphere
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlSphereNode {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: String,
    /// Jurisdiction code (e.g., "DE", "LU", "GB")
    pub jurisdiction: Option<String>,
    /// Is this a natural person (proper person)?
    pub is_natural_person: bool,
    /// Is this a UBO (ultimate beneficial owner)?
    pub is_ubo: bool,
    /// PSC categories this person meets (e.g., ">25% shares", ">50% votes", "Appoints board")
    #[serde(default)]
    pub psc_categories: Vec<String>,
    /// Depth in the ownership tree (0 = anchor)
    pub depth: u8,
    /// Whether this node is on the winning control path
    pub on_winning_path: bool,
    /// Source register (GLEIF, UK_PSC, LUX_RBE, MANUAL)
    pub source: Option<String>,
}

/// An edge in the control sphere
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlSphereEdge {
    pub from_entity_id: Uuid,
    pub to_entity_id: Uuid,
    /// BODS interest type
    pub interest_type: String,
    /// Ownership percentage (if applicable)
    pub ownership_pct: Option<f64>,
    /// Voting rights percentage (if applicable)
    pub voting_pct: Option<f64>,
    /// Whether this edge is on the winning control path
    pub on_winning_path: bool,
    /// Source register
    pub source: Option<String>,
}

// =============================================================================
// BOARD CONTROL STATE
// =============================================================================

/// State for the board control view
#[derive(Debug, Clone, Default)]
pub struct BoardControlState {
    /// Control sphere data (fetched from API)
    pub control_sphere: Option<ControlSphereData>,
    /// Source CBU ID (for breadcrumb navigation)
    pub source_cbu_id: Option<Uuid>,
    /// Source CBU name
    pub source_cbu_name: Option<String>,
    /// Whether evidence panel is expanded
    pub evidence_panel_expanded: bool,
    /// Currently selected node (for detail panel)
    pub selected_node_id: Option<Uuid>,
    /// Enhance level (0-3)
    pub enhance_level: u8,
}

impl BoardControlState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set control sphere data
    pub fn set_control_sphere(&mut self, data: ControlSphereData) {
        self.control_sphere = Some(data);
    }

    /// Set source CBU for breadcrumb
    pub fn set_source_cbu(&mut self, cbu_id: Uuid, cbu_name: String) {
        self.source_cbu_id = Some(cbu_id);
        self.source_cbu_name = Some(cbu_name);
    }
}

// =============================================================================
// OWNERSHIP TREE LAYOUT
// =============================================================================

/// Layout constants for board control view
const NODE_WIDTH: f32 = 180.0;
const NODE_HEIGHT: f32 = 80.0;
const H_SPACING: f32 = 60.0;
const V_SPACING: f32 = 140.0;
#[allow(dead_code)]
const CORNER_RADIUS: f32 = 8.0;

/// Compute hierarchical tree layout for control sphere
/// Tree flows UPWARD: anchor at bottom, UBOs at top
pub fn compute_ownership_tree_layout(data: &ControlSphereData) -> LayoutGraph {
    let mut graph = LayoutGraph::new(Uuid::nil());

    // Group nodes by depth
    let mut nodes_by_depth: HashMap<u8, Vec<&ControlSphereNode>> = HashMap::new();
    for node in &data.nodes {
        nodes_by_depth.entry(node.depth).or_default().push(node);
    }

    // Find max depth
    let max_depth = nodes_by_depth.keys().copied().max().unwrap_or(0);

    // Position nodes: anchor at bottom (y = max_depth * V_SPACING), UBOs at top (y = 0)
    let mut node_positions: HashMap<Uuid, Pos2> = HashMap::new();

    for depth in 0..=max_depth {
        if let Some(nodes) = nodes_by_depth.get(&depth) {
            let count = nodes.len();
            let total_width = count as f32 * NODE_WIDTH + (count - 1) as f32 * H_SPACING;
            let start_x = -total_width / 2.0 + NODE_WIDTH / 2.0;

            // Y position: invert so depth 0 (anchor) is at bottom
            let y = (max_depth - depth) as f32 * V_SPACING;

            for (i, node) in nodes.iter().enumerate() {
                let x = start_x + i as f32 * (NODE_WIDTH + H_SPACING);
                let position = Pos2::new(x, y);
                node_positions.insert(node.entity_id, position);

                // Create LayoutNode
                let entity_type = if node.is_natural_person {
                    EntityType::ProperPerson
                } else {
                    EntityType::LimitedCompany
                };

                let style = node_style_for_control_node(node);
                let node_size = Vec2::new(NODE_WIDTH, NODE_HEIGHT);

                let layout_node = LayoutNode {
                    id: node.entity_id.to_string(),
                    entity_type,
                    primary_role: super::types::PrimaryRole::Unknown,
                    all_roles: node.psc_categories.clone(),
                    label: node.name.clone(),
                    sublabel: node.jurisdiction.clone(),
                    jurisdiction: node.jurisdiction.clone(),
                    status: None,
                    base_position: position,
                    offset: Vec2::ZERO,
                    position,
                    base_size: node_size,
                    size_override: None,
                    size: node_size,
                    in_focus: node.on_winning_path,
                    is_cbu_root: depth == 0, // Anchor is the "root" of this view
                    style,
                    // Visual hints
                    importance: if node.on_winning_path { 1.0 } else { 0.6 },
                    hierarchy_depth: depth as i32,
                    kyc_completion: None,
                    verification_summary: None,
                    needs_attention: false,
                    entity_category: if node.is_natural_person {
                        Some("PERSON".to_string())
                    } else {
                        Some("SHELL".to_string())
                    },
                    person_state: None,
                    // Container fields (not used in board control view)
                    is_container: false,
                    contains_type: None,
                    child_count: None,
                    browse_nickname: None,
                    parent_key: None,
                    container_parent_id: None,
                    // Control portal fields
                    control_confidence: None,
                    control_explanation: None,
                    control_data_gaps: None,
                    control_rule: None,
                    // Cluster/hierarchy fields (not applicable for board control view)
                    cluster_id: None,
                    parent_id: None,
                };

                graph.nodes.insert(node.entity_id.to_string(), layout_node);
            }
        }
    }

    // Create edges
    for edge in &data.edges {
        let label = format_edge_label(edge);

        let layout_edge = LayoutEdge {
            id: format!("{}-{}", edge.from_entity_id, edge.to_entity_id),
            source_id: edge.from_entity_id.to_string(),
            target_id: edge.to_entity_id.to_string(),
            edge_type: EdgeType::Owns, // Default to ownership edge
            label: Some(label),
            control_points: Vec::new(),
            in_focus: edge.on_winning_path,
            style: edge_style_for_control_edge(edge),
            weight: edge.ownership_pct.map(|p| p as f32),
            verification_status: None,
        };

        graph.edges.push(layout_edge);
    }

    graph.recompute_bounds();
    graph
}

/// Format edge label with ownership/voting percentage and interest type
fn format_edge_label(edge: &ControlSphereEdge) -> String {
    let mut parts = Vec::new();

    if let Some(pct) = edge.ownership_pct {
        parts.push(format!("{:.1}%", pct));
    }

    if let Some(voting) = edge.voting_pct {
        if edge.ownership_pct != Some(voting) {
            parts.push(format!("{}% votes", voting as i32));
        }
    }

    // Add interest type badge
    let interest_badge = match edge.interest_type.to_uppercase().as_str() {
        "SHAREHOLDING" => "shares",
        "VOTING_RIGHTS" => "votes",
        "APPOINTS_BOARD" => "board",
        "SIGNIFICANT_INFLUENCE" => "influence",
        "INDIRECT" => "indirect",
        _ => &edge.interest_type,
    };

    if !parts.is_empty() {
        format!("{} ({})", parts.join(" / "), interest_badge)
    } else {
        interest_badge.to_string()
    }
}

/// Get node style for control sphere node
fn node_style_for_control_node(node: &ControlSphereNode) -> NodeStyle {
    if node.is_ubo {
        // UBO nodes: green
        NodeStyle {
            fill_color: Color32::from_rgb(21, 128, 61),
            border_color: Color32::from_rgb(34, 197, 94),
            text_color: Color32::WHITE,
            border_width: if node.on_winning_path { 3.0 } else { 2.0 },
        }
    } else if node.is_natural_person {
        // Natural person (not UBO): light blue
        NodeStyle {
            fill_color: Color32::from_rgb(30, 64, 175),
            border_color: Color32::from_rgb(96, 165, 250),
            text_color: Color32::WHITE,
            border_width: if node.on_winning_path { 3.0 } else { 2.0 },
        }
    } else if node.depth == 0 {
        // Anchor entity: purple (board controller)
        NodeStyle {
            fill_color: Color32::from_rgb(88, 28, 135),
            border_color: Color32::from_rgb(168, 85, 247),
            text_color: Color32::WHITE,
            border_width: 3.0,
        }
    } else {
        // Legal entity: gray-blue
        NodeStyle {
            fill_color: Color32::from_rgb(55, 65, 81),
            border_color: if node.on_winning_path {
                Color32::from_rgb(251, 191, 36) // Amber for winning path
            } else {
                Color32::from_rgb(107, 114, 128)
            },
            text_color: Color32::WHITE,
            border_width: if node.on_winning_path { 3.0 } else { 2.0 },
        }
    }
}

/// Get edge style for control sphere edge
fn edge_style_for_control_edge(edge: &ControlSphereEdge) -> super::types::EdgeStyle {
    let color = if edge.on_winning_path {
        Color32::from_rgb(251, 191, 36) // Amber for winning path
    } else {
        Color32::from_rgb(107, 114, 128) // Gray for other edges
    };

    let width = if edge.on_winning_path { 3.0 } else { 1.5 };

    super::types::EdgeStyle {
        color,
        width,
        dashed: false,
    }
}

// =============================================================================
// BOARD CONTROL RENDERING
// =============================================================================

/// Render the board control HUD (top bar with breadcrumb and info)
pub fn render_board_control_hud(
    painter: &egui::Painter,
    screen_rect: Rect,
    state: &BoardControlState,
) {
    let hud_height = 60.0;
    let hud_rect = Rect::from_min_size(
        screen_rect.left_top(),
        Vec2::new(screen_rect.width(), hud_height),
    );

    // Background
    painter.rect_filled(
        hud_rect,
        0.0,
        Color32::from_rgba_unmultiplied(20, 25, 35, 230),
    );

    // Border
    painter.line_segment(
        [hud_rect.left_bottom(), hud_rect.right_bottom()],
        Stroke::new(1.0, Color32::from_rgb(70, 80, 100)),
    );

    let padding = 16.0;
    let x = hud_rect.left() + padding;
    let center_y = hud_rect.center().y;

    // Breadcrumb: "< Back to CBU: [name]"
    if let Some(ref cbu_name) = state.source_cbu_name {
        let back_text = format!("< Back to CBU: {}", cbu_name);
        painter.text(
            Pos2::new(x, center_y),
            egui::Align2::LEFT_CENTER,
            &back_text,
            FontId::proportional(14.0),
            Color32::from_rgb(147, 197, 253), // Blue for clickable
        );
        // x += 300.0; // Reserved for future elements
    }

    // Title
    painter.text(
        Pos2::new(hud_rect.center().x, center_y - 10.0),
        egui::Align2::CENTER_CENTER,
        "BOARD CONTROL",
        FontId::proportional(16.0),
        Color32::WHITE,
    );

    // Anchor entity name
    if let Some(ref sphere) = state.control_sphere {
        painter.text(
            Pos2::new(hud_rect.center().x, center_y + 10.0),
            egui::Align2::CENTER_CENTER,
            &sphere.anchor_entity_name,
            FontId::proportional(12.0),
            Color32::from_rgb(180, 190, 210),
        );
    }

    // Enhance level indicator
    let enhance_text = format!("L{}", state.enhance_level);
    painter.text(
        Pos2::new(hud_rect.right() - padding, center_y),
        egui::Align2::RIGHT_CENTER,
        &enhance_text,
        FontId::proportional(14.0),
        Color32::from_rgb(156, 163, 175),
    );
}

/// Render PSC category badges on a node
pub fn render_psc_badges(
    painter: &egui::Painter,
    node_rect: Rect,
    psc_categories: &[String],
    zoom: f32,
) {
    if psc_categories.is_empty() {
        return;
    }

    let badge_height = 16.0 * zoom;
    let badge_y = node_rect.bottom() + 4.0 * zoom;
    let mut badge_x = node_rect.center().x;

    // Center the badges
    let total_width: f32 = psc_categories
        .iter()
        .map(|c| psc_badge_width(c, zoom) + 4.0 * zoom)
        .sum();
    badge_x -= total_width / 2.0;

    for category in psc_categories {
        let (badge_text, badge_color) = psc_badge_style(category);
        let badge_width = psc_badge_width(category, zoom);

        let badge_rect = Rect::from_min_size(
            Pos2::new(badge_x, badge_y),
            Vec2::new(badge_width, badge_height),
        );

        // Badge background
        painter.rect_filled(badge_rect, 3.0 * zoom, badge_color);

        // Badge text
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            badge_text,
            FontId::proportional(9.0 * zoom),
            Color32::WHITE,
        );

        badge_x += badge_width + 4.0 * zoom;
    }
}

/// Get badge display text and color for PSC category
fn psc_badge_style(category: &str) -> (&'static str, Color32) {
    match category.to_uppercase().as_str() {
        ">25% SHARES" | "SHAREHOLDING_25_50" => (">25%", Color32::from_rgb(59, 130, 246)),
        ">50% SHARES" | "SHAREHOLDING_50_75" => (">50%", Color32::from_rgb(37, 99, 235)),
        ">75% SHARES" | "SHAREHOLDING_75_100" => (">75%", Color32::from_rgb(29, 78, 216)),
        ">25% VOTES" | "VOTING_25_50" => (">25% votes", Color32::from_rgb(168, 85, 247)),
        ">50% VOTES" | "VOTING_50_75" => (">50% votes", Color32::from_rgb(147, 51, 234)),
        "APPOINTS BOARD" | "APPOINTS_MAJORITY" => ("Board", Color32::from_rgb(220, 38, 38)),
        "SIGNIFICANT INFLUENCE" => ("Influence", Color32::from_rgb(251, 146, 60)),
        _ => ("PSC", Color32::from_rgb(107, 114, 128)),
    }
}

/// Calculate badge width based on text
fn psc_badge_width(category: &str, zoom: f32) -> f32 {
    let (text, _) = psc_badge_style(category);
    (text.len() as f32 * 6.0 + 12.0) * zoom
}

/// Render source register indicator
pub fn render_source_indicator(painter: &egui::Painter, position: Pos2, source: &str, zoom: f32) {
    let (text, color) = match source.to_uppercase().as_str() {
        "GLEIF" => ("GLEIF", Color32::from_rgb(34, 197, 94)),
        "UK_PSC" | "PSC" => ("UK PSC", Color32::from_rgb(59, 130, 246)),
        "LUX_RBE" | "RBE" => ("LUX", Color32::from_rgb(168, 85, 247)),
        "MANUAL" => ("Manual", Color32::from_rgb(251, 191, 36)),
        _ => ("?", Color32::from_rgb(107, 114, 128)),
    };

    let font_size = 8.0 * zoom;
    painter.text(
        position,
        egui::Align2::LEFT_TOP,
        text,
        FontId::proportional(font_size),
        color,
    );
}

// =============================================================================
// EVIDENCE PANEL
// =============================================================================

/// Evidence item for display in panel
#[derive(Debug, Clone)]
pub struct EvidenceItem {
    pub source: String,
    pub document_type: String,
    pub date: Option<String>,
    pub description: String,
    pub url: Option<String>,
}

/// Render the evidence panel (right side panel)
pub fn render_evidence_panel(
    painter: &egui::Painter,
    screen_rect: Rect,
    evidence: &[EvidenceItem],
    expanded: bool,
) {
    if !expanded {
        return;
    }

    let panel_width = 300.0;
    let panel_rect = Rect::from_min_max(
        Pos2::new(screen_rect.right() - panel_width, screen_rect.top() + 60.0),
        screen_rect.right_bottom(),
    );

    // Background
    painter.rect_filled(
        panel_rect,
        0.0,
        Color32::from_rgba_unmultiplied(25, 30, 40, 240),
    );

    // Border
    painter.line_segment(
        [panel_rect.left_top(), panel_rect.left_bottom()],
        Stroke::new(1.0, Color32::from_rgb(70, 80, 100)),
    );

    // Title
    let title_y = panel_rect.top() + 20.0;
    painter.text(
        Pos2::new(panel_rect.center().x, title_y),
        egui::Align2::CENTER_CENTER,
        "Evidence Sources",
        FontId::proportional(14.0),
        Color32::WHITE,
    );

    // Evidence items
    let mut y = title_y + 30.0;
    let item_height = 60.0;
    let padding = 12.0;

    for item in evidence.iter().take(5) {
        // Item background
        let item_rect = Rect::from_min_size(
            Pos2::new(panel_rect.left() + padding, y),
            Vec2::new(panel_width - padding * 2.0, item_height - 8.0),
        );
        painter.rect_filled(
            item_rect,
            4.0,
            Color32::from_rgba_unmultiplied(40, 45, 55, 200),
        );

        // Source badge
        let (source_text, source_color) = match item.source.to_uppercase().as_str() {
            "GLEIF" => ("GLEIF", Color32::from_rgb(34, 197, 94)),
            "UK_PSC" => ("UK PSC", Color32::from_rgb(59, 130, 246)),
            "LUX_RBE" => ("LUX RBE", Color32::from_rgb(168, 85, 247)),
            _ => (&item.source as &str, Color32::from_rgb(107, 114, 128)),
        };

        painter.text(
            Pos2::new(item_rect.left() + 8.0, item_rect.top() + 12.0),
            egui::Align2::LEFT_CENTER,
            source_text,
            FontId::proportional(10.0),
            source_color,
        );

        // Document type
        painter.text(
            Pos2::new(item_rect.left() + 8.0, item_rect.top() + 28.0),
            egui::Align2::LEFT_CENTER,
            &item.document_type,
            FontId::proportional(11.0),
            Color32::WHITE,
        );

        // Description (truncated)
        let desc = if item.description.len() > 40 {
            format!("{}...", &item.description[..37])
        } else {
            item.description.clone()
        };
        painter.text(
            Pos2::new(item_rect.left() + 8.0, item_rect.top() + 44.0),
            egui::Align2::LEFT_CENTER,
            &desc,
            FontId::proportional(9.0),
            Color32::from_rgb(156, 163, 175),
        );

        y += item_height;
    }
}

// =============================================================================
// ACTIONS
// =============================================================================

/// Actions returned from board control view
#[derive(Debug, Clone)]
pub enum BoardControlAction {
    /// No action
    None,
    /// Navigate back to source CBU
    BackToCbu { cbu_id: Uuid },
    /// Select a node for detail view
    SelectNode { entity_id: Uuid },
    /// Toggle evidence panel
    ToggleEvidencePanel,
    /// Change enhance level
    SetEnhanceLevel { level: u8 },
}
