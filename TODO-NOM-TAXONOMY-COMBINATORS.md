# TODO: Nom-Style Taxonomy Combinators

## Overview

Refactor `TaxonomyBuilder` from imperative style to nom-style combinators. The taxonomy construction should follow the same pattern as DSL parsing and navigation command parsing - declarative, composable, extensible.

**Principle**: Same mental model across all parsing - DSL text, navigation commands, entity taxonomies.

**Key Insight**: Taxonomies are fractal. Every node IS a taxonomy waiting to be expanded. A taxonomy of taxonomies.

```
Universe (taxonomy of clients/books)
    └── Galaxy (taxonomy of clusters)
            └── Constellation (taxonomy of CBUs)
                    └── Planet (taxonomy of entities)
                            └── Moon (taxonomy of ownership chain)
```

Each zoom level reveals a new taxonomy with its own:
- Membership rules (what nodes belong)
- Grouping strategy (how to cluster)  
- Metaphor (how to visualize)
- Expansion rule (what taxonomy to build when zoomed)

---

## Why Nom-Style

Current approach (imperative):
```rust
fn build_subtree(&self, node: &mut TaxonomyNode, ...) {
    if depth >= self.rules.max_depth { return; }
    if visited.contains(&node.id) { return; }
    match self.rules.grouping { ... }
    // Lots of if/else/match - hard to extend, test, compose
}
```

Target approach (combinators):
```rust
fn taxonomy_for(ctx: &TaxonomyContext) -> impl TaxonomyParser {
    match ctx {
        TaxonomyContext::Universe => {
            universe_root()
                .with_children(all_cbus())
                .grouped_by(jurisdiction())
                .summarized()
                .expandable()
        }
        TaxonomyContext::CbuUbo { cbu_id } => {
            cbu_root(*cbu_id)
                .traverse_up(ownership_edges())
                .until(natural_person().or(public_company()))
        }
    }
}
```

---

## Size Limits & Lazy Loading

**Hard limits:**
- Per-expansion: 200 nodes max
- Total in-memory: 10,000 nodes (soft cap with warning)
- Render: Handled by LOD (not taxonomy concern)

**Combinators for bounded building:**
```rust
.limited(n)           // Stop after n nodes
.summarized()         // Return cluster with count, not full children
.expandable()         // Mark has_more_children = true
.with_continuation()  // Add "Load more..." node if truncated
.lazy()               // Don't load children until expand() called
```

**Expansion flow:**
```
Universe (root)
├── Luxembourg (cluster, 412 CBUs) [expandable]
├── Ireland (cluster, 891 CBUs) [expandable]
└── Germany (cluster, 203 CBUs) [expandable]

User clicks Luxembourg:

Universe (root)
├── Luxembourg (expanded)
│   ├── UCITS (cluster, 287) [expandable]
│   ├── AIF (cluster, 98) [expandable]
│   └── Other (cluster, 27) [expandable]
├── Ireland (cluster, 891 CBUs) [expandable]
└── Germany (cluster, 203 CBUs) [expandable]
```

---

## Fractal Taxonomy Model

**Every node IS a taxonomy:**

```rust
pub struct TaxonomyNode {
    // ... existing fields ...
    
    /// How to expand this node into its own taxonomy
    pub expansion: Option<ExpansionRule>,
}

pub enum ExpansionRule {
    /// Use this parser when expanded
    Parser(Box<dyn TaxonomyParser + Send + Sync>),
    
    /// Derive parser from context
    Context(TaxonomyContext),
    
    /// Already fully loaded (leaf)
    Complete,
    
    /// No expansion possible
    Terminal,
}
```

**Nested combinator:**

```rust
universe_root()
    .with_children(all_clients())
    .grouped_by(jurisdiction())
    .summarized()
    .each_is_taxonomy(|client| {
        // Each client IS a Book taxonomy
        book_root(client.id)
            .with_children(cbus_for_client(client.id))
            .grouped_by(fund_type())
            .each_is_taxonomy(|cbu| {
                // Each CBU IS a Trading taxonomy
                cbu_root(cbu.id)
                    .with_children(role_holders())
                    .grouped_by(role_category())
                    .each_is_taxonomy(|entity| {
                        // Each entity IS an ownership taxonomy
                        entity_root(entity.id)
                            .traverse_up(ownership_edges())
                            .until(natural_person().or(public_company()))
                    })
            })
    })
```

**Navigation as taxonomy stack:**

```rust
pub struct TaxonomyStack {
    /// Stack of taxonomies (zoom levels)
    stack: Vec<TaxonomyFrame>,
    
    /// Current data source
    source: Arc<dyn DataSource>,
}

pub struct TaxonomyFrame {
    /// The taxonomy at this level
    pub taxonomy: TaxonomyNode,
    
    /// Which node was selected to drill down
    pub selected_id: Option<Uuid>,
    
    /// Breadcrumb label
    pub label: String,
}

impl TaxonomyStack {
    /// Zoom into a node (push new taxonomy)
    pub fn zoom_in(&mut self, node_id: Uuid) -> Result<()> {
        let current = self.stack.last().ok_or(anyhow!("Empty stack"))?;
        let node = current.taxonomy.find(node_id)
            .ok_or(anyhow!("Node not found"))?;
        
        match &node.expansion {
            Some(ExpansionRule::Parser(parser)) => {
                let expanded = parser.parse(self.source.as_ref())?;
                self.stack.push(TaxonomyFrame {
                    taxonomy: expanded,
                    selected_id: Some(node_id),
                    label: node.label.clone(),
                });
                Ok(())
            }
            Some(ExpansionRule::Context(ctx)) => {
                let parser = parser_for_context(ctx);
                let expanded = parser.parse(self.source.as_ref())?;
                self.stack.push(TaxonomyFrame {
                    taxonomy: expanded,
                    selected_id: Some(node_id),
                    label: node.label.clone(),
                });
                Ok(())
            }
            Some(ExpansionRule::Complete) => {
                // Already fully loaded, just focus
                Ok(())
            }
            Some(ExpansionRule::Terminal) | None => {
                Err(anyhow!("Node cannot be expanded"))
            }
        }
    }
    
    /// Zoom out (pop taxonomy)
    pub fn zoom_out(&mut self) -> Option<TaxonomyFrame> {
        if self.stack.len() > 1 {
            self.stack.pop()
        } else {
            None // Can't zoom out of universe
        }
    }
    
    /// Current taxonomy
    pub fn current(&self) -> Option<&TaxonomyNode> {
        self.stack.last().map(|f| &f.taxonomy)
    }
    
    /// Breadcrumb path
    pub fn breadcrumbs(&self) -> Vec<&str> {
        self.stack.iter().map(|f| f.label.as_str()).collect()
    }
}
```

**Visual flow:**

```
UNIVERSE VIEW (depth 0)
    ⭐ Allianz (412)     ⭐ BlackRock (891)     ⭐ Vanguard (544)
    [each star has expansion: Book taxonomy]
         │
         │ zoom_in(Allianz)
         ▼
BOOK VIEW (depth 1) - breadcrumb: Universe > Allianz
    🌍 Luxembourg (177)   🌍 Ireland (142)   🌍 Germany (93)
    [each cluster has expansion: Jurisdiction taxonomy]
         │
         │ zoom_in(Luxembourg)
         ▼
JURISDICTION VIEW (depth 2) - breadcrumb: Universe > Allianz > Luxembourg
    ✦ Equity (87)    ✦ Bond (52)    ✦ Mixed (38)
    [each group has expansion: CBU list taxonomy]
         │
         │ zoom_in(Equity) then select specific CBU
         ▼
CBU VIEW (depth 3) - breadcrumb: Universe > Allianz > Luxembourg > Allianz Global Equity
    [CUSTODIAN]       [IM]           [TA]
    BNY Mellon        Allianz GI     CACEIS
    [each entity has expansion: UBO taxonomy]
         │
         │ zoom_in(Allianz GI)
         ▼
UBO VIEW (depth 4) - breadcrumb: ... > Allianz GI
         👤 Oliver Bäte
              │
         Allianz SE
         /        \
    Allianz AM   Allianz GI
    [natural persons are Terminal - no further expansion]
```

**Voice navigation maps to stack:**

| Voice | Action |
|-------|--------|
| "Show me Allianz" | `zoom_in(allianz_id)` |
| "Luxembourg" | `zoom_in(lu_cluster_id)` |
| "Open that fund" | `zoom_in(selected_cbu_id)` |
| "Show UBOs" | `zoom_in(entity_id)` with UBO context |
| "Back" | `zoom_out()` |
| "Back to universe" | `zoom_out()` until depth 0 |

---

## Phase 1: Core Trait & Primitives

**File**: `rust/src/taxonomy/combinators/mod.rs`

Define the core trait:
```rust
pub trait TaxonomyParser: Sized {
    /// Parse/build taxonomy from data source
    fn parse(&self, source: &dyn DataSource) -> Result<TaxonomyNode>;
    
    /// Combinator: add children from a node source
    fn with_children<C: NodeSource>(self, children: C) -> WithChildren<Self, C>;
    
    /// Combinator: group children by dimension
    fn grouped_by<G: Grouper>(self, grouper: G) -> GroupedBy<Self, G>;
    
    /// Combinator: traverse edges in direction
    fn traverse<E: EdgeFilter>(self, edges: E, direction: Direction) -> Traverse<Self, E>;
    
    /// Combinator: stop at terminus condition
    fn until<T: Terminus>(self, terminus: T) -> Until<Self, T>;
    
    /// Combinator: limit depth
    fn max_depth(self, depth: u32) -> MaxDepth<Self>;
    
    /// Combinator: limit node count per level
    fn limited(self, max_nodes: usize) -> Limited<Self>;
    
    /// Combinator: return summary instead of full tree
    fn summarized(self) -> Summarized<Self>;
    
    /// Combinator: mark as expandable (lazy)
    fn expandable(self) -> Expandable<Self>;
    
    /// Combinator: alternative parser
    fn or<P: TaxonomyParser>(self, other: P) -> Or<Self, P>;
    
    /// Combinator: transform result
    fn map<F, T>(self, f: F) -> Map<Self, F> where F: Fn(TaxonomyNode) -> T;
    
    /// Combinator: each child is itself a taxonomy (fractal)
    fn each_is_taxonomy<F, P>(self, f: F) -> NestedTaxonomy<Self, F>
    where
        F: Fn(&TaxonomyNode) -> P + Send + Sync + 'static,
        P: TaxonomyParser + Send + Sync + 'static;
}
```

**Supporting traits:**
```rust
pub trait DataSource: Send + Sync {
    fn load_entities(&self, filter: &EntityFilter) -> Result<Vec<EntityData>>;
    fn load_edges(&self, filter: &EdgeFilter) -> Result<Vec<EdgeData>>;
    fn count_entities(&self, filter: &EntityFilter) -> Result<usize>;
}

pub trait NodeSource {
    fn load(&self, source: &dyn DataSource) -> Result<Vec<EntityData>>;
}

pub trait Grouper {
    fn group(&self, entities: &[EntityData]) -> HashMap<String, Vec<EntityData>>;
    fn dimension(&self) -> Dimension;
}

pub trait Terminus {
    fn is_terminus(&self, entity: &EntityData) -> bool;
}

pub trait EdgeFilter {
    fn matches(&self, edge: &EdgeData) -> bool;
}
```

---

## Phase 2: Primitive Parsers

**File**: `rust/src/taxonomy/combinators/primitives.rs`

Root parsers:
```rust
pub fn universe_root() -> UniverseRoot { ... }
pub fn book_root(client_id: Uuid) -> BookRoot { ... }
pub fn cbu_root(cbu_id: Uuid) -> CbuRoot { ... }
pub fn entity_root(entity_id: Uuid) -> EntityRoot { ... }
```

Node sources:
```rust
pub fn all_cbus() -> AllCbus { ... }
pub fn cbus_for_client(client_id: Uuid) -> ClientCbus { ... }
pub fn entities_for_cbu(cbu_id: Uuid) -> CbuEntities { ... }
pub fn role_holders_for_cbu(cbu_id: Uuid) -> RoleHolders { ... }
pub fn owners_of(entity_id: Uuid) -> Owners { ... }
pub fn controlled_by(entity_id: Uuid) -> Controllers { ... }
```

Groupers:
```rust
pub fn jurisdiction() -> JurisdictionGrouper { ... }
pub fn fund_type() -> FundTypeGrouper { ... }
pub fn entity_type() -> EntityTypeGrouper { ... }
pub fn role_category() -> RoleCategoryGrouper { ... }
pub fn status() -> StatusGrouper { ... }
```

Terminus conditions:
```rust
pub fn natural_person() -> NaturalPersonTerminus { ... }
pub fn public_company() -> PublicCompanyTerminus { ... }
pub fn max_depth(n: u32) -> MaxDepthTerminus { ... }
pub fn no_more_edges() -> NoMoreEdgesTerminus { ... }
```

Edge filters:
```rust
pub fn ownership_edges() -> OwnershipEdgeFilter { ... }
pub fn control_edges() -> ControlEdgeFilter { ... }
pub fn fund_edges() -> FundEdgeFilter { ... }
pub fn any_edge() -> AnyEdgeFilter { ... }
```

---

## Phase 3: Combinator Implementations

**File**: `rust/src/taxonomy/combinators/impls.rs`

Each combinator is a struct that wraps inner parser and implements `TaxonomyParser`:

```rust
pub struct WithChildren<P, C> {
    inner: P,
    children: C,
}

impl<P: TaxonomyParser, C: NodeSource> TaxonomyParser for WithChildren<P, C> {
    fn parse(&self, source: &dyn DataSource) -> Result<TaxonomyNode> {
        let mut node = self.inner.parse(source)?;
        let children = self.children.load(source)?;
        for child in children {
            node.add_child(entity_to_node(&child));
        }
        Ok(node)
    }
    // ... delegate other combinators
}

pub struct GroupedBy<P, G> {
    inner: P,
    grouper: G,
}

impl<P: TaxonomyParser, G: Grouper> TaxonomyParser for GroupedBy<P, G> {
    fn parse(&self, source: &dyn DataSource) -> Result<TaxonomyNode> {
        let node = self.inner.parse(source)?;
        // Extract children, group them, create cluster nodes
        // ...
    }
}

pub struct Limited<P> {
    inner: P,
    max_nodes: usize,
}

impl<P: TaxonomyParser> TaxonomyParser for Limited<P> {
    fn parse(&self, source: &dyn DataSource) -> Result<TaxonomyNode> {
        let mut node = self.inner.parse(source)?;
        // Truncate children, add continuation node if needed
        if node.children.len() > self.max_nodes {
            let remaining = node.children.len() - self.max_nodes;
            node.children.truncate(self.max_nodes);
            node.children.push(continuation_node(remaining));
        }
        Ok(node)
    }
}

pub struct Summarized<P> {
    inner: P,
}

impl<P: TaxonomyParser> TaxonomyParser for Summarized<P> {
    fn parse(&self, source: &dyn DataSource) -> Result<TaxonomyNode> {
        // Don't load full children, just counts
        // Return cluster nodes with descendant_count set but children empty
        // Mark has_more_children = true
    }
}
```

---

## Phase 4: Context-to-Parser Mapping

**File**: `rust/src/taxonomy/combinators/contexts.rs`

Replace `MembershipRules::from_context` with combinator composition:

```rust
pub fn parser_for_context(ctx: &TaxonomyContext) -> Box<dyn TaxonomyParser> {
    match ctx {
        TaxonomyContext::Universe => Box::new(
            universe_root()
                .with_children(all_cbus())
                .grouped_by(jurisdiction())
                .summarized()
                .expandable()
        ),
        
        TaxonomyContext::Book { client_id } => Box::new(
            book_root(*client_id)
                .with_children(cbus_for_client(*client_id))
                .grouped_by(jurisdiction())
                .limited(200)
        ),
        
        TaxonomyContext::CbuTrading { cbu_id } => Box::new(
            cbu_root(*cbu_id)
                .with_children(role_holders_for_cbu(*cbu_id))
                .grouped_by(role_category())
        ),
        
        TaxonomyContext::CbuUbo { cbu_id } => Box::new(
            cbu_root(*cbu_id)
                .traverse(ownership_edges(), Direction::Up)
                .until(natural_person().or(public_company()))
                .max_depth(15)
        ),
        
        TaxonomyContext::EntityForest { filters } => Box::new(
            universe_root()
                .with_children(entities_matching(filters))
                .grouped_by(entity_type())
                .summarized()
        ),
    }
}
```

---

## Phase 5: Expand Operation

**File**: `rust/src/taxonomy/combinators/expand.rs`

Handle lazy expansion:

```rust
pub fn expand_node(
    node_id: Uuid,
    current_taxonomy: &TaxonomyNode,
    source: &dyn DataSource,
) -> Result<TaxonomyNode> {
    // Find the node to expand
    let node = current_taxonomy.find(node_id)
        .ok_or_else(|| anyhow!("Node not found"))?;
    
    // Determine expansion parser from node context
    let parser = expansion_parser_for(node);
    
    // Build expanded subtree
    let expanded = parser.parse(source)?;
    
    // Return new taxonomy with node replaced by expanded version
    Ok(current_taxonomy.with_replacement(node_id, expanded))
}

fn expansion_parser_for(node: &TaxonomyNode) -> Box<dyn TaxonomyParser> {
    match node.node_type {
        NodeType::Cluster => {
            // Expand cluster to show children grouped by next dimension
            cluster_expansion(node)
        }
        NodeType::Cbu => {
            // Expand CBU to show entities
            cbu_expansion(node.id)
        }
        NodeType::Entity => {
            // Expand entity to show owners/subsidiaries
            entity_expansion(node.id)
        }
        _ => empty_parser()
    }
}
```

---

## Phase 6: Taxonomy Stack (Fractal Navigation)

**File**: `rust/src/taxonomy/stack.rs`

The navigation model - a stack of taxonomies representing zoom levels:

```rust
pub struct TaxonomyStack {
    stack: Vec<TaxonomyFrame>,
    source: Arc<dyn DataSource>,
}

pub struct TaxonomyFrame {
    pub taxonomy: TaxonomyNode,
    pub selected_id: Option<Uuid>,
    pub label: String,
    pub context: TaxonomyContext,
}

impl TaxonomyStack {
    pub fn new(source: Arc<dyn DataSource>) -> Self;
    
    /// Initialize with universe view
    pub async fn init_universe(&mut self) -> Result<()>;
    
    /// Zoom into a node
    pub fn zoom_in(&mut self, node_id: Uuid) -> Result<()>;
    
    /// Zoom out one level
    pub fn zoom_out(&mut self) -> Option<TaxonomyFrame>;
    
    /// Zoom out to specific depth
    pub fn zoom_to_depth(&mut self, depth: usize);
    
    /// Current taxonomy
    pub fn current(&self) -> Option<&TaxonomyNode>;
    
    /// Current depth
    pub fn depth(&self) -> usize;
    
    /// Breadcrumb path
    pub fn breadcrumbs(&self) -> Vec<Breadcrumb>;
    
    /// Can zoom in on node?
    pub fn can_zoom_in(&self, node_id: Uuid) -> bool;
    
    /// Can zoom out?
    pub fn can_zoom_out(&self) -> bool;
}

pub struct Breadcrumb {
    pub label: String,
    pub depth: usize,
    pub node_id: Option<Uuid>,
}
```

**Integration with ViewState:**

```rust
// In session/view_state.rs
pub struct ViewState {
    /// Taxonomy stack (replaces single taxonomy)
    pub stack: TaxonomyStack,
    
    /// Active refinements at current level
    pub refinements: Vec<Refinement>,
    
    /// Selection at current level
    pub selection: Vec<Uuid>,
    
    /// Pending operation
    pub pending: Option<PendingOperation>,
    
    // ... rest unchanged
}

impl ViewState {
    pub fn current_taxonomy(&self) -> Option<&TaxonomyNode> {
        self.stack.current()
    }
    
    pub fn zoom_in(&mut self, node_id: Uuid) -> Result<()> {
        self.stack.zoom_in(node_id)?;
        self.refinements.clear();  // Reset refinements at new level
        self.recompute_selection();
        Ok(())
    }
    
    pub fn zoom_out(&mut self) -> Result<()> {
        self.stack.zoom_out();
        self.refinements.clear();
        self.recompute_selection();
        Ok(())
    }
}
```

---

## Phase 7: Integration

**File**: `rust/src/taxonomy/builder.rs`

Refactor `TaxonomyBuilder` to use combinators internally:

```rust
impl TaxonomyBuilder {
    pub fn new(rules: MembershipRules) -> Self {
        // Convert rules to context (temporary compatibility)
        Self { rules }
    }
    
    pub async fn build(&self, pool: &PgPool) -> Result<TaxonomyNode> {
        let source = DatabaseSource::new(pool);
        let ctx = self.rules_to_context();
        let parser = parser_for_context(&ctx);
        
        let mut taxonomy = parser.parse(&source)?;
        taxonomy.compute_metrics();
        
        Ok(taxonomy)
    }
}
```

Eventually, `TaxonomyBuilder` becomes a thin wrapper or is replaced entirely by direct combinator use.

---

## Phase 8: Tests

**File**: `rust/src/taxonomy/combinators/tests.rs`

Test combinators in isolation:

```rust
#[test]
fn test_grouped_by_jurisdiction() {
    let parser = universe_root()
        .with_children(mock_cbus(100))
        .grouped_by(jurisdiction());
    
    let result = parser.parse(&mock_source()).unwrap();
    
    assert_eq!(result.children.len(), 5); // LU, IE, DE, US, Other
    assert!(result.children.iter().all(|c| c.node_type == NodeType::Cluster));
}

#[test]
fn test_limited_truncates() {
    let parser = universe_root()
        .with_children(mock_cbus(500))
        .limited(100);
    
    let result = parser.parse(&mock_source()).unwrap();
    
    assert_eq!(result.children.len(), 101); // 100 + continuation
    assert!(result.children.last().unwrap().node_type == NodeType::Continuation);
}

#[test]
fn test_summarized_loads_counts_only() {
    let parser = universe_root()
        .with_children(all_cbus())
        .grouped_by(jurisdiction())
        .summarized();
    
    let result = parser.parse(&mock_source()).unwrap();
    
    // Children are clusters with counts but no loaded children
    for cluster in &result.children {
        assert!(cluster.children.is_empty());
        assert!(cluster.has_more_children);
        assert!(cluster.descendant_count > 0);
    }
}

#[test]
fn test_ubo_traversal_stops_at_terminus() {
    let parser = cbu_root(test_cbu_id())
        .traverse(ownership_edges(), Direction::Up)
        .until(natural_person());
    
    let result = parser.parse(&mock_source()).unwrap();
    
    // All leaf nodes should be natural persons
    for leaf in result.leaves() {
        assert_eq!(leaf.entity_data.unwrap().entity_type, "proper_person");
    }
}

#[test]
fn test_nested_taxonomy_attaches_expansion() {
    let parser = universe_root()
        .with_children(mock_clients(3))
        .each_is_taxonomy(|client| {
            book_root(client.id)
                .with_children(mock_cbus(10))
        });
    
    let result = parser.parse(&mock_source()).unwrap();
    
    // Each child should have an expansion rule
    for child in &result.children {
        assert!(child.expansion.is_some());
        match &child.expansion {
            Some(ExpansionRule::Parser(_)) => {}
            _ => panic!("Expected Parser expansion rule"),
        }
    }
}

#[test]
fn test_taxonomy_stack_zoom_in_out() {
    let mut stack = TaxonomyStack::new(mock_source());
    stack.init_universe().unwrap();
    
    assert_eq!(stack.depth(), 1);
    assert_eq!(stack.breadcrumbs().len(), 1);
    
    // Zoom into first client
    let client_id = stack.current().unwrap().children[0].id;
    stack.zoom_in(client_id).unwrap();
    
    assert_eq!(stack.depth(), 2);
    assert_eq!(stack.breadcrumbs().len(), 2);
    
    // Zoom out
    stack.zoom_out();
    assert_eq!(stack.depth(), 1);
}

#[test]
fn test_taxonomy_stack_breadcrumbs() {
    let mut stack = TaxonomyStack::new(mock_source());
    stack.init_universe().unwrap();
    
    // Drill down: Universe > Allianz > Luxembourg > CBU
    stack.zoom_in(allianz_id()).unwrap();
    stack.zoom_in(lu_cluster_id()).unwrap();
    stack.zoom_in(cbu_id()).unwrap();
    
    let crumbs: Vec<_> = stack.breadcrumbs().iter().map(|b| &b.label).collect();
    assert_eq!(crumbs, vec!["Universe", "Allianz", "Luxembourg", "Allianz Global Equity"]);
}

#[test]
fn test_terminal_nodes_cannot_expand() {
    let parser = entity_root(natural_person_id())
        .traverse_up(ownership_edges())
        .until(natural_person());
    
    let result = parser.parse(&mock_source()).unwrap();
    
    // Natural person leaf should be terminal
    let leaf = result.leaves().next().unwrap();
    assert!(matches!(leaf.expansion, Some(ExpansionRule::Terminal)));
}
```

---

## Key Files Summary

| File | Purpose |
|------|---------|
| `rust/src/taxonomy/combinators/mod.rs` | Core trait, re-exports |
| `rust/src/taxonomy/combinators/traits.rs` | `TaxonomyParser`, `NodeSource`, `Grouper`, `Terminus` |
| `rust/src/taxonomy/combinators/primitives.rs` | Root parsers, node sources, groupers, terminus |
| `rust/src/taxonomy/combinators/impls.rs` | Combinator struct implementations |
| `rust/src/taxonomy/combinators/nested.rs` | `NestedTaxonomy`, `ExpansionRule` |
| `rust/src/taxonomy/combinators/contexts.rs` | Context → parser mapping |
| `rust/src/taxonomy/combinators/expand.rs` | Lazy expansion logic |
| `rust/src/taxonomy/stack.rs` | `TaxonomyStack` for fractal navigation |
| `rust/src/taxonomy/combinators/tests.rs` | Unit tests |
| `rust/src/taxonomy/node.rs` | Add `expansion: Option<ExpansionRule>` field |
| `rust/src/taxonomy/builder.rs` | Refactor to use combinators |
| `rust/src/session/view_state.rs` | Use `TaxonomyStack` instead of single taxonomy |

---

## Validation

- [ ] `cargo build` succeeds
- [ ] Existing `TaxonomyBuilder` tests pass (backward compat)
- [ ] Combinator unit tests pass
- [ ] Universe view loads summarized (not 5000 full nodes)
- [ ] Expansion loads bounded children (≤200)
- [ ] UBO traversal stops at natural person/public company
- [ ] Trading view groups by role category
- [ ] **Nested taxonomy: each node has expansion rule**
- [ ] **TaxonomyStack: zoom in/out works**
- [ ] **Breadcrumbs: correct path at each level**
- [ ] **Terminal nodes: cannot expand further**
- [ ] Performance: <50ms for universe summary, <5ms for expansion

---

## Notes

- This is a refactor of BUILD mechanism, not data model
- `TaxonomyNode` struct gets one new field: `expansion: Option<ExpansionRule>`
- `TaxonomyContext` enum unchanged  
- Combinators compose the same operations, just declaratively
- Follow nom pattern: small composable functions, not big match statements

**Key architectural insight:**

The data IS fractal. Every node is a potential taxonomy. The "enhance" from Blade Runner is exactly this - each zoom level reveals a new taxonomy with its own rules, its own structure, its own visualization metaphor.

```
Universe → Galaxy → Constellation → Planet → Moon → Asteroid
   │          │           │           │        │        │
   │          │           │           │        │        └── Terminal (no expand)
   │          │           │           │        └── UBO chain taxonomy
   │          │           │           └── Trading network taxonomy
   │          │           └── CBU list taxonomy
   │          └── Cluster taxonomy (by fund type, etc.)
   └── Book taxonomy (client's CBUs)
```

Navigation = traversing taxonomy stack. Voice commands map directly:
- "Show me X" = zoom_in(X)
- "Back" = zoom_out()
- "Back to universe" = zoom_to_depth(0)
- "That one" = zoom_in(gaze_target)
