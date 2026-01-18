//! Entity Resolution Panel
//!
//! A modal panel for resolving ambiguous entity references in DSL.
//! Renders entity-type-specific search fields based on config from EntityGateway.
//!
//! Follows EGUI-RULES.md:
//! - Panel returns Option<ResolutionPanelAction>, no callbacks
//! - UI-only state passed in via ResolutionPanelData
//! - Server data (matches) passed in, never mutated
//! - Single action pipeline - no parallel channels

use crate::state::{ResolutionPanelUi, WindowData, WindowEntry};
use egui::{Align2, Color32, RichText, TextEdit, Ui, Vec2};
use ob_poc_types::resolution::{
    DiscriminatorField, DiscriminatorFieldType, ResolutionModeHint, SearchKeyField,
    SearchKeyFieldType, SearchSuggestions, SuggestedActionType,
};
use std::collections::HashMap;

// =============================================================================
// ACTIONS (returned to app.rs for handling)
// =============================================================================

/// Actions that can be returned from the resolution panel
#[derive(Clone, Debug)]
pub enum ResolutionPanelAction {
    /// Multi-key search triggered (debounced)
    SearchMultiKey {
        search_key_values: HashMap<String, String>,
        discriminators: HashMap<String, String>,
    },
    /// User selected a match
    Select { index: usize, entity_id: String },
    /// User selected from fallback matches (found elsewhere)
    SelectFallback { index: usize, entity_id: String },
    /// User wants to skip this ref
    Skip,
    /// User wants to create a new entity
    CreateNew,
    /// User completed resolution (apply=true) or cancelled (apply=false)
    Complete { apply: bool },
    /// User closed the modal
    Close,
    /// User typed in the chat input
    SendMessage { message: String },
    /// Toggle voice input
    ToggleVoice,
    /// Clear a specific filter
    ClearFilter { key: String },
    /// Clear all filters (keep only name)
    ClearAllFilters,
}

// =============================================================================
// DISPLAY TYPES
// =============================================================================

/// Match result from entity search (display-ready)
#[derive(Clone, Debug)]
pub struct EntityMatchDisplay {
    /// Entity UUID or CODE
    pub id: String,
    /// Display name
    pub name: String,
    /// Match score (0.0 - 1.0)
    pub score: f32,
    /// Additional details (nationality, DOB, role, etc.)
    pub details: Option<String>,
    /// Entity type
    pub entity_type: Option<String>,
}

// =============================================================================
// PANEL DATA (extracted before render, read-only)
// =============================================================================

/// Data needed to render the resolution panel (extracted before render)
pub struct ResolutionPanelData<'a> {
    /// Window entry from the stack
    pub window: Option<&'a WindowEntry>,
    /// Search results from server
    pub matches: Option<&'a [EntityMatchDisplay]>,
    /// Whether search is in progress
    pub searching: bool,
    /// Current ref name being resolved
    pub current_ref_name: Option<String>,
    /// DSL context around the unresolved ref
    pub dsl_context: Option<String>,
    /// Chat messages in the sub-session
    pub messages: Vec<(String, String)>,
    /// Whether voice input is active
    pub voice_active: bool,

    // Entity-specific config
    /// Entity type being resolved
    pub entity_type: Option<&'a str>,
    /// Search key fields from entity config
    pub search_keys: &'a [SearchKeyField],
    /// Discriminator fields from entity config
    pub discriminator_fields: &'a [DiscriminatorField],
    /// Resolution mode (SearchModal vs Autocomplete)
    pub resolution_mode: ResolutionModeHint,

    // Fallback/suggestions from search response
    /// Fallback matches (found elsewhere when filters applied)
    pub fallback_matches: Option<&'a [EntityMatchDisplay]>,
    /// Which filters narrowed to zero
    pub filtered_by: Option<&'a HashMap<String, String>>,
    /// Suggestions for no-result scenarios
    pub suggestions: Option<&'a SearchSuggestions>,
}

// =============================================================================
// MAIN ENTRY POINT
// =============================================================================

/// Render the resolution modal panel
///
/// Returns an action if the user interacted, None otherwise.
pub fn resolution_modal(
    ctx: &egui::Context,
    ui_state: &mut ResolutionPanelUi,
    data: &ResolutionPanelData<'_>,
) -> Option<ResolutionPanelAction> {
    let window = data.window?;

    // Extract window data
    let (subsession_id, current_ref_index, total_refs) = match &window.data {
        Some(WindowData::Resolution {
            subsession_id,
            current_ref_index,
            total_refs,
            ..
        }) => (subsession_id.clone(), *current_ref_index, *total_refs),
        _ => return None,
    };

    let mut action: Option<ResolutionPanelAction> = None;

    egui::Window::new("Entity Resolution")
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(520.0, 500.0))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            action = render_resolution_content(
                ui,
                ui_state,
                data,
                &subsession_id,
                current_ref_index,
                total_refs,
            );
        });

    action
}

// =============================================================================
// MAIN CONTENT RENDERER
// =============================================================================

fn render_resolution_content(
    ui: &mut Ui,
    ui_state: &mut ResolutionPanelUi,
    data: &ResolutionPanelData<'_>,
    _subsession_id: &str,
    current_ref_index: usize,
    total_refs: usize,
) -> Option<ResolutionPanelAction> {
    let mut action: Option<ResolutionPanelAction> = None;

    // Header with progress and entity type badge
    render_header(ui, data, current_ref_index, total_refs, &mut action);

    ui.separator();
    ui.add_space(4.0);

    // DSL Context (if available)
    render_dsl_context(ui, data);

    // Current ref being resolved
    render_current_ref_label(ui, data);

    // === SWITCH ON RESOLUTION MODE ===
    match data.resolution_mode {
        ResolutionModeHint::Autocomplete => {
            // Simple dropdown for reference data (jurisdiction, role, etc.)
            if let Some(a) = render_autocomplete_mode(ui, data, ui_state) {
                action = Some(a);
            }
        }
        ResolutionModeHint::SearchModal => {
            // Full search modal with multiple fields
            if let Some(a) = render_search_modal_mode(ui, data, ui_state) {
                action = Some(a);
            }
        }
    }

    ui.add_space(8.0);
    ui.separator();

    // Chat area (agent-driven conversation)
    render_chat_area(ui, ui_state, data, &mut action);

    ui.add_space(8.0);
    ui.separator();

    // Footer buttons
    render_footer_buttons(ui, current_ref_index, total_refs, &mut action);

    action
}

// =============================================================================
// HEADER
// =============================================================================

fn render_header(
    ui: &mut Ui,
    data: &ResolutionPanelData<'_>,
    current_ref_index: usize,
    total_refs: usize,
    action: &mut Option<ResolutionPanelAction>,
) {
    ui.horizontal(|ui| {
        ui.heading("Resolve Entity Reference");

        // Entity type badge
        if let Some(entity_type) = data.entity_type {
            ui.add_space(8.0);
            egui::Frame::default()
                .fill(Color32::from_rgb(60, 80, 100))
                .inner_margin(egui::vec2(6.0, 2.0))
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(entity_type.to_uppercase())
                            .small()
                            .color(Color32::WHITE),
                    );
                });
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("X").clicked() {
                *action = Some(ResolutionPanelAction::Close);
            }
            ui.label(
                RichText::new(format!("{} of {}", current_ref_index + 1, total_refs))
                    .small()
                    .color(Color32::LIGHT_GRAY),
            );
        });
    });
}

// =============================================================================
// DSL CONTEXT
// =============================================================================

fn render_dsl_context(ui: &mut Ui, data: &ResolutionPanelData<'_>) {
    if let Some(ref context) = data.dsl_context {
        ui.group(|ui| {
            ui.label(RichText::new("DSL Context:").small().color(Color32::GRAY));
            ui.add_space(2.0);
            egui::Frame::default()
                .fill(Color32::from_rgb(35, 40, 45))
                .inner_margin(8.0)
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.monospace(context);
                });
        });
        ui.add_space(4.0);
    }
}

// =============================================================================
// CURRENT REF LABEL
// =============================================================================

fn render_current_ref_label(ui: &mut Ui, data: &ResolutionPanelData<'_>) {
    if let Some(ref name) = data.current_ref_name {
        ui.horizontal(|ui| {
            ui.label("Resolving:");
            ui.label(RichText::new(name).strong().color(Color32::YELLOW));
        });
        ui.add_space(4.0);
    }
}

// =============================================================================
// AUTOCOMPLETE MODE (for reference data: jurisdiction, role, currency, etc.)
// =============================================================================

fn render_autocomplete_mode(
    ui: &mut Ui,
    data: &ResolutionPanelData<'_>,
    ui_state: &mut ResolutionPanelUi,
) -> Option<ResolutionPanelAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        ui.label("Select:");

        let current_value = ui_state
            .search_key_values
            .get("name")
            .cloned()
            .unwrap_or_default();

        egui::ComboBox::from_id_salt("ref_autocomplete")
            .selected_text(if current_value.is_empty() {
                "Select..."
            } else {
                &current_value
            })
            .width(300.0)
            .show_ui(ui, |ui| {
                if let Some(matches) = data.matches {
                    for (idx, m) in matches.iter().enumerate() {
                        let label = if m.id != m.name {
                            format!("{} ({})", m.name, m.id)
                        } else {
                            m.name.clone()
                        };
                        if ui.selectable_label(current_value == m.id, &label).clicked() {
                            action = Some(ResolutionPanelAction::Select {
                                index: idx,
                                entity_id: m.id.clone(),
                            });
                        }
                    }
                }
            });

        if data.searching {
            ui.spinner();
        }
    });

    action
}

// =============================================================================
// SEARCH MODAL MODE (for entities: cbu, person, legal_entity, etc.)
// =============================================================================

fn render_search_modal_mode(
    ui: &mut Ui,
    data: &ResolutionPanelData<'_>,
    ui_state: &mut ResolutionPanelUi,
) -> Option<ResolutionPanelAction> {
    let mut action: Option<ResolutionPanelAction> = None;
    let mut search_changed = false;

    // 1. Render search keys (multi-field)
    if !data.search_keys.is_empty() {
        ui.label(RichText::new("Search:").small().color(Color32::GRAY));
        if render_search_keys(ui, data.search_keys, &mut ui_state.search_key_values) {
            search_changed = true;
        }
    } else {
        // Fallback: single text field if no search keys configured
        ui.horizontal(|ui| {
            ui.label("Search:");
            let response = TextEdit::singleline(&mut ui_state.search_query)
                .desired_width(280.0)
                .hint_text("Type to search...")
                .show(ui);
            if response.response.changed() {
                ui_state
                    .search_key_values
                    .insert("name".to_string(), ui_state.search_query.clone());
                search_changed = true;
            }
        });
    }

    // Voice input button
    ui.horizontal(|ui| {
        let mic_label = if data.voice_active { "🎤" } else { "🎙" };
        let mic_color = if data.voice_active {
            Color32::from_rgb(255, 100, 100)
        } else {
            Color32::LIGHT_GRAY
        };
        if ui
            .button(RichText::new(mic_label).color(mic_color))
            .on_hover_text(if data.voice_active {
                "Stop listening"
            } else {
                "Start voice input"
            })
            .clicked()
        {
            action = Some(ResolutionPanelAction::ToggleVoice);
        }

        if data.searching {
            ui.spinner();
            ui.label(RichText::new("Searching...").small().color(Color32::GRAY));
        }

        if data.voice_active {
            ui.label(RichText::new("Listening...").small().color(Color32::RED));
        }
    });

    // 2. Render discriminators (if any)
    if !data.discriminator_fields.is_empty()
        && render_discriminators(
            ui,
            data.discriminator_fields,
            &mut ui_state.discriminator_values,
        )
    {
        search_changed = true;
    }

    // 3. Trigger search on change (will be debounced in app.rs)
    if search_changed {
        action = Some(ResolutionPanelAction::SearchMultiKey {
            search_key_values: ui_state.search_key_values.clone(),
            discriminators: ui_state.discriminator_values.clone(),
        });
    }

    ui.add_space(4.0);
    ui.separator();

    // 4. Results area
    ui.label(RichText::new("Matches:").small().color(Color32::GRAY));

    egui::ScrollArea::vertical()
        .max_height(160.0)
        .show(ui, |ui| {
            if let Some(matches) = data.matches {
                if matches.is_empty() {
                    // No direct matches - check for suggestions/fallback
                    render_no_results(ui, data, &mut action);
                } else {
                    // Show matches
                    for (idx, m) in matches.iter().enumerate() {
                        if let Some(select_action) = render_match_row(ui, idx, m, false) {
                            action = Some(select_action);
                        }
                    }
                }
            } else {
                ui.label(
                    RichText::new("Enter search criteria above")
                        .color(Color32::GRAY)
                        .italics(),
                );
            }
        });

    action
}

// =============================================================================
// SEARCH KEYS RENDERER
// =============================================================================

/// Render search key fields based on entity config
/// Returns true if any field changed
fn render_search_keys(
    ui: &mut Ui,
    search_keys: &[SearchKeyField],
    values: &mut HashMap<String, String>,
) -> bool {
    let mut changed = false;

    ui.horizontal_wrapped(|ui| {
        for key in search_keys {
            ui.vertical(|ui| {
                ui.label(RichText::new(&key.label).small());

                match key.field_type {
                    SearchKeyFieldType::Text => {
                        let width = if key.is_default { 160.0 } else { 100.0 };
                        let response =
                            TextEdit::singleline(values.entry(key.name.clone()).or_default())
                                .hint_text(if key.is_default { "Search..." } else { "" })
                                .desired_width(width)
                                .show(ui);
                        changed |= response.response.changed();
                    }
                    SearchKeyFieldType::Enum => {
                        let current = values.get(&key.name).cloned().unwrap_or_default();
                        let display_text = if current.is_empty() {
                            "Any".to_string()
                        } else if let Some(enum_values) = &key.enum_values {
                            enum_values
                                .iter()
                                .find(|ev| ev.code == current)
                                .map(|ev| ev.display.clone())
                                .unwrap_or(current.clone())
                        } else {
                            current.clone()
                        };

                        egui::ComboBox::from_id_salt(&key.name)
                            .selected_text(&display_text)
                            .width(120.0)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(current.is_empty(), "Any").clicked() {
                                    values.remove(&key.name);
                                    changed = true;
                                }
                                if let Some(enum_values) = &key.enum_values {
                                    for ev in enum_values {
                                        let label = format!("{} ({})", ev.display, ev.code);
                                        if ui.selectable_label(current == ev.code, &label).clicked()
                                        {
                                            values.insert(key.name.clone(), ev.code.clone());
                                            changed = true;
                                        }
                                    }
                                }
                            });
                    }
                    SearchKeyFieldType::Uuid => {
                        let response =
                            TextEdit::singleline(values.entry(key.name.clone()).or_default())
                                .hint_text("UUID")
                                .desired_width(250.0)
                                .show(ui);
                        changed |= response.response.changed();
                    }
                }
            });
            ui.add_space(8.0);
        }
    });

    changed
}

// =============================================================================
// DISCRIMINATORS RENDERER
// =============================================================================

/// Render discriminator fields for scoring refinement
/// Returns true if any field changed
fn render_discriminators(
    ui: &mut Ui,
    fields: &[DiscriminatorField],
    values: &mut HashMap<String, String>,
) -> bool {
    let mut changed = false;

    ui.add_space(4.0);
    egui::CollapsingHeader::new(RichText::new("Refinement (optional)").small())
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for field in fields {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&field.label).small().color(Color32::GRAY));

                        match field.field_type {
                            DiscriminatorFieldType::Enum => {
                                let current = values.get(&field.name).cloned().unwrap_or_default();
                                let display_text = if current.is_empty() {
                                    "—".to_string()
                                } else if let Some(enum_values) = &field.enum_values {
                                    enum_values
                                        .iter()
                                        .find(|ev| ev.code == current)
                                        .map(|ev| ev.display.clone())
                                        .unwrap_or(current.clone())
                                } else {
                                    current.clone()
                                };

                                egui::ComboBox::from_id_salt(format!("disc_{}", field.name))
                                    .selected_text(&display_text)
                                    .width(100.0)
                                    .show_ui(ui, |ui| {
                                        if ui.selectable_label(current.is_empty(), "—").clicked()
                                        {
                                            values.remove(&field.name);
                                            changed = true;
                                        }
                                        if let Some(enum_values) = &field.enum_values {
                                            for ev in enum_values {
                                                if ui
                                                    .selectable_label(
                                                        current == ev.code,
                                                        &ev.display,
                                                    )
                                                    .clicked()
                                                {
                                                    values.insert(
                                                        field.name.clone(),
                                                        ev.code.clone(),
                                                    );
                                                    changed = true;
                                                }
                                            }
                                        }
                                    });
                            }
                            DiscriminatorFieldType::Date => {
                                let response = TextEdit::singleline(
                                    values.entry(field.name.clone()).or_default(),
                                )
                                .hint_text("YYYY or YYYY-MM-DD")
                                .desired_width(110.0)
                                .show(ui);
                                changed |= response.response.changed();
                            }
                            DiscriminatorFieldType::String => {
                                let response = TextEdit::singleline(
                                    values.entry(field.name.clone()).or_default(),
                                )
                                .desired_width(80.0)
                                .show(ui);
                                changed |= response.response.changed();
                            }
                        }
                    });
                    ui.add_space(8.0);
                }
            });
        });

    changed
}

// =============================================================================
// NO RESULTS / SUGGESTIONS / FALLBACK
// =============================================================================

fn render_no_results(
    ui: &mut Ui,
    data: &ResolutionPanelData<'_>,
    action: &mut Option<ResolutionPanelAction>,
) {
    // Check for suggestions
    if let Some(suggestions) = data.suggestions {
        render_suggestions(ui, suggestions, action);
    } else {
        ui.label(
            RichText::new("No matches found")
                .color(Color32::GRAY)
                .italics(),
        );
    }

    // Check for fallback matches
    if let (Some(fallback), Some(filtered_by)) = (data.fallback_matches, data.filtered_by) {
        if !fallback.is_empty() {
            render_fallback_matches(ui, fallback, filtered_by, action);
        }
    }
}

fn render_suggestions(
    ui: &mut Ui,
    suggestions: &SearchSuggestions,
    action: &mut Option<ResolutionPanelAction>,
) {
    ui.add_space(4.0);
    egui::Frame::default()
        .fill(Color32::from_rgb(50, 45, 40))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("💡").small());
                ui.label(&suggestions.message);
            });

            if !suggestions.actions.is_empty() {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for suggested in &suggestions.actions {
                        match &suggested.action {
                            SuggestedActionType::ClearFilters => {
                                if ui.small_button(&suggested.label).clicked() {
                                    *action = Some(ResolutionPanelAction::ClearAllFilters);
                                }
                            }
                            SuggestedActionType::ClearFilter { key } => {
                                if ui.small_button(&suggested.label).clicked() {
                                    *action = Some(ResolutionPanelAction::ClearFilter {
                                        key: key.clone(),
                                    });
                                }
                            }
                            SuggestedActionType::CreateNew => {
                                if ui.small_button(&suggested.label).clicked() {
                                    *action = Some(ResolutionPanelAction::CreateNew);
                                }
                            }
                            SuggestedActionType::SimplifyQuery => {
                                // SimplifyQuery is informational - no action yet
                                let _ = ui.small_button(&suggested.label);
                            }
                        }
                    }
                });
            }
        });
}

fn render_fallback_matches(
    ui: &mut Ui,
    matches: &[EntityMatchDisplay],
    filtered_by: &HashMap<String, String>,
    action: &mut Option<ResolutionPanelAction>,
) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Found elsewhere:")
                .small()
                .color(Color32::YELLOW),
        );

        // Show what filters were applied
        let filter_text: Vec<_> = filtered_by
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        if !filter_text.is_empty() {
            ui.label(
                RichText::new(format!("(not in {})", filter_text.join(", ")))
                    .small()
                    .color(Color32::GRAY),
            );
        }
    });

    for (idx, m) in matches.iter().enumerate() {
        if let Some(select_action) = render_match_row(ui, idx, m, true) {
            *action = Some(select_action);
        }
    }
}

// =============================================================================
// MATCH ROW RENDERER
// =============================================================================

fn render_match_row(
    ui: &mut Ui,
    index: usize,
    m: &EntityMatchDisplay,
    is_fallback: bool,
) -> Option<ResolutionPanelAction> {
    let mut action: Option<ResolutionPanelAction> = None;

    let score_color = if m.score > 0.9 {
        Color32::from_rgb(100, 200, 100)
    } else if m.score > 0.7 {
        Color32::from_rgb(200, 180, 80)
    } else {
        Color32::from_rgb(180, 140, 100)
    };

    let bg_color = if is_fallback {
        Color32::from_rgb(55, 50, 45)
    } else {
        Color32::from_rgb(45, 50, 55)
    };

    egui::Frame::default()
        .fill(bg_color)
        .inner_margin(6.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Selection number
                ui.label(
                    RichText::new(format!("{}.", index + 1))
                        .small()
                        .color(Color32::GRAY),
                );

                // Select button
                if ui.small_button("Select").clicked() {
                    action = if is_fallback {
                        Some(ResolutionPanelAction::SelectFallback {
                            index,
                            entity_id: m.id.clone(),
                        })
                    } else {
                        Some(ResolutionPanelAction::Select {
                            index,
                            entity_id: m.id.clone(),
                        })
                    };
                }

                // Name
                ui.label(RichText::new(&m.name).strong());

                // Entity type
                if let Some(ref etype) = m.entity_type {
                    ui.label(
                        RichText::new(format!("[{}]", etype))
                            .small()
                            .color(Color32::LIGHT_BLUE),
                    );
                }

                // Details
                if let Some(ref details) = m.details {
                    ui.label(RichText::new(details).small().color(Color32::LIGHT_GRAY));
                }

                // Score on right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.0}%", m.score * 100.0))
                            .small()
                            .color(score_color),
                    );
                });
            });
        });

    ui.add_space(2.0);
    action
}

// =============================================================================
// CHAT AREA
// =============================================================================

fn render_chat_area(
    ui: &mut Ui,
    ui_state: &mut ResolutionPanelUi,
    data: &ResolutionPanelData<'_>,
    action: &mut Option<ResolutionPanelAction>,
) {
    ui.label(RichText::new("Conversation:").small().color(Color32::GRAY));

    egui::ScrollArea::vertical()
        .max_height(80.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for (role, content) in &data.messages {
                let color = if role == "user" {
                    Color32::from_rgb(100, 150, 255)
                } else {
                    Color32::from_rgb(150, 200, 150)
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}:", role)).small().color(color));
                    ui.label(content);
                });
            }
        });

    // Chat input
    ui.horizontal(|ui| {
        let response = TextEdit::singleline(&mut ui_state.chat_buffer)
            .desired_width(380.0)
            .hint_text("Type refinement (e.g., 'UK citizen, born 1965')...")
            .show(ui);

        let enter_pressed = response.response.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
            && !ui_state.chat_buffer.is_empty();

        let send_clicked = ui.button("Send").clicked() && !ui_state.chat_buffer.is_empty();

        if enter_pressed || send_clicked {
            *action = Some(ResolutionPanelAction::SendMessage {
                message: ui_state.chat_buffer.clone(),
            });
            ui_state.chat_buffer.clear();
        }
    });
}

// =============================================================================
// FOOTER BUTTONS
// =============================================================================

fn render_footer_buttons(
    ui: &mut Ui,
    current_ref_index: usize,
    total_refs: usize,
    action: &mut Option<ResolutionPanelAction>,
) {
    ui.horizontal(|ui| {
        if ui.button("+ Create New").clicked() {
            *action = Some(ResolutionPanelAction::CreateNew);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Cancel").clicked() {
                *action = Some(ResolutionPanelAction::Complete { apply: false });
            }
            if ui.button("Skip").clicked() {
                *action = Some(ResolutionPanelAction::Skip);
            }
            if current_ref_index + 1 >= total_refs
                && ui
                    .button(RichText::new("Complete").color(Color32::GREEN))
                    .clicked()
            {
                *action = Some(ResolutionPanelAction::Complete { apply: true });
            }
        });
    });
}
