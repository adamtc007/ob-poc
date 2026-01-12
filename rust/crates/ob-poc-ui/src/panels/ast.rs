//! AST Panel
//!
//! Displays the DSL statements from the session.
//! Note: Server sends combined_dsl as string, not parsed AST.

use crate::state::AppState;
use egui::{Color32, RichText, ScrollArea, Ui};
use ob_poc_types::SessionStateEnum;

pub fn ast_panel(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        ui.heading("DSL");
        ui.separator();

        let Some(ref session) = state.session else {
            ui.centered_and_justified(|ui| {
                ui.label("No session");
            });
            return;
        };

        // Check if we have any DSL content
        if !session.has_dsl() {
            ui.centered_and_justified(|ui| {
                ui.label("Chat with the agent to generate DSL");
            });
            return;
        }

        // Display assembled DSL statements
        let stmt_count = session.assembled_dsl.len();
        ui.label(RichText::new(format!("{} statement(s)", stmt_count)).small());
        ui.add_space(4.0);

        ScrollArea::vertical().show(ui, |ui| {
            for (i, dsl_stmt) in session.assembled_dsl.iter().enumerate() {
                egui::Frame::default()
                    .fill(Color32::from_rgb(35, 35, 45))
                    .inner_margin(6.0)
                    .rounding(4.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{}.", i + 1)).color(Color32::GRAY));
                            ui.label(RichText::new(dsl_stmt).monospace());
                        });
                    });
                ui.add_space(4.0);
            }
        });

        // Show session state
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new("State:").small());
            let (state_str, color) = match &session.state {
                SessionStateEnum::ReadyToExecute => ("ready_to_execute", Color32::GREEN),
                SessionStateEnum::New => ("new", Color32::GRAY),
                SessionStateEnum::PendingValidation => ("pending_validation", Color32::YELLOW),
                SessionStateEnum::Executing => ("executing", Color32::LIGHT_BLUE),
                SessionStateEnum::Executed => ("executed", Color32::GREEN),
                SessionStateEnum::Closed => ("closed", Color32::GRAY),
            };
            ui.label(RichText::new(state_str).small().color(color));

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
