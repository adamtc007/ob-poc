# Phase 7: Viewport Scaling + Resolution Handling

## Overview

Same content looks appropriate on 13" laptop and 60" 4K monitor.

---

## Two Concerns

### 1. DPI Scaling (egui handles this)

```rust
// egui gives logical points, not pixels
let scale = ctx.pixels_per_point();  // 1.0, 1.5, 2.0

// Work in points - egui does pixel math
let node_radius = 40.0;  // 40pt = 40px @1x, 80px @2x
```

**No action needed** - just use logical points everywhere.

### 2. Viewport Real Estate (our problem)

```rust
let viewport = ctx.available_rect();
// 13" laptop: 1200 x 700 points
// 60" 4K:     2400 x 1400 points (scaled)
```

Same CBU set needs to render sensibly on both.

---

## Scaling Rules

| Thing | How It Scales |
|-------|---------------|
| Node radius | Fixed points (LOD handles density) |
| Font size | Fixed points (system DPI handles it) |
| Force sim boundary | `= viewport.size()` |
| Auto-zoom | `= viewport / content_bounds` |
| LOD thresholds | Viewport-aware density |
| Pan/zoom speed | Could scale with viewport |

---

## Viewport-Aware LOD

```rust
impl NodeLOD {
    pub fn for_viewport(
        compression: f32,
        node_count: usize,
        viewport: Rect,
    ) -> Self {
        let area = viewport.width() * viewport.height();
        let density = node_count as f32 / area * 10000.0;  // per 10k pt²
        
        // High density = collapse earlier
        let density_penalty = (density / 5.0).min(0.3);
        let effective = compression + density_penalty;
        
        match effective {
            c if c > 0.8 => NodeLOD::Icon,
            c if c > 0.5 => NodeLOD::Label,
            c if c > 0.2 => NodeLOD::Full,
            _ => NodeLOD::Expanded,
        }
    }
}
```

**Effect:**
- 50 nodes on 13" laptop → mostly Labels
- 50 nodes on 60" 4K → mostly Full
- Same data, appropriate density

---

## Force Sim Boundary

```rust
impl ForceSimulation {
    pub fn set_viewport(&mut self, viewport: Rect) {
        self.center = viewport.center();
        self.config.boundary_radius = viewport.width().min(viewport.height()) * 0.45;
    }
}
```

Boundary scales with viewport - nodes use available space.

---

## Auto-Fit (from Phase 5)

```rust
impl ViewportFit {
    pub fn compute(viewport: Rect, clusters: &[Cluster]) -> f32 {
        let bounds = content_bounds(clusters);
        
        let zoom_x = viewport.width() / bounds.width();
        let zoom_y = viewport.height() / bounds.height();
        
        zoom_x.min(zoom_y) * 0.9  // 10% margin
    }
}
```

- 13" laptop: auto-zoom 0.3 → content fits, more collapsed
- 60" 4K: auto-zoom 0.8 → content fits, more expanded

---

## Resize Handling

```rust
fn on_viewport_resize(&mut self, new_viewport: Rect) {
    // Update force sim boundary
    self.force_sim.set_viewport(new_viewport);
    
    // Recalc auto-fit if enabled
    if self.auto_fit_enabled {
        self.camera.target_zoom = ViewportFit::compute(new_viewport, &self.clusters);
    }
    
    // LOD will recalc on next render (uses viewport)
}
```

---

## Implementation

### Batch 1: Force Sim Boundary (1h)
- [ ] Add `set_viewport()` to ForceSimulation
- [ ] Call on resize
- [ ] Boundary = min(width, height) * 0.45

### Batch 2: Viewport-Aware LOD (1h)
- [ ] Pass viewport to `NodeLOD::for_viewport()`
- [ ] Density penalty calculation
- [ ] Test on different window sizes

### Batch 3: Resize Handler (30m)
- [ ] Wire egui resize event
- [ ] Update force sim + auto-fit
- [ ] Smooth transition (animate zoom change)

---

## Success Criteria

- [ ] Resize window → content reflows smoothly
- [ ] Same 50 CBUs: laptop shows Labels, 4K shows Full
- [ ] Force sim uses available space (no wasted margins)
- [ ] No hardcoded pixel values anywhere
- [ ] Works on 13" @1x, 15" @2x, 27" @1.5x, 60" 4K

---

## Files

| File | Action |
|------|--------|
| `ob-poc-graph/src/graph/force_sim.rs` | Add `set_viewport()` |
| `ob-poc-graph/src/graph/lod.rs` | Viewport-aware density |
| `ob-poc-ui/src/app.rs` | Resize handler |

---

## Total Effort: ~2.5h
