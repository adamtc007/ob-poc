# Investor Register Visualization

> **Status:** Implemented
> **Priority:** High - Required for cap table usability at scale
> **Created:** 2026-01-10
> **Completed:** 2026-01-10
> **Estimated Effort:** 45-50 hours
> **Dependencies:** 016-capital-structure-ownership-model.md (schema must exist)

---

## Key Files

| File | Purpose |
|------|---------|
| `rust/src/graph/investor_register.rs` | Server response types |
| `rust/src/api/capital_routes.rs` | API endpoints |
| `rust/crates/ob-poc-types/src/investor_register.rs` | Client types |
| `rust/crates/ob-poc-ui/src/panels/investor_register.rs` | Panel component |
| `rust/crates/ob-poc-ui/src/state.rs` | UI state management |
| `rust/crates/ob-poc-ui/src/app.rs` | Action handling and wiring |

## Design Highlights

1. **Server-side threshold partitioning** - Control holders (>5% or special rights) vs aggregate
2. **Decimal handling** - `rust_decimal::Decimal` server-side, `f64` client-side for JSON
3. **Tier-based coloring** - Control (red), Significant (yellow), Disclosure (blue), SpecialRights (purple)
4. **Expandable breakdown** - By investor type, KYC status, or jurisdiction
5. **Drill-down list** - Paginated with filters for viewing all investors

---

## MANDATORY: Read Before Implementing

**Claude MUST read these documentation sections before writing ANY code:**

```bash
# REQUIRED READING - Execute these view commands first:

# 1. Core patterns and mandatory egui rules
view /Users/adamtc007/Developer/ob-poc/CLAUDE.md

# 2. egui patterns and anti-patterns (CRITICAL)
view /Users/adamtc007/Developer/ob-poc/docs/strategy-patterns.md

# 3. Current graph data structures
view /Users/adamtc007/Developer/ob-poc/rust/crates/ob-poc-graph/src/graph/types.rs

# 4. Current graph rendering patterns
view /Users/adamtc007/Developer/ob-poc/rust/crates/ob-poc-graph/src/graph/render.rs

# 5. Taxonomy panel patterns (fractal navigation)
view /Users/adamtc007/Developer/ob-poc/rust/crates/ob-poc-ui/src/panels/taxonomy.rs

# 6. State management rules
view /Users/adamtc007/Developer/ob-poc/rust/crates/ob-poc-ui/src/state.rs

# 7. UI/viewport interaction patterns
view /Users/adamtc007/Developer/ob-poc/docs/repl-viewport.md

# 8. Entity model and investor register schema
view /Users/adamtc007/Developer/ob-poc/docs/entity-model-ascii.md

# 9. Capital structure TODO (investor holdings)
view /Users/adamtc007/Developer/ob-poc/ai-thoughts/016-capital-structure-ownership-model.md
```

**DO NOT proceed without reading these files. They contain mandatory patterns.**

---

## Problem Statement

The taxonomy graph visualization works for UBO/control chains (5-50 nodes). It breaks completely for economic investor registers with 10,000+ holders.

**The bifurcation:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                  VISUALIZATION SCALING PROBLEM                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   UBO / CONTROL VIEW                      ECONOMIC INVESTOR VIEW            │
│   ─────────────────                       ──────────────────────            │
│                                                                              │
│   Typical count: 5-50 nodes               Typical count: 100 - 100,000+     │
│   Question: "Who controls?"               Question: "Who owns?"             │
│   Relationships: Critical                 Relationships: Irrelevant         │
│   Individual ID: Must see                 Individual ID: Aggregated OK      │
│                                                                              │
│   ┌─────────────┐                         ┌─────────────────────────────┐   │
│   │  WORKS ✓    │                         │        BREAKS ✗             │   │
│   │             │                         │                             │   │
│   │   ┌─┐ ┌─┐   │                         │  ┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐  │   │
│   │   │A│─│B│   │                         │  │ ││ ││ ││ ││ ││ ││ ││ │  │   │
│   │   └┬┘ └─┘   │                         │  └─┘└─┘└─┘└─┘└─┘└─┘└─┘└─┘  │   │
│   │    │        │                         │  ┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐  │   │
│   │   ┌┴┐       │                         │  │ ││ ││ ││ ││ ││ ││ ││ │  │   │
│   │   │C│       │                         │  └─┘└─┘└─┘└─┘└─┘└─┘└─┘└─┘  │   │
│   │   └─┘       │                         │     ... x 10,000 more       │   │
│   │             │                         │                             │   │
│   └─────────────┘                         └─────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Solution: Dual-Mode Visualization with Threshold Collapse

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  ACME FUND LTD - Capital Structure                                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  CONTROL VIEW (Taxonomy Graph)                                       │   │
│  │  Threshold: >5% OR board rights OR special rights                   │   │
│  │                                                                      │   │
│  │       ┌──────────────┐        ┌──────────────┐                      │   │
│  │       │ AllianzGI    │        │ Sequoia      │                      │   │
│  │       │ 35.2% ⚡     │        │ 22.1% 🪑    │                      │   │
│  │       │ VOTES        │        │ 2 board seats│                      │   │
│  │       └──────────────┘        └──────────────┘                      │   │
│  │              │                       │                               │   │
│  │       ┌──────┴───────┐        ┌──────┴───────┐                      │   │
│  │       │ Management   │        │ Founders     │                      │   │
│  │       │ 8.3%         │        │ 12.4%        │                      │   │
│  │       └──────────────┘        └──────────────┘                      │   │
│  │                                                                      │   │
│  │  ┌─────────────────────────────────────────────────────────┐        │   │
│  │  │  📊 AGGREGATE: 4,847 other investors (22.0% economic)   │        │   │
│  │  │     [Click to expand investor breakdown]                │        │   │
│  │  └─────────────────────────────────────────────────────────┘        │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  INVESTOR BREAKDOWN (Expanded Panel - Below Graph)                  │   │
│  │                                                                      │   │
│  │  Summary by Type:                                                   │   │
│  │  ┌──────────────────────────────────────────────────────────────┐  │   │
│  │  │ Type           │ Count │ Units      │ % Econ │ Avg Holding  │  │   │
│  │  ├──────────────────────────────────────────────────────────────┤  │   │
│  │  │ INSTITUTIONAL  │    23 │  1,250,000 │   7.5% │    54,348    │  │   │
│  │  │ PROFESSIONAL   │   184 │    890,000 │   5.3% │     4,837    │  │   │
│  │  │ RETAIL         │ 4,521 │  1,200,000 │   7.2% │       265    │  │   │
│  │  │ NOMINEE        │     8 │    320,000 │   1.9% │    40,000    │  │   │
│  │  └──────────────────────────────────────────────────────────────┘  │   │
│  │                                                                      │   │
│  │  [🔍 Search...] [Filter: Type ▼] [Filter: Status ▼] [📥 Export]     │   │
│  │                                                                      │   │
│  │  Showing: INSTITUTIONAL (23 investors)                      Page 1/1│   │
│  │  ┌──────────────────────────────────────────────────────────────┐  │   │
│  │  │ Name                    │ Units    │ % Econ │ KYC Status    │  │   │
│  │  ├──────────────────────────────────────────────────────────────┤  │   │
│  │  │ BlackRock Fund A        │  450,000 │   2.7% │ ✓ Approved    │  │   │
│  │  │ Vanguard Total Market   │  320,000 │   1.9% │ ✓ Approved    │  │   │
│  │  │ State Street ETF        │  180,000 │   1.1% │ ✓ Approved    │  │   │
│  │  └──────────────────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Threshold Rules

| Condition | Visualization | Node Type |
|-----------|---------------|-----------|
| >disclosure_threshold_pct (default 5%) | Individual taxonomy node | `ControlHolder` |
| Has BOARD_APPOINTMENT right | Individual taxonomy node | `ControlHolder` |
| Has any VETO_* right | Individual taxonomy node | `ControlHolder` |
| Has >significant_threshold_pct (25%) | Individual node + ⚡ | `ControlHolder` |
| Has >control_threshold_pct (50%) | Individual node + ⚡ + control edge | `ControlHolder` |
| Below all thresholds, no rights | Collapsed into aggregate node | `AggregateInvestors` |

---

## Implementation Phases

### Phase 1: Server-Side Response Struct
### Phase 2: API Endpoint
### Phase 3: Client-Side Types (ob-poc-graph)
### Phase 4: State Management (ob-poc-ui)
### Phase 5: Aggregate Node Rendering
### Phase 6: Investor Panel Component
### Phase 7: Interaction Handling
### Phase 8: Testing & Validation
### Phase 9: Documentation

---

## Phase 1: Server-Side Response Struct

**File:** `rust/src/graph/investor_register.rs` (new)

**Claude: Read `docs/entity-model-ascii.md` for investor holdings schema first.**

### 1.1 Core Response Structure

```rust
//! Investor Register Visualization Data
//!
//! Provides response structures for the investor register visualization.
//! Server computes thresholds and returns two components:
//! 1. Control holders (individual taxonomy nodes)
//! 2. Aggregate investors (collapsed node + breakdown data)
//!
//! PATTERN: Server owns thresholds and aggregation logic.
//! Client owns rendering and interaction.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Complete response for investor register visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestorRegisterView {
    /// The issuer entity (fund, company)
    pub issuer: IssuerSummary,
    
    /// Share class being viewed (if specific), or all classes
    pub share_class_filter: Option<Uuid>,
    
    /// As-of date for snapshot
    pub as_of_date: NaiveDate,
    
    /// Thresholds used for this view (from issuer_control_config)
    pub thresholds: ThresholdConfig,
    
    // =========================================================================
    // CONTROL HOLDERS (Individual Taxonomy Nodes)
    // =========================================================================
    
    /// Holders above threshold or with special rights
    /// These become individual nodes in the taxonomy graph
    pub control_holders: Vec<ControlHolderNode>,
    
    // =========================================================================
    // AGGREGATE INVESTORS (Collapsed Node)
    // =========================================================================
    
    /// Summary of all holders below threshold
    /// Becomes single "N other investors" node in taxonomy
    pub aggregate: Option<AggregateInvestorsNode>,
    
    // =========================================================================
    // VISUALIZATION HINTS
    // =========================================================================
    
    /// Total investor count (for UI display)
    pub total_investor_count: i32,
    
    /// Total issued units (denominator)
    pub total_issued_units: Decimal,
    
    /// Whether dilution data is available
    pub has_dilution_data: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuerSummary {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub lei: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    pub disclosure_pct: Decimal,      // Default 5%
    pub material_pct: Decimal,        // Default 10%
    pub significant_pct: Decimal,     // Default 25%
    pub control_pct: Decimal,         // Default 50%
    pub control_basis: String,        // VOTES or ECONOMIC
}
```

### 1.2 Control Holder Node (Individual)

```rust
/// Individual holder displayed as taxonomy node
/// Only for holders above threshold or with special rights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlHolderNode {
    /// Entity ID of the holder
    pub entity_id: Uuid,
    
    /// Display name
    pub name: String,
    
    /// Entity type: PROPER_PERSON, LIMITED_COMPANY, etc.
    pub entity_type: String,
    
    /// Investor classification: INSTITUTIONAL, PROFESSIONAL, RETAIL, etc.
    pub investor_type: Option<String>,
    
    // =========================================================================
    // OWNERSHIP DATA
    // =========================================================================
    
    /// Total units held (across all share classes if not filtered)
    pub units: Decimal,
    
    /// Voting percentage (may differ from economic)
    pub voting_pct: Decimal,
    
    /// Economic percentage
    pub economic_pct: Decimal,
    
    // =========================================================================
    // CONTROL FLAGS (Visualization Hints)
    // =========================================================================
    
    /// Has voting control (>control_threshold)
    pub has_control: bool,
    
    /// Has significant influence (>significant_threshold)
    pub has_significant_influence: bool,
    
    /// Above disclosure threshold
    pub above_disclosure: bool,
    
    // =========================================================================
    // SPECIAL RIGHTS
    // =========================================================================
    
    /// Board appointment rights
    pub board_seats: i32,
    
    /// Veto rights held
    pub veto_rights: Vec<String>,  // ["VETO_MA", "VETO_FUNDRAISE"]
    
    /// Other special rights
    pub other_rights: Vec<String>,
    
    // =========================================================================
    // RENDERING HINTS
    // =========================================================================
    
    /// Why this holder is shown individually (for tooltip)
    pub inclusion_reason: String,  // ">5% voting", "Board rights", etc.
    
    /// KYC status for badge
    pub kyc_status: String,
    
    /// Position in hierarchy (for layout)
    pub hierarchy_depth: i32,
    
    // =========================================================================
    // INSTITUTIONAL LOOK-THROUGH / UBO CHAIN
    // =========================================================================
    // Holders can be institutions (not proper persons). Institutions themselves
    // have ownership structures that need to be navigable.
    
    /// Is this holder a proper person (end of chain) or an institution?
    pub is_terminal: bool,  // true = PROPER_PERSON, false = institution/fund/trust
    
    /// Does this institutional holder have its own UBO structure to explore?
    pub has_ubo_structure: bool,
    
    /// If this entity exists as a CBU in our system, its ID for navigation
    pub cbu_id: Option<Uuid>,
    
    /// Known UBOs of this holder (pre-fetched summary, max 5)
    pub known_ubos: Vec<UboSummary>,
    
    /// Ownership chain depth (how many levels to reach all proper persons)
    pub chain_depth: Option<i32>,
    
    /// UBO discovery status for this holder
    pub ubo_discovery_status: String,  // COMPLETE, PARTIAL, PENDING, NOT_REQUIRED, UNKNOWN
    
    /// For LP/GP structures: number of LPs
    pub lp_count: Option<i32>,
    
    /// For corporate groups: ultimate parent name
    pub ultimate_parent_name: Option<String>,
}

/// Summary of a UBO behind an institutional holder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboSummary {
    /// Entity ID of the UBO
    pub entity_id: Uuid,
    
    /// UBO name
    pub name: String,
    
    /// Entity type (should be PROPER_PERSON for true UBO)
    pub entity_type: String,
    
    /// Effective ownership percentage through the chain
    pub effective_pct: Decimal,
    
    /// How they control: DIRECT, INDIRECT, VOTING_RIGHTS, BOARD_CONTROL
    pub control_type: String,
    
    /// Number of hops in the ownership chain
    pub chain_hops: i32,
}
```

### 1.3 Aggregate Investors Node (Collapsed)

```rust
/// Collapsed node representing all holders below threshold
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateInvestorsNode {
    /// Number of investors in aggregate
    pub investor_count: i32,
    
    /// Total units held by aggregate
    pub total_units: Decimal,
    
    /// Voting percentage of aggregate
    pub voting_pct: Decimal,
    
    /// Economic percentage of aggregate  
    pub economic_pct: Decimal,
    
    // =========================================================================
    // BREAKDOWN DATA (For Drill-Down)
    // =========================================================================
    
    /// Breakdown by investor type
    pub by_type: Vec<AggregateBreakdown>,
    
    /// Breakdown by KYC status
    pub by_kyc_status: Vec<AggregateBreakdown>,
    
    /// Breakdown by jurisdiction (top 10)
    pub by_jurisdiction: Vec<AggregateBreakdown>,
    
    // =========================================================================
    // VISUALIZATION HINTS
    // =========================================================================
    
    /// Whether drill-down is available (false if count > MAX_DRILLDOWN)
    pub can_drill_down: bool,
    
    /// Maximum page size for drill-down
    pub page_size: i32,
    
    /// Label for collapsed node
    pub display_label: String,  // "4,847 other investors (22.0%)"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateBreakdown {
    /// Category key (e.g., "RETAIL", "APPROVED", "LU")
    pub key: String,
    
    /// Display label
    pub label: String,
    
    /// Count of investors in category
    pub count: i32,
    
    /// Total units
    pub units: Decimal,
    
    /// Percentage of total
    pub pct: Decimal,
}
```

### 1.4 Paginated Investor List (For Drill-Down)

```rust
/// Paginated list of investors for drill-down view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestorListResponse {
    /// Current page items
    pub items: Vec<InvestorListItem>,
    
    /// Pagination info
    pub pagination: PaginationInfo,
    
    /// Applied filters
    pub filters: InvestorFilters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestorListItem {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub investor_type: Option<String>,
    pub units: Decimal,
    pub economic_pct: Decimal,
    pub kyc_status: String,
    pub jurisdiction: Option<String>,
    pub acquisition_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    pub page: i32,
    pub page_size: i32,
    pub total_items: i32,
    pub total_pages: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InvestorFilters {
    pub investor_type: Option<String>,
    pub kyc_status: Option<String>,
    pub jurisdiction: Option<String>,
    pub search: Option<String>,
    pub min_units: Option<Decimal>,
}
```

---

## Phase 2: API Endpoints

**File:** `rust/src/api/investor_routes.rs` (new)

**Claude: Follow existing patterns in `rust/src/api/graph_routes.rs`.**

### 2.1 Route Definitions

```rust
//! Investor Register API Routes
//!
//! Provides endpoints for investor register visualization:
//! - GET /api/capital/:issuer_id/investors - Full register view
//! - GET /api/capital/:issuer_id/investors/list - Paginated investor list
//!
//! Session-scoped:
//! - GET /api/session/:id/investors - Uses session's active issuer

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use sqlx::PgPool;
use uuid::Uuid;

pub fn investor_routes() -> Router<AppState> {
    Router::new()
        // Primary view (taxonomy + aggregate)
        .route("/api/capital/:issuer_id/investors", get(get_investor_register))
        
        // Paginated list (for drill-down)
        .route("/api/capital/:issuer_id/investors/list", get(get_investor_list))
        
        // Session-scoped variant
        .route("/api/session/:session_id/investors", get(get_session_investor_register))
}

#[derive(Debug, Deserialize)]
pub struct InvestorRegisterQuery {
    /// Filter to specific share class
    pub share_class_id: Option<Uuid>,
    /// As-of date (defaults to today)
    pub as_of: Option<String>,
    /// Include dilution instruments in view
    pub include_dilution: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct InvestorListQuery {
    /// Page number (1-indexed)
    pub page: Option<i32>,
    /// Page size (default 50, max 200)
    pub page_size: Option<i32>,
    /// Filter by investor type
    pub investor_type: Option<String>,
    /// Filter by KYC status
    pub kyc_status: Option<String>,
    /// Filter by jurisdiction
    pub jurisdiction: Option<String>,
    /// Search by name
    pub search: Option<String>,
    /// Sort field
    pub sort_by: Option<String>,
    /// Sort direction
    pub sort_dir: Option<String>,
}
```

### 2.2 Handler Implementation Pattern

```rust
/// GET /api/capital/:issuer_id/investors
///
/// Returns investor register view with:
/// - Control holders as individual nodes
/// - Aggregate node for remaining investors
/// - Breakdown data for drill-down
pub async fn get_investor_register(
    State(pool): State<PgPool>,
    Path(issuer_id): Path<Uuid>,
    Query(params): Query<InvestorRegisterQuery>,
) -> Result<Json<InvestorRegisterView>, (StatusCode, String)> {
    let as_of = parse_as_of_date(params.as_of.as_deref())?;
    
    // 1. Get issuer info
    let issuer = get_issuer_summary(&pool, issuer_id).await?;
    
    // 2. Get threshold config (from issuer_control_config or defaults)
    let thresholds = get_threshold_config(&pool, issuer_id, as_of).await?;
    
    // 3. Compute control positions
    let positions = compute_holder_positions(&pool, issuer_id, as_of).await?;
    
    // 4. Split into control holders vs aggregate
    let (control_holders, aggregate_investors) = 
        partition_by_threshold(&positions, &thresholds);
    
    // 5. Build control holder nodes with rights
    let control_nodes = build_control_holder_nodes(
        &pool, &control_holders, &thresholds
    ).await?;
    
    // 6. Build aggregate node with breakdowns
    let aggregate_node = if !aggregate_investors.is_empty() {
        Some(build_aggregate_node(&aggregate_investors, &thresholds))
    } else {
        None
    };
    
    Ok(Json(InvestorRegisterView {
        issuer,
        share_class_filter: params.share_class_id,
        as_of_date: as_of,
        thresholds,
        control_holders: control_nodes,
        aggregate: aggregate_node,
        total_investor_count: positions.len() as i32,
        total_issued_units: compute_total_issued(&pool, issuer_id, as_of).await?,
        has_dilution_data: check_dilution_data(&pool, issuer_id).await?,
    }))
}
```

---

## Phase 3: Client-Side Types (ob-poc-graph)

**File:** `rust/crates/ob-poc-graph/src/graph/investor_register.rs` (new)

**Claude: Mirror server types exactly. These come from API, NEVER modified locally.**

### 3.1 Response Types (Deserialized from API)

```rust
//! Client-side types for investor register visualization
//!
//! RULE: These mirror server types exactly. Never modify locally.
//! All data comes from API and is treated as read-only.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Re-export server types (if using shared crate) or duplicate definitions
// See Phase 1 for structure - these should match exactly
```

### 3.2 Layout Types (Client Computed)

```rust
/// Layout-ready control holder for rendering
#[derive(Debug, Clone)]
pub struct LayoutControlHolder {
    /// Source data from server
    pub data: ControlHolderNode,
    
    /// Computed position
    pub pos: egui::Pos2,
    
    /// Computed size (based on importance)
    pub size: egui::Vec2,
    
    /// Whether currently selected
    pub selected: bool,
    
    /// Whether currently hovered
    pub hovered: bool,
}

/// Layout-ready aggregate node
#[derive(Debug, Clone)]
pub struct LayoutAggregateNode {
    /// Source data from server
    pub data: AggregateInvestorsNode,
    
    /// Position (below control holders)
    pub pos: egui::Pos2,
    
    /// Size (wider than control holders)
    pub size: egui::Vec2,
    
    /// Whether expanded (showing breakdown panel)
    pub expanded: bool,
    
    /// Currently active breakdown category
    pub active_breakdown: Option<String>,
}
```

---

## Phase 4: State Management (ob-poc-ui)

**File:** `rust/crates/ob-poc-ui/src/state.rs` (extend)

**Claude: Read state.rs header comments - follow SERVER DATA vs UI-ONLY patterns strictly.**

### 4.1 Add to AppState

```rust
// In AppState struct:

// =========================================================================
// SERVER DATA (fetched via API, NEVER modified locally)
// =========================================================================

/// Investor register view data
pub investor_register: Option<InvestorRegisterView>,

/// Paginated investor list (for drill-down)
pub investor_list: Option<InvestorListResponse>,

// =========================================================================
// UI-ONLY STATE (ephemeral, not persisted)
// =========================================================================

/// Investor panel UI state
pub investor_panel_state: InvestorPanelState,
```

### 4.2 Investor Panel State

```rust
/// UI-only state for investor panel
#[derive(Debug, Clone, Default)]
pub struct InvestorPanelState {
    /// Whether aggregate node is expanded
    pub aggregate_expanded: bool,
    
    /// Currently selected breakdown category
    pub selected_category: Option<String>,
    
    /// Search text buffer (TextBuffer for immediate mode)
    pub search_text: String,
    
    /// Active filters
    pub filters: InvestorFilters,
    
    /// Current page (1-indexed)
    pub current_page: i32,
    
    /// Selected investor for detail view
    pub selected_investor_id: Option<Uuid>,
    
    /// Sort state
    pub sort_by: String,
    pub sort_ascending: bool,
}
```

---

## Phase 5: Aggregate Node Rendering

**File:** `rust/crates/ob-poc-graph/src/graph/render_investor.rs` (new)

**Claude: Read `graph/render.rs` for existing patterns. Follow LOD and camera transformation.**

### 5.1 Aggregate Node Rendering

```rust
//! Investor register node rendering
//!
//! Renders:
//! 1. Control holder nodes (using existing node rendering)
//! 2. Aggregate investor node (special collapsed node)
//!
//! The aggregate node is rendered differently:
//! - Wider rectangle at bottom of graph
//! - Shows investor count and percentage
//! - Click to expand breakdown panel

use egui::{Color32, Pos2, Rect, Stroke, Vec2};

/// Colors for investor visualization
pub struct InvestorColors;

impl InvestorColors {
    /// Aggregate node background
    pub const AGGREGATE_BG: Color32 = Color32::from_rgb(60, 70, 90);
    
    /// Aggregate node border
    pub const AGGREGATE_BORDER: Color32 = Color32::from_rgb(100, 120, 150);
    
    /// Aggregate node when expanded
    pub const AGGREGATE_EXPANDED: Color32 = Color32::from_rgb(70, 85, 110);
    
    /// Control indicator color (⚡)
    pub const CONTROL_INDICATOR: Color32 = Color32::from_rgb(255, 180, 0);
    
    /// Board seat indicator color (🪑)
    pub const BOARD_INDICATOR: Color32 = Color32::from_rgb(180, 100, 255);
    
    /// Category colors for breakdown
    pub fn category_color(key: &str) -> Color32 {
        match key {
            "INSTITUTIONAL" => Color32::from_rgb(100, 150, 255),
            "PROFESSIONAL" => Color32::from_rgb(100, 200, 150),
            "RETAIL" => Color32::from_rgb(200, 150, 100),
            "NOMINEE" => Color32::from_rgb(150, 150, 200),
            _ => Color32::from_rgb(150, 150, 150),
        }
    }
}

/// Render aggregate investor node
pub fn render_aggregate_node(
    painter: &egui::Painter,
    node: &LayoutAggregateNode,
    camera: &Camera2D,
    screen_rect: Rect,
) {
    let screen_pos = camera.world_to_screen(node.pos);
    let screen_size = node.size * camera.zoom;
    
    // Skip if off-screen
    let node_rect = Rect::from_min_size(screen_pos, screen_size);
    if !screen_rect.intersects(node_rect) {
        return;
    }
    
    let bg_color = if node.expanded {
        InvestorColors::AGGREGATE_EXPANDED
    } else {
        InvestorColors::AGGREGATE_BG
    };
    
    // Draw rounded rectangle
    painter.rect(
        node_rect,
        8.0,  // corner radius
        bg_color,
        Stroke::new(2.0, InvestorColors::AGGREGATE_BORDER),
    );
    
    // Draw icon and label
    let label = &node.data.display_label;
    let icon = "📊";
    
    painter.text(
        node_rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{} {}", icon, label),
        egui::FontId::proportional(14.0 * camera.zoom.max(0.5)),
        Color32::WHITE,
    );
    
    // Draw expand indicator
    let expand_text = if node.expanded { "▼" } else { "▶" };
    painter.text(
        Pos2::new(node_rect.right() - 20.0, node_rect.center().y),
        egui::Align2::CENTER_CENTER,
        expand_text,
        egui::FontId::proportional(12.0),
        Color32::from_gray(200),
    );
    
    // If expanded, draw mini breakdown bars
    if node.expanded && camera.zoom > 0.6 {
        render_breakdown_bars(painter, &node.data.by_type, node_rect);
    }
}

/// Render mini breakdown bars inside aggregate node
fn render_breakdown_bars(
    painter: &egui::Painter,
    breakdowns: &[AggregateBreakdown],
    container: Rect,
) {
    let bar_height = 4.0;
    let bar_y = container.bottom() - 15.0;
    let bar_width = container.width() - 20.0;
    let bar_x = container.left() + 10.0;
    
    let mut x_offset = 0.0;
    
    for breakdown in breakdowns {
        let segment_width = bar_width * (breakdown.pct.to_f32().unwrap_or(0.0) / 100.0);
        let color = InvestorColors::category_color(&breakdown.key);
        
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(bar_x + x_offset, bar_y),
                Vec2::new(segment_width, bar_height),
            ),
            2.0,
            color,
        );
        
        x_offset += segment_width;
    }
}

/// Render a control holder node (individual investor above threshold)
/// Handles both terminal (proper person) and institutional (has UBO structure) holders
pub fn render_control_holder_node(
    painter: &egui::Painter,
    node: &LayoutControlHolder,
    camera: &Camera2D,
    screen_rect: Rect,
) -> Option<ControlHolderAction> {
    let screen_pos = camera.world_to_screen(node.pos);
    let screen_size = node.size * camera.zoom;
    
    // Skip if off-screen
    let node_rect = Rect::from_min_size(screen_pos, screen_size);
    if !screen_rect.intersects(node_rect) {
        return None;
    }
    
    let data = &node.data;
    
    // Background color based on holder type
    let bg_color = if data.is_terminal {
        // Proper person - end of chain
        Color32::from_rgb(60, 90, 60)  // Greenish - verified end
    } else if data.has_ubo_structure {
        // Institution with UBO structure to explore
        Color32::from_rgb(70, 70, 100)  // Bluish - drillable
    } else {
        // Institution without known UBO structure
        Color32::from_rgb(90, 70, 60)  // Brownish - needs investigation
    };
    
    // Draw node rectangle
    painter.rect(
        node_rect,
        8.0,
        bg_color,
        Stroke::new(
            if node.selected { 3.0 } else { 1.5 },
            if node.selected { Color32::WHITE } else { Color32::from_gray(120) },
        ),
    );
    
    // === LINE 1: Name + Control Indicators ===
    let mut line1 = data.name.clone();
    if data.has_control {
        line1.push_str(" ⚡");  // Control indicator
    }
    if data.board_seats > 0 {
        line1.push_str(" 🪑");  // Board seats
    }
    
    painter.text(
        Pos2::new(node_rect.left() + 8.0, node_rect.top() + 12.0),
        egui::Align2::LEFT_CENTER,
        &line1,
        egui::FontId::proportional(13.0 * camera.zoom.max(0.6)),
        Color32::WHITE,
    );
    
    // === LINE 2: Percentage + Entity Type ===
    let line2 = format!(
        "{:.1}% {} • {}",
        data.voting_pct,
        if data.voting_pct != data.economic_pct { 
            format!("({:.1}% econ)", data.economic_pct) 
        } else { 
            String::new() 
        },
        data.entity_type
    );
    
    painter.text(
        Pos2::new(node_rect.left() + 8.0, node_rect.top() + 28.0),
        egui::Align2::LEFT_CENTER,
        &line2,
        egui::FontId::proportional(11.0 * camera.zoom.max(0.5)),
        Color32::from_gray(200),
    );
    
    // === LINE 3: UBO Status (for institutions only) ===
    if !data.is_terminal {
        let ubo_line = match data.ubo_discovery_status.as_str() {
            "COMPLETE" => format!(
                "UBOs: {} identified {}",
                data.known_ubos.len(),
                if data.chain_depth.unwrap_or(0) > 1 {
                    format!("({}+ levels)", data.chain_depth.unwrap())
                } else {
                    String::new()
                }
            ),
            "PARTIAL" => format!("UBOs: {} found, discovery incomplete", data.known_ubos.len()),
            "PENDING" => "UBO discovery pending...".to_string(),
            "NOT_REQUIRED" => "UBO: Not required (regulated entity)".to_string(),
            _ => "UBO: Unknown".to_string(),
        };
        
        painter.text(
            Pos2::new(node_rect.left() + 8.0, node_rect.top() + 44.0),
            egui::Align2::LEFT_CENTER,
            &ubo_line,
            egui::FontId::proportional(10.0 * camera.zoom.max(0.5)),
            Color32::from_rgb(150, 180, 220),
        );
        
        // === DRILL-DOWN BUTTON ===
        if data.has_ubo_structure || data.cbu_id.is_some() {
            let button_rect = Rect::from_min_size(
                Pos2::new(node_rect.left() + 8.0, node_rect.bottom() - 22.0),
                Vec2::new(node_rect.width() - 16.0, 18.0),
            );
            
            painter.rect(
                button_rect,
                4.0,
                Color32::from_rgb(50, 80, 120),
                Stroke::new(1.0, Color32::from_rgb(80, 120, 180)),
            );
            
            painter.text(
                button_rect.center(),
                egui::Align2::CENTER_CENTER,
                "🔍 View UBO Chain",
                egui::FontId::proportional(10.0),
                Color32::from_rgb(180, 200, 255),
            );
        }
        
        // === INLINE UBO PREVIEW (if expanded) ===
        // Shows first 3 UBOs directly in the node
        if !data.known_ubos.is_empty() && camera.zoom > 0.8 {
            let preview_y = node_rect.top() + 58.0;
            for (i, ubo) in data.known_ubos.iter().take(3).enumerate() {
                let ubo_text = format!(
                    "└ {} ({:.1}%)",
                    truncate_name(&ubo.name, 20),
                    ubo.effective_pct
                );
                painter.text(
                    Pos2::new(node_rect.left() + 16.0, preview_y + (i as f32 * 12.0)),
                    egui::Align2::LEFT_CENTER,
                    &ubo_text,
                    egui::FontId::proportional(9.0),
                    Color32::from_gray(180),
                );
            }
            if data.known_ubos.len() > 3 {
                painter.text(
                    Pos2::new(node_rect.left() + 16.0, preview_y + 36.0),
                    egui::Align2::LEFT_CENTER,
                    &format!("  +{} more...", data.known_ubos.len() - 3),
                    egui::FontId::proportional(9.0),
                    Color32::from_gray(140),
                );
            }
        }
    } else {
        // Terminal node (proper person) - show verification status
        let status_icon = match data.kyc_status.as_str() {
            "APPROVED" => "✓ Verified",
            "PENDING" => "⏳ Pending",
            _ => "? Unknown",
        };
        painter.text(
            Pos2::new(node_rect.left() + 8.0, node_rect.top() + 44.0),
            egui::Align2::LEFT_CENTER,
            status_icon,
            egui::FontId::proportional(10.0 * camera.zoom.max(0.5)),
            Color32::from_rgb(100, 200, 100),
        );
    }
    
    None  // Action handling done via hit testing
}

fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        format!("{}...", &name[..max_len-3])
    }
}

/// Action from control holder node interaction
#[derive(Debug, Clone)]
pub enum ControlHolderAction {
    /// User clicked the node
    Select { entity_id: Uuid },
    /// User clicked drill-down button
    DrillIntoUbo { entity_id: Uuid, cbu_id: Option<Uuid> },
    /// User hovered (for tooltip)
    Hover { entity_id: Uuid },
}
```

---

## Phase 6: Investor Panel Component

**File:** `rust/crates/ob-poc-ui/src/panels/investor_panel.rs` (new)

**Claude: Read `panels/taxonomy.rs` for panel patterns. Return actions, no callbacks.**

### 6.1 Panel Actions

```rust
//! Investor Panel - Drill-down view for aggregate investors
//!
//! Displayed below the graph when aggregate node is expanded.
//! Provides:
//! - Summary breakdown by type/status/jurisdiction
//! - Paginated investor table
//! - Search and filter
//!
//! PATTERN: Actions returned, not callbacks. State in AppState.

use egui::Ui;
use uuid::Uuid;

/// Actions from investor panel interactions
#[derive(Debug, Clone)]
pub enum InvestorPanelAction {
    /// No action
    None,
    
    /// User clicked a breakdown category
    SelectCategory { category: String },
    
    /// User cleared category filter
    ClearCategory,
    
    /// User changed search text
    Search { text: String },
    
    /// User changed page
    ChangePage { page: i32 },
    
    /// User clicked an investor row
    SelectInvestor { entity_id: Uuid },
    
    /// User wants to view investor in graph
    ViewInGraph { entity_id: Uuid },
    
    /// User changed filter
    ApplyFilter { filters: InvestorFilters },
    
    /// User wants to export list
    Export,
    
    /// User changed sort
    ChangeSort { field: String, ascending: bool },
    
    /// User collapsed the panel
    Collapse,
    
    // =========================================================================
    // INSTITUTIONAL LOOK-THROUGH ACTIONS
    // =========================================================================
    
    /// User wants to drill into institutional holder's UBO structure
    /// Navigates to the institution's CBU graph if available
    DrillIntoUbo { entity_id: Uuid, cbu_id: Option<Uuid> },
    
    /// User wants to see UBO chain inline (expand within current view)
    ExpandUboChain { entity_id: Uuid },
    
    /// User wants to collapse inline UBO chain
    CollapseUboChain { entity_id: Uuid },
    
    /// User clicked on a UBO in the expanded chain
    SelectUbo { ubo_entity_id: Uuid, parent_entity_id: Uuid },
}
```

### 6.2 Panel Rendering

```rust
/// Render the investor panel
pub fn render_investor_panel(
    ui: &mut Ui,
    aggregate: &AggregateInvestorsNode,
    investor_list: Option<&InvestorListResponse>,
    state: &InvestorPanelState,
) -> InvestorPanelAction {
    let mut action = InvestorPanelAction::None;
    
    ui.horizontal(|ui| {
        ui.heading("Investor Breakdown");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("✕").clicked() {
                action = InvestorPanelAction::Collapse;
            }
        });
    });
    
    ui.separator();
    
    // Summary table by type
    ui.label("By Investor Type:");
    egui::Grid::new("investor_type_breakdown")
        .striped(true)
        .show(ui, |ui| {
            ui.label("Type");
            ui.label("Count");
            ui.label("Units");
            ui.label("% Economic");
            ui.end_row();
            
            for breakdown in &aggregate.by_type {
                let selected = state.selected_category.as_ref() == Some(&breakdown.key);
                let response = ui.selectable_label(selected, &breakdown.label);
                if response.clicked() {
                    action = InvestorPanelAction::SelectCategory {
                        category: breakdown.key.clone(),
                    };
                }
                ui.label(format!("{}", breakdown.count));
                ui.label(format!("{:.0}", breakdown.units));
                ui.label(format!("{:.1}%", breakdown.pct));
                ui.end_row();
            }
        });
    
    ui.separator();
    
    // Search and filters
    ui.horizontal(|ui| {
        ui.label("🔍");
        let search_response = ui.text_edit_singleline(&mut state.search_text.clone());
        if search_response.changed() {
            action = InvestorPanelAction::Search {
                text: state.search_text.clone(),
            };
        }
        
        if ui.button("Clear").clicked() {
            action = InvestorPanelAction::ClearCategory;
        }
        
        if ui.button("📥 Export").clicked() {
            action = InvestorPanelAction::Export;
        }
    });
    
    // Investor table
    if let Some(list) = investor_list {
        render_investor_table(ui, list, state, &mut action);
    }
    
    action
}

fn render_investor_table(
    ui: &mut Ui,
    list: &InvestorListResponse,
    state: &InvestorPanelState,
    action: &mut InvestorPanelAction,
) {
    // Header with sort controls
    egui::Grid::new("investor_table")
        .striped(true)
        .show(ui, |ui| {
            // Headers
            for (field, label) in [
                ("name", "Name"),
                ("units", "Units"),
                ("pct", "% Econ"),
                ("kyc_status", "KYC"),
            ] {
                if ui.selectable_label(state.sort_by == field, label).clicked() {
                    *action = InvestorPanelAction::ChangeSort {
                        field: field.to_string(),
                        ascending: if state.sort_by == field {
                            !state.sort_ascending
                        } else {
                            true
                        },
                    };
                }
            }
            ui.end_row();
            
            // Data rows
            for item in &list.items {
                if ui.selectable_label(false, &item.name).clicked() {
                    *action = InvestorPanelAction::SelectInvestor {
                        entity_id: item.entity_id,
                    };
                }
                ui.label(format!("{:.0}", item.units));
                ui.label(format!("{:.2}%", item.economic_pct));
                ui.label(kyc_status_badge(&item.kyc_status));
                ui.end_row();
            }
        });
    
    // Pagination
    ui.horizontal(|ui| {
        let pagination = &list.pagination;
        ui.label(format!(
            "Page {} of {} ({} total)",
            pagination.page, pagination.total_pages, pagination.total_items
        ));
        
        if ui.button("◀").clicked() && pagination.page > 1 {
            *action = InvestorPanelAction::ChangePage {
                page: pagination.page - 1,
            };
        }
        if ui.button("▶").clicked() && pagination.page < pagination.total_pages {
            *action = InvestorPanelAction::ChangePage {
                page: pagination.page + 1,
            };
        }
    });
}

fn kyc_status_badge(status: &str) -> egui::RichText {
    let (text, color) = match status {
        "APPROVED" => ("✓", egui::Color32::GREEN),
        "PENDING" => ("⏳", egui::Color32::YELLOW),
        "REJECTED" => ("✗", egui::Color32::RED),
        "EXPIRED" => ("⚠", egui::Color32::LIGHT_RED),
        _ => ("?", egui::Color32::GRAY),
    };
    egui::RichText::new(text).color(color)
}
```

---

## Phase 7: Interaction Handling

**File:** `rust/crates/ob-poc-ui/src/app.rs` (extend)

**Claude: Follow existing action handling patterns in app.rs.**

### 7.1 Handle Aggregate Node Click

```rust
// In main event loop:

// Handle graph interactions
match graph_action {
    GraphAction::ClickAggregateNode => {
        // Toggle aggregate panel
        state.investor_panel_state.aggregate_expanded = 
            !state.investor_panel_state.aggregate_expanded;
        
        // Fetch investor list if expanding
        if state.investor_panel_state.aggregate_expanded {
            trigger_fetch_investor_list(state, ctx);
        }
    }
    GraphAction::ClickControlHolder { entity_id } => {
        // Navigate to entity detail
        trigger_entity_detail_navigation(state, entity_id, ctx);
    }
    // ... other actions
}
```

### 7.2 Handle Panel Actions

```rust
// Handle investor panel actions
match panel_action {
    InvestorPanelAction::SelectCategory { category } => {
        state.investor_panel_state.selected_category = Some(category.clone());
        state.investor_panel_state.filters.investor_type = Some(category);
        state.investor_panel_state.current_page = 1;
        trigger_fetch_investor_list(state, ctx);
    }
    InvestorPanelAction::ChangePage { page } => {
        state.investor_panel_state.current_page = page;
        trigger_fetch_investor_list(state, ctx);
    }
    InvestorPanelAction::Search { text } => {
        state.investor_panel_state.search_text = text.clone();
        state.investor_panel_state.filters.search = Some(text);
        state.investor_panel_state.current_page = 1;
        trigger_fetch_investor_list(state, ctx);
    }
    InvestorPanelAction::ViewInGraph { entity_id } => {
        // Promote investor to temporary control node
        // This is a client-side view transformation
        promote_to_control_node(state, entity_id);
    }
    InvestorPanelAction::Collapse => {
        state.investor_panel_state.aggregate_expanded = false;
    }
    
    // =========================================================================
    // INSTITUTIONAL LOOK-THROUGH ACTIONS
    // =========================================================================
    
    InvestorPanelAction::DrillIntoUbo { entity_id, cbu_id } => {
        if let Some(cbu_id) = cbu_id {
            // Navigate to the institution's CBU graph
            // This triggers a full scope change
            trigger_navigate_to_cbu(state, cbu_id, ctx);
        } else {
            // No CBU exists - could trigger CBU creation workflow
            // or show "entity not onboarded" message
            state.show_toast("Institution not yet onboarded. Create CBU?");
        }
    }
    InvestorPanelAction::ExpandUboChain { entity_id } => {
        // Track which institutional holders have expanded UBO chains
        state.investor_panel_state.expanded_ubo_chains.insert(entity_id);
        // Optionally fetch more detailed UBO data if not already loaded
        if needs_ubo_detail_fetch(state, entity_id) {
            trigger_fetch_ubo_detail(state, entity_id, ctx);
        }
    }
    InvestorPanelAction::CollapseUboChain { entity_id } => {
        state.investor_panel_state.expanded_ubo_chains.remove(&entity_id);
    }
    InvestorPanelAction::SelectUbo { ubo_entity_id, parent_entity_id } => {
        // Navigate to the UBO entity detail
        // Context: we came from parent_entity_id's UBO chain
        state.selected_entity = Some(ubo_entity_id);
        state.navigation_context = Some(NavigationContext::UboChain {
            from_entity: parent_entity_id,
        });
        trigger_fetch_entity_detail(state, ubo_entity_id, ctx);
    }
    // ... other actions
}
```

### 7.3 State Extensions for UBO Tracking

```rust
// Add to InvestorPanelState:

/// Set of institutional holder entity_ids with expanded UBO chains
pub expanded_ubo_chains: HashSet<Uuid>,

/// Cached UBO detail data (fetched on demand)
pub ubo_detail_cache: HashMap<Uuid, Vec<UboSummary>>,
```

---

## Phase 8: Testing & Validation

**File:** `rust/tests/integration/investor_register_test.rs`

### 8.1 Test Cases

```rust
#[tokio::test]
async fn test_threshold_partitioning() {
    // Setup: Create issuer with mix of large and small holders
    // Verify: Correct split between control_holders and aggregate
}

#[tokio::test]
async fn test_special_rights_inclusion() {
    // Setup: Holder with <5% but has board rights
    // Verify: Appears in control_holders, not aggregate
}

#[tokio::test]
async fn test_aggregate_breakdown_sums() {
    // Setup: Create investors of various types
    // Verify: Sum of breakdowns equals total in aggregate
}

#[tokio::test]
async fn test_pagination() {
    // Setup: 150 investors
    // Verify: Correct pagination with page_size=50
}

#[tokio::test]
async fn test_search_filter() {
    // Setup: Investors with various names
    // Verify: Search returns correct subset
}

#[tokio::test]
async fn test_dilution_basis() {
    // Setup: Issuer with dilution instruments
    // Verify: FULLY_DILUTED basis affects thresholds correctly
}

// =========================================================================
// INSTITUTIONAL HOLDER / UBO CHAIN TESTS
// =========================================================================

#[tokio::test]
async fn test_institutional_holder_marked_non_terminal() {
    // Setup: Create holder with entity_type = LIMITED_COMPANY
    // Verify: is_terminal = false, has_ubo_structure populated
}

#[tokio::test]
async fn test_proper_person_marked_terminal() {
    // Setup: Create holder with entity_type = PROPER_PERSON
    // Verify: is_terminal = true, no UBO fields populated
}

#[tokio::test]
async fn test_institutional_with_known_ubos() {
    // Setup: Institution with CBU that has UBO discovery complete
    // Verify: known_ubos populated, ubo_discovery_status = COMPLETE
}

#[tokio::test]
async fn test_institutional_without_cbu() {
    // Setup: Institution holder without corresponding CBU
    // Verify: cbu_id = None, has_ubo_structure = false (or UNKNOWN)
}

#[tokio::test]
async fn test_ubo_chain_depth_calculation() {
    // Setup: Nested ownership: Fund → HoldCo → SubCo → Person
    // Verify: chain_depth = 3 for Fund's control holder view
}

#[tokio::test]
async fn test_effective_ownership_through_chain() {
    // Setup: Fund owns 50% of HoldCo, HoldCo owns 60% of Person's company
    // Verify: effective_pct for Person = 30% (0.5 * 0.6)
}

#[tokio::test]
async fn test_lp_structure_summary() {
    // Setup: PE Fund with 15 LPs
    // Verify: lp_count = 15, entity displayed with LP indicator
}
```

---

## Phase 9: Documentation Updates

### 9.1 Update CLAUDE.md

Add to mandatory reading:

```markdown
| Investor register visualization | `ai-thoughts/018-investor-register-visualization.md` | Dual-mode cap table display |
```

### 9.2 Update docs/repl-viewport.md

Add section on investor panel layout.

### 9.3 Update docs/entity-model-ascii.md

Add investor register visualization diagram.

---

## Implementation Checklist

### Phase 1: Server-Side Response Struct
- [ ] 1.1 Create `rust/src/graph/investor_register.rs`
- [ ] 1.2 Define `InvestorRegisterView` struct
- [ ] 1.3 Define `ControlHolderNode` struct with UBO fields
- [ ] 1.4 Define `UboSummary` struct
- [ ] 1.5 Define `AggregateInvestorsNode` struct
- [ ] 1.6 Define `InvestorListResponse` struct
- [ ] 1.7 Add to `rust/src/graph/mod.rs` exports

### Phase 2: API Endpoints
- [ ] 2.1 Create `rust/src/api/investor_routes.rs`
- [ ] 2.2 Implement `get_investor_register` handler
- [ ] 2.3 Implement `get_investor_list` handler
- [ ] 2.4 Implement threshold partitioning logic
- [ ] 2.5 Implement aggregate breakdown computation
- [ ] 2.6 Implement UBO lookup for institutional holders
- [ ] 2.7 Implement chain depth calculation
- [ ] 2.8 Register routes in main router

### Phase 3: Client-Side Types (ob-poc-graph)
- [ ] 3.1 Create `rust/crates/ob-poc-graph/src/graph/investor_register.rs`
- [ ] 3.2 Mirror server response types
- [ ] 3.3 Define `LayoutControlHolder` type
- [ ] 3.4 Define `LayoutAggregateNode` type
- [ ] 3.5 Add to crate exports

### Phase 4: State Management (ob-poc-ui)
- [ ] 4.1 Add `investor_register` to AppState
- [ ] 4.2 Add `investor_list` to AppState
- [ ] 4.3 Define `InvestorPanelState` struct with UBO tracking
- [ ] 4.4 Add `expanded_ubo_chains: HashSet<Uuid>`
- [ ] 4.5 Add `ubo_detail_cache: HashMap<Uuid, Vec<UboSummary>>`
- [ ] 4.6 Implement state update handlers
- [ ] 4.7 Add async fetch triggers

### Phase 5: Control Holder + Aggregate Node Rendering
- [ ] 5.1 Create `rust/crates/ob-poc-graph/src/graph/render_investor.rs`
- [ ] 5.2 Define `InvestorColors`
- [ ] 5.3 Implement `render_aggregate_node`
- [ ] 5.4 Implement `render_breakdown_bars`
- [ ] 5.5 Implement `render_control_holder_node` (terminal vs institutional)
- [ ] 5.6 Implement UBO preview rendering in institutional nodes
- [ ] 5.7 Implement "View UBO Chain" button rendering
- [ ] 5.8 Integrate with main graph renderer

### Phase 6: Investor Panel Component
- [ ] 6.1 Create `rust/crates/ob-poc-ui/src/panels/investor_panel.rs`
- [ ] 6.2 Define `InvestorPanelAction` enum with UBO actions
- [ ] 6.3 Define `ControlHolderAction` enum
- [ ] 6.4 Implement `render_investor_panel`
- [ ] 6.5 Implement `render_investor_table`
- [ ] 6.6 Add pagination controls
- [ ] 6.7 Add search and filter controls

### Phase 7: Interaction Handling
- [ ] 7.1 Handle aggregate node click in app.rs
- [ ] 7.2 Handle panel actions in app.rs
- [ ] 7.3 Implement `trigger_fetch_investor_list`
- [ ] 7.4 Implement `promote_to_control_node`
- [ ] 7.5 Handle `DrillIntoUbo` action (navigate to CBU)
- [ ] 7.6 Handle `ExpandUboChain` / `CollapseUboChain` actions
- [ ] 7.7 Handle `SelectUbo` action (entity detail navigation)
- [ ] 7.8 Implement `trigger_fetch_ubo_detail`
- [ ] 7.9 Wire up panel collapse

### Phase 8: Testing
- [ ] 8.1 Test threshold partitioning
- [ ] 8.2 Test special rights inclusion
- [ ] 8.3 Test aggregate breakdown sums
- [ ] 8.4 Test pagination
- [ ] 8.5 Test search and filters
- [ ] 8.6 Test dilution basis
- [ ] 8.7 Test institutional holder marked non-terminal
- [ ] 8.8 Test proper person marked terminal
- [ ] 8.9 Test institutional with known UBOs
- [ ] 8.10 Test UBO chain depth calculation
- [ ] 8.11 Test effective ownership through chain
- [ ] 8.12 Test LP structure summary

### Phase 9: Documentation
- [ ] 9.1 Update CLAUDE.md mandatory reading
- [ ] 9.2 Update docs/repl-viewport.md
- [ ] 9.3 Update docs/entity-model-ascii.md

---

## Estimated Effort by Phase

| Phase | Effort | Dependencies |
|-------|--------|--------------|
| 1. Server Structs | 4h | 016 schema |
| 2. API Endpoints | 7h | Phase 1, existing UBO/CBU queries |
| 3. Client Types | 2h | Phase 1 |
| 4. State Management | 4h | Phase 3 |
| 5. Control + Aggregate Rendering | 7h | Phases 3, 4 |
| 6. Investor Panel | 8h | Phases 4, 5 |
| 7. Interactions | 6h | Phases 5, 6 |
| 8. Testing | 6h | All above |
| 9. Documentation | 2h | All above |
| **Total** | **~46h** | |

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Performance with 100k+ investors | Pagination mandatory, no full list fetch |
| Aggregate breakdown accuracy | SQL SUM validation in tests |
| State sync issues | Follow SERVER DATA pattern strictly |
| Panel layout conflicts | Use collapsible panel pattern |
| UBO chain depth explosion | Limit known_ubos to 5, cap chain_depth display |
| Missing CBU for institution | Graceful fallback, offer "Create CBU" action |
| Circular ownership chains | Detect cycles in chain traversal, mark as CIRCULAR |
| UBO data staleness | Show ubo_discovery_status clearly, offer refresh |

---

## Success Criteria

1. **Control holders render as taxonomy nodes** with ⚡ and 🪑 indicators
2. **Institutional holders show UBO summary** with drill-down button
3. **Terminal holders (proper persons) show as end-of-chain**
4. **Aggregate node shows count + percentage** and is clickable
5. **Breakdown panel shows summary by type** with correct totals
6. **Paginated table supports search** and filter
7. **Clicking "View UBO Chain" navigates to institution's CBU**
8. **UBO preview shows first 3 beneficial owners inline**
9. **Works with 50,000+ investors** without performance issues
10. **Effective ownership calculated correctly through chains**

---

## Implementation Summary (2026-01-10)

### Files Created/Modified

**Server-side (rust/src/):**
- `graph/investor_register.rs` - Server-side response structs with `rust_decimal::Decimal`
- `api/capital_routes.rs` - API endpoints with threshold-based partitioning

**Client-side types (rust/crates/ob-poc-types/src/):**
- `investor_register.rs` - Client-side types with `f64` for JSON compatibility
- `lib.rs` - Added module exports

**UI State (rust/crates/ob-poc-ui/src/):**
- `state.rs` - Added `InvestorRegisterUi`, `investor_register`, `investor_list` fields
- `api.rs` - Added `get_investor_register()` and `get_investor_list()` functions

**UI Panel (rust/crates/ob-poc-ui/src/panels/):**
- `investor_register.rs` - Complete panel implementation with:
  - Control holder cards with tier colors (Control/Significant/Disclosure/SpecialRights)
  - KYC status badges
  - Aggregate section with expandable breakdown
  - Drill-down list with pagination
- `mod.rs` - Added module exports

**App wiring (rust/crates/ob-poc-ui/src/):**
- `app.rs` - Added `handle_investor_register_action()` and panel rendering

### Key Design Decisions

1. **Server owns thresholds** - Client doesn't know threshold values, server partitions
2. **Decimal handling** - `rust_decimal::Decimal` server-side, `f64` client-side via JSON
3. **BigDecimal conversion** - String parsing: `Decimal::from_str(&bd.to_string())`
4. **UI-only state** - `InvestorRegisterUi` holds expand/collapse, filters, pagination
5. **Action pattern** - Panel returns `InvestorRegisterAction`, app handles mutations

### API Endpoints

- `GET /api/capital/:issuer_id/investors` → `InvestorRegisterView`
- `GET /api/capital/:issuer_id/investors/list` → `InvestorListResponse`

### Remaining Work

- Full UBO chain visualization (institutional holder drill-down)
- LP structure summary in aggregate breakdown
- Integration tests with database

