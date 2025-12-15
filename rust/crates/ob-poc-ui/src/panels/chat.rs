//! Chat Panel
//!
//! Displays chat messages and allows sending new messages.
//! Messages are accumulated locally from ChatResponse (server doesn't persist full history).

use crate::state::{AppState, ChatMessage, MessageRole};
use egui::{Color32, RichText, ScrollArea, TextEdit, Ui};

pub fn chat_panel(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        // Header
        ui.horizontal(|ui| {
            ui.heading("Agent Chat");
            if let Ok(async_state) = state.async_state.lock() {
                if async_state.loading_chat {
                    ui.spinner();
                }
            }
        });

        ui.separator();

        // Messages area (scrollable)
        let available_height = ui.available_height() - 60.0; // Reserve space for input
        ScrollArea::vertical()
            .max_height(available_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if state.messages.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label("Start a conversation with the agent...");
                    });
                } else {
                    for msg in &state.messages {
                        render_message(ui, msg);
                        ui.add_space(8.0);
                    }
                }
            });

        ui.separator();

        // Input area
        let chat_input_id = egui::Id::new("chat_input");

        // Request focus on chat input after agent responds (loading just finished)
        let should_focus = state
            .async_state
            .lock()
            .map(|s| !s.loading_chat && s.chat_just_finished)
            .unwrap_or(false);
        if should_focus {
            ui.memory_mut(|mem| mem.request_focus(chat_input_id));
            if let Ok(mut s) = state.async_state.lock() {
                s.chat_just_finished = false;
            }
        }

        ui.horizontal(|ui| {
            let response = TextEdit::singleline(&mut state.buffers.chat_input)
                .desired_width(ui.available_width() - 80.0)
                .hint_text("Ask the agent to generate DSL...")
                .id(chat_input_id)
                .show(ui);

            // Send on Enter (without shift for newline)
            let send_shortcut =
                response.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            let can_send = !state.buffers.chat_input.trim().is_empty()
                && state
                    .async_state
                    .lock()
                    .map(|s| !s.loading_chat)
                    .unwrap_or(true);

            // Add button with hover text showing session state for debugging
            let button = ui
                .add_enabled(can_send, egui::Button::new("Send"))
                .on_hover_text(format!("Session: {:?}", state.session_id));

            if (button.clicked() || send_shortcut) && can_send {
                web_sys::console::log_1(
                    &format!(
                        ">>> SEND CLICKED: '{}' session={:?}",
                        state.buffers.chat_input, state.session_id
                    )
                    .into(),
                );
                state.send_chat_message();
            }
        });
    });
}

fn render_message(ui: &mut Ui, msg: &ChatMessage) {
    let is_user = msg.role == MessageRole::User;
    let bg_color = if is_user {
        Color32::from_rgb(40, 60, 80)
    } else {
        Color32::from_rgb(50, 50, 60)
    };

    egui::Frame::default()
        .fill(bg_color)
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let role_text = if is_user { "You" } else { "Agent" };
                ui.label(RichText::new(role_text).strong().color(if is_user {
                    Color32::LIGHT_BLUE
                } else {
                    Color32::LIGHT_GREEN
                }));

                // Timestamp
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
