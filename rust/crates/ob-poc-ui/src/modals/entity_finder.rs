//! Entity Finder Modal
//!
//! Search modal for resolving unresolved EntityRefs.
//! Allows user to search for entities and select one to resolve the reference.

use egui::{Color32, RichText, ScrollArea, TextEdit};

use crate::state::EntityMatch;

/// Entity Finder modal state
pub struct EntityFinderModal {
    /// Whether the modal is open
    open: bool,
    /// Search query
    query: String,
    /// Entity type being searched
    entity_type: String,
    /// Context: which statement/arg this is resolving
    context: Option<ResolveContext>,
    /// Search results
    results: Vec<EntityMatch>,
    /// Loading state
    loading: bool,
    /// Selected index (for keyboard navigation)
    selected_index: Option<usize>,
}

/// Context for what we're resolving
#[derive(Debug, Clone)]
pub struct ResolveContext {
    pub statement_idx: usize,
    pub arg_key: String,
    pub original_text: String,
}

/// Result from the entity finder
#[derive(Debug, Clone)]
pub enum EntityFinderResult {
    /// No action taken
    None,
    /// User selected an entity
    Selected {
        context: ResolveContext,
        entity: EntityMatch,
    },
    /// User closed the modal
    Closed,
    /// User wants to search (returns query for API call)
    Search { entity_type: String, query: String },
}

impl Default for EntityFinderModal {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityFinderModal {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            entity_type: String::new(),
            context: None,
            results: Vec::new(),
            loading: false,
            selected_index: None,
        }
    }

    /// Open the modal for a specific entity type
    pub fn open(&mut self, entity_type: &str, initial_query: &str, context: ResolveContext) {
        self.open = true;
        self.entity_type = entity_type.to_string();
        self.query = initial_query.to_string();
        self.context = Some(context);
        self.results.clear();
        self.loading = false;
        self.selected_index = None;
    }

    /// Close the modal
    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.entity_type.clear();
        self.context = None;
        self.results.clear();
        self.loading = false;
        self.selected_index = None;
    }

    /// Set search results
    pub fn set_results(&mut self, results: Vec<EntityMatch>) {
        self.results = results;
        self.loading = false;
        if !self.results.is_empty() {
            self.selected_index = Some(0);
        }
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    /// Check if modal is open
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Render the modal
    pub fn ui(&mut self, ctx: &egui::Context) -> EntityFinderResult {
        if !self.open {
            return EntityFinderResult::None;
        }

        let mut result = EntityFinderResult::None;
        let mut should_close = false;
        let mut should_search = false;

        egui::Window::new("Find Entity")
            .collapsible(false)
            .resizable(true)
            .default_width(400.0)
            .default_height(300.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Search for: {}", self.entity_type))
                            .strong()
                            .size(14.0),
                    );
                });
                ui.separator();

                // Search input
                ui.horizontal(|ui| {
                    let response = ui.add(
                        TextEdit::singleline(&mut self.query)
                            .hint_text("Type to search...")
                            .desired_width(ui.available_width() - 70.0),
                    );

                    // Auto-focus the search input
                    if response.gained_focus() || self.query.is_empty() {
                        response.request_focus();
                    }

                    // Search on Enter or button click
                    if ui.button("Search").clicked()
                        || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        should_search = true;
                    }
                });

                ui.add_space(8.0);

                // Results area
                if self.loading {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Searching...");
                    });
                } else if self.results.is_empty() {
                    if self.query.is_empty() {
                        ui.label(
                            RichText::new("Enter a search term above")
                                .color(Color32::GRAY)
                                .italics(),
                        );
                    } else {
                        ui.label(
                            RichText::new("No results found")
                                .color(Color32::from_rgb(251, 191, 36)),
                        );
                    }
                } else {
                    // Handle keyboard navigation
                    let up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
                    let down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));

                    if up {
                        if let Some(idx) = self.selected_index {
                            if idx > 0 {
                                self.selected_index = Some(idx - 1);
                            }
                        }
                    }
                    if down {
                        if let Some(idx) = self.selected_index {
                            if idx < self.results.len() - 1 {
                                self.selected_index = Some(idx + 1);
                            }
                        } else if !self.results.is_empty() {
                            self.selected_index = Some(0);
                        }
                    }
                    if enter {
                        if let Some(idx) = self.selected_index {
                            if let (Some(entity), Some(context)) =
                                (self.results.get(idx).cloned(), self.context.clone())
                            {
                                result = EntityFinderResult::Selected { context, entity };
                                should_close = true;
                            }
                        }
                    }

                    // Results list
                    ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for (idx, entity) in self.results.iter().enumerate() {
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
                                        // Entity name
                                        ui.label(
                                            RichText::new(&entity.name)
                                                .strong()
                                                .color(Color32::WHITE),
                                        );

                                        // Entity type badge
                                        if let Some(ref etype) = entity.entity_type {
                                            ui.label(
                                                RichText::new(etype)
                                                    .size(10.0)
                                                    .color(Color32::GRAY),
                                            );
                                        }

                                        // Jurisdiction
                                        if let Some(ref juris) = entity.jurisdiction {
                                            ui.label(
                                                RichText::new(format!("({})", juris))
                                                    .size(10.0)
                                                    .color(Color32::GRAY),
                                            );
                                        }

                                        // Score
                                        if entity.score > 0.0 {
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{:.0}%",
                                                            entity.score * 100.0
                                                        ))
                                                        .size(10.0)
                                                        .color(Color32::from_rgb(134, 239, 172)),
                                                    );
                                                },
                                            );
                                        }
                                    });
                                })
                                .response;

                            // Handle click
                            if response.interact(egui::Sense::click()).clicked() {
                                if let Some(context) = self.context.clone() {
                                    result = EntityFinderResult::Selected {
                                        context,
                                        entity: entity.clone(),
                                    };
                                    should_close = true;
                                }
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

                // Footer with Cancel button
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Escape))
                    {
                        should_close = true;
                        result = EntityFinderResult::Closed;
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

        if should_close {
            self.close();
        }

        if should_search && !self.query.is_empty() {
            self.loading = true;
            return EntityFinderResult::Search {
                entity_type: self.entity_type.clone(),
                query: self.query.clone(),
            };
        }

        result
    }
}
