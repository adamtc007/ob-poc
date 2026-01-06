# Galaxy View: Node/Edge Taxonomy & Rendering Contract

## Overview

This document defines the complete contract between server and client for galaxy/universe visualization:
1. Node types at each zoom level
2. Edge types connecting nodes
3. Server response structures
4. Client rendering rules per LOD (level of detail)
5. Taxonomy tree traversal pattern

---

## Zoom Levels (Taxonomy Depth)

```
LEVEL 0: UNIVERSE
├── Clusters (by jurisdiction, client, risk, product)
│
LEVEL 1: CLUSTER  
├── CBU summaries (compact cards)
│
LEVEL 2: CBU (existing)
├── Entities with roles
├── Ownership chains
├── Trading profiles
│
LEVEL 3: ENTITY
├── Entity detail
├── Cross-CBU appearances
├── Document/evidence links
```

---

## Node Types by Level

### Level 0: Universe Nodes

| Node Type | Description | Visual |
|-----------|-------------|--------|
| `UniverseRoot` | Implicit root, not rendered | - |
| `JurisdictionCluster` | CBUs grouped by jurisdiction | Glowing orb, size ∝ cbu_count |
| `ClientCluster` | CBUs grouped by commercial client | Glowing orb, color by risk |
| `RiskCluster` | CBUs grouped by risk rating | Orb with risk color |
| `ProductCluster` | CBUs grouped by product type | Orb with product icon |

**Cluster Node Fields:**
```rust
pub struct ClusterNode {
    pub id: String,              // "jurisdiction:LU" or "client:{uuid}"
    pub cluster_type: ClusterType,
    pub label: String,           // "Luxembourg" or "Allianz SE"
    pub short_label: String,     // "LU" or "ALZ"
    pub cbu_count: usize,
    pub cbu_ids: Vec<Uuid>,      // For drill-down
    pub risk_summary: RiskSummary,
    pub position: Option<Vec2>,  // Client computes via force sim
    pub radius: f32,             // Computed from cbu_count
}

pub enum ClusterType {
    Jurisdiction,
    Client,
    Risk,
    Product,
}
```

### Level 1: Cluster Detail Nodes

| Node Type | Description | Visual |
|-----------|-------------|--------|
| `ClusterHeader` | Cluster identity banner | Fixed top bar |
| `CbuCard` | Compact CBU summary | Card with risk badge |
| `SharedEntityMarker` | Entity appearing in multiple CBUs | Connector line hub |

**CBU Card Node Fields:**
```rust
pub struct CbuCardNode {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub risk_rating: RiskRating,
    pub status: CbuStatus,
    pub entity_count: usize,
    pub completion_pct: f32,      // KYC completion
}
```

### Level 2: CBU Graph Nodes (Existing)

| Node Type | Description | Visual |
|-----------|-------------|--------|
| `CbuContainer` | The CBU itself | Rounded rect container |
| `LegalEntity` | Company, fund, trust | Box with entity icon |
| `NaturalPerson` | Individual (UBO, director) | Circle with person icon |
| `TradingProfile` | Trading config root | Diamond |
| `InstrumentClass` | Equity, Fixed Income, etc. | Colored tag |
| `Market` | Exchange (XNYS, XLON) | Square with MIC |
| `Document` | KYC document | Paper icon |

### Level 3: Entity Detail Nodes

| Node Type | Description | Visual |
|-----------|-------------|--------|
| `EntityHeader` | Entity identity | Large card |
| `CbuAppearance` | CBU where entity appears | Mini CBU badge |
| `RoleAssignment` | Role in specific CBU | Role tag |
| `DocumentLink` | Associated document | Paper icon |
| `OwnershipLink` | Ownership relationship | Arrow with % |

---

## Edge Types by Level

### Level 0: Universe Edges

| Edge Type | From | To | Visual |
|-----------|------|-----|--------|
| `ClusterProximity` | Cluster | Cluster | Faint line (optional, for related clusters) |

**Note:** At universe level, clusters float independently via force simulation. Edges are optional visual hints for related clusters (e.g., same client spans multiple jurisdictions).

### Level 1: Cluster Detail Edges

| Edge Type | From | To | Visual |
|-----------|------|-----|--------|
| `SharedEntity` | CbuCard | CbuCard | Dashed line through shared entity marker |
| `ClientOwnership` | ClientHeader | CbuCard | Ownership arrow (if showing client→CBU hierarchy) |

### Level 2: CBU Graph Edges (Existing + Extended)

| Edge Type | Code | From | To | Visual |
|-----------|------|------|-----|--------|
| `CbuRole` | `CBU_ROLE` | CbuContainer | Entity | Solid line with role label |
| `Ownership` | `OWNS` | Entity | Entity | Arrow with % label |
| `Control` | `CONTROLS` | Entity | Entity | Dashed arrow |
| `BoardMember` | `BOARD_MEMBER` | Person | Entity | Thin solid |
| `TrustRelation` | `TRUST_*` | Entity | Entity | Dotted with role |
| `HasTradingProfile` | `HAS_TRADING_PROFILE` | Entity | TradingProfile | Solid blue |
| `HasMatrix` | `HAS_MATRIX` | TradingProfile | InstrumentClass | Solid |
| `TradedOn` | `TRADED_ON` | InstrumentClass | Market | Thin solid |
| `IsdaCoverage` | `ISDA_COVERS` | IsdaAgreement | Counterparty | Red solid |

### Level 3: Entity Detail Edges

| Edge Type | From | To | Visual |
|-----------|------|-----|--------|
| `AppearsIn` | EntityHeader | CbuAppearance | Solid line |
| `HasRole` | EntityHeader | RoleAssignment | Thin line with role |
| `HasDocument` | EntityHeader | DocumentLink | Dotted line |
| `OwnsUpstream` | EntityHeader | OwnershipLink | Arrow pointing up |
| `OwnedByDownstream` | EntityHeader | OwnershipLink | Arrow pointing down |

---

## Server Response Structures

### Universe Response (`GET /api/universe`)

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct UniverseGraph {
    /// Graph metadata
    pub scope: GraphScope,
    pub as_of: NaiveDate,
    pub total_cbu_count: usize,
    
    /// Cluster nodes (the main content)
    pub clusters: Vec<ClusterNode>,
    
    /// Optional: cross-cluster edges
    pub cluster_edges: Vec<ClusterEdge>,
    
    /// Aggregated stats
    pub stats: UniverseStats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterNode {
    pub id: String,                    // "jurisdiction:LU"
    pub node_type: ClusterNodeType,
    pub cluster_type: ClusterType,
    pub label: String,
    pub short_label: String,
    pub cbu_count: usize,
    pub cbu_ids: Vec<Uuid>,
    pub risk_summary: RiskSummary,
    
    // Layout hints (server can suggest, client decides)
    pub suggested_radius: f32,
    pub suggested_color: Option<String>,  // Hex color
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterEdge {
    pub id: String,
    pub edge_type: ClusterEdgeType,
    pub source_cluster_id: String,
    pub target_cluster_id: String,
    pub weight: f32,                   // Strength of relationship
    pub label: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RiskSummary {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
    pub unrated: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UniverseStats {
    pub total_clusters: usize,
    pub total_cbus: usize,
    pub total_entities: usize,
    pub risk_distribution: RiskSummary,
}
```

### Cluster Detail Response (`GET /api/cluster/:type/:id`)

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterDetailGraph {
    /// Which cluster this is
    pub cluster: ClusterNode,
    
    /// CBU cards within this cluster
    pub cbus: Vec<CbuCardNode>,
    
    /// Shared entities (appear in >1 CBU)
    pub shared_entities: Vec<SharedEntityNode>,
    
    /// Edges showing shared entity connections
    pub shared_edges: Vec<SharedEntityEdge>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CbuCardNode {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub risk_rating: RiskRating,
    pub status: CbuStatus,
    pub entity_count: usize,
    pub completion_pct: f32,
    
    // Which shared entities this CBU contains
    pub shared_entity_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SharedEntityNode {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: EntityType,
    pub roles: Vec<String>,
    pub cbu_count: usize,
    pub cbu_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SharedEntityEdge {
    pub id: String,
    pub shared_entity_id: Uuid,
    pub cbu_id: Uuid,
    pub roles: Vec<String>,
}
```

### CBU Graph Response (Existing, for reference)

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct CbuGraph {
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub view_mode: String,
    pub nodes: Vec<GraphNodeData>,
    pub edges: Vec<GraphEdgeData>,
    pub layout_bounds: Rect,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphNodeData {
    pub id: String,
    pub node_type: String,           // "LEGAL_ENTITY", "NATURAL_PERSON", etc.
    pub label: String,
    pub layer: String,               // "core", "ownership", "trading"
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphEdgeData {
    pub id: String,
    pub edge_type: String,
    pub source: String,
    pub target: String,
    pub label: Option<String>,
    pub weight: Option<f32>,
}
```

### Entity Detail Response (`GET /api/entity/:id/detail`)

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct EntityDetailGraph {
    /// The focal entity
    pub entity: EntityDetailNode,
    
    /// CBUs where this entity appears
    pub cbu_appearances: Vec<CbuAppearanceNode>,
    
    /// Ownership relationships (up and down)
    pub ownership_chain: OwnershipChainData,
    
    /// Documents/evidence
    pub documents: Vec<DocumentNode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityDetailNode {
    pub entity_id: Uuid,
    pub entity_type: EntityType,
    pub name: String,
    pub lei: Option<String>,
    pub jurisdiction: Option<String>,
    pub incorporation_date: Option<NaiveDate>,
    pub status: EntityStatus,
    pub attributes: serde_json::Value,  // Type-specific fields
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CbuAppearanceNode {
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub roles: Vec<RoleInCbu>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoleInCbu {
    pub role_id: Uuid,
    pub role_name: String,
    pub effective_from: Option<NaiveDate>,
    pub effective_to: Option<NaiveDate>,
    pub target_entity_id: Option<Uuid>,  // For directed roles
    pub ownership_pct: Option<Decimal>,
}
```

---

## Client Rendering Rules by LOD

### LOD 0: Universe View (GalaxyView)

**When:** `zoom < 0.3` or `ViewLevel::Universe`

**Data Source:** `UniverseGraph`

**Rendering:**
```
┌─────────────────────────────────────────────────────────────────┐
│                         UNIVERSE                                │
│                                                                 │
│     ○ LU (177)              ○ IE (150)                         │
│         ◐                       ◐                              │
│                                                                 │
│              ○ DE (200)                    ○ FR (80)           │
│                  ◐                             ◐               │
│                                                                 │
│                      ○ UK (45)      ○ CH (19)                  │
│                          ◐              ◐                       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

Legend:
  ○ = Cluster orb (size ∝ sqrt(cbu_count))
  ◐ = Glow (color = dominant risk)
```

**Rendering Rules:**
1. Use `ForceSimulation` with `ForceConfig::galaxy()` for positioning
2. Orb radius = `20.0 + sqrt(cbu_count) * 3.0`
3. Orb color = `risk_summary.dominant()` mapped via `astronomy_colors::risk_color()`
4. Glow radius = `orb_radius * 1.3`
5. Label = `short_label` when compressed, `label` when expanded
6. Count badge below label when `compression < 0.7`

**Interactions:**
- Hover: Increase glow, show tooltip with full stats
- Click: `GalaxyAction::DrillDown` → transition to cluster detail
- Drag: Pin cluster, move position
- Scroll: Zoom in/out (affects compression factor)

**Code Path:**
```
GalaxyView::ui()
  → ForceSimulation::tick()
  → for each ClusterNode:
      → render_cluster(painter, pos, radius, node, glow, compression)
```

---

### LOD 1: Cluster Detail View

**When:** Drilled into a cluster from universe, or `ViewLevel::Client`

**Data Source:** `ClusterDetailGraph`

**Rendering:**
```
┌─────────────────────────────────────────────────────────────────┐
│  ← Back    LUXEMBOURG (177 CBUs)                    [Filter ▾]  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Shared Entities:                                               │
│  ┌─────────────────┐   ┌─────────────────┐                     │
│  │ Allianz GI GmbH │───│ PIMCO Europe    │                     │
│  │ ManCo (35 CBUs) │   │ IM (12 CBUs)    │                     │
│  └────────┬────────┘   └────────┬────────┘                     │
│           │                     │                               │
│  ─────────┼─────────────────────┼───────────────                │
│           │                     │                               │
│  ┌────────┴────────┐   ┌───────┴─────────┐   ┌───────────────┐ │
│  │ Fund A     [●]  │   │ Fund B     [◐]  │   │ Fund C   [○]  │ │
│  │ UCITS  LU       │   │ UCITS  LU       │   │ AIF    LU     │ │
│  │ 23 entities     │   │ 18 entities     │   │ 12 entities   │ │
│  └─────────────────┘   └─────────────────┘   └───────────────┘ │
│                                                                 │
│  ┌─────────────────┐   ┌─────────────────┐   ...               │
│  │ Fund D     [●]  │   │ Fund E     [◐]  │                     │
│  │ UCITS  LU       │   │ UCITS  LU       │                     │
│  └─────────────────┘   └─────────────────┘                     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

Legend:
  [●] = LOW risk (green)
  [◐] = MEDIUM risk (amber)  
  [○] = HIGH risk (red)
  ─── = Shared entity connection
```

**Rendering Rules:**
1. Grid layout for CBU cards (responsive columns)
2. Card size = fixed width, height varies with content
3. Risk badge = colored circle in top-right
4. Shared entities = hub nodes above grid with lines to connected CBUs
5. Shared entity lines use `astronomy_colors::orbit_line()` (faint)

**Interactions:**
- Click CBU card: `ClusterAction::OpenCbu(cbu_id)` → transition to CBU graph
- Click shared entity: Highlight all connected CBU cards
- Filter dropdown: Filter by risk, client_type, status
- Back button: Return to universe view

**Code Path:**
```
ClusterDetailPanel::ui()
  → render_header(cluster)
  → render_shared_entities(shared_entities, shared_edges)
  → render_cbu_grid(cbus)
      → for each CbuCardNode:
          → render_cbu_card(card, is_highlighted)
```

---

### LOD 2: CBU Graph View (Existing + Enhanced)

**When:** Drilled into a CBU, or direct CBU selection

**Data Source:** `CbuGraph` (existing)

**Rendering:** (Current implementation with enhancements)

```
┌─────────────────────────────────────────────────────────────────┐
│  ← Back to LU    ALLIANZ LUX FUND A              [KYC▾] [⚙]    │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│                    ┌─────────────────────┐                      │
│                    │   FUND A (UCITS)    │                      │
│                    │   ══════════════    │                      │
│                    └──────────┬──────────┘                      │
│                               │                                 │
│         ┌─────────────────────┼─────────────────────┐          │
│         │                     │                     │          │
│    ┌────┴────┐          ┌────┴────┐          ┌────┴────┐      │
│    │ ManCo   │          │ IM      │          │ Custodian│      │
│    │ Allianz │          │ PIMCO   │          │ BNY      │      │
│    └────┬────┘          └────┬────┘          └─────────┘      │
│         │                    │                                  │
│    ┌────┴────┐          ┌────┴────┐                            │
│    │Director │          │Portfolio│                            │
│    │ J.Smith │          │ Mgr     │                            │
│    └────┬────┘          └─────────┘                            │
│         │                                                       │
│    ┌────┴────┐                                                  │
│    │  UBO    │    ← Ownership chain terminus                   │
│    │ Person  │                                                  │
│    └─────────┘                                                  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**View Modes** (lens switch):
- `KYC_UBO`: Ownership chains, control, UBO terminus
- `TRADING`: Trading profiles, instrument matrix, markets
- `SERVICE`: Service providers, custodian, admin, broker

**Rendering Rules:** (Existing `CbuGraphWidget` + `LayoutEngineV2`)

**Interactions:**
- Click entity: Select, show focus card with details
- Double-click entity: `CbuAction::OpenEntity(entity_id)` → entity detail
- View mode switch: Re-fetch graph with different `view_mode` param
- Back button: Return to cluster detail (or universe if direct)

---

### LOD 3: Entity Detail View

**When:** Drilled into an entity from CBU graph

**Data Source:** `EntityDetailGraph`

**Rendering:**
```
┌─────────────────────────────────────────────────────────────────┐
│  ← Back to Fund A    ALLIANZ GLOBAL INVESTORS GMBH     [Edit]  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │  ALLIANZ GLOBAL INVESTORS GMBH                            │ │
│  │  ─────────────────────────────────────────────────────    │ │
│  │  Type: Limited Company    LEI: 5299009QD...               │ │
│  │  Jurisdiction: DE         Incorporated: 1998-03-15        │ │
│  │  Status: ACTIVE                                           │ │
│  └───────────────────────────────────────────────────────────┘ │
│                                                                 │
│  Appears in 35 CBUs:                                           │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────┐  │
│  │ Fund A      │ │ Fund B      │ │ Fund C      │ │ +32 more│  │
│  │ ManCo       │ │ ManCo       │ │ ManCo       │ │         │  │
│  └─────────────┘ └─────────────┘ └─────────────┘ └─────────┘  │
│                                                                 │
│  Ownership:                                                     │
│       ┌─────────────┐                                          │
│       │ Allianz SE  │  ← 100% owner                            │
│       │ (Ultimate)  │                                          │
│       └──────┬──────┘                                          │
│              │ 100%                                             │
│       ┌──────┴──────┐                                          │
│       │ THIS ENTITY │                                          │
│       └─────────────┘                                          │
│                                                                 │
│  Documents:                                                     │
│  📄 Certificate of Incorporation (verified)                    │
│  📄 Director Resolution 2024 (pending review)                  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Rendering Rules:**
1. Entity header card with all attributes
2. CBU appearances as mini badges (scrollable if many)
3. Ownership chain as vertical tree (up to ultimate, down to subsidiaries)
4. Documents as list with status icons

**Interactions:**
- Click CBU badge: Navigate to that CBU's graph
- Click owner/subsidiary: Navigate to that entity's detail
- Click document: Open document viewer
- Edit button: Open entity edit form

---

## Taxonomy Tree Traversal Pattern

### Navigation Stack Structure

```rust
pub struct NavigationStack {
    frames: Vec<NavigationFrame>,
    max_depth: usize,
}

pub struct NavigationFrame {
    pub level: ViewLevel,
    pub scope: NavigationScope,
    pub label: String,
    pub entered_at: DateTime<Utc>,
}

pub enum ViewLevel {
    Universe,
    Cluster,
    Cbu,
    Entity,
}

pub enum NavigationScope {
    Universe,
    Cluster { cluster_type: ClusterType, cluster_id: String },
    Cbu { cbu_id: Uuid },
    Entity { entity_id: Uuid },
}
```

### Navigation Operations

```rust
impl NavigationStack {
    /// Push a new frame (drill down)
    pub fn push(&mut self, frame: NavigationFrame) {
        self.frames.push(frame);
        if self.frames.len() > self.max_depth {
            self.frames.remove(0);
        }
    }
    
    /// Pop current frame (go back)
    pub fn pop(&mut self) -> Option<NavigationFrame> {
        if self.frames.len() > 1 {
            self.frames.pop()
        } else {
            None  // Can't pop universe root
        }
    }
    
    /// Jump to specific level (breadcrumb click)
    pub fn jump_to(&mut self, index: usize) {
        if index < self.frames.len() {
            self.frames.truncate(index + 1);
        }
    }
    
    /// Get breadcrumb trail
    pub fn breadcrumbs(&self) -> Vec<Breadcrumb> {
        self.frames.iter().enumerate().map(|(i, f)| Breadcrumb {
            index: i,
            label: f.label.clone(),
            level: f.level.clone(),
        }).collect()
    }
    
    /// Current view level
    pub fn current_level(&self) -> ViewLevel {
        self.frames.last().map(|f| f.level.clone()).unwrap_or(ViewLevel::Universe)
    }
}
```

### Traversal Flow

```
User at UNIVERSE
  │
  ├─ Click "Luxembourg" cluster
  │    └─ push(Cluster { jurisdiction, "LU" })
  │    └─ fetch GET /api/cluster/jurisdiction/LU
  │    └─ render ClusterDetailPanel
  │
  ├─ Click "Fund A" CBU card  
  │    └─ push(Cbu { cbu_id })
  │    └─ fetch GET /api/cbu/{id}/graph?view_mode=KYC_UBO
  │    └─ render CbuGraphWidget
  │
  ├─ Double-click "Allianz GI" entity
  │    └─ push(Entity { entity_id })
  │    └─ fetch GET /api/entity/{id}/detail
  │    └─ render EntityDetailPanel
  │
  ├─ Click breadcrumb "Luxembourg"
  │    └─ jump_to(1)  // Index of cluster frame
  │    └─ render ClusterDetailPanel (cached or re-fetch)
  │
  └─ Click back button
       └─ pop()
       └─ render previous level
```

### Data Caching Strategy

```rust
pub struct ViewCache {
    /// Universe data (rarely changes, cache longer)
    universe: Option<CachedData<UniverseGraph>>,
    
    /// Cluster details (keyed by cluster_id)
    clusters: HashMap<String, CachedData<ClusterDetailGraph>>,
    
    /// CBU graphs (keyed by cbu_id + view_mode)
    cbus: HashMap<(Uuid, String), CachedData<CbuGraph>>,
    
    /// Entity details (keyed by entity_id)
    entities: HashMap<Uuid, CachedData<EntityDetailGraph>>,
}

pub struct CachedData<T> {
    data: T,
    fetched_at: DateTime<Utc>,
    ttl: Duration,
}

impl<T> CachedData<T> {
    pub fn is_stale(&self) -> bool {
        Utc::now() - self.fetched_at > self.ttl
    }
}
```

**TTL Guidelines:**
- Universe: 5 minutes (aggregate data, slow to change)
- Cluster: 2 minutes
- CBU: 1 minute (may have active edits)
- Entity: 1 minute

---

## Rendering State Machine

```rust
pub enum RenderState {
    /// Showing universe view
    Universe {
        data: UniverseGraph,
        galaxy_view: GalaxyView,
        hovered_cluster: Option<String>,
    },
    
    /// Showing cluster detail
    ClusterDetail {
        cluster: ClusterNode,
        data: ClusterDetailGraph,
        selected_cbu: Option<Uuid>,
        filter: ClusterFilter,
    },
    
    /// Showing CBU graph
    CbuGraph {
        cbu_id: Uuid,
        view_mode: ViewMode,
        data: CbuGraph,
        widget: CbuGraphWidget,
    },
    
    /// Showing entity detail
    EntityDetail {
        entity_id: Uuid,
        data: EntityDetailGraph,
    },
    
    /// Transitioning between states
    Transitioning {
        from: Box<RenderState>,
        to: Box<RenderState>,
        progress: f32,  // 0.0 to 1.0
        animation: TransitionAnimation,
    },
    
    /// Loading data for a state
    Loading {
        target: LoadingTarget,
        started_at: f64,
    },
    
    /// Error state
    Error {
        message: String,
        retry_target: Option<LoadingTarget>,
    },
}

pub enum TransitionAnimation {
    ZoomIn,   // Universe → Cluster, Cluster → CBU
    ZoomOut,  // Reverse
    CrossFade, // Same level switch
}
```

---

## Summary: What Each Component Needs

### Server Responsibilities

1. **Return typed nodes and edges** with consistent ID schemes
2. **Include layout hints** (suggested radius, color) but let client decide final positions
3. **Support view_mode parameter** for CBU graphs
4. **Include risk_summary** at every aggregation level
5. **Return shared_entity connections** for cluster detail

### Client Responsibilities

1. **Maintain NavigationStack** for breadcrumb and back/forward
2. **Cache fetched data** with appropriate TTL
3. **Run ForceSimulation** for universe cluster positioning
4. **Handle transitions** smoothly between levels
5. **Render appropriate panel** based on current RenderState

### Type Alignment

Server response types (`UniverseGraph`, `ClusterDetailGraph`, etc.) should be defined in `ob-poc-types` crate and shared between server and WASM client. This ensures:
- Same struct definitions on both sides
- Serde serialization/deserialization works
- Type changes caught at compile time
