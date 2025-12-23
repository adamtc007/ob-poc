//! Context Panel
//!
//! Displays session context: active CBU, linked entities (KYC cases, products, etc.)
//! Context is fetched from server (state.session_context), never stored locally.
//!
//! Following EGUI rules:
//! - No local state mirroring server data
//! - Actions return values, no callbacks
//! - Server round-trip for mutations

use crate::state::AppState;
use egui::{Color32, ProgressBar, RichText, ScrollArea, Ui};
use ob_poc_types::semantic_stage::{SemanticState, StageStatus, StageWithStatus};
use ob_poc_types::{LinkedContext, SessionContext};

/// Action returned from context panel interactions
#[derive(Debug, Clone)]
#[allow(dead_code)] // Variants for future use
pub enum ContextPanelAction {
    /// User wants to switch to a different scope
    SwitchScope {
        scope_type: String,
        scope_id: String,
    },
    /// User clicked on a linked context
    SelectContext {
        context_type: String,
        context_id: String,
    },
    /// User clicked on a stage to focus on it
    FocusStage { stage_code: String },
    /// User wants to clear the stage focus
    ClearStageFocus,
}

/// Render the context panel
/// Returns an action if the user interacted with an item
pub fn context_panel(ui: &mut Ui, state: &AppState) -> Option<ContextPanelAction> {
    let mut action = None;

    ui.vertical(|ui| {
        ui.heading("Session Context");
        ui.separator();

        // Check loading state
        let is_loading = state
            .async_state
            .lock()
            .map(|s| s.loading_session_context)
            .unwrap_or(false);

        if is_loading {
            ui.centered_and_justified(|ui| {
                ui.spinner();
                ui.label("Loading context...");
            });
            return;
        }

        let Some(ref context) = state.session_context else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a CBU to see context");
            });
            return;
        };

        // Render the context
        action = render_context(ui, context);
    });

    action
}

/// Render session context contents
fn render_context(ui: &mut Ui, context: &SessionContext) -> Option<ContextPanelAction> {
    let mut action = None;

    ScrollArea::vertical().show(ui, |ui| {
        // CBU Section
        if let Some(ref cbu) = context.cbu {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("CBU").strong());
                    ui.label(&cbu.name);
                });

                ui.indent("cbu_details", |ui| {
                    if let Some(ref jurisdiction) = cbu.jurisdiction {
                        ui.horizontal(|ui| {
                            ui.label("Jurisdiction:");
                            ui.label(jurisdiction);
                        });
                    }
                    if let Some(ref client_type) = cbu.client_type {
                        ui.horizontal(|ui| {
                            ui.label("Type:");
                            ui.label(client_type);
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label("Entities:");
                        ui.label(format!("{}", cbu.entity_count));
                        ui.label("Roles:");
                        ui.label(format!("{}", cbu.role_count));
                    });
                    if let Some(ref status) = cbu.kyc_status {
                        ui.horizontal(|ui| {
                            ui.label("KYC:");
                            ui.label(status_label(status));
                        });
                    }
                    if let Some(ref rating) = cbu.risk_rating {
                        ui.horizontal(|ui| {
                            ui.label("Risk:");
                            ui.label(risk_label(rating));
                        });
                    }
                });
            });

            ui.add_space(8.0);
        }

        // Onboarding Journey (Semantic State)
        if let Some(ref semantic_state) = context.semantic_state {
            let focused = context.stage_focus.as_deref();
            if let Some(a) = render_semantic_stages(ui, semantic_state, focused) {
                action = Some(a);
            }
        }

        // KYC Cases
        if !context.kyc_cases.is_empty() {
            if let Some(a) = render_linked_section(ui, "KYC Cases", &context.kyc_cases) {
                action = Some(a);
            }
        }

        // Trading Matrix
        if let Some(ref trading) = context.trading_matrix {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Trading Profile").strong());
                    if ui.small_button(&trading.label).clicked() {
                        action = Some(ContextPanelAction::SelectContext {
                            context_type: trading.context_type.clone(),
                            context_id: trading.id.clone(),
                        });
                    }
                });
                if let Some(ref status) = trading.status {
                    ui.label(status_label(status));
                }
            });
            ui.add_space(8.0);
        }

        // ISDA Agreements
        if !context.isda_agreements.is_empty() {
            if let Some(a) = render_linked_section(ui, "ISDA Agreements", &context.isda_agreements)
            {
                action = Some(a);
            }
        }

        // Products
        if !context.product_subscriptions.is_empty() {
            if let Some(a) = render_linked_section(ui, "Products", &context.product_subscriptions) {
                action = Some(a);
            }
        }

        // Symbols (bindings)
        if !context.symbols.is_empty() {
            ui.group(|ui| {
                ui.label(RichText::new("Symbols").strong());
                ui.indent("symbols", |ui| {
                    for (name, value) in &context.symbols {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("@{}", name)).monospace());
                            ui.label("->");
                            ui.label(&value.display_name);
                            ui.label(
                                RichText::new(format!("({})", value.entity_type))
                                    .small()
                                    .weak(),
                            );
                        });
                    }
                });
            });
        }
    });

    action
}

/// Render a section of linked contexts
fn render_linked_section(
    ui: &mut Ui,
    title: &str,
    items: &[LinkedContext],
) -> Option<ContextPanelAction> {
    let mut action = None;

    ui.group(|ui| {
        ui.label(RichText::new(title).strong());
        ui.indent(title, |ui| {
            for item in items {
                ui.horizontal(|ui| {
                    if ui.small_button(&item.label).clicked() {
                        action = Some(ContextPanelAction::SelectContext {
                            context_type: item.context_type.clone(),
                            context_id: item.id.clone(),
                        });
                    }
                    if let Some(ref status) = item.status {
                        ui.label(status_label(status));
                    }
                });
            }
        });
    });

    ui.add_space(8.0);
    action
}

/// Render status with color
fn status_label(status: &str) -> RichText {
    let color = match status.to_uppercase().as_str() {
        "ACTIVE" | "APPROVED" | "COMPLETE" | "VERIFIED" => Color32::GREEN,
        "PENDING" | "INTAKE" | "DISCOVERY" | "DRAFT" => Color32::YELLOW,
        "REJECTED" | "BLOCKED" | "FAILED" => Color32::RED,
        "INACTIVE" | "SUSPENDED" => Color32::GRAY,
        _ => Color32::WHITE,
    };
    RichText::new(status).color(color).small()
}

/// Render risk rating with color
fn risk_label(rating: &str) -> RichText {
    let color = match rating.to_uppercase().as_str() {
        "LOW" => Color32::GREEN,
        "MEDIUM" => Color32::YELLOW,
        "HIGH" => Color32::from_rgb(255, 165, 0), // Orange
        "VERY_HIGH" | "PROHIBITED" => Color32::RED,
        _ => Color32::WHITE,
    };
    RichText::new(rating).color(color).small()
}

/// Render the semantic stages (onboarding journey progress)
/// Shows stages as a visual progress tracker
fn render_semantic_stages(
    ui: &mut Ui,
    semantic_state: &SemanticState,
    focused_stage: Option<&str>,
) -> Option<ContextPanelAction> {
    let mut action = None;

    ui.group(|ui| {
        // Header with overall progress and clear focus button
        ui.horizontal(|ui| {
            ui.label(RichText::new("Onboarding Journey").strong());
            let progress = &semantic_state.overall_progress;
            ui.label(
                RichText::new(format!(
                    "{}/{} ({:.0}%)",
                    progress.stages_complete, progress.stages_total, progress.percentage
                ))
                .small()
                .weak(),
            );

            // Show clear button if a stage is focused
            if focused_stage.is_some() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").on_hover_text("Clear focus").clicked() {
                        action = Some(ContextPanelAction::ClearStageFocus);
                    }
                });
            }
        });

        // Progress bar
        let progress_ratio = semantic_state.overall_progress.percentage / 100.0;
        ui.add(ProgressBar::new(progress_ratio).show_percentage());

        ui.add_space(4.0);

        // Stages list
        ui.indent("stages", |ui| {
            for stage in &semantic_state.required_stages {
                let is_focused = focused_stage == Some(stage.code.as_str());
                if let Some(a) = render_stage_row(ui, stage, is_focused) {
                    action = Some(a);
                }
            }
        });

        // Next actionable stages hint
        if !semantic_state.next_actionable.is_empty() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Next:").small().weak());
                for stage_code in &semantic_state.next_actionable {
                    ui.label(RichText::new(stage_code).small().color(Color32::LIGHT_BLUE));
                }
            });
        }

        // Missing entities hint (if any)
        if !semantic_state.missing_entities.is_empty() && semantic_state.missing_entities.len() <= 3
        {
            ui.add_space(4.0);
            ui.label(RichText::new("Missing:").small().weak());
            for missing in semantic_state.missing_entities.iter().take(3) {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("  {} ({})", missing.entity_type, missing.stage))
                            .small()
                            .color(Color32::YELLOW),
                    );
                });
            }
        }
    });

    ui.add_space(8.0);
    action
}

/// Render a single stage row with status indicator
/// Rows are clickable to focus on that stage
fn render_stage_row(
    ui: &mut Ui,
    stage: &StageWithStatus,
    is_focused: bool,
) -> Option<ContextPanelAction> {
    let mut action = None;

    // Use a frame for the clickable row with focus highlighting
    let frame = if is_focused {
        egui::Frame::none()
            .fill(Color32::from_rgba_unmultiplied(100, 149, 237, 40)) // Cornflower blue, low alpha
            .rounding(2.0)
            .inner_margin(2.0)
    } else {
        egui::Frame::none().rounding(2.0).inner_margin(2.0)
    };

    let response = frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            // Focus indicator for focused stage
            if is_focused {
                ui.label(RichText::new("▶").color(Color32::LIGHT_BLUE).small());
            }

            // Status icon
            let (icon, color) = stage_status_icon(&stage.status);
            ui.label(RichText::new(icon).color(color));

            // Stage name - highlight if focused
            let name_style = if is_focused {
                RichText::new(&stage.name)
                    .color(Color32::LIGHT_BLUE)
                    .strong()
            } else {
                match stage.status {
                    StageStatus::Complete => RichText::new(&stage.name).color(Color32::GRAY),
                    StageStatus::InProgress => RichText::new(&stage.name).color(Color32::WHITE),
                    StageStatus::NotStarted => RichText::new(&stage.name).weak(),
                    StageStatus::Blocked => RichText::new(&stage.name).color(Color32::DARK_GRAY),
                    StageStatus::NotRequired => RichText::new(&stage.name)
                        .strikethrough()
                        .color(Color32::DARK_GRAY),
                }
            };
            ui.label(name_style);

            // Entity count if in progress
            if stage.status == StageStatus::InProgress {
                let done = stage.required_entities.iter().filter(|e| e.exists).count();
                let total = stage.required_entities.len();
                ui.label(
                    RichText::new(format!("({}/{})", done, total))
                        .small()
                        .weak(),
                );
            }

            // Blocking indicator
            if stage.is_blocking && stage.status != StageStatus::Complete {
                ui.label(RichText::new("!").color(Color32::RED).small());
            }
        });
    });

    // Make the entire row clickable
    let row_response = response.response.interact(egui::Sense::click());
    if row_response.clicked() {
        action = Some(ContextPanelAction::FocusStage {
            stage_code: stage.code.clone(),
        });
    }

    // Show tooltip on hover
    if row_response.hovered() {
        row_response.on_hover_text(&stage.description);
    }

    action
}

/// Get icon and color for stage status
fn stage_status_icon(status: &StageStatus) -> (&'static str, Color32) {
    match status {
        StageStatus::Complete => ("✓", Color32::GREEN),
        StageStatus::InProgress => ("◐", Color32::YELLOW),
        StageStatus::NotStarted => ("○", Color32::GRAY),
        StageStatus::Blocked => ("⊘", Color32::RED),
        StageStatus::NotRequired => ("─", Color32::DARK_GRAY),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_label_colors() {
        // Just verify these don't panic
        let _ = status_label("ACTIVE");
        let _ = status_label("PENDING");
        let _ = status_label("REJECTED");
        let _ = status_label("unknown");
    }

    #[test]
    fn test_risk_label_colors() {
        let _ = risk_label("LOW");
        let _ = risk_label("HIGH");
        let _ = risk_label("unknown");
    }
}
