# TODO: Unified Session Context + Agent RAG Discovery

**Priority**: HIGH  
**Type**: EXTENSION (builds on completed EntityGraph refactoring)  
**Estimated Effort**: 4-5 days

---

## Context: What's Already Implemented

Claude has completed the EntityGraph refactoring. The following is now in place:

| Component | Location | Status |
|-----------|----------|--------|
| `EntityGraph` struct | `rust/src/graph/types.rs` | ✅ Complete - nodes, edges, history, filters, scope, layout |
| `GraphFilters` + visibility | `rust/src/graph/filters.rs` | ✅ Complete - temporal, jurisdiction, prong filtering |
| `NavCommand` enum | `rust/src/navigation/commands.rs` | ✅ Complete - all command types defined |
| Nom parser | `rust/src/navigation/parser.rs` | ✅ Complete - natural language parsing |
| `NavExecutor` trait | `rust/src/navigation/executor.rs` | ⚠️ Partial - in-memory works, DB loading stubbed |
| `GraphRepository` | `rust/src/database/graph_repository.rs` | ✅ Complete - all scope loading methods |
| `AgentContextBuilder` | `rust/crates/ob-agentic/src/context_builder.rs` | ⚠️ Basic - DSL bindings only, no navigation |

**This TODO extends the existing implementation, not replaces it.**

---

## Goal: Two Missing Capabilities

### 1. Unified Session Context
Single service handling:
- DSL REPL execution state (existing `ExecutionContext`)
- Graph navigation state (cursor, history, filters)
- Viewport state (zoom, pan, visible/off-screen)
- Scope management (with windowing for large datasets)

### 2. Agent RAG Discovery
Enable agent to dynamically discover:
- What verbs/commands exist
- When to use each command
- What parameters are needed
- State-aware suggestions

---

## Phase 1: ViewportContext (New)

### Task 1.1: Create ViewportContext struct

**File**: `rust/src/graph/viewport.rs` (NEW)

```rust
use std::collections::HashSet;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

/// Viewport state for graph visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportContext {
    /// Zoom level (0.0 = overview showing all, 1.0 = detail)
    pub zoom: f32,
    
    /// Zoom level name for agent context
    pub zoom_name: ZoomName,
    
    /// Pan offset from center (pixels)
    pub pan_offset: (f32, f32),
    
    /// Canvas dimensions
    pub canvas_size: (f32, f32),
    
    /// Entity IDs currently visible in viewport
    pub visible_entities: HashSet<Uuid>,
    
    /// Summary of what's off-screen by direction
    pub off_screen: OffScreenSummary,
    
    /// Whether viewport has been explicitly set
    pub is_default: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ZoomName {
    Overview,   // 0.0 - 0.3: See entire structure
    Standard,   // 0.3 - 0.7: Normal working view
    Detail,     // 0.7 - 1.0: Close-up with all labels
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OffScreenSummary {
    pub above: usize,      // Entities above viewport (owners/parents)
    pub below: usize,      // Entities below viewport (owned/children)
    pub left: usize,       // Entities to the left (siblings)
    pub right: usize,      // Entities to the right (siblings)
    
    /// Hint about what's off-screen for agent context
    pub above_hint: Option<String>,  // e.g., "3 owner entities including Allianz SE"
    pub below_hint: Option<String>,  // e.g., "47 subfunds"
}

impl ViewportContext {
    pub fn new(canvas_width: f32, canvas_height: f32) -> Self {
        Self {
            zoom: 0.5,
            zoom_name: ZoomName::Standard,
            pan_offset: (0.0, 0.0),
            canvas_size: (canvas_width, canvas_height),
            visible_entities: HashSet::new(),
            off_screen: OffScreenSummary::default(),
            is_default: true,
        }
    }
    
    /// Compute what's visible given current zoom/pan and node positions
    pub fn compute_visibility(&mut self, graph: &EntityGraph) {
        self.visible_entities.clear();
        self.off_screen = OffScreenSummary::default();
        
        let (vp_left, vp_top, vp_right, vp_bottom) = self.viewport_bounds();
        
        for (id, node) in &graph.nodes {
            if let (Some(x), Some(y)) = (node.x, node.y) {
                if x >= vp_left && x <= vp_right && y >= vp_top && y <= vp_bottom {
                    self.visible_entities.insert(*id);
                } else {
                    // Track off-screen direction
                    if y < vp_top { self.off_screen.above += 1; }
                    if y > vp_bottom { self.off_screen.below += 1; }
                    if x < vp_left { self.off_screen.left += 1; }
                    if x > vp_right { self.off_screen.right += 1; }
                }
            }
        }
        
        self.update_zoom_name();
    }
    
    fn viewport_bounds(&self) -> (f32, f32, f32, f32) {
        let half_w = self.canvas_size.0 / 2.0 / self.zoom;
        let half_h = self.canvas_size.1 / 2.0 / self.zoom;
        let center_x = self.canvas_size.0 / 2.0 + self.pan_offset.0;
        let center_y = self.canvas_size.1 / 2.0 + self.pan_offset.1;
        (center_x - half_w, center_y - half_h, center_x + half_w, center_y + half_h)
    }
    
    fn update_zoom_name(&mut self) {
        self.zoom_name = match self.zoom {
            z if z < 0.3 => ZoomName::Overview,
            z if z > 0.7 => ZoomName::Detail,
            _ => ZoomName::Standard,
        };
    }
    
    // Viewport commands
    pub fn pan(&mut self, direction: PanDirection, amount: f32) {
        match direction {
            PanDirection::Up => self.pan_offset.1 -= amount,
            PanDirection::Down => self.pan_offset.1 += amount,
            PanDirection::Left => self.pan_offset.0 -= amount,
            PanDirection::Right => self.pan_offset.0 += amount,
        }
        self.is_default = false;
    }
    
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).min(2.0);
        self.update_zoom_name();
        self.is_default = false;
    }
    
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).max(0.1);
        self.update_zoom_name();
        self.is_default = false;
    }
    
    pub fn fit_all(&mut self) {
        self.zoom = 0.25;
        self.pan_offset = (0.0, 0.0);
        self.update_zoom_name();
        self.is_default = false;
    }
    
    pub fn center_on(&mut self, x: f32, y: f32) {
        let center_x = self.canvas_size.0 / 2.0;
        let center_y = self.canvas_size.1 / 2.0;
        self.pan_offset = (center_x - x, center_y - y);
        self.is_default = false;
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PanDirection {
    Up,
    Down,
    Left,
    Right,
}
```

### Task 1.2: Integrate viewport into executor

**File**: `rust/src/navigation/executor.rs`

Add viewport command handling to the `NavExecutor` impl:

```rust
NavCommand::ZoomIn => {
    // Need viewport context passed in or stored on graph
    NavResult::ZoomChanged(ZoomLevel::Custom(1.25))
}

NavCommand::ZoomOut => {
    NavResult::ZoomChanged(ZoomLevel::Custom(0.8))
}

NavCommand::FitToView => {
    NavResult::ZoomChanged(ZoomLevel::Overview)
}
```

**Note**: This requires either:
1. Adding `ViewportContext` to `EntityGraph`, or
2. Passing viewport as separate parameter to executor

**Recommendation**: Keep viewport separate from EntityGraph (viewport is UI state, graph is data).

---

## Phase 2: UnifiedSessionContext

### Task 2.1: Create unified session struct

**File**: `rust/src/session/mod.rs` (NEW directory)

```rust
//! Unified Session Context
//!
//! Single service handling REPL execution, graph navigation, and viewport.
//! Supports multiple scope sizes with windowing for large datasets.

use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::graph::{EntityGraph, GraphScope, GraphFilters};
use crate::navigation::NavCommand;
use ob_execution_types::ExecutionContext;

pub mod scope;
pub mod agent_context;

/// Unified session context - handles REPL + Visualization + Navigation
pub struct UnifiedSessionContext {
    /// Session identity
    pub session_id: Uuid,
    pub user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    
    /// DSL Execution state (existing - from ob-execution-types)
    pub execution: ExecutionContext,
    
    /// Graph data (from completed EntityGraph implementation)
    pub graph: Option<EntityGraph>,
    
    /// Viewport state (new)
    pub viewport: ViewportContext,
    
    /// Scope definition and stats
    pub scope: SessionScope,
    
    /// Command history for undo/replay
    pub command_history: Vec<ExecutedCommand>,
    
    /// Named bookmarks
    pub bookmarks: HashMap<String, Bookmark>,
}

/// Executed command with timestamp for history
#[derive(Debug, Clone)]
pub struct ExecutedCommand {
    pub command: NavCommand,
    pub executed_at: DateTime<Utc>,
    pub result_summary: String,
}

/// Named position bookmark
#[derive(Debug, Clone)]
pub struct Bookmark {
    pub name: String,
    pub cursor: Option<Uuid>,
    pub filters: GraphFilters,
    pub zoom: f32,
    pub pan_offset: (f32, f32),
}

impl UnifiedSessionContext {
    pub fn new() -> Self {
        Self {
            session_id: Uuid::new_v4(),
            user_id: None,
            created_at: Utc::now(),
            execution: ExecutionContext::new(),
            graph: None,
            viewport: ViewportContext::new(1200.0, 800.0),
            scope: SessionScope::empty(),
            command_history: Vec::new(),
            bookmarks: HashMap::new(),
        }
    }
    
    /// Execute a navigation command
    pub fn execute_nav(&mut self, cmd: NavCommand) -> NavResult {
        // Record in history
        let result = if let Some(graph) = &mut self.graph {
            graph.execute_nav(cmd.clone())
        } else {
            NavResult::Error { 
                message: "No graph loaded. Use load_cbu, load_book, or load_jurisdiction first.".into() 
            }
        };
        
        // Update viewport visibility after navigation
        if let Some(graph) = &self.graph {
            self.viewport.compute_visibility(graph);
        }
        
        self.command_history.push(ExecutedCommand {
            command: cmd,
            executed_at: Utc::now(),
            result_summary: format!("{:?}", result),
        });
        
        result
    }
    
    /// Load a scope (delegates to GraphRepository)
    pub async fn load_scope<R: GraphRepository>(
        &mut self,
        scope: GraphScope,
        repo: &R,
    ) -> Result<(), anyhow::Error> {
        let graph = EntityGraph::load(scope.clone(), repo).await?;
        
        // Update scope stats
        self.scope = SessionScope::from_graph(&graph, scope);
        
        // Store graph
        self.graph = Some(graph);
        
        // Reset viewport for new scope
        self.viewport = ViewportContext::new(1200.0, 800.0);
        if let Some(g) = &self.graph {
            self.viewport.compute_visibility(g);
        }
        
        Ok(())
    }
}
```

### Task 2.2: Create session scope with windowing

**File**: `rust/src/session/scope.rs`

```rust
//! Session Scope Management
//!
//! Handles scope definitions and windowing for large datasets.

use uuid::Uuid;
use std::collections::HashMap;
use crate::graph::{EntityGraph, GraphScope};

/// Session scope with stats and windowing info
#[derive(Debug, Clone)]
pub struct SessionScope {
    /// How scope was defined
    pub definition: GraphScope,
    
    /// Summary statistics
    pub stats: ScopeSummary,
    
    /// Whether full data is loaded or windowed
    pub load_status: LoadStatus,
}

#[derive(Debug, Clone, Default)]
pub struct ScopeSummary {
    pub total_entities: usize,
    pub total_cbus: usize,
    pub total_edges: usize,
    pub by_jurisdiction: HashMap<String, usize>,
    pub by_entity_type: HashMap<String, usize>,
    pub terminus_count: usize,
}

#[derive(Debug, Clone)]
pub enum LoadStatus {
    /// All data loaded in memory
    Full,
    
    /// Only summary loaded, expand on demand
    SummaryOnly {
        expandable_nodes: Vec<ExpandableNode>,
    },
    
    /// Windowed around a focal point
    Windowed {
        center_entity_id: Uuid,
        loaded_hops: u32,
        total_reachable: usize,
    },
}

#[derive(Debug, Clone)]
pub struct ExpandableNode {
    pub entity_id: Uuid,
    pub name: String,
    pub collapsed_child_count: usize,
    pub child_type_hint: String,  // e.g., "47 subfunds"
}

impl SessionScope {
    pub fn empty() -> Self {
        Self {
            definition: GraphScope::Empty,
            stats: ScopeSummary::default(),
            load_status: LoadStatus::Full,
        }
    }
    
    pub fn from_graph(graph: &EntityGraph, definition: GraphScope) -> Self {
        Self {
            definition,
            stats: ScopeSummary {
                total_entities: graph.nodes.len(),
                total_cbus: graph.cbus.len(),
                total_edges: graph.ownership_edges.len() 
                    + graph.control_edges.len()
                    + graph.fund_edges.len(),
                by_jurisdiction: Self::count_by_jurisdiction(graph),
                by_entity_type: Self::count_by_type(graph),
                terminus_count: graph.termini.len(),
            },
            load_status: LoadStatus::Full,
        }
    }
    
    fn count_by_jurisdiction(graph: &EntityGraph) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for node in graph.nodes.values() {
            if let Some(j) = &node.jurisdiction {
                *counts.entry(j.clone()).or_insert(0) += 1;
            }
        }
        counts
    }
    
    fn count_by_type(graph: &EntityGraph) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for node in graph.nodes.values() {
            let type_str = format!("{:?}", node.entity_type);
            *counts.entry(type_str).or_insert(0) += 1;
        }
        counts
    }
}
```

---

## Phase 3: Agent Graph Context

### Task 3.1: Create agent-specific context struct

**File**: `rust/src/session/agent_context.rs`

```rust
//! Agent Context for Graph Navigation
//!
//! Provides structured context for LLM agents to understand:
//! - Current position and state
//! - What's visible and what's off-screen
//! - What commands make sense now

use uuid::Uuid;
use serde::Serialize;
use crate::graph::{EntityGraph, GraphNode, GraphScope, ProngFilter};
use crate::session::{UnifiedSessionContext, SessionScope, ViewportContext};

/// Context injected into agent prompts for graph navigation
#[derive(Debug, Clone, Serialize)]
pub struct AgentGraphContext {
    /// Current scope summary
    pub scope: ScopeSummaryForAgent,
    
    /// Current cursor position (if set)
    pub cursor: Option<CursorContext>,
    
    /// What's around the cursor
    pub neighborhood: Option<NeighborhoodContext>,
    
    /// Active filters
    pub filters: FilterContext,
    
    /// Viewport state
    pub viewport: ViewportForAgent,
    
    /// Commands that make sense given current state
    pub suggested_commands: Vec<SuggestedCommand>,
    
    /// DSL bindings available (from ExecutionContext)
    pub bindings: Vec<BindingSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScopeSummaryForAgent {
    pub scope_type: String,           // "SingleCbu", "Book", "Jurisdiction"
    pub scope_name: String,           // "Alpha Fund", "Allianz SE", "LU"
    pub total_entities: usize,
    pub total_cbus: usize,
    pub jurisdictions: Vec<String>,   // Top jurisdictions in scope
}

#[derive(Debug, Clone, Serialize)]
pub struct CursorContext {
    pub entity_id: String,            // UUID as string
    pub name: String,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub depth_from_terminus: u32,
    pub is_terminus: bool,
    pub is_natural_person: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NeighborhoodContext {
    /// Parent owners (can go up to these)
    pub owners: Vec<NeighborSummary>,
    /// Owned children (can go down to these)
    pub children: Vec<NeighborSummary>,
    /// Control relationships
    pub controllers: Vec<NeighborSummary>,
    pub controlled: Vec<NeighborSummary>,
    /// Hint about total if truncated
    pub children_truncated: Option<usize>,  // "and 47 more subfunds"
}

#[derive(Debug, Clone, Serialize)]
pub struct NeighborSummary {
    pub name: String,
    pub entity_type: String,
    pub hint: Option<String>,  // "100% owner", "Chairman"
}

#[derive(Debug, Clone, Serialize)]
pub struct FilterContext {
    pub prong: String,                // "both", "ownership", "control"
    pub jurisdictions: Option<Vec<String>>,
    pub as_of_date: String,
    pub min_ownership_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewportForAgent {
    pub zoom_level: String,           // "Overview", "Standard", "Detail"
    pub visible_count: usize,
    pub off_screen_above: usize,
    pub off_screen_below: usize,
    pub off_screen_left: usize,
    pub off_screen_right: usize,
    /// Natural language hint
    pub off_screen_hint: Option<String>,  // "12 entities below (subfunds)"
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestedCommand {
    pub command: String,              // "go up"
    pub description: String,          // "Navigate to Allianz GI GmbH (parent owner)"
    pub relevance: f32,               // 0.0 - 1.0
}

#[derive(Debug, Clone, Serialize)]
pub struct BindingSummary {
    pub name: String,                 // "@fund"
    pub binding_type: String,         // "cbu"
    pub display_name: Option<String>, // "Alpha Fund"
}

impl AgentGraphContext {
    /// Build context from unified session
    pub fn from_session(session: &UnifiedSessionContext) -> Self {
        let scope = Self::build_scope_summary(session);
        let cursor = Self::build_cursor_context(session);
        let neighborhood = Self::build_neighborhood(session);
        let filters = Self::build_filter_context(session);
        let viewport = Self::build_viewport_context(session);
        let suggested = Self::compute_suggestions(session);
        let bindings = Self::build_bindings(session);
        
        Self {
            scope,
            cursor,
            neighborhood,
            filters,
            viewport,
            suggested_commands: suggested,
            bindings,
        }
    }
    
    fn build_scope_summary(session: &UnifiedSessionContext) -> ScopeSummaryForAgent {
        let (scope_type, scope_name) = match &session.scope.definition {
            GraphScope::SingleCbu { cbu_name, .. } => ("SingleCbu".into(), cbu_name.clone()),
            GraphScope::Book { apex_name, .. } => ("Book".into(), apex_name.clone()),
            GraphScope::Jurisdiction { code } => ("Jurisdiction".into(), code.clone()),
            GraphScope::Empty => ("Empty".into(), "None".into()),
            _ => ("Custom".into(), "Custom scope".into()),
        };
        
        let jurisdictions: Vec<String> = session.scope.stats.by_jurisdiction
            .keys()
            .take(5)
            .cloned()
            .collect();
        
        ScopeSummaryForAgent {
            scope_type,
            scope_name,
            total_entities: session.scope.stats.total_entities,
            total_cbus: session.scope.stats.total_cbus,
            jurisdictions,
        }
    }
    
    fn build_cursor_context(session: &UnifiedSessionContext) -> Option<CursorContext> {
        let graph = session.graph.as_ref()?;
        let cursor_id = graph.cursor?;
        let node = graph.nodes.get(&cursor_id)?;
        
        Some(CursorContext {
            entity_id: cursor_id.to_string(),
            name: node.name.clone(),
            entity_type: format!("{:?}", node.entity_type),
            jurisdiction: node.jurisdiction.clone(),
            depth_from_terminus: node.depth_from_terminus.unwrap_or(0),
            is_terminus: graph.termini.contains(&cursor_id),
            is_natural_person: node.is_natural_person,
        })
    }
    
    fn build_neighborhood(session: &UnifiedSessionContext) -> Option<NeighborhoodContext> {
        let graph = session.graph.as_ref()?;
        let cursor_id = graph.cursor?;
        let node = graph.nodes.get(&cursor_id)?;
        
        let max_show = 5;
        
        let owners: Vec<NeighborSummary> = node.owners.iter()
            .filter_map(|id| graph.nodes.get(id))
            .take(max_show)
            .map(|n| NeighborSummary {
                name: n.name.clone(),
                entity_type: format!("{:?}", n.entity_type),
                hint: None, // TODO: Add ownership percentage
            })
            .collect();
        
        let children: Vec<NeighborSummary> = node.owned.iter()
            .filter_map(|id| graph.nodes.get(id))
            .take(max_show)
            .map(|n| NeighborSummary {
                name: n.name.clone(),
                entity_type: format!("{:?}", n.entity_type),
                hint: None,
            })
            .collect();
        
        let children_truncated = if node.owned.len() > max_show {
            Some(node.owned.len() - max_show)
        } else {
            None
        };
        
        Some(NeighborhoodContext {
            owners,
            children,
            controllers: vec![], // TODO: Implement
            controlled: vec![],
            children_truncated,
        })
    }
    
    fn build_filter_context(session: &UnifiedSessionContext) -> FilterContext {
        let filters = session.graph.as_ref()
            .map(|g| &g.filters)
            .cloned()
            .unwrap_or_default();
        
        FilterContext {
            prong: match filters.prong {
                ProngFilter::Both => "both",
                ProngFilter::OwnershipOnly => "ownership",
                ProngFilter::ControlOnly => "control",
            }.to_string(),
            jurisdictions: filters.jurisdictions,
            as_of_date: filters.as_of_date.to_string(),
            min_ownership_pct: filters.min_ownership_pct.map(|d| d.to_string().parse().unwrap_or(0.0)),
        }
    }
    
    fn build_viewport_context(session: &UnifiedSessionContext) -> ViewportForAgent {
        let vp = &session.viewport;
        
        let off_screen_hint = if vp.off_screen.below > 0 {
            Some(format!("{} entities below", vp.off_screen.below))
        } else if vp.off_screen.above > 0 {
            Some(format!("{} entities above", vp.off_screen.above))
        } else {
            None
        };
        
        ViewportForAgent {
            zoom_level: format!("{:?}", vp.zoom_name),
            visible_count: vp.visible_entities.len(),
            off_screen_above: vp.off_screen.above,
            off_screen_below: vp.off_screen.below,
            off_screen_left: vp.off_screen.left,
            off_screen_right: vp.off_screen.right,
            off_screen_hint,
        }
    }
    
    fn compute_suggestions(session: &UnifiedSessionContext) -> Vec<SuggestedCommand> {
        let mut suggestions = Vec::new();
        
        // Check if we have a graph and cursor
        if let Some(graph) = &session.graph {
            if let Some(cursor_id) = graph.cursor {
                if let Some(node) = graph.nodes.get(&cursor_id) {
                    // Can go up?
                    if !node.owners.is_empty() {
                        let parent_name = node.owners.first()
                            .and_then(|id| graph.nodes.get(id))
                            .map(|n| n.name.as_str())
                            .unwrap_or("parent");
                        suggestions.push(SuggestedCommand {
                            command: "go up".into(),
                            description: format!("Navigate to {} (owner)", parent_name),
                            relevance: 0.9,
                        });
                    }
                    
                    // Can go down?
                    if !node.owned.is_empty() {
                        suggestions.push(SuggestedCommand {
                            command: format!("go down ({})", node.owned.len()),
                            description: format!("Navigate to one of {} owned entities", node.owned.len()),
                            relevance: 0.8,
                        });
                    }
                    
                    // At terminus?
                    if graph.termini.contains(&cursor_id) {
                        suggestions.push(SuggestedCommand {
                            command: "show tree 3".into(),
                            description: "Show ownership tree from this terminus".into(),
                            relevance: 0.7,
                        });
                    }
                }
            } else {
                // No cursor - suggest setting one
                suggestions.push(SuggestedCommand {
                    command: "go to [entity name]".into(),
                    description: "Set cursor on an entity to navigate".into(),
                    relevance: 1.0,
                });
            }
            
            // Viewport suggestions
            let vp = &session.viewport;
            if vp.off_screen.below > 5 {
                suggestions.push(SuggestedCommand {
                    command: "pan down".into(),
                    description: format!("See {} more entities below", vp.off_screen.below),
                    relevance: 0.6,
                });
            }
            if vp.off_screen.above > 5 {
                suggestions.push(SuggestedCommand {
                    command: "pan up".into(),
                    description: format!("See {} more entities above", vp.off_screen.above),
                    relevance: 0.6,
                });
            }
        } else {
            // No graph loaded
            suggestions.push(SuggestedCommand {
                command: "load cbu [name]".into(),
                description: "Load a CBU to start navigating".into(),
                relevance: 1.0,
            });
            suggestions.push(SuggestedCommand {
                command: "load book [client name]".into(),
                description: "Load all CBUs under a commercial client".into(),
                relevance: 0.9,
            });
        }
        
        suggestions
    }
    
    fn build_bindings(session: &UnifiedSessionContext) -> Vec<BindingSummary> {
        session.execution.symbols.iter()
            .map(|(name, _uuid)| BindingSummary {
                name: format!("@{}", name),
                binding_type: session.execution.symbol_types
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| "unknown".into()),
                display_name: None, // TODO: Look up display name
            })
            .collect()
    }
    
    /// Format as JSON for injection into agent prompt
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
    
    /// Format as concise text for agent prompt
    pub fn to_prompt_text(&self) -> String {
        let mut parts = Vec::new();
        
        // Scope
        parts.push(format!(
            "[SCOPE: {} \"{}\" - {} entities, {} CBUs]",
            self.scope.scope_type,
            self.scope.scope_name,
            self.scope.total_entities,
            self.scope.total_cbus
        ));
        
        // Cursor
        if let Some(cursor) = &self.cursor {
            parts.push(format!(
                "[CURSOR: {} ({}) depth={} {}]",
                cursor.name,
                cursor.entity_type,
                cursor.depth_from_terminus,
                if cursor.is_terminus { "TERMINUS" } else { "" }
            ));
        } else {
            parts.push("[CURSOR: None - use 'go to X' to set]".into());
        }
        
        // Neighborhood
        if let Some(hood) = &self.neighborhood {
            if !hood.owners.is_empty() {
                let owners: Vec<&str> = hood.owners.iter().map(|n| n.name.as_str()).collect();
                parts.push(format!("[OWNERS: {}]", owners.join(", ")));
            }
            if !hood.children.is_empty() {
                let count = hood.children.len() + hood.children_truncated.unwrap_or(0);
                parts.push(format!("[CHILDREN: {} entities]", count));
            }
        }
        
        // Viewport
        parts.push(format!(
            "[VIEW: {} - {} visible, off-screen: ↑{} ↓{} ←{} →{}]",
            self.viewport.zoom_level,
            self.viewport.visible_count,
            self.viewport.off_screen_above,
            self.viewport.off_screen_below,
            self.viewport.off_screen_left,
            self.viewport.off_screen_right
        ));
        
        // Suggestions
        if !self.suggested_commands.is_empty() {
            let cmds: Vec<String> = self.suggested_commands.iter()
                .take(3)
                .map(|s| format!("'{}': {}", s.command, s.description))
                .collect();
            parts.push(format!("[SUGGESTED: {}]", cmds.join(" | ")));
        }
        
        parts.join("\n")
    }
}
```

---

## Phase 4: Agent RAG Index for Verb Discovery

### Task 4.1: Extend verb YAML schema with agent_guidance

**File**: Update verb YAML files to include agent guidance

Example in `rust/config/verbs/graph.yaml`:

```yaml
domains:
  graph:
    description: "Graph visualization and traversal operations"
    
    # NEW: Agent context for the domain
    agent_context:
      summary: |
        Graph domain provides navigation and visualization of entity ownership
        and control structures. Use these verbs to explore relationships,
        find entities, and understand ownership chains.
      
      when_to_use: |
        - User wants to visualize entity relationships
        - User asks about ownership or control structures
        - User wants to find where a person appears across CBUs
        - User needs to navigate up/down ownership chains
      
      prerequisites:
        - "Scope must be loaded (CBU, Book, or Jurisdiction)"
    
    verbs:
      view:
        description: "Build full graph view from a CBU root"
        
        # NEW: Extended agent guidance
        agent_guidance:
          intent_patterns:
            - "show me the graph"
            - "visualize {cbu_name}"
            - "display the structure"
            - "what does {cbu_name} look like"
          
          when_to_use: |
            Use when user wants to see the visual structure of a CBU.
            This is typically the FIRST graph command after loading a CBU.
          
          preconditions:
            - "CBU must be loaded or specified"
          
          postconditions:
            - "Graph view available for navigation"
            - "Cursor can now be set on any visible entity"
          
          example_prompts:
            - "Show me the Alpha Fund structure"
            - "Visualize the Allianz book"
          
          related_verbs:
            - nav.go-up: "Navigate up ownership chain"
            - nav.go-down: "Navigate down to owned entities"
            - graph.filter: "Filter the visible entities"
```

### Task 4.2: Create verb index for RAG

**File**: `rust/config/verb_index.yaml` (NEW)

```yaml
# Verb Index for Agent RAG Discovery
#
# This file provides semantic search metadata for each verb/command.
# Entries are embedded and used for intent → verb matching.
#
# Fields:
# - verb: Full verb name (domain.verb)
# - domain: Domain name
# - category: Functional category
# - search_text: Text for embedding (intent patterns + description)
# - intent_tags: Keyword tags for filtering
# - prerequisites: What must exist before using
# - produces: What this verb creates/changes
# - typical_next: What usually follows this verb
# - example_prompt: Natural language that would trigger this
# - example_command: DSL command syntax

version: "1.0"

entries:
  # ===========================================================================
  # SCOPE COMMANDS
  # ===========================================================================
  
  - verb: nav.load-cbu
    domain: nav
    category: scope
    search_text: |
      load cbu fund show visualize open display structure
      start with single cbu client business unit
    intent_tags: [load, scope, cbu, single, start]
    prerequisites: []
    produces: scope
    typical_next: [nav.go-to, nav.show-tree, graph.view]
    example_prompt: "Show me the Alpha Fund"
    example_command: "load cbu \"Alpha Fund\""
    
  - verb: nav.load-book
    domain: nav
    category: scope
    search_text: |
      load book client commercial all cbus under owned by
      show everything apex owner group structure
    intent_tags: [load, scope, book, multiple, group, apex]
    prerequisites: []
    produces: scope
    typical_next: [nav.filter-jurisdiction, nav.go-to, nav.list-cbus]
    example_prompt: "Show me all the Allianz funds"
    example_command: "load book \"Allianz SE\""
    
  - verb: nav.load-jurisdiction
    domain: nav
    category: scope
    search_text: |
      load jurisdiction country all entities in
      luxembourg ireland cayman show everything lu ie ky
    intent_tags: [load, scope, jurisdiction, country, large]
    prerequisites: []
    produces: scope
    typical_next: [nav.filter-fund-type, nav.list-cbus]
    example_prompt: "Show me all Luxembourg entities"
    example_command: "load jurisdiction LU"

  # ===========================================================================
  # NAVIGATION COMMANDS
  # ===========================================================================
  
  - verb: nav.go-up
    domain: nav
    category: navigation
    search_text: |
      go up parent owner who owns this climb chain
      ancestor ownership traverse upward higher
    intent_tags: [navigate, up, parent, owner, climb]
    prerequisites: [cursor_set, has_parent]
    produces: cursor_move
    typical_next: [nav.go-up, nav.show-context, nav.show-path]
    spatial: {direction: up, affects: cursor}
    example_prompt: "Who owns this entity?"
    example_command: "go up"
    
  - verb: nav.go-down
    domain: nav
    category: navigation
    search_text: |
      go down child owned subsidiaries what does this own
      descend into children below lower
    intent_tags: [navigate, down, child, owned, descend]
    prerequisites: [cursor_set, has_children]
    produces: cursor_move
    typical_next: [nav.go-down, nav.show-context, nav.list-children]
    spatial: {direction: down, affects: cursor}
    example_prompt: "What does this entity own?"
    example_command: "go down"
    
  - verb: nav.go-to-terminus
    domain: nav
    category: navigation
    search_text: |
      go to terminus top ultimate owner ubo apex
      climb to top ownership chain final owner
    intent_tags: [navigate, terminus, top, ubo, apex]
    prerequisites: [cursor_set]
    produces: cursor_move
    typical_next: [nav.show-tree, nav.show-context]
    example_prompt: "Take me to the ultimate owner"
    example_command: "go to terminus"

  # ===========================================================================
  # FILTER COMMANDS
  # ===========================================================================
  
  - verb: nav.filter-jurisdiction
    domain: nav
    category: filter
    search_text: |
      filter jurisdiction focus on country only show
      luxembourg ireland lu ie ky restrict to
    intent_tags: [filter, jurisdiction, country, focus]
    prerequisites: [scope_loaded]
    produces: filter
    typical_next: [nav.go-to, nav.list-cbus]
    example_prompt: "Focus on Luxembourg entities"
    example_command: "focus on LU"
    
  - verb: nav.filter-prong
    domain: nav
    category: filter
    search_text: |
      filter prong ownership control only show
      who owns who controls directors board
    intent_tags: [filter, prong, ownership, control]
    prerequisites: [scope_loaded]
    produces: filter
    typical_next: [nav.show-tree, nav.list-controllers]
    example_prompt: "Show me the control structure"
    example_command: "show control prong"

  # ===========================================================================
  # QUERY COMMANDS
  # ===========================================================================
  
  - verb: nav.where-is
    domain: nav
    category: query
    search_text: |
      where is find person role director officer
      search locate all roles held by across cbus
    intent_tags: [query, find, person, role, cross_cbu]
    prerequisites: [scope_loaded]
    produces: query_result
    cross_cbu: true
    example_prompt: "Where is Hans a director?"
    example_command: "where is \"Hans\" a director"
    
  - verb: nav.find
    domain: nav
    category: query
    search_text: |
      find search locate entity name pattern
      look for match contains
    intent_tags: [query, find, search, name]
    prerequisites: [scope_loaded]
    produces: query_result
    example_prompt: "Find the AI fund"
    example_command: "find \"AI\""

  # ===========================================================================
  # VIEWPORT COMMANDS
  # ===========================================================================
  
  - verb: nav.pan
    domain: nav
    category: viewport
    search_text: |
      pan scroll move camera view see more
      up down left right above below
    intent_tags: [viewport, pan, scroll, camera]
    prerequisites: [scope_loaded]
    produces: viewport_change
    spatial: {direction: parameter, affects: viewport_only}
    example_prompt: "Show me what's below"
    example_command: "pan down"
    
  - verb: nav.zoom
    domain: nav
    category: viewport
    search_text: |
      zoom in out closer further magnify scale
      detail overview see more less
    intent_tags: [viewport, zoom, scale, detail]
    prerequisites: [scope_loaded]
    produces: viewport_change
    example_prompt: "Zoom out to see the whole structure"
    example_command: "zoom out"
    
  - verb: nav.fit-all
    domain: nav
    category: viewport
    search_text: |
      fit all show everything entire graph
      see whole structure overview
    intent_tags: [viewport, fit, all, overview]
    prerequisites: [scope_loaded]
    produces: viewport_change
    example_prompt: "Show me everything"
    example_command: "fit all"
```

### Task 4.3: Create RAG query service

**File**: `rust/src/agent/verb_discovery.rs` (NEW)

```rust
//! Verb Discovery Service
//!
//! Uses embeddings to match user intent to available verbs/commands.
//! Integrates with pgvector for semantic similarity search.

use anyhow::Result;
use sqlx::PgPool;

/// A verb entry from the index
#[derive(Debug, Clone)]
pub struct VerbEntry {
    pub verb: String,
    pub domain: String,
    pub category: String,
    pub search_text: String,
    pub intent_tags: Vec<String>,
    pub prerequisites: Vec<String>,
    pub example_prompt: String,
    pub example_command: String,
}

/// Discovery result with similarity score
#[derive(Debug, Clone)]
pub struct DiscoveryResult {
    pub verb: VerbEntry,
    pub similarity: f32,
}

/// Service for discovering verbs from user intent
pub struct VerbDiscoveryService {
    pool: PgPool,
}

impl VerbDiscoveryService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Find verbs matching user intent using semantic similarity
    pub async fn discover(
        &self,
        user_intent: &str,
        category_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<DiscoveryResult>> {
        // Use pgvector for embedding similarity search
        // This assumes verb_index entries have been embedded and stored
        
        let results = sqlx::query_as!(
            VerbIndexRow,
            r#"
            WITH intent_embedding AS (
                -- Would use embedding function here
                SELECT $1::text as intent
            )
            SELECT 
                vi.verb,
                vi.domain,
                vi.category,
                vi.search_text,
                vi.intent_tags,
                vi.prerequisites,
                vi.example_prompt,
                vi.example_command,
                -- Similarity score (would use vector similarity)
                1.0 as similarity
            FROM "ob-poc".verb_index vi
            WHERE ($2::text IS NULL OR vi.category = $2)
            ORDER BY similarity DESC
            LIMIT $3
            "#,
            user_intent,
            category_filter,
            limit as i32
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(results.into_iter().map(|r| DiscoveryResult {
            verb: VerbEntry {
                verb: r.verb,
                domain: r.domain,
                category: r.category,
                search_text: r.search_text,
                intent_tags: r.intent_tags,
                prerequisites: r.prerequisites,
                example_prompt: r.example_prompt,
                example_command: r.example_command,
            },
            similarity: r.similarity as f32,
        }).collect())
    }
    
    /// Get verbs applicable to current state
    pub async fn suggest_for_state(
        &self,
        has_scope: bool,
        has_cursor: bool,
        cursor_has_parent: bool,
        cursor_has_children: bool,
    ) -> Result<Vec<VerbEntry>> {
        let mut applicable = Vec::new();
        
        if !has_scope {
            // Only scope commands applicable
            let scope_verbs = self.discover("load scope", Some("scope"), 5).await?;
            applicable.extend(scope_verbs.into_iter().map(|r| r.verb));
        } else if !has_cursor {
            // Scope loaded but no cursor
            let nav_verbs = self.discover("go to find", Some("navigation"), 3).await?;
            applicable.extend(nav_verbs.into_iter().map(|r| r.verb));
        } else {
            // Cursor is set
            if cursor_has_parent {
                let up = self.discover("go up", Some("navigation"), 1).await?;
                applicable.extend(up.into_iter().map(|r| r.verb));
            }
            if cursor_has_children {
                let down = self.discover("go down", Some("navigation"), 1).await?;
                applicable.extend(down.into_iter().map(|r| r.verb));
            }
        }
        
        Ok(applicable)
    }
}

#[derive(sqlx::FromRow)]
struct VerbIndexRow {
    verb: String,
    domain: String,
    category: String,
    search_text: String,
    intent_tags: Vec<String>,
    prerequisites: Vec<String>,
    example_prompt: String,
    example_command: String,
    similarity: f64,
}
```

### Task 4.4: Create verb_index table migration

**File**: `rust/migrations/YYYYMMDD_verb_index.sql` (NEW)

```sql
-- Verb Index Table for Agent RAG Discovery
-- Stores verb metadata with embeddings for semantic search

CREATE TABLE IF NOT EXISTS "ob-poc".verb_index (
    verb_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    verb VARCHAR(100) NOT NULL UNIQUE,  -- e.g., "nav.go-up"
    domain VARCHAR(50) NOT NULL,
    category VARCHAR(50) NOT NULL,
    search_text TEXT NOT NULL,           -- Text used for embedding
    intent_tags TEXT[] NOT NULL DEFAULT '{}',
    prerequisites TEXT[] NOT NULL DEFAULT '{}',
    produces VARCHAR(50),
    typical_next TEXT[] DEFAULT '{}',
    example_prompt TEXT,
    example_command TEXT,
    spatial JSONB,                        -- {direction: "up", affects: "cursor"}
    cross_cbu BOOLEAN DEFAULT FALSE,
    
    -- Embedding for semantic search (requires pgvector)
    embedding vector(1536),
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Index for fast similarity search
CREATE INDEX IF NOT EXISTS idx_verb_index_embedding 
    ON "ob-poc".verb_index 
    USING ivfflat (embedding vector_cosine_ops)
    WITH (lists = 100);

-- Index for category filtering
CREATE INDEX IF NOT EXISTS idx_verb_index_category 
    ON "ob-poc".verb_index(category);

-- Index for domain filtering
CREATE INDEX IF NOT EXISTS idx_verb_index_domain 
    ON "ob-poc".verb_index(domain);

COMMENT ON TABLE "ob-poc".verb_index IS 
    'Verb metadata for agent RAG discovery - enables semantic intent matching';
```

---

## Phase 5: Integration

### Task 5.1: Wire up session to API

**File**: `rust/src/api/session_routes.rs` (NEW or modify existing)

Add endpoints:
- `GET /api/session/{session_id}/context` → Returns `AgentGraphContext`
- `POST /api/session/{session_id}/nav` → Execute navigation command
- `POST /api/session/{session_id}/load` → Load scope
- `GET /api/agent/discover?intent={text}` → Verb discovery

### Task 5.2: Update ob-agentic to use new context

**File**: `rust/crates/ob-agentic/src/context_builder.rs`

Extend `AgentContextBuilder` to include navigation context:

```rust
impl AgentContextBuilder {
    /// Add navigation context from unified session
    pub fn with_navigation_context(mut self, session: &UnifiedSessionContext) -> Self {
        self.nav_context = Some(AgentGraphContext::from_session(session));
        self
    }
    
    pub fn build(self) -> AgentContext {
        // ... existing logic ...
        
        // Append navigation context if available
        if let Some(nav) = &self.nav_context {
            // Add to prompt context
        }
    }
}
```

---

## Testing

### Unit Tests

1. **ViewportContext tests**
   - `test_pan_updates_offset`
   - `test_zoom_bounds`
   - `test_visibility_computation`

2. **UnifiedSessionContext tests**
   - `test_execute_nav_updates_viewport`
   - `test_load_scope_resets_viewport`
   - `test_command_history_recorded`

3. **AgentGraphContext tests**
   - `test_suggestions_when_no_cursor`
   - `test_suggestions_at_terminus`
   - `test_off_screen_hints`

4. **VerbDiscoveryService tests**
   - `test_discover_navigation_verbs`
   - `test_suggest_for_empty_state`
   - `test_suggest_with_cursor`

### Integration Tests

1. **Full navigation flow**
   ```rust
   #[tokio::test]
   async fn test_full_navigation_flow() {
       let session = UnifiedSessionContext::new();
       session.load_scope(GraphScope::Book { apex: allianz_se }, &repo).await?;
       
       let ctx = AgentGraphContext::from_session(&session);
       assert!(!ctx.scope.scope_name.is_empty());
       assert!(ctx.cursor.is_none());
       
       session.execute_nav(NavCommand::GoTo { entity_name: "AI Fund".into() });
       
       let ctx = AgentGraphContext::from_session(&session);
       assert!(ctx.cursor.is_some());
       assert!(!ctx.suggested_commands.is_empty());
   }
   ```

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `rust/src/graph/viewport.rs` | CREATE | ViewportContext struct |
| `rust/src/graph/mod.rs` | MODIFY | Export viewport module |
| `rust/src/session/mod.rs` | CREATE | UnifiedSessionContext |
| `rust/src/session/scope.rs` | CREATE | SessionScope with windowing |
| `rust/src/session/agent_context.rs` | CREATE | AgentGraphContext |
| `rust/src/lib.rs` | MODIFY | Export session module |
| `rust/config/verb_index.yaml` | CREATE | RAG verb index |
| `rust/src/agent/mod.rs` | CREATE | Agent module |
| `rust/src/agent/verb_discovery.rs` | CREATE | VerbDiscoveryService |
| `rust/migrations/YYYYMMDD_verb_index.sql` | CREATE | verb_index table |
| `rust/config/verbs/*.yaml` | MODIFY | Add agent_guidance sections |
| `rust/crates/ob-agentic/src/context_builder.rs` | MODIFY | Integrate navigation context |

---

## Success Criteria

1. **Session unification works**
   - [ ] Single context handles DSL execution + navigation + viewport
   - [ ] State persists across command execution
   - [ ] History supports undo/replay

2. **Viewport tracking works**
   - [ ] Zoom/pan commands update viewport state
   - [ ] Visibility computed correctly
   - [ ] Off-screen counts accurate

3. **Agent context complete**
   - [ ] `AgentGraphContext` serializes to JSON
   - [ ] Suggested commands reflect current state
   - [ ] Off-screen hints generated

4. **RAG discovery works**
   - [ ] verb_index.yaml loaded into database
   - [ ] Semantic search returns relevant verbs
   - [ ] State-aware suggestions work

5. **Integration complete**
   - [ ] API endpoints return correct context
   - [ ] ob-agentic uses new context builder
   - [ ] Agent can dynamically discover commands
