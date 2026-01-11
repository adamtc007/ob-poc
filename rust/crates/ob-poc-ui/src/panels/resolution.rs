//! Entity Resolution Panel
//!
//! A modal panel for resolving ambiguous entity references in DSL.
//! Uses the sub-session architecture with agent-driven conversation.
//!
//! Follows EGUI-RULES.md:
//! - Panel returns Option<ResolutionPanelAction>, no callbacks
//! - UI-only state passed in via ResolutionPanelData
//! - Server data (matches) passed in, never mutated

use crate::state::{WindowData, WindowEntry};
use egui::{Align2, Color32, RichText, TextEdit, Ui, Vec2};

/// Actions that can be returned from the resolution panel
#[derive(Clone, Debug)]
pub enum ResolutionPanelAction {
    /// User typed in the search box - trigger search
    Search { query: String },
    /// User selected a match
    Select { index: usize, entity_id: String },
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
}

/// Match result from entity search
#[derive(Clone, Debug)]
pub struct EntityMatchDisplay {
    /// Entity UUID
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
    pub messages: Vec<(String, String)>, // (role, content)
    /// Whether voice input is active
    pub voice_active: bool,
}

/// Render the resolution modal panel
///
/// Returns an action if the user interacted, None otherwise.
pub fn resolution_modal(
    ctx: &egui::Context,
    search_buffer: &mut String,
    chat_buffer: &mut String,
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
        .default_size(Vec2::new(500.0, 450.0))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            action = render_resolution_content(
                ui,
                search_buffer,
                chat_buffer,
                data,
                &subsession_id,
                current_ref_index,
                total_refs,
            );
        });

    action
}

/// Render the resolution modal content
fn render_resolution_content(
    ui: &mut Ui,
    search_buffer: &mut String,
    chat_buffer: &mut String,
    data: &ResolutionPanelData<'_>,
    _subsession_id: &str,
    current_ref_index: usize,
    total_refs: usize,
) -> Option<ResolutionPanelAction> {
    let mut action: Option<ResolutionPanelAction> = None;

    // Header with progress
    ui.horizontal(|ui| {
        ui.heading("Resolve Entity Reference");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("X").clicked() {
                action = Some(ResolutionPanelAction::Close);
            }
            ui.label(
                RichText::new(format!("{} of {}", current_ref_index + 1, total_refs))
                    .small()
                    .color(Color32::LIGHT_GRAY),
            );
        });
    });

    ui.separator();
    ui.add_space(4.0);

    // DSL Context (if available)
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

    // Current ref being resolved
    if let Some(ref name) = data.current_ref_name {
        ui.horizontal(|ui| {
            ui.label("Resolving:");
            ui.label(RichText::new(name).strong().color(Color32::YELLOW));
        });
        ui.add_space(4.0);
    }

    // Search input with voice button
    ui.horizontal(|ui| {
        ui.label("Search:");
        let response = TextEdit::singleline(search_buffer)
            .desired_width(280.0)
            .hint_text("Type name or add discriminators...")
            .show(ui);

        if response.response.changed() && search_buffer.len() >= 2 {
            action = Some(ResolutionPanelAction::Search {
                query: search_buffer.clone(),
            });
        }

        // Voice input button
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
        }

        // Voice listening indicator
        if data.voice_active {
            ui.label(RichText::new("Listening...").small().color(Color32::RED));
        }
    });

    ui.add_space(8.0);
    ui.separator();

    // Results area
    ui.label(RichText::new("Matches:").small().color(Color32::GRAY));

    egui::ScrollArea::vertical()
        .max_height(180.0)
        .show(ui, |ui| {
            if let Some(matches) = data.matches {
                if matches.is_empty() {
                    ui.label(
                        RichText::new("No matches found")
                            .color(Color32::GRAY)
                            .italics(),
                    );
                } else {
                    for (idx, m) in matches.iter().enumerate() {
                        if let Some(select_action) = render_match_row(ui, idx, m) {
                            action = Some(select_action);
                        }
                    }
                }
            } else {
                ui.label(
                    RichText::new("Search to find matching entities")
                        .color(Color32::GRAY)
                        .italics(),
                );
            }
        });

    ui.add_space(8.0);
    ui.separator();

    // Chat area (agent-driven conversation)
    ui.label(RichText::new("Conversation:").small().color(Color32::GRAY));

    egui::ScrollArea::vertical()
        .max_height(100.0)
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
        let response = TextEdit::singleline(chat_buffer)
            .desired_width(380.0)
            .hint_text("Type refinement (e.g., 'UK citizen, born 1965')...")
            .show(ui);

        if response.response.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
            && !chat_buffer.is_empty()
        {
            action = Some(ResolutionPanelAction::SendMessage {
                message: chat_buffer.clone(),
            });
            chat_buffer.clear();
        }

        if ui.button("Send").clicked() && !chat_buffer.is_empty() {
            action = Some(ResolutionPanelAction::SendMessage {
                message: chat_buffer.clone(),
            });
            chat_buffer.clear();
        }
    });

    ui.add_space(8.0);
    ui.separator();

    // Footer buttons
    ui.horizontal(|ui| {
        if ui.button("+ Create New").clicked() {
            action = Some(ResolutionPanelAction::CreateNew);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Cancel").clicked() {
                action = Some(ResolutionPanelAction::Complete { apply: false });
            }
            if ui.button("Skip").clicked() {
                action = Some(ResolutionPanelAction::Skip);
            }
            if current_ref_index + 1 >= total_refs
                && ui
                    .button(RichText::new("Complete").color(Color32::GREEN))
                    .clicked()
            {
                action = Some(ResolutionPanelAction::Complete { apply: true });
            }
        });
    });

    action
}

/// Render a single match row
fn render_match_row(
    ui: &mut Ui,
    index: usize,
    m: &EntityMatchDisplay,
) -> Option<ResolutionPanelAction> {
    let mut action: Option<ResolutionPanelAction> = None;

    let score_color = if m.score > 0.9 {
        Color32::from_rgb(100, 200, 100)
    } else if m.score > 0.7 {
        Color32::from_rgb(200, 180, 80)
    } else {
        Color32::from_rgb(180, 140, 100)
    };

    egui::Frame::default()
        .fill(Color32::from_rgb(45, 50, 55))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Selection number (keyboard shortcut hint)
                ui.label(
                    RichText::new(format!("{}.", index + 1))
                        .small()
                        .color(Color32::GRAY),
                );

                // Select button
                if ui.button("Select").clicked() {
                    action = Some(ResolutionPanelAction::Select {
                        index,
                        entity_id: m.id.clone(),
                    });
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
