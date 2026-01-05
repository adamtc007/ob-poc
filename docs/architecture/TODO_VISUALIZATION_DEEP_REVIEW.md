# TODO: Config-Driven Visualization & Session Architecture

**Created**: 2026-01-05  
**Updated**: 2026-01-05  
**Status**: Phase 1-2 Complete, Phase 3 Partial  
**Priority**: HIGH  
**Estimated Effort**: 8 days remaining  
**Revision**: 4 - Updated after legacy code removal

---

## CRITICAL PRINCIPLE: WHAT YOU SEE = WHAT AGENT OPERATES ON

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   THE USER MUST ALWAYS SEE WHAT THE AGENT IS LOOKING AT                    │
│                                                                             │
│   When user says "add CUSTODY to those" or "remove the Luxembourg ones"    │
│   "those" and "the Luxembourg ones" MUST BE VISIBLE ON SCREEN              │
│                                                                             │
│   If REPL scope ≠ Visual scope → REPL is unusable                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**The viewport IS the prompt context.**

---

## ARCHITECTURE: SESSION AS SINGLE SOURCE OF TRUTH

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SESSION                                        │
│                    (Single Source of Current Truth)                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  KEYS (what we're looking at):                                              │
│    cbu_id: Option<Uuid>                                                     │
│    book_id: Option<Uuid>                                                    │
│    entity_ids: Vec<Uuid>                                                    │
│    case_id: Option<Uuid>                                                    │
│                                                                             │
│  FILTERS (how we're filtering):                                             │
│    jurisdiction: Option<String>                                             │
│    status: Option<Status>                                                   │
│    aum_range: Option<Range>                                                 │
│    role_category: Option<String>                                            │
│                                                                             │
│  VIEW MODE (which visualization):                                           │
│    view_mode: "KYC_UBO" | "TRADING" | "SERVICE" | "FUND_STRUCTURE"         │
│                                                                             │
│  ZOOM / FOCUS:                                                              │
│    zoom_level: f32                                                          │
│    focused_node: Option<Uuid>                                               │
│    expanded_nodes: HashSet<Uuid>                                            │
│                                                                             │
│  SELECTION (the "those" / "it"):                                            │
│    selection: Vec<Uuid>        ← WHAT AGENT WILL OPERATE ON                │
│    refinements: Vec<Refinement> ← "except...", "plus...", "only..."        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │
            ┌───────────────────────┴───────────────────────┐
            │                                               │
            ▼                                               ▼
┌───────────────────────────────┐           ┌───────────────────────────────┐
│           REPL                │           │         EGUI VIEW(S)          │
│                               │           │                               │
│  Reads: session.selection     │           │  Renders: taxonomy structs    │
│  Reads: session.keys          │           │  Built from: session keys +   │
│  Reads: session.filters       │           │             filters + view    │
│                               │           │                               │
│  "add CUSTODY to those"       │           │  User SEES "those" visually   │
│       │                       │           │       │                       │
│       └───────────────────────┼───────────┼───────┘                       │
│                               │           │                               │
│  SAME "those" ════════════════╪═══════════╪══ SAME "those"                │
│                               │           │                               │
└───────────────────────────────┘           └───────────────────────────────┘

                    REPL SCOPE === VISUAL SCOPE === SESSION STATE
```

---

## TAXONOMY GENERATION FLOW

Session provides keys/filters/view_mode → Builders generate structs → UI renders

```
SESSION (keys + filters + view_mode + zoom)
              │
              │
              ▼
      ┌───────────────┐
      │ ViewConfig    │◄─── DB tables: edge_types, node_types, view_modes
      │ Service       │     (Config, not code!)
      └───────┬───────┘
              │
              │  edge_types for this view_mode
              │  node_types for this view_mode  
              │  zoom thresholds
              │
              ▼
      ┌───────────────┐
      │ Membership    │◄─── Built FROM config + session keys/filters
      │ Rules         │     (No hardcoded edge lists!)
      └───────┬───────┘
              │
              ▼
      ┌───────────────┐
      │ Taxonomy      │     Can produce multiple taxonomies:
      │ Builder(s)    │     - CBU ownership tree
      │               │     - Instrument matrix
      │               │     - Product/service chain
      └───────┬───────┘
              │
              ▼
      ┌───────────────────────────────────────────────────┐
      │           Vec<TaxonomyNode> structs               │
      │                                                   │
      │  Pre-filtered, pre-computed                       │
      │  Ready to render                                  │
      │  Zero business logic needed by UI                 │
      └───────────────────────────────────────────────────┘
              │
              │  (structs over wire)
              │
              ▼
      ┌───────────────────────────────────────────────────┐
      │                 EGUI (WASM)                       │
      │                                                   │
      │  Receives structs → Renders them                  │
      │  DUMB RENDERER                                    │
      │  ZERO filtering logic                             │
      │  ZERO business rules                              │
      └───────────────────────────────────────────────────┘
```

---

## FRACTAL ZOOM: TAXONOMY OF TAXONOMIES

Each node can expand into its own sub-taxonomy. Zoom level determines expansion.

### Example: Allianz Lux Book (40 CBUs) - Trading View

**Zoomed Out (zoom 0.3) - Book Level:**

```
┌─────────────────────────────────────────────────────────────────┐
│  Allianz Lux Book                                               │
│  Trading View                                                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐      │
│  │CBU 1│ │CBU 2│ │CBU 3│ │CBU 4│ │CBU 5│ │CBU 6│ │CBU 7│ ...  │
│  └─────┘ └─────┘ └─────┘ └─────┘ └─────┘ └─────┘ └─────┘      │
│                                                                 │
│  Each CBU = COLLAPSED (just name label)                         │
│  No sub-taxonomy loaded yet                                     │
│                                                                 │
│  session.selection = [all 40 CBU ids]                          │
│  REPL "add CUSTODY" → operates on all 40                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Zoomed In (zoom 0.8) - CBU 3 Expanded:**

```
┌─────────────────────────────────────────────────────────────────┐
│  Allianz Lux Book > CBU 3                                       │
│  Trading View                                                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                        CBU 3                             │   │
│  │                                                          │   │
│  │  ┌────────────────┐       ┌────────────────────────┐    │   │
│  │  │ Trading Profile│       │ Entity: Asset Owner    │    │   │
│  │  │                │       │         │              │    │   │
│  │  │  ┌──────────┐  │       │    ownership (60%)     │    │   │
│  │  │  │Instrument│  │       │         │              │    │   │
│  │  │  │ Matrix   │  │       │         ▼              │    │   │
│  │  │  │          │  │       │ Entity: Holding Co     │    │   │
│  │  │  │ Equities │  │       │         │              │    │   │
│  │  │  │ FI       │  │       │    ownership (100%)    │    │   │
│  │  │  │ FX       │  │       │         │              │    │   │
│  │  │  └──────────┘  │       │         ▼              │    │   │
│  │  └────────────────┘       │ Entity: J. Smith (UBO) │    │   │
│  │                           └────────────────────────┘    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────┐ ┌─────┐        ┌─────┐ ┌─────┐  (siblings collapsed)  │
│  │CBU 2│ │CBU 4│  ...   │CBU39│ │CBU40│                        │
│  └─────┘ └─────┘        └─────┘ └─────┘                        │
│                                                                 │
│  session.selection = [CBU 3]                                   │
│  session.focused_node = CBU 3                                  │
│  REPL "add signatory @john" → operates on CBU 3                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### TaxonomyNode Structure for Fractal Zoom

```rust
pub struct TaxonomyNode {
    pub id: Uuid,
    pub name: String,
    pub node_type: NodeType,
    
    // Static children (always present at this level)
    pub children: Vec<TaxonomyNode>,
    
    // ═══════════════════════════════════════════════════════════
    // FRACTAL EXPANSION
    // ═══════════════════════════════════════════════════════════
    
    /// Can this node expand into a sub-taxonomy?
    pub is_expandable: bool,
    
    /// What context builds the sub-taxonomy?
    /// e.g., TaxonomyContext::CbuTrading { cbu_id: self.id }
    pub expansion_context: Option<TaxonomyContext>,
    
    /// The expanded sub-taxonomy (loaded on demand)
    pub expanded: Option<Box<TaxonomyNode>>,
    
    // ═══════════════════════════════════════════════════════════
    // ZOOM THRESHOLDS (from node_types config!)
    // ═══════════════════════════════════════════════════════════
    
    /// Collapse this node when zoom < this value
    pub collapse_below_zoom: f32,  // e.g., 0.3
    
    /// Auto-expand this node when zoom > this value  
    pub expand_above_zoom: f32,    // e.g., 0.7
    
    /// Hide label when zoom < this value
    pub hide_label_below_zoom: f32, // e.g., 0.2
}
```

---

## WHAT NEEDS WIRING

### Current State (Hardcoded)

```rust
// rust/src/taxonomy/rules.rs - HARDCODED EDGE TYPES!

impl MembershipRules {
    pub fn cbu_ubo(cbu_id: Uuid) -> Self {
        Self {
            edge_types: vec![
                EdgeType::Owns,      // ← HARDCODED!
                EdgeType::Controls,  // ← HARDCODED!
                EdgeType::TrustRole, // ← HARDCODED!
            ],
            // ...
        }
    }
    
    pub fn cbu_trading(cbu_id: Uuid) -> Self {
        Self {
            edge_types: vec![
                EdgeType::HasRole,   // ← HARDCODED!
                EdgeType::ManagedBy, // ← HARDCODED!
                // ...
            ],
        }
    }
}
```

### Target State (Config-Driven)

```rust
impl MembershipRules {
    /// Build rules from ViewConfigService + session context
    pub async fn from_session_context(
        pool: &PgPool,
        session: &Session,
    ) -> Result<Self> {
        // Get edge types from DB config based on view_mode
        let edge_configs = ViewConfigService::get_view_edge_types(
            pool, 
            &session.view_mode
        ).await?;
        
        // Convert to EdgeType enum
        let edge_types: Vec<EdgeType> = edge_configs.iter()
            .filter_map(|ec| EdgeType::from_code(&ec.edge_type_code))
            .collect();
        
        // Get node type config for zoom thresholds
        let node_configs = ViewConfigService::get_view_node_types(
            pool,
            &session.view_mode
        ).await?;
        
        // Get hierarchy direction from view_modes config
        let view_config = ViewConfigService::get_view_mode_config(
            pool,
            &session.view_mode
        ).await?;
        
        Ok(Self {
            root_filter: RootFilter::from_session_keys(&session.keys),
            entity_filter: EntityFilter::from_session_filters(&session.filters),
            edge_types,  // ← FROM DATABASE CONFIG!
            direction: view_config.primary_traversal_direction.into(),
            // ... rest from config
        })
    }
}
```

---

## DATABASE TABLES (Config, Not Code)

### edge_types - Which edges appear in which views

```sql
CREATE TABLE "ob-poc".edge_types (
    edge_type_code VARCHAR(50) PRIMARY KEY,
    
    -- VIEW APPLICABILITY (the core config!)
    show_in_ubo_view BOOLEAN DEFAULT false,
    show_in_trading_view BOOLEAN DEFAULT false,
    show_in_fund_structure_view BOOLEAN DEFAULT false,
    show_in_service_view BOOLEAN DEFAULT false,
    
    -- Layout hints
    layout_direction VARCHAR(20),  -- UP, DOWN
    tier_delta INTEGER,            -- How many levels
    is_hierarchical BOOLEAN,       -- Creates tree structure
    
    -- ... (full schema in migration file)
);

-- Example data:
-- OWNERSHIP:     show_in_ubo_view=true,  direction=UP
-- CONTROL:       show_in_ubo_view=true,  show_in_trading_view=true
-- HAS_ROLE:      show_in_trading_view=true
-- USES_PRODUCT:  show_in_service_view=true
```

### node_types - Which nodes + zoom thresholds

```sql
CREATE TABLE "ob-poc".node_types (
    node_type_code VARCHAR(30) PRIMARY KEY,
    
    -- VIEW APPLICABILITY
    show_in_ubo_view BOOLEAN DEFAULT false,
    show_in_trading_view BOOLEAN DEFAULT false,
    -- ...
    
    -- ZOOM THRESHOLDS (for fractal rendering)
    collapse_below_zoom NUMERIC(3,2) DEFAULT 0.3,
    hide_label_below_zoom NUMERIC(3,2) DEFAULT 0.2,
    expand_above_zoom NUMERIC(3,2) DEFAULT 0.7,
    
    -- ... (full schema in migration file)
);
```

### view_modes - Per-view configuration

```sql
CREATE TABLE "ob-poc".view_modes (
    view_mode_code VARCHAR(30) PRIMARY KEY,
    
    -- Root identification
    root_identification_rule VARCHAR(50),  -- 'CBU', 'TERMINUS_ENTITIES', etc.
    
    -- Traversal
    primary_traversal_direction VARCHAR(10),  -- UP, DOWN
    
    -- Which edges create hierarchy vs overlay
    hierarchy_edge_types JSONB,  -- ["OWNERSHIP", "TRUST_BENEFICIARY"]
    overlay_edge_types JSONB,    -- ["CONTROL", "BOARD_MEMBER"]
);
```

---

## MIGRATION SQL

Files:
- `rust/migrations/20260105_visualization_config.sql` - Schema definition
- `rust/migrations/20260105_visualization_layout.sql` - Layout config table

**Status**: ✅ COMPLETE - Migrations run 2026-01-05

```bash
# Verification (already run):
SELECT COUNT(*) FROM "ob-poc".node_types;    -- 13 rows
SELECT COUNT(*) FROM "ob-poc".edge_types;    -- 20 rows  
SELECT COUNT(*) FROM "ob-poc".view_modes;    -- 8 rows
SELECT COUNT(*) FROM "ob-poc".layout_config; -- 1 row
```

---

## LEGACY CODE REMOVED (2026-01-05)

The following legacy code has been deleted as part of the migration to config-driven approach:

| File | Status | Notes |
|------|--------|-------|
| `rust/src/graph/layout.rs` | DELETED | ~700 lines of hardcoded LayoutEngine |
| `rust/src/graph/builder.rs` | DELETED | Old CbuGraphBuilder replaced by ConfigDrivenGraphBuilder |
| `types.rs` methods | REMOVED | `is_ubo_relevant()`, `is_trading_relevant()`, `filter_to_*()` |

**Replacement architecture:**
- `ConfigDrivenGraphBuilder` - queries DB config for node/edge visibility
- `LayoutEngineV2` - BFS-based layout with config from `layout_config` table
- `ViewConfigService` - service layer for querying view configuration

---

## IMPLEMENTATION CHECKLIST

### Phase 1: Database (DONE)
- [x] Migration file created
- [x] Migration run against database (2026-01-05)
- [x] Tables populated with seed data (13 node_types, 20 edge_types, 8 view_modes)
- [x] Verify queries work

### Phase 2: Config Service + Graph Builder (DONE)
- [x] `ViewConfigService` created (`rust/src/graph/view_config_service.rs`)
- [x] `get_view_edge_types()` implemented
- [x] `get_view_node_types()` implemented
- [x] `get_view_mode_config()` implemented
- [x] `ConfigDrivenGraphBuilder` implemented (`rust/src/graph/config_driven_builder.rs`)
- [x] `LayoutEngineV2` implemented (`rust/src/graph/layout_v2.rs`)
- [x] Graph routes updated to use new builders (`graph_routes.rs`, `ob-poc-web/routes/api.rs`)

### Phase 3: Remove Hardcoded Logic (PARTIAL - Legacy Deleted, MembershipRules Pending)
- [x] Delete `rust/src/graph/layout.rs` (legacy LayoutEngine)
- [x] Delete `rust/src/graph/builder.rs` (legacy CbuGraphBuilder)
- [x] Delete `is_ubo_relevant()` from types.rs
- [x] Delete `is_trading_relevant()` from types.rs
- [x] Delete `filter_to_ubo_only()` from types.rs
- [x] Delete `filter_to_trading_entities()` from types.rs
- [x] Update `rust/src/graph/mod.rs` to remove legacy exports
- [ ] Update `MembershipRules::cbu_ubo()` to use ViewConfigService
- [ ] Update `MembershipRules::cbu_trading()` to use ViewConfigService
- [ ] Update `MembershipRules::cbu_kyc()` to use ViewConfigService

### Phase 4: Session Integration
- [ ] Add `view_mode` to Session (if not present)
- [ ] Add `zoom_level` to Session
- [ ] Add `expanded_nodes: HashSet<Uuid>` to Session
- [ ] `MembershipRules::from_session_context()` method

### Phase 5: TaxonomyNode Fractal Support
- [ ] Add `collapse_below_zoom` to TaxonomyNode
- [ ] Add `expand_above_zoom` to TaxonomyNode
- [ ] Add `expansion_context` to TaxonomyNode
- [ ] Add `expanded: Option<Box<TaxonomyNode>>` to TaxonomyNode
- [ ] TaxonomyBuilder populates zoom thresholds from node_types config

### Phase 6: Verify REPL/View Alignment
- [ ] REPL reads `session.selection`
- [ ] View renders from same `session` state
- [ ] Changing view updates `session.selection`
- [ ] REPL operations target what user sees

---

## FILES TO MODIFY

| File | Change |
|------|--------|
| `rust/src/taxonomy/rules.rs` | `MembershipRules::from_session_context()` uses ViewConfigService |
| `rust/src/taxonomy/node.rs` | Add zoom threshold fields to TaxonomyNode |
| `rust/src/taxonomy/builder.rs` | Populate zoom thresholds from config |
| `rust/src/session/mod.rs` | Ensure view_mode, zoom_level, expanded_nodes present |
| `rust/src/session/view_state.rs` | Verify selection sync with visual state |

---

## VERIFICATION TESTS

### Test 1: REPL/View Alignment

```
1. Set session to Book view (Allianz, 40 CBUs)
2. Visual shows 40 CBU boxes
3. REPL: "add CUSTODY to those"
4. Verify: All 40 CBUs get CUSTODY product

5. Zoom into CBU 3
6. Visual shows CBU 3 expanded, others collapsed
7. session.selection now = [CBU 3]
8. REPL: "add signatory @john"
9. Verify: Only CBU 3 gets signatory, not all 40
```

### Test 2: Config-Driven Edge Filtering

```
1. Set view_mode = "KYC_UBO"
2. Query edge_types WHERE show_in_ubo_view = true
3. Build taxonomy
4. Verify: Only ownership/control/trust edges appear
5. Switch view_mode = "TRADING"
6. Verify: Different edges appear (has_role, managed_by, etc.)
```

### Test 3: Zoom Threshold Behavior

```
1. Book view, zoom = 0.2
2. All CBUs show as collapsed boxes (no labels)
3. Zoom to 0.4
4. CBUs show labels but no expansion
5. Zoom to 0.8
6. Focused CBU auto-expands, shows sub-taxonomy
```

---

## SUMMARY

**The core principle:**

```
SESSION is the single source of truth.
REPL reads from SESSION.
VIEW renders from SESSION.
WHAT USER SEES === WHAT AGENT OPERATES ON.

If this invariant breaks, the REPL is unusable.
```

**The config principle:**

```
View filtering rules live in DATABASE (edge_types, node_types, view_modes).
MembershipRules queries ViewConfigService.
No hardcoded is_ubo_relevant() or edge lists in Rust code.

Config, not code.
```

---

## CURRENT STATUS (2026-01-05)

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1: Database | ✅ DONE | Tables created with seed data |
| Phase 2: Config Service | ✅ DONE | ViewConfigService, ConfigDrivenGraphBuilder, LayoutEngineV2 |
| Phase 3: Remove Hardcoded | 🟡 PARTIAL | Legacy deleted, MembershipRules pending |
| Phase 4: Session Integration | ⬜ TODO | view_mode, zoom_level, expanded_nodes |
| Phase 5: TaxonomyNode Fractal | ⬜ TODO | Zoom thresholds, expansion context |
| Phase 6: REPL/View Alignment | ⬜ TODO | Verification tests |

**What's working now:**
- Graph visualization uses `ConfigDrivenGraphBuilder` which queries database config
- Node/edge visibility driven by `view_modes.node_types` and `view_modes.edge_types` JSONB
- Layout computed by `LayoutEngineV2` with config from `layout_config` table
- All legacy hardcoded filtering code removed

**Next action:** Wire `MembershipRules` to use `ViewConfigService` with session context.
