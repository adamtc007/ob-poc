# 041: Solar System Config-Driven Architecture

> **Status:** Draft
> **Created:** 2025-01-20
> **Problem:** CBU visualization requires heterogeneous struct generation, layout rules, and rendering - currently hardcoded in Rust

## Executive Summary

A CBU is a "solar system" containing a sun (SPV/Fund), planets (logical groupings like Products, ISDA, KYC), and moons (actual data records). Today, the server hardcodes what to fetch, how to structure it, and egui hardcodes how to render it.

This doc proposes a **config-driven architecture** where:
1. **Schema config** defines what to fetch and how to nest it
2. **Layout config** defines spatial positioning
3. **Render config** defines visual treatment and drill behaviors
4. All three consumers (Server, egui, ESPER) read from shared config

---

## The Metaphor (Locked In)

```
Universe    = All regions client operates in (global footprint)
  Galaxy    = Regional (LU, DE, IE) - may have multiple ManCos
    Cluster = ManCo's controlled CBUs (gravitational grouping)
      Solar System = CBU (container)
        Sun     = SPV/Fund/Asset Owner (core entity)
        Planets = Logical groupings (Products, ISDA, KYC, Instrument Matrix)
          Moons = Actual data records (Custody Account, Counterparty, KYC Case)
```

**Key insight:** Planets are **containers/categories**, moons are **data**.

---

## The Problem

### Current State (Hardcoded)

```rust
// Server: hardcoded struct assembly
fn build_cbu_graph(cbu_id: Uuid) -> CbuGraph {
    let fund = fetch_sicav_entity(cbu_id);           // hardcoded
    let products = fetch_trading_profiles(cbu_id);   // hardcoded
    let custody = fetch_custody_accounts(cbu_id);    // hardcoded
    // ... 20 more hardcoded fetches
}

// egui: hardcoded rendering
fn render_node(node: &GraphNode) {
    match node.entity_type {
        "SICAV" => draw_sun(node),           // hardcoded
        "CUSTODY_ACCOUNT" => draw_moon(node), // hardcoded
        // ... 20 more matches
    }
}
```

**Problems:**
- Adding a new entity type = code changes in 3+ places
- Layout rules buried in code
- ESPER commands don't know what's valid to navigate to
- No consistency guarantee across server/egui/ESPER

### Desired State (Config-Driven)

```yaml
# Single source of truth
solar_system_schema.yaml  → Server reads, fetches, assembles
layout_rules.yaml         → Graph builder reads, positions
render_rules.yaml         → egui reads, draws
planet_types.yaml         → All three read for vocabulary
```

**Benefits:**
- Add new planet type = edit YAML, no code changes
- Layout/render rules explicit and auditable
- ESPER can introspect valid navigation targets
- Guaranteed consistency

---

## Architecture

### The Triangle

```
                    ┌─────────────────────┐
                    │   CONFIG (YAML)     │
                    │                     │
                    │ - planet_types      │
                    │ - schema            │
                    │ - layout            │
                    │ - render            │
                    └─────────┬───────────┘
                              │
            ┌─────────────────┼─────────────────┐
            │                 │                 │
            ▼                 ▼                 ▼
    ┌───────────────┐ ┌───────────────┐ ┌───────────────┐
    │    SERVER     │ │     EGUI      │ │    ESPER      │
    │               │ │               │ │               │
    │ Reads schema  │ │ Reads render  │ │ Reads types   │
    │ Fetches data  │ │ Draws nodes   │ │ Validates nav │
    │ Emits graph   │ │ Handles drill │ │ Updates focus │
    └───────┬───────┘ └───────┬───────┘ └───────┬───────┘
            │                 │                 │
            │    ┌────────────┴────────────┐    │
            │    │                         │    │
            └────►   NavigationContext     ◄────┘
                 │   (shared state)        │
                 │                         │
                 │ - current_scope         │
                 │ - focus_path            │
                 │ - available_targets     │
                 │ - history_stack         │
                 └─────────────────────────┘
```

### Data Flow

```
1. User: "show Allianz Lux book"
   │
   ▼
2. ESPER: parse → (session.load-cluster :manco <Allianz> :jurisdiction "LU")
   │
   ▼
3. SERVER: 
   - Read solar_system_schema.yaml
   - For each CBU in cluster:
     - Fetch sun (SICAV entity)
     - Fetch planets (by planet_type config)
     - Fetch moons (by moon_type config)
   - Emit generic graph JSON
   │
   ▼
4. EGUI:
   - Receive graph JSON
   - Read layout_rules.yaml → position nodes
   - Read render_rules.yaml → draw nodes
   - Update NavigationContext.available_targets
   │
   ▼
5. User: "drill into products"
   │
   ▼
6. ESPER:
   - Check NavigationContext.available_targets → "products" valid? ✓
   - Update NavigationContext.focus_path
   - Emit drill event to egui
   │
   ▼
7. EGUI:
   - Animate camera to products planet
   - Expand moon children
   - (Optionally) request deeper data from server
```

---

## Config Schemas

### 1. Planet Types (`planet_types.yaml`)

The vocabulary of node types - shared by all three consumers.

```yaml
# rust/config/planet_types.yaml

node_types:
  sun:
    description: "Central entity - SPV/Fund/Asset Owner"
    entity_roles: [SICAV, SPV, FUND, ASSET_OWNER, UMBRELLA]
    singular: true  # Only one sun per solar system
    
  planets:
    products:
      description: "Product container - trading capabilities"
      source_table: trading_profiles
      source_fk: cbu_id
      moon_types: [custody_account, fa_account, ta_account]
      
    instrument_matrix:
      description: "Trading permissions by asset class"
      source_table: cbu_instrument_permissions
      source_fk: cbu_id
      moon_types: [asset_class_permission]
      drill_behavior: matrix  # Special grid view
      
    isda:
      description: "ISDA master agreements"
      source_table: isda_agreements
      source_fk: cbu_id
      moon_types: [counterparty, csa_annex, netting_set]
      
    kyc:
      description: "KYC cases and documentation"
      source_table: kyc_cases
      source_fk: cbu_id
      moon_types: [kyc_case, document, risk_flag]
      
    entities:
      description: "Entities with roles on this CBU"
      source_table: cbu_entity_roles
      source_fk: cbu_id
      moon_types: [entity_role]
      group_by: role_category  # Sub-group by OwnershipChain, ServiceProvider, etc.

  moons:
    custody_account:
      description: "Custody/safekeeping account"
      source_table: custody_accounts
      parent_fk: trading_profile_id
      
    fa_account:
      description: "Fund accounting account"
      source_table: fa_accounts
      parent_fk: trading_profile_id
      
    counterparty:
      description: "ISDA counterparty"
      source_table: isda_counterparties
      parent_fk: isda_id
      
    entity_role:
      description: "Entity with role assignment"
      source_table: cbu_entity_roles
      parent_fk: cbu_id
      includes: [entity]  # Join to entities table
```

### 2. Layout Rules (`layout_rules.yaml`)

Spatial positioning - where things go.

```yaml
# rust/config/layout_rules.yaml

solar_system_layout:
  sun:
    position: center
    size: large
    z_index: 100
    
  orbits:
    inner:
      radius: 150
      planets: [products, entities]
      distribution: even  # Spread evenly around orbit
      
    middle:
      radius: 300
      planets: [instrument_matrix, isda]
      distribution: even
      
    outer:
      radius: 450
      planets: [kyc]
      distribution: even

  moon_layout:
    style: cluster  # Moons cluster around parent planet
    max_visible: 10  # Collapse to "+N more" if exceeds
    expansion: on_drill  # Show all when drilled into

cluster_layout:
  style: grid  # Multiple solar systems in a grid
  spacing: 800
  max_per_row: 5

galaxy_layout:
  style: spiral  # Clusters arranged in spiral arms
  arm_count: 3
```

### 3. Render Rules (`render_rules.yaml`)

Visual treatment - how things look and behave.

```yaml
# rust/config/render_rules.yaml

node_rendering:
  sun:
    shape: circle
    color: "#FFD700"  # Gold
    glow: true
    glow_color: "#FFA500"
    icon: sun
    label_position: below
    
  planets:
    products:
      shape: circle
      color: "#4A90D9"  # Blue
      icon: package
      size: medium
      
    instrument_matrix:
      shape: hexagon
      color: "#D4A017"  # Amber
      icon: grid
      size: medium
      
    isda:
      shape: circle
      color: "#2E8B57"  # Sea green
      icon: file-contract
      size: medium
      
    kyc:
      shape: circle
      color: "#DC143C"  # Crimson
      icon: shield
      size: medium
      badge: risk_count  # Show count of risk flags
      
    entities:
      shape: circle
      color: "#9370DB"  # Purple
      icon: users
      size: medium

  moons:
    default:
      shape: circle
      size: small
      label_position: right
      
    custody_account:
      color: "#708090"  # Slate
      icon: vault
      
    counterparty:
      color: "#20B2AA"  # Light sea green
      icon: handshake

drill_behaviors:
  default:
    style: expand_children  # Show moons around planet
    animation: zoom_and_fade
    
  matrix:
    style: grid_overlay  # Full-screen grid
    animation: slide_up
    
  timeline:
    style: horizontal_timeline
    animation: slide_left

interactions:
  hover:
    highlight: true
    show_tooltip: true
    
  click:
    single: select
    double: drill
    
  drag:
    enabled: false  # Nodes are positioned by layout rules
```

### 4. Solar System Schema (`solar_system_schema.yaml`)

How server assembles the graph - fetch instructions.

```yaml
# rust/config/solar_system_schema.yaml

solar_system:
  # First, identify the sun
  sun:
    query: |
      SELECT e.entity_id, e.name, et.type_code as entity_type
      FROM "ob-poc".cbu_entity_roles cer
      JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
      JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
      JOIN "ob-poc".roles r ON r.role_id = cer.role_id
      WHERE cer.cbu_id = $1
        AND r.name IN ('SICAV', 'SPV', 'FUND', 'ASSET_OWNER')
        AND cer.effective_to IS NULL
      LIMIT 1
    node_type: sun

  # Then, fetch each planet type
  planets:
    products:
      query: |
        SELECT tp.trading_profile_id, tp.name, tp.status
        FROM "ob-poc".trading_profiles tp
        WHERE tp.cbu_id = $1
      node_type: planet
      planet_type: products
      children:
        custody:
          query: |
            SELECT ca.account_id, ca.account_number, ca.custodian_name
            FROM "ob-poc".custody_accounts ca
            WHERE ca.trading_profile_id = $1
          node_type: moon
          moon_type: custody_account
          
    isda:
      query: |
        SELECT ia.isda_id, ia.agreement_date, ia.status
        FROM "ob-poc".isda_agreements ia
        WHERE ia.cbu_id = $1
      node_type: planet
      planet_type: isda
      children:
        counterparties:
          query: |
            SELECT ic.counterparty_id, e.name as counterparty_name
            FROM "ob-poc".isda_counterparties ic
            JOIN "ob-poc".entities e ON e.entity_id = ic.counterparty_entity_id
            WHERE ic.isda_id = $1
          node_type: moon
          moon_type: counterparty

    # ... other planets

  # Lazy loading config
  fetch_strategy:
    initial: [sun, planets]  # Fetch sun + planet summaries
    on_drill: [moons]        # Fetch moons when drilling into planet
    prefetch_depth: 1        # Prefetch one level ahead
```

---

## Server Output Contract

The server emits a **generic typed tree**, not domain-specific structs.

```typescript
// TypeScript-style schema for clarity

interface SolarSystemGraph {
  cbu_id: string;
  cbu_name: string;
  
  sun: SunNode | null;
  planets: PlanetNode[];
  
  // Metadata for client
  meta: {
    fetched_at: string;
    fetch_depth: number;
    has_more: boolean;
  };
}

interface SunNode {
  node_type: "sun";
  entity_id: string;
  entity_type: string;
  name: string;
  data: Record<string, any>;  // Flexible payload
}

interface PlanetNode {
  node_type: "planet";
  planet_type: string;  // "products", "isda", "kyc", etc.
  id: string;
  name: string;
  data: Record<string, any>;
  
  // Summary counts (moons not fully loaded)
  moon_counts: Record<string, number>;  // { "custody_account": 3, "fa_account": 1 }
  
  // Moons (populated on drill or prefetch)
  moons?: MoonNode[];
}

interface MoonNode {
  node_type: "moon";
  moon_type: string;  // "custody_account", "counterparty", etc.
  id: string;
  name: string;
  data: Record<string, any>;
  
  // Nested moons (if any)
  children?: MoonNode[];
}
```

**Example output:**

```json
{
  "cbu_id": "abc-123",
  "cbu_name": "ALLIANZ STRATEGY 75",
  "sun": {
    "node_type": "sun",
    "entity_id": "def-456",
    "entity_type": "SICAV",
    "name": "Allianz Global Investors Fund",
    "data": { "lei": "549300...", "jurisdiction": "LU" }
  },
  "planets": [
    {
      "node_type": "planet",
      "planet_type": "products",
      "id": "prod-1",
      "name": "Trading Products",
      "data": {},
      "moon_counts": { "custody_account": 2, "fa_account": 1 },
      "moons": null
    },
    {
      "node_type": "planet",
      "planet_type": "isda",
      "id": "isda-1", 
      "name": "ISDA Master Agreement",
      "data": { "agreement_date": "2023-01-15" },
      "moon_counts": { "counterparty": 5 },
      "moons": null
    }
  ],
  "meta": {
    "fetched_at": "2025-01-20T12:00:00Z",
    "fetch_depth": 1,
    "has_more": true
  }
}
```

---

## NavigationContext (Shared State)

The critical piece that synchronizes server, egui, and ESPER.

```rust
/// Shared navigation state - all three consumers read/write
pub struct NavigationContext {
    // Current scope (what data is loaded)
    pub scope: NavigationScope,
    
    // Current focus (what user is looking at)
    pub focus: FocusPath,
    
    // Available targets (what can be navigated to)
    pub available_targets: AvailableTargets,
    
    // History for undo/redo
    pub history: NavigationHistory,
    
    // Pending operations
    pub pending: PendingState,
}

pub enum NavigationScope {
    Empty,
    Universe { client_id: Option<Uuid> },
    Galaxy { jurisdiction: String },
    Cluster { manco_entity_id: Uuid, jurisdiction: Option<String> },
    System { cbu_id: Uuid },
}

pub struct FocusPath {
    pub scale_level: ScaleLevel,  // Universe/Galaxy/Cluster/System/Planet/Moon
    pub path: Vec<FocusNode>,     // [Cluster("allianz"), System("cbu-123"), Planet("products")]
}

pub struct FocusNode {
    pub node_type: String,        // "cluster", "system", "planet", "moon"
    pub node_id: String,          // ID or type name
    pub display_name: String,
}

pub struct AvailableTargets {
    // What planets exist in current system
    pub planets: Vec<String>,     // ["products", "isda", "kyc"]
    
    // What moons exist in current planet (if drilled)
    pub moons: Vec<String>,       // ["custody_account", "fa_account"]
    
    // Computed valid ESPER commands
    pub valid_commands: HashSet<String>,
}

pub struct NavigationHistory {
    pub past: Vec<NavigationSnapshot>,
    pub future: Vec<NavigationSnapshot>,
}

pub struct PendingState {
    pub fetching: bool,
    pub animating: bool,
    pub ready_for_input: bool,
}
```

---

## Addressing the 7 Gaps

### Gap 1: State Synchronization
**Solution:** `NavigationContext` is the single source of truth.
- Server updates `scope` and `available_targets` after fetch
- egui updates `focus` and `pending` during navigation
- ESPER reads `available_targets` to validate commands

### Gap 2: Schema Awareness in ESPER
**Solution:** ESPER reads `available_targets` from `NavigationContext`.
```rust
// Before executing "drill into custody"
if !nav_context.available_targets.moons.contains("custody_account") {
    return Err("No custody accounts in current view");
}
```

### Gap 3: Incremental Fetch
**Solution:** `fetch_strategy` in schema config.
- Initial load: sun + planet summaries + moon counts
- On drill: fetch moons for that planet
- `meta.has_more` indicates more data available

### Gap 4: Type Registry Sync
**Solution:** Single `planet_types.yaml` loaded by all three.
- Server: validates emitted `planet_type` values
- egui: looks up render rules by `planet_type`
- ESPER: validates navigation targets against known types

### Gap 5: Bidirectional Intent
**Solution:** Event system between egui ↔ ESPER.
```rust
// egui emits when user clicks
pub enum NavigationEvent {
    NodeClicked { node_type: String, node_id: String },
    NodeDoubleClicked { node_type: String, node_id: String },
    BackRequested,
    ZoomRequested { direction: ZoomDirection },
}

// ESPER consumes and updates context
fn handle_navigation_event(event: NavigationEvent, ctx: &mut NavigationContext) {
    match event {
        NavigationEvent::NodeDoubleClicked { node_type, node_id } => {
            // Same as "drill" command
            ctx.focus.drill_to(node_type, node_id);
        }
        // ...
    }
}
```

### Gap 6: Animation/Transition Timing
**Solution:** `PendingState` coordinates async operations.
```rust
// State machine
enum NavigationPhase {
    Idle,                    // Ready for input
    Fetching { target },     // Waiting for server
    Animating { duration },  // Camera in motion
    Ready,                   // Animation done, accept input
}

// ESPER checks before accepting command
if nav_context.pending.animating {
    return Err("Navigation in progress, please wait");
}
```

### Gap 7: Unified History
**Solution:** `NavigationHistory` captures full state.
```rust
pub struct NavigationSnapshot {
    pub scope: NavigationScope,
    pub focus: FocusPath,
    pub timestamp: DateTime<Utc>,
}

// Undo restores both scope AND focus
fn undo(ctx: &mut NavigationContext) {
    if let Some(snapshot) = ctx.history.past.pop() {
        ctx.history.future.push(ctx.current_snapshot());
        ctx.scope = snapshot.scope;
        ctx.focus = snapshot.focus;
    }
}
```

---

## Implementation Plan

### Phase 1: Config Foundation
1. Create `rust/config/planet_types.yaml` with initial types
2. Create `rust/config/layout_rules.yaml` with basic orbits
3. Create `rust/config/render_rules.yaml` with visual defaults
4. Add config loader to read YAML at startup

### Phase 2: Server Graph Builder
1. Create `SolarSystemBuilder` that reads `planet_types.yaml`
2. Implement generic fetch based on schema config
3. Emit `SolarSystemGraph` JSON structure
4. Add `/api/cbu/:id/solar-system` endpoint

### Phase 3: NavigationContext
1. Define `NavigationContext` struct
2. Wire into session state
3. Update on scope changes (load-cluster, etc.)
4. Update on focus changes (drill, surface, etc.)

### Phase 4: egui Integration
1. Load `render_rules.yaml` at startup
2. Render nodes by looking up `planet_type` → visual rules
3. Emit `NavigationEvent` on user interactions
4. Respect `PendingState` for input gating

### Phase 5: ESPER Integration
1. Load `planet_types.yaml` for vocabulary
2. Read `available_targets` to validate commands
3. Update `NavigationContext.focus` on navigation commands
4. Consume `NavigationEvent` from egui clicks

### Phase 6: Lazy Loading
1. Implement `fetch_strategy` from schema
2. Add `/api/cbu/:id/solar-system/planet/:type` for moon fetch
3. egui requests moons on drill
4. Update `available_targets` after fetch

---

## Example: "Show Allianz Lux Book"

Complete flow through the system:

```
1. User: "show Allianz Lux book"
   
2. ESPER parses:
   - Intent: load-cluster
   - Args: { manco: "Allianz Global Investors GmbH", jurisdiction: "LU" }
   - DSL: (session.load-cluster :manco-entity-id <uuid> :jurisdiction "LU")

3. Server executes:
   - Query cbu_groups WHERE manco_entity_id = <uuid> AND jurisdiction = 'LU'
   - Returns 114 CBU IDs
   - For each CBU, build SolarSystemGraph (shallow - sun + planet summaries)
   
4. NavigationContext updates:
   - scope: Cluster { manco_entity_id: <uuid>, jurisdiction: Some("LU") }
   - focus: { scale_level: Cluster, path: [Cluster("Allianz LU")] }
   - available_targets: { systems: ["cbu-1", "cbu-2", ...] }

5. egui renders:
   - Read layout_rules.yaml → cluster_layout: grid
   - Show 114 solar systems in grid
   - Each shows sun + planet icons (collapsed)

6. User: "zoom into Strategy 75"

7. ESPER:
   - Check available_targets.systems → "ALLIANZ STRATEGY 75" valid? ✓
   - Update focus: { scale_level: System, path: [..., System("strategy-75")] }

8. egui:
   - Animate zoom to that solar system
   - Show sun (SICAV) at center
   - Show planets in orbits (products, isda, kyc)
   - Update available_targets.planets: ["products", "isda", "kyc"]

9. User: "drill into products"

10. ESPER:
    - Check available_targets.planets → "products" valid? ✓
    - Update focus: { scale_level: Planet, path: [..., Planet("products")] }
    - Set pending.fetching = true

11. Server:
    - Receive GET /api/cbu/:id/solar-system/planet/products
    - Fetch moons (custody_account, fa_account, ta_account)
    - Return moon data

12. egui:
    - Receive moon data
    - Animate zoom to products planet
    - Show moons orbiting planet
    - pending.fetching = false, pending.animating = true
    - After animation: pending.ready_for_input = true
    - Update available_targets.moons: ["custody_account", "fa_account"]

13. User: "show me the custody accounts"

14. ESPER:
    - Already at Planet level with moons visible
    - Highlight custody_account moons
    - Or drill further if inspector view needed
```

---

## Open Questions

1. **Config hot-reload?** Can we update YAML without restart?
2. **Validation?** How do we validate config consistency across files?
3. **Versioning?** How do we handle schema evolution?
4. **Performance?** Is YAML parsing overhead acceptable? Cache parsed config?
5. **Testing?** How do we test config-driven behavior?

---

## Files to Create

```
rust/config/
├── planet_types.yaml          # Node type vocabulary
├── layout_rules.yaml          # Spatial positioning
├── render_rules.yaml          # Visual treatment
└── solar_system_schema.yaml   # Fetch instructions

rust/src/
├── config/
│   ├── mod.rs
│   ├── planet_types.rs        # Load & validate planet_types.yaml
│   ├── layout_rules.rs        # Load & validate layout_rules.yaml
│   └── solar_system_schema.rs # Load & validate schema.yaml
├── graph/
│   └── solar_system_builder.rs # Config-driven graph assembly
└── session/
    └── navigation_context.rs   # Shared navigation state

rust/crates/ob-poc-ui/src/
├── config/
│   └── render_rules.rs        # Load render_rules.yaml for WASM
└── solar_system/
    └── renderer.rs            # Config-driven rendering
```

---

## Summary

This architecture transforms the server/egui/ESPER triangle from hardcoded to config-driven:

| Aspect | Before | After |
|--------|--------|-------|
| Add planet type | 3+ code files | 1 YAML edit |
| Layout rules | Buried in Rust | Explicit YAML |
| Render rules | Buried in egui | Explicit YAML |
| Valid nav targets | Hardcoded | Dynamic from config |
| State sync | Ad-hoc | NavigationContext |
| History | Partial (scope only) | Full (scope + focus) |

The key insight: **the config files are the contract** between server, egui, and ESPER. They all read the same source of truth, guaranteeing consistency.
