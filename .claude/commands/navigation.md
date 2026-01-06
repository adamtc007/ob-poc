# Galaxy Navigation Development

Before writing navigation/visualization code, understand the architecture.

## Core Principle

**NavigationService is the SINGLE orchestrator** (user directive).

No parallel navigation systems. No state in widgets. One service owns all navigation state.

## Architecture

```
AsyncState.pending_*  →  NavigationService  →  Widgets (read-only render)
                              ↓
                         Camera2D
                              ↓
                         Server API
```

## NavigationService Responsibilities

```rust
pub struct NavigationService {
    // Position in taxonomy
    pub level: ViewLevel,           // Universe, Cluster, Cbu, Entity
    pub scope: NavigationScope,     // What we're looking at
    
    // Physics
    pub velocity: Vec2,
    pub camera: Camera2D,
    
    // Animation
    pub transition: Option<ViewTransition>,
    
    // Agent
    pub agent: AgentState,
}

impl NavigationService {
    // Called from app.rs update() - BEFORE ui()
    pub fn tick(&mut self, dt: f32) {
        self.update_physics(dt);
        self.update_transitions(dt);
        self.update_camera(dt);
    }
    
    // Process pending commands from AsyncState
    pub fn process_pending(&mut self, async_state: &mut AsyncState) {
        if let Some(cluster_id) = async_state.pending_scale_galaxy.take() {
            self.navigate_to_cluster(cluster_id);
        }
        // ... handle other pending_* commands
    }
}
```

## pending_* Commands

AsyncState already has 50+ navigation commands wired:
- `pending_scale_universe` - Go to universe view
- `pending_scale_galaxy` - Go to cluster/galaxy view  
- `pending_scale_system` - Go to CBU view
- `pending_taxonomy_zoom_in` - Drill deeper
- `pending_taxonomy_zoom_out` - Surface up
- `pending_drill_through` - Navigate to specific node
- `pending_orbit` - Circle around current focus

**Don't create new parallel systems.** Wire these to NavigationService.

## Key Files

| File | Purpose |
|------|---------|
| `ob-poc-ui/src/navigation_service.rs` | 🆕 Single nav orchestrator |
| `ob-poc-ui/src/state.rs` | AsyncState with pending_* |
| `ob-poc-ui/src/app.rs` | Calls NavigationService.tick() |
| `ob-poc-graph/src/graph/galaxy.rs` | Universe/cluster renderer |
| `ob-poc-graph/src/graph/camera.rs` | Camera2D |
| `ob-poc-graph/src/graph/animation.rs` | SpringF32, SpringVec2 |
| `ob-poc-types/src/galaxy.rs` | 🆕 Shared types |

## Shared Types

All navigation types live in `ob-poc-types/src/galaxy.rs`:
- `UniverseGraph`, `ClusterNode`, `ClusterDetailGraph`
- `ViewLevel`, `NavigationScope`, `NavigationPosition`
- `AgentState`, `AgentSuggestion`, `Anomaly`

Server and client use SAME types. No translation layer.

## Widget Pattern for Navigation

```rust
// Galaxy widget - STATELESS, returns actions
impl GalaxyView {
    pub fn ui(
        &self, 
        ui: &mut Ui, 
        nav: &NavigationService,
        data: &UniverseGraph,
    ) -> Option<GalaxyAction> {
        // Read current state
        let camera = nav.camera();
        let zoom = nav.zoom();
        
        // Render clusters
        for cluster in &data.clusters {
            let screen_pos = camera.world_to_screen(cluster.position);
            // ... render
            
            if cluster_clicked {
                return Some(GalaxyAction::DrillDown { 
                    cluster_id: cluster.id.clone() 
                });
            }
        }
        None
    }
}

// In app.rs
if let Some(action) = galaxy_view.ui(ui, &nav_service, &universe_data) {
    match action {
        GalaxyAction::DrillDown { cluster_id } => {
            async_state.pending_scale_galaxy = Some(cluster_id);
        }
    }
}
```

## Level Transitions

```
Universe (all clusters)
    ↓ DrillDown
Cluster (CBUs in cluster)  
    ↓ DrillDown
CBU (entities in CBU)
    ↓ DrillDown
Entity (detail view)
    ↓ Surface
Back up the chain
```

Each transition:
1. Sets `transition = Some(ViewTransition { from, to, progress: 0.0 })`
2. `tick()` advances progress using springs
3. Widget interpolates render based on progress
4. On complete, `level` and `scope` update

## Reference Document

**Read TODO-GALAXY-NAVIGATION-SYSTEM.md** for:
- Full type definitions (Part 5)
- Server API endpoints (Part 6)
- Implementation phases (Part 7)
- Animation timing (Appendix A)
- Spring configs (Appendix B)
