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

        // screen_width = node.size.x * camera.zoom()
        // Typical node is ~140px wide, so at zoom 1.0 screen_width=140
        // At zoom 0.5, screen_width=70. At zoom 2.0, screen_width=280
        match screen_width {
            w if w < 20.0 => DetailLevel::Micro,
            w if w < 40.0 => DetailLevel::Icon,
            w if w < 70.0 => DetailLevel::Compact,
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

/// Standard: Name at top (wrapped), role labels at bottom-right
/// Clean layout - no jurisdiction or entity type clutter
fn render_standard(painter: &egui::Painter, node: &LayoutNode, ctx: &RenderContext) {
    let rect = ctx.rect();
    let border_width = if node.is_cbu_root { 3.0 } else { 2.0 };

    painter.rect_filled(rect, 8.0, ctx.fill);
    painter.rect_stroke(rect, 8.0, Stroke::new(border_width, ctx.border));

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
