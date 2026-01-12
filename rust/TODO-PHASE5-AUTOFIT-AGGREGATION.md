# Phase 5: Auto-Fit + Galaxy Aggregation

## Overview

RIP automatically computes optimal view. User can override, reset returns to auto.

---

## Auto-Fit Logic

```rust
pub struct ViewportFit {
    pub auto_enabled: bool,      // User hasn't overridden
    pub content_bounds: Rect,    // Bounding box of all clusters
    pub optimal_zoom: f32,       // Max detail that fits
}

impl ViewportFit {
    pub fn compute(viewport_size: Vec2, clusters: &[Cluster]) -> Self {
        let bounds = Self::content_bounds(clusters);
        
        let zoom_x = viewport_size.x / bounds.width();
        let zoom_y = viewport_size.y / bounds.height();
        let optimal = zoom_x.min(zoom_y) * 0.9; // 10% margin
        
        Self {
            auto_enabled: true,
            content_bounds: bounds,
            optimal_zoom: optimal,
        }
    }
}
```

---

## Galaxy Aggregation (Cap Strategy)

When content exceeds threshold, aggregate to galaxies:

```rust
pub const MAX_VISIBLE_CLUSTERS: usize = 50;
pub const MAX_VISIBLE_NODES: usize = 200;

pub enum ViewLevel {
    Galaxy,      // Jurisdiction bubbles: LU(47), DE(23), US(89)
    Region,      // ManCo clusters within jurisdiction
    Cluster,     // Individual CBUs
    Solar,       // CBU internals (fund + services)
}

pub fn determine_view_level(total_cbus: usize, viewport_zoom: f32) -> ViewLevel {
    match (total_cbus, viewport_zoom) {
        (n, z) if n > 100 && z < 0.3 => ViewLevel::Galaxy,
        (n, z) if n > 30 && z < 0.5  => ViewLevel::Region,
        (_, z) if z < 0.8            => ViewLevel::Cluster,
        _                            => ViewLevel::Solar,
    }
}
```

**Aggregation rules:**

```rust
/// Collapse CBUs into jurisdiction galaxies
fn aggregate_to_galaxies(clusters: &[Cluster]) -> Vec<GalaxyNode> {
    let mut by_jurisdiction: HashMap<String, Vec<&Cluster>> = HashMap::new();
    
    for c in clusters {
        let jur = c.jurisdiction.clone().unwrap_or("XX".into());
        by_jurisdiction.entry(jur).or_default().push(c);
    }
    
    by_jurisdiction.iter().map(|(jur, cbus)| {
        GalaxyNode {
            id: format!("galaxy-{}", jur),
            label: jur.clone(),
            count: cbus.len(),
            children: cbus.iter().map(|c| c.id).collect(),
        }
    }).collect()
}
```

**Visual:**

```
500 CBUs loaded, zoom 0.2:
┌─────────────────────────────────────┐
│     ◉         ◉                     │
│    LU(47)   DE(23)     ◉            │
│                       US(89)        │
│         ◉        ◉                  │
│       IE(12)   UK(34)               │
└─────────────────────────────────────┘
Galaxy view - 5 nodes rendered, not 500

Zoom into LU:
┌─────────────────────────────────────┐
│   ◉ Allianz(8)    ◉ DWS(6)         │
│                                     │
│      ◉ BlackRock(5)   ◉ Other(28)  │
└─────────────────────────────────────┘
Region view - ManCo clusters

Zoom into Allianz:
┌─────────────────────────────────────┐
│  [Allianz Lux 1]  [Allianz Lux 2]  │
│  [Allianz Lux 3]  [Allianz Lux 4]  │
└─────────────────────────────────────┘
Cluster view - individual CBUs
```

---

## Recalc Triggers

```rust
fn on_content_change(&mut self) {
    self.fit.content_bounds = ViewportFit::content_bounds(&self.clusters);
    if self.fit.auto_enabled {
        self.camera.target_zoom = self.fit.optimal_zoom;
    }
    self.view_level = determine_view_level(self.clusters.len(), self.camera.zoom);
}

fn on_viewport_resize(&mut self, new_size: Vec2) {
    self.viewport_size = new_size;
    self.fit = ViewportFit::compute(new_size, &self.clusters);
    if self.fit.auto_enabled {
        self.camera.target_zoom = self.fit.optimal_zoom;
    }
}

fn on_user_zoom(&mut self, delta: f32) {
    self.fit.auto_enabled = false;  // User takes control
    self.camera.target_zoom *= delta;
    self.view_level = determine_view_level(self.clusters.len(), self.camera.target_zoom);
}

fn on_reset(&mut self) {
    self.fit.auto_enabled = true;
    self.camera.target_zoom = self.fit.optimal_zoom;
    self.camera.target_center = Vec2::ZERO;
}
```

---

## Implementation Order

### Batch 1: Auto-Fit (2h)
- [ ] Add `ViewportFit` struct
- [ ] `content_bounds()` from clusters
- [ ] `optimal_zoom` calculation
- [ ] Wire to resize + content change events

### Batch 2: View Level (2h)
- [ ] Add `ViewLevel` enum
- [ ] `determine_view_level()` function
- [ ] Wire zoom changes to view level recalc

### Batch 3: Galaxy Aggregation (4h)
- [ ] `aggregate_to_galaxies()` - group by jurisdiction
- [ ] `aggregate_to_regions()` - group by ManCo
- [ ] Render aggregated nodes when ViewLevel::Galaxy/Region
- [ ] Click galaxy → zoom into that jurisdiction

### Batch 4: Auto/Manual Toggle (1h)
- [ ] `auto_enabled` flag
- [ ] User zoom/pan disables auto
- [ ] "reset" command re-enables auto

---

## Success Criteria

- [ ] Load 500 CBUs → see ~5 jurisdiction galaxies
- [ ] Zoom into LU → see ManCo clusters
- [ ] Zoom into Allianz → see individual CBUs
- [ ] Resize window → content auto-fits
- [ ] Manual zoom → auto-fit disabled
- [ ] "reset" → back to auto-fit
- [ ] Never render > 200 nodes (aggregation kicks in)

---

## Files

| File | Action |
|------|--------|
| `ob-poc-graph/src/graph/viewport_fit.rs` | NEW - auto-fit logic |
| `ob-poc-graph/src/graph/aggregation.rs` | NEW - galaxy/region collapse |
| `ob-poc-types/src/lib.rs` | ADD `ViewLevel`, `GalaxyNode` |
| `ob-poc-ui/src/app.rs` | Wire resize, zoom, reset handlers |
