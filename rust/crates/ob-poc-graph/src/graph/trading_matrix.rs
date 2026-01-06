//! Trading Matrix Taxonomy Browser
//!
//! Provides a hierarchical drill-down view of CBU trading configuration:
//! - Instrument Classes (EQUITY, GOVT_BOND, OTC_IRS, etc.)
//!   └── Markets/Counterparties (XNYS, XLON, Goldman Sachs)
//!       └── Universe Entries (currencies, settlement types)
//!           └── Resources:
//!               ├── SSIs (with booking rules)
//!               ├── Settlement Chains (with hops)
//!               ├── Tax Configuration (treaty rates, reclaim)
//!               └── ISDA/CSA (for OTC)
//!
//! # Design Principles (EGUI-RULES Compliant)
//! - Data fetched from server, not mutated locally
//! - Actions return values (TradingMatrixAction enum)
//! - No callbacks, pure render functions
//! - Expand/collapse state is UI-only (TradingMatrixState)
//!
//! # Type Architecture
//!
//! This module re-exports unified types from `ob_poc_types::trading_matrix` and
//! adds UI-specific extensions:
//!
//! - **Re-exported from ob_poc_types**: `TradingMatrixNodeId`, `TradingMatrixNodeType`,
//!   `TradingMatrixNode`, `StatusColor`, `TradingMatrixDocument`, `TradingMatrixResponse`,
//!   `BookingMatchCriteria`
//! - **UI-only (local)**: `TradingMatrixState`, `TradingMatrixAction`, colors, render functions

use std::collections::{HashMap, HashSet};

use egui::{Color32, Rect, RichText, Ui, Vec2};

use super::animation::{SpringConfig, SpringF32};

// =============================================================================
// RE-EXPORT UNIFIED TYPES FROM ob-poc-types
// =============================================================================

pub use ob_poc_types::trading_matrix::{
    BookingMatchCriteria, DocumentStatus, StatusColor, TradingMatrixDocument,
    TradingMatrixMetadata, TradingMatrixNode, TradingMatrixNodeId, TradingMatrixNodeType,
    TradingMatrixOp, TradingMatrixResponse,
};

// =============================================================================
// UI EXTENSIONS FOR UNIFIED TYPES
// =============================================================================

/// Extension trait for TradingMatrixNodeId to add UI-specific methods
pub trait TradingMatrixNodeIdExt {
    /// Get a string key for HashMap storage (used by TradingMatrixState)
    fn as_key(&self) -> String;

    /// Create a root node ID
    fn root() -> TradingMatrixNodeId;
}

impl TradingMatrixNodeIdExt for TradingMatrixNodeId {
    fn as_key(&self) -> String {
        self.0.join("/")
    }

    fn root() -> TradingMatrixNodeId {
        TradingMatrixNodeId(Vec::new())
    }
}

/// Extension trait for StatusColor to add egui color conversion
pub trait StatusColorExt {
    /// Convert to egui Color32
    fn to_color32(self) -> Color32;
}

impl StatusColorExt for StatusColor {
    fn to_color32(self) -> Color32 {
        match self {
            StatusColor::Green => Color32::from_rgb(34, 197, 94), // green-500
            StatusColor::Yellow => Color32::from_rgb(234, 179, 8), // yellow-500
            StatusColor::Red => Color32::from_rgb(239, 68, 68),   // red-500
            StatusColor::Gray => Color32::from_rgb(107, 114, 128), // gray-500
        }
    }
}

// =============================================================================
// UI WRAPPER FOR TRADING MATRIX
// =============================================================================

/// UI wrapper around TradingMatrixDocument with computed metadata for display.
///
/// This provides a simpler interface for the UI while the document holds the full AST.
#[derive(Debug, Clone)]
pub struct TradingMatrix {
    /// The underlying document (the AST)
    pub document: TradingMatrixDocument,
    /// Computed display metadata
    pub display_metadata: DisplayMetadata,
}

/// Display-focused metadata computed from the document
#[derive(Debug, Clone, Default)]
pub struct DisplayMetadata {
    /// Total instrument classes
    pub instrument_class_count: usize,
    /// Total markets
    pub market_count: usize,
    /// Total universe entries
    pub universe_entry_count: usize,
    /// Total SSIs
    pub ssi_count: usize,
    /// Total booking rules
    pub booking_rule_count: usize,
    /// Total settlement chains
    pub settlement_chain_count: usize,
    /// Total ISDA agreements
    pub isda_count: usize,
}

impl TradingMatrix {
    /// Create a new trading matrix from a document
    pub fn from_document(document: TradingMatrixDocument) -> Self {
        let display_metadata = Self::compute_display_metadata(&document);
        Self {
            document,
            display_metadata,
        }
    }

    /// Create a new empty trading matrix for a CBU
    pub fn new(cbu_id: &str, cbu_name: &str) -> Self {
        let document = TradingMatrixDocument::new(cbu_id, cbu_name);
        Self::from_document(document)
    }

    /// Create from API response
    pub fn from_response(response: TradingMatrixResponse) -> Self {
        let document = TradingMatrixDocument {
            cbu_id: response.cbu_id,
            cbu_name: response.cbu_name,
            version: 1,
            status: DocumentStatus::default(),
            children: response.children,
            total_leaf_count: response.total_leaf_count,
            metadata: TradingMatrixMetadata::default(),
            created_at: None,
            updated_at: None,
        };
        Self::from_document(document)
    }

    /// Get the root children (category nodes)
    pub fn children(&self) -> &[TradingMatrixNode] {
        &self.document.children
    }

    /// Compute display metadata from document
    fn compute_display_metadata(document: &TradingMatrixDocument) -> DisplayMetadata {
        let mut metadata = DisplayMetadata::default();

        fn visit_node(node: &TradingMatrixNode, metadata: &mut DisplayMetadata) {
            match &node.node_type {
                TradingMatrixNodeType::InstrumentClass { .. } => {
                    metadata.instrument_class_count += 1
                }
                TradingMatrixNodeType::Market { .. } => metadata.market_count += 1,
                TradingMatrixNodeType::Counterparty { .. } => metadata.market_count += 1,
                TradingMatrixNodeType::UniverseEntry { .. } => metadata.universe_entry_count += 1,
                TradingMatrixNodeType::Ssi { .. } => metadata.ssi_count += 1,
                TradingMatrixNodeType::BookingRule { .. } => metadata.booking_rule_count += 1,
                TradingMatrixNodeType::SettlementChain { .. } => {
                    metadata.settlement_chain_count += 1
                }
                TradingMatrixNodeType::IsdaAgreement { .. } => metadata.isda_count += 1,
                _ => {}
            }
            for child in &node.children {
                visit_node(child, metadata);
            }
        }

        for child in &document.children {
            visit_node(child, &mut metadata);
        }

        metadata
    }

    /// Recompute counts after modifications
    pub fn recompute_counts(&mut self) {
        self.document.compute_leaf_counts();
        self.display_metadata = Self::compute_display_metadata(&self.document);
    }
}

// =============================================================================
// TRADING MATRIX STATE (UI-only expand/collapse)
// =============================================================================

/// Manages expand/collapse state and animations for the trading matrix browser
#[derive(Debug, Clone)]
pub struct TradingMatrixState {
    /// Which nodes are expanded (by node ID key)
    expanded: HashSet<String>,
    /// Animation progress for each node (0.0 = collapsed, 1.0 = expanded)
    expand_progress: HashMap<String, SpringF32>,
    /// Currently selected node (for detail panel)
    selected_node: Option<String>,
    /// Currently hovered node
    hovered_node: Option<String>,
    /// Scroll position
    #[allow(dead_code)]
    scroll_offset: f32,
}

impl Default for TradingMatrixState {
    fn default() -> Self {
        Self::new()
    }
}

impl TradingMatrixState {
    pub fn new() -> Self {
        Self {
            expanded: HashSet::new(),
            expand_progress: HashMap::new(),
            selected_node: None,
            hovered_node: None,
            scroll_offset: 0.0,
        }
    }

    /// Toggle expand/collapse for a node
    pub fn toggle(&mut self, node_id: &TradingMatrixNodeId) {
        let key = node_id.as_key();
        if self.expanded.contains(&key) {
            self.collapse(node_id);
        } else {
            self.expand(node_id);
        }
    }

    /// Expand a node
    pub fn expand(&mut self, node_id: &TradingMatrixNodeId) {
        let key = node_id.as_key();
        self.expanded.insert(key.clone());
        self.expand_progress
            .entry(key)
            .or_insert_with(|| SpringF32::with_config(0.0, SpringConfig::FAST))
            .set_target(1.0);
    }

    /// Collapse a node
    pub fn collapse(&mut self, node_id: &TradingMatrixNodeId) {
        let key = node_id.as_key();
        self.expanded.remove(&key);
        if let Some(progress) = self.expand_progress.get_mut(&key) {
            progress.set_target(0.0);
        }
    }

    /// Check if a node is expanded
    pub fn is_expanded(&self, node_id: &TradingMatrixNodeId) -> bool {
        self.expanded.contains(&node_id.as_key())
    }

    /// Get expand animation progress (0.0 - 1.0)
    #[allow(dead_code)]
    pub fn get_expand_progress(&self, node_id: &TradingMatrixNodeId) -> f32 {
        self.expand_progress
            .get(&node_id.as_key())
            .map(|p| p.get())
            .unwrap_or(0.0)
    }

    /// Select a node (for showing detail panel)
    pub fn select(&mut self, node_id: Option<&TradingMatrixNodeId>) {
        self.selected_node = node_id.map(|id| id.as_key());
    }

    /// Get selected node key
    pub fn selected(&self) -> Option<&str> {
        self.selected_node.as_deref()
    }

    /// Set hovered node
    #[allow(dead_code)]
    pub fn set_hover(&mut self, node_id: Option<&TradingMatrixNodeId>) {
        self.hovered_node = node_id.map(|id| id.as_key());
    }

    /// Get hovered node key
    pub fn hovered(&self) -> Option<&str> {
        self.hovered_node.as_deref()
    }

    /// Expand to a node (expands all ancestors)
    #[allow(dead_code)]
    pub fn expand_to(&mut self, node_id: &TradingMatrixNodeId) {
        // Expand all ancestors
        for i in 0..node_id.0.len() {
            let ancestor = TradingMatrixNodeId(node_id.0[..i].to_vec());
            self.expand(&ancestor);
        }
    }

    /// Collapse all nodes
    pub fn collapse_all(&mut self) {
        let keys: Vec<String> = self.expanded.iter().cloned().collect();
        for key in keys {
            if let Some(progress) = self.expand_progress.get_mut(&key) {
                progress.set_target(0.0);
            }
        }
        self.expanded.clear();
    }

    /// Expand first level (root categories)
    #[allow(dead_code)]
    pub fn expand_first_level(&mut self, matrix: &TradingMatrix) {
        // Expand each top-level category
        for child in &matrix.document.children {
            self.expand(&child.id);
        }
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
// TRADING MATRIX ACTIONS
// =============================================================================

/// Actions returned from the trading matrix browser UI
#[derive(Debug, Clone, PartialEq)]
pub enum TradingMatrixAction {
    /// No action
    None,
    /// Toggle expand/collapse of a node
    ToggleExpand { node_id: TradingMatrixNodeId },
    /// Select a node (show in detail panel)
    SelectNode { node_id: TradingMatrixNodeId },
    /// Clear selection
    ClearSelection,
    /// Request to load children for a node (lazy loading)
    LoadChildren { node_id: TradingMatrixNodeId },
    /// Navigate to entity in main graph
    NavigateToEntity { entity_id: String },
    /// Open SSI detail
    OpenSsiDetail { ssi_id: String },
    /// Open ISDA detail
    OpenIsdaDetail { isda_id: String },
    /// Expand all nodes
    ExpandAll,
    /// Collapse all nodes
    CollapseAll,
    /// Expand to depth
    ExpandToDepth { depth: usize },
}

// =============================================================================
// COLORS
// =============================================================================

pub mod matrix_colors {
    use egui::Color32;

    pub fn panel_bg() -> Color32 {
        Color32::from_rgba_unmultiplied(25, 25, 30, 245)
    }

    pub fn header_bg() -> Color32 {
        Color32::from_rgba_unmultiplied(35, 35, 42, 255)
    }

    pub fn header_text() -> Color32 {
        Color32::from_rgb(220, 220, 230)
    }

    pub fn node_label() -> Color32 {
        Color32::from_rgb(200, 200, 210)
    }

    pub fn node_sublabel() -> Color32 {
        Color32::from_rgb(140, 140, 150)
    }

    pub fn count_text() -> Color32 {
        Color32::from_rgb(160, 160, 170)
    }

    pub fn hover_bg() -> Color32 {
        Color32::from_rgba_unmultiplied(60, 60, 75, 200)
    }

    pub fn selected_bg() -> Color32 {
        Color32::from_rgba_unmultiplied(59, 130, 246, 80)
    }

    pub fn expand_icon() -> Color32 {
        Color32::from_rgb(120, 120, 135)
    }

    pub fn tree_line() -> Color32 {
        Color32::from_rgb(50, 50, 60)
    }

    // Node type colors
    pub fn instrument_class() -> Color32 {
        Color32::from_rgb(147, 197, 253) // blue-300
    }

    pub fn market() -> Color32 {
        Color32::from_rgb(134, 239, 172) // green-300
    }

    pub fn counterparty() -> Color32 {
        Color32::from_rgb(253, 186, 116) // orange-300
    }

    pub fn universe_entry() -> Color32 {
        Color32::from_rgb(196, 181, 253) // violet-300
    }

    pub fn ssi() -> Color32 {
        Color32::from_rgb(252, 211, 77) // amber-300
    }

    pub fn booking_rule() -> Color32 {
        Color32::from_rgb(253, 164, 175) // rose-300
    }

    pub fn settlement_chain() -> Color32 {
        Color32::from_rgb(103, 232, 249) // cyan-300
    }

    pub fn tax_config() -> Color32 {
        Color32::from_rgb(190, 242, 100) // lime-300
    }

    pub fn isda() -> Color32 {
        Color32::from_rgb(249, 168, 212) // pink-300
    }
}

/// Get color for a node type
pub fn get_node_type_color(node_type: &TradingMatrixNodeType) -> Color32 {
    match node_type {
        TradingMatrixNodeType::Category { .. } => matrix_colors::header_text(),
        TradingMatrixNodeType::InstrumentClass { .. } => matrix_colors::instrument_class(),
        TradingMatrixNodeType::Market { .. } => matrix_colors::market(),
        TradingMatrixNodeType::Counterparty { .. } => matrix_colors::counterparty(),
        TradingMatrixNodeType::UniverseEntry { .. } => matrix_colors::universe_entry(),
        TradingMatrixNodeType::Ssi { .. } => matrix_colors::ssi(),
        TradingMatrixNodeType::BookingRule { .. } => matrix_colors::booking_rule(),
        TradingMatrixNodeType::SettlementChain { .. } => matrix_colors::settlement_chain(),
        TradingMatrixNodeType::SettlementHop { .. } => matrix_colors::settlement_chain(),
        TradingMatrixNodeType::TaxConfig { .. } => matrix_colors::tax_config(),
        TradingMatrixNodeType::TaxJurisdiction { .. } => matrix_colors::tax_config(),
        TradingMatrixNodeType::IsdaAgreement { .. } => matrix_colors::isda(),
        TradingMatrixNodeType::CsaAgreement { .. } => matrix_colors::isda(),
        TradingMatrixNodeType::ProductCoverage { .. } => matrix_colors::isda(),
        TradingMatrixNodeType::InvestmentManagerMandate { .. } => matrix_colors::counterparty(),
        TradingMatrixNodeType::PricingRule { .. } => matrix_colors::universe_entry(),
    }
}

/// Get icon for a node type
pub fn get_node_type_icon(node_type: &TradingMatrixNodeType) -> &'static str {
    match node_type {
        TradingMatrixNodeType::Category { .. } => "📁",
        TradingMatrixNodeType::InstrumentClass { is_otc, .. } => {
            if *is_otc {
                "📊"
            } else {
                "📈"
            }
        }
        TradingMatrixNodeType::Market { .. } => "🏛",
        TradingMatrixNodeType::Counterparty { .. } => "🤝",
        TradingMatrixNodeType::UniverseEntry { .. } => "🌐",
        TradingMatrixNodeType::Ssi { .. } => "📋",
        TradingMatrixNodeType::BookingRule { .. } => "📐",
        TradingMatrixNodeType::SettlementChain { .. } => "🔗",
        TradingMatrixNodeType::SettlementHop { .. } => "➡",
        TradingMatrixNodeType::TaxConfig { .. } => "💰",
        TradingMatrixNodeType::TaxJurisdiction { .. } => "🌍",
        TradingMatrixNodeType::IsdaAgreement { .. } => "📝",
        TradingMatrixNodeType::CsaAgreement { .. } => "🛡",
        TradingMatrixNodeType::ProductCoverage { .. } => "📦",
        TradingMatrixNodeType::InvestmentManagerMandate { .. } => "👔",
        TradingMatrixNodeType::PricingRule { .. } => "💹",
    }
}

// =============================================================================
// RENDER FUNCTIONS
// =============================================================================

/// Render the trading matrix browser panel
///
/// Returns an action if the user interacted with the browser.
/// EGUI-RULES: Pure function, returns action, no callbacks.
pub fn render_trading_matrix_browser(
    ui: &mut Ui,
    matrix: &TradingMatrix,
    state: &TradingMatrixState,
    max_height: f32,
) -> TradingMatrixAction {
    let mut action = TradingMatrixAction::None;

    // Panel frame
    egui::Frame::none()
        .fill(matrix_colors::panel_bg())
        .inner_margin(8.0)
        .rounding(6.0)
        .show(ui, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Trading Matrix")
                        .size(13.0)
                        .strong()
                        .color(matrix_colors::header_text()),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Collapse all
                    if ui.small_button("−").on_hover_text("Collapse all").clicked() {
                        action = TradingMatrixAction::CollapseAll;
                    }
                    // Expand all
                    if ui.small_button("+").on_hover_text("Expand all").clicked() {
                        action = TradingMatrixAction::ExpandAll;
                    }
                    // Clear selection
                    if state.selected().is_some() {
                        if ui
                            .small_button("×")
                            .on_hover_text("Clear selection")
                            .clicked()
                        {
                            action = TradingMatrixAction::ClearSelection;
                        }
                    }
                });
            });

            // Stats bar
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                stat_badge(
                    ui,
                    "📈",
                    matrix.display_metadata.instrument_class_count,
                    "Classes",
                );
                stat_badge(ui, "🏛", matrix.display_metadata.market_count, "Markets");
                stat_badge(ui, "📋", matrix.display_metadata.ssi_count, "SSIs");
                stat_badge(
                    ui,
                    "📐",
                    matrix.display_metadata.booking_rule_count,
                    "Rules",
                );
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Scrollable tree
            egui::ScrollArea::vertical()
                .max_height(max_height - 80.0)
                .show(ui, |ui| {
                    // Render from root's children (the category nodes)
                    for child in matrix.children() {
                        let child_action = render_matrix_node(ui, child, state, 0);
                        if child_action != TradingMatrixAction::None {
                            action = child_action;
                        }
                    }
                });
        });

    action
}

/// Render a stat badge
fn stat_badge(ui: &mut Ui, icon: &str, count: usize, label: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.label(RichText::new(icon).size(10.0));
        ui.label(
            RichText::new(format!("{}", count))
                .size(10.0)
                .color(matrix_colors::count_text()),
        );
        ui.label(
            RichText::new(label)
                .size(9.0)
                .color(matrix_colors::node_sublabel()),
        );
    });
}

/// Render a single node and its children recursively
fn render_matrix_node(
    ui: &mut Ui,
    node: &TradingMatrixNode,
    state: &TradingMatrixState,
    depth: usize,
) -> TradingMatrixAction {
    let mut action = TradingMatrixAction::None;
    let has_children = !node.children.is_empty();
    let is_expanded = state.is_expanded(&node.id);
    let is_selected = state.selected() == Some(&node.id.as_key());
    let is_hovered = state.hovered() == Some(&node.id.as_key());

    // Indentation
    let indent = depth as f32 * 16.0;

    // Row area
    let row_rect = ui.available_rect_before_wrap();
    let row_height = 24.0;
    let row_rect = Rect::from_min_size(row_rect.min, Vec2::new(ui.available_width(), row_height));

    // Background
    if is_selected {
        ui.painter()
            .rect_filled(row_rect, 3.0, matrix_colors::selected_bg());
    } else if is_hovered {
        ui.painter()
            .rect_filled(row_rect, 3.0, matrix_colors::hover_bg());
    }

    ui.horizontal(|ui| {
        // Indent
        ui.add_space(indent);

        // Tree line (vertical connector)
        if depth > 0 {
            let line_x = row_rect.min.x + indent - 8.0;
            let line_rect = Rect::from_min_size(
                egui::pos2(line_x, row_rect.min.y),
                Vec2::new(1.0, row_height),
            );
            ui.painter()
                .rect_filled(line_rect, 0.0, matrix_colors::tree_line());
        }

        // Expand/collapse button
        if has_children {
            let icon = if is_expanded { "▼" } else { "▶" };
            let response = ui.add(
                egui::Label::new(
                    RichText::new(icon)
                        .size(9.0)
                        .color(matrix_colors::expand_icon()),
                )
                .sense(egui::Sense::click()),
            );
            if response.clicked() {
                action = TradingMatrixAction::ToggleExpand {
                    node_id: node.id.clone(),
                };
            }
        } else {
            ui.add_space(12.0);
        }

        // Type color indicator
        let type_color = get_node_type_color(&node.node_type);
        let indicator_rect = ui.available_rect_before_wrap();
        let indicator_rect = Rect::from_min_size(
            indicator_rect.min + Vec2::new(0.0, 6.0),
            Vec2::new(10.0, 10.0),
        );
        ui.painter().rect_filled(indicator_rect, 3.0, type_color);
        ui.add_space(14.0);

        // Icon
        let icon = get_node_type_icon(&node.node_type);
        ui.label(RichText::new(icon).size(11.0));
        ui.add_space(2.0);

        // Label (clickable for selection)
        let label_response = ui.add(
            egui::Label::new(
                RichText::new(&node.label)
                    .size(11.0)
                    .color(matrix_colors::node_label()),
            )
            .sense(egui::Sense::click()),
        );

        if label_response.clicked() {
            action = TradingMatrixAction::SelectNode {
                node_id: node.id.clone(),
            };
        }

        // Sublabel
        if let Some(ref sublabel) = node.sublabel {
            ui.label(
                RichText::new(sublabel)
                    .size(10.0)
                    .color(matrix_colors::node_sublabel()),
            );
        }

        // Status indicator
        if let Some(status) = node.status_color {
            ui.add_space(4.0);
            let status_rect = Rect::from_min_size(
                ui.available_rect_before_wrap().min + Vec2::new(0.0, 8.0),
                Vec2::new(6.0, 6.0),
            );
            ui.painter()
                .circle_filled(status_rect.center(), 3.0, status.to_color32());
            ui.add_space(10.0);
        }

        // Count badge (for non-leaf nodes)
        if has_children && node.leaf_count > 0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("({})", node.leaf_count))
                        .size(9.0)
                        .color(matrix_colors::count_text()),
                );
            });
        }
    });

    ui.add_space(1.0);

    // Render children if expanded
    if has_children && is_expanded {
        for child in &node.children {
            let child_action = render_matrix_node(ui, child, state, depth + 1);
            if child_action != TradingMatrixAction::None {
                action = child_action;
            }
        }
    }

    action
}

/// Render a detail panel for the selected node
pub fn render_node_detail_panel(ui: &mut Ui, node: &TradingMatrixNode) -> TradingMatrixAction {
    let mut action = TradingMatrixAction::None;

    egui::Frame::none()
        .fill(matrix_colors::header_bg())
        .inner_margin(12.0)
        .rounding(6.0)
        .show(ui, |ui| {
            // Header with icon and title
            ui.horizontal(|ui| {
                let icon = get_node_type_icon(&node.node_type);
                ui.label(RichText::new(icon).size(18.0));
                ui.label(
                    RichText::new(&node.label)
                        .size(14.0)
                        .strong()
                        .color(matrix_colors::header_text()),
                );
            });

            if let Some(ref sublabel) = node.sublabel {
                ui.label(
                    RichText::new(sublabel)
                        .size(11.0)
                        .color(matrix_colors::node_sublabel()),
                );
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Type-specific details
            match &node.node_type {
                TradingMatrixNodeType::Category { name } => {
                    detail_row(ui, "Category", name);
                }
                TradingMatrixNodeType::InstrumentClass {
                    class_code,
                    cfi_prefix,
                    is_otc,
                } => {
                    detail_row(ui, "Code", class_code);
                    if let Some(cfi) = cfi_prefix {
                        detail_row(ui, "CFI Prefix", cfi);
                    }
                    detail_row(ui, "Type", if *is_otc { "OTC" } else { "Listed" });
                }
                TradingMatrixNodeType::Market {
                    mic,
                    market_name,
                    country_code,
                } => {
                    detail_row(ui, "MIC", mic);
                    detail_row(ui, "Name", market_name);
                    detail_row(ui, "Country", country_code);
                }
                TradingMatrixNodeType::Counterparty {
                    entity_id,
                    entity_name,
                    lei,
                } => {
                    detail_row(ui, "Name", entity_name);
                    if let Some(lei) = lei {
                        detail_row(ui, "LEI", lei);
                    }
                    if ui.button("View Entity").clicked() {
                        action = TradingMatrixAction::NavigateToEntity {
                            entity_id: entity_id.clone(),
                        };
                    }
                }
                TradingMatrixNodeType::UniverseEntry {
                    currencies,
                    settlement_types,
                    is_held,
                    is_traded,
                    ..
                } => {
                    detail_row(ui, "Currencies", &currencies.join(", "));
                    detail_row(ui, "Settlement", &settlement_types.join(", "));
                    detail_row(ui, "Held", if *is_held { "Yes" } else { "No" });
                    detail_row(ui, "Traded", if *is_traded { "Yes" } else { "No" });
                }
                TradingMatrixNodeType::Ssi {
                    ssi_id,
                    ssi_name,
                    ssi_type,
                    status,
                    safekeeping_account,
                    safekeeping_bic,
                    cash_account,
                    cash_bic,
                    ..
                } => {
                    detail_row(ui, "Name", ssi_name);
                    detail_row(ui, "Type", ssi_type);
                    detail_row(ui, "Status", status);
                    if let Some(acc) = safekeeping_account {
                        detail_row(ui, "Safekeeping", acc);
                    }
                    if let Some(bic) = safekeeping_bic {
                        detail_row(ui, "SK BIC", bic);
                    }
                    if let Some(acc) = cash_account {
                        detail_row(ui, "Cash Acct", acc);
                    }
                    if let Some(bic) = cash_bic {
                        detail_row(ui, "Cash BIC", bic);
                    }
                    if ui.button("View SSI Details").clicked() {
                        action = TradingMatrixAction::OpenSsiDetail {
                            ssi_id: ssi_id.clone(),
                        };
                    }
                }
                TradingMatrixNodeType::BookingRule {
                    rule_name,
                    priority,
                    specificity_score,
                    is_active,
                    ..
                } => {
                    detail_row(ui, "Name", rule_name);
                    detail_row(ui, "Priority", &priority.to_string());
                    detail_row(ui, "Specificity", &specificity_score.to_string());
                    detail_row(ui, "Active", if *is_active { "Yes" } else { "No" });
                }
                TradingMatrixNodeType::SettlementChain {
                    chain_name,
                    hop_count,
                    is_active,
                    ..
                } => {
                    detail_row(ui, "Name", chain_name);
                    detail_row(ui, "Hops", &hop_count.to_string());
                    detail_row(ui, "Active", if *is_active { "Yes" } else { "No" });
                }
                TradingMatrixNodeType::SettlementHop {
                    sequence,
                    intermediary_bic,
                    intermediary_name,
                    role,
                    ..
                } => {
                    detail_row(ui, "Sequence", &sequence.to_string());
                    detail_row(ui, "Role", role);
                    if let Some(name) = intermediary_name {
                        detail_row(ui, "Intermediary", name);
                    }
                    if let Some(bic) = intermediary_bic {
                        detail_row(ui, "BIC", bic);
                    }
                }
                TradingMatrixNodeType::TaxConfig {
                    investor_type,
                    tax_exempt,
                    documentation_status,
                    ..
                } => {
                    detail_row(ui, "Investor Type", investor_type);
                    detail_row(ui, "Tax Exempt", if *tax_exempt { "Yes" } else { "No" });
                    if let Some(doc_status) = documentation_status {
                        detail_row(ui, "Documentation", doc_status);
                    }
                }
                TradingMatrixNodeType::TaxJurisdiction {
                    jurisdiction_code,
                    jurisdiction_name,
                    default_withholding_rate,
                    reclaim_available,
                    ..
                } => {
                    detail_row(ui, "Jurisdiction", jurisdiction_name);
                    detail_row(ui, "Code", jurisdiction_code);
                    if let Some(rate) = default_withholding_rate {
                        detail_row(ui, "Default Rate", &format!("{:.1}%", rate));
                    }
                    detail_row(
                        ui,
                        "Reclaim Available",
                        if *reclaim_available { "Yes" } else { "No" },
                    );
                }
                TradingMatrixNodeType::IsdaAgreement {
                    isda_id,
                    counterparty_name,
                    governing_law,
                    agreement_date,
                    ..
                } => {
                    detail_row(ui, "Counterparty", counterparty_name);
                    if let Some(law) = governing_law {
                        detail_row(ui, "Law", law);
                    }
                    if let Some(date) = agreement_date {
                        detail_row(ui, "Date", date);
                    }
                    if ui.button("View ISDA Details").clicked() {
                        action = TradingMatrixAction::OpenIsdaDetail {
                            isda_id: isda_id.clone(),
                        };
                    }
                }
                TradingMatrixNodeType::CsaAgreement {
                    csa_type,
                    threshold_currency,
                    threshold_amount,
                    ..
                } => {
                    detail_row(ui, "Type", csa_type);
                    if let Some(currency) = threshold_currency {
                        detail_row(ui, "Currency", currency);
                    }
                    if let Some(amount) = threshold_amount {
                        detail_row(ui, "Threshold", &format!("{:.0}", amount));
                    }
                }
                TradingMatrixNodeType::ProductCoverage {
                    asset_class,
                    base_products,
                    ..
                } => {
                    detail_row(ui, "Asset Class", asset_class);
                    detail_row(ui, "Products", &base_products.join(", "));
                }
                TradingMatrixNodeType::InvestmentManagerMandate {
                    manager_name,
                    role,
                    can_trade,
                    can_settle,
                    ..
                } => {
                    detail_row(ui, "Manager", manager_name);
                    detail_row(ui, "Role", role);
                    detail_row(ui, "Can Trade", if *can_trade { "Yes" } else { "No" });
                    detail_row(ui, "Can Settle", if *can_settle { "Yes" } else { "No" });
                }
                TradingMatrixNodeType::PricingRule {
                    source,
                    priority,
                    fallback_source,
                    price_type,
                    ..
                } => {
                    detail_row(ui, "Source", source);
                    detail_row(ui, "Priority", &priority.to_string());
                    if let Some(fallback) = fallback_source {
                        detail_row(ui, "Fallback", fallback);
                    }
                    if let Some(pt) = price_type {
                        detail_row(ui, "Price Type", pt);
                    }
                }
            }
        });

    action
}

/// Render a detail row
fn detail_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{}:", label))
                .size(10.0)
                .color(matrix_colors::node_sublabel()),
        );
        ui.label(
            RichText::new(value)
                .size(10.0)
                .color(matrix_colors::node_label()),
        );
    });
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_path() {
        let root = TradingMatrixNodeIdExt::root();
        assert_eq!(root.depth(), 0);
        assert_eq!(root.as_key(), "");

        let child = root.child("EQUITY");
        assert_eq!(child.depth(), 0); // depth is len() - 1
        assert_eq!(child.as_key(), "EQUITY");

        let grandchild = child.child("XNYS");
        assert_eq!(grandchild.depth(), 1);
        assert_eq!(grandchild.as_key(), "EQUITY/XNYS");
    }

    #[test]
    fn test_trading_matrix_creation() {
        let mut matrix = TradingMatrix::new("cbu-123", "Test Fund");

        let equity_id = TradingMatrixNodeId::category("UNIVERSE").child("EQUITY");
        let equity = TradingMatrixNode::new(
            equity_id.clone(),
            TradingMatrixNodeType::InstrumentClass {
                class_code: "EQUITY".to_string(),
                cfi_prefix: Some("E".to_string()),
                is_otc: false,
            },
            "Equities",
        );

        // Ensure universe category exists and add equity
        matrix.document.ensure_category("Trading Universe");
        if let Some(universe) = matrix.document.children.first_mut() {
            universe.children.push(equity);
        }
        matrix.recompute_counts();

        assert_eq!(matrix.display_metadata.instrument_class_count, 1);
    }

    #[test]
    fn test_state_expand_collapse() {
        let mut state = TradingMatrixState::new();
        let id = TradingMatrixNodeId::category("UNIVERSE").child("EQUITY");

        assert!(!state.is_expanded(&id));

        state.expand(&id);
        assert!(state.is_expanded(&id));

        state.collapse(&id);
        assert!(!state.is_expanded(&id));

        state.toggle(&id);
        assert!(state.is_expanded(&id));
    }

    #[test]
    fn test_from_response() {
        let response = TradingMatrixResponse {
            cbu_id: "cbu-123".to_string(),
            cbu_name: "Test Fund".to_string(),
            children: vec![],
            total_leaf_count: 0,
        };

        let matrix = TradingMatrix::from_response(response);
        assert_eq!(matrix.document.cbu_id, "cbu-123");
        assert_eq!(matrix.document.cbu_name, "Test Fund");
    }
}
