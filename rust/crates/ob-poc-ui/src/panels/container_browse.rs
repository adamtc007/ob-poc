//! Container Browse Panel
//!
//! Slide-in side panel for browsing container contents (investors, resources).
//! Reuses EntityGateway for search and follows Resolution Panel patterns.
//!
//! Follows EGUI-RULES.md:
//! - Panel returns Option<ContainerBrowseAction>, no callbacks
//! - UI-only state: search buffer, selected index, pagination
//! - Server data (results) passed in, never mutated

use egui::{Color32, RichText, ScrollArea, TextEdit, Ui};

// =============================================================================
// ACTIONS - returned to caller for handling
// =============================================================================

/// Actions that can be returned from the container browse panel
#[derive(Clone, Debug)]
pub enum ContainerBrowseAction {
    /// Close the panel
    Close,
    /// Trigger search with current filters
    Search {
        query: String,
        filters: Vec<(String, String)>,
        offset: i32,
        limit: i32,
    },
    /// Change page
    PageChange { offset: i32 },
    /// Item selected (single click)
    SelectItem { id: String },
    /// Item opened (double click) - navigate to detail
    OpenItem { id: String },
    /// Filter changed
    FilterChange {
        field: String,
        value: Option<String>,
    },
}

// =============================================================================
// VIEW DATA - passed in from caller (extracted from async state)
// =============================================================================

/// Data needed to render the container browse panel
#[allow(dead_code)] // Fields will be used when EntityGateway integration is complete
pub struct ContainerBrowseData<'a> {
    /// Whether the panel is open
    pub open: bool,

    /// Container being browsed
    pub container_id: Option<&'a str>,
    pub container_type: Option<&'a str>,
    pub container_label: Option<&'a str>,

    /// EntityGateway nickname for children
    pub browse_nickname: Option<&'a str>,
    pub parent_key: Option<&'a str>,

    /// Search/filter state
    pub active_filters: &'a [(String, String)],
    pub available_facets: &'a [FacetInfo],

    /// Pagination state
    pub offset: i32,
    pub limit: i32,
    pub total_count: i64,

    /// Results
    pub items: &'a [BrowseItemView],

    /// Loading state
    pub loading: bool,
    pub error: Option<&'a str>,

    /// Selected item index
    pub selected_idx: Option<usize>,
}

/// View model for a browse item
#[derive(Clone, Debug, Default)]
#[allow(dead_code)] // Will be used when EntityGateway integration is complete
pub struct BrowseItemView {
    pub id: String,
    pub display: String,
    pub sublabel: String,
    pub status: String,
    pub status_color: Color32,
    pub fields: Vec<(String, String)>,
}

/// Facet info for filter dropdowns
#[derive(Clone, Debug, Default)]
pub struct FacetInfo {
    pub field: String,
    pub label: String,
    pub values: Vec<(String, i64)>,
}

// =============================================================================
// UI STATE - owned by caller, passed mutably
// =============================================================================

/// UI-only state for the container browse panel
#[derive(Default, Clone)]
pub struct ContainerBrowseState {
    /// Search query buffer
    pub search_query: String,
    /// Whether the panel is open
    pub open: bool,
    /// Selected item index
    pub selected_idx: Option<usize>,
    /// Container being browsed
    pub container_id: Option<String>,
    pub container_type: Option<String>,
    pub container_label: Option<String>,
    /// Browse config
    pub browse_nickname: Option<String>,
    pub parent_key: Option<String>,
    /// Pagination
    pub offset: i32,
    pub limit: i32,
    /// Active filters
    pub active_filters: Vec<(String, String)>,
}

impl ContainerBrowseState {
    /// Open the panel for a container
    pub fn open_container(
        &mut self,
        container_id: String,
        container_type: String,
        container_label: String,
        parent_key: Option<String>,
        browse_nickname: Option<String>,
    ) {
        self.open = true;
        self.container_id = Some(container_id);
        self.container_type = Some(container_type);
        self.container_label = Some(container_label);
        self.browse_nickname = browse_nickname;
        self.parent_key = parent_key;
        self.search_query.clear();
        self.active_filters.clear();
        self.offset = 0;
        self.limit = 50;
        self.selected_idx = None;
    }

    /// Close the panel
    pub fn close(&mut self) {
        self.open = false;
        self.container_id = None;
    }

    /// Add or update a filter
    pub fn set_filter(&mut self, field: String, value: Option<String>) {
        self.active_filters.retain(|(f, _)| f != &field);
        if let Some(v) = value {
            self.active_filters.push((field, v));
        }
        self.offset = 0; // Reset pagination when filter changes
    }

    /// Build search action from current state
    pub fn build_search_action(&self) -> Option<ContainerBrowseAction> {
        if !self.open || self.container_id.is_none() {
            return None;
        }
        Some(ContainerBrowseAction::Search {
            query: self.search_query.clone(),
            filters: self.active_filters.clone(),
            offset: self.offset,
            limit: self.limit,
        })
    }
}

// =============================================================================
// RENDER FUNCTION
// =============================================================================

/// Render the container browse panel
///
/// Returns an action if the user interacted, None otherwise.
pub fn container_browse_panel(
    ctx: &egui::Context,
    state: &mut ContainerBrowseState,
    data: &ContainerBrowseData<'_>,
) -> Option<ContainerBrowseAction> {
    if !data.open {
        return None;
    }

    let mut action: Option<ContainerBrowseAction> = None;

    // Side panel (slide-in from right)
    egui::SidePanel::right("container_browse_panel")
        .resizable(true)
        .default_width(420.0)
        .min_width(320.0)
        .max_width(600.0)
        .show(ctx, |ui| {
            action = render_panel_content(ui, state, data);
        });

    action
}

/// Render the panel content
fn render_panel_content(
    ui: &mut Ui,
    state: &mut ContainerBrowseState,
    data: &ContainerBrowseData<'_>,
) -> Option<ContainerBrowseAction> {
    let mut action: Option<ContainerBrowseAction> = None;

    // Header
    ui.horizontal(|ui| {
        ui.heading(data.container_label.unwrap_or("Container"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("X").clicked() {
                action = Some(ContainerBrowseAction::Close);
            }
        });
    });

    // Subtitle with count
    ui.horizontal(|ui| {
        if let Some(container_type) = data.container_type {
            ui.label(
                RichText::new(container_type)
                    .small()
                    .color(Color32::LIGHT_GRAY),
            );
            ui.label(RichText::new("|").small().color(Color32::DARK_GRAY));
        }
        ui.label(
            RichText::new(format!("{} items", data.total_count))
                .small()
                .color(Color32::LIGHT_GRAY),
        );
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // Search bar
    ui.horizontal(|ui| {
        ui.label("Search:");
        let response = TextEdit::singleline(&mut state.search_query)
            .desired_width(250.0)
            .hint_text("Filter by name...")
            .show(ui);

        if response.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(search_action) = state.build_search_action() {
                action = Some(search_action);
            }
        }

        if ui.button("Go").clicked() {
            if let Some(search_action) = state.build_search_action() {
                action = Some(search_action);
            }
        }

        if data.loading {
            ui.spinner();
        }
    });

    // Filters row
    if !data.available_facets.is_empty() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for facet in data.available_facets {
                let current_value = data
                    .active_filters
                    .iter()
                    .find(|(f, _)| f == &facet.field)
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("All");

                egui::ComboBox::from_id_salt(&facet.field)
                    .selected_text(format!("{}: {}", facet.label, current_value))
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(current_value == "All", "All").clicked() {
                            action = Some(ContainerBrowseAction::FilterChange {
                                field: facet.field.clone(),
                                value: None,
                            });
                        }
                        for (value, count) in &facet.values {
                            let label = format!("{} ({})", value, count);
                            if ui
                                .selectable_label(current_value == value, &label)
                                .clicked()
                            {
                                action = Some(ContainerBrowseAction::FilterChange {
                                    field: facet.field.clone(),
                                    value: Some(value.clone()),
                                });
                            }
                        }
                    });
            }
        });
    }

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // Error message
    if let Some(error) = data.error {
        ui.colored_label(Color32::RED, error);
        ui.add_space(4.0);
    }

    // Results list (scrollable)
    let available_height = ui.available_height() - 60.0; // Reserve space for pagination
    ScrollArea::vertical()
        .max_height(available_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if data.items.is_empty() && !data.loading {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new("No items found")
                            .color(Color32::GRAY)
                            .italics(),
                    );
                });
            } else {
                for (idx, item) in data.items.iter().enumerate() {
                    let is_selected = data.selected_idx == Some(idx);

                    let item_action = render_item_row(ui, item, is_selected, idx);
                    if item_action.is_some() {
                        action = item_action;
                    }
                }
            }
        });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // Pagination footer
    ui.horizontal(|ui| {
        let page = (data.offset / data.limit) + 1;
        let total_pages = ((data.total_count as i32 + data.limit - 1) / data.limit).max(1);

        if ui
            .add_enabled(data.offset > 0, egui::Button::new("< Prev"))
            .clicked()
        {
            let new_offset = (data.offset - data.limit).max(0);
            action = Some(ContainerBrowseAction::PageChange { offset: new_offset });
        }

        ui.label(format!("Page {} of {}", page, total_pages));

        let has_next = data.offset + data.limit < data.total_count as i32;
        if ui
            .add_enabled(has_next, egui::Button::new("Next >"))
            .clicked()
        {
            let new_offset = data.offset + data.limit;
            action = Some(ContainerBrowseAction::PageChange { offset: new_offset });
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Back to Graph").clicked() {
                action = Some(ContainerBrowseAction::Close);
            }
        });
    });

    action
}

/// Render a single item row
fn render_item_row(
    ui: &mut Ui,
    item: &BrowseItemView,
    is_selected: bool,
    idx: usize,
) -> Option<ContainerBrowseAction> {
    let mut action: Option<ContainerBrowseAction> = None;

    let bg_color = if is_selected {
        Color32::from_rgb(60, 80, 100)
    } else {
        Color32::from_rgb(40, 45, 50)
    };

    egui::Frame::default()
        .fill(bg_color)
        .inner_margin(8.0)
        .outer_margin(egui::Margin::symmetric(0.0, 2.0))
        .rounding(4.0)
        .show(ui, |ui| {
            let response = ui.interact(
                ui.available_rect_before_wrap(),
                ui.id().with(idx),
                egui::Sense::click(),
            );

            ui.horizontal(|ui| {
                // Status indicator bar
                let status_rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(4.0, 36.0));
                ui.painter()
                    .rect_filled(status_rect, 2.0, item.status_color);
                ui.add_space(12.0);

                // Content
                ui.vertical(|ui| {
                    ui.label(RichText::new(&item.display).strong());
                    if !item.sublabel.is_empty() {
                        ui.label(
                            RichText::new(&item.sublabel)
                                .small()
                                .color(Color32::LIGHT_GRAY),
                        );
                    }
                });

                // Status badge on right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !item.status.is_empty() {
                        ui.label(RichText::new(&item.status).small().color(item.status_color));
                    }
                });
            });

            if response.clicked() {
                action = Some(ContainerBrowseAction::SelectItem {
                    id: item.id.clone(),
                });
            }

            if response.double_clicked() {
                action = Some(ContainerBrowseAction::OpenItem {
                    id: item.id.clone(),
                });
            }
        });

    action
}

// =============================================================================
// HELPER - status color mapping
// =============================================================================

/// Get status color for common status values
#[allow(dead_code)] // Will be used when EntityGateway integration is complete
pub fn status_color(status: &str) -> Color32 {
    match status.to_uppercase().as_str() {
        "ACTIVE" | "VERIFIED" | "COMPLETE" | "APPROVED" => Color32::from_rgb(100, 200, 100),
        "PENDING" | "IN_PROGRESS" | "PROCESSING" => Color32::from_rgb(200, 180, 80),
        "EXPIRED" | "SUSPENDED" | "BLOCKED" => Color32::from_rgb(200, 100, 100),
        "DRAFT" | "NEW" => Color32::from_rgb(150, 150, 150),
        _ => Color32::GRAY,
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_open_close() {
        let mut state = ContainerBrowseState::default();
        assert!(!state.open);

        state.open_container(
            "uuid-123".to_string(),
            "ShareClass".to_string(),
            "Class A USD".to_string(),
            Some("INVESTOR_HOLDING".to_string()),
            Some("share_class_id".to_string()),
        );

        assert!(state.open);
        assert_eq!(state.container_id, Some("uuid-123".to_string()));
        assert_eq!(state.browse_nickname, Some("INVESTOR_HOLDING".to_string()));

        state.close();
        assert!(!state.open);
        assert!(state.container_id.is_none());
    }

    #[test]
    fn test_filter_management() {
        let mut state = ContainerBrowseState {
            open: true,
            container_id: Some("test".to_string()),
            ..Default::default()
        };

        // Add filter
        state.set_filter("jurisdiction".to_string(), Some("US".to_string()));
        assert_eq!(state.active_filters.len(), 1);
        assert_eq!(
            state.active_filters[0],
            ("jurisdiction".to_string(), "US".to_string())
        );

        // Update filter
        state.set_filter("jurisdiction".to_string(), Some("GB".to_string()));
        assert_eq!(state.active_filters.len(), 1);
        assert_eq!(
            state.active_filters[0],
            ("jurisdiction".to_string(), "GB".to_string())
        );

        // Remove filter
        state.set_filter("jurisdiction".to_string(), None);
        assert!(state.active_filters.is_empty());
    }

    #[test]
    fn test_status_colors() {
        assert_eq!(status_color("ACTIVE"), Color32::from_rgb(100, 200, 100));
        assert_eq!(status_color("PENDING"), Color32::from_rgb(200, 180, 80));
        assert_eq!(status_color("EXPIRED"), Color32::from_rgb(200, 100, 100));
        assert_eq!(status_color("unknown"), Color32::GRAY);
    }
}
