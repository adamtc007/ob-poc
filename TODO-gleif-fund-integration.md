@# TODO: GLEIF Fund Relationship Integration

## Status: IMPLEMENTED
## Priority: HIGH
## Created: 2025-01-26
## Completed: 2026-01-26
## Context: Fund entities and IM relationships are now loaded during group research

## Key Decision: Load ALL funds into client_group_entity

**The `client_group_entity` table is the staging area.**

When researching a group like Allianz, we load:
- ✅ Ownership hierarchy (consolidation) → `relationship_category = 'OWNERSHIP'`
- ✅ Managed funds (IM relationship) → `relationship_category = 'INVESTMENT_MANAGEMENT'`

Loading 1000+ funds is fine - the `relationship_category` field enables filtering.
UBO analysis uses `WHERE relationship_category = 'OWNERSHIP'` to exclude IM links.

---

## Problem Statement

When researching a corporate group (e.g., Allianz, BlackRock, AIG, Aviva), the current
`fetch_corporate_tree()` only follows **consolidation relationships** (IS_DIRECTLY_CONSOLIDATED_BY).

This misses a critical dimension: **Investment Management (IM) relationships**.

### What We Load Today
```
Allianz SE
  └── Allianz Asset Management GmbH     (consolidation ✓)
       └── Allianz Global Investors GmbH (consolidation ✓)
            └── [STOPS - no fund relationships loaded]
```

### What We're Missing
```
AGI GmbH (ManCo)
  ├── IS_FUND-MANAGED_BY → Allianz Europa Equity Growth (UCITS)
  ├── IS_FUND-MANAGED_BY → Allianz Global Small Cap (AIF)
  ├── IS_FUND-MANAGED_BY → Allianz Dynamic Multi Asset Plus
  └── ... 500-1000+ funds managed by AGI
       │
       ├── IS_SUBFUND_OF → Allianz Global Investors Fund (Umbrella)
       └── IS_FEEDER_TO → Master Fund structures
```

### Why This Matters

1. **Business Scope**: Understanding what the IM subsidiary actually does
2. **AUM Context**: The funds represent AGI's book of business
3. **Service Relationships**: These are AGI's CLIENTS, not Allianz's subsidiaries
4. **Onboarding Context**: When onboarding a fund, we need to know its ManCo

### Key Insight: IM Links ≠ Ownership

```
┌──────────────────────────────────────────────────────────────────────────┐
│                                                                          │
│   OWNERSHIP (Consolidation)         IM RELATIONSHIP (Service)            │
│   ─────────────────────────         ─────────────────────────            │
│                                                                          │
│   Allianz SE                        External Client (Pension Fund)       │
│     │                                    │                               │
│     └── AGI GmbH ─────────────────────── manages → Client's Fund         │
│         (subsidiary)                     (service provider)              │
│                                                                          │
│   UBO of Allianz: Follow left       Fund's UBO: Follow fund's owners     │
│   AGI's business: Follow right      (NOT Allianz - they just manage it)  │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

**Loading 1000+ funds is NOT a problem** - as long as we tag them correctly:
- `relationship_category: IM_CLIENT` (managed fund)
- `relationship_category: OWNED_ENTITY` (consolidated subsidiary)

---

## GLEIF Fund Relationship Types

GLEIF provides three fund-specific relationship types (since Nov 2020):

| Relationship | Direction | Meaning |
|--------------|-----------|---------|
| `IS_FUND-MANAGED_BY` | Fund → ManCo | Fund is managed by this entity |
| `IS_SUBFUND_OF` | SubFund → Umbrella | Compartment of umbrella structure |
| `IS_FEEDER_TO` | Feeder → Master | Feeder invests exclusively in master |

### Current API Endpoints (already implemented in client.rs)

```rust
// These methods EXIST but aren't called during tree traversal:
get_managed_funds(manager_lei)     // /{lei}/managed-funds
get_umbrella_fund(subfund_lei)     // Via relationships.umbrella_fund
get_fund_manager(fund_lei)         // Via relationships.fund_manager  
get_master_fund(feeder_lei)        // Via relationships.master_fund
```

---

## Staging Flow: client_group_entity

When researching a corporate group, ALL discovered entities flow into `client_group_entity`:

```
Research "Allianz"
    │
    ├── GLEIF corporate tree (consolidation)
    │     └── Inserts with:
    │           membership_type: 'confirmed'
    │           added_by: 'gleif'
    │           relationship_category: 'OWNERSHIP'
    │           is_fund: false
    │
    └── GLEIF managed funds (IM relationships)
          └── Inserts with:
                membership_type: 'confirmed'
                added_by: 'gleif_im'
                relationship_category: 'INVESTMENT_MANAGEMENT'
                is_fund: true
                related_via_lei: '<ManCo LEI>'

Result in client_group_entity:
┌──────────────────────────────────────────────────────────────────────────┐
│ group_id: Allianz                                                        │
├────────────────────────┬────────────────────┬────────────────────────────┤
│ entity                 │ relationship_cat   │ added_by                   │
├────────────────────────┼────────────────────┼────────────────────────────┤
│ Allianz SE             │ OWNERSHIP          │ gleif                      │
│ Allianz Asset Mgmt     │ OWNERSHIP          │ gleif                      │
│ AGI GmbH               │ OWNERSHIP          │ gleif                      │
│ AGI US Holdings        │ OWNERSHIP          │ gleif                      │
│ ...50 more subs...     │ OWNERSHIP          │ gleif                      │
├────────────────────────┼────────────────────┼────────────────────────────┤
│ Allianz Europa Equity  │ INVESTMENT_MGMT    │ gleif_im (via AGI GmbH)    │
│ Allianz Global SmallCap│ INVESTMENT_MGMT    │ gleif_im (via AGI GmbH)    │
│ Client Pension Fund X  │ INVESTMENT_MGMT    │ gleif_im (via AGI GmbH)    │
│ ...800 more funds...   │ INVESTMENT_MGMT    │ gleif_im                   │
└────────────────────────┴────────────────────┴────────────────────────────┘
```

### Query Patterns

```sql
-- Get ONLY ownership hierarchy (for UBO analysis)
SELECT * FROM "ob-poc".client_group_entity
WHERE group_id = :allianz_group_id
  AND relationship_category = 'OWNERSHIP';

-- Get ONLY managed funds (for IM business scope)
SELECT * FROM "ob-poc".client_group_entity  
WHERE group_id = :allianz_group_id
  AND relationship_category = 'INVESTMENT_MANAGEMENT';

-- Get ALL entities in client universe
SELECT * FROM "ob-poc".client_group_entity
WHERE group_id = :allianz_group_id;

-- Get funds managed by a specific ManCo within the group
SELECT * FROM "ob-poc".client_group_entity
WHERE group_id = :allianz_group_id
  AND relationship_category = 'INVESTMENT_MANAGEMENT'
  AND related_via_lei = :agi_lei;
```

---

## Implementation Plan

### Phase 1: Enhanced Relationship Categories

**File: `rust/src/gleif/types.rs`**

```rust
/// Relationship category for filtering and display
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipCategory {
    /// Corporate ownership - consolidation hierarchy
    Ownership,
    /// Investment management - ManCo manages fund
    InvestmentManagement,
    /// Fund structure - umbrella/subfund, master/feeder
    FundStructure,
    /// Branch relationship
    Branch,
    /// Unknown/other
    Unknown(String),
}

impl RelationshipCategory {
    pub fn from_relationship_type(rel_type: &RelationshipType) -> Self {
        match rel_type {
            RelationshipType::IsDirectlyConsolidatedBy |
            RelationshipType::IsUltimatelyConsolidatedBy => Self::Ownership,
            
            RelationshipType::IsFundManagedBy => Self::InvestmentManagement,
            
            RelationshipType::IsSubfundOf |
            RelationshipType::IsFeederTo => Self::FundStructure,
            
            RelationshipType::Unknown(_) => Self::Unknown("UNKNOWN".to_string()),
        }
    }
    
    pub fn is_ownership(&self) -> bool {
        matches!(self, Self::Ownership)
    }
    
    pub fn is_im_relationship(&self) -> bool {
        matches!(self, Self::InvestmentManagement)
    }
}
```

**Update DiscoveredRelationship:**

```rust
pub struct DiscoveredRelationship {
    pub child_lei: String,
    pub parent_lei: String,
    pub relationship_type: String,
    pub relationship_category: RelationshipCategory,  // NEW
    pub is_fund: bool,                                 // NEW - quick filter
}
```

### Phase 2: Enhanced Tree Traversal

**File: `rust/src/gleif/client.rs`**

Add options to `fetch_corporate_tree`:

```rust
#[derive(Debug, Clone, Default)]
pub struct TreeFetchOptions {
    /// Maximum depth for consolidation hierarchy
    pub max_depth: usize,
    
    /// Include funds managed by entities in the tree
    pub include_managed_funds: bool,
    
    /// Include umbrella/subfund relationships
    pub include_fund_structures: bool,
    
    /// Include master/feeder relationships  
    pub include_master_feeder: bool,
    
    /// Filter funds by category (UCITS, AIF, etc.)
    pub fund_type_filter: Option<Vec<String>>,
    
    /// Filter funds by jurisdiction
    pub fund_jurisdiction_filter: Option<Vec<String>>,
    
    /// Maximum funds to load per ManCo (safety limit)
    pub max_funds_per_manco: Option<usize>,
}

impl TreeFetchOptions {
    pub fn ownership_only() -> Self {
        Self {
            max_depth: 10,
            include_managed_funds: false,
            include_fund_structures: false,
            include_master_feeder: false,
            ..Default::default()
        }
    }
    
    pub fn full_with_funds() -> Self {
        Self {
            max_depth: 10,
            include_managed_funds: true,
            include_fund_structures: true,
            include_master_feeder: true,
            max_funds_per_manco: Some(500), // Safety limit
            ..Default::default()
        }
    }
}
```

**Enhanced traversal logic:**

```rust
pub async fn fetch_corporate_tree_v2(
    &self,
    root_lei: &str,
    options: TreeFetchOptions,
) -> Result<CorporateTreeResult> {
    let mut all_records = Vec::new();
    let mut all_relationships = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut queue = vec![(root_lei.to_string(), 0usize, None::<String>)];

    while let Some((lei, depth, parent_lei)) = queue.pop() {
        if visited.contains(&lei) || depth > options.max_depth {
            continue;
        }
        visited.insert(lei.clone());

        // ... existing consolidation logic ...

        match self.get_lei_record(&lei).await {
            Ok(record) => {
                // === NEW: Check if entity manages funds ===
                if options.include_managed_funds {
                    if let Some(ref rels) = record.relationships {
                        if rels.managed_funds.is_some() {
                            match self.get_managed_funds(&lei).await {
                                Ok(funds) => {
                                    let fund_count = funds.len();
                                    tracing::info!(
                                        lei = %lei,
                                        fund_count = fund_count,
                                        "Found ManCo with managed funds"
                                    );
                                    
                                    for (idx, fund) in funds.into_iter().enumerate() {
                                        // Apply safety limit
                                        if let Some(max) = options.max_funds_per_manco {
                                            if idx >= max {
                                                tracing::warn!(
                                                    lei = %lei,
                                                    total = fund_count,
                                                    loaded = max,
                                                    "Truncated fund loading at limit"
                                                );
                                                break;
                                            }
                                        }
                                        
                                        let fund_lei = fund.lei().to_string();
                                        
                                        // Record IM relationship
                                        all_relationships.push(DiscoveredRelationship {
                                            child_lei: fund_lei.clone(),
                                            parent_lei: lei.clone(),
                                            relationship_type: "IS_FUND-MANAGED_BY".to_string(),
                                            relationship_category: RelationshipCategory::InvestmentManagement,
                                            is_fund: true,
                                        });
                                        
                                        // Queue fund for structure discovery (subfunds, feeders)
                                        if options.include_fund_structures && !visited.contains(&fund_lei) {
                                            // Don't increase depth for IM links - parallel dimension
                                            queue.push((fund_lei, depth, None));
                                        }
                                        
                                        all_records.push(fund);
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(lei = %lei, error = %e, "Failed to fetch managed funds");
                                }
                            }
                        }
                    }
                }
                
                // === NEW: Check for fund structure relationships ===
                if options.include_fund_structures && record.is_fund() {
                    if let Some(ref rels) = record.relationships {
                        // Subfund → Umbrella
                        if let Some(ref umbrella) = rels.umbrella_fund {
                            if let Some(ref url) = umbrella.links.related {
                                if let Some(umbrella_lei) = extract_lei_from_url(url) {
                                    all_relationships.push(DiscoveredRelationship {
                                        child_lei: lei.clone(),
                                        parent_lei: umbrella_lei.clone(),
                                        relationship_type: "IS_SUBFUND_OF".to_string(),
                                        relationship_category: RelationshipCategory::FundStructure,
                                        is_fund: true,
                                    });
                                    
                                    if !visited.contains(&umbrella_lei) {
                                        queue.push((umbrella_lei, depth, None));
                                    }
                                }
                            }
                        }
                        
                        // Feeder → Master
                        if options.include_master_feeder {
                            if let Some(ref master) = rels.master_fund {
                                if let Some(ref url) = master.links.related {
                                    if let Some(master_lei) = extract_lei_from_url(url) {
                                        all_relationships.push(DiscoveredRelationship {
                                            child_lei: lei.clone(),
                                            parent_lei: master_lei.clone(),
                                            relationship_type: "IS_FEEDER_TO".to_string(),
                                            relationship_category: RelationshipCategory::FundStructure,
                                            is_fund: true,
                                        });
                                        
                                        if !visited.contains(&master_lei) {
                                            queue.push((master_lei, depth, None));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                all_records.push(record);
            }
            Err(e) => {
                tracing::warn!(lei = %lei, error = %e, "Failed to fetch LEI record");
            }
        }
    }

    Ok(CorporateTreeResult {
        records: all_records,
        relationships: all_relationships,
    })
}
```

### Phase 3: Database Schema Updates

**Key Insight**: The `client_group_entity` table IS the staging area for research.

When loading a corporate group (Allianz), all discovered entities - including 1000+ 
managed funds - go into `client_group_entity` with appropriate classification.

**Migration: `056_fund_relationship_categories.sql`**

```sql
-- ============================================================================
-- Fund Relationship Categories
-- ============================================================================

-- Add relationship category to distinguish ownership vs IM
CREATE TYPE IF NOT EXISTS relationship_category AS ENUM (
    'OWNERSHIP',              -- Consolidation hierarchy
    'INVESTMENT_MANAGEMENT',  -- ManCo manages fund
    'FUND_STRUCTURE',         -- Umbrella/subfund, master/feeder
    'BRANCH',                 -- International branch
    'OTHER'
);

-- ============================================================================
-- Extend client_group_entity with IM classification
-- ============================================================================

-- Add relationship context to client_group_entity
ALTER TABLE "ob-poc".client_group_entity
ADD COLUMN IF NOT EXISTS relationship_category relationship_category DEFAULT 'OWNERSHIP';

ALTER TABLE "ob-poc".client_group_entity
ADD COLUMN IF NOT EXISTS related_via_lei VARCHAR(20);  -- The ManCo/parent LEI that links this entity

ALTER TABLE "ob-poc".client_group_entity
ADD COLUMN IF NOT EXISTS is_fund BOOLEAN DEFAULT FALSE;

-- Update added_by enum options comment
COMMENT ON COLUMN "ob-poc".client_group_entity.added_by IS 
    'manual | discovery | gleif | gleif_im | ownership_trace | user_confirmed';

-- Index for filtering by relationship type
CREATE INDEX IF NOT EXISTS idx_cge_rel_category 
    ON "ob-poc".client_group_entity(group_id, relationship_category);

CREATE INDEX IF NOT EXISTS idx_cge_funds
    ON "ob-poc".client_group_entity(group_id) WHERE is_fund = TRUE;

-- ============================================================================
-- Enhance entity_relationships table for graph edges
-- ============================================================================

ALTER TABLE "ob-poc".entity_relationships 
ADD COLUMN IF NOT EXISTS relationship_category relationship_category;

ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS is_fund_relationship BOOLEAN DEFAULT FALSE;

-- Backfill existing relationships
UPDATE "ob-poc".entity_relationships
SET relationship_category = 'OWNERSHIP',
    is_fund_relationship = FALSE
WHERE relationship_type IN ('IS_DIRECTLY_CONSOLIDATED_BY', 'IS_ULTIMATELY_CONSOLIDATED_BY');

UPDATE "ob-poc".entity_relationships
SET relationship_category = 'INVESTMENT_MANAGEMENT',
    is_fund_relationship = TRUE
WHERE relationship_type = 'IS_FUND-MANAGED_BY';

UPDATE "ob-poc".entity_relationships
SET relationship_category = 'FUND_STRUCTURE',
    is_fund_relationship = TRUE
WHERE relationship_type IN ('IS_SUBFUND_OF', 'IS_FEEDER_TO');

-- ============================================================================
-- Fund-Specific Metadata (supplements client_group_entity for funds)
-- ============================================================================

-- Track fund types when we can infer them
CREATE TABLE IF NOT EXISTS "ob-poc".fund_metadata (
    entity_id UUID PRIMARY KEY REFERENCES "ob-poc".entities(entity_id),
    lei VARCHAR(20) NOT NULL,
    
    -- Fund classification (may be inferred from name, jurisdiction, or external sources)
    fund_structure_type VARCHAR(50),   -- SICAV, ICAV, OEIC, FCP, etc.
    fund_type VARCHAR(50),             -- UCITS, AIF, ETF, etc.
    
    -- Umbrella relationship
    umbrella_lei VARCHAR(20),
    is_umbrella BOOLEAN DEFAULT FALSE,
    
    -- Master-feeder
    master_fund_lei VARCHAR(20),
    is_feeder BOOLEAN DEFAULT FALSE,
    is_master BOOLEAN DEFAULT FALSE,
    
    -- ManCo relationship (denormalized for query speed)
    manco_lei VARCHAR(20),
    manco_name TEXT,
    
    -- Source tracking
    source VARCHAR(50) DEFAULT 'gleif',
    confidence_score DECIMAL(3,2),
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_fund_metadata_manco ON fund_metadata(manco_lei);
CREATE INDEX idx_fund_metadata_umbrella ON fund_metadata(umbrella_lei);
CREATE INDEX idx_fund_metadata_master ON fund_metadata(master_fund_lei);
CREATE INDEX idx_fund_metadata_type ON fund_metadata(fund_type);

-- ============================================================================
-- Views for Fund Analysis
-- ============================================================================

-- View: Funds managed by each ManCo
CREATE OR REPLACE VIEW v_manco_funds AS
SELECT 
    m.manco_lei,
    m.manco_name,
    COUNT(*) as fund_count,
    COUNT(*) FILTER (WHERE m.fund_type = 'UCITS') as ucits_count,
    COUNT(*) FILTER (WHERE m.fund_type = 'AIF') as aif_count,
    COUNT(*) FILTER (WHERE m.is_umbrella) as umbrella_count,
    COUNT(*) FILTER (WHERE m.is_feeder) as feeder_count,
    array_agg(DISTINCT m.fund_type) FILTER (WHERE m.fund_type IS NOT NULL) as fund_types
FROM fund_metadata m
WHERE m.manco_lei IS NOT NULL
GROUP BY m.manco_lei, m.manco_name;

-- View: Entity tree with relationship categories
CREATE OR REPLACE VIEW v_entity_tree_categorized AS
SELECT 
    er.parent_entity_id,
    er.child_entity_id,
    er.relationship_type,
    er.relationship_category,
    er.is_fund_relationship,
    pe.name as parent_name,
    ce.name as child_name,
    ce.entity_category,
    CASE 
        WHEN er.relationship_category = 'OWNERSHIP' THEN 'Corporate Structure'
        WHEN er.relationship_category = 'INVESTMENT_MANAGEMENT' THEN 'IM Client'
        WHEN er.relationship_category = 'FUND_STRUCTURE' THEN 'Fund Structure'
        ELSE 'Other'
    END as display_category
FROM entity_relationships er
JOIN entities pe ON pe.entity_id = er.parent_entity_id
JOIN entities ce ON ce.entity_id = er.child_entity_id;

-- ============================================================================
-- Helper Functions
-- ============================================================================

-- Get all entities in ownership hierarchy (excluding IM relationships)
CREATE OR REPLACE FUNCTION get_ownership_tree(root_lei VARCHAR)
RETURNS TABLE (
    entity_id UUID,
    lei VARCHAR,
    name TEXT,
    depth INT
) AS $$
WITH RECURSIVE ownership AS (
    SELECT e.entity_id, e.lei, e.name, 0 as depth
    FROM entities e
    WHERE e.lei = root_lei
    
    UNION ALL
    
    SELECT e.entity_id, e.lei, e.name, o.depth + 1
    FROM entities e
    JOIN entity_relationships er ON er.child_entity_id = e.entity_id
    JOIN ownership o ON o.entity_id = er.parent_entity_id
    WHERE er.relationship_category = 'OWNERSHIP'
      AND o.depth < 20
)
SELECT * FROM ownership;
$$ LANGUAGE SQL STABLE;

-- Get all funds managed by entities in a corporate group
CREATE OR REPLACE FUNCTION get_managed_funds_for_group(root_lei VARCHAR)
RETURNS TABLE (
    fund_entity_id UUID,
    fund_lei VARCHAR,
    fund_name TEXT,
    manco_lei VARCHAR,
    manco_name TEXT,
    fund_type VARCHAR
) AS $$
SELECT 
    f.entity_id,
    f.lei,
    f.name,
    fm.manco_lei,
    fm.manco_name,
    fm.fund_type
FROM fund_metadata fm
JOIN entities f ON f.entity_id = fm.entity_id
WHERE fm.manco_lei IN (
    SELECT lei FROM get_ownership_tree(root_lei)
);
$$ LANGUAGE SQL STABLE;
```

### Phase 4: DSL Verbs for Fund Discovery

**File: `rust/config/verbs/gleif.yaml`** (additions)

```yaml
gleif:
  funds:
    description: "Discover funds managed by an entity or corporate group"
    args:
      - name: manager
        type: entity_ref
        description: "ManCo LEI or entity reference"
      - name: group
        type: entity_ref
        description: "Root LEI to find all managed funds in group"
        optional: true
      - name: type
        type: string
        description: "Filter by fund type (UCITS, AIF, ETF, etc.)"
        optional: true
      - name: structure
        type: string
        description: "Filter by structure (SICAV, ICAV, etc.)"
        optional: true
      - name: limit
        type: integer
        default: 100
    returns:
      type: record_set
      fields:
        - fund_lei
        - fund_name
        - fund_type
        - manco_lei
        - manco_name
        - is_umbrella
        - is_feeder
    examples:
      - "(gleif.funds :manager \"549300ABC...\" :type ucits)"
      - "(gleif.funds :group @allianz :limit 500)"

  fund-structure:
    description: "Get fund structure details (umbrella, subfunds, master/feeder)"
    args:
      - name: fund
        type: entity_ref
        description: "Fund LEI or entity reference"
    returns:
      type: object
      fields:
        - fund_lei
        - fund_name
        - is_umbrella
        - umbrella_lei
        - subfund_count
        - is_feeder
        - master_fund_lei
        - is_master
        - feeder_count
        - manco_lei
    examples:
      - "(gleif.fund-structure :fund \"549300XYZ...\")"

  tree:
    description: "Fetch corporate tree with optional fund inclusion"
    args:
      - name: root
        type: entity_ref
        description: "Root LEI for tree traversal"
      - name: include-funds
        type: boolean
        default: false
        description: "Include IM-managed funds in tree"
      - name: depth
        type: integer
        default: 10
    returns:
      type: object
      fields:
        - entities
        - relationships
        - ownership_count
        - fund_count
    examples:
      - "(gleif.tree :root \"549300ABC...\")"
      - "(gleif.tree :root @blackrock :include-funds true)"
```

### Phase 5: Graph Visualization Support

**Update graph builder to distinguish relationship types:**

```rust
// In graph/config_driven_builder.rs or similar

pub fn build_edge_style(rel: &DiscoveredRelationship) -> EdgeStyle {
    match rel.relationship_category {
        RelationshipCategory::Ownership => EdgeStyle {
            color: "#2563eb",        // Blue - ownership
            stroke_width: 2.0,
            dash_array: None,        // Solid line
            label: "owns".to_string(),
        },
        RelationshipCategory::InvestmentManagement => EdgeStyle {
            color: "#059669",        // Green - IM relationship  
            stroke_width: 1.5,
            dash_array: Some("5,5"), // Dashed - service relationship
            label: "manages".to_string(),
        },
        RelationshipCategory::FundStructure => EdgeStyle {
            color: "#7c3aed",        // Purple - fund structure
            stroke_width: 1.5,
            dash_array: Some("3,3"), // Dotted
            label: match rel.relationship_type.as_str() {
                "IS_SUBFUND_OF" => "subfund of",
                "IS_FEEDER_TO" => "feeds into",
                _ => "related",
            }.to_string(),
        },
        _ => EdgeStyle::default(),
    }
}
```

---

## What GLEIF Doesn't Cover (Future Enhancement)

GLEIF explicitly **does not** capture Fund-of-Funds structures where a fund invests in 
multiple other funds. This is common in large AM groups:

```
Shared Asset Pool Pattern (NOT IN GLEIF):

  Retail Fund A ──┐
                  │ 25%
  Instit Fund B ──┼─────→ Internal Asset Pool SPV ────→ Underlying Strategies
                  │ 45%         (concentrates expertise)
  Pension Fund C ─┘
                  30%
```

### Future: Extended Fund Investment Model

For capturing fund-of-funds and shared asset pools:

```sql
CREATE TABLE fund_investments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investor_fund_id UUID REFERENCES entities(entity_id),
    target_fund_id UUID REFERENCES entities(entity_id),
    
    -- Investment details (NOT from GLEIF)
    allocation_percentage DECIMAL(5,2),
    nav_amount DECIMAL(18,2),
    allocation_currency CHAR(3),
    
    -- Temporal
    effective_date DATE,
    termination_date DATE,
    
    -- Source (prospectus, manual entry, inferred)
    source VARCHAR(50) NOT NULL,
    source_document_id UUID,
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

This would enable economic lookthrough queries that GLEIF cannot support.

---

## Implementation Status

### ✅ Phase 1: RelationshipCategory Enum (COMPLETE)
- `RelationshipCategory` enum in `rust/src/gleif/types.rs`
- Variants: `Ownership`, `InvestmentManagement`, `FundStructure`, `Branch`, `Unknown`
- `from_relationship_type()` method to derive category from `RelationshipType`
- Helper methods: `is_ownership()`, `is_im_relationship()`, `is_fund_structure()`

### ✅ Phase 2: Enhanced Tree Traversal (COMPLETE)
- `TreeFetchOptions` struct in `rust/src/gleif/client.rs`
- `fetch_corporate_tree_with_options()` with fund expansion
- Supports: managed funds, umbrella/subfund, master/feeder discovery
- Safety limits via `max_funds_per_manco`

### ✅ Phase 3: Database Migration (COMPLETE)
- `migrations/056_fund_relationship_categories.sql`
- `client_group_entity` extended with:
  - `relationship_category` (OWNERSHIP, INVESTMENT_MANAGEMENT, FUND_STRUCTURE)
  - `is_fund` boolean flag
  - `related_via_lei` (ManCo LEI linking fund to group)
- `fund_metadata` table created
- Views: `v_manco_funds`, `v_client_group_entities_categorized`, `v_client_group_category_summary`
- Helper functions: `get_ownership_tree()`, `get_managed_funds_for_group()`

### ✅ Phase 4: DSL Verbs (COMPLETE)
- `gleif.import-to-client-group` verb in `rust/src/domain_ops/gleif_ops.rs`
- Args: `:include-funds`, `:max-funds-per-manco`
- Populates `client_group_entity` with proper `relationship_category`
- Populates `fund_metadata` for fund entities
- Role assignment (FUND, SUBSIDIARY, ULTIMATE_PARENT, etc.)
- YAML definition in `rust/config/verbs/gleif.yaml`

### ⏳ Phase 5: Graph Visualization (FUTURE)
- Edge styling by relationship category not yet implemented
- Could add to graph builder to show ownership vs IM differently

---

## Testing Checklist

### Unit Tests
- [x] RelationshipCategory correctly classifies all relationship types
- [x] TreeFetchOptions presets work correctly
- [ ] Fund metadata extraction from GLEIF records

### Integration Tests
- [ ] fetch_corporate_tree_with_options with include_managed_funds=true
- [ ] Safety limits respected (max_funds_per_manco)
- [ ] Deduplication of relationships discovered from both directions

### Manual Validation (Real GLEIF Data)
- [ ] Research Allianz SE with funds - verify AGI funds loaded
- [ ] Research BlackRock - verify scale handling (2000+ funds)
- [ ] Research Aviva Investors - verify fund structures
- [ ] Verify IM relationships don't pollute UBO discovery

### Performance
- [ ] Measure time for large groups (BlackRock scale)
- [ ] Verify pagination works for 500+ funds per ManCo
- [ ] Database insert performance for bulk fund loading

---

## Notes

- GLEIF has ~142,000 entities with fund relationship structures (Q4 2024)
- Loading 1000+ funds per group is acceptable with proper categorization
- Key is metadata tagging to distinguish "owns" vs "manages"
- IM relationships should NOT affect UBO discovery for the corporate group
- Fund UBO discovery is a separate concern (follows the fund's actual owners)
