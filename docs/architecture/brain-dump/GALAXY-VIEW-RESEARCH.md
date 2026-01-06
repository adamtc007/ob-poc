# Galaxy View Research: Client-Level CBU Visualization

## Executive Summary

The infrastructure for galaxy (client/universe) level visualization largely EXISTS but is NOT WIRED together. The pieces are scattered across server-side taxonomy, graph API, and client-side egui modules but lack the integration layer.

---

## What EXISTS

### Server-Side Infrastructure

#### 1. Taxonomy System (`rust/src/taxonomy/`)

**TaxonomyContext** (rules.rs:35-53):
```rust
pub enum TaxonomyContext {
    Universe,                        // All CBUs the user can see
    Book { client_id: Uuid },        // All CBUs for a commercial client
    CbuTrading { cbu_id: Uuid },     // Single CBU - trading view
    CbuUbo { cbu_id: Uuid },         // Single CBU - UBO view
    // ...
}
```

**MembershipRules** (rules.rs:192-223):
```rust
pub fn universe() -> Self {
    Self {
        root_filter: RootFilter::Universe,
        grouping: GroupingStrategy::ByDimension(Dimension::Jurisdiction),
        max_depth: 2,  // Just clusters and CBUs
        // ...
    }
}

pub fn book(client_id: Uuid) -> Self {
    Self {
        root_filter: RootFilter::Client { client_id },
        grouping: GroupingStrategy::ByDimension(Dimension::FundType),
        max_depth: 2,
        // ...
    }
}
```

**TaxonomyBuilder** (builder.rs:300-318):
```rust
async fn load_client_cbus(&self, pool: &PgPool, client_id: Uuid) -> Result<Vec<EntityData>> {
    sqlx::query_as::<_, CbuRow>(
        "SELECT c.cbu_id, c.name, c.jurisdiction, c.client_type
         FROM \"ob-poc\".cbus c
         WHERE c.commercial_client_entity_id = $1
         ORDER BY c.name"
    )
    .bind(client_id)
    .fetch_all(pool).await
}
```

#### 2. Graph API (`rust/src/api/graph_routes.rs`)

**Existing Endpoints:**
```
GET /api/cbu                         - List all CBUs (flat list)
GET /api/cbu/:id/graph               - Single CBU graph
GET /api/graph/book/:apex_entity_id  - EntityGraph for ownership book
GET /api/graph/jurisdiction/:code    - EntityGraph for jurisdiction
GET /api/graph/entity/:id/neighborhood - Entity N-hop neighborhood
```

**GraphScope Enum** (graph/types.rs:1661-1685):
```rust
pub enum GraphScope {
    Empty,
    SingleCbu { cbu_id: Uuid, cbu_name: String },
    Book { apex_entity_id: Uuid, apex_name: String },
    Jurisdiction { code: String },
    EntityNeighborhood { entity_id: Uuid, hops: u32 },
}
```

#### 3. Database Schema

**CBU → Commercial Client Link:**
```sql
-- cbus table has:
commercial_client_entity_id uuid
-- Comment: "Head office entity that contracted with the bank (e.g., Blackrock Inc)"
```

**This enables:**
```sql
-- Find all CBUs for Allianz
SELECT * FROM cbus 
WHERE commercial_client_entity_id = @allianz_entity_id;

-- Find shared entities across CBUs
SELECT entity_id, COUNT(DISTINCT cbu_id) as cbu_count
FROM cbu_entity_roles
WHERE cbu_id IN (SELECT cbu_id FROM cbus WHERE commercial_client_entity_id = @client)
GROUP BY entity_id
HAVING COUNT(DISTINCT cbu_id) > 1;
```

---

### Client-Side Infrastructure

#### 1. GalaxyView (`ob-poc-graph/src/graph/galaxy.rs`)

**ClusterData:**
```rust
pub struct ClusterData {
    pub id: String,           // e.g., jurisdiction code "LU"
    pub label: String,        // "Luxembourg"
    pub short_label: String,  // "LU"
    pub cbu_count: usize,     // 177
    pub cbu_ids: Vec<Uuid>,   // For drill-down
    pub cluster_type: ClusterType,  // Jurisdiction, ManCo, ProductType, etc.
    pub risk_summary: Option<RiskSummary>,  // Aggregate risk
}
```

**GalaxyAction:**
```rust
pub enum GalaxyAction {
    None,
    DrillDown { cluster_id: String, cluster_label: String, cbu_ids: Vec<Uuid> },
    HoverChanged { cluster_id: Option<String> },
}
```

**GalaxyView Widget:**
- Force simulation for cluster positioning
- Glow animation for hover effects  
- Zoom-responsive compression
- Mock data loader exists: `load_mock_data()`

#### 2. AstronomyView (`ob-poc-graph/src/graph/astronomy.rs`)

**View Mode Enum:**
```rust
pub enum AstronomyView {
    Universe,                                    // All CBUs as stars
    SolarSystem { cbu_id: Uuid, cbu_name: String },  // Single CBU focus
    Transitioning { from, to, progress },        // Animation state
}
```

**ViewTransition Manager:**
- Navigation stack (breadcrumbs)
- `zoom_into_cbu()` - Universe → SolarSystem
- `zoom_out_to_universe()` - SolarSystem → Universe
- Opacity springs for fade in/out
- Camera fly-to animations

#### 3. ForceSimulation (`ob-poc-graph/src/graph/force_sim.rs`)

**ClusterNode:**
```rust
pub struct ClusterNode {
    pub id: String,
    pub label: String,
    pub count: usize,        // Affects radius
    pub position: Pos2,      // Updated by simulation
    pub color: Color32,
    pub pinned: bool,        // Dragging support
}
```

**ForceConfig presets:**
- `ForceConfig::galaxy()` - For cluster-level view
- Repulsion, center attraction, damping, boundary containment

---

## What's MISSING (Not Wired)

### Server-Side Gaps

#### 1. No Universe/Galaxy API Endpoint

**Need:**
```
GET /api/universe?cluster_by=jurisdiction
```

**Returns:**
```json
{
  "total_cbu_count": 671,
  "clusters": [
    {
      "id": "LU",
      "label": "Luxembourg", 
      "cbu_count": 177,
      "cbu_ids": ["uuid1", "uuid2", ...],
      "cluster_type": "JURISDICTION",
      "risk_summary": { "low": 150, "medium": 20, "high": 5, "unrated": 2 }
    },
    ...
  ]
}
```

#### 2. No Commercial Clients List API

**Need:**
```
GET /api/commercial-clients
```

**Returns:**
```json
[
  {
    "entity_id": "allianz-uuid",
    "name": "Allianz SE",
    "cbu_count": 47,
    "jurisdictions": ["LU", "IE", "DE"],
    "total_aum": 500000000000
  },
  ...
]
```

#### 3. Shared Entities API

**Need:**
```
GET /api/commercial-client/:id/shared-entities
```

**Returns entities that appear in multiple CBUs:**
```json
{
  "client_id": "allianz-uuid",
  "shared_entities": [
    {
      "entity_id": "manco-uuid",
      "name": "Allianz Global Investors GmbH",
      "roles": ["MANAGEMENT_COMPANY"],
      "cbu_count": 35,
      "cbu_ids": [...]
    },
    {
      "entity_id": "im-uuid", 
      "name": "PIMCO Europe Ltd",
      "roles": ["INVESTMENT_MANAGER"],
      "cbu_count": 12,
      "cbu_ids": [...]
    }
  ]
}
```

---

### Client-Side Gaps

#### 1. No AppState Integration

**Current** (`ob-poc-ui/src/state.rs`):
```rust
pub struct AppState {
    pub session: Option<SessionStateResponse>,
    pub graph_data: Option<CbuGraphData>,  // Single CBU only!
    pub graph_widget: CbuGraphWidget,
    // No galaxy_view, no astronomy_state
}
```

**Need:**
```rust
pub struct AppState {
    // Existing...
    
    // NEW: Galaxy/Universe state
    pub view_level: ViewLevel,           // Universe | Client | CBU | Entity
    pub galaxy_view: GalaxyView,         // Cluster visualization
    pub astronomy_state: ViewTransition, // Zoom transitions
    pub active_client: Option<ClientSummary>,  // Current commercial client
    pub universe_data: Option<UniverseData>,   // Cached cluster data
}

pub enum ViewLevel {
    Universe,                          // All CBUs grouped by cluster
    Client { client_id: Uuid },        // Single client's CBU book
    Cbu { cbu_id: Uuid },              // Single CBU (current behavior)
    Entity { entity_id: Uuid },        // Entity detail focus
}
```

#### 2. UI Not Wired

**Current** (`ob-poc-ui/src/app.rs:634`):
```rust
if self.state.pending_scale_universe {
    web_sys::console::log_1(&"update: scale universe (full book view)".into());
    // TODO: Implement universe/full book view - zoom out to show all CBUs
    self.state.graph_widget.zoom_fit();  // <-- Just zooms, doesn't change view!
}
```

**Need:**
- Galaxy panel that shows when `view_level == Universe`
- Toolbar with zoom level indicator
- Click handler that calls `/api/universe` on first load
- `GalaxyAction::DrillDown` handler that switches to CBU view

#### 3. Session State Lacks Scope Level

**Current:** Session has `active_cbu: Option<CbuSummary>`

**Need:**
```rust
pub struct SessionContext {
    // Current scope level
    pub scope: SessionScope,
    
    // Breadcrumb: Universe > Allianz > Lux Fund A
    pub scope_stack: Vec<ScopeFrame>,
}

pub enum SessionScope {
    Universe,
    Client { client_id: Uuid, client_name: String },
    Cbu { cbu_id: Uuid, cbu_name: String },  // Existing
}
```

---

## Navigation Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              UNIVERSE                                       │
│                         (All CBUs, grouped)                                 │
│                                                                             │
│   [Luxembourg: 177]  [Ireland: 150]  [Germany: 200]  [France: 80]          │
│                                                                             │
│                    Click cluster → Filter to jurisdiction                   │
│                    Click CBU bubble → Drill to client book                  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                              Zoom In │ (or direct client search)
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          CLIENT BOOK                                        │
│                      (e.g., "Allianz SE")                                   │
│                                                                             │
│   Shared Entities (appear across multiple CBUs):                           │
│   ────────────────────────────────────────                                 │
│   [Allianz GI GmbH - ManCo - 35 CBUs]                                      │
│   [PIMCO Europe - IM - 12 CBUs]                                            │
│                                                                             │
│   CBUs:                                                                     │
│   ────────────────────────────────────────                                 │
│   [Allianz Lux Fund A]  [Allianz Lux Fund B]  [Allianz Ireland UCITS]     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                              Zoom In │ (click CBU)
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            CBU VIEW                                         │
│                    (Current implementation)                                 │
│                                                                             │
│   Switch lens: [KYC/UBO] [Trading] [Onboarding]                            │
│                                                                             │
│                          [CBU Node]                                         │
│                         /    |     \                                        │
│                    [ManCo] [IM]  [Director]                                │
│                       |                                                     │
│                    [UBO]                                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Priority

### Phase 1: Server API (Minimal Viable)

1. **`GET /api/universe?cluster_by=jurisdiction`**
   - Query CBUs grouped by jurisdiction
   - Return ClusterData format for GalaxyView

2. **`GET /api/commercial-clients`**
   - List distinct commercial_client_entity_ids with counts
   - Include basic entity info (name, LEI)

### Phase 2: Client Integration

1. **Add `ViewLevel` to AppState**
2. **Wire GalaxyView to real API data**
3. **Handle GalaxyAction::DrillDown → switch to CBU view**
4. **Add toolbar button to "zoom out" to universe**

### Phase 3: Client Book View

1. **`GET /api/commercial-client/:id/book`**
   - All CBUs for client
   - Shared entities highlighted

2. **Client book panel in UI**
   - Show shared ManCos, IMs with multi-CBU indicators
   - Click shared entity → show all CBUs it's in

### Phase 4: Cross-CBU Operations

1. **Update propagation** - Change ManCo director in one CBU → show affected CBUs
2. **Gap analysis** - KYC gaps across all client CBUs
3. **Clone operations** - New fund from template

---

## File Changes Required

### Server (`rust/src/`)

| File | Change |
|------|--------|
| `api/mod.rs` | Add universe_routes, commercial_client_routes |
| `api/universe_routes.rs` | NEW - `/api/universe` endpoint |
| `api/commercial_client_routes.rs` | NEW - `/api/commercial-clients`, `/api/commercial-client/:id/book` |
| `database/mod.rs` | Add universe_repository |
| `database/universe_repository.rs` | NEW - Cluster queries |

### Client (`rust/crates/ob-poc-ui/src/`)

| File | Change |
|------|--------|
| `state.rs` | Add `ViewLevel`, `galaxy_view`, `astronomy_state` |
| `app.rs` | Wire galaxy panel, handle drill-down actions |
| `api.rs` | Add `get_universe()`, `get_commercial_clients()` |
| `panels/mod.rs` | Add `galaxy_panel.rs` |
| `panels/galaxy_panel.rs` | NEW - Render GalaxyView widget |

### Shared (`rust/crates/ob-poc-graph/src/`)

| File | Change |
|------|--------|
| `graph/galaxy.rs` | Remove mock, add `set_clusters_from_api()` |
| `lib.rs` | Ensure GalaxyView, AstronomyView exported |

---

## Key Insight: The "Galaxy" Already Exists

The naming and metaphor are already baked in:
- `GalaxyView` - cluster visualization
- `AstronomyView::Universe` / `::SolarSystem` - zoom levels  
- `ForceConfig::galaxy()` - physics preset
- `astronomy_colors` - themed palette

Someone designed this system. It just needs to be connected to real data and wired into the app flow.

The hardest part isn't building new infrastructure - it's understanding what exists and connecting the dots.
