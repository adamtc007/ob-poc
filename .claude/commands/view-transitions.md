# View Transitions & Density Rules Implementation

Implement density-based view mode switching and smooth layout transitions.

## Files to Create

1. `rust/crates/ob-poc-ui/src/view/mod.rs`
2. `rust/crates/ob-poc-ui/src/view/density.rs`
3. `rust/crates/ob-poc-ui/src/view/transition.rs`

## Density Rules

### density.rs
```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct DensityRule {
    pub threshold: DensityThreshold,
    pub mode: ViewMode,
    pub node_rendering: NodeRenderMode,
    #[serde(default)]
    pub expand_taxonomy: Option<String>,
    #[serde(default)]
    pub cluster_by: Option<String>,
    #[serde(default)]
    pub show_floating_persons: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum DensityThreshold {
    GreaterThan {
        gt: u32,
        entity_type: String,
    },
    LessThan {
        lt: u32,
        entity_type: String,
    },
    Range {
        min: u32,
        max: u32,
        entity_type: String,
    },
    Single,
}

impl DensityThreshold {
    pub fn matches(&self, count: u32, entity_type: &str) -> bool {
        match self {
            Self::GreaterThan { gt, entity_type: et } => {
                entity_type == et && count > *gt
            }
            Self::LessThan { lt, entity_type: et } => {
                entity_type == et && count < *lt
            }
            Self::Range { min, max, entity_type: et } => {
                entity_type == et && count >= *min && count <= *max
            }
            Self::Single => count == 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    AstroOverview,
    AstroClustered,
    HybridDrilldown,
    MultiCbuDetail,
    SingleCbuPyramid,
    FullDetail,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeRenderMode {
    CompactDot,
    LabeledCircle,
    ExpandedTaxonomy,
    FullTaxonomyPyramid,
}

/// Computed visibility information
#[derive(Debug, Clone, Default)]
pub struct VisibleEntities {
    pub cbus: Vec<EntityId>,
    pub persons: Vec<EntityId>,
    pub other: Vec<EntityId>,
    pub total_count: u32,
    pub density: f32,
}

impl VisibleEntities {
    pub fn compute(
        nodes: &[PositionedNode],
        viewport: Rect,
        zoom: f32,
        min_visible_size: f32,
    ) -> Self {
        let mut cbus = vec![];
        let mut persons = vec![];
        let mut other = vec![];
        
        for node in nodes {
            // Check if node is in viewport
            let node_rect = Rect::from_center_size(node.position, node.size);
            if !viewport.intersects(node_rect) {
                continue;
            }
            
            // Check if node is large enough to be "visible"
            let screen_size = node.size * zoom;
            if screen_size.x < min_visible_size && screen_size.y < min_visible_size {
                continue;
            }
            
            match node.entity_type.to_uppercase().as_str() {
                "CBU" => cbus.push(node.id),
                "PERSON" => persons.push(node.id),
                _ => other.push(node.id),
            }
        }
        
        let total_count = (cbus.len() + persons.len() + other.len()) as u32;
        let density = if viewport.area() > 0.0 {
            total_count as f32 / viewport.area()
        } else {
            0.0
        };
        
        Self {
            cbus,
            persons,
            other,
            total_count,
            density,
        }
    }
    
    pub fn cbu_count(&self) -> u32 {
        self.cbus.len() as u32
    }
}

/// Evaluate density rules to determine view mode
pub fn evaluate_density_rules(
    visible: &VisibleEntities,
    rules: &[DensityRule],
) -> Option<&DensityRule> {
    for rule in rules {
        let matches = match &rule.threshold {
            DensityThreshold::GreaterThan { gt, entity_type } => {
                let count = get_count_for_type(visible, entity_type);
                count > *gt
            }
            DensityThreshold::LessThan { lt, entity_type } => {
                let count = get_count_for_type(visible, entity_type);
                count < *lt
            }
            DensityThreshold::Range { min, max, entity_type } => {
                let count = get_count_for_type(visible, entity_type);
                count >= *min && count <= *max
            }
            DensityThreshold::Single => visible.total_count == 1,
        };
        
        if matches {
            return Some(rule);
        }
    }
    
    None
}

fn get_count_for_type(visible: &VisibleEntities, entity_type: &str) -> u32 {
    match entity_type.to_lowercase().as_str() {
        "visible_cbu" | "cbu" => visible.cbus.len() as u32,
        "visible_person" | "person" => visible.persons.len() as u32,
        "total" | "all" => visible.total_count,
        _ => visible.total_count,
    }
}
```

## View Transitions

### transition.rs
```rust
use std::time::{Duration, Instant};
use egui::{Pos2, Vec2, Color32};

#[derive(Debug, Clone)]
pub struct ViewTransition {
    pub from_layout: PositionedGraph,
    pub to_layout: PositionedGraph,
    pub from_mode: ViewMode,
    pub to_mode: ViewMode,
    pub progress: f32,
    pub started_at: Instant,
    pub duration: Duration,
}

impl ViewTransition {
    pub fn new(
        from_layout: PositionedGraph,
        to_layout: PositionedGraph,
        from_mode: ViewMode,
        to_mode: ViewMode,
        duration_ms: u64,
    ) -> Self {
        Self {
            from_layout,
            to_layout,
            from_mode,
            to_mode,
            progress: 0.0,
            started_at: Instant::now(),
            duration: Duration::from_millis(duration_ms),
        }
    }
    
    pub fn is_complete(&self) -> bool {
        self.progress >= 1.0
    }
    
    pub fn update(&mut self, dt: f32) {
        let duration_secs = self.duration.as_secs_f32();
        self.progress = (self.progress + dt / duration_secs).min(1.0);
    }
    
    pub fn current_layout(&self) -> PositionedGraph {
        if self.progress <= 0.0 {
            return self.from_layout.clone();
        }
        if self.progress >= 1.0 {
            return self.to_layout.clone();
        }
        
        let t = ease_out_cubic(self.progress);
        interpolate_layouts(&self.from_layout, &self.to_layout, t)
    }
}

/// Interpolate between two layouts
pub fn interpolate_layouts(
    from: &PositionedGraph,
    to: &PositionedGraph,
    t: f32,
) -> PositionedGraph {
    use std::collections::HashMap;
    
    // Build lookup maps
    let from_positions: HashMap<EntityId, &PositionedNode> = from.nodes
        .iter()
        .map(|n| (n.id, n))
        .collect();
    
    let to_positions: HashMap<EntityId, &PositionedNode> = to.nodes
        .iter()
        .map(|n| (n.id, n))
        .collect();
    
    let mut result_nodes = vec![];
    
    // Interpolate nodes that exist in both
    for from_node in &from.nodes {
        if let Some(to_node) = to_positions.get(&from_node.id) {
            // Node exists in both - interpolate
            result_nodes.push(PositionedNode {
                id: from_node.id,
                name: from_node.name.clone(),
                entity_type: from_node.entity_type.clone(),
                position: lerp_pos(from_node.position, to_node.position, t),
                size: lerp_vec(from_node.size, to_node.size, t),
                level: if t < 0.5 { from_node.level } else { to_node.level },
                ring: if t < 0.5 { from_node.ring.clone() } else { to_node.ring.clone() },
                style: interpolate_style(&from_node.style, &to_node.style, t),
                is_floating: if t < 0.5 { from_node.is_floating } else { to_node.is_floating },
                can_drill_down: to_node.can_drill_down,
                alpha: 1.0,
            });
        } else {
            // Node only in 'from' - fade out
            result_nodes.push(PositionedNode {
                size: lerp_vec(from_node.size, Vec2::ZERO, t),
                alpha: 1.0 - t,
                ..from_node.clone()
            });
        }
    }
    
    // Add nodes that only exist in 'to' (fade in)
    for to_node in &to.nodes {
        if !from_positions.contains_key(&to_node.id) {
            result_nodes.push(PositionedNode {
                size: lerp_vec(Vec2::ZERO, to_node.size, t),
                alpha: t,
                ..to_node.clone()
            });
        }
    }
    
    // Interpolate edges (simplified - just use 'to' edges with alpha)
    let edges = to.edges.iter().map(|e| {
        PositionedEdge {
            style: EdgeStyle {
                color: Color32::from_rgba_unmultiplied(
                    e.style.color.r(),
                    e.style.color.g(),
                    e.style.color.b(),
                    (e.style.color.a() as f32 * t) as u8,
                ),
                ..e.style.clone()
            },
            ..e.clone()
        }
    }).collect();
    
    PositionedGraph {
        nodes: result_nodes,
        edges,
        floating_zone: if t < 0.5 { 
            from.floating_zone.clone() 
        } else { 
            to.floating_zone.clone() 
        },
        bounds: lerp_rect(from.bounds, to.bounds, t),
        ..Default::default()
    }
}

/// Easing function: ease-out cubic
pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Easing function: ease-in-out cubic
pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

fn lerp_pos(from: Pos2, to: Pos2, t: f32) -> Pos2 {
    Pos2 {
        x: from.x + (to.x - from.x) * t,
        y: from.y + (to.y - from.y) * t,
    }
}

fn lerp_vec(from: Vec2, to: Vec2, t: f32) -> Vec2 {
    Vec2 {
        x: from.x + (to.x - from.x) * t,
        y: from.y + (to.y - from.y) * t,
    }
}

fn lerp_rect(from: Rect, to: Rect, t: f32) -> Rect {
    Rect::from_min_max(
        lerp_pos(from.min, to.min, t),
        lerp_pos(from.max, to.max, t),
    )
}

fn lerp_color(from: Color32, to: Color32, t: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (from.r() as f32 + (to.r() as f32 - from.r() as f32) * t) as u8,
        (from.g() as f32 + (to.g() as f32 - from.g() as f32) * t) as u8,
        (from.b() as f32 + (to.b() as f32 - from.b() as f32) * t) as u8,
        (from.a() as f32 + (to.a() as f32 - from.a() as f32) * t) as u8,
    )
}

fn interpolate_style(from: &NodeStyle, to: &NodeStyle, t: f32) -> NodeStyle {
    NodeStyle {
        fill_color: lerp_color(from.fill_color, to.fill_color, t),
        stroke_color: lerp_color(from.stroke_color, to.stroke_color, t),
        stroke_width: from.stroke_width + (to.stroke_width - from.stroke_width) * t,
        shape: if t < 0.5 { from.shape } else { to.shape },
        font_size: from.font_size + (to.font_size - from.font_size) * t,
    }
}
```

## View State Manager

### state.rs (update ViewportState)
```rust
use std::time::{Duration, Instant};

pub struct ViewModeController {
    pub current_mode: ViewMode,
    pub pending_transition: Option<ViewTransition>,
    pub density_rules: Vec<DensityRule>,
    
    // Debouncing
    last_mode_change: Instant,
    debounce_duration: Duration,
    pending_mode: Option<ViewMode>,
}

impl ViewModeController {
    pub fn new(initial_mode: ViewMode, rules: Vec<DensityRule>) -> Self {
        Self {
            current_mode: initial_mode,
            pending_transition: None,
            density_rules: rules,
            last_mode_change: Instant::now(),
            debounce_duration: Duration::from_millis(300),
            pending_mode: None,
        }
    }
    
    /// Update with new visibility info - may trigger mode change
    pub fn update(
        &mut self,
        visible: &VisibleEntities,
        current_layout: &PositionedGraph,
        layout_engine: &dyn LayoutEngine,
        graph: &SemanticGraph,
        config: &TaxonomyLayoutConfig,
        viewport: Rect,
        dt: f32,
    ) {
        // Update any in-progress transition
        if let Some(ref mut transition) = self.pending_transition {
            transition.update(dt);
            if transition.is_complete() {
                self.current_mode = transition.to_mode.clone();
                self.pending_transition = None;
            }
            return;  // Don't evaluate rules during transition
        }
        
        // Evaluate density rules
        if let Some(rule) = evaluate_density_rules(visible, &self.density_rules) {
            let target_mode = rule.mode.clone();
            
            if target_mode != self.current_mode {
                // Check debounce
                if self.pending_mode.as_ref() == Some(&target_mode) {
                    // Same pending mode - check if debounce elapsed
                    if self.last_mode_change.elapsed() >= self.debounce_duration {
                        // Trigger transition
                        self.initiate_transition(
                            target_mode,
                            current_layout,
                            layout_engine,
                            graph,
                            config,
                            viewport,
                        );
                    }
                } else {
                    // New pending mode - reset debounce
                    self.pending_mode = Some(target_mode);
                    self.last_mode_change = Instant::now();
                }
            } else {
                // Mode matches - clear pending
                self.pending_mode = None;
            }
        }
    }
    
    fn initiate_transition(
        &mut self,
        target_mode: ViewMode,
        current_layout: &PositionedGraph,
        layout_engine: &dyn LayoutEngine,
        graph: &SemanticGraph,
        config: &TaxonomyLayoutConfig,
        viewport: Rect,
    ) {
        // Compute new layout for target mode
        let new_config = config.with_view_mode(&target_mode);
        let new_layout = layout_engine.layout(graph, &new_config, viewport);
        
        self.pending_transition = Some(ViewTransition::new(
            current_layout.clone(),
            new_layout,
            self.current_mode.clone(),
            target_mode.clone(),
            400,  // 400ms transition
        ));
        
        self.pending_mode = None;
    }
    
    /// Get current layout (may be mid-transition)
    pub fn current_layout(&self, base_layout: &PositionedGraph) -> PositionedGraph {
        if let Some(ref transition) = self.pending_transition {
            transition.current_layout()
        } else {
            base_layout.clone()
        }
    }
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_density_threshold_greater_than() {
        let threshold = DensityThreshold::GreaterThan {
            gt: 20,
            entity_type: "visible_cbu".to_string(),
        };
        
        assert!(!threshold.matches(20, "visible_cbu"));
        assert!(threshold.matches(21, "visible_cbu"));
        assert!(!threshold.matches(21, "person"));
    }
    
    #[test]
    fn test_density_threshold_range() {
        let threshold = DensityThreshold::Range {
            min: 5,
            max: 20,
            entity_type: "visible_cbu".to_string(),
        };
        
        assert!(!threshold.matches(4, "visible_cbu"));
        assert!(threshold.matches(5, "visible_cbu"));
        assert!(threshold.matches(15, "visible_cbu"));
        assert!(threshold.matches(20, "visible_cbu"));
        assert!(!threshold.matches(21, "visible_cbu"));
    }
    
    #[test]
    fn test_interpolate_layouts() {
        let from = PositionedGraph {
            nodes: vec![
                PositionedNode {
                    id: EntityId(1),
                    position: pos2(0.0, 0.0),
                    size: vec2(100.0, 50.0),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        
        let to = PositionedGraph {
            nodes: vec![
                PositionedNode {
                    id: EntityId(1),
                    position: pos2(100.0, 100.0),
                    size: vec2(100.0, 50.0),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        
        let mid = interpolate_layouts(&from, &to, 0.5);
        
        assert_eq!(mid.nodes[0].position.x, 50.0);
        assert_eq!(mid.nodes[0].position.y, 50.0);
    }
    
    #[test]
    fn test_ease_out_cubic() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        
        // Should be > 0.5 at t=0.5 (ease out accelerates)
        assert!(ease_out_cubic(0.5) > 0.5);
    }
}
```

## Acceptance Criteria

- [ ] Density rules correctly match visible entity counts
- [ ] View mode changes trigger after debounce period
- [ ] Layout interpolation produces smooth transitions
- [ ] Nodes fade in/out when appearing/disappearing
- [ ] Easing functions produce natural motion
- [ ] No flip-flopping during active zoom
