//! CBU Search Modal
//!
//! A modal dialog for fuzzy searching CBUs using EntityGateway.
//!
//! Follows EGUI-RULES.md:
//! - Panel returns Option<CbuSearchAction>, no callbacks
//! - UI-only state: search buffer, open/closed
//! - Server data (results) passed in, never mutated

use crate::api::CbuSearchMatch;
use egui::{Align2, Color32, RichText, TextEdit, Ui, Vec2};

/// Actions that can be returned from the CBU search modal
#[derive(Clone, Debug)]
pub enum CbuSearchAction {
    /// User typed in the search box - trigger search
    Search { query: String },
    /// User selected a CBU
    Select { id: String, name: String },
    /// User closed the modal
    Close,
}

/// Data needed to render the CBU search modal (extracted before render)
pub struct CbuSearchData<'a> {
    /// Whether the modal is open
    pub open: bool,
    /// Search results from server (None = not yet searched)
    pub results: Option<&'a [CbuSearchMatch]>,
    /// Whether search is in progress
    pub searching: bool,
    /// Whether there are more results than shown
    pub truncated: bool,
}

/// Render the CBU search modal
///
/// Returns an action if the user interacted, None otherwise.
/// The caller handles the action (dispatch search, select CBU, close modal).
pub fn cbu_search_modal(
    ctx: &egui::Context,
    query_buffer: &mut String,
    data: &CbuSearchData<'_>,
) -> Option<CbuSearchAction> {
    if !data.open {
        return None;
    }

    let mut action: Option<CbuSearchAction> = None;

    egui::Window::new("Search CBU")
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(400.0, 350.0))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            action = render_modal_content(ui, query_buffer, data);
        });

    action
}

/// Render the modal content
fn render_modal_content(
    ui: &mut Ui,
    query_buffer: &mut String,
    data: &CbuSearchData<'_>,
) -> Option<CbuSearchAction> {
    let mut action: Option<CbuSearchAction> = None;

    // Search input row
    ui.horizontal(|ui| {
        ui.label("Search:");
        let response = TextEdit::singleline(query_buffer)
            .desired_width(280.0)
            .hint_text("Type CBU name...")
            .show(ui);

        if response.response.changed() && query_buffer.len() >= 2 {
            action = Some(CbuSearchAction::Search {
                query: query_buffer.clone(),
            });
        }

        if data.searching {
            ui.spinner();
        }
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // Results area
    egui::ScrollArea::vertical()
        .max_height(250.0)
        .show(ui, |ui| {
            if let Some(results) = data.results {
                if results.is_empty() {
                    ui.label(
                        RichText::new("No matches found")
                            .color(Color32::GRAY)
                            .italics(),
                    );
                } else {
                    for m in results {
                        if let Some(select_action) = render_match_row(ui, m) {
                            action = Some(select_action);
                        }
                    }

                    if data.truncated {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("More results available - refine your search")
                                .small()
                                .color(Color32::GRAY),
                        );
                    }
                }
            } else {
                ui.label(
                    RichText::new("Type at least 2 characters to search")
                        .color(Color32::GRAY)
                        .italics(),
                );
            }
        });

    ui.add_space(8.0);
    ui.separator();

    // Footer with close button
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Cancel").clicked() {
                action = Some(CbuSearchAction::Close);
            }
        });
    });

    action
}

/// Render a single match row
fn render_match_row(ui: &mut Ui, m: &CbuSearchMatch) -> Option<CbuSearchAction> {
    let mut action: Option<CbuSearchAction> = None;

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
                // Select button
                if ui.button("Select").clicked() {
                    action = Some(CbuSearchAction::Select {
                        id: m.value.clone(),
                        name: m.display.clone(),
                    });
                }

                // Name
                ui.label(RichText::new(&m.display).strong());

                // Detail (jurisdiction, etc.)
                if let Some(ref detail) = m.detail {
                    ui.label(RichText::new(detail).small().color(Color32::LIGHT_GRAY));
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

    action
}
