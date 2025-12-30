# TODO: Unified EntityGraph - CBU + UBO Navigation

**Priority**: HIGH  
**Type**: CONSOLIDATION / REFACTOR  
**Estimated Effort**: 5-7 days

---

## ⚠️ CRITICAL: This is a REPLACEMENT, not an addition

This task **consolidates and replaces** the existing CBU-only visualization code with a unified `EntityGraph` structure that handles both:
- CBU container visualization (existing functionality)
- UBO forest navigation (new functionality)

**DO NOT** create parallel implementations. **REPLACE** the existing code.

---

## Context

### Current State (to be replaced)
```
rust/src/graph/
├── types.rs           # GraphNode, CbuGraph, etc. - CBU-ONLY
├── layout.rs          # LayoutEngine - CBU-ONLY  
├── builder.rs         # GraphBuilder - CBU-ONLY
└── mod.rs

rust/src/database/
└── visualization_repository.rs  # CBU queries only
```

### Target State (unified)
```
rust/src/graph/
├── entity_graph.rs    # NEW: Unified EntityGraph struct
├── navigation.rs      # NEW: Navigation primitives (up/down/sibling)
├── filters.rs         # NEW: Filter state and application
├── builder.rs         # REPLACE: Build from DB for any scope
├── layout.rs          # REPLACE: Layout by role category
├── types.rs           # REPLACE: Unified node/edge types
└── mod.rs

rust/src/navigation/
├── parser.rs          # NEW: Nom grammar for nav commands
├── commands.rs        # NEW: NavCommand enum
├── executor.rs        # NEW: Execute commands against graph
└── mod.rs

rust/src/database/
└── graph_repository.rs  # REPLACE: Queries for CBU + UBO + Book scope
```

---

## Phase 1: Core Data Structures

### Task 1.1: Create unified types (`rust/src/graph/types.rs`)

**REPLACE** existing `GraphNode`, `CbuGraph`, etc. with:

```rust
// Key structs to implement:

pub struct EntityGraph {
    pub nodes: HashMap<Uuid, GraphNode>,
    pub cbus: HashMap<Uuid, CbuNode>,
    pub ownership_edges: Vec<OwnershipEdge>,
    pub control_edges: Vec<ControlEdge>,
    pub fund_edges: Vec<FundEdge>,
    pub service_edges: Vec<ServiceEdge>,
    pub role_assignments: Vec<RoleAssignment>,
    pub cursor: Option<Uuid>,
    pub history: NavigationHistory,
    pub filters: GraphFilters,
    pub scope: GraphScope,
    pub termini: Vec<Uuid>,
    pub commercial_clients: Vec<Uuid>,
}

pub struct GraphNode {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: EntityType,
    pub jurisdiction: Option<String>,
    // Adjacency lists (populated from edges)
    pub owners: Vec<Uuid>,
    pub owned: Vec<Uuid>,
    pub controlled_by: Vec<Uuid>,
    pub controls: Vec<Uuid>,
    pub cbu_memberships: Vec<Uuid>,
    // Classification
    pub primary_role_category: Option<RoleCategory>,
    pub ubo_treatment: Option<UboTreatment>,
    pub is_natural_person: bool,
    pub depth_from_terminus: Option<u32>,
    // Rendering (set by layout engine)
    pub position: Option<Position>,
    pub size: Option<Size>,
    pub visible: bool,
}

pub struct CbuNode {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub status: CbuStatus,
    pub commercial_client_id: Uuid,
    pub member_entities: Vec<Uuid>,
    pub products: Vec<Uuid>,
    pub position: Option<Position>,
    pub size: Option<Size>,
    pub expanded: bool,
}

// Edge types - see design doc for full definitions
pub struct OwnershipEdge { ... }
pub struct ControlEdge { ... }
pub struct FundEdge { ... }
pub struct ServiceEdge { ... }
pub struct RoleAssignment { ... }

// Filter and scope types
pub struct GraphFilters {
    pub prong: ProngFilter,
    pub jurisdictions: Option<Vec<String>>,
    pub fund_types: Option<Vec<String>>,
    pub entity_types: Option<Vec<EntityType>>,
    pub as_of_date: NaiveDate,
    pub min_ownership_pct: Option<Decimal>,
    pub path_only: bool,
}

pub enum ProngFilter { Both, OwnershipOnly, ControlOnly }

pub enum GraphScope {
    SingleCbu { cbu_id: Uuid },
    Book { apex_entity_id: Uuid, apex_name: String },
    Jurisdiction { code: String },
    EntityNeighborhood { entity_id: Uuid, hops: u32 },
    Custom { description: String },
}
```

**Acceptance Criteria:**
- [ ] All existing CBU visualization tests still pass
- [ ] New types support both CBU container view and UBO forest view
- [ ] Serde serialization works for API responses

---

### Task 1.2: Create navigation history (`rust/src/graph/navigation.rs`)

```rust
pub struct NavigationHistory {
    back_stack: Vec<Uuid>,
    forward_stack: Vec<Uuid>,
    max_size: usize,
}

impl NavigationHistory {
    pub fn push(&mut self, entity_id: Uuid);
    pub fn go_back(&mut self) -> Option<Uuid>;
    pub fn go_forward(&mut self) -> Option<Uuid>;
    pub fn clear(&mut self);
}
```

---

### Task 1.3: Create filter logic (`rust/src/graph/filters.rs`)

```rust
impl EntityGraph {
    /// Check if an ownership edge passes current filters
    pub fn edge_visible(&self, edge: &OwnershipEdge) -> bool;
    
    /// Check if a node passes current filters
    pub fn node_visible(&self, node: &GraphNode) -> bool;
    
    /// Recompute visibility for all nodes/edges after filter change
    pub fn recompute_visibility(&mut self);
    
    /// Get visible children of a node
    pub fn visible_children(&self, entity_id: Uuid) -> Vec<Uuid>;
    
    /// Get visible parents of a node
    pub fn visible_parents(&self, entity_id: Uuid) -> Vec<Uuid>;
}
```

**Filter rules:**
- Temporal: edge must be effective as of `filters.as_of_date`
- Jurisdiction: node jurisdiction must be in `filters.jurisdictions` (if set)
- Prong: ownership edges hidden when `ControlOnly`, control edges hidden when `OwnershipOnly`
- Percentage: ownership edges below `min_ownership_pct` hidden (if set)

---

## Phase 2: Nom Navigation Parser

### Task 2.1: Create command enum (`rust/src/navigation/commands.rs`)

```rust
#[derive(Debug, Clone)]
pub enum NavCommand {
    // Scope commands
    LoadCbu { cbu_name: String },
    LoadBook { client_name: String },
    LoadJurisdiction { code: String },
    
    // Filter commands
    FilterJurisdiction { code: String },
    FilterFundType { fund_type: String },
    FilterProng { prong: ProngFilter },
    ClearFilters,
    AsOfDate { date: NaiveDate },
    
    // Navigation commands
    GoTo { entity_name: String },
    GoUp,
    GoDown { index: Option<usize>, name: Option<String> },
    GoSibling { direction: Direction },
    GoToTerminus,
    GoToClient,
    GoBack,
    GoForward,
    
    // Query commands
    Find { name: String },
    WhereIs { person_name: String, role: Option<String> },
    FindByRole { role: String },
    ListChildren,
    ListOwners,
    ListControllers,
    
    // Display commands
    ShowPath,
    ShowContext,
    ShowTree { depth: u32 },
    ExpandCbu { cbu_name: Option<String> },
    CollapseCbu { cbu_name: Option<String> },
    Zoom { level: ZoomLevel },
}

#[derive(Debug, Clone, Copy)]
pub enum Direction { Left, Right, Next, Prev }

#[derive(Debug, Clone, Copy)]
pub enum ZoomLevel { Overview, Standard, Detail, Custom(f32) }
```

---

### Task 2.2: Create Nom parser (`rust/src/navigation/parser.rs`)

**Add to Cargo.toml:**
```toml
nom = "7"
```

**Implement parsers for each command category:**

```rust
use nom::{branch::alt, bytes::complete::tag_no_case, ...};

pub fn parse_nav_command(input: &str) -> IResult<&str, NavCommand> {
    let input = input.trim();
    alt((
        parse_load_cbu,
        parse_load_book,
        parse_filter_jurisdiction,
        parse_filter_prong,
        parse_go_up,
        parse_go_down,
        parse_go_terminus,
        parse_where_is,
        parse_find,
        parse_show_path,
        parse_zoom,
        // ... etc
    ))(input)
}

// Natural language patterns to support:
// - "show me the Allianz book"
// - "focus on Lux" / "focus on Luxembourg" / "focus on LU"
// - "go up" / "parent" / "owner"
// - "go down" / "go down to subfund A" / "down 0"
// - "show ownership prong" / "show control prong"
// - "where is Hans a director"
// - "find AI fund"
// - "show path" / "path to UBO"
// - "zoom in" / "zoom out" / "zoom 0.5"
```

**Acceptance Criteria:**
- [ ] Parser handles case-insensitive input
- [ ] Parser handles quoted strings for entity names with spaces
- [ ] Parser returns clear error on unrecognized input
- [ ] All example commands in design doc parse correctly

---

### Task 2.3: Create executor (`rust/src/navigation/executor.rs`)

```rust
impl EntityGraph {
    pub fn execute(&mut self, cmd: NavCommand, db: &impl GraphRepository) -> NavResult {
        match cmd {
            NavCommand::LoadCbu { cbu_name } => self.load_cbu_scope(&cbu_name, db),
            NavCommand::LoadBook { client_name } => self.load_book_scope(&client_name, db),
            NavCommand::FilterProng { prong } => {
                self.filters.prong = prong;
                self.recompute_visibility();
                NavResult::FilterApplied
            }
            NavCommand::GoUp => self.navigate_up(),
            NavCommand::GoDown { index, name } => self.navigate_down(index, name),
            NavCommand::WhereIs { person_name, role } => {
                self.query_person_roles(&person_name, role.as_deref())
            }
            // ... implement all commands
        }
    }
}

pub enum NavResult {
    Navigated { from: Uuid, to: Uuid, node: Option<GraphNode> },
    AtTerminus,
    NoChildren,
    NoCursor,
    NotFound,
    FilterApplied,
    ScopeLoaded { scope: GraphScope, node_count: usize, edge_count: usize },
    QueryResult { query: String, results: Vec<QueryResultItem> },
    PathResult { path: Vec<PathNode> },
    ContextResult { node: GraphNode, roles: Vec<RoleAssignment>, edges: EdgeSummary },
    ZoomChanged(ZoomLevel),
    Error { message: String },
}
```

---

## Phase 3: Database Layer

### Task 3.1: Create unified repository (`rust/src/database/graph_repository.rs`)

**REPLACE** `visualization_repository.rs` with unified queries.

```rust
#[async_trait]
pub trait GraphRepository {
    /// Load graph for a single CBU (existing functionality)
    async fn load_cbu_graph(&self, cbu_id: Uuid, as_of: NaiveDate) -> Result<EntityGraph>;
    
    /// Load graph for all CBUs under an ownership apex (book)
    async fn load_book_graph(&self, apex_entity_id: Uuid, as_of: NaiveDate) -> Result<EntityGraph>;
    
    /// Load graph for a jurisdiction
    async fn load_jurisdiction_graph(&self, jurisdiction: &str, as_of: NaiveDate) -> Result<EntityGraph>;
    
    /// Find ownership apex (UBO terminus) for an entity
    async fn find_ownership_apex(&self, entity_id: Uuid, as_of: NaiveDate) -> Result<Option<Uuid>>;
    
    /// Derive book membership for all CBUs
    async fn derive_books(&self) -> Result<Vec<DerivedBook>>;
    
    /// Search entities by name
    async fn search_entities(&self, name_pattern: &str, scope: &GraphScope) -> Result<Vec<GraphNode>>;
    
    /// Find person's roles across scope
    async fn find_person_roles(&self, person_name: &str, scope: &GraphScope) -> Result<Vec<RoleAssignment>>;
}
```

**Key queries to implement:**

```sql
-- Walk ownership chain to terminus (CTE)
WITH RECURSIVE ownership_chain AS (
    SELECT entity_id, name, entity_type, jurisdiction,
           NULL::uuid as parent_id, NULL::numeric as pct, 0 as depth
    FROM "ob-poc".entities
    WHERE entity_id = $1
    
    UNION ALL
    
    SELECT e.entity_id, e.name, e.entity_type, e.jurisdiction,
           er.from_entity_id, er.percentage, oc.depth + 1
    FROM ownership_chain oc
    JOIN "ob-poc".entity_relationships er 
        ON er.to_entity_id = oc.entity_id
        AND er.relationship_type = 'ownership'
        AND (er.effective_to IS NULL OR er.effective_to >= $2)
        AND (er.effective_from IS NULL OR er.effective_from <= $2)
    JOIN "ob-poc".entities e ON e.entity_id = er.from_entity_id
    WHERE oc.depth < 20
)
SELECT * FROM ownership_chain;

-- Find all CBUs under an apex (for book loading)
WITH apex_descendants AS (
    -- Recursive CTE to find all entities owned by apex
    ...
)
SELECT c.* 
FROM "ob-poc".cbus c
JOIN apex_descendants ad ON ad.entity_id = c.commercial_client_entity_id;
```

---

### Task 3.2: Update existing views

Ensure `v_cbu_entity_with_roles` includes all needed fields:
- `role_category`
- `ubo_treatment`
- `kyc_obligation`
- `effective_from` / `effective_to`

---

## Phase 4: Graph Builder

### Task 4.1: Replace builder (`rust/src/graph/builder.rs`)

**REPLACE** existing `GraphBuilder` with one that:
1. Loads based on scope (CBU, Book, Jurisdiction)
2. Builds adjacency lists from edges
3. Computes `depth_from_terminus` for each node
4. Identifies termini (nodes with no parent owner)
5. Respects temporal filters

```rust
impl EntityGraph {
    pub async fn load(scope: GraphScope, db: &impl GraphRepository) -> Result<Self> {
        let mut graph = match &scope {
            GraphScope::SingleCbu { cbu_id } => {
                db.load_cbu_graph(*cbu_id, Local::now().date_naive()).await?
            }
            GraphScope::Book { apex_entity_id, .. } => {
                db.load_book_graph(*apex_entity_id, Local::now().date_naive()).await?
            }
            GraphScope::Jurisdiction { code } => {
                db.load_jurisdiction_graph(code, Local::now().date_naive()).await?
            }
            _ => return Err(anyhow!("Unsupported scope")),
        };
        
        graph.build_adjacency_lists();
        graph.compute_depths();
        graph.identify_termini();
        graph.scope = scope;
        
        Ok(graph)
    }
    
    fn build_adjacency_lists(&mut self) {
        // Clear existing
        for node in self.nodes.values_mut() {
            node.owners.clear();
            node.owned.clear();
            node.controlled_by.clear();
            node.controls.clear();
        }
        
        // Populate from ownership edges
        for edge in &self.ownership_edges {
            if let Some(owner) = self.nodes.get_mut(&edge.from_entity_id) {
                owner.owned.push(edge.to_entity_id);
            }
            if let Some(owned) = self.nodes.get_mut(&edge.to_entity_id) {
                owned.owners.push(edge.from_entity_id);
            }
        }
        
        // Populate from control edges
        for edge in &self.control_edges {
            if let Some(controller) = self.nodes.get_mut(&edge.controller_id) {
                controller.controls.push(edge.controlled_id);
            }
            if let Some(controlled) = self.nodes.get_mut(&edge.controlled_id) {
                controlled.controlled_by.push(edge.controller_id);
            }
        }
    }
    
    fn compute_depths(&mut self) {
        // BFS from each terminus to set depth_from_terminus
        for &terminus_id in &self.termini {
            let mut queue = VecDeque::new();
            queue.push_back((terminus_id, 0u32));
            
            while let Some((id, depth)) = queue.pop_front() {
                if let Some(node) = self.nodes.get_mut(&id) {
                    if node.depth_from_terminus.is_none() || depth < node.depth_from_terminus.unwrap() {
                        node.depth_from_terminus = Some(depth);
                    }
                    for &child_id in &node.owned {
                        queue.push_back((child_id, depth + 1));
                    }
                }
            }
        }
    }
    
    fn identify_termini(&mut self) {
        self.termini = self.nodes.values()
            .filter(|n| n.owners.is_empty())
            .filter(|n| !n.owned.is_empty()) // Has children, so is apex of something
            .map(|n| n.entity_id)
            .collect();
    }
}
```

---

## Phase 5: Layout Engine

### Task 5.1: Replace layout engine (`rust/src/graph/layout.rs`)

**REPLACE** existing tier-based layout with role-category-based layout.

```rust
impl EntityGraph {
    pub fn layout(&mut self, view_mode: ViewMode, canvas: CanvasSize) {
        match view_mode {
            ViewMode::CbuContainer => self.layout_cbu_container(canvas),
            ViewMode::UboForest => self.layout_ubo_forest(canvas),
            ViewMode::Combined => self.layout_combined(canvas),
        }
    }
    
    fn layout_ubo_forest(&mut self, canvas: CanvasSize) {
        // 1. Group nodes by role category
        let groups = self.group_by_role_category();
        
        // 2. Layout ownership pyramid (top section)
        self.layout_ownership_pyramid(&groups.ownership, canvas);
        
        // 3. Layout control overlay (adjacent to owned entities)
        self.layout_control_overlay(&groups.control);
        
        // 4. Layout services (flat bottom)
        self.layout_services_flat(&groups.services, canvas);
        
        // 5. Layout trading (flat right)
        self.layout_trading_flat(&groups.trading, canvas);
    }
    
    fn layout_ownership_pyramid(&mut self, ownership_ids: &[Uuid], canvas: CanvasSize) {
        // Layer by depth_from_terminus
        // Terminus at top, commercial clients lower
        // Within layer: sort by ownership percentage (larger = more central)
        ...
    }
}

pub enum ViewMode {
    CbuContainer,   // CBU as box with entities inside
    UboForest,      // Ownership pyramid with control overlay
    FundStructure,  // Umbrella → subfund tree
    Combined,       // All views merged
}
```

---

## Phase 6: API Integration

### Task 6.1: Update API endpoints

**Modify existing CBU visualization endpoint to use unified graph:**

```rust
// GET /api/cbu/{cbu_id}/graph
async fn get_cbu_graph(cbu_id: Uuid) -> impl IntoResponse {
    let graph = EntityGraph::load(
        GraphScope::SingleCbu { cbu_id },
        &repo
    ).await?;
    
    graph.layout(ViewMode::CbuContainer, default_canvas());
    Json(graph)
}

// NEW: GET /api/book/{client_name}/graph
async fn get_book_graph(client_name: String) -> impl IntoResponse {
    let apex = repo.find_apex_by_client_name(&client_name).await?;
    let graph = EntityGraph::load(
        GraphScope::Book { apex_entity_id: apex.entity_id, apex_name: apex.name },
        &repo
    ).await?;
    
    graph.layout(ViewMode::UboForest, default_canvas());
    Json(graph)
}

// NEW: POST /api/graph/navigate
async fn navigate(
    State(graph): State<Arc<Mutex<EntityGraph>>>,
    Json(command): Json<String>,
) -> impl IntoResponse {
    let cmd = parse_nav_command(&command)
        .map_err(|e| ApiError::ParseError(e.to_string()))?;
    
    let mut graph = graph.lock().await;
    let result = graph.execute(cmd, &repo).await;
    
    Json(NavigationResponse {
        result,
        cursor: graph.cursor,
        graph_update: graph.visible_subgraph(),
    })
}
```

---

## Phase 7: Testing

### Task 7.1: Unit tests for navigation

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_navigate_up_from_subfund() {
        let graph = build_test_graph_allianz();
        graph.cursor = Some(subfund_entity_id);
        
        let result = graph.navigate_up();
        
        assert!(matches!(result, NavResult::Navigated { .. }));
        assert_eq!(graph.cursor, Some(umbrella_entity_id));
    }
    
    #[test]
    fn test_navigate_up_at_terminus() {
        let graph = build_test_graph_allianz();
        graph.cursor = Some(allianz_se_id); // terminus
        
        let result = graph.navigate_up();
        
        assert!(matches!(result, NavResult::AtTerminus));
    }
    
    #[test]
    fn test_filter_ownership_prong() {
        let graph = build_test_graph_with_directors();
        graph.filters.prong = ProngFilter::OwnershipOnly;
        graph.recompute_visibility();
        
        assert!(graph.control_edges.iter().all(|e| !graph.edge_visible(e)));
        assert!(graph.ownership_edges.iter().any(|e| graph.edge_visible(e)));
    }
    
    #[test]
    fn test_where_is_query() {
        let graph = build_test_graph_with_directors();
        let result = graph.query_person_roles("Hans", Some("DIRECTOR"));
        
        assert!(matches!(result, NavResult::QueryResult { results, .. } if results.len() >= 1));
    }
}
```

### Task 7.2: Integration tests

```rust
#[tokio::test]
async fn test_load_allianz_book() {
    let repo = TestRepository::with_allianz_data();
    let graph = EntityGraph::load(
        GraphScope::Book { 
            apex_entity_id: ALLIANZ_SE_ID, 
            apex_name: "Allianz SE".into() 
        },
        &repo
    ).await.unwrap();
    
    assert!(graph.nodes.len() > 50); // ManCos + funds + entities
    assert!(graph.termini.contains(&ALLIANZ_SE_ID));
}

#[tokio::test]
async fn test_full_navigation_flow() {
    let graph = EntityGraph::load(...).await.unwrap();
    
    // Start at apex
    graph.execute(NavCommand::GoToTerminus, &repo);
    assert_eq!(graph.cursor, Some(ALLIANZ_SE_ID));
    
    // Go down to ManCo
    graph.execute(NavCommand::GoDown { index: Some(0), name: None }, &repo);
    // Verify we're at a ManCo
    
    // Filter to Lux
    graph.execute(NavCommand::FilterJurisdiction { code: "LU".into() }, &repo);
    // Verify only LU entities visible
    
    // Show ownership prong
    graph.execute(NavCommand::FilterProng { prong: ProngFilter::OwnershipOnly }, &repo);
    // Verify control edges hidden
}
```

### Task 7.3: Parser tests

```rust
#[test]
fn test_parse_show_book() {
    let (_, cmd) = parse_nav_command("show me the Allianz book").unwrap();
    assert!(matches!(cmd, NavCommand::LoadBook { client_name } if client_name == "Allianz"));
}

#[test]
fn test_parse_focus_lux() {
    let (_, cmd) = parse_nav_command("focus on Lux").unwrap();
    assert!(matches!(cmd, NavCommand::FilterJurisdiction { code } if code == "LU"));
}

#[test]
fn test_parse_where_is() {
    let (_, cmd) = parse_nav_command("where is Hans a director").unwrap();
    assert!(matches!(cmd, NavCommand::WhereIs { person_name, role } 
        if person_name == "Hans" && role == Some("DIRECTOR".into())));
}

#[test]
fn test_parse_go_down_named() {
    let (_, cmd) = parse_nav_command("go down to AI fund").unwrap();
    assert!(matches!(cmd, NavCommand::GoDown { name: Some(n), .. } if n.contains("AI")));
}
```

---

## Migration Checklist

### Before starting:
- [ ] Review existing `rust/src/graph/` code
- [ ] Review existing `visualization_repository.rs`
- [ ] Run existing CBU visualization tests - note which ones exist

### During implementation:
- [ ] Keep existing tests passing as you refactor
- [ ] Add new tests for each new capability
- [ ] Document breaking changes to API responses

### After completion:
- [ ] All existing CBU visualization functionality works
- [ ] New UBO navigation functionality works
- [ ] All navigation commands parse correctly
- [ ] API endpoints return correct data
- [ ] UI can render graph (test manually)

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `rust/src/graph/types.rs` | REPLACE | Unified EntityGraph, GraphNode, edges |
| `rust/src/graph/navigation.rs` | CREATE | NavigationHistory, nav primitives |
| `rust/src/graph/filters.rs` | CREATE | Filter logic, visibility computation |
| `rust/src/graph/builder.rs` | REPLACE | Build graph from DB for any scope |
| `rust/src/graph/layout.rs` | REPLACE | Role-category-based layout |
| `rust/src/graph/mod.rs` | MODIFY | Export new modules |
| `rust/src/navigation/mod.rs` | CREATE | Navigation module |
| `rust/src/navigation/commands.rs` | CREATE | NavCommand enum |
| `rust/src/navigation/parser.rs` | CREATE | Nom parser |
| `rust/src/navigation/executor.rs` | CREATE | Command execution |
| `rust/src/database/graph_repository.rs` | CREATE | Unified DB queries |
| `rust/src/database/visualization_repository.rs` | DELETE | Replaced by graph_repository |
| `rust/src/database/mod.rs` | MODIFY | Export new module |
| `rust/src/lib.rs` | MODIFY | Export navigation module |
| `Cargo.toml` | MODIFY | Add `nom = "7"` |

---

## Reference Documents

- `/docs/UBO_TREE_NAVIGATION_OPTIONS.md` - Design options paper
- `/docs/ROLE_TAXONOMY_V2_SPEC.md` - Role categories and layout hints
- `/ROLE_TAXONOMY_V2_SUMMARY.md` - Implementation summary
- `/docs/ARCHITECTURE_REVIEW_CBU_KYC_UBO.md` - Current architecture review

---

## Success Criteria

1. **CBU view works as before** - existing functionality preserved
2. **UBO view works** - can load book, navigate tree, filter prongs
3. **Navigation parser works** - all example commands parse
4. **Temporal filtering works** - as_of_date respected
5. **Zoom levels work** - graph has appropriate detail at each level
6. **Cross-CBU queries work** - "where is Hans a director" returns results across book
