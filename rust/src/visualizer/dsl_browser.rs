//! DSL Browser Panel
//!
//! This module implements the DSL browser panel for the visualizer application.
//! It provides a list view of DSL instances with filtering, searching, and selection
//! capabilities. The panel connects to the backend to fetch DSL data and allows
//! users to browse and select DSL instances for AST visualization.

use super::{
    constants::*,
    models::{DSLEntry, FilterOptions},
    VisualizerResult,
};
use eframe::egui;
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// DSL Browser Panel state and functionality
pub struct DSLBrowserPanel {
    /// List of DSL instances
    dsl_entries: Vec<DSLEntry>,

    /// Currently selected DSL entry index
    selected_index: Option<usize>,

    /// Search filter text
    search_filter: String,

    /// Domain filter
    domain_filter: String,

    /// Date filter options
    date_filter: DateFilter,

    /// Filter options state
    filter_options: FilterOptions,

    /// Cached filtered results
    filtered_entries: Vec<usize>, // indices into dsl_entries

    /// Sort configuration
    sort_by: SortBy,
    sort_ascending: bool,

    /// UI state
    show_filters: bool,
    scroll_to_selected: bool,
}

/// Available sorting options
#[derive(Debug, Clone, PartialEq)]
pub enum SortBy {
    Name,
    Domain,
    CreatedAt,
    UpdatedAt,
    Version,
}

/// Date filtering options
#[derive(Debug, Clone)]
pub struct DateFilter {
    pub enabled: bool,
    pub from_date: Option<chrono::NaiveDate>,
    pub to_date: Option<chrono::NaiveDate>,
}

impl Default for DateFilter {
    fn default() -> Self {
        Self {
            enabled: false,
            from_date: None,
            to_date: None,
        }
    }
}

impl DSLBrowserPanel {
    /// Create a new DSL browser panel
    pub fn new() -> Self {
        Self {
            dsl_entries: Vec::new(),
            selected_index: None,
            search_filter: String::new(),
            domain_filter: String::new(),
            date_filter: DateFilter::default(),
            filter_options: FilterOptions::default(),
            filtered_entries: Vec::new(),
            sort_by: SortBy::CreatedAt,
            sort_ascending: false, // Most recent first by default
            show_filters: true,
            scroll_to_selected: false,
        }
    }

    /// Update the list of DSL instances
    pub fn update_instances(&mut self, instances: Vec<DSLEntry>) {
        info!("Updating DSL instances: {} entries", instances.len());
        self.dsl_entries = instances;
        self.apply_filters();

        // Reset selection if it's out of bounds
        if let Some(selected) = self.selected_index {
            if selected >= self.filtered_entries.len() {
                self.selected_index = None;
            }
        }
    }

    /// Get the number of instances
    pub fn get_instance_count(&self) -> usize {
        self.filtered_entries.len()
    }

    /// Get the selected index
    pub fn get_selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Get the selected DSL entry
    pub fn get_selected_entry(&self) -> Option<&DSLEntry> {
        if let Some(selected_idx) = self.selected_index {
            if let Some(&entry_idx) = self.filtered_entries.get(selected_idx) {
                return self.dsl_entries.get(entry_idx);
            }
        }
        None
    }

    /// Apply current filters to the DSL entries
    fn apply_filters(&mut self) {
        debug!("Applying filters to {} entries", self.dsl_entries.len());

        self.filtered_entries = self
            .dsl_entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| self.entry_matches_filters(entry))
            .map(|(idx, _)| idx)
            .collect();

        // Apply sorting
        self.sort_entries();

        debug!("Filtered results: {} entries", self.filtered_entries.len());
    }

    /// Check if an entry matches the current filters
    fn entry_matches_filters(&self, entry: &DSLEntry) -> bool {
        // Text search filter
        if !self.search_filter.is_empty() {
            let search_lower = self.search_filter.to_lowercase();
            let matches_name = entry.name.to_lowercase().contains(&search_lower);
            let matches_description = entry
                .description
                .as_ref()
                .map(|d| d.to_lowercase().contains(&search_lower))
                .unwrap_or(false);
            let matches_preview = entry.content_preview.to_lowercase().contains(&search_lower);

            if !(matches_name || matches_description || matches_preview) {
                return false;
            }
        }

        // Domain filter
        if !self.domain_filter.is_empty() {
            if entry.domain != self.domain_filter {
                return false;
            }
        }

        // Date filter
        if self.date_filter.enabled {
            if let Some(from_date) = self.date_filter.from_date {
                if entry.created_at.date_naive() < from_date {
                    return false;
                }
            }
            if let Some(to_date) = self.date_filter.to_date {
                if entry.created_at.date_naive() > to_date {
                    return false;
                }
            }
        }

        // Version filter
        if self.filter_options.min_version.is_some() || self.filter_options.max_version.is_some() {
            if let Some(min_ver) = self.filter_options.min_version {
                if entry.version < min_ver {
                    return false;
                }
            }
            if let Some(max_ver) = self.filter_options.max_version {
                if entry.version > max_ver {
                    return false;
                }
            }
        }

        true
    }

    /// Sort the filtered entries
    fn sort_entries(&mut self) {
        self.filtered_entries.sort_by(|&a, &b| {
            let entry_a = &self.dsl_entries[a];
            let entry_b = &self.dsl_entries[b];

            let order = match self.sort_by {
                SortBy::Name => entry_a.name.cmp(&entry_b.name),
                SortBy::Domain => entry_a.domain.cmp(&entry_b.domain),
                SortBy::CreatedAt => entry_a.created_at.cmp(&entry_b.created_at),
                SortBy::UpdatedAt => entry_a.created_at.cmp(&entry_b.created_at), // Simplified
                SortBy::Version => entry_a.version.cmp(&entry_b.version),
            };

            if self.sort_ascending {
                order
            } else {
                order.reverse()
            }
        });
    }

    /// Get available domains from current entries
    fn get_available_domains(&self) -> Vec<String> {
        let mut domains: Vec<String> = self
            .dsl_entries
            .iter()
            .map(|entry| entry.domain.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        domains.sort();
        domains
    }

    /// Render the filter panel
    fn render_filters(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Filters")
            .default_open(self.show_filters)
            .show(ui, |ui| {
                // Search filter
                ui.horizontal(|ui| {
                    ui.label("Search:");
                    let response = ui.text_edit_singleline(&mut self.search_filter);
                    if response.changed() {
                        self.apply_filters();
                    }

                    if ui.button("Clear").clicked() {
                        self.search_filter.clear();
                        self.apply_filters();
                    }
                });

                // Domain filter
                ui.horizontal(|ui| {
                    ui.label("Domain:");
                    let domains = self.get_available_domains();
                    let mut changed = false;

                    egui::ComboBox::from_id_salt("domain_filter")
                        .selected_text(if self.domain_filter.is_empty() {
                            "All Domains"
                        } else {
                            &self.domain_filter
                        })
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(
                                    &mut self.domain_filter,
                                    String::new(),
                                    "All Domains",
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                            for domain in domains {
                                if ui
                                    .selectable_value(
                                        &mut self.domain_filter,
                                        domain.clone(),
                                        &domain,
                                    )
                                    .clicked()
                                {
                                    changed = true;
                                }
                            }
                        });

                    if changed {
                        self.apply_filters();
                    }
                });

                // Sort options
                ui.horizontal(|ui| {
                    ui.label("Sort by:");
                    let mut sort_changed = false;

                    egui::ComboBox::from_id_salt("sort_by")
                        .selected_text(format!("{:?}", self.sort_by))
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(&mut self.sort_by, SortBy::Name, "Name")
                                .clicked()
                            {
                                sort_changed = true;
                            }
                            if ui
                                .selectable_value(&mut self.sort_by, SortBy::Domain, "Domain")
                                .clicked()
                            {
                                sort_changed = true;
                            }
                            if ui
                                .selectable_value(&mut self.sort_by, SortBy::CreatedAt, "Created")
                                .clicked()
                            {
                                sort_changed = true;
                            }
                            if ui
                                .selectable_value(&mut self.sort_by, SortBy::Version, "Version")
                                .clicked()
                            {
                                sort_changed = true;
                            }
                        });

                    if ui.checkbox(&mut self.sort_ascending, "Ascending").clicked() {
                        sort_changed = true;
                    }

                    if sort_changed {
                        self.sort_entries();
                    }
                });

                ui.separator();

                // Statistics
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Showing {} of {} entries",
                        self.filtered_entries.len(),
                        self.dsl_entries.len()
                    ));
                });
            });
    }

    /// Render a single DSL entry
    fn render_dsl_entry(&self, ui: &mut egui::Ui, entry: &DSLEntry, is_selected: bool) -> bool {
        let mut clicked = false;

        ui.push_id(entry.id.clone(), |ui| {
            let response = ui.selectable_label(is_selected, "");

            if response.clicked() {
                clicked = true;
            }

            // Custom rendering on top of the selectable area
            let rect = response.rect;
            let painter = ui.painter();

            // Background for selected item
            if is_selected {
                painter.rect_filled(rect, 2.0, ui.visuals().selection.bg_fill);
            }

            // Content layout
            let mut content_rect = rect;
            content_rect.min.x += 8.0; // Left padding
            content_rect.max.x -= 8.0; // Right padding
            content_rect.min.y += 4.0; // Top padding
            content_rect.max.y -= 4.0; // Bottom padding

            // Name and domain
            let name_pos = content_rect.min + egui::vec2(0.0, 2.0);
            painter.text(
                name_pos,
                egui::Align2::LEFT_TOP,
                &entry.name,
                egui::FontId::proportional(DEFAULT_FONT_SIZE),
                if is_selected {
                    ui.visuals().selection.stroke.color
                } else {
                    ui.visuals().text_color()
                },
            );

            // Domain badge
            let domain_text = format!("[{}]", entry.domain);
            let domain_pos = egui::pos2(content_rect.max.x - 80.0, name_pos.y);
            painter.text(
                domain_pos,
                egui::Align2::RIGHT_TOP,
                &domain_text,
                egui::FontId::monospace(11.0),
                egui::Color32::GRAY,
            );

            // Version and date info
            let version_text = format!("v{}", entry.version);
            let date_text = entry.created_at.format("%m/%d %H:%M").to_string();
            let info_text = format!("{} • {}", version_text, date_text);
            let info_pos = name_pos + egui::vec2(0.0, 18.0);
            painter.text(
                info_pos,
                egui::Align2::LEFT_TOP,
                &info_text,
                egui::FontId::monospace(10.0),
                egui::Color32::GRAY,
            );

            // Content preview
            if !entry.content_preview.is_empty() {
                let preview_pos = info_pos + egui::vec2(0.0, 14.0);
                let preview_text = if entry.content_preview.len() > 60 {
                    format!("{}...", &entry.content_preview[..57])
                } else {
                    entry.content_preview.clone()
                };
                painter.text(
                    preview_pos,
                    egui::Align2::LEFT_TOP,
                    &preview_text,
                    egui::FontId::monospace(CODE_FONT_SIZE),
                    if is_selected {
                        ui.visuals().selection.stroke.color.gamma_multiply(0.8)
                    } else {
                        ui.visuals().text_color().gamma_multiply(0.7)
                    },
                );
            }
        });

        clicked
    }

    /// Render the main DSL browser panel
    pub fn render<F>(&mut self, ui: &mut egui::Ui, mut on_selection: F)
    where
        F: FnMut(&DSLEntry),
    {
        ui.vertical(|ui| {
            // Render filters
            self.render_filters(ui);

            ui.separator();

            // Main content area
            if self.filtered_entries.is_empty() {
                ui.centered_and_justified(|ui| {
                    if self.dsl_entries.is_empty() {
                        ui.label("No DSL instances found.\nTry refreshing or check your connection.");
                    } else {
                        ui.label("No entries match the current filters.\nTry adjusting your search criteria.");
                    }
                });
                return;
            }

            // DSL entries list
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;

                    for (display_idx, &entry_idx) in self.filtered_entries.iter().enumerate() {
                        if let Some(entry) = self.dsl_entries.get(entry_idx) {
                            let is_selected = self.selected_index == Some(display_idx);

                            // Calculate height for this entry
                            let entry_height = if entry.content_preview.is_empty() {
                                44.0
                            } else {
                                62.0
                            };

                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), entry_height),
                                egui::Sense::click()
                            );

                            // Handle selection
                            if response.clicked() {
                                debug!("DSL entry selected: {} ({})", entry.name, entry.id);
                                self.selected_index = Some(display_idx);
                                on_selection(entry);
                            }

                            // Custom rendering
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
                                self.render_dsl_entry(ui, entry, is_selected);
                            });
                        }
                    }
                });

            // Status bar
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} entries • {} selected",
                    self.filtered_entries.len(),
                    if self.selected_index.is_some() { "1" } else { "0" }
                ));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("↻ Refresh").clicked() {
                        // Refresh would be handled by parent
                        info!("Refresh requested from DSL browser");
                    }
                });
            });
        });
    }
}

impl Default for DSLBrowserPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    fn create_test_entry(id: &str, name: &str, domain: &str, version: i32) -> DSLEntry {
        DSLEntry {
            id: id.to_string(),
            name: name.to_string(),
            domain: domain.to_string(),
            created_at: Utc::now(),
            version,
            description: Some(format!("Test entry for {}", name)),
            content_preview: format!("(test.operation :id \"{}\")", id),
        }
    }

    #[test]
    fn test_dsl_browser_panel_creation() {
        let panel = DSLBrowserPanel::new();
        assert_eq!(panel.get_instance_count(), 0);
        assert_eq!(panel.get_selected_index(), None);
        assert!(panel.show_filters);
    }

    #[test]
    fn test_update_instances() {
        let mut panel = DSLBrowserPanel::new();
        let entries = vec![
            create_test_entry("1", "Test 1", "onboarding", 1),
            create_test_entry("2", "Test 2", "kyc", 2),
        ];

        panel.update_instances(entries);
        assert_eq!(panel.get_instance_count(), 2);
    }

    #[test]
    fn test_search_filter() {
        let mut panel = DSLBrowserPanel::new();
        let entries = vec![
            create_test_entry("1", "Onboarding Test", "onboarding", 1),
            create_test_entry("2", "KYC Test", "kyc", 2),
        ];

        panel.update_instances(entries);
        assert_eq!(panel.get_instance_count(), 2);

        panel.search_filter = "onboarding".to_string();
        panel.apply_filters();
        assert_eq!(panel.get_instance_count(), 1);
    }

    #[test]
    fn test_domain_filter() {
        let mut panel = DSLBrowserPanel::new();
        let entries = vec![
            create_test_entry("1", "Test 1", "onboarding", 1),
            create_test_entry("2", "Test 2", "kyc", 2),
            create_test_entry("3", "Test 3", "onboarding", 3),
        ];

        panel.update_instances(entries);
        assert_eq!(panel.get_instance_count(), 3);

        panel.domain_filter = "onboarding".to_string();
        panel.apply_filters();
        assert_eq!(panel.get_instance_count(), 2);
    }

    #[test]
    fn test_available_domains() {
        let mut panel = DSLBrowserPanel::new();
        let entries = vec![
            create_test_entry("1", "Test 1", "onboarding", 1),
            create_test_entry("2", "Test 2", "kyc", 2),
            create_test_entry("3", "Test 3", "compliance", 3),
            create_test_entry("4", "Test 4", "kyc", 4), // Duplicate domain
        ];

        panel.update_instances(entries);
        let domains = panel.get_available_domains();

        assert_eq!(domains.len(), 3);
        assert!(domains.contains(&"onboarding".to_string()));
        assert!(domains.contains(&"kyc".to_string()));
        assert!(domains.contains(&"compliance".to_string()));
    }
}
