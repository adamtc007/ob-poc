# TODO: CBU Container System

## ⛔ MANDATORY FIRST STEP

**Read these files before starting:**
- `/EGUI-RULES.md` - UI patterns and constraints
- `/rust/src/graph/types.rs` - Current graph model
- `/rust/crates/entity-gateway/config/entity_index.yaml` - Entity search config
- `/rust/src/services/resolution_service.rs` - Resolution pattern to reuse
- `/docs/ENTITY_RESOLUTION_UI.md` - UI design for entity search

---

## Overview

This TODO implements **container browsing** - the ability to click on a container 
(ShareClass, ServiceInstance) and explore its contents (investors, resources) 
using the existing EntityGateway search infrastructure.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  CONTAINER BROWSE FLOW                                                      │
│                                                                             │
│  1. User clicks ShareClass token in graph view                             │
│  2. Slide-in panel opens (same pattern as Entity Resolution)               │
│  3. Panel calls EntityGateway with parent scope                            │
│  4. User searches/filters/pages through contents                           │
│  5. User clicks item to view detail or returns to graph                    │
│                                                                             │
│  REUSES: EntityGateway fuzzy search, Resolution Panel UI patterns          │
│  NEW: Pagination, sorting, parent-scoped queries, container entity types   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Decision:** Virtual scrolling through a slide-in panel (not 3D visualization).
This reuses existing infrastructure and is appropriate for a business viewer.

---

## Part 1: Cleanup - Archive Superseded TODOs

The following TODO files contain 3D visualization concepts (silos, GPU instancing,
wgpu, ray-casting) that are **not being implemented**. Archive them to prevent
confusion.

### 1.1 Files to Archive

```bash
# Create archive directory
mkdir -p /docs/archive/3d-concepts

# Move superseded TODOs
mv TODO-CBU-TOKEN-SYSTEM.md docs/archive/3d-concepts/
mv TODO-CBU-CONTAINER-ARCHITECTURE.md docs/archive/3d-concepts/

# Create README explaining why archived
cat > docs/archive/3d-concepts/README.md << 'EOF'
# Archived: 3D Visualization Concepts

These TODOs explored 3D "silo" visualization for containers:
- Flying into containers as 3D tubes
- GPU instancing for 10,000+ items
- wgpu rendering pipeline
- 3D ray-casting for hit testing

**Decision:** Virtual scrolling through slide-in panels is sufficient for
business viewer use case. Container contents are searched/browsed via
EntityGateway, not rendered as 3D geometry.

The following concepts from these TODOs ARE still relevant and have been
incorporated into TODO-CBU-CONTAINERS.md:
- Container data model (is_container, child_count)
- GraphNode container fields
- Database tables for share_classes, investor_holdings
- Token type definitions (without 3D LOD rules)

Archived: [DATE]
EOF
```

### 1.2 Tasks

- [ ] Create `docs/archive/3d-concepts/` directory
- [ ] Move `TODO-CBU-TOKEN-SYSTEM.md` to archive
- [ ] Move `TODO-CBU-CONTAINER-ARCHITECTURE.md` to archive
- [ ] Create archive README with explanation
- [ ] Verify no 3D code exists in codebase (there shouldn't be any)

---

## Part 2: Database Schema - Container Tables

### 2.1 Share Classes Table

Share classes are containers for investor holdings (registers).

**File:** `migrations/YYYYMMDD_add_share_classes.sql`

```sql
-- Share classes - fund share class containers
CREATE TABLE IF NOT EXISTS "ob-poc".share_classes (
    share_class_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Parent reference
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    
    -- Identity
    share_class_code VARCHAR(50) NOT NULL,
    share_class_name VARCHAR(255) NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'USD',
    
    -- Classification
    share_class_type VARCHAR(50), -- INSTITUTIONAL, RETAIL, etc.
    distribution_type VARCHAR(50), -- ACCUMULATING, DISTRIBUTING
    
    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'ACTIVE',
    -- ACTIVE, SOFT_CLOSED, HARD_CLOSED, TERMINATED
    
    -- Aggregates (denormalized for performance)
    investor_count INTEGER DEFAULT 0,
    total_aum DECIMAL(20, 2) DEFAULT 0,
    
    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Constraints
    UNIQUE(cbu_id, share_class_code)
);

CREATE INDEX idx_share_classes_cbu ON "ob-poc".share_classes(cbu_id);
CREATE INDEX idx_share_classes_status ON "ob-poc".share_classes(status);

-- Trigger to update updated_at
CREATE TRIGGER share_classes_updated_at
    BEFORE UPDATE ON "ob-poc".share_classes
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_updated_at();
```

### 2.2 Investor Holdings Table

Investor holdings are the items inside share class containers.

```sql
-- Investor holdings - contents of share class registers
CREATE TABLE IF NOT EXISTS "ob-poc".investor_holdings (
    investor_holding_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Parent reference (the container)
    share_class_id UUID NOT NULL REFERENCES "ob-poc".share_classes(share_class_id),
    
    -- The investor entity
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Holding details
    units DECIMAL(20, 6) NOT NULL DEFAULT 0,
    holding_value DECIMAL(20, 2) NOT NULL DEFAULT 0,
    percentage DECIMAL(8, 4) NOT NULL DEFAULT 0, -- % of share class
    
    -- Investor classification (denormalized for search)
    investor_name VARCHAR(255) NOT NULL, -- Copied from entity for search
    investor_type VARCHAR(50), -- INSTITUTIONAL, RETAIL, PENSION, etc.
    jurisdiction VARCHAR(10), -- Copied from entity for filtering
    
    -- KYC status
    kyc_status VARCHAR(50) DEFAULT 'PENDING',
    -- PENDING, IN_PROGRESS, VERIFIED, EXPIRED, BLOCKED
    
    -- Dates
    acquisition_date DATE,
    last_transaction_date DATE,
    
    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Constraints
    UNIQUE(share_class_id, entity_id)
);

CREATE INDEX idx_investor_holdings_share_class ON "ob-poc".investor_holdings(share_class_id);
CREATE INDEX idx_investor_holdings_entity ON "ob-poc".investor_holdings(entity_id);
CREATE INDEX idx_investor_holdings_name ON "ob-poc".investor_holdings(investor_name);
CREATE INDEX idx_investor_holdings_jurisdiction ON "ob-poc".investor_holdings(jurisdiction);
CREATE INDEX idx_investor_holdings_value ON "ob-poc".investor_holdings(holding_value DESC);

-- Trigram index for fuzzy name search
CREATE INDEX idx_investor_holdings_name_trgm 
    ON "ob-poc".investor_holdings 
    USING gin (investor_name gin_trgm_ops);

-- Trigger to update updated_at
CREATE TRIGGER investor_holdings_updated_at
    BEFORE UPDATE ON "ob-poc".investor_holdings
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_updated_at();
```

### 2.3 Service Resources Table (if not exists)

Check if `cbu_service_resources` exists. If not, create it:

```sql
-- Service resource instances - contents of service containers
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_service_resources (
    cbu_service_resource_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Parent reference (the container)
    cbu_service_id UUID NOT NULL REFERENCES "ob-poc".cbu_services(cbu_service_id),
    
    -- Resource definition reference
    resource_type_code VARCHAR(50) NOT NULL,
    
    -- Instance details
    resource_name VARCHAR(255) NOT NULL,
    resource_identifier VARCHAR(255), -- Account number, URL, etc.
    
    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'ACTIVE',
    
    -- Configuration (JSON blob for type-specific data)
    configuration JSONB DEFAULT '{}',
    
    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_cbu_service_resources_service ON "ob-poc".cbu_service_resources(cbu_service_id);
CREATE INDEX idx_cbu_service_resources_type ON "ob-poc".cbu_service_resources(resource_type_code);
CREATE INDEX idx_cbu_service_resources_name_trgm 
    ON "ob-poc".cbu_service_resources 
    USING gin (resource_name gin_trgm_ops);
```

### 2.4 Tasks

- [ ] Create migration for share_classes table
- [ ] Create migration for investor_holdings table
- [ ] Verify/create cbu_service_resources table
- [ ] Add trigram indexes for fuzzy search
- [ ] Run migrations
- [ ] Add seed data for testing (see Part 7)

---

## Part 3: GraphNode Container Extensions

### 3.1 Add Container Fields to GraphNode

**File:** `rust/src/graph/types.rs`

Add these fields to the `GraphNode` struct:

```rust
/// A node in the graph representing an entity, document, or resource
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphNode {
    // ... existing fields ...

    // =========================================================================
    // CONTAINER FIELDS - for nodes that contain browseable children
    // =========================================================================
    
    /// Is this node a container with browseable contents?
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_container: bool,
    
    /// Entity type of children (e.g., "investor_holding", "service_resource")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains_type: Option<String>,
    
    /// Number of children (for badge display)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_count: Option<i64>,
    
    /// EntityGateway nickname for browsing children
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browse_nickname: Option<String>,
    
    /// Parent key field name for scoped queries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_key: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}
```

### 3.2 Add Container NodeTypes

**File:** `rust/src/graph/types.rs`

Add to the `NodeType` enum:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    // ... existing types ...
    
    // Container types
    ShareClass,       // Contains investor holdings
    ServiceInstance,  // Contains resource instances
    
    // Container item types (for detail views)
    InvestorHolding,
    ServiceResource,
}
```

### 3.3 Tasks

- [ ] Add container fields to GraphNode struct
- [ ] Add helper function `is_false` for serde skip
- [ ] Add ShareClass, ServiceInstance, InvestorHolding, ServiceResource to NodeType
- [ ] Update serde serialization tests

---

## Part 4: Graph Builder - Container Detection

### 4.1 Annotate Containers in Builder

**File:** `rust/src/graph/builder.rs`

Add container annotation when building share class nodes:

```rust
impl GraphBuilder {
    /// Build a share class node with container metadata
    fn build_share_class_node(&self, share_class: &ShareClassRow) -> GraphNode {
        // Get child count from database or cache
        let child_count = self.get_investor_count(share_class.share_class_id);
        
        GraphNode {
            id: share_class.share_class_id.to_string(),
            node_type: NodeType::ShareClass,
            layer: LayerType::Core,
            label: share_class.share_class_code.clone(),
            sublabel: Some(format!("{} {}", share_class.currency, share_class.share_class_type.as_deref().unwrap_or(""))),
            status: self.map_status(&share_class.status),
            
            // Container fields
            is_container: true,
            contains_type: Some("investor_holding".to_string()),
            child_count: Some(child_count),
            browse_nickname: Some("INVESTOR_HOLDING".to_string()),
            parent_key: Some("share_class_id".to_string()),
            
            // Data payload
            data: serde_json::json!({
                "share_class_id": share_class.share_class_id,
                "currency": share_class.currency,
                "total_aum": share_class.total_aum,
                "investor_count": child_count,
            }),
            
            ..Default::default()
        }
    }
    
    /// Get investor count for a share class (from cache or DB)
    fn get_investor_count(&self, share_class_id: Uuid) -> i64 {
        // Check cache first, then query if needed
        // SELECT COUNT(*) FROM investor_holdings WHERE share_class_id = $1
        0 // TODO: implement
    }
}
```

### 4.2 Tasks

- [ ] Add `build_share_class_node` method to GraphBuilder
- [ ] Add `build_service_instance_node` method for service containers
- [ ] Implement `get_investor_count` with caching
- [ ] Wire share class loading into graph building pipeline
- [ ] Add unit tests for container node building

---

## Part 5: EntityGateway Extensions

### 5.1 Extend Proto for Paginated Browse

**File:** `rust/crates/entity-gateway/proto/ob/gateway/v1/entity_gateway.proto`

```protobuf
syntax = "proto3";

package ob.gateway.v1;

service EntityGateway {
  // Existing: resolve entity references
  rpc Search(SearchRequest) returns (SearchResponse);
  
  // NEW: browse container contents with pagination
  rpc Browse(BrowseRequest) returns (BrowseResponse);
}

// Existing messages unchanged...

// NEW: Browse request for container contents
message BrowseRequest {
  // Entity type to browse (e.g., "investor_holding", "service_resource")
  string nickname = 1;
  
  // Parent scope - filter to children of this parent
  string parent_key = 2;   // e.g., "share_class_id"
  string parent_value = 3; // e.g., "uuid-of-share-class"
  
  // Optional search query (fuzzy match on searchable fields)
  optional string query = 4;
  
  // Optional filters (field=value pairs)
  map<string, string> filters = 5;
  
  // Pagination
  int32 offset = 6;
  int32 limit = 7; // Default 50, max 100
  
  // Sorting
  optional string sort_field = 8;
  optional SortOrder sort_order = 9;
}

enum SortOrder {
  DESC = 0;
  ASC = 1;
}

message BrowseResponse {
  // Total count (for pagination UI)
  int64 total_count = 1;
  
  // Results for current page
  repeated BrowseItem items = 2;
  
  // Facets for filter UI (optional)
  map<string, Facet> facets = 3;
  
  // Pagination echo
  int32 offset = 4;
  int32 limit = 5;
}

message BrowseItem {
  // Primary key
  string id = 1;
  
  // Display fields
  string display = 2;
  string sublabel = 3;
  
  // Status for badge
  string status = 4;
  
  // All fields as JSON for detail view
  string data_json = 5;
}

message Facet {
  repeated FacetValue values = 1;
}

message FacetValue {
  string value = 1;
  int64 count = 2;
}
```

### 5.2 Add Entity Definitions for Container Items

**File:** `rust/crates/entity-gateway/config/entity_index.yaml`

Add these entity definitions:

```yaml
  # Investor holdings - items in share class containers
  investor_holding:
    nickname: "INVESTOR_HOLDING"
    source_table: '"ob-poc".investor_holdings'
    return_key: "investor_holding_id"
    display_template: "{investor_name}"
    index_mode: trigram
    
    # Parent scope for container queries
    parent_scope:
      key: share_class_id
      table: '"ob-poc".share_classes'
    
    search_keys:
      - name: "name"
        column: "investor_name"
        default: true
      - name: "jurisdiction"
        column: "jurisdiction"
      - name: "investor_type"
        column: "investor_type"
      - name: "kyc_status"
        column: "kyc_status"
    
    # Fields to return in browse results
    browse_fields:
      - investor_holding_id
      - investor_name
      - holding_value
      - percentage
      - jurisdiction
      - investor_type
      - kyc_status
      - acquisition_date
    
    # Default sort for browse
    default_sort:
      field: holding_value
      order: desc
    
    # Facets to compute for filter UI
    facets:
      - jurisdiction
      - investor_type
      - kyc_status
    
    shard:
      enabled: false # Queries are always scoped to parent

  # Service resources - items in service containers
  service_resource:
    nickname: "SERVICE_RESOURCE"
    source_table: '"ob-poc".cbu_service_resources'
    return_key: "cbu_service_resource_id"
    display_template: "{resource_name}"
    index_mode: trigram
    
    parent_scope:
      key: cbu_service_id
      table: '"ob-poc".cbu_services'
    
    search_keys:
      - name: "name"
        column: "resource_name"
        default: true
      - name: "resource_type"
        column: "resource_type_code"
      - name: "status"
        column: "status"
    
    browse_fields:
      - cbu_service_resource_id
      - resource_name
      - resource_type_code
      - resource_identifier
      - status
      - created_at
    
    default_sort:
      field: resource_name
      order: asc
    
    facets:
      - resource_type_code
      - status
    
    shard:
      enabled: false
```

### 5.3 Implement Browse Handler

**File:** `rust/crates/entity-gateway/src/server/browse.rs` (new file)

```rust
//! Browse handler for container contents

use crate::config::EntityConfig;
use crate::proto::ob::gateway::v1::{
    BrowseItem, BrowseRequest, BrowseResponse, Facet, FacetValue,
};
use anyhow::{bail, Result};
use sqlx::PgPool;
use std::collections::HashMap;

pub struct BrowseHandler {
    pool: PgPool,
    configs: HashMap<String, EntityConfig>,
}

impl BrowseHandler {
    pub fn new(pool: PgPool, configs: HashMap<String, EntityConfig>) -> Self {
        Self { pool, configs }
    }
    
    pub async fn browse(&self, req: BrowseRequest) -> Result<BrowseResponse> {
        let config = self.configs.get(&req.nickname)
            .ok_or_else(|| anyhow::anyhow!("Unknown entity type: {}", req.nickname))?;
        
        let parent_scope = config.parent_scope.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Entity {} is not browseable", req.nickname))?;
        
        // Validate parent key matches config
        if req.parent_key != parent_scope.key {
            bail!("Invalid parent key: expected {}, got {}", parent_scope.key, req.parent_key);
        }
        
        // Build query
        let limit = req.limit.min(100).max(1);
        let offset = req.offset.max(0);
        
        let (items, total_count) = self.execute_browse_query(
            config,
            &req.parent_key,
            &req.parent_value,
            req.query.as_deref(),
            &req.filters,
            req.sort_field.as_deref(),
            req.sort_order,
            offset,
            limit,
        ).await?;
        
        // Compute facets if configured
        let facets = if !config.facets.is_empty() {
            self.compute_facets(config, &req.parent_key, &req.parent_value).await?
        } else {
            HashMap::new()
        };
        
        Ok(BrowseResponse {
            total_count,
            items,
            facets,
            offset,
            limit,
        })
    }
    
    async fn execute_browse_query(
        &self,
        config: &EntityConfig,
        parent_key: &str,
        parent_value: &str,
        query: Option<&str>,
        filters: &HashMap<String, String>,
        sort_field: Option<&str>,
        sort_order: i32,
        offset: i32,
        limit: i32,
    ) -> Result<(Vec<BrowseItem>, i64)> {
        // Build SELECT clause from browse_fields
        let select_fields = config.browse_fields.join(", ");
        
        // Build WHERE clause
        let mut where_clauses = vec![format!("\"{}\" = $1", parent_key)];
        let mut params: Vec<String> = vec![parent_value.to_string()];
        let mut param_idx = 2;
        
        // Add search query if present
        if let Some(q) = query {
            if !q.is_empty() {
                let search_col = config.search_keys.iter()
                    .find(|k| k.default)
                    .map(|k| &k.column)
                    .unwrap_or(&config.search_keys[0].column);
                where_clauses.push(format!("\"{}\" ILIKE ${}", search_col, param_idx));
                params.push(format!("%{}%", q));
                param_idx += 1;
            }
        }
        
        // Add filters
        for (key, value) in filters {
            if let Some(search_key) = config.search_keys.iter().find(|k| k.name == *key) {
                where_clauses.push(format!("\"{}\" = ${}", search_key.column, param_idx));
                params.push(value.clone());
                param_idx += 1;
            }
        }
        
        let where_clause = where_clauses.join(" AND ");
        
        // Build ORDER BY
        let sort_col = sort_field
            .and_then(|f| config.browse_fields.iter().find(|&bf| bf == f))
            .or(config.default_sort.as_ref().map(|s| &s.field))
            .unwrap_or(&config.browse_fields[0]);
        let sort_dir = if sort_order == 1 { "ASC" } else { "DESC" };
        
        // Count query
        let count_sql = format!(
            "SELECT COUNT(*) FROM {} WHERE {}",
            config.source_table, where_clause
        );
        
        // Data query
        let data_sql = format!(
            "SELECT {} FROM {} WHERE {} ORDER BY \"{}\" {} LIMIT {} OFFSET {}",
            select_fields, config.source_table, where_clause, sort_col, sort_dir, limit, offset
        );
        
        // Execute queries
        // ... (implementation with sqlx)
        
        todo!("Implement query execution")
    }
    
    async fn compute_facets(
        &self,
        config: &EntityConfig,
        parent_key: &str,
        parent_value: &str,
    ) -> Result<HashMap<String, Facet>> {
        let mut facets = HashMap::new();
        
        for facet_field in &config.facets {
            let sql = format!(
                "SELECT \"{}\", COUNT(*) as cnt FROM {} WHERE \"{}\" = $1 GROUP BY \"{}\" ORDER BY cnt DESC LIMIT 20",
                facet_field, config.source_table, parent_key, facet_field
            );
            
            // Execute and build facet
            // ... (implementation with sqlx)
        }
        
        Ok(facets)
    }
}
```

### 5.4 Tasks

- [ ] Extend proto with BrowseRequest/BrowseResponse
- [ ] Regenerate Rust code from proto (`make proto`)
- [ ] Add `parent_scope`, `browse_fields`, `default_sort`, `facets` to entity config schema
- [ ] Add investor_holding entity to entity_index.yaml
- [ ] Add service_resource entity to entity_index.yaml
- [ ] Implement BrowseHandler
- [ ] Wire Browse RPC into EntityGateway server
- [ ] Add integration tests for Browse

---

## Part 6: UI - Container Browse Panel

### 6.1 Panel Component

**File:** `crates/ob-poc-ui/src/panels/container_browse.rs` (new file)

This panel reuses patterns from the Entity Resolution Panel.

```rust
//! Container Browse Panel
//!
//! Slide-in panel for browsing container contents (investors, resources).
//! Reuses EntityGateway for search and follows Resolution Panel patterns.

use egui::{Ui, Vec2, Response, ScrollArea, RichText};
use uuid::Uuid;

/// Container browse panel state
pub struct ContainerBrowsePanel {
    /// Is panel open?
    open: bool,
    
    /// Container being browsed
    container_id: Option<Uuid>,
    container_type: Option<String>,
    container_label: Option<String>,
    
    /// EntityGateway nickname for children
    browse_nickname: Option<String>,
    parent_key: Option<String>,
    
    /// Search state
    search_query: String,
    
    /// Filter state
    active_filters: Vec<(String, String)>,
    available_facets: Vec<FacetInfo>,
    
    /// Sort state
    sort_field: String,
    sort_ascending: bool,
    
    /// Pagination state
    offset: i32,
    limit: i32,
    total_count: i64,
    
    /// Results
    items: Vec<BrowseItemView>,
    
    /// Loading state
    loading: bool,
    error: Option<String>,
    
    /// Selected item (for detail view)
    selected_item: Option<usize>,
}

/// View model for a browse item
#[derive(Clone)]
pub struct BrowseItemView {
    pub id: String,
    pub display: String,
    pub sublabel: String,
    pub status: String,
    pub status_color: egui::Color32,
    pub fields: Vec<(String, String)>,
}

/// Facet info for filter dropdowns
pub struct FacetInfo {
    pub field: String,
    pub label: String,
    pub values: Vec<(String, i64)>,
}

impl ContainerBrowsePanel {
    pub fn new() -> Self {
        Self {
            open: false,
            container_id: None,
            container_type: None,
            container_label: None,
            browse_nickname: None,
            parent_key: None,
            search_query: String::new(),
            active_filters: Vec::new(),
            available_facets: Vec::new(),
            sort_field: String::new(),
            sort_ascending: false,
            offset: 0,
            limit: 50,
            total_count: 0,
            items: Vec::new(),
            loading: false,
            error: None,
            selected_item: None,
        }
    }
    
    /// Open panel for a container
    pub fn open_container(
        &mut self,
        container_id: Uuid,
        container_type: &str,
        container_label: &str,
        browse_nickname: &str,
        parent_key: &str,
    ) {
        self.open = true;
        self.container_id = Some(container_id);
        self.container_type = Some(container_type.to_string());
        self.container_label = Some(container_label.to_string());
        self.browse_nickname = Some(browse_nickname.to_string());
        self.parent_key = Some(parent_key.to_string());
        
        // Reset state
        self.search_query.clear();
        self.active_filters.clear();
        self.offset = 0;
        self.items.clear();
        self.selected_item = None;
        self.error = None;
        
        // Trigger initial load
        self.loading = true;
    }
    
    /// Close panel
    pub fn close(&mut self) {
        self.open = false;
        self.container_id = None;
    }
    
    /// Is panel open?
    pub fn is_open(&self) -> bool {
        self.open
    }
    
    /// Render the panel
    pub fn show(&mut self, ctx: &egui::Context) -> Option<ContainerBrowseAction> {
        if !self.open {
            return None;
        }
        
        let mut action = None;
        
        // Side panel (slide-in from right)
        egui::SidePanel::right("container_browse_panel")
            .resizable(true)
            .default_width(400.0)
            .min_width(300.0)
            .max_width(600.0)
            .show(ctx, |ui| {
                action = self.render_panel_content(ui);
            });
        
        action
    }
    
    fn render_panel_content(&mut self, ui: &mut Ui) -> Option<ContainerBrowseAction> {
        let mut action = None;
        
        // Header
        ui.horizontal(|ui| {
            ui.heading(self.container_label.as_deref().unwrap_or("Container"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✕").clicked() {
                    action = Some(ContainerBrowseAction::Close);
                }
            });
        });
        
        ui.label(format!("{} items", self.total_count));
        ui.separator();
        
        // Search bar
        ui.horizontal(|ui| {
            let response = ui.text_edit_singleline(&mut self.search_query);
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                action = Some(ContainerBrowseAction::Search);
            }
            if ui.button("🔍").clicked() {
                action = Some(ContainerBrowseAction::Search);
            }
        });
        
        // Filters
        ui.horizontal(|ui| {
            for facet in &self.available_facets {
                let current = self.active_filters.iter()
                    .find(|(f, _)| f == &facet.field)
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("All");
                
                egui::ComboBox::from_label(&facet.label)
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(current == "All", "All").clicked() {
                            self.active_filters.retain(|(f, _)| f != &facet.field);
                            action = Some(ContainerBrowseAction::Search);
                        }
                        for (value, count) in &facet.values {
                            let label = format!("{} ({})", value, count);
                            if ui.selectable_label(current == value, &label).clicked() {
                                self.active_filters.retain(|(f, _)| f != &facet.field);
                                self.active_filters.push((facet.field.clone(), value.clone()));
                                action = Some(ContainerBrowseAction::Search);
                            }
                        }
                    });
            }
        });
        
        ui.separator();
        
        // Loading indicator
        if self.loading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Loading...");
            });
        }
        
        // Error message
        if let Some(ref error) = self.error {
            ui.colored_label(egui::Color32::RED, error);
        }
        
        // Results list (scrollable)
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (idx, item) in self.items.iter().enumerate() {
                    let is_selected = self.selected_item == Some(idx);
                    
                    let response = ui.selectable_label(is_selected, "");
                    let rect = response.rect;
                    
                    // Custom rendering within the selectable area
                    ui.allocate_ui_at_rect(rect, |ui| {
                        ui.horizontal(|ui| {
                            // Status indicator
                            let status_rect = egui::Rect::from_min_size(
                                rect.min + egui::vec2(4.0, 4.0),
                                egui::vec2(8.0, rect.height() - 8.0),
                            );
                            ui.painter().rect_filled(status_rect, 2.0, item.status_color);
                            
                            ui.add_space(16.0);
                            
                            // Content
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&item.display).strong());
                                ui.label(RichText::new(&item.sublabel).small().weak());
                            });
                        });
                    });
                    
                    if response.clicked() {
                        self.selected_item = Some(idx);
                        action = Some(ContainerBrowseAction::SelectItem(item.id.clone()));
                    }
                    
                    if response.double_clicked() {
                        action = Some(ContainerBrowseAction::OpenItem(item.id.clone()));
                    }
                }
            });
        
        ui.separator();
        
        // Pagination footer
        ui.horizontal(|ui| {
            let page = (self.offset / self.limit) + 1;
            let total_pages = ((self.total_count as i32 + self.limit - 1) / self.limit).max(1);
            
            if ui.add_enabled(self.offset > 0, egui::Button::new("◀ Prev")).clicked() {
                self.offset = (self.offset - self.limit).max(0);
                action = Some(ContainerBrowseAction::Search);
            }
            
            ui.label(format!("Page {} of {}", page, total_pages));
            
            let has_next = self.offset + self.limit < self.total_count as i32;
            if ui.add_enabled(has_next, egui::Button::new("Next ▶")).clicked() {
                self.offset += self.limit;
                action = Some(ContainerBrowseAction::Search);
            }
        });
        
        // Back button
        ui.separator();
        if ui.button("← Back to Graph").clicked() {
            action = Some(ContainerBrowseAction::Close);
        }
        
        action
    }
    
    /// Update with browse response
    pub fn update_results(&mut self, response: BrowseResponseView) {
        self.loading = false;
        self.error = None;
        self.total_count = response.total_count;
        self.items = response.items;
        self.available_facets = response.facets;
    }
    
    /// Set error state
    pub fn set_error(&mut self, error: String) {
        self.loading = false;
        self.error = Some(error);
    }
    
    /// Build request for current state
    pub fn build_request(&self) -> Option<BrowseRequestView> {
        let container_id = self.container_id?;
        let browse_nickname = self.browse_nickname.as_ref()?;
        let parent_key = self.parent_key.as_ref()?;
        
        Some(BrowseRequestView {
            nickname: browse_nickname.clone(),
            parent_key: parent_key.clone(),
            parent_value: container_id.to_string(),
            query: if self.search_query.is_empty() { None } else { Some(self.search_query.clone()) },
            filters: self.active_filters.clone(),
            offset: self.offset,
            limit: self.limit,
            sort_field: if self.sort_field.is_empty() { None } else { Some(self.sort_field.clone()) },
            sort_ascending: self.sort_ascending,
        })
    }
}

/// Actions emitted by the panel
pub enum ContainerBrowseAction {
    /// Close the panel
    Close,
    /// Trigger search with current filters
    Search,
    /// Item selected (single click)
    SelectItem(String),
    /// Item opened (double click)
    OpenItem(String),
}

/// Request view model (for sending to backend)
pub struct BrowseRequestView {
    pub nickname: String,
    pub parent_key: String,
    pub parent_value: String,
    pub query: Option<String>,
    pub filters: Vec<(String, String)>,
    pub offset: i32,
    pub limit: i32,
    pub sort_field: Option<String>,
    pub sort_ascending: bool,
}

/// Response view model (from backend)
pub struct BrowseResponseView {
    pub total_count: i64,
    pub items: Vec<BrowseItemView>,
    pub facets: Vec<FacetInfo>,
}
```

### 6.2 Wire to Graph View

**File:** `crates/ob-poc-ui/src/views/graph_view.rs`

Add container click handling:

```rust
impl GraphView {
    fn handle_node_interaction(&mut self, node: &GraphNode, double_click: bool) {
        if double_click && node.is_container {
            // Open container browse panel
            if let (Some(browse_nickname), Some(parent_key)) = 
                (&node.browse_nickname, &node.parent_key) 
            {
                self.container_panel.open_container(
                    Uuid::parse_str(&node.id).unwrap(),
                    &node.node_type.to_string(),
                    &node.label,
                    browse_nickname,
                    parent_key,
                );
            }
        } else {
            // Existing selection logic
            self.selected_node = Some(node.id.clone());
        }
    }
}
```

### 6.3 Tasks

- [ ] Create `container_browse.rs` panel component
- [ ] Implement search, filter, pagination UI
- [ ] Implement results list with virtual scrolling hints
- [ ] Add status badge coloring
- [ ] Wire panel to graph view
- [ ] Handle double-click on container nodes
- [ ] Connect panel actions to EntityGateway calls
- [ ] Add keyboard navigation (Escape to close)
- [ ] Test with mock data

---

## Part 7: Seed Data for Testing

### 7.1 Test Data Script

**File:** `scripts/seed_container_test_data.sql`

```sql
-- Seed data for container browse testing

-- Create a test fund CBU
INSERT INTO "ob-poc".cbus (cbu_id, name, client_type, jurisdiction, status)
VALUES ('11111111-1111-1111-1111-111111111111', 'Test Growth Fund', 'FUND', 'IE', 'ACTIVE')
ON CONFLICT (cbu_id) DO NOTHING;

-- Create share classes
INSERT INTO "ob-poc".share_classes (share_class_id, cbu_id, share_class_code, share_class_name, currency, share_class_type, status)
VALUES 
  ('22222222-2222-2222-2222-222222222201', '11111111-1111-1111-1111-111111111111', 'A1', 'Institutional USD', 'USD', 'INSTITUTIONAL', 'ACTIVE'),
  ('22222222-2222-2222-2222-222222222202', '11111111-1111-1111-1111-111111111111', 'A2', 'Institutional EUR', 'EUR', 'INSTITUTIONAL', 'ACTIVE'),
  ('22222222-2222-2222-2222-222222222203', '11111111-1111-1111-1111-111111111111', 'R1', 'Retail USD', 'USD', 'RETAIL', 'ACTIVE')
ON CONFLICT DO NOTHING;

-- Create test investor entities
INSERT INTO "ob-poc".entities (entity_id, name, jurisdiction)
SELECT 
  gen_random_uuid(),
  'Investor ' || n,
  (ARRAY['US', 'GB', 'DE', 'SG', 'HK', 'JP', 'AU', 'CH'])[1 + (n % 8)]
FROM generate_series(1, 500) n
ON CONFLICT DO NOTHING;

-- Create investor holdings for first share class
INSERT INTO "ob-poc".investor_holdings (
  share_class_id, 
  entity_id, 
  investor_name,
  units, 
  holding_value, 
  percentage,
  investor_type,
  jurisdiction,
  kyc_status
)
SELECT 
  '22222222-2222-2222-2222-222222222201',
  e.entity_id,
  e.name,
  (random() * 100000)::decimal(20,6),
  (random() * 10000000)::decimal(20,2),
  (random() * 5)::decimal(8,4),
  (ARRAY['INSTITUTIONAL', 'PENSION', 'SOVEREIGN', 'INSURANCE'])[1 + (row_number() over () % 4)],
  e.jurisdiction,
  (ARRAY['VERIFIED', 'PENDING', 'IN_PROGRESS', 'EXPIRED'])[1 + (row_number() over () % 4)]
FROM "ob-poc".entities e
LIMIT 200;

-- Update share class aggregates
UPDATE "ob-poc".share_classes sc
SET 
  investor_count = (SELECT COUNT(*) FROM "ob-poc".investor_holdings ih WHERE ih.share_class_id = sc.share_class_id),
  total_aum = (SELECT COALESCE(SUM(holding_value), 0) FROM "ob-poc".investor_holdings ih WHERE ih.share_class_id = sc.share_class_id);
```

### 7.2 Tasks

- [ ] Create seed script
- [ ] Run seed script against test database
- [ ] Verify data loads correctly
- [ ] Test EntityGateway browse with seeded data

---

## Part 8: Integration Testing

### 8.1 Test Scenarios

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  TEST SCENARIOS                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. BASIC BROWSE                                                            │
│     - Open share class container                                           │
│     - Verify item count matches                                            │
│     - Verify first page loads                                              │
│                                                                             │
│  2. SEARCH                                                                  │
│     - Enter search term                                                    │
│     - Verify results filter                                                │
│     - Verify total count updates                                           │
│                                                                             │
│  3. FILTER                                                                  │
│     - Select jurisdiction filter                                           │
│     - Verify results filter                                                │
│     - Verify facet counts update                                           │
│                                                                             │
│  4. PAGINATION                                                              │
│     - Click next page                                                      │
│     - Verify different results                                             │
│     - Click previous                                                       │
│     - Verify returns to first page                                         │
│                                                                             │
│  5. SORT                                                                    │
│     - Change sort field                                                    │
│     - Verify order changes                                                 │
│     - Toggle ascending/descending                                          │
│                                                                             │
│  6. COMBINED                                                                │
│     - Search + filter + sort                                               │
│     - Verify all applied together                                          │
│     - Paginate through filtered results                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 Tasks

- [ ] Write integration tests for EntityGateway Browse
- [ ] Write UI tests for ContainerBrowsePanel
- [ ] Test with large dataset (10,000+ items)
- [ ] Performance test pagination
- [ ] Test empty results handling
- [ ] Test error handling

---

## Summary

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  IMPLEMENTATION ORDER                                                       │
│                                                                             │
│  1. Part 1: Archive superseded TODOs                                       │
│  2. Part 2: Database schema (share_classes, investor_holdings)             │
│  3. Part 3: GraphNode container fields                                     │
│  4. Part 4: Graph builder container detection                              │
│  5. Part 5: EntityGateway Browse extension                                 │
│  6. Part 7: Seed test data                                                 │
│  7. Part 6: UI panel (can parallelize with 5)                              │
│  8. Part 8: Integration testing                                            │
│                                                                             │
│  ESTIMATED EFFORT: 3-4 days                                                │
│                                                                             │
│  REUSES:                                                                    │
│  • EntityGateway fuzzy search infrastructure                               │
│  • Entity Resolution Panel UI patterns                                     │
│  • Existing proto/gRPC patterns                                            │
│                                                                             │
│  NEW CODE:                                                                  │
│  • Browse RPC in EntityGateway (~300 lines)                               │
│  • ContainerBrowsePanel UI (~400 lines)                                   │
│  • GraphNode container fields (~20 lines)                                 │
│  • DB migrations (~100 lines)                                              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Success Criteria

- [ ] Double-click on ShareClass opens browse panel
- [ ] Panel shows correct total count
- [ ] Search filters results with fuzzy matching
- [ ] Jurisdiction/type filters work
- [ ] Pagination works smoothly
- [ ] Panel closes cleanly (X button, Escape, Back button)
- [ ] No 3D/GPU/wgpu code in codebase
- [ ] Superseded TODOs archived with explanation

---

*Container browsing via EntityGateway - simple, reusable, business-appropriate.*
