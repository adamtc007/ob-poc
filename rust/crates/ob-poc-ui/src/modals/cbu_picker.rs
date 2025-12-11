//! CBU Picker Modal
//!
//! Search modal for selecting a CBU to work with.
//! Used from the header to switch between CBUs.

use egui::{Color32, RichText, ScrollArea, TextEdit};

use crate::state::CbuSummary;

/// CBU Picker modal state
pub struct CbuPickerModal {
    /// Whether the modal is open
    open: bool,
    /// Search query
    query: String,
    /// Filtered CBU list
    filtered: Vec<CbuSummary>,
    /// All CBUs (source for filtering)
    all_cbus: Vec<CbuSummary>,
    /// Selected index (for keyboard navigation)
    selected_index: Option<usize>,
}

/// Result from the CBU picker
#[derive(Debug, Clone)]
pub enum CbuPickerResult {
    /// No action taken
    None,
    /// User selected a CBU
    Selected(CbuSummary),
    /// User closed the modal
    Closed,
}

impl Default for CbuPickerModal {
    fn default() -> Self {
        Self::new()
    }
}

impl CbuPickerModal {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            filtered: Vec::new(),
            all_cbus: Vec::new(),
            selected_index: None,
        }
    }

    /// Open the modal with a list of CBUs
    pub fn open(&mut self, cbus: Vec<CbuSummary>) {
        self.open = true;
        self.query.clear();
        self.all_cbus = cbus;
        self.filtered = self.all_cbus.clone();
        self.selected_index = if self.filtered.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Close the modal
    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.filtered.clear();
        self.all_cbus.clear();
        self.selected_index = None;
    }

    /// Check if modal is open
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Filter CBUs based on current query
    fn update_filter(&mut self) {
        let query_lower = self.query.to_lowercase();
        self.filtered = self
            .all_cbus
            .iter()
            .filter(|cbu| {
                cbu.name.to_lowercase().contains(&query_lower)
                    || cbu
                        .jurisdiction
                        .as_ref()
                        .map(|j| j.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
                    || cbu
                        .client_type
                        .as_ref()
                        .map(|t| t.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
            })
            .cloned()
            .collect();

        // Reset selection
        self.selected_index = if self.filtered.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Render the modal
    pub fn ui(&mut self, ctx: &egui::Context) -> CbuPickerResult {
        if !self.open {
            return CbuPickerResult::None;
        }

        let mut result = CbuPickerResult::None;
        let mut should_close = false;
        let mut query_changed = false;

        egui::Window::new("Select CBU")
            .collapsible(false)
            .resizable(true)
            .default_width(450.0)
            .default_height(350.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Select a Client Business Unit")
                            .strong()
                            .size(14.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{} CBUs", self.all_cbus.len()))
                                .size(11.0)
                                .color(Color32::GRAY),
                        );
                    });
                });
                ui.separator();

                // Search input
                let old_query = self.query.clone();
                ui.horizontal(|ui| {
                    let response = ui.add(
                        TextEdit::singleline(&mut self.query)
                            .hint_text("Filter by name, jurisdiction, or type...")
                            .desired_width(ui.available_width()),
                    );

                    // Auto-focus the search input
                    if !response.has_focus() {
                        response.request_focus();
                    }
                });

                if self.query != old_query {
                    query_changed = true;
                }

                ui.add_space(8.0);

                // Handle keyboard navigation
                let up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
                let down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));

                if escape {
                    should_close = true;
                    result = CbuPickerResult::Closed;
                }

                if up {
                    if let Some(idx) = self.selected_index {
                        if idx > 0 {
                            self.selected_index = Some(idx - 1);
                        }
                    }
                }
                if down {
                    if let Some(idx) = self.selected_index {
                        if idx < self.filtered.len() - 1 {
                            self.selected_index = Some(idx + 1);
                        }
                    } else if !self.filtered.is_empty() {
                        self.selected_index = Some(0);
                    }
                }
                if enter {
                    if let Some(idx) = self.selected_index {
                        if let Some(cbu) = self.filtered.get(idx).cloned() {
                            result = CbuPickerResult::Selected(cbu);
                            should_close = true;
                        }
                    }
                }

                // Results area
                if self.filtered.is_empty() {
                    if self.query.is_empty() && self.all_cbus.is_empty() {
                        ui.label(
                            RichText::new("No CBUs available")
                                .color(Color32::GRAY)
                                .italics(),
                        );
                    } else if !self.query.is_empty() {
                        ui.label(
                            RichText::new("No matching CBUs found")
                                .color(Color32::from_rgb(251, 191, 36)),
                        );
                    }
                } else {
                    ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                        for (idx, cbu) in self.filtered.iter().enumerate() {
                            let is_selected = self.selected_index == Some(idx);

                            let response = egui::Frame::none()
                                .fill(if is_selected {
                                    Color32::from_rgb(60, 60, 80)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .rounding(4.0)
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // CBU name
                                        ui.label(
                                            RichText::new(&cbu.name).strong().color(Color32::WHITE),
                                        );

                                        // Client type badge
                                        if let Some(ref ctype) = cbu.client_type {
                                            ui.label(
                                                RichText::new(ctype)
                                                    .size(10.0)
                                                    .color(Color32::from_rgb(134, 239, 172)),
                                            );
                                        }

                                        // Jurisdiction
                                        if let Some(ref juris) = cbu.jurisdiction {
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(juris)
                                                            .size(11.0)
                                                            .color(Color32::GRAY),
                                                    );
                                                },
                                            );
                                        }
                                    });
                                })
                                .response;

                            // Handle click
                            if response.interact(egui::Sense::click()).clicked() {
                                result = CbuPickerResult::Selected(cbu.clone());
                                should_close = true;
                            }

                            // Hover effect
                            if response.hovered() && !is_selected {
                                self.selected_index = Some(idx);
                            }
                        }
                    });
                }

                ui.add_space(8.0);
                ui.separator();

                // Footer
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        should_close = true;
                        result = CbuPickerResult::Closed;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("↑↓ Navigate • Enter Select • Esc Cancel")
                                .size(10.0)
                                .color(Color32::GRAY),
                        );
                    });
                });
            });

        if query_changed {
            self.update_filter();
        }

        if should_close {
            self.close();
        }

        result
    }
}
