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
//! # Taxonomy Structure
//! ```text
//! CBU (root)
//! └── InstrumentClass (EQUITY, GOVT_BOND, CORP_BOND, ETF, OTC_IRS, OTC_FX)
//!     └── Market (XETR, XLON, XNYS) OR Counterparty (for OTC)
//!         └── UniverseEntry (currency, settlement_type, is_held, is_traded)
//!             ├── SSI (ssi_name, ssi_type, status)
//!             │   └── BookingRule (priority, specificity)
//!             ├── SettlementChain (chain_name, hop_count)
//!             │   └── Hop (sequence, location_bic, instruction_type)
//!             ├── TaxConfig (jurisdiction, applicable_rate)
//!             │   └── TreatyRate (source, investor, rate)
//!             └── IsdaAgreement (counterparty, governing_law) [OTC only]
//!                 └── CsaAgreement (csa_type, collateral_currency)
//! ```

use std::collections::{HashMap, HashSet};

use egui::{Color32, Rect, RichText, Ui, Vec2};
use serde::{Deserialize, Serialize};

use super::animation::{SpringConfig, SpringF32};

// =============================================================================
// NODE TYPES
// =============================================================================

/// Unique identifier for a node in the trading matrix tree
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradingMatrixNodeId {
    /// Path from root (e.g., ["EQUITY", "XNYS", "USD"])
    pub path: Vec<String>,
}

impl TradingMatrixNodeId {
    pub fn root() -> Self {
        Self { path: Vec::new() }
    }

    pub fn child(&self, segment: &str) -> Self {
        let mut path = self.path.clone();
        path.push(segment.to_string());
        Self { path }
    }

    pub fn as_key(&self) -> String {
        self.path.join("/")
    }

    pub fn depth(&self) -> usize {
        self.path.len()
    }
}

/// The type of a node in the trading matrix hierarchy
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TradingMatrixNodeType {
    /// Root CBU node
    Cbu { cbu_id: String, cbu_name: String },
    /// Instrument class (EQUITY, GOVT_BOND, OTC_IRS, etc.)
    InstrumentClass {
        class_code: String,
        cfi_prefix: Option<String>,
        is_otc: bool,
    },
    /// Exchange/market for listed instruments
    Market {
        mic: String,
        market_name: String,
        country_code: String,
    },
    /// Counterparty for OTC instruments
    Counterparty {
        entity_id: String,
        entity_name: String,
        lei: Option<String>,
    },
    /// Universe entry (instrument + market/counterparty + currency)
    UniverseEntry {
        universe_id: String,
        currencies: Vec<String>,
        settlement_types: Vec<String>,
        is_held: bool,
        is_traded: bool,
    },
    /// Standing Settlement Instruction
    Ssi {
        ssi_id: String,
        ssi_name: String,
        ssi_type: String, // SECURITIES, CASH, COLLATERAL
        status: String,   // PENDING, ACTIVE, SUSPENDED
        safekeeping_account: Option<String>,
        safekeeping_bic: Option<String>,
        cash_account: Option<String>,
        cash_bic: Option<String>,
    },
    /// Booking rule that routes to an SSI
    BookingRule {
        rule_id: String,
        rule_name: String,
        priority: i32,
        specificity_score: i32,
        is_active: bool,
    },
    /// Settlement chain for multi-hop settlement
    SettlementChain {
        chain_id: String,
        chain_name: String,
        hop_count: usize,
        is_active: bool,
    },
    /// A hop in a settlement chain
    SettlementHop {
        hop_id: String,
        sequence: i32,
        location_bic: String,
        location_name: String,
        instruction_type: String,
    },
    /// Tax configuration for a jurisdiction
    TaxConfig {
        config_id: String,
        jurisdiction_code: String,
        jurisdiction_name: String,
        applicable_rate: Option<f64>,
        has_reclaim: bool,
    },
    /// Treaty rate between jurisdictions
    TreatyRate {
        treaty_id: String,
        source_jurisdiction: String,
        investor_jurisdiction: String,
        rate_percent: f64,
        income_type: String,
    },
    /// ISDA master agreement (for OTC)
    IsdaAgreement {
        isda_id: String,
        counterparty_name: String,
        governing_law: String,
        agreement_date: Option<String>,
    },
    /// CSA under an ISDA
    CsaAgreement {
        csa_id: String,
        csa_type: String, // VM, IM
        collateral_currency: String,
        threshold: Option<f64>,
    },
    /// Investment Manager assignment
    InvestmentManager {
        assignment_id: String,
        im_name: String,
        im_entity_id: String,
        scope_description: Option<String>,
    },
    /// Pricing configuration
    PricingConfig {
        config_id: String,
        pricing_source: String,
        priority: i32,
    },
}

/// A node in the trading matrix hierarchy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMatrixNode {
    /// Unique identifier for this node
    pub id: TradingMatrixNodeId,
    /// Node type with type-specific data
    pub node_type: TradingMatrixNodeType,
    /// Display label
    pub label: String,
    /// Optional sublabel (e.g., jurisdiction, status)
    pub sublabel: Option<String>,
    /// Child nodes
    pub children: Vec<TradingMatrixNode>,
    /// Aggregate count of leaf nodes below this node
    pub leaf_count: usize,
    /// Status indicator color (green=active, yellow=pending, red=suspended)
    pub status_color: Option<StatusColor>,
    /// Whether this node has been expanded (for lazy loading)
    pub is_loaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusColor {
    Green,
    Yellow,
    Red,
    Gray,
}

impl StatusColor {
    pub fn to_color32(self) -> Color32 {
        match self {
            StatusColor::Green => Color32::from_rgb(34, 197, 94), // green-500
            StatusColor::Yellow => Color32::from_rgb(234, 179, 8), // yellow-500
            StatusColor::Red => Color32::from_rgb(239, 68, 68),   // red-500
            StatusColor::Gray => Color32::from_rgb(107, 114, 128), // gray-500
        }
    }
}

impl TradingMatrixNode {
    /// Create a new node
    pub fn new(id: TradingMatrixNodeId, node_type: TradingMatrixNodeType, label: &str) -> Self {
        Self {
            id,
            node_type,
            label: label.to_string(),
            sublabel: None,
            children: Vec::new(),
            leaf_count: 0,
            status_color: None,
            is_loaded: true,
        }
    }

    /// Builder: add sublabel
    pub fn with_sublabel(mut self, sublabel: &str) -> Self {
        self.sublabel = Some(sublabel.to_string());
        self
    }

    /// Builder: add status color
    pub fn with_status(mut self, status: StatusColor) -> Self {
        self.status_color = Some(status);
        self
    }

    /// Builder: add a child
    pub fn with_child(mut self, child: TradingMatrixNode) -> Self {
        self.children.push(child);
        self
    }

    /// Builder: set children
    pub fn with_children(mut self, children: Vec<TradingMatrixNode>) -> Self {
        self.children = children;
        self
    }

    /// Check if this is a leaf node (no children)
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Get depth in tree
    pub fn depth(&self) -> usize {
        self.id.depth()
    }

    /// Recursively compute leaf counts
    pub fn compute_leaf_counts(&mut self) {
        if self.is_leaf() {
            self.leaf_count = 1;
        } else {
            for child in &mut self.children {
                child.compute_leaf_counts();
            }
            self.leaf_count = self.children.iter().map(|c| c.leaf_count).sum();
        }
    }

    /// Find a node by ID
    pub fn find(&self, id: &TradingMatrixNodeId) -> Option<&TradingMatrixNode> {
        if &self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find(id) {
                return Some(found);
            }
        }
        None
    }

    /// Find a mutable node by ID
    pub fn find_mut(&mut self, id: &TradingMatrixNodeId) -> Option<&mut TradingMatrixNode> {
        if &self.id == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut(id) {
                return Some(found);
            }
        }
        None
    }
}

// =============================================================================
// TRADING MATRIX (Complete Tree)
// =============================================================================

/// The complete trading matrix tree for a CBU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMatrix {
    /// Root node (the CBU)
    pub root: TradingMatrixNode,
    /// Metadata about the matrix
    pub metadata: TradingMatrixMetadata,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TradingMatrixMetadata {
    /// CBU ID
    pub cbu_id: String,
    /// CBU name
    pub cbu_name: String,
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
    /// Last updated timestamp
    pub last_updated: Option<String>,
}

impl TradingMatrix {
    /// Create a new trading matrix with a CBU root
    pub fn new(cbu_id: &str, cbu_name: &str) -> Self {
        let root_id = TradingMatrixNodeId::root();
        let root = TradingMatrixNode::new(
            root_id,
            TradingMatrixNodeType::Cbu {
                cbu_id: cbu_id.to_string(),
                cbu_name: cbu_name.to_string(),
            },
            cbu_name,
        );

        Self {
            root,
            metadata: TradingMatrixMetadata {
                cbu_id: cbu_id.to_string(),
                cbu_name: cbu_name.to_string(),
                ..Default::default()
            },
        }
    }

    /// Add an instrument class to the root
    pub fn add_instrument_class(&mut self, node: TradingMatrixNode) {
        self.root.children.push(node);
        self.metadata.instrument_class_count = self.root.children.len();
    }

    /// Recompute all counts
    pub fn recompute_counts(&mut self) {
        self.root.compute_leaf_counts();
        self.metadata.instrument_class_count = self.root.children.len();
        // Additional counts computed from tree traversal
        self.compute_metadata_counts();
    }

    fn compute_metadata_counts(&mut self) {
        let mut market_count = 0;
        let mut universe_count = 0;
        let mut ssi_count = 0;
        let mut rule_count = 0;
        let mut chain_count = 0;
        let mut isda_count = 0;

        self.visit_all(&mut |node| match &node.node_type {
            TradingMatrixNodeType::Market { .. } => market_count += 1,
            TradingMatrixNodeType::Counterparty { .. } => market_count += 1,
            TradingMatrixNodeType::UniverseEntry { .. } => universe_count += 1,
            TradingMatrixNodeType::Ssi { .. } => ssi_count += 1,
            TradingMatrixNodeType::BookingRule { .. } => rule_count += 1,
            TradingMatrixNodeType::SettlementChain { .. } => chain_count += 1,
            TradingMatrixNodeType::IsdaAgreement { .. } => isda_count += 1,
            _ => {}
        });

        self.metadata.market_count = market_count;
        self.metadata.universe_entry_count = universe_count;
        self.metadata.ssi_count = ssi_count;
        self.metadata.booking_rule_count = rule_count;
        self.metadata.settlement_chain_count = chain_count;
        self.metadata.isda_count = isda_count;
    }

    /// Visit all nodes with a callback
    fn visit_all<F: FnMut(&TradingMatrixNode)>(&self, f: &mut F) {
        Self::visit_node(&self.root, f);
    }

    fn visit_node<F: FnMut(&TradingMatrixNode)>(node: &TradingMatrixNode, f: &mut F) {
        f(node);
        for child in &node.children {
            Self::visit_node(child, f);
        }
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
    pub fn set_hover(&mut self, node_id: Option<&TradingMatrixNodeId>) {
        self.hovered_node = node_id.map(|id| id.as_key());
    }

    /// Get hovered node key
    pub fn hovered(&self) -> Option<&str> {
        self.hovered_node.as_deref()
    }

    /// Expand to a node (expands all ancestors)
    pub fn expand_to(&mut self, node_id: &TradingMatrixNodeId) {
        // Expand all ancestors
        for i in 0..node_id.path.len() {
            let ancestor = TradingMatrixNodeId {
                path: node_id.path[..i].to_vec(),
            };
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

    /// Expand first level (instrument classes)
    pub fn expand_first_level(&mut self, matrix: &TradingMatrix) {
        // Expand root
        self.expand(&matrix.root.id);
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

    pub fn count_badge() -> Color32 {
        Color32::from_rgb(80, 80, 90)
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
        TradingMatrixNodeType::Cbu { .. } => Color32::WHITE,
        TradingMatrixNodeType::InstrumentClass { .. } => matrix_colors::instrument_class(),
        TradingMatrixNodeType::Market { .. } => matrix_colors::market(),
        TradingMatrixNodeType::Counterparty { .. } => matrix_colors::counterparty(),
        TradingMatrixNodeType::UniverseEntry { .. } => matrix_colors::universe_entry(),
        TradingMatrixNodeType::Ssi { .. } => matrix_colors::ssi(),
        TradingMatrixNodeType::BookingRule { .. } => matrix_colors::booking_rule(),
        TradingMatrixNodeType::SettlementChain { .. } => matrix_colors::settlement_chain(),
        TradingMatrixNodeType::SettlementHop { .. } => matrix_colors::settlement_chain(),
        TradingMatrixNodeType::TaxConfig { .. } => matrix_colors::tax_config(),
        TradingMatrixNodeType::TreatyRate { .. } => matrix_colors::tax_config(),
        TradingMatrixNodeType::IsdaAgreement { .. } => matrix_colors::isda(),
        TradingMatrixNodeType::CsaAgreement { .. } => matrix_colors::isda(),
        TradingMatrixNodeType::InvestmentManager { .. } => matrix_colors::counterparty(),
        TradingMatrixNodeType::PricingConfig { .. } => matrix_colors::universe_entry(),
    }
}

/// Get icon for a node type
pub fn get_node_type_icon(node_type: &TradingMatrixNodeType) -> &'static str {
    match node_type {
        TradingMatrixNodeType::Cbu { .. } => "🏢",
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
        TradingMatrixNodeType::TreatyRate { .. } => "📜",
        TradingMatrixNodeType::IsdaAgreement { .. } => "📝",
        TradingMatrixNodeType::CsaAgreement { .. } => "🛡",
        TradingMatrixNodeType::InvestmentManager { .. } => "👔",
        TradingMatrixNodeType::PricingConfig { .. } => "💹",
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
                stat_badge(ui, "📈", matrix.metadata.instrument_class_count, "Classes");
                stat_badge(ui, "🏛", matrix.metadata.market_count, "Markets");
                stat_badge(ui, "📋", matrix.metadata.ssi_count, "SSIs");
                stat_badge(ui, "📐", matrix.metadata.booking_rule_count, "Rules");
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Scrollable tree
            egui::ScrollArea::vertical()
                .max_height(max_height - 80.0)
                .show(ui, |ui| {
                    // Render from root's children (skip CBU root in display)
                    for child in &matrix.root.children {
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
                    location_bic,
                    location_name,
                    instruction_type,
                    ..
                } => {
                    detail_row(ui, "Sequence", &sequence.to_string());
                    detail_row(ui, "Location", location_name);
                    detail_row(ui, "BIC", location_bic);
                    detail_row(ui, "Instruction", instruction_type);
                }
                TradingMatrixNodeType::TaxConfig {
                    jurisdiction_code,
                    jurisdiction_name,
                    applicable_rate,
                    has_reclaim,
                    ..
                } => {
                    detail_row(ui, "Jurisdiction", jurisdiction_name);
                    detail_row(ui, "Code", jurisdiction_code);
                    if let Some(rate) = applicable_rate {
                        detail_row(ui, "Rate", &format!("{:.1}%", rate));
                    }
                    detail_row(ui, "Reclaim", if *has_reclaim { "Yes" } else { "No" });
                }
                TradingMatrixNodeType::TreatyRate {
                    source_jurisdiction,
                    investor_jurisdiction,
                    rate_percent,
                    income_type,
                    ..
                } => {
                    detail_row(ui, "Source", source_jurisdiction);
                    detail_row(ui, "Investor", investor_jurisdiction);
                    detail_row(ui, "Rate", &format!("{:.1}%", rate_percent));
                    detail_row(ui, "Income Type", income_type);
                }
                TradingMatrixNodeType::IsdaAgreement {
                    isda_id,
                    counterparty_name,
                    governing_law,
                    agreement_date,
                    ..
                } => {
                    detail_row(ui, "Counterparty", counterparty_name);
                    detail_row(ui, "Law", governing_law);
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
                    collateral_currency,
                    threshold,
                    ..
                } => {
                    detail_row(ui, "Type", csa_type);
                    detail_row(ui, "Currency", collateral_currency);
                    if let Some(thresh) = threshold {
                        detail_row(ui, "Threshold", &format!("{:.0}", thresh));
                    }
                }
                TradingMatrixNodeType::InvestmentManager {
                    im_name,
                    scope_description,
                    ..
                } => {
                    detail_row(ui, "Name", im_name);
                    if let Some(scope) = scope_description {
                        detail_row(ui, "Scope", scope);
                    }
                }
                TradingMatrixNodeType::PricingConfig {
                    pricing_source,
                    priority,
                    ..
                } => {
                    detail_row(ui, "Source", pricing_source);
                    detail_row(ui, "Priority", &priority.to_string());
                }
                TradingMatrixNodeType::Cbu { cbu_name, .. } => {
                    detail_row(ui, "CBU", cbu_name);
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
        let root = TradingMatrixNodeId::root();
        assert_eq!(root.depth(), 0);
        assert_eq!(root.as_key(), "");

        let child = root.child("EQUITY");
        assert_eq!(child.depth(), 1);
        assert_eq!(child.as_key(), "EQUITY");

        let grandchild = child.child("XNYS");
        assert_eq!(grandchild.depth(), 2);
        assert_eq!(grandchild.as_key(), "EQUITY/XNYS");
    }

    #[test]
    fn test_trading_matrix_creation() {
        let mut matrix = TradingMatrix::new("cbu-123", "Test Fund");

        let equity_id = TradingMatrixNodeId::root().child("EQUITY");
        let equity = TradingMatrixNode::new(
            equity_id.clone(),
            TradingMatrixNodeType::InstrumentClass {
                class_code: "EQUITY".to_string(),
                cfi_prefix: Some("E".to_string()),
                is_otc: false,
            },
            "Equities",
        );

        matrix.add_instrument_class(equity);
        assert_eq!(matrix.metadata.instrument_class_count, 1);
    }

    #[test]
    fn test_state_expand_collapse() {
        let mut state = TradingMatrixState::new();
        let id = TradingMatrixNodeId::root().child("EQUITY");

        assert!(!state.is_expanded(&id));

        state.expand(&id);
        assert!(state.is_expanded(&id));

        state.collapse(&id);
        assert!(!state.is_expanded(&id));

        state.toggle(&id);
        assert!(state.is_expanded(&id));
    }

    #[test]
    fn test_find_node() {
        let mut matrix = TradingMatrix::new("cbu-123", "Test Fund");

        let equity_id = TradingMatrixNodeId::root().child("EQUITY");
        let xnys_id = equity_id.child("XNYS");

        let xnys = TradingMatrixNode::new(
            xnys_id.clone(),
            TradingMatrixNodeType::Market {
                mic: "XNYS".to_string(),
                market_name: "New York Stock Exchange".to_string(),
                country_code: "US".to_string(),
            },
            "NYSE",
        );

        let equity = TradingMatrixNode::new(
            equity_id.clone(),
            TradingMatrixNodeType::InstrumentClass {
                class_code: "EQUITY".to_string(),
                cfi_prefix: None,
                is_otc: false,
            },
            "Equities",
        )
        .with_child(xnys);

        matrix.add_instrument_class(equity);

        // Find EQUITY
        let found = matrix.root.find(&equity_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().label, "Equities");

        // Find XNYS
        let found = matrix.root.find(&xnys_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().label, "NYSE");
    }
}
