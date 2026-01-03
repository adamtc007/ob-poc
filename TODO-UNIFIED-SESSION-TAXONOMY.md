# TODO: Unified Session + Taxonomy Architecture

## Overview

Session = Intent Scope = Visual State = Operation Target. They are THE SAME THING.

The user expresses intent to the agent. That intent always needs session context - what is the DSL "looking at". The session shows the user the scope/coverage of their intent, and IS what they see visually.

## Core Principle

```
User Intent → Session State Change → { Agent Context, DSL State, Visual } all update
```

One source of truth. Three expressions:
- **Agent context**: "You're looking at 12 LU equity CBUs with CUSTODY pending"
- **DSL REPL**: `@_ = [12 UUIDs]`, `@pending = CUSTODY`
- **Visualization**: 12 highlighted stars, preview overlay

---

## Phase 1: Core Structs

### 1.1 TaxonomyNode (Universal Tree Structure)

**File**: `rust/src/taxonomy/node.rs` (new module)

```rust
/// Universal taxonomy node - works for everything
/// Shape determines metaphor, not the other way around
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyNode {
    pub id: Uuid,
    pub node_type: NodeType,
    pub label: String,
    pub short_label: Option<String>,
    pub children: Vec<TaxonomyNode>,
    
    // Computed by builder
    pub depth: u32,
    pub descendant_count: usize,
    
    // For layout
    pub dimensions: DimensionValues,
    
    // Optional entity data (lazy loaded)
    pub entity_data: Option<EntitySummary>,
}

impl TaxonomyNode {
    /// Astro level derived from characteristics
    pub fn astro_level(&self) -> AstroLevel;
    
    /// Visual metaphor derived from tree shape
    pub fn metaphor(&self) -> Metaphor;
    
    /// Max depth of subtree
    pub fn max_depth(&self) -> u32;
    
    /// Max width at any level
    pub fn max_width(&self) -> usize;
    
    /// Collect all node IDs in subtree
    pub fn all_ids(&self) -> Vec<Uuid>;
    
    /// Find node by ID
    pub fn find(&self, id: Uuid) -> Option<&TaxonomyNode>;
}
```

**File**: `rust/src/taxonomy/types.rs`

```rust
#[derive(Debug, Clone, Copy)]
pub enum AstroLevel {
    Universe,      // Root with 1000s of descendants
    Galaxy,        // Client/Book with 100s
    SolarSystem,   // Cluster with 10s
    Planet,        // CBU with entities
    Moon,          // Entity
    Asteroid,      // Detail (doc, etc.)
}

#[derive(Debug, Clone, Copy)]
pub enum Metaphor {
    Galaxy,        // Massive (500+) - semantic clustering
    Constellation, // Large (50-500) - grouped stars
    Pyramid,       // Deep (5+ levels) - ownership chain
    Network,       // Wide (10+ at level) - force directed
    Tree,          // Default - simple tree
}

#[derive(Debug, Clone, Copy)]
pub enum NodeType {
    Root,
    Client,
    Cluster,       // Jurisdiction, ManCo, FundType grouping
    Cbu,
    Entity,
    Position,      // Role/office holder
    Document,
    Observation,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DimensionValues {
    pub jurisdiction: Option<u8>,      // index into jurisdiction list
    pub fund_type: Option<u8>,         // enum as u8
    pub status: Option<u8>,            // RED=0, AMBER=1, GREEN=2
    pub aum_log: Option<u8>,           // log scale 0-255
    pub kyc_completion: Option<u8>,    // 0-100
    pub entity_type: Option<u8>,       // index
    pub role_category: Option<u8>,     // index
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySummary {
    pub name: String,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub status: Option<String>,
}
```

### 1.2 MembershipRules + TaxonomyBuilder

**File**: `rust/src/taxonomy/rules.rs`

```rust
/// Context determines which taxonomy to build
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaxonomyContext {
    /// All CBUs user can see
    Universe,
    
    /// All CBUs for a client
    Book { client_id: Uuid },
    
    /// Single CBU - trading network view
    CbuTrading { cbu_id: Uuid },
    
    /// Single CBU - UBO ownership view
    CbuUbo { cbu_id: Uuid },
    
    /// Entity forest by type/ownership
    EntityForest { filters: Vec<Filter> },
}

/// Membership rules - compiled from context
#[derive(Debug, Clone)]
pub struct MembershipRules {
    /// Root entity filter
    pub root_filter: RootFilter,
    
    /// Which entities to include as nodes
    pub entity_filter: EntityFilter,
    
    /// Which edges to traverse
    pub edge_types: Vec<EdgeType>,
    
    /// How to group/nest children
    pub grouping: GroupingStrategy,
    
    /// Traversal direction
    pub direction: TraversalDirection,
    
    /// Terminus conditions (when to stop)
    pub terminus: TerminusCondition,
    
    /// Max depth
    pub max_depth: u32,
}

#[derive(Debug, Clone)]
pub enum RootFilter {
    AllCbus,
    Client { client_id: Uuid },
    SingleCbu { cbu_id: Uuid },
    Entities { filters: Vec<Filter> },
}

#[derive(Debug, Clone)]
pub enum GroupingStrategy {
    None,                              // Flat children
    ByDimension(Dimension),            // Group by jurisdiction, fund_type, etc.
    ByRole,                            // Group by role category (trading view)
    ByOwnership,                       // Ownership chain hierarchy
}

#[derive(Debug, Clone, Copy)]
pub enum TraversalDirection {
    Down,      // From root to leaves (normal)
    Up,        // From target to root (UBO chain)
    Both,      // Bidirectional
}

#[derive(Debug, Clone)]
pub enum TerminusCondition {
    MaxDepth,
    NaturalPerson,         // Stop at natural persons
    PublicCompany,         // Stop at public companies
    NoMoreOwners,          // Stop when no parent owners
    Custom(String),        // Custom predicate name
}

impl MembershipRules {
    /// Build rules from context
    pub fn from_context(ctx: &TaxonomyContext) -> Self {
        match ctx {
            TaxonomyContext::Universe => Self::universe_rules(),
            TaxonomyContext::Book { client_id } => Self::book_rules(*client_id),
            TaxonomyContext::CbuTrading { cbu_id } => Self::trading_rules(*cbu_id),
            TaxonomyContext::CbuUbo { cbu_id } => Self::ubo_rules(*cbu_id),
            TaxonomyContext::EntityForest { filters } => Self::forest_rules(filters),
        }
    }
    
    fn universe_rules() -> Self;
    fn book_rules(client_id: Uuid) -> Self;
    fn trading_rules(cbu_id: Uuid) -> Self;
    fn ubo_rules(cbu_id: Uuid) -> Self;
    fn forest_rules(filters: &[Filter]) -> Self;
}
```

**File**: `rust/src/taxonomy/builder.rs`

```rust
/// Builds taxonomy tree from data + rules
/// Uses nom parser for fast tree construction
pub struct TaxonomyBuilder {
    rules: MembershipRules,
}

impl TaxonomyBuilder {
    pub fn new(rules: MembershipRules) -> Self {
        Self { rules }
    }
    
    /// Build taxonomy from database
    #[cfg(feature = "database")]
    pub async fn build(&self, pool: &PgPool) -> Result<TaxonomyNode> {
        // 1. Load root entities based on rules.root_filter
        // 2. For each root, traverse based on rules.direction and rules.edge_types
        // 3. Apply rules.grouping to organize children
        // 4. Stop at rules.terminus conditions
        // 5. Compute depth, descendant_count, dimensions
    }
    
    /// Build taxonomy from pre-loaded data
    pub fn build_from_data(&self, data: &DataSet) -> Result<TaxonomyNode>;
    
    /// Rebuild subtree (for expand operations)
    pub async fn expand_node(&self, node_id: Uuid, pool: &PgPool) -> Result<TaxonomyNode>;
}
```

**File**: `rust/src/taxonomy/mod.rs`

```rust
mod builder;
mod node;
mod rules;
mod types;

pub use builder::TaxonomyBuilder;
pub use node::TaxonomyNode;
pub use rules::{MembershipRules, TaxonomyContext};
pub use types::*;
```

### 1.3 ViewState (Session's Visual State)

**File**: `rust/src/session/view_state.rs` (new)

```rust
/// View state - the "it" that session is looking at
/// This IS what the user sees, what operations target, what agent knows about
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewState {
    /// Taxonomy stack - fractal navigation (zoom levels)
    /// See TODO-NOM-TAXONOMY-COMBINATORS.md for TaxonomyStack details
    pub stack: TaxonomyStack,
    
    /// Active refinements at CURRENT level ("except...", "plus...")
    pub refinements: Vec<Refinement>,
    
    /// Computed selection at CURRENT level (the actual "those" after refinements)
    /// This is what operations target
    pub selection: Vec<Uuid>,
    
    /// Staged operation awaiting confirmation
    pub pending: Option<PendingOperation>,
    
    /// Layout result for CURRENT level (computed positions for rendering)
    pub layout: Option<LayoutResult>,
    
    /// When this view was computed
    pub computed_at: DateTime<Utc>,
}

impl ViewState {
    /// Current taxonomy (top of stack)
    pub fn current_taxonomy(&self) -> Option<&TaxonomyNode> {
        self.stack.current()
    }
    
    /// Zoom into a node (push new taxonomy)
    pub fn zoom_in(&mut self, node_id: Uuid) -> Result<()> {
        self.stack.zoom_in(node_id)?;
        self.refinements.clear();  // Reset at new level
        self.recompute_selection();
        self.layout = None;  // Will recompute
        Ok(())
    }
    
    /// Zoom out (pop taxonomy)
    pub fn zoom_out(&mut self) -> Result<()> {
        self.stack.zoom_out()
            .ok_or_else(|| anyhow!("Already at universe level"))?;
        self.refinements.clear();
        self.recompute_selection();
        self.layout = None;
        Ok(())
    }
    
    /// Breadcrumb path
    pub fn breadcrumbs(&self) -> Vec<Breadcrumb> {
        self.stack.breadcrumbs()
    }
    
    /// Current depth (0 = universe)
    pub fn depth(&self) -> usize {
        self.stack.depth()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Refinement {
    /// Add filter: "only the Luxembourg ones"
    Include { filter: Filter },
    
    /// Remove filter: "except under 100M"
    Exclude { filter: Filter },
    
    /// Add specific entities: "and also ABC Fund"
    Add { ids: Vec<Uuid> },
    
    /// Remove specific entities: "but not that one"
    Remove { ids: Vec<Uuid> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOperation {
    /// What operation
    pub operation: BatchOperation,
    
    /// Target IDs (from selection)
    pub targets: Vec<Uuid>,
    
    /// Generated DSL verbs
    pub verbs: String,
    
    /// Preview of what will happen
    pub preview: OperationPreview,
    
    /// When staged
    pub staged_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchOperation {
    Subscribe { product: String },
    Unsubscribe { product: String },
    SetStatus { status: String },
    AssignRole { entity_id: Uuid, role: String },
    CreateFromResearch,
    EnrichFromResearch,
    Custom { verb: String, args: HashMap<String, serde_json::Value> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationPreview {
    pub summary: String,              // "Add CUSTODY to 12 CBUs"
    pub affected_count: usize,
    pub already_done_count: usize,    // "3 already have it"
    pub would_fail_count: usize,      // "2 missing prerequisites"
    pub estimated_duration: Option<Duration>,
}

impl ViewState {
    /// Create empty view state
    pub fn empty() -> Self;
    
    /// Create from taxonomy
    pub fn from_taxonomy(taxonomy: TaxonomyNode, context: TaxonomyContext) -> Self;
    
    /// Apply refinement, recompute selection
    pub fn refine(&mut self, refinement: Refinement);
    
    /// Clear refinements
    pub fn clear_refinements(&mut self);
    
    /// Stage an operation
    pub fn stage_operation(&mut self, operation: BatchOperation) -> Result<()>;
    
    /// Clear pending operation
    pub fn clear_pending(&mut self);
    
    /// Get selection count
    pub fn selection_count(&self) -> usize;
    
    /// Check if has pending operation
    pub fn has_pending(&self) -> bool;
    
    /// Generate DSL for pending operation
    fn generate_verbs(&self, operation: &BatchOperation) -> String;
}
```

### 1.4 Extend UnifiedSessionContext

**File**: `rust/src/session/mod.rs` (extend existing)

```rust
/// Unified session context
pub struct UnifiedSessionContext {
    // ... existing fields ...
    
    /// View state - the "it" (NEW)
    pub view: Option<ViewState>,
}

impl UnifiedSessionContext {
    /// Initialize view with universe (starting point)
    pub async fn init_universe_view(&mut self, pool: &PgPool) -> Result<()> {
        let source = Arc::new(DatabaseSource::new(pool.clone()));
        let mut stack = TaxonomyStack::new(source);
        stack.init_universe().await?;
        
        self.view = Some(ViewState {
            stack,
            refinements: Vec::new(),
            selection: Vec::new(),
            pending: None,
            layout: None,
            computed_at: Utc::now(),
        });
        
        // Compute initial selection (all nodes at current level)
        if let Some(view) = &mut self.view {
            view.recompute_selection();
        }
        
        Ok(())
    }
    
    /// Zoom into a node (push taxonomy onto stack)
    pub fn zoom_in(&mut self, node_id: Uuid) -> Result<()> {
        if let Some(view) = &mut self.view {
            view.zoom_in(node_id)
        } else {
            Err(anyhow!("No active view"))
        }
    }
    
    /// Zoom out (pop taxonomy from stack)
    pub fn zoom_out(&mut self) -> Result<()> {
        if let Some(view) = &mut self.view {
            view.zoom_out()
        } else {
            Err(anyhow!("No active view"))
        }
    }
    
    /// Apply refinement to current view level
    pub fn refine_view(&mut self, refinement: Refinement) -> Result<()> {
        if let Some(view) = &mut self.view {
            view.refine(refinement);
            Ok(())
        } else {
            Err(anyhow!("No active view"))
        }
    }
    
    /// Stage batch operation on current selection
    pub fn stage_operation(&mut self, operation: BatchOperation) -> Result<()> {
        if let Some(view) = &mut self.view {
            view.stage_operation(operation)
        } else {
            Err(anyhow!("No active view"))
        }
    }
    
    /// Execute pending operation
    pub async fn execute_pending(&mut self, pool: &PgPool) -> Result<ExecutionResult> {
        if let Some(view) = &mut self.view {
            if let Some(pending) = view.pending.take() {
                let result = self.execute_dsl(&pending.verbs, pool).await?;
                self.command_history.push(ExecutedCommand {
                    command: NavCommand::BatchExecute { 
                        operation: pending.operation.clone(),
                        count: pending.targets.len(),
                    },
                    executed_at: Utc::now(),
                    result_summary: format!("{} operations executed", pending.targets.len()),
                });
                Ok(result)
            } else {
                Err(anyhow!("No pending operation"))
            }
        } else {
            Err(anyhow!("No active view"))
        }
    }
    
    /// Get current view for rendering
    pub fn current_view(&self) -> Option<&ViewState>;
    
    /// Get breadcrumbs for UI
    pub fn breadcrumbs(&self) -> Vec<Breadcrumb> {
        self.view.as_ref()
            .map(|v| v.breadcrumbs())
            .unwrap_or_default()
    }
    
    /// Build agent context including view state
    pub fn build_agent_context(&self) -> AgentContext {
        // Include view state summary + breadcrumbs for agent prompt
    }
}
```

### 1.5 Integrate with ExecutionContext

**File**: `rust/src/dsl_v2/executor.rs` (extend)

```rust
pub struct ExecutionContext {
    // ... existing fields ...
    
    /// Current selection (from view.selection) - for batch operations
    /// Populated when view.* verbs execute
    pub current_selection: Option<Vec<Uuid>>,
}

impl ExecutionContext {
    /// Set selection from view state (called by view.* verbs)
    pub fn set_selection(&mut self, selection: Vec<Uuid>) {
        self.current_selection = Some(selection);
        // Also bind as @_ for DSL access
        self.bind_json("_selection", serde_json::to_value(&selection).unwrap());
    }
    
    /// Get current selection
    pub fn get_selection(&self) -> Option<&Vec<Uuid>> {
        self.current_selection.as_ref()
    }
}
```

---

## Phase 2: View Verbs

### 2.1 view.yaml

**File**: `rust/config/verbs/view.yaml` (new)

```yaml
domains:
  view:
    description: "Semantic view operations - set session scope and selection"
    verbs:
      # =========================================================================
      # SCOPE SELECTION
      # =========================================================================
      
      universe:
        description: "View all CBUs with optional filters"
        behavior: plugin
        plugin:
          handler: ViewUniverseOp
        args:
          - name: client
            type: uuid
            required: false
            description: "Filter to CBUs for this client entity"
            lookup:
              table: entities
              search_key: name
              primary_key: entity_id
          - name: jurisdiction
            type: string_list
            required: false
            description: "Filter by jurisdiction(s)"
          - name: fund-type
            type: string_list
            required: false
            description: "Filter by fund type(s)"
          - name: status
            type: string_list
            required: false
            description: "Filter by status (RED, AMBER, GREEN)"
          - name: needs-attention
            type: boolean
            required: false
            description: "Filter to items needing attention"
        returns:
          type: view_state
          capture: true
          description: "Sets session view state, returns summary"
      
      book:
        description: "View all CBUs for a commercial client (book view)"
        behavior: plugin
        plugin:
          handler: ViewBookOp
        args:
          - name: client
            type: uuid
            required: true
            description: "Commercial client entity"
            lookup:
              table: entities
              search_key: name
              primary_key: entity_id
        returns:
          type: view_state
          capture: true
      
      cbu:
        description: "Focus on a single CBU"
        behavior: plugin
        plugin:
          handler: ViewCbuOp
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              search_key: name
              primary_key: cbu_id
          - name: mode
            type: string
            required: false
            default: "trading"
            valid_values:
              - trading   # Trading network view
              - ubo       # UBO ownership view
        returns:
          type: view_state
          capture: true
      
      # =========================================================================
      # REFINEMENT
      # =========================================================================
      
      refine:
        description: "Refine current view with additional filter"
        behavior: plugin
        plugin:
          handler: ViewRefineOp
        args:
          - name: include
            type: object
            required: false
            description: "Filter to include (narrows selection)"
          - name: exclude
            type: object
            required: false
            description: "Filter to exclude (removes from selection)"
          - name: add
            type: uuid_list
            required: false
            description: "Specific IDs to add"
          - name: remove
            type: uuid_list
            required: false
            description: "Specific IDs to remove"
        returns:
          type: view_state
          capture: true
      
      clear:
        description: "Clear refinements, return to base view"
        behavior: plugin
        plugin:
          handler: ViewClearOp
        args: []
        returns:
          type: view_state
          capture: true
      
      # =========================================================================
      # ZOOM NAVIGATION (Fractal)
      # =========================================================================
      
      zoom-in:
        description: "Zoom into a node, pushing its taxonomy onto stack"
        behavior: plugin
        plugin:
          handler: ViewZoomInOp
        args:
          - name: node-id
            type: uuid
            required: true
            description: "Node to zoom into"
        returns:
          type: view_state
          capture: true
      
      zoom-out:
        description: "Zoom out one level, popping taxonomy stack"
        behavior: plugin
        plugin:
          handler: ViewZoomOutOp
        args: []
        returns:
          type: view_state
          capture: true
      
      back-to:
        description: "Zoom out to specific depth level"
        behavior: plugin
        plugin:
          handler: ViewBackToOp
        args:
          - name: depth
            type: integer
            required: false
            default: 0
            description: "Target depth (0 = universe)"
          - name: label
            type: string
            required: false
            description: "Or specify by breadcrumb label"
        returns:
          type: view_state
          capture: true
      
      # =========================================================================
      # LAYOUT
      # =========================================================================
      
      layout:
        description: "Change layout strategy for current view"
        behavior: plugin
        plugin:
          handler: ViewLayoutOp
        args:
          - name: mode
            type: string
            required: false
            valid_values:
              - auto      # Derive from taxonomy shape
              - galaxy    # Semantic clustering
              - grid      # Rows/columns
              - tree      # Hierarchical
              - network   # Force directed
          - name: primary-axis
            type: string
            required: false
            valid_values:
              - jurisdiction
              - fund_type
              - status
              - manco
              - role_category
          - name: size-by
            type: string
            required: false
          - name: color-by
            type: string
            required: false
        returns:
          type: layout_result
          capture: true
```

### 2.2 View Plugin Handlers

**File**: `rust/src/dsl_v2/custom_ops/view_ops.rs` (new)

```rust
//! View operations - manage session view state

use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::session::{UnifiedSessionContext, ViewState};
use crate::taxonomy::{MembershipRules, TaxonomyBuilder, TaxonomyContext};

/// view.universe handler
pub struct ViewUniverseOp;

impl ViewUniverseOp {
    pub async fn execute(
        args: &HashMap<String, serde_json::Value>,
        session: &mut UnifiedSessionContext,
        pool: &PgPool,
    ) -> Result<ViewOpResult> {
        // Build context from args
        let ctx = if let Some(client_id) = get_uuid_arg(args, "client") {
            TaxonomyContext::Book { client_id }
        } else {
            TaxonomyContext::Universe
        };
        
        // Set view context (rebuilds taxonomy)
        session.set_view_context(ctx, pool).await?;
        
        // Apply any filters as refinements
        if let Some(jurisdictions) = get_string_list_arg(args, "jurisdiction") {
            session.refine_view(Refinement::Include { 
                filter: Filter::Jurisdiction(jurisdictions) 
            })?;
        }
        // ... other filters
        
        Ok(ViewOpResult::from_view_state(session.current_view().unwrap()))
    }
}

/// view.book handler
pub struct ViewBookOp;

impl ViewBookOp {
    pub async fn execute(
        args: &HashMap<String, serde_json::Value>,
        session: &mut UnifiedSessionContext,
        pool: &PgPool,
    ) -> Result<ViewOpResult> {
        let client_id = get_uuid_arg(args, "client")
            .ok_or_else(|| anyhow!("client is required"))?;
        
        let ctx = TaxonomyContext::Book { client_id };
        session.set_view_context(ctx, pool).await?;
        
        Ok(ViewOpResult::from_view_state(session.current_view().unwrap()))
    }
}

/// view.cbu handler
pub struct ViewCbuOp;

impl ViewCbuOp {
    pub async fn execute(
        args: &HashMap<String, serde_json::Value>,
        session: &mut UnifiedSessionContext,
        pool: &PgPool,
    ) -> Result<ViewOpResult> {
        let cbu_id = get_uuid_arg(args, "cbu-id")
            .ok_or_else(|| anyhow!("cbu-id is required"))?;
        let mode = get_string_arg(args, "mode").unwrap_or("trading".into());
        
        let ctx = match mode.as_str() {
            "ubo" => TaxonomyContext::CbuUbo { cbu_id },
            _ => TaxonomyContext::CbuTrading { cbu_id },
        };
        
        session.set_view_context(ctx, pool).await?;
        
        Ok(ViewOpResult::from_view_state(session.current_view().unwrap()))
    }
}

/// view.refine handler
pub struct ViewRefineOp;

impl ViewRefineOp {
    pub async fn execute(
        args: &HashMap<String, serde_json::Value>,
        session: &mut UnifiedSessionContext,
        _pool: &PgPool,
    ) -> Result<ViewOpResult> {
        // Apply refinements
        if let Some(include) = args.get("include") {
            let filter = parse_filter(include)?;
            session.refine_view(Refinement::Include { filter })?;
        }
        if let Some(exclude) = args.get("exclude") {
            let filter = parse_filter(exclude)?;
            session.refine_view(Refinement::Exclude { filter })?;
        }
        if let Some(add_ids) = get_uuid_list_arg(args, "add") {
            session.refine_view(Refinement::Add { ids: add_ids })?;
        }
        if let Some(remove_ids) = get_uuid_list_arg(args, "remove") {
            session.refine_view(Refinement::Remove { ids: remove_ids })?;
        }
        
        Ok(ViewOpResult::from_view_state(session.current_view().unwrap()))
    }
}

/// view.zoom-in handler
pub struct ViewZoomInOp;

impl ViewZoomInOp {
    pub async fn execute(
        args: &HashMap<String, serde_json::Value>,
        session: &mut UnifiedSessionContext,
        _pool: &PgPool,
    ) -> Result<ViewOpResult> {
        let node_id = get_uuid_arg(args, "node-id")
            .ok_or_else(|| anyhow!("node-id is required"))?;
        
        session.zoom_in(node_id)?;
        
        Ok(ViewOpResult::from_view_state(session.current_view().unwrap()))
    }
}

/// view.zoom-out handler
pub struct ViewZoomOutOp;

impl ViewZoomOutOp {
    pub async fn execute(
        session: &mut UnifiedSessionContext,
        _pool: &PgPool,
    ) -> Result<ViewOpResult> {
        session.zoom_out()?;
        Ok(ViewOpResult::from_view_state(session.current_view().unwrap()))
    }
}

/// view.back-to handler
pub struct ViewBackToOp;

impl ViewBackToOp {
    pub async fn execute(
        args: &HashMap<String, serde_json::Value>,
        session: &mut UnifiedSessionContext,
        _pool: &PgPool,
    ) -> Result<ViewOpResult> {
        if let Some(depth) = get_int_arg(args, "depth") {
            // Zoom out to specific depth
            while session.view.as_ref().map(|v| v.depth()).unwrap_or(0) > depth {
                session.zoom_out()?;
            }
        } else if let Some(label) = get_string_arg(args, "label") {
            // Zoom out until we find matching breadcrumb
            while let Some(view) = &session.view {
                let crumbs = view.breadcrumbs();
                if crumbs.last().map(|b| &b.label) == Some(&label) {
                    break;
                }
                if crumbs.len() <= 1 {
                    return Err(anyhow!("Label not found in breadcrumbs: {}", label));
                }
                session.zoom_out()?;
            }
        } else {
            // Default: back to universe (depth 0)
            while session.view.as_ref().map(|v| v.depth()).unwrap_or(0) > 0 {
                session.zoom_out()?;
            }
        }
        
        Ok(ViewOpResult::from_view_state(session.current_view().unwrap()))
    }
}

/// Result type for view operations
#[derive(Debug, Clone, Serialize)]
pub struct ViewOpResult {
    pub depth: usize,
    pub breadcrumbs: Vec<String>,
    pub total_count: usize,
    pub selection_count: usize,
    pub refinement_count: usize,
    pub has_pending: bool,
    pub metaphor: String,
    pub can_zoom_out: bool,
}

impl ViewOpResult {
    pub fn from_view_state(view: &ViewState) -> Self {
        let taxonomy = view.current_taxonomy();
        Self {
            depth: view.depth(),
            breadcrumbs: view.breadcrumbs().iter().map(|b| b.label.clone()).collect(),
            total_count: taxonomy.map(|t| t.descendant_count).unwrap_or(0),
            selection_count: view.selection.len(),
            refinement_count: view.refinements.len(),
            has_pending: view.pending.is_some(),
            metaphor: taxonomy.map(|t| format!("{:?}", t.metaphor())).unwrap_or_default(),
            can_zoom_out: view.depth() > 0,
        }
    }
}
```

---

## Phase 3: Batch Verbs Enhancement

### 3.1 Extend batch.yaml

**File**: `rust/config/verbs/batch.yaml` (extend existing)

Add these verbs:

```yaml
      # =========================================================================
      # SELECTION-BASED OPERATIONS
      # =========================================================================
      
      subscribe:
        description: "Add product subscription to current selection"
        behavior: plugin
        plugin:
          handler: BatchSubscribeOp
        args:
          - name: product
            type: string
            required: true
            description: "Product code to subscribe"
            lookup:
              table: products
              search_key: product_code
              primary_key: product_code
          - name: targets
            type: uuid_list
            required: false
            description: "Explicit targets (defaults to current selection)"
        returns:
          type: pending_operation
          description: "Stages operation, returns preview"
      
      unsubscribe:
        description: "Remove product subscription from current selection"
        behavior: plugin
        plugin:
          handler: BatchUnsubscribeOp
        args:
          - name: product
            type: string
            required: true
          - name: targets
            type: uuid_list
            required: false
        returns:
          type: pending_operation
      
      assign-role:
        description: "Assign entity to role across current selection"
        behavior: plugin
        plugin:
          handler: BatchAssignRoleOp
        args:
          - name: entity-id
            type: uuid
            required: true
            lookup:
              table: entities
              search_key: name
              primary_key: entity_id
          - name: role
            type: string
            required: true
            lookup:
              table: roles
              search_key: name
              primary_key: name
          - name: targets
            type: uuid_list
            required: false
        returns:
          type: pending_operation
      
      preview:
        description: "Show preview of pending operation"
        behavior: plugin
        plugin:
          handler: BatchPreviewOp
        args: []
        returns:
          type: operation_preview
      
      confirm:
        description: "Execute pending operation"
        behavior: plugin
        plugin:
          handler: BatchConfirmOp
        args: []
        returns:
          type: execution_result
      
      cancel:
        description: "Cancel pending operation"
        behavior: plugin
        plugin:
          handler: BatchCancelOp
        args: []
        returns:
          type: affected
```

### 3.2 Batch Plugin Handlers

**File**: `rust/src/dsl_v2/custom_ops/batch_selection_ops.rs` (new)

```rust
//! Batch operations on current session selection

use anyhow::Result;
use sqlx::PgPool;

use crate::session::{BatchOperation, PendingOperation, UnifiedSessionContext};

/// batch.subscribe handler
pub struct BatchSubscribeOp;

impl BatchSubscribeOp {
    pub async fn execute(
        args: &HashMap<String, serde_json::Value>,
        session: &mut UnifiedSessionContext,
        pool: &PgPool,
    ) -> Result<PendingOpResult> {
        let product = get_string_arg(args, "product")
            .ok_or_else(|| anyhow!("product is required"))?;
        
        // Get targets: explicit or from selection
        let targets = if let Some(explicit) = get_uuid_list_arg(args, "targets") {
            explicit
        } else if let Some(view) = &session.view {
            view.selection.clone()
        } else {
            return Err(anyhow!("No selection and no explicit targets"));
        };
        
        if targets.is_empty() {
            return Err(anyhow!("No targets for operation"));
        }
        
        // Stage the operation
        let operation = BatchOperation::Subscribe { product };
        session.stage_operation(operation)?;
        
        Ok(PendingOpResult::from_pending(session.view.as_ref().unwrap().pending.as_ref().unwrap()))
    }
}

/// batch.confirm handler
pub struct BatchConfirmOp;

impl BatchConfirmOp {
    pub async fn execute(
        session: &mut UnifiedSessionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        session.execute_pending(pool).await
    }
}

/// batch.cancel handler
pub struct BatchCancelOp;

impl BatchCancelOp {
    pub async fn execute(
        session: &mut UnifiedSessionContext,
    ) -> Result<()> {
        if let Some(view) = &mut session.view {
            view.clear_pending();
            Ok(())
        } else {
            Err(anyhow!("No active view"))
        }
    }
}
```

---

## Phase 4: Agent Context Integration

### 4.1 Extend AgentContext

**File**: `rust/src/session/agent_context.rs` (extend)

```rust
impl AgentGraphContext {
    pub fn from_session(session: &UnifiedSessionContext) -> Self {
        let mut ctx = Self {
            // ... existing fields ...
            
            // NEW: View state summary
            view_summary: session.view.as_ref().map(|v| ViewSummary {
                context: format!("{:?}", v.context),
                total_count: v.taxonomy.descendant_count,
                selection_count: v.selection.len(),
                refinements: v.refinements.iter()
                    .map(|r| format!("{:?}", r))
                    .collect(),
                pending: v.pending.as_ref().map(|p| PendingSummary {
                    operation: format!("{:?}", p.operation),
                    target_count: p.targets.len(),
                    preview: p.preview.summary.clone(),
                }),
                metaphor: format!("{:?}", v.taxonomy.metaphor()),
            }),
        };
        
        ctx
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewSummary {
    pub context: String,
    pub total_count: usize,
    pub selection_count: usize,
    pub refinements: Vec<String>,
    pub pending: Option<PendingSummary>,
    pub metaphor: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingSummary {
    pub operation: String,
    pub target_count: usize,
    pub preview: String,
}
```

### 4.2 Agent Prompt Template

**File**: `rust/src/session/prompts/view_context.md` (new)

```markdown
## Current View State

{{#if view_summary}}
**Scope**: {{view_summary.context}}
**Total items**: {{view_summary.total_count}}
**Selected**: {{view_summary.selection_count}}
**Visualization**: {{view_summary.metaphor}}

{{#if view_summary.refinements}}
**Active filters**:
{{#each view_summary.refinements}}
- {{this}}
{{/each}}
{{/if}}

{{#if view_summary.pending}}
**Pending operation**: {{view_summary.pending.operation}}
- Targets: {{view_summary.pending.target_count}} items
- Preview: {{view_summary.pending.preview}}

User can say "confirm", "do it", or "cancel" to proceed or cancel.
{{/if}}
{{else}}
No active view. User can say "show me..." to set a view scope.
{{/if}}
```

---

## Phase 5: Visualization Integration

### 5.1 Update Galaxy View

**File**: `rust/crates/ob-poc-graph/src/graph/galaxy.rs` (extend)

```rust
impl GalaxyView {
    /// Update from ViewState (called when session changes)
    pub fn update_from_view_state(&mut self, view: &ViewState) {
        // Convert taxonomy to cluster data
        let clusters = self.taxonomy_to_clusters(&view.taxonomy);
        self.set_clusters(clusters);
        
        // Apply selection highlighting
        self.set_selection(&view.selection);
        
        // Show pending operation overlay if present
        if let Some(pending) = &view.pending {
            self.show_pending_overlay(&pending.preview);
        } else {
            self.hide_pending_overlay();
        }
    }
    
    fn taxonomy_to_clusters(&self, taxonomy: &TaxonomyNode) -> Vec<ClusterData> {
        // Convert based on metaphor
        match taxonomy.metaphor() {
            Metaphor::Galaxy => self.build_galaxy_clusters(taxonomy),
            Metaphor::Constellation => self.build_constellation_clusters(taxonomy),
            _ => self.build_simple_clusters(taxonomy),
        }
    }
}
```

### 5.2 Update App State

**File**: `rust/crates/ob-poc-ui/src/state.rs` (extend)

```rust
impl AsyncState {
    /// Called when session view state changes
    pub fn on_view_state_changed(&mut self, view: &ViewState) {
        // Update galaxy view
        self.galaxy_view.update_from_view_state(view);
        
        // Update layout mode if needed
        if let Some(layout) = &view.layout {
            self.current_layout_mode = layout.mode;
        }
        
        // Request repaint
        self.needs_repaint = true;
    }
}
```

---

## Phase 6: Voice Integration

### 6.1 Extend Voice Command Handling

**File**: `rust/crates/ob-poc-ui/src/command.rs` (extend)

```rust
/// Map voice command to DSL
pub fn voice_to_dsl(cmd: &VoiceCommand, session: &UnifiedSessionContext) -> Option<String> {
    match cmd.command.as_str() {
        // View commands
        "show_universe" => Some("(view.universe)".into()),
        "show_book" => {
            let client = cmd.params.get("client")?;
            Some(format!("(view.book :client \"{}\")", client))
        },
        "filter_jurisdiction" => {
            let jurisdiction = cmd.params.get("jurisdiction")?;
            Some(format!("(view.refine :include {{:jurisdiction [\"{}\"]}})", jurisdiction))
        },
        "filter_status" => {
            let status = cmd.params.get("status")?;
            Some(format!("(view.refine :include {{:status [\"{}\"]}})", status))
        },
        "except" => {
            // Parse natural language exception
            let filter = parse_exception_filter(&cmd.transcript)?;
            Some(format!("(view.refine :exclude {})", filter))
        },
        
        // Operation commands
        "add_product" => {
            let product = cmd.params.get("product")?;
            Some(format!("(batch.subscribe :product \"{}\")", product))
        },
        "confirm" | "do_it" => Some("(batch.confirm)".into()),
        "cancel" => Some("(batch.cancel)".into()),
        
        // Navigation
        "open" | "focus" => {
            // Use current cursor or hovered item
            if let Some(view) = &session.view {
                if let Some(focused) = &view.layout.as_ref()?.focused_id {
                    Some(format!("(view.cbu :cbu-id \"{}\")", focused))
                } else {
                    None
                }
            } else {
                None
            }
        },
        "back" => {
            // Return to previous view context
            Some("(view.back)".into())
        },
        
        _ => None,
    }
}
```

---

## Phase 7: Tests

### 7.1 Taxonomy Builder Tests

**File**: `rust/src/taxonomy/tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metaphor_derivation() {
        // Large flat -> Galaxy
        let large = TaxonomyNode::mock_with_descendants(1500, 2);
        assert_eq!(large.metaphor(), Metaphor::Galaxy);
        
        // Deep narrow -> Pyramid
        let deep = TaxonomyNode::mock_with_depth(8, 2);
        assert_eq!(deep.metaphor(), Metaphor::Pyramid);
        
        // Wide shallow -> Network
        let wide = TaxonomyNode::mock_with_width(15, 2);
        assert_eq!(wide.metaphor(), Metaphor::Network);
    }
    
    #[test]
    fn test_astro_level() {
        let root = TaxonomyNode::mock_with_descendants(2000, 3);
        assert_eq!(root.astro_level(), AstroLevel::Universe);
        
        let child = &root.children[0];
        assert_eq!(child.astro_level(), AstroLevel::Galaxy);
    }
    
    #[tokio::test]
    async fn test_builder_universe() {
        let pool = test_pool().await;
        let rules = MembershipRules::from_context(&TaxonomyContext::Universe);
        let taxonomy = TaxonomyBuilder::new(rules).build(&pool).await.unwrap();
        
        assert!(taxonomy.descendant_count > 0);
    }
    
    #[tokio::test]
    async fn test_builder_book() {
        let pool = test_pool().await;
        let client_id = seed_client(&pool).await;
        
        let rules = MembershipRules::from_context(&TaxonomyContext::Book { client_id });
        let taxonomy = TaxonomyBuilder::new(rules).build(&pool).await.unwrap();
        
        // All nodes should be CBUs for this client
        for id in taxonomy.all_ids() {
            // verify belongs to client
        }
    }
}
```

### 7.2 View State Tests

**File**: `rust/src/session/view_state_tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_refinement_include() {
        let mut view = ViewState::mock_with_nodes(100);
        assert_eq!(view.selection.len(), 100);
        
        view.refine(Refinement::Include { 
            filter: Filter::Jurisdiction(vec!["LU".into()]) 
        });
        
        // Selection should be smaller
        assert!(view.selection.len() < 100);
    }
    
    #[test]
    fn test_refinement_exclude() {
        let mut view = ViewState::mock_with_nodes(100);
        let original_len = view.selection.len();
        
        view.refine(Refinement::Exclude { 
            filter: Filter::Status(vec!["GREEN".into()]) 
        });
        
        // Selection should be smaller
        assert!(view.selection.len() < original_len);
    }
    
    #[test]
    fn test_stage_operation() {
        let mut view = ViewState::mock_with_nodes(10);
        
        view.stage_operation(BatchOperation::Subscribe { 
            product: "CUSTODY".into() 
        }).unwrap();
        
        assert!(view.has_pending());
        assert_eq!(view.pending.as_ref().unwrap().targets.len(), 10);
    }
}
```

### 7.3 DSL Integration Tests

**File**: `rust/tests/scenarios/valid/20_view_selection.dsl`

```dsl
; Test view selection and batch operations

; 1. Set universe view
(view.universe)

; 2. Filter to Luxembourg
(view.refine :include {:jurisdiction ["LU"]})

; 3. Further filter to equity funds
(view.refine :include {:fund-type ["EQUITY"]})

; 4. Stage custody subscription
(batch.subscribe :product "CUSTODY")

; 5. Preview (implicit - captured in session)

; 6. Exclude small funds
(view.refine :exclude {:aum-below 100000000})

; 7. Confirm operation
(batch.confirm)
```

---

## Execution Order

1. **Phase 1.1-1.2**: TaxonomyNode + types (foundation)
2. **Phase 1.3**: MembershipRules + TaxonomyBuilder
3. **Phase 1.4**: ViewState struct
4. **Phase 1.5**: Extend UnifiedSessionContext
5. **Phase 1.6**: Extend ExecutionContext
6. **Phase 2**: view.yaml + view plugin handlers
7. **Phase 3**: batch.yaml extensions + handlers
8. **Phase 4**: Agent context integration
9. **Phase 5**: Visualization integration
10. **Phase 6**: Voice integration
11. **Phase 7**: Tests

---

## Validation Checklist

- [ ] `cargo build` succeeds
- [ ] `TaxonomyNode` correctly derives `metaphor()` and `astro_level()`
- [ ] `TaxonomyBuilder` produces valid trees for all context types
- [ ] `ViewState` correctly applies refinements
- [ ] `view.universe` sets session view state
- [ ] `view.refine` narrows selection
- [ ] `batch.subscribe` stages operation using selection
- [ ] `batch.confirm` executes pending verbs
- [ ] Agent context includes view summary
- [ ] Galaxy view renders from ViewState
- [ ] Voice commands map to view/batch DSL
- [ ] Existing tests still pass
- [ ] **Fractal navigation: zoom in/out works**
- [ ] **Breadcrumbs: correct path at each depth**
- [ ] **view.zoom-in pushes taxonomy onto stack**
- [ ] **view.zoom-out pops taxonomy from stack**
- [ ] **view.back-to navigates to specific depth**

---

## Key Files Summary

| File | Purpose |
|------|---------|
| `rust/src/taxonomy/mod.rs` | New module |
| `rust/src/taxonomy/node.rs` | TaxonomyNode struct + `expansion: Option<ExpansionRule>` |
| `rust/src/taxonomy/types.rs` | AstroLevel, Metaphor, DimensionValues |
| `rust/src/taxonomy/rules.rs` | MembershipRules, TaxonomyContext |
| `rust/src/taxonomy/builder.rs` | TaxonomyBuilder |
| `rust/src/taxonomy/stack.rs` | **TaxonomyStack for fractal navigation** |
| `rust/src/session/view_state.rs` | ViewState with TaxonomyStack, Refinement, PendingOperation |
| `rust/src/session/mod.rs` | Extend UnifiedSessionContext with zoom_in/zoom_out |
| `rust/src/dsl_v2/executor.rs` | Extend ExecutionContext |
| `rust/config/verbs/view.yaml` | New view verbs including zoom-in, zoom-out, back-to |
| `rust/src/dsl_v2/custom_ops/view_ops.rs` | View plugin handlers including zoom handlers |
| `rust/src/dsl_v2/custom_ops/batch_selection_ops.rs` | Batch selection handlers |
| `rust/src/session/agent_context.rs` | Extend with ViewSummary + breadcrumbs |
| `rust/crates/ob-poc-graph/src/graph/galaxy.rs` | Extend for ViewState |
| `rust/crates/ob-poc-ui/src/command.rs` | Voice to DSL mapping |

---

## Relationship to TODO-NOM-TAXONOMY-COMBINATORS.md

This TODO defines WHAT the session/view architecture looks like.
The NOM-TAXONOMY-COMBINATORS TODO defines HOW taxonomies are built using combinators.

They work together:
- **This TODO**: `TaxonomyStack`, `ViewState`, view verbs, session integration
- **Combinators TODO**: `TaxonomyParser` trait, `.each_is_taxonomy()`, `ExpansionRule`

The stack uses expansion rules defined by combinators to know how to zoom into nodes.
