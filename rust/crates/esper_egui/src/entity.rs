//! Entity rendering.
//!
//! Renders entities from a ChamberSnapshot with LOD-based detail levels.

use crate::{config::EntityRenderConfig, painter::EsperPainter};
use egui::Vec2;
use esper_snapshot::ChamberSnapshot;

/// Entity renderer with LOD support.
#[derive(Debug, Clone)]
pub struct EntityRenderer {
    /// Entity rendering configuration.
    config: EntityRenderConfig,
}

impl Default for EntityRenderer {
    fn default() -> Self {
        Self::new(EntityRenderConfig::default())
    }
}

impl EntityRenderer {
    /// Create a new entity renderer.
    pub fn new(config: EntityRenderConfig) -> Self {
        Self { config }
    }

    /// Render all visible entities in a chamber.
    pub fn render(
        &self,
        painter: &EsperPainter<'_>,
        chamber: &ChamberSnapshot,
        selection: Option<u32>,
        hover: Option<u32>,
        focus: Option<u32>,
    ) {
        let camera = painter.camera();
        let visible_rect = camera.visible_world_rect();

        // Iterate through entities and render visible ones
        for idx in 0..chamber.entity_ids.len() {
            let x = chamber.x[idx];
            let y = chamber.y[idx];
            let pos = Vec2::new(x, y);

            // Simple visibility check
            let entity_rect = egui::Rect::from_center_size(
                egui::Pos2::new(x, y),
                egui::vec2(50.0, 50.0), // Rough bounds
            );
            if !visible_rect.intersects(entity_rect) {
                continue;
            }

            let idx_u32 = idx as u32;
            let is_selected = selection == Some(idx_u32);
            let is_hovered = hover == Some(idx_u32);
            let is_focused = focus == Some(idx_u32);

            // Default radius (chamber doesn't store radii - computed elsewhere)
            let base_radius = 10.0;

            // Calculate screen size for LOD
            let screen_radius = camera.scale_to_screen(base_radius);
            let lod = self.calculate_lod(screen_radius);

            // Render based on LOD
            match lod {
                EntityLOD::Dot => {
                    self.render_dot(painter, pos, is_selected, is_hovered, is_focused);
                }
                EntityLOD::Simple => {
                    self.render_simple(
                        painter,
                        pos,
                        base_radius,
                        is_selected,
                        is_hovered,
                        is_focused,
                    );
                }
                EntityLOD::Normal | EntityLOD::Detailed => {
                    // For Normal and Detailed, we'd need to look up labels from string table
                    // For now, just render as simple
                    self.render_simple(
                        painter,
                        pos,
                        base_radius,
                        is_selected,
                        is_hovered,
                        is_focused,
                    );
                }
            }
        }
    }

    /// Calculate LOD level based on screen size.
    fn calculate_lod(&self, screen_radius: f32) -> EntityLOD {
        if screen_radius < self.config.min_size {
            EntityLOD::Dot
        } else if screen_radius < self.config.label_threshold {
            EntityLOD::Simple
        } else if screen_radius < self.config.detail_threshold {
            EntityLOD::Normal
        } else {
            EntityLOD::Detailed
        }
    }

    /// Render entity as a simple dot (very zoomed out).
    fn render_dot(
        &self,
        painter: &EsperPainter<'_>,
        pos: Vec2,
        selected: bool,
        hovered: bool,
        focused: bool,
    ) {
        let style = painter.style();
        let fill = if focused {
            style.entity.fill_focused
        } else if selected {
            style.entity.fill_selected
        } else if hovered {
            style.entity.fill_hovered
        } else {
            style.entity.fill
        };

        // Draw as small fixed-size circle (in screen space)
        let screen_pos = painter.world_to_screen(pos);
        painter.egui_painter().circle_filled(screen_pos, 2.0, fill);
    }

    /// Render entity with simple shape (no label).
    fn render_simple(
        &self,
        painter: &EsperPainter<'_>,
        pos: Vec2,
        radius: f32,
        selected: bool,
        hovered: bool,
        focused: bool,
    ) {
        painter.entity_node(pos, radius, selected, hovered, focused);
    }

    /// Render edges between entities using first_child/next_sibling structure.
    pub fn render_edges(
        &self,
        painter: &EsperPainter<'_>,
        chamber: &ChamberSnapshot,
        highlighted_entity: Option<u32>,
    ) {
        // The chamber uses first_child/next_sibling for tree structure
        // We need to traverse and draw parent-child edges
        for (idx, &first_child) in chamber.first_child.iter().enumerate() {
            if first_child == esper_snapshot::NONE_IDX {
                continue;
            }

            let parent_x = chamber.x[idx];
            let parent_y = chamber.y[idx];

            // Draw edge from parent to first child
            let child_idx = first_child as usize;
            if child_idx < chamber.x.len() {
                let child_x = chamber.x[child_idx];
                let child_y = chamber.y[child_idx];

                let is_highlighted = highlighted_entity == Some(idx as u32)
                    || highlighted_entity == Some(first_child);

                painter.entity_edge(
                    Vec2::new(parent_x, parent_y),
                    Vec2::new(child_x, child_y),
                    is_highlighted,
                    false,
                );
            }

            // Follow next_sibling chain - draw edges from parent to all siblings
            let mut sibling_idx = first_child as usize;
            while sibling_idx < chamber.next_sibling.len() {
                let next = chamber.next_sibling[sibling_idx];
                if next == esper_snapshot::NONE_IDX {
                    break;
                }

                let next_x = chamber.x[next as usize];
                let next_y = chamber.y[next as usize];

                // Draw edge from parent to next sibling
                painter.entity_edge(
                    Vec2::new(parent_x, parent_y),
                    Vec2::new(next_x, next_y),
                    highlighted_entity == Some(next),
                    false,
                );

                sibling_idx = next as usize;
            }
        }
    }
}

/// Level of detail for entity rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityLOD {
    /// Render as single pixel/dot.
    Dot,
    /// Render as simple shape without label.
    Simple,
    /// Render with shape and label.
    Normal,
    /// Render with full details (icon, label, kind).
    Detailed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_calculation() {
        let renderer = EntityRenderer::default();

        assert_eq!(renderer.calculate_lod(2.0), EntityLOD::Dot);
        assert_eq!(renderer.calculate_lod(10.0), EntityLOD::Simple);
        assert_eq!(renderer.calculate_lod(25.0), EntityLOD::Normal);
        assert_eq!(renderer.calculate_lod(50.0), EntityLOD::Detailed);
    }

    #[test]
    fn entity_renderer_default() {
        let renderer = EntityRenderer::default();
        assert!(renderer.config.show_labels);
        assert!(renderer.config.show_icons);
    }
}
