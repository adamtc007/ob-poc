//! DSL Panel
//!
//! Displays the generated DSL source with syntax highlighting and copy functionality.

use egui::{Color32, RichText, ScrollArea, Ui};

/// DSL panel widget
pub struct DslPanel;

impl Default for DslPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl DslPanel {
    pub fn new() -> Self {
        Self
    }

    /// Render the DSL panel
    /// Returns true if copy button was clicked
    pub fn ui(&self, ui: &mut Ui, dsl_source: &str) -> bool {
        let mut copy_clicked = false;

        ui.vertical(|ui| {
            // Panel header with copy button
            ui.horizontal(|ui| {
                ui.label(RichText::new("DSL Source").strong().size(14.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !dsl_source.is_empty() && ui.small_button("📋 Copy").clicked() {
                        copy_clicked = true;
                    }
                });
            });
            ui.separator();

            // DSL content
            if dsl_source.is_empty() {
                self.render_empty_state(ui);
            } else {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.render_dsl_highlighted(ui, dsl_source);
                    });
            }
        });

        copy_clicked
    }

    fn render_empty_state(&self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(RichText::new("No DSL generated yet").color(Color32::GRAY));
            ui.add_space(8.0);
            ui.label(
                RichText::new("DSL statements will appear here as you build")
                    .color(Color32::DARK_GRAY)
                    .size(12.0),
            );
        });
    }

    fn render_dsl_highlighted(&self, ui: &mut Ui, dsl_source: &str) {
        // Simple syntax highlighting for DSL
        // Full highlighting would parse the DSL, but for now we use regex-like patterns

        egui::Frame::none()
            .fill(Color32::from_rgb(30, 30, 30))
            .rounding(4.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                for line in dsl_source.lines() {
                    self.render_dsl_line(ui, line);
                }
            });
    }

    fn render_dsl_line(&self, ui: &mut Ui, line: &str) {
        let trimmed = line.trim();

        // Comment lines
        if trimmed.starts_with(';') {
            ui.label(
                RichText::new(line)
                    .monospace()
                    .size(12.0)
                    .color(Color32::from_rgb(106, 153, 85)), // Green for comments
            );
            return;
        }

        // For now, render with basic monospace styling
        // A full implementation would tokenize and color:
        // - (domain.verb) in yellow
        // - :keyword in blue
        // - "strings" in orange
        // - @symbols in teal
        // - numbers in light green

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            let chars = line.chars().peekable();
            let mut current_token = String::new();
            let mut in_string = false;

            for c in chars {
                if in_string {
                    current_token.push(c);
                    if c == '"' {
                        // End of string
                        ui.label(
                            RichText::new(&current_token)
                                .monospace()
                                .size(12.0)
                                .color(Color32::from_rgb(206, 145, 120)), // Orange for strings
                        );
                        current_token.clear();
                        in_string = false;
                    }
                } else if c == '"' {
                    // Flush current token
                    if !current_token.is_empty() {
                        self.render_token(ui, &current_token);
                        current_token.clear();
                    }
                    current_token.push(c);
                    in_string = true;
                } else if c == '(' || c == ')' || c == ' ' {
                    // Flush current token
                    if !current_token.is_empty() {
                        self.render_token(ui, &current_token);
                        current_token.clear();
                    }
                    // Render delimiter
                    ui.label(
                        RichText::new(c.to_string())
                            .monospace()
                            .size(12.0)
                            .color(Color32::from_rgb(212, 212, 212)),
                    );
                } else {
                    current_token.push(c);
                }
            }

            // Flush remaining token
            if !current_token.is_empty() {
                if in_string {
                    ui.label(
                        RichText::new(&current_token)
                            .monospace()
                            .size(12.0)
                            .color(Color32::from_rgb(206, 145, 120)),
                    );
                } else {
                    self.render_token(ui, &current_token);
                }
            }
        });
    }

    fn render_token(&self, ui: &mut Ui, token: &str) {
        let color = if token.starts_with(':') {
            // Keyword
            Color32::from_rgb(156, 220, 254) // Light blue
        } else if token.starts_with('@') {
            // Symbol reference
            Color32::from_rgb(78, 201, 176) // Teal
        } else if token.contains('.') && !token.starts_with('"') {
            // Domain.verb
            Color32::from_rgb(220, 220, 170) // Yellow
        } else if token.parse::<f64>().is_ok() {
            // Number
            Color32::from_rgb(181, 206, 168) // Light green
        } else {
            // Default
            Color32::from_rgb(212, 212, 212)
        };

        ui.label(RichText::new(token).monospace().size(12.0).color(color));
    }
}
