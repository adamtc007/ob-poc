# TODO: Wire Galaxy View (Client-Level CBU Visualization)

## Status: READY TO IMPLEMENT

**Dependency Cleared:** Trading Matrix AST refactoring complete (2026-01-06).

Patterns now established:
- ✅ Document-as-source-of-truth (`TradingMatrixDocument` tree)
- ✅ Hierarchical taxonomy traversal (path-based node IDs)
- ✅ `ast_db::apply_and_save` pattern for mutations
- ✅ API returns document directly (no SQL reconstruction)

These patterns should guide Galaxy View implementation.

---

## Reference Documents

| Document | Purpose |
|----------|---------|
| **`TUNNEL-NAVIGATION-EXPERIENCE.md`** | **START HERE** - The experiential brief. How it should FEEL. |
| `GALAXY-NODE-EDGE-TAXONOMY.md` | Node types, edge types, LOD levels, response structures |
| `ESPER-NAVIGATION-MODEL.md` | Soft focus, inline expansion, voice commands, camera behavior |
| `NATURAL-TREE-TRAVERSAL.md` | Animation timing, spring physics, organic growth rendering |
| `TODO-GALAXY-SERVER-API.md` | Server endpoints, SQL queries, repository/routes code |
| `brain-dump/GALAXY-VIEW-RESEARCH.md` | Analysis of existing infrastructure |

**Read order:** TUNNEL first (the feel), then TAXONOMY (the structure), then ESPER (the commands), then TREE (the animations), then SERVER-API (the implementation).

---

## Context

The galaxy/astronomy visualization infrastructure EXISTS but is NOT WIRED:

| Component | Location | Status |
|-----------|----------|--------|
| `GalaxyView` widget | `ob-poc-graph/src/graph/galaxy.rs` | ✅ Built, uses mock data |
| `AstronomyView` enum | `ob-poc-graph/src/graph/astronomy.rs` | ✅ Built |
| `ViewTransition` | `ob-poc-graph/src/graph/astronomy.rs` | ✅ Built |
| `ForceSimulation` | `ob-poc-graph/src/graph/force_sim.rs` | ✅ Built |
| `TaxonomyContext::Book` | `rust/src/taxonomy/rules.rs` | ✅ Rules defined |
| `GraphScope::Book` | `rust/src/graph/types.rs` | ✅ Type exists |
| `/api/graph/book/:apex_id` | `rust/src/api/graph_routes.rs` | ✅ Endpoint exists |
| `load_client_cbus()` | `rust/src/taxonomy/builder.rs` | ✅ Query exists |
| `cbus.commercial_client_entity_id` | Database | ✅ FK exists |

**Missing:** API endpoints for universe/clusters, AppState integration, UI wiring.

---

## Implementation Plan

### Phase 1: Server API for Universe/Clusters

#### 1.1 Create Universe Repository

**File:** `rust/src/database/universe_repository.rs` (NEW)

```rust
use sqlx::PgPool;
use uuid::Uuid;

pub struct UniverseRepository {
    pool: PgPool,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ClusterRow {
    pub cluster_id: String,
    pub cluster_label: String,
    pub cbu_count: i64,
    pub low_risk: i64,
    pub medium_risk: i64,
    pub high_risk: i64,
    pub unrated: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct CommercialClientRow {
    pub entity_id: Uuid,
    pub name: String,
    pub cbu_count: i64,
    pub jurisdictions: Vec<String>,
}

impl UniverseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get CBUs grouped by jurisdiction
    pub async fn get_clusters_by_jurisdiction(&self) -> Result<Vec<ClusterRow>, sqlx::Error> {
        sqlx::query_as::<_, ClusterRow>(r#"
            SELECT 
                c.jurisdiction as cluster_id,
                c.jurisdiction as cluster_label,
                COUNT(*) as cbu_count,
                COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'LOW') as low_risk,
                COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'MEDIUM') as medium_risk,
                COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'HIGH') as high_risk,
                COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' IS NULL 
                                    OR c.risk_context->>'risk_rating' = 'UNRATED') as unrated
            FROM "ob-poc".cbus c
            WHERE c.jurisdiction IS NOT NULL
            GROUP BY c.jurisdiction
            ORDER BY COUNT(*) DESC
        "#)
        .fetch_all(&self.pool)
        .await
    }

    /// Get CBU IDs for a jurisdiction cluster
    pub async fn get_cbu_ids_for_jurisdiction(&self, jurisdiction: &str) -> Result<Vec<Uuid>, sqlx::Error> {
        sqlx::query_scalar::<_, Uuid>(r#"
            SELECT cbu_id FROM "ob-poc".cbus WHERE jurisdiction = $1
        "#)
        .bind(jurisdiction)
        .fetch_all(&self.pool)
        .await
    }

    /// List commercial clients with CBU counts
    pub async fn list_commercial_clients(&self) -> Result<Vec<CommercialClientRow>, sqlx::Error> {
        sqlx::query_as::<_, CommercialClientRow>(r#"
            SELECT 
                e.entity_id,
                COALESCE(lc.registered_name, pp.full_name, 'Unknown') as name,
                COUNT(c.cbu_id) as cbu_count,
                ARRAY_AGG(DISTINCT c.jurisdiction) FILTER (WHERE c.jurisdiction IS NOT NULL) as jurisdictions
            FROM "ob-poc".cbus c
            JOIN "ob-poc".entities e ON e.entity_id = c.commercial_client_entity_id
            LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
            WHERE c.commercial_client_entity_id IS NOT NULL
            GROUP BY e.entity_id, lc.registered_name, pp.full_name
            ORDER BY COUNT(c.cbu_id) DESC
        "#)
        .fetch_all(&self.pool)
        .await
    }

    /// Get shared entities across CBUs for a commercial client
    pub async fn get_shared_entities(&self, client_id: Uuid) -> Result<Vec<SharedEntityRow>, sqlx::Error> {
        sqlx::query_as::<_, SharedEntityRow>(r#"
            WITH client_cbus AS (
                SELECT cbu_id FROM "ob-poc".cbus WHERE commercial_client_entity_id = $1
            )
            SELECT 
                cer.entity_id,
                COALESCE(lc.registered_name, pp.full_name, 'Unknown') as name,
                ARRAY_AGG(DISTINCT r.name) as roles,
                COUNT(DISTINCT cer.cbu_id) as cbu_count,
                ARRAY_AGG(DISTINCT cer.cbu_id) as cbu_ids
            FROM "ob-poc".cbu_entity_roles cer
            JOIN client_cbus cc ON cc.cbu_id = cer.cbu_id
            JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
            JOIN "ob-poc".roles r ON r.role_id = cer.role_id
            LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
            GROUP BY cer.entity_id, lc.registered_name, pp.full_name
            HAVING COUNT(DISTINCT cer.cbu_id) > 1
            ORDER BY COUNT(DISTINCT cer.cbu_id) DESC
        "#)
        .bind(client_id)
        .fetch_all(&self.pool)
        .await
    }
}
```

#### 1.2 Create Universe Routes

**File:** `rust/src/api/universe_routes.rs` (NEW)

```rust
use axum::{extract::State, routing::get, Json, Router};
use sqlx::PgPool;

// Response types matching GalaxyView's ClusterData
#[derive(Debug, Serialize)]
pub struct UniverseResponse {
    pub total_cbu_count: usize,
    pub clusters: Vec<ClusterResponse>,
}

#[derive(Debug, Serialize)]
pub struct ClusterResponse {
    pub id: String,
    pub label: String,
    pub short_label: String,
    pub cbu_count: usize,
    pub cbu_ids: Vec<Uuid>,
    pub cluster_type: String,  // "JURISDICTION", "MANCO", etc.
    pub risk_summary: RiskSummaryResponse,
}

#[derive(Debug, Serialize)]
pub struct RiskSummaryResponse {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
    pub unrated: usize,
}

/// GET /api/universe?cluster_by=jurisdiction
pub async fn get_universe(
    State(pool): State<PgPool>,
    Query(params): Query<UniverseParams>,
) -> Result<Json<UniverseResponse>, (StatusCode, String)> {
    let repo = UniverseRepository::new(pool);
    
    let cluster_by = params.cluster_by.as_deref().unwrap_or("jurisdiction");
    
    let clusters = match cluster_by {
        "jurisdiction" => repo.get_clusters_by_jurisdiction().await,
        // Future: "manco", "product_type", etc.
        _ => repo.get_clusters_by_jurisdiction().await,
    }.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Convert to response format
    let mut response_clusters = Vec::new();
    let mut total = 0;
    
    for cluster in clusters {
        let cbu_ids = repo.get_cbu_ids_for_jurisdiction(&cluster.cluster_id)
            .await
            .unwrap_or_default();
        
        total += cluster.cbu_count as usize;
        
        response_clusters.push(ClusterResponse {
            id: cluster.cluster_id.clone(),
            label: cluster.cluster_label,
            short_label: cluster.cluster_id.chars().take(2).collect(),
            cbu_count: cluster.cbu_count as usize,
            cbu_ids,
            cluster_type: "JURISDICTION".to_string(),
            risk_summary: RiskSummaryResponse {
                low: cluster.low_risk as usize,
                medium: cluster.medium_risk as usize,
                high: cluster.high_risk as usize,
                unrated: cluster.unrated as usize,
            },
        });
    }
    
    Ok(Json(UniverseResponse {
        total_cbu_count: total,
        clusters: response_clusters,
    }))
}

/// GET /api/commercial-clients
pub async fn list_commercial_clients(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<CommercialClientResponse>>, (StatusCode, String)> {
    let repo = UniverseRepository::new(pool);
    let clients = repo.list_commercial_clients().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(Json(clients.into_iter().map(|c| CommercialClientResponse {
        entity_id: c.entity_id,
        name: c.name,
        cbu_count: c.cbu_count as usize,
        jurisdictions: c.jurisdictions,
    }).collect()))
}

pub fn create_universe_router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/universe", get(get_universe))
        .route("/api/commercial-clients", get(list_commercial_clients))
        .route("/api/commercial-client/:id/shared-entities", get(get_shared_entities))
        .with_state(pool)
}
```

#### 1.3 Register Routes

**File:** `rust/src/api/mod.rs`

Add:
```rust
pub mod universe_routes;
pub use universe_routes::create_universe_router;
```

**File:** `rust/src/main.rs` (or wherever routes are composed)

Add universe router to the app.

---

### Phase 2: Client State Integration

#### 2.1 Add ViewLevel to AppState

**File:** `rust/crates/ob-poc-ui/src/state.rs`

```rust
// Add to imports
use ob_poc_graph::{GalaxyView, ViewTransition, AstronomyView};

/// View level in the navigation hierarchy
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ViewLevel {
    #[default]
    Universe,                           // Galaxy view - all CBUs clustered
    Client { client_id: Uuid, client_name: String },  // Client book
    Cbu { cbu_id: Uuid, cbu_name: String },           // Single CBU (existing)
    Entity { entity_id: Uuid, entity_name: String },  // Entity detail
}

// Add to AppState struct:
pub struct AppState {
    // ... existing fields ...
    
    // NEW: Galaxy/Universe navigation
    pub view_level: ViewLevel,
    pub galaxy_view: GalaxyView,
    pub astronomy_transition: ViewTransition,
    pub universe_data: Option<UniverseData>,  // Cached from API
}

/// Cached universe data from API
#[derive(Debug, Clone)]
pub struct UniverseData {
    pub total_cbu_count: usize,
    pub clusters: Vec<ClusterData>,
    pub fetched_at: f64,
}
```

#### 2.2 Add API Functions

**File:** `rust/crates/ob-poc-ui/src/api.rs`

```rust
/// Universe/cluster response from server
#[derive(Debug, Clone, Deserialize)]
pub struct UniverseResponse {
    pub total_cbu_count: usize,
    pub clusters: Vec<ClusterData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClusterData {
    pub id: String,
    pub label: String,
    pub short_label: String,
    pub cbu_count: usize,
    pub cbu_ids: Vec<Uuid>,
    pub cluster_type: String,
    pub risk_summary: RiskSummary,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RiskSummary {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
    pub unrated: usize,
}

/// Fetch universe cluster data
pub async fn get_universe(cluster_by: Option<&str>) -> Result<UniverseResponse, String> {
    let param = cluster_by.unwrap_or("jurisdiction");
    get(&format!("/api/universe?cluster_by={}", param)).await
}

/// Fetch commercial clients list
pub async fn get_commercial_clients() -> Result<Vec<CommercialClientSummary>, String> {
    get("/api/commercial-clients").await
}
```

---

### Phase 3: UI Wiring

#### 3.1 Create Galaxy Panel

**File:** `rust/crates/ob-poc-ui/src/panels/galaxy_panel.rs` (NEW)

```rust
use egui::Ui;
use ob_poc_graph::{GalaxyView, GalaxyAction, ClusterData};

use crate::state::AppState;

/// Render the galaxy (universe) view panel
pub fn render_galaxy_panel(ui: &mut Ui, state: &mut AppState) -> Option<GalaxyPanelAction> {
    let mut action = None;
    
    // Allocate painter for galaxy view
    let available = ui.available_size();
    let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());
    let screen_rect = response.rect;
    
    // Handle input
    let galaxy_action = state.galaxy_view.handle_input(
        &response,
        &state.graph_widget.camera(),  // Share camera? Or separate?
        screen_rect,
    );
    
    // Render
    let dt = ui.input(|i| i.stable_dt);
    state.galaxy_view.ui(
        &painter,
        &state.graph_widget.camera(),
        screen_rect,
        dt,
    );
    
    // Handle actions
    match galaxy_action {
        GalaxyAction::DrillDown { cluster_id, cluster_label, cbu_ids } => {
            // If single CBU, go directly to CBU view
            if cbu_ids.len() == 1 {
                action = Some(GalaxyPanelAction::OpenCbu(cbu_ids[0]));
            } else {
                // Show cluster detail (list of CBUs in this jurisdiction)
                action = Some(GalaxyPanelAction::OpenCluster {
                    cluster_id,
                    cluster_label,
                    cbu_ids,
                });
            }
        }
        GalaxyAction::HoverChanged { cluster_id } => {
            // Update tooltip/status
        }
        GalaxyAction::None => {}
    }
    
    // Request repaint if animating
    if state.galaxy_view.needs_repaint() {
        ui.ctx().request_repaint();
    }
    
    action
}

#[derive(Debug, Clone)]
pub enum GalaxyPanelAction {
    OpenCbu(Uuid),
    OpenCluster { cluster_id: String, cluster_label: String, cbu_ids: Vec<Uuid> },
    OpenClient(Uuid),
}
```

#### 3.2 Wire Galaxy Panel to App

**File:** `rust/crates/ob-poc-ui/src/app.rs`

In the main `update()` function, add view level switching:

```rust
// In the main panel rendering section:
match self.state.view_level {
    ViewLevel::Universe => {
        // Render galaxy panel
        if let Some(action) = render_galaxy_panel(ui, &mut self.state) {
            match action {
                GalaxyPanelAction::OpenCbu(cbu_id) => {
                    // Switch to CBU view
                    self.state.view_level = ViewLevel::Cbu { 
                        cbu_id, 
                        cbu_name: "Loading...".to_string() 
                    };
                    // Trigger CBU data fetch
                    self.state.set_pending_cbu_fetch(cbu_id);
                }
                GalaxyPanelAction::OpenCluster { cluster_id, cbu_ids, .. } => {
                    // Show cluster detail panel (list CBUs)
                    self.state.set_pending_cluster_detail(cluster_id, cbu_ids);
                }
                GalaxyPanelAction::OpenClient(client_id) => {
                    self.state.view_level = ViewLevel::Client { 
                        client_id, 
                        client_name: "Loading...".to_string() 
                    };
                }
            }
        }
    }
    ViewLevel::Client { client_id, .. } => {
        // Render client book panel
        // TODO: Implement client book view
    }
    ViewLevel::Cbu { cbu_id, .. } => {
        // Existing CBU graph rendering
        // ... current implementation ...
    }
    ViewLevel::Entity { entity_id, .. } => {
        // Entity detail view
        // TODO: Implement
    }
}
```

#### 3.3 Add Zoom Out Button

In toolbar or header:

```rust
// Breadcrumb / zoom out controls
ui.horizontal(|ui| {
    match &self.state.view_level {
        ViewLevel::Universe => {
            ui.label("🌌 Universe");
        }
        ViewLevel::Client { client_name, .. } => {
            if ui.button("🌌 Universe").clicked() {
                self.state.view_level = ViewLevel::Universe;
            }
            ui.label("›");
            ui.label(format!("🏢 {}", client_name));
        }
        ViewLevel::Cbu { cbu_name, .. } => {
            if ui.button("🌌 Universe").clicked() {
                self.state.view_level = ViewLevel::Universe;
            }
            ui.label("›");
            // TODO: Show client if known
            ui.label(format!("📦 {}", cbu_name));
        }
        ViewLevel::Entity { entity_name, .. } => {
            // Full breadcrumb
        }
    }
});
```

#### 3.4 Initial Data Fetch

On app startup or when entering Universe view:

```rust
// In update() or init:
if matches!(self.state.view_level, ViewLevel::Universe) && self.state.universe_data.is_none() {
    // Fetch universe data
    let ctx = ui.ctx().clone();
    let async_state = self.state.async_state.clone();
    
    wasm_bindgen_futures::spawn_local(async move {
        match api::get_universe(None).await {
            Ok(response) => {
                if let Ok(mut state) = async_state.lock() {
                    state.pending_universe_data = Some(response);
                }
                ctx.request_repaint();
            }
            Err(e) => {
                web_sys::console::error_1(&format!("Failed to fetch universe: {}", e).into());
            }
        }
    });
}

// Process pending universe data
if let Some(data) = self.state.take_pending_universe_data() {
    // Convert to ClusterData format for GalaxyView
    let clusters: Vec<ClusterData> = data.clusters.into_iter().map(|c| {
        ob_poc_graph::ClusterData {
            id: c.id,
            label: c.label,
            short_label: c.short_label,
            cbu_count: c.cbu_count,
            cbu_ids: c.cbu_ids,
            cluster_type: ob_poc_graph::ClusterType::Jurisdiction,
            risk_summary: Some(ob_poc_graph::RiskSummary {
                low: c.risk_summary.low,
                medium: c.risk_summary.medium,
                high: c.risk_summary.high,
                unrated: c.risk_summary.unrated,
            }),
        }
    }).collect();
    
    self.state.galaxy_view.set_clusters(clusters);
    self.state.universe_data = Some(UniverseData {
        total_cbu_count: data.total_cbu_count,
        clusters: data.clusters,
        fetched_at: ui.input(|i| i.time),
    });
}
```

---

### Phase 4: Session Integration

#### 4.1 Add Scope to Session Context

**File:** `rust/src/api/session.rs` (or wherever UnifiedSessionContext is)

```rust
pub struct UnifiedSessionContext {
    // Existing...
    pub active_cbu: Option<BoundCbu>,
    
    // NEW: View scope
    pub scope: SessionScope,
    pub scope_stack: Vec<ScopeFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum SessionScope {
    #[default]
    Universe,
    Client { client_id: Uuid, client_name: String },
    Cbu { cbu_id: Uuid, cbu_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeFrame {
    pub scope: SessionScope,
    pub entered_at: chrono::DateTime<chrono::Utc>,
}
```

#### 4.2 DSL Verbs for Navigation

```lisp
;; Navigate to universe view
(view.universe)

;; Navigate to client book
(view.client :id @allianz)

;; Navigate to CBU (existing)
(cbu.focus :id @fund-a)

;; Show shared entities for current client
(view.shared-entities)
```

---

## Testing Checklist

- [ ] `/api/universe` returns clusters with correct counts
- [ ] `/api/commercial-clients` lists clients with CBU counts
- [ ] GalaxyView renders with real data (not mock)
- [ ] Click cluster → shows cluster detail or drills to single CBU
- [ ] Zoom out button returns to Universe view
- [ ] Breadcrumb displays correct hierarchy
- [ ] Session scope persists across page refresh
- [ ] Shared entities query works for multi-CBU clients

---

## Dependencies

**Completed:**
1. ✅ Trading Matrix AST refactoring (2026-01-06)
   - 399 profiles migrated to tree format
   - `TradingMatrixDocument` / `TradingMatrixNode` types established
   - `ast_db::apply_and_save` pattern for mutations
   - Path-based node IDs for stable references

**Ready to Start:**
- Phase 1: Server API (universe/clusters endpoints)
- Phase 2-3: Client wiring (after Phase 1)

---

## Notes for Claude

### The Core Principle

**You are building a tunnel navigation system, not a page-based UI.**

The user is the squiddy from The Matrix. They're INSIDE the data, steering through it. They don't click links - they fly. They don't load pages - they enter spaces. Every implementation decision should answer: "Does this feel like piloting through tunnels?"

Read `TUNNEL-NAVIGATION-EXPERIENCE.md` FIRST before touching any code.

### Technical Notes

1. **Don't remove mock data yet** - Keep `load_mock_data()` in GalaxyView until API is wired
2. **Reuse existing ClusterData** - The types in `galaxy.rs` match what API should return
3. **ViewTransition already handles animations** - Don't rebuild, just call `zoom_into_cbu()` / `zoom_out_to_universe()`
4. **Camera might need separation** - Galaxy view may need its own camera vs CBU graph camera
5. **commercial_client_entity_id is nullable** - Some CBUs may not have a parent client, handle gracefully

### Esper Navigation

6. **Soft focus vs hard dive** - See `ESPER-NAVIGATION-MODEL.md` for focus stack pattern
7. **Inline expansion** - `/api/node/:id/preview` endpoint for "enhance" without full navigation
8. **Voice command mapping** - DSL verbs `esper.enhance`, `esper.dive-in`, `esper.pull-back`

### Natural Tree Rendering

9. **Spatial stability** - Parent node is anchor, never moves during child expansion
10. **Spring physics** - Use ORGANIC preset (stiffness: 180, damping: 12) for growth animations
11. **Cascade timing** - Children start at 60% parent completion, 150ms stagger per level
12. **Growth phases** - Bud (50ms) → Sprout (150ms) → Edge (200ms) → Unfurl (200ms) → Settle (100ms)
13. **Collapse is faster** - ~300ms total, reverse order (deepest children first)
14. **Camera anticipation** - Pan to final bounds BEFORE growth completes

### Tunnel Feel

15. **No teleporting** - Everything moves. Camera flies. Nodes grow. Nothing snaps.
16. **Momentum** - Movement has acceleration, coasting, braking. Not instant start/stop.
17. **Leading camera** - Camera arrives at destination before you do. Anticipate, don't follow.
18. **Peripheral vision** - Siblings visible but muted. You're in a tunnel, not a void.
19. **Depth is physical** - Deeper = narrower, closer, more focused. Zoom correlates with depth.
20. **Interruptible** - Any navigation can abort mid-flight. User is always in control.

---

## Reference Files

| Purpose | File |
|---------|------|
| GalaxyView widget | `ob-poc-graph/src/graph/galaxy.rs` |
| AstronomyView/ViewTransition | `ob-poc-graph/src/graph/astronomy.rs` |
| ForceSimulation | `ob-poc-graph/src/graph/force_sim.rs` |
| Spring animation | `ob-poc-graph/src/graph/animation.rs` |
| TaxonomyContext::Book | `rust/src/taxonomy/rules.rs` |
| GraphScope enum | `rust/src/graph/types.rs` |
| Existing book endpoint | `rust/src/api/graph_routes.rs:get_book_graph` |
| CBU → Client FK | `cbus.commercial_client_entity_id` |

### Architecture Docs

| Document | Level | Content |
|----------|-------|---------|
| **`TUNNEL-NAVIGATION-EXPERIENCE.md`** | **FEEL** | The experiential brief - piloting, momentum, peripheral vision |
| `GALAXY-NODE-EDGE-TAXONOMY.md` | STRUCTURE | Node/edge types per LOD, response structures |
| `ESPER-NAVIGATION-MODEL.md` | COMMANDS | Soft focus, inline expansion, voice commands |
| `NATURAL-TREE-TRAVERSAL.md` | ANIMATION | Animation timing, spring physics, growth rendering |
| `TODO-GALAXY-SERVER-API.md` | CODE | Server implementation (endpoints, SQL, code) |
| `brain-dump/GALAXY-VIEW-RESEARCH.md` | RESEARCH | Infrastructure analysis |
