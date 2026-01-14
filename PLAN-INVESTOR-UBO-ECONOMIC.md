# Implementation Plan: Investor Register + UBO Control vs Economic Exposure

> **Created:** 2026-01-14  
> **Revised:** 2026-01-14 (incorporated user feedback)  
> **Status:** PENDING USER APPROVAL  
> **Source:** TODO-INVESTOR-UBO-ECONOMIC-PIPELINE.md (ChatGPT analysis)

---

## Executive Summary

The TODO document from ChatGPT is **strategically sound** and **mostly accurate** about existing schema. However, verification revealed:

1. **Critical verb YAML mismatches** - DSL verbs won't work as-is
2. **UBO sync trigger flaw** - Currently syncs ALL holdings ≥25%, not just UBO-intent holdings
3. **Missing tables** - `investor_role_profiles`, `fund_vehicles`, `fund_compartments` don't exist
4. **Some hallucinations** - `investor_name` and `tax_residence` columns don't exist in `kyc.investors`

**Core Architecture Validated:** The "store direct edges only, compute look-through on-demand" pattern is correct. Materialized implied edges become unmaintainable at scale (1K investors × 200 SPVs × N tiers = millions of edges).

---

## Verification Results

### What the TODO Got RIGHT

| Claim | Status | Evidence |
|-------|--------|----------|
| `kyc.investors.owning_cbu_id` exists | ✅ Correct | Column exists (not `cbu_id`) |
| `kyc.holdings.usage_type` exists | ✅ Correct | Default `'TA'` |
| `kyc.ownership_snapshots` exists | ✅ Correct | Migration 013 |
| `kyc.special_rights` exists | ✅ Correct | Migration 013 |
| `ob-poc.control_edges` exists | ✅ Correct | Migration 022 |
| UBO trigger at ≥25% threshold | ✅ Correct | But doesn't check `usage_type` |

### What the TODO Got WRONG (Hallucinations)

| Claim | Reality |
|-------|---------|
| `kyc.investors.investor_name` column | ❌ Does NOT exist - name comes from `entities.name` via FK |
| `kyc.investors.tax_residence` column | ❌ Does NOT exist - column is `tax_jurisdiction` |
| `ob-poc.fund_investments` in main migrations | ❌ Exists but in pre-numbered migration, not documented |

### Critical DSL Verb Issues (Will Fail Silently)

| File | Issue | Impact |
|------|-------|--------|
| `investor.yaml` | `cbu-id` maps to `cbu_id` | Column is `owning_cbu_id` - inserts fail |
| `investor.yaml` | `investor-name` arg | Column doesn't exist |
| `investor.yaml` | `tax-residence` arg | Column doesn't exist (it's `tax_jurisdiction`) |
| `investor.yaml` | `investor-type` enum | Values `natural_person, legal_entity` vs schema `RETAIL, PROFESSIONAL, INSTITUTIONAL` |
| `holding.yaml` | `usage-type` enum | Values `ta_kyc, ubo_tracking` vs schema `TA, UBO` |
| `holding.yaml` | `holding-status` enum | 6 values vs schema 4, lowercase vs uppercase |

---

## Implementation Plan

### Phase A: Fix DSL Verb YAML (Critical - Do First)

**Why:** Without this, agent-generated DSL fails silently. This is blocking for any investor register workflow.

#### A1. Fix `investor.yaml`

```yaml
# CHANGES NEEDED:

# 1. Rename cbu-id → owning-cbu-id
- name: owning-cbu-id          # was: cbu-id
  maps_to: owning_cbu_id       # was: cbu_id

# 2. REMOVE non-existent columns
# DELETE: investor-name (use entity.name via FK)
# DELETE: tax-residence (column doesn't exist)

# 3. Fix investor-type enum to match schema
- name: investor-type
  validation:
    enum:
      - RETAIL
      - PROFESSIONAL  
      - INSTITUTIONAL
      - NOMINEE
      - INTRA_GROUP
```

**Files:** `rust/config/verbs/registry/investor.yaml`  
**Estimate:** 30 min

#### A2. Fix `holding.yaml`

```yaml
# CHANGES NEEDED:

# 1. Fix usage-type enum to match schema
- name: usage-type
  validation:
    enum:
      - TA           # was: ta_kyc
      - UBO          # was: ubo_tracking

# 2. Fix holding-status enum to match schema (uppercase, 5 values)
- name: holding-status
  default: ACTIVE    # was: pending
  validation:
    enum:
      - PENDING
      - ACTIVE
      - SUSPENDED
      - TRANSFERRED   # Added for movement tracking
      - CLOSED
```

**Files:** `rust/config/verbs/registry/holding.yaml`  
**Estimate:** 20 min

#### A3. Add Regression Test

Create DSL test fixture that exercises investor + holding verbs end-to-end.

**Files:** `rust/tests/fixtures/investor_holding_regression.dsl`  
**Estimate:** 30 min

---

### Phase B: Add Holder Role Profiles (Prevents FoF/Pool → UBO Misclassification)

**Why:** The current trigger treats ALL ≥25% holders as UBO candidates. For institutional FoF structures (Allianz), pooled vehicles should NOT create UBO edges.

**Design Elegance:** Issuer-scoped holder roles solve "same entity, different treatment" cleanly—AllianzLife can be an end-investor in Fund A but a master pool operator for Fund B.

#### B1. New Migration: `024_investor_role_profiles.sql`

```sql
CREATE TABLE IF NOT EXISTS kyc.investor_role_profiles (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  holder_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  share_class_id UUID NULL REFERENCES kyc.share_classes(id),
  
  role_type VARCHAR(50) NOT NULL,
  lookthrough_policy VARCHAR(30) NOT NULL DEFAULT 'NONE',
  holder_affiliation VARCHAR(20) NOT NULL DEFAULT 'UNKNOWN',
  beneficial_owner_data_available BOOLEAN NOT NULL DEFAULT false,
  is_ubo_eligible BOOLEAN NOT NULL DEFAULT true,
  
  -- TEMPORAL VERSIONING (user feedback: point-in-time queries needed)
  effective_from DATE NOT NULL DEFAULT CURRENT_DATE,
  effective_to DATE NULL,  -- NULL = current
  
  source VARCHAR(50) DEFAULT 'MANUAL',
  notes TEXT NULL,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  
  -- Role type enum
  CONSTRAINT chk_role_type CHECK (role_type IN (
    'END_INVESTOR',
    'NOMINEE',
    'OMNIBUS',
    'INTERMEDIARY_FOF',
    'MASTER_POOL',
    'INTRA_GROUP_POOL',
    'TREASURY',
    'OTHER'
  )),
  
  -- Lookthrough policy enum
  CONSTRAINT chk_lookthrough CHECK (lookthrough_policy IN (
    'NONE',
    'ON_DEMAND',
    'AUTO_IF_DATA',
    'ALWAYS'
  )),
  
  -- Holder affiliation enum
  CONSTRAINT chk_holder_affiliation CHECK (holder_affiliation IN (
    'INTRA_GROUP',
    'EXTERNAL',
    'MIXED',
    'UNKNOWN'
  )),
  
  -- Unique constraint (temporal: only one active profile per issuer+holder+share_class)
  CONSTRAINT uq_role_profile_active UNIQUE (
    issuer_entity_id, 
    holder_entity_id, 
    COALESCE(share_class_id, '00000000-0000-0000-0000-000000000000'::uuid),
    COALESCE(effective_to, '9999-12-31'::date)
  )
);

CREATE INDEX idx_role_profiles_issuer ON kyc.investor_role_profiles(issuer_entity_id);
CREATE INDEX idx_role_profiles_holder ON kyc.investor_role_profiles(holder_entity_id);
CREATE INDEX idx_role_profiles_active ON kyc.investor_role_profiles(issuer_entity_id, holder_entity_id) 
  WHERE effective_to IS NULL;
```

**Estimate:** 45 min

#### B2. DSL Verbs for Role Profiles

New file: `rust/config/verbs/registry/investor-role.yaml`

- `investor-role.set` (upsert with temporal close of previous)
- `investor-role.read` (current or as-of-date)
- `investor-role.list-by-issuer`
- `investor-role.history` (all versions for a holder)

**Estimate:** 1 hour

#### B3. Patch UBO Sync Trigger

Modify `kyc.sync_holding_to_ubo_relationship()` to:
1. Only fire when `NEW.usage_type = 'UBO'`
2. Check `investor_role_profiles.is_ubo_eligible` (current version)
3. Default-deny for pooled vehicle role types

```sql
-- Add to trigger function:
IF NEW.usage_type != 'UBO' THEN
    RETURN NEW;  -- Skip TA holdings
END IF;

-- Check role profile (current version only)
SELECT is_ubo_eligible INTO v_is_eligible
FROM kyc.investor_role_profiles
WHERE holder_entity_id = NEW.investor_entity_id
  AND issuer_entity_id = v_fund_entity_id
  AND effective_to IS NULL;  -- Current version

IF v_is_eligible = false THEN
    RETURN NEW;  -- Skip ineligible holders
END IF;
```

**Estimate:** 45 min

---

### Phase C: Add Fund Vehicle Taxonomy (Supports Allianz Structure)

**Why:** To properly represent FoF/umbrella/master pool structures without treating them as UBOs.

#### C1. New Migration: `025_fund_vehicles.sql`

```sql
CREATE TABLE IF NOT EXISTS kyc.fund_vehicles (
  fund_entity_id UUID PRIMARY KEY REFERENCES "ob-poc".entities(entity_id),
  vehicle_type VARCHAR(30) NOT NULL,
  umbrella_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id),
  domicile_country CHAR(2) NULL,
  manager_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id),
  is_umbrella BOOLEAN NOT NULL DEFAULT false,
  meta JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ DEFAULT now(),
  
  -- Vehicle type enum (expanded for broader use)
  CONSTRAINT chk_vehicle_type CHECK (vehicle_type IN (
    'SCSP',           -- Luxembourg SCSp
    'SICAV_RAIF',     -- Luxembourg SICAV-RAIF
    'SICAV_SIF',      -- Luxembourg SICAV-SIF  
    'SIF',            -- Luxembourg SIF
    'SICAV_UCITS',    -- UCITS umbrella
    'FCP',            -- Fonds Commun de Placement
    'LLC',            -- US LLC
    'LP',             -- Limited Partnership (generic)
    'TRUST',          -- Unit trust structure
    'OEIC',           -- UK Open-Ended Investment Company
    'ETF',            -- Exchange-traded fund
    'OTHER'
  ))
);

CREATE TABLE IF NOT EXISTS kyc.fund_compartments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  umbrella_fund_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  compartment_code TEXT NOT NULL,
  compartment_name TEXT NULL,
  meta JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT uq_compartment UNIQUE (umbrella_fund_entity_id, compartment_code)
);

CREATE INDEX idx_fund_vehicles_umbrella ON kyc.fund_vehicles(umbrella_entity_id);
CREATE INDEX idx_fund_vehicles_manager ON kyc.fund_vehicles(manager_entity_id);
CREATE INDEX idx_fund_compartments_umbrella ON kyc.fund_compartments(umbrella_fund_entity_id);
```

**Estimate:** 30 min

#### C2. DSL Verbs for Fund Vehicles

New file: `rust/config/verbs/registry/fund-vehicle.yaml`

- `fund-vehicle.upsert`
- `fund-vehicle.read`
- `fund-compartment.upsert`
- `fund-compartment.list`

**Estimate:** 1 hour

#### C3. Extend share_classes with instrument_type

Add column to existing table:

```sql
ALTER TABLE kyc.share_classes 
ADD COLUMN IF NOT EXISTS instrument_type VARCHAR(30) DEFAULT 'SHARES';

COMMENT ON COLUMN kyc.share_classes.instrument_type IS 
'UNITS, SHARES, LP_INTEREST, PARTNERSHIP_INTEREST, NOMINEE_POSITION, TRACKING_SHARES, CARRIED_INTEREST';
```

**Instrument types expanded (user feedback):**
- `TRACKING_SHARES` - for synthetic exposure in internal allocation models
- `CARRIED_INTEREST` - for GP economics

**Estimate:** 15 min

---

### Phase D: Economic Look-Through (Bounded, On-Demand)

**Why:** Avoid cartesian edge explosion. 1000 investors × 200 SPVs = 200K edges if materialized.

#### D1. Create Economic Edges View

```sql
CREATE OR REPLACE VIEW kyc.v_economic_edges_direct AS
SELECT 
    os.owner_entity_id AS from_entity_id,
    os.issuer_entity_id AS to_entity_id,
    os.percentage AS pct_of_to,
    sc.instrument_type,
    os.share_class_id,
    fv.vehicle_type,
    os.basis,
    'OWNERSHIP_SNAPSHOT' AS source,
    os.as_of_date
FROM kyc.ownership_snapshots os
LEFT JOIN kyc.share_classes sc ON os.share_class_id = sc.id
LEFT JOIN kyc.fund_vehicles fv ON os.issuer_entity_id = fv.fund_entity_id
WHERE os.basis = 'ECONOMIC'
  AND os.is_direct = true
  AND os.superseded_at IS NULL;
```

**Estimate:** 30 min

#### D2. Bounded Look-Through Function with Explicit Stop Conditions

**User feedback incorporated:** Explicit precedence for stop conditions and cycle detection.

```sql
CREATE OR REPLACE FUNCTION kyc.fn_compute_economic_exposure(
    p_root_entity_id UUID,
    p_as_of_date DATE DEFAULT CURRENT_DATE,
    p_max_depth INT DEFAULT 6,
    p_min_pct NUMERIC DEFAULT 0.0001,
    p_max_rows INT DEFAULT 200,
    -- Explicit stop condition config (user feedback)
    p_stop_on_no_bo_data BOOLEAN DEFAULT true,
    p_stop_on_policy_none BOOLEAN DEFAULT true
) RETURNS TABLE (
    root_entity_id UUID,
    leaf_entity_id UUID,
    cumulative_pct NUMERIC,
    depth INT,
    path_entities UUID[],
    stopped_reason TEXT  -- Why traversal stopped at this leaf
) AS $$
WITH RECURSIVE exposure_tree AS (
    -- Base case: direct holdings from root
    SELECT 
        p_root_entity_id AS root_id,
        e.to_entity_id AS current_id,
        e.pct_of_to AS cumulative_pct,
        1 AS depth,
        ARRAY[p_root_entity_id, e.to_entity_id] AS path,
        CASE 
            WHEN rp.lookthrough_policy = 'NONE' AND p_stop_on_policy_none THEN 'POLICY_NONE'
            WHEN rp.beneficial_owner_data_available = false AND p_stop_on_no_bo_data THEN 'NO_BO_DATA'
            ELSE NULL
        END AS stop_reason
    FROM kyc.v_economic_edges_direct e
    LEFT JOIN kyc.investor_role_profiles rp 
        ON rp.holder_entity_id = e.from_entity_id
        AND rp.issuer_entity_id = e.to_entity_id
        AND rp.effective_to IS NULL
    WHERE e.from_entity_id = p_root_entity_id
      AND e.as_of_date <= p_as_of_date
    
    UNION ALL
    
    -- Recursive case: traverse deeper
    SELECT 
        t.root_id,
        e.to_entity_id,
        t.cumulative_pct * (e.pct_of_to / 100),
        t.depth + 1,
        t.path || e.to_entity_id,
        CASE 
            -- STOP CONDITION PRECEDENCE (user feedback):
            -- 1. Cycle detection (highest priority - prevents infinite loops)
            WHEN e.to_entity_id = ANY(t.path) THEN 'CYCLE_DETECTED'
            -- 2. Depth limit
            WHEN t.depth + 1 >= p_max_depth THEN 'MAX_DEPTH'
            -- 3. Percentage threshold
            WHEN t.cumulative_pct * (e.pct_of_to / 100) < p_min_pct THEN 'BELOW_MIN_PCT'
            -- 4. Lookthrough policy
            WHEN rp.lookthrough_policy = 'NONE' AND p_stop_on_policy_none THEN 'POLICY_NONE'
            -- 5. BO data availability
            WHEN rp.beneficial_owner_data_available = false AND p_stop_on_no_bo_data THEN 'NO_BO_DATA'
            ELSE NULL
        END AS stop_reason
    FROM exposure_tree t
    JOIN kyc.v_economic_edges_direct e ON e.from_entity_id = t.current_id
    LEFT JOIN kyc.investor_role_profiles rp 
        ON rp.holder_entity_id = e.from_entity_id
        AND rp.issuer_entity_id = e.to_entity_id
        AND rp.effective_to IS NULL
    WHERE t.stop_reason IS NULL  -- Only continue if not stopped
      AND t.depth < p_max_depth
      AND t.cumulative_pct >= p_min_pct
      AND e.as_of_date <= p_as_of_date
      -- CYCLE DETECTION (user feedback): prevent visiting same node twice
      AND NOT (e.to_entity_id = ANY(t.path))
)
SELECT 
    root_id,
    current_id,
    cumulative_pct,
    depth,
    path,
    COALESCE(stop_reason, 'LEAF_NODE') AS stopped_reason
FROM exposure_tree
WHERE stop_reason IS NOT NULL  -- Only return leaf nodes
   OR NOT EXISTS (  -- Or nodes with no further edges
        SELECT 1 FROM kyc.v_economic_edges_direct e2 
        WHERE e2.from_entity_id = exposure_tree.current_id
   )
ORDER BY cumulative_pct DESC
LIMIT p_max_rows;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION kyc.fn_compute_economic_exposure IS 
'Bounded look-through computation. Stop condition precedence:
1. CYCLE_DETECTED - prevents infinite loops in malformed data
2. MAX_DEPTH - hard depth limit
3. BELOW_MIN_PCT - percentage threshold
4. POLICY_NONE - role profile says no lookthrough
5. NO_BO_DATA - beneficial owner data unavailable';
```

**Estimate:** 2.5 hours (complex recursive SQL with cycle detection)

#### D3. Skip Cache Table (User Feedback)

**Decision:** Skip the cache table for now. Rely on bounded query limits to keep response times acceptable. Cache invalidation complexity (holdings change, role profiles change, fund structure changes) outweighs benefits at current scale.

If performance becomes an issue later, consider:
- Version-bumping approach (coarse but simple)
- LISTEN/NOTIFY for targeted invalidation

**Estimate:** 0 (deferred)

#### D4. Rust API + DSL Integration

- Plugin handler: `EconomicComputeExposureOp`
- API endpoint: `GET /api/economic/exposure`

**Estimate:** 1.5 hours

---

### Phase E: Investor Register Visualization Wiring

#### E1. Query Builder Service

`rust/src/services/investor_register_service.rs`

- Load thresholds from `kyc.issuer_control_config`
- Apply role profiles for collapse rules
- Return `InvestorRegisterView` DTO

**Estimate:** 2 hours

#### E2. API Endpoints

- `GET /api/issuer/{entity_id}/investor-register`
- `GET /api/issuer/{entity_id}/investor-list` (paginated)

**Estimate:** 1 hour

---

### Phase F: Generic Fund Programme Loader (User Feedback)

**Changed from:** Allianz-specific `load-allianz-group` xtask  
**Changed to:** Generic `load-fund-programme` with config-driven schema

#### F1. Config-Driven Loader

```bash
cargo xtask load-fund-programme --config ./data/allianz_programme.yaml --input ./data/allianz_funds.csv
```

**Config schema (YAML):**
```yaml
programme_name: "Allianz Global Investors"
column_mapping:
  lei: "LEI"
  entity_name: "Fund Name"
  vehicle_type: "Vehicle Type"
  umbrella_lei: "Umbrella LEI"
  compartment_code: "Compartment"
  holder_affiliation_default: "Affiliation"
  bo_data_available_default: "BO Data Available"
defaults:
  holder_affiliation: INTRA_GROUP
  bo_data_available: false
vehicle_type_mapping:
  "SCSp": SCSP
  "SICAV-RAIF": SICAV_RAIF
  "SICAV-SIF": SICAV_SIF
```

This allows loading BlackRock iShares, Vanguard, or any other fund programme with different CSV schemas.

**Estimate:** 2 hours

---

### Phase G: Tests & Safety Rails

#### G1. SQL Tests

- TA holding → no UBO edge
- Role profile `is_ubo_eligible=false` → no UBO edge
- UBO holding ≥25% + eligible → edge created
- Temporal: role profile change mid-year, verify point-in-time queries

#### G2. Look-Through Safety Tests

- Verify `max_depth`, `min_pct`, `max_rows` respected
- Verify `lookthrough_policy=NONE` stops traversal
- **Cycle detection test:** Insert circular ownership, verify no infinite loop
- **Stop condition precedence test:** Verify order matches documentation

#### G3. End-to-End DSL Regression

- Full workflow: entity → investor → holding → role profile → verify edges

**Estimate:** 2.5 hours total

---

## Summary: Implementation Order

| Phase | Description | Effort | Priority |
|-------|-------------|--------|----------|
| **A** | Fix DSL verb YAML | 1.5h | **CRITICAL** |
| **B** | Investor role profiles (with temporal) | 3h | HIGH |
| **C** | Fund vehicle taxonomy | 2h | HIGH |
| **D** | Economic look-through (with cycle detection) | 4h | MEDIUM |
| **E** | Visualization wiring | 3h | MEDIUM |
| **F** | Generic fund programme loader | 2h | MEDIUM |
| **G** | Tests (expanded) | 2.5h | HIGH |

**Total estimated effort:** ~18 hours

---

## Key Design Decisions (Incorporated from User Feedback)

| Decision | Rationale |
|----------|-----------|
| Temporal versioning on role profiles | Mid-year reclassifications need point-in-time queries |
| Explicit stop condition precedence | Debugging edge cases requires deterministic behavior |
| Skip cache table | Invalidation complexity outweighs benefits; bounded queries are fast enough |
| Generic fund loader (not Allianz-specific) | Supports BlackRock, Vanguard, etc. with config mapping |
| Cycle detection in look-through | Garbage-in is inevitable; WITH RECURSIVE doesn't protect by default |
| Extended instrument types | Added TRACKING_SHARES, CARRIED_INTEREST for synthetic/GP economics |
| TRANSFERRED holding status | Needed for movement tracking |

---

## Files to be Modified/Created

### Modified
- `rust/config/verbs/registry/investor.yaml`
- `rust/config/verbs/registry/holding.yaml`
- `migrations/011_investor_register.sql` (trigger patch via new migration)

### Created
- `migrations/024_investor_role_profiles.sql`
- `migrations/025_fund_vehicles.sql`
- `migrations/026_patch_ubo_trigger.sql`
- `rust/config/verbs/registry/investor-role.yaml`
- `rust/config/verbs/registry/fund-vehicle.yaml`
- `rust/src/services/investor_register_service.rs`
- `rust/src/dsl_v2/custom_ops/economic_exposure.rs`
- `rust/xtask/src/load_fund_programme.rs`
- `rust/tests/fixtures/investor_holding_regression.dsl`

---

## Entity-Ref Syntax Verification

**User note:** The S-expression `(k lei "...")` syntax must match the DSL parser.

Current entity-ref syntax in this codebase:
```lisp
(entity-ref legal-entity (k lei "529900KRX8A6KQV2XK98"))
```

This matches the existing `entity-ref` resolution pattern. Will verify parser compatibility during implementation.

---

**AWAITING USER APPROVAL BEFORE STARTING ANY CODE CHANGES**
