//! Entity Detail Panel
//!
//! Displays details for the selected entity in the graph.
//! Entity data comes from graph_data (server), never stored locally.

use crate::state::AppState;
use egui::{Color32, RichText, ScrollArea, Ui};

pub fn entity_detail_panel(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        ui.heading("Entity Details");
        ui.separator();

        let Some(ref entity_id) = state.selected_entity_id else {
            ui.centered_and_justified(|ui| {
                ui.label("Select an entity in the graph");
            });
            return;
        };

        // Find entity in graph data
        let Some(ref graph) = state.graph_data else {
            ui.label("No graph loaded");
            return;
        };

        let Some(node) = graph.nodes.iter().find(|n| &n.id == entity_id) else {
            ui.label(format!("Entity {} not found", entity_id));
            return;
        };

        ScrollArea::vertical().show(ui, |ui| {
            // Entity header
            ui.horizontal(|ui| {
                ui.heading(&node.label);
                ui.label(
                    RichText::new(&node.node_type)
                        .italics()
                        .color(Color32::GRAY),
                );
            });

            // Sublabel if present
            if let Some(ref sublabel) = node.sublabel {
                ui.label(RichText::new(sublabel).small().color(Color32::GRAY));
            }

            ui.add_space(8.0);

            // Status badge
            ui.horizontal(|ui| {
                status_badge(ui, &node.status);

                // Extract risk rating from data JSON if present
                if let Some(risk) = node.data.get("risk_rating").and_then(|v| v.as_str()) {
                    risk_rating_badge(ui, risk);
                }
            });

            ui.add_space(12.0);
            ui.separator();

            // Roles
            if !node.roles.is_empty() {
                ui.label(RichText::new("Roles").strong());
                for role in &node.roles {
                    ui.horizontal(|ui| {
                        ui.label("-");
                        ui.label(role);
                    });
                }
                ui.add_space(8.0);
            }

            // Primary role
            if let Some(ref primary_role) = node.primary_role {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Primary Role:").strong());
                    ui.label(primary_role);
                });
            }

            // Jurisdiction
            if let Some(ref jurisdiction) = node.jurisdiction {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Jurisdiction:").strong());
                    ui.label(jurisdiction);
                });
            }

            // Ownership percentage from data JSON
            if let Some(pct) = node.data.get("ownership_pct").and_then(|v| v.as_f64()) {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Ownership:").strong());
                    ui.label(format!("{:.1}%", pct));
                });
            }

            // Layer info
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Layer:").small());
                ui.label(RichText::new(&node.layer).small());
            });

            // Entity ID
            ui.horizontal(|ui| {
                ui.label(RichText::new("ID:").small());
                ui.label(RichText::new(&node.id).monospace().small());
            });
        });
    });
}

fn status_badge(ui: &mut Ui, status: &str) {
    let (bg_color, text_color) = match status.to_uppercase().as_str() {
        "APPROVED" | "ACTIVE" | "VERIFIED" => (Color32::DARK_GREEN, Color32::WHITE),
        "PENDING" | "IN_PROGRESS" => (Color32::from_rgb(180, 140, 0), Color32::WHITE),
        "REJECTED" | "BLOCKED" | "FAILED" => (Color32::DARK_RED, Color32::WHITE),
        _ => (Color32::GRAY, Color32::WHITE),
    };

    egui::Frame::default()
        .fill(bg_color)
        .inner_margin(egui::vec2(6.0, 2.0))
        .rounding(3.0)
        .show(ui, |ui| {
            ui.label(RichText::new(status).color(text_color).small());
        });
}

fn risk_rating_badge(ui: &mut Ui, risk: &str) {
    let (bg_color, text_color) = match risk.to_uppercase().as_str() {
        "LOW" => (Color32::DARK_GREEN, Color32::WHITE),
        "MEDIUM" => (Color32::from_rgb(180, 140, 0), Color32::WHITE),
        "HIGH" => (Color32::from_rgb(180, 80, 0), Color32::WHITE),
        "VERY_HIGH" | "PROHIBITED" => (Color32::DARK_RED, Color32::WHITE),
        _ => (Color32::GRAY, Color32::WHITE),
    };

    egui::Frame::default()
        .fill(bg_color)
        .inner_margin(egui::vec2(6.0, 2.0))
        .rounding(3.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("Risk: {}", risk))
                    .color(text_color)
                    .small(),
            );
        });
}
