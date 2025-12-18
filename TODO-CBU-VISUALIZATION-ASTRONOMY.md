# TODO: CBU Visualization - Astronomy View

## ⛔ MANDATORY FIRST STEP

**Read these files first:**
- `/EGUI-RULES.md` - Non-negotiable UI patterns
- `/TODO-CBU-VISUALIZATION-ANIMATION.md` - Animation foundation (build this first)
- `/docs/CBU_UNIVERSE_VISUALIZATION_SPEC.md` - Original vision doc

**Dependencies:** This builds ON TOP of the animation engine (springs, camera, gestures).

---

## Overview

The Astronomy view treats CBU data as a **spatial universe**:
- CBUs are stars
- Jurisdictions are galaxies/clusters  
- Relationships are gravitational forces
- Status is encoded in visual properties (color, size, brightness)

Two zoom levels:
1. **Universe View** - All CBUs as a galaxy
2. **Solar System View** - Single CBU with orbiting entities

---

## Part 1: Universe View

### 1.1 Concept

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│                           CBU UNIVERSE                                      │
│                                                                             │
│         EUROPE                    AMERICAS                 APAC             │
│        ┌──────────┐              ┌──────────┐           ┌──────────┐       │
│        │  · ·  ·  │              │    ·     │           │   · ·    │       │
│        │ ·    ·   │              │  · ● ·   │           │  ·   ·   │       │
│        │  · ·  ·  │              │    ·     │           │    ·     │       │
│        └──────────┘              └──────────┘           └──────────┘       │
│              ↑                         ↑                      ↑            │
│         Cluster anchor           Selected CBU            Smaller cluster   │
│                                  (highlighted)                             │
│                                                                             │
│   Node encoding:                                                            │
│   • Position → Jurisdiction cluster + force repulsion                       │
│   • Size → AUM / economic weight (log scale)                               │
│   • Color → KYC risk rating (green → amber → red)                          │
│   • Brightness → Activity level (recent changes)                           │
│   • Pulse → Onboarding in progress                                         │
│   • Ring → Client type (fund, corporate, individual)                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Force-Directed Simulation

```rust
//! Force simulation for CBU universe layout
//! 
//! Each CBU experiences forces:
//! 1. Attraction to jurisdiction cluster anchor
//! 2. Repulsion from other CBUs
//! 3. Weak center gravity (keeps universe bounded)

pub struct UniverseSimulation {
    /// Cluster anchor positions (jurisdiction → position)
    cluster_anchors: HashMap<String, Vec2>,
    
    /// Per-CBU simulation state
    bodies: HashMap<Uuid, SimBody>,
    
    /// Simulation parameters
    config: SimConfig,
    
    /// Current simulation temperature (cools over time)
    alpha: f32,
}

pub struct SimBody {
    pub cbu_id: Uuid,
    pub position: Vec2,
    pub velocity: Vec2,
    pub mass: f32,           // Based on AUM
    pub jurisdiction: String, // Which cluster to attract to
}

pub struct SimConfig {
    /// Repulsion strength between CBUs
    pub repulsion_strength: f32,      // Default: 500.0
    /// How far repulsion reaches
    pub repulsion_distance: f32,      // Default: 200.0
    /// Attraction to cluster anchor
    pub cluster_attraction: f32,      // Default: 0.1
    /// Weak pull toward center
    pub center_gravity: f32,          // Default: 0.01
    /// Velocity damping (0.0 - 1.0)
    pub damping: f32,                 // Default: 0.9
    /// Cooling rate
    pub alpha_decay: f32,             // Default: 0.02
    /// Minimum alpha (simulation stops)
    pub alpha_min: f32,               // Default: 0.001
}

impl UniverseSimulation {
    /// Run one simulation tick
    pub fn tick(&mut self, dt: f32) {
        if self.alpha < self.config.alpha_min {
            return; // Simulation settled
        }
        
        // Calculate forces
        let mut forces: HashMap<Uuid, Vec2> = HashMap::new();
        
        let bodies: Vec<_> = self.bodies.values().collect();
        
        for (i, body_a) in bodies.iter().enumerate() {
            let mut force = Vec2::ZERO;
            
            // 1. Repulsion from other CBUs
            for (j, body_b) in bodies.iter().enumerate() {
                if i == j { continue; }
                
                let delta = body_a.position - body_b.position;
                let distance = delta.length().max(1.0);
                
                if distance < self.config.repulsion_distance {
                    let strength = self.config.repulsion_strength / (distance * distance);
                    force += delta.normalize() * strength;
                }
            }
            
            // 2. Attraction to cluster anchor
            if let Some(anchor) = self.cluster_anchors.get(&body_a.jurisdiction) {
                let delta = *anchor - body_a.position;
                force += delta * self.config.cluster_attraction;
            }
            
            // 3. Center gravity
            force += -body_a.position * self.config.center_gravity;
            
            forces.insert(body_a.cbu_id, force);
        }
        
        // Apply forces
        for body in self.bodies.values_mut() {
            if let Some(force) = forces.get(&body.cbu_id) {
                // F = ma, so a = F/m
                let acceleration = *force / body.mass.max(1.0);
                body.velocity += acceleration * dt * self.alpha;
                body.velocity *= self.config.damping;
                body.position += body.velocity * dt;
            }
        }
        
        // Cool down
        self.alpha *= 1.0 - self.config.alpha_decay;
    }
    
    /// Reheat simulation (e.g., when data changes)
    pub fn reheat(&mut self) {
        self.alpha = 1.0;
    }
    
    /// Is simulation still running?
    pub fn is_active(&self) -> bool {
        self.alpha >= self.config.alpha_min
    }
}
```

### 1.3 Cluster Anchor Layout

```rust
/// Arrange jurisdiction clusters in a pleasing layout
/// Roughly follows world geography
pub fn compute_cluster_anchors(jurisdictions: &[String], canvas_size: Vec2) -> HashMap<String, Vec2> {
    // Predefined positions for common jurisdictions (normalized 0-1)
    let predefined: HashMap<&str, (f32, f32)> = [
        // Americas (left)
        ("US", (0.15, 0.4)),
        ("CA", (0.15, 0.25)),
        ("KY", (0.2, 0.55)),   // Cayman
        ("BVI", (0.22, 0.5)),  // British Virgin Islands
        ("BR", (0.25, 0.7)),   // Brazil
        
        // Europe (center-left)
        ("UK", (0.4, 0.3)),
        ("IE", (0.38, 0.28)),  // Ireland
        ("LU", (0.45, 0.35)),  // Luxembourg
        ("DE", (0.48, 0.32)),  // Germany
        ("FR", (0.43, 0.4)),   // France
        ("CH", (0.47, 0.4)),   // Switzerland
        ("NL", (0.45, 0.28)),  // Netherlands
        
        // Middle East
        ("AE", (0.6, 0.5)),    // UAE
        ("SA", (0.58, 0.55)),  // Saudi
        
        // Asia Pacific (right)
        ("SG", (0.75, 0.55)),  // Singapore
        ("HK", (0.8, 0.45)),   // Hong Kong
        ("JP", (0.85, 0.35)),  // Japan
        ("AU", (0.85, 0.75)),  // Australia
        ("CN", (0.78, 0.4)),   // China
    ].into_iter().collect();
    
    let mut anchors = HashMap::new();
    let mut used_positions: Vec<Vec2> = Vec::new();
    
    for jurisdiction in jurisdictions {
        let pos = if let Some(&(x, y)) = predefined.get(jurisdiction.as_str()) {
            Vec2::new(x * canvas_size.x, y * canvas_size.y)
        } else {
            // Unknown jurisdiction: find empty spot
            find_empty_position(&used_positions, canvas_size)
        };
        
        used_positions.push(pos);
        anchors.insert(jurisdiction.clone(), pos);
    }
    
    anchors
}
```

### 1.4 CBU Node Rendering (Universe Level)

```rust
impl UniverseRenderer {
    /// Render a single CBU as a star
    fn render_cbu_star(&self, ui: &mut egui::Ui, cbu: &CbuSummary, pos: Vec2, zoom: f32) {
        // Size based on AUM (log scale)
        let base_size = 5.0;
        let aum_factor = (cbu.aum.max(1.0) as f32).log10() / 12.0; // Normalize to ~0-1
        let size = base_size + aum_factor * 15.0;
        let screen_size = size * zoom;
        
        // Skip tiny nodes at low zoom
        if screen_size < 2.0 {
            return;
        }
        
        // Color based on risk rating
        let color = match cbu.risk_rating.as_str() {
            "STANDARD" | "LOW" => egui::Color32::from_rgb(76, 175, 80),   // Green
            "MEDIUM" => egui::Color32::from_rgb(255, 193, 7),            // Amber
            "HIGH" => egui::Color32::from_rgb(255, 87, 34),              // Deep orange
            "PROHIBITED" => egui::Color32::from_rgb(33, 33, 33),         // Near black
            _ => egui::Color32::from_rgb(158, 158, 158),                 // Grey (unrated)
        };
        
        // Brightness based on activity
        let brightness = 0.5 + cbu.activity_score * 0.5;
        let bright_color = brighten(color, brightness);
        
        // Draw glow (for active CBUs)
        if cbu.activity_score > 0.5 {
            let glow_size = screen_size * 2.0;
            let glow_color = color.linear_multiply(0.3);
            ui.painter().circle_filled(
                egui::pos2(pos.x, pos.y),
                glow_size,
                glow_color,
            );
        }
        
        // Draw star
        ui.painter().circle_filled(
            egui::pos2(pos.x, pos.y),
            screen_size,
            bright_color,
        );
        
        // Onboarding pulse animation
        if cbu.is_onboarding {
            let pulse = (ui.input(|i| i.time) * 2.0).sin() as f32 * 0.5 + 0.5;
            let pulse_size = screen_size * (1.2 + pulse * 0.3);
            ui.painter().circle_stroke(
                egui::pos2(pos.x, pos.y),
                pulse_size,
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100, 180, 255, (pulse * 200.0) as u8)),
            );
        }
        
        // Client type ring
        if screen_size > 8.0 {
            let ring_style = match cbu.client_type.as_str() {
                "FUND_MANDATE" => (egui::Color32::from_rgb(66, 165, 245), false),  // Solid blue
                "CORPORATE_GROUP" => (egui::Color32::from_rgb(171, 71, 188), true), // Dashed purple
                "INDIVIDUAL" => (egui::Color32::from_rgb(255, 167, 38), false),    // Solid orange
                _ => (egui::Color32::GRAY, true),
            };
            
            ui.painter().circle_stroke(
                egui::pos2(pos.x, pos.y),
                screen_size + 3.0,
                egui::Stroke::new(1.5, ring_style.0),
            );
        }
        
        // Label (only at higher zoom)
        if screen_size > 15.0 {
            ui.painter().text(
                egui::pos2(pos.x, pos.y + screen_size + 8.0),
                egui::Align2::CENTER_TOP,
                &cbu.name,
                egui::FontId::proportional(10.0 * zoom.min(1.5)),
                egui::Color32::WHITE,
            );
        }
    }
}
```

### 1.5 Tasks - Universe View

- [ ] Create `UniverseSimulation` struct with force-directed physics
- [ ] Implement cluster anchor positioning (geography-based)
- [ ] Add repulsion, attraction, and center gravity forces
- [ ] Implement simulation cooling and reheating
- [ ] Create `CbuSummary` struct (lightweight, for universe view)
- [ ] Add universe-level API endpoint (`/api/cbus/universe`)
- [ ] Render CBUs as colored dots with size encoding
- [ ] Add risk rating color palette
- [ ] Add activity glow effect
- [ ] Add onboarding pulse animation
- [ ] Add client type ring indicators
- [ ] Implement zoom-dependent label visibility
- [ ] Add cluster label overlays
- [ ] Implement CBU selection (click to highlight)
- [ ] Add tooltip on hover (CBU name, jurisdiction, status)

---

## Part 2: Solar System View

### 2.1 Concept

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│                        CBU SOLAR SYSTEM                                     │
│                                                                             │
│                              ╭───╮                                          │
│                              │UBO│ ← Outer orbit: Beneficial owners         │
│          ╭───╮               ╰─┬─╯               ╭───╮                      │
│          │UBO│                 │                 │UBO│                      │
│          ╰─┬─╯     ╭───────────┴───────────╮     ╰─┬─╯                      │
│            │       │      ENTITIES         │       │                        │
│            │       │   (ManCo, HoldCo)     │       │   ← Middle orbit       │
│            │       ╰───────────┬───────────╯       │                        │
│            │                   │                   │                        │
│            │       ╭───────────┴───────────╮       │                        │
│            │       │                       │       │                        │
│            │   ╭───┴───╮             ╭─────┴───╮   │                        │
│            │   │       │             │         │   │                        │
│            └───┤  ☀️   │─────────────│   ☀️    ├───┘                        │
│                │  CBU  │             │  (alt)  │                            │
│            ┌───┤       │             │         ├───┐                        │
│            │   ╰───┬───╯             ╰────┬────╯   │                        │
│            │       │                      │        │                        │
│            │       ╰──────────┬───────────╯        │                        │
│            │                  │                    │                        │
│        ┌───┴───┐          ┌───┴───┐          ┌────┴────┐                   │
│        │Prod 1 │          │Prod 2 │          │ Prod 3  │  ← Inner orbit    │
│        │       │          │       │          │         │    (Products)     │
│        └───┬───┘          └───┬───┘          └────┬────┘                   │
│            │                  │                   │                        │
│         ┌──┴──┐            ┌──┴──┐            ┌───┴───┐                    │
│         │moon │            │moon │            │ moon  │   ← Moons          │
│         │(svc)│            │(svc)│            │ (svc) │     (Services)     │
│         └─────┘            └─────┘            └───────┘                    │
│                                                                             │
│   Orbital mechanics:                                                        │
│   • Distance from center = Relationship closeness                           │
│   • Orbital position = Arbitrary (spread evenly)                           │
│   • Orbital speed = Activity level (recent changes)                        │
│   • Size = Importance / completeness                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Orbital Layout

```rust
/// Orbital ring in the solar system
pub struct Orbit {
    pub radius: f32,
    pub bodies: Vec<OrbitalBody>,
    pub rotation_speed: f32,  // Radians per second
    pub layer: OrbitLayer,
}

pub enum OrbitLayer {
    Inner,      // Products
    Middle,     // Entities (ManCo, custodian, etc.)
    Outer,      // UBOs, control persons
}

pub struct OrbitalBody {
    pub node_id: String,
    pub angle: f32,           // Current angle in radians
    pub angular_velocity: f32, // Override for individual speed
    pub size: f32,
    pub moons: Vec<Moon>,     // Sub-orbiting bodies (services around products)
}

pub struct Moon {
    pub node_id: String,
    pub orbit_radius: f32,
    pub angle: f32,
    pub size: f32,
}

impl Orbit {
    /// Distribute bodies evenly around orbit
    pub fn distribute_evenly(&mut self) {
        let count = self.bodies.len();
        if count == 0 { return; }
        
        let angle_step = std::f32::consts::TAU / count as f32;
        for (i, body) in self.bodies.iter_mut().enumerate() {
            body.angle = i as f32 * angle_step;
        }
    }
    
    /// Update orbital positions
    pub fn tick(&mut self, dt: f32) {
        for body in &mut self.bodies {
            let speed = body.angular_velocity + self.rotation_speed;
            body.angle += speed * dt;
            body.angle %= std::f32::consts::TAU;
            
            // Update moons
            for moon in &mut body.moons {
                moon.angle += speed * 2.0 * dt;  // Moons orbit faster
                moon.angle %= std::f32::consts::TAU;
            }
        }
    }
    
    /// Get world position of a body
    pub fn body_position(&self, body: &OrbitalBody, center: Vec2) -> Vec2 {
        Vec2::new(
            center.x + self.radius * body.angle.cos(),
            center.y + self.radius * body.angle.sin(),
        )
    }
    
    /// Get world position of a moon
    pub fn moon_position(&self, body: &OrbitalBody, moon: &Moon, center: Vec2) -> Vec2 {
        let body_pos = self.body_position(body, center);
        Vec2::new(
            body_pos.x + moon.orbit_radius * moon.angle.cos(),
            body_pos.y + moon.orbit_radius * moon.angle.sin(),
        )
    }
}
```

### 2.3 Solar System Builder

```rust
/// Build solar system layout from CBU graph
pub fn build_solar_system(graph: &CbuGraph, center: Vec2) -> SolarSystem {
    let mut system = SolarSystem {
        center,
        sun: graph.nodes.iter().find(|n| n.node_type == NodeType::Cbu).cloned(),
        orbits: vec![],
    };
    
    // Inner orbit: Products
    let products: Vec<_> = graph.nodes.iter()
        .filter(|n| n.node_type == NodeType::Product)
        .collect();
    
    if !products.is_empty() {
        let mut inner_orbit = Orbit {
            radius: 150.0,
            bodies: vec![],
            rotation_speed: 0.1,
            layer: OrbitLayer::Inner,
        };
        
        for product in products {
            // Find services under this product (moons)
            let services: Vec<_> = graph.edges.iter()
                .filter(|e| e.source == product.id && e.edge_type == EdgeType::Delivers)
                .filter_map(|e| graph.nodes.iter().find(|n| n.id == e.target))
                .map(|svc| Moon {
                    node_id: svc.id.clone(),
                    orbit_radius: 30.0,
                    angle: 0.0,
                    size: 10.0,
                })
                .collect();
            
            inner_orbit.bodies.push(OrbitalBody {
                node_id: product.id.clone(),
                angle: 0.0,
                angular_velocity: 0.0,
                size: 25.0,
                moons: services,
            });
        }
        
        inner_orbit.distribute_evenly();
        system.orbits.push(inner_orbit);
    }
    
    // Middle orbit: Entities (trading execution)
    let trading_entities: Vec<_> = graph.nodes.iter()
        .filter(|n| n.node_type == NodeType::Entity)
        .filter(|n| n.role_categories.contains(&"TRADING_EXECUTION".to_string()))
        .collect();
    
    if !trading_entities.is_empty() {
        let mut middle_orbit = Orbit {
            radius: 250.0,
            bodies: trading_entities.iter().map(|e| OrbitalBody {
                node_id: e.id.clone(),
                angle: 0.0,
                angular_velocity: 0.0,
                size: 20.0,
                moons: vec![],
            }).collect(),
            rotation_speed: 0.05,
            layer: OrbitLayer::Middle,
        };
        middle_orbit.distribute_evenly();
        system.orbits.push(middle_orbit);
    }
    
    // Outer orbit: UBOs and ownership entities
    let ownership_entities: Vec<_> = graph.nodes.iter()
        .filter(|n| n.node_type == NodeType::Entity)
        .filter(|n| n.role_categories.contains(&"OWNERSHIP_CONTROL".to_string()) 
                 || n.entity_category.as_deref() == Some("PERSON"))
        .collect();
    
    if !ownership_entities.is_empty() {
        let mut outer_orbit = Orbit {
            radius: 350.0,
            bodies: ownership_entities.iter().map(|e| OrbitalBody {
                node_id: e.id.clone(),
                angle: 0.0,
                angular_velocity: 0.0,
                size: if e.entity_category.as_deref() == Some("PERSON") { 30.0 } else { 20.0 },
                moons: vec![],
            }).collect(),
            rotation_speed: 0.02,
            layer: OrbitLayer::Outer,
        };
        outer_orbit.distribute_evenly();
        system.orbits.push(outer_orbit);
    }
    
    system
}
```

### 2.4 Solar System Rendering

```rust
impl SolarSystemRenderer {
    fn render(&self, ui: &mut egui::Ui, system: &SolarSystem, camera: &Camera) {
        let center = camera.world_to_screen(system.center.x, system.center.y);
        
        // Draw orbit rings (faint)
        for orbit in &system.orbits {
            let screen_radius = orbit.radius * camera.zoom.get();
            ui.painter().circle_stroke(
                egui::pos2(center.0, center.1),
                screen_radius,
                egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30)),
            );
        }
        
        // Draw sun (CBU)
        if let Some(sun) = &system.sun {
            self.render_sun(ui, sun, center);
        }
        
        // Draw orbital bodies
        for orbit in &system.orbits {
            for body in &orbit.bodies {
                let world_pos = orbit.body_position(body, system.center);
                let screen_pos = camera.world_to_screen(world_pos.x, world_pos.y);
                
                self.render_orbital_body(ui, body, screen_pos, &orbit.layer);
                
                // Draw moons
                for moon in &body.moons {
                    let moon_world = orbit.moon_position(body, moon, system.center);
                    let moon_screen = camera.world_to_screen(moon_world.x, moon_world.y);
                    self.render_moon(ui, moon, moon_screen);
                }
            }
        }
        
        // Draw connection lines (ownership/relationship edges)
        self.render_relationships(ui, system, camera);
    }
    
    fn render_sun(&self, ui: &mut egui::Ui, cbu: &GraphNode, screen_pos: (f32, f32)) {
        // Glow layers
        for i in (1..=4).rev() {
            let glow_radius = 40.0 + i as f32 * 15.0;
            let alpha = 50 - i * 10;
            ui.painter().circle_filled(
                egui::pos2(screen_pos.0, screen_pos.1),
                glow_radius,
                egui::Color32::from_rgba_unmultiplied(255, 200, 50, alpha as u8),
            );
        }
        
        // Core
        ui.painter().circle_filled(
            egui::pos2(screen_pos.0, screen_pos.1),
            40.0,
            egui::Color32::from_rgb(255, 215, 0),
        );
        
        // Label
        ui.painter().text(
            egui::pos2(screen_pos.0, screen_pos.1),
            egui::Align2::CENTER_CENTER,
            &cbu.label,
            egui::FontId::proportional(14.0),
            egui::Color32::BLACK,
        );
    }
}
```

### 2.5 Tasks - Solar System View

- [ ] Create `SolarSystem` struct with orbits
- [ ] Create `Orbit` struct with bodies and moons
- [ ] Implement `build_solar_system()` from graph data
- [ ] Add orbital mechanics (rotation, distribution)
- [ ] Implement orbit ring rendering
- [ ] Implement sun (CBU) rendering with glow
- [ ] Implement orbital body rendering by layer
- [ ] Implement moon rendering (services around products)
- [ ] Add relationship lines between related bodies
- [ ] Add hover highlighting (highlight connected bodies)
- [ ] Add click to select body
- [ ] Add double-click to focus on body
- [ ] Animate orbit rotation (slow, ambient)
- [ ] Add velocity boost on hover (draws attention)

---

## Part 3: Universe ↔ Solar System Transitions

### 3.1 Zoom Transition

```rust
impl ViewTransition {
    /// Transition from universe to solar system (zoom into CBU)
    pub fn zoom_into_cbu(&mut self, cbu_id: Uuid, camera: &mut Camera, universe: &Universe) {
        // Find CBU position in universe
        let cbu_pos = universe.get_cbu_position(cbu_id);
        
        // Start flight animation
        camera.fly_to_zoom(cbu_pos.x, cbu_pos.y, 3.0);
        
        // Fade out other CBUs
        for other_cbu in universe.cbus.iter().filter(|c| c.id != cbu_id) {
            self.fade_out(other_cbu.id.to_string());
        }
        
        // Request solar system data load
        self.request_cbu_detail(cbu_id);
        
        // Set state
        self.target_view = ViewMode::SolarSystem(cbu_id);
    }
    
    /// Transition from solar system back to universe
    pub fn zoom_out_to_universe(&mut self, camera: &mut Camera) {
        // Zoom out
        camera.zoom_to(0.5);
        camera.fly_to(0.0, 0.0);  // Universe center
        
        // Fade in all CBUs
        for cbu in &self.universe.cbus {
            self.fade_in(cbu.id.to_string());
        }
        
        // Collapse solar system
        self.collapse_solar_system();
        
        // Set state
        self.target_view = ViewMode::Universe;
    }
}
```

### 3.2 Tasks - Transitions

- [ ] Implement `zoom_into_cbu()` with camera flight
- [ ] Implement `zoom_out_to_universe()` with camera pullback
- [ ] Fade out non-selected CBUs during zoom in
- [ ] Fade in universe CBUs during zoom out
- [ ] Load solar system data asynchronously during transition
- [ ] Show loading indicator if data not ready
- [ ] Add breadcrumb update on transition

---

## Part 4: Status Encoding

### 4.1 Visual Properties Table

| Property | Universe View | Solar System View |
|----------|---------------|-------------------|
| **Position** | Force simulation (clusters) | Orbital (radius/angle) |
| **Size** | AUM (log scale) | Importance score |
| **Color** | Risk rating | KYC completion |
| **Brightness** | Activity level | Verification status |
| **Pulse** | Onboarding active | Needs attention |
| **Ring** | Client type | Entity category |
| **Glow** | High activity | Fully verified |

### 4.2 Color Palettes

```rust
pub mod colors {
    use egui::Color32;
    
    // Risk rating (universe view)
    pub const RISK_STANDARD: Color32 = Color32::from_rgb(76, 175, 80);   // Green
    pub const RISK_LOW: Color32 = Color32::from_rgb(139, 195, 74);       // Light green
    pub const RISK_MEDIUM: Color32 = Color32::from_rgb(255, 193, 7);     // Amber
    pub const RISK_HIGH: Color32 = Color32::from_rgb(255, 87, 34);       // Deep orange
    pub const RISK_PROHIBITED: Color32 = Color32::from_rgb(33, 33, 33);  // Near black
    pub const RISK_UNRATED: Color32 = Color32::from_rgb(158, 158, 158);  // Grey
    
    // KYC completion (solar system view)
    pub const KYC_COMPLETE: Color32 = Color32::from_rgb(76, 175, 80);    // Green
    pub const KYC_PARTIAL: Color32 = Color32::from_rgb(255, 193, 7);     // Amber
    pub const KYC_DRAFT: Color32 = Color32::from_rgb(158, 158, 158);     // Grey
    pub const KYC_PENDING: Color32 = Color32::from_rgb(66, 165, 245);    // Blue
    pub const KYC_OVERDUE: Color32 = Color32::from_rgb(244, 67, 54);     // Red
    
    // Entity category (rings)
    pub const ENTITY_SHELL: Color32 = Color32::from_rgb(66, 165, 245);   // Blue
    pub const ENTITY_PERSON: Color32 = Color32::from_rgb(102, 187, 106); // Green
    pub const ENTITY_PRODUCT: Color32 = Color32::from_rgb(255, 167, 38); // Orange
    pub const ENTITY_SERVICE: Color32 = Color32::from_rgb(171, 71, 188); // Purple
}
```

### 4.3 Tasks - Status Encoding

- [ ] Implement risk rating color mapping
- [ ] Implement KYC completion color mapping
- [ ] Add brightness calculation from activity score
- [ ] Add pulse animation for onboarding/attention
- [ ] Add ring rendering for entity category
- [ ] Add glow effect for verified/complete status
- [ ] Create visual legend overlay

---

## Success Criteria

- [ ] Universe view shows all CBUs with meaningful clustering
- [ ] Force simulation settles into stable layout
- [ ] CBU colors clearly indicate risk rating
- [ ] CBU size reflects economic importance
- [ ] Zoom into CBU triggers smooth flight animation
- [ ] Solar system shows entities in orbital arrangement
- [ ] Orbit animation is smooth and ambient (not distracting)
- [ ] Products/services/UBOs are visually distinct
- [ ] Relationships are visible without clutter
- [ ] Zoom out returns to universe smoothly
- [ ] Performance handles 500+ CBUs at 60fps

---

## References

- Force-directed layouts: https://github.com/d3/d3-force
- Astronomy visualization: https://stellarium-web.org/
- egui animations: https://docs.rs/egui/latest/egui/
- Original spec: `/docs/CBU_UNIVERSE_VISUALIZATION_SPEC.md`

---

*Astronomy view: CBU data as an explorable universe.*
