//! Results Panel
//!
//! Displays execution results from the server.
//! Results are fetched from server (state.execution), never stored locally.

use crate::state::AppState;
use egui::{Color32, RichText, ScrollArea, Ui};

pub fn results_panel(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        ui.heading("Execution Results");
        ui.separator();

        let Some(ref execution) = state.execution else {
            ui.centered_and_justified(|ui| {
                ui.label("Execute DSL to see results");
            });
            return;
        };

        // Summary
        ui.horizontal(|ui| {
            if execution.success {
                ui.label(RichText::new("Success").color(Color32::GREEN));
            } else {
                ui.label(RichText::new("Failed").color(Color32::RED));
            }
            ui.separator();
            ui.label(format!("{} steps", execution.results.len()));
        });

        ui.add_space(8.0);

        // Errors (if any)
        if !execution.errors.is_empty() {
            ui.label(RichText::new("Errors:").color(Color32::RED).strong());
            for error in &execution.errors {
                ui.label(RichText::new(error).color(Color32::RED).small());
            }
            ui.add_space(8.0);
        }

        // Step results
        ScrollArea::vertical().show(ui, |ui| {
            for result in &execution.results {
                ui.horizontal(|ui| {
                    let icon = if result.success { "+" } else { "x" };
                    let color = if result.success {
                        Color32::GREEN
                    } else {
                        Color32::RED
                    };
                    ui.label(RichText::new(icon).color(color));
                    ui.label(format!("{}.", result.statement_index + 1));
                    ui.label(&result.message);
                });

                if let Some(ref entity_id) = result.entity_id {
                    ui.indent(format!("step_{}_entity", result.statement_index), |ui| {
                        ui.label(
                            RichText::new(format!("-> {}", entity_id))
                                .monospace()
                                .small(),
                        );
                    });
                }

                ui.add_space(4.0);
            }
        });

        // Bindings summary
        if let Some(ref bindings) = execution.bindings {
            if !bindings.is_empty() {
                ui.separator();
                ui.label(RichText::new("Bindings").strong());
                for (name, value) in bindings {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("@{}", name)).monospace());
                        ui.label("->");
                        ui.label(RichText::new(value).small());
                    });
                }
            }
        }
    });
}
