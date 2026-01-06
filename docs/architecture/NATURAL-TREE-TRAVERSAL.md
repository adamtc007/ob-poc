# Natural Tree Traversal: Visual Design for Taxonomy Navigation

## The Problem

Tree visualizations typically feel mechanical:
- Nodes snap into position
- Expansions are instant or linear
- Layout recalculates and everything jumps
- No sense of spatial continuity
- Feels like clicking through a file browser

**Goal:** Make tree navigation feel like exploring a living system - organic, continuous, spatially stable.

---

## Design Principles

### 1. Spatial Stability

**Rule:** Nodes should never teleport. A node's position can change, but only through visible motion.

```
BAD (instant relayout):
                                    
  [A]                    [A]─────[D]
   │         expand       │         
  [B]        ──────→     [B]  [E]    ← B moved! Where did it go?
                          │
                         [C]

GOOD (stable anchor):

  [A]                    [A]
   │         expand       │
  [B]        ──────→     [B]────┬────┐
                          │     │    │
                         [C]   [D]  [E]
                               ↑
                          Children grow FROM B, B stays put
```

**Implementation:**
- Parent node is the **anchor** - never moves during child expansion
- Children animate outward FROM parent position
- Siblings shift to make room (animate, don't snap)
- Camera adjusts to keep anchor visible

### 2. Organic Growth

**Rule:** Expansions should feel like growth, not appearance.

```
Frame 0:     Frame 5:      Frame 10:     Frame 15:

  [A]          [A]           [A]           [A]
   │            │             │             │
  [B]          [B]           [B]───┐       [B]───┬───┐
                              ·    │        │    │   │
                                  [C]      [C]  [D] [E]
                                  (small)  (full size)
```

**Animation sequence:**
1. **Bud** (0-20%) - Small dot appears at expansion point
2. **Sprout** (20-60%) - Dot grows, edge line extends from parent
3. **Unfurl** (60-90%) - Node reaches full size, label fades in
4. **Settle** (90-100%) - Micro-bounce, final position lock

**Easing:** Use spring physics, not linear or ease-in-out
```rust
// Spring config for organic feel
pub const TREE_GROWTH_SPRING: SpringConfig = SpringConfig {
    stiffness: 180.0,   // Responsive but not snappy
    damping: 12.0,      // Slight overshoot then settle
    mass: 1.0,
};
```

### 3. Progressive Disclosure

**Rule:** Don't show everything at once. Reveal in layers.

```
"Show ownership chain" for deeply nested structure:

Frame 0:           Frame 30:          Frame 60:          Frame 90:

[Entity]          [Entity]           [Entity]           [Entity]
                      │                  │                  │
                  [Owner1]           [Owner1]           [Owner1]
                  (growing)              │                  │
                                    [Owner2]           [Owner2]
                                    (growing)              │
                                                      [Ultimate]
                                                      (growing)
```

**Stagger timing:**
- Each level waits for previous level to reach 60% growth
- Creates "cascade" effect like dominos
- User sees the chain building, understands the structure

```rust
pub struct StaggerConfig {
    /// Delay before each level starts (ms)
    pub level_delay: f32,      // 150ms
    
    /// Threshold of parent completion before child starts
    pub cascade_threshold: f32, // 0.6 (60%)
    
    /// Max concurrent growing nodes
    pub max_concurrent: usize,  // 3
}
```

### 4. Breathing Room

**Rule:** Leave space for expansion. Don't pack nodes tight.

```
BAD (cramped):                    GOOD (room to grow):

[A][B][C][D][E]                   [A]    [B]    [C]    [D]    [E]
     ↓ expand B                        ↓ expand B
[A][B     ][C][D][E]              [A]   [B]     [C]    [D]    [E]
   [X][Y][Z]                             │
   ↑ everything shifts!                [X]  [Y]  [Z]
                                       ↑ grows into reserved space
```

**Layout strategy:**
- Base spacing = 1.5x node width (not 1.0x)
- Collapsed nodes have "potential space" reserved below/beside
- Expansion uses reserved space first, only shifts siblings if needed

### 5. Edge Animation

**Rule:** Edges are living connections, not static lines.

```
Edge growth sequence:

1. Dot at parent           2. Line extends          3. Reaches child
   [Parent]                   [Parent]                 [Parent]
       ·                          │                        │
                                  │                        │
                                  ·                    [Child]
```

**Edge properties during animation:**
- Start as point at parent anchor
- Grow toward child (Bezier control points animate)
- Thickness pulses slightly on connection
- Color can indicate state (growing = lighter, settled = full)

```rust
pub struct EdgeAnimation {
    /// Progress 0.0 to 1.0
    progress: f32,
    
    /// Edge grows from source toward target
    fn current_endpoint(&self) -> Pos2 {
        let full_vec = self.target - self.source;
        self.source + full_vec * self.progress
    }
    
    /// Thickness pulses at connection
    fn current_thickness(&self) -> f32 {
        let base = 2.0;
        let pulse = (self.progress * PI).sin() * 0.5;  // Pulse at 0.5
        base + pulse
    }
}
```

### 6. Collapse as Reverse Growth

**Rule:** Collapse should mirror expansion, not just disappear.

```
Collapse sequence (reverse of growth):

Frame 0:           Frame 10:          Frame 20:          Frame 30:

[Parent]          [Parent]           [Parent]           [Parent]
    │                 │                  │
[Child1]          [Child1]           [Child1]
    │             (shrinking)        (tiny dot)
[Child2]
(shrinking)
```

**Collapse order:**
- Deepest children collapse first (leaves before branches)
- Parent waits until children are gone
- Edge retracts as child shrinks
- Final dot absorbed into parent

### 7. Focus Follows Growth

**Rule:** Camera should anticipate growth direction.

```
Before expansion:              During expansion:

    ┌─────────────┐               ┌─────────────────────┐
    │   [Node]    │               │   [Node]            │
    │      ↓      │    ──→        │      │              │
    │   expand    │               │   [Child1] [Child2] │
    └─────────────┘               └─────────────────────┘
          ↑                                ↑
    Camera centered              Camera panned to show growth area
    on node                      BEFORE children fully appear
```

**Camera behavior:**
1. On expansion start, calculate bounds of final state
2. Begin camera animation immediately (don't wait for growth)
3. Camera arrives at new position as growth completes
4. Creates sense of "making room" for new content

---

## Animation Timing Reference

### Expansion Animations

| Phase | Duration | Easing | Description |
|-------|----------|--------|-------------|
| Bud | 50ms | ease-out | Dot appears |
| Sprout | 150ms | spring | Dot grows to 30% size |
| Edge Extend | 200ms | spring | Line grows from parent |
| Unfurl | 200ms | spring | Node reaches full size |
| Label Fade | 100ms | ease-in | Text appears |
| Settle | 100ms | spring | Micro-bounce |
| **Total** | ~500ms | | |

### Collapse Animations

| Phase | Duration | Easing | Description |
|-------|----------|--------|-------------|
| Label Fade | 50ms | ease-out | Text disappears |
| Shrink | 150ms | spring | Node shrinks to dot |
| Edge Retract | 150ms | spring | Line pulls back |
| Absorb | 50ms | ease-in | Dot absorbed into parent |
| **Total** | ~300ms | | Faster than expansion |

### Cascade Timing

| Level | Start Delay | Notes |
|-------|-------------|-------|
| Level 1 | 0ms | Immediate |
| Level 2 | 150ms | After L1 at 60% |
| Level 3 | 300ms | After L2 at 60% |
| Level N | 150ms * (N-1) | Staggered |

---

## Layout Algorithms

### Tree Layout with Reserved Space

```rust
pub struct TreeLayout {
    /// Base spacing between siblings
    base_spacing: f32,  // 1.5x node width
    
    /// Extra space reserved for potential expansion
    expansion_reserve: f32,  // 0.5x node height per potential child
    
    /// Direction of tree growth
    direction: TreeDirection,  // Down, Right, Radial
}

impl TreeLayout {
    pub fn layout(&self, root: &mut TreeNode) {
        // Pass 1: Calculate subtree sizes (including reserved space)
        self.measure_subtree(root);
        
        // Pass 2: Assign positions (anchor parents, position children)
        self.position_subtree(root, Pos2::ZERO);
    }
    
    fn measure_subtree(&self, node: &mut TreeNode) -> Vec2 {
        let own_size = node.size();
        
        if node.is_collapsed() {
            // Reserve space for potential children
            let reserve = self.expansion_reserve * node.potential_child_count() as f32;
            return Vec2::new(own_size.x, own_size.y + reserve);
        }
        
        // Measure children
        let mut child_extent = Vec2::ZERO;
        for child in &mut node.children {
            let child_size = self.measure_subtree(child);
            child_extent.x += child_size.x + self.base_spacing;
            child_extent.y = child_extent.y.max(child_size.y);
        }
        
        Vec2::new(
            own_size.x.max(child_extent.x),
            own_size.y + self.base_spacing + child_extent.y,
        )
    }
}
```

### Radial Layout for Galaxy/Cluster Views

```rust
pub struct RadialLayout {
    /// Center of radial layout
    center: Pos2,
    
    /// Starting radius for first ring
    base_radius: f32,
    
    /// Radius increment per level
    radius_step: f32,
    
    /// Angular spread for children
    spread_angle: f32,  // radians
}

impl RadialLayout {
    pub fn layout_children(&self, parent: &TreeNode, children: &mut [TreeNode]) {
        let n = children.len();
        if n == 0 { return; }
        
        // Calculate angular positions
        let angle_step = self.spread_angle / (n as f32 - 1.0).max(1.0);
        let start_angle = parent.angle - self.spread_angle / 2.0;
        
        for (i, child) in children.iter_mut().enumerate() {
            let angle = start_angle + angle_step * i as f32;
            let radius = parent.radius + self.radius_step;
            
            child.target_position = Pos2::new(
                self.center.x + radius * angle.cos(),
                self.center.y + radius * angle.sin(),
            );
            child.angle = angle;
            child.radius = radius;
        }
    }
}
```

---

## Rendering Pipeline

### Per-Frame Update

```rust
impl TreeRenderer {
    pub fn update(&mut self, dt: f32) {
        // 1. Update all node animations
        for node in self.nodes.values_mut() {
            node.animation.tick(dt);
        }
        
        // 2. Update all edge animations
        for edge in self.edges.values_mut() {
            edge.animation.tick(dt);
        }
        
        // 3. Check for cascade triggers
        for node in self.nodes.values_mut() {
            if node.animation.progress() > CASCADE_THRESHOLD {
                self.trigger_child_animations(node.id);
            }
        }
        
        // 4. Update camera
        self.camera.tick(dt);
    }
    
    pub fn render(&self, painter: &Painter) {
        // 1. Render edges first (behind nodes)
        for edge in self.edges.values() {
            self.render_edge(painter, edge);
        }
        
        // 2. Render nodes back-to-front (by depth)
        let mut nodes: Vec<_> = self.nodes.values().collect();
        nodes.sort_by_key(|n| n.depth);
        
        for node in nodes {
            self.render_node(painter, node);
        }
    }
    
    fn render_node(&self, painter: &Painter, node: &TreeNode) {
        let anim = &node.animation;
        
        // Scale based on animation progress
        let scale = anim.scale();  // 0.0 → 1.0 with overshoot
        let size = node.base_size * scale;
        
        // Position interpolated
        let pos = anim.current_position();
        
        // Alpha for fade-in
        let alpha = anim.alpha();
        
        // Draw node
        let rect = Rect::from_center_size(pos, size);
        let color = node.color.linear_multiply(alpha);
        painter.rect_filled(rect, 4.0, color);
        
        // Draw label (fades in later)
        if anim.progress() > 0.7 {
            let label_alpha = ((anim.progress() - 0.7) / 0.3).min(1.0);
            painter.text(
                pos,
                Align2::CENTER_CENTER,
                &node.label,
                FontId::proportional(12.0),
                Color32::WHITE.linear_multiply(label_alpha),
            );
        }
    }
    
    fn render_edge(&self, painter: &Painter, edge: &TreeEdge) {
        let anim = &edge.animation;
        
        // Edge grows from source toward target
        let start = edge.source_pos;
        let end = anim.current_endpoint();
        
        // Bezier control points for smooth curve
        let ctrl1 = start + Vec2::new(0.0, 20.0);
        let ctrl2 = end + Vec2::new(0.0, -20.0);
        
        // Draw with animated thickness
        let thickness = anim.current_thickness();
        let alpha = anim.alpha();
        
        let path = PathShape::cubic_bezier(
            start, ctrl1, ctrl2, end,
            Stroke::new(thickness, edge.color.linear_multiply(alpha)),
        );
        painter.add(path);
    }
}
```

---

## Spring Physics Reference

### Spring Formula

```rust
pub struct Spring {
    position: f32,
    velocity: f32,
    target: f32,
    config: SpringConfig,
}

impl Spring {
    pub fn tick(&mut self, dt: f32) {
        let displacement = self.position - self.target;
        let spring_force = -self.config.stiffness * displacement;
        let damping_force = -self.config.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.config.mass;
        
        self.velocity += acceleration * dt;
        self.position += self.velocity * dt;
    }
    
    pub fn is_settled(&self) -> bool {
        let displacement = (self.position - self.target).abs();
        let speed = self.velocity.abs();
        displacement < 0.01 && speed < 0.01
    }
}
```

### Preset Configs

```rust
pub mod spring_presets {
    use super::SpringConfig;
    
    /// Snappy UI response
    pub const SNAPPY: SpringConfig = SpringConfig {
        stiffness: 300.0,
        damping: 20.0,
        mass: 1.0,
    };
    
    /// Organic growth feel
    pub const ORGANIC: SpringConfig = SpringConfig {
        stiffness: 180.0,
        damping: 12.0,
        mass: 1.0,
    };
    
    /// Gentle, floaty
    pub const GENTLE: SpringConfig = SpringConfig {
        stiffness: 120.0,
        damping: 14.0,
        mass: 1.0,
    };
    
    /// Bouncy overshoot
    pub const BOUNCY: SpringConfig = SpringConfig {
        stiffness: 200.0,
        damping: 8.0,
        mass: 1.0,
    };
    
    /// Camera movements
    pub const CAMERA: SpringConfig = SpringConfig {
        stiffness: 150.0,
        damping: 18.0,
        mass: 1.0,
    };
}
```

---

## Special Cases

### Ownership Chain (Vertical Growth)

```
Ownership chains grow UPWARD (toward ultimate owner):

     [Ultimate]      ← Appears last (depth 3)
          │
     [Holding Co]    ← Appears second (depth 2)
          │
     [Parent]        ← Appears first (depth 1)
          │
     [Entity]        ← Anchor (doesn't move)
```

**Animation order:** Bottom-up (reverse of typical tree)

### Trading Matrix (Grid Expansion)

```
Trading matrix expands as categorized grid:

[Trading Profile]
       │
       ├─── EQUITY ──────┬──────┬──────┐
       │                 │      │      │
       │              [XNYS] [XLON] [XPAR]
       │
       ├─── FIXED_INCOME ┬──────┐
       │                 │      │
       │              [XNYS] [XLON]
       │
       └─── FX ──────────┐
                         │
                      [OTC]
```

**Animation:** 
- Categories expand first (EQUITY, FIXED_INCOME, FX)
- Markets cascade within each category
- Horizontal layout for markets under each category

### Galaxy Clusters (Radial Growth)

```
Cluster expansion at universe level:

                    ·
                  ·   ·
        Before:    ○     After:    ○ ·
                  LU              / | \
                               ·  ·  ·  (CBU dots)
```

**Animation:**
- CBU dots burst outward from cluster center
- Settle into orbital positions
- Faint lines connect back to cluster center

---

## Implementation Checklist

### Animation System

- [ ] `Spring` struct with `tick()` and `is_settled()`
- [ ] `SpringConfig` presets (ORGANIC, CAMERA, etc.)
- [ ] `NodeAnimation` with growth phases
- [ ] `EdgeAnimation` with extension tracking
- [ ] Cascade trigger system

### Layout System

- [ ] `TreeLayout` with reserved space calculation
- [ ] `RadialLayout` for galaxy/cluster views
- [ ] Anchor-stable expansion (parent doesn't move)
- [ ] Sibling shift animation

### Rendering System

- [ ] Node rendering with scale/alpha animation
- [ ] Edge rendering with progressive extension
- [ ] Label fade-in timing
- [ ] Depth-sorted rendering

### Camera System

- [ ] Anticipatory pan (before growth completes)
- [ ] Bounds calculation for expanded state
- [ ] Smooth zoom during expansion
- [ ] Focus lock on anchor node

---

## Visual Reference

### Growth Curve

```
Progress vs Visual State:

1.0 ─────────────────────────────────╮
                                     │ ← Settled
0.9 ─────────────────────────────╮   │
                                 │   │ ← Label visible
0.7 ─────────────────────────╮   │   │
                             │   │   │
0.5 ─────────────────────╮   │   │   │ ← Edge reaches child
                         │   │   │   │
0.3 ─────────────────╮   │   │   │   │ ← Node 30% size
                     │   │   │   │   │
0.1 ─────────────╮   │   │   │   │   │ ← Dot appears
                 │   │   │   │   │   │
0.0 ─────────────┼───┼───┼───┼───┼───┼─────→ Time
                50  150 200 350 450 500ms
                 │   │   │   │   │
               Bud  │  Edge │  Label
                 Sprout  Unfurl Settle
```

### Collapse Curve (Faster)

```
1.0 ╮
    │ ← Start: label fades
0.8 ┼───╮
         │ ← Shrinking
0.4 ─────┼───╮
              │ ← Edge retracts
0.1 ──────────┼───╮
                   │ ← Absorbed
0.0 ───────────────┴────→ Time
    0   50   150  250  300ms
```
