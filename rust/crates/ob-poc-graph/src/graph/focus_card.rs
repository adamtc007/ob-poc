//! Focus card panel - shows entity details when focused
//!
//! Displays a floating panel with entity information when a node is clicked.

#![allow(dead_code)]

use egui::{Align2, Color32, RichText, Ui, Vec2};

use super::colors::{kyc_status_color, risk_color, role_color, KycStatus, RiskRating};
use super::types::{EntityType, LayoutGraph, LayoutNode, PrimaryRole};

// =============================================================================
// FOCUS CARD DATA
// =============================================================================

/// Data structure for the focus card
#[derive(Debug, Clone)]
pub struct FocusCardData {
    pub entity_id: String,
    pub name: String,
    pub entity_type: EntityType,
    pub jurisdiction: Option<String>,
    pub roles: Vec<RoleInfo>,
    pub kyc_status: Option<KycStatus>,
    pub risk_rating: Option<RiskRating>,
    pub connected_entities: Vec<ConnectedEntity>,
}

/// Role information for the focus card
#[derive(Debug, Clone)]
pub struct RoleInfo {
    pub role: String,
    pub target_entity: Option<String>,
    pub ownership_pct: Option<f32>,
}

/// Connected entity for navigation
#[derive(Debug, Clone)]
pub struct ConnectedEntity {
    pub entity_id: String,
    pub name: String,
    pub relationship: String,
}

// =============================================================================
// BUILD FOCUS CARD DATA
// =============================================================================

/// Build focus card data from a layout node and graph
pub fn build_focus_card_data(node: &LayoutNode, graph: &LayoutGraph) -> FocusCardData {
    // Find connected entities from edges
    let mut connected = Vec::new();

    for edge in &graph.edges {
        if edge.source_id == node.id {
            if let Some(target) = graph.get_node(&edge.target_id) {
                connected.push(ConnectedEntity {
                    entity_id: target.id.clone(),
                    name: target.label.clone(),
                    relationship: format!("{:?} ->", edge.edge_type),
                });
            }
        } else if edge.target_id == node.id {
            if let Some(source) = graph.get_node(&edge.source_id) {
                connected.push(ConnectedEntity {
                    entity_id: source.id.clone(),
                    name: source.label.clone(),
                    relationship: format!("<- {:?}", edge.edge_type),
                });
            }
        }
    }

    // Build role info from all_roles
    let roles: Vec<RoleInfo> = node
        .all_roles
        .iter()
        .map(|r| RoleInfo {
            role: r.clone(),
            target_entity: None,
            ownership_pct: None,
        })
        .collect();

    FocusCardData {
        entity_id: node.id.clone(),
        name: node.label.clone(),
        entity_type: node.entity_type,
        jurisdiction: node.jurisdiction.clone(),
        roles,
        kyc_status: None, // TODO: Get from node data when available
        risk_rating: None,
        connected_entities: connected,
    }
}

// =============================================================================
// RENDER FOCUS CARD
// =============================================================================

/// Render the focus card as an egui::Window
pub fn render_focus_card(
    ctx: &egui::Context,
    data: &FocusCardData,
    on_close: &mut dyn FnMut(),
    on_navigate: &mut dyn FnMut(&str),
) {
    egui::Window::new("Entity Details")
        .id(egui::Id::new("focus_card"))
        .default_size([280.0, 350.0])
        .anchor(Align2::RIGHT_TOP, [-20.0, 60.0])
        .collapsible(true)
        .resizable(true)
        .show(ctx, |ui| {
            render_focus_card_content(ui, data, on_close, on_navigate);
        });
}

/// Render the content of the focus card
fn render_focus_card_content(
    ui: &mut Ui,
    data: &FocusCardData,
    on_close: &mut dyn FnMut(),
    on_navigate: &mut dyn FnMut(&str),
) {
    // Header
    ui.horizontal(|ui| {
        ui.heading(&data.name);
    });

    // Entity type badge
    ui.horizontal(|ui| {
        let type_text = format!("{:?}", data.entity_type);
        ui.label(RichText::new(type_text).color(Color32::from_rgb(107, 114, 128)));

        if let Some(ref jurisdiction) = data.jurisdiction {
            ui.label("|");
            ui.label(RichText::new(jurisdiction).color(Color32::from_rgb(107, 114, 128)));
        }
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // Roles section
    if !data.roles.is_empty() {
        ui.collapsing(RichText::new("Roles").strong(), |ui| {
            for role in &data.roles {
                ui.horizontal(|ui| {
                    let role_parsed: PrimaryRole =
                        role.role.parse().unwrap_or(PrimaryRole::Unknown);
                    let color = role_color(role_parsed);
                    ui.colored_label(color, format!("  {}", role.role));

                    if let Some(ref target) = role.target_entity {
                        ui.label(RichText::new(format!("-> {}", target)).small());
                    }
                    if let Some(pct) = role.ownership_pct {
                        ui.label(RichText::new(format!("({}%)", pct)).small());
                    }
                });
            }
        });
    }

    // KYC Status
    if let Some(ref status) = data.kyc_status {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("KYC Status:");
            ui.colored_label(kyc_status_color(*status), format!("{:?}", status));
        });
    }

    // Risk Rating
    if let Some(ref risk) = data.risk_rating {
        ui.horizontal(|ui| {
            ui.label("Risk Rating:");
            ui.colored_label(risk_color(*risk), format!("{:?}", risk));
        });
    }

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // Connected entities section
    if !data.connected_entities.is_empty() {
        ui.collapsing(
            RichText::new(format!("Connections ({})", data.connected_entities.len())).strong(),
            |ui| {
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for conn in &data.connected_entities {
                            ui.horizontal(|ui| {
                                if ui.link(&conn.name).clicked() {
                                    on_navigate(&conn.entity_id);
                                }
                            });
                            ui.label(
                                RichText::new(format!("    {}", conn.relationship))
                                    .small()
                                    .color(Color32::from_rgb(156, 163, 175)),
                            );
                        }
                    });
            },
        );
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // Actions
    ui.horizontal(|ui| {
        if ui.button("Close").clicked() {
            on_close();
        }
        // Future: Add more actions like "View Full Details", "Edit", etc.
    });
}

// =============================================================================
// INLINE FOCUS INDICATOR
// =============================================================================

/// Render a small focus indicator next to the focused node
pub fn render_focus_indicator(
    painter: &egui::Painter,
    screen_pos: egui::Pos2,
    node_size: Vec2,
    zoom: f32,
) {
    let indicator_size = 8.0 * zoom;
    let offset = Vec2::new(node_size.x / 2.0 + indicator_size, -node_size.y / 2.0);
    let indicator_pos = screen_pos + offset;

    // Pulsing blue dot
    painter.circle_filled(
        indicator_pos,
        indicator_size,
        Color32::from_rgb(59, 130, 246),
    );

    // White inner
    painter.circle_filled(indicator_pos, indicator_size * 0.5, Color32::WHITE);
}
