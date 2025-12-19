# Plan: Make CBU Graph Visualization POP

## Goal
Transform the functional graph visualization into a "wow factor" experience that impresses users/punters. The visualization is the primary user touchpoint.

## Current State
- Solid foundation: spring physics, camera animations, LOD system, bezier edges
- Good semantics: colors encode meaning (risk, status, verification)
- Missing: visual polish that creates the "alive" feeling

## High-Impact Visual Enhancements

### Phase 1: Depth & Glow (Immediate Visual Impact)

**1. Drop Shadows on Nodes** (~30 min)
- Render a darker, offset, blurred rectangle behind each node
- Scale shadow with node importance (CBU root = larger shadow)
- Creates depth perception - nodes "float" above canvas

**2. Glow Effect on Focused Node** (~30 min)
- Multiple concentric rectangles with decreasing opacity
- Pulsing animation (subtle scale oscillation using sin wave)
- Blue glow for selection, matches existing focus ring color

**3. Hover Glow on Nodes** (~45 min)
- Track hovered node in InputState
- Apply warm glow (amber/white) on hover
- Slightly scale up node (1.0 → 1.05) with SpringF32

### Phase 2: Motion & Life (Feel Alive)

**4. Attention Badge Pulse** (~20 min)
- Animate the red "!" badge opacity with sine wave
- Subtle scale pulse (0.9 → 1.1) 
- Draws eye to nodes needing action

**5. Smooth Opacity Transitions** (~45 min)
- Replace instant blur_opacity (0.25 ↔ 1.0) with SpringF32
- Fade duration ~0.2s when focus changes
- Feels smoother, less jarring

**6. Connection Highlighting on Hover** (~30 min)
- When hovering a node, brighten its edges
- Dim all other edges to blur_opacity
- Shows relationships instantly

### Phase 3: Entrance Animations (Polish)

**7. Node Entrance Fade/Scale** (~1 hour)
- Track which nodes are "new" (just appeared)
- Animate opacity 0→1 and scale 0.8→1.0
- Stagger by depth from CBU root
- Graph feels dynamic, not static

## Files to Modify

| File | Changes |
|------|---------|
| `ob-poc-graph/src/graph/render.rs` | Add shadow rendering, glow effects |
| `ob-poc-graph/src/graph/lod.rs` | Add glow to focused node, pulse to attention badge |
| `ob-poc-graph/src/graph/input.rs` | Track hovered_node_id for hover effects |
| `ob-poc-graph/src/graph/types.rs` | Add entrance_time to LayoutNode for animations |
| `ob-poc-graph/src/graph/mod.rs` | Wire up animation state, time tracking |
| `ob-poc-graph/src/widget.rs` | Pass frame time to renderer |

## Implementation Order

1. **Drop shadows** - Biggest visual bang, simplest code
2. **Hover tracking + glow** - Makes it feel interactive
3. **Focus glow + pulse** - Emphasizes selection
4. **Attention badge pulse** - Draws eye to action items
5. **Smooth opacity transitions** - Polish
6. **Connection highlighting** - Relationship context
7. **Entrance animations** - Final flourish

## Technical Approach

### Shadow Rendering
```rust
// In render_node, before main rect:
let shadow_offset = Vec2::new(4.0, 6.0) * camera.zoom();
let shadow_rect = node_rect.translate(shadow_offset);
let shadow_color = Color32::from_rgba_unmultiplied(0, 0, 0, 40);
painter.rect_filled(shadow_rect, corner_radius + 2.0, shadow_color);
```

### Glow Effect
```rust
// Multiple passes with decreasing opacity
for i in (1..=3).rev() {
    let glow_size = node_rect.expand(i as f32 * 4.0 * camera.zoom());
    let alpha = 30 / i;
    let glow_color = Color32::from_rgba_unmultiplied(59, 130, 246, alpha as u8);
    painter.rect_filled(glow_size, corner_radius + i as f32 * 2.0, glow_color);
}
```

### Pulse Animation
```rust
// Time-based oscillation
let pulse = 0.9 + 0.2 * (time * 3.0).sin(); // 0.9 to 1.1, 3Hz
let badge_radius = base_radius * pulse;
```

## Success Criteria
- First impression: "That looks professional"
- Interactions feel responsive and alive
- Focus/selection is immediately clear
- Relationships readable at a glance
- No performance regression (LOD still works)

## Estimated Time
- Phase 1: ~1.5 hours
- Phase 2: ~1.5 hours  
- Phase 3: ~1 hour
- **Total: ~4 hours**

---

## Questions for Review

1. **Color scheme preference?** The plan uses blue glow for focus (matches current), amber for hover. Any preference for different accent colors?

2. **Animation intensity?** Should effects be subtle/professional or more bold/eye-catching?

3. **Priority order?** The plan prioritizes depth/shadows first. Would you prefer to start with hover interactions or entrance animations instead?

4. **Performance budget?** Any concerns about animation overhead in WASM? (LOD system should protect us, but worth confirming)
