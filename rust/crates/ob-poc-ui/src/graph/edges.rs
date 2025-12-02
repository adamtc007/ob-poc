//! Bezier edge routing and rendering
//!
//! Provides smooth curved edges with arrows and labels.

#![allow(dead_code)]

use egui::{Color32, Pos2, Stroke, Vec2};

use super::types::{EdgeType, PrimaryRole};

// =============================================================================
// BEZIER CURVE
// =============================================================================

/// Quadratic bezier curve for edge routing
#[derive(Debug, Clone, Copy)]
pub struct EdgeCurve {
    pub from: Pos2,
    pub to: Pos2,
    pub control: Pos2,
}

impl EdgeCurve {
    /// Create a new bezier curve between two points
    pub fn new(from: Pos2, to: Pos2, curve_strength: f32) -> Self {
        let delta = to - from;
        let distance = delta.length();

        // Perpendicular offset for control point
        let perpendicular = Vec2::new(-delta.y, delta.x).normalized();
        let offset = perpendicular * distance * curve_strength;

        let control = Pos2::new(
            (from.x + to.x) / 2.0 + offset.x,
            (from.y + to.y) / 2.0 + offset.y,
        );

        Self { from, to, control }
    }

    /// Create a straight line (no curve)
    pub fn straight(from: Pos2, to: Pos2) -> Self {
        let mid = Pos2::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
        Self {
            from,
            to,
            control: mid,
        }
    }

    /// Get point on curve at parameter t (0.0 to 1.0)
    pub fn point_at(&self, t: f32) -> Pos2 {
        let t2 = t * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;

        Pos2::new(
            self.from.x * mt2 + self.control.x * (2.0 * mt * t) + self.to.x * t2,
            self.from.y * mt2 + self.control.y * (2.0 * mt * t) + self.to.y * t2,
        )
    }

    /// Get tangent direction at parameter t
    pub fn tangent_at(&self, t: f32) -> Vec2 {
        let mt = 1.0 - t;
        Vec2::new(
            2.0 * mt * (self.control.x - self.from.x) + 2.0 * t * (self.to.x - self.control.x),
            2.0 * mt * (self.control.y - self.from.y) + 2.0 * t * (self.to.y - self.control.y),
        )
        .normalized()
    }

    /// Get direction at the end of the curve (for arrow)
    pub fn end_direction(&self) -> Vec2 {
        (self.to - self.control).normalized()
    }

    /// Get the midpoint of the curve
    pub fn midpoint(&self) -> Pos2 {
        self.point_at(0.5)
    }
}

// =============================================================================
// CURVE STRENGTH BY ROLE
// =============================================================================

/// Determine curve strength based on edge type/role
pub fn curve_strength_for_edge(edge_type: EdgeType, _role: Option<&PrimaryRole>) -> f32 {
    match edge_type {
        EdgeType::Owns => 0.15,
        EdgeType::Controls => 0.25,
        EdgeType::HasRole => 0.0, // straight lines for role edges
        EdgeType::Other => 0.10,
    }
}

/// Edge priority for intersection ordering (higher = rendered on top)
pub fn edge_priority(edge_type: EdgeType) -> u32 {
    match edge_type {
        EdgeType::Owns => 100,
        EdgeType::Controls => 90,
        EdgeType::HasRole => 50,
        EdgeType::Other => 30,
    }
}

// =============================================================================
// RENDERING
// =============================================================================

/// Number of segments to use when rendering bezier curves
const BEZIER_SEGMENTS: usize = 20;

/// Render a bezier edge as line segments
pub fn render_bezier_edge(
    painter: &egui::Painter,
    curve: &EdgeCurve,
    stroke: Stroke,
    dashed: bool,
) {
    if dashed {
        render_dashed_bezier(painter, curve, stroke);
    } else {
        let points: Vec<Pos2> = (0..=BEZIER_SEGMENTS)
            .map(|i| {
                let t = i as f32 / BEZIER_SEGMENTS as f32;
                curve.point_at(t)
            })
            .collect();

        painter.add(egui::Shape::line(points, stroke));
    }
}

/// Render a dashed bezier curve
fn render_dashed_bezier(painter: &egui::Painter, curve: &EdgeCurve, stroke: Stroke) {
    let dash_len = 8.0;
    let gap_len = 4.0;
    let total_len = approximate_curve_length(curve);

    let mut dist = 0.0;
    let mut drawing = true;

    while dist < total_len {
        let segment_len = if drawing { dash_len } else { gap_len };
        let next_dist = (dist + segment_len).min(total_len);

        if drawing {
            let t1 = dist / total_len;
            let t2 = next_dist / total_len;
            let p1 = curve.point_at(t1);
            let p2 = curve.point_at(t2);
            painter.line_segment([p1, p2], stroke);
        }

        dist = next_dist;
        drawing = !drawing;
    }
}

/// Approximate the length of a bezier curve
fn approximate_curve_length(curve: &EdgeCurve) -> f32 {
    let mut length = 0.0;
    let mut prev = curve.from;

    for i in 1..=BEZIER_SEGMENTS {
        let t = i as f32 / BEZIER_SEGMENTS as f32;
        let point = curve.point_at(t);
        length += (point - prev).length();
        prev = point;
    }

    length
}

// =============================================================================
// ARROW HEAD
// =============================================================================

/// Arrow head size
const ARROW_SIZE: f32 = 8.0;

/// Render an arrow head at the end of an edge
pub fn render_arrow_head(
    painter: &egui::Painter,
    tip: Pos2,
    direction: Vec2,
    zoom: f32,
    color: Color32,
) {
    let size = ARROW_SIZE * zoom;
    let dir = direction.normalized();
    let perp = Vec2::new(-dir.y, dir.x);

    let p1 = tip;
    let p2 = tip - dir * size + perp * size * 0.5;
    let p3 = tip - dir * size - perp * size * 0.5;

    painter.add(egui::Shape::convex_polygon(
        vec![p1, p2, p3],
        color,
        Stroke::NONE,
    ));
}

// =============================================================================
// EDGE LABELS
// =============================================================================

/// Render a label (e.g., ownership %) at the edge midpoint
pub fn render_edge_label(
    painter: &egui::Painter,
    position: Pos2,
    label: &str,
    zoom: f32,
    bg_color: Color32,
    text_color: Color32,
) {
    let font_size = 9.0 * zoom;
    let padding = Vec2::new(6.0 * zoom, 3.0 * zoom);

    // Measure text for background pill
    let galley = painter.layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(font_size),
        text_color,
    );

    let text_size = galley.size();
    let pill_size = text_size + padding * 2.0;
    let pill_rect = egui::Rect::from_center_size(position, pill_size);

    // Background pill
    painter.rect_filled(pill_rect, pill_size.y / 2.0, bg_color);
    painter.rect_stroke(
        pill_rect,
        pill_size.y / 2.0,
        Stroke::new(0.5 * zoom, Color32::from_rgb(200, 200, 200)),
    );

    // Text
    painter.galley(
        Pos2::new(
            position.x - text_size.x / 2.0,
            position.y - text_size.y / 2.0,
        ),
        galley,
        text_color,
    );
}

/// Check if edge label should be shown based on zoom level
pub fn should_show_edge_label(has_label: bool, zoom: f32) -> bool {
    has_label && zoom > 0.6
}

// =============================================================================
// EDGE INTERSECTION (for hop-over rendering)
// =============================================================================

/// Find intersection point between two bezier curves (simplified)
pub fn find_edge_intersection(curve1: &EdgeCurve, curve2: &EdgeCurve) -> Option<(Pos2, f32, f32)> {
    // Simplified: check line segment intersections along the curves
    const CHECK_SEGMENTS: usize = 10;

    for i in 0..CHECK_SEGMENTS {
        let t1_start = i as f32 / CHECK_SEGMENTS as f32;
        let t1_end = (i + 1) as f32 / CHECK_SEGMENTS as f32;
        let p1_start = curve1.point_at(t1_start);
        let p1_end = curve1.point_at(t1_end);

        for j in 0..CHECK_SEGMENTS {
            let t2_start = j as f32 / CHECK_SEGMENTS as f32;
            let t2_end = (j + 1) as f32 / CHECK_SEGMENTS as f32;
            let p2_start = curve2.point_at(t2_start);
            let p2_end = curve2.point_at(t2_end);

            if let Some((t1, t2)) = line_segment_intersection(p1_start, p1_end, p2_start, p2_end) {
                let actual_t1 = t1_start + t1 * (t1_end - t1_start);
                let actual_t2 = t2_start + t2 * (t2_end - t2_start);
                let intersection = curve1.point_at(actual_t1);
                return Some((intersection, actual_t1, actual_t2));
            }
        }
    }

    None
}

/// Find intersection point of two line segments
fn line_segment_intersection(p1: Pos2, p2: Pos2, p3: Pos2, p4: Pos2) -> Option<(f32, f32)> {
    let d1 = p2 - p1;
    let d2 = p4 - p3;
    let d3 = p1 - p3;

    let cross = d1.x * d2.y - d1.y * d2.x;

    if cross.abs() < 1e-10 {
        return None; // Parallel
    }

    let t = (d3.x * d2.y - d3.y * d2.x) / cross;
    let u = (d3.x * d1.y - d3.y * d1.x) / cross;

    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some((t, u))
    } else {
        None
    }
}

/// Render a hop (arc) over an intersection point
pub fn render_hop(
    painter: &egui::Painter,
    intersection: Pos2,
    direction: Vec2,
    zoom: f32,
    color: Color32,
) {
    let hop_radius = 6.0 * zoom;
    let perp = Vec2::new(-direction.y, direction.x).normalized();

    // Draw a small arc
    let arc_start = intersection - direction * hop_radius;
    let arc_end = intersection + direction * hop_radius;
    let arc_control = intersection + perp * hop_radius * 1.5;

    let points: Vec<Pos2> = (0..=10)
        .map(|i| {
            let t = i as f32 / 10.0;
            let mt = 1.0 - t;
            Pos2::new(
                arc_start.x * mt * mt + arc_control.x * 2.0 * mt * t + arc_end.x * t * t,
                arc_start.y * mt * mt + arc_control.y * 2.0 * mt * t + arc_end.y * t * t,
            )
        })
        .collect();

    painter.add(egui::Shape::line(points, Stroke::new(1.5 * zoom, color)));
}
