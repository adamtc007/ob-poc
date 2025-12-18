//! Level of Detail (LOD) system for graph rendering
//!
//! Adjusts rendering detail based on screen-space size of nodes.

use egui::{Color32, FontId, Pos2, Stroke, Vec2};

use super::types::{EntityType, LayoutNode, PrimaryRole};

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
}

impl RenderContext {
    fn new(node: &LayoutNode, pos: Pos2, size: Vec2, opacity: f32) -> Self {
        Self {
            pos,
            size,
            fill: apply_opacity(node.style.fill_color, opacity),
            border: apply_opacity(node.style.border_color, opacity),
            text_color: apply_opacity(node.style.text_color, opacity),
            opacity,
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

impl DetailLevel {
    /// Determine LOD from screen-space radius
    pub fn from_screen_size(screen_width: f32, is_focused: bool) -> Self {
        if is_focused {
            return DetailLevel::Focused;
        }

        match screen_width {
            w if w < 8.0 => DetailLevel::Micro,
            w if w < 30.0 => DetailLevel::Icon,
            w if w < 80.0 => DetailLevel::Compact,
            w if w < 120.0 => DetailLevel::Standard,
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

    match node.entity_type {
        EntityType::ProperPerson => {
            painter.circle_filled(ctx.pos, radius, ctx.fill);
            painter.circle_stroke(ctx.pos, radius, Stroke::new(2.0, ctx.border));
        }
        EntityType::Trust => {
            render_triangle(painter, ctx.pos, radius, ctx.fill, ctx.border);
        }
        _ => {
            let rect = egui::Rect::from_center_size(ctx.pos, Vec2::splat(radius * 1.8));
            painter.rect_filled(rect, 4.0, ctx.fill);
            painter.rect_stroke(rect, 4.0, Stroke::new(2.0, ctx.border));
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

    painter.rect_filled(rect, 6.0, ctx.fill);
    painter.rect_stroke(rect, 6.0, Stroke::new(1.5, ctx.border));

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

/// Standard: Full name + role badge + KYC completion bar
fn render_standard(painter: &egui::Painter, node: &LayoutNode, ctx: &RenderContext) {
    let rect = ctx.rect();
    let border_width = if node.is_cbu_root { 3.0 } else { 2.0 };

    painter.rect_filled(rect, 8.0, ctx.fill);
    painter.rect_stroke(rect, 8.0, Stroke::new(border_width, ctx.border));

    // Label - shift up slightly if we have a KYC bar
    let has_kyc_bar = node.kyc_completion.is_some();
    let label_offset = if has_kyc_bar { 0.18 } else { 0.15 };
    let font_size = (ctx.size.y * 0.18).clamp(10.0, 13.0);
    painter.text(
        ctx.pos - Vec2::new(0.0, ctx.size.y * label_offset),
        egui::Align2::CENTER_CENTER,
        &node.label,
        FontId::proportional(font_size),
        ctx.text_color,
    );

    // Sublabel
    if let Some(ref sublabel) = node.sublabel {
        let sublabel_color = apply_opacity(Color32::from_rgb(180, 180, 180), ctx.opacity);
        let sublabel_offset = if has_kyc_bar { 0.08 } else { 0.12 };
        painter.text(
            ctx.pos + Vec2::new(0.0, ctx.size.y * sublabel_offset),
            egui::Align2::CENTER_CENTER,
            sublabel,
            FontId::proportional(font_size * 0.8),
            sublabel_color,
        );
    }

    if !node.is_cbu_root {
        render_role_badge(painter, node, rect, ctx.opacity);
    }

    // Entity category indicator (top-left) - only if no jurisdiction
    if node.jurisdiction.is_none() {
        if let Some(ref category) = node.entity_category {
            render_entity_category_indicator(painter, rect, category, ctx.opacity);
        }
    }

    // Jurisdiction (top-left)
    if let Some(ref jurisdiction) = node.jurisdiction {
        let flag_color = apply_opacity(Color32::from_rgb(120, 120, 120), ctx.opacity);
        painter.text(
            rect.left_top() + Vec2::new(5.0, 5.0),
            egui::Align2::LEFT_TOP,
            jurisdiction,
            FontId::proportional(9.0),
            flag_color,
        );
    }

    // KYC completion bar at bottom
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

fn render_role_badge(painter: &egui::Painter, node: &LayoutNode, rect: egui::Rect, opacity: f32) {
    let badge_text = role_badge_text(&node.primary_role);
    if badge_text.is_empty() {
        return;
    }

    let text_color = apply_opacity(Color32::BLACK, opacity);
    let badge_pos = rect.right_top() + Vec2::new(-5.0, 5.0);

    painter.text(
        badge_pos,
        egui::Align2::RIGHT_TOP,
        badge_text,
        FontId::proportional(9.0),
        text_color,
    );
}

fn render_triangle(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    fill: Color32,
    border: Color32,
) {
    let top = center - Vec2::new(0.0, radius);
    let bottom_left = center + Vec2::new(-radius * 0.866, radius * 0.5);
    let bottom_right = center + Vec2::new(radius * 0.866, radius * 0.5);

    painter.add(egui::Shape::convex_polygon(
        vec![top, bottom_left, bottom_right],
        fill,
        Stroke::new(1.5, border),
    ));
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

/// Render entity category indicator (PERSON = circle, SHELL = square) at top-left
fn render_entity_category_indicator(
    painter: &egui::Painter,
    rect: egui::Rect,
    category: &str,
    opacity: f32,
) {
    let indicator_size = 8.0;
    let indicator_pos = rect.left_top() + Vec2::new(8.0, 8.0);

    let color = if category == "PERSON" {
        apply_opacity(Color32::from_rgb(96, 165, 250), opacity) // Blue for person
    } else {
        apply_opacity(Color32::from_rgb(168, 85, 247), opacity) // Purple for shell
    };

    if category == "PERSON" {
        painter.circle_filled(indicator_pos, indicator_size / 2.0, color);
    } else {
        let indicator_rect =
            egui::Rect::from_center_size(indicator_pos, Vec2::splat(indicator_size));
        painter.rect_filled(indicator_rect, 2.0, color);
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
