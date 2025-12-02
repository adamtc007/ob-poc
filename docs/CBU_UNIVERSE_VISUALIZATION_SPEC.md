# CBU Universe Visualization Specification

## Overview

An interactive Rust→Wasm visualization with two primary views:

1. **Universe View** - All CBUs as force-directed nodes, clustered by jurisdiction, client type, and product mix
2. **Solar System View** - Single-CBU deep dive with orbiting entities, services, resources, KYC cases, and documents

Built in Rust, compiled to WebAssembly, rendered via WebGL/Canvas.

---

## 1. Universe View

### 1.1 Conceptual Model

```
┌─────────────────────────────────────────────────────────────────┐
│                        CBU UNIVERSE                              │
│                                                                  │
│    ┌──────────┐                         ┌──────────┐            │
│    │ EUROPE   │                         │   APAC   │            │
│    │ cluster  │                         │ cluster  │            │
│    │  ○ ○     │                         │    ○     │            │
│    │ ○   ○    │      ┌──────────┐       │  ○   ○   │            │
│    │    ○     │      │ AMERICAS │       │    ○     │            │
│    └──────────┘      │ cluster  │       └──────────┘            │
│                      │  ●  ○    │                                │
│    ┌──────────┐      │ ○    ●   │       ┌──────────┐            │
│    │  FUNDS   │      └──────────┘       │ OFFSHORE │            │
│    │ cluster  │                         │ cluster  │            │
│    │  ◉  ◎    │                         │   ◎  ○   │            │
│    └──────────┘                         └──────────┘            │
│                                                                  │
│  ○ = CBU node    ● = selected    ◉ = high risk    ◎ = onboarding│
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 Node Representation (CBU)

Each CBU is a circular node with:

| Visual Property | Data Source | Encoding |
|-----------------|-------------|----------|
| **Position** | Force simulation | Attracted to cluster anchors |
| **Size** | Economic weight (AUM, revenue, account balance) | Radius 10-50px (log scale) |
| **Fill Color** | KYC risk rating | Gradient: green→yellow→orange→red→black |
| **Halo** | Onboarding status | Animated ring: pulsing=active, solid=complete, dashed=blocked |
| **Border** | CBU category | Solid=Fund, Dashed=Corporate, Dotted=Individual |
| **Icon** | Client type | Optional center glyph |
| **Label** | CBU name | Shown on hover or zoom |

#### Color Palette (KYC Risk)
```
UNRATED:    #808080 (grey)
STANDARD:   #4CAF50 (green)
LOW:        #8BC34A (light green)
MEDIUM:     #FFC107 (amber)
HIGH:       #FF5722 (deep orange)
PROHIBITED: #212121 (near black)
```

#### Halo States (Onboarding)
```
PROSPECT:    No halo
ONBOARDING:  Pulsing blue ring (animated)
ONBOARDED:   Solid green ring
ACTIVE:      No ring (clean)
SUSPENDED:   Solid red ring
BLOCKED:     Flashing red ring
EXITING:     Fading ring
```

### 1.3 Cluster Model

Clusters are **invisible anchor points** that attract CBUs. Multiple cluster dimensions overlay:

#### Primary Clusters (Jurisdiction)
```rust
struct JurisdictionCluster {
    jurisdiction_code: String,      // "LU", "UK", "US", "KY"
    anchor_position: Vec2,          // Fixed position in universe
    attraction_strength: f32,       // How strongly CBUs are pulled
    color_hint: Color,              // For debug/optional region shading
}
```

Anchor positions form a world-map-ish layout:
```
        [UK] [EU]      [APAC]
[US]           [CH]         [SG] [HK]
        [KY] [BVI]     [AU]
```

#### Secondary Clusters (Client Type)
Within jurisdiction regions, sub-clustering by client type:
```
FUND_MANDATE      → upper region
CORPORATE_GROUP   → middle region  
INSTITUTIONAL     → lower region
```

#### Tertiary Clusters (Product Mix)
Fine-grained attraction based on services:
```
Custody-heavy     → slight left pull
TA/Registry-heavy → slight right pull
Execution-heavy   → slight down pull
```

### 1.4 Force Model

Standard force-directed graph with custom forces:

```rust
struct ForceSimulation {
    // Node-node repulsion (all nodes push apart)
    repulsion: RepulsionForce {
        strength: -300.0,
        distance_cap: 200.0,
    },
    
    // Cluster attraction (nodes pulled to anchors)
    cluster_attraction: Vec<ClusterForce> {
        // Each cluster exerts pull on matching nodes
        strength: 0.1..0.5,  // Varies by cluster type
    },
    
    // Center gravity (keeps universe from drifting)
    center_gravity: CenterForce {
        strength: 0.01,
        center: Vec2::ZERO,
    },
    
    // Collision prevention
    collision: CollisionForce {
        radius: |node| node.visual_radius + 5.0,
        strength: 0.7,
    },
    
    // Velocity damping (prevents oscillation)
    damping: 0.9,
    
    // Simulation parameters
    alpha: 1.0,           // Current "heat"
    alpha_decay: 0.0228,  // Cooling rate
    alpha_min: 0.001,     // Stop threshold
}
```

#### Force Equations

**Repulsion** (Coulomb's law variant):
```
F_repel = -strength * (1 / distance²)
```

**Cluster Attraction** (spring force):
```
F_attract = strength * (distance_to_anchor) * membership_weight
```
Where `membership_weight` is 1.0 for primary cluster, 0.3 for secondary, 0.1 for tertiary.

**Damping**:
```
velocity = velocity * damping
position = position + velocity
```

### 1.5 Interaction Model

| Action | Effect |
|--------|--------|
| **Pan** | Drag empty space → translate viewport |
| **Zoom** | Scroll wheel / pinch → scale around cursor |
| **Hover** | Show CBU tooltip (name, status, risk, key metrics) |
| **Click** | Select CBU, highlight connections |
| **Double-click** | Transition to Solar System view |
| **Right-click** | Context menu (view details, start workflow, etc.) |
| **Drag node** | Pin node position, reheat simulation |
| **Filter panel** | Toggle visibility by jurisdiction/type/status |

### 1.6 Filtering

Filter controls (sidebar or floating panel):

```rust
struct UniverseFilters {
    jurisdictions: HashSet<String>,    // Show only these
    client_types: HashSet<CbuCategory>,
    risk_ratings: HashSet<RiskRating>,
    onboarding_status: HashSet<OnboardingState>,
    search_query: String,              // Name search
    min_economic_weight: Option<f64>,
    has_open_kyc_case: Option<bool>,
}
```

Filtered-out nodes either:
- Fade to 10% opacity (ghost mode)
- Hide completely
- Collapse to cluster summary node

---

## 2. Solar System View (Single CBU)

### 2.1 Conceptual Model

```
┌─────────────────────────────────────────────────────────────────┐
│                     CBU SOLAR SYSTEM                             │
│                                                                  │
│                         [Doc]                                    │
│                    [Doc]     [Doc]                               │
│                                                                  │
│         [Entity]                      [Service]                  │
│                     ┌─────────┐                                  │
│    [Entity]         │   CBU   │          [Service]               │
│                     │ CENTER  │                                  │
│         [Entity]    └─────────┘      [Resource]                  │
│                                                                  │
│              [KYC Case]        [Resource]                        │
│                    [KYC Case]                                    │
│                                                                  │
│  ORBITAL RINGS:                                                  │
│  Ring 1 (inner):  Entities (by role importance)                 │
│  Ring 2:          Services & Resources                           │
│  Ring 3:          KYC Cases & Workstreams                        │
│  Ring 4 (outer):  Documents                                      │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Orbital Layers

| Ring | Radius | Contents | Orbital Speed |
|------|--------|----------|---------------|
| **0** | 0 | CBU (center, fixed) | Static |
| **1** | 80-120px | Entities by role | Slow (0.01 rad/s) |
| **2** | 150-200px | Services, Resources | Medium (0.005 rad/s) |
| **3** | 230-280px | KYC Cases, Workstreams | Medium (0.003 rad/s) |
| **4** | 320-400px | Documents | Fast (0.02 rad/s) |

### 2.3 Entity Orbital Positioning

Entities are positioned by **role importance** (not random):

```rust
enum EntityOrbitalPriority {
    // Inner positions (closest to CBU center)
    CommercialClient = 0,   // The contracting entity - always at "12 o'clock"
    Principal = 1,          // Fund vehicle, main operating entity
    
    // Middle positions
    Director = 2,
    UltimateBeneficialOwner = 3,
    AuthorizedSignatory = 4,
    
    // Outer positions
    Investor = 5,
    ServiceProvider = 6,    // Admin, custodian, auditor
    
    // Peripheral
    RelatedParty = 7,
    Other = 8,
}
```

Entities with multiple roles use highest priority.

#### Entity Node Encoding

| Visual Property | Data Source | Encoding |
|-----------------|-------------|----------|
| **Shape** | Entity type | Circle=Person, Rounded rect=Company, Diamond=Fund, Hexagon=Trust |
| **Size** | Ownership % or role importance | 15-40px |
| **Fill** | Entity KYC status | Same risk palette as CBU |
| **Border** | Verification status | Solid=Verified, Dashed=Pending, Dotted=Allegation only |
| **Badge** | Role | Small icon (crown=UBO, briefcase=Director, etc.) |
| **Connection** | Role to CBU | Line style varies by role type |

### 2.4 Service/Resource Nodes (Ring 2)

```rust
struct ServiceNode {
    service_id: Uuid,
    service_name: String,
    delivery_status: DeliveryStatus,  // PENDING, DELIVERED, FAILED
    
    // Visual
    shape: Shape::RoundedRect,
    color: status_color(delivery_status),
    icon: service_type_icon(service_type),
}

struct ResourceNode {
    instance_id: Uuid,
    resource_type: String,  // "CUSTODY_ACCOUNT", "SWIFT_BIC", etc.
    status: ResourceStatus, // PENDING, ACTIVE, SUSPENDED
    
    // Visual
    shape: Shape::Rect,
    color: status_color(status),
    icon: resource_type_icon(resource_type),
}
```

### 2.5 KYC Case Nodes (Ring 3)

```rust
struct KycCaseNode {
    case_id: Uuid,
    case_type: CaseType,      // NEW_CLIENT, PERIODIC_REVIEW, EVENT_DRIVEN
    status: CaseStatus,
    risk_rating: RiskRating,
    
    // Visual
    shape: Shape::Pentagon,   // Distinctive shape for cases
    color: risk_color(risk_rating),
    pulsing: status.is_active(),
    
    // Sub-nodes (workstreams) orbit this case node
    workstreams: Vec<WorkstreamNode>,
}
```

### 2.6 Document Nodes (Ring 4)

Documents are small, numerous, and grouped by category:

```rust
struct DocumentNode {
    doc_id: Uuid,
    doc_type: String,
    category: DocCategory,    // IDENTITY, CORPORATE, FINANCIAL, etc.
    
    // Grouping - documents of same category cluster together in their arc
    cluster_angle: f32,       // Category determines base angle
    
    // Visual
    shape: Shape::SmallRect,  // Like a page
    color: category_color(category),
    icon: doc_type_icon(doc_type),
    
    // Status indicators
    has_expiry_warning: bool, // Yellow corner if expiring soon
    is_missing: bool,         // Red outline if required but missing
}
```

Document category arc positions:
```
IDENTITY:   0° - 45°    (top right)
CORPORATE:  45° - 90°   (right)
FINANCIAL:  90° - 135°  (bottom right)
TAX:        135° - 180° (bottom)
ADDRESS:    180° - 225° (bottom left)
REGULATORY: 225° - 270° (left)
UBO:        270° - 315° (top left)
OTHER:      315° - 360° (top)
```

### 2.7 Connections (Edges)

Edges connect related nodes:

| Connection Type | Style | Example |
|-----------------|-------|---------|
| Entity → CBU (role) | Solid line, color by role | Director → CBU |
| Entity → Entity (ownership) | Thick line with % label | HoldCo → SubCo (100%) |
| Entity → Entity (control) | Dashed arrow | Director → Company |
| Service → Resource | Thin dotted | Custody Service → Account |
| Document → Entity | Faint line (on hover) | Passport → Person |
| KYC Case → Entity | Highlighted when active | Case → Subject entities |

### 2.8 Solar System Forces

Different force model than Universe view - **orbital mechanics**:

```rust
struct OrbitalSimulation {
    // Radial force - keeps nodes at their ring distance
    radial: RadialForce {
        target_radius: |node| ring_radius(node.orbital_ring),
        strength: 0.3,
    },
    
    // Angular force - slow rotation
    orbital: OrbitalForce {
        angular_velocity: |node| ring_speed(node.orbital_ring),
    },
    
    // Angular repulsion - spread nodes within ring
    angular_repulsion: AngularRepulsionForce {
        strength: 0.05,
        same_ring_only: true,
    },
    
    // Category clustering - same-category nodes attract angularly
    category_attraction: CategoryClusterForce {
        strength: 0.02,
    },
}
```

---

## 3. View Transitions

### 3.1 Universe → Solar System

When user double-clicks a CBU:

1. **Zoom** - Camera zooms toward selected CBU
2. **Fade** - Other CBUs fade to 0% opacity
3. **Expand** - Selected CBU grows, internal structure emerges
4. **Spawn** - Orbital nodes fly out from CBU center to their rings
5. **Stabilize** - Orbital forces take over

Animation duration: ~800ms with easing.

```rust
fn transition_to_solar_system(cbu_id: Uuid) {
    // Phase 1: Zoom (0-300ms)
    camera.animate_to(cbu_position, zoom: 2.0, duration: 300ms, ease: EaseOutQuad);
    
    // Phase 2: Fade others (200-500ms)
    for other_cbu in universe.nodes.except(cbu_id) {
        other_cbu.animate_opacity(0.0, duration: 300ms, delay: 200ms);
    }
    
    // Phase 3: Expand center (300-600ms)
    selected_cbu.animate_radius(80.0, duration: 300ms, delay: 300ms);
    
    // Phase 4: Spawn orbitals (400-800ms)
    for (i, orbital) in solar_system.nodes.enumerate() {
        orbital.animate_from_center(
            target: orbital.ring_position(),
            duration: 400ms,
            delay: 400ms + i * 20ms,  // Staggered
            ease: EaseOutBack,  // Slight overshoot
        );
    }
    
    // Phase 5: Enable orbital forces
    simulation.switch_to(OrbitalSimulation);
}
```

### 3.2 Solar System → Universe

When user clicks "back" or presses Escape:

1. **Collapse** - Orbital nodes fly back to CBU center
2. **Shrink** - CBU returns to normal size
3. **Fade in** - Other CBUs reappear
4. **Zoom out** - Camera returns to universe view
5. **Reheat** - Universe forces resume

---

## 4. Data Model

### 4.1 Graph Data Structures

```rust
// Universe view data
struct CbuUniverse {
    nodes: Vec<CbuNode>,
    clusters: Vec<ClusterAnchor>,
    filters: UniverseFilters,
    simulation: ForceSimulation,
    camera: Camera2D,
}

struct CbuNode {
    cbu_id: Uuid,
    name: String,
    
    // Cluster membership
    jurisdiction: String,
    cbu_category: CbuCategory,
    products: Vec<ProductType>,
    
    // Visual properties (from v_cbu_lifecycle)
    risk_rating: RiskRating,
    onboarding_state: OnboardingState,
    economic_weight: f64,
    
    // Simulation state
    position: Vec2,
    velocity: Vec2,
    pinned: bool,
    
    // Computed visual
    radius: f32,
    color: Color,
    halo: Option<HaloState>,
}

// Solar system view data
struct CbuSolarSystem {
    center: CbuCenterNode,
    entities: Vec<EntityNode>,
    services: Vec<ServiceNode>,
    resources: Vec<ResourceNode>,
    kyc_cases: Vec<KycCaseNode>,
    documents: Vec<DocumentNode>,
    edges: Vec<Edge>,
    simulation: OrbitalSimulation,
}

struct EntityNode {
    entity_id: Uuid,
    name: String,
    entity_type: EntityType,
    roles: Vec<RoleCode>,
    
    // From v_cbu_entity_graph
    kyc_status: KycStatus,
    risk_rating: RiskRating,
    ownership_pct: Option<f32>,
    
    // Orbital position
    ring: OrbitalRing,
    priority: u8,
    angle: f32,
    
    // Visual
    shape: Shape,
    radius: f32,
    color: Color,
}
```

### 4.2 Database Views Required

The visualization needs these views (some already exist, some to create):

| View | Purpose | Status |
|------|---------|--------|
| `v_cbu_lifecycle` | CBU status, risk, state | ✓ In TODO |
| `v_cbu_kyc_summary` | KYC metrics for CBU | ✓ In TODO |
| `v_cbu_entity_graph` | Entities with roles | ✓ In TODO |
| `v_cbu_services` | Services and delivery status | NEW |
| `v_cbu_resources` | Resource instances | NEW |
| `v_cbu_documents` | Documents with validity status | NEW |
| `v_cbu_kyc_cases` | Cases and workstreams | NEW |
| `v_universe_clusters` | Cluster anchor definitions | NEW |

### 4.3 New Views for Visualization

```sql
-- Services for a CBU
CREATE OR REPLACE VIEW "ob-poc".v_cbu_services AS
SELECT 
    sdm.cbu_id,
    sdm.service_id,
    s.service_name,
    s.service_type,
    p.product_id,
    p.product_name,
    sdm.delivery_status,
    sdm.delivered_at,
    sdm.notes
FROM "ob-poc".service_delivery_map sdm
JOIN "ob-poc".services s ON sdm.service_id = s.service_id
JOIN "ob-poc".products p ON sdm.product_id = p.product_id;

-- Resources for a CBU
CREATE OR REPLACE VIEW "ob-poc".v_cbu_resources AS
SELECT 
    cri.cbu_id,
    cri.instance_id,
    cri.resource_type_id,
    rt.type_name as resource_type,
    rt.category as resource_category,
    cri.status,
    cri.external_id,
    cri.attributes,
    cri.created_at
FROM "ob-poc".cbu_resource_instances cri
JOIN "ob-poc".resource_types rt ON cri.resource_type_id = rt.type_id;

-- Documents for a CBU with validity status
CREATE OR REPLACE VIEW "ob-poc".v_cbu_documents AS
SELECT 
    dc.cbu_id,
    dc.document_id,
    dc.entity_id,
    dt.type_code,
    dt.display_name as doc_type_name,
    dt.category as doc_category,
    dc.status as doc_status,
    dc.received_at,
    dc.verified_at,
    dc.expiry_date,
    -- Validity calculation
    CASE 
        WHEN dc.expiry_date < CURRENT_DATE THEN 'EXPIRED'
        WHEN dc.expiry_date < CURRENT_DATE + INTERVAL '30 days' THEN 'EXPIRING_SOON'
        WHEN dc.status = 'VERIFIED' THEN 'VALID'
        WHEN dc.status = 'PENDING' THEN 'PENDING'
        ELSE 'UNKNOWN'
    END as validity_status
FROM "ob-poc".document_catalog dc
JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id;

-- KYC cases with workstream summary
CREATE OR REPLACE VIEW "ob-poc".v_cbu_kyc_cases AS
SELECT 
    kc.cbu_id,
    kc.case_id,
    kc.case_type,
    kc.status as case_status,
    kc.risk_rating,
    kc.assigned_to,
    kc.created_at,
    kc.updated_at,
    -- Workstream counts
    COUNT(ew.workstream_id) as workstream_count,
    COUNT(*) FILTER (WHERE ew.status = 'COMPLETE') as complete_workstreams,
    COUNT(*) FILTER (WHERE ew.status IN ('PENDING', 'IN_PROGRESS')) as active_workstreams
FROM kyc.cases kc
LEFT JOIN kyc.entity_workstreams ew ON kc.case_id = ew.case_id
GROUP BY kc.cbu_id, kc.case_id, kc.case_type, kc.status, 
         kc.risk_rating, kc.assigned_to, kc.created_at, kc.updated_at;

-- Universe cluster configuration
CREATE TABLE IF NOT EXISTS "ob-poc".universe_clusters (
    cluster_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_type VARCHAR(50) NOT NULL,  -- JURISDICTION, CLIENT_TYPE, PRODUCT
    cluster_code VARCHAR(50) NOT NULL,
    display_name VARCHAR(200),
    anchor_x FLOAT NOT NULL,            -- Position in universe
    anchor_y FLOAT NOT NULL,
    attraction_strength FLOAT DEFAULT 0.1,
    color_hint VARCHAR(7),              -- Hex color
    sort_order INT DEFAULT 0,
    is_active BOOLEAN DEFAULT TRUE,
    UNIQUE(cluster_type, cluster_code)
);

-- Seed jurisdiction clusters
INSERT INTO "ob-poc".universe_clusters (cluster_type, cluster_code, display_name, anchor_x, anchor_y, attraction_strength, color_hint) VALUES
('JURISDICTION', 'US', 'United States', -300, 0, 0.15, '#3F51B5'),
('JURISDICTION', 'UK', 'United Kingdom', 0, -200, 0.15, '#E91E63'),
('JURISDICTION', 'LU', 'Luxembourg', 100, -150, 0.12, '#009688'),
('JURISDICTION', 'IE', 'Ireland', 50, -180, 0.12, '#4CAF50'),
('JURISDICTION', 'CH', 'Switzerland', 150, -100, 0.12, '#F44336'),
('JURISDICTION', 'KY', 'Cayman Islands', -200, 150, 0.10, '#FF9800'),
('JURISDICTION', 'BVI', 'British Virgin Islands', -150, 180, 0.10, '#9C27B0'),
('JURISDICTION', 'SG', 'Singapore', 350, 50, 0.12, '#00BCD4'),
('JURISDICTION', 'HK', 'Hong Kong', 400, 0, 0.12, '#FFEB3B'),
('JURISDICTION', 'AU', 'Australia', 350, 150, 0.10, '#795548'),
-- Client type sub-clusters (smaller attraction, offset from jurisdiction)
('CLIENT_TYPE', 'FUND_MANDATE', 'Fund Mandates', 0, -50, 0.05, NULL),
('CLIENT_TYPE', 'CORPORATE_GROUP', 'Corporate Groups', 0, 0, 0.05, NULL),
('CLIENT_TYPE', 'INSTITUTIONAL_ACCOUNT', 'Institutional', 0, 50, 0.05, NULL)
ON CONFLICT (cluster_type, cluster_code) DO UPDATE SET
    anchor_x = EXCLUDED.anchor_x,
    anchor_y = EXCLUDED.anchor_y;

-- View for visualization to consume
CREATE OR REPLACE VIEW "ob-poc".v_universe_clusters AS
SELECT 
    cluster_id,
    cluster_type,
    cluster_code,
    display_name,
    anchor_x,
    anchor_y,
    attraction_strength,
    color_hint
FROM "ob-poc".universe_clusters
WHERE is_active = TRUE
ORDER BY cluster_type, sort_order;
```

---

## 5. Technology Stack

### 5.1 Recommended Stack

| Layer | Technology | Rationale |
|-------|------------|-----------|
| **Language** | Rust | Performance, WASM compilation |
| **Graphics** | wgpu + custom renderer | WebGL2/WebGPU, full control |
| **UI Framework** | egui (for controls) | Rust-native, WASM-compatible |
| **Math** | glam | Fast vector math |
| **WASM Bindgen** | wasm-bindgen | JS interop |
| **State Management** | Custom ECS-lite | Nodes/edges as entities |

### 5.2 Alternative: Bevy

Could use Bevy game engine for:
- Built-in ECS
- Renderer abstraction
- Plugin ecosystem

Trade-off: Larger WASM bundle, more opinionated.

### 5.3 Performance Targets

| Metric | Target |
|--------|--------|
| CBU count (Universe) | 10,000 nodes smooth |
| Entity count (Solar) | 500 nodes smooth |
| Frame rate | 60 FPS |
| Initial load | < 2 seconds |
| WASM bundle | < 2 MB gzipped |
| Memory | < 100 MB |

### 5.4 Optimization Strategies

1. **Spatial indexing** - Quadtree for collision/proximity queries
2. **View culling** - Only render nodes in viewport
3. **LOD** - Simplify distant nodes (circle only, no label)
4. **Batched rendering** - Instanced draw calls for similar nodes
5. **Web Workers** - Offload force simulation
6. **Incremental updates** - Only recalc changed nodes

---

## 6. Data Flow

### 6.1 Initial Load

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Database   │────▶│  Rust API   │────▶│  WASM Viz   │
│  (Postgres) │     │  (REST/WS)  │     │  (Browser)  │
└─────────────┘     └─────────────┘     └─────────────┘
      │                   │                   │
      │ v_cbu_lifecycle   │ JSON payload      │ Parse → Nodes
      │ v_universe_clusters│                  │ Init simulation
      │                   │                   │ Start render loop
```

### 6.2 Real-time Updates (Optional)

For live updates, WebSocket subscription:

```rust
// Server pushes
enum VizUpdate {
    CbuStatusChanged { cbu_id: Uuid, new_status: Status },
    CbuRiskChanged { cbu_id: Uuid, new_risk: RiskRating },
    CbuAdded { node: CbuNode },
    CbuRemoved { cbu_id: Uuid },
    EntityKycChanged { cbu_id: Uuid, entity_id: Uuid, status: KycStatus },
}

// Client applies
fn apply_update(universe: &mut CbuUniverse, update: VizUpdate) {
    match update {
        CbuStatusChanged { cbu_id, new_status } => {
            if let Some(node) = universe.get_mut(cbu_id) {
                node.onboarding_state = new_status;
                node.recalc_visuals();
            }
        }
        // ...
    }
}
```

---

## 7. API Endpoints

### 7.1 Universe Data

```
GET /api/viz/universe
Response: {
    nodes: [CbuNode],
    clusters: [ClusterAnchor],
    meta: { total_count, filtered_count, timestamp }
}

Query params:
  ?jurisdictions=LU,UK
  ?categories=FUND_MANDATE
  ?risk_min=MEDIUM
  ?search=acme
```

### 7.2 Solar System Data

```
GET /api/viz/cbu/{cbu_id}/solar
Response: {
    center: CbuCenterNode,
    entities: [EntityNode],
    services: [ServiceNode],
    resources: [ResourceNode],
    kyc_cases: [KycCaseNode],
    documents: [DocumentNode],
    edges: [Edge]
}
```

### 7.3 WebSocket (Live Updates)

```
WS /api/viz/subscribe
→ { action: "subscribe", topics: ["universe", "cbu:{id}"] }
← { type: "update", payload: VizUpdate }
```

---

## 8. Implementation Phases

### Phase 1: Static Universe View (MVP)
- [ ] Database views for universe data
- [ ] Rust WASM scaffold with wgpu
- [ ] Basic force simulation (repulsion + center gravity)
- [ ] Node rendering (circles with color)
- [ ] Pan/zoom controls
- [ ] Click to select

### Phase 2: Clustered Universe
- [ ] Cluster anchor table and seeding
- [ ] Multi-force simulation (jurisdiction + type attraction)
- [ ] Cluster region hints (optional shading)
- [ ] Filter panel
- [ ] Node labels on zoom/hover

### Phase 3: Solar System View
- [ ] Solar system data endpoint
- [ ] Orbital simulation
- [ ] Entity/service/resource/doc nodes
- [ ] Ring-based layout
- [ ] Edge rendering

### Phase 4: Transitions & Polish
- [ ] Animated universe → solar transition
- [ ] Halo animations for status
- [ ] Tooltips and context menus
- [ ] Performance optimization
- [ ] Mobile touch support

### Phase 5: Real-time & Integration
- [ ] WebSocket live updates
- [ ] Integration with main application
- [ ] Action triggers (start workflow from viz)
- [ ] Export/screenshot

---

## 9. Open Questions

1. **Offline-first?** - Should viz work with cached data when disconnected?
2. **Collaboration?** - Multiple users viewing same universe with cursors?
3. **Historical view?** - Time-travel to see universe at past date?
4. **3D option?** - Worth exploring 3D universe with Three.js/Bevy?
5. **Accessibility?** - Screen reader support for graph viz is hard. Alt mode?
6. **Embedding?** - Will this be standalone or embedded in larger app?

---

## 10. Appendix: Visual Reference

### Node Shape Reference
```
Person:    ○     (circle)
Company:   ▢     (rounded rect)
Fund:      ◇     (diamond)  
Trust:     ⬡     (hexagon)
Service:   ▭     (wide rect)
Resource:  ▯     (narrow rect)
Document:  📄    (page icon)
KYC Case:  ⬠     (pentagon)
```

### Connection Style Reference
```
Role:           ───────  (solid)
Ownership:      ━━━━━━━  (thick solid)
Control:        ╌╌╌╌╌╌╌  (dashed)
Service link:   ┈┈┈┈┈┈┈  (dotted)
Document link:  ·······  (faint dotted, hover only)
```

### Risk Color Gradient
```
STANDARD  ████████████  #4CAF50
LOW       ████████████  #8BC34A  
MEDIUM    ████████████  #FFC107
HIGH      ████████████  #FF5722
PROHIBITED████████████  #212121
```
