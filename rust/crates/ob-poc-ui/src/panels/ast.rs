//! AST Panel
//!
//! Displays the parsed AST from the session's assembled_dsl.

use crate::state::AppState;
use egui::{Color32, RichText, ScrollArea, Ui};

pub fn ast_panel(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        ui.heading("AST");
        ui.separator();

        let Some(ref session) = state.session else {
            ui.centered_and_justified(|ui| {
                ui.label("No session");
            });
            return;
        };

        // Get assembled_dsl from session
        let assembled = session.assembled_dsl.as_ref();

        if assembled.is_none() || assembled.map(|v| v.is_null()).unwrap_or(true) {
            ui.centered_and_justified(|ui| {
                ui.label("Chat with the agent to generate DSL");
            });
            return;
        }

        let assembled = assembled.unwrap();

        // assembled_dsl can be either:
        // 1. An object with {statements: [...], combined: "...", intent_count: N}
        // 2. An array of DSL strings directly
        // 3. A single string

        // Try to extract statements from object format first
        let statements: Vec<&serde_json::Value> = if let Some(obj) = assembled.as_object() {
            // Object format: {statements: [...], combined: "..."}
            if let Some(stmts) = obj.get("statements").and_then(|v| v.as_array()) {
                stmts.iter().collect()
            } else {
                vec![]
            }
        } else if let Some(arr) = assembled.as_array() {
            // Direct array format
            arr.iter().collect()
        } else {
            vec![]
        };

        if statements.is_empty() {
            // Maybe it's just a string?
            if let Some(s) = assembled.as_str() {
                ui.label(RichText::new("1 statement").small());
                ui.add_space(4.0);
                ScrollArea::vertical().show(ui, |ui| {
                    egui::Frame::default()
                        .fill(Color32::from_rgb(35, 35, 45))
                        .inner_margin(6.0)
                        .rounding(4.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new(s).monospace());
                        });
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("No DSL statements yet");
                });
            }
            return;
        }

        ui.label(RichText::new(format!("{} statement(s)", statements.len())).small());
        ui.add_space(4.0);

        ScrollArea::vertical().show(ui, |ui| {
            for (i, stmt) in statements.iter().enumerate() {
                egui::Frame::default()
                    .fill(Color32::from_rgb(35, 35, 45))
                    .inner_margin(6.0)
                    .rounding(4.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{}.", i + 1)).color(Color32::GRAY));
                            if let Some(s) = stmt.as_str() {
                                ui.label(RichText::new(s).monospace());
                            } else {
                                // If it's not a string, pretty-print the JSON
                                let json_str = serde_json::to_string_pretty(stmt)
                                    .unwrap_or_else(|_| stmt.to_string());
                                ui.label(RichText::new(json_str).monospace().small());
                            }
                        });
                    });
                ui.add_space(4.0);
            }
        });

        // Show session state
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new("State:").small());
            if let Some(state_str) = session.state.as_str() {
                let color = match state_str {
                    "ready_to_execute" => Color32::GREEN,
                    "new" => Color32::GRAY,
                    _ => Color32::YELLOW,
                };
                ui.label(RichText::new(state_str).small().color(color));
            }

            if session.can_execute {
                ui.separator();
                ui.label(
                    RichText::new("Ready to execute")
                        .small()
                        .color(Color32::GREEN),
                );
            }
        });
    });
}
