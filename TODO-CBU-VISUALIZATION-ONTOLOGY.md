# TODO: CBU Visualization - Ontology / Taxonomy View

## ⛔ MANDATORY FIRST STEP

**Read these files first:**
- `/EGUI-RULES.md` - Non-negotiable UI patterns
- `/TODO-CBU-VISUALIZATION-ANIMATION.md` - Animation foundation (build this first)
- `/TODO-CBU-VISUALIZATION-ENHANCEMENT.md` - Visual properties (node styling)

**Dependencies:** This builds ON TOP of the animation engine (springs, camera, gestures).

---

## Overview

The Ontology view treats CBU data as **hierarchical structures**:
- Ownership chains (who owns what)
- Product → Service → Resource taxonomy
- Entity type classification
- Role hierarchies

Navigation is about **walking the tree** - expand, collapse, prune, focus.

---

## Part 1: Tree Layout Engine

### 1.1 The Problem with Basic Tier Layout

Current `layout.rs` uses fixed tiers - all nodes at same depth get same Y position. This fails for:
- DAGs (directed acyclic graphs) - node has multiple parents
- Wide trees - nodes overlap horizontally
- Unbalanced trees - wasted space
- Edge crossings - hard to follow relationships

### 1.2 Sugiyama Algorithm (Layered Graph Drawing)

The gold standard for hierarchical layouts:

```
Step 1: Cycle Removal     - Make graph acyclic (reverse some edges)
Step 2: Layer Assignment  - Assign nodes to horizontal layers
Step 3: Crossing Reduction - Reorder nodes within layers to minimize edge crossings
Step 4: Coordinate Assignment - Set X positions to minimize edge length
Step 5: Edge Routing      - Draw edges (straight, orthogonal, or curved)
```

### 1.3 Implementation

```rust
//! Sugiyama-style hierarchical layout for tree/DAG visualization
//!
//! Produces clean, readable layouts for ownership chains and taxonomies.

pub struct TreeLayout {
    /// Nodes organized by layer (0 = root)
    layers: Vec<Vec<String>>,
    /// Node positions after layout
    positions: HashMap<String, Vec2>,
    /// Edge routing control points
    edge_routes: HashMap<String, Vec<Vec2>>,
    /// Configuration
    config: TreeLayoutConfig,
}

pub struct TreeLayoutConfig {
    /// Vertical spacing between layers
    pub layer_spacing: f32,      // Default: 100.0
    /// Minimum horizontal spacing between siblings
    pub sibling_spacing: f32,    // Default: 50.0
    /// Minimum horizontal spacing between subtrees
    pub subtree_spacing: f32,    // Default: 80.0
    /// Node width (for spacing calculations)
    pub node_width: f32,         // Default: 120.0
    /// Node height
    pub node_height: f32,        // Default: 50.0
    /// Layout direction
    pub direction: LayoutDirection,
}

pub enum LayoutDirection {
    TopToBottom,   // Root at top (ownership chains)
    BottomToTop,   // Root at bottom (reverse ownership)
    LeftToRight,   // Root at left
    RightToLeft,   // Root at right
}

impl TreeLayout {
    /// Build layout from graph edges
    pub fn from_graph(
        nodes: &[GraphNode],
        edges: &[GraphEdge],
        root_id: &str,
        config: TreeLayoutConfig,
    ) -> Self {
        let mut layout = Self {
            layers: vec![],
            positions: HashMap::new(),
            edge_routes: HashMap::new(),
            config,
        };
        
        // Step 1: Build adjacency for tree traversal
        let children = build_children_map(edges);
        let parents = build_parents_map(edges);
        
        // Step 2: Assign layers via BFS from root
        layout.assign_layers(root_id, &children);
        
        // Step 3: Order nodes within layers to minimize crossings
        layout.minimize_crossings(&children, &parents);
        
        // Step 4: Assign X coordinates
        layout.assign_coordinates(nodes);
        
        // Step 5: Route edges
        layout.route_edges(edges);
        
        layout
    }
    
    /// Assign nodes to layers (BFS)
    fn assign_layers(&mut self, root_id: &str, children: &HashMap<String, Vec<String>>) {
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        
        queue.push_back((root_id.to_string(), 0));
        visited.insert(root_id.to_string());
        
        while let Some((node_id, layer)) = queue.pop_front() {
            // Ensure layer exists
            while self.layers.len() <= layer {
                self.layers.push(vec![]);
            }
            self.layers[layer].push(node_id.clone());
            
            // Queue children
            if let Some(child_ids) = children.get(&node_id) {
                for child_id in child_ids {
                    if !visited.contains(child_id) {
                        visited.insert(child_id.clone());
                        queue.push_back((child_id.clone(), layer + 1));
                    }
                }
            }
        }
    }
    
    /// Reorder nodes within layers to minimize edge crossings (barycenter method)
    fn minimize_crossings(&mut self, children: &HashMap<String, Vec<String>>, parents: &HashMap<String, Vec<String>>) {
        // Multiple passes for convergence
        for _ in 0..10 {
            // Forward pass (top to bottom)
            for layer_idx in 1..self.layers.len() {
                self.reorder_layer_by_parents(layer_idx, parents);
            }
            
            // Backward pass (bottom to top)
            for layer_idx in (0..self.layers.len() - 1).rev() {
                self.reorder_layer_by_children(layer_idx, children);
            }
        }
    }
    
    fn reorder_layer_by_parents(&mut self, layer_idx: usize, parents: &HashMap<String, Vec<String>>) {
        let prev_layer = &self.layers[layer_idx - 1];
        let prev_positions: HashMap<_, _> = prev_layer.iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i))
            .collect();
        
        // Calculate barycenter for each node
        let mut barycenters: Vec<(String, f32)> = self.layers[layer_idx]
            .iter()
            .map(|node_id| {
                let parent_ids = parents.get(node_id).cloned().unwrap_or_default();
                let parent_positions: Vec<f32> = parent_ids.iter()
                    .filter_map(|p| prev_positions.get(p).map(|&i| i as f32))
                    .collect();
                
                let barycenter = if parent_positions.is_empty() {
                    0.0
                } else {
                    parent_positions.iter().sum::<f32>() / parent_positions.len() as f32
                };
                
                (node_id.clone(), barycenter)
            })
            .collect();
        
        // Sort by barycenter
        barycenters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Update layer order
        self.layers[layer_idx] = barycenters.into_iter().map(|(id, _)| id).collect();
    }
    
    fn reorder_layer_by_children(&mut self, layer_idx: usize, children: &HashMap<String, Vec<String>>) {
        let next_layer = &self.layers[layer_idx + 1];
        let next_positions: HashMap<_, _> = next_layer.iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i))
            .collect();
        
        let mut barycenters: Vec<(String, f32)> = self.layers[layer_idx]
            .iter()
            .map(|node_id| {
                let child_ids = children.get(node_id).cloned().unwrap_or_default();
                let child_positions: Vec<f32> = child_ids.iter()
                    .filter_map(|c| next_positions.get(c).map(|&i| i as f32))
                    .collect();
                
                let barycenter = if child_positions.is_empty() {
                    0.0
                } else {
                    child_positions.iter().sum::<f32>() / child_positions.len() as f32
                };
                
                (node_id.clone(), barycenter)
            })
            .collect();
        
        barycenters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        self.layers[layer_idx] = barycenters.into_iter().map(|(id, _)| id).collect();
    }
    
    /// Assign X coordinates using Reingold-Tilford style algorithm
    fn assign_coordinates(&mut self, nodes: &[GraphNode]) {
        let node_map: HashMap<_, _> = nodes.iter().map(|n| (n.id.clone(), n)).collect();
        
        for (layer_idx, layer) in self.layers.iter().enumerate() {
            let y = layer_idx as f32 * self.config.layer_spacing;
            
            // Simple spacing for now (can be improved with subtree centering)
            let total_width = layer.len() as f32 * (self.config.node_width + self.config.sibling_spacing);
            let start_x = -total_width / 2.0;
            
            for (i, node_id) in layer.iter().enumerate() {
                let x = start_x + i as f32 * (self.config.node_width + self.config.sibling_spacing) 
                    + self.config.node_width / 2.0;
                
                self.positions.insert(node_id.clone(), Vec2::new(x, y));
            }
        }
    }
    
    /// Route edges with curves to avoid overlaps
    fn route_edges(&mut self, edges: &[GraphEdge]) {
        for edge in edges {
            if let (Some(&src_pos), Some(&tgt_pos)) = (
                self.positions.get(&edge.source),
                self.positions.get(&edge.target)
            ) {
                // For now: straight lines
                // TODO: Bezier curves for edges that cross other nodes
                self.edge_routes.insert(
                    edge.id.clone(),
                    vec![src_pos, tgt_pos],
                );
            }
        }
    }
    
    /// Get position for a node
    pub fn get_position(&self, node_id: &str) -> Option<Vec2> {
        self.positions.get(node_id).copied()
    }
    
    /// Get all edge routes
    pub fn get_edge_routes(&self) -> &HashMap<String, Vec<Vec2>> {
        &self.edge_routes
    }
}
```

### 1.4 Tasks - Tree Layout

- [ ] Create `TreeLayout` struct
- [ ] Implement `assign_layers()` via BFS
- [ ] Implement `minimize_crossings()` with barycenter method
- [ ] Implement `assign_coordinates()` with proper spacing
- [ ] Implement `route_edges()` with straight lines
- [ ] Add Bezier edge routing for crossing edges
- [ ] Support different `LayoutDirection` options
- [ ] Test with ownership chain data
- [ ] Test with product taxonomy data

---

## Part 2: Ownership Chain View

### 2.1 Concept

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  OWNERSHIP CHAIN (read bottom-up: "Who owns the fund?")                     │
│  ═══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│  LAYER 0 (Ultimate)      ┌─────────────────┐                               │
│                          │   ALLIANZ SE    │ ← Public company (terminus)    │
│                          │   [DE] ════════ │   Double border = verified     │
│                          └────────┬────────┘                               │
│                                   │                                         │
│                              100% ║ ← Thick line = high ownership           │
│                                   ║   Double = proven                       │
│  LAYER 1 (Intermediate)          │                                         │
│                          ┌────────┴────────┐                               │
│                          │  Allianz Asset  │                               │
│                          │  Management     │                               │
│                          │   [DE] ════════ │                               │
│                          └────────┬────────┘                               │
│                                   │                                         │
│                              100% ║                                         │
│                                   │                                         │
│  LAYER 2 (ManCo)                 │                                         │
│                          ┌────────┴────────┐                               │
│                          │   Allianz GI    │                               │
│                          │   GmbH          │                               │
│                          │   [LU] ▓▓▓▓▓▓▓▓ │ ← Partial fill = 80% KYC      │
│                          └────────┬────────┘                               │
│                    ┌──────────────┼──────────────┐                         │
│                    │              │              │                          │
│               100% ║         100% ║         100% ║                          │
│                    │              │              │                          │
│  LAYER 3 (Funds)   │              │              │                          │
│             ┌──────┴──────┐ ┌─────┴─────┐ ┌──────┴──────┐                  │
│             │   Fund A    │ │  Fund B   │ │   Fund C    │                  │
│             │   [CBU] ████│ │    ▓▓▓▓▓▓ │ │    ░░░░░░░░ │                  │
│             └─────────────┘ └───────────┘ └─────────────┘                  │
│              KYC: Complete   KYC: 60%      KYC: Draft                       │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│  LEGEND:                                                                    │
│  ════ Edge: Proven ownership    ──── Edge: Alleged ownership               │
│  ████ Node: KYC complete        ▓▓▓▓ Node: KYC partial                     │
│  ░░░░ Node: KYC draft           ○○○○ Node: KYC pending                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Ownership Chain Builder

```rust
/// Build ownership tree from entity_relationships
pub async fn build_ownership_chain(
    pool: &PgPool,
    cbu_id: Uuid,
) -> Result<OwnershipChain> {
    // Get the CBU's subject entities (funds we're analyzing)
    let subjects = sqlx::query_as!(
        EntityRecord,
        r#"
        SELECT e.* FROM entities e
        JOIN cbu_entity_roles cer ON e.entity_id = cer.entity_id
        WHERE cer.cbu_id = $1
          AND cer.role_code IN ('ASSET_OWNER', 'FUND', 'SUBJECT')
        "#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;
    
    let mut chain = OwnershipChain {
        cbu_id,
        layers: vec![],
        edges: vec![],
        terminus_entities: HashSet::new(),
    };
    
    // BFS upward through ownership relationships
    let mut visited: HashSet<Uuid> = HashSet::new();
    let mut current_layer: Vec<Uuid> = subjects.iter().map(|e| e.entity_id).collect();
    
    while !current_layer.is_empty() {
        let mut layer_entities = vec![];
        let mut next_layer: Vec<Uuid> = vec![];
        
        for entity_id in current_layer {
            if visited.contains(&entity_id) {
                continue;
            }
            visited.insert(entity_id);
            
            // Get entity details
            let entity = get_entity(pool, entity_id).await?;
            layer_entities.push(entity.clone());
            
            // Get owners of this entity
            let owners = sqlx::query_as!(
                OwnershipRecord,
                r#"
                SELECT er.*, crv.status as verification_status
                FROM entity_relationships er
                LEFT JOIN cbu_relationship_verification crv 
                    ON er.relationship_id = crv.relationship_id
                    AND crv.cbu_id = $2
                WHERE er.to_entity_id = $1
                  AND er.relationship_type = 'ownership'
                  AND (er.effective_to IS NULL OR er.effective_to > NOW())
                "#,
                entity_id,
                cbu_id
            )
            .fetch_all(pool)
            .await?;
            
            if owners.is_empty() {
                // Terminus - no further owners
                chain.terminus_entities.insert(entity_id);
            } else {
                for owner in owners {
                    chain.edges.push(OwnershipEdge {
                        from_entity_id: owner.from_entity_id,
                        to_entity_id: owner.to_entity_id,
                        percentage: owner.percentage,
                        verification_status: owner.verification_status.unwrap_or("unverified".to_string()),
                    });
                    
                    if !visited.contains(&owner.from_entity_id) {
                        next_layer.push(owner.from_entity_id);
                    }
                }
            }
        }
        
        if !layer_entities.is_empty() {
            chain.layers.push(layer_entities);
        }
        current_layer = next_layer;
    }
    
    // Reverse layers so root (terminus) is at top
    chain.layers.reverse();
    
    Ok(chain)
}
```

### 2.3 Tasks - Ownership View

- [ ] Create `build_ownership_chain()` query function
- [ ] Add API endpoint `/api/cbu/{id}/ownership-chain`
- [ ] Build graph nodes from ownership chain
- [ ] Build graph edges with percentage labels
- [ ] Apply `TreeLayout` to ownership data
- [ ] Render ownership percentages on edges
- [ ] Color edges by verification status
- [ ] Highlight terminus entities (UBOs, public companies)
- [ ] Add "walk up" button to show more layers
- [ ] Add "walk down" to show owned entities

---

## Part 3: Product/Service Taxonomy View

### 3.1 Concept

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PRODUCT/SERVICE TAXONOMY (read top-down: "What do we deliver?")            │
│  ═══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│                              ┌─────────────────┐                           │
│  LAYER 0 (CBU)               │      CBU        │                           │
│                              │  Allianz GI     │                           │
│                              └────────┬────────┘                           │
│                       ┌───────────────┼───────────────┐                    │
│                       │               │               │                     │
│                       ▼               ▼               ▼                     │
│  LAYER 1          ┌───────┐      ┌────────┐     ┌─────────┐                │
│  (Products)       │CUSTODY│      │FUND_ACC│     │ TRADING │                │
│                   │  [+]  │      │  [-]   │     │   [+]   │                │
│                   └───────┘      └────┬───┘     └─────────┘                │
│                                       │                                     │
│                        ┌──────────────┼──────────────┐                     │
│                        │              │              │                      │
│                        ▼              ▼              ▼                      │
│  LAYER 2          ┌────────┐    ┌─────────┐    ┌─────────┐                 │
│  (Services)       │  NAV   │    │INVESTOR │    │TA_AGENT │                 │
│                   │  CALC  │    │REGISTRY │    │         │                 │
│                   │  [-]   │    │  [+]    │    │   [+]   │                 │
│                   └────┬───┘    └─────────┘    └─────────┘                 │
│                        │                                                    │
│             ┌──────────┴──────────┐                                        │
│             │                     │                                         │
│             ▼                     ▼                                         │
│  LAYER 3  ┌──────────┐      ┌──────────┐                                   │
│  (Resources)│ PRICING │      │ VALUATION│                                   │
│           │  FEED   │      │  ENGINE  │                                   │
│           └──────────┘      └──────────┘                                   │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│  [+] Collapsed - click to expand                                           │
│  [-] Expanded - click to collapse                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Expand/Collapse State

```rust
/// Manages expand/collapse state for taxonomy tree
pub struct TaxonomyState {
    /// Which nodes are expanded (show children)
    expanded: HashSet<String>,
    /// Animation progress for each node (0.0 = collapsed, 1.0 = expanded)
    expand_progress: HashMap<String, SpringF32>,
    /// Visibility of each node (affected by parent collapse)
    visibility: HashMap<String, f32>,
}

impl TaxonomyState {
    pub fn new() -> Self {
        Self {
            expanded: HashSet::new(),
            expand_progress: HashMap::new(),
            visibility: HashMap::new(),
        }
    }
    
    /// Toggle expand/collapse for a node
    pub fn toggle(&mut self, node_id: &str) {
        if self.expanded.contains(node_id) {
            self.collapse(node_id);
        } else {
            self.expand(node_id);
        }
    }
    
    pub fn expand(&mut self, node_id: &str) {
        self.expanded.insert(node_id.to_string());
        self.expand_progress
            .entry(node_id.to_string())
            .or_insert_with(|| SpringF32::new(0.0))
            .set_target(1.0);
    }
    
    pub fn collapse(&mut self, node_id: &str) {
        self.expanded.remove(node_id);
        if let Some(progress) = self.expand_progress.get_mut(node_id) {
            progress.set_target(0.0);
        }
    }
    
    /// Expand all nodes up to given depth
    pub fn expand_to_depth(&mut self, graph: &CbuGraph, depth: usize) {
        for node in &graph.nodes {
            if let Some(d) = node.hierarchy_depth {
                if (d as usize) < depth {
                    self.expand(&node.id);
                }
            }
        }
    }
    
    /// Collapse all
    pub fn collapse_all(&mut self) {
        for node_id in self.expanded.clone() {
            self.collapse(&node_id);
        }
    }
    
    /// Is node expanded?
    pub fn is_expanded(&self, node_id: &str) -> bool {
        self.expanded.contains(node_id)
    }
    
    /// Get expand progress (for animation)
    pub fn get_expand_progress(&self, node_id: &str) -> f32 {
        self.expand_progress.get(node_id).map(|p| p.get()).unwrap_or(0.0)
    }
    
    /// Should node be visible?
    pub fn is_visible(&self, node_id: &str, parent_chain: &[String]) -> bool {
        // Node is visible if all ancestors are expanded
        for ancestor in parent_chain {
            if !self.is_expanded(ancestor) {
                return false;
            }
        }
        true
    }
    
    pub fn tick(&mut self, dt: f32) {
        for progress in self.expand_progress.values_mut() {
            progress.tick(dt);
        }
    }
}
```

### 3.3 Animated Tree Layout

```rust
/// Tree layout that supports animated expand/collapse
pub struct AnimatedTreeLayout {
    /// Base layout (fully expanded)
    full_layout: TreeLayout,
    /// Current animated positions
    animated_positions: HashMap<String, SpringVec2>,
    /// Current animated scales (for collapse animation)
    animated_scales: HashMap<String, SpringF32>,
    /// Current animated opacity
    animated_opacity: HashMap<String, SpringF32>,
}

impl AnimatedTreeLayout {
    pub fn from_graph(graph: &CbuGraph, root_id: &str, config: TreeLayoutConfig) -> Self {
        let full_layout = TreeLayout::from_graph(
            &graph.nodes,
            &graph.edges,
            root_id,
            config,
        );
        
        let mut animated_positions = HashMap::new();
        let mut animated_scales = HashMap::new();
        let mut animated_opacity = HashMap::new();
        
        for (node_id, &pos) in full_layout.positions.iter() {
            animated_positions.insert(node_id.clone(), SpringVec2::new(pos.x, pos.y));
            animated_scales.insert(node_id.clone(), SpringF32::new(1.0));
            animated_opacity.insert(node_id.clone(), SpringF32::new(1.0));
        }
        
        Self {
            full_layout,
            animated_positions,
            animated_scales,
            animated_opacity,
        }
    }
    
    /// Update layout based on expand/collapse state
    pub fn update_for_state(&mut self, state: &TaxonomyState, graph: &CbuGraph) {
        // Recalculate visible nodes
        let parent_map = build_parent_map(&graph.edges);
        
        for node in &graph.nodes {
            let parent_chain = get_parent_chain(&node.id, &parent_map);
            let is_visible = state.is_visible(&node.id, &parent_chain);
            
            // Update opacity target
            if let Some(opacity) = self.animated_opacity.get_mut(&node.id) {
                opacity.set_target(if is_visible { 1.0 } else { 0.0 });
            }
            
            // Update scale target (collapsed nodes shrink to parent position)
            if let Some(scale) = self.animated_scales.get_mut(&node.id) {
                scale.set_target(if is_visible { 1.0 } else { 0.0 });
            }
            
            // If collapsing, animate position toward parent
            if !is_visible {
                if let Some(parent_id) = parent_chain.first() {
                    if let Some(parent_pos) = self.full_layout.get_position(parent_id) {
                        if let Some(anim_pos) = self.animated_positions.get_mut(&node.id) {
                            anim_pos.set_target(parent_pos.x, parent_pos.y);
                        }
                    }
                }
            } else {
                // Restore to full layout position
                if let Some(&full_pos) = self.full_layout.positions.get(&node.id) {
                    if let Some(anim_pos) = self.animated_positions.get_mut(&node.id) {
                        anim_pos.set_target(full_pos.x, full_pos.y);
                    }
                }
            }
        }
    }
    
    pub fn tick(&mut self, dt: f32) {
        for pos in self.animated_positions.values_mut() {
            pos.tick(dt);
        }
        for scale in self.animated_scales.values_mut() {
            scale.tick(dt);
        }
        for opacity in self.animated_opacity.values_mut() {
            opacity.tick(dt);
        }
    }
    
    pub fn get_animated_position(&self, node_id: &str) -> Option<(f32, f32)> {
        self.animated_positions.get(node_id).map(|p| p.get())
    }
    
    pub fn get_animated_scale(&self, node_id: &str) -> f32 {
        self.animated_scales.get(node_id).map(|s| s.get()).unwrap_or(1.0)
    }
    
    pub fn get_animated_opacity(&self, node_id: &str) -> f32 {
        self.animated_opacity.get(node_id).map(|o| o.get()).unwrap_or(1.0)
    }
}
```

### 3.4 Tasks - Taxonomy View

- [ ] Create `TaxonomyState` for expand/collapse tracking
- [ ] Create `AnimatedTreeLayout` with spring animations
- [ ] Implement expand/collapse toggle
- [ ] Animate children appearing/disappearing
- [ ] Animate positions when siblings collapse
- [ ] Add expand/collapse button to nodes
- [ ] Add "Expand All" / "Collapse All" buttons
- [ ] Add "Expand to Level N" option
- [ ] Show child count on collapsed nodes (e.g., "[+3]")

---

## Part 4: Type Hierarchy Browser

### 4.1 Concept

Browse entities by type classification:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  ENTITY TYPE HIERARCHY                                                      │
│  ═══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│                         ┌─────────────────┐                                │
│                         │     ENTITY      │                                │
│                         └────────┬────────┘                                │
│                    ┌─────────────┴─────────────┐                           │
│                    │                           │                            │
│             ┌──────┴──────┐             ┌──────┴──────┐                    │
│             │    SHELL    │             │   PERSON    │                    │
│             │  (12 items) │             │  (5 items)  │                    │
│             └──────┬──────┘             └──────┬──────┘                    │
│                    │                           │                            │
│       ┌────────┬───┴───┬────────┐        ┌────┴────┐                       │
│       │        │       │        │        │         │                        │
│    ┌──┴──┐  ┌──┴──┐ ┌──┴──┐ ┌──┴──┐  ┌──┴──┐  ┌──┴──┐                     │
│    │CORP │  │FUND │ │TRUST│ │ LLC │  │ UBO │  │CTRL │                     │
│    │ (3) │  │ (5) │ │ (2) │ │ (2) │  │ (3) │  │ (2) │                     │
│    └─────┘  └─────┘ └─────┘ └─────┘  └─────┘  └─────┘                     │
│                                                                             │
│  Click type to filter graph view to entities of that type                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Type Hierarchy Data

```rust
/// Entity type ontology (from entity_taxonomy.yaml)
pub struct EntityTypeOntology {
    pub root: TypeNode,
}

pub struct TypeNode {
    pub type_code: String,
    pub label: String,
    pub children: Vec<TypeNode>,
    /// Entities in current graph matching this type
    pub matching_entities: Vec<Uuid>,
}

impl EntityTypeOntology {
    /// Build from taxonomy config + graph entities
    pub fn from_config_and_graph(config: &TaxonomyConfig, graph: &CbuGraph) -> Self {
        // Load type hierarchy from config
        let mut root = load_type_tree(config);
        
        // Count entities per type
        populate_entity_counts(&mut root, graph);
        
        Self { root }
    }
}

/// Load type tree from entity_taxonomy.yaml
fn load_type_tree(config: &TaxonomyConfig) -> TypeNode {
    TypeNode {
        type_code: "ENTITY".to_string(),
        label: "Entity".to_string(),
        children: vec![
            TypeNode {
                type_code: "SHELL".to_string(),
                label: "Legal Vehicle".to_string(),
                children: vec![
                    TypeNode { type_code: "CORPORATION".to_string(), label: "Corporation".to_string(), children: vec![], matching_entities: vec![] },
                    TypeNode { type_code: "FUND".to_string(), label: "Fund".to_string(), children: vec![], matching_entities: vec![] },
                    TypeNode { type_code: "TRUST".to_string(), label: "Trust".to_string(), children: vec![], matching_entities: vec![] },
                    TypeNode { type_code: "LLC".to_string(), label: "LLC".to_string(), children: vec![], matching_entities: vec![] },
                    TypeNode { type_code: "PARTNERSHIP".to_string(), label: "Partnership".to_string(), children: vec![], matching_entities: vec![] },
                ],
                matching_entities: vec![],
            },
            TypeNode {
                type_code: "PERSON".to_string(),
                label: "Natural Person".to_string(),
                children: vec![
                    TypeNode { type_code: "UBO".to_string(), label: "Beneficial Owner".to_string(), children: vec![], matching_entities: vec![] },
                    TypeNode { type_code: "CONTROL_PERSON".to_string(), label: "Control Person".to_string(), children: vec![], matching_entities: vec![] },
                    TypeNode { type_code: "AUTHORIZED_SIGNER".to_string(), label: "Authorized Signer".to_string(), children: vec![], matching_entities: vec![] },
                ],
                matching_entities: vec![],
            },
        ],
        matching_entities: vec![],
    }
}
```

### 4.3 Tasks - Type Browser

- [ ] Create `EntityTypeOntology` struct
- [ ] Load type hierarchy from `entity_taxonomy.yaml`
- [ ] Count entities per type from current graph
- [ ] Render type tree with counts
- [ ] Click type to highlight matching entities
- [ ] Click type to filter view to that type
- [ ] Show type in entity node tooltip

---

## Part 5: Tree Navigation Controls

### 5.1 Walk Up/Down

```rust
pub struct TreeNavigator {
    /// Current focus node (center of attention)
    focus_node: Option<String>,
    /// How many levels above focus to show
    levels_up: i32,
    /// How many levels below focus to show
    levels_down: i32,
    /// Pruned subtrees (hidden branches)
    pruned: HashSet<String>,
}

impl TreeNavigator {
    pub fn new() -> Self {
        Self {
            focus_node: None,
            levels_up: 2,
            levels_down: 2,
            pruned: HashSet::new(),
        }
    }
    
    /// Set focus to a specific node
    pub fn focus_on(&mut self, node_id: &str) {
        self.focus_node = Some(node_id.to_string());
    }
    
    /// Show one more level above
    pub fn walk_up(&mut self) {
        self.levels_up += 1;
    }
    
    /// Show one more level below
    pub fn walk_down(&mut self) {
        self.levels_down += 1;
    }
    
    /// Hide a subtree
    pub fn prune(&mut self, node_id: &str) {
        self.pruned.insert(node_id.to_string());
    }
    
    /// Restore a pruned subtree
    pub fn restore(&mut self, node_id: &str) {
        self.pruned.remove(node_id);
    }
    
    /// Is node visible given current navigation state?
    pub fn is_visible(&self, node_id: &str, depth_from_focus: i32, graph: &CbuGraph) -> bool {
        // Check prune
        if self.is_in_pruned_subtree(node_id, graph) {
            return false;
        }
        
        // Check depth limits
        if depth_from_focus < -self.levels_up {
            return false;
        }
        if depth_from_focus > self.levels_down {
            return false;
        }
        
        true
    }
    
    fn is_in_pruned_subtree(&self, node_id: &str, graph: &CbuGraph) -> bool {
        // Check if this node or any ancestor is pruned
        let parent_map = build_parent_map(&graph.edges);
        let mut current = Some(node_id.to_string());
        
        while let Some(id) = current {
            if self.pruned.contains(&id) {
                return true;
            }
            current = parent_map.get(&id).cloned();
        }
        
        false
    }
}
```

### 5.2 Keyboard Shortcuts

| Key | Action |
|-----|--------|
| ↑ | Walk up (show more ancestors) |
| ↓ | Walk down (show more descendants) |
| ← | Collapse focused node |
| → | Expand focused node |
| Enter | Focus on selected node |
| Backspace | Remove focus (show all) |
| P | Prune selected subtree |
| R | Restore pruned subtree |
| Home | Reset to default view |

### 5.3 Tasks - Navigation

- [ ] Create `TreeNavigator` struct
- [ ] Implement `walk_up()` and `walk_down()`
- [ ] Implement `prune()` and `restore()`
- [ ] Add keyboard shortcuts
- [ ] Show "+" indicator on pruned nodes
- [ ] Show depth indicator (e.g., "Level 2 of 5")
- [ ] Add breadcrumb trail for focus path

---

## Part 6: View Morphing (Tree ↔ Tree)

### 6.1 Concept

Smooth transition between different tree views:
- Ownership → Taxonomy (same entities, different hierarchy)
- Type filter change (subset changes)

### 6.2 Morph Animation

```rust
/// Animate transition between two tree layouts
pub struct TreeMorphAnimator {
    /// Source positions (where nodes are now)
    source_positions: HashMap<String, Vec2>,
    /// Target positions (where nodes will be)
    target_positions: HashMap<String, Vec2>,
    /// Animated positions
    animated_positions: HashMap<String, SpringVec2>,
    /// Nodes that appear (not in source)
    appearing: HashSet<String>,
    /// Nodes that disappear (not in target)
    disappearing: HashSet<String>,
}

impl TreeMorphAnimator {
    pub fn morph_to(&mut self, new_layout: &TreeLayout) {
        let old_nodes: HashSet<_> = self.source_positions.keys().cloned().collect();
        let new_nodes: HashSet<_> = new_layout.positions.keys().cloned().collect();
        
        // Identify appearing/disappearing nodes
        self.appearing = new_nodes.difference(&old_nodes).cloned().collect();
        self.disappearing = old_nodes.difference(&new_nodes).cloned().collect();
        
        // Update targets for existing nodes
        for (node_id, &target_pos) in new_layout.positions.iter() {
            if let Some(anim) = self.animated_positions.get_mut(node_id) {
                anim.set_target(target_pos.x, target_pos.y);
            } else {
                // New node - start at center, animate to position
                let mut anim = SpringVec2::new(0.0, 0.0);
                anim.set_target(target_pos.x, target_pos.y);
                self.animated_positions.insert(node_id.clone(), anim);
            }
        }
        
        // Disappearing nodes animate to center then remove
        for node_id in &self.disappearing {
            if let Some(anim) = self.animated_positions.get_mut(node_id) {
                anim.set_target(0.0, 0.0);  // Or to parent position
            }
        }
        
        self.target_positions = new_layout.positions.clone();
    }
}
```

### 6.3 Tasks - Morphing

- [ ] Create `TreeMorphAnimator`
- [ ] Detect appearing/disappearing nodes
- [ ] Animate existing nodes to new positions
- [ ] Fade in appearing nodes
- [ ] Fade out disappearing nodes
- [ ] Handle mid-morph interruption (new target)

---

## Success Criteria

- [ ] Ownership chain displays correctly (bottom-up reading)
- [ ] Product taxonomy displays correctly (top-down reading)
- [ ] Edge crossings are minimized
- [ ] Expand/collapse animates smoothly
- [ ] Children fly in/out from parent position
- [ ] Walk up/down reveals more levels
- [ ] Pruned branches show indicator
- [ ] Type filter highlights matching entities
- [ ] View transitions morph smoothly
- [ ] Keyboard navigation feels natural
- [ ] Performance handles 100+ node trees at 60fps

---

## References

- Sugiyama algorithm: https://en.wikipedia.org/wiki/Layered_graph_drawing
- Reingold-Tilford: https://reingold.co/tidier-drawings.pdf
- D3 hierarchy: https://github.com/d3/d3-hierarchy
- Entity taxonomy: `/rust/config/ontology/entity_taxonomy.yaml`

---

*Ontology view: CBU data as navigable hierarchies.*
