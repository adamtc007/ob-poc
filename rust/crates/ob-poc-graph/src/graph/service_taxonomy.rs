//! Service Resource Taxonomy Browser
//!
//! Provides a hierarchical drill-down view of CBU service configuration:
//! - Products (Global Custody, Securities Lending, etc.)
//!   - Services (Settlement, Safekeeping, Corporate Actions, etc.)
//!     - Service Intents (active subscriptions with options)
//!       - Discovered SRDEFs (required resources)
//!         - Attribute Requirements (data needed)
//!           - Attribute Values (satisfied/missing)
//!
//! # Design Principles (EGUI-RULES Compliant)
//! - Data fetched from server, not mutated locally
//! - Actions return values (ServiceTaxonomyAction enum)
//! - No callbacks, pure render functions
//! - Expand/collapse state is UI-only (ServiceTaxonomyState)

use std::collections::{HashMap, HashSet};

use egui::{Color32, RichText, Ui};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::animation::{SpringConfig, SpringF32};

// =============================================================================
// SERVICE TAXONOMY NODE TYPES
// =============================================================================

/// Unique identifier for a node in the service taxonomy tree
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ServiceTaxonomyNodeId(pub Vec<String>);

impl ServiceTaxonomyNodeId {
    /// Get a string key for HashMap storage
    pub fn as_key(&self) -> String {
        self.0.join("/")
    }

    /// Create a root node ID
    pub fn root() -> Self {
        Self(Vec::new())
    }

    /// Create a child node ID
    pub fn child(&self, segment: &str) -> Self {
        let mut segments = self.0.clone();
        segments.push(segment.to_string());
        Self(segments)
    }

    /// Get the depth of this node (0 = root)
    pub fn depth(&self) -> usize {
        self.0.len()
    }

    /// Parse from a key string
    pub fn parse(key: &str) -> Self {
        if key.is_empty() {
            Self::root()
        } else {
            Self(key.split('/').map(|s| s.to_string()).collect())
        }
    }
}

/// Type of node in the service taxonomy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServiceTaxonomyNodeType {
    /// Root node (CBU level)
    Root { cbu_id: Uuid },
    /// Product category (e.g., "Global Custody")
    Product { product_id: Uuid },
    /// Service within a product (e.g., "Settlement")
    Service { service_id: Uuid, product_id: Uuid },
    /// Active service intent (subscription with options)
    ServiceIntent { intent_id: Uuid },
    /// Discovered resource definition
    Resource {
        srdef_id: String,
        resource_type: String,
    },
    /// Attribute requirement category
    AttributeCategory { category: String },
    /// Individual attribute requirement
    Attribute { attr_id: Uuid, satisfied: bool },
    /// Attribute value (when populated)
    AttributeValue { source: String },
}

/// Status indicator for service/resource readiness
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    /// Ready to use
    Ready,
    /// Partially ready (some requirements met)
    Partial,
    /// Blocked (missing critical requirements)
    Blocked,
    /// Pending processing
    #[default]
    Pending,
}

impl ServiceStatus {
    /// Convert to egui Color32
    pub fn to_color32(self) -> Color32 {
        match self {
            ServiceStatus::Ready => Color32::from_rgb(34, 197, 94), // green-500
            ServiceStatus::Partial => Color32::from_rgb(234, 179, 8), // yellow-500
            ServiceStatus::Blocked => Color32::from_rgb(239, 68, 68), // red-500
            ServiceStatus::Pending => Color32::from_rgb(107, 114, 128), // gray-500
        }
    }

    /// Get icon character
    pub fn icon(self) -> &'static str {
        match self {
            ServiceStatus::Ready => "✓",
            ServiceStatus::Partial => "◐",
            ServiceStatus::Blocked => "✗",
            ServiceStatus::Pending => "⏳",
        }
    }
}

/// A node in the service taxonomy tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceTaxonomyNode {
    /// Unique node identifier
    pub id: ServiceTaxonomyNodeId,
    /// Node type with associated data
    pub node_type: ServiceTaxonomyNodeType,
    /// Display label
    pub label: String,
    /// Optional sublabel (e.g., status text)
    pub sublabel: Option<String>,
    /// Status indicator
    pub status: ServiceStatus,
    /// Child nodes
    pub children: Vec<ServiceTaxonomyNode>,
    /// Count of leaf nodes (for display)
    pub leaf_count: usize,
    /// Blocking reasons (if status is Blocked)
    pub blocking_reasons: Vec<String>,
    /// Attribute satisfaction ratio (satisfied/total)
    pub attr_progress: Option<(usize, usize)>,
}

impl ServiceTaxonomyNode {
    /// Create a new node
    pub fn new(
        id: ServiceTaxonomyNodeId,
        node_type: ServiceTaxonomyNodeType,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id,
            node_type,
            label: label.into(),
            sublabel: None,
            status: ServiceStatus::Pending,
            children: Vec::new(),
            leaf_count: 0,
            blocking_reasons: Vec::new(),
            attr_progress: None,
        }
    }

    /// Add a child node
    pub fn add_child(&mut self, child: ServiceTaxonomyNode) {
        self.children.push(child);
    }

    /// Check if this is a leaf node
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Get the icon for this node type
    pub fn icon(&self) -> &'static str {
        match &self.node_type {
            ServiceTaxonomyNodeType::Root { .. } => "🏢",
            ServiceTaxonomyNodeType::Product { .. } => "📦",
            ServiceTaxonomyNodeType::Service { .. } => "🔧",
            ServiceTaxonomyNodeType::ServiceIntent { .. } => "🎯",
            ServiceTaxonomyNodeType::Resource { .. } => "🔌",
            ServiceTaxonomyNodeType::AttributeCategory { .. } => "📋",
            ServiceTaxonomyNodeType::Attribute { satisfied, .. } => {
                if *satisfied {
                    "✓"
                } else {
                    "○"
                }
            }
            ServiceTaxonomyNodeType::AttributeValue { .. } => "•",
        }
    }

    /// Compute leaf counts recursively
    pub fn compute_leaf_counts(&mut self) -> usize {
        if self.children.is_empty() {
            self.leaf_count = 1;
        } else {
            self.leaf_count = self
                .children
                .iter_mut()
                .map(|c| c.compute_leaf_counts())
                .sum();
        }
        self.leaf_count
    }
}

// =============================================================================
// SERVICE TAXONOMY (Container)
// =============================================================================

/// Container for the complete service taxonomy tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceTaxonomy {
    /// Root node containing the entire tree
    pub root: ServiceTaxonomyNode,
    /// CBU ID this taxonomy is for
    pub cbu_id: Uuid,
    /// CBU name for display
    pub cbu_name: String,
    /// Summary statistics
    pub stats: ServiceTaxonomyStats,
}

/// Summary statistics for the taxonomy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceTaxonomyStats {
    /// Total products
    pub product_count: usize,
    /// Total services
    pub service_count: usize,
    /// Total active intents
    pub intent_count: usize,
    /// Total discovered resources
    pub resource_count: usize,
    /// Total attributes (satisfied, total)
    pub attribute_progress: (usize, usize),
    /// Services by status
    pub services_ready: usize,
    pub services_partial: usize,
    pub services_blocked: usize,
}

impl ServiceTaxonomy {
    /// Create a new empty taxonomy for a CBU
    pub fn new(cbu_id: Uuid, cbu_name: impl Into<String>) -> Self {
        let name = cbu_name.into();
        let root = ServiceTaxonomyNode::new(
            ServiceTaxonomyNodeId::root(),
            ServiceTaxonomyNodeType::Root { cbu_id },
            name.clone(),
        );
        Self {
            root,
            cbu_id,
            cbu_name: name,
            stats: ServiceTaxonomyStats::default(),
        }
    }

    /// Get children of the root (products)
    pub fn products(&self) -> &[ServiceTaxonomyNode] {
        &self.root.children
    }

    /// Add a product to the taxonomy
    pub fn add_product(&mut self, product: ServiceTaxonomyNode) {
        self.root.children.push(product);
        self.stats.product_count += 1;
    }

    /// Recompute all statistics and leaf counts
    pub fn recompute_stats(&mut self) {
        self.root.compute_leaf_counts();

        // Reset stats
        self.stats = ServiceTaxonomyStats::default();

        // Walk tree and collect stats
        fn visit(node: &ServiceTaxonomyNode, stats: &mut ServiceTaxonomyStats) {
            match &node.node_type {
                ServiceTaxonomyNodeType::Product { .. } => stats.product_count += 1,
                ServiceTaxonomyNodeType::Service { .. } => {
                    stats.service_count += 1;
                    match node.status {
                        ServiceStatus::Ready => stats.services_ready += 1,
                        ServiceStatus::Partial => stats.services_partial += 1,
                        ServiceStatus::Blocked => stats.services_blocked += 1,
                        ServiceStatus::Pending => {}
                    }
                }
                ServiceTaxonomyNodeType::ServiceIntent { .. } => stats.intent_count += 1,
                ServiceTaxonomyNodeType::Resource { .. } => stats.resource_count += 1,
                ServiceTaxonomyNodeType::Attribute { satisfied, .. } => {
                    stats.attribute_progress.1 += 1;
                    if *satisfied {
                        stats.attribute_progress.0 += 1;
                    }
                }
                _ => {}
            }
            for child in &node.children {
                visit(child, stats);
            }
        }

        for child in &self.root.children {
            visit(child, &mut self.stats);
        }
    }
}

// =============================================================================
// SERVICE TAXONOMY STATE (UI-only expand/collapse)
// =============================================================================

/// Manages expand/collapse state and animations for the service taxonomy browser
#[derive(Debug, Clone)]
pub struct ServiceTaxonomyState {
    /// Which nodes are expanded (by node ID key)
    expanded: HashSet<String>,
    /// Animation progress for each node (0.0 = collapsed, 1.0 = expanded)
    expand_progress: HashMap<String, SpringF32>,
    /// Currently selected node (for detail panel)
    selected_node: Option<String>,
    /// Filter: show only blocked items
    pub show_blocked_only: bool,
    /// Filter: show attribute details
    pub show_attributes: bool,
}

impl Default for ServiceTaxonomyState {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceTaxonomyState {
    pub fn new() -> Self {
        Self {
            expanded: HashSet::new(),
            expand_progress: HashMap::new(),
            selected_node: None,
            show_blocked_only: false,
            show_attributes: true,
        }
    }

    /// Toggle expand/collapse for a node
    pub fn toggle(&mut self, node_id: &ServiceTaxonomyNodeId) {
        let key = node_id.as_key();
        if self.expanded.contains(&key) {
            self.collapse(node_id);
        } else {
            self.expand(node_id);
        }
    }

    /// Expand a node
    pub fn expand(&mut self, node_id: &ServiceTaxonomyNodeId) {
        let key = node_id.as_key();
        self.expanded.insert(key.clone());
        self.expand_progress
            .entry(key)
            .or_insert_with(|| SpringF32::with_config(0.0, SpringConfig::from_preset("fast")))
            .set_target(1.0);
    }

    /// Collapse a node
    pub fn collapse(&mut self, node_id: &ServiceTaxonomyNodeId) {
        let key = node_id.as_key();
        self.expanded.remove(&key);
        if let Some(progress) = self.expand_progress.get_mut(&key) {
            progress.set_target(0.0);
        }
    }

    /// Check if a node is expanded
    pub fn is_expanded(&self, node_id: &ServiceTaxonomyNodeId) -> bool {
        self.expanded.contains(&node_id.as_key())
    }

    /// Get expand animation progress (0.0 - 1.0)
    pub fn get_expand_progress(&self, node_id: &ServiceTaxonomyNodeId) -> f32 {
        self.expand_progress
            .get(&node_id.as_key())
            .map(|p| p.get())
            .unwrap_or(0.0)
    }

    /// Select a node (for showing detail panel)
    pub fn select(&mut self, node_id: Option<&ServiceTaxonomyNodeId>) {
        self.selected_node = node_id.map(|id| id.as_key());
    }

    /// Get selected node key
    pub fn selected(&self) -> Option<&str> {
        self.selected_node.as_deref()
    }

    /// Expand all nodes at a given depth
    pub fn expand_to_depth(&mut self, root: &ServiceTaxonomyNode, depth: usize) {
        fn expand_recursive(
            node: &ServiceTaxonomyNode,
            state: &mut ServiceTaxonomyState,
            current_depth: usize,
            max_depth: usize,
        ) {
            if current_depth < max_depth {
                state.expand(&node.id);
                for child in &node.children {
                    expand_recursive(child, state, current_depth + 1, max_depth);
                }
            }
        }
        expand_recursive(root, self, 0, depth);
    }

    /// Collapse all nodes
    pub fn collapse_all(&mut self) {
        self.expanded.clear();
        for progress in self.expand_progress.values_mut() {
            progress.set_target(0.0);
        }
    }

    /// Tick animations (returns true if any animation is still running)
    pub fn tick(&mut self, dt: f32) -> bool {
        let mut any_animating = false;
        for progress in self.expand_progress.values_mut() {
            progress.tick(dt);
            if progress.is_animating() {
                any_animating = true;
            }
        }
        any_animating
    }
}

// =============================================================================
// SERVICE TAXONOMY ACTIONS (returned from render)
// =============================================================================

/// Actions returned from the service taxonomy browser
#[derive(Debug, Clone)]
pub enum ServiceTaxonomyAction {
    /// No action
    None,
    /// Toggle expand/collapse for a node
    ToggleExpand { node_id: ServiceTaxonomyNodeId },
    /// Select a node (show in detail panel)
    SelectNode { node_id: ServiceTaxonomyNodeId },
    /// Drill into a resource for detail view
    DrillIntoResource { srdef_id: String },
    /// Show blocking reason detail
    ShowBlockingReason {
        node_id: ServiceTaxonomyNodeId,
        reason: String,
    },
    /// Toggle blocked-only filter
    ToggleBlockedFilter,
    /// Toggle attribute detail visibility
    ToggleAttributeDetail,
    /// Expand all nodes
    ExpandAll,
    /// Collapse all nodes
    CollapseAll,
    /// Refresh data from server
    Refresh,
}

// =============================================================================
// RENDER FUNCTIONS
// =============================================================================

/// Render the service taxonomy browser
pub fn render_service_taxonomy(
    ui: &mut Ui,
    taxonomy: &ServiceTaxonomy,
    state: &mut ServiceTaxonomyState,
    max_height: f32,
) -> ServiceTaxonomyAction {
    let mut action = ServiceTaxonomyAction::None;

    // Header with stats
    ui.horizontal(|ui| {
        ui.heading("Service Resources");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Refresh button
            if ui.small_button("↻").clicked() {
                action = ServiceTaxonomyAction::Refresh;
            }
            // Expand/collapse buttons
            if ui.small_button("▼").on_hover_text("Expand all").clicked() {
                action = ServiceTaxonomyAction::ExpandAll;
            }
            if ui.small_button("▶").on_hover_text("Collapse all").clicked() {
                action = ServiceTaxonomyAction::CollapseAll;
            }
        });
    });

    // Stats bar
    ui.horizontal(|ui| {
        let stats = &taxonomy.stats;
        ui.label(
            RichText::new(format!(
                "{}P {}S {}I {}R",
                stats.product_count, stats.service_count, stats.intent_count, stats.resource_count
            ))
            .small()
            .color(Color32::GRAY),
        );

        ui.separator();

        // Readiness summary
        if stats.services_ready > 0 {
            ui.label(
                RichText::new(format!("✓{}", stats.services_ready))
                    .small()
                    .color(ServiceStatus::Ready.to_color32()),
            );
        }
        if stats.services_partial > 0 {
            ui.label(
                RichText::new(format!("◐{}", stats.services_partial))
                    .small()
                    .color(ServiceStatus::Partial.to_color32()),
            );
        }
        if stats.services_blocked > 0 {
            ui.label(
                RichText::new(format!("✗{}", stats.services_blocked))
                    .small()
                    .color(ServiceStatus::Blocked.to_color32()),
            );
        }

        // Attribute progress
        let (satisfied, total) = stats.attribute_progress;
        if total > 0 {
            ui.separator();
            let pct = (satisfied as f32 / total as f32) * 100.0;
            ui.label(
                RichText::new(format!("Attrs: {}/{} ({:.0}%)", satisfied, total, pct))
                    .small()
                    .color(if pct >= 100.0 {
                        ServiceStatus::Ready.to_color32()
                    } else if pct >= 50.0 {
                        ServiceStatus::Partial.to_color32()
                    } else {
                        ServiceStatus::Blocked.to_color32()
                    }),
            );
        }
    });

    // Filter toggles
    ui.horizontal(|ui| {
        if ui
            .selectable_label(state.show_blocked_only, "Blocked only")
            .clicked()
        {
            action = ServiceTaxonomyAction::ToggleBlockedFilter;
        }
        if ui
            .selectable_label(state.show_attributes, "Show attrs")
            .clicked()
        {
            action = ServiceTaxonomyAction::ToggleAttributeDetail;
        }
    });

    ui.separator();

    // Tree view
    egui::ScrollArea::vertical()
        .max_height(max_height - 100.0)
        .show(ui, |ui| {
            for product in taxonomy.products() {
                if let ServiceTaxonomyAction::None = action {
                    action = render_service_node(ui, product, state, 0);
                } else {
                    // Already have an action, skip remaining nodes
                    render_service_node(ui, product, state, 0);
                }
            }
        });

    action
}

/// Render a single node in the tree (recursive)
fn render_service_node(
    ui: &mut Ui,
    node: &ServiceTaxonomyNode,
    state: &mut ServiceTaxonomyState,
    depth: usize,
) -> ServiceTaxonomyAction {
    let mut action = ServiceTaxonomyAction::None;

    // Filter: skip non-blocked if filter is on
    if state.show_blocked_only
        && node.status != ServiceStatus::Blocked
        && !has_blocked_descendant(node)
    {
        return action;
    }

    // Skip attribute nodes if not showing attributes
    if !state.show_attributes
        && matches!(
            node.node_type,
            ServiceTaxonomyNodeType::Attribute { .. }
                | ServiceTaxonomyNodeType::AttributeValue { .. }
                | ServiceTaxonomyNodeType::AttributeCategory { .. }
        )
    {
        return action;
    }

    let is_expanded = state.is_expanded(&node.id);
    let has_children = !node.children.is_empty();
    let is_selected = state.selected() == Some(&node.id.as_key());

    // Indent based on depth
    let indent = depth as f32 * 16.0;

    ui.horizontal(|ui| {
        ui.add_space(indent);

        // Expand/collapse toggle
        if has_children {
            let toggle_text = if is_expanded { "▼" } else { "▶" };
            if ui
                .add(
                    egui::Label::new(RichText::new(toggle_text).monospace())
                        .sense(egui::Sense::click()),
                )
                .clicked()
            {
                action = ServiceTaxonomyAction::ToggleExpand {
                    node_id: node.id.clone(),
                };
            }
        } else {
            // Spacer for alignment
            ui.add_space(12.0);
        }

        // Status indicator
        ui.label(
            RichText::new(node.status.icon())
                .color(node.status.to_color32())
                .strong(),
        );

        // Node icon
        ui.label(RichText::new(node.icon()));

        // Node label (clickable for selection)
        let label_text = if node.leaf_count > 0 && has_children {
            format!("{} ({})", node.label, node.leaf_count)
        } else {
            node.label.clone()
        };

        let label_color = if is_selected {
            Color32::from_rgb(96, 165, 250) // blue-400
        } else {
            Color32::WHITE
        };

        let label_response = ui.add(
            egui::Label::new(RichText::new(&label_text).color(label_color))
                .sense(egui::Sense::click()),
        );

        if label_response.clicked() {
            action = ServiceTaxonomyAction::SelectNode {
                node_id: node.id.clone(),
            };
        }

        // Sublabel (if any)
        if let Some(ref sublabel) = node.sublabel {
            ui.label(RichText::new(sublabel).small().color(Color32::GRAY));
        }

        // Attribute progress bar (if available)
        if let Some((satisfied, total)) = node.attr_progress {
            let pct = if total > 0 {
                satisfied as f32 / total as f32
            } else {
                0.0
            };
            let bar_width = 40.0;
            let bar_height = 6.0;
            let (rect, _) = ui
                .allocate_exact_size(egui::Vec2::new(bar_width, bar_height), egui::Sense::hover());
            let painter = ui.painter();

            // Background
            painter.rect_filled(rect, 2.0, Color32::from_rgb(40, 40, 40));

            // Progress fill
            let fill_rect =
                egui::Rect::from_min_size(rect.min, egui::Vec2::new(bar_width * pct, bar_height));
            let fill_color = if pct >= 1.0 {
                ServiceStatus::Ready.to_color32()
            } else if pct >= 0.5 {
                ServiceStatus::Partial.to_color32()
            } else {
                ServiceStatus::Blocked.to_color32()
            };
            painter.rect_filled(fill_rect, 2.0, fill_color);
        }

        // Blocking reasons indicator
        if !node.blocking_reasons.is_empty() {
            let reason_count = node.blocking_reasons.len();
            let tooltip_text = node.blocking_reasons.join("\n");
            ui.label(
                RichText::new(format!("⚠{}", reason_count))
                    .small()
                    .color(ServiceStatus::Blocked.to_color32()),
            )
            .on_hover_text(tooltip_text);
        }
    });

    // Render children if expanded
    if is_expanded && has_children {
        for child in &node.children {
            if let ServiceTaxonomyAction::None = action {
                action = render_service_node(ui, child, state, depth + 1);
            } else {
                render_service_node(ui, child, state, depth + 1);
            }
        }
    }

    action
}

/// Check if any descendant is blocked (for filtering)
fn has_blocked_descendant(node: &ServiceTaxonomyNode) -> bool {
    if node.status == ServiceStatus::Blocked {
        return true;
    }
    node.children.iter().any(has_blocked_descendant)
}

/// Render detail panel for selected node
pub fn render_service_detail_panel(
    ui: &mut Ui,
    node: &ServiceTaxonomyNode,
) -> ServiceTaxonomyAction {
    let mut action = ServiceTaxonomyAction::None;

    egui::Frame::none()
        .fill(Color32::from_rgb(30, 30, 30))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(RichText::new(node.icon()).heading());
                ui.label(RichText::new(&node.label).heading());
                ui.label(
                    RichText::new(node.status.icon())
                        .heading()
                        .color(node.status.to_color32()),
                );
            });

            if let Some(ref sublabel) = node.sublabel {
                ui.label(RichText::new(sublabel).color(Color32::GRAY));
            }

            ui.separator();

            // Type-specific details
            match &node.node_type {
                ServiceTaxonomyNodeType::Product { product_id } => {
                    ui.label(format!("Product ID: {}", product_id));
                }
                ServiceTaxonomyNodeType::Service {
                    service_id,
                    product_id,
                } => {
                    ui.label(format!("Service ID: {}", service_id));
                    ui.label(format!("Product ID: {}", product_id));
                }
                ServiceTaxonomyNodeType::ServiceIntent { intent_id } => {
                    ui.label(format!("Intent ID: {}", intent_id));
                }
                ServiceTaxonomyNodeType::Resource {
                    srdef_id,
                    resource_type,
                } => {
                    ui.label(format!("SRDEF: {}", srdef_id));
                    ui.label(format!("Type: {}", resource_type));
                    if ui.button("View Details").clicked() {
                        action = ServiceTaxonomyAction::DrillIntoResource {
                            srdef_id: srdef_id.clone(),
                        };
                    }
                }
                ServiceTaxonomyNodeType::Attribute { attr_id, satisfied } => {
                    ui.label(format!("Attribute ID: {}", attr_id));
                    ui.label(format!(
                        "Status: {}",
                        if *satisfied { "Satisfied" } else { "Missing" }
                    ));
                }
                ServiceTaxonomyNodeType::AttributeValue { source } => {
                    ui.label(format!("Source: {}", source));
                }
                _ => {}
            }

            // Blocking reasons
            if !node.blocking_reasons.is_empty() {
                ui.separator();
                ui.label(
                    RichText::new("Blocking Reasons:")
                        .strong()
                        .color(ServiceStatus::Blocked.to_color32()),
                );
                for reason in &node.blocking_reasons {
                    ui.label(format!("• {}", reason));
                }
            }

            // Attribute progress
            if let Some((satisfied, total)) = node.attr_progress {
                ui.separator();
                ui.label(RichText::new("Attribute Progress:").strong());
                ui.label(format!("{} / {} satisfied", satisfied, total));
                let pct = if total > 0 {
                    (satisfied as f32 / total as f32) * 100.0
                } else {
                    0.0
                };
                ui.add(egui::ProgressBar::new(pct / 100.0).text(format!("{:.0}%", pct)));
            }
        });

    action
}

// =============================================================================
// BUILDER HELPERS (for constructing taxonomy from API responses)
// =============================================================================

impl ServiceTaxonomy {
    /// Build taxonomy from service readiness data
    pub fn from_readiness_data(cbu_id: Uuid, cbu_name: &str, products: Vec<ProductData>) -> Self {
        let mut taxonomy = Self::new(cbu_id, cbu_name);

        for product in products {
            let product_node_id = ServiceTaxonomyNodeId(vec![format!("P:{}", product.id)]);
            let mut product_node = ServiceTaxonomyNode::new(
                product_node_id.clone(),
                ServiceTaxonomyNodeType::Product {
                    product_id: product.id,
                },
                &product.name,
            );

            for service in product.services {
                let service_node_id = product_node_id.child(&format!("S:{}", service.id));
                let mut service_node = ServiceTaxonomyNode::new(
                    service_node_id.clone(),
                    ServiceTaxonomyNodeType::Service {
                        service_id: service.id,
                        product_id: product.id,
                    },
                    &service.name,
                );
                service_node.status = service.status;
                service_node.blocking_reasons = service.blocking_reasons;
                service_node.attr_progress = service.attr_progress;

                // Add intents
                for intent in service.intents {
                    let intent_node_id = service_node_id.child(&format!("I:{}", intent.id));
                    let mut intent_node = ServiceTaxonomyNode::new(
                        intent_node_id.clone(),
                        ServiceTaxonomyNodeType::ServiceIntent {
                            intent_id: intent.id,
                        },
                        "Intent",
                    );
                    intent_node.sublabel = intent.options_summary;
                    intent_node.status = service.status; // Inherit from service

                    // Add resources
                    for resource in intent.resources {
                        let resource_node_id =
                            intent_node_id.child(&format!("R:{}", resource.srdef_id));
                        let mut resource_node = ServiceTaxonomyNode::new(
                            resource_node_id,
                            ServiceTaxonomyNodeType::Resource {
                                srdef_id: resource.srdef_id.clone(),
                                resource_type: resource.resource_type.clone(),
                            },
                            &resource.name,
                        );
                        resource_node.status = resource.status;
                        resource_node.blocking_reasons = resource.blocking_reasons;

                        intent_node.add_child(resource_node);
                    }

                    service_node.add_child(intent_node);
                }

                product_node.add_child(service_node);
            }

            // Compute product status from children
            product_node.status = compute_aggregate_status(&product_node.children);
            taxonomy.add_product(product_node);
        }

        taxonomy.recompute_stats();
        taxonomy
    }
}

/// Compute aggregate status from children (worst status wins)
fn compute_aggregate_status(children: &[ServiceTaxonomyNode]) -> ServiceStatus {
    let mut has_blocked = false;
    let mut has_partial = false;
    let mut has_ready = false;

    for child in children {
        match child.status {
            ServiceStatus::Blocked => has_blocked = true,
            ServiceStatus::Partial => has_partial = true,
            ServiceStatus::Ready => has_ready = true,
            ServiceStatus::Pending => {}
        }
    }

    if has_blocked {
        ServiceStatus::Blocked
    } else if has_partial {
        ServiceStatus::Partial
    } else if has_ready {
        ServiceStatus::Ready
    } else {
        ServiceStatus::Pending
    }
}

// =============================================================================
// DATA TRANSFER TYPES (for building taxonomy from API)
// =============================================================================

/// Product data for building taxonomy
#[derive(Debug, Clone)]
pub struct ProductData {
    pub id: Uuid,
    pub name: String,
    pub services: Vec<ServiceData>,
}

/// Service data for building taxonomy
#[derive(Debug, Clone)]
pub struct ServiceData {
    pub id: Uuid,
    pub name: String,
    pub status: ServiceStatus,
    pub blocking_reasons: Vec<String>,
    pub attr_progress: Option<(usize, usize)>,
    pub intents: Vec<IntentData>,
}

/// Intent data for building taxonomy
#[derive(Debug, Clone)]
pub struct IntentData {
    pub id: Uuid,
    pub options_summary: Option<String>,
    pub resources: Vec<ResourceData>,
}

/// Resource data for building taxonomy
#[derive(Debug, Clone)]
pub struct ResourceData {
    pub srdef_id: String,
    pub name: String,
    pub resource_type: String,
    pub status: ServiceStatus,
    pub blocking_reasons: Vec<String>,
}
