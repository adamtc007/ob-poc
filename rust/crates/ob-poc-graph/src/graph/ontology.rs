//! Entity Type Ontology Browser
//!
//! Provides a hierarchical view of entity types with counts from the current graph.
//! Supports expand/collapse with spring animations and type-based filtering.
//!
//! # EGUI-RULES Compliance
//! - Type hierarchy is static config (not server data that changes)
//! - Expand/collapse state is UI-only
//! - Actions return values (TypeBrowserAction enum)
//! - No callbacks, no server data mutation

use std::collections::{HashMap, HashSet};

use egui::{Color32, Rect, Ui, Vec2};

use super::animation::{SpringConfig, SpringF32};
use super::types::{EntityType, LayoutGraph, LayoutNode};

// =============================================================================
// TYPE NODE
// =============================================================================

/// A node in the entity type hierarchy tree
#[derive(Debug, Clone)]
pub struct TypeNode {
    /// Type code (e.g., "SHELL", "PROPER_PERSON")
    pub type_code: String,
    /// Display label
    pub label: String,
    /// Child type nodes
    pub children: Vec<TypeNode>,
    /// Entity IDs in current graph matching this type (leaf types only)
    pub matching_entities: Vec<String>,
    /// Count including all descendants
    pub total_count: usize,
    /// Depth in tree (0 = root)
    pub depth: usize,
}

impl TypeNode {
    /// Create a new type node
    pub fn new(type_code: &str, label: &str) -> Self {
        Self {
            type_code: type_code.to_string(),
            label: label.to_string(),
            children: Vec::new(),
            matching_entities: Vec::new(),
            total_count: 0,
            depth: 0,
        }
    }

    /// Add a child node
    pub fn with_child(mut self, child: TypeNode) -> Self {
        self.children.push(child);
        self
    }

    /// Set depth recursively
    fn set_depths(&mut self, depth: usize) {
        self.depth = depth;
        for child in &mut self.children {
            child.set_depths(depth + 1);
        }
    }

    /// Get all type codes in this subtree (for filtering)
    pub fn all_type_codes(&self) -> Vec<String> {
        let mut codes = vec![self.type_code.clone()];
        for child in &self.children {
            codes.extend(child.all_type_codes());
        }
        codes
    }

    /// Check if this node or any descendant has entities
    pub fn has_entities(&self) -> bool {
        self.total_count > 0
    }
}

// =============================================================================
// ENTITY TYPE ONTOLOGY
// =============================================================================

/// The complete entity type hierarchy
#[derive(Debug, Clone)]
pub struct EntityTypeOntology {
    /// Root of the type tree
    pub root: TypeNode,
}

impl Default for EntityTypeOntology {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityTypeOntology {
    /// Create the standard entity type ontology
    pub fn new() -> Self {
        let mut root = TypeNode::new("ENTITY", "All Entities")
            .with_child(
                TypeNode::new("SHELL", "Legal Vehicles")
                    .with_child(TypeNode::new("LIMITED_COMPANY", "Corporation"))
                    .with_child(TypeNode::new("FUND", "Fund"))
                    .with_child(TypeNode::new("TRUST", "Trust"))
                    .with_child(TypeNode::new("PARTNERSHIP", "Partnership"))
                    .with_child(TypeNode::new("LLC", "LLC")),
            )
            .with_child(
                TypeNode::new("PERSON", "Natural Persons")
                    .with_child(TypeNode::new("PROPER_PERSON", "Individual"))
                    .with_child(TypeNode::new("UBO", "Beneficial Owner"))
                    .with_child(TypeNode::new("CONTROL_PERSON", "Control Person")),
            )
            .with_child(
                TypeNode::new("SERVICE_LAYER", "Services")
                    .with_child(TypeNode::new("PRODUCT", "Product"))
                    .with_child(TypeNode::new("SERVICE", "Service"))
                    .with_child(TypeNode::new("RESOURCE", "Resource")),
            );

        root.set_depths(0);

        Self { root }
    }

    /// Populate entity counts from a layout graph
    pub fn populate_counts(&mut self, graph: &LayoutGraph) {
        // Build a map of type_code -> entity_ids
        let mut type_entities: HashMap<String, Vec<String>> = HashMap::new();

        for (id, node) in &graph.nodes {
            let type_code = self.classify_node(node);
            type_entities.entry(type_code).or_default().push(id.clone());
        }

        // Recursively populate counts
        let mut root = self.root.clone();
        Self::populate_node_counts(&mut root, &type_entities);
        self.root = root;
    }

    fn populate_node_counts(node: &mut TypeNode, type_entities: &HashMap<String, Vec<String>>) {
        // Set matching entities for this node
        node.matching_entities = type_entities
            .get(&node.type_code)
            .cloned()
            .unwrap_or_default();

        // Recursively populate children
        for child in &mut node.children {
            Self::populate_node_counts(child, type_entities);
        }

        // Total count = own matches + all descendant matches
        node.total_count = node.matching_entities.len()
            + node.children.iter().map(|c| c.total_count).sum::<usize>();
    }

    /// Classify a layout node into a type code
    fn classify_node(&self, node: &LayoutNode) -> String {
        // First check entity_category from server
        if let Some(ref category) = node.entity_category {
            match category.to_uppercase().as_str() {
                "PERSON" => return "PERSON".to_string(),
                "SHELL" => return "SHELL".to_string(),
                "TRADING" => return "TRADING_LAYER".to_string(),
                _ => {}
            }
        }

        // Then check entity_type
        match node.entity_type {
            EntityType::ProperPerson => "PROPER_PERSON".to_string(),
            EntityType::LimitedCompany => "LIMITED_COMPANY".to_string(),
            EntityType::Partnership => "PARTNERSHIP".to_string(),
            EntityType::Trust => "TRUST".to_string(),
            EntityType::Fund => "FUND".to_string(),
            EntityType::Product => "PRODUCT".to_string(),
            EntityType::Service => "SERVICE".to_string(),
            EntityType::Resource => "RESOURCE".to_string(),
            // Trading layer types
            EntityType::TradingProfile => "TRADING_PROFILE".to_string(),
            EntityType::InstrumentMatrix => "INSTRUMENT_MATRIX".to_string(),
            EntityType::InstrumentClass => "INSTRUMENT_CLASS".to_string(),
            EntityType::Market => "MARKET".to_string(),
            EntityType::Counterparty => "COUNTERPARTY".to_string(),
            EntityType::IsdaAgreement => "ISDA_AGREEMENT".to_string(),
            EntityType::CsaAgreement => "CSA_AGREEMENT".to_string(),
            // Control layer types
            EntityType::ControlPortal => "CONTROL_PORTAL".to_string(),
            EntityType::Unknown => "ENTITY".to_string(),
        }
    }

    /// Get a node by type code (searches the tree)
    pub fn get_node(&self, type_code: &str) -> Option<&TypeNode> {
        Self::find_node(&self.root, type_code)
    }

    fn find_node<'a>(node: &'a TypeNode, type_code: &str) -> Option<&'a TypeNode> {
        if node.type_code == type_code {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = Self::find_node(child, type_code) {
                return Some(found);
            }
        }
        None
    }
}

// =============================================================================
// TAXONOMY STATE (Expand/Collapse)
// =============================================================================

/// Manages expand/collapse state for the type browser
#[derive(Debug, Clone)]
pub struct TaxonomyState {
    /// Which nodes are expanded
    expanded: HashSet<String>,
    /// Animation progress for each node (0.0 = collapsed, 1.0 = expanded)
    expand_progress: HashMap<String, SpringF32>,
    /// Currently selected type (for highlighting)
    selected_type: Option<String>,
    /// Hovered type
    hovered_type: Option<String>,
}

impl Default for TaxonomyState {
    fn default() -> Self {
        Self::new()
    }
}

impl TaxonomyState {
    pub fn new() -> Self {
        let mut state = Self {
            expanded: HashSet::new(),
            expand_progress: HashMap::new(),
            selected_type: None,
            hovered_type: None,
        };
        // Start with root expanded
        state.expand("ENTITY");
        state
    }

    /// Toggle expand/collapse for a node
    pub fn toggle(&mut self, type_code: &str) {
        if self.expanded.contains(type_code) {
            self.collapse(type_code);
        } else {
            self.expand(type_code);
        }
    }

    /// Expand a node
    pub fn expand(&mut self, type_code: &str) {
        self.expanded.insert(type_code.to_string());
        self.expand_progress
            .entry(type_code.to_string())
            .or_insert_with(|| SpringF32::with_config(0.0, SpringConfig::FAST))
            .set_target(1.0);
    }

    /// Collapse a node
    pub fn collapse(&mut self, type_code: &str) {
        self.expanded.remove(type_code);
        if let Some(progress) = self.expand_progress.get_mut(type_code) {
            progress.set_target(0.0);
        }
    }

    /// Expand all nodes to a given depth
    pub fn expand_to_depth(&mut self, ontology: &EntityTypeOntology, depth: usize) {
        self.expand_node_to_depth(&ontology.root, depth);
    }

    fn expand_node_to_depth(&mut self, node: &TypeNode, max_depth: usize) {
        if node.depth < max_depth {
            self.expand(&node.type_code);
            for child in &node.children {
                self.expand_node_to_depth(child, max_depth);
            }
        }
    }

    /// Collapse all nodes
    pub fn collapse_all(&mut self) {
        let codes: Vec<String> = self.expanded.iter().cloned().collect();
        for code in codes {
            self.collapse(&code);
        }
    }

    /// Check if a node is expanded
    pub fn is_expanded(&self, type_code: &str) -> bool {
        self.expanded.contains(type_code)
    }

    /// Get expand animation progress (0.0 - 1.0)
    pub fn get_expand_progress(&self, type_code: &str) -> f32 {
        self.expand_progress
            .get(type_code)
            .map(|p| p.get())
            .unwrap_or(0.0)
    }

    /// Select a type (for highlighting entities)
    pub fn select(&mut self, type_code: Option<&str>) {
        self.selected_type = type_code.map(|s| s.to_string());
    }

    /// Get selected type
    pub fn selected(&self) -> Option<&str> {
        self.selected_type.as_deref()
    }

    /// Set hovered type
    pub fn set_hover(&mut self, type_code: Option<&str>) {
        self.hovered_type = type_code.map(|s| s.to_string());
    }

    /// Get hovered type
    pub fn hovered(&self) -> Option<&str> {
        self.hovered_type.as_deref()
    }

    /// Update animations (call each frame)
    pub fn tick(&mut self, dt: f32) {
        for progress in self.expand_progress.values_mut() {
            progress.tick(dt);
        }
    }

    /// Check if any animations are in progress
    pub fn is_animating(&self) -> bool {
        self.expand_progress.values().any(|p| p.is_animating())
    }
}

// =============================================================================
// TYPE BROWSER ACTIONS
// =============================================================================

/// Actions returned from the type browser UI
#[derive(Debug, Clone, PartialEq)]
pub enum TypeBrowserAction {
    /// No action
    None,
    /// Toggle expand/collapse of a type node
    ToggleExpand { type_code: String },
    /// Select a type (highlight matching entities in graph)
    SelectType { type_code: String },
    /// Clear type selection
    ClearSelection,
    /// Filter graph to show only entities of this type
    FilterToType { type_code: String },
    /// Expand all nodes
    ExpandAll,
    /// Collapse all nodes
    CollapseAll,
}

// =============================================================================
// TYPE BROWSER COLORS
// =============================================================================

pub mod type_colors {
    use egui::Color32;

    /// Background color for the browser panel
    pub fn panel_bg() -> Color32 {
        Color32::from_rgba_unmultiplied(30, 30, 35, 240)
    }

    /// Header text color
    pub fn header_text() -> Color32 {
        Color32::from_rgb(200, 200, 210)
    }

    /// Normal type label color
    pub fn type_label() -> Color32 {
        Color32::from_rgb(180, 180, 190)
    }

    /// Count badge color
    pub fn count_badge() -> Color32 {
        Color32::from_rgb(100, 100, 110)
    }

    /// Count text color
    pub fn count_text() -> Color32 {
        Color32::from_rgb(150, 150, 160)
    }

    /// Hovered row background
    pub fn hover_bg() -> Color32 {
        Color32::from_rgba_unmultiplied(60, 60, 70, 200)
    }

    /// Selected row background
    pub fn selected_bg() -> Color32 {
        Color32::from_rgba_unmultiplied(59, 130, 246, 100)
    }

    /// Expand/collapse icon color
    pub fn expand_icon() -> Color32 {
        Color32::from_rgb(140, 140, 150)
    }

    /// SHELL type color
    pub fn shell_type() -> Color32 {
        Color32::from_rgb(147, 197, 253) // blue-300
    }

    /// PERSON type color
    pub fn person_type() -> Color32 {
        Color32::from_rgb(134, 239, 172) // green-300
    }

    /// SERVICE type color
    pub fn service_type() -> Color32 {
        Color32::from_rgb(253, 186, 116) // orange-300
    }
}

// =============================================================================
// RENDER TYPE BROWSER
// =============================================================================

/// Render the type hierarchy browser panel
///
/// Returns an action if the user interacted with the browser.
/// EGUI-RULES: Pure function, returns action, no callbacks.
pub fn render_type_browser(
    ui: &mut Ui,
    ontology: &EntityTypeOntology,
    state: &TaxonomyState,
    max_height: f32,
) -> TypeBrowserAction {
    let mut action = TypeBrowserAction::None;

    // Panel frame
    egui::Frame::none()
        .fill(type_colors::panel_bg())
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Entity Types")
                        .size(12.0)
                        .color(type_colors::header_text()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("−").on_hover_text("Collapse all").clicked() {
                        action = TypeBrowserAction::CollapseAll;
                    }
                    if ui.small_button("+").on_hover_text("Expand all").clicked() {
                        action = TypeBrowserAction::ExpandAll;
                    }
                    if state.selected().is_some()
                        && ui
                            .small_button("×")
                            .on_hover_text("Clear selection")
                            .clicked()
                    {
                        action = TypeBrowserAction::ClearSelection;
                    }
                });
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Scrollable tree
            egui::ScrollArea::vertical()
                .max_height(max_height - 50.0)
                .show(ui, |ui| {
                    let tree_action = render_type_node(ui, &ontology.root, state, true);
                    if tree_action != TypeBrowserAction::None {
                        action = tree_action;
                    }
                });
        });

    action
}

/// Render a single type node and its children recursively
fn render_type_node(
    ui: &mut Ui,
    node: &TypeNode,
    state: &TaxonomyState,
    is_visible: bool,
) -> TypeBrowserAction {
    if !is_visible {
        return TypeBrowserAction::None;
    }

    let mut action = TypeBrowserAction::None;
    let has_children = !node.children.is_empty();
    let is_expanded = state.is_expanded(&node.type_code);
    let is_selected = state.selected() == Some(&node.type_code);
    let is_hovered = state.hovered() == Some(&node.type_code);

    // Indentation based on depth
    let indent = node.depth as f32 * 16.0;

    // Row background
    let row_rect = ui.available_rect_before_wrap();
    let row_height = 22.0;
    let row_rect = Rect::from_min_size(row_rect.min, Vec2::new(ui.available_width(), row_height));

    // Background fill
    if is_selected {
        ui.painter()
            .rect_filled(row_rect, 2.0, type_colors::selected_bg());
    } else if is_hovered {
        ui.painter()
            .rect_filled(row_rect, 2.0, type_colors::hover_bg());
    }

    ui.horizontal(|ui| {
        // Indent
        ui.add_space(indent);

        // Expand/collapse button
        if has_children {
            let icon = if is_expanded { "▼" } else { "▶" };
            let response = ui.add(
                egui::Label::new(
                    egui::RichText::new(icon)
                        .size(10.0)
                        .color(type_colors::expand_icon()),
                )
                .sense(egui::Sense::click()),
            );
            if response.clicked() {
                action = TypeBrowserAction::ToggleExpand {
                    type_code: node.type_code.clone(),
                };
            }
        } else {
            // Spacer for alignment
            ui.add_space(12.0);
        }

        // Type color indicator
        let type_color = get_type_color(&node.type_code);
        let indicator_rect = ui.available_rect_before_wrap();
        let indicator_rect = Rect::from_min_size(
            indicator_rect.min + Vec2::new(0.0, 6.0),
            Vec2::new(8.0, 8.0),
        );
        ui.painter().rect_filled(indicator_rect, 2.0, type_color);
        ui.add_space(12.0);

        // Type label (clickable)
        let label_response = ui.add(
            egui::Label::new(
                egui::RichText::new(&node.label)
                    .size(11.0)
                    .color(type_colors::type_label()),
            )
            .sense(egui::Sense::click()),
        );

        if label_response.clicked() {
            action = TypeBrowserAction::SelectType {
                type_code: node.type_code.clone(),
            };
        }
        if label_response.double_clicked() {
            action = TypeBrowserAction::FilterToType {
                type_code: node.type_code.clone(),
            };
        }

        // Count badge (if has entities)
        if node.total_count > 0 {
            ui.add_space(4.0);
            let count_text = format!("({})", node.total_count);
            ui.label(
                egui::RichText::new(count_text)
                    .size(10.0)
                    .color(type_colors::count_text()),
            );
        }
    });

    ui.add_space(2.0);

    // Render children if expanded
    if has_children && is_expanded {
        let expand_progress = state.get_expand_progress(&node.type_code);
        let children_visible = expand_progress > 0.1;

        if children_visible {
            // Animate children visibility (simple alpha for now)
            for child in &node.children {
                let child_action = render_type_node(ui, child, state, true);
                if child_action != TypeBrowserAction::None {
                    action = child_action;
                }
            }
        }
    }

    action
}

/// Get the color for a type code
fn get_type_color(type_code: &str) -> Color32 {
    match type_code {
        // SHELL types - blue
        "SHELL" | "LIMITED_COMPANY" | "FUND" | "TRUST" | "PARTNERSHIP" | "LLC" => {
            type_colors::shell_type()
        }
        // PERSON types - green
        "PERSON" | "PROPER_PERSON" | "UBO" | "CONTROL_PERSON" => type_colors::person_type(),
        // SERVICE types - orange
        "SERVICE_LAYER" | "PRODUCT" | "SERVICE" | "RESOURCE" => type_colors::service_type(),
        // Root - neutral
        _ => Color32::from_rgb(160, 160, 170),
    }
}

// =============================================================================
// INTEGRATION HELPERS
// =============================================================================

/// Get entity IDs matching a type code (including descendants)
pub fn get_entities_for_type(ontology: &EntityTypeOntology, type_code: &str) -> Vec<String> {
    if let Some(node) = ontology.get_node(type_code) {
        collect_all_entities(node)
    } else {
        Vec::new()
    }
}

fn collect_all_entities(node: &TypeNode) -> Vec<String> {
    let mut entities = node.matching_entities.clone();
    for child in &node.children {
        entities.extend(collect_all_entities(child));
    }
    entities
}

/// Check if an entity matches a type filter
pub fn entity_matches_type(
    entity_id: &str,
    type_code: &str,
    ontology: &EntityTypeOntology,
) -> bool {
    let matching = get_entities_for_type(ontology, type_code);
    matching.contains(&entity_id.to_string())
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ontology_creation() {
        let ontology = EntityTypeOntology::new();
        assert_eq!(ontology.root.type_code, "ENTITY");
        assert_eq!(ontology.root.children.len(), 3); // SHELL, PERSON, SERVICE_LAYER
    }

    #[test]
    fn test_type_node_lookup() {
        let ontology = EntityTypeOntology::new();
        let shell = ontology.get_node("SHELL");
        assert!(shell.is_some());
        assert_eq!(shell.unwrap().label, "Legal Vehicles");

        let person = ontology.get_node("PROPER_PERSON");
        assert!(person.is_some());
        assert_eq!(person.unwrap().label, "Individual");
    }

    #[test]
    fn test_taxonomy_state_expand_collapse() {
        let mut state = TaxonomyState::new();

        // Root should be expanded by default
        assert!(state.is_expanded("ENTITY"));

        // Expand SHELL
        state.expand("SHELL");
        assert!(state.is_expanded("SHELL"));

        // Collapse SHELL
        state.collapse("SHELL");
        assert!(!state.is_expanded("SHELL"));

        // Toggle
        state.toggle("SHELL");
        assert!(state.is_expanded("SHELL"));
        state.toggle("SHELL");
        assert!(!state.is_expanded("SHELL"));
    }

    #[test]
    fn test_all_type_codes() {
        let ontology = EntityTypeOntology::new();
        let all_codes = ontology.root.all_type_codes();

        assert!(all_codes.contains(&"ENTITY".to_string()));
        assert!(all_codes.contains(&"SHELL".to_string()));
        assert!(all_codes.contains(&"LIMITED_COMPANY".to_string()));
        assert!(all_codes.contains(&"PERSON".to_string()));
        assert!(all_codes.contains(&"PROPER_PERSON".to_string()));
    }
}
