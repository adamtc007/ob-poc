# TODO: CBU Data Model - Container/Silo Architecture

## ⛔ MANDATORY FIRST STEP

**Read these files:**
- `/rust/src/graph/types.rs` - Current graph model
- `/rust/config/ontology/entity_taxonomy.yaml` - Entity type definitions

---

## The Problem

Current data model is **flat**: nodes and edges on a plane.

The 3D visualization needs **depth**: containers that hold things, silos you can dive into.

```
CURRENT MODEL (flat):                    NEEDED MODEL (depth):
                                         
   ○ Fund A                                    ╔═══════════╗
   │                                           ║  Fund A   ║
   ├── ○ ManCo                                 ╠═══════════╣
   │                                           ║ ShareClass║ ← tube opening
   └── ○ Custodian                             ║    A1     ║
                                               ╠═══════════╣
   All nodes at same "altitude"                ║ [12,847]  ║ ← depth indicator
                                               ║ investors ║
                                               ║    ↓↓↓    ║ ← dive in
                                               ╚═══════════╝
```

---

## The "Tube of Sweets" Metaphor

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  3D VIEW: Fund with Share Classes as "Silos"                                │
│  ═══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│                    BIRD'S EYE VIEW (fly-over)                               │
│                                                                             │
│                         ┌─────┐                                             │
│                         │     │                                             │
│              ┌─────┐    │  ● ─┼─── ShareClass A2 (EUR Retail)              │
│              │     │    │     │    3,421 investors                          │
│              │  ●  │    └─────┘                                             │
│              │     │         ┌─────┐                                        │
│              └─────┘         │     │                                        │
│                 │            │  ●  │── ShareClass A3 (GBP Hedged)          │
│                 │            │     │   891 investors                        │
│      ShareClass A1           └─────┘                                        │
│      (USD Institutional)                                                    │
│      12,847 investors                                                       │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│                    DIVE VIEW (into ShareClass A1)                           │
│                                                                             │
│              ┌─────────────────────────────────────────┐                   │
│              │         ShareClass A1                   │ ← opening         │
│              │         USD Institutional               │                    │
│              ├─────────────────────────────────────────┤                   │
│              │  │                                   │  │                    │
│              │  │  ┌────┐ ┌────┐ ┌────┐ ┌────┐    │  │                    │
│              │  │  │Inv1│ │Inv2│ │Inv3│ │Inv4│    │  │ ← visible layer    │
│              │  │  └────┘ └────┘ └────┘ └────┘    │  │                    │
│              │  │                                   │  │                    │
│              │  │  ┌────┐ ┌────┐ ┌────┐ ┌────┐    │  │                    │
│              │  │  │Inv5│ │Inv6│ │Inv7│ │Inv8│    │  │ ← scroll down      │
│              │  │  └────┘ └────┘ └────┘ └────┘    │  │                    │
│              │  │              ...                  │  │                    │
│              │  │         [12,839 more]            │  │                    │
│              │  ▼                                   ▼  │                    │
│              └─────────────────────────────────────────┘                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Container Types in CBU Domain

| Container | Contains | Typical Count | Depth |
|-----------|----------|---------------|-------|
| **Fund** | Share Classes | 1-20 | Shallow |
| **Share Class** | Investors | 100 - 100,000 | Deep silo |
| **Investor Register** | Investor Records | 100 - 100,000 | Deep silo |
| **Document Library** | Documents | 10 - 1,000 | Medium |
| **Ownership Chain** | Ownership Links | 2 - 10 | Shallow |
| **Product** | Services | 1 - 10 | Shallow |
| **Service** | Resources | 1 - 20 | Shallow |
| **Market** | Instruments | 100 - 10,000 | Deep silo |
| **SSI Set** | SSI Records | 10 - 500 | Medium |

---

## Part 1: Enhanced Graph Node (Container Concept)

### 1.1 Add Container Fields to GraphNode

```rust
/// A node in the graph representing an entity, document, or resource
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphNode {
    // ... existing fields ...
    
    // =========================================================================
    // CONTAINER/SILO FIELDS - for 3D depth visualization
    // =========================================================================
    
    /// Is this node a container that holds other items?
    /// Containers render as "silos" / "tubes" in 3D view
    #[serde(default)]
    pub is_container: bool,
    
    /// What type of items does this container hold?
    /// e.g., "investor", "document", "instrument"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains_type: Option<String>,
    
    /// How many items are in this container? (without loading them)
    /// Used to show depth indicator on silo
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_count: Option<i64>,
    
    /// Summary statistics for container contents
    /// e.g., { "total_aum": 1500000000, "countries": 24, "avg_holding": 116500 }
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_summary: Option<ContainerSummary>,
    
    /// Maximum nesting depth from this node (0 = leaf)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<i32>,
    
    /// Can children be loaded lazily? (pagination supported)
    #[serde(default)]
    pub supports_lazy_load: bool,
    
    /// API endpoint for loading children
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children_endpoint: Option<String>,
}

/// Summary statistics for container contents
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerSummary {
    /// Total count of items
    pub count: i64,
    
    /// Count by status/state
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub by_status: HashMap<String, i64>,
    
    /// Count by type (if heterogeneous container)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub by_type: HashMap<String, i64>,
    
    /// Count by jurisdiction/country
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub by_jurisdiction: HashMap<String, i64>,
    
    /// Aggregate numeric values
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub aggregates: HashMap<String, f64>,
    
    /// Date range of contents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_range: Option<DateRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub earliest: chrono::NaiveDate,
    pub latest: chrono::NaiveDate,
}
```

### 1.2 Tasks - GraphNode Enhancement

- [ ] Add `is_container` field
- [ ] Add `contains_type` field
- [ ] Add `child_count` field
- [ ] Add `ContainerSummary` struct
- [ ] Add `max_depth` field
- [ ] Add `supports_lazy_load` field
- [ ] Add `children_endpoint` field
- [ ] Update serialization

---

## Part 2: New Entity Types (Fund Structure)

### 2.1 Share Class Entity

Add to `entity_taxonomy.yaml`:

```yaml
  # ===========================================================================
  # Share Class (fund share class)
  # ===========================================================================
  share_class:
    description: "Fund share class - a specific investor segment within a fund"
    category: fund_structure
    parent_type: null  # Standalone, linked to fund via relationship
    
    db:
      schema: ob-poc
      table: share_classes
      pk: share_class_id
      
    search_keys:
      - column: isin
        unique: true
      - column: share_class_code
        unique: false
      - columns: [fund_entity_id, currency]
        unique: false
        
    container:
      is_container: true
      contains: investor_holding
      summary_query: |
        SELECT 
          COUNT(*) as count,
          SUM(holding_value) as total_aum,
          COUNT(DISTINCT investor_entity_id) as unique_investors,
          COUNT(DISTINCT jurisdiction) as jurisdictions
        FROM investor_holdings
        WHERE share_class_id = $1 AND status = 'ACTIVE'
        
    lifecycle:
      status_column: status
      states:
        - DRAFT
        - PENDING_LAUNCH
        - ACTIVE
        - SOFT_CLOSED
        - HARD_CLOSED
        - TERMINATED
      transitions:
        - from: DRAFT
          to: [PENDING_LAUNCH, TERMINATED]
        - from: PENDING_LAUNCH
          to: [ACTIVE, TERMINATED]
        - from: ACTIVE
          to: [SOFT_CLOSED, HARD_CLOSED, TERMINATED]
        - from: SOFT_CLOSED
          to: [ACTIVE, HARD_CLOSED]
        - from: HARD_CLOSED
          to: [SOFT_CLOSED, TERMINATED]
      initial_state: DRAFT
      
    implicit_create:
      allowed: true
      canonical_verb: fund.create-share-class
      required_args: [fund-id, currency, share-class-code]
```

### 2.2 Investor Holding Entity

```yaml
  # ===========================================================================
  # Investor Holding (position in a share class)
  # ===========================================================================
  investor_holding:
    description: "Investor's holding in a specific share class"
    category: fund_structure
    
    db:
      schema: ob-poc
      table: investor_holdings
      pk: holding_id
      
    search_keys:
      - columns: [share_class_id, investor_entity_id]
        unique: true
      - column: account_number
        unique: true
        
    lifecycle:
      status_column: status
      states:
        - PENDING
        - ACTIVE
        - FROZEN
        - REDEEMED
      transitions:
        - from: PENDING
          to: [ACTIVE, REDEEMED]
        - from: ACTIVE
          to: [FROZEN, REDEEMED]
        - from: FROZEN
          to: [ACTIVE, REDEEMED]
      initial_state: PENDING
      
    implicit_create:
      allowed: true
      canonical_verb: fund.create-holding
      required_args: [share-class-id, investor-id, units]
```

### 2.3 Database Tables

```sql
-- Share Classes table
CREATE TABLE share_classes (
    share_class_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fund_entity_id UUID NOT NULL REFERENCES entities(entity_id),
    share_class_code VARCHAR(50) NOT NULL,
    isin VARCHAR(12),
    currency CHAR(3) NOT NULL,
    share_class_type VARCHAR(50), -- 'institutional', 'retail', 'founder', etc.
    inception_date DATE,
    management_fee_bps INTEGER,
    min_investment NUMERIC(18,2),
    status VARCHAR(20) NOT NULL DEFAULT 'DRAFT',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(fund_entity_id, share_class_code)
);

-- Investor Holdings table (the "contents" of the silo)
CREATE TABLE investor_holdings (
    holding_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    share_class_id UUID NOT NULL REFERENCES share_classes(share_class_id),
    investor_entity_id UUID NOT NULL REFERENCES entities(entity_id),
    account_number VARCHAR(50) NOT NULL UNIQUE,
    units NUMERIC(18,6) NOT NULL,
    holding_value NUMERIC(18,2),
    percentage_of_class NUMERIC(8,4),
    acquisition_date DATE,
    jurisdiction CHAR(2), -- Investor's jurisdiction
    investor_type VARCHAR(50), -- 'institutional', 'retail', 'HNW', etc.
    kyc_status VARCHAR(20),
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(share_class_id, investor_entity_id)
);

-- Index for fast container summary
CREATE INDEX idx_investor_holdings_summary 
    ON investor_holdings(share_class_id, status) 
    INCLUDE (holding_value, jurisdiction);
```

### 2.4 Tasks - Database Schema

- [ ] Create `share_classes` table
- [ ] Create `investor_holdings` table
- [ ] Add container metadata to `entity_taxonomy.yaml`
- [ ] Create summary view for share class investor stats
- [ ] Add migration script

---

## Part 3: Container-Aware Graph Builder

### 3.1 Container Detection

```rust
impl CbuGraphBuilder {
    /// Detect and annotate container nodes
    async fn annotate_containers(&self, graph: &mut CbuGraph, repo: &VisualizationRepository) {
        for node in &mut graph.nodes {
            // Check if this node type is a container
            if let Some(container_config) = self.get_container_config(&node.node_type) {
                node.is_container = true;
                node.contains_type = Some(container_config.contains.clone());
                node.supports_lazy_load = container_config.supports_pagination;
                
                // Build children endpoint
                node.children_endpoint = Some(format!(
                    "/api/{}/{}/children",
                    node.node_type.as_str(),
                    node.id
                ));
                
                // Load summary (without loading all children)
                if let Ok(summary) = repo.get_container_summary(&node.id, &container_config).await {
                    node.child_count = Some(summary.count);
                    node.container_summary = Some(summary);
                }
            }
        }
    }
    
    /// Get container configuration for a node type
    fn get_container_config(&self, node_type: &NodeType) -> Option<ContainerConfig> {
        match node_type {
            NodeType::ShareClass => Some(ContainerConfig {
                contains: "investor_holding".to_string(),
                summary_query: "SELECT COUNT(*) FROM investor_holdings WHERE share_class_id = $1",
                supports_pagination: true,
                default_page_size: 50,
            }),
            NodeType::Market => Some(ContainerConfig {
                contains: "instrument".to_string(),
                summary_query: "SELECT COUNT(*) FROM market_instruments WHERE market_id = $1",
                supports_pagination: true,
                default_page_size: 100,
            }),
            NodeType::DocumentLibrary => Some(ContainerConfig {
                contains: "document".to_string(),
                summary_query: "SELECT COUNT(*) FROM document_catalog WHERE entity_id = $1",
                supports_pagination: true,
                default_page_size: 20,
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContainerConfig {
    pub contains: String,
    pub summary_query: &'static str,
    pub supports_pagination: bool,
    pub default_page_size: usize,
}
```

### 3.2 Container Summary Query

```rust
impl VisualizationRepository {
    /// Get summary statistics for a container without loading all children
    pub async fn get_container_summary(
        &self,
        container_id: &str,
        config: &ContainerConfig,
    ) -> Result<ContainerSummary> {
        match config.contains.as_str() {
            "investor_holding" => self.get_share_class_summary(container_id).await,
            "instrument" => self.get_market_summary(container_id).await,
            "document" => self.get_document_library_summary(container_id).await,
            _ => Ok(ContainerSummary::default()),
        }
    }
    
    async fn get_share_class_summary(&self, share_class_id: &str) -> Result<ContainerSummary> {
        let uuid = Uuid::parse_str(share_class_id)?;
        
        let row = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as count,
                COALESCE(SUM(holding_value), 0) as total_aum,
                COUNT(DISTINCT investor_entity_id) as unique_investors,
                COUNT(DISTINCT jurisdiction) as jurisdictions,
                MIN(acquisition_date) as earliest_date,
                MAX(acquisition_date) as latest_date
            FROM investor_holdings
            WHERE share_class_id = $1 AND status = 'ACTIVE'
            "#,
            uuid
        )
        .fetch_one(&self.pool)
        .await?;
        
        let by_jurisdiction = sqlx::query!(
            r#"
            SELECT jurisdiction, COUNT(*) as cnt
            FROM investor_holdings
            WHERE share_class_id = $1 AND status = 'ACTIVE'
            GROUP BY jurisdiction
            ORDER BY cnt DESC
            LIMIT 10
            "#,
            uuid
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .filter_map(|r| r.jurisdiction.map(|j| (j, r.cnt.unwrap_or(0))))
        .collect();
        
        let by_type = sqlx::query!(
            r#"
            SELECT investor_type, COUNT(*) as cnt
            FROM investor_holdings
            WHERE share_class_id = $1 AND status = 'ACTIVE'
            GROUP BY investor_type
            "#,
            uuid
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .filter_map(|r| r.investor_type.map(|t| (t, r.cnt.unwrap_or(0))))
        .collect();
        
        Ok(ContainerSummary {
            count: row.count.unwrap_or(0),
            by_status: HashMap::new(),
            by_type,
            by_jurisdiction,
            aggregates: [
                ("total_aum".to_string(), row.total_aum.map(|d| d.to_f64().unwrap_or(0.0)).unwrap_or(0.0)),
                ("unique_investors".to_string(), row.unique_investors.unwrap_or(0) as f64),
                ("jurisdictions".to_string(), row.jurisdictions.unwrap_or(0) as f64),
            ].into_iter().collect(),
            date_range: match (row.earliest_date, row.latest_date) {
                (Some(e), Some(l)) => Some(DateRange { earliest: e, latest: l }),
                _ => None,
            },
        })
    }
}
```

### 3.3 Tasks - Graph Builder

- [ ] Add `annotate_containers()` to builder
- [ ] Create `ContainerConfig` registry
- [ ] Implement `get_container_summary()` queries
- [ ] Add container fields to graph output
- [ ] Test with share class → investors

---

## Part 4: Lazy Load API

### 4.1 Children Endpoint

```rust
/// GET /api/share-class/{id}/children?offset=0&limit=50&sort=holding_value&filter=jurisdiction:US
pub async fn get_container_children(
    Path(container_id): Path<Uuid>,
    Query(params): Query<ChildrenParams>,
    State(repo): State<Arc<VisualizationRepository>>,
) -> Result<Json<ChildrenResponse>, ApiError> {
    let children = repo.get_children_paginated(
        &container_id,
        params.offset.unwrap_or(0),
        params.limit.unwrap_or(50),
        params.sort.as_deref(),
        params.filter.as_deref(),
    ).await?;
    
    Ok(Json(children))
}

#[derive(Debug, Deserialize)]
pub struct ChildrenParams {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub sort: Option<String>,
    pub filter: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChildrenResponse {
    pub container_id: Uuid,
    pub total_count: i64,
    pub offset: i64,
    pub limit: i64,
    pub children: Vec<GraphNode>,
    pub has_more: bool,
}
```

### 4.2 Tasks - Lazy Load API

- [ ] Create `/api/{type}/{id}/children` endpoint
- [ ] Support pagination (offset/limit)
- [ ] Support sorting
- [ ] Support filtering
- [ ] Return `GraphNode` format for UI consumption

---

## Part 5: 3D Visualization Extensions

### 5.1 Silo Geometry

```rust
/// 3D silo representation of a container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiloGeometry {
    /// Position of silo opening (top) in 3D space
    pub position: Vec3,
    
    /// Radius of the silo opening
    pub radius: f32,
    
    /// Depth of the silo (based on child_count)
    pub depth: f32,
    
    /// Visual style
    pub style: SiloStyle,
    
    /// Current camera depth (0.0 = above opening, 1.0 = at bottom)
    pub view_depth: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SiloStyle {
    /// Transparent glass tube
    Glass { tint: [f32; 4] },
    /// Solid walls with lit interior
    Solid { wall_color: [f32; 4], interior_color: [f32; 4] },
    /// Wireframe
    Wireframe { color: [f32; 4] },
}

impl SiloGeometry {
    /// Compute silo depth from child count (log scale)
    pub fn depth_from_count(count: i64) -> f32 {
        let base_depth = 50.0;
        let log_factor = (count.max(1) as f32).log10();
        base_depth + log_factor * 30.0  // 50 + 0-150 range
    }
    
    /// Get visible depth range for current view
    pub fn visible_range(&self) -> (f32, f32) {
        let view_height = 100.0; // How much depth is visible at once
        let top = self.view_depth * self.depth;
        let bottom = (top + view_height).min(self.depth);
        (top, bottom)
    }
}
```

### 5.2 Dive/Ascend Controls

```rust
/// Navigation actions for silo exploration
pub enum SiloAction {
    /// Start diving into a silo
    Dive { silo_id: String },
    /// Ascend back out of current silo
    Ascend,
    /// Scroll within silo (virtual scroll through children)
    Scroll { delta: f32 },
    /// Jump to specific depth (e.g., from search result)
    JumpTo { depth: f32, highlight_id: Option<String> },
    /// Exit silo completely (fly back to surface)
    Exit,
}
```

### 5.3 Tasks - 3D Extensions

- [ ] Add `SiloGeometry` struct
- [ ] Add depth calculation from child count
- [ ] Add view depth tracking
- [ ] Define silo visual styles
- [ ] Add dive/ascend action handling
- [ ] Integrate with camera system

---

## Part 6: New NodeTypes

Add to `types.rs`:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    // ... existing types ...
    
    // Fund Structure (containers)
    Fund,           // Fund entity (links to share classes)
    ShareClass,     // Container: holds investor holdings
    InvestorRegister, // Alternative view of share class holdings
    
    // Holdings (container contents)
    InvestorHolding,  // Individual holding record
    
    // Market Structure (containers)
    Market,         // Container: holds instruments
    Instrument,     // Individual tradeable instrument
    
    // Document Structure (containers)
    DocumentLibrary,  // Container: holds documents
    
    // SSI Structure (containers)
    SsiSet,         // Container: holds SSI records
}
```

---

## Summary: What's Missing vs What's Needed

| Aspect | Current State | Needed for 3D Silos |
|--------|---------------|---------------------|
| **Container concept** | ❌ None | ✅ `is_container`, `contains_type` |
| **Child count** | ❌ None | ✅ `child_count` (without loading) |
| **Summary stats** | ❌ None | ✅ `ContainerSummary` struct |
| **Depth info** | ❌ None | ✅ `max_depth`, `SiloGeometry.depth` |
| **Lazy loading** | ❌ None | ✅ `supports_lazy_load`, pagination API |
| **Share Class entity** | ❌ None | ✅ New table + taxonomy entry |
| **Investor Holding** | ❌ None | ✅ New table + taxonomy entry |
| **3D geometry** | ❌ None | ✅ `SiloGeometry` struct |

---

## Implementation Order

1. **Database tables** - `share_classes`, `investor_holdings`
2. **Taxonomy entries** - Add to `entity_taxonomy.yaml`
3. **GraphNode fields** - Container metadata
4. **ContainerSummary** - Summary queries
5. **Builder enhancement** - Detect and annotate containers
6. **Lazy load API** - Children endpoint
7. **3D geometry** - Silo calculations

---

## Success Criteria

- [ ] Share class shows as container with investor count
- [ ] Summary stats load without loading all investors
- [ ] Diving into share class loads first page of investors
- [ ] Scrolling through silo loads more investors
- [ ] 12,000+ investors handled smoothly (pagination)
- [ ] Container depth visually represents child count
- [ ] Ascend/exit returns to surface view

---

*Container architecture for 3D silo visualization.*
