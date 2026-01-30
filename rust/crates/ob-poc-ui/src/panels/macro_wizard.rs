//! Macro Expansion Wizard Panel
//!
//! Shows a step-by-step wizard for filling in missing macro arguments.
//! When a macro is invoked with missing required arguments, this wizard
//! guides the user through providing values.
//!
//! Features:
//! - Step-by-step progress indicator
//! - Enum dropdown for enum args
//! - Entity picker search for party_ref/structure_ref args
//! - Text input for string args
//! - "Use Placeholder" option for service provider args
//! - Keyboard navigation (Enter to proceed, Escape to cancel)
//!
//! Follows EGUI-RULES:
//! - Returns Option<MacroWizardAction>, no callbacks
//! - Data passed in, not mutated directly

use crate::state::{MacroEnumOption, MacroExpansionState, MissingArgInfo};
use egui::{Align2, Color32, Key, RichText, ScrollArea, TextEdit, Vec2};
use ob_poc_types::EntityMatch;

/// Actions from macro wizard
#[derive(Clone, Debug)]
pub enum MacroWizardAction {
    /// User provided a value for current arg and wants to proceed
    Next { arg_name: String, value: String },

    /// User wants to go back to previous step
    Back,

    /// User selected to use a placeholder for this arg
    UsePlaceholder { arg_name: String },

    /// User selected an entity from picker
    SelectEntity {
        arg_name: String,
        entity_id: String,
        display_name: String,
    },

    /// User wants to search for entities
    Search { query: String, entity_type: String },

    /// User cancelled the wizard
    Cancel,

    /// User completed all steps - ready to execute
    Complete,

    /// User skipped an optional argument
    Skip { arg_name: String },
}

/// Render macro expansion wizard modal
/// Returns action if user interacted
pub fn macro_wizard_modal(
    ctx: &egui::Context,
    state: &MacroExpansionState,
    search_buffer: &mut String,
) -> Option<MacroWizardAction> {
    if !state.active {
        return None;
    }

    // Handle global keyboard shortcuts first (outside window closure)
    let keyboard_action = handle_keyboard_shortcuts(ctx, state);
    if keyboard_action.is_some() {
        return keyboard_action;
    }

    let mut result_action: Option<MacroWizardAction> = None;

    egui::Window::new("Structure Setup Wizard")
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(550.0, 480.0))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            // Header with macro info
            ui.horizontal(|ui| {
                if let Some(ref label) = state.macro_label {
                    ui.heading(label);
                } else if let Some(ref fqn) = state.macro_fqn {
                    ui.heading(fqn);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("").clicked() {
                        result_action = Some(MacroWizardAction::Cancel);
                    }
                });
            });

            // Description
            if let Some(ref desc) = state.macro_description {
                ui.label(RichText::new(desc).italics().color(Color32::LIGHT_GRAY));
            }

            ui.add_space(4.0);
            ui.separator();

            // Progress indicator
            render_progress(ui, state);

            ui.add_space(8.0);
            ui.separator();

            // Current argument form
            if result_action.is_none() {
                if let Some(current_arg) = state.current_arg() {
                    result_action = render_argument_form(ui, current_arg, state, search_buffer);
                } else {
                    // All args filled - show summary
                    result_action = render_summary(ui, state);
                }
            }

            ui.add_space(8.0);
            ui.separator();

            // Footer buttons
            if result_action.is_none() {
                result_action = render_footer_buttons(ui, state);
            }
        });

    result_action
}

/// Handle keyboard shortcuts
fn handle_keyboard_shortcuts(
    ctx: &egui::Context,
    state: &MacroExpansionState,
) -> Option<MacroWizardAction> {
    ctx.input(|i| {
        // Escape to cancel
        if i.key_pressed(Key::Escape) {
            return Some(MacroWizardAction::Cancel);
        }

        // Enter to proceed (if we have input)
        if i.key_pressed(Key::Enter) {
            if let Some(arg) = state.current_arg() {
                if !state.current_input.is_empty() {
                    return Some(MacroWizardAction::Next {
                        arg_name: arg.name.clone(),
                        value: state.current_input.clone(),
                    });
                }
            }
        }

        None
    })
}

/// Render progress indicator showing steps
fn render_progress(ui: &mut egui::Ui, state: &MacroExpansionState) {
    let total = state.total_steps();
    let current = state.current_step;

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("Step {} of {}", current + 1, total))
                .small()
                .color(Color32::GRAY),
        );

        ui.add_space(8.0);

        // Visual progress dots
        for i in 0..total {
            let color = if i < current {
                Color32::from_rgb(100, 180, 100) // Completed
            } else if i == current {
                Color32::from_rgb(100, 150, 220) // Current
            } else {
                Color32::from_rgb(80, 80, 90) // Pending
            };

            let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, color);
        }
    });
}

/// Render the form for current argument
fn render_argument_form(
    ui: &mut egui::Ui,
    arg: &MissingArgInfo,
    state: &MacroExpansionState,
    search_buffer: &mut String,
) -> Option<MacroWizardAction> {
    #[allow(unused_assignments)]
    let mut action: Option<MacroWizardAction> = None;

    ui.add_space(8.0);

    // Argument label and required indicator
    ui.horizontal(|ui| {
        ui.label(RichText::new(&arg.ui_label).strong().size(16.0));
        if arg.required {
            ui.label(RichText::new("*").color(Color32::from_rgb(220, 100, 100)));
        } else {
            ui.label(RichText::new("(optional)").small().color(Color32::GRAY));
        }
    });

    // Description/help text
    if let Some(ref desc) = arg.description {
        ui.label(
            RichText::new(desc)
                .small()
                .italics()
                .color(Color32::from_rgb(150, 150, 170)),
        );
    }

    ui.add_space(8.0);

    // Render appropriate input widget based on arg type
    match arg.arg_type.as_str() {
        "enum" => {
            action = render_enum_input(ui, arg, state);
        }
        "party_ref" | "structure_ref" | "case_ref" | "mandate_ref" => {
            action = render_entity_picker(ui, arg, state, search_buffer);
        }
        "str" | "date" => {
            action = render_text_input(ui, arg, state);
        }
        _ => {
            // Default to text input
            action = render_text_input(ui, arg, state);
        }
    }

    // Placeholder option for service provider refs
    if is_service_provider_arg(&arg.arg_type) && !arg.required {
        ui.add_space(8.0);
        if ui
            .button("Use Placeholder (TBD)")
            .on_hover_text("Creates a placeholder entity to be resolved later")
            .clicked()
        {
            action = Some(MacroWizardAction::UsePlaceholder {
                arg_name: arg.name.clone(),
            });
        }
    }

    action
}

/// Render enum dropdown
fn render_enum_input(
    ui: &mut egui::Ui,
    arg: &MissingArgInfo,
    _state: &MacroExpansionState,
) -> Option<MacroWizardAction> {
    let mut result: Option<MacroWizardAction> = None;

    for opt in &arg.valid_values {
        let is_default = arg
            .default_value
            .as_ref()
            .map(|d| d == &opt.key)
            .unwrap_or(false);

        let label = if is_default {
            format!("{} (default)", opt.label)
        } else {
            opt.label.clone()
        };

        let button = egui::Button::new(&label)
            .fill(Color32::from_rgb(50, 55, 65))
            .min_size(Vec2::new(300.0, 32.0));

        if ui.add(button).clicked() {
            result = Some(MacroWizardAction::Next {
                arg_name: arg.name.clone(),
                value: opt.key.clone(),
            });
        }

        ui.add_space(4.0);
    }

    result
}

/// Render entity picker with search
fn render_entity_picker(
    ui: &mut egui::Ui,
    arg: &MissingArgInfo,
    state: &MacroExpansionState,
    search_buffer: &mut String,
) -> Option<MacroWizardAction> {
    let mut action: Option<MacroWizardAction> = None;

    // Search input
    ui.horizontal(|ui| {
        ui.label("Search:");
        let response = TextEdit::singleline(search_buffer)
            .desired_width(300.0)
            .hint_text(format!("Search for {}...", arg.ui_label.to_lowercase()))
            .show(ui);

        // Trigger search on Enter
        if response.response.lost_focus()
            && ui.input(|i| i.key_pressed(Key::Enter))
            && search_buffer.len() >= 2
        {
            action = Some(MacroWizardAction::Search {
                query: search_buffer.clone(),
                entity_type: arg.arg_type.clone(),
            });
        }

        if ui.button("Search").clicked() && search_buffer.len() >= 2 {
            action = Some(MacroWizardAction::Search {
                query: search_buffer.clone(),
                entity_type: arg.arg_type.clone(),
            });
        }

        if state.loading {
            ui.spinner();
        }
    });

    // Show picker results
    if let Some(ref results) = state.picker_results {
        ui.add_space(8.0);

        ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
            if results.is_empty() {
                ui.label(
                    RichText::new("No matches found")
                        .italics()
                        .color(Color32::GRAY),
                );
            } else {
                for (idx, m) in results.iter().enumerate() {
                    if let Some(select_action) = render_entity_row(ui, idx, m, &arg.name) {
                        action = Some(select_action);
                    }
                }
            }
        });
    }

    // Manual entry option
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Or enter ID directly:")
                .small()
                .color(Color32::GRAY),
        );
    });

    action
}

/// Render entity match row
fn render_entity_row(
    ui: &mut egui::Ui,
    index: usize,
    m: &EntityMatch,
    arg_name: &str,
) -> Option<MacroWizardAction> {
    let mut action: Option<MacroWizardAction> = None;

    let shortcut_hint = if index < 9 {
        format!("[{}] ", index + 1)
    } else {
        "    ".to_string()
    };

    egui::Frame::default()
        .fill(Color32::from_rgb(45, 50, 55))
        .inner_margin(6.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&shortcut_hint)
                        .monospace()
                        .small()
                        .color(Color32::from_rgb(100, 150, 200)),
                );

                if ui.button("Select").clicked() {
                    action = Some(MacroWizardAction::SelectEntity {
                        arg_name: arg_name.to_string(),
                        entity_id: m.entity_id.clone(),
                        display_name: m.name.clone(),
                    });
                }

                ui.label(RichText::new(&m.name).strong());

                if let Some(ref jur) = m.jurisdiction {
                    ui.label(RichText::new(jur).small().color(Color32::LIGHT_GRAY));
                }
            });
        });

    ui.add_space(2.0);
    action
}

/// Render text input
fn render_text_input(
    ui: &mut egui::Ui,
    arg: &MissingArgInfo,
    _state: &MacroExpansionState,
) -> Option<MacroWizardAction> {
    let mut action: Option<MacroWizardAction> = None;
    let mut input_value = String::new();

    ui.horizontal(|ui| {
        let response = TextEdit::singleline(&mut input_value)
            .desired_width(350.0)
            .hint_text(&arg.ui_label)
            .show(ui);

        // Submit on Enter
        if response.response.lost_focus()
            && ui.input(|i| i.key_pressed(Key::Enter))
            && !input_value.is_empty()
        {
            action = Some(MacroWizardAction::Next {
                arg_name: arg.name.clone(),
                value: input_value.clone(),
            });
        }
    });

    // Show default value hint if available
    if let Some(ref default) = arg.default_value {
        ui.label(
            RichText::new(format!("Default: {}", default))
                .small()
                .color(Color32::GRAY),
        );
    }

    action
}

/// Render summary of all provided args
fn render_summary(ui: &mut egui::Ui, state: &MacroExpansionState) -> Option<MacroWizardAction> {
    ui.heading("Review");
    ui.add_space(8.0);

    ui.label("All arguments have been provided:");
    ui.add_space(4.0);

    egui::Frame::default()
        .fill(Color32::from_rgb(40, 45, 50))
        .inner_margin(12.0)
        .rounding(6.0)
        .show(ui, |ui| {
            for (name, value) in &state.provided_args {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}:", name)).color(Color32::GRAY));
                    ui.label(RichText::new(value).strong());
                });
            }
        });

    ui.add_space(12.0);

    // DSL preview
    if let Some(ref fqn) = state.macro_fqn {
        ui.label(RichText::new("Generated DSL:").small().color(Color32::GRAY));
        let dsl = format!("({} {})", fqn, state.to_dsl_args());
        ui.add(
            TextEdit::multiline(&mut dsl.as_str())
                .code_editor()
                .desired_width(f32::INFINITY)
                .desired_rows(3)
                .interactive(false),
        );
    }

    None
}

/// Render footer buttons
fn render_footer_buttons(
    ui: &mut egui::Ui,
    state: &MacroExpansionState,
) -> Option<MacroWizardAction> {
    let mut action: Option<MacroWizardAction> = None;

    ui.horizontal(|ui| {
        // Back button (if not on first step)
        if state.current_step > 0 && ui.button(" Back").clicked() {
            action = Some(MacroWizardAction::Back);
        }

        // Skip button (for optional args)
        if let Some(arg) = state.current_arg() {
            if !arg.required && ui.button("Skip").clicked() {
                action = Some(MacroWizardAction::Skip {
                    arg_name: arg.name.clone(),
                });
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Cancel button
            if ui.button("Cancel").clicked() {
                action = Some(MacroWizardAction::Cancel);
            }

            // Complete button (if all args filled)
            if state.current_step >= state.total_steps()
                && ui
                    .add(egui::Button::new(" Complete").fill(Color32::from_rgb(60, 120, 60)))
                    .clicked()
            {
                action = Some(MacroWizardAction::Complete);
            }
        });
    });

    action
}

/// Check if arg type is a service provider reference
fn is_service_provider_arg(arg_type: &str) -> bool {
    matches!(arg_type, "party_ref" | "structure_ref")
}

/// Convert macro enum options to state format
#[allow(dead_code)]
pub fn convert_enum_values(values: &[MacroEnumOption]) -> Vec<MacroEnumOption> {
    values.to_vec()
}
