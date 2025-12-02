//! Agent Panel - DSL Agent Chat Interface
//!
//! Placeholder module for the agentic DSL chat panel.
//! This will be implemented to provide a chat-style interface for
//! generating and executing DSL commands.

#![allow(dead_code)]

use egui::Ui;

/// Agent panel widget for DSL generation and execution
pub struct AgentPanel {
    /// Chat input buffer
    input: String,
    /// Chat history
    history: Vec<ChatMessage>,
}

struct ChatMessage {
    role: MessageRole,
    content: String,
}

enum MessageRole {
    User,
    Assistant,
    System,
}

impl Default for AgentPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentPanel {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            history: Vec::new(),
        }
    }

    /// Render the agent panel UI
    pub fn ui(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            // Header
            ui.heading("DSL Agent");
            ui.separator();

            // Chat history area
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for msg in &self.history {
                        self.render_message(ui, msg);
                    }
                });

            ui.separator();

            // Input area
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.input)
                        .hint_text("Describe what you want to do...")
                        .desired_width(ui.available_width() - 60.0),
                );

                if ui.button("Send").clicked()
                    || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    if !self.input.trim().is_empty() {
                        self.send_message();
                    }
                }
            });
        });
    }

    fn render_message(&self, ui: &mut Ui, msg: &ChatMessage) {
        let (prefix, color) = match msg.role {
            MessageRole::User => ("You: ", egui::Color32::from_rgb(96, 165, 250)),
            MessageRole::Assistant => ("Agent: ", egui::Color32::from_rgb(134, 239, 172)),
            MessageRole::System => ("System: ", egui::Color32::from_rgb(251, 191, 36)),
        };

        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(prefix).color(color).strong());
            ui.label(&msg.content);
        });
        ui.add_space(4.0);
    }

    fn send_message(&mut self) {
        let content = std::mem::take(&mut self.input);
        self.history.push(ChatMessage {
            role: MessageRole::User,
            content,
        });

        // TODO: Send to agent API and get response
        self.history.push(ChatMessage {
            role: MessageRole::System,
            content: "Agent integration not yet implemented".to_string(),
        });
    }
}
