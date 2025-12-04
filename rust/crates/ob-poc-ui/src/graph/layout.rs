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

/// Spacing between nodes
pub const H_SPACING: f32 = 40.0;
pub const V_SPACING: f32 = 120.0;

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
}

impl LayoutEngine {
    pub fn new(category: CbuCategory) -> Self {
        Self {
            template: get_template(category),
        }
    }

    /// Compute layout from graph data
    pub fn compute_layout(&self, data: &CbuGraphData) -> LayoutGraph {
        let mut graph = LayoutGraph::new(data.cbu_id);
        graph.cbu_category = data
            .cbu_category
            .as_ref()
            .map(|s| CbuCategory::from_str(s))
            .unwrap_or_default();
        graph.jurisdiction = data.jurisdiction.clone();

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
            let layout_edge = LayoutEdge {
                id: edge.id.clone(),
                source_id: edge.source.clone(),
                target_id: edge.target.clone(),
                edge_type: EdgeType::from_str(&edge.edge_type),
                label: edge.label.clone(),
                control_points: Vec::new(),
                in_focus: true,
                style: edge_style_for_type(EdgeType::from_str(&edge.edge_type)),
            };
            graph.edges.push(layout_edge);
        }

        // Compute bounds
        graph.recompute_bounds();

        graph
    }

    fn position_slot_nodes(&self, graph: &mut LayoutGraph, slot: &Slot, nodes: &[&GraphNodeData]) {
        let node_size = Vec2::new(NODE_WIDTH, NODE_HEIGHT);
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

            let layout_node = LayoutNode {
                id: node.id.clone(),
                entity_type: EntityType::from_str(&node.node_type),
                primary_role: node
                    .primary_role
                    .as_ref()
                    .map(|r| PrimaryRole::from_str(r))
                    .unwrap_or(PrimaryRole::Unknown),
                all_roles: node.roles.clone(),
                label: node.label.clone(),
                sublabel: node.sublabel.clone(),
                jurisdiction: node.jurisdiction.clone(),
                position,
                size: node_size,
                in_focus: true,
                is_cbu_root: node.node_type == "cbu",
                style: node_style_for_role(
                    node.primary_role
                        .as_ref()
                        .map(|r| PrimaryRole::from_str(r))
                        .unwrap_or(PrimaryRole::Unknown),
                    node.node_type == "cbu",
                ),
            };

            graph.nodes.insert(node.id.clone(), layout_node);
        }
    }

    fn position_unassigned(&self, graph: &mut LayoutGraph, nodes: &[&GraphNodeData]) {
        let node_size = Vec2::new(NODE_WIDTH, NODE_HEIGHT);
        let y = TIER_INVESTORS + V_SPACING; // Below investors

        let count = nodes.len();
        let total_width = count as f32 * NODE_WIDTH + (count - 1) as f32 * H_SPACING;
        let start_x = -total_width / 2.0 + NODE_WIDTH / 2.0;

        for (i, node) in nodes.iter().enumerate() {
            let x = start_x + i as f32 * (NODE_WIDTH + H_SPACING);
            let position = Pos2::new(x, y);

            let layout_node = LayoutNode {
                id: node.id.clone(),
                entity_type: EntityType::from_str(&node.node_type),
                primary_role: node
                    .primary_role
                    .as_ref()
                    .map(|r| PrimaryRole::from_str(r))
                    .unwrap_or(PrimaryRole::Unknown),
                all_roles: node.roles.clone(),
                label: node.label.clone(),
                sublabel: node.sublabel.clone(),
                jurisdiction: node.jurisdiction.clone(),
                position,
                size: node_size,
                in_focus: true,
                is_cbu_root: false,
                style: NodeStyle::default(),
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

        let node_size = Vec2::new(NODE_WIDTH, NODE_HEIGHT);

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

                let layout_node = LayoutNode {
                    id: product.id.clone(),
                    entity_type: EntityType::Product,
                    primary_role: PrimaryRole::Unknown,
                    all_roles: vec![],
                    label: product.label.clone(),
                    sublabel: product.sublabel.clone(),
                    jurisdiction: None,
                    position,
                    size: node_size,
                    in_focus: true,
                    is_cbu_root: false,
                    style: NodeStyle {
                        fill_color: Color32::from_rgb(88, 28, 135), // purple
                        border_color: Color32::from_rgb(168, 85, 247),
                        text_color: Color32::WHITE,
                        border_width: 2.0,
                    },
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

                        let svc_node = LayoutNode {
                            id: service.id.clone(),
                            entity_type: EntityType::Service,
                            primary_role: PrimaryRole::Unknown,
                            all_roles: vec![],
                            label: service.label.clone(),
                            sublabel: service.sublabel.clone(),
                            jurisdiction: None,
                            position: svc_position,
                            size: node_size,
                            in_focus: true,
                            is_cbu_root: false,
                            style: NodeStyle {
                                fill_color: Color32::from_rgb(30, 58, 138), // blue
                                border_color: Color32::from_rgb(96, 165, 250),
                                text_color: Color32::WHITE,
                                border_width: 2.0,
                            },
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

                                let res_node = LayoutNode {
                                    id: resource.id.clone(),
                                    entity_type: EntityType::Resource,
                                    primary_role: PrimaryRole::Unknown,
                                    all_roles: vec![],
                                    label: resource.label.clone(),
                                    sublabel: resource.sublabel.clone(),
                                    jurisdiction: None,
                                    position: res_position,
                                    size: node_size,
                                    in_focus: true,
                                    is_cbu_root: false,
                                    style: NodeStyle {
                                        fill_color: Color32::from_rgb(20, 83, 45), // green
                                        border_color: Color32::from_rgb(74, 222, 128),
                                        text_color: Color32::WHITE,
                                        border_width: 2.0,
                                    },
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

                let layout_node = LayoutNode {
                    id: service.id.clone(),
                    entity_type: EntityType::Service,
                    primary_role: PrimaryRole::Unknown,
                    all_roles: vec![],
                    label: service.label.clone(),
                    sublabel: service.sublabel.clone(),
                    jurisdiction: None,
                    position,
                    size: node_size,
                    in_focus: true,
                    is_cbu_root: false,
                    style: NodeStyle {
                        fill_color: Color32::from_rgb(30, 58, 138),
                        border_color: Color32::from_rgb(96, 165, 250),
                        text_color: Color32::WHITE,
                        border_width: 2.0,
                    },
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

                let layout_node = LayoutNode {
                    id: resource.id.clone(),
                    entity_type: EntityType::Resource,
                    primary_role: PrimaryRole::Unknown,
                    all_roles: vec![],
                    label: resource.label.clone(),
                    sublabel: resource.sublabel.clone(),
                    jurisdiction: None,
                    position,
                    size: node_size,
                    in_focus: true,
                    is_cbu_root: false,
                    style: NodeStyle {
                        fill_color: Color32::from_rgb(20, 83, 45),
                        border_color: Color32::from_rgb(74, 222, 128),
                        text_color: Color32::WHITE,
                        border_width: 2.0,
                    },
                };
                graph.nodes.insert(resource.id.clone(), layout_node);
            }
        }
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
        PrimaryRole::UltimateBeneficialOwner => NodeStyle {
            fill_color: Color32::from_rgb(21, 128, 61),
            border_color: Color32::from_rgb(34, 197, 94),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        PrimaryRole::Shareholder => NodeStyle {
            fill_color: Color32::from_rgb(22, 101, 52),
            border_color: Color32::from_rgb(74, 222, 128),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        PrimaryRole::ManagementCompany => NodeStyle {
            fill_color: Color32::from_rgb(124, 45, 18),
            border_color: Color32::from_rgb(251, 146, 60),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        PrimaryRole::Director => NodeStyle {
            fill_color: Color32::from_rgb(30, 64, 175),
            border_color: Color32::from_rgb(96, 165, 250),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        PrimaryRole::Principal => NodeStyle {
            fill_color: Color32::from_rgb(88, 28, 135),
            border_color: Color32::from_rgb(192, 132, 252),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        PrimaryRole::Trustee | PrimaryRole::Protector => NodeStyle {
            fill_color: Color32::from_rgb(120, 53, 15),
            border_color: Color32::from_rgb(217, 119, 6),
            text_color: Color32::WHITE,
            border_width: 2.0,
        },
        _ => NodeStyle::default(),
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
