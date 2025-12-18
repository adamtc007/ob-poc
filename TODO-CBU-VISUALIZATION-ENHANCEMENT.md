# TODO: CBU Visualization Enhancement

## ⛔ MANDATORY FIRST STEP

**Before writing ANY code, read:**
- `/EGUI-RULES.md` - Non-negotiable UI patterns
- `/docs/CBU_UNIVERSE_VISUALIZATION_SPEC.md` - Existing vision doc
- `/rust/src/graph/layout.rs` - Current layout engine
- `/rust/src/graph/types.rs` - Graph data structures

---

## The Problem

The CBU visualization "works" but doesn't "pop". By design, the DSL/integrated data model achieves STP (straight-through processing) with minimal UI. This means the **graph visualization IS the UI** - it must clearly communicate:

1. What entities are involved in this CBU
2. How they relate (ownership chains, control, roles)
3. What state things are in (KYC status, verification)
4. What needs attention (gaps, issues, pending items)

Current layout is mechanistic - fixed tiers, uniform spacing, no semantic awareness.

---

## Target State

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│                         ┌─────────────────┐                                 │
│                         │   ALLIANZ SE    │ ← Large node (ultimate parent)  │
│                         │   ████████████  │   Filled = fully verified       │
│                         └────────▲────────┘                                 │
│                                  │                                          │
│                            100% ═╪═ ← Thick edge = high ownership           │
│                                  │    Double line = proven                  │
│                         ┌────────┴────────┐                                 │
│                         │  Allianz GI     │ ← Medium node (intermediate)    │
│                         │  GmbH  ▓▓▓▓▓▓   │   Partial fill = partial KYC    │
│                         └────────▲────────┘                                 │
│                                  │                                          │
│              ┌───────────────────┼───────────────────┐                      │
│              │                   │                   │                      │
│         100% ═                  100%                100%                    │
│              │                   │                   │                      │
│      ┌───────┴───────┐   ┌───────┴───────┐   ┌───────┴───────┐             │
│      │   Fund A      │   │   Fund B      │   │   Fund C      │             │
│      │   ○○○○○○○○    │   │   ████████    │   │   ░░░░░░░░    │             │
│      └───────────────┘   └───────────────┘   └───────────────┘             │
│      KYC: Pending        KYC: Complete       KYC: Draft                    │
│                                                                             │
│  ════════════════════════════════════════════════════════════════════════  │
│                                                                             │
│      ┌─────────┐         ┌─────────┐         ┌─────────┐                   │
│      │ CUSTODY │─────────│FUND_ACCT│─────────│ TRADING │  ← Products       │
│      └─────────┘         └─────────┘         └─────────┘    (smaller)      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘

LEGEND:
  Node Size    = Importance (CBU large, intermediates medium, leaves small)
  Fill Pattern = KYC Status (████ complete, ▓▓▓▓ partial, ░░░░ draft, ○○○○ pending)
  Edge Width   = Ownership % (thick = high, thin = low)
  Edge Style   = Verification (══ proven, ── alleged, ╌╌ disputed)
  Color        = Entity Category (blue=SHELL, green=PERSON, orange=PRODUCT)
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Server Side (Rust)                                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  CbuGraphBuilder                                                            │
│  └── Loads data, computes:                                                  │
│      • node_importance (for sizing)                                         │
│      • edge_weight (ownership %)                                            │
│      • verification_status (for styling)                                    │
│      • depth_in_chain (for hierarchy)                                       │
│                                                                             │
│  LayoutEngine (ENHANCED)                                                    │
│  └── Computes positions using:                                              │
│      • Ownership-aware hierarchical layout                                  │
│      • Force-directed refinement                                            │
│      • Edge-crossing minimization                                           │
│      • Cluster grouping                                                     │
│                                                                             │
│  Output: CbuGraph with x, y, width, height, visual_hints                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              │ JSON
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  Client Side (EGUI/WASM)                                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  GraphRenderer                                                              │
│  └── Renders using server-computed positions:                               │
│      • Variable node sizes based on importance                              │
│      • Fill patterns based on status                                        │
│      • Edge curves with Bezier routing                                      │
│      • Labels with smart positioning                                        │
│      • Hover/selection highlighting                                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Enhanced Node Properties

### 1.1 Add Visual Hint Fields to GraphNode

**File:** `rust/src/graph/types.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphNode {
    // ... existing fields ...
    
    // =========================================================================
    // VISUAL HINTS - computed by server, used by renderer
    // =========================================================================
    
    /// Node importance score (0.0 - 1.0) - affects rendered size
    /// CBU = 1.0, direct children = 0.8, deeper = decreasing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance: Option<f32>,
    
    /// Depth in ownership hierarchy (0 = root CBU, 1 = direct, 2+ = chain)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hierarchy_depth: Option<i32>,
    
    /// KYC completion percentage (0-100) - affects fill pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kyc_completion: Option<i32>,
    
    /// Verification status for this entity's relationships
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_summary: Option<VerificationSummary>,
    
    /// Whether this node needs attention (has issues/gaps)
    #[serde(default)]
    pub needs_attention: bool,
    
    /// Suggested color hint (override default by type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_hint: Option<String>,
    
    /// Group/cluster this node belongs to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VerificationSummary {
    pub total_edges: i32,
    pub proven_edges: i32,
    pub alleged_edges: i32,
    pub disputed_edges: i32,
}
```

### 1.2 Add Visual Hint Fields to GraphEdge

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    // ... existing fields ...
    
    // =========================================================================
    // VISUAL HINTS
    // =========================================================================
    
    /// Ownership percentage (0-100) - affects edge thickness
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
    
    /// Verification status - affects line style
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<String>,  // "proven", "alleged", "disputed", "pending"
    
    /// Control points for curved edge routing
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub control_points: Vec<(f32, f32)>,
    
    /// Whether to animate this edge (e.g., data flow)
    #[serde(default)]
    pub animated: bool,
}
```

### 1.3 Tasks

- [ ] Add `importance`, `hierarchy_depth`, `kyc_completion` to `GraphNode`
- [ ] Add `VerificationSummary` struct
- [ ] Add `weight`, `verification_status`, `control_points` to `GraphEdge`
- [ ] Update serialization

---

## Phase 2: Semantic Layout Engine

### 2.1 Ownership-Aware Hierarchical Layout

**File:** `rust/src/graph/layout.rs`

The key insight: **ownership flows upward** (Fund → ManCo → HoldCo → UBO). Layout should reflect this.

```rust
impl LayoutEngine {
    /// UBO view: Tree layout following ownership chains
    /// 
    /// Algorithm:
    /// 1. Find root entities (CBU's direct subject entities)
    /// 2. Build ownership tree by following entity_relationships
    /// 3. Assign tiers based on ownership depth
    /// 4. Position nodes to minimize edge crossings
    /// 5. Apply force-directed refinement for spacing
    fn layout_ubo_tree(&self, graph: &mut CbuGraph) {
        // Step 1: Identify ownership hierarchy from edges
        let ownership_tree = self.build_ownership_tree(graph);
        
        // Step 2: Assign tiers (depth in tree)
        let tiers = self.assign_tiers(&ownership_tree);
        
        // Step 3: Order nodes within tiers to minimize crossings
        let ordered_tiers = self.minimize_crossings(&tiers, graph);
        
        // Step 4: Compute initial positions
        self.position_tiers(graph, &ordered_tiers);
        
        // Step 5: Refine with force-directed adjustment
        self.force_refine(graph, 50);  // 50 iterations
        
        // Step 6: Route edges with curves to avoid overlaps
        self.route_edges(graph);
    }
    
    /// Build tree structure from ownership edges
    fn build_ownership_tree(&self, graph: &CbuGraph) -> OwnershipTree {
        let mut tree = OwnershipTree::new();
        
        // Find edges of type "Owns"
        for edge in &graph.edges {
            if edge.edge_type == EdgeType::Owns {
                tree.add_edge(&edge.source, &edge.target, edge.weight.unwrap_or(0.0));
            }
        }
        
        tree
    }
    
    /// Minimize edge crossings using barycenter method
    fn minimize_crossings(&self, tiers: &[Vec<String>], graph: &CbuGraph) -> Vec<Vec<String>> {
        let mut result = tiers.to_vec();
        
        // Iterate: reorder each tier based on average position of neighbors
        for _ in 0..10 {  // 10 iterations usually enough
            // Forward pass
            for tier_idx in 1..result.len() {
                result[tier_idx] = self.reorder_by_barycenter(
                    &result[tier_idx],
                    &result[tier_idx - 1],
                    graph,
                    true
                );
            }
            // Backward pass
            for tier_idx in (0..result.len() - 1).rev() {
                result[tier_idx] = self.reorder_by_barycenter(
                    &result[tier_idx],
                    &result[tier_idx + 1],
                    graph,
                    false
                );
            }
        }
        
        result
    }
    
    /// Force-directed refinement for fine-tuning positions
    fn force_refine(&self, graph: &mut CbuGraph, iterations: usize) {
        for _ in 0..iterations {
            // Repulsion between all nodes
            let mut forces: HashMap<String, (f32, f32)> = HashMap::new();
            
            for i in 0..graph.nodes.len() {
                for j in (i + 1)..graph.nodes.len() {
                    let (fx, fy) = self.repulsion_force(&graph.nodes[i], &graph.nodes[j]);
                    
                    let id_i = graph.nodes[i].id.clone();
                    let id_j = graph.nodes[j].id.clone();
                    
                    forces.entry(id_i.clone()).or_default().0 += fx;
                    forces.entry(id_i).or_default().1 += fy;
                    forces.entry(id_j.clone()).or_default().0 -= fx;
                    forces.entry(id_j).or_default().1 -= fy;
                }
            }
            
            // Attraction along edges
            for edge in &graph.edges {
                if let (Some(src), Some(tgt)) = (
                    graph.nodes.iter().find(|n| n.id == edge.source),
                    graph.nodes.iter().find(|n| n.id == edge.target)
                ) {
                    let (fx, fy) = self.attraction_force(src, tgt, edge.weight.unwrap_or(50.0));
                    
                    forces.entry(edge.source.clone()).or_default().0 += fx;
                    forces.entry(edge.source.clone()).or_default().1 += fy;
                    forces.entry(edge.target.clone()).or_default().0 -= fx;
                    forces.entry(edge.target.clone()).or_default().1 -= fy;
                }
            }
            
            // Apply forces (with damping)
            let damping = 0.1;
            for node in &mut graph.nodes {
                if let Some((fx, fy)) = forces.get(&node.id) {
                    if let (Some(x), Some(y)) = (node.x.as_mut(), node.y.as_mut()) {
                        *x += fx * damping;
                        *y += fy * damping;
                    }
                }
            }
        }
    }
}
```

### 2.2 Edge Routing with Bezier Curves

```rust
impl LayoutEngine {
    /// Route edges as curves to avoid node overlaps
    fn route_edges(&self, graph: &mut CbuGraph) {
        for edge in &mut graph.edges {
            let src = graph.nodes.iter().find(|n| n.id == edge.source);
            let tgt = graph.nodes.iter().find(|n| n.id == edge.target);
            
            if let (Some(src), Some(tgt)) = (src, tgt) {
                let (sx, sy) = (src.x.unwrap_or(0.0), src.y.unwrap_or(0.0));
                let (tx, ty) = (tgt.x.unwrap_or(0.0), tgt.y.unwrap_or(0.0));
                
                // Calculate control points for a smooth curve
                let mid_y = (sy + ty) / 2.0;
                
                // Vertical layout: curve horizontally at midpoint
                edge.control_points = vec![
                    (sx, mid_y),      // Control point 1
                    (tx, mid_y),      // Control point 2
                ];
            }
        }
    }
}
```

### 2.3 Tasks

- [ ] Implement `build_ownership_tree()` from edges
- [ ] Implement `assign_tiers()` based on tree depth
- [ ] Implement `minimize_crossings()` using barycenter method
- [ ] Implement `force_refine()` for spacing adjustment
- [ ] Implement `route_edges()` with Bezier control points
- [ ] Add ownership tree traversal for `entity_relationships` table

---

## Phase 3: Visual Differentiation in Builder

### 3.1 Compute Node Importance

**File:** `rust/src/graph/builder.rs`

```rust
impl CbuGraphBuilder {
    /// Compute importance score for each node
    /// - CBU = 1.0 (always most important)
    /// - Direct entities (Asset Owner, ManCo) = 0.9
    /// - Intermediate owners = 0.7
    /// - Ultimate beneficial owners = 0.8 (important despite depth)
    /// - Products/Services = 0.5
    /// - Resources = 0.3
    fn compute_importance(&self, graph: &mut CbuGraph) {
        // First pass: base importance by node type
        for node in &mut graph.nodes {
            node.importance = Some(match node.node_type {
                NodeType::Cbu => 1.0,
                NodeType::Entity => {
                    // Natural persons (UBOs) get higher importance
                    if node.entity_category.as_deref() == Some("PERSON") {
                        0.85
                    } else {
                        // Role-based importance
                        match node.primary_role.as_deref() {
                            Some("ASSET_OWNER") => 0.9,
                            Some("MANAGEMENT_COMPANY") => 0.85,
                            Some("INVESTMENT_MANAGER") => 0.8,
                            Some("CUSTODIAN") | Some("SUB_CUSTODIAN") => 0.7,
                            Some("BROKER") | Some("COUNTERPARTY") => 0.6,
                            _ => 0.5,
                        }
                    }
                }
                NodeType::Product => 0.6,
                NodeType::Service => 0.5,
                NodeType::Resource => 0.4,
                NodeType::Document => 0.3,
                _ => 0.5,
            });
        }
        
        // Second pass: adjust by hierarchy depth
        for node in &mut graph.nodes {
            if let Some(depth) = node.hierarchy_depth {
                // Decay importance with depth, but not too much
                let depth_factor = 1.0 - (depth as f32 * 0.05).min(0.3);
                if let Some(imp) = node.importance.as_mut() {
                    *imp *= depth_factor;
                }
            }
        }
    }
    
    /// Compute verification summary for entities
    async fn compute_verification_status(&self, graph: &mut CbuGraph, repo: &VisualizationRepository) {
        // Query cbu_relationship_verification for each entity's edges
        let verifications = repo.get_relationship_verifications(self.cbu_id).await?;
        
        // Group by entity
        let mut by_entity: HashMap<Uuid, VerificationSummary> = HashMap::new();
        
        for v in verifications {
            let entry = by_entity.entry(v.from_entity_id).or_default();
            entry.total_edges += 1;
            match v.status.as_str() {
                "proven" => entry.proven_edges += 1,
                "alleged" => entry.alleged_edges += 1,
                "disputed" => entry.disputed_edges += 1,
                _ => {}
            }
        }
        
        // Apply to nodes
        for node in &mut graph.nodes {
            if let Ok(entity_id) = Uuid::parse_str(&node.id) {
                if let Some(summary) = by_entity.get(&entity_id) {
                    node.verification_summary = Some(summary.clone());
                    node.kyc_completion = Some(
                        (summary.proven_edges as f32 / summary.total_edges.max(1) as f32 * 100.0) as i32
                    );
                    node.needs_attention = summary.disputed_edges > 0 || summary.alleged_edges > 0;
                }
            }
        }
    }
}
```

### 3.2 Compute Edge Weights

```rust
impl CbuGraphBuilder {
    /// Add ownership percentage as edge weight
    async fn load_ownership_edges(&self, graph: &mut CbuGraph, repo: &VisualizationRepository) {
        let relationships = repo.get_entity_relationships(self.cbu_id).await?;
        
        for rel in relationships {
            let edge = GraphEdge {
                id: rel.relationship_id.to_string(),
                source: rel.from_entity_id.to_string(),
                target: rel.to_entity_id.to_string(),
                edge_type: match rel.relationship_type.as_str() {
                    "ownership" => EdgeType::Owns,
                    "control" => EdgeType::Controls,
                    _ => EdgeType::HasRole,
                },
                label: rel.percentage.map(|p| format!("{:.0}%", p)),
                weight: rel.percentage.map(|p| p as f32),
                verification_status: Some(rel.status),
                control_points: vec![],
                animated: false,
            };
            
            graph.add_edge(edge);
        }
    }
}
```

### 3.3 Tasks

- [ ] Implement `compute_importance()` based on node type and role
- [ ] Implement `compute_verification_status()` from verification table
- [ ] Add `get_relationship_verifications()` to VisualizationRepository
- [ ] Add `get_entity_relationships()` to VisualizationRepository
- [ ] Compute `hierarchy_depth` during ownership tree traversal
- [ ] Set `needs_attention` flag for entities with issues

---

## Phase 4: EGUI Renderer Enhancements

### 4.1 Variable Node Sizing

**File:** `crates/ob-poc-ui/src/graph_renderer.rs` (or equivalent)

```rust
impl GraphRenderer {
    fn render_node(&self, ui: &mut egui::Ui, node: &GraphNode, ctx: &RenderContext) {
        // Compute actual size from importance
        let base_size = 60.0;
        let importance = node.importance.unwrap_or(0.5);
        let size = base_size * (0.5 + importance * 0.8);  // Range: 30-108px
        
        let rect = egui::Rect::from_center_size(
            egui::pos2(node.x.unwrap_or(0.0), node.y.unwrap_or(0.0)),
            egui::vec2(size * 1.5, size),  // Slightly wider than tall
        );
        
        // Background with rounded corners
        let rounding = size * 0.15;
        let fill_color = self.node_fill_color(node);
        let stroke_color = self.node_stroke_color(node);
        
        ui.painter().rect(
            rect,
            rounding,
            fill_color,
            egui::Stroke::new(2.0, stroke_color),
        );
        
        // KYC completion indicator (fill bar at bottom)
        if let Some(completion) = node.kyc_completion {
            self.render_completion_bar(ui, rect, completion);
        }
        
        // Attention indicator (icon or pulse)
        if node.needs_attention {
            self.render_attention_indicator(ui, rect);
        }
        
        // Label
        self.render_node_label(ui, node, rect);
    }
    
    fn node_fill_color(&self, node: &GraphNode) -> egui::Color32 {
        // Base color by entity category
        let base = match node.entity_category.as_deref() {
            Some("PERSON") => egui::Color32::from_rgb(144, 238, 144),  // Light green
            Some("SHELL") => egui::Color32::from_rgb(135, 206, 250),   // Light blue
            _ => match node.node_type {
                NodeType::Cbu => egui::Color32::from_rgb(255, 215, 0),       // Gold
                NodeType::Product => egui::Color32::from_rgb(255, 182, 108), // Orange
                NodeType::Service => egui::Color32::from_rgb(221, 160, 221), // Plum
                _ => egui::Color32::from_rgb(200, 200, 200),                 // Gray
            }
        };
        
        // Darken based on KYC completion
        if let Some(completion) = node.kyc_completion {
            let factor = 0.5 + (completion as f32 / 100.0) * 0.5;
            egui::Color32::from_rgb(
                (base.r() as f32 * factor) as u8,
                (base.g() as f32 * factor) as u8,
                (base.b() as f32 * factor) as u8,
            )
        } else {
            base
        }
    }
    
    fn render_completion_bar(&self, ui: &mut egui::Ui, rect: egui::Rect, completion: i32) {
        let bar_height = 4.0;
        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 2.0, rect.bottom() - bar_height - 2.0),
            egui::pos2(rect.right() - 2.0, rect.bottom() - 2.0),
        );
        
        // Background
        ui.painter().rect_filled(bar_rect, 2.0, egui::Color32::from_gray(60));
        
        // Fill
        let fill_width = bar_rect.width() * (completion as f32 / 100.0);
        let fill_rect = egui::Rect::from_min_max(
            bar_rect.min,
            egui::pos2(bar_rect.left() + fill_width, bar_rect.bottom()),
        );
        
        let fill_color = if completion >= 80 {
            egui::Color32::from_rgb(76, 175, 80)   // Green
        } else if completion >= 50 {
            egui::Color32::from_rgb(255, 193, 7)   // Amber
        } else {
            egui::Color32::from_rgb(244, 67, 54)   // Red
        };
        
        ui.painter().rect_filled(fill_rect, 2.0, fill_color);
    }
}
```

### 4.2 Edge Rendering with Curves and Styles

```rust
impl GraphRenderer {
    fn render_edge(&self, ui: &mut egui::Ui, edge: &GraphEdge, nodes: &[GraphNode]) {
        let src = nodes.iter().find(|n| n.id == edge.source);
        let tgt = nodes.iter().find(|n| n.id == edge.target);
        
        let (src, tgt) = match (src, tgt) {
            (Some(s), Some(t)) => (s, t),
            _ => return,
        };
        
        let start = egui::pos2(src.x.unwrap_or(0.0), src.y.unwrap_or(0.0));
        let end = egui::pos2(tgt.x.unwrap_or(0.0), tgt.y.unwrap_or(0.0));
        
        // Edge style based on verification status
        let (color, stroke_width, dash) = match edge.verification_status.as_deref() {
            Some("proven") => (egui::Color32::from_rgb(76, 175, 80), 2.0, None),
            Some("alleged") => (egui::Color32::from_rgb(158, 158, 158), 1.5, Some(5.0)),
            Some("disputed") => (egui::Color32::from_rgb(244, 67, 54), 2.0, Some(3.0)),
            Some("pending") => (egui::Color32::from_rgb(255, 193, 7), 1.5, Some(8.0)),
            _ => (egui::Color32::from_gray(120), 1.0, None),
        };
        
        // Width based on ownership percentage
        let weight_factor = edge.weight.map(|w| w / 100.0).unwrap_or(0.5);
        let final_width = stroke_width * (0.5 + weight_factor);
        
        // Draw as Bezier curve if control points exist
        if edge.control_points.len() >= 2 {
            let cp1 = egui::pos2(edge.control_points[0].0, edge.control_points[0].1);
            let cp2 = egui::pos2(edge.control_points[1].0, edge.control_points[1].1);
            
            // Sample Bezier curve
            let points: Vec<egui::Pos2> = (0..=20)
                .map(|i| {
                    let t = i as f32 / 20.0;
                    self.cubic_bezier(start, cp1, cp2, end, t)
                })
                .collect();
            
            // Draw polyline
            ui.painter().add(egui::Shape::line(
                points,
                egui::Stroke::new(final_width, color),
            ));
        } else {
            // Straight line fallback
            ui.painter().line_segment(
                [start, end],
                egui::Stroke::new(final_width, color),
            );
        }
        
        // Arrow head at target
        self.render_arrow_head(ui, end, start, color, final_width);
        
        // Edge label (percentage)
        if let Some(label) = &edge.label {
            let mid = egui::pos2((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
            ui.painter().text(
                mid,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(10.0),
                egui::Color32::WHITE,
            );
        }
    }
    
    fn cubic_bezier(&self, p0: egui::Pos2, p1: egui::Pos2, p2: egui::Pos2, p3: egui::Pos2, t: f32) -> egui::Pos2 {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;
        
        egui::pos2(
            mt3 * p0.x + 3.0 * mt2 * t * p1.x + 3.0 * mt * t2 * p2.x + t3 * p3.x,
            mt3 * p0.y + 3.0 * mt2 * t * p1.y + 3.0 * mt * t2 * p2.y + t3 * p3.y,
        )
    }
}
```

### 4.3 Tasks

- [ ] Implement variable node sizing based on `importance`
- [ ] Implement fill patterns/colors based on `kyc_completion`
- [ ] Implement attention indicators (pulsing border, icon)
- [ ] Implement edge curves using Bezier control points
- [ ] Implement edge styling by verification status
- [ ] Implement edge width by ownership percentage
- [ ] Add edge labels (percentage) with smart positioning
- [ ] Implement arrow heads for directional edges

---

## Phase 5: View-Specific Optimizations

### 5.1 UBO View Specifics

```rust
impl LayoutEngine {
    fn layout_ubo_view(&self, graph: &mut CbuGraph) {
        // UBO view focuses on ownership/control
        // Tree flows: CBU → Fund → ManCo → HoldCo → Natural Persons (UBOs)
        
        // Key visual priorities:
        // 1. Natural persons (UBOs) highlighted and positioned prominently
        // 2. Ownership % clearly visible on edges
        // 3. Verification status obvious (green=proven, red=disputed)
        // 4. Chain depth visible through tier positioning
        
        // Position UBOs in a row at the top (they're the "answer")
        let ubos: Vec<_> = graph.nodes.iter()
            .filter(|n| n.entity_category.as_deref() == Some("PERSON"))
            .collect();
        
        // Position intermediaries below
        let intermediaries: Vec<_> = graph.nodes.iter()
            .filter(|n| n.entity_category.as_deref() == Some("SHELL"))
            .collect();
        
        // CBU at bottom (the subject of analysis)
        // This inverts typical org chart - we're showing "who owns this?"
    }
}
```

### 5.2 Products/Services View Specifics

```rust
impl LayoutEngine {
    fn layout_products_view(&self, graph: &mut CbuGraph) {
        // Products view focuses on services
        // Tree flows: CBU → Products → Services → Resources
        
        // Key visual priorities:
        // 1. Products are main nodes
        // 2. Services cluster under their product
        // 3. Resources small, attached to services
        // 4. Trading entities shown separately
        
        // Radial layout around CBU works well here
        self.layout_radial(graph, NodeType::Cbu, NodeType::Product, 200.0);
        
        // Services in second ring
        self.layout_radial_children(graph, NodeType::Product, NodeType::Service, 100.0);
    }
}
```

### 5.3 Tasks

- [ ] Create UBO-specific layout with inverted hierarchy
- [ ] Create Products-specific radial layout
- [ ] Create Service Delivery tree layout
- [ ] Add view mode selector in UI
- [ ] Animate transitions between views

---

## Phase 6: Polish & Interaction

### 6.1 Hover and Selection

- [ ] Highlight connected edges on node hover
- [ ] Dim unconnected nodes on hover
- [ ] Show tooltip with entity details on hover
- [ ] Click to select and show detail panel
- [ ] Multi-select for comparison

### 6.2 Zoom and Pan

- [ ] Smooth zoom in/out
- [ ] Pan with drag
- [ ] Fit-to-view button
- [ ] Mini-map for large graphs

### 6.3 Legends and Labels

- [ ] Color legend for entity types
- [ ] Edge style legend for verification status
- [ ] Size legend for importance
- [ ] Toggleable labels (show all / hover only)

---

## Implementation Order

1. **Phase 1** (types.rs changes) - Foundation
2. **Phase 3** (builder.rs) - Compute visual hints server-side
3. **Phase 2** (layout.rs) - Smarter positioning
4. **Phase 4** (EGUI renderer) - Visual rendering
5. **Phase 5** (view-specific) - Optimize per view
6. **Phase 6** (polish) - Interaction refinements

---

## Success Criteria

- [ ] Ownership chains are visually clear (tree structure)
- [ ] UBOs (natural persons) are visually prominent
- [ ] Verification status obvious at a glance
- [ ] Node importance reflected in size
- [ ] Edge weight reflected in thickness
- [ ] Edges don't cross through nodes
- [ ] Labels readable without overlap
- [ ] View transitions are smooth
- [ ] Graph "pops" - immediately communicates structure

---

## References

- Current layout: `/rust/src/graph/layout.rs`
- Current types: `/rust/src/graph/types.rs`
- Current builder: `/rust/src/graph/builder.rs`
- EGUI rules: `/EGUI-RULES.md`
- Visualization spec: `/docs/CBU_UNIVERSE_VISUALIZATION_SPEC.md`
- Visualization repo: `/rust/src/database/visualization_repository.rs`

---

*Enhancement plan for CBU graph visualization to achieve visual impact and clarity.*
