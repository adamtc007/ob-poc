//! Disambiguation Modal Panel
//!
//! Shows entity matches from agent chat and lets user pick one.
//! Simpler than full resolution - just pick from pre-searched results.
//!
//! Features:
//! - Keyboard shortcuts (1-9 to select, Escape to cancel, Enter to search)
//! - Auto-search after typing pause (300ms debounce)
//! - Create New Entity option
//!
//! Follows EGUI-RULES:
//! - Returns Option<DisambiguationAction>, no callbacks
//! - Data passed in, not mutated

use crate::state::{WindowData, WindowEntry};
use egui::{Align2, Color32, Key, RichText, ScrollArea, TextEdit, Ui, Vec2};
use ob_poc_types::{DisambiguationItem, EntityMatch};

/// Actions from disambiguation modal
#[derive(Clone, Debug)]
pub enum DisambiguationAction {
    /// User selected a match
    Select {
        entity_id: String,
        entity_type: String,
        display_name: String,
    },
    /// User wants to search with different text
    Search { query: String, entity_type: String },
    /// User wants to skip this item
    Skip,
    /// User cancelled entire disambiguation
    Cancel,
    /// User closed modal
    Close,
    /// User wants to create a new entity with the given name
    CreateNew { name: String, entity_type: String },
}

/// Data needed to render disambiguation modal
pub struct DisambiguationModalData<'a> {
    pub window: Option<&'a WindowEntry>,
    pub searching: bool,
    /// Last time the search buffer was modified (for debounce)
    pub last_search_change: Option<std::time::Instant>,
}

/// Debounce delay for auto-search (milliseconds)
const AUTO_SEARCH_DELAY_MS: u64 = 400;

/// Render disambiguation modal
/// Returns action if user interacted
pub fn disambiguation_modal(
    ctx: &egui::Context,
    search_buffer: &mut String,
    data: &DisambiguationModalData<'_>,
) -> Option<DisambiguationAction> {
    let window = data.window?;

    // Extract disambiguation data from window
    let (request, current_item_index, search_results) = match &window.data {
        Some(WindowData::Disambiguation {
            request,
            current_item_index,
            search_results,
        }) => (request, *current_item_index, search_results.as_ref()),
        _ => return None,
    };

    let mut action: Option<DisambiguationAction> = None;
    let total_items = request.items.len();

    // Get current item
    let current_item = request.items.get(current_item_index)?;

    // Extract search text and entity type from the item
    let (search_text, entity_type, matches) = match current_item {
        DisambiguationItem::EntityMatch {
            search_text,
            matches,
            ..
        } => (search_text.as_str(), "entity", matches.as_slice()),
        DisambiguationItem::InterpretationChoice { text, .. } => {
            (text.as_str(), "interpretation", &[][..])
        }
        DisambiguationItem::ClientGroupMatch { search_text, .. } => {
            (search_text.as_str(), "client_group", &[][..])
        }
    };

    // Show matches from either search results or the item itself
    let display_matches: &[EntityMatch] = match search_results {
        Some(results) => results.as_slice(),
        None => matches,
    };

    // Handle global keyboard shortcuts (number keys 1-9 for selection)
    if action.is_none() {
        action = handle_keyboard_shortcuts(ctx, display_matches, entity_type);
    }

    egui::Window::new("Select Entity")
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(520.0, 450.0))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            // Header with progress and keyboard hint
            ui.horizontal(|ui| {
                ui.heading("Resolve Reference");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✕").clicked() {
                        action = Some(DisambiguationAction::Close);
                    }
                    ui.label(
                        RichText::new(format!("{} of {}", current_item_index + 1, total_items))
                            .small()
                            .color(Color32::GRAY),
                    );
                });
            });

            ui.separator();

            // Keyboard shortcut hint
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Tip: Press 1-9 to select, Esc to cancel")
                        .small()
                        .italics()
                        .color(Color32::from_rgb(120, 120, 140)),
                );
            });

            ui.add_space(4.0);

            // Agent prompt
            ui.label(
                RichText::new(&request.prompt)
                    .italics()
                    .color(Color32::LIGHT_GRAY),
            );
            ui.add_space(8.0);

            // What we're resolving
            ui.horizontal(|ui| {
                ui.label("Looking for:");
                ui.label(RichText::new(search_text).strong().color(Color32::YELLOW));
            });

            // Search refinement with auto-search hint
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Refine:");
                let response = TextEdit::singleline(search_buffer)
                    .desired_width(320.0)
                    .hint_text("Type to search (auto-searches after pause)...")
                    .show(ui);

                // Search on Enter key
                if response.response.lost_focus()
                    && ui.input(|i| i.key_pressed(Key::Enter))
                    && search_buffer.len() >= 2
                {
                    action = Some(DisambiguationAction::Search {
                        query: search_buffer.clone(),
                        entity_type: entity_type.to_string(),
                    });
                }

                // Auto-search after typing pause (debounced)
                if response.response.changed() && search_buffer.len() >= 2 {
                    // Check if enough time has passed since last change
                    if let Some(last_change) = data.last_search_change {
                        if last_change.elapsed().as_millis() as u64 >= AUTO_SEARCH_DELAY_MS {
                            action = Some(DisambiguationAction::Search {
                                query: search_buffer.clone(),
                                entity_type: entity_type.to_string(),
                            });
                        }
                    }
                }

                if data.searching {
                    ui.spinner();
                }
            });

            ui.add_space(8.0);
            ui.separator();

            // Matches label with count
            ui.horizontal(|ui| {
                ui.label(RichText::new("Matches:").small().color(Color32::GRAY));
                ui.label(
                    RichText::new(format!("({})", display_matches.len()))
                        .small()
                        .color(Color32::GRAY),
                );
            });

            // Matches scroll area
            ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                if display_matches.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            RichText::new("No matches found")
                                .color(Color32::GRAY)
                                .italics(),
                        );
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("Try a different search term or create a new entity")
                                .small()
                                .color(Color32::from_rgb(100, 100, 120)),
                        );
                    });
                } else {
                    for (idx, m) in display_matches.iter().enumerate() {
                        if let Some(select_action) = render_match_row(ui, idx, m) {
                            action = Some(select_action);
                        }
                    }
                }
            });

            ui.add_space(8.0);
            ui.separator();

            // Footer buttons with Create New option
            ui.horizontal(|ui| {
                // Create New button (prominent)
                if ui
                    .add(egui::Button::new("➕ Create New").fill(Color32::from_rgb(50, 80, 50)))
                    .on_hover_text(format!("Create a new entity named '{}'", search_text))
                    .clicked()
                {
                    action = Some(DisambiguationAction::CreateNew {
                        name: if search_buffer.is_empty() {
                            search_text.to_string()
                        } else {
                            search_buffer.clone()
                        },
                        entity_type: entity_type.to_string(),
                    });
                }

                if ui.button("Skip").clicked() {
                    action = Some(DisambiguationAction::Skip);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        action = Some(DisambiguationAction::Cancel);
                    }
                });
            });
        });

    action
}

/// Handle keyboard shortcuts for selection
fn handle_keyboard_shortcuts(
    ctx: &egui::Context,
    matches: &[EntityMatch],
    _entity_type: &str,
) -> Option<DisambiguationAction> {
    ctx.input(|i| {
        // Escape to cancel
        if i.key_pressed(Key::Escape) {
            return Some(DisambiguationAction::Cancel);
        }

        // Number keys 1-9 to select
        let number_keys = [
            (Key::Num1, 0),
            (Key::Num2, 1),
            (Key::Num3, 2),
            (Key::Num4, 3),
            (Key::Num5, 4),
            (Key::Num6, 5),
            (Key::Num7, 6),
            (Key::Num8, 7),
            (Key::Num9, 8),
        ];

        for (key, idx) in number_keys {
            if i.key_pressed(key) {
                if let Some(m) = matches.get(idx) {
                    return Some(DisambiguationAction::Select {
                        entity_id: m.entity_id.clone(),
                        entity_type: m.entity_type.clone(),
                        display_name: m.name.clone(),
                    });
                }
            }
        }

        None
    })
}

fn render_match_row(ui: &mut Ui, index: usize, m: &EntityMatch) -> Option<DisambiguationAction> {
    let mut action: Option<DisambiguationAction> = None;

    let score = m.score.unwrap_or(0.0) as f32;
    let score_color = if score > 0.9 {
        Color32::from_rgb(100, 200, 100)
    } else if score > 0.7 {
        Color32::from_rgb(200, 180, 80)
    } else {
        Color32::from_rgb(180, 140, 100)
    };

    // Keyboard shortcut indicator (1-9)
    let shortcut_hint = if index < 9 {
        format!("[{}] ", index + 1)
    } else {
        "    ".to_string()
    };

    egui::Frame::default()
        .fill(Color32::from_rgb(45, 50, 55))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Keyboard shortcut hint
                ui.label(
                    RichText::new(&shortcut_hint)
                        .monospace()
                        .color(Color32::from_rgb(100, 150, 200)),
                );

                // Select button
                if ui.button("Select").clicked() {
                    action = Some(DisambiguationAction::Select {
                        entity_id: m.entity_id.clone(),
                        entity_type: m.entity_type.clone(),
                        display_name: m.name.clone(),
                    });
                }

                // Display name
                ui.label(RichText::new(&m.name).strong());

                // Entity type badge
                ui.label(
                    RichText::new(format!("[{}]", m.entity_type))
                        .small()
                        .color(Color32::from_rgb(150, 150, 180)),
                );

                // Detail (jurisdiction, etc.)
                if let Some(ref jurisdiction) = m.jurisdiction {
                    ui.label(
                        RichText::new(jurisdiction)
                            .small()
                            .color(Color32::LIGHT_GRAY),
                    );
                }
                if let Some(ref context) = m.context {
                    ui.label(RichText::new(context).small().color(Color32::LIGHT_GRAY));
                }

                // Score on right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.0}%", score * 100.0))
                            .small()
                            .color(score_color),
                    );
                });
            });
        });

    ui.add_space(2.0);
    action
}
