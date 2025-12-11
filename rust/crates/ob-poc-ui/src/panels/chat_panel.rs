//! Chat Panel
//!
//! Agent chat interface with message history, input field, and action buttons.

use egui::{Color32, RichText, ScrollArea, TextEdit, Ui};

use crate::state::{ChatMessage, MessageStatus, SystemLevel};

/// Chat panel widget
pub struct ChatPanel {
    /// Input text buffer
    input: String,
    /// Whether input is focused
    input_focused: bool,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatPanel {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            input_focused: false,
        }
    }

    /// Render the chat panel
    /// Returns Some(message) if user submitted a message
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        messages: &[ChatMessage],
        _can_execute: bool,
        has_pending_dsl: bool,
    ) -> ChatPanelAction {
        let mut action = ChatPanelAction::None;

        ui.vertical(|ui| {
            // Panel header
            ui.horizontal(|ui| {
                ui.label(RichText::new("Chat").strong().size(14.0));
            });
            ui.separator();

            // Messages area
            let available_height = ui.available_height() - 50.0; // Reserve space for input
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .max_height(available_height)
                .show(ui, |ui| {
                    if messages.is_empty() {
                        self.render_empty_state(ui);
                    } else {
                        for msg in messages {
                            self.render_message(ui, msg);
                        }
                    }
                });

            ui.separator();

            // Action buttons if we have pending DSL
            if has_pending_dsl {
                ui.horizontal(|ui| {
                    if ui.button("✓ Execute").clicked() {
                        action = ChatPanelAction::Execute;
                    }
                    if ui.button("✗ Cancel").clicked() {
                        action = ChatPanelAction::Cancel;
                    }
                });
                ui.add_space(4.0);
            }

            // Input area
            ui.horizontal(|ui| {
                let response = ui.add(
                    TextEdit::singleline(&mut self.input)
                        .hint_text("Describe your onboarding scenario...")
                        .desired_width(ui.available_width() - 60.0),
                );

                self.input_focused = response.has_focus();

                // Send on Enter or button click
                let enter_pressed =
                    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                if (ui.button("Send").clicked() || enter_pressed) && !self.input.trim().is_empty() {
                    let message = std::mem::take(&mut self.input);
                    action = ChatPanelAction::SendMessage(message);
                }
            });
        });

        action
    }

    fn render_empty_state(&self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(RichText::new("Describe what you want to onboard").color(Color32::GRAY));
            ui.add_space(8.0);
            ui.label(
                RichText::new("e.g., \"Create a fund in Luxembourg called Apex Capital\"")
                    .color(Color32::DARK_GRAY)
                    .italics()
                    .size(12.0),
            );
        });
    }

    fn render_message(&self, ui: &mut Ui, msg: &ChatMessage) {
        match msg {
            ChatMessage::User { text } => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("You: ")
                            .color(Color32::from_rgb(96, 165, 250))
                            .strong(),
                    );
                    ui.label(text);
                });
            }
            ChatMessage::Assistant { text, dsl, status } => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("Agent: ")
                            .color(Color32::from_rgb(134, 239, 172))
                            .strong(),
                    );
                    ui.label(text);
                });

                // Show status badge
                let (status_text, status_color) = match status {
                    MessageStatus::Info => ("", Color32::GRAY),
                    MessageStatus::Valid => ("✓ Valid", Color32::from_rgb(74, 222, 128)),
                    MessageStatus::Error => ("✗ Error", Color32::from_rgb(248, 113, 113)),
                    MessageStatus::PendingConfirmation => {
                        ("⏳ Pending", Color32::from_rgb(251, 191, 36))
                    }
                    MessageStatus::Executed => ("✓ Executed", Color32::from_rgb(52, 211, 153)),
                };

                if !status_text.is_empty() {
                    ui.label(RichText::new(status_text).color(status_color).size(11.0));
                }

                // Show DSL preview if available
                if let Some(dsl_text) = dsl {
                    ui.add_space(4.0);
                    egui::Frame::none()
                        .fill(Color32::from_rgb(30, 30, 30))
                        .rounding(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(dsl_text)
                                    .monospace()
                                    .size(11.0)
                                    .color(Color32::from_rgb(212, 212, 212)),
                            );
                        });
                }
            }
            ChatMessage::System { text, level } => {
                let color = match level {
                    SystemLevel::Info => Color32::from_rgb(251, 191, 36),
                    SystemLevel::Warning => Color32::from_rgb(251, 191, 36),
                    SystemLevel::Error => Color32::from_rgb(248, 113, 113),
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("System: ").color(color).strong());
                    ui.label(RichText::new(text).color(color));
                });
            }
            ChatMessage::ExecutionResult {
                success,
                message,
                created_entities,
            } => {
                let (icon, color) = if *success {
                    ("✓", Color32::from_rgb(52, 211, 153))
                } else {
                    ("✗", Color32::from_rgb(248, 113, 113))
                };

                egui::Frame::none()
                    .fill(if *success {
                        Color32::from_rgb(30, 58, 30)
                    } else {
                        Color32::from_rgb(58, 30, 30)
                    })
                    .rounding(4.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new(format!("{} {}", icon, message)).color(color));

                        // Show created entities
                        if !created_entities.is_empty() {
                            ui.add_space(4.0);
                            for entity in created_entities {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&entity.entity_type)
                                            .size(10.0)
                                            .color(Color32::GRAY),
                                    );
                                    ui.label(
                                        RichText::new(&entity.name)
                                            .color(Color32::from_rgb(212, 212, 212)),
                                    );
                                    ui.label(
                                        RichText::new(format!("@{}", entity.binding))
                                            .monospace()
                                            .color(Color32::from_rgb(78, 201, 176)),
                                    );
                                });
                            }
                        }
                    });
            }
        }
        ui.add_space(8.0);
    }

    /// Check if input field has focus (for keyboard shortcut handling)
    pub fn has_input_focus(&self) -> bool {
        self.input_focused
    }
}

/// Actions that can be triggered from the chat panel
#[derive(Debug, Clone)]
pub enum ChatPanelAction {
    None,
    SendMessage(String),
    Execute,
    Cancel,
}
