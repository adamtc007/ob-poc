# Ownership Graph & UBO Computation

> **Status:** Planning
> **Priority:** High - Foundation for normalized UBO across CBUs
> **Created:** 2026-01-10
> **Estimated Effort:** 70-80 hours
> **Dependencies:** 
>   - 016-capital-structure-ownership-model.md (investor register schema)
>   - 018-investor-register-visualization.md (institutional look-through)
>   - Existing GLEIF/BODS integration

---

## Core Principle

**UBO is COMPUTED, not STORED.**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    SEPARATION OF CONCERNS                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   LAYER 1: RAW DATA (what we store)                                         │
│   ══════════════════════════════════                                         │
│   • Ownership edges: Entity A owns X shares in Entity B                     │
│   • Control edges: Person P is director of Entity C                         │
│   • Voting edges: Entity D has voting agreement over Entity E               │
│                                                                              │
│   NO KYC CONCEPTS HERE - just ownership and control FACTS                   │
│                                                                              │
│   ───────────────────────────────────────────────────────────────────────   │
│                                                                              │
│   LAYER 2: COVERAGE (what we know vs don't know)                            │
│   ══════════════════════════════════════════════                             │
│   • Known beneficial: 57% - can trace to natural person                     │
│   • Known legal only: 15% - nominee, beneficial unknown                     │
│   • Aggregate: 18% - public float, retail                                   │
│   • Unaccounted: 10% - gap in share register                                │
│                                                                              │
│   INCOMPLETE DATA IS VALID STATE - drives research, not errors              │
│                                                                              │
│   ───────────────────────────────────────────────────────────────────────   │
│                                                                              │
│   LAYER 3: RULES (jurisdiction-specific)                                    │
│   ══════════════════════════════════════                                     │
│   • EU 4AMLD: >25% ownership OR >25% voting OR control                      │
│   • UK PSC: >25% shares OR >25% voting OR appoints majority board           │
│   • US FinCEN: >25% equity OR "significant control"                         │
│                                                                              │
│   CONFIGURABLE PER JURISDICTION - not hard-coded                            │
│                                                                              │
│   ───────────────────────────────────────────────────────────────────────   │
│                                                                              │
│   LAYER 4: COMPUTATION (dynamic)                                             │
│   ══════════════════════════════                                             │
│   fn_compute_ubos(entity_id, jurisdiction, as_of_date)                      │
│       → Traverses ownership graph                                            │
│       → Applies jurisdiction rules                                           │
│       → Returns UBO list with basis                                          │
│                                                                              │
│   SAME GRAPH → DIFFERENT UBO LISTS depending on jurisdiction                │
│                                                                              │
│   ───────────────────────────────────────────────────────────────────────   │
│                                                                              │
│   LAYER 5: OUTPUT (BODS, regulatory filings)                                │
│   ══════════════════════════════════════════                                 │
│   • Export computed UBOs in required format                                 │
│   • Handle gaps explicitly (unableToConfirm reasons)                        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Ownership Graph Model

### Edge Types

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    OWNERSHIP EDGE TYPES                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   1. BENEFICIAL OWNERSHIP                                                    │
│      ────────────────────────                                                │
│      Entity A ──[35% beneficial]──► Entity B                                │
│                                                                              │
│      • Full chain traceable to natural person (or regulated terminal)       │
│      • Holder entity linked and verified                                    │
│      • Basis: share register, GLEIF, declaration                            │
│                                                                              │
│   2. LEGAL OWNERSHIP (nominee/custodian)                                    │
│      ──────────────────────────────────                                      │
│      Nominee X ──[15% legal]──► Entity B                                    │
│                                                                              │
│      • Legal holder known, beneficial holder UNKNOWN                        │
│      • Requires look-through research                                       │
│      • Common: Clearstream, Euroclear, DTC, nominee accounts                │
│                                                                              │
│   3. AGGREGATE (public float, retail pools)                                 │
│      ─────────────────────────────────────                                   │
│      [PUBLIC_FLOAT] ──[18%]──► Entity B                                     │
│                                                                              │
│      • Distributed holdings, individually immaterial (<threshold)           │
│      • May never be fully resolved - accepted unknown                       │
│      • Synthetic holder record                                              │
│                                                                              │
│   4. UNACCOUNTED GAP                                                         │
│      ────────────────                                                        │
│      [UNACCOUNTED] ──[10%]──► Entity B                                      │
│                                                                              │
│      • Issued shares with no holder record                                  │
│      • Data quality issue OR bearer shares OR timing gap                    │
│      • Synthetic holder record, triggers research                           │
│                                                                              │
│   5. CONTROL (non-ownership)                                                 │
│      ────────────────────────                                                │
│      Person P ──[DIRECTOR]──► Entity B                                      │
│      Entity A ──[APPOINTS_BOARD]──► Entity B                                │
│      Entity A ──[VOTING_AGREEMENT]──► Entity B                              │
│                                                                              │
│      • Control without direct share ownership                               │
│      • Critical for UBO where control = UBO                                 │
│                                                                              │
│   6. BROKEN CHAIN                                                            │
│      ────────────                                                            │
│      Entity A ──[35%]──► Entity B ──[???]──► ???                            │
│                                                                              │
│      • Ownership known at one level, breaks at next                         │
│      • Non-terminal entity with no onward ownership data                    │
│      • Research required to complete                                        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Terminal vs Non-Terminal

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TERMINAL CLASSIFICATION                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   TERMINAL (chain ends here - no further look-through required):            │
│   ──────────────────────────────────────────────────────────────            │
│                                                                              │
│   • NATURAL_PERSON (PROPER_PERSON in our taxonomy)                          │
│     The actual human - always terminal                                      │
│                                                                              │
│   • REGULATED_ENTITY (with exemption)                                       │
│     Listed company, regulated fund, bank                                    │
│     May be treated as terminal per jurisdiction rules                       │
│                                                                              │
│   • GOVERNMENT_ENTITY                                                        │
│     Sovereign, state-owned - typically terminal                             │
│                                                                              │
│   • PUBLIC_FLOAT                                                             │
│     Synthetic terminal - distributed retail                                 │
│                                                                              │
│   NON-TERMINAL (requires look-through):                                     │
│   ─────────────────────────────────────                                      │
│                                                                              │
│   • LIMITED_COMPANY (private)                                               │
│   • PARTNERSHIP                                                              │
│   • TRUST                                                                    │
│   • FOUNDATION                                                               │
│   • FUND (private, unregulated)                                             │
│   • SPV                                                                      │
│   • NOMINEE                                                                  │
│                                                                              │
│   BROKEN CHAIN = Non-terminal with no onward edges                          │
│   This is valid state, triggers research                                    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Coverage Model

### Metrics

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    OWNERSHIP COVERAGE METRICS                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   For Entity X (issued 1,000,000 shares):                                   │
│                                                                              │
│   ┌───────────────────────────────────────────────────────────────────┐     │
│   │  COVERAGE BREAKDOWN                                                │     │
│   │                                                                    │     │
│   │  Category              Units      Pct    Chain Status             │     │
│   │  ────────              ─────      ───    ────────────             │     │
│   │  Known beneficial      570,000    57%    Complete to terminal     │     │
│   │  Known legal only      150,000    15%    Stops at nominee         │     │
│   │  Aggregate (float)     180,000    18%    Accepted incomplete      │     │
│   │  Unaccounted            100,000    10%    No holder record         │     │
│   │                        ─────────  ────                             │     │
│   │  Total issued        1,000,000   100%                              │     │
│   │                                                                    │     │
│   │  ═══════════════════════════════════════════════════════════════  │     │
│   │                                                                    │     │
│   │  DERIVED METRICS                                                   │     │
│   │                                                                    │     │
│   │  Coverage score:           57%  (known beneficial / total)        │     │
│   │  Traceable score:          72%  (beneficial + legal / total)      │     │
│   │  Gap score:                10%  (unaccounted / total)             │     │
│   │                                                                    │     │
│   │  Discovery status:     PARTIAL                                    │     │
│   │  Research required:    YES                                        │     │
│   │                                                                    │     │
│   └───────────────────────────────────────────────────────────────────┘     │
│                                                                              │
│   STATUS THRESHOLDS (configurable per issuer/jurisdiction):                 │
│   ─────────────────────────────────────────────────────────                 │
│                                                                              │
│   Coverage > 75% beneficial    → SUFFICIENT                                 │
│   Coverage 50-75%              → PARTIAL (research recommended)             │
│   Coverage < 50%               → INSUFFICIENT (research required)           │
│   Any holder >25% unresolved   → BLOCKING (cannot complete KYC)            │
│   Gap > 10%                    → DATA_QUALITY_ISSUE                         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Research Triggers

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    GAP → RESEARCH WORKFLOW                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   GAP TYPE                    RESEARCH ACTION                VERB           │
│   ────────                    ───────────────                ────           │
│                                                                              │
│   Nominee >10%                Request nominee disclosure     research.      │
│                               Trigger KYC on underlying      request-       │
│                                                              disclosure     │
│                                                                              │
│   Broken chain at             GLEIF lookup for parent        gleif.         │
│   non-terminal entity         Request ownership declaration  import-        │
│                               Agent research macro           hierarchy      │
│                                                                              │
│   Unaccounted >5%             Reconciliation with registrar  research.      │
│                               Request share register extract request-       │
│                                                              register       │
│                                                                              │
│   Chain depth >5              Review for circular structures ownership.     │
│   without terminal            May indicate evasion           detect-        │
│                                                              cycles         │
│                                                                              │
│   No natural person           Apply "senior manager" fallback ubo.          │
│   found after full traverse   Request board composition      apply-         │
│                                                              fallback       │
│                                                                              │
│   RESEARCH TRIGGERS stored as action items, not blocking errors             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## UBO Jurisdiction Rules

### Rule Structure

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    JURISDICTION-SPECIFIC UBO RULES                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   EU 4AMLD/5AMLD:                                                           │
│   ───────────────                                                            │
│   • Ownership threshold: >25%                                               │
│   • Voting threshold: >25%                                                  │
│   • Control: "otherwise exercises control"                                  │
│   • Fallback: Senior managing official if no UBO identified                │
│   • Listed company exemption: Yes (regulated market)                        │
│                                                                              │
│   UK PSC (People with Significant Control):                                 │
│   ──────────────────────────────────────────                                 │
│   • Shares threshold: >25%                                                  │
│   • Voting threshold: >25%                                                  │
│   • Board control: Appoints/removes majority of directors                   │
│   • Significant influence: Right to exercise significant influence          │
│   • Listed company exemption: Yes                                           │
│                                                                              │
│   US FinCEN (CDD Rule):                                                     │
│   ─────────────────────                                                      │
│   • Equity threshold: >25%                                                  │
│   • Control: "significant responsibility to control/manage"                 │
│   • Fallback: Senior executive (CEO, CFO, COO, etc.)                       │
│   • Exemptions: Banks, public companies, registered entities               │
│                                                                              │
│   Cayman Islands:                                                           │
│   ───────────────                                                            │
│   • Follows UK model broadly                                                │
│   • Additional: Regulated fund exemption                                    │
│                                                                              │
│   Luxembourg:                                                               │
│   ───────────                                                                │
│   • EU 4AMLD base                                                           │
│   • Additional: RBE (Register of Beneficial Owners) specific rules          │
│                                                                              │
│   Ireland:                                                                  │
│   ────────                                                                   │
│   • EU 4AMLD base                                                           │
│   • Additional: ICAV, UCITS specific exemptions                             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Rule Application

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    UBO COMPUTATION FLOW                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   INPUT:                                                                     │
│   ──────                                                                     │
│   • Target entity (the company we need UBOs for)                            │
│   • Jurisdiction (which rules to apply)                                     │
│   • As-of date (point in time)                                              │
│                                                                              │
│   PROCESS:                                                                   │
│   ────────                                                                   │
│                                                                              │
│   1. TRAVERSE OWNERSHIP GRAPH                                               │
│      ─────────────────────────                                               │
│      Start at target, walk all ownership edges                              │
│      Track: path, cumulative ownership %, chain depth                       │
│      Stop at: terminal entities OR broken chains OR cycle                   │
│                                                                              │
│   2. TRAVERSE CONTROL GRAPH                                                 │
│      ───────────────────────                                                 │
│      Find all control edges pointing to target (and subsidiaries)           │
│      Track: control type, controller entity                                 │
│                                                                              │
│   3. APPLY JURISDICTION RULES                                               │
│      ────────────────────────                                                │
│      For each terminal person found:                                        │
│      • Does cumulative ownership >= threshold? → OWNERSHIP UBO              │
│      • Does cumulative voting >= threshold? → VOTING UBO                    │
│      • Does control apply? → CONTROL UBO                                    │
│                                                                              │
│   4. APPLY EXEMPTIONS                                                       │
│      ─────────────────                                                       │
│      For each intermediate entity:                                          │
│      • Is it a listed company? → May terminate chain                        │
│      • Is it a regulated fund? → May terminate chain                        │
│      • Is it a government entity? → Terminates chain                        │
│                                                                              │
│   5. APPLY FALLBACK                                                         │
│      ──────────────                                                          │
│      If no natural person UBO found:                                        │
│      • Identify senior managing official(s)                                 │
│      • Mark as FALLBACK_UBO with basis "no_ubo_identified"                  │
│                                                                              │
│   6. HANDLE GAPS                                                            │
│      ───────────                                                             │
│      For each broken chain:                                                 │
│      • Record as UNRESOLVED with reason                                     │
│      • Generate research trigger                                            │
│                                                                              │
│   OUTPUT:                                                                    │
│   ───────                                                                    │
│   List of UBO records, each with:                                           │
│   • person_entity_id (natural person)                                       │
│   • ubo_type: OWNERSHIP | VOTING | CONTROL | FALLBACK                       │
│   • basis: ownership_25_pct | voting_25_pct | board_control | senior_mgr   │
│   • effective_pct (cumulative through chain)                                │
│   • chain_path (entities traversed)                                         │
│   • is_verified (all links verified?)                                       │
│                                                                              │
│   Plus: coverage metrics, research triggers, gaps                           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Group Context

### What is a Group?

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    GROUP = CONNECTED OWNERSHIP SUBGRAPH                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   A GROUP is:                                                               │
│   • Set of entities connected by >50% ownership (control)                   │
│   • Anchored by a "group parent" (ManCo, holding company)                   │
│   • Shares common UBO structure                                             │
│                                                                              │
│   Example: AllianzGI Group                                                  │
│   ────────────────────────                                                   │
│                                                                              │
│   ┌─────────────────┐                                                       │
│   │   Allianz SE    │ ◄── Ultimate parent (listed - terminal for UBO)      │
│   └────────┬────────┘                                                       │
│            │ 100%                                                            │
│   ┌────────┴────────┐                                                       │
│   │ AllianzGI GmbH  │ ◄── Group anchor (ManCo)                              │
│   └────────┬────────┘                                                       │
│            │                                                                 │
│   ┌────────┼────────┬────────────┐                                          │
│   │ 100%   │ 100%   │ 100%       │                                          │
│   ▼        ▼        ▼            ▼                                          │
│  AGI UK   AGI LU   AGI SG    AGI US                                         │
│   │        │        │            │                                          │
│   │        │        │            │                                          │
│   ▼        ▼        ▼            ▼                                          │
│ Fund A   Fund D   Fund G     Fund J    ◄── CBUs (our clients)              │
│ Fund B   Fund E   Fund H                                                    │
│ Fund C   Fund F                                                             │
│                                                                              │
│   GROUP BOUNDARY = All entities reachable via >50% ownership from anchor   │
│                                                                              │
│   WHY GROUPS MATTER:                                                        │
│   ──────────────────                                                         │
│   • Same UBO structure shared across all CBUs in group                      │
│   • Update once (at group level), propagates to all CBUs                    │
│   • Group-level periodic review, not per-CBU                                │
│   • GLEIF validation at group level                                         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Group vs CBU Scope

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    SESSION SCOPE MODEL                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   SCOPE: GROUP                                                              │
│   ────────────                                                               │
│   session.set-scope(:type "group" :anchor-entity-id @allianzgi)             │
│                                                                              │
│   What you see:                                                              │
│   • All entities in group (via ownership traversal)                         │
│   • Intra-group ownership edges                                             │
│   • Group-level UBO computation                                             │
│   • All CBUs serviced by group                                              │
│                                                                              │
│   What you do:                                                               │
│   • Maintain ownership data (once, for whole group)                         │
│   • Run UBO computation (applies to all CBUs)                               │
│   • GLEIF validation/refresh                                                │
│   • Group-level BODS export                                                 │
│   • Trigger group research                                                  │
│                                                                              │
│   ───────────────────────────────────────────────────────────────────────   │
│                                                                              │
│   SCOPE: CBU                                                                │
│   ──────────                                                                 │
│   session.set-scope(:type "cbu" :cbu-id @fund-alpha)                        │
│                                                                              │
│   What you see:                                                              │
│   • CBU's investor register (Fund Alpha's investors)                        │
│   • CBU-specific KYC cases                                                  │
│   • REFERENCE to group's UBO structure (read-only here)                     │
│                                                                              │
│   What you do:                                                               │
│   • CBU-specific onboarding                                                 │
│   • Investor KYC                                                            │
│   • CBU-level BODS export (uses group UBO data)                             │
│                                                                              │
│   The UBO section of CBU KYC = focused view into group's UBO structure     │
│   CBU doesn't store its own UBO - it REFERENCES the group's                 │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Schema

### Ownership Edges

```sql
-- =============================================================================
-- OWNERSHIP EDGE CLASSIFICATION
-- =============================================================================

-- Extend holdings with ownership classification
ALTER TABLE kyc.holdings ADD COLUMN IF NOT EXISTS 
    ownership_nature VARCHAR(30) DEFAULT 'BENEFICIAL';

COMMENT ON COLUMN kyc.holdings.ownership_nature IS 
'BENEFICIAL (full chain known), LEGAL_ONLY (nominee), INDIRECT (via intermediate)';

-- Control constraint
ALTER TABLE kyc.holdings DROP CONSTRAINT IF EXISTS chk_ownership_nature;
ALTER TABLE kyc.holdings ADD CONSTRAINT chk_ownership_nature CHECK (
    ownership_nature IN ('BENEFICIAL', 'LEGAL_ONLY', 'INDIRECT')
);

-- =============================================================================
-- SYNTHETIC HOLDERS (for gaps)
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.synthetic_holders (
    synthetic_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- What entity's cap table has this gap
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Type of synthetic holder
    holder_type VARCHAR(30) NOT NULL,
    
    -- Description
    description TEXT,
    
    -- Estimated size
    estimated_units NUMERIC(20,6),
    estimated_pct DECIMAL(7,4),
    
    -- As-of tracking
    as_of_date DATE NOT NULL DEFAULT CURRENT_DATE,
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by UUID,
    
    CONSTRAINT chk_synthetic_type CHECK (
        holder_type IN (
            'PUBLIC_FLOAT',      -- Distributed retail, individually immaterial
            'NOMINEE_POOL',      -- Aggregated nominee holdings
            'UNACCOUNTED',       -- Gap in share register
            'BEARER_SHARES',     -- Bearer instruments, holder unknown
            'PENDING_SETTLEMENT' -- Recently traded, not yet registered
        )
    )
);

CREATE INDEX idx_synthetic_issuer ON kyc.synthetic_holders(issuer_entity_id);

COMMENT ON TABLE kyc.synthetic_holders IS 
'Placeholder records for ownership gaps. Not real holders - represent unknown or aggregated positions.';

-- =============================================================================
-- CONTROL EDGES (non-ownership control)
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.control_relationships (
    control_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Who has control
    controller_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Over what entity
    controlled_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Type of control
    control_type VARCHAR(30) NOT NULL,
    
    -- Details
    control_basis TEXT,  -- Description of how control is exercised
    
    -- Validity
    effective_from DATE NOT NULL DEFAULT CURRENT_DATE,
    effective_to DATE,
    
    -- Evidence
    evidence_type VARCHAR(50),  -- BOARD_RESOLUTION, SHAREHOLDER_AGREEMENT, VOTING_PROXY
    evidence_ref TEXT,
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    verified_at TIMESTAMPTZ,
    verified_by UUID,
    
    CONSTRAINT chk_control_type CHECK (
        control_type IN (
            'BOARD_APPOINTMENT',  -- Can appoint/remove majority of board
            'VOTING_AGREEMENT',   -- Has voting rights via agreement
            'MANAGEMENT_CONTRACT',-- Contractual control over operations
            'VETO_RIGHTS',        -- Can block significant decisions
            'GOLDEN_SHARE',       -- Special share with control rights
            'OTHER_CONTROL'       -- Other control mechanism
        )
    ),
    
    UNIQUE(controller_entity_id, controlled_entity_id, control_type, effective_from)
);

CREATE INDEX idx_control_controller ON kyc.control_relationships(controller_entity_id);
CREATE INDEX idx_control_controlled ON kyc.control_relationships(controlled_entity_id);

COMMENT ON TABLE kyc.control_relationships IS 
'Control relationships that are NOT via share ownership. Supplements holdings for UBO computation.';

-- =============================================================================
-- OWNERSHIP COVERAGE
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.ownership_coverage (
    coverage_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Which entity
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- As-of date
    as_of_date DATE NOT NULL DEFAULT CURRENT_DATE,
    
    -- Raw figures
    total_issued NUMERIC(20,6),
    
    -- Coverage breakdown
    known_beneficial_units NUMERIC(20,6) DEFAULT 0,
    known_beneficial_pct DECIMAL(7,4) DEFAULT 0,
    
    known_legal_only_units NUMERIC(20,6) DEFAULT 0,
    known_legal_only_pct DECIMAL(7,4) DEFAULT 0,
    
    aggregate_units NUMERIC(20,6) DEFAULT 0,
    aggregate_pct DECIMAL(7,4) DEFAULT 0,
    
    unaccounted_units NUMERIC(20,6) DEFAULT 0,
    unaccounted_pct DECIMAL(7,4) DEFAULT 0,
    
    -- Derived scores
    coverage_score DECIMAL(5,2),       -- known_beneficial_pct
    traceable_score DECIMAL(5,2),      -- beneficial + legal_only
    gap_score DECIMAL(5,2),            -- unaccounted_pct
    
    -- Status
    discovery_status VARCHAR(30) NOT NULL DEFAULT 'NOT_STARTED',
    
    -- Research
    research_required BOOLEAN DEFAULT false,
    research_triggers JSONB,  -- [{type: "nominee_lookup", holder_id: "...", pct: 15}]
    
    -- Audit
    computed_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT chk_discovery_status CHECK (
        discovery_status IN (
            'NOT_STARTED',    -- No UBO work done
            'IN_PROGRESS',    -- Currently researching
            'SUFFICIENT',     -- Coverage > 75%, acceptable
            'PARTIAL',        -- Coverage 50-75%, more research recommended
            'INSUFFICIENT',   -- Coverage < 50%, research required
            'BLOCKED'         -- Cannot proceed without resolution
        )
    ),
    
    UNIQUE(entity_id, as_of_date)
);

CREATE INDEX idx_coverage_entity ON kyc.ownership_coverage(entity_id);
CREATE INDEX idx_coverage_status ON kyc.ownership_coverage(discovery_status);

COMMENT ON TABLE kyc.ownership_coverage IS 
'Computed coverage metrics per entity. Drives research triggers and UBO discovery status.';
```

### UBO Rules

```sql
-- =============================================================================
-- UBO JURISDICTION RULES
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.ubo_jurisdiction_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Jurisdiction
    jurisdiction_code VARCHAR(10) NOT NULL,  -- EU, UK, US, KY, IE, LU, etc.
    jurisdiction_name VARCHAR(100),
    
    -- Ownership thresholds
    ownership_threshold_pct DECIMAL(5,2) DEFAULT 25.00,
    voting_threshold_pct DECIMAL(5,2) DEFAULT 25.00,
    
    -- Control criteria
    board_majority_is_control BOOLEAN DEFAULT true,
    board_majority_definition VARCHAR(100) DEFAULT 'APPOINT_REMOVE_MAJORITY',
    
    -- Fallback
    senior_manager_fallback BOOLEAN DEFAULT true,
    fallback_roles TEXT[] DEFAULT ARRAY['CEO', 'CFO', 'COO', 'MANAGING_DIRECTOR'],
    
    -- Exemptions
    listed_company_exempt BOOLEAN DEFAULT true,
    regulated_fund_exempt BOOLEAN DEFAULT false,
    government_entity_exempt BOOLEAN DEFAULT true,
    
    -- Traversal
    max_chain_depth INTEGER DEFAULT 10,
    follow_indirect_ownership BOOLEAN DEFAULT true,
    cumulative_threshold BOOLEAN DEFAULT true,  -- Multiply through chain vs any single link
    
    -- Effective dates (for rule changes over time)
    effective_from DATE NOT NULL,
    effective_to DATE,
    
    -- Reference
    regulation_reference TEXT,  -- "EU Directive 2015/849 Article 3(6)"
    
    UNIQUE(jurisdiction_code, effective_from)
);

-- Seed with common jurisdictions
INSERT INTO kyc.ubo_jurisdiction_rules (
    jurisdiction_code, jurisdiction_name, 
    ownership_threshold_pct, voting_threshold_pct,
    board_majority_is_control, senior_manager_fallback,
    listed_company_exempt, regulated_fund_exempt,
    effective_from, regulation_reference
) VALUES 
(
    'EU', 'European Union (4AMLD/5AMLD)', 
    25.00, 25.00,
    true, true,
    true, false,
    '2017-06-26', 'Directive 2015/849 as amended by 2018/843'
),
(
    'UK', 'United Kingdom (PSC)', 
    25.00, 25.00,
    true, true,
    true, false,
    '2016-04-06', 'Small Business, Enterprise and Employment Act 2015'
),
(
    'US', 'United States (FinCEN CDD)', 
    25.00, NULL,  -- US uses equity only, not voting
    true, true,
    true, false,
    '2018-05-11', '31 CFR 1010.230'
),
(
    'KY', 'Cayman Islands', 
    25.00, 25.00,
    true, true,
    true, true,  -- Regulated fund exempt
    '2017-07-01', 'Beneficial Ownership Regime'
),
(
    'IE', 'Ireland', 
    25.00, 25.00,
    true, true,
    true, true,  -- UCITS/AIF exempt
    '2019-03-22', 'S.I. No. 110/2019'
),
(
    'LU', 'Luxembourg', 
    25.00, 25.00,
    true, true,
    true, true,  -- Regulated fund exempt
    '2019-03-01', 'Law of 13 January 2019'
)
ON CONFLICT DO NOTHING;

COMMENT ON TABLE kyc.ubo_jurisdiction_rules IS 
'Jurisdiction-specific rules for UBO determination. Same ownership graph produces different UBO lists based on rules applied.';
```

### Group Registry

```sql
-- =============================================================================
-- GROUP REGISTRY
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.ownership_groups (
    group_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Anchor entity (the ManCo or holding company)
    anchor_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Group metadata
    group_name TEXT NOT NULL,
    group_type VARCHAR(30) NOT NULL,
    
    -- Ultimate parent (may be different from anchor)
    ultimate_parent_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    ultimate_parent_lei VARCHAR(20),
    
    -- Traversal rule for group membership
    membership_threshold_pct DECIMAL(5,2) DEFAULT 50.00,  -- >50% = subsidiary
    
    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
    
    -- Periodic review
    last_review_date DATE,
    next_review_date DATE,
    review_frequency_months INTEGER DEFAULT 12,
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT chk_group_type CHECK (
        group_type IN ('MANCO', 'BANK', 'INSURANCE', 'CORPORATE', 'PE_GP', 'FAMILY_OFFICE')
    ),
    
    CONSTRAINT uq_group_anchor UNIQUE (anchor_entity_id)
);

COMMENT ON TABLE kyc.ownership_groups IS 
'Registry of corporate groups. Group membership is computed by traversing ownership graph from anchor.';

-- =============================================================================
-- GROUP CBU LINKS
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.group_cbu_links (
    group_id UUID NOT NULL REFERENCES kyc.ownership_groups(group_id),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    
    -- Which group entity services this CBU
    servicing_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Role
    service_role VARCHAR(30) NOT NULL,
    
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE',
    
    PRIMARY KEY (group_id, cbu_id, servicing_entity_id, service_role),
    
    CONSTRAINT chk_service_role CHECK (
        service_role IN ('MANAGER', 'ADMINISTRATOR', 'CUSTODIAN', 'DEPOSITARY', 'DISTRIBUTOR')
    )
);

CREATE INDEX idx_group_cbu_links_cbu ON kyc.group_cbu_links(cbu_id);

COMMENT ON TABLE kyc.group_cbu_links IS 
'Links CBUs to the groups that service them. One CBU may be serviced by multiple groups in different roles.';
```

### Research Triggers

```sql
-- =============================================================================
-- RESEARCH TRIGGERS
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.ownership_research_triggers (
    trigger_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- What entity needs research
    target_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- What holder/gap triggered this
    source_holder_id UUID,  -- NULL for synthetic gaps
    source_synthetic_id UUID REFERENCES kyc.synthetic_holders(synthetic_id),
    
    -- Type of research needed
    research_type VARCHAR(30) NOT NULL,
    
    -- Priority
    priority VARCHAR(10) DEFAULT 'MEDIUM',
    
    -- Details
    description TEXT,
    affected_pct DECIMAL(7,4),  -- How much of ownership this affects
    
    -- Status
    status VARCHAR(20) DEFAULT 'OPEN',
    assigned_to UUID,
    
    -- Resolution
    resolved_at TIMESTAMPTZ,
    resolution_notes TEXT,
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT chk_research_type CHECK (
        research_type IN (
            'NOMINEE_DISCLOSURE',     -- Request look-through from nominee
            'GLEIF_LOOKUP',           -- Look up entity in GLEIF
            'OWNERSHIP_DECLARATION',  -- Request declaration from entity
            'REGISTER_RECONCILE',     -- Reconcile with share register
            'BOARD_COMPOSITION',      -- Get board/officer details
            'CYCLE_REVIEW',           -- Review potential circular structure
            'CHAIN_COMPLETION'        -- Complete broken chain
        )
    ),
    
    CONSTRAINT chk_priority CHECK (priority IN ('HIGH', 'MEDIUM', 'LOW')),
    CONSTRAINT chk_status CHECK (status IN ('OPEN', 'IN_PROGRESS', 'RESOLVED', 'WONT_FIX'))
);

CREATE INDEX idx_research_entity ON kyc.ownership_research_triggers(target_entity_id);
CREATE INDEX idx_research_status ON kyc.ownership_research_triggers(status);

COMMENT ON TABLE kyc.ownership_research_triggers IS 
'Action items generated from ownership gaps. Drives KYC workflow to complete UBO discovery.';
```

---

## Computed Functions

### Coverage Computation

```sql
-- =============================================================================
-- fn_compute_ownership_coverage
-- =============================================================================
-- Computes coverage metrics for an entity's cap table

CREATE OR REPLACE FUNCTION kyc.fn_compute_ownership_coverage(
    p_entity_id UUID,
    p_as_of_date DATE DEFAULT CURRENT_DATE
) RETURNS kyc.ownership_coverage AS $$
DECLARE
    v_result kyc.ownership_coverage;
    v_total_issued NUMERIC;
    v_beneficial NUMERIC := 0;
    v_legal_only NUMERIC := 0;
    v_aggregate NUMERIC := 0;
    v_unaccounted NUMERIC := 0;
BEGIN
    -- Get total issued from share classes
    SELECT COALESCE(SUM(sc.issued_shares), 0)
    INTO v_total_issued
    FROM kyc.share_classes sc
    WHERE sc.issuer_entity_id = p_entity_id;
    
    IF v_total_issued = 0 THEN
        -- No shares issued, return empty coverage
        v_result.entity_id := p_entity_id;
        v_result.as_of_date := p_as_of_date;
        v_result.discovery_status := 'NOT_STARTED';
        RETURN v_result;
    END IF;
    
    -- Sum holdings by ownership_nature
    SELECT 
        COALESCE(SUM(CASE WHEN h.ownership_nature = 'BENEFICIAL' THEN h.units ELSE 0 END), 0),
        COALESCE(SUM(CASE WHEN h.ownership_nature = 'LEGAL_ONLY' THEN h.units ELSE 0 END), 0)
    INTO v_beneficial, v_legal_only
    FROM kyc.holdings h
    JOIN kyc.share_classes sc ON sc.id = h.share_class_id
    WHERE sc.issuer_entity_id = p_entity_id
    AND h.status = 'active';
    
    -- Sum synthetic holders
    SELECT 
        COALESCE(SUM(CASE WHEN sh.holder_type = 'PUBLIC_FLOAT' THEN sh.estimated_units ELSE 0 END), 0),
        COALESCE(SUM(CASE WHEN sh.holder_type IN ('UNACCOUNTED', 'BEARER_SHARES') THEN sh.estimated_units ELSE 0 END), 0)
    INTO v_aggregate, v_unaccounted
    FROM kyc.synthetic_holders sh
    WHERE sh.issuer_entity_id = p_entity_id
    AND sh.as_of_date <= p_as_of_date;
    
    -- Calculate unaccounted as remainder if not explicitly tracked
    IF v_unaccounted = 0 THEN
        v_unaccounted := GREATEST(0, v_total_issued - v_beneficial - v_legal_only - v_aggregate);
    END IF;
    
    -- Build result
    v_result.entity_id := p_entity_id;
    v_result.as_of_date := p_as_of_date;
    v_result.total_issued := v_total_issued;
    
    v_result.known_beneficial_units := v_beneficial;
    v_result.known_beneficial_pct := ROUND((v_beneficial / v_total_issued) * 100, 2);
    
    v_result.known_legal_only_units := v_legal_only;
    v_result.known_legal_only_pct := ROUND((v_legal_only / v_total_issued) * 100, 2);
    
    v_result.aggregate_units := v_aggregate;
    v_result.aggregate_pct := ROUND((v_aggregate / v_total_issued) * 100, 2);
    
    v_result.unaccounted_units := v_unaccounted;
    v_result.unaccounted_pct := ROUND((v_unaccounted / v_total_issued) * 100, 2);
    
    -- Derived scores
    v_result.coverage_score := v_result.known_beneficial_pct;
    v_result.traceable_score := v_result.known_beneficial_pct + v_result.known_legal_only_pct;
    v_result.gap_score := v_result.unaccounted_pct;
    
    -- Determine status
    v_result.discovery_status := CASE
        WHEN v_result.coverage_score >= 75 THEN 'SUFFICIENT'
        WHEN v_result.coverage_score >= 50 THEN 'PARTIAL'
        WHEN v_result.coverage_score > 0 THEN 'INSUFFICIENT'
        ELSE 'NOT_STARTED'
    END;
    
    -- Check for blocking conditions
    IF v_result.known_legal_only_pct >= 25 THEN
        v_result.discovery_status := 'BLOCKED';
        v_result.research_required := true;
    END IF;
    
    IF v_result.unaccounted_pct >= 10 THEN
        v_result.research_required := true;
    END IF;
    
    v_result.computed_at := NOW();
    
    RETURN v_result;
END;
$$ LANGUAGE plpgsql;
```

### UBO Computation

```sql
-- =============================================================================
-- fn_compute_ubos
-- =============================================================================
-- Computes UBOs for an entity under a specific jurisdiction's rules

CREATE OR REPLACE FUNCTION kyc.fn_compute_ubos(
    p_entity_id UUID,
    p_jurisdiction VARCHAR(10),
    p_as_of_date DATE DEFAULT CURRENT_DATE
) RETURNS TABLE (
    person_entity_id UUID,
    person_name TEXT,
    ubo_type VARCHAR(30),
    ubo_basis VARCHAR(50),
    effective_ownership_pct DECIMAL(7,4),
    effective_voting_pct DECIMAL(7,4),
    chain_path UUID[],
    chain_depth INTEGER,
    is_verified BOOLEAN,
    is_fallback BOOLEAN,
    notes TEXT
) AS $$
DECLARE
    v_rules kyc.ubo_jurisdiction_rules;
    v_found_ubo BOOLEAN := false;
BEGIN
    -- Get jurisdiction rules
    SELECT * INTO v_rules
    FROM kyc.ubo_jurisdiction_rules r
    WHERE r.jurisdiction_code = p_jurisdiction
    AND r.effective_from <= p_as_of_date
    AND (r.effective_to IS NULL OR r.effective_to > p_as_of_date)
    ORDER BY r.effective_from DESC
    LIMIT 1;
    
    IF v_rules IS NULL THEN
        RAISE EXCEPTION 'No UBO rules found for jurisdiction %', p_jurisdiction;
    END IF;
    
    -- Traverse ownership graph and find UBOs
    -- This is a recursive CTE that walks the ownership chain
    RETURN QUERY
    WITH RECURSIVE ownership_chain AS (
        -- Base case: direct holders of target entity
        SELECT 
            h.investor_entity_id AS holder_id,
            e.name AS holder_name,
            et.entity_category,
            ARRAY[p_entity_id, h.investor_entity_id] AS path,
            1 AS depth,
            (h.units / NULLIF(sc.issued_shares, 0)) * 100 AS ownership_pct,
            (h.units * COALESCE(sc.voting_rights_per_share, 1) / 
             NULLIF(sc.issued_shares * COALESCE(sc.voting_rights_per_share, 1), 0)) * 100 AS voting_pct,
            h.ownership_nature
        FROM kyc.holdings h
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        JOIN "ob-poc".entities e ON e.entity_id = h.investor_entity_id
        JOIN "ob-poc".entity_types et ON et.type_code = e.entity_type_code
        WHERE sc.issuer_entity_id = p_entity_id
        AND h.status = 'active'
        
        UNION ALL
        
        -- Recursive case: holders of holders (indirect ownership)
        SELECT 
            h.investor_entity_id,
            e.name,
            et.entity_category,
            oc.path || h.investor_entity_id,
            oc.depth + 1,
            CASE WHEN v_rules.cumulative_threshold 
                 THEN oc.ownership_pct * (h.units / NULLIF(sc.issued_shares, 0))
                 ELSE (h.units / NULLIF(sc.issued_shares, 0)) * 100
            END,
            CASE WHEN v_rules.cumulative_threshold
                 THEN oc.voting_pct * (h.units * COALESCE(sc.voting_rights_per_share, 1) / 
                      NULLIF(sc.issued_shares * COALESCE(sc.voting_rights_per_share, 1), 0))
                 ELSE (h.units * COALESCE(sc.voting_rights_per_share, 1) / 
                      NULLIF(sc.issued_shares * COALESCE(sc.voting_rights_per_share, 1), 0)) * 100
            END,
            h.ownership_nature
        FROM ownership_chain oc
        JOIN kyc.share_classes sc ON sc.issuer_entity_id = oc.holder_id
        JOIN kyc.holdings h ON h.share_class_id = sc.id AND h.status = 'active'
        JOIN "ob-poc".entities e ON e.entity_id = h.investor_entity_id
        JOIN "ob-poc".entity_types et ON et.type_code = e.entity_type_code
        WHERE oc.entity_category = 'SHELL'  -- Keep traversing non-terminals
        AND oc.depth < v_rules.max_chain_depth
        AND NOT h.investor_entity_id = ANY(oc.path)  -- Cycle detection
    )
    -- Filter to UBOs based on jurisdiction rules
    SELECT 
        oc.holder_id,
        oc.holder_name,
        CASE 
            WHEN oc.ownership_pct >= v_rules.ownership_threshold_pct THEN 'OWNERSHIP'
            WHEN oc.voting_pct >= COALESCE(v_rules.voting_threshold_pct, 999) THEN 'VOTING'
            ELSE 'CONTROL'
        END::VARCHAR(30),
        CASE 
            WHEN oc.ownership_pct >= v_rules.ownership_threshold_pct 
                THEN 'ownership_' || v_rules.ownership_threshold_pct || '_pct'
            WHEN oc.voting_pct >= COALESCE(v_rules.voting_threshold_pct, 999)
                THEN 'voting_' || v_rules.voting_threshold_pct || '_pct'
            ELSE 'control_rights'
        END::VARCHAR(50),
        oc.ownership_pct::DECIMAL(7,4),
        oc.voting_pct::DECIMAL(7,4),
        oc.path,
        oc.depth,
        oc.ownership_nature = 'BENEFICIAL',
        false,  -- Not fallback
        NULL::TEXT
    FROM ownership_chain oc
    WHERE oc.entity_category = 'PERSON'  -- Terminal = natural person
    AND (
        oc.ownership_pct >= v_rules.ownership_threshold_pct
        OR oc.voting_pct >= COALESCE(v_rules.voting_threshold_pct, 999)
    );
    
    -- TODO: Add control-based UBOs (board control)
    -- TODO: Add fallback UBOs (senior managers) if no ownership UBOs found
    
END;
$$ LANGUAGE plpgsql;
```

### Group Membership

```sql
-- =============================================================================
-- fn_compute_group_members
-- =============================================================================
-- Traverses ownership graph from anchor to find all group members

CREATE OR REPLACE FUNCTION kyc.fn_compute_group_members(
    p_anchor_entity_id UUID,
    p_threshold_pct DECIMAL DEFAULT 50.00,
    p_max_depth INTEGER DEFAULT 10
) RETURNS TABLE (
    entity_id UUID,
    entity_name TEXT,
    relationship_type VARCHAR(30),
    ownership_pct DECIMAL(7,4),
    depth_from_anchor INTEGER,
    path_from_anchor UUID[]
) AS $$
BEGIN
    RETURN QUERY
    WITH RECURSIVE group_tree AS (
        -- Anchor
        SELECT 
            e.entity_id,
            e.name,
            'ANCHOR'::VARCHAR(30) AS rel_type,
            100.00::DECIMAL(7,4) AS own_pct,
            0 AS depth,
            ARRAY[e.entity_id] AS path
        FROM "ob-poc".entities e
        WHERE e.entity_id = p_anchor_entity_id
        
        UNION ALL
        
        -- Subsidiaries (entities where parent holds >threshold%)
        SELECT 
            child_e.entity_id,
            child_e.name,
            'SUBSIDIARY'::VARCHAR(30),
            ((h.units / NULLIF(sc.issued_shares, 0)) * 100)::DECIMAL(7,4),
            gt.depth + 1,
            gt.path || child_e.entity_id
        FROM group_tree gt
        JOIN kyc.holdings h ON h.investor_entity_id = gt.entity_id
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        JOIN "ob-poc".entities child_e ON child_e.entity_id = sc.issuer_entity_id
        WHERE h.status = 'active'
        AND (h.units / NULLIF(sc.issued_shares, 0)) * 100 >= p_threshold_pct
        AND gt.depth < p_max_depth
        AND NOT child_e.entity_id = ANY(gt.path)  -- Cycle detection
    )
    SELECT 
        gt.entity_id,
        gt.entity_name,
        gt.rel_type,
        gt.own_pct,
        gt.depth,
        gt.path
    FROM group_tree gt
    ORDER BY gt.depth, gt.entity_name;
END;
$$ LANGUAGE plpgsql;
```

---

## DSL Verbs

### ownership.yaml

```yaml
domains:
  ownership:
    description: "Ownership graph management"
    
    verbs:
      # =====================================================================
      # EDGE MANAGEMENT
      # =====================================================================
      
      add-holding:
        description: "Add ownership edge (holding)"
        behavior: crud
        crud:
          operation: insert
          table: holdings
          schema: kyc
          returning: id
        args:
          - name: investor-entity-id
            type: uuid
            required: true
            maps_to: investor_entity_id
            lookup:
              table: entities
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: share-class-id
            type: uuid
            required: true
            maps_to: share_class_id
          - name: units
            type: decimal
            required: true
            maps_to: units
          - name: ownership-nature
            type: string
            default: "BENEFICIAL"
            maps_to: ownership_nature
            valid_values: [BENEFICIAL, LEGAL_ONLY, INDIRECT]
        returns:
          type: uuid
          capture: true

      add-synthetic-holder:
        description: "Add synthetic holder for ownership gaps"
        behavior: crud
        crud:
          operation: insert
          table: synthetic_holders
          schema: kyc
          returning: synthetic_id
        args:
          - name: issuer-entity-id
            type: uuid
            required: true
            maps_to: issuer_entity_id
          - name: holder-type
            type: string
            required: true
            maps_to: holder_type
            valid_values: [PUBLIC_FLOAT, NOMINEE_POOL, UNACCOUNTED, BEARER_SHARES, PENDING_SETTLEMENT]
          - name: estimated-pct
            type: decimal
            required: true
            maps_to: estimated_pct
          - name: description
            type: string
            maps_to: description
        returns:
          type: uuid
          capture: true

      add-control:
        description: "Add non-ownership control relationship"
        behavior: crud
        crud:
          operation: insert
          table: control_relationships
          schema: kyc
          returning: control_id
        args:
          - name: controller-entity-id
            type: uuid
            required: true
            maps_to: controller_entity_id
          - name: controlled-entity-id
            type: uuid
            required: true
            maps_to: controlled_entity_id
          - name: control-type
            type: string
            required: true
            maps_to: control_type
            valid_values: [BOARD_APPOINTMENT, VOTING_AGREEMENT, MANAGEMENT_CONTRACT, VETO_RIGHTS, GOLDEN_SHARE, OTHER_CONTROL]
          - name: control-basis
            type: string
            maps_to: control_basis
        returns:
          type: uuid
          capture: true

      # =====================================================================
      # COVERAGE
      # =====================================================================
      
      compute-coverage:
        description: "Compute ownership coverage metrics for an entity"
        behavior: plugin
        handler: OwnershipComputeCoverageOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: as-of-date
            type: date
        returns:
          type: object
          description: "Coverage metrics"

      get-coverage:
        description: "Get stored coverage metrics"
        behavior: crud
        crud:
          operation: select
          table: ownership_coverage
          schema: kyc
        args:
          - name: entity-id
            type: uuid
            required: true
            maps_to: entity_id

      identify-gaps:
        description: "Identify ownership gaps requiring research"
        behavior: plugin
        handler: OwnershipIdentifyGapsOp
        args:
          - name: entity-id
            type: uuid
            required: true
        returns:
          type: array
          description: "List of gaps with research triggers"

      # =====================================================================
      # TRAVERSAL
      # =====================================================================
      
      trace-chain:
        description: "Trace ownership chain from entity to terminals"
        behavior: plugin
        handler: OwnershipTraceChainOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: max-depth
            type: integer
            default: 10
        returns:
          type: object
          description: "Ownership chain with terminals and gaps"

      detect-cycles:
        description: "Detect circular ownership structures"
        behavior: plugin
        handler: OwnershipDetectCyclesOp
        args:
          - name: entity-id
            type: uuid
            required: true
        returns:
          type: array
          description: "List of cycles found"
```

### ubo.yaml

```yaml
domains:
  ubo:
    description: "UBO computation and management"
    
    verbs:
      # =====================================================================
      # COMPUTATION
      # =====================================================================
      
      compute:
        description: "Compute UBOs for entity under jurisdiction rules"
        behavior: plugin
        handler: UboComputeOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: jurisdiction
            type: string
            required: true
            valid_values: [EU, UK, US, KY, IE, LU]
          - name: as-of-date
            type: date
        returns:
          type: array
          description: "List of UBOs with basis"

      compute-for-cbu:
        description: "Compute UBOs for a CBU (uses CBU's jurisdiction)"
        behavior: plugin
        handler: UboComputeForCbuOp
        args:
          - name: cbu-id
            type: uuid
            required: true
        returns:
          type: array
          description: "List of UBOs with basis"

      apply-fallback:
        description: "Apply senior manager fallback when no UBO found"
        behavior: plugin
        handler: UboApplyFallbackOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: jurisdiction
            type: string
            required: true
        returns:
          type: array
          description: "Fallback UBOs (senior managers)"

      # =====================================================================
      # RULES
      # =====================================================================
      
      list-rules:
        description: "List all jurisdiction rules"
        behavior: crud
        crud:
          operation: select
          table: ubo_jurisdiction_rules
          schema: kyc

      get-rule:
        description: "Get rule for specific jurisdiction"
        behavior: crud
        crud:
          operation: select
          table: ubo_jurisdiction_rules
          schema: kyc
        args:
          - name: jurisdiction
            type: string
            required: true
            maps_to: jurisdiction_code

      create-rule:
        description: "Create jurisdiction rule"
        behavior: crud
        crud:
          operation: insert
          table: ubo_jurisdiction_rules
          schema: kyc
          returning: rule_id
        args:
          - name: jurisdiction-code
            type: string
            required: true
            maps_to: jurisdiction_code
          - name: jurisdiction-name
            type: string
            maps_to: jurisdiction_name
          - name: ownership-threshold-pct
            type: decimal
            default: 25.00
            maps_to: ownership_threshold_pct
          - name: voting-threshold-pct
            type: decimal
            maps_to: voting_threshold_pct
          - name: senior-manager-fallback
            type: boolean
            default: true
            maps_to: senior_manager_fallback
          - name: effective-from
            type: date
            required: true
            maps_to: effective_from
        returns:
          type: uuid
          capture: true

      # =====================================================================
      # EXPORT
      # =====================================================================
      
      export-bods:
        description: "Export UBOs as BODS 0.4 document"
        behavior: plugin
        handler: UboExportBodsOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: jurisdiction
            type: string
            required: true
          - name: include-gaps
            type: boolean
            default: true
        returns:
          type: object
          description: "BODS document"
```

### group.yaml

```yaml
domains:
  group:
    description: "Corporate group management"
    
    verbs:
      # =====================================================================
      # GROUP LIFECYCLE
      # =====================================================================
      
      create:
        description: "Create corporate group"
        behavior: crud
        crud:
          operation: insert
          table: ownership_groups
          schema: kyc
          returning: group_id
        args:
          - name: anchor-entity-id
            type: uuid
            required: true
            maps_to: anchor_entity_id
          - name: group-name
            type: string
            required: true
            maps_to: group_name
          - name: group-type
            type: string
            required: true
            maps_to: group_type
            valid_values: [MANCO, BANK, INSURANCE, CORPORATE, PE_GP, FAMILY_OFFICE]
          - name: membership-threshold-pct
            type: decimal
            default: 50.00
            maps_to: membership_threshold_pct
        returns:
          type: uuid
          capture: true

      compute-members:
        description: "Compute group membership by traversing ownership"
        behavior: plugin
        handler: GroupComputeMembersOp
        args:
          - name: group-id
            type: uuid
            required: true
        returns:
          type: array
          description: "List of group member entities"

      link-cbu:
        description: "Link CBU to group"
        behavior: crud
        crud:
          operation: insert
          table: group_cbu_links
          schema: kyc
        args:
          - name: group-id
            type: uuid
            required: true
            maps_to: group_id
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
          - name: servicing-entity-id
            type: uuid
            required: true
            maps_to: servicing_entity_id
          - name: service-role
            type: string
            required: true
            maps_to: service_role
            valid_values: [MANAGER, ADMINISTRATOR, CUSTODIAN, DEPOSITARY, DISTRIBUTOR]

      list-cbus:
        description: "List CBUs serviced by group"
        behavior: crud
        crud:
          operation: select
          table: group_cbu_links
          schema: kyc
        args:
          - name: group-id
            type: uuid
            required: true
            maps_to: group_id

      # =====================================================================
      # GROUP UBO
      # =====================================================================
      
      compute-ubos:
        description: "Compute UBOs for entire group"
        behavior: plugin
        handler: GroupComputeUbosOp
        args:
          - name: group-id
            type: uuid
            required: true
          - name: jurisdiction
            type: string
            required: true
        returns:
          type: object
          description: "Group UBO structure"

      export-bods:
        description: "Export group ownership as BODS"
        behavior: plugin
        handler: GroupExportBodsOp
        args:
          - name: group-id
            type: uuid
            required: true
          - name: jurisdiction
            type: string
            required: true
        returns:
          type: object
          description: "BODS document for entire group"

      # =====================================================================
      # GLEIF INTEGRATION
      # =====================================================================
      
      validate-gleif:
        description: "Validate group ownership against GLEIF"
        behavior: plugin
        handler: GroupValidateGleifOp
        args:
          - name: group-id
            type: uuid
            required: true
        returns:
          type: object
          description: "Validation report with discrepancies"

      import-gleif-hierarchy:
        description: "Import/refresh group structure from GLEIF"
        behavior: plugin
        handler: GroupImportGleifOp
        args:
          - name: group-id
            type: uuid
            required: true
          - name: anchor-lei
            type: string
            required: true
```

### research.yaml

```yaml
domains:
  research:
    description: "Ownership research workflows"
    
    verbs:
      # =====================================================================
      # TRIGGER MANAGEMENT
      # =====================================================================
      
      list-triggers:
        description: "List research triggers for entity"
        behavior: crud
        crud:
          operation: select
          table: ownership_research_triggers
          schema: kyc
        args:
          - name: entity-id
            type: uuid
            maps_to: target_entity_id
          - name: status
            type: string
            maps_to: status
            valid_values: [OPEN, IN_PROGRESS, RESOLVED, WONT_FIX]

      create-trigger:
        description: "Create research trigger"
        behavior: crud
        crud:
          operation: insert
          table: ownership_research_triggers
          schema: kyc
          returning: trigger_id
        args:
          - name: target-entity-id
            type: uuid
            required: true
            maps_to: target_entity_id
          - name: research-type
            type: string
            required: true
            maps_to: research_type
            valid_values: [NOMINEE_DISCLOSURE, GLEIF_LOOKUP, OWNERSHIP_DECLARATION, REGISTER_RECONCILE, BOARD_COMPOSITION, CYCLE_REVIEW, CHAIN_COMPLETION]
          - name: description
            type: string
            maps_to: description
          - name: affected-pct
            type: decimal
            maps_to: affected_pct
          - name: priority
            type: string
            default: "MEDIUM"
            maps_to: priority
            valid_values: [HIGH, MEDIUM, LOW]
        returns:
          type: uuid
          capture: true

      resolve-trigger:
        description: "Resolve research trigger"
        behavior: crud
        crud:
          operation: update
          table: ownership_research_triggers
          schema: kyc
        args:
          - name: trigger-id
            type: uuid
            required: true
            maps_to: trigger_id
          - name: resolution-notes
            type: string
            maps_to: resolution_notes

      # =====================================================================
      # RESEARCH ACTIONS
      # =====================================================================
      
      request-disclosure:
        description: "Request nominee look-through disclosure"
        behavior: plugin
        handler: ResearchRequestDisclosureOp
        args:
          - name: nominee-entity-id
            type: uuid
            required: true
          - name: target-entity-id
            type: uuid
            required: true

      request-declaration:
        description: "Request ownership declaration from entity"
        behavior: plugin
        handler: ResearchRequestDeclarationOp
        args:
          - name: entity-id
            type: uuid
            required: true

      lookup-gleif:
        description: "Look up entity in GLEIF and import hierarchy"
        behavior: plugin
        handler: ResearchGleifLookupOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: lei
            type: string

      reconcile-register:
        description: "Reconcile holdings with external share register"
        behavior: plugin
        handler: ResearchReconcileRegisterOp
        args:
          - name: issuer-entity-id
            type: uuid
            required: true
          - name: register-source
            type: string
            required: true
```

---

## Key Files

| File | Purpose |
|------|---------|
| `rust/src/ownership/graph.rs` | Ownership graph model and traversal |
| `rust/src/ownership/coverage.rs` | Coverage computation |
| `rust/src/ubo/compute.rs` | UBO computation engine |
| `rust/src/ubo/rules.rs` | Jurisdiction rules |
| `rust/src/group/members.rs` | Group membership computation |
| `rust/src/api/ownership_routes.rs` | API endpoints |
| `rust/config/verbs/ownership.yaml` | Ownership verbs |
| `rust/config/verbs/ubo.yaml` | UBO verbs |
| `rust/config/verbs/group.yaml` | Group verbs |
| `rust/config/verbs/research.yaml` | Research verbs |
| `migrations/015_ownership_graph.sql` | Schema |

---

## Implementation Phases

### Phase 1: Schema (10h)
- [ ] 1.1 Add `ownership_nature` to holdings
- [ ] 1.2 Create `synthetic_holders` table
- [ ] 1.3 Create `control_relationships` table
- [ ] 1.4 Create `ownership_coverage` table
- [ ] 1.5 Create `ubo_jurisdiction_rules` table
- [ ] 1.6 Create `ownership_groups` table
- [ ] 1.7 Create `group_cbu_links` table
- [ ] 1.8 Create `ownership_research_triggers` table
- [ ] 1.9 Seed jurisdiction rules

### Phase 2: Coverage Computation (8h)
- [ ] 2.1 Implement `fn_compute_ownership_coverage`
- [ ] 2.2 Implement coverage breakdown by ownership_nature
- [ ] 2.3 Implement status determination
- [ ] 2.4 Implement research trigger generation
- [ ] 2.5 Add coverage refresh trigger on holding changes

### Phase 3: Ownership Traversal (10h)
- [ ] 3.1 Implement recursive ownership chain CTE
- [ ] 3.2 Implement cycle detection
- [ ] 3.3 Implement terminal classification
- [ ] 3.4 Implement broken chain detection
- [ ] 3.5 Implement control edge traversal

### Phase 4: UBO Computation (12h)
- [ ] 4.1 Implement `fn_compute_ubos`
- [ ] 4.2 Implement ownership threshold filtering
- [ ] 4.3 Implement voting threshold filtering
- [ ] 4.4 Implement control-based UBO detection
- [ ] 4.5 Implement exemption handling (listed, regulated)
- [ ] 4.6 Implement senior manager fallback
- [ ] 4.7 Implement cumulative vs per-link threshold

### Phase 5: Group Management (8h)
- [ ] 5.1 Implement `fn_compute_group_members`
- [ ] 5.2 Implement group CBU linking
- [ ] 5.3 Implement group-level UBO computation
- [ ] 5.4 Implement GLEIF validation

### Phase 6: BODS Integration (6h)
- [ ] 6.1 Implement entity BODS export
- [ ] 6.2 Implement group BODS export
- [ ] 6.3 Implement gap handling (unableToConfirm)
- [ ] 6.4 Implement BODS import

### Phase 7: DSL Verbs (8h)
- [ ] 7.1 Implement ownership verbs
- [ ] 7.2 Implement ubo verbs
- [ ] 7.3 Implement group verbs
- [ ] 7.4 Implement research verbs

### Phase 8: Research Workflow (6h)
- [ ] 8.1 Implement trigger generation from gaps
- [ ] 8.2 Implement disclosure request workflow
- [ ] 8.3 Implement GLEIF lookup workflow
- [ ] 8.4 Implement register reconciliation

### Phase 9: Testing (10h)
- [ ] 9.1 Test coverage computation
- [ ] 9.2 Test chain traversal
- [ ] 9.3 Test UBO computation by jurisdiction
- [ ] 9.4 Test group membership
- [ ] 9.5 Test cycle detection
- [ ] 9.6 Test broken chain handling
- [ ] 9.7 Test BODS export
- [ ] 9.8 Test with complex real-world structure (AllianzGI-style)

---

## Estimated Effort

| Phase | Effort |
|-------|--------|
| 1. Schema | 10h |
| 2. Coverage | 8h |
| 3. Traversal | 10h |
| 4. UBO Computation | 12h |
| 5. Group Management | 8h |
| 6. BODS Integration | 6h |
| 7. DSL Verbs | 8h |
| 8. Research Workflow | 6h |
| 9. Testing | 10h |
| **Total** | **~78h** |

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Deep chains cause performance issues | Cap depth, use materialized views |
| Circular ownership breaks traversal | Cycle detection with path tracking |
| Jurisdiction rules change | Temporal rules table with effective dates |
| Gaps block KYC completion | Research workflow, not errors |
| GLEIF data doesn't match internal | Discrepancy report, manual override |

---

## Success Criteria

1. **Coverage computed** - Every issuer has coverage metrics
2. **UBO dynamic** - Same graph → different UBOs per jurisdiction
3. **Gaps accepted** - Incomplete data drives research, not errors
4. **Groups shared** - Group UBO computed once, referenced by CBUs
5. **BODS compliant** - Export handles gaps with proper reasons
6. **Rules configurable** - Jurisdiction thresholds in data, not code

---

Generated: 2026-01-10
