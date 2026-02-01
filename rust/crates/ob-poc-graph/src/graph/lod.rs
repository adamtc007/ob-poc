//! Level of Detail (LOD) system for graph rendering
//!
//! Adjusts rendering detail based on screen-space size of nodes.
//! Thresholds are loaded from `config/graph_settings.yaml` via `global_config()`.

use egui::{Color32, FontId, Pos2, Stroke, Vec2};

use super::colors::{control_portal_border, control_portal_fill, ControlConfidence};
use super::types::{EntityType, LayoutNode, PrimaryRole};
use crate::config::global_config;

// =============================================================================
// RENDER CONTEXT
// =============================================================================

/// Bundled rendering parameters to reduce function arguments
struct RenderContext {
    pos: Pos2,
    size: Vec2,
    fill: Color32,
    border: Color32,
    text_color: Color32,
    opacity: f32,
    /// Whether this is a ghost entity (minimal info, needs identification)
    is_ghost: bool,
}

impl RenderContext {
    fn new(node: &LayoutNode, pos: Pos2, size: Vec2, opacity: f32) -> Self {
        // Ghost entities get reduced opacity for faded appearance
        let is_ghost = node.person_state.as_deref() == Some("GHOST");
        let ghost_opacity_multiplier = if is_ghost { 0.55 } else { 1.0 };
        let effective_opacity = opacity * ghost_opacity_multiplier;

        Self {
            pos,
            size,
            fill: apply_opacity(node.style.fill_color, effective_opacity),
            border: apply_opacity(node.style.border_color, opacity), // Keep border visible
            text_color: apply_opacity(node.style.text_color, effective_opacity),
            opacity,
            is_ghost,
        }
    }

    fn rect(&self) -> egui::Rect {
        egui::Rect::from_center_size(self.pos, self.size)
    }

    fn radius(&self) -> f32 {
        self.size.x.min(self.size.y) / 2.0
    }
}

// =============================================================================
// DETAIL LEVEL
// =============================================================================

/// Level of detail for node rendering
/// Thresholds: Micro < 8px < Icon < 30px < Compact < 80px < Standard < 120px < Expanded
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    /// < 8px: colored dot only
    Micro,
    /// 8-30px: shape + status color (no text)
    Icon,
    /// 30-80px: shape + truncated name (label)
    Compact,
    /// 80-120px: shape + full name + badge (full text)
    Standard,
    /// 120px+: all details inline
    Expanded,
    /// clicked node: full card (rendered separately)
    Focused,
}

/// Configuration for viewport-aware LOD thresholds
#[derive(Debug, Clone, Copy)]
pub struct LodConfig {
    /// Base node count threshold for density calculations
    /// Scales with viewport area
    pub density_base: f32,
    /// Multiplier applied to density factor (0.0-1.0)
    /// Higher = more aggressive aggregation at same node count
    pub density_weight: f32,
    /// Current viewport area in pixels
    pub viewport_area: f32,
}

impl Default for LodConfig {
    fn default() -> Self {
        let cfg = global_config();
        Self {
            density_base: cfg.lod.density.base,
            density_weight: cfg.lod.density.weight,
            viewport_area: cfg.lod.density.reference_viewport_area,
        }
    }
}

impl LodConfig {
    /// Create LOD config for a specific viewport size
    pub fn for_viewport(width: f32, height: f32) -> Self {
        let cfg = global_config();
        let area = width * height;
        let area_ratio = area / cfg.lod.density.reference_viewport_area;

        // Larger viewport = higher density threshold (can show more nodes before aggregating)
        // Smaller viewport = lower threshold (aggregate sooner)
        let scaled_base = cfg.lod.density.base * area_ratio.sqrt();

        Self {
            density_base: scaled_base.clamp(10.0, 60.0),
            density_weight: cfg.lod.density.weight,
            viewport_area: area,
        }
    }

    /// Update config for new viewport dimensions
    pub fn update_viewport(&mut self, width: f32, height: f32) {
        let new = Self::for_viewport(width, height);
        self.density_base = new.density_base;
        self.viewport_area = new.viewport_area;
    }

    /// Calculate density factor for current node count
    pub fn density_factor(&self, node_count: usize) -> f32 {
        (node_count as f32 / self.density_base).min(1.0)
    }
}

impl DetailLevel {
    /// Determine LOD from screen-space radius
    /// Thresholds loaded from config/graph_settings.yaml
    pub fn from_screen_size(screen_width: f32, is_focused: bool) -> Self {
        if is_focused {
            return DetailLevel::Focused;
        }

        // screen_width = node.size.x * camera.zoom()
        // Typical node is ~140px wide, so at zoom 1.0 screen_width=140
        // At zoom 0.5, screen_width=70. At zoom 2.0, screen_width=280
        let thresholds = &global_config().lod.thresholds;
        match screen_width {
            w if w < thresholds.micro => DetailLevel::Micro,
            w if w < thresholds.icon => DetailLevel::Icon,
            w if w < thresholds.compact => DetailLevel::Compact,
            w if w < thresholds.standard => DetailLevel::Standard,
            _ => DetailLevel::Expanded,
        }
    }

    /// Determine LOD from compression factor and node density
    ///
    /// Compression is inverse of zoom (0.0 = zoomed in, 1.0 = zoomed out).
    /// Density factor adjusts for crowded views.
    /// Thresholds loaded from config/graph_settings.yaml
    pub fn for_compression(compression: f32, node_count: usize) -> Self {
        let cfg = global_config();
        let density_factor = (node_count as f32 / cfg.lod.density.base).min(1.0);
        let effective = compression + (density_factor * cfg.lod.density.weight);

        let comp = &cfg.lod.compression;
        match effective {
            c if c > comp.icon => DetailLevel::Icon,
            c if c > comp.compact => DetailLevel::Compact,
            c if c > comp.standard => DetailLevel::Standard,
            _ => DetailLevel::Expanded,
        }
    }

    /// Determine LOD with viewport-aware density scaling
    ///
    /// Uses LodConfig to adjust thresholds based on viewport size.
    /// Larger viewport = can show more detail at same node count.
    pub fn for_compression_with_config(
        compression: f32,
        node_count: usize,
        config: &LodConfig,
    ) -> Self {
        let cfg = global_config();
        let density_factor = config.density_factor(node_count);
        let effective = compression + (density_factor * config.density_weight);

        let comp = &cfg.lod.compression;
        match effective {
            c if c > comp.icon => DetailLevel::Icon,
            c if c > comp.compact => DetailLevel::Compact,
            c if c > comp.standard => DetailLevel::Standard,
            _ => DetailLevel::Expanded,
        }
    }
}

// =============================================================================
// LOD RENDERING
// =============================================================================

/// Render a node at the appropriate level of detail
pub fn render_node_at_lod(
    painter: &egui::Painter,
    node: &LayoutNode,
    screen_pos: Pos2,
    screen_size: Vec2,
    lod: DetailLevel,
    opacity: f32,
) {
    let ctx = RenderContext::new(node, screen_pos, screen_size, opacity);

    match lod {
        DetailLevel::Micro => render_micro(painter, &ctx),
        DetailLevel::Icon => render_icon(painter, node, &ctx),
        DetailLevel::Compact => render_compact(painter, node, &ctx),
        DetailLevel::Standard => render_standard(painter, node, &ctx),
        DetailLevel::Expanded => render_expanded(painter, node, &ctx),
        DetailLevel::Focused => {
            render_standard(painter, node, &ctx);
            render_focus_ring(painter, &ctx);
        }
    }
}

// =============================================================================
// LOD RENDERERS
// =============================================================================

/// Micro: Just a colored dot
fn render_micro(painter: &egui::Painter, ctx: &RenderContext) {
    painter.circle_filled(ctx.pos, 6.0, ctx.fill);
    painter.circle_stroke(ctx.pos, 6.0, Stroke::new(1.0, Color32::WHITE));
}

/// Icon: Shape with entity type color - NO TEXT
fn render_icon(painter: &egui::Painter, node: &LayoutNode, ctx: &RenderContext) {
    let radius = ctx.radius();
    let stroke = Stroke::new(2.0, ctx.border);

    match node.entity_type {
        EntityType::ProperPerson => {
            painter.circle_filled(ctx.pos, radius, ctx.fill);
            if ctx.is_ghost {
                draw_dashed_circle(painter, ctx.pos, radius, stroke);
            } else {
                painter.circle_stroke(ctx.pos, radius, stroke);
            }
        }
        EntityType::Trust => {
            render_triangle(painter, ctx.pos, radius, ctx.fill, ctx.border, ctx.is_ghost);
        }
        EntityType::ControlPortal => {
            // Hexagon shape with confidence-based coloring
            let confidence = node
                .control_confidence
                .as_deref()
                .and_then(|s| s.parse::<ControlConfidence>().ok())
                .unwrap_or(ControlConfidence::Medium);
            let fill = apply_opacity(control_portal_fill(confidence), ctx.opacity);
            let border = apply_opacity(control_portal_border(confidence), ctx.opacity);
            render_hexagon(painter, ctx.pos, radius, fill, border);
        }
        _ => {
            let rect = egui::Rect::from_center_size(ctx.pos, Vec2::splat(radius * 1.8));
            painter.rect_filled(rect, 4.0, ctx.fill);
            if ctx.is_ghost {
                draw_dashed_rect(painter, rect, 4.0, stroke);
            } else {
                painter.rect_stroke(rect, 4.0, stroke);
            }
        }
    }

    // UBO indicator
    if node.primary_role == PrimaryRole::UltimateBeneficialOwner {
        let badge_pos = ctx.pos + Vec2::new(radius * 0.7, -radius * 0.7);
        let badge_color = apply_opacity(Color32::from_rgb(76, 175, 80), ctx.opacity);
        painter.circle_filled(badge_pos, 5.0, badge_color);
    }

    // Attention indicator (small dot for icon size)
    if node.needs_attention {
        let attention_pos = ctx.pos + Vec2::new(radius * 0.7, radius * 0.7);
        let attention_color = apply_opacity(Color32::from_rgb(239, 68, 68), ctx.opacity);
        painter.circle_filled(attention_pos, 4.0, attention_color);
    }
}

/// Compact: Shape + truncated name
fn render_compact(painter: &egui::Painter, node: &LayoutNode, ctx: &RenderContext) {
    let rect = ctx.rect();
    let stroke = Stroke::new(1.5, ctx.border);

    painter.rect_filled(rect, 6.0, ctx.fill);
    if ctx.is_ghost {
        draw_dashed_rect(painter, rect, 6.0, stroke);
    } else {
        painter.rect_stroke(rect, 6.0, stroke);
    }

    // Truncated name
    let name = truncate_name(&node.label, 12);
    let font_size = (ctx.size.y * 0.25).clamp(8.0, 11.0);
    painter.text(
        ctx.pos,
        egui::Align2::CENTER_CENTER,
        name,
        FontId::proportional(font_size),
        ctx.text_color,
    );

    if !node.is_cbu_root {
        render_role_badge(painter, node, rect, ctx.opacity);
    }

    // Attention indicator
    if node.needs_attention {
        render_attention_indicator(painter, rect, ctx.opacity);
    }
}

/// Standard: Name at top (wrapped), role labels at bottom-right
/// Clean layout - no jurisdiction or entity type clutter
fn render_standard(painter: &egui::Painter, node: &LayoutNode, ctx: &RenderContext) {
    // Special handling for ControlPortal nodes - use hexagon
    if node.entity_type == EntityType::ControlPortal {
        render_hexagon_with_label(painter, node, ctx.pos, ctx.size, ctx.opacity);
        return;
    }

    let rect = ctx.rect();
    let border_width = if node.is_cbu_root { 3.0 } else { 2.0 };
    let stroke = Stroke::new(border_width, ctx.border);

    painter.rect_filled(rect, 8.0, ctx.fill);
    if ctx.is_ghost {
        draw_dashed_rect(painter, rect, 8.0, stroke);
    } else {
        painter.rect_stroke(rect, 8.0, stroke);
    }

    // Name at top - wrapped to fit
    let font_size = (ctx.size.y * 0.18).clamp(10.0, 14.0);
    let max_width = ctx.size.x - 16.0; // padding on each side

    let font = FontId::proportional(font_size);
    let layout_job =
        egui::text::LayoutJob::simple(node.label.clone(), font, ctx.text_color, max_width);
    let galley = painter.layout_job(layout_job);

    // Position name near top, centered horizontally
    let text_pos = Pos2::new(ctx.pos.x - galley.size().x / 2.0, rect.top() + 8.0);
    painter.galley(text_pos, galley, Color32::WHITE);

    // Role labels at bottom-right
    if !node.is_cbu_root {
        render_role_labels_bottom(painter, node, rect, ctx.opacity);
    }

    // KYC completion bar at bottom (above role labels)
    if let Some(completion) = node.kyc_completion {
        render_kyc_completion_bar(painter, rect, completion, ctx.opacity);
    }

    // Attention indicator
    if node.needs_attention {
        render_attention_indicator(painter, rect, ctx.opacity);
    }
}

/// Expanded: All details inline including verification summary
fn render_expanded(painter: &egui::Painter, node: &LayoutNode, ctx: &RenderContext) {
    render_standard(painter, node, ctx);

    let rect = ctx.rect();

    // Verification summary (top-right area, below role badge)
    if let Some(ref summary) = node.verification_summary {
        if summary.total_edges > 0 {
            let summary_text = format!("{}/{} proven", summary.proven_edges, summary.total_edges);
            let summary_color = if summary.disputed_edges > 0 {
                apply_opacity(Color32::from_rgb(239, 68, 68), ctx.opacity) // Red
            } else if summary.proven_edges == summary.total_edges {
                apply_opacity(Color32::from_rgb(34, 197, 94), ctx.opacity) // Green
            } else {
                apply_opacity(Color32::from_rgb(251, 191, 36), ctx.opacity) // Amber
            };
            painter.text(
                rect.right_top() + Vec2::new(-5.0, 20.0),
                egui::Align2::RIGHT_TOP,
                summary_text,
                FontId::proportional(8.0),
                summary_color,
            );
        }
    }

    // Additional roles at bottom (shift up if KYC bar present)
    if node.all_roles.len() > 1 {
        let roles_text = node.all_roles.join(", ");
        let roles_color = apply_opacity(Color32::from_rgb(150, 150, 150), ctx.opacity);
        let bottom_offset = if node.kyc_completion.is_some() {
            14.0
        } else {
            8.0
        };
        painter.text(
            rect.center_bottom() - Vec2::new(0.0, bottom_offset),
            egui::Align2::CENTER_BOTTOM,
            truncate_name(&roles_text, 30),
            FontId::proportional(8.0),
            roles_color,
        );
    }
}

// =============================================================================
// HELPER RENDERERS
// =============================================================================

/// Render role labels at bottom-right of node, supporting multiple roles
fn render_role_labels_bottom(
    painter: &egui::Painter,
    node: &LayoutNode,
    rect: egui::Rect,
    opacity: f32,
) {
    // Collect roles to display - prefer all_roles, fall back to primary_role
    let roles: Vec<String> = if !node.all_roles.is_empty() {
        node.all_roles.iter().map(|r| abbreviate_role(r)).collect()
    } else {
        let primary_text = role_badge_text(&node.primary_role);
        if !primary_text.is_empty() {
            vec![primary_text.to_string()]
        } else {
            return; // No roles to display
        }
    };

    let font = FontId::proportional(9.0);
    let padding = Vec2::new(3.0, 1.0);
    let spacing = 3.0; // Space between role pills
    let bottom_margin = 4.0;
    let right_margin = 4.0;

    // Calculate pill sizes
    let mut pill_infos: Vec<(String, Vec2)> = Vec::new();
    for role in &roles {
        let galley = painter.layout_no_wrap(role.clone(), font.clone(), Color32::WHITE);
        pill_infos.push((role.clone(), galley.size()));
    }

    // Position pills from right to left at bottom-right
    let mut x_offset = rect.right() - right_margin;
    let y_pos = rect.bottom() - bottom_margin;

    let bg_color = apply_opacity(Color32::from_rgba_unmultiplied(0, 0, 0, 180), opacity);
    let text_color = apply_opacity(Color32::from_rgb(220, 220, 220), opacity);

    for (role_text, text_size) in pill_infos {
        let pill_width = text_size.x + padding.x * 2.0;
        let pill_height = text_size.y + padding.y * 2.0;

        // Check if pill would go past left edge
        if x_offset - pill_width < rect.left() + 4.0 {
            break; // Stop adding pills if we run out of space
        }

        let pill_rect = egui::Rect::from_min_size(
            Pos2::new(x_offset - pill_width, y_pos - pill_height),
            Vec2::new(pill_width, pill_height),
        );

        // Draw background pill
        painter.rect_filled(pill_rect, 3.0, bg_color);

        // Draw text centered in pill
        painter.text(
            pill_rect.center(),
            egui::Align2::CENTER_CENTER,
            role_text,
            font.clone(),
            text_color,
        );

        // Move left for next pill
        x_offset -= pill_width + spacing;
    }
}

fn render_role_badge(painter: &egui::Painter, node: &LayoutNode, rect: egui::Rect, opacity: f32) {
    // Try primary_role first, then fall back to first role in all_roles
    let badge_text = {
        let from_primary = role_badge_text(&node.primary_role);
        if !from_primary.is_empty() {
            from_primary.to_string()
        } else if let Some(first_role) = node.all_roles.first() {
            // Use abbreviated form of first role from all_roles
            abbreviate_role(first_role)
        } else {
            return; // No role to display
        }
    };

    // Use a subtle background pill for better visibility
    let badge_pos = rect.right_top() + Vec2::new(-5.0, 5.0);
    let font = FontId::proportional(11.0);

    // Measure text for background
    let galley = painter.layout_no_wrap(badge_text.clone(), font.clone(), Color32::WHITE);
    let text_size = galley.size();
    let padding = Vec2::new(4.0, 2.0);
    let bg_rect = egui::Rect::from_min_size(
        badge_pos - Vec2::new(text_size.x + padding.x, 0.0),
        text_size + padding * 2.0,
    );

    // Draw background pill
    let bg_color = apply_opacity(Color32::from_rgba_unmultiplied(0, 0, 0, 160), opacity);
    painter.rect_filled(bg_rect, 3.0, bg_color);

    // Draw text
    let text_color = apply_opacity(Color32::from_rgb(220, 220, 220), opacity);
    painter.text(
        badge_pos,
        egui::Align2::RIGHT_TOP,
        badge_text,
        font,
        text_color,
    );
}

/// Abbreviate a role string (e.g., "MANAGEMENT_COMPANY" -> "ManCo")
fn abbreviate_role(role: &str) -> String {
    match role.to_uppercase().replace('-', "_").as_str() {
        "MANAGEMENT_COMPANY" | "MANCO" => "ManCo".to_string(),
        "INVESTMENT_MANAGER" => "IM".to_string(),
        "ASSET_OWNER" => "AO".to_string(),
        "ULTIMATE_BENEFICIAL_OWNER" | "UBO" => "UBO".to_string(),
        "BENEFICIAL_OWNER" => "BO".to_string(),
        "DIRECTOR" => "Dir".to_string(),
        "SHAREHOLDER" => "SH".to_string(),
        "AUTHORIZED_SIGNATORY" => "AS".to_string(),
        "CUSTODIAN" => "Cust".to_string(),
        "DEPOSITARY" => "Dep".to_string(),
        "ADMINISTRATOR" => "Admin".to_string(),
        "TRANSFER_AGENT" => "TA".to_string(),
        "PRIME_BROKER" => "PB".to_string(),
        "PRINCIPAL" => "Prin".to_string(),
        "TRUSTEE" => "Trustee".to_string(),
        "SETTLOR" => "Settlor".to_string(),
        "BENEFICIARY" => "Ben".to_string(),
        "PROTECTOR" => "Prot".to_string(),
        _ => {
            // For unknown roles, take first 6 chars or capitalize nicely
            if role.len() <= 6 {
                role.to_string()
            } else {
                role.chars().take(6).collect()
            }
        }
    }
}

fn render_triangle(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    fill: Color32,
    border: Color32,
    is_ghost: bool,
) {
    let top = center - Vec2::new(0.0, radius);
    let bottom_left = center + Vec2::new(-radius * 0.866, radius * 0.5);
    let bottom_right = center + Vec2::new(radius * 0.866, radius * 0.5);

    if is_ghost {
        // Fill without stroke, then draw dashed edges
        painter.add(egui::Shape::convex_polygon(
            vec![top, bottom_left, bottom_right],
            fill,
            Stroke::NONE,
        ));
        let stroke = Stroke::new(1.5, border);
        // Draw dashed triangle edges
        draw_dashed_line_segment(painter, top, bottom_right, stroke);
        draw_dashed_line_segment(painter, bottom_right, bottom_left, stroke);
        draw_dashed_line_segment(painter, bottom_left, top, stroke);
    } else {
        painter.add(egui::Shape::convex_polygon(
            vec![top, bottom_left, bottom_right],
            fill,
            Stroke::new(1.5, border),
        ));
    }
}

/// Render a hexagon shape for ControlPortal nodes
fn render_hexagon(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    fill: Color32,
    border: Color32,
) {
    // 6 points, starting from top, going clockwise
    let mut points = Vec::with_capacity(6);
    for i in 0..6 {
        let angle = std::f32::consts::FRAC_PI_3 * i as f32 - std::f32::consts::FRAC_PI_2;
        let x = center.x + radius * angle.cos();
        let y = center.y + radius * angle.sin();
        points.push(Pos2::new(x, y));
    }

    painter.add(egui::Shape::convex_polygon(
        points,
        fill,
        Stroke::new(2.0, border),
    ));
}

/// Render a hexagon with label and optional glow for Standard/Expanded LOD
fn render_hexagon_with_label(
    painter: &egui::Painter,
    node: &LayoutNode,
    center: Pos2,
    size: Vec2,
    opacity: f32,
) {
    let confidence = node
        .control_confidence
        .as_deref()
        .and_then(|s| s.parse::<ControlConfidence>().ok())
        .unwrap_or(ControlConfidence::Medium);

    let fill = apply_opacity(control_portal_fill(confidence), opacity);
    let border = apply_opacity(control_portal_border(confidence), opacity);

    // Use width as radius base (hexagon fits in a circle)
    let radius = size.x.min(size.y) / 2.0;

    // Outer glow for emphasis
    let glow_color = apply_opacity(
        super::colors::control_portal_glow(confidence),
        opacity * 0.5,
    );
    painter.circle_filled(center, radius + 4.0, glow_color);

    // Draw hexagon
    render_hexagon(painter, center, radius, fill, border);

    // Label inside hexagon
    let font_size = (size.y * 0.18).clamp(10.0, 14.0);
    let text_color = apply_opacity(Color32::WHITE, opacity);

    // Primary label: "Board Controller" or entity name
    let label = if node.label.is_empty() {
        "Board Controller"
    } else {
        &node.label
    };

    painter.text(
        center - Vec2::new(0.0, font_size * 0.5),
        egui::Align2::CENTER_CENTER,
        truncate_name(label, 14),
        FontId::proportional(font_size),
        text_color,
    );

    // Confidence badge below label
    let confidence_text = match confidence {
        ControlConfidence::High => "High",
        ControlConfidence::Medium => "Medium",
        ControlConfidence::Low => "Low",
    };
    let badge_color = apply_opacity(Color32::from_rgba_unmultiplied(0, 0, 0, 140), opacity);
    let badge_text_color = apply_opacity(Color32::WHITE, opacity);

    let badge_pos = center + Vec2::new(0.0, font_size * 0.8);
    let badge_font = FontId::proportional(font_size * 0.7);
    let galley = painter.layout_no_wrap(
        confidence_text.to_string(),
        badge_font.clone(),
        badge_text_color,
    );
    let badge_rect = egui::Rect::from_center_size(badge_pos, galley.size() + Vec2::new(8.0, 4.0));
    painter.rect_filled(badge_rect, 3.0, badge_color);
    painter.text(
        badge_pos,
        egui::Align2::CENTER_CENTER,
        confidence_text,
        badge_font,
        badge_text_color,
    );

    // Rule indicator (small letter in corner)
    if let Some(ref rule) = node.control_rule {
        let rule_pos = center + Vec2::new(radius * 0.6, -radius * 0.6);
        let rule_color = apply_opacity(Color32::from_rgb(200, 200, 200), opacity);
        painter.text(
            rule_pos,
            egui::Align2::CENTER_CENTER,
            format!("R{}", rule),
            FontId::proportional(font_size * 0.6),
            rule_color,
        );
    }
}

fn render_focus_ring(painter: &egui::Painter, ctx: &RenderContext) {
    let ring_size = ctx.size + Vec2::splat(8.0);
    let rect = egui::Rect::from_center_size(ctx.pos, ring_size);
    painter.rect_stroke(
        rect,
        12.0,
        Stroke::new(2.0, Color32::from_rgb(59, 130, 246)),
    );
}

/// Render a KYC completion progress bar at the bottom of the node
fn render_kyc_completion_bar(
    painter: &egui::Painter,
    rect: egui::Rect,
    completion: i32,
    opacity: f32,
) {
    let bar_height = 4.0;
    let bar_margin = 6.0;
    let bar_width = rect.width() - bar_margin * 2.0;

    // Background track
    let track_rect = egui::Rect::from_min_size(
        Pos2::new(rect.left() + bar_margin, rect.bottom() - bar_height - 4.0),
        Vec2::new(bar_width, bar_height),
    );
    let track_color = apply_opacity(Color32::from_rgb(55, 65, 81), opacity);
    painter.rect_filled(track_rect, bar_height / 2.0, track_color);

    // Progress fill
    let progress = (completion as f32 / 100.0).clamp(0.0, 1.0);
    let fill_width = bar_width * progress;

    if fill_width > 0.0 {
        let fill_rect =
            egui::Rect::from_min_size(track_rect.min, Vec2::new(fill_width, bar_height));

        // Color based on completion level
        let fill_color = if completion >= 80 {
            apply_opacity(Color32::from_rgb(34, 197, 94), opacity) // Green
        } else if completion >= 50 {
            apply_opacity(Color32::from_rgb(251, 191, 36), opacity) // Amber
        } else {
            apply_opacity(Color32::from_rgb(239, 68, 68), opacity) // Red
        };

        painter.rect_filled(fill_rect, bar_height / 2.0, fill_color);
    }
}

/// Render attention indicator (exclamation badge) for nodes needing action
fn render_attention_indicator(painter: &egui::Painter, rect: egui::Rect, opacity: f32) {
    let badge_size = 16.0;
    let badge_pos = rect.right_top() + Vec2::new(4.0, -4.0);

    // Red circle background
    let bg_color = apply_opacity(Color32::from_rgb(239, 68, 68), opacity);
    painter.circle_filled(badge_pos, badge_size / 2.0, bg_color);

    // White border
    painter.circle_stroke(
        badge_pos,
        badge_size / 2.0,
        Stroke::new(1.5, apply_opacity(Color32::WHITE, opacity)),
    );

    // Exclamation mark
    let text_color = apply_opacity(Color32::WHITE, opacity);
    painter.text(
        badge_pos,
        egui::Align2::CENTER_CENTER,
        "!",
        FontId::proportional(10.0),
        text_color,
    );
}

// =============================================================================
// UTILITIES
// =============================================================================

fn truncate_name(name: &str, max_chars: usize) -> String {
    if name.len() <= max_chars {
        name.to_string()
    } else {
        format!("{}...", &name[..max_chars.saturating_sub(3)])
    }
}

fn apply_opacity(color: Color32, opacity: f32) -> Color32 {
    let [r, g, b, a] = color.to_array();
    Color32::from_rgba_unmultiplied(r, g, b, (a as f32 * opacity) as u8)
}

/// Draw a dashed rectangle for ghost entities
/// Uses individual line segments to create a dashed effect
fn draw_dashed_rect(painter: &egui::Painter, rect: egui::Rect, rounding: f32, stroke: Stroke) {
    let dash_len = 6.0;
    let gap_len = 4.0;
    let segment = dash_len + gap_len;

    // Helper to draw dashed line between two points
    let draw_dashed_line = |p1: Pos2, p2: Pos2| {
        let delta = p2 - p1;
        let length = delta.length();
        let dir = delta / length;

        let mut pos = 0.0;
        while pos < length {
            let start = p1 + dir * pos;
            let end_pos = (pos + dash_len).min(length);
            let end = p1 + dir * end_pos;
            painter.line_segment([start, end], stroke);
            pos += segment;
        }
    };

    // Get corners with rounding offset
    let r = rounding.min(rect.width() / 2.0).min(rect.height() / 2.0);

    // Top edge (excluding corners)
    draw_dashed_line(
        Pos2::new(rect.left() + r, rect.top()),
        Pos2::new(rect.right() - r, rect.top()),
    );
    // Right edge
    draw_dashed_line(
        Pos2::new(rect.right(), rect.top() + r),
        Pos2::new(rect.right(), rect.bottom() - r),
    );
    // Bottom edge
    draw_dashed_line(
        Pos2::new(rect.right() - r, rect.bottom()),
        Pos2::new(rect.left() + r, rect.bottom()),
    );
    // Left edge
    draw_dashed_line(
        Pos2::new(rect.left(), rect.bottom() - r),
        Pos2::new(rect.left(), rect.top() + r),
    );

    // Draw corner arcs as short segments (approximation)
    // Top-left corner
    painter.line_segment(
        [
            Pos2::new(rect.left(), rect.top() + r),
            Pos2::new(rect.left() + r * 0.3, rect.top() + r * 0.3),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(rect.left() + r * 0.3, rect.top() + r * 0.3),
            Pos2::new(rect.left() + r, rect.top()),
        ],
        stroke,
    );
    // Top-right corner
    painter.line_segment(
        [
            Pos2::new(rect.right() - r, rect.top()),
            Pos2::new(rect.right() - r * 0.3, rect.top() + r * 0.3),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(rect.right() - r * 0.3, rect.top() + r * 0.3),
            Pos2::new(rect.right(), rect.top() + r),
        ],
        stroke,
    );
    // Bottom-right corner
    painter.line_segment(
        [
            Pos2::new(rect.right(), rect.bottom() - r),
            Pos2::new(rect.right() - r * 0.3, rect.bottom() - r * 0.3),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(rect.right() - r * 0.3, rect.bottom() - r * 0.3),
            Pos2::new(rect.right() - r, rect.bottom()),
        ],
        stroke,
    );
    // Bottom-left corner
    painter.line_segment(
        [
            Pos2::new(rect.left() + r, rect.bottom()),
            Pos2::new(rect.left() + r * 0.3, rect.bottom() - r * 0.3),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(rect.left() + r * 0.3, rect.bottom() - r * 0.3),
            Pos2::new(rect.left(), rect.bottom() - r),
        ],
        stroke,
    );
}

/// Draw a dashed line segment between two points
fn draw_dashed_line_segment(painter: &egui::Painter, p1: Pos2, p2: Pos2, stroke: Stroke) {
    let dash_len = 6.0;
    let gap_len = 4.0;
    let segment = dash_len + gap_len;

    let delta = p2 - p1;
    let length = delta.length();
    if length < 0.001 {
        return;
    }
    let dir = delta / length;

    let mut pos = 0.0;
    while pos < length {
        let start = p1 + dir * pos;
        let end_pos = (pos + dash_len).min(length);
        let end = p1 + dir * end_pos;
        painter.line_segment([start, end], stroke);
        pos += segment;
    }
}

/// Draw a dashed circle for ghost entities
fn draw_dashed_circle(painter: &egui::Painter, center: Pos2, radius: f32, stroke: Stroke) {
    let dash_arc = 0.3; // Radians per dash
    let gap_arc = 0.2; // Radians per gap
    let segment_arc = dash_arc + gap_arc;
    let total = std::f32::consts::TAU;

    let mut angle = 0.0;
    while angle < total {
        let start_angle = angle;
        let end_angle = (angle + dash_arc).min(total);

        // Draw arc segment as short lines
        let segments = 4;
        for i in 0..segments {
            let a1 = start_angle + (end_angle - start_angle) * (i as f32 / segments as f32);
            let a2 = start_angle + (end_angle - start_angle) * ((i + 1) as f32 / segments as f32);
            let p1 = center + Vec2::new(a1.cos(), a1.sin()) * radius;
            let p2 = center + Vec2::new(a2.cos(), a2.sin()) * radius;
            painter.line_segment([p1, p2], stroke);
        }

        angle += segment_arc;
    }
}

fn role_badge_text(role: &PrimaryRole) -> &'static str {
    match role {
        PrimaryRole::UltimateBeneficialOwner => "UBO",
        PrimaryRole::BeneficialOwner => "BO",
        PrimaryRole::Shareholder => "SH",
        PrimaryRole::GeneralPartner => "GP",
        PrimaryRole::LimitedPartner => "LP",
        PrimaryRole::Director => "Dir",
        PrimaryRole::Officer => "Off",
        PrimaryRole::ConductingOfficer => "CO",
        PrimaryRole::ChiefComplianceOfficer => "CCO",
        PrimaryRole::Trustee => "Trustee",
        PrimaryRole::Protector => "Prot",
        PrimaryRole::Beneficiary => "Ben",
        PrimaryRole::Settlor => "Settlor",
        PrimaryRole::Principal => "Prin",
        PrimaryRole::AssetOwner => "AO",
        PrimaryRole::MasterFund => "Master",
        PrimaryRole::FeederFund => "Feeder",
        PrimaryRole::SegregatedPortfolio => "SP",
        PrimaryRole::ManagementCompany => "ManCo",
        PrimaryRole::InvestmentManager => "IM",
        PrimaryRole::InvestmentAdvisor => "IA",
        PrimaryRole::Sponsor => "Sponsor",
        PrimaryRole::Administrator => "Admin",
        PrimaryRole::Custodian => "Cust",
        PrimaryRole::Depositary => "Dep",
        PrimaryRole::TransferAgent => "TA",
        PrimaryRole::Distributor => "Dist",
        PrimaryRole::PrimeBroker => "PB",
        PrimaryRole::Auditor => "Audit",
        PrimaryRole::LegalCounsel => "Legal",
        PrimaryRole::AuthorizedSignatory => "AS",
        PrimaryRole::ContactPerson => "CP",
        PrimaryRole::CommercialClient => "Client",
        PrimaryRole::Unknown => "",
    }
}
