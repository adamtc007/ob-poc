//! AST Panel
//!
//! Displays the parsed AST from the session's DslState.

use crate::state::AppState;
use egui::{Color32, RichText, ScrollArea, Ui};
use ob_poc_types::{AstStatement, SessionStateEnum};

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

        // Get DslState from session
        let Some(ref dsl_state) = session.dsl else {
            ui.centered_and_justified(|ui| {
                ui.label("Chat with the agent to generate DSL");
            });
            return;
        };

        // Check if we have AST statements
        let has_ast = dsl_state
            .ast
            .as_ref()
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        // Check if we have source
        let has_source = dsl_state
            .source
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false);

        if !has_ast && !has_source {
            ui.centered_and_justified(|ui| {
                ui.label("No DSL statements yet");
            });
            return;
        }

        // Display AST statements if available
        if let Some(ref ast) = dsl_state.ast {
            if !ast.is_empty() {
                ui.label(RichText::new(format!("{} statement(s)", ast.len())).small());
                ui.add_space(4.0);

                ScrollArea::vertical().show(ui, |ui| {
                    for (i, stmt) in ast.iter().enumerate() {
                        egui::Frame::default()
                            .fill(Color32::from_rgb(35, 35, 45))
                            .inner_margin(6.0)
                            .rounding(4.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("{}.", i + 1)).color(Color32::GRAY),
                                    );
                                    // Display verb and binding based on statement type
                                    let label = match stmt {
                                        AstStatement::VerbCall {
                                            domain,
                                            verb,
                                            binding,
                                            ..
                                        } => {
                                            let base = format!("{}.{}", domain, verb);
                                            if let Some(ref bind) = binding {
                                                format!("{} :as @{}", base, bind)
                                            } else {
                                                base
                                            }
                                        }
                                        AstStatement::Comment { text, .. } => {
                                            format!("; {}", text)
                                        }
                                    };
                                    ui.label(RichText::new(label).monospace());
                                });
                            });
                        ui.add_space(4.0);
                    }
                });
            }
        } else if let Some(ref source) = dsl_state.source {
            // Fallback: display raw source if no AST
            ui.label(RichText::new("DSL Source").small());
            ui.add_space(4.0);
            ScrollArea::vertical().show(ui, |ui| {
                egui::Frame::default()
                    .fill(Color32::from_rgb(35, 35, 45))
                    .inner_margin(6.0)
                    .rounding(4.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new(source).monospace());
                    });
            });
        }

        // Show validation status if available
        if let Some(ref validation) = dsl_state.validation {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Validation:").small());
                if validation.valid {
                    ui.label(RichText::new("Valid").small().color(Color32::GREEN));
                } else {
                    ui.label(RichText::new("Invalid").small().color(Color32::RED));
                }
            });

            // Show errors if any
            if !validation.errors.is_empty() {
                for err in &validation.errors {
                    ui.label(
                        RichText::new(format!("  • {}", err.message))
                            .small()
                            .color(Color32::RED),
                    );
                }
            }

            // Show warnings if any
            if !validation.warnings.is_empty() {
                for warn in &validation.warnings {
                    ui.label(
                        RichText::new(format!("  • {}", warn))
                            .small()
                            .color(Color32::YELLOW),
                    );
                }
            }
        }

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
                SessionStateEnum::Error => ("error", Color32::RED),
            };
            ui.label(RichText::new(state_str).small().color(color));

            if dsl_state.can_execute {
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
