//! DSL Editor Panel
//!
//! Text editor for DSL with syntax highlighting, validation, and execution.
//! The DSL text is stored in state.buffers.dsl_editor (the ONLY local mutable state).

use crate::state::AppState;
use egui::{Color32, RichText, ScrollArea, TextEdit, Ui};

pub fn dsl_editor_panel(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        // Header with actions
        ui.horizontal(|ui| {
            ui.heading("DSL Editor");

            if state.buffers.dsl_dirty {
                ui.label(RichText::new("*").color(Color32::YELLOW));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Execute button
                let executing = state
                    .async_state
                    .lock()
                    .map(|s| s.executing)
                    .unwrap_or(false);

                if ui
                    .add_enabled(!executing, egui::Button::new("Execute"))
                    .clicked()
                {
                    state.execute_dsl();
                }

                // Validate button
                if ui.button("Validate").clicked() {
                    state.validate_dsl();
                }

                // Clear button
                if ui.button("Clear").clicked() {
                    state.buffers.dsl_editor.clear();
                    state.buffers.dsl_dirty = true;
                    state.validation_errors.clear();
                }
            });
        });

        ui.separator();

        // Validation errors (if any)
        if !state.validation_errors.is_empty() {
            egui::Frame::default()
                .fill(Color32::from_rgb(60, 30, 30))
                .inner_margin(4.0)
                .show(ui, |ui| {
                    for error in &state.validation_errors {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("!").color(Color32::RED));
                            ui.label(RichText::new(error).color(Color32::LIGHT_RED).small());
                        });
                    }
                });
            ui.separator();
        }

        // Editor area
        ScrollArea::vertical().show(ui, |ui| {
            let response = TextEdit::multiline(&mut state.buffers.dsl_editor)
                .font(egui::TextStyle::Monospace)
                .code_editor()
                .desired_width(f32::INFINITY)
                .desired_rows(15)
                .show(ui);

            if response.response.changed() {
                state.buffers.dsl_dirty = true;
                // Clear validation when content changes
                state.validation_errors.clear();
            }
        });

        // Status bar
        ui.separator();
        ui.horizontal(|ui| {
            let line_count = state.buffers.dsl_editor.lines().count();
            ui.label(RichText::new(format!("{} lines", line_count)).small());

            if let Some(ref session) = state.session {
                if let Some(ref cbu) = session.active_cbu {
                    ui.separator();
                    ui.label(RichText::new(format!("@cbu: {}", cbu.name)).small());
                }
            }
        });
    });
}
