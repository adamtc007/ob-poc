# Phase 2: CBU Control Anchors & ControlSphere Navigation

## Context

Phase 1 establishes the CBU trading view: disambiguation → graph → trading matrix.

Phase 2 adds the **ownership/control dimension** without breaking the CBU container model.

Key insight: CBU is operational (who trades, under what mandate, with which services). UBO/ownership is a separate graph domain. They connect via **control anchors** - entities that exist in both worlds.

---

## Core Concepts

### EdgeDomain Separation

```
OPERATING edges (CBU graph):
  - MANAGES_UNDER_MANDATE (IM → Fund)
  - SERVICES (Custodian/TA/Admin → Fund)
  - EXECUTES_FOR (Broker → Fund)
  - ACCOUNT_OF (Fund → Account)

CONTROL edges (Ownership/Board Control graph):
  - Standard-aligned interest types (see below)
  - Derived: CONTROLS_BOARD (computed rollup)
```

### Standards Alignment

We store **interest types** aligned to BODS/GLEIF/PSC, then **derive** board control.

#### BODS Interest Types (source of truth)
Reference: https://standard.openownership.org/en/0.3.0/schema/reference.html#interest

| BODS Interest | Our Edge Type | Description |
|---------------|---------------|-------------|
| `shareholding` | `HOLDS_SHARES` | Direct/indirect equity stake |
| `voting-rights` | `HOLDS_VOTING_RIGHTS` | May differ from shareholding |
| `appointment-of-board` | `APPOINTS_BOARD` | Can appoint/remove directors |
| `other-influence-or-control` | `EXERCISES_INFLUENCE` | Dominant influence, shadow director |
| `senior-managing-official` | `IS_SENIOR_MANAGER` | Fallback when no UBO identified |
| `settlor-of-trust` | `IS_SETTLOR` | Trust arrangements |
| `trustee-of-trust` | `IS_TRUSTEE` | Trust arrangements |
| `protector-of-trust` | `IS_PROTECTOR` | Trust arrangements |
| `beneficiary-of-trust` | `IS_BENEFICIARY` | Trust arrangements |
| `rights-to-surplus-assets` | `HAS_DISSOLUTION_RIGHTS` | Residual claims |
| `rights-to-profit-or-income` | `HAS_PROFIT_RIGHTS` | Economic interest |

#### GLEIF Relationship Records (for legal entity hierarchy)
Reference: https://www.gleif.org/en/about-lei/common-data-file-format

| GLEIF RR Type | Our Edge Type | Description |
|---------------|---------------|-------------|
| `IS_DIRECTLY_CONSOLIDATED_BY` | `CONSOLIDATED_BY` | Accounting consolidation |
| `IS_ULTIMATELY_CONSOLIDATED_BY` | `ULTIMATELY_CONSOLIDATED_BY` | Ultimate parent |
| `IS_FUND_MANAGED_BY` | `MANAGED_BY` | Fund ↔ ManCo |
| `IS_SUBFUND_OF` | `SUBFUND_OF` | Umbrella structure |
| `IS_FEEDER_TO` | `FEEDS_INTO` | Feeder/master |

#### UK PSC Categories (for thresholds)
Reference: https://www.gov.uk/guidance/people-with-significant-control-pscs

| PSC Category | Mapped To | Threshold |
|--------------|-----------|-----------|
| >25% shares | `HOLDS_SHARES` | `pct > 25` |
| >25% voting | `HOLDS_VOTING_RIGHTS` | `pct > 25` |
| Appoints majority of board | `APPOINTS_BOARD` | `pct > 50` or boolean |
| Significant influence/control | `EXERCISES_INFLUENCE` | boolean |
| Trust with above | Trust edges + above | composite |

#### EU 4AMLD/5AMLD Thresholds
- Standard threshold: >25% ownership OR control
- Enhanced (listed): may differ by jurisdiction
- Fallback: senior management when no UBO found

### Edge Schema with Standard XRefs

```sql
CREATE TABLE control_edges (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_entity_id UUID NOT NULL REFERENCES entities(id),
    to_entity_id UUID NOT NULL REFERENCES entities(id),
    
    -- Our canonical type
    edge_type TEXT NOT NULL,
    
    -- Standard cross-references
    bods_interest_type TEXT,           -- e.g., 'shareholding', 'voting-rights'
    gleif_relationship_type TEXT,      -- e.g., 'IS_DIRECTLY_CONSOLIDATED_BY'
    psc_category TEXT,                 -- e.g., 'ownership-of-shares-25-to-50'
    
    -- Quantitative
    percentage DECIMAL(5,2),           -- NULL if boolean/qualitative
    is_direct BOOLEAN DEFAULT true,    -- direct vs indirect
    
    -- Qualitative flags
    is_beneficial BOOLEAN DEFAULT false,
    is_legal BOOLEAN DEFAULT true,     -- legal vs beneficial ownership
    
    -- Provenance
    source_document_id UUID,
    source_register TEXT,              -- 'gleif', 'uk-psc', 'lux-rbe', 'manual'
    effective_date DATE,
    end_date DATE,
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT valid_edge_type CHECK (edge_type IN (
        -- Ownership/Voting
        'HOLDS_SHARES', 'HOLDS_VOTING_RIGHTS',
        -- Board control
        'APPOINTS_BOARD', 'EXERCISES_INFLUENCE', 'IS_SENIOR_MANAGER',
        -- Trust
        'IS_SETTLOR', 'IS_TRUSTEE', 'IS_PROTECTOR', 'IS_BENEFICIARY',
        -- Economic
        'HAS_DISSOLUTION_RIGHTS', 'HAS_PROFIT_RIGHTS',
        -- GLEIF hierarchy
        'CONSOLIDATED_BY', 'ULTIMATELY_CONSOLIDATED_BY', 
        'MANAGED_BY', 'SUBFUND_OF', 'FEEDS_INTO'
    ))
);

CREATE INDEX idx_control_edges_from ON control_edges(from_entity_id);
CREATE INDEX idx_control_edges_to ON control_edges(to_entity_id);
CREATE INDEX idx_control_edges_type ON control_edges(edge_type);
```

### Derived: Board Control Computation

The key output is: **"Who can appoint/remove the majority of the board?"**

```sql
-- Compute board control for an entity
-- Returns persons/entities with effective board control

WITH RECURSIVE control_chain AS (
    -- Base: direct board appointment rights
    SELECT 
        from_entity_id as controller_id,
        to_entity_id as controlled_id,
        CASE 
            WHEN edge_type = 'APPOINTS_BOARD' THEN COALESCE(percentage, 100)
            WHEN edge_type = 'HOLDS_VOTING_RIGHTS' THEN percentage
            WHEN edge_type = 'HOLDS_SHARES' THEN percentage
            ELSE 0
        END as control_pct,
        1 as depth,
        ARRAY[from_entity_id] as path
    FROM control_edges
    WHERE to_entity_id = $target_entity_id
    
    UNION ALL
    
    -- Recursive: walk up the chain
    SELECT
        ce.from_entity_id,
        cc.controlled_id,
        cc.control_pct * COALESCE(ce.percentage, 100) / 100,
        cc.depth + 1,
        cc.path || ce.from_entity_id
    FROM control_chain cc
    JOIN control_edges ce ON ce.to_entity_id = cc.controller_id
    WHERE cc.depth < 10
    AND NOT ce.from_entity_id = ANY(cc.path)  -- prevent cycles
)
SELECT 
    controller_id,
    e.name as controller_name,
    e.entity_type,
    SUM(control_pct) as total_control_pct,
    BOOL_OR(control_pct > 50) as has_board_control
FROM control_chain cc
JOIN entities e ON e.id = cc.controller_id
WHERE e.entity_type = 'Person'  -- UBOs are persons
GROUP BY controller_id, e.name, e.entity_type
HAVING SUM(control_pct) > 25  -- PSC/4AMLD threshold
ORDER BY total_control_pct DESC;
```

### Derivation Priority

When determining "who controls the board", evaluate in order:

1. **Explicit `APPOINTS_BOARD` edge** → direct answer
2. **>50% `HOLDS_VOTING_RIGHTS`** → controls shareholder votes → controls board
3. **>50% `HOLDS_SHARES`** → presumed voting control unless share class says otherwise
4. **`EXERCISES_INFLUENCE`** → qualitative, requires human review flag
5. **Multiple parties sum to control** → joint/concert party arrangements

---

## Board Controller Derivation (Rules Engine)

The CBU → ControlSphere "portal edge" is a **derived relationship**, not hand-authored.

### Derived Edge Type

```rust
/// Derived edge: CBU's computed board controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardControllerEdge {
    pub cbu_id: Uuid,
    pub controller_entity_id: Option<Uuid>,  // None = no single controller
    pub method: BoardControlMethod,
    pub confidence: ControlConfidence,
    pub score: f32,  // 0.0-1.0
    pub as_of: chrono::NaiveDate,
    pub explanation: BoardControlExplanation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BoardControlMethod {
    /// Rule A: Can appoint/remove majority of directors
    BoardAppointmentRights,
    /// Rule B: >50% voting power → controls board
    VotingRightsMajority,
    /// Rule C: Golden share, GP authority, trustee powers
    SpecialInstrument,
    /// Rule A+B+C combined
    Mixed,
    /// Rule D: No entity meets threshold
    NoSingleController,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ControlConfidence {
    High,    // Direct evidence from authoritative source
    Medium,  // Inferred from voting/ownership with some gaps
    Low,     // Weak signals only (affiliations, incomplete data)
}
```

### Rules Engine (Ordered Priority)

```
┌─────────────────────────────────────────────────────────────────────┐
│ RULE A: Appointment Rights (BEST - deterministic)                   │
│                                                                     │
│ IF appointable_seats(entity) / total_board_seats >= 0.5            │
│ THEN controller = entity, method = BoardAppointmentRights          │
│                                                                     │
│ Evidence: Board seat appointment rights, shareholder agreements,   │
│           ManCo governance docs, articles of association           │
│ Confidence: HIGH if explicit docs, MEDIUM if inferred              │
└─────────────────────────────────────────────────────────────────────┘
                              ↓ fallback
┌─────────────────────────────────────────────────────────────────────┐
│ RULE B: Voting Rights Majority (FALLBACK)                          │
│                                                                     │
│ IF voting_power(entity) >= 0.5                                     │
│ THEN controller = entity, method = VotingRightsMajority            │
│                                                                     │
│ Evidence: Investor register → share class → votes-per-share        │
│ Confidence: HIGH if complete register, MEDIUM if partial           │
└─────────────────────────────────────────────────────────────────────┘
                              ↓ fallback
┌─────────────────────────────────────────────────────────────────────┐
│ RULE C: Special Instruments (OVERRIDE - can supersede A/B)         │
│                                                                     │
│ IF has_golden_share OR has_gp_authority OR has_trustee_control     │
│ THEN controller = entity, method = SpecialInstrument               │
│                                                                     │
│ Evidence: Golden share rights, GP/LP agreements, trust deeds       │
│ Confidence: HIGH if explicit docs                                  │
└─────────────────────────────────────────────────────────────────────┘
                              ↓ fallback
┌─────────────────────────────────────────────────────────────────────┐
│ RULE D: No Single Controller                                       │
│                                                                     │
│ IF no entity meets any threshold above                             │
│ THEN controller = None, method = NoSingleController                │
│                                                                     │
│ Output: Top N candidates with scores, fragmented control flag      │
│ Confidence: N/A                                                    │
└─────────────────────────────────────────────────────────────────────┘
```

### Evidence Sources

| Source | What it provides | Strength |
|--------|------------------|----------|
| **GLEIF/BODS** | Entity normalization, identifiers, org scaffold, current officers | Entity data (not control evidence) |
| **Board composition** | Current directors + their affiliations | WEAK signal (who's on board ≠ who appoints) |
| **Investor register** | Ownership by share class, votes per share, beneficial vs nominee | STRONG for Rule B |
| **Governance docs** | Articles, shareholder agreements, delegation framework | STRONG for Rule A |
| **Special instruments** | Golden share, GP/LP docs, trust deeds | STRONG for Rule C |

### Scoring with Partial Data

When evidence is incomplete, compute weighted score:

```rust
pub struct ControlScore {
    /// From appointment rights coverage (Rule A)
    pub s_appoint: f32,      // 0.0-1.0
    /// From voting power (Rule B)  
    pub s_vote: f32,         // 0.0-1.0
    /// From board member affiliations (weak signal)
    pub s_affiliation: f32,  // 0.0-1.0
    /// From special instruments (Rule C)
    pub s_override: f32,     // 0.0 or 1.0 (binary)
    /// Data completeness penalty
    pub data_coverage: f32,  // 0.0-1.0
}

impl ControlScore {
    pub fn total(&self) -> f32 {
        // Override trumps everything
        if self.s_override > 0.0 {
            return self.s_override;
        }
        // Weighted combination, penalized by data coverage
        let raw = 0.70 * self.s_appoint 
                + 0.25 * self.s_vote 
                + 0.05 * self.s_affiliation;
        raw * self.data_coverage
    }
    
    pub fn confidence(&self) -> ControlConfidence {
        match (self.data_coverage, self.total()) {
            (c, s) if c > 0.8 && s > 0.7 => ControlConfidence::High,
            (c, s) if c > 0.5 && s > 0.5 => ControlConfidence::Medium,
            _ => ControlConfidence::Low,
        }
    }
}
```

### Explanation Payload (Audit Trail)

Store the derivation, not just the answer:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardControlExplanation {
    pub as_of: chrono::NaiveDate,
    pub rule_fired: BoardControlMethod,
    pub candidates: Vec<ControlCandidate>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub data_gaps: Vec<String>,  // What's missing
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCandidate {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub score: ControlScore,
    pub total_score: f32,
    pub why: Vec<String>,  // Human-readable reasons
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub source_type: EvidenceSource,
    pub source_id: String,        // Document ID, register entry ID
    pub description: String,
    pub as_of: Option<chrono::NaiveDate>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EvidenceSource {
    GleifRr,           // GLEIF Relationship Record
    BodsStatement,     // BODS ownership statement
    InvestorRegister,  // Share register entry
    GovernanceDoc,     // Articles, shareholder agreement
    ManualEntry,       // User-entered override
}
```

### Storage

```sql
-- Derived board controller edge (materialized)
CREATE TABLE cbu_board_controller (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus(id) ON DELETE CASCADE,
    controller_entity_id UUID REFERENCES entities(id),  -- NULL = no single controller
    method TEXT NOT NULL,
    confidence TEXT NOT NULL,
    score DECIMAL(3,2) NOT NULL,
    as_of DATE NOT NULL,
    explanation JSONB NOT NULL,  -- BoardControlExplanation
    computed_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(cbu_id)  -- One controller per CBU
);

-- Evidence sources used in derivation
CREATE TABLE board_control_evidence (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_board_controller_id UUID REFERENCES cbu_board_controller(id) ON DELETE CASCADE,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    description TEXT,
    as_of DATE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

### Recomputation Triggers

Recompute `cbu_board_controller` when:
1. Control edges added/modified/removed for entities in the control chain
2. Investor register updated
3. Manual refresh requested
4. Scheduled nightly refresh for stale data

### Control Anchors

A CBU points to 1-3 anchor entities that bridge into the ownership graph:

| Role | Typical Entity | Purpose |
|------|----------------|---------|
| GOVERNANCE | ManCo | Board appointments, delegation framework, regulatory oversight |
| SPONSOR | Parent Group Entity | Group rollup, ultimate parent chain |
| ISSUER | Fund Legal Entity | The thing that's owned, issues shares |

ManCo is usually the primary anchor for Lux fund structures - it's operational (TA, oversight) AND control-adjacent (governance, board).

### ControlSphere

A queryable subgraph of ownership/control edges rooted at an anchor entity. Not a stored container - derived by walking control edges from anchor up to ultimate parent.

---

## Data Model Changes

### 1. Add EdgeDomain to edges

```sql
-- Migration
ALTER TABLE entity_edges 
ADD COLUMN domain TEXT DEFAULT 'operating' 
CHECK (domain IN ('operating', 'control'));

-- Backfill: mark ownership edges as 'control'
UPDATE entity_edges 
SET domain = 'control' 
WHERE edge_type IN (
  'OWNS_EQUITY', 'CONTROLS_VOTES', 'HAS_APPOINTMENT_RIGHTS',
  'IS_DIRECTOR_OF', 'UBO_OF', 'PARENT_OF'
);
```

### 2. CBU Control Anchors table

```sql
CREATE TABLE cbu_control_anchors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus(id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES entities(id),
    anchor_role TEXT NOT NULL CHECK (anchor_role IN ('governance', 'sponsor', 'issuer')),
    display_name TEXT,  -- cached for UI: "Allianz Global Investors GmbH"
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (cbu_id, entity_id, anchor_role)
);

CREATE INDEX idx_cbu_control_anchors_cbu ON cbu_control_anchors(cbu_id);
```

### 3. Types (ob-poc-types)

```rust
// src/control.rs

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Interest types aligned to BODS standard
/// https://standard.openownership.org/en/0.3.0/schema/reference.html#interest
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControlEdgeType {
    // Ownership/Voting
    HoldsShares,
    HoldsVotingRights,
    
    // Board control
    AppointsBoard,
    ExercisesInfluence,
    IsSeniorManager,
    
    // Trust arrangements
    IsSettlor,
    IsTrustee,
    IsProtector,
    IsBeneficiary,
    
    // Economic rights
    HasDissolutionRights,
    HasProfitRights,
    
    // GLEIF hierarchy
    ConsolidatedBy,
    UltimatelyConsolidatedBy,
    ManagedBy,
    SubfundOf,
    FeedsInto,
}

impl ControlEdgeType {
    /// Map to BODS interest type string
    pub fn to_bods_interest(&self) -> Option<&'static str> {
        match self {
            Self::HoldsShares => Some("shareholding"),
            Self::HoldsVotingRights => Some("voting-rights"),
            Self::AppointsBoard => Some("appointment-of-board"),
            Self::ExercisesInfluence => Some("other-influence-or-control"),
            Self::IsSeniorManager => Some("senior-managing-official"),
            Self::IsSettlor => Some("settlor-of-trust"),
            Self::IsTrustee => Some("trustee-of-trust"),
            Self::IsProtector => Some("protector-of-trust"),
            Self::IsBeneficiary => Some("beneficiary-of-trust"),
            Self::HasDissolutionRights => Some("rights-to-surplus-assets-on-dissolution"),
            Self::HasProfitRights => Some("rights-to-profit-or-income"),
            // GLEIF types don't map to BODS
            _ => None,
        }
    }
    
    /// Map to GLEIF relationship type
    pub fn to_gleif_relationship(&self) -> Option<&'static str> {
        match self {
            Self::ConsolidatedBy => Some("IS_DIRECTLY_CONSOLIDATED_BY"),
            Self::UltimatelyConsolidatedBy => Some("IS_ULTIMATELY_CONSOLIDATED_BY"),
            Self::ManagedBy => Some("IS_FUND_MANAGED_BY"),
            Self::SubfundOf => Some("IS_SUBFUND_OF"),
            Self::FeedsInto => Some("IS_FEEDER_TO"),
            _ => None,
        }
    }
}

/// A control/ownership edge with standard xrefs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlEdge {
    pub id: Uuid,
    pub from_entity_id: Uuid,
    pub to_entity_id: Uuid,
    pub edge_type: ControlEdgeType,
    
    // Quantitative
    pub percentage: Option<f32>,
    pub is_direct: bool,
    
    // Standard xrefs (derived from edge_type, but can override)
    pub bods_interest_type: Option<String>,
    pub gleif_relationship_type: Option<String>,
    pub psc_category: Option<String>,
    
    // Provenance
    pub source_register: Option<String>,  // 'gleif', 'uk-psc', 'lux-rbe'
    pub effective_date: Option<String>,
}

/// UK PSC threshold categories
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PscCategory {
    OwnershipOfShares25To50,
    OwnershipOfShares50To75,
    OwnershipOfShares75To100,
    VotingRights25To50,
    VotingRights50To75,
    VotingRights75To100,
    AppointsMajorityOfBoard,
    SignificantInfluenceOrControl,
}

impl PscCategory {
    pub fn from_percentage(edge_type: ControlEdgeType, pct: f32) -> Option<Self> {
        match edge_type {
            ControlEdgeType::HoldsShares => match pct as u8 {
                25..=50 => Some(Self::OwnershipOfShares25To50),
                51..=75 => Some(Self::OwnershipOfShares50To75),
                76..=100 => Some(Self::OwnershipOfShares75To100),
                _ => None,
            },
            ControlEdgeType::HoldsVotingRights => match pct as u8 {
                25..=50 => Some(Self::VotingRights25To50),
                51..=75 => Some(Self::VotingRights50To75),
                76..=100 => Some(Self::VotingRights75To100),
                _ => None,
            },
            ControlEdgeType::AppointsBoard if pct > 50.0 => Some(Self::AppointsMajorityOfBoard),
            ControlEdgeType::ExercisesInfluence => Some(Self::SignificantInfluenceOrControl),
            _ => None,
        }
    }
}

/// Derived board control result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardControlResult {
    pub target_entity_id: Uuid,
    pub controllers: Vec<BoardController>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardController {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub entity_type: String,  // "Person" or "LegalEntity"
    pub total_control_pct: f32,
    pub has_board_majority: bool,
    pub control_path: Vec<ControlPathStep>,
    pub psc_categories: Vec<PscCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPathStep {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub edge_type: ControlEdgeType,
    pub percentage: Option<f32>,
}

/// Role of a control anchor in bridging CBU to ownership graph
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnchorRole {
    BoardGovernance,  // ManCo, board oversight - who controls the board
    Sponsor,          // Parent group, ultimate controller
    Issuer,           // Fund legal entity itself
}

/// A control anchor linking CBU to ownership/control graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlAnchor {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub anchor_role: AnchorRole,
    pub jurisdiction: Option<String>,
}

/// Summary of board control accessible from an anchor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardControlSummary {
    pub anchor: ControlAnchor,
    pub ultimate_controller: Option<EntityRef>,
    pub intermediate_count: u32,
    pub ubo_persons: Vec<UboPersonSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboPersonSummary {
    pub person_id: Uuid,
    pub name: String,
    pub total_control_pct: f32,
    pub psc_categories: Vec<PscCategory>,
    pub control_path: Vec<String>,  // entity names in chain
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    pub id: Uuid,
    pub name: String,
    pub entity_type: String,
}
```

### 4. Extend CbuContext

```rust
// In CbuContext (already exists)
pub struct CbuContext {
    pub cbu: CbuSummary,
    pub participants: Vec<ParticipantSummary>,
    pub service_providers: Vec<ServiceProviderSummary>,
    // ... existing fields
    
    // NEW - Derived board controller (computed, not hand-authored)
    pub board_controller: Option<BoardControllerSummary>,
}

/// Summary of derived board controller for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardControllerSummary {
    pub controller_entity_id: Option<Uuid>,
    pub controller_name: Option<String>,
    pub method: BoardControlMethod,
    pub confidence: ControlConfidence,
    pub score: f32,
    pub as_of: chrono::NaiveDate,
    /// Key evidence points for tooltip
    pub evidence_summary: Vec<String>,
    /// What's missing
    pub data_gaps: Vec<String>,
}
```

### 5. Viewport Focus State

```rust
// In viewport.rs - extend ViewportFocusState enum

pub enum ViewportFocusState {
    // Existing
    None,
    CbuContainer { cbu: CbuRef, enhance_level: u8 },
    EntityNode { entity: EntityRef, container_enhance: u8 },
    InstrumentMatrix { cbu: CbuRef, matrix: MatrixRef, matrix_enhance: u8, container_enhance: u8 },
    
    // NEW - Board control view
    BoardControl {
        /// The anchor entity we entered through
        anchor: EntityRef,
        /// Which CBU we came from (for breadcrumb back)
        source_cbu: Option<CbuRef>,
        /// How many ownership layers to show (0 = anchor only, 1 = +direct owners, etc.)
        enhance_level: u8,
        /// Focus on specific entity within control graph
        focused_entity: Option<EntityRef>,
    },
}
```

---

## API Endpoints

### GET /api/cbu/{id}/control-anchors

Returns control anchors for a CBU.

```json
{
  "cbu_id": "uuid",
  "anchors": [
    {
      "entity_id": "uuid",
      "entity_name": "Allianz Global Investors GmbH",
      "anchor_role": "governance",
      "jurisdiction": "DE"
    },
    {
      "entity_id": "uuid", 
      "entity_name": "Allianz SE",
      "anchor_role": "sponsor",
      "jurisdiction": "DE"
    }
  ]
}
```

### GET /api/control-sphere/{entity_id}?depth=3

Returns ownership/control subgraph rooted at entity, up to N layers.
Edges include BODS/GLEIF xrefs for standards compliance.

```json
{
  "anchor_entity": { "id": "uuid", "name": "...", "type": "LegalEntity" },
  "ultimate_controller": { "id": "uuid", "name": "Allianz SE" },
  "nodes": [
    { "id": "uuid", "name": "...", "type": "LegalEntity", "jurisdiction": "LU" },
    { "id": "uuid", "name": "...", "type": "Person", "is_ubo": true }
  ],
  "edges": [
    { 
      "from": "uuid", 
      "to": "uuid", 
      "type": "HOLDS_SHARES", 
      "percentage": 100.0,
      "bods_interest_type": "shareholding",
      "psc_category": "ownership-of-shares-75-to-100"
    },
    { 
      "from": "uuid", 
      "to": "uuid", 
      "type": "HOLDS_VOTING_RIGHTS", 
      "percentage": 51.0,
      "bods_interest_type": "voting-rights",
      "psc_category": "voting-rights-50-to-75"
    }
  ],
  "board_control_summary": [
    { 
      "person_id": "uuid", 
      "name": "...", 
      "total_control_pct": 25.5,
      "psc_categories": ["ownership-of-shares-25-to-50"],
      "has_board_majority": false
    }
  ]
}
```

### POST /api/cbu/{id}/control-anchors

Set/update control anchors for a CBU.

```json
{
  "anchors": [
    { "entity_id": "uuid", "anchor_role": "governance" },
    { "entity_id": "uuid", "anchor_role": "sponsor" }
  ]
}
```

---

## UI Components

### 1. Control Portal Node (in CBU graph)

Render as special node when CBU has computed board controller:

```rust
// In graph rendering - use derived BoardControllerEdge, not raw anchors
if let Some(board_ctrl) = &cbu_context.board_controller {
    let (label, sublabel) = match &board_ctrl.method {
        BoardControlMethod::NoSingleController => (
            "Board Control →".to_string(),
            "Fragmented (no majority)".to_string(),
        ),
        _ => (
            "Board Control →".to_string(),
            format!(
                "{} ({}% confidence)",
                board_ctrl.controller_name.as_deref().unwrap_or("Unknown"),
                (board_ctrl.score * 100.0) as u8
            ),
        ),
    };
    
    let portal_node = GraphNode {
        id: "control-portal".to_string(),
        node_type: NodeType::ControlPortal,
        label,
        sublabel,
        confidence: board_ctrl.confidence,
        position: calculate_portal_position(),
    };
    nodes.push(portal_node);
}
```

Visual: 
- Hexagon shape
- Color gradient based on confidence: High=green, Medium=amber, Low=red
- Arrow indicator

Tooltip (shows explanation):
```
Board Controller: Allianz Global Investors GmbH
Method: Voting Rights Majority (Rule B)
Confidence: HIGH (92%)

Evidence:
• Investor register: 51% voting rights (as of 2024-12-01)
• GLEIF RR: IS_FUND_MANAGED_BY relationship

Data gaps:
• No articles of association on file
```

Click action:
```rust
ViewportAction::NavigateToBoardControl {
    anchor_entity_id: board_ctrl.controller_entity_id,
    source_cbu: Some(current_cbu.id),
}
```

### 2. Board Control View

New graph rendering mode when `ViewportFocusState::BoardControl`:

- Tree/hierarchy layout (ownership flows up)
- Person nodes at top (UBOs meeting PSC thresholds)
- Legal entity nodes below
- Edge labels show: ownership %, edge type, BODS interest badge
- PSC category badges on nodes (">25% shares", ">50% votes", "Appoints board")
- Highlight path from anchor to UBOs
- "← Back to CBU Trading View" breadcrumb button
- Source register indicators (GLEIF, UK PSC, Lux RBE, manual)

### 3. Enhance in Board Control View

- Level 0: Anchor entity only
- Level 1: + Direct owners/controllers
- Level 2: + Their owners (grandparent level)
- Level 3+: Full chain to ultimate controller

### 4. Esper HUD in Board Control View

```
┌─────────────────────────────────────────────┐
│ BOARD CONTROL                               │
│ Anchor: Allianz Global Investors GmbH [DE]  │
│ Ultimate Controller: Allianz SE             │
│ UBOs: 3 persons meeting PSC thresholds      │
│ ← CBU: Allianz Lux Fund 1                   │
└─────────────────────────────────────────────┘
```

---

## DSL Extensions

### Set control anchors via DSL

```yaml
cbu:
  name: "Allianz Luxembourg Fund 1"
  fund_entity: { lei: "529900..." }
  control_anchors:
    - role: governance
      entity: { lei: "529900MANCO..." }  # ManCo
    - role: sponsor  
      entity: { lei: "529900ALLIANZ..." }  # Allianz SE
```

### Query control sphere

```
SHOW CONTROL FOR CBU "Allianz Lux Fund 1"
SHOW OWNERSHIP CHAIN FOR ENTITY "Allianz Global Investors GmbH" DEPTH 5
```

---

## Implementation Order

### Phase 2a: Data Model (4h)
1. Migration: create `control_edges` table with BODS/GLEIF xrefs
2. Create `cbu_board_controller` table (derived edge storage)
3. Create `board_control_evidence` table
4. Add `control.rs` to ob-poc-types with all edge/derivation types

### Phase 2b: Rules Engine Service (8h)
1. Implement Rule A: appointment rights detection
2. Implement Rule B: voting power computation from investor register
3. Implement Rule C: special instrument detection  
4. Implement Rule D: no single controller fallback
5. Scoring with partial data + confidence levels
6. Explanation payload generation
7. Recomputation triggers (on edge change, manual, scheduled)

### Phase 2c: API (4h)
1. GET /api/cbu/{id}/board-controller (returns derived edge + explanation)
2. GET /api/control-sphere/{entity_id}?depth=N
3. POST /api/cbu/{id}/board-controller/recompute (force refresh)
4. Update session context to include board_controller

### Phase 2d: Portal Node (4h)
1. Add NodeType::ControlPortal to graph types
2. Render portal with confidence color coding
3. Tooltip shows explanation + evidence + gaps
4. Wire click → ViewportFocusState::BoardControl

### Phase 2e: Board Control View (8h)
1. New graph layout for ownership trees (hierarchical, flows up)
2. Fetch control-sphere data on focus change
3. Render control edges with BODS interest labels + %
4. PSC category badges on person nodes
5. Highlight the path that fired the winning rule
6. Evidence panel showing source docs
7. Breadcrumb back to source CBU
8. Esper HUD for board control

### Phase 2f: DSL (4h)
1. SHOW BOARD CONTROLLER FOR CBU "..." (shows derivation)
2. RECOMPUTE BOARD CONTROLLER FOR CBU "..."
3. Import from GLEIF/PSC register formats
4. Manual override: SET BOARD CONTROLLER FOR CBU "..." TO ENTITY "..."

---

## Success Criteria

- [ ] CBU computes board controller via rules engine (A→B→C→D)
- [ ] Portal node shows controller name + confidence color
- [ ] Portal tooltip shows evidence + data gaps
- [ ] Clicking portal transitions to Board Control view
- [ ] Board Control view shows ownership tree from controller up
- [ ] Winning rule path highlighted in graph
- [ ] UBO persons show PSC category badges
- [ ] Control edges labeled with BODS interest types
- [ ] Evidence panel shows source documents
- [ ] Breadcrumb returns to CBU trading view
- [ ] Trading matrix untouched (CBU-only)
- [ ] Recompute triggers work (edge change, manual, scheduled)
- [ ] SHOW BOARD CONTROLLER DSL returns explanation

---

## Files to Create/Modify

| File | Changes |
|------|---------|
| `migrations/XXXX_control_edges.sql` | `control_edges`, `cbu_board_controller`, `board_control_evidence` tables |
| `ob-poc-types/src/control.rs` | NEW - ControlEdgeType, BoardControllerEdge, BoardControlMethod, ControlScore, explanation types |
| `ob-poc-types/src/viewport.rs` | ViewportFocusState::BoardControl |
| `rust/src/services/board_control_rules.rs` | NEW - Rules engine (A/B/C/D), scoring, explanation generation |
| `rust/src/api/control_routes.rs` | NEW - control sphere/board control endpoints |
| `rust/src/api/cbu_routes.rs` | Add board-controller endpoint |
| `ob-poc-graph/src/graph/board_control.rs` | NEW - ownership tree rendering, evidence panel |
| `ob-poc-graph/src/graph/mod.rs` | Add board_control module |
| `ob-poc-ui/src/app.rs` | Handle BoardControl focus, portal clicks |
| `ob-poc-ui/src/panels/evidence.rs` | NEW - evidence source panel |

---

## Anti-Patterns to Avoid

1. **NO control edges in CBU operating graph** - they're different domains
2. **NO ownership % on CBU service-provider edges** - those are contractual, not equity
3. **NO rendering full group tree in CBU view** - use portal, transition viewport
4. **NO hand-authoring board controller** - it's DERIVED via rules engine, not entered
5. **NO storing "UBO" as static field** - compute from control edges on demand via rules engine
6. **NO mixing mandate authority with voting control** - IM manages under mandate, doesn't own
7. **NO inventing edge types outside BODS/GLEIF** - use standard interest types, add xrefs
8. **NO hardcoding thresholds** - PSC uses 25%, other regimes may differ, make configurable
9. **NO assuming shares = votes** - store separately, voting rights can diverge from equity
10. **NO hiding the derivation** - always show WHY someone is the controller (evidence + gaps)

---

## Dependency

**Requires Phase 1 complete**: CBU trading view must be working (graph + matrix auto-load on CBU selection) before adding control sphere navigation.
