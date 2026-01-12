//! Session Panel - Human-readable session state display
//!
//! Shows the current session state including:
//! - Active CBU context
//! - Symbol bindings (@fund, @person, etc.)
//! - Pending DSL (if any)
//! - View state (current navigation scope)

use crate::state::AppState;
use egui::Ui;

/// Render the session panel showing human-readable session state
pub fn session_panel(ui: &mut Ui, state: &mut AppState) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Session");
                if state.session_id.is_some() {
                    ui.label(
                        egui::RichText::new("Active")
                            .color(egui::Color32::GREEN)
                            .small(),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("None")
                            .color(egui::Color32::GRAY)
                            .small(),
                    );
                }
            });

            ui.separator();

            // Active CBU context
            if let Some(ref session) = state.session {
                if let Some(cbu_name) = session.active_cbu_name() {
                    ui.horizontal(|ui| {
                        ui.label("CBU:");
                        ui.strong(&cbu_name);
                    });
                }

                // Symbol bindings from context
                let bindings = session.get_bindings();
                if !bindings.is_empty() {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Bindings:").small().strong());
                    for (name, entity) in &bindings {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("@{}", name))
                                    .monospace()
                                    .small(),
                            );
                            ui.label(egui::RichText::new(&entity.name).small());
                        });
                    }
                }

                // Pending DSL preview - use combined_dsl
                if session.has_dsl() {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Pending DSL:").small().strong());
                    // Show first 100 chars with ellipsis
                    let preview: String = session.combined_dsl.chars().take(100).collect();
                    let display = if session.combined_dsl.len() > 100 {
                        format!("{}...", preview)
                    } else {
                        preview
                    };
                    ui.label(egui::RichText::new(display).monospace().small());
                }

                // Session state
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("State:").small());
                    let state_text = format!("{:?}", session.state);
                    ui.label(egui::RichText::new(state_text).monospace().small());
                });
            } else {
                ui.label(egui::RichText::new("No session loaded").italics().weak());
            }

            // Navigation scope (view state)
            ui.add_space(4.0);
            ui.label(egui::RichText::new("View:").small().strong());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{:?}", state.view_level))
                        .monospace()
                        .small(),
                );
                ui.label(egui::RichText::new("|").weak().small());
                ui.label(
                    egui::RichText::new(format!("{:?}", state.view_mode))
                        .monospace()
                        .small(),
                );
            });
        });
}
