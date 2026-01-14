//! Viewport Focus Rendering Module
//!
//! Provides egui rendering for the CBU Viewport focus state machine.
//! Follows EGUI-RULES: pure functions, returns actions, no callbacks.
//!
//! # Architecture
//!
//! ```text
//! ViewportState (from session)
//!        │
//!        ▼
//! ViewportRenderer (draws focus-aware overlays)
//!        │
//!        ├──► ConfidenceZoneRenderer (dashed/ghost rendering)
//!        ├──► EnhanceLevelIndicator (HUD showing L0-L4)
//!        ├──► FocusBreadcrumbs (navigation path)
//!        └──► FocusRing (animated selection indicator)
//! ```
//!
//! # Design Principles (EGUI-RULES Compliant)
//!
//! - Data flows IN via function parameters
//! - Actions flow OUT via return values (ViewportAction enum)
//! - No callbacks, no mutable state beyond local render state
//! - Pure render functions with minimal side effects

use egui::{Color32, Painter, Pos2, Rect, Stroke, Ui, Vec2};
use ob_poc_types::viewport::{
    CbuViewType, ConcreteEntityType, ConfidenceZone, EnhanceArg, FocusManager, InstrumentType,
    ViewportFilters, ViewportFocusState, ViewportState,
};
use uuid::Uuid;

use super::animation::{SpringConfig, SpringF32, SpringVec2};

// =============================================================================
// VIEWPORT ACTIONS
// =============================================================================

/// Actions returned from viewport UI interactions
#[derive(Debug, Clone, PartialEq)]
pub enum ViewportAction {
    /// No action
    None,
    /// Enhance current focus level
    Enhance { arg: EnhanceArg },
    /// Focus on a CBU
    FocusCbu { cbu_id: Uuid },
    /// Focus on an entity within current CBU
    FocusEntity { entity_id: Uuid },
    /// Focus on instrument matrix
    FocusMatrix,
    /// Focus on instrument type within matrix
    FocusInstrumentType { instrument_type: InstrumentType },
    /// Ascend to parent focus level
    Ascend,
    /// Ascend to root (clear all focus)
    AscendToRoot,
    /// Change view type
    ChangeViewType { view_type: CbuViewType },
    /// Update confidence threshold
    SetConfidenceThreshold { threshold: f32 },
    /// Toggle entity type filter
    ToggleEntityTypeFilter { entity_type: ConcreteEntityType },
    /// Clear all filters
    ClearFilters,
    /// Set search text
    SetSearchText { text: String },
    /// Navigate to breadcrumb index
    NavigateToBreadcrumb { index: usize },
}

// =============================================================================
// COLORS
// =============================================================================

pub mod viewport_colors {
    use egui::Color32;

    /// Background for viewport HUD panels
    pub fn hud_bg() -> Color32 {
        Color32::from_rgba_unmultiplied(20, 20, 25, 230)
    }

    /// Border for HUD panels
    pub fn hud_border() -> Color32 {
        Color32::from_rgba_unmultiplied(60, 60, 70, 200)
    }

    /// Text color for HUD
    pub fn hud_text() -> Color32 {
        Color32::from_rgb(200, 200, 210)
    }

    /// Subdued text
    pub fn hud_text_dim() -> Color32 {
        Color32::from_rgb(120, 120, 135)
    }

    /// Accent color for active elements
    pub fn accent() -> Color32 {
        Color32::from_rgb(96, 165, 250) // blue-400
    }

    /// Focus ring color
    pub fn focus_ring() -> Color32 {
        Color32::from_rgb(250, 204, 21) // yellow-400
    }

    /// Confidence zone colors
    pub fn zone_core() -> Color32 {
        Color32::from_rgb(34, 197, 94) // green-500
    }

    pub fn zone_shell() -> Color32 {
        Color32::from_rgb(59, 130, 246) // blue-500
    }

    pub fn zone_penumbra() -> Color32 {
        Color32::from_rgb(234, 179, 8) // yellow-500
    }

    pub fn zone_speculative() -> Color32 {
        Color32::from_rgb(239, 68, 68) // red-500
    }

    /// Get color for confidence zone
    pub fn zone_color(zone: ob_poc_types::viewport::ConfidenceZone) -> Color32 {
        use ob_poc_types::viewport::ConfidenceZone;
        match zone {
            ConfidenceZone::Core => zone_core(),
            ConfidenceZone::Shell => zone_shell(),
            ConfidenceZone::Penumbra => zone_penumbra(),
            ConfidenceZone::Speculative => zone_speculative(),
        }
    }

    /// Breadcrumb separator
    pub fn breadcrumb_separator() -> Color32 {
        Color32::from_rgb(80, 80, 90)
    }

    /// Enhance level indicator colors
    pub fn enhance_level_active() -> Color32 {
        Color32::from_rgb(147, 197, 253) // blue-300
    }

    pub fn enhance_level_inactive() -> Color32 {
        Color32::from_rgb(55, 65, 81) // gray-700
    }

    /// View type badge colors
    pub fn view_type_structure() -> Color32 {
        Color32::from_rgb(147, 197, 253) // blue-300
    }

    pub fn view_type_ownership() -> Color32 {
        Color32::from_rgb(134, 239, 172) // green-300
    }

    pub fn view_type_accounts() -> Color32 {
        Color32::from_rgb(253, 186, 116) // orange-300
    }

    pub fn view_type_compliance() -> Color32 {
        Color32::from_rgb(252, 211, 77) // amber-300
    }

    pub fn view_type_geographic() -> Color32 {
        Color32::from_rgb(103, 232, 249) // cyan-300
    }

    pub fn view_type_temporal() -> Color32 {
        Color32::from_rgb(196, 181, 253) // violet-300
    }

    pub fn view_type_instruments() -> Color32 {
        Color32::from_rgb(249, 168, 212) // pink-300
    }
}

// =============================================================================
// VIEWPORT STATE FOR RENDERING
// =============================================================================

/// UI-specific viewport rendering state (animations, transitions)
#[derive(Debug, Clone)]
pub struct ViewportRenderState {
    /// Animated focus ring position
    focus_ring_pos: SpringVec2,
    /// Animated focus ring scale
    focus_ring_scale: SpringF32,
    /// Breadcrumb panel height animation
    breadcrumb_height: SpringF32,
    /// HUD opacity animation
    hud_opacity: SpringF32,
    /// Current focus ring target (for tracking changes)
    focus_ring_target_id: Option<String>,
    /// Confidence threshold for entity visibility (from ViewportState)
    confidence_threshold: f32,
}

impl Default for ViewportRenderState {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportRenderState {
    pub fn new() -> Self {
        Self {
            focus_ring_pos: SpringVec2::with_config(0.0, 0.0, SpringConfig::MEDIUM),
            focus_ring_scale: SpringF32::with_config(1.0, SpringConfig::FAST),
            breadcrumb_height: SpringF32::with_config(0.0, SpringConfig::FAST),
            hud_opacity: SpringF32::with_config(1.0, SpringConfig::MEDIUM),
            focus_ring_target_id: None,
            confidence_threshold: 0.0, // Show all by default
        }
    }

    /// Set confidence threshold for entity visibility
    pub fn set_confidence_threshold(&mut self, threshold: f32) {
        self.confidence_threshold = threshold;
    }

    /// Get current confidence threshold
    pub fn confidence_threshold(&self) -> f32 {
        self.confidence_threshold
    }

    /// Update animations each frame
    pub fn tick(&mut self, dt: f32) {
        self.focus_ring_pos.tick(dt);
        self.focus_ring_scale.tick(dt);
        self.breadcrumb_height.tick(dt);
        self.hud_opacity.tick(dt);
    }

    /// Check if any animations are in progress
    pub fn is_animating(&self) -> bool {
        self.focus_ring_pos.is_animating()
            || self.focus_ring_scale.is_animating()
            || self.breadcrumb_height.is_animating()
            || self.hud_opacity.is_animating()
    }

    /// Update focus ring target position
    pub fn set_focus_ring_target(&mut self, pos: Vec2, target_id: Option<String>) {
        // Only animate if target changed
        if self.focus_ring_target_id != target_id {
            self.focus_ring_pos.set_target(pos.x, pos.y);
            self.focus_ring_scale.set_immediate(0.8); // Start small
            self.focus_ring_scale.set_target(1.0); // Animate to full size
            self.focus_ring_target_id = target_id;
        }
    }

    /// Set breadcrumb visibility
    pub fn set_breadcrumb_visible(&mut self, visible: bool) {
        self.breadcrumb_height
            .set_target(if visible { 32.0 } else { 0.0 });
    }
}

// =============================================================================
// ESPER RENDER STATE - Local visual mode toggles (no server round-trip)
// =============================================================================

/// Esper-style visual render modes.
/// These are LOCAL toggles that affect how the graph is rendered.
/// They don't require server round-trips - just flip bools and repaint.
///
/// # Modes
///
/// - **Xray**: Make non-essential elements semi-transparent to see through structure
/// - **Peel**: Hide outer layers to reveal inner structure
/// - **Shadow**: Dim non-relevant entities to focus on target
/// - **RedFlagScan**: Highlight entities with risk indicators or anomalies
/// - **BlackHole**: Highlight entities with missing/incomplete data
/// - **Illuminate**: Glow/emphasize a specific aspect (ownership, control, etc.)
/// - **DepthIndicator**: Show visual depth cues for hierarchical navigation
#[derive(Debug, Clone, Default)]
pub struct EsperRenderState {
    // =========================================================================
    // TRANSPARENCY MODES
    // =========================================================================
    /// X-ray mode: make non-focused elements semi-transparent
    pub xray_enabled: bool,
    /// X-ray alpha for non-focused elements (0.0 = invisible, 1.0 = opaque)
    pub xray_alpha: f32,

    /// Peel mode: hide outer layers to reveal structure
    pub peel_enabled: bool,
    /// Current peel depth (0 = show all, higher = hide more outer layers)
    pub peel_depth: u8,

    // =========================================================================
    // FOCUS MODES
    // =========================================================================
    /// Shadow mode: dim non-relevant entities
    pub shadow_enabled: bool,
    /// Shadow alpha for dimmed entities
    pub shadow_alpha: f32,

    /// Illuminate mode: glow specific aspect
    pub illuminate_enabled: bool,
    /// What aspect to illuminate (ownership, control, risk, etc.)
    pub illuminate_aspect: IlluminateAspect,

    // =========================================================================
    // SCANNING MODES
    // =========================================================================
    /// Red flag scan: highlight entities with anomalies/risk indicators
    pub red_flag_scan_enabled: bool,
    /// Category of red flags to scan for (None = all)
    pub red_flag_category: Option<RedFlagCategory>,

    /// Black hole mode: highlight entities with missing data
    pub black_hole_enabled: bool,
    /// Type of gaps to highlight (None = all)
    pub black_hole_gap_type: Option<GapType>,

    // =========================================================================
    // DEPTH VISUALIZATION
    // =========================================================================
    /// Show depth indicators (for hierarchical navigation)
    pub depth_indicator_enabled: bool,

    /// Cross-section mode: show slice through structure
    pub cross_section_enabled: bool,
}

/// What aspect to illuminate/emphasize in illuminate mode
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IlluminateAspect {
    /// Ownership relationships
    #[default]
    Ownership,
    /// Control relationships
    Control,
    /// Risk indicators
    Risk,
    /// Document status
    Documents,
    /// KYC status
    KycStatus,
    /// Custom aspect by name
    Custom,
}

/// Category of red flags to scan for
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedFlagCategory {
    /// High risk entities
    HighRisk,
    /// Pending KYC
    PendingKyc,
    /// Sanctions matches
    Sanctions,
    /// PEP matches
    Pep,
    /// Adverse media
    AdverseMedia,
    /// Ownership gaps
    OwnershipGaps,
    /// All categories
    All,
}

/// Type of data gaps to highlight in black hole mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapType {
    /// Missing documents
    MissingDocuments,
    /// Incomplete ownership chain
    IncompleteOwnership,
    /// Missing screening
    MissingScreening,
    /// Expired data
    ExpiredData,
    /// All gap types
    All,
}

impl EsperRenderState {
    pub fn new() -> Self {
        Self {
            xray_alpha: 0.3,
            shadow_alpha: 0.2,
            ..Default::default()
        }
    }

    // =========================================================================
    // TOGGLE METHODS - Called from NavigationVerb handlers
    // =========================================================================

    /// Toggle X-ray mode
    pub fn toggle_xray(&mut self) {
        self.xray_enabled = !self.xray_enabled;
    }

    /// Enable X-ray with specific alpha
    pub fn enable_xray(&mut self, alpha: f32) {
        self.xray_enabled = true;
        self.xray_alpha = alpha.clamp(0.0, 1.0);
    }

    /// Disable X-ray mode
    pub fn disable_xray(&mut self) {
        self.xray_enabled = false;
    }

    /// Toggle peel mode, incrementing depth each toggle
    pub fn toggle_peel(&mut self) {
        if self.peel_enabled {
            self.peel_depth = self.peel_depth.saturating_add(1);
            if self.peel_depth > 5 {
                // Max peel depth, disable
                self.peel_enabled = false;
                self.peel_depth = 0;
            }
        } else {
            self.peel_enabled = true;
            self.peel_depth = 1;
        }
    }

    /// Set specific peel depth
    pub fn set_peel_depth(&mut self, depth: u8) {
        self.peel_enabled = depth > 0;
        self.peel_depth = depth;
    }

    /// Toggle shadow mode
    pub fn toggle_shadow(&mut self) {
        self.shadow_enabled = !self.shadow_enabled;
    }

    /// Enable shadow with specific alpha
    pub fn enable_shadow(&mut self, alpha: f32) {
        self.shadow_enabled = true;
        self.shadow_alpha = alpha.clamp(0.0, 1.0);
    }

    /// Disable shadow mode
    pub fn disable_shadow(&mut self) {
        self.shadow_enabled = false;
    }

    /// Toggle illuminate mode
    pub fn toggle_illuminate(&mut self, aspect: IlluminateAspect) {
        if self.illuminate_enabled && self.illuminate_aspect == aspect {
            self.illuminate_enabled = false;
        } else {
            self.illuminate_enabled = true;
            self.illuminate_aspect = aspect;
        }
    }

    /// Set illuminate aspect
    pub fn set_illuminate(&mut self, aspect: IlluminateAspect) {
        self.illuminate_enabled = true;
        self.illuminate_aspect = aspect;
    }

    /// Disable illuminate mode
    pub fn disable_illuminate(&mut self) {
        self.illuminate_enabled = false;
    }

    /// Toggle red flag scan
    pub fn toggle_red_flag_scan(&mut self, category: Option<RedFlagCategory>) {
        if self.red_flag_scan_enabled && self.red_flag_category == category {
            self.red_flag_scan_enabled = false;
        } else {
            self.red_flag_scan_enabled = true;
            self.red_flag_category = category;
        }
    }

    /// Enable red flag scan for specific category
    pub fn enable_red_flag_scan(&mut self, category: Option<RedFlagCategory>) {
        self.red_flag_scan_enabled = true;
        self.red_flag_category = category;
    }

    /// Disable red flag scan
    pub fn disable_red_flag_scan(&mut self) {
        self.red_flag_scan_enabled = false;
    }

    /// Toggle black hole mode
    pub fn toggle_black_hole(&mut self, gap_type: Option<GapType>) {
        if self.black_hole_enabled && self.black_hole_gap_type == gap_type {
            self.black_hole_enabled = false;
        } else {
            self.black_hole_enabled = true;
            self.black_hole_gap_type = gap_type;
        }
    }

    /// Enable black hole mode for specific gap type
    pub fn enable_black_hole(&mut self, gap_type: Option<GapType>) {
        self.black_hole_enabled = true;
        self.black_hole_gap_type = gap_type;
    }

    /// Disable black hole mode
    pub fn disable_black_hole(&mut self) {
        self.black_hole_enabled = false;
    }

    /// Toggle depth indicator
    pub fn toggle_depth_indicator(&mut self) {
        self.depth_indicator_enabled = !self.depth_indicator_enabled;
    }

    /// Toggle cross-section mode
    pub fn toggle_cross_section(&mut self) {
        self.cross_section_enabled = !self.cross_section_enabled;
    }

    /// Reset all modes to default
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    // =========================================================================
    // QUERY METHODS - Used by render pass to determine effects
    // =========================================================================

    /// Check if any visual mode is active
    pub fn any_mode_active(&self) -> bool {
        self.xray_enabled
            || self.peel_enabled
            || self.shadow_enabled
            || self.illuminate_enabled
            || self.red_flag_scan_enabled
            || self.black_hole_enabled
            || self.depth_indicator_enabled
            || self.cross_section_enabled
    }

    /// Get alpha multiplier for a node based on current modes
    /// Returns (base_alpha, should_highlight)
    pub fn get_node_alpha(
        &self,
        is_focused: bool,
        depth: u8,
        has_red_flag: bool,
        has_gap: bool,
    ) -> (f32, bool) {
        let mut alpha = 1.0;
        let mut highlight = false;

        // X-ray: dim non-focused nodes
        if self.xray_enabled && !is_focused {
            alpha *= self.xray_alpha;
        }

        // Peel: hide nodes beyond peel depth
        if self.peel_enabled && depth > self.peel_depth {
            alpha = 0.0; // Hidden
        }

        // Shadow: dim non-focused nodes
        if self.shadow_enabled && !is_focused {
            alpha *= self.shadow_alpha;
        }

        // Red flag scan: highlight flagged nodes
        if self.red_flag_scan_enabled && has_red_flag {
            highlight = true;
            alpha = 1.0; // Ensure visible
        }

        // Black hole: highlight nodes with gaps
        if self.black_hole_enabled && has_gap {
            highlight = true;
            alpha = 1.0; // Ensure visible
        }

        (alpha.clamp(0.0, 1.0), highlight)
    }
}

// =============================================================================
// RENDER FUNCTIONS
// =============================================================================

/// Render viewport HUD overlay on top of the graph
///
/// This renders:
/// - Breadcrumb navigation bar (top)
/// - Enhance level indicator (top-right)
/// - Confidence zone legend (bottom-left)
/// - View type selector (bottom-right)
///
/// Returns an action if the user interacted with the HUD.
pub fn render_viewport_hud(
    ui: &mut Ui,
    viewport: &ViewportState,
    render_state: &mut ViewportRenderState,
    screen_rect: Rect,
) -> ViewportAction {
    let mut action = ViewportAction::None;

    // Update animations
    let dt = ui.input(|i| i.stable_dt);
    render_state.tick(dt);

    // Top bar: Breadcrumbs + Enhance indicator
    let top_bar_rect = Rect::from_min_size(screen_rect.min, Vec2::new(screen_rect.width(), 40.0));

    // Render breadcrumbs
    let breadcrumb_action = render_focus_breadcrumbs(ui, &viewport.focus, top_bar_rect);
    if breadcrumb_action != ViewportAction::None {
        action = breadcrumb_action;
    }

    // Render specialized HUD based on focus state
    match &viewport.focus.state {
        ViewportFocusState::BoardControl {
            anchor_entity_name,
            source_cbu,
            ..
        } => {
            render_board_control_title_hud(ui, anchor_entity_name, Some(source_cbu), screen_rect);
        }
        ViewportFocusState::InstrumentMatrix { .. }
        | ViewportFocusState::InstrumentType { .. }
        | ViewportFocusState::ConfigNode { .. } => {
            render_matrix_title_hud(ui, &viewport.focus.state, screen_rect);
        }
        _ => {}
    }

    // Enhance level indicator (top-right)
    let enhance_rect = Rect::from_min_size(
        Pos2::new(screen_rect.right() - 120.0, screen_rect.min.y + 8.0),
        Vec2::new(110.0, 28.0),
    );
    let enhance_action = render_enhance_level_indicator(ui, &viewport.focus.state, enhance_rect);
    if enhance_action != ViewportAction::None {
        action = enhance_action;
    }

    // Confidence zone legend (bottom-left)
    let legend_rect = Rect::from_min_size(
        Pos2::new(screen_rect.min.x + 10.0, screen_rect.bottom() - 90.0),
        Vec2::new(140.0, 80.0),
    );
    render_confidence_zone_legend(ui, &viewport.filters, legend_rect);

    // View type selector (bottom-right)
    let view_type_rect = Rect::from_min_size(
        Pos2::new(screen_rect.right() - 160.0, screen_rect.bottom() - 40.0),
        Vec2::new(150.0, 32.0),
    );
    let view_action = render_view_type_selector(ui, viewport.view_type, view_type_rect);
    if view_action != ViewportAction::None {
        action = view_action;
    }

    // Request repaint if animating
    if render_state.is_animating() {
        ui.ctx().request_repaint();
    }

    action
}

/// Render Board Control title HUD (centered title bar below breadcrumbs)
fn render_board_control_title_hud(
    ui: &mut Ui,
    anchor_entity_name: &str,
    source_cbu: Option<&ob_poc_types::viewport::CbuRef>,
    screen_rect: Rect,
) {
    let painter = ui.painter();

    // Title bar below breadcrumbs
    let title_rect = Rect::from_min_size(
        Pos2::new(screen_rect.min.x, screen_rect.min.y + 44.0),
        Vec2::new(screen_rect.width(), 36.0),
    );

    // Background
    painter.rect_filled(
        title_rect,
        0.0,
        Color32::from_rgba_unmultiplied(20, 25, 35, 200),
    );
    painter.line_segment(
        [title_rect.left_bottom(), title_rect.right_bottom()],
        Stroke::new(1.0, Color32::from_rgb(70, 80, 100)),
    );

    // Title: "BOARD CONTROL"
    painter.text(
        Pos2::new(title_rect.center().x, title_rect.center().y - 6.0),
        egui::Align2::CENTER_CENTER,
        "BOARD CONTROL",
        egui::FontId::proportional(14.0),
        Color32::from_rgb(147, 197, 253), // Blue accent
    );

    // Anchor entity name below
    painter.text(
        Pos2::new(title_rect.center().x, title_rect.center().y + 8.0),
        egui::Align2::CENTER_CENTER,
        anchor_entity_name,
        egui::FontId::proportional(11.0),
        Color32::from_rgb(180, 190, 210),
    );

    // Source CBU indicator (left side)
    if let Some(cbu_ref) = source_cbu {
        let back_text = format!("← CBU: {:8}", cbu_ref.0.to_string().get(..8).unwrap_or(""));
        painter.text(
            Pos2::new(title_rect.left() + 12.0, title_rect.center().y),
            egui::Align2::LEFT_CENTER,
            &back_text,
            egui::FontId::proportional(10.0),
            Color32::from_rgb(100, 116, 139), // Muted
        );
    }
}

/// Render Trading Matrix title HUD (centered title bar below breadcrumbs)
fn render_matrix_title_hud(ui: &mut Ui, focus_state: &ViewportFocusState, screen_rect: Rect) {
    let painter = ui.painter();

    // Title bar below breadcrumbs
    let title_rect = Rect::from_min_size(
        Pos2::new(screen_rect.min.x, screen_rect.min.y + 44.0),
        Vec2::new(screen_rect.width(), 36.0),
    );

    // Background
    painter.rect_filled(
        title_rect,
        0.0,
        Color32::from_rgba_unmultiplied(20, 25, 35, 200),
    );
    painter.line_segment(
        [title_rect.left_bottom(), title_rect.right_bottom()],
        Stroke::new(1.0, Color32::from_rgb(70, 80, 100)),
    );

    // Title and subtitle based on focus state
    let (title, subtitle): (&str, String) = match focus_state {
        ViewportFocusState::InstrumentMatrix { .. } => {
            ("TRADING MATRIX", "Instrument Configuration".to_string())
        }
        ViewportFocusState::InstrumentType {
            instrument_type, ..
        } => ("INSTRUMENT TYPE", format!("{:?}", instrument_type)),
        ViewportFocusState::ConfigNode { config_node, .. } => {
            let node_name = match config_node {
                ob_poc_types::viewport::ConfigNodeRef::Mic { code } => format!("MIC: {}", code),
                ob_poc_types::viewport::ConfigNodeRef::Bic { code } => format!("BIC: {}", code),
                ob_poc_types::viewport::ConfigNodeRef::Pricing { .. } => {
                    "Pricing Config".to_string()
                }
                ob_poc_types::viewport::ConfigNodeRef::Restrictions { .. } => {
                    "Restrictions".to_string()
                }
            };
            ("CONFIG NODE", node_name)
        }
        _ => ("MATRIX", String::new()),
    };

    // Title
    painter.text(
        Pos2::new(title_rect.center().x, title_rect.center().y - 6.0),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(14.0),
        Color32::from_rgb(253, 224, 71), // Yellow accent for trading
    );

    // Subtitle
    if !subtitle.is_empty() {
        painter.text(
            Pos2::new(title_rect.center().x, title_rect.center().y + 8.0),
            egui::Align2::CENTER_CENTER,
            subtitle,
            egui::FontId::proportional(11.0),
            Color32::from_rgb(180, 190, 210),
        );
    }
}

/// Render focus breadcrumb navigation bar
fn render_focus_breadcrumbs(ui: &mut Ui, focus: &FocusManager, rect: Rect) -> ViewportAction {
    let mut action = ViewportAction::None;

    // Background panel
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, viewport_colors::hud_bg());
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, viewport_colors::hud_border()));

    // Build breadcrumb items
    let mut crumbs: Vec<BreadcrumbItem> = Vec::new();

    // Always show root
    crumbs.push(BreadcrumbItem {
        label: "Universe".to_string(),
        is_current: focus.state == ViewportFocusState::None,
        icon: "🌌",
    });

    // Add focus stack items
    for (i, state) in focus.focus_stack.iter().enumerate() {
        crumbs.push(breadcrumb_item_for_state(state, i));
    }

    // Add current state if not None
    if focus.state != ViewportFocusState::None {
        crumbs.push(breadcrumb_item_for_state(&focus.state, crumbs.len()));
        // Mark last as current
        if let Some(last) = crumbs.last_mut() {
            last.is_current = true;
        }
    }

    // Render breadcrumbs
    let mut x = rect.min.x + 12.0;
    let y = rect.center().y;

    for (i, crumb) in crumbs.iter().enumerate() {
        // Separator
        if i > 0 {
            painter.text(
                Pos2::new(x, y),
                egui::Align2::LEFT_CENTER,
                "›",
                egui::FontId::proportional(14.0),
                viewport_colors::breadcrumb_separator(),
            );
            x += 16.0;
        }

        // Icon
        painter.text(
            Pos2::new(x, y),
            egui::Align2::LEFT_CENTER,
            crumb.icon,
            egui::FontId::proportional(12.0),
            viewport_colors::hud_text(),
        );
        x += 18.0;

        // Label (clickable if not current)
        let text_color = if crumb.is_current {
            viewport_colors::accent()
        } else {
            viewport_colors::hud_text()
        };

        let label_galley = painter.layout_no_wrap(
            crumb.label.clone(),
            egui::FontId::proportional(12.0),
            text_color,
        );
        let label_rect = Rect::from_min_size(
            Pos2::new(x, y - label_galley.size().y / 2.0),
            label_galley.size(),
        );

        painter.galley(label_rect.min, label_galley, text_color);

        // Click detection
        if !crumb.is_current {
            let response = ui.interact(
                label_rect.expand(4.0),
                ui.id().with(("breadcrumb", i)),
                egui::Sense::click(),
            );
            if response.clicked() {
                action = ViewportAction::NavigateToBreadcrumb { index: i };
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }

        x += label_rect.width() + 8.0;
    }

    action
}

/// Render enhance level indicator (L0-L4 dots)
fn render_enhance_level_indicator(
    ui: &mut Ui,
    state: &ViewportFocusState,
    rect: Rect,
) -> ViewportAction {
    let mut action = ViewportAction::None;

    if *state == ViewportFocusState::None {
        return action;
    }

    let painter = ui.painter();

    // Background
    painter.rect_filled(rect, 6.0, viewport_colors::hud_bg());
    painter.rect_stroke(rect, 6.0, Stroke::new(1.0, viewport_colors::hud_border()));

    let current_level = state.primary_enhance_level();
    let max_level = state.max_enhance_level();

    // Label
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Level",
        egui::FontId::proportional(10.0),
        viewport_colors::hud_text_dim(),
    );

    // Level dots
    let dot_start_x = rect.min.x + 42.0;
    let dot_y = rect.center().y;
    let dot_spacing = 14.0;
    let dot_radius = 4.0;

    for level in 0..=max_level {
        let x = dot_start_x + (level as f32) * dot_spacing;
        let is_active = level <= current_level;

        let color = if is_active {
            viewport_colors::enhance_level_active()
        } else {
            viewport_colors::enhance_level_inactive()
        };

        painter.circle_filled(Pos2::new(x, dot_y), dot_radius, color);

        // Click to set level
        let dot_rect = Rect::from_center_size(Pos2::new(x, dot_y), Vec2::splat(dot_radius * 3.0));
        let response = ui.interact(
            dot_rect,
            ui.id().with(("enhance_dot", level)),
            egui::Sense::click(),
        );
        if response.clicked() {
            action = ViewportAction::Enhance {
                arg: EnhanceArg::Level(level),
            };
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    // +/- buttons
    let btn_y = rect.center().y;

    // Minus button
    let minus_rect =
        Rect::from_center_size(Pos2::new(rect.right() - 28.0, btn_y), Vec2::splat(16.0));
    let minus_response = ui.interact(
        minus_rect,
        ui.id().with("enhance_minus"),
        egui::Sense::click(),
    );
    let minus_color = if state.can_reduce() {
        viewport_colors::hud_text()
    } else {
        viewport_colors::hud_text_dim()
    };
    painter.text(
        minus_rect.center(),
        egui::Align2::CENTER_CENTER,
        "−",
        egui::FontId::proportional(14.0),
        minus_color,
    );
    if minus_response.clicked() && state.can_reduce() {
        action = ViewportAction::Enhance {
            arg: EnhanceArg::Decrement,
        };
    }

    // Plus button
    let plus_rect =
        Rect::from_center_size(Pos2::new(rect.right() - 10.0, btn_y), Vec2::splat(16.0));
    let plus_response = ui.interact(
        plus_rect,
        ui.id().with("enhance_plus"),
        egui::Sense::click(),
    );
    let plus_color = if state.can_enhance() {
        viewport_colors::hud_text()
    } else {
        viewport_colors::hud_text_dim()
    };
    painter.text(
        plus_rect.center(),
        egui::Align2::CENTER_CENTER,
        "+",
        egui::FontId::proportional(14.0),
        plus_color,
    );
    if plus_response.clicked() && state.can_enhance() {
        action = ViewportAction::Enhance {
            arg: EnhanceArg::Increment,
        };
    }

    action
}

/// Render confidence zone legend
fn render_confidence_zone_legend(ui: &mut Ui, filters: &ViewportFilters, rect: Rect) {
    let painter = ui.painter();

    // Background
    painter.rect_filled(rect, 6.0, viewport_colors::hud_bg());
    painter.rect_stroke(rect, 6.0, Stroke::new(1.0, viewport_colors::hud_border()));

    // Title
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.min.y + 12.0),
        egui::Align2::LEFT_CENTER,
        "Confidence",
        egui::FontId::proportional(10.0),
        viewport_colors::hud_text_dim(),
    );

    // Zone items
    let zones = [
        (ConfidenceZone::Core, "Core", "≥95%"),
        (ConfidenceZone::Shell, "Shell", "≥70%"),
        (ConfidenceZone::Penumbra, "Penumbra", "≥40%"),
        (ConfidenceZone::Speculative, "Speculative", "<40%"),
    ];

    let mut y = rect.min.y + 28.0;
    let row_height = 14.0;

    for (zone, label, range) in zones {
        let is_filtered = filters.confidence_zone == Some(zone);

        // Color dot
        let color = viewport_colors::zone_color(zone);
        let alpha = if is_filtered { 255 } else { 180 };
        let dot_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);
        painter.circle_filled(Pos2::new(rect.min.x + 14.0, y), 4.0, dot_color);

        // Label
        painter.text(
            Pos2::new(rect.min.x + 24.0, y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(10.0),
            viewport_colors::hud_text(),
        );

        // Range
        painter.text(
            Pos2::new(rect.right() - 8.0, y),
            egui::Align2::RIGHT_CENTER,
            range,
            egui::FontId::proportional(9.0),
            viewport_colors::hud_text_dim(),
        );

        y += row_height;
    }
}

/// Render view type selector tabs
fn render_view_type_selector(ui: &mut Ui, current: CbuViewType, rect: Rect) -> ViewportAction {
    let mut action = ViewportAction::None;
    let painter = ui.painter();

    // Background
    painter.rect_filled(rect, 6.0, viewport_colors::hud_bg());
    painter.rect_stroke(rect, 6.0, Stroke::new(1.0, viewport_colors::hud_border()));

    // View types with icons
    let view_types = [
        (CbuViewType::Structure, "🏛", "Structure"),
        (CbuViewType::Ownership, "👥", "Ownership"),
        (CbuViewType::Instruments, "📊", "Trading"),
    ];

    let btn_width = (rect.width() - 16.0) / view_types.len() as f32;
    let mut x = rect.min.x + 8.0;

    for (view_type, icon, _label) in view_types {
        let btn_rect = Rect::from_min_size(
            Pos2::new(x, rect.min.y + 4.0),
            Vec2::new(btn_width - 4.0, rect.height() - 8.0),
        );

        let is_selected = current == view_type;
        let bg_color = if is_selected {
            Color32::from_rgba_unmultiplied(96, 165, 250, 60)
        } else {
            Color32::TRANSPARENT
        };

        painter.rect_filled(btn_rect, 4.0, bg_color);

        // Icon
        let color = if is_selected {
            get_view_type_color(view_type)
        } else {
            viewport_colors::hud_text_dim()
        };

        painter.text(
            btn_rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(14.0),
            color,
        );

        // Click handler
        let response = ui.interact(
            btn_rect,
            ui.id().with(("view_type", view_type as u8)),
            egui::Sense::click(),
        );
        if response.clicked() && !is_selected {
            action = ViewportAction::ChangeViewType { view_type };
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        x += btn_width;
    }

    action
}

/// Get color for a view type
fn get_view_type_color(view_type: CbuViewType) -> Color32 {
    match view_type {
        CbuViewType::Structure => viewport_colors::view_type_structure(),
        CbuViewType::Ownership => viewport_colors::view_type_ownership(),
        CbuViewType::Accounts => viewport_colors::view_type_accounts(),
        CbuViewType::Compliance => viewport_colors::view_type_compliance(),
        CbuViewType::Geographic => viewport_colors::view_type_geographic(),
        CbuViewType::Temporal => viewport_colors::view_type_temporal(),
        CbuViewType::Instruments => viewport_colors::view_type_instruments(),
    }
}

// =============================================================================
// FOCUS RING RENDERING
// =============================================================================

/// Render an animated focus ring around a node
pub fn render_focus_ring(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    render_state: &ViewportRenderState,
    time: f32,
) {
    let scale = render_state.focus_ring_scale.get();
    let animated_radius = radius * scale;

    // Outer glow
    let glow_color = Color32::from_rgba_unmultiplied(250, 204, 21, 40);
    painter.circle_filled(center, animated_radius + 8.0, glow_color);

    // Main ring (animated dash pattern)
    let ring_color = viewport_colors::focus_ring();
    let dash_offset = (time * 2.0) % 1.0;

    // Draw dashed circle
    draw_dashed_circle(
        painter,
        center,
        animated_radius + 4.0,
        ring_color,
        2.0,
        8.0,
        dash_offset,
    );

    // Inner ring
    painter.circle_stroke(
        center,
        animated_radius,
        Stroke::new(1.5, Color32::from_rgba_unmultiplied(250, 204, 21, 120)),
    );
}

/// Draw a dashed circle
fn draw_dashed_circle(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    color: Color32,
    stroke_width: f32,
    dash_length: f32,
    offset: f32,
) {
    let circumference = 2.0 * std::f32::consts::PI * radius;
    let num_dashes = (circumference / (dash_length * 2.0)).ceil() as usize;

    for i in 0..num_dashes {
        let start_angle = (i as f32 / num_dashes as f32 + offset) * 2.0 * std::f32::consts::PI;
        let end_angle = start_angle + (dash_length / radius);

        let start = Pos2::new(
            center.x + radius * start_angle.cos(),
            center.y + radius * start_angle.sin(),
        );
        let end = Pos2::new(
            center.x + radius * end_angle.cos(),
            center.y + radius * end_angle.sin(),
        );

        painter.line_segment([start, end], Stroke::new(stroke_width, color));
    }
}

// =============================================================================
// CONFIDENCE ZONE RENDERING
// =============================================================================

/// Stroke style for confidence zone
pub fn zone_stroke(zone: ConfidenceZone, base_width: f32) -> Stroke {
    let color = viewport_colors::zone_color(zone);
    let alpha = (zone.opacity() * 255.0) as u8;
    let stroke_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);

    Stroke::new(base_width, stroke_color)
}

/// Get opacity multiplier for confidence zone rendering
pub fn zone_opacity(zone: ConfidenceZone) -> f32 {
    zone.opacity()
}

/// Whether to use dashed stroke for this zone
pub fn zone_is_dashed(zone: ConfidenceZone) -> bool {
    zone.is_dashed()
}

/// Render a node with confidence zone styling
pub fn render_node_with_confidence(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    zone: ConfidenceZone,
    fill_color: Color32,
    time: f32,
) {
    let opacity = zone.opacity();
    let fill_alpha = (opacity * fill_color.a() as f32) as u8;
    let fill =
        Color32::from_rgba_unmultiplied(fill_color.r(), fill_color.g(), fill_color.b(), fill_alpha);

    // Fill
    painter.circle_filled(center, radius, fill);

    // Stroke based on zone
    let stroke_color = viewport_colors::zone_color(zone);
    let stroke_alpha = (opacity * 200.0) as u8;
    let stroke = Color32::from_rgba_unmultiplied(
        stroke_color.r(),
        stroke_color.g(),
        stroke_color.b(),
        stroke_alpha,
    );

    if zone.is_dashed() {
        // Animated dashed stroke for penumbra/speculative
        let dash_offset = (time * 0.5) % 1.0;
        draw_dashed_circle(painter, center, radius, stroke, 2.0, 6.0, dash_offset);
    } else {
        // Solid stroke for core/shell
        painter.circle_stroke(center, radius, Stroke::new(2.0, stroke));
    }
}

// =============================================================================
// HELPER TYPES
// =============================================================================

/// Breadcrumb item for rendering
struct BreadcrumbItem {
    label: String,
    is_current: bool,
    icon: &'static str,
}

/// Create breadcrumb item from focus state
fn breadcrumb_item_for_state(state: &ViewportFocusState, _index: usize) -> BreadcrumbItem {
    match state {
        ViewportFocusState::None => BreadcrumbItem {
            label: "Universe".to_string(),
            is_current: false,
            icon: "🌌",
        },
        ViewportFocusState::CbuContainer { .. } => BreadcrumbItem {
            label: "CBU".to_string(),
            is_current: false,
            icon: "📦",
        },
        ViewportFocusState::CbuEntity { entity, .. } => BreadcrumbItem {
            label: format!("{:?}", entity.entity_type),
            is_current: false,
            icon: match entity.entity_type {
                ConcreteEntityType::Company => "🏢",
                ConcreteEntityType::Partnership => "🤝",
                ConcreteEntityType::Trust => "📜",
                ConcreteEntityType::Person => "👤",
            },
        },
        ViewportFocusState::CbuProductService { target, .. } => BreadcrumbItem {
            label: match target {
                ob_poc_types::viewport::ProductServiceRef::Product { .. } => "Product".to_string(),
                ob_poc_types::viewport::ProductServiceRef::Service { .. } => "Service".to_string(),
                ob_poc_types::viewport::ProductServiceRef::ServiceResource { .. } => {
                    "Resource".to_string()
                }
            },
            is_current: false,
            icon: "📋",
        },
        ViewportFocusState::InstrumentMatrix { .. } => BreadcrumbItem {
            label: "Trading Matrix".to_string(),
            is_current: false,
            icon: "📊",
        },
        ViewportFocusState::InstrumentType {
            instrument_type, ..
        } => BreadcrumbItem {
            label: format!("{:?}", instrument_type),
            is_current: false,
            icon: "📈",
        },
        ViewportFocusState::ConfigNode { config_node, .. } => BreadcrumbItem {
            label: match config_node {
                ob_poc_types::viewport::ConfigNodeRef::Mic { code } => format!("MIC: {}", code),
                ob_poc_types::viewport::ConfigNodeRef::Bic { code } => format!("BIC: {}", code),
                ob_poc_types::viewport::ConfigNodeRef::Pricing { .. } => "Pricing".to_string(),
                ob_poc_types::viewport::ConfigNodeRef::Restrictions { .. } => {
                    "Restrictions".to_string()
                }
            },
            is_current: false,
            icon: "⚙",
        },
        ViewportFocusState::BoardControl {
            anchor_entity_name, ..
        } => BreadcrumbItem {
            label: format!("Board Control: {}", anchor_entity_name),
            is_current: false,
            icon: "🎯",
        },
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_render_state_animation() {
        let mut state = ViewportRenderState::new();

        // Initially not animating
        assert!(!state.is_animating());

        // Set a target
        state.set_focus_ring_target(Vec2::new(100.0, 100.0), Some("test".to_string()));

        // Now should be animating
        assert!(state.is_animating());

        // Tick until settled
        for _ in 0..100 {
            state.tick(0.016);
        }

        // Should eventually settle
        // (might still be animating slightly due to spring dynamics)
    }

    #[test]
    fn test_confidence_zone_colors() {
        let core_color = viewport_colors::zone_color(ConfidenceZone::Core);
        assert_eq!(core_color, Color32::from_rgb(34, 197, 94));

        let spec_color = viewport_colors::zone_color(ConfidenceZone::Speculative);
        assert_eq!(spec_color, Color32::from_rgb(239, 68, 68));
    }

    #[test]
    fn test_zone_opacity() {
        assert_eq!(zone_opacity(ConfidenceZone::Core), 1.0);
        assert_eq!(zone_opacity(ConfidenceZone::Shell), 0.85);
        assert_eq!(zone_opacity(ConfidenceZone::Penumbra), 0.6);
        assert_eq!(zone_opacity(ConfidenceZone::Speculative), 0.35);
    }

    #[test]
    fn test_zone_is_dashed() {
        assert!(!zone_is_dashed(ConfidenceZone::Core));
        assert!(!zone_is_dashed(ConfidenceZone::Shell));
        assert!(zone_is_dashed(ConfidenceZone::Penumbra));
        assert!(zone_is_dashed(ConfidenceZone::Speculative));
    }
}
