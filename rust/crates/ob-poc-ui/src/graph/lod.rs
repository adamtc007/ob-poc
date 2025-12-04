//! Level of Detail (LOD) system for graph rendering
//!
//! Adjusts rendering detail based on screen-space size of nodes.

use egui::{Color32, FontId, Pos2, Stroke, Vec2};

use super::colors::role_color;
use super::types::{EntityType, LayoutNode, PrimaryRole};

// =============================================================================
// DETAIL LEVEL
// =============================================================================

/// Level of detail for node rendering
/// Thresholds adjusted: Micro < 8px < Icon < 20px < Compact < 40px < Standard < 80px < Expanded
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    /// < 8px: colored dot only
    Micro,
    /// 8-20px: shape + status color (no text)
    Icon,
    /// 20-40px: shape + truncated name
    Compact,
    /// 40-80px: shape + full name + badge
    Standard,
    /// 80px+: all details inline
    Expanded,
    /// clicked node: full card (rendered separately)
    Focused,
}

impl DetailLevel {
    /// Determine LOD from screen-space radius
    /// Based on zoom percentage of base node size (~100px):
    /// - Icon only: < 20% (< 20px)
    /// - Labels (Compact): 20-70% (20-70px)
    /// - Full text (Standard+): 70%+ (70px+)
    pub fn from_screen_size(screen_width: f32, is_focused: bool) -> Self {
        if is_focused {
            return DetailLevel::Focused;
        }

        match screen_width {
            w if w < 10.0 => DetailLevel::Micro,     // tiny dot
            w if w < 20.0 => DetailLevel::Icon,      // icon only, no text (< 20%)
            w if w < 70.0 => DetailLevel::Compact,   // labels/truncated name (20-70%)
            w if w < 120.0 => DetailLevel::Standard, // full text (70%+)
            _ => DetailLevel::Expanded,              // all details
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
    let fill = apply_opacity(node.style.fill_color, opacity);
    let border = apply_opacity(node.style.border_color, opacity);
    let text_color = apply_opacity(node.style.text_color, opacity);

    match lod {
        DetailLevel::Micro => {
            render_micro(painter, screen_pos, fill);
        }
        DetailLevel::Icon => {
            render_icon(
                painter,
                node,
                screen_pos,
                screen_size,
                fill,
                border,
                opacity,
            );
        }
        DetailLevel::Compact => {
            render_compact(
                painter,
                node,
                screen_pos,
                screen_size,
                fill,
                border,
                text_color,
            );
        }
        DetailLevel::Standard => {
            render_standard(
                painter,
                node,
                screen_pos,
                screen_size,
                fill,
                border,
                text_color,
                opacity,
            );
        }
        DetailLevel::Expanded => {
            render_expanded(
                painter,
                node,
                screen_pos,
                screen_size,
                fill,
                border,
                text_color,
                opacity,
            );
        }
        DetailLevel::Focused => {
            // Focused nodes render at Standard detail, card is separate
            render_standard(
                painter,
                node,
                screen_pos,
                screen_size,
                fill,
                border,
                text_color,
                opacity,
            );
            render_focus_ring(painter, screen_pos, screen_size);
        }
    }
}

// =============================================================================
// LOD RENDERERS
// =============================================================================

/// Micro: Just a colored dot
fn render_micro(painter: &egui::Painter, pos: Pos2, color: Color32) {
    painter.circle_filled(pos, 6.0, color);
    // White outline for visibility
    painter.circle_stroke(pos, 6.0, Stroke::new(1.0, Color32::WHITE));
}

/// Icon: Shape with entity type color - NO TEXT at this level
fn render_icon(
    painter: &egui::Painter,
    node: &LayoutNode,
    pos: Pos2,
    size: Vec2,
    fill: Color32,
    border: Color32,
    opacity: f32,
) {
    let radius = size.x.min(size.y) / 2.0;

    // Render shape based on entity type - NO TEXT, just shapes
    match node.entity_type {
        EntityType::ProperPerson => {
            // Circle for persons
            painter.circle_filled(pos, radius, fill);
            painter.circle_stroke(pos, radius, Stroke::new(2.0, border));
        }
        EntityType::Trust => {
            // Triangle for trusts
            render_triangle(painter, pos, radius, fill, border);
        }
        _ => {
            // Rounded rect for companies/funds/CBU
            let rect = egui::Rect::from_center_size(pos, Vec2::splat(radius * 1.8));
            painter.rect_filled(rect, 4.0, fill);
            painter.rect_stroke(rect, 4.0, Stroke::new(2.0, border));
        }
    }

    // UBO indicator - small green dot
    if node.primary_role == PrimaryRole::UltimateBeneficialOwner {
        let badge_pos = pos + Vec2::new(radius * 0.7, -radius * 0.7);
        let badge_color = apply_opacity(Color32::from_rgb(76, 175, 80), opacity);
        painter.circle_filled(badge_pos, 5.0, badge_color);
    }
}

/// Compact: Shape + truncated name
fn render_compact(
    painter: &egui::Painter,
    node: &LayoutNode,
    pos: Pos2,
    size: Vec2,
    fill: Color32,
    border: Color32,
    text_color: Color32,
) {
    let rect = egui::Rect::from_center_size(pos, size);
    let corner = 6.0;

    painter.rect_filled(rect, corner, fill);
    painter.rect_stroke(rect, corner, Stroke::new(1.5, border));

    // Truncated name
    let name = truncate_name(&node.label, 12);
    let font_size = (size.y * 0.25).clamp(8.0, 11.0);
    painter.text(
        pos,
        egui::Align2::CENTER_CENTER,
        name,
        FontId::proportional(font_size),
        text_color,
    );
}

/// Standard: Full name + role badge
fn render_standard(
    painter: &egui::Painter,
    node: &LayoutNode,
    pos: Pos2,
    size: Vec2,
    fill: Color32,
    border: Color32,
    text_color: Color32,
    opacity: f32,
) {
    let rect = egui::Rect::from_center_size(pos, size);
    let corner = 8.0;
    let border_width = if node.is_cbu_root { 3.0 } else { 2.0 };

    painter.rect_filled(rect, corner, fill);
    painter.rect_stroke(rect, corner, Stroke::new(border_width, border));

    // Label
    let font_size = (size.y * 0.18).clamp(10.0, 13.0);
    painter.text(
        pos - Vec2::new(0.0, size.y * 0.15),
        egui::Align2::CENTER_CENTER,
        &node.label,
        FontId::proportional(font_size),
        text_color,
    );

    // Sublabel
    if let Some(ref sublabel) = node.sublabel {
        let sublabel_color = apply_opacity(Color32::from_rgb(180, 180, 180), opacity);
        painter.text(
            pos + Vec2::new(0.0, size.y * 0.12),
            egui::Align2::CENTER_CENTER,
            sublabel,
            FontId::proportional(font_size * 0.8),
            sublabel_color,
        );
    }

    // Role badge (top-right)
    if !node.is_cbu_root {
        render_role_badge(painter, node, rect, opacity);
    }

    // Jurisdiction (top-left)
    if let Some(ref jurisdiction) = node.jurisdiction {
        let flag_color = apply_opacity(Color32::from_rgb(120, 120, 120), opacity);
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
fn render_expanded(
    painter: &egui::Painter,
    node: &LayoutNode,
    pos: Pos2,
    size: Vec2,
    fill: Color32,
    border: Color32,
    text_color: Color32,
    opacity: f32,
) {
    // Base rendering same as standard
    render_standard(painter, node, pos, size, fill, border, text_color, opacity);

    // Additional details at bottom of node
    let rect = egui::Rect::from_center_size(pos, size);

    // Show all roles
    if node.all_roles.len() > 1 {
        let roles_text = node.all_roles.join(", ");
        let roles_color = apply_opacity(Color32::from_rgb(150, 150, 150), opacity);
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

/// Render role badge in top-right corner
fn render_role_badge(painter: &egui::Painter, node: &LayoutNode, rect: egui::Rect, opacity: f32) {
    let badge_text = role_badge_text(&node.primary_role);
    if badge_text.is_empty() {
        return;
    }

    let badge_color = apply_opacity(role_color(node.primary_role), opacity);
    let badge_pos = rect.right_top() + Vec2::new(-5.0, 5.0);

    painter.text(
        badge_pos,
        egui::Align2::RIGHT_TOP,
        badge_text,
        FontId::proportional(9.0),
        badge_color,
    );
}

/// Render triangle shape for trusts
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

/// Render focus ring around selected node
fn render_focus_ring(painter: &egui::Painter, pos: Pos2, size: Vec2) {
    let ring_size = size + Vec2::splat(8.0);
    let rect = egui::Rect::from_center_size(pos, ring_size);
    painter.rect_stroke(
        rect,
        12.0,
        Stroke::new(2.0, Color32::from_rgb(59, 130, 246)),
    );
}

// =============================================================================
// UTILITIES
// =============================================================================

/// Truncate name to max characters
fn truncate_name(name: &str, max_chars: usize) -> String {
    if name.len() <= max_chars {
        name.to_string()
    } else {
        format!("{}...", &name[..max_chars.saturating_sub(3)])
    }
}

/// Apply opacity to a color
fn apply_opacity(color: Color32, opacity: f32) -> Color32 {
    let [r, g, b, a] = color.to_array();
    Color32::from_rgba_unmultiplied(r, g, b, (a as f32 * opacity) as u8)
}

/// Get abbreviated role text for badge
fn role_badge_text(role: &PrimaryRole) -> &'static str {
    match role {
        PrimaryRole::UltimateBeneficialOwner => "UBO",
        PrimaryRole::Shareholder => "SH",
        PrimaryRole::ManagementCompany => "ManCo",
        PrimaryRole::Director => "Dir",
        PrimaryRole::Officer => "Off",
        PrimaryRole::Principal => "Prin",
        PrimaryRole::Trustee => "Trustee",
        PrimaryRole::Protector => "Prot",
        PrimaryRole::Beneficiary => "Ben",
        PrimaryRole::Settlor => "Settlor",
        PrimaryRole::AuthorizedSignatory => "AS",
        PrimaryRole::ContactPerson => "CP",
        PrimaryRole::Unknown => "",
    }
}
