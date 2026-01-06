# Animation & Physics

Spring-based animation system for smooth 60fps navigation.

## Core Principle

**Update physics BEFORE render. Widget reads, never mutates.**

```rust
// In app.rs update loop
fn update(&mut self, ctx: &egui::Context) {
    let dt = ctx.input(|i| i.stable_dt);
    
    // 1. Physics FIRST
    self.navigation_service.tick(dt);
    
    // 2. Then render
    self.render(ctx);
}
```

## Existing Springs (REUSE THESE)

Located in `ob-poc-graph/src/graph/animation.rs`:

```rust
pub struct SpringF32 {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
    // ... config
}

pub struct SpringVec2 {
    pub value: Vec2,
    pub velocity: Vec2,
    pub target: Vec2,
}

impl SpringF32 {
    pub fn tick(&mut self, dt: f32);
    pub fn set_target(&mut self, target: f32);
    pub fn is_settled(&self) -> bool;
}
```

## Spring Configurations

```rust
// Snappy - UI response, quick settle
pub const SPRING_SNAPPY: SpringConfig = SpringConfig {
    stiffness: 300.0,
    damping: 20.0,
    mass: 1.0,
};

// Organic - node growth, feels alive
pub const SPRING_ORGANIC: SpringConfig = SpringConfig {
    stiffness: 180.0,
    damping: 12.0,
    mass: 1.0,
};

// Gentle - level transitions, smooth fly-through
pub const SPRING_GENTLE: SpringConfig = SpringConfig {
    stiffness: 120.0,
    damping: 14.0,
    mass: 1.0,
};

// Camera - responsive but not jarring
pub const SPRING_CAMERA: SpringConfig = SpringConfig {
    stiffness: 150.0,
    damping: 18.0,
    mass: 1.0,
};
```

## Animation Timing Reference

| Animation | Duration | Spring | Notes |
|-----------|----------|--------|-------|
| Node growth (expand) | 500ms | ORGANIC | Bud → Sprout → Unfurl → Settle |
| Node collapse | 300ms | SNAPPY | Faster than expand |
| Edge extend | 200ms | ORGANIC | Grows from parent |
| Edge retract | 150ms | SNAPPY | Faster |
| Label fade in | 100ms | ease-in | After node at 70% |
| Label fade out | 50ms | ease-out | First thing to go |
| Camera pan | 400ms | CAMERA | Leads movement |
| Camera zoom | 300ms | CAMERA | Smooth |
| Level transition | 600ms | GENTLE | Full dive/surface |
| Hover highlight | 150ms | ease-out | Quick response |
| Anomaly pulse | 2000ms | sine | Continuous loop |

## Animation Phases (Node Growth)

```rust
pub enum AnimationPhase {
    Hidden,       // 0%: invisible
    Budding,      // 0-20%: dot appears
    Sprouting,    // 20-50%: growing
    Unfurling,    // 50-80%: reaching full size
    Settling,     // 80-100%: micro-adjustments
    Visible,      // 100%: stable
    Collapsing,   // Reverse (faster)
}
```

## Cascade Timing

Children don't all appear at once. They cascade:

```rust
const CASCADE_THRESHOLD: f32 = 0.6;  // 60% parent completion
const CASCADE_DELAY_MS: f32 = 150.0;  // Stagger between siblings

// Parent at 60% → first child starts
// First child at 60% → second child starts
// Creates "domino" effect
```

## Camera Lead

Camera arrives BEFORE content:

```rust
impl NavigationService {
    fn update_camera(&mut self, dt: f32) {
        // Camera leads by 30%
        let lead_target = self.position + self.velocity * 0.3;
        self.camera.target_spring.set_target(lead_target);
        self.camera.target_spring.tick(dt);
    }
}
```

## Common Patterns

### Animating a Value

```rust
// Setup
let mut zoom = SpringF32::new(1.0, SPRING_CAMERA);

// On navigation
zoom.set_target(0.5);  // Zoom out

// Every frame (in tick, not ui)
zoom.tick(dt);

// In render (read only)
let current_zoom = zoom.value;
```

### Coordinated Animation

```rust
pub struct NodeAnimation {
    growth: SpringF32,      // 0.0 to 1.0
    position: SpringVec2,
    opacity: SpringF32,
    phase: AnimationPhase,
}

impl NodeAnimation {
    pub fn tick(&mut self, dt: f32) {
        self.growth.tick(dt);
        self.position.tick(dt);
        self.opacity.tick(dt);
        self.update_phase();
    }
    
    fn update_phase(&mut self) {
        self.phase = match self.growth.value {
            v if v < 0.01 => AnimationPhase::Hidden,
            v if v < 0.20 => AnimationPhase::Budding,
            v if v < 0.50 => AnimationPhase::Sprouting,
            v if v < 0.80 => AnimationPhase::Unfurling,
            v if v < 0.99 => AnimationPhase::Settling,
            _ => AnimationPhase::Visible,
        };
    }
}
```

## Key Files

| File | Contents |
|------|----------|
| `ob-poc-graph/src/graph/animation.rs` | SpringF32, SpringVec2 |
| `ob-poc-graph/src/graph/camera.rs` | Camera2D with fly_to |
| `ob-poc-ui/src/navigation_service.rs` | Owns all animation state |

## Reference

See TODO-GALAXY-NAVIGATION-SYSTEM.md:
- Appendix A: Animation Timing Quick Reference
- Appendix B: Spring Configurations
- Part 4.4: Physics Parameters
