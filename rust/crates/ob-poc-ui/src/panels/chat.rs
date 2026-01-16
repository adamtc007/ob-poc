//! Chat Panel
//!
//! Displays chat messages and allows sending new messages.
//! Messages are accumulated locally from ChatResponse (server doesn't persist full history).

use crate::state::{AppState, ChatMessage, MessageRole};
use egui::{Color32, RichText, ScrollArea, TextEdit, Ui};

pub fn chat_panel(ui: &mut Ui, state: &mut AppState) {
    // Rule 3: Single lock, extract all needed data, then render
    let (loading_chat, should_focus) = {
        let mut guard = match state.async_state.lock() {
            Ok(g) => g,
            Err(_) => return, // Poisoned lock, skip rendering
        };
        let loading = guard.loading_chat;
        // Focus on: chat completion OR initial app load
        let focus = !guard.loading_chat && (guard.chat_just_finished || guard.needs_initial_focus);
        if guard.chat_just_finished {
            guard.chat_just_finished = false;
        }
        if guard.needs_initial_focus {
            guard.needs_initial_focus = false;
        }
        (loading, focus)
    };
    // Lock released here

    ui.vertical(|ui| {
        // Header
        ui.horizontal(|ui| {
            ui.heading("Agent Chat");
            if loading_chat {
                ui.spinner();
            }
        });

        ui.separator();

        // Messages area (scrollable)
        // Reserve space for 5-line input area + separator + button
        let input_area_height = 120.0; // ~5 lines of text + padding + button
        let available_height = ui.available_height() - input_area_height;
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

        // Request focus on chat input after agent responds
        if should_focus {
            ui.memory_mut(|mem| mem.request_focus(chat_input_id));
        }

        // Multiline input area (5 lines tall) - use fixed min height for consistent layout
        let text_height = 5.0 * ui.text_style_height(&egui::TextStyle::Body) + 12.0;

        ui.horizontal_top(|ui| {
            let _response = TextEdit::multiline(&mut state.buffers.chat_input)
                .desired_width(ui.available_width() - 80.0)
                .desired_rows(5)
                .min_size(egui::vec2(0.0, text_height))
                .hint_text("Ask the agent to generate DSL...")
                .id(chat_input_id)
                .show(ui);

            // Send on Ctrl+Enter or Cmd+Enter (Enter alone adds newline in multiline)
            let modifiers = ui.input(|i| i.modifiers);
            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let send_shortcut = enter_pressed && (modifiers.ctrl || modifiers.command);

            let can_send = !state.buffers.chat_input.trim().is_empty() && !loading_chat;

            ui.vertical(|ui| {
                // Add button with hover text showing session state for debugging
                let button = ui
                    .add_enabled(can_send, egui::Button::new("Send"))
                    .on_hover_text(format!(
                        "Session: {:?}\n(Ctrl+Enter to send)",
                        state.session_id
                    ));

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
