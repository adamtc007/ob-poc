# TODO: Graph DSL Domain

## ⛔ MANDATORY FIRST STEP

**Read these files before starting:**
- `/EGUI-RULES.md` - UI patterns and constraints
- `/rust/src/dsl_v2/runtime_registry.rs` - RuntimeBehavior enum
- `/rust/src/dsl_v2/generic_executor.rs` - Executor patterns
- `/rust/config/verbs/entity.yaml` - Verb YAML structure
- `/ALLIANZ-DATA-ACQUISITION.md` - Context on multi-source graph

---

## Overview

The `graph.*` DSL domain provides query/visualization verbs that operate on the entity 
relationship graph. Unlike CRUD verbs that modify data, these are **read-only query 
operations** that return view models for the UI.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  EXISTING DOMAINS              NEW DOMAIN                                   │
│  ════════════════              ══════════                                   │
│                                                                             │
│  entity.*   → Create/update    graph.view      → Query for visualization   │
│  ubo.*      → Ownership CRUD   graph.focus     → Set focus point           │
│  register.* → Share classes    graph.filter    → Apply filters             │
│  kyc.*      → Documents        graph.path      → Find paths                │
│  catalogue.* → Products        graph.connected → Find related              │
│                                graph.compare   → Compare entities          │
│                                                                             │
│  CRUD operations               QUERY operations                            │
│  (modify database)             (read-only, return view model)              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key difference:** Graph verbs don't map to SQL INSERT/UPDATE. They execute 
graph traversal algorithms and return `GraphViewModel` for rendering.

---

## Part 1: New Behavior Type

### 1.1 Extend RuntimeBehavior Enum

**File:** `rust/src/dsl_v2/runtime_registry.rs`

```rust
#[derive(Debug, Clone)]
pub enum RuntimeBehavior {
    /// Standard CRUD operation (boxed to reduce enum size)
    Crud(Box<RuntimeCrudConfig>),
    /// Plugin handler (Rust function)
    Plugin(String),
    /// Graph query operation (NEW)
    GraphQuery(Box<GraphQueryConfig>),
}

#[derive(Debug, Clone)]
pub struct GraphQueryConfig {
    /// Query type
    pub operation: GraphOperation,
    /// Default depth for traversal
    pub default_depth: Option<i32>,
    /// Default edge types to include
    pub default_edges: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy)]
pub enum GraphOperation {
    /// Build view from focus point
    View,
    /// Find path between two entities
    Path,
    /// Find all connected via edge type
    Connected,
    /// Compare multiple entities
    Compare,
    /// Aggregate/group entities
    Aggregate,
}
```

### 1.2 Tasks - Behavior Type

- [ ] Add `GraphQuery` variant to `RuntimeBehavior` enum
- [ ] Add `GraphQueryConfig` struct
- [ ] Add `GraphOperation` enum
- [ ] Update YAML parser to recognize `behavior: graph_query`
- [ ] Update executor dispatch to handle `GraphQuery`

---

## Part 2: Graph Query Engine

### 2.1 Core Query Engine

**File:** `rust/src/graph/query_engine.rs` (new)

```rust
//! Graph Query Engine
//!
//! Executes graph DSL verbs and returns view models.

use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

use super::types::{GraphNode, GraphEdge, EdgeType};

/// Result of a graph query - used to build visualization
#[derive(Debug, Clone)]
pub struct GraphQueryResult {
    /// Nodes to display
    pub nodes: Vec<QueryNode>,
    /// Edges to display
    pub edges: Vec<QueryEdge>,
    /// Grouping information (if grouped)
    pub groups: Vec<QueryGroup>,
    /// Breadcrumb path to focus
    pub breadcrumb: Vec<BreadcrumbItem>,
    /// Total entities matching query
    pub total_count: usize,
    /// Entities actually returned (may be limited)
    pub returned_count: usize,
}

#[derive(Debug, Clone)]
pub struct QueryNode {
    pub id: Uuid,
    pub entity_type: String,
    pub label: String,
    pub sublabel: Option<String>,
    pub status: Option<String>,
    pub jurisdiction: Option<String>,
    pub attributes: HashMap<String, serde_json::Value>,
    pub child_count: i32,
    pub is_focus: bool,
    pub depth_from_focus: i32,
    pub group_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct QueryEdge {
    pub from_id: Uuid,
    pub to_id: Uuid,
    pub edge_type: EdgeType,
    pub label: Option<String>,
    pub attributes: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct QueryGroup {
    pub id: String,
    pub label: String,
    pub node_count: usize,
    pub collapsed: bool,
}

#[derive(Debug, Clone)]
pub struct BreadcrumbItem {
    pub id: Uuid,
    pub label: String,
    pub entity_type: String,
}

/// Query parameters from DSL
#[derive(Debug, Clone, Default)]
pub struct GraphQueryParams {
    /// Focus entity (center of view)
    pub focus_id: Option<Uuid>,
    /// Depth to traverse
    pub depth: i32,
    /// Edge types to include
    pub edge_types: Vec<EdgeType>,
    /// Direction of traversal
    pub direction: TraversalDirection,
    /// Filters to apply
    pub filters: Vec<QueryFilter>,
    /// Group by attribute
    pub group_by: Option<String>,
    /// Show siblings of focus
    pub show_siblings: bool,
    /// Maximum nodes to return
    pub limit: Option<usize>,
    /// For path queries: target entity
    pub target_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum TraversalDirection {
    Up,      // Parents only
    Down,    // Children only
    #[default]
    Both,    // Both directions
}

#[derive(Debug, Clone)]
pub struct QueryFilter {
    pub field: String,
    pub operator: FilterOperator,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Copy)]
pub enum FilterOperator {
    Eq,
    Ne,
    In,
    Contains,
    StartsWith,
}

/// Graph Query Engine
pub struct GraphQueryEngine {
    // Could hold reference to graph data or DB connection
}

impl GraphQueryEngine {
    pub fn new() -> Self {
        Self {}
    }

    /// Execute a view query
    pub async fn execute_view(
        &self,
        params: &GraphQueryParams,
        // DB pool or graph data reference
    ) -> Result<GraphQueryResult, QueryError> {
        // 1. Start from focus node
        // 2. BFS/DFS to depth limit
        // 3. Filter edges by type
        // 4. Filter nodes by criteria
        // 5. Build breadcrumb path up to root
        // 6. Apply grouping if specified
        // 7. Return QueryResult
        
        todo!("Implement view query")
    }

    /// Execute a path query (find shortest path)
    pub async fn execute_path(
        &self,
        params: &GraphQueryParams,
    ) -> Result<GraphQueryResult, QueryError> {
        // 1. BFS from focus to target
        // 2. Return path as highlighted nodes/edges
        
        todo!("Implement path query")
    }

    /// Execute a connected query (find all reachable)
    pub async fn execute_connected(
        &self,
        params: &GraphQueryParams,
    ) -> Result<GraphQueryResult, QueryError> {
        // 1. Traverse via specified edge types
        // 2. Collect all reachable nodes
        // 3. Optionally group/aggregate
        
        todo!("Implement connected query")
    }

    /// Build breadcrumb path to ultimate parent
    async fn build_breadcrumb(
        &self,
        from_id: Uuid,
    ) -> Result<Vec<BreadcrumbItem>, QueryError> {
        // Traverse OWNERSHIP edges upward to root
        todo!()
    }

    /// Apply filters to node set
    fn apply_filters(
        &self,
        nodes: Vec<QueryNode>,
        filters: &[QueryFilter],
    ) -> Vec<QueryNode> {
        nodes.into_iter()
            .filter(|n| filters.iter().all(|f| self.matches_filter(n, f)))
            .collect()
    }

    fn matches_filter(&self, node: &QueryNode, filter: &QueryFilter) -> bool {
        let value = node.attributes.get(&filter.field);
        match (&filter.operator, value) {
            (FilterOperator::Eq, Some(v)) => v == &filter.value,
            (FilterOperator::Ne, Some(v)) => v != &filter.value,
            (FilterOperator::In, Some(v)) => {
                if let serde_json::Value::Array(arr) = &filter.value {
                    arr.contains(v)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Group nodes by attribute
    fn group_nodes(
        &self,
        nodes: Vec<QueryNode>,
        group_by: &str,
    ) -> (Vec<QueryNode>, Vec<QueryGroup>) {
        let mut groups: HashMap<String, Vec<QueryNode>> = HashMap::new();
        
        for mut node in nodes {
            let group_key = node.attributes
                .get(group_by)
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            
            node.group_id = Some(group_key.clone());
            groups.entry(group_key).or_default().push(node);
        }

        let query_groups: Vec<QueryGroup> = groups.iter()
            .map(|(id, nodes)| QueryGroup {
                id: id.clone(),
                label: id.clone(),
                node_count: nodes.len(),
                collapsed: false,
            })
            .collect();

        let all_nodes: Vec<QueryNode> = groups.into_values().flatten().collect();

        (all_nodes, query_groups)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("Focus entity not found: {0}")]
    FocusNotFound(Uuid),
    #[error("Target entity not found: {0}")]
    TargetNotFound(Uuid),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Invalid query: {0}")]
    InvalidQuery(String),
}
```

### 2.2 Tasks - Query Engine

- [ ] Create `rust/src/graph/query_engine.rs`
- [ ] Implement `GraphQueryResult` struct
- [ ] Implement `GraphQueryParams` struct
- [ ] Implement `execute_view` method
- [ ] Implement `execute_path` method
- [ ] Implement `execute_connected` method
- [ ] Implement `build_breadcrumb` method
- [ ] Implement `apply_filters` method
- [ ] Implement `group_nodes` method
- [ ] Add to `rust/src/graph/mod.rs`

---

## Part 3: Verb YAML Configuration

### 3.1 Graph Domain Verbs

**File:** `rust/config/verbs/graph.yaml` (new)

```yaml
domains:
  graph:
    description: Graph query and visualization operations
    verbs:
      view:
        description: Generate a graph view centered on a focus entity
        behavior: graph_query
        graph_query:
          operation: view
          default_depth: 2
          default_edges:
            - ownership
        args:
          - name: focus
            type: uuid
            required: true
            description: Entity to center the view on
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: depth
            type: integer
            required: false
            default: 2
            description: Levels to traverse from focus
          - name: edges
            type: string_array
            required: false
            description: Edge types to include
            valid_values:
              - ownership
              - management
              - administration
              - custody
              - product
              - feeder
          - name: direction
            type: string
            required: false
            default: both
            valid_values:
              - up
              - down
              - both
          - name: show-siblings
            type: boolean
            required: false
            default: false
        returns:
          type: graph_view_model
          description: View model for rendering

      focus:
        description: Change the focus point of current view
        behavior: graph_query
        graph_query:
          operation: view
          default_depth: 1
        args:
          - name: entity
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
        returns:
          type: graph_view_model

      filter:
        description: Apply filters to current view
        behavior: graph_query
        graph_query:
          operation: view
        args:
          - name: jurisdiction
            type: string
            required: false
          - name: entity-type
            type: string
            required: false
          - name: status
            type: string
            required: false
          - name: manager
            type: string
            required: false
        returns:
          type: graph_view_model

      group-by:
        description: Group visible entities by attribute
        behavior: graph_query
        graph_query:
          operation: view
        args:
          - name: attribute
            type: string
            required: true
            valid_values:
              - jurisdiction
              - entity_type
              - manager
              - product
              - status
          - name: collapse
            type: boolean
            required: false
            default: true
        returns:
          type: graph_view_model

      path:
        description: Find and highlight path between two entities
        behavior: graph_query
        graph_query:
          operation: path
        args:
          - name: from
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: to
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: via
            type: string
            required: false
            default: ownership
            valid_values:
              - ownership
              - management
              - any
        returns:
          type: graph_view_model

      find-connected:
        description: Find all entities connected via relationship type
        behavior: graph_query
        graph_query:
          operation: connected
        args:
          - name: from
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: via
            type: string
            required: true
            valid_values:
              - ownership
              - management
              - product
              - custody
          - name: filter-type
            type: string
            required: false
            description: Only include entities of this type
          - name: collect-as
            type: string
            required: false
            description: Binding name for result set
        returns:
          type: entity_set

      compare:
        description: Compare multiple entities side by side
        behavior: graph_query
        graph_query:
          operation: compare
        args:
          - name: entities
            type: uuid_array
            required: true
            description: Entities to compare
          - name: attributes
            type: string_array
            required: false
            default:
              - jurisdiction
              - manager
              - products
        returns:
          type: comparison_view_model

      ancestors:
        description: Get all ancestors (ownership chain upward)
        behavior: graph_query
        graph_query:
          operation: connected
          default_edges:
            - ownership
        args:
          - name: entity
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: max-depth
            type: integer
            required: false
            default: 10
        returns:
          type: entity_list

      descendants:
        description: Get all descendants (ownership chain downward)
        behavior: graph_query
        graph_query:
          operation: connected
          default_edges:
            - ownership
        args:
          - name: entity
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: max-depth
            type: integer
            required: false
            default: 10
          - name: filter-type
            type: string
            required: false
        returns:
          type: entity_list
```

### 3.2 Tasks - Verb YAML

- [ ] Create `rust/config/verbs/graph.yaml`
- [ ] Define `graph.view` verb
- [ ] Define `graph.focus` verb
- [ ] Define `graph.filter` verb
- [ ] Define `graph.group-by` verb
- [ ] Define `graph.path` verb
- [ ] Define `graph.find-connected` verb
- [ ] Define `graph.compare` verb
- [ ] Define `graph.ancestors` verb
- [ ] Define `graph.descendants` verb
- [ ] Add tests for YAML parsing

---

## Part 4: Executor Integration

### 4.1 Graph Query Executor

**File:** `rust/src/dsl_v2/graph_executor.rs` (new)

```rust
//! Graph Query Executor
//!
//! Handles execution of graph.* DSL verbs.

use uuid::Uuid;
use std::collections::HashMap;

use crate::graph::query_engine::{GraphQueryEngine, GraphQueryParams, GraphQueryResult};
use super::runtime_registry::GraphQueryConfig;

pub struct GraphQueryExecutor {
    engine: GraphQueryEngine,
}

impl GraphQueryExecutor {
    pub fn new() -> Self {
        Self {
            engine: GraphQueryEngine::new(),
        }
    }

    /// Execute a graph query verb
    pub async fn execute(
        &self,
        config: &GraphQueryConfig,
        args: &HashMap<String, serde_json::Value>,
        // context with DB pool etc.
    ) -> Result<GraphQueryResult, ExecutionError> {
        // 1. Build GraphQueryParams from args
        let params = self.build_params(config, args)?;

        // 2. Dispatch to appropriate engine method
        match config.operation {
            GraphOperation::View => self.engine.execute_view(&params).await,
            GraphOperation::Path => self.engine.execute_path(&params).await,
            GraphOperation::Connected => self.engine.execute_connected(&params).await,
            GraphOperation::Compare => self.execute_compare(&params).await,
            GraphOperation::Aggregate => self.execute_aggregate(&params).await,
        }
        .map_err(|e| ExecutionError::QueryFailed(e.to_string()))
    }

    fn build_params(
        &self,
        config: &GraphQueryConfig,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<GraphQueryParams, ExecutionError> {
        let mut params = GraphQueryParams::default();

        // Focus entity
        if let Some(focus) = args.get("focus").or(args.get("entity")).or(args.get("from")) {
            params.focus_id = Some(
                serde_json::from_value(focus.clone())
                    .map_err(|_| ExecutionError::InvalidArg("focus".into()))?
            );
        }

        // Depth
        params.depth = args.get("depth")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(config.default_depth.unwrap_or(2));

        // Edge types
        if let Some(edges) = args.get("edges") {
            if let Some(arr) = edges.as_array() {
                params.edge_types = arr.iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|s| s.parse().ok())
                    .collect();
            }
        } else if let Some(default_edges) = &config.default_edges {
            params.edge_types = default_edges.iter()
                .filter_map(|s| s.parse().ok())
                .collect();
        }

        // Direction
        if let Some(dir) = args.get("direction").and_then(|v| v.as_str()) {
            params.direction = match dir {
                "up" => TraversalDirection::Up,
                "down" => TraversalDirection::Down,
                _ => TraversalDirection::Both,
            };
        }

        // Filters from individual args
        for (key, value) in args {
            if matches!(key.as_str(), "jurisdiction" | "entity-type" | "status" | "manager") {
                params.filters.push(QueryFilter {
                    field: key.replace("-", "_"),
                    operator: FilterOperator::Eq,
                    value: value.clone(),
                });
            }
        }

        // Group by
        params.group_by = args.get("attribute")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Target for path queries
        if let Some(to) = args.get("to") {
            params.target_id = Some(
                serde_json::from_value(to.clone())
                    .map_err(|_| ExecutionError::InvalidArg("to".into()))?
            );
        }

        // Show siblings
        params.show_siblings = args.get("show-siblings")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(params)
    }

    async fn execute_compare(&self, params: &GraphQueryParams) -> Result<GraphQueryResult, QueryError> {
        todo!("Implement compare")
    }

    async fn execute_aggregate(&self, params: &GraphQueryParams) -> Result<GraphQueryResult, QueryError> {
        todo!("Implement aggregate")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Invalid argument: {0}")]
    InvalidArg(String),
    #[error("Query failed: {0}")]
    QueryFailed(String),
}
```

### 4.2 Update Generic Executor

**File:** `rust/src/dsl_v2/generic_executor.rs`

Add dispatch for GraphQuery behavior:

```rust
// In execute_verb or equivalent method:

match &verb.behavior {
    RuntimeBehavior::Crud(config) => {
        self.execute_crud(config, &resolved_args, context).await
    }
    RuntimeBehavior::Plugin(handler) => {
        self.execute_plugin(handler, &resolved_args, context).await
    }
    RuntimeBehavior::GraphQuery(config) => {
        self.graph_executor.execute(config, &resolved_args, context).await
    }
}
```

### 4.3 Tasks - Executor

- [ ] Create `rust/src/dsl_v2/graph_executor.rs`
- [ ] Implement `build_params` method
- [ ] Implement dispatch to query engine
- [ ] Update `generic_executor.rs` to handle GraphQuery behavior
- [ ] Add `GraphQueryExecutor` to executor initialization
- [ ] Test execution of graph verbs

---

## Part 5: Edge Types Extension

### 5.1 Extend EdgeType Enum

**File:** `rust/src/graph/types.rs`

```rust
/// Types of edges in the entity graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    // Structural (from GLEIF/legal structure)
    Ownership,
    UltimateOwnership,
    
    // Operational (from fund docs/internal data)
    ManagedBy,
    AdministeredBy,
    CustodiedBy,
    
    // Product/Service (from BNY data)
    UsesProduct,
    UsesService,
    
    // Cross-border structures
    FeederTo,
    MasterOf,
    SPVFor,
    
    // Generic
    RelatedTo,
}

impl std::str::FromStr for EdgeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ownership" => Ok(Self::Ownership),
            "ultimate_ownership" | "ultimate-ownership" => Ok(Self::UltimateOwnership),
            "managed_by" | "managed-by" | "management" => Ok(Self::ManagedBy),
            "administered_by" | "administered-by" | "administration" => Ok(Self::AdministeredBy),
            "custodied_by" | "custodied-by" | "custody" => Ok(Self::CustodiedBy),
            "uses_product" | "uses-product" | "product" => Ok(Self::UsesProduct),
            "uses_service" | "uses-service" | "service" => Ok(Self::UsesService),
            "feeder_to" | "feeder-to" | "feeder" => Ok(Self::FeederTo),
            "master_of" | "master-of" => Ok(Self::MasterOf),
            "spv_for" | "spv-for" | "spv" => Ok(Self::SPVFor),
            "related_to" | "related-to" | "any" => Ok(Self::RelatedTo),
            _ => Err(format!("Unknown edge type: {}", s)),
        }
    }
}

impl EdgeType {
    /// Get the inverse relationship type
    pub fn inverse(&self) -> Self {
        match self {
            Self::Ownership => Self::Ownership, // bidirectional in queries
            Self::UltimateOwnership => Self::UltimateOwnership,
            Self::ManagedBy => Self::ManagedBy,
            Self::AdministeredBy => Self::AdministeredBy,
            Self::CustodiedBy => Self::CustodiedBy,
            Self::UsesProduct => Self::UsesProduct,
            Self::UsesService => Self::UsesService,
            Self::FeederTo => Self::MasterOf,
            Self::MasterOf => Self::FeederTo,
            Self::SPVFor => Self::SPVFor,
            Self::RelatedTo => Self::RelatedTo,
        }
    }

    /// Is this a hierarchical (parent-child) relationship?
    pub fn is_hierarchical(&self) -> bool {
        matches!(self, 
            Self::Ownership | 
            Self::UltimateOwnership | 
            Self::FeederTo | 
            Self::MasterOf
        )
    }
}
```

### 5.2 Database Support for Edge Types

Ensure `entity_relationships` table supports these edge types:

```sql
-- If not already present, add relationship_type values
ALTER TABLE "ob-poc".entity_relationships 
ADD CONSTRAINT valid_relationship_type 
CHECK (relationship_type IN (
    'OWNERSHIP',
    'ULTIMATE_OWNERSHIP', 
    'MANAGED_BY',
    'ADMINISTERED_BY',
    'CUSTODIED_BY',
    'USES_PRODUCT',
    'USES_SERVICE',
    'FEEDER_TO',
    'MASTER_OF',
    'SPV_FOR',
    'RELATED_TO'
));
```

### 5.3 Tasks - Edge Types

- [ ] Extend `EdgeType` enum with all relationship types
- [ ] Implement `FromStr` for EdgeType
- [ ] Implement `inverse()` method
- [ ] Update database schema if needed
- [ ] Add migration for new edge types

---

## Part 6: View Model for UI

### 6.1 GraphViewModel

**File:** `rust/src/graph/view_model.rs` (new)

```rust
//! Graph View Model
//!
//! Output of graph queries, consumed by UI renderer.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;

/// Complete view model for graph rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphViewModel {
    /// Focus entity ID
    pub focus_id: Option<Uuid>,
    
    /// Nodes to render
    pub nodes: Vec<NodeViewModel>,
    
    /// Edges to render
    pub edges: Vec<EdgeViewModel>,
    
    /// Groups (if grouped view)
    pub groups: Vec<GroupViewModel>,
    
    /// Breadcrumb path from focus to root
    pub breadcrumb: Vec<BreadcrumbViewModel>,
    
    /// Active filters
    pub active_filters: Vec<FilterViewModel>,
    
    /// Active edge type toggles
    pub active_edges: Vec<String>,
    
    /// Group by attribute (if any)
    pub grouped_by: Option<String>,
    
    /// Total matching count
    pub total_count: usize,
    
    /// Count shown (may be limited)
    pub shown_count: usize,
    
    /// DSL that generated this view (for save/share)
    pub source_dsl: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeViewModel {
    pub id: Uuid,
    pub entity_type: String,
    pub label: String,
    pub sublabel: Option<String>,
    pub status: Option<String>,
    pub jurisdiction: Option<String>,
    
    /// Token type for visual styling
    pub token_type: String,
    
    /// Is this the focus node?
    pub is_focus: bool,
    
    /// Depth from focus (0 = focus, negative = ancestors, positive = descendants)
    pub depth: i32,
    
    /// Is this a container with children?
    pub is_container: bool,
    pub child_count: i32,
    
    /// Group ID if grouped
    pub group_id: Option<String>,
    
    /// Position hint (layout can override)
    pub position_hint: Option<(f32, f32)>,
    
    /// Is node expanded (children visible)?
    pub expanded: bool,
    
    /// Highlighted (e.g., on path)?
    pub highlighted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeViewModel {
    pub from_id: Uuid,
    pub to_id: Uuid,
    pub edge_type: String,
    pub label: Option<String>,
    
    /// Visual style based on edge type
    pub style: EdgeStyle,
    
    /// Highlighted (e.g., on path)?
    pub highlighted: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EdgeStyle {
    Solid,      // Ownership
    Dashed,     // Management
    Dotted,     // Product
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupViewModel {
    pub id: String,
    pub label: String,
    pub node_count: usize,
    pub collapsed: bool,
    /// Bounding box hint (layout calculates actual)
    pub bounds_hint: Option<(f32, f32, f32, f32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreadcrumbViewModel {
    pub id: Uuid,
    pub label: String,
    pub entity_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterViewModel {
    pub field: String,
    pub display: String,
    pub value: String,
    pub removable: bool,
}

impl GraphViewModel {
    /// Create empty view model
    pub fn empty() -> Self {
        Self {
            focus_id: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            groups: Vec::new(),
            breadcrumb: Vec::new(),
            active_filters: Vec::new(),
            active_edges: vec!["ownership".into()],
            grouped_by: None,
            total_count: 0,
            shown_count: 0,
            source_dsl: None,
        }
    }

    /// Convert from QueryResult
    pub fn from_query_result(result: GraphQueryResult, source_dsl: Option<String>) -> Self {
        Self {
            focus_id: result.nodes.iter().find(|n| n.is_focus).map(|n| n.id),
            nodes: result.nodes.into_iter().map(NodeViewModel::from).collect(),
            edges: result.edges.into_iter().map(EdgeViewModel::from).collect(),
            groups: result.groups.into_iter().map(GroupViewModel::from).collect(),
            breadcrumb: result.breadcrumb.into_iter().map(BreadcrumbViewModel::from).collect(),
            active_filters: Vec::new(),
            active_edges: Vec::new(),
            grouped_by: None,
            total_count: result.total_count,
            shown_count: result.returned_count,
            source_dsl,
        }
    }
}
```

### 6.2 Tasks - View Model

- [ ] Create `rust/src/graph/view_model.rs`
- [ ] Define `GraphViewModel` struct
- [ ] Define `NodeViewModel` struct  
- [ ] Define `EdgeViewModel` struct
- [ ] Define `GroupViewModel` struct
- [ ] Implement `from_query_result` conversion
- [ ] Add to `rust/src/graph/mod.rs`

---

## Summary

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  IMPLEMENTATION ORDER                                                       │
│                                                                             │
│  1. Part 1: RuntimeBehavior extension (GraphQuery variant)                 │
│  2. Part 5: EdgeType enum extension                                        │
│  3. Part 6: GraphViewModel struct                                          │
│  4. Part 2: GraphQueryEngine implementation                                │
│  5. Part 3: graph.yaml verb definitions                                    │
│  6. Part 4: GraphQueryExecutor + integration                               │
│                                                                             │
│  ESTIMATED: 3-4 days                                                        │
│                                                                             │
│  DEPENDS ON:                                                                │
│  - Existing DSL infrastructure                                             │
│  - entity_relationships table with edge types                              │
│  - GLEIF data loaded (for testing)                                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## New DSL Capabilities After Implementation

```lisp
;; Basic view
(graph.view :focus @allianz-gi-lu :depth 2 :edges [ownership])

;; With filters
(graph.view :focus @allianz-se :depth 3 
            :edges [ownership management]
            :jurisdiction "LU" 
            :entity-type "FUND")

;; Grouped
(graph.view :focus @allianz-se :depth 2 :group-by jurisdiction)

;; Path finding
(graph.path :from @fund-123 :to @allianz-se :via ownership)

;; Find related
(graph.find-connected :from @allianz-gi-lu :via management 
                      :filter-type "FUND")

;; Ancestors (UBO chain up)
(graph.ancestors :entity @fund-123)

;; Descendants
(graph.descendants :entity @allianz-gi-lu :filter-type "FUND")

;; Compare entities
(graph.compare :entities [@fund-a @fund-b @fund-c] 
               :attributes [jurisdiction manager products])
```

---

## Success Criteria

- [ ] `graph.view` returns valid GraphViewModel
- [ ] Edge type filtering works
- [ ] Depth limiting works
- [ ] Filters apply correctly
- [ ] Grouping produces groups in view model
- [ ] Path finding returns highlighted path
- [ ] Breadcrumb builds correctly to root
- [ ] DSL executes through standard verb pipeline
- [ ] Agent can generate graph DSL from natural language

---

*Graph queries: same DSL, same engine, visual output.*
