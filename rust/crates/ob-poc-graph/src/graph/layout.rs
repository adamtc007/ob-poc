//! Layout algorithm - template-based positioning
//!
//! Entities are assigned to "slots" based on their primary role.
//! Templates define slot positions for different CBU categories.

#![allow(dead_code)]

use super::types::*;
use egui::{Pos2, Vec2};
use std::collections::HashMap;

// =============================================================================
// LAYOUT CONSTANTS
// =============================================================================

/// Default node size
pub const NODE_WIDTH: f32 = 160.0;
pub const NODE_HEIGHT: f32 = 70.0;

/// Minimum and maximum node size scale factors based on importance
const MIN_SIZE_SCALE: f32 = 0.7;
const MAX_SIZE_SCALE: f32 = 1.3;

/// Spacing between nodes
pub const H_SPACING: f32 = 40.0;
pub const V_SPACING: f32 = 120.0;

/// Compute node size based on importance (0.0 - 1.0)
/// Returns scaled (width, height) tuple
/// CBU (importance=1.0) gets MAX_SIZE_SCALE, leaves (importance~0.3) get MIN_SIZE_SCALE
fn size_for_importance(importance: f32) -> Vec2 {
    let scale = MIN_SIZE_SCALE + (MAX_SIZE_SCALE - MIN_SIZE_SCALE) * importance.clamp(0.0, 1.0);
    Vec2::new(NODE_WIDTH * scale, NODE_HEIGHT * scale)
}

/// Vertical positions for role tiers (Y coordinates)
const TIER_CBU: f32 = 0.0;
const TIER_STRUCTURE: f32 = 150.0; // ManCo, Principal, Fund entity
const TIER_OFFICERS: f32 = 300.0; // Directors, Officers
const TIER_UBO: f32 = 450.0; // UBOs, Shareholders
const TIER_INVESTORS: f32 = 600.0; // Investors (collapsed)

// =============================================================================
// SLOT DEFINITIONS
// =============================================================================

/// A slot is a named position in a template
#[derive(Debug, Clone)]
pub struct Slot {
    pub name: &'static str,
    pub position: Pos2,
    /// Roles that can fill this slot (in priority order)
    pub accepts_roles: &'static [&'static str],
    /// Maximum entities in this slot (0 = unlimited, arranged horizontally)
    pub max_count: usize,
}

/// Template defines slots for a CBU category
#[derive(Debug, Clone)]
pub struct LayoutTemplate {
    pub category: CbuCategory,
    pub slots: Vec<Slot>,
}

// =============================================================================
// TEMPLATES
// =============================================================================

fn fund_mandate_template() -> LayoutTemplate {
    LayoutTemplate {
        category: CbuCategory::FundMandate,
        slots: vec![
            Slot {
                name: "cbu",
                position: Pos2::new(0.0, TIER_CBU),
                accepts_roles: &["CBU"],
                max_count: 1,
            },
            Slot {
                name: "manco",
                position: Pos2::new(-200.0, TIER_STRUCTURE),
                accepts_roles: &["MANAGEMENT_COMPANY", "MANCO"],
                max_count: 1,
            },
            Slot {
                name: "principal",
                position: Pos2::new(0.0, TIER_STRUCTURE),
                accepts_roles: &["PRINCIPAL"],
                max_count: 1,
            },
            Slot {
                name: "fund_admin",
                position: Pos2::new(200.0, TIER_STRUCTURE),
                accepts_roles: &["FUND_ADMINISTRATOR"],
                max_count: 1,
            },
            Slot {
                name: "directors",
                position: Pos2::new(-150.0, TIER_OFFICERS),
                accepts_roles: &["DIRECTOR"],
                max_count: 0, // unlimited, horizontal
            },
            Slot {
                name: "officers",
                position: Pos2::new(150.0, TIER_OFFICERS),
                accepts_roles: &["OFFICER", "AUTHORIZED_SIGNATORY", "CONTACT_PERSON"],
                max_count: 0,
            },
            Slot {
                name: "ubos",
                position: Pos2::new(0.0, TIER_UBO),
                accepts_roles: &["ULTIMATE_BENEFICIAL_OWNER", "UBO", "SHAREHOLDER"],
                max_count: 0,
            },
            Slot {
                name: "investors",
                position: Pos2::new(0.0, TIER_INVESTORS),
                accepts_roles: &["INVESTOR"],
                max_count: 0,
            },
        ],
    }
}

fn corporate_group_template() -> LayoutTemplate {
    LayoutTemplate {
        category: CbuCategory::CorporateGroup,
        slots: vec![
            Slot {
                name: "cbu",
                position: Pos2::new(0.0, TIER_CBU),
                accepts_roles: &["CBU"],
                max_count: 1,
            },
            Slot {
                name: "principal",
                position: Pos2::new(0.0, TIER_STRUCTURE),
                accepts_roles: &["PRINCIPAL"],
                max_count: 1,
            },
            Slot {
                name: "subsidiaries",
                position: Pos2::new(-200.0, TIER_STRUCTURE),
                accepts_roles: &["SUBSIDIARY"],
                max_count: 0,
            },
            Slot {
                name: "directors",
                position: Pos2::new(-100.0, TIER_OFFICERS),
                accepts_roles: &["DIRECTOR"],
                max_count: 0,
            },
            Slot {
                name: "officers",
                position: Pos2::new(100.0, TIER_OFFICERS),
                accepts_roles: &["OFFICER", "AUTHORIZED_SIGNATORY", "CONTACT_PERSON"],
                max_count: 0,
            },
            Slot {
                name: "ubos",
                position: Pos2::new(0.0, TIER_UBO),
                accepts_roles: &["ULTIMATE_BENEFICIAL_OWNER", "UBO", "SHAREHOLDER"],
                max_count: 0,
            },
        ],
    }
}

fn family_trust_template() -> LayoutTemplate {
    LayoutTemplate {
        category: CbuCategory::FamilyTrust,
        slots: vec![
            Slot {
                name: "cbu",
                position: Pos2::new(0.0, TIER_CBU),
                accepts_roles: &["CBU"],
                max_count: 1,
            },
            Slot {
                name: "trustee",
                position: Pos2::new(-150.0, TIER_STRUCTURE),
                accepts_roles: &["TRUSTEE"],
                max_count: 1,
            },
            Slot {
                name: "protector",
                position: Pos2::new(150.0, TIER_STRUCTURE),
                accepts_roles: &["PROTECTOR"],
                max_count: 1,
            },
            Slot {
                name: "settlor",
                position: Pos2::new(-150.0, TIER_OFFICERS),
                accepts_roles: &["SETTLOR"],
                max_count: 1,
            },
            Slot {
                name: "beneficiaries",
                position: Pos2::new(0.0, TIER_UBO),
                accepts_roles: &["BENEFICIARY"],
                max_count: 0,
            },
            Slot {
                name: "ubos",
                position: Pos2::new(0.0, TIER_UBO + 120.0),
                accepts_roles: &["ULTIMATE_BENEFICIAL_OWNER", "UBO"],
                max_count: 0,
            },
        ],
    }
}

/// Get template for a CBU category
pub fn get_template(category: CbuCategory) -> LayoutTemplate {
    match category {
        CbuCategory::FundMandate => fund_mandate_template(),
        CbuCategory::FamilyTrust => family_trust_template(),
        _ => corporate_group_template(), // default template
    }
}

// =============================================================================
// LAYOUT ENGINE
// =============================================================================

/// Layout engine computes node positions
pub struct LayoutEngine {
    template: LayoutTemplate,
    view_mode: super::ViewMode,
}

impl LayoutEngine {
    pub fn new(category: CbuCategory) -> Self {
        Self {
            template: get_template(category),
            view_mode: super::ViewMode::KycUbo,
        }
    }

    pub fn with_view_mode(category: CbuCategory, view_mode: super::ViewMode) -> Self {
        Self {
            template: get_template(category),
            view_mode,
        }
    }

    /// Compute layout from graph data
    ///
    /// If server provides x/y positions, use them directly.
    /// Otherwise fall back to template-based layout.
    pub fn compute_layout(&self, data: &CbuGraphData) -> LayoutGraph {
        let mut graph = LayoutGraph::new(data.cbu_id);
        graph.cbu_category = data
            .cbu_category
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();
        graph.jurisdiction = data.jurisdiction.clone();

        // Check if server provided positions (first node has x/y)
        let has_server_positions = data.nodes.first().map(|n| n.x.is_some()).unwrap_or(false);

        if has_server_positions {
            // Use server-provided positions directly
            return self.use_server_positions(data, graph);
        }

        // Fall back to template-based layout
        self.compute_template_layout(data, graph)
    }

    /// Use server-provided x/y positions directly
    fn use_server_positions(&self, data: &CbuGraphData, mut graph: LayoutGraph) -> LayoutGraph {
        for node in &data.nodes {
            let x = node.x.unwrap_or(0.0) as f32;
            let y = node.y.unwrap_or(0.0) as f32;
            let position = Pos2::new(x, y);

            let hints = NodeVisualHints::from_node_data(node);
            // Compute size based on importance
            let node_size = size_for_importance(hints.importance);

            let layout_node = LayoutNode {
                id: node.id.clone(),
                entity_type: node.node_type.parse().unwrap_or_default(),
                primary_role: node
                    .primary_role
                    .as_ref()
                    .and_then(|r| r.parse().ok())
                    .unwrap_or(PrimaryRole::Unknown),
                all_roles: node.roles.clone(),
                label: node.label.clone(),
                sublabel: node.sublabel.clone(),
                jurisdiction: node.jurisdiction.clone(),
                base_position: position,
                offset: Vec2::ZERO,
                position,
                base_size: node_size,
                size_override: None,
                size: node_size,
                in_focus: true,
                is_cbu_root: node.node_type == "cbu",
                style: node_style_for_role(
                    node.primary_role
                        .as_ref()
                        .and_then(|r| r.parse().ok())
                        .unwrap_or(PrimaryRole::Unknown),
                    node.node_type == "cbu",
                ),
                // Visual hints
                importance: hints.importance,
                hierarchy_depth: hints.hierarchy_depth,
                kyc_completion: hints.kyc_completion,
                verification_summary: hints.verification_summary,
                needs_attention: hints.needs_attention,
                entity_category: hints.entity_category,
            };

            graph.nodes.insert(node.id.clone(), layout_node);
        }

        // Create edges
        for edge in &data.edges {
            let edge_hints = EdgeVisualHints::from_edge_data(edge);
            let layout_edge = LayoutEdge {
                id: edge.id.clone(),
                source_id: edge.source.clone(),
                target_id: edge.target.clone(),
                edge_type: edge.edge_type.parse().unwrap_or(EdgeType::Other),
                label: edge.label.clone(),
                control_points: Vec::new(),
                in_focus: true,
                style: EdgeStyle::default(),
                // Visual hints
                weight: edge_hints.weight,
                verification_status: edge_hints.verification_status,
            };
            graph.edges.push(layout_edge);
        }

        graph.recompute_bounds();
        graph
    }

    /// Template-based layout (fallback when server doesn't provide positions)
    fn compute_template_layout(&self, data: &CbuGraphData, mut graph: LayoutGraph) -> LayoutGraph {
        // Group nodes by slot assignment
        let mut slot_assignments: HashMap<&str, Vec<&GraphNodeData>> = HashMap::new();
        let mut unassigned: Vec<&GraphNodeData> = Vec::new();

        // Collect service layer nodes separately
        let mut products: Vec<&GraphNodeData> = Vec::new();
        let mut services: Vec<&GraphNodeData> = Vec::new();
        let mut resources: Vec<&GraphNodeData> = Vec::new();

        for node in &data.nodes {
            if node.node_type == "cbu" {
                slot_assignments.entry("cbu").or_default().push(node);
            } else if node.node_type == "entity" {
                // Assign to slot based on primary role
                let role = node.primary_role.as_deref().unwrap_or("UNKNOWN");
                let role_upper = role.to_uppercase().replace('-', "_");

                let mut assigned = false;
                for slot in &self.template.slots {
                    if slot.accepts_roles.iter().any(|r| *r == role_upper) {
                        slot_assignments.entry(slot.name).or_default().push(node);
                        assigned = true;
                        break;
                    }
                }

                if !assigned {
                    unassigned.push(node);
                }
            } else if node.node_type == "product" {
                products.push(node);
            } else if node.node_type == "service" {
                services.push(node);
            } else if node.node_type == "resource" {
                resources.push(node);
            }
            // Skip other node types (kyc, ubo verification nodes, etc.)
        }

        // Position nodes in each slot
        for slot in &self.template.slots {
            if let Some(nodes) = slot_assignments.get(slot.name) {
                self.position_slot_nodes(&mut graph, slot, nodes);
            }
        }

        // Position unassigned nodes at the bottom
        if !unassigned.is_empty() {
            self.position_unassigned(&mut graph, &unassigned);
        }

        // Position service layer nodes (below CBU)
        // Products at -150, Services at -300, Resources at -450 (negative Y = below)
        if !products.is_empty() || !services.is_empty() || !resources.is_empty() {
            self.position_service_nodes(&mut graph, &products, &services, &resources, &data.edges);
        }

        // Create edges
        for edge in &data.edges {
            let edge_hints = EdgeVisualHints::from_edge_data(edge);
            let layout_edge = LayoutEdge {
                id: edge.id.clone(),
                source_id: edge.source.clone(),
                target_id: edge.target.clone(),
                edge_type: edge.edge_type.parse().unwrap_or(EdgeType::Other),
                label: edge.label.clone(),
                control_points: Vec::new(),
                in_focus: true,
                style: edge_style_for_type(edge.edge_type.parse().unwrap_or(EdgeType::Other)),
                // Visual hints
                weight: edge_hints.weight,
                verification_status: edge_hints.verification_status,
            };
            graph.edges.push(layout_edge);
        }

        // Apply layout refinements
        // 1. Minimize edge crossings by reordering nodes within tiers
        self.minimize_crossings(&mut graph);

        // 2. Apply force-directed refinement for spacing (10 iterations)
        self.force_refine(&mut graph, 10);

        // 3. Apply view-mode specific layout adjustments
        if self.view_mode == super::ViewMode::KycUbo {
            // UBO view: invert hierarchy so UBOs (persons) are at top, CBU at bottom
            self.apply_ubo_layout(&mut graph);
        }

        // Compute bounds
        graph.recompute_bounds();

        graph
    }

    fn position_slot_nodes(&self, graph: &mut LayoutGraph, slot: &Slot, nodes: &[&GraphNodeData]) {
        let count = nodes.len();

        if count == 0 {
            return;
        }

        // Calculate horizontal spread
        let total_width = count as f32 * NODE_WIDTH + (count - 1) as f32 * H_SPACING;
        let start_x = slot.position.x - total_width / 2.0 + NODE_WIDTH / 2.0;

        for (i, node) in nodes.iter().enumerate() {
            let x = start_x + i as f32 * (NODE_WIDTH + H_SPACING);
            let position = Pos2::new(x, slot.position.y);

            let hints = NodeVisualHints::from_node_data(node);
            // Compute size based on importance
            let node_size = size_for_importance(hints.importance);

            let layout_node = LayoutNode {
                base_position: position,
                offset: Vec2::ZERO,
                id: node.id.clone(),
                entity_type: node.node_type.parse().unwrap_or_default(),
                primary_role: node
                    .primary_role
                    .as_ref()
                    .and_then(|r| r.parse().ok())
                    .unwrap_or(PrimaryRole::Unknown),
                all_roles: node.roles.clone(),
                label: node.label.clone(),
                sublabel: node.sublabel.clone(),
                jurisdiction: node.jurisdiction.clone(),
                position,
                base_size: node_size,
                size_override: None,
                size: node_size,
                in_focus: true,
                is_cbu_root: node.node_type == "cbu",
                style: node_style_for_role(
                    node.primary_role
                        .as_ref()
                        .and_then(|r| r.parse().ok())
                        .unwrap_or(PrimaryRole::Unknown),
                    node.node_type == "cbu",
                ),
                // Visual hints
                importance: hints.importance,
                hierarchy_depth: hints.hierarchy_depth,
                kyc_completion: hints.kyc_completion,
                verification_summary: hints.verification_summary,
                needs_attention: hints.needs_attention,
                entity_category: hints.entity_category,
            };

            graph.nodes.insert(node.id.clone(), layout_node);
        }
    }

    fn position_unassigned(&self, graph: &mut LayoutGraph, nodes: &[&GraphNodeData]) {
        let y = TIER_INVESTORS + V_SPACING; // Below investors

        let count = nodes.len();
        let total_width = count as f32 * NODE_WIDTH + (count - 1) as f32 * H_SPACING;
        let start_x = -total_width / 2.0 + NODE_WIDTH / 2.0;

        for (i, node) in nodes.iter().enumerate() {
            let x = start_x + i as f32 * (NODE_WIDTH + H_SPACING);
            let position = Pos2::new(x, y);

            let hints = NodeVisualHints::from_node_data(node);
            // Compute size based on importance
            let node_size = size_for_importance(hints.importance);

            let layout_node = LayoutNode {
                base_position: position,
                offset: Vec2::ZERO,
                id: node.id.clone(),
                entity_type: node.node_type.parse().unwrap_or_default(),
                primary_role: node
                    .primary_role
                    .as_ref()
                    .and_then(|r| r.parse().ok())
                    .unwrap_or(PrimaryRole::Unknown),
                all_roles: node.roles.clone(),
                label: node.label.clone(),
                sublabel: node.sublabel.clone(),
                jurisdiction: node.jurisdiction.clone(),
                position,
                base_size: node_size,
                size_override: None,
                size: node_size,
                in_focus: true,
                is_cbu_root: false,
                style: NodeStyle::default(),
                // Visual hints
                importance: hints.importance,
                hierarchy_depth: hints.hierarchy_depth,
                kyc_completion: hints.kyc_completion,
                verification_summary: hints.verification_summary,
                needs_attention: hints.needs_attention,
                entity_category: hints.entity_category,
            };

            graph.nodes.insert(node.id.clone(), layout_node);
        }
    }

    /// Position service layer nodes in a hierarchy below CBU
    /// Layout: CBU (0) -> Products (-150) -> Services (-300) -> Resources (-450)
    fn position_service_nodes(
        &self,
        graph: &mut LayoutGraph,
        products: &[&GraphNodeData],
        services: &[&GraphNodeData],
        resources: &[&GraphNodeData],
        edges: &[GraphEdgeData],
    ) {
        use egui::Color32;

        // Tier positions (negative Y = below CBU in screen coords)
        const TIER_PRODUCT: f32 = -150.0;
        const TIER_SERVICE: f32 = -300.0;
        const TIER_RESOURCE: f32 = -450.0;

        // Build parent->children maps from edges
        let mut product_services: HashMap<String, Vec<&GraphNodeData>> = HashMap::new();
        let mut service_resources: HashMap<String, Vec<&GraphNodeData>> = HashMap::new();

        // Map service IDs to their parent product
        for edge in edges {
            if edge.edge_type == "has_service" || edge.edge_type == "product_service" {
                // source = product, target = service
                if let Some(service) = services.iter().find(|s| s.id == edge.target) {
                    product_services
                        .entry(edge.source.clone())
                        .or_default()
                        .push(*service);
                }
            } else if edge.edge_type == "has_resource" || edge.edge_type == "service_resource" {
                // source = service, target = resource
                if let Some(resource) = resources.iter().find(|r| r.id == edge.target) {
                    service_resources
                        .entry(edge.source.clone())
                        .or_default()
                        .push(*resource);
                }
            }
        }

        // Position products horizontally centered
        let product_count = products.len();
        if product_count > 0 {
            let total_width =
                product_count as f32 * NODE_WIDTH + (product_count - 1) as f32 * H_SPACING;
            let start_x = -total_width / 2.0 + NODE_WIDTH / 2.0;

            for (i, product) in products.iter().enumerate() {
                let x = start_x + i as f32 * (NODE_WIDTH + H_SPACING);
                let position = Pos2::new(x, TIER_PRODUCT);

                let hints = NodeVisualHints::default_for_service_layer();
                // Compute size based on importance
                let node_size = size_for_importance(hints.importance);

                let layout_node = LayoutNode {
                    base_position: position,
                    offset: Vec2::ZERO,
                    id: product.id.clone(),
                    entity_type: EntityType::Product,
                    primary_role: PrimaryRole::Unknown,
                    all_roles: vec![],
                    label: product.label.clone(),
                    sublabel: product.sublabel.clone(),
                    jurisdiction: None,
                    position,
                    base_size: node_size,
                    size_override: None,
                    size: node_size,
                    in_focus: true,
                    is_cbu_root: false,
                    style: NodeStyle {
                        fill_color: Color32::from_rgb(88, 28, 135), // purple
                        border_color: Color32::from_rgb(168, 85, 247),
                        text_color: Color32::WHITE,
                        border_width: 2.0,
                    },
                    // Visual hints
                    importance: hints.importance,
                    hierarchy_depth: hints.hierarchy_depth,
                    kyc_completion: hints.kyc_completion,
                    verification_summary: hints.verification_summary,
                    needs_attention: hints.needs_attention,
                    entity_category: hints.entity_category,
                };
                graph.nodes.insert(product.id.clone(), layout_node);

                // Position services under this product
                if let Some(prod_services) = product_services.get(&product.id) {
                    let svc_count = prod_services.len();
                    let svc_total_width =
                        svc_count as f32 * NODE_WIDTH + (svc_count - 1) as f32 * (H_SPACING / 2.0);
                    let svc_start_x = x - svc_total_width / 2.0 + NODE_WIDTH / 2.0;

                    for (j, service) in prod_services.iter().enumerate() {
                        let svc_x = svc_start_x + j as f32 * (NODE_WIDTH + H_SPACING / 2.0);
                        let svc_position = Pos2::new(svc_x, TIER_SERVICE);

                        let svc_hints = NodeVisualHints::default_for_service_layer();
                        // Compute size based on importance
                        let svc_size = size_for_importance(svc_hints.importance);

                        let svc_node = LayoutNode {
                            id: service.id.clone(),
                            entity_type: EntityType::Service,
                            primary_role: PrimaryRole::Unknown,
                            all_roles: vec![],
                            label: service.label.clone(),
                            sublabel: service.sublabel.clone(),
                            jurisdiction: None,
                            base_position: svc_position,
                            offset: Vec2::ZERO,
                            position: svc_position,
                            base_size: svc_size,
                            size_override: None,
                            size: svc_size,
                            in_focus: true,
                            is_cbu_root: false,
                            style: NodeStyle {
                                fill_color: Color32::from_rgb(30, 58, 138), // blue
                                border_color: Color32::from_rgb(96, 165, 250),
                                text_color: Color32::WHITE,
                                border_width: 2.0,
                            },
                            // Visual hints
                            importance: svc_hints.importance,
                            hierarchy_depth: svc_hints.hierarchy_depth,
                            kyc_completion: svc_hints.kyc_completion,
                            verification_summary: svc_hints.verification_summary,
                            needs_attention: svc_hints.needs_attention,
                            entity_category: svc_hints.entity_category,
                        };
                        graph.nodes.insert(service.id.clone(), svc_node);

                        // Position resources under this service
                        if let Some(svc_resources) = service_resources.get(&service.id) {
                            let res_count = svc_resources.len();
                            let res_total_width = res_count as f32 * NODE_WIDTH
                                + (res_count - 1) as f32 * (H_SPACING / 3.0);
                            let res_start_x = svc_x - res_total_width / 2.0 + NODE_WIDTH / 2.0;

                            for (k, resource) in svc_resources.iter().enumerate() {
                                let res_x = res_start_x + k as f32 * (NODE_WIDTH + H_SPACING / 3.0);
                                let res_position = Pos2::new(res_x, TIER_RESOURCE);

                                let res_hints = NodeVisualHints::default_for_service_layer();
                                // Compute size based on importance
                                let res_size = size_for_importance(res_hints.importance);

                                let res_node = LayoutNode {
                                    id: resource.id.clone(),
                                    entity_type: EntityType::Resource,
                                    primary_role: PrimaryRole::Unknown,
                                    all_roles: vec![],
                                    label: resource.label.clone(),
                                    sublabel: resource.sublabel.clone(),
                                    jurisdiction: None,
                                    base_position: res_position,
                                    offset: Vec2::ZERO,
                                    position: res_position,
                                    base_size: res_size,
                                    size_override: None,
                                    size: res_size,
                                    in_focus: true,
                                    is_cbu_root: false,
                                    style: NodeStyle {
                                        fill_color: Color32::from_rgb(20, 83, 45), // green
                                        border_color: Color32::from_rgb(74, 222, 128),
                                        text_color: Color32::WHITE,
                                        border_width: 2.0,
                                    },
                                    // Visual hints
                                    importance: res_hints.importance,
                                    hierarchy_depth: res_hints.hierarchy_depth,
                                    kyc_completion: res_hints.kyc_completion,
                                    verification_summary: res_hints.verification_summary,
                                    needs_attention: res_hints.needs_attention,
                                    entity_category: res_hints.entity_category,
                                };
                                graph.nodes.insert(resource.id.clone(), res_node);
                            }
                        }
                    }
                }
            }
        }

        // Position orphan services (no parent product) - shouldn't happen but handle it
        let positioned_services: std::collections::HashSet<&str> = product_services
            .values()
            .flatten()
            .map(|s| s.id.as_str())
            .collect();
        let orphan_services: Vec<_> = services
            .iter()
            .filter(|s| !positioned_services.contains(s.id.as_str()))
            .collect();

        if !orphan_services.is_empty() {
            let count = orphan_services.len();
            let total_width = count as f32 * NODE_WIDTH + (count - 1) as f32 * H_SPACING;
            let start_x = -total_width / 2.0 + NODE_WIDTH / 2.0;

            for (i, service) in orphan_services.iter().enumerate() {
                let x = start_x + i as f32 * (NODE_WIDTH + H_SPACING);
                let position = Pos2::new(x, TIER_SERVICE);

                let hints = NodeVisualHints::default_for_service_layer();
                // Compute size based on importance
                let node_size = size_for_importance(hints.importance);

                let layout_node = LayoutNode {
                    base_position: position,
                    offset: Vec2::ZERO,
                    id: service.id.clone(),
                    entity_type: EntityType::Service,
                    primary_role: PrimaryRole::Unknown,
                    all_roles: vec![],
                    label: service.label.clone(),
                    sublabel: service.sublabel.clone(),
                    jurisdiction: None,
                    position,
                    base_size: node_size,
                    size_override: None,
                    size: node_size,
                    in_focus: true,
                    is_cbu_root: false,
                    style: NodeStyle {
                        fill_color: Color32::from_rgb(30, 58, 138),
                        border_color: Color32::from_rgb(96, 165, 250),
                        text_color: Color32::WHITE,
                        border_width: 2.0,
                    },
                    // Visual hints
                    importance: hints.importance,
                    hierarchy_depth: hints.hierarchy_depth,
                    kyc_completion: hints.kyc_completion,
                    verification_summary: hints.verification_summary,
                    needs_attention: hints.needs_attention,
                    entity_category: hints.entity_category,
                };
                graph.nodes.insert(service.id.clone(), layout_node);
            }
        }

        // Position orphan resources (no parent service)
        let positioned_resources: std::collections::HashSet<&str> = service_resources
            .values()
            .flatten()
            .map(|r| r.id.as_str())
            .collect();
        let orphan_resources: Vec<_> = resources
            .iter()
            .filter(|r| !positioned_resources.contains(r.id.as_str()))
            .collect();

        if !orphan_resources.is_empty() {
            let count = orphan_resources.len();
            let total_width = count as f32 * NODE_WIDTH + (count - 1) as f32 * H_SPACING;
            let start_x = -total_width / 2.0 + NODE_WIDTH / 2.0;

            for (i, resource) in orphan_resources.iter().enumerate() {
                let x = start_x + i as f32 * (NODE_WIDTH + H_SPACING);
                let position = Pos2::new(x, TIER_RESOURCE);

                let hints = NodeVisualHints::default_for_service_layer();
                // Compute size based on importance
                let node_size = size_for_importance(hints.importance);

                let layout_node = LayoutNode {
                    base_position: position,
                    offset: Vec2::ZERO,
                    id: resource.id.clone(),
                    entity_type: EntityType::Resource,
                    primary_role: PrimaryRole::Unknown,
                    all_roles: vec![],
                    label: resource.label.clone(),
                    sublabel: resource.sublabel.clone(),
                    jurisdiction: None,
                    position,
                    base_size: node_size,
                    size_override: None,
                    size: node_size,
                    in_focus: true,
                    is_cbu_root: false,
                    style: NodeStyle {
                        fill_color: Color32::from_rgb(20, 83, 45),
                        border_color: Color32::from_rgb(74, 222, 128),
                        text_color: Color32::WHITE,
                        border_width: 2.0,
                    },
                    // Visual hints
                    importance: hints.importance,
                    hierarchy_depth: hints.hierarchy_depth,
                    kyc_completion: hints.kyc_completion,
                    verification_summary: hints.verification_summary,
                    needs_attention: hints.needs_attention,
                    entity_category: hints.entity_category,
                };
                graph.nodes.insert(resource.id.clone(), layout_node);
            }
        }
    }

    // =========================================================================
    // FORCE-DIRECTED REFINEMENT
    // =========================================================================

    /// Apply force-directed refinement to reduce node overlaps and improve spacing
    /// This is called after initial layout to fine-tune positions
    #[allow(dead_code)]
    pub fn force_refine(&self, graph: &mut LayoutGraph, iterations: usize) {
        let repulsion_strength = 5000.0;
        let attraction_strength = 0.01;
        let damping = 0.85;
        let min_distance = 80.0;

        for _ in 0..iterations {
            // Collect node IDs and positions
            let node_data: Vec<(String, Pos2)> = graph
                .nodes
                .iter()
                .map(|(id, n)| (id.clone(), n.position))
                .collect();

            // Calculate forces
            let mut forces: HashMap<String, Vec2> = HashMap::new();
            for (id, _) in &node_data {
                forces.insert(id.clone(), Vec2::ZERO);
            }

            // Repulsion between all node pairs
            for i in 0..node_data.len() {
                for j in (i + 1)..node_data.len() {
                    let (id_i, pos_i) = &node_data[i];
                    let (id_j, pos_j) = &node_data[j];

                    let delta = *pos_i - *pos_j;
                    let dist = delta.length().max(min_distance);
                    let force_magnitude = repulsion_strength / (dist * dist);
                    let force = delta.normalized() * force_magnitude;

                    if let Some(f) = forces.get_mut(id_i) {
                        *f += force;
                    }
                    if let Some(f) = forces.get_mut(id_j) {
                        *f -= force;
                    }
                }
            }

            // Attraction along edges (keep connected nodes close)
            for edge in &graph.edges {
                let src_pos = graph.nodes.get(&edge.source_id).map(|n| n.position);
                let tgt_pos = graph.nodes.get(&edge.target_id).map(|n| n.position);

                if let (Some(src), Some(tgt)) = (src_pos, tgt_pos) {
                    let delta = tgt - src;
                    let dist = delta.length();
                    if dist > 0.0 {
                        let force = delta * attraction_strength * dist;

                        if let Some(f) = forces.get_mut(&edge.source_id) {
                            *f += force;
                        }
                        if let Some(f) = forces.get_mut(&edge.target_id) {
                            *f -= force;
                        }
                    }
                }
            }

            // Apply forces with damping (only adjust X, keep Y tiers fixed)
            for (id, node) in graph.nodes.iter_mut() {
                if let Some(force) = forces.get(id) {
                    // Only apply horizontal force to preserve tier structure
                    node.position.x += force.x * damping * 0.1;
                    node.base_position.x = node.position.x;
                }
            }
        }
    }

    /// Minimize edge crossings within tiers using barycenter heuristic
    /// Reorders nodes within each tier based on average position of connected nodes
    #[allow(dead_code)]
    pub fn minimize_crossings(&self, graph: &mut LayoutGraph) {
        // Group nodes by Y position (tier)
        let mut tiers: HashMap<i32, Vec<String>> = HashMap::new();
        for (id, node) in &graph.nodes {
            let tier_key = (node.position.y / V_SPACING).round() as i32;
            tiers.entry(tier_key).or_default().push(id.clone());
        }

        // Sort tier keys
        let mut tier_keys: Vec<i32> = tiers.keys().copied().collect();
        tier_keys.sort();

        // Iterate forward and backward to minimize crossings
        for _pass in 0..5 {
            // Forward pass
            for i in 1..tier_keys.len() {
                let current_tier = tier_keys[i];
                let prev_tier = tier_keys[i - 1];
                self.reorder_tier_by_barycenter(graph, current_tier, prev_tier, &mut tiers);
            }

            // Backward pass
            for i in (0..tier_keys.len().saturating_sub(1)).rev() {
                let current_tier = tier_keys[i];
                let next_tier = tier_keys[i + 1];
                self.reorder_tier_by_barycenter(graph, current_tier, next_tier, &mut tiers);
            }
        }

        // Apply new X positions based on order within tier
        for (tier_key, node_ids) in &tiers {
            let tier_y = *tier_key as f32 * V_SPACING;
            let count = node_ids.len();
            if count == 0 {
                continue;
            }

            let total_width = count as f32 * NODE_WIDTH + (count - 1) as f32 * H_SPACING;
            let start_x = -total_width / 2.0 + NODE_WIDTH / 2.0;

            for (i, id) in node_ids.iter().enumerate() {
                if let Some(node) = graph.nodes.get_mut(id) {
                    let new_x = start_x + i as f32 * (NODE_WIDTH + H_SPACING);
                    node.position.x = new_x;
                    node.position.y = tier_y;
                    node.base_position = node.position;
                }
            }
        }
    }

    /// Reorder nodes in current_tier based on barycenter of connections to reference_tier
    fn reorder_tier_by_barycenter(
        &self,
        graph: &LayoutGraph,
        current_tier: i32,
        reference_tier: i32,
        tiers: &mut HashMap<i32, Vec<String>>,
    ) {
        let Some(current_nodes) = tiers.get(&current_tier) else {
            return;
        };
        let Some(ref_nodes) = tiers.get(&reference_tier) else {
            return;
        };

        // Create position map for reference tier
        let ref_positions: HashMap<&str, usize> = ref_nodes
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();

        // Calculate barycenter for each node in current tier
        let mut barycenters: Vec<(String, f32)> = Vec::new();

        for node_id in current_nodes {
            let mut sum = 0.0;
            let mut count = 0;

            // Find all edges connecting this node to reference tier
            for edge in &graph.edges {
                let neighbor_id = if edge.source_id == *node_id {
                    &edge.target_id
                } else if edge.target_id == *node_id {
                    &edge.source_id
                } else {
                    continue;
                };

                if let Some(&pos) = ref_positions.get(neighbor_id.as_str()) {
                    sum += pos as f32;
                    count += 1;
                }
            }

            let barycenter = if count > 0 {
                sum / count as f32
            } else {
                // Keep original relative position for unconnected nodes
                current_nodes
                    .iter()
                    .position(|id| id == node_id)
                    .unwrap_or(0) as f32
            };

            barycenters.push((node_id.clone(), barycenter));
        }

        // Sort by barycenter
        barycenters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Update tier order
        if let Some(tier) = tiers.get_mut(&current_tier) {
            *tier = barycenters.into_iter().map(|(id, _)| id).collect();
        }
    }

    // =========================================================================
    // UBO-SPECIFIC LAYOUT
    // =========================================================================

    /// Apply UBO-specific layout adjustments
    /// In UBO view, ownership flows upward: CBU at bottom, UBOs (natural persons) at top
    /// This inverts the typical hierarchy for clearer ownership chain visualization
    #[allow(dead_code)]
    pub fn apply_ubo_layout(&self, graph: &mut LayoutGraph) {
        // Find the vertical extent of the graph
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        for node in graph.nodes.values() {
            min_y = min_y.min(node.position.y);
            max_y = max_y.max(node.position.y);
        }

        if min_y >= max_y {
            return; // No inversion needed for single-tier graphs
        }

        let center_y = (min_y + max_y) / 2.0;

        // Invert Y positions around the center
        // This puts UBOs (which were at bottom in normal layout) at the top
        for node in graph.nodes.values_mut() {
            let offset_from_center = node.position.y - center_y;
            node.position.y = center_y - offset_from_center;
            node.base_position.y = node.position.y;
        }

        // Reposition based on entity category:
        // PERSON nodes (natural persons/UBOs) should be at the very top
        // SHELL nodes (companies, trusts) in the middle
        // CBU at the bottom
        let mut persons: Vec<String> = Vec::new();
        let mut shells: Vec<String> = Vec::new();
        let mut cbu_id: Option<String> = None;

        for (id, node) in &graph.nodes {
            if node.is_cbu_root {
                cbu_id = Some(id.clone());
            } else if node.entity_category.as_deref() == Some("PERSON") {
                persons.push(id.clone());
            } else {
                shells.push(id.clone());
            }
        }

        // Assign final tiers: PERSON at top (y=0), then shells, then CBU at bottom
        const UBO_TIER_PERSON: f32 = 0.0;
        const UBO_TIER_SHELL_BASE: f32 = 150.0;
        const UBO_TIER_CBU: f32 = 450.0;

        // Position persons at top, spread horizontally
        if !persons.is_empty() {
            let total_width =
                persons.len() as f32 * NODE_WIDTH + (persons.len() - 1) as f32 * H_SPACING;
            let start_x = -total_width / 2.0 + NODE_WIDTH / 2.0;

            for (i, person_id) in persons.iter().enumerate() {
                if let Some(node) = graph.nodes.get_mut(person_id) {
                    node.position.x = start_x + i as f32 * (NODE_WIDTH + H_SPACING);
                    node.position.y = UBO_TIER_PERSON;
                    node.base_position = node.position;
                }
            }
        }

        // Position shells in middle tiers based on their original relative depth
        // Sort shells by their original Y position to maintain ownership chain order
        let mut shell_positions: Vec<(String, f32)> = shells
            .iter()
            .filter_map(|id| graph.nodes.get(id).map(|n| (id.clone(), n.position.y)))
            .collect();
        shell_positions.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Group shells into tiers
        let shell_tier_count = if shell_positions.is_empty() {
            0
        } else {
            ((UBO_TIER_CBU - UBO_TIER_SHELL_BASE) / V_SPACING).floor() as usize
        };

        if shell_tier_count > 0 && !shell_positions.is_empty() {
            let shells_per_tier =
                (shell_positions.len() as f32 / shell_tier_count as f32).ceil() as usize;

            for (i, (shell_id, _)) in shell_positions.iter().enumerate() {
                let tier = i / shells_per_tier.max(1);
                let pos_in_tier = i % shells_per_tier.max(1);

                if let Some(node) = graph.nodes.get_mut(shell_id) {
                    let tier_y = UBO_TIER_SHELL_BASE + tier as f32 * V_SPACING;
                    let entities_in_this_tier = shell_positions
                        .iter()
                        .skip(tier * shells_per_tier)
                        .take(shells_per_tier)
                        .count();
                    let total_width = entities_in_this_tier as f32 * NODE_WIDTH
                        + (entities_in_this_tier - 1) as f32 * H_SPACING;
                    let start_x = -total_width / 2.0 + NODE_WIDTH / 2.0;

                    node.position.x = start_x + pos_in_tier as f32 * (NODE_WIDTH + H_SPACING);
                    node.position.y = tier_y;
                    node.base_position = node.position;
                }
            }
        }

        // Position CBU at bottom center
        if let Some(ref cbu) = cbu_id {
            if let Some(node) = graph.nodes.get_mut(cbu) {
                node.position.x = 0.0;
                node.position.y = UBO_TIER_CBU;
                node.base_position = node.position;
            }
        }

        graph.recompute_bounds();
    }
}

// =============================================================================
// STYLE HELPERS
// =============================================================================

fn node_style_for_role(role: PrimaryRole, is_cbu: bool) -> NodeStyle {
    use egui::Color32;

    if is_cbu {
        return NodeStyle {
            fill_color: Color32::from_rgb(31, 41, 55),
            border_color: Color32::from_rgb(156, 163, 175),
            text_color: Color32::WHITE,
            border_width: 3.0,
        };
    }

    match role {
        // Ownership/Control - Green family
        PrimaryRole::UltimateBeneficialOwner | PrimaryRole::BeneficialOwner => NodeStyle {
            fill_color: Color32::from_rgb(21, 128, 61),
            border_color: Color32::from_rgb(34, 197, 94),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        PrimaryRole::Shareholder | PrimaryRole::GeneralPartner | PrimaryRole::LimitedPartner => {
            NodeStyle {
                fill_color: Color32::from_rgb(22, 101, 52),
                border_color: Color32::from_rgb(74, 222, 128),
                text_color: Color32::WHITE,
                border_width: 2.0,
            }
        }
        // Governance - Blue family
        PrimaryRole::Director
        | PrimaryRole::Officer
        | PrimaryRole::ConductingOfficer
        | PrimaryRole::ChiefComplianceOfficer => NodeStyle {
            fill_color: Color32::from_rgb(30, 64, 175),
            border_color: Color32::from_rgb(96, 165, 250),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        // Trust roles - Brown family
        PrimaryRole::Trustee
        | PrimaryRole::Protector
        | PrimaryRole::Beneficiary
        | PrimaryRole::Settlor => NodeStyle {
            fill_color: Color32::from_rgb(120, 53, 15),
            border_color: Color32::from_rgb(217, 119, 6),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        // Fund structure - Purple family
        PrimaryRole::Principal
        | PrimaryRole::AssetOwner
        | PrimaryRole::MasterFund
        | PrimaryRole::FeederFund
        | PrimaryRole::SegregatedPortfolio => NodeStyle {
            fill_color: Color32::from_rgb(88, 28, 135),
            border_color: Color32::from_rgb(192, 132, 252),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        // Management - Orange family
        PrimaryRole::ManagementCompany
        | PrimaryRole::InvestmentManager
        | PrimaryRole::InvestmentAdvisor
        | PrimaryRole::Sponsor
        | PrimaryRole::CommercialClient => NodeStyle {
            fill_color: Color32::from_rgb(124, 45, 18),
            border_color: Color32::from_rgb(251, 146, 60),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        // Service providers - Teal family
        PrimaryRole::Administrator
        | PrimaryRole::Custodian
        | PrimaryRole::Depositary
        | PrimaryRole::TransferAgent
        | PrimaryRole::Distributor => NodeStyle {
            fill_color: Color32::from_rgb(17, 94, 89),
            border_color: Color32::from_rgb(45, 212, 191),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        PrimaryRole::PrimeBroker | PrimaryRole::Auditor | PrimaryRole::LegalCounsel => NodeStyle {
            fill_color: Color32::from_rgb(55, 65, 81),
            border_color: Color32::from_rgb(156, 163, 175),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        // Other
        PrimaryRole::AuthorizedSignatory | PrimaryRole::ContactPerson => NodeStyle {
            fill_color: Color32::from_rgb(55, 65, 81),
            border_color: Color32::from_rgb(107, 114, 128),
            text_color: Color32::WHITE,
            border_width: 1.5,
        },
        PrimaryRole::Unknown => NodeStyle::default(),
    }
}

fn edge_style_for_type(edge_type: EdgeType) -> EdgeStyle {
    use egui::Color32;

    match edge_type {
        EdgeType::HasRole => EdgeStyle {
            color: Color32::from_rgb(107, 114, 128),
            width: 1.5,
            dashed: false,
        },
        EdgeType::Owns => EdgeStyle {
            color: Color32::from_rgb(34, 197, 94),
            width: 2.0,
            dashed: false,
        },
        EdgeType::Controls => EdgeStyle {
            color: Color32::from_rgb(251, 191, 36),
            width: 2.0,
            dashed: true,
        },
        EdgeType::Other => EdgeStyle::default(),
    }
}

// =============================================================================
// VISUAL HINT HELPERS
// =============================================================================

/// Visual hints extracted from GraphNodeData for LayoutNode construction
struct NodeVisualHints {
    importance: f32,
    hierarchy_depth: i32,
    kyc_completion: Option<i32>,
    verification_summary: Option<VerificationSummary>,
    needs_attention: bool,
    entity_category: Option<String>,
}

impl NodeVisualHints {
    /// Extract visual hints from GraphNodeData
    fn from_node_data(node: &GraphNodeData) -> Self {
        Self {
            importance: node.importance.unwrap_or(0.5),
            hierarchy_depth: node.hierarchy_depth.unwrap_or(0),
            kyc_completion: node.kyc_completion,
            verification_summary: node.verification_summary.clone(),
            needs_attention: node.needs_attention,
            entity_category: node.entity_category.clone(),
        }
    }

    /// Default visual hints for nodes without source data (e.g., products, services)
    fn default_for_service_layer() -> Self {
        Self {
            importance: 0.5,
            hierarchy_depth: 0,
            kyc_completion: None,
            verification_summary: None,
            needs_attention: false,
            entity_category: None,
        }
    }
}

/// Visual hints extracted from GraphEdgeData for LayoutEdge construction
struct EdgeVisualHints {
    weight: Option<f32>,
    verification_status: Option<String>,
}

impl EdgeVisualHints {
    /// Extract visual hints from GraphEdgeData
    fn from_edge_data(edge: &GraphEdgeData) -> Self {
        Self {
            weight: edge.weight,
            verification_status: edge.verification_status.clone(),
        }
    }
}
