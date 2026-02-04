//! Inspector Panel - Tree/Table-based visualization for CBU data
//!
//! Replaces 3D Astronomy/ESPER with a deterministic tree/table UI.
//! Uses InspectorProjection for data, follows egui state management rules:
//!
//! 1. NO local state mirroring server data - uses AppState.inspector_projection
//! 2. Actions return values, no callbacks
//! 3. Read state, render, return action
//! 4. Server round-trip for data changes
//!
//! Layout:
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │ [Search] [LOD: ▼] [Depth: ▼]                                       │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │ Breadcrumb: CBU > Members > Fund: Allianz IE ETF SICAV              │
//! ├───────────────────┬─────────────────────────┬───────────────────────┤
//! │ Navigation Tree   │ Main View               │ Detail Pane           │
//! │ (30%)             │ (45%)                   │ (25%)                 │
//! └───────────────────┴─────────────────────────┴───────────────────────┘

use egui::{Color32, RichText, ScrollArea, Ui};
use egui_extras::{Column, TableBuilder};
use inspector_projection::{InspectorProjection, Node, NodeId, NodeKind, RefOrList, RenderPolicy};
use std::collections::HashSet;

// ============================================================================
// ACTIONS (returned from panel, handled by app.rs)
// ============================================================================

/// Actions returned from inspector panel interactions
#[derive(Debug, Clone)]
pub enum InspectorAction {
    /// No action
    None,
    /// User focused on a node (push to focus stack)
    FocusNode { node_id: NodeId },
    /// User wants to go back in focus stack
    GoBack,
    /// User toggled expand/collapse on a tree node
    ToggleExpand { node_id: NodeId },
    /// User selected a node (show in detail pane)
    SelectNode { node_id: NodeId },
    /// User changed LOD level
    SetLod { lod: u8 },
    /// User changed max depth
    SetMaxDepth { depth: u8 },
    /// User wants to load a page of items
    LoadPage {
        list_ref: NodeId,
        next_token: NodeId,
    },
    /// User wants to refresh the projection
    Refresh,
    /// User cleared selection
    ClearSelection,
    /// User searched
    Search { query: String },
    /// User toggled a chamber visibility
    ToggleChamber { chamber: String },
    /// User toggled a node kind filter
    ToggleNodeKind { kind: String },
    /// User set confidence threshold filter
    SetConfidenceThreshold { threshold: f32 },
    /// User reset all filters to defaults
    ResetFilters,
}

// ============================================================================
// UI STATE (in AppState, not local)
// ============================================================================

/// Known chambers for filtering
pub const KNOWN_CHAMBERS: &[&str] = &["members", "products", "matrix", "registers"];

/// Known node kinds for filtering
pub const KNOWN_NODE_KINDS: &[&str] = &[
    "Entity",
    "Product",
    "Service",
    "Resource",
    "HoldingEdge",
    "ControlEdge",
];

/// Inspector panel UI state (stored in AppState)
#[derive(Default, Clone)]
pub struct InspectorState {
    /// Expanded nodes in tree view
    pub expanded: HashSet<NodeId>,
    /// Currently selected node (shown in detail pane)
    pub selected: Option<NodeId>,
    /// Focus stack for navigation (back button)
    pub focus_stack: Vec<NodeId>,
    /// LOD override (None = use projection's policy)
    pub lod_override: Option<u8>,
    /// Max depth override
    pub max_depth_override: Option<u8>,
    /// Search query
    pub search_query: String,
    /// Enabled chambers (empty = all enabled)
    pub enabled_chambers: HashSet<String>,
    /// Enabled node kinds (empty = all enabled)
    pub enabled_node_kinds: HashSet<String>,
    /// Minimum confidence threshold (0.0 = show all)
    pub confidence_threshold: f32,
}

impl InspectorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get current focus node (top of stack)
    pub fn current_focus(&self) -> Option<&NodeId> {
        self.focus_stack.last()
    }

    /// Push a focus node
    pub fn push_focus(&mut self, node_id: NodeId) {
        // Don't push duplicates
        if self.focus_stack.last() != Some(&node_id) {
            self.focus_stack.push(node_id);
        }
    }

    /// Pop focus (go back)
    pub fn pop_focus(&mut self) -> Option<NodeId> {
        if self.focus_stack.len() > 1 {
            self.focus_stack.pop()
        } else {
            None // Don't pop the last item
        }
    }

    /// Toggle node expansion
    pub fn toggle_expand(&mut self, node_id: &NodeId) {
        if self.expanded.contains(node_id) {
            self.expanded.remove(node_id);
        } else {
            self.expanded.insert(node_id.clone());
        }
    }

    /// Check if node is expanded
    pub fn is_expanded(&self, node_id: &NodeId) -> bool {
        self.expanded.contains(node_id)
    }

    /// Get effective LOD (override or from policy)
    pub fn effective_lod(&self, policy: &RenderPolicy) -> u8 {
        self.lod_override.unwrap_or(policy.lod)
    }

    /// Check if a chamber is enabled (empty set = all enabled)
    pub fn is_chamber_enabled(&self, chamber: &str) -> bool {
        self.enabled_chambers.is_empty() || self.enabled_chambers.contains(chamber)
    }

    /// Toggle chamber visibility
    pub fn toggle_chamber(&mut self, chamber: &str) {
        if self.enabled_chambers.contains(chamber) {
            self.enabled_chambers.remove(chamber);
        } else {
            // If going from "all enabled" to selective, enable all except toggled
            if self.enabled_chambers.is_empty() {
                for c in KNOWN_CHAMBERS {
                    if *c != chamber {
                        self.enabled_chambers.insert(c.to_string());
                    }
                }
            } else {
                self.enabled_chambers.insert(chamber.to_string());
            }
        }
    }

    /// Check if a node kind is enabled (empty set = all enabled)
    pub fn is_node_kind_enabled(&self, kind: &str) -> bool {
        self.enabled_node_kinds.is_empty() || self.enabled_node_kinds.contains(kind)
    }

    /// Toggle node kind filter
    pub fn toggle_node_kind(&mut self, kind: &str) {
        if self.enabled_node_kinds.contains(kind) {
            self.enabled_node_kinds.remove(kind);
        } else {
            // If going from "all enabled" to selective, enable all except toggled
            if self.enabled_node_kinds.is_empty() {
                for k in KNOWN_NODE_KINDS {
                    if *k != kind {
                        self.enabled_node_kinds.insert(k.to_string());
                    }
                }
            } else {
                self.enabled_node_kinds.insert(kind.to_string());
            }
        }
    }

    /// Reset all filters to defaults
    pub fn reset_filters(&mut self) {
        self.enabled_chambers.clear();
        self.enabled_node_kinds.clear();
        self.confidence_threshold = 0.0;
    }
}

// ============================================================================
// MAIN PANEL RENDER
// ============================================================================

/// Render the inspector panel
///
/// Returns an action if the user interacted with the panel.
/// The caller (app.rs) handles the action.
pub fn inspector_panel(
    ui: &mut Ui,
    projection: Option<&InspectorProjection>,
    state: &InspectorState,
) -> InspectorAction {
    // Handle missing projection
    let Some(projection) = projection else {
        ui.centered_and_justified(|ui| {
            ui.label("No projection loaded. Select a CBU to view.");
        });
        return InspectorAction::None;
    };

    let mut action = InspectorAction::None;

    // Toolbar
    let toolbar_action = render_toolbar(ui, projection, state);
    if !matches!(toolbar_action, InspectorAction::None) {
        action = toolbar_action;
    }

    ui.separator();

    // Breadcrumbs
    let breadcrumb_action = render_breadcrumbs(ui, projection, state);
    if !matches!(breadcrumb_action, InspectorAction::None) {
        action = breadcrumb_action;
    }

    ui.separator();

    // Three-panel layout
    let available = ui.available_size();
    let tree_width = available.x * 0.30;
    let main_width = available.x * 0.45;
    let detail_width = available.x * 0.25 - 16.0; // Account for separators

    ui.horizontal(|ui| {
        // Left: Navigation Tree
        ui.allocate_ui_with_layout(
            egui::vec2(tree_width, available.y),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                let tree_action = render_tree_panel(ui, projection, state);
                if !matches!(tree_action, InspectorAction::None) {
                    action = tree_action;
                }
            },
        );

        ui.separator();

        // Middle: Main View (depends on focused node kind)
        ui.allocate_ui_with_layout(
            egui::vec2(main_width, available.y),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                let main_action = render_main_panel(ui, projection, state);
                if !matches!(main_action, InspectorAction::None) {
                    action = main_action;
                }
            },
        );

        ui.separator();

        // Right: Detail Pane
        ui.allocate_ui_with_layout(
            egui::vec2(detail_width, available.y),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                let detail_action = render_detail_panel(ui, projection, state);
                if !matches!(detail_action, InspectorAction::None) {
                    action = detail_action;
                }
            },
        );
    });

    action
}

// ============================================================================
// TOOLBAR
// ============================================================================

fn render_toolbar(
    ui: &mut Ui,
    projection: &InspectorProjection,
    state: &InspectorState,
) -> InspectorAction {
    let mut action = InspectorAction::None;

    ui.horizontal(|ui| {
        // Search box (read-only display, actual searching happens via action)
        ui.label("🔍");
        let search_response = ui.text_edit_singleline(&mut state.search_query.clone());
        if search_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            action = InspectorAction::Search {
                query: state.search_query.clone(),
            };
        }

        ui.separator();

        // LOD selector
        let current_lod = state.effective_lod(&projection.render_policy);
        ui.label("LOD:");
        egui::ComboBox::from_id_salt("lod_selector")
            .selected_text(format!("{}", current_lod))
            .show_ui(ui, |ui| {
                for lod in 0..=3 {
                    let label = match lod {
                        0 => "0 - Glyph only",
                        1 => "1 - Labels",
                        2 => "2 - Summary",
                        3 => "3 - Full detail",
                        _ => "Unknown",
                    };
                    if ui.selectable_label(current_lod == lod, label).clicked() {
                        action = InspectorAction::SetLod { lod };
                    }
                }
            });

        ui.separator();

        // Depth selector
        let current_depth = state
            .max_depth_override
            .unwrap_or(projection.render_policy.max_depth);
        ui.label("Depth:");
        egui::ComboBox::from_id_salt("depth_selector")
            .selected_text(format!("{}", current_depth))
            .show_ui(ui, |ui| {
                for depth in 1..=10 {
                    if ui
                        .selectable_label(current_depth == depth, format!("{}", depth))
                        .clicked()
                    {
                        action = InspectorAction::SetMaxDepth { depth };
                    }
                }
            });

        ui.separator();

        // Chamber toggles dropdown
        egui::ComboBox::from_id_salt("chamber_filter")
            .selected_text("Chambers")
            .show_ui(ui, |ui| {
                for chamber in KNOWN_CHAMBERS {
                    let enabled = state.is_chamber_enabled(chamber);
                    let label = format!(
                        "{} {}",
                        if enabled { "✓" } else { "○" },
                        chamber_display_name(chamber)
                    );
                    if ui.selectable_label(enabled, label).clicked() {
                        action = InspectorAction::ToggleChamber {
                            chamber: chamber.to_string(),
                        };
                    }
                }
            });

        ui.separator();

        // Filter toggles dropdown
        egui::ComboBox::from_id_salt("kind_filter")
            .selected_text("Filters")
            .show_ui(ui, |ui| {
                ui.label(RichText::new("Node Types").strong());
                for kind in KNOWN_NODE_KINDS {
                    let enabled = state.is_node_kind_enabled(kind);
                    let label = format!("{} {}", if enabled { "✓" } else { "○" }, kind);
                    if ui.selectable_label(enabled, label).clicked() {
                        action = InspectorAction::ToggleNodeKind {
                            kind: kind.to_string(),
                        };
                    }
                }

                ui.separator();
                ui.label(RichText::new("Confidence").strong());

                // Confidence threshold slider
                let thresholds = [0.0, 0.5, 0.7, 0.9];
                for threshold in thresholds {
                    let label = if threshold == 0.0 {
                        "All".to_string()
                    } else {
                        format!("≥ {:.0}%", threshold * 100.0)
                    };
                    let is_selected = (state.confidence_threshold - threshold).abs() < 0.01;
                    if ui.selectable_label(is_selected, label).clicked() {
                        action = InspectorAction::SetConfidenceThreshold { threshold };
                    }
                }

                ui.separator();
                if ui.button("Reset All Filters").clicked() {
                    action = InspectorAction::ResetFilters;
                }
            });

        ui.separator();

        // Refresh button
        if ui.button("⟳ Refresh").clicked() {
            action = InspectorAction::Refresh;
        }
    });

    action
}

/// Get display name for chamber
fn chamber_display_name(chamber: &str) -> &str {
    match chamber {
        "members" => "Members",
        "products" => "Products",
        "matrix" => "Matrix",
        "registers" => "Registers",
        _ => chamber,
    }
}

// ============================================================================
// BREADCRUMBS
// ============================================================================

fn render_breadcrumbs(
    ui: &mut Ui,
    projection: &InspectorProjection,
    state: &InspectorState,
) -> InspectorAction {
    let mut action = InspectorAction::None;

    ui.horizontal(|ui| {
        // Back button
        let can_go_back = state.focus_stack.len() > 1;
        if ui
            .add_enabled(can_go_back, egui::Button::new("←"))
            .on_hover_text("Go back")
            .clicked()
        {
            action = InspectorAction::GoBack;
        }

        ui.separator();

        // Breadcrumb trail
        for (i, node_id) in state.focus_stack.iter().enumerate() {
            let is_last = i == state.focus_stack.len() - 1;

            // Get node label
            let label = projection
                .get_node(node_id)
                .map(|n| n.label_short.as_str())
                .unwrap_or(node_id.as_str());

            if is_last {
                // Current node - not clickable, bold
                ui.label(RichText::new(label).strong());
            } else {
                // Clickable breadcrumb
                if ui.link(label).clicked() {
                    action = InspectorAction::FocusNode {
                        node_id: node_id.clone(),
                    };
                }
                ui.label(RichText::new(" › ").color(Color32::GRAY));
            }
        }
    });

    action
}

// ============================================================================
// TREE PANEL (Left)
// ============================================================================

fn render_tree_panel(
    ui: &mut Ui,
    projection: &InspectorProjection,
    state: &InspectorState,
) -> InspectorAction {
    let mut action = InspectorAction::None;

    ui.heading("Navigation");
    ui.separator();

    ScrollArea::vertical()
        .id_salt("inspector_tree")
        .show(ui, |ui| {
            // Start from roots
            for (chamber_name, root_ref) in &projection.root {
                let tree_action =
                    render_tree_node(ui, projection, state, root_ref.target(), 0, chamber_name);
                if !matches!(tree_action, InspectorAction::None) {
                    action = tree_action;
                }
            }
        });

    action
}

fn render_tree_node(
    ui: &mut Ui,
    projection: &InspectorProjection,
    state: &InspectorState,
    node_id: &NodeId,
    depth: usize,
    label_override: &str,
) -> InspectorAction {
    let mut action = InspectorAction::None;

    let Some(node) = projection.get_node(node_id) else {
        // Dangling ref - show error
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 16.0);
            ui.label(RichText::new("⚠ Missing node").color(Color32::RED));
        });
        return action;
    };

    let is_expanded = state.is_expanded(node_id);
    let is_selected = state.selected.as_ref() == Some(node_id);
    let has_children = !node.branches.is_empty();

    // Indent based on depth
    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 16.0);

        // Expand/collapse toggle
        if has_children {
            let toggle_text = if is_expanded { "▼" } else { "▶" };
            if ui.small_button(toggle_text).clicked() {
                action = InspectorAction::ToggleExpand {
                    node_id: node_id.clone(),
                };
            }
        } else {
            ui.add_space(20.0); // Align with toggle button
        }

        // Glyph
        let glyph = node.glyph.as_deref().unwrap_or(node.kind.default_glyph());
        ui.label(glyph);

        // Label (clickable to select)
        let label = if label_override.is_empty() {
            &node.label_short
        } else {
            label_override
        };

        let label_text = if is_selected {
            RichText::new(label).strong().color(Color32::LIGHT_BLUE)
        } else {
            RichText::new(label)
        };

        if ui.link(label_text).clicked() {
            action = InspectorAction::SelectNode {
                node_id: node_id.clone(),
            };
        }

        // Double-click to focus
        if ui.input(|i| i.pointer.any_down()) && ui.rect_contains_pointer(ui.min_rect()) {
            // Note: egui doesn't have built-in double-click, would need to track timing
            // For now, use a "focus" button
        }
    });

    // Render children if expanded
    if is_expanded && has_children {
        for (branch_name, ref_or_list) in &node.branches {
            match ref_or_list {
                RefOrList::Single(ref_val) => {
                    let child_action =
                        render_tree_node(ui, projection, state, ref_val.target(), depth + 1, "");
                    if !matches!(child_action, InspectorAction::None) {
                        action = child_action;
                    }
                }
                RefOrList::List(paging_list) => {
                    // Show branch name as a label
                    ui.horizontal(|ui| {
                        ui.add_space((depth + 1) as f32 * 16.0);
                        ui.label(RichText::new(format!("{}:", branch_name)).color(Color32::GRAY));
                    });

                    // Render each item in the list
                    for ref_val in &paging_list.items {
                        let child_action = render_tree_node(
                            ui,
                            projection,
                            state,
                            ref_val.target(),
                            depth + 2,
                            "",
                        );
                        if !matches!(child_action, InspectorAction::None) {
                            action = child_action;
                        }
                    }

                    // Show "Load more" if there's a next page
                    if let Some(next) = &paging_list.paging.next {
                        ui.horizontal(|ui| {
                            ui.add_space((depth + 2) as f32 * 16.0);
                            if ui.small_button("Load more...").clicked() {
                                action = InspectorAction::LoadPage {
                                    list_ref: node_id.clone(),
                                    next_token: next.clone(),
                                };
                            }
                        });
                    }
                }
            }
        }
    }

    action
}

// ============================================================================
// MAIN PANEL (Middle)
// ============================================================================

fn render_main_panel(
    ui: &mut Ui,
    projection: &InspectorProjection,
    state: &InspectorState,
) -> InspectorAction {
    let mut action = InspectorAction::None;

    // Get focused node (or show root summary)
    let focused_node = state.current_focus().and_then(|id| projection.get_node(id));

    ui.heading("Main View");
    ui.separator();

    ScrollArea::vertical()
        .id_salt("inspector_main")
        .show(ui, |ui| {
            if let Some(node) = focused_node {
                let main_action = render_node_main_view(ui, projection, state, node);
                if !matches!(main_action, InspectorAction::None) {
                    action = main_action;
                }
            } else {
                // Show root summary
                ui.label("Select a node from the tree to view details.");
                ui.separator();

                // Show root nodes as cards
                for (chamber_name, root_ref) in &projection.root {
                    if let Some(root_node) = projection.get_node(root_ref.target()) {
                        render_node_card(ui, root_node, chamber_name);
                    }
                }
            }
        });

    action
}

fn render_node_main_view(
    ui: &mut Ui,
    projection: &InspectorProjection,
    _state: &InspectorState,
    node: &Node,
) -> InspectorAction {
    // Show different views based on node kind
    match node.kind {
        NodeKind::MatrixSlice | NodeKind::InstrumentMatrix => {
            render_matrix_view(ui, projection, node)
        }
        NodeKind::HoldingEdgeList | NodeKind::InvestorRegister => {
            render_holding_list_view(ui, projection, node)
        }
        NodeKind::ControlTree | NodeKind::ControlRegister => {
            render_control_tree_view(ui, projection, node)
        }
        _ => {
            // Default: show node info and branches as cards
            render_node_card(ui, node, "");
            ui.separator();

            // Show children as cards
            for (branch_name, ref_or_list) in &node.branches {
                ui.label(RichText::new(branch_name).strong());
                match ref_or_list {
                    RefOrList::Single(ref_val) => {
                        if let Some(child) = projection.get_node(ref_val.target()) {
                            render_node_card(ui, child, "");
                        }
                    }
                    RefOrList::List(list) => {
                        for ref_val in &list.items {
                            if let Some(child) = projection.get_node(ref_val.target()) {
                                render_node_card(ui, child, "");
                            }
                        }
                    }
                }
                ui.add_space(8.0);
            }
            InspectorAction::None
        }
    }
}

fn render_node_card(ui: &mut Ui, node: &Node, label_override: &str) {
    egui::Frame::none()
        .fill(Color32::from_gray(40))
        .rounding(4.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Glyph
                let glyph = node.glyph.as_deref().unwrap_or(node.kind.default_glyph());
                ui.label(RichText::new(glyph).size(24.0));

                ui.vertical(|ui| {
                    // Label
                    let label = if label_override.is_empty() {
                        &node.label_short
                    } else {
                        label_override
                    };
                    ui.label(RichText::new(label).strong());

                    // Full label if different
                    if let Some(full) = &node.label_full {
                        if full != label {
                            ui.label(RichText::new(full).color(Color32::GRAY));
                        }
                    }

                    // Summary
                    if let Some(summary) = &node.summary {
                        ui.label(format!("{} items", summary.item_count));
                    }
                });
            });
        });
    ui.add_space(4.0);
}

fn render_matrix_view(
    ui: &mut Ui,
    projection: &InspectorProjection,
    node: &Node,
) -> InspectorAction {
    let mut action = InspectorAction::None;

    // Header with node info
    ui.horizontal(|ui| {
        let glyph = node.glyph.as_deref().unwrap_or(node.kind.default_glyph());
        ui.label(RichText::new(glyph).size(20.0));
        ui.label(RichText::new(&node.label_short).strong().size(16.0));
        if let Some(full) = &node.label_full {
            ui.label(RichText::new(format!("({})", full)).color(Color32::GRAY));
        }
    });

    ui.separator();

    // Collect slices from branches
    let mut slices: Vec<(&str, &Node)> = Vec::new();
    for (branch_name, ref_or_list) in &node.branches {
        match ref_or_list {
            RefOrList::Single(ref_val) => {
                if let Some(child) = projection.get_node(ref_val.target()) {
                    if matches!(child.kind, NodeKind::MatrixSlice) {
                        slices.push((branch_name.as_str(), child));
                    }
                }
            }
            RefOrList::List(paging_list) => {
                for ref_val in &paging_list.items {
                    if let Some(child) = projection.get_node(ref_val.target()) {
                        if matches!(child.kind, NodeKind::MatrixSlice) {
                            slices.push((branch_name.as_str(), child));
                        }
                    }
                }
            }
        }
    }

    if slices.is_empty() {
        ui.label("No matrix slices found.");
        return action;
    }

    // Determine columns based on attributes present in slices
    let mut column_keys: Vec<String> = Vec::new();
    for (_, slice) in &slices {
        for key in slice.attributes.keys() {
            if !column_keys.contains(key) {
                column_keys.push(key.clone());
            }
        }
    }
    // Sort for consistent ordering
    column_keys.sort();

    // Render table
    let available_height = ui.available_height().min(400.0);

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto().at_least(60.0)) // Label column
        .columns(Column::auto().at_least(80.0), column_keys.len()) // Attribute columns
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("Market");
            });
            for key in &column_keys {
                header.col(|ui| {
                    ui.strong(format_column_header(key));
                });
            }
        })
        .body(|mut body| {
            for (_, slice) in &slices {
                body.row(24.0, |mut row| {
                    // Label column
                    row.col(|ui| {
                        let glyph = slice.glyph.as_deref().unwrap_or("📊");
                        ui.horizontal(|ui| {
                            ui.label(glyph);
                            ui.label(&slice.label_short);
                        });
                    });

                    // Attribute columns
                    for key in &column_keys {
                        row.col(|ui| {
                            if let Some(value) = slice.attributes.get(key) {
                                let display = format_attribute_value(value);
                                // Color-code certain values
                                let text = if key == "status" {
                                    match display.as_str() {
                                        "ACTIVE" | "active" => {
                                            RichText::new(&display).color(Color32::GREEN)
                                        }
                                        "INACTIVE" | "inactive" => {
                                            RichText::new(&display).color(Color32::RED)
                                        }
                                        "PENDING" | "pending" => {
                                            RichText::new(&display).color(Color32::YELLOW)
                                        }
                                        _ => RichText::new(&display),
                                    }
                                } else {
                                    RichText::new(&display)
                                };
                                ui.label(text);
                            } else {
                                ui.label(RichText::new("-").color(Color32::DARK_GRAY));
                            }
                        });
                    }
                });
            }
        });

    // Show paging info if present
    for ref_or_list in node.branches.values() {
        if let RefOrList::List(paging_list) = ref_or_list {
            if let Some(next_token) = &paging_list.paging.next {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Showing {} of {} items",
                        paging_list.items.len(),
                        paging_list.paging.total.unwrap_or(paging_list.items.len())
                    ));
                    if ui.button("Load more...").clicked() {
                        action = InspectorAction::LoadPage {
                            list_ref: node.id.clone(),
                            next_token: next_token.clone(),
                        };
                    }
                });
            }
        }
    }

    action
}

/// Format column header from snake_case to Title Case
fn format_column_header(key: &str) -> String {
    key.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format JSON value for display
fn format_attribute_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => if *b { "Yes" } else { "No" }.to_string(),
        serde_json::Value::Null => "-".to_string(),
        serde_json::Value::Array(arr) => format!("[{} items]", arr.len()),
        serde_json::Value::Object(obj) => format!("{{{} keys}}", obj.len()),
    }
}

fn render_holding_list_view(
    ui: &mut Ui,
    projection: &InspectorProjection,
    node: &Node,
) -> InspectorAction {
    let mut action = InspectorAction::None;

    // Header
    ui.horizontal(|ui| {
        let glyph = node.glyph.as_deref().unwrap_or(node.kind.default_glyph());
        ui.label(RichText::new(glyph).size(20.0));
        ui.label(RichText::new(&node.label_short).strong().size(16.0));
    });

    ui.separator();

    // Collect holding edges from branches
    let mut holdings: Vec<&Node> = Vec::new();
    for ref_or_list in node.branches.values() {
        match ref_or_list {
            RefOrList::Single(ref_val) => {
                if let Some(child) = projection.get_node(ref_val.target()) {
                    if matches!(child.kind, NodeKind::HoldingEdge) {
                        holdings.push(child);
                    }
                }
            }
            RefOrList::List(paging_list) => {
                for ref_val in &paging_list.items {
                    if let Some(child) = projection.get_node(ref_val.target()) {
                        if matches!(child.kind, NodeKind::HoldingEdge) {
                            holdings.push(child);
                        }
                    }
                }

                // Show paging if available
                if let Some(next_token) = &paging_list.paging.next {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Showing {} of {} holdings",
                            paging_list.items.len(),
                            paging_list.paging.total.unwrap_or(paging_list.items.len())
                        ));
                        if ui.button("Load more...").clicked() {
                            action = InspectorAction::LoadPage {
                                list_ref: node.id.clone(),
                                next_token: next_token.clone(),
                            };
                        }
                    });
                }
            }
        }
    }

    if holdings.is_empty() {
        ui.label("No holdings found.");
        return action;
    }

    // Render holdings table
    let available_height = ui.available_height().min(400.0);

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto().at_least(120.0)) // Holder
        .column(Column::auto().at_least(80.0)) // Percentage
        .column(Column::auto().at_least(80.0)) // Type
        .column(Column::auto().at_least(80.0)) // Confidence
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("Holder");
            });
            header.col(|ui| {
                ui.strong("Holding %");
            });
            header.col(|ui| {
                ui.strong("Type");
            });
            header.col(|ui| {
                ui.strong("Confidence");
            });
        })
        .body(|mut body| {
            for holding in &holdings {
                body.row(24.0, |mut row| {
                    // Holder name
                    row.col(|ui| {
                        let glyph = holding.glyph.as_deref().unwrap_or("👤");
                        ui.horizontal(|ui| {
                            ui.label(glyph);
                            ui.label(&holding.label_short);
                        });
                    });

                    // Holding percentage
                    row.col(|ui| {
                        if let Some(pct) = holding.attributes.get("holding_pct") {
                            let pct_val = pct.as_f64().unwrap_or(0.0);
                            ui.label(format!("{:.1}%", pct_val));
                        } else {
                            ui.label("-");
                        }
                    });

                    // Holding type
                    row.col(|ui| {
                        if let Some(htype) = holding.attributes.get("holding_type") {
                            ui.label(format_attribute_value(htype));
                        } else {
                            ui.label("-");
                        }
                    });

                    // Confidence from provenance
                    row.col(|ui| {
                        if let Some(prov) = &holding.provenance {
                            if let Some(conf) = prov.confidence {
                                let color = if conf >= 0.9 {
                                    Color32::GREEN
                                } else if conf >= 0.7 {
                                    Color32::YELLOW
                                } else {
                                    Color32::RED
                                };
                                ui.label(
                                    RichText::new(format!("{:.0}%", conf * 100.0)).color(color),
                                );
                            } else {
                                ui.label("-");
                            }
                        } else {
                            ui.label(RichText::new("?").color(Color32::DARK_GRAY));
                        }
                    });
                });
            }
        });

    action
}

fn render_control_tree_view(
    ui: &mut Ui,
    projection: &InspectorProjection,
    node: &Node,
) -> InspectorAction {
    let mut action = InspectorAction::None;

    // Header
    ui.horizontal(|ui| {
        let glyph = node.glyph.as_deref().unwrap_or(node.kind.default_glyph());
        ui.label(RichText::new(glyph).size(20.0));
        ui.label(RichText::new(&node.label_short).strong().size(16.0));
    });

    ui.separator();

    // Collect control edges from branches
    let mut edges: Vec<&Node> = Vec::new();
    for ref_or_list in node.branches.values() {
        match ref_or_list {
            RefOrList::Single(ref_val) => {
                if let Some(child) = projection.get_node(ref_val.target()) {
                    if matches!(child.kind, NodeKind::ControlEdge | NodeKind::ControlNode) {
                        edges.push(child);
                    }
                }
            }
            RefOrList::List(paging_list) => {
                for ref_val in &paging_list.items {
                    if let Some(child) = projection.get_node(ref_val.target()) {
                        if matches!(child.kind, NodeKind::ControlEdge | NodeKind::ControlNode) {
                            edges.push(child);
                        }
                    }
                }

                // Show paging if available
                if let Some(next_token) = &paging_list.paging.next {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Showing {} of {} nodes",
                            paging_list.items.len(),
                            paging_list.paging.total.unwrap_or(paging_list.items.len())
                        ));
                        if ui.button("Load more...").clicked() {
                            action = InspectorAction::LoadPage {
                                list_ref: node.id.clone(),
                                next_token: next_token.clone(),
                            };
                        }
                    });
                }
            }
        }
    }

    if edges.is_empty() {
        ui.label("No control relationships found.");
        return action;
    }

    // Render control tree table
    let available_height = ui.available_height().min(400.0);

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto().at_least(150.0)) // Entity
        .column(Column::auto().at_least(80.0)) // Control %
        .column(Column::auto().at_least(80.0)) // Voting %
        .column(Column::auto().at_least(80.0)) // Confidence
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("Controller");
            });
            header.col(|ui| {
                ui.strong("Control %");
            });
            header.col(|ui| {
                ui.strong("Voting %");
            });
            header.col(|ui| {
                ui.strong("Confidence");
            });
        })
        .body(|mut body| {
            for edge in &edges {
                body.row(24.0, |mut row| {
                    // Controller name
                    row.col(|ui| {
                        let glyph = edge.glyph.as_deref().unwrap_or("🏢");
                        ui.horizontal(|ui| {
                            ui.label(glyph);
                            ui.label(&edge.label_short);
                        });
                    });

                    // Control percentage
                    row.col(|ui| {
                        if let Some(pct) = edge.attributes.get("control_pct") {
                            let pct_val = pct.as_f64().unwrap_or(0.0);
                            ui.label(format!("{:.1}%", pct_val));
                        } else {
                            ui.label("-");
                        }
                    });

                    // Voting percentage
                    row.col(|ui| {
                        if let Some(pct) = edge.attributes.get("voting_pct") {
                            let pct_val = pct.as_f64().unwrap_or(0.0);
                            ui.label(format!("{:.1}%", pct_val));
                        } else {
                            ui.label("-");
                        }
                    });

                    // Confidence from provenance
                    row.col(|ui| {
                        if let Some(prov) = &edge.provenance {
                            if let Some(conf) = prov.confidence {
                                let color = if conf >= 0.9 {
                                    Color32::GREEN
                                } else if conf >= 0.7 {
                                    Color32::YELLOW
                                } else {
                                    Color32::RED
                                };
                                ui.label(
                                    RichText::new(format!("{:.0}%", conf * 100.0)).color(color),
                                );
                            } else {
                                ui.label("-");
                            }
                        } else {
                            ui.label(RichText::new("?").color(Color32::DARK_GRAY));
                        }
                    });
                });
            }
        });

    action
}

// ============================================================================
// DETAIL PANEL (Right)
// ============================================================================

fn render_detail_panel(
    ui: &mut Ui,
    projection: &InspectorProjection,
    state: &InspectorState,
) -> InspectorAction {
    let mut action = InspectorAction::None;

    ui.heading("Details");
    ui.separator();

    let selected_node = state
        .selected
        .as_ref()
        .and_then(|id| projection.get_node(id));

    ScrollArea::vertical()
        .id_salt("inspector_detail")
        .show(ui, |ui| {
            if let Some(node) = selected_node {
                render_node_detail(ui, projection, state, node);
            } else {
                ui.label("Select a node to view details.");
            }
        });

    // Clear selection button
    if state.selected.is_some() {
        ui.separator();
        if ui.button("Clear Selection").clicked() {
            action = InspectorAction::ClearSelection;
        }
    }

    action
}

fn render_node_detail(
    ui: &mut Ui,
    projection: &InspectorProjection,
    state: &InspectorState,
    node: &Node,
) {
    let policy = &projection.render_policy;
    let lod = state.effective_lod(policy);

    // Always show: kind and ID
    ui.horizontal(|ui| {
        ui.label(RichText::new("Kind:").color(Color32::GRAY));
        ui.label(format!("{:?}", node.kind));
    });

    ui.horizontal(|ui| {
        ui.label(RichText::new("ID:").color(Color32::GRAY));
        ui.label(RichText::new(node.id.as_str()).monospace());
    });

    // LOD 1+: labels
    if lod >= 1 {
        ui.separator();
        ui.label(RichText::new(node.label_short.as_str()).strong().size(16.0));
        if let Some(full) = &node.label_full {
            ui.label(full);
        }
    }

    // LOD 2+: summary, tags
    if lod >= 2 {
        if let Some(summary) = &node.summary {
            ui.separator();
            ui.label(format!("Items: {}", summary.item_count));
            if let Some(status) = &summary.status {
                ui.label(format!("Status: {}", status));
            }
        }
    }

    // LOD 3: attributes, provenance
    if lod >= 3 {
        // Attributes
        if !node.attributes.is_empty() {
            ui.separator();
            ui.label(RichText::new("Attributes").strong());
            for (key, value) in &node.attributes {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}:", key)).color(Color32::GRAY));
                    ui.label(format!("{}", value));
                });
            }
        }

        // Provenance
        if let Some(prov) = &node.provenance {
            ui.separator();
            ui.label(RichText::new("Provenance").strong());

            ui.horizontal(|ui| {
                ui.label(RichText::new("Sources:").color(Color32::GRAY));
                ui.label(prov.sources.join(", "));
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new("Asserted:").color(Color32::GRAY));
                ui.label(&prov.asserted_at);
            });

            if let Some(conf) = prov.confidence {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Confidence:").color(Color32::GRAY));
                    ui.label(format!("{:.0}%", conf * 100.0));
                });
            }

            if let Some(notes) = &prov.notes {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Notes:").color(Color32::GRAY));
                    ui.label(notes);
                });
            }
        }

        // Links
        if !node.links.is_empty() {
            ui.separator();
            ui.label(RichText::new("Links").strong());
            for link in &node.links {
                if let Some(linked_node) = projection.get_node(link.target()) {
                    ui.horizontal(|ui| {
                        ui.label("→");
                        ui.label(&linked_node.label_short);
                    });
                }
            }
        }
    }
}
