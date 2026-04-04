//! Cluster-level renderer — force within fixed boundary.
//!
//! Same Fruchterman-Reingold force algorithm as Universe, with an additional
//! boundary constraint: nodes that exit radius 400 are projected back.
//! CBU nodes (radius 18), entity nodes (radius 8).
//! Boundary circle drawn with dashed stroke.

use egui::{Color32, Painter, Pos2, Stroke, Vec2};

use ob_poc_types::graph_scene::{GraphSceneModel, SceneEdge, SceneNode, SceneNodeType};

use crate::state::CanvasApp;

const BOUNDARY_RADIUS: f32 = 400.0;

/// Paint Cluster-level: CBU nodes within a constrained boundary.
pub fn paint(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    scene: &GraphSceneModel,
    app: &CanvasApp,
) {
    let nodes = &scene.nodes;
    let edges = &scene.edges;

    // ── Boundary circle (dashed) ──
    paint_dashed_boundary(painter, transform);

    if nodes.is_empty() {
        return;
    }

    // ── Compute bounded force-directed positions ──
    let positions = bounded_fruchterman_reingold(nodes, edges);

    // ── Paint edges ──
    for edge in edges {
        paint_edge(painter, transform, edge, nodes, &positions);
    }

    // ── Paint nodes ──
    for (i, node) in nodes.iter().enumerate() {
        let (x, y) = positions[i];
        let screen_pos = transform.transform_pos(Pos2::new(x, y));
        let is_selected = app.interaction.selected_node.as_deref() == Some(&node.id);
        let is_hovered = app.interaction.hovered_node.as_deref() == Some(&node.id);

        paint_node(painter, screen_pos, node, is_selected, is_hovered);
    }
}

// ── Dashed boundary circle ───────────────────────────────────

fn paint_dashed_boundary(painter: &Painter, transform: &egui::emath::RectTransform) {
    let center = transform.transform_pos(Pos2::ZERO);
    let edge_point = transform.transform_pos(Pos2::new(BOUNDARY_RADIUS, 0.0));
    let screen_radius = (edge_point.x - center.x).abs();

    // Draw dashed circle as short arc segments
    let dash_count = 48;
    let dash_fraction = 0.6; // fraction of each segment that is drawn
    let color = Color32::from_rgba_premultiplied(71, 85, 105, 60);

    for i in 0..dash_count {
        let angle_start =
            (i as f32 / dash_count as f32) * std::f32::consts::TAU;
        let angle_end = angle_start
            + (dash_fraction / dash_count as f32) * std::f32::consts::TAU;

        let p1 = Pos2::new(
            center.x + angle_start.cos() * screen_radius,
            center.y + angle_start.sin() * screen_radius,
        );
        let p2 = Pos2::new(
            center.x + angle_end.cos() * screen_radius,
            center.y + angle_end.sin() * screen_radius,
        );

        painter.line_segment([p1, p2], Stroke::new(1.0, color));
    }
}

// ── Bounded Fruchterman-Reingold force simulation ────────────

fn bounded_fruchterman_reingold(
    nodes: &[SceneNode],
    edges: &[SceneEdge],
) -> Vec<(f32, f32)> {
    let n = nodes.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![(0.0, 0.0)];
    }

    let area = (BOUNDARY_RADIUS * 2.0).powi(2);
    let k = (area / n as f32).sqrt();

    // Initialize positions deterministically (golden angle spiral)
    let mut pos: Vec<(f32, f32)> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            if let Some(hint) = node.position_hint {
                hint
            } else {
                let angle = i as f32 * 2.399;
                let r = 30.0 * (i as f32).sqrt();
                (angle.cos() * r, angle.sin() * r)
            }
        })
        .collect();

    // Build edge index
    let edge_pairs: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|e| {
            let si = nodes.iter().position(|n| n.id == e.source)?;
            let ti = nodes.iter().position(|n| n.id == e.target)?;
            Some((si, ti))
        })
        .collect();

    let damping = 0.9f32;
    let iterations = 150;
    let mut temperature = 150.0f32;
    let cooling = temperature / iterations as f32;

    for _ in 0..iterations {
        let mut disp: Vec<(f32, f32)> = vec![(0.0, 0.0); n];

        // Repulsive forces
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = pos[i].0 - pos[j].0;
                let dy = pos[i].1 - pos[j].1;
                let dist = (dx * dx + dy * dy).sqrt().max(0.01);
                let force = (k * k) / dist;
                let fx = (dx / dist) * force;
                let fy = (dy / dist) * force;
                disp[i].0 += fx;
                disp[i].1 += fy;
                disp[j].0 -= fx;
                disp[j].1 -= fy;
            }
        }

        // Attractive forces
        for &(si, ti) in &edge_pairs {
            let dx = pos[si].0 - pos[ti].0;
            let dy = pos[si].1 - pos[ti].1;
            let dist = (dx * dx + dy * dy).sqrt().max(0.01);
            let force = (dist * dist) / k;
            let fx = (dx / dist) * force;
            let fy = (dy / dist) * force;
            disp[si].0 -= fx;
            disp[si].1 -= fy;
            disp[ti].0 += fx;
            disp[ti].1 += fy;
        }

        // Apply displacement with temperature clamping
        for i in 0..n {
            let dx = disp[i].0;
            let dy = disp[i].1;
            let mag = (dx * dx + dy * dy).sqrt().max(0.01);
            let clamped = mag.min(temperature);
            pos[i].0 += (dx / mag) * clamped * damping;
            pos[i].1 += (dy / mag) * clamped * damping;

            // ── Boundary constraint: project back inside radius ──
            let dist_from_center =
                (pos[i].0 * pos[i].0 + pos[i].1 * pos[i].1).sqrt();
            if dist_from_center > BOUNDARY_RADIUS {
                let scale = BOUNDARY_RADIUS / dist_from_center;
                pos[i].0 *= scale;
                pos[i].1 *= scale;
            }
        }

        temperature -= cooling;
        if temperature < 0.1 {
            break;
        }
    }

    pos
}

// ── Node painting ────────────────────────────────────────────

fn paint_node(
    painter: &Painter,
    screen_pos: Pos2,
    node: &SceneNode,
    selected: bool,
    hovered: bool,
) {
    let base_radius = match node.node_type {
        SceneNodeType::Cbu => 18.0,
        SceneNodeType::Entity => 8.0,
        _ => 12.0,
    };

    let radius = if hovered {
        base_radius * 1.15
    } else {
        base_radius
    };

    let fill_color = node_color(node);
    painter.circle_filled(screen_pos, radius, fill_color);

    // Selection ring
    if selected {
        painter.circle_stroke(
            screen_pos,
            radius + 3.0,
            Stroke::new(2.0, Color32::from_rgb(59, 130, 246)),
        );
    }

    // Hover ring
    if hovered && !selected {
        painter.circle_stroke(
            screen_pos,
            radius + 2.0,
            Stroke::new(1.5, Color32::from_rgb(148, 163, 184)),
        );
    }

    // Progress arc indicator
    if node.progress > 0 && node.progress < 100 {
        painter.circle_stroke(
            screen_pos,
            radius + 2.0,
            Stroke::new(2.0, Color32::from_rgb(34, 197, 94)),
        );
    }

    // Blocking indicator
    if node.blocking {
        painter.circle_stroke(
            screen_pos,
            radius + 2.0,
            Stroke::new(2.0, Color32::from_rgb(239, 68, 68)),
        );
    }

    // Label (always for CBU, on hover for entities)
    if matches!(node.node_type, SceneNodeType::Cbu) || hovered {
        painter.text(
            screen_pos + Vec2::new(0.0, radius + 6.0),
            egui::Align2::CENTER_TOP,
            &node.label,
            egui::FontId::proportional(10.0),
            Color32::from_rgb(203, 213, 225),
        );
    }
}

// ── Edge painting ────────────────────────────────────────────

fn paint_edge(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    edge: &SceneEdge,
    nodes: &[SceneNode],
    positions: &[(f32, f32)],
) {
    let src_idx = nodes.iter().position(|n| n.id == edge.source);
    let tgt_idx = nodes.iter().position(|n| n.id == edge.target);

    let (Some(si), Some(ti)) = (src_idx, tgt_idx) else {
        return;
    };

    let src_pos = transform.transform_pos(Pos2::new(positions[si].0, positions[si].1));
    let tgt_pos = transform.transform_pos(Pos2::new(positions[ti].0, positions[ti].1));

    let edge_color = match edge.edge_type {
        ob_poc_types::graph_scene::SceneEdgeType::Dependency => {
            Color32::from_rgb(245, 158, 11)
        }
        ob_poc_types::graph_scene::SceneEdgeType::Ownership => {
            Color32::from_rgb(139, 92, 246)
        }
        ob_poc_types::graph_scene::SceneEdgeType::Control => {
            Color32::from_rgb(59, 130, 246)
        }
        ob_poc_types::graph_scene::SceneEdgeType::SharedEntity => {
            Color32::from_rgba_premultiplied(100, 116, 139, 80)
        }
        _ => Color32::from_rgb(71, 85, 105),
    };

    let stroke_width = (edge.weight * 1.5).clamp(0.5, 3.0);
    painter.line_segment([src_pos, tgt_pos], Stroke::new(stroke_width, edge_color));

    // Edge label at midpoint
    if let Some(ref label) = edge.label {
        let mid = Pos2::new(
            (src_pos.x + tgt_pos.x) / 2.0,
            (src_pos.y + tgt_pos.y) / 2.0,
        );
        painter.text(
            mid,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(9.0),
            Color32::from_rgb(148, 163, 184),
        );
    }
}

// ── Color utility ────────────────────────────────────────────

fn node_color(node: &SceneNode) -> Color32 {
    match node.state.as_deref() {
        Some("complete") => Color32::from_rgb(34, 197, 94),
        Some("filled") => Color32::from_rgb(59, 130, 246),
        Some("blocked") => Color32::from_rgb(239, 68, 68),
        Some("empty") => Color32::from_rgb(71, 85, 105),
        _ => match node.node_type {
            SceneNodeType::Cbu => Color32::from_rgb(139, 92, 246),
            SceneNodeType::Entity => Color32::from_rgb(59, 130, 246),
            SceneNodeType::Case => Color32::from_rgb(245, 158, 11),
            _ => Color32::from_rgb(100, 116, 139),
        },
    }
}
