# Session & Visualization Architecture

> **Status:** ✅ Complete
> **Last Updated:** 2026-01-11
> **Related:** CLAUDE.md Session State Management section

---

## Overview

Session = Intent Scope = Visual State = Operation Target. They are THE SAME THING.

```
User Intent → Session State Change → { Agent Context, DSL State, Visual } all update
```

One source of truth. Three expressions:
- **Agent context**: "You're looking at 12 LU equity CBUs with CUSTODY pending"
- **DSL REPL**: `@_selection = [12 UUIDs]`
- **Visualization**: 12 highlighted nodes in the taxonomy view

---

## Implementation Status

### Group-Level Context (Allianz, BlackRock, etc.)

**Verbs (session.yaml → session_ops.rs):**

| Verb | Purpose | Status |
|------|---------|--------|
| `session.set-galaxy` | All CBUs under apex entity (e.g., Allianz SE) | ✅ |
| `session.set-book` | Filtered subset with jurisdictions/entity-types/cbu-types | ✅ |
| `session.set-cbu` | Single CBU focus | ✅ |
| `session.set-jurisdiction` | All CBUs in a jurisdiction | ✅ |
| `session.set-neighborhood` | N hops from focal entity | ✅ |

**Database (012_session_scope_management.sql):**
```sql
CREATE TABLE "ob-poc".session_scopes (
    session_scope_id UUID PRIMARY KEY,
    session_id UUID NOT NULL,
    scope_type VARCHAR(50),           -- galaxy, book, cbu, jurisdiction, neighborhood
    apex_entity_id UUID,              -- For galaxy/book: Allianz, BlackRock
    apex_entity_name VARCHAR(255),
    cbu_id UUID,                       -- For cbu scope
    jurisdiction_code VARCHAR(10),     -- For jurisdiction scope
    scope_filters JSONB,               -- Additional filters
    cursor_entity_id UUID,             -- Current focus within scope
    active_cbu_ids UUID[],             -- Multi-CBU selection (0..n)
    history_position INTEGER,          -- For back/forward navigation
    ...
);
```

### Filters (regional/fund type/status)

**GraphFilters (graph/types.rs):**
```rust
pub struct GraphFilters {
    pub prong: ProngFilter,                    // Both, OwnershipOnly, ControlOnly
    pub jurisdictions: Option<Vec<String>>,    // LU, IE, DE, etc.
    pub fund_types: Option<Vec<String>>,       // EQUITY, FIXED_INCOME, etc.
    pub entity_types: Option<Vec<EntityType>>,
    pub as_of_date: NaiveDate,
    pub min_ownership_pct: Option<Decimal>,
    pub path_only: bool,
}
```

**View Verbs (view.yaml → view_ops.rs):**
```yaml
view.universe:
  args:
    - jurisdiction: string_list    # Filter by jurisdiction(s)
    - fund-type: string_list       # Filter by fund type(s)
    - status: [RED, AMBER, GREEN]  # Filter by status
    - needs-attention: boolean     # Filter to items needing attention

view.refine:
  args:
    - include: object   # Narrows selection
    - exclude: object   # Removes from selection
    - add: uuid_list    # Add specific IDs
    - remove: uuid_list # Remove specific IDs
```

### Active CBU Set (0..n)

**Migration 019:**
```sql
ALTER TABLE "ob-poc".session_scopes
ADD COLUMN active_cbu_ids UUID[] DEFAULT '{}';

COMMENT ON COLUMN "ob-poc".session_scopes.active_cbu_ids IS
'Set of active CBU IDs (0..n) for multi-CBU selection workflows.';
```

**Session State (session_ops.rs):**
```rust
pub struct SessionScopeState {
    pub active_cbu_ids: Option<Vec<Uuid>>,
    // ...
}
```

### Session History (Snapshot/Go Back)

**Migration 019 Functions:**
```sql
-- Push current scope to history before change
CREATE FUNCTION "ob-poc".push_scope_history(session_id, change_source, change_verb)

-- Navigate back (returns previous scope snapshot)
CREATE FUNCTION "ob-poc".navigate_back(session_id) → scope_snapshot

-- Navigate forward
CREATE FUNCTION "ob-poc".navigate_forward(session_id) → scope_snapshot
```

**Verbs:**
- `session.back` → Calls `navigate_back()`
- `session.forward` → Calls `navigate_forward()`

**History Table:**
```sql
CREATE TABLE "ob-poc".session_scope_history (
    history_id UUID PRIMARY KEY,
    session_id UUID NOT NULL,
    position INTEGER NOT NULL,
    scope_snapshot JSONB NOT NULL,     -- Full scope state at this point
    change_source VARCHAR(50),         -- 'dsl', 'ui', 'agent'
    change_verb VARCHAR(100),          -- e.g., 'session.set-cbu'
    created_at TIMESTAMPTZ
);
```

### ESPER Navigation Verbs

**Implemented (view.yaml):**

| Verb | Description | Args |
|------|-------------|------|
| `view.drill` | Drill into entity | `entity-id`, `direction` (down/up), `depth` |
| `view.surface` | Surface up from drill | `levels`, `to-universe` |
| `view.trace` | Follow threads | `mode` (money/control/risk/documents/alerts), `from-entity`, `depth` |
| `view.xray` | Show hidden layers | `layers` (custody/ubo/services/documents/screenings), `off` |
| `view.peel` | Remove outer layer | `layer`, `reset` |
| `view.illuminate` | Highlight aspect | `aspect`, `threshold` |

### View Modes

**TaxonomyContext (taxonomy/rules.rs):**
```rust
pub enum TaxonomyContext {
    Universe,                          // All CBUs user can see
    Book { client_id: Uuid },          // All CBUs for a client
    CbuTrading { cbu_id: Uuid },       // Trading network view
    CbuUbo { cbu_id: Uuid },           // UBO ownership view
    EntityForest { filters: Vec<Filter> },
}
```

**GraphScope (graph/types.rs):**
```rust
pub enum GraphScope {
    Empty,
    SingleCbu { cbu_id, cbu_name },
    Book { apex_entity_id, apex_name },
    Jurisdiction { code },
    EntityNeighborhood { entity_id, hops },
    Custom { description },
}
```

### MCP Tool Domain Filtering

**verbs_list Tool (mcp/tools.rs):**
```rust
Tool {
    name: "verbs_list".into(),
    description: "List available DSL verbs.".into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "domain": {
                "type": "string",
                "description": "Filter by domain (e.g., cbu, entity, document)"
            }
        }
    }),
}
```

---

## Recent Completions (2026-01-11)

### ✅ Same ManCo / Same SICAV Filtering

**GraphFilters (graph/types.rs):**
```rust
pub struct GraphFilters {
    // ... existing ...
    pub same_manco_id: Option<Uuid>,   // Filter to entities with same ManCo
    pub same_sicav_id: Option<Uuid>,   // Filter to entities in same SICAV umbrella
}
```

**GraphNode (graph/types.rs):**
```rust
pub struct GraphNode {
    // ... existing ...
    pub manco_id: Option<Uuid>,        // ManCo managing this entity
    pub sicav_id: Option<Uuid>,        // SICAV this entity belongs to
}
```

**Filter Logic (graph/filters.rs):**
- Entities with matching `manco_id` pass the `same_manco_id` filter
- Entities with matching `sicav_id` pass the `same_sicav_id` filter
- Entities without these IDs pass through (may be the ManCo/SICAV itself)

### ✅ REPL `:verbs` Command

**CLI REPL (dsl_cli.rs):**
```
:verbs           → List all domains with verb counts
:verbs kyc       → List all verbs in KYC domain with args
:verbs session   → List all session verbs
```

**MCP Tool:**
```json
{
  "name": "verbs_list",
  "arguments": { "domain": "kyc" }
}
```

### ✅ Fractal Zoom Navigation (TaxonomyStack)

**API Endpoints:**
- `POST /api/session/:id/taxonomy/zoom-in` - Push type onto stack
- `POST /api/session/:id/taxonomy/zoom-out` - Pop from stack
- `POST /api/session/:id/taxonomy/back-to` - Jump to breadcrumb level
- `POST /api/session/:id/taxonomy/reset` - Clear stack to root
- `GET /api/session/:id/taxonomy/breadcrumbs` - Get current trail

**Client (ob-poc-ui):**
- `TaxonomyPanelAction::ZoomIn/ZoomOut/BackTo`
- `state.taxonomy_zoom_in/out/back_to/reset()`
- Breadcrumb UI with clickable navigation

**Transitions (view/transition.rs):**
- `LayoutTransition` - Smooth position/size/opacity interpolation
- `EsperTransition` - Stepped Blade Runner-style enhance levels
- Easing functions: ease_out_cubic, ease_in_out_cubic, etc.
- `TransitionParams::QUICK/STANDARD/DRAMATIC/RESPONSIVE`

### ✅ Astro Navigation (Scale Levels)

**NavigationVerb (command.rs):**
```rust
ScaleUniverse           // All CBUs as dots
ScaleBook { client }    // All CBUs for commercial client
ScaleGalaxy { segment } // Cluster/segment view
ScaleSystem { cbu_id }  // Single CBU (solar system)
ScalePlanet { entity }  // Single entity focus
ScaleSurface            // High zoom detail
ScaleCore               // Deepest zoom
```

**ViewLevel (galaxy.rs):**
```rust
pub enum ViewLevel {
    Universe,
    Cluster,
    System,
    Planet,
    Surface,
    Core,
}
```

**Wiring (app.rs):**
- Each scale verb updates `navigation_scope` and `view_level`
- Triggers appropriate API fetches (`fetch_universe_graph`, `fetch_client_book`)
- Integrates with galaxy view focus stack

---

## Key Files

| File | Purpose |
|------|---------|
| `rust/config/verbs/session.yaml` | Session scope verbs |
| `rust/config/verbs/view.yaml` | View/ESPER verbs |
| `rust/src/dsl_v2/custom_ops/session_ops.rs` | Session verb handlers |
| `rust/src/dsl_v2/custom_ops/view_ops.rs` | View verb handlers (2102 lines) |
| `rust/src/graph/types.rs` | GraphFilters, GraphScope, NavigationHistory |
| `rust/src/graph/filters.rs` | Filter application logic |
| `rust/src/api/session.rs` | AgentSession, SessionState |
| `rust/src/api/session_manager.rs` | SessionManager with watch channels |
| `migrations/012_session_scope_management.sql` | Session scopes table |
| `migrations/019_session_navigation_history.sql` | History, active_cbu_ids |
| `rust/src/mcp/tools.rs` | MCP tools including verbs_list |

---

## Agent Context Integration

**AgentGraphContext includes:**
- Current scope (galaxy/book/cbu/jurisdiction)
- Active CBU(s)
- Selection count
- Pending operations
- Breadcrumbs

**Example agent context:**
```
Scope: Book (Allianz SE) → Luxembourg → Equity Funds
Active CBUs: 12
Selection: 8 (filtered by CUSTODY pending)
Pending: Add CUSTODY product to 8 CBUs
```

---

## Usage Examples

```dsl
; Set group context (Allianz)
(session.set-galaxy :apex-entity-id "allianz-se-uuid")

; Filter to Luxembourg equity funds
(session.set-book 
  :apex-entity-id "allianz-se-uuid"
  :jurisdictions ["LU"]
  :cbu-types ["EQUITY"])

; View with filters
(view.universe :jurisdiction ["LU", "IE"] :fund-type ["EQUITY"])

; Refine selection
(view.refine :exclude {:status ["GREEN"]})

; Drill into entity
(view.drill :entity-id "entity-uuid" :direction "down")

; Navigate back
(session.back)
```

