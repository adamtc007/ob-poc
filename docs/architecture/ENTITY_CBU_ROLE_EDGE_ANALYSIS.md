# Entity / CBU / Role / Edge Architecture Analysis

## Executive Summary

**Critical Finding:** There is a **schema/verb mismatch** in fund hierarchy handling:
- Verbs populate `entity_funds.parent_fund_id`
- Graph queries use `fund_structure` table (which is EMPTY)

This means **fund hierarchies are invisible to graph navigation**.

---

## 1. Edge/Relationship Tables

| Table | Purpose | Row Count | Status |
|-------|---------|-----------|--------|
| `cbu_entity_roles` | Entity → CBU with role | 2,105 | ✅ Populated |
| `entity_relationships` | Entity → Entity (ownership/control/trust) | 15 | ⚠️ Minimal |
| `fund_structure` | Fund hierarchy (umbrella→subfund→shareclass) | **0** | ❌ EMPTY |
| `delegation_relationships` | ManCo → Sub-Advisor delegations | 0 | Not used yet |
| `entity_funds.parent_fund_id` | Alternative fund parent link | 2 | ⚠️ Minimal |

---

## 2. Critical Gap: Fund Hierarchy Disconnect

### The Problem

```
┌─────────────────────────────────────────────────────────────────┐
│                        DSL EXECUTION                            │
│                                                                 │
│  fund.create-subfund :umbrella-id @sicav                       │
│         │                                                       │
│         ▼                                                       │
│  entity_funds.parent_fund_id = @sicav.entity_id  ✅ Written    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                      GRAPH REPOSITORY                           │
│                                                                 │
│  load_fund_edges() queries fund_structure table                │
│         │                                                       │
│         ▼                                                       │
│  fund_structure is EMPTY  ❌ No edges returned                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Result:** Graph navigation cannot traverse fund hierarchies.

### The Fix Options

1. **Update graph_repository.rs** to also query `entity_funds.parent_fund_id`
2. **Add trigger** on `entity_funds` to sync to `fund_structure`
3. **Update verbs** to write to BOTH tables

Recommended: Option 1 (simplest, backward compatible)

---

## 3. cbu_entity_roles Analysis

### Schema
```sql
cbu_entity_role_id    UUID PRIMARY KEY
cbu_id                UUID NOT NULL  -- FK to cbus
entity_id             UUID NOT NULL  -- FK to entities
role_id               UUID NOT NULL  -- FK to roles
target_entity_id      UUID           -- Never used! (0 of 2105 rows)
ownership_percentage  NUMERIC        -- For ownership roles
effective_from/to     DATE           -- Temporal validity
```

### Gaps

| Column | Expected Use | Actual Use |
|--------|--------------|------------|
| `target_entity_id` | "Entity X is DEPOSITARY **OF** Fund Y" | **NEVER USED** (0 rows) |
| `ownership_percentage` | Ownership role percentages | Rarely populated |

### Implication
Roles are CBU-scoped, not entity-scoped. Can't express "State Street is depositary OF Allianz SICAV specifically" - only "State Street is depositary WITHIN Allianz Group CBU".

---

## 4. Role Taxonomy

### Summary
- **98 roles** defined with rich metadata
- **11 role_categories**: OWNERSHIP_CHAIN, CONTROL_CHAIN, FUND_MANAGEMENT, SERVICE_PROVIDER, TRUST_ROLES, etc.
- **UBO treatment**: LOOK_THROUGH, TERMINUS, BY_PERCENTAGE, CONTROL_PRONG
- **KYC obligations**: FULL_KYC, SIMPLIFIED, SCREEN_AND_ID, RECORD_ONLY

### Key UBO Roles
| Role | ubo_treatment | requires_percentage |
|------|---------------|---------------------|
| BENEFICIAL_OWNER | BY_PERCENTAGE | ✓ |
| ULTIMATE_BENEFICIAL_OWNER | TERMINUS | ✓ |
| SHAREHOLDER | LOOK_THROUGH | ✓ |
| HOLDING_COMPANY | LOOK_THROUGH | ✓ |
| GENERAL_PARTNER | LOOK_THROUGH | ✓ |
| TRUSTEE | CONTROL_PRONG | ✗ |
| DIRECTOR | CONTROL_PRONG | ✗ |

### Role Requirements (Validation Rules)
```
SUB_FUND requires UMBRELLA_FUND in SAME_CBU
FEEDER_FUND requires MASTER_FUND in SAME_CBU
SUB_ADVISOR requires INVESTMENT_MANAGER in SAME_CBU
```

---

## 5. entity_relationships Analysis

### Schema
```sql
relationship_id       UUID PRIMARY KEY
from_entity_id        UUID NOT NULL
to_entity_id          UUID NOT NULL
relationship_type     VARCHAR NOT NULL  -- 'ownership', 'control', 'trust_role'
percentage            NUMERIC           -- For ownership
ownership_type        VARCHAR           -- DIRECT, INDIRECT, BENEFICIAL
control_type          VARCHAR           -- For control relationships
trust_role            VARCHAR           -- For trust relationships
effective_from/to     DATE              -- Temporal validity
source                VARCHAR           -- GLEIF, DSL, etc.
```

### Current Content
```
relationship_type | count 
------------------|-------
control           |     2
ownership         |     8
trust_role        |     5
```

### Gap: Verb Mismatch
The GLEIF DSL file uses:
```lisp
(cbu.role:assign-ownership ...)  -- DOES NOT EXIST
```

Should use:
```lisp
(ubo.add-ownership ...)  -- EXISTS in ubo.yaml
```

---

## 6. Graph Repository Query Analysis

### Tables Queried
| Function | Table | Purpose |
|----------|-------|---------|
| `load_cbu_roles()` | cbu_entity_roles | Role assignments |
| `load_ownership_edges()` | entity_relationships | Ownership chains |
| `load_control_edges()` | entity_relationships | Control chains |
| `load_fund_edges()` | **fund_structure** | Fund hierarchies |

### Critical: `load_fund_edges()` 
```rust
async fn load_fund_edges(&self, entity_ids: &HashSet<Uuid>) -> Result<Vec<FundStructureRow>> {
    // Queries fund_structure table ONLY
    // Does NOT query entity_funds.parent_fund_id
    // fund_structure is EMPTY → returns nothing
}
```

---

## 7. Trading View Requirements

For a complete Trading CBU view:

| Requirement | Table(s) | Status |
|-------------|----------|--------|
| CBU details | cbus | ✅ |
| ManCo assignment | cbu_entity_roles | ✅ |
| Depositary assignment | cbu_entity_roles | ✅ |
| Auditor assignment | cbu_entity_roles | ✅ |
| Umbrella → Sub-fund | fund_structure OR entity_funds | ❌ BROKEN |
| Sub-fund → Share Class | fund_structure OR entity_funds | ❌ BROKEN |
| Master → Feeder | fund_structure | ❌ EMPTY |

---

## 8. UBO/KYC View Requirements

For a complete UBO view:

| Requirement | Table(s) | Status |
|-------------|----------|--------|
| Ownership chain | entity_relationships | ⚠️ Minimal (8 rows) |
| Control chain | entity_relationships | ⚠️ Minimal (2 rows) |
| Trust relationships | entity_relationships | ⚠️ Minimal (5 rows) |
| UBO terminus marking | ??? | ❌ No verb exists |
| Director/Officer | cbu_entity_roles | ✅ Role exists |
| Person roles | cbu_entity_roles | ✅ Role exists |

---

## 9. Action Items

### P0 - Critical (Blocks Demo)

1. **Fix fund hierarchy gap**
   - Update `graph_repository.rs` to query `entity_funds.parent_fund_id` in addition to `fund_structure`
   - OR add trigger to sync entity_funds → fund_structure

2. **Fix GLEIF DSL verb alignment**
   - Update `allianzgi_ownership_chain.dsl` to use `ubo.add-ownership` instead of `cbu.role:assign-ownership`

### P1 - High (Needed for Complete Demo)

3. **Create UBO terminus verb**
   - Add `ubo.mark-terminus` or use entity attribute to flag chain termination
   - Reason codes: NO_KNOWN_PERSON, PUBLIC_COMPANY, DISPERSED_OWNERSHIP

4. **Populate target_entity_id**
   - Enable "Entity X is DEPOSITARY OF Fund Y" directed roles
   - Update `cbu.assign-role` verb to accept optional `:target-entity-id`

### P2 - Medium

5. **Add director/officer data**
   - Currently no person entities with DIRECTOR role on Allianz
   - Need to source from GLEIF or prospectus

6. **Enrich ManCo entities with LEI**
   - Current DSL creates ManCos without LEI
   - Could be enriched from GLEIF

---

## 10. Verification Queries

### Check fund hierarchy is working
```sql
-- After fix, this should return umbrella→subfund links
SELECT 
  p.name as parent,
  c.name as child,
  fs.relationship_type
FROM "ob-poc".fund_structure fs
JOIN "ob-poc".entities p ON fs.parent_entity_id = p.entity_id
JOIN "ob-poc".entities c ON fs.child_entity_id = c.entity_id
WHERE p.name ILIKE '%allianz%'
LIMIT 20;

-- Fallback: check entity_funds.parent_fund_id
SELECT 
  c.name as child,
  p.name as parent
FROM "ob-poc".entity_funds ef
JOIN "ob-poc".entities c ON ef.entity_id = c.entity_id
JOIN "ob-poc".entities p ON ef.parent_fund_id = p.entity_id
WHERE c.name ILIKE '%allianz%';
```

### Check ownership chains
```sql
SELECT 
  fe.name as owner,
  te.name as owned,
  er.percentage,
  er.source
FROM "ob-poc".entity_relationships er
JOIN "ob-poc".entities fe ON er.from_entity_id = fe.entity_id
JOIN "ob-poc".entities te ON er.to_entity_id = te.entity_id
WHERE er.relationship_type = 'ownership'
  AND (fe.name ILIKE '%allianz%' OR te.name ILIKE '%allianz%');
```

### Check role coverage
```sql
SELECT 
  r.role_category,
  r.name as role,
  COUNT(cer.cbu_entity_role_id) as assignments
FROM "ob-poc".roles r
LEFT JOIN "ob-poc".cbu_entity_roles cer ON r.role_id = cer.role_id
GROUP BY r.role_category, r.name
ORDER BY r.role_category, assignments DESC;
```
