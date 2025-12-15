//! Entity Search Popup Widget
//!
//! A popup for searching and selecting entities.
//! Returns selected entity via return value (not callback).

use egui::{Color32, RichText, TextEdit, Ui};
use uuid::Uuid;

/// Response from entity search popup
#[derive(Default)]
pub struct EntitySearchResponse {
    /// User selected an entity
    pub selected: Option<SelectedEntity>,
    /// User closed the popup without selecting
    pub closed: bool,
    /// Query text changed
    pub query_changed: bool,
}

/// A selected entity from search
#[derive(Clone)]
pub struct SelectedEntity {
    pub id: Uuid,
    pub name: String,
    pub entity_type: String,
}

/// Entity search result (from API)
#[derive(Clone)]
pub struct EntitySearchResult {
    pub id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub extra_info: Option<String>,
}

/// Render entity search popup
///
/// Returns EntitySearchResponse with selection or close events.
/// Caller is responsible for:
/// 1. Fetching search results from API when query changes
/// 2. Handling the selected entity
pub fn entity_search_popup(
    ui: &mut Ui,
    query: &mut String,
    results: &[EntitySearchResult],
    loading: bool,
) -> EntitySearchResponse {
    let mut response = EntitySearchResponse::default();

    egui::Frame::default()
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke)
        .inner_margin(8.0)
        .rounding(4.0)
        .shadow(egui::Shadow::NONE) // Simple - no shadow needed for popup
        .show(ui, |ui| {
            ui.set_min_width(300.0);
            ui.set_max_height(400.0);

            // Search input
            ui.horizontal(|ui| {
                ui.label("Search:");
                let text_response = TextEdit::singleline(query)
                    .hint_text("Type to search...")
                    .desired_width(200.0)
                    .show(ui);

                if text_response.response.changed() {
                    response.query_changed = true;
                }

                if loading {
                    ui.spinner();
                }

                if ui.button("X").clicked() {
                    response.closed = true;
                }
            });

            ui.separator();

            // Results list
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    if results.is_empty() && !query.is_empty() && !loading {
                        ui.label("No results found");
                    }

                    for result in results {
                        let item_response = ui
                            .horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&result.name).strong());
                                        ui.label(
                                            RichText::new(&result.entity_type)
                                                .small()
                                                .color(Color32::GRAY),
                                        );
                                    });

                                    if let Some(ref jurisdiction) = result.jurisdiction {
                                        ui.label(
                                            RichText::new(jurisdiction)
                                                .small()
                                                .color(Color32::GRAY),
                                        );
                                    }

                                    if let Some(ref extra) = result.extra_info {
                                        ui.label(RichText::new(extra).small().color(Color32::GRAY));
                                    }
                                });
                            })
                            .response;

                        if item_response.clicked() {
                            response.selected = Some(SelectedEntity {
                                id: result.id,
                                name: result.name.clone(),
                                entity_type: result.entity_type.clone(),
                            });
                        }

                        if item_response.hovered() {
                            ui.painter().rect_stroke(
                                item_response.rect,
                                2.0,
                                egui::Stroke::new(1.0, Color32::LIGHT_BLUE),
                            );
                        }

                        ui.separator();
                    }
                });
        });

    response
}
