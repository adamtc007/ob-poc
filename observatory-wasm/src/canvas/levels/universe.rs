//! Universe-level renderer — force-directed cluster layout.
//!
//! Fruchterman-Reingold force simulation: repulsion (Coulomb), attraction (spring),
//! damping (0.9), 150 iterations. Node radius = 20 + sqrt(child_count) * 8.
//! Deterministic HSV coloring from node ID hash. Computed positions written
//! so hit_test works via position_hint fallback.

use egui::{Color32, Painter, Pos2, Stroke, Vec2};

use ob_poc_types::graph_scene::{GraphSceneModel, SceneEdge, SceneNode, SceneNodeType};

use crate::state::CanvasApp;

/// Paint Universe-level: cluster bubbles with force-directed layout.
pub fn paint(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    scene: &GraphSceneModel,
    app: &CanvasApp,
) {
    let nodes = &scene.nodes;
    let edges = &scene.edges;

    if nodes.is_empty() {
        return;
    }

    // ── Compute force-directed positions ──
    let positions = fruchterman_reingold(nodes, edges);

    // ── Paint group boundaries (if any) ──
    for group in &scene.groups {
        if group.node_ids.is_empty() {
            continue;
        }
        // Compute bounding circle for group members
        let member_positions: Vec<(f32, f32)> = group
            .node_ids
            .iter()
            .filter_map(|nid| {
                nodes
                    .iter()
                    .position(|n| n.id == *nid)
                    .map(|idx| positions[idx])
            })
            .collect();
        if member_positions.is_empty() {
            continue;
        }
        let cx: f32 = member_positions.iter().map(|p| p.0).sum::<f32>()
            / member_positions.len() as f32;
        let cy: f32 = member_positions.iter().map(|p| p.1).sum::<f32>()
            / member_positions.len() as f32;
        let max_dist = member_positions
            .iter()
            .map(|p| ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt())
            .fold(0.0f32, f32::max);
        let group_radius = max_dist + 60.0;
        let screen_center = transform.transform_pos(Pos2::new(cx, cy));
        let screen_edge = transform.transform_pos(Pos2::new(cx + group_radius, cy));
        let screen_r = (screen_edge.x - screen_center.x).abs();
        painter.circle_stroke(
            screen_center,
            screen_r,
            Stroke::new(
                1.0,
                Color32::from_rgba_premultiplied(100, 116, 139, 30),
            ),
        );
        // Group label
        painter.text(
            screen_center + Vec2::new(0.0, -screen_r - 8.0),
            egui::Align2::CENTER_BOTTOM,
            &group.label,
            egui::FontId::proportional(10.0),
            Color32::from_rgb(148, 163, 184),
        );
    }

    // ── Paint edges (below nodes) ──
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

// ── Fruchterman-Reingold force simulation ────────────────────

fn fruchterman_reingold(nodes: &[SceneNode], edges: &[SceneEdge]) -> Vec<(f32, f32)> {
    let n = nodes.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![(0.0, 0.0)];
    }

    // Area and optimal distance
    let area = 800.0 * 800.0;
    let k = (area / n as f32).sqrt(); // optimal distance

    // Initialize positions deterministically from node index (spiral)
    let mut pos: Vec<(f32, f32)> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            if let Some(hint) = node.position_hint {
                hint
            } else {
                let angle = i as f32 * 2.399; // golden angle
                let r = 50.0 * (i as f32).sqrt();
                (angle.cos() * r, angle.sin() * r)
            }
        })
        .collect();

    let damping = 0.9f32;
    let iterations = 150;
    let mut temperature = 200.0f32;
    let cooling = temperature / iterations as f32;

    // Build edge index for O(1) lookup
    let edge_pairs: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|e| {
            let si = nodes.iter().position(|n| n.id == e.source)?;
            let ti = nodes.iter().position(|n| n.id == e.target)?;
            Some((si, ti))
        })
        .collect();

    for _ in 0..iterations {
        // Displacement accumulator
        let mut disp: Vec<(f32, f32)> = vec![(0.0, 0.0); n];

        // Repulsive forces (Coulomb) between all pairs
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = pos[i].0 - pos[j].0;
                let dy = pos[i].1 - pos[j].1;
                let dist = (dx * dx + dy * dy).sqrt().max(0.01);
                let force = (k * k) / dist; // repulsion
                let fx = (dx / dist) * force;
                let fy = (dy / dist) * force;
                disp[i].0 += fx;
                disp[i].1 += fy;
                disp[j].0 -= fx;
                disp[j].1 -= fy;
            }
        }

        // Attractive forces (spring) along edges
        for &(si, ti) in &edge_pairs {
            let dx = pos[si].0 - pos[ti].0;
            let dy = pos[si].1 - pos[ti].1;
            let dist = (dx * dx + dy * dy).sqrt().max(0.01);
            let force = (dist * dist) / k; // attraction
            let fx = (dx / dist) * force;
            let fy = (dy / dist) * force;
            disp[si].0 -= fx;
            disp[si].1 -= fy;
            disp[ti].0 += fx;
            disp[ti].1 += fy;
        }

        // Apply displacement with temperature clamping and damping
        for i in 0..n {
            let dx = disp[i].0;
            let dy = disp[i].1;
            let mag = (dx * dx + dy * dy).sqrt().max(0.01);
            let clamped = mag.min(temperature);
            pos[i].0 += (dx / mag) * clamped * damping;
            pos[i].1 += (dy / mag) * clamped * damping;
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
        SceneNodeType::Cluster | SceneNodeType::Aggregate => {
            20.0 + (node.child_count as f32).sqrt() * 8.0
        }
        _ => 12.0,
    };

    let radius = if hovered {
        base_radius * 1.1
    } else {
        base_radius
    };

    // Deterministic color from node ID hash (HSV)
    let fill = color_from_id(&node.id, node.depth);
    painter.circle_filled(screen_pos, radius, fill);

    // Selection ring
    if selected {
        painter.circle_stroke(
            screen_pos,
            radius + 3.0,
            Stroke::new(2.0, Color32::from_rgb(59, 130, 246)),
        );
    }

    // Hover glow
    if hovered && !selected {
        painter.circle_stroke(
            screen_pos,
            radius + 2.0,
            Stroke::new(1.5, Color32::from_rgb(148, 163, 184)),
        );
    }

    // Child count badge (centered on node)
    if node.child_count > 0 {
        painter.text(
            screen_pos,
            egui::Align2::CENTER_CENTER,
            format!("{}", node.child_count),
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );
    }

    // Label below
    painter.text(
        screen_pos + Vec2::new(0.0, radius + 8.0),
        egui::Align2::CENTER_TOP,
        &node.label,
        egui::FontId::proportional(11.0),
        Color32::from_rgb(203, 213, 225),
    );
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

    // Thin cross-cluster edges
    let alpha = ((edge.weight * 80.0) as u8).max(40).min(120);
    painter.line_segment(
        [src_pos, tgt_pos],
        Stroke::new(
            (edge.weight * 0.5).clamp(0.5, 2.0),
            Color32::from_rgba_premultiplied(100, 116, 139, alpha),
        ),
    );

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

// ── Color utilities ──────────────────────────────────────────

fn color_from_id(id: &str, depth: usize) -> Color32 {
    let hash = id
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let hue = (hash % 360) as f32;
    let saturation = 0.5 + (depth as f32 * 0.1).min(0.3);
    hsv_to_rgb(hue, saturation, 0.6)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color32 {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}
