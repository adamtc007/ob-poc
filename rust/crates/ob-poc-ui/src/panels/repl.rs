//! Unified REPL Panel
//!
//! Combines chat, DSL editor, and entity resolution into a single
//! cohesive panel that represents the REPL session workflow:
//!
//! 1. Chat history (scrollable)
//! 2. Verb disambiguation card (when multiple verbs match)
//! 3. Resolution card (inline, when needed)
//! 4. DSL view (collapsible)
//! 5. Input area with actions

use crate::panels::macro_wizard::{macro_wizard_modal, MacroWizardAction};
use crate::state::{AppState, ChatMessage, MessageRole};
use egui::{Color32, RichText, ScrollArea, TextEdit, Ui};
use ob_poc_types::{
    EntityMatchResponse, ResolutionStateResponse, ResolvedRefResponse, ReviewRequirement, RunSheet,
    RunSheetEntry, UnresolvedRefResponse, VerbOption,
};

/// Action returned from verb disambiguation card
#[derive(Debug, Clone)]
pub enum VerbDisambiguationAction {
    /// User selected a verb
    Select { verb_fqn: String },
    /// User cancelled disambiguation
    Cancel,
}

/// Combined action enum for all REPL panel actions
#[derive(Debug, Clone)]
pub enum ReplAction {
    /// Verb disambiguation action
    VerbDisambiguation(VerbDisambiguationAction),
    /// Macro wizard action
    MacroWizard(MacroWizardAction),
}

/// Main REPL panel - combines chat, resolution, and DSL
///
/// Returns `ReplAction` for actions that need to be handled by the app.
pub fn repl_panel(ui: &mut Ui, state: &mut AppState) -> Option<ReplAction> {
    let mut action: Option<ReplAction> = None;

    ui.vertical(|ui| {
        // Calculate available height for content vs input area
        let available_height = ui.available_height();
        let input_height = 80.0;
        let content_height = available_height - input_height;

        // Scrollable content area
        ScrollArea::vertical()
            .max_height(content_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                // Chat history
                render_chat_history(ui, state);

                // Verb disambiguation card (when multiple verbs match user input)
                // This is higher priority than entity resolution - happens earlier in pipeline
                if state.verb_disambiguation_ui.active {
                    ui.add_space(8.0);
                    if let Some(a) = render_verb_disambiguation_card(ui, state) {
                        action = Some(ReplAction::VerbDisambiguation(a));
                    }
                }

                // Macro wizard card (when macro has missing required args)
                // Shows step-by-step wizard to collect missing arguments
                if state.macro_expansion_ui.active {
                    ui.add_space(8.0);
                    let ctx = ui.ctx().clone();
                    let mut search_buffer = state.macro_expansion_ui.current_input.clone();
                    if let Some(a) =
                        macro_wizard_modal(&ctx, &state.macro_expansion_ui, &mut search_buffer)
                    {
                        // Update the search buffer back to state
                        state.macro_expansion_ui.current_input = search_buffer;
                        action = Some(ReplAction::MacroWizard(a));
                    } else {
                        // Even if no action, sync the buffer back (user may have typed)
                        state.macro_expansion_ui.current_input = search_buffer;
                    }
                }

                // Resolution card (inline, when active)
                if should_show_resolution(state) {
                    ui.add_space(8.0);
                    render_resolution_card(ui, state);
                }

                // DSL view (collapsible)
                if has_dsl_content(state) {
                    ui.add_space(8.0);
                    render_dsl_section(ui, state);
                }
            });

        ui.separator();

        // Input area
        render_input_area(ui, state);
    });

    action
}

// =============================================================================
// CHAT HISTORY
// =============================================================================

fn render_chat_history(ui: &mut Ui, state: &AppState) {
    if state.messages.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                RichText::new("Start a conversation with the agent...")
                    .color(Color32::GRAY)
                    .italics(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Try: \"Create a Luxembourg fund called Apex Capital\"")
                    .color(Color32::DARK_GRAY)
                    .small(),
            );
        });
    } else {
        for msg in &state.messages {
            render_message(ui, msg);
            ui.add_space(4.0);
        }
    }
}

fn render_message(ui: &mut Ui, msg: &ChatMessage) {
    let is_user = msg.role == MessageRole::User;
    let bg_color = if is_user {
        Color32::from_rgb(35, 55, 75)
    } else {
        Color32::from_rgb(45, 50, 55)
    };

    egui::Frame::default()
        .fill(bg_color)
        .inner_margin(8.0)
        .rounding(6.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (role_text, role_color) = if is_user {
                    ("You", Color32::from_rgb(100, 180, 255))
                } else {
                    ("Agent", Color32::from_rgb(100, 220, 150))
                };
                ui.label(RichText::new(role_text).strong().color(role_color));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(msg.timestamp.format("%H:%M").to_string())
                            .small()
                            .color(Color32::GRAY),
                    );
                });
            });

            ui.label(&msg.content);
        });
}

// =============================================================================
// VERB DISAMBIGUATION CARD
// =============================================================================

/// Render verb disambiguation card with clickable buttons
///
/// Shown when the agent returns `verb_disambiguation` in a ChatResponse,
/// meaning multiple verbs matched the user's input and they need to pick one.
fn render_verb_disambiguation_card(
    ui: &mut Ui,
    state: &AppState,
) -> Option<VerbDisambiguationAction> {
    let mut action = None;

    let request = state.verb_disambiguation_ui.request.as_ref()?;

    let is_loading = state.verb_disambiguation_ui.loading;

    // Card frame with amber border (needs attention)
    egui::Frame::default()
        .fill(Color32::from_rgb(40, 45, 50))
        .stroke(egui::Stroke::new(2.0, Color32::from_rgb(180, 130, 50)))
        .inner_margin(12.0)
        .rounding(8.0)
        .show(ui, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("?")
                        .size(18.0)
                        .color(Color32::from_rgb(180, 130, 50)),
                );
                ui.label(
                    RichText::new("Which action did you mean?")
                        .strong()
                        .color(Color32::WHITE),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Show original input
                    ui.label(
                        RichText::new(format!("\"{}\"", request.original_input))
                            .small()
                            .color(Color32::LIGHT_GRAY)
                            .italics(),
                    );
                });
            });

            ui.add_space(12.0);

            // Verb option buttons
            if is_loading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Processing selection...").color(Color32::LIGHT_GRAY));
                });
            } else {
                for option in &request.options {
                    if let Some(a) = render_verb_option_button(ui, option) {
                        action = Some(a);
                    }
                    ui.add_space(4.0);
                }
            }

            ui.add_space(8.0);

            // Cancel button
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(!is_loading, egui::Button::new("Cancel"))
                        .on_hover_text("None of these - start fresh")
                        .clicked()
                    {
                        action = Some(VerbDisambiguationAction::Cancel);
                    }

                    // Timeout indicator
                    if let Some(shown_at) = state.verb_disambiguation_ui.shown_at {
                        let current_time = web_sys::window()
                            .and_then(|w| w.performance())
                            .map(|p| p.now() / 1000.0)
                            .unwrap_or(0.0);
                        let elapsed = (current_time - shown_at) as i32;
                        let remaining = 30 - elapsed;
                        if remaining > 0 && remaining <= 10 {
                            ui.label(
                                RichText::new(format!("{}s", remaining))
                                    .small()
                                    .color(Color32::from_rgb(220, 80, 80)),
                            );
                        }
                    }
                });
            });
        });

    action
}

/// Render a single verb option as a clickable button
fn render_verb_option_button(ui: &mut Ui, option: &VerbOption) -> Option<VerbDisambiguationAction> {
    let mut action = None;

    // Score color: green for high, amber for medium
    let score_color = if option.score > 0.8 {
        Color32::from_rgb(100, 200, 100)
    } else if option.score > 0.6 {
        Color32::from_rgb(200, 180, 80)
    } else {
        Color32::from_rgb(180, 180, 180)
    };

    // Button frame
    let response = egui::Frame::default()
        .fill(Color32::from_rgb(50, 55, 65))
        .inner_margin(10.0)
        .rounding(6.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Main content
                ui.vertical(|ui| {
                    // Verb name (e.g., "cbu.create")
                    ui.label(
                        RichText::new(&option.verb_fqn)
                            .strong()
                            .color(Color32::from_rgb(100, 180, 255)),
                    );

                    // Description
                    ui.label(
                        RichText::new(&option.description)
                            .small()
                            .color(Color32::LIGHT_GRAY),
                    );

                    // Example (if different from verb name)
                    if !option.example.is_empty()
                        && option.example != format!("({})", option.verb_fqn)
                    {
                        ui.label(
                            RichText::new(&option.example)
                                .small()
                                .monospace()
                                .color(Color32::DARK_GRAY),
                        );
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Score badge
                    ui.label(
                        RichText::new(format!("{:.0}%", option.score * 100.0))
                            .small()
                            .color(score_color),
                    );
                });
            });
        })
        .response;

    // Make the whole frame clickable
    if response.interact(egui::Sense::click()).clicked() {
        action = Some(VerbDisambiguationAction::Select {
            verb_fqn: option.verb_fqn.clone(),
        });
    }

    // Hover effect
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    action
}

// =============================================================================
// RESOLUTION CARD
// =============================================================================

fn should_show_resolution(state: &AppState) -> bool {
    if let Some(ref resolution) = state.resolution {
        matches!(
            resolution.state,
            ResolutionStateResponse::Resolving | ResolutionStateResponse::Reviewing
        )
    } else {
        false
    }
}

fn render_resolution_card(ui: &mut Ui, state: &mut AppState) {
    let resolution = match &state.resolution {
        Some(r) => r.clone(),
        None => return,
    };

    let header_color = if resolution.unresolved.is_empty() {
        Color32::from_rgb(50, 120, 80) // Green - all resolved
    } else {
        Color32::from_rgb(180, 130, 50) // Amber - needs attention
    };

    egui::Frame::default()
        .fill(Color32::from_rgb(40, 45, 50))
        .stroke(egui::Stroke::new(2.0, header_color))
        .inner_margin(12.0)
        .rounding(8.0)
        .show(ui, |ui| {
            // Header with status
            ui.horizontal(|ui| {
                let icon = if resolution.unresolved.is_empty() {
                    "✓"
                } else {
                    "⚠"
                };
                ui.label(RichText::new(icon).size(18.0).color(header_color));
                ui.label(
                    RichText::new("Entity Resolution")
                        .strong()
                        .color(Color32::WHITE),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Summary stats
                    let summary = &resolution.summary;
                    ui.label(
                        RichText::new(format!(
                            "{}/{} resolved",
                            summary.resolved_count, summary.total_refs
                        ))
                        .small()
                        .color(Color32::LIGHT_GRAY),
                    );
                });
            });

            ui.add_space(8.0);

            // Unresolved refs (need user action)
            if !resolution.unresolved.is_empty() {
                ui.label(
                    RichText::new("Needs Resolution:")
                        .small()
                        .color(Color32::LIGHT_GRAY),
                );
                ui.add_space(4.0);

                for unresolved in &resolution.unresolved {
                    render_unresolved_ref(ui, state, unresolved);
                    ui.add_space(4.0);
                }
            }

            // Resolved refs (for review)
            if !resolution.resolved.is_empty() || !resolution.auto_resolved.is_empty() {
                let all_resolved: Vec<_> = resolution
                    .auto_resolved
                    .iter()
                    .chain(resolution.resolved.iter())
                    .collect();

                if !all_resolved.is_empty() {
                    ui.add_space(4.0);
                    ui.collapsing(
                        RichText::new(format!("Resolved ({})", all_resolved.len()))
                            .small()
                            .color(Color32::LIGHT_GRAY),
                        |ui| {
                            for resolved in all_resolved {
                                render_resolved_ref(ui, resolved);
                            }
                        },
                    );
                }
            }

            ui.add_space(8.0);

            // Action buttons
            ui.horizontal(|ui| {
                let can_commit = resolution.summary.can_commit;

                if ui
                    .add_enabled(can_commit, egui::Button::new("Commit All"))
                    .on_hover_text("Apply resolutions to DSL")
                    .clicked()
                {
                    state.commit_resolution();
                }

                if ui
                    .button("Confirm All")
                    .on_hover_text("Mark all high-confidence resolutions as reviewed")
                    .clicked()
                {
                    state.confirm_all_resolutions();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("Cancel")
                        .on_hover_text("Cancel resolution")
                        .clicked()
                    {
                        state.cancel_resolution();
                    }
                });
            });
        });
}

fn render_unresolved_ref(ui: &mut Ui, state: &mut AppState, unresolved: &UnresolvedRefResponse) {
    let is_selected = state.resolution_ui.selected_ref_id.as_ref() == Some(&unresolved.ref_id);
    let bg_color = if is_selected {
        Color32::from_rgb(50, 60, 75)
    } else {
        Color32::from_rgb(35, 40, 45)
    };

    let requirement_color = match unresolved.review_requirement {
        ReviewRequirement::Required => Color32::from_rgb(220, 80, 80),
        ReviewRequirement::Recommended => Color32::from_rgb(220, 180, 80),
        ReviewRequirement::Optional => Color32::from_rgb(100, 180, 100),
    };

    egui::Frame::default()
        .fill(bg_color)
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            // Header: entity type and search value
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&unresolved.entity_type)
                        .small()
                        .color(Color32::LIGHT_BLUE),
                );
                ui.label(RichText::new("→").small().color(Color32::GRAY));
                ui.label(RichText::new(&unresolved.search_value).strong());

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Requirement indicator
                    let req_text = match unresolved.review_requirement {
                        ReviewRequirement::Required => "Required",
                        ReviewRequirement::Recommended => "Review",
                        ReviewRequirement::Optional => "Auto",
                    };
                    ui.label(RichText::new(req_text).small().color(requirement_color));
                });
            });

            // Context info
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "in {} :{}",
                        unresolved.context.verb, unresolved.context.arg_name
                    ))
                    .small()
                    .color(Color32::GRAY),
                );
            });

            // Expand to show matches when selected
            if is_selected {
                ui.add_space(4.0);
                render_match_selection(ui, state, unresolved);
            } else {
                // Click to select
                if ui
                    .small_button("Select match →")
                    .on_hover_text("Click to choose an entity")
                    .clicked()
                {
                    state.resolution_ui.selected_ref_id = Some(unresolved.ref_id.clone());
                    state.resolution_ui.search_query = unresolved.search_value.clone();
                    state.resolution_ui.search_results = None;
                }
            }
        });
}

fn render_match_selection(ui: &mut Ui, state: &mut AppState, unresolved: &UnresolvedRefResponse) {
    // Rule 3: Extract async state before rendering
    let searching_resolution = state
        .async_state
        .lock()
        .map(|s| s.searching_resolution)
        .unwrap_or(false);

    // Search input
    ui.horizontal(|ui| {
        ui.label("Search:");
        let response = TextEdit::singleline(&mut state.resolution_ui.search_query)
            .desired_width(200.0)
            .hint_text("Type to search...")
            .show(ui);

        if response.response.changed() {
            // Trigger search on typing
            state.search_resolution(&unresolved.ref_id);
        }

        if searching_resolution {
            ui.spinner();
        }
    });

    ui.add_space(4.0);

    // Show matches (from search or initial)
    // Clone to avoid borrow conflicts when calling render_match_option
    let matches: Vec<EntityMatchResponse> =
        if let Some(ref search_results) = state.resolution_ui.search_results {
            search_results.matches.clone()
        } else {
            unresolved.initial_matches.clone()
        };

    let ref_id = unresolved.ref_id.clone();
    let match_count = matches.len();

    if matches.is_empty() {
        ui.label(
            RichText::new("No matches found")
                .small()
                .color(Color32::GRAY),
        );
    } else {
        for entity_match in matches.into_iter().take(5) {
            render_match_option(ui, state, &ref_id, &entity_match);
        }

        if match_count > 5 {
            ui.label(
                RichText::new(format!("... and {} more", match_count - 5))
                    .small()
                    .color(Color32::GRAY),
            );
        }
    }

    // Cancel selection
    ui.add_space(4.0);
    if ui.small_button("Cancel").clicked() {
        state.resolution_ui.selected_ref_id = None;
        state.resolution_ui.search_results = None;
    }
}

fn render_match_option(
    ui: &mut Ui,
    state: &mut AppState,
    ref_id: &str,
    entity_match: &EntityMatchResponse,
) {
    let score_color = if entity_match.score > 0.9 {
        Color32::from_rgb(100, 200, 100)
    } else if entity_match.score > 0.7 {
        Color32::from_rgb(200, 180, 80)
    } else {
        Color32::from_rgb(200, 100, 100)
    };

    egui::Frame::default()
        .fill(Color32::from_rgb(45, 50, 55))
        .inner_margin(6.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Radio-style selection
                if ui.button("Select").clicked() {
                    state.select_resolution(ref_id, &entity_match.id);
                }

                ui.label(RichText::new(&entity_match.display).strong());

                // Discriminators as badges
                for (key, value) in &entity_match.discriminators {
                    ui.label(
                        RichText::new(format!("{}:{}", key, value))
                            .small()
                            .color(Color32::LIGHT_GRAY)
                            .background_color(Color32::from_rgb(60, 65, 70)),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.0}%", entity_match.score * 100.0))
                            .small()
                            .color(score_color),
                    );
                });
            });
        });
}

fn render_resolved_ref(ui: &mut Ui, resolved: &ResolvedRefResponse) {
    ui.horizontal(|ui| {
        let icon = if resolved.reviewed { "✓" } else { "○" };
        let icon_color = if resolved.reviewed {
            Color32::from_rgb(100, 200, 100)
        } else {
            Color32::GRAY
        };

        ui.label(RichText::new(icon).color(icon_color));
        ui.label(
            RichText::new(&resolved.entity_type)
                .small()
                .color(Color32::LIGHT_BLUE),
        );
        ui.label(RichText::new(&resolved.original_search).small());
        ui.label(RichText::new("→").small().color(Color32::GRAY));
        ui.label(RichText::new(&resolved.display).small().strong());

        if !resolved.warnings.is_empty() {
            ui.label(
                RichText::new(format!("⚠ {}", resolved.warnings.len()))
                    .small()
                    .color(Color32::from_rgb(220, 180, 80)),
            );
        }
    });
}

// =============================================================================
// RUN SHEET / DSL SECTION
// =============================================================================

fn has_dsl_content(state: &AppState) -> bool {
    !state.buffers.dsl_editor.trim().is_empty()
        || state.session.as_ref().map(|s| s.has_dsl()).unwrap_or(false)
        || state
            .session
            .as_ref()
            .and_then(|s| s.run_sheet.as_ref())
            .map(|rs| !rs.is_empty())
            .unwrap_or(false)
}

fn render_dsl_section(ui: &mut Ui, state: &mut AppState) {
    // Extract run sheet from session (Rule 3: extract, then render)
    let run_sheet = state.session.as_ref().and_then(|s| s.run_sheet.clone());

    // Extract bindings
    let bindings = state
        .session
        .as_ref()
        .map(|s| s.bindings.clone())
        .unwrap_or_default();

    let has_run_sheet = run_sheet.as_ref().map(|rs| !rs.is_empty()).unwrap_or(false);

    // Header with status summary
    let header_text = if let Some(ref rs) = run_sheet {
        let executed = rs.executed_count();
        let pending = rs.pending_count();
        if executed > 0 || pending > 0 {
            format!("Run Sheet ({} executed, {} pending)", executed, pending)
        } else {
            "Run Sheet".to_string()
        }
    } else {
        "DSL".to_string()
    };

    let header = egui::CollapsingHeader::new(
        RichText::new(header_text)
            .strong()
            .color(Color32::from_rgb(200, 180, 120)),
    )
    .default_open(true);

    header.show(ui, |ui| {
        // Run sheet entries (if any)
        if has_run_sheet {
            if let Some(ref rs) = run_sheet {
                render_run_sheet(ui, rs);
            }
            ui.add_space(8.0);
        }

        // Symbol bindings (if any)
        if !bindings.is_empty() {
            render_bindings(ui, &bindings);
            ui.add_space(8.0);
        }

        // DSL editor (for new/draft DSL)
        egui::Frame::default()
            .fill(Color32::from_rgb(30, 32, 35))
            .inner_margin(8.0)
            .rounding(4.0)
            .show(ui, |ui| {
                ui.label(RichText::new("Draft DSL").small().color(Color32::GRAY));
                ui.add_space(4.0);

                // DSL editor
                let response = TextEdit::multiline(&mut state.buffers.dsl_editor)
                    .font(egui::TextStyle::Monospace)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(4)
                    .show(ui);

                if response.response.changed() {
                    state.buffers.dsl_dirty = true;
                }

                // Validation errors
                if let Some(ref validation) = state.validation_result {
                    if !validation.errors.is_empty() {
                        ui.add_space(4.0);
                        for error in &validation.errors {
                            let error_text = if let Some(line) = error.line {
                                format!("✗ L{}: {}", line, error.message)
                            } else {
                                format!("✗ {}", error.message)
                            };
                            ui.label(
                                RichText::new(error_text)
                                    .small()
                                    .color(Color32::from_rgb(220, 80, 80)),
                            );
                        }
                    }
                }
            });
    });
}

/// Render the run sheet entries with per-statement status
fn render_run_sheet(ui: &mut Ui, run_sheet: &RunSheet) {
    for (idx, entry) in run_sheet.entries.iter().enumerate() {
        let is_current = idx == run_sheet.cursor;
        render_run_sheet_entry(ui, entry, is_current);
        ui.add_space(4.0);
    }
}

/// Render a single run sheet entry
fn render_run_sheet_entry(ui: &mut Ui, entry: &RunSheetEntry, is_current: bool) {
    let (r, g, b) = entry.status.color_rgb();
    let status_color = Color32::from_rgb(r, g, b);

    let bg_color = if is_current {
        Color32::from_rgb(40, 50, 60) // Highlighted
    } else {
        Color32::from_rgb(30, 32, 35)
    };

    let border_color = if is_current {
        Color32::from_rgb(80, 120, 180)
    } else {
        Color32::TRANSPARENT
    };

    egui::Frame::default()
        .fill(bg_color)
        .stroke(egui::Stroke::new(1.0, border_color))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            // Header: status icon + status text + timestamp
            ui.horizontal(|ui| {
                ui.label(RichText::new(entry.status.icon()).color(status_color));
                ui.label(
                    RichText::new(format!("{:?}", entry.status))
                        .small()
                        .color(status_color),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(ref ts) = entry.executed_at {
                        ui.label(RichText::new(ts).small().color(Color32::DARK_GRAY));
                    } else if let Some(ref ts) = entry.created_at {
                        ui.label(RichText::new(ts).small().color(Color32::DARK_GRAY));
                    }
                });
            });

            // DSL source (truncated for display)
            let display_dsl = entry.display_dsl.as_deref().unwrap_or(&entry.dsl_source);
            let truncated = if display_dsl.len() > 120 {
                format!("{}...", &display_dsl[..120])
            } else {
                display_dsl.to_string()
            };

            ui.add_space(4.0);
            ui.label(
                RichText::new(truncated)
                    .monospace()
                    .size(11.0)
                    .color(Color32::LIGHT_GRAY),
            );

            // Bindings created by this entry
            if !entry.bindings.is_empty() {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("→").small().color(Color32::GRAY));
                    for (symbol, info) in &entry.bindings {
                        ui.label(
                            RichText::new(format!("@{}", symbol))
                                .small()
                                .color(Color32::from_rgb(180, 140, 255))
                                .background_color(Color32::from_rgb(40, 35, 50)),
                        );
                        ui.label(
                            RichText::new(format!("= {}", info.name))
                                .small()
                                .color(Color32::LIGHT_GRAY),
                        );
                    }
                });
            }

            // Affected entities count
            if !entry.affected_entities.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "Affected: {} entities",
                            entry.affected_entities.len()
                        ))
                        .small()
                        .color(Color32::DARK_GRAY),
                    );
                });
            }

            // Error message (if failed)
            if let Some(ref error) = entry.error {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("✗ {}", error))
                        .small()
                        .color(Color32::from_rgb(220, 80, 80)),
                );
            }
        });
}

/// Render symbol bindings
fn render_bindings(
    ui: &mut Ui,
    bindings: &std::collections::HashMap<String, ob_poc_types::BoundEntityInfo>,
) {
    if bindings.is_empty() {
        return;
    }

    egui::Frame::default()
        .fill(Color32::from_rgb(35, 30, 45))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new("Symbol Bindings")
                    .small()
                    .strong()
                    .color(Color32::from_rgb(180, 140, 255)),
            );
            ui.add_space(4.0);

            for (symbol, info) in bindings {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("@{}", symbol))
                            .monospace()
                            .color(Color32::from_rgb(180, 140, 255)),
                    );
                    ui.label(RichText::new("→").small().color(Color32::GRAY));
                    ui.label(RichText::new(&info.name).small().color(Color32::LIGHT_GRAY));
                    ui.label(
                        RichText::new(format!("({})", info.entity_type))
                            .small()
                            .color(Color32::DARK_GRAY),
                    );
                });
            }
        });
}

// =============================================================================
// INPUT AREA
// =============================================================================

fn render_input_area(ui: &mut Ui, state: &mut AppState) {
    let chat_input_id = egui::Id::new("repl_chat_input");

    // Rule 3: Single lock, extract all needed data, then render
    let (should_focus, is_loading) = {
        let mut guard = match state.async_state.lock() {
            Ok(g) => g,
            Err(_) => return, // Poisoned lock, skip rendering
        };
        let focus = !guard.loading_chat && guard.chat_just_finished;
        let loading = guard.loading_chat || guard.executing;
        if focus {
            guard.chat_just_finished = false;
        }
        (focus, loading)
    };
    // Lock released here

    // Now render using extracted data
    if should_focus {
        ui.memory_mut(|mem| mem.request_focus(chat_input_id));
    }

    ui.horizontal(|ui| {
        // Chat input
        let response = TextEdit::singleline(&mut state.buffers.chat_input)
            .desired_width(ui.available_width() - 180.0)
            .hint_text("Ask the agent or type DSL commands...")
            .id(chat_input_id)
            .show(ui);

        let enter_pressed =
            response.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        let can_send = !state.buffers.chat_input.trim().is_empty() && !is_loading;
        let can_execute = state
            .session
            .as_ref()
            .map(|s| s.can_execute)
            .unwrap_or(false)
            && !is_loading
            && has_dsl_content(state);

        // Send button
        if ui
            .add_enabled(can_send, egui::Button::new("Send"))
            .clicked()
            || (enter_pressed && can_send)
        {
            state.send_chat_message();
        }

        // Execute button
        if ui
            .add_enabled(can_execute, egui::Button::new("▶ Execute"))
            .on_hover_text("Execute the DSL")
            .clicked()
        {
            state.execute_dsl();
        }

        // Loading indicator
        if is_loading {
            ui.spinner();
        }
    });

    // Status line
    ui.horizontal(|ui| {
        if let Some(ref session) = state.session {
            ui.label(
                RichText::new(format!("Session: {}", &session.session_id[..8]))
                    .small()
                    .color(Color32::DARK_GRAY),
            );

            if let Some(cbu_name) = session.active_cbu_name() {
                ui.label(
                    RichText::new(format!("| CBU: {}", cbu_name))
                        .small()
                        .color(Color32::DARK_GRAY),
                );
            }
        }

        // Show current scope
        if let Some(ref scope) = state.current_scope {
            let scope_icon = match scope.scope_type.as_str() {
                "galaxy" => "🌌",
                "book" => "📖",
                "cbu" => "🏢",
                "jurisdiction" => "🌍",
                "neighborhood" => "🔗",
                _ => "📍",
            };
            let scope_color = if scope.is_loaded {
                Color32::from_rgb(100, 180, 255) // Blue when loaded
            } else {
                Color32::from_rgb(180, 130, 50) // Amber when loading
            };
            let scope_display = if scope.scope_path.is_empty() {
                scope.scope_type.clone()
            } else {
                format!("{}: {}", scope.scope_type, scope.scope_path)
            };
            ui.label(
                RichText::new(format!("| {} {}", scope_icon, scope_display))
                    .small()
                    .color(scope_color),
            );
        }

        // Show resolution status
        if let Some(ref resolution) = state.resolution {
            let status_text = match resolution.state {
                ResolutionStateResponse::Resolving => "Resolving...",
                ResolutionStateResponse::Reviewing => "Ready to commit",
                ResolutionStateResponse::Committed => "Committed",
                ResolutionStateResponse::Cancelled => "Cancelled",
            };
            ui.label(
                RichText::new(format!("| {}", status_text))
                    .small()
                    .color(Color32::from_rgb(180, 130, 50)),
            );
        }
    });
}
