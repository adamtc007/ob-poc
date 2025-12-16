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
}

/// Standard: Full name + role badge
fn render_standard(painter: &egui::Painter, node: &LayoutNode, ctx: &RenderContext) {
    let rect = ctx.rect();
    let border_width = if node.is_cbu_root { 3.0 } else { 2.0 };

    painter.rect_filled(rect, 8.0, ctx.fill);
    painter.rect_stroke(rect, 8.0, Stroke::new(border_width, ctx.border));

    // Label
    let font_size = (ctx.size.y * 0.18).clamp(10.0, 13.0);
    painter.text(
        ctx.pos - Vec2::new(0.0, ctx.size.y * 0.15),
        egui::Align2::CENTER_CENTER,
        &node.label,
        FontId::proportional(font_size),
        ctx.text_color,
    );

    // Sublabel
    if let Some(ref sublabel) = node.sublabel {
        let sublabel_color = apply_opacity(Color32::from_rgb(180, 180, 180), ctx.opacity);
        painter.text(
            ctx.pos + Vec2::new(0.0, ctx.size.y * 0.12),
            egui::Align2::CENTER_CENTER,
            sublabel,
            FontId::proportional(font_size * 0.8),
            sublabel_color,
        );
    }

    if !node.is_cbu_root {
        render_role_badge(painter, node, rect, ctx.opacity);
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
}

/// Expanded: All details inline
fn render_expanded(painter: &egui::Painter, node: &LayoutNode, ctx: &RenderContext) {
    render_standard(painter, node, ctx);

    // Additional details at bottom
    if node.all_roles.len() > 1 {
        let rect = ctx.rect();
        let roles_text = node.all_roles.join(", ");
        let roles_color = apply_opacity(Color32::from_rgb(150, 150, 150), ctx.opacity);
        painter.text(
            rect.center_bottom() - Vec2::new(0.0, 8.0),
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
