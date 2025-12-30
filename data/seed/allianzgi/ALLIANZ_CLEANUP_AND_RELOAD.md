# Allianz Data Cleanup and Reload Plan

## Executive Summary

The Allianz fund data was incorrectly loaded via Python scripts which flattened the fund hierarchy. This document provides the cleanup and reload procedure to establish the correct structure for demo purposes.

**Current State (Incorrect):**
- 599 CBUs (each sub-fund as separate CBU)
- 240 LIMITED_COMPANY_PRIVATE entities (wrong entity type)
- 1,803 cbu_entity_roles linking to wrong CBUs
- 0 Umbrella Fund entities
- 0 Share Classes in kyc.share_classes
- 0 fund_structure relationships

**Target State (Correct):**
```
CBU: Allianz Global Investors (Group)
├── Entity: Allianz Global Investors GmbH (Management Company) [+ 10 regional ManCos]
├── Entity: State Street Bank... (Depositary)
├── Entity: PwC (Auditor)
├── Entity: Allianz Global Investors Fund (Umbrella Fund / SICAV)
│   ├── Entity: Allianz Global Artificial Intelligence (Sub-fund)
│   │   ├── Entity: A - EUR (Share Class) [ISIN: LU1603246700]
│   │   ├── Entity: IT - USD (Share Class) [ISIN: LU1548498150]
│   │   └── ... (1,973 total share classes)
│   ├── Entity: Allianz Emerging Markets Equity (Sub-fund)
│   └── ... (205 total sub-funds)
└── Entity: Allianz Global Investors Fund II (Umbrella Fund / SICAV)
```

---

## PHASE 1: Cleanup

### 1.1 Backup First (CRITICAL)

```bash
# Create timestamped backup
pg_dump -U adamtc007 -d data_designer -n "ob-poc" -n "kyc" \
  -F c -f ~/Developer/ob-poc/backups/pre_allianz_cleanup_$(date +%Y%m%d_%H%M%S).dump
```

### 1.2 Cleanup SQL Script

Save as: `cleanup_allianz_incorrect_data.sql`

```sql
-- ============================================================================
-- ALLIANZ INCORRECT DATA CLEANUP
-- Generated: 2024-12-30
-- 
-- This script removes incorrectly loaded Allianz data that was flattened
-- by Python scripts instead of loaded via proper DSL hierarchy.
--
-- PRESERVES:
--   - GLEIF-sourced entities (have LEI codes)
--   - UBO Test entities
--   - Any non-Allianz data
-- ============================================================================

BEGIN;

-- Create temp table of CBUs to delete (the 599 incorrectly created sub-fund CBUs)
CREATE TEMP TABLE cbus_to_delete AS
SELECT cbu_id, name 
FROM "ob-poc".cbus 
WHERE name ILIKE '%allianz%'
  -- Exclude any that might be the correct group CBU if it exists
  AND name NOT ILIKE '%allianz global investors (group)%'
  AND name NOT ILIKE '%allianz global investors group%';

-- Report what will be deleted
SELECT 'CBUs to delete: ' || COUNT(*)::text FROM cbus_to_delete;

-- Create temp table of entities to delete
-- Only LIMITED_COMPANY_PRIVATE that don't have LEI (not GLEIF sourced)
CREATE TEMP TABLE entities_to_delete AS
SELECT e.entity_id, e.name
FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entity_funds ef ON e.entity_id = ef.entity_id
WHERE e.name ILIKE '%allianz%'
  AND et.name = 'LIMITED_COMPANY_PRIVATE'
  AND ef.lei IS NULL  -- Preserve GLEIF-sourced entities
  -- Exclude test entities
  AND e.name NOT ILIKE 'UBO Test:%';

SELECT 'Entities to delete: ' || COUNT(*)::text FROM entities_to_delete;

-- ============================================================================
-- DELETE IN CORRECT ORDER (respecting FK constraints)
-- ============================================================================

-- 1. Delete cbu_entity_roles for affected CBUs
DELETE FROM "ob-poc".cbu_entity_roles 
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

-- 2. Delete any KYC cases (should be 0 but be safe)
DELETE FROM "kyc".cases 
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

-- 3. Delete CBU evidence
DELETE FROM "ob-poc".cbu_evidence
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

-- 4. Delete CBU change log
DELETE FROM "ob-poc".cbu_change_log
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

-- 5. Delete DSL sessions
DELETE FROM "ob-poc".dsl_sessions
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

-- 6. Delete UBO registry entries
DELETE FROM "ob-poc".ubo_registry
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

-- 7. Delete service delivery map
DELETE FROM "ob-poc".service_delivery_map
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

-- 8. Delete CBU resource instances
DELETE FROM "ob-poc".cbu_resource_instances
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

-- 9. Delete client allegations
DELETE FROM "ob-poc".client_allegations
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

-- 10. Delete onboarding requests
DELETE FROM "ob-poc".onboarding_requests
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

-- 11. Delete share classes referencing these CBUs
DELETE FROM "kyc".share_classes
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

-- 12. Now delete the CBUs themselves
DELETE FROM "ob-poc".cbus 
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

-- ============================================================================
-- DELETE INCORRECTLY TYPED ENTITIES
-- ============================================================================

-- Delete entity relationships first
DELETE FROM "ob-poc".entity_relationships
WHERE from_entity_id IN (SELECT entity_id FROM entities_to_delete)
   OR to_entity_id IN (SELECT entity_id FROM entities_to_delete);

-- Delete from extension tables
DELETE FROM "ob-poc".entity_funds
WHERE entity_id IN (SELECT entity_id FROM entities_to_delete);

DELETE FROM "ob-poc".entity_share_classes
WHERE entity_id IN (SELECT entity_id FROM entities_to_delete);

-- Delete from fund_structure
DELETE FROM "ob-poc".fund_structure
WHERE parent_entity_id IN (SELECT entity_id FROM entities_to_delete)
   OR child_entity_id IN (SELECT entity_id FROM entities_to_delete);

-- Delete the entities
DELETE FROM "ob-poc".entities
WHERE entity_id IN (SELECT entity_id FROM entities_to_delete);

-- ============================================================================
-- VERIFICATION
-- ============================================================================

SELECT 'Remaining Allianz CBUs: ' || COUNT(*)::text 
FROM "ob-poc".cbus WHERE name ILIKE '%allianz%';

SELECT 'Remaining Allianz entities: ' || COUNT(*)::text 
FROM "ob-poc".entities WHERE name ILIKE '%allianz%';

SELECT 'GLEIF entities preserved: ' || COUNT(*)::text 
FROM "ob-poc".entities e
JOIN "ob-poc".entity_funds ef ON e.entity_id = ef.entity_id
WHERE e.name ILIKE '%allianz%' AND ef.lei IS NOT NULL;

-- Cleanup temp tables
DROP TABLE cbus_to_delete;
DROP TABLE entities_to_delete;

COMMIT;

-- If everything looks correct, remove this line to auto-commit
-- ROLLBACK;
```

### 1.3 Run Cleanup

```bash
# Review first (will rollback)
/Applications/pgAdmin\ 4.app/Contents/SharedSupport/psql \
  postgresql://adamtc007@localhost:5432/data_designer \
  -f cleanup_allianz_incorrect_data.sql

# If satisfied, edit script to remove ROLLBACK and re-run
```

---

## PHASE 2: Reload Correct Structure

### 2.1 Load Sequence

Execute in this exact order:

```bash
cd /Users/adamtc007/Developer/ob-poc

# Step 1: Bootstrap - Creates Group CBU, ManCos, Service Providers, Umbrellas
./target/release/ob-poc-dsl < data/seed/allianzgi/03_load_allianzgi.dsl

# Step 2: Load full Luxembourg sub-funds and share classes (205 sub-funds, 1973 share classes)
./target/release/ob-poc-dsl < data/seed/allianzgi/out/LU_funds_ensure.dsl

# Step 3: Load GB funds (if downloaded)
./target/release/ob-poc-dsl < data/seed/allianzgi/out/GB_funds_ensure.dsl
```

### 2.2 What Each Script Creates

**03_load_allianzgi.dsl:**
- 1 CBU: "Allianz Global Investors (Group)"
- 11 ManCo entities (DE, LU, GB, IE, CH, HK, SG, JP, TW, CN, ID)
- 3 Service provider entities (Depositary LU, Depositary IE, Auditor)
- 2 Umbrella entities (SICAV main + SICAV II)
- 6 sample sub-funds
- 15 sample share classes

**LU_funds_ensure.dsl:**
- 205 sub-funds under "Allianz Global Investors Fund" umbrella
- 1,973 share classes with ISINs

**GB_funds_ensure.dsl:**
- UK OEIC structure (separate umbrella)
- GB-domiciled funds

---

## PHASE 3: Verification Queries

After reload, run these to verify correct structure:

```sql
-- 1. Check CBU exists
SELECT cbu_id, name, cbu_category, jurisdiction 
FROM "ob-poc".cbus 
WHERE name ILIKE '%allianz global investors%group%';

-- 2. Check umbrella entities exist
SELECT e.entity_id, e.name, et.name as type
FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
WHERE et.name = 'Umbrella Fund'
  AND e.name ILIKE '%allianz%';

-- 3. Count sub-funds
SELECT COUNT(*) as subfund_count
FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
WHERE et.name = 'Sub-fund/Compartment'
  AND e.name ILIKE '%allianz%';

-- 4. Count share classes
SELECT COUNT(*) as share_class_count
FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
WHERE et.name = 'Share Class'
  AND e.name ILIKE '%allianz%';

-- 5. Check fund_structure relationships
SELECT COUNT(*) as fund_links
FROM "ob-poc".fund_structure fs
JOIN "ob-poc".entities e ON fs.parent_entity_id = e.entity_id
WHERE e.name ILIKE '%allianz%';

-- 6. Check ManCo assignments
SELECT e.name as manco, r.role_code
FROM "ob-poc".cbu_entity_roles r
JOIN "ob-poc".entities e ON r.entity_id = e.entity_id
JOIN "ob-poc".cbus c ON r.cbu_id = c.cbu_id
WHERE c.name ILIKE '%allianz%group%'
  AND r.role_code = 'MANAGEMENT_COMPANY';

-- 7. Full hierarchy view
SELECT 
  c.name as cbu,
  umb.name as umbrella,
  sf.name as subfund,
  sc.name as share_class,
  esc.isin
FROM "ob-poc".cbus c
JOIN "ob-poc".cbu_entity_roles cer ON c.cbu_id = cer.cbu_id
JOIN "ob-poc".entities umb ON cer.entity_id = umb.entity_id
JOIN "ob-poc".entity_types ut ON umb.entity_type_id = ut.entity_type_id
JOIN "ob-poc".fund_structure fs_sf ON umb.entity_id = fs_sf.parent_entity_id
JOIN "ob-poc".entities sf ON fs_sf.child_entity_id = sf.entity_id
JOIN "ob-poc".fund_structure fs_sc ON sf.entity_id = fs_sc.parent_entity_id
JOIN "ob-poc".entities sc ON fs_sc.child_entity_id = sc.entity_id
JOIN "ob-poc".entity_share_classes esc ON sc.entity_id = esc.entity_id
WHERE c.name ILIKE '%allianz%group%'
  AND ut.name = 'Umbrella Fund'
LIMIT 20;
```

---

## PHASE 4: UBO Structure (Trading vs Ownership)

After fund structure is correct, the UBO/ownership chain needs to be loaded separately.

### 4.1 Trading CBU View
The fund hierarchy above represents the **Trading CBU** view:
- What products/funds does this client have?
- What share classes are we servicing?
- What's the NAV, the fee structure?

### 4.2 Ownership CBU View  
Separate load required for **Ownership/UBO** view:
- Who owns Allianz Global Investors GmbH?
- What's the chain to Allianz SE (ultimate parent)?
- Who are the directors/officers?

**Pre-existing GLEIF-derived ownership chain:**
```
/Users/adamtc007/Developer/ob-poc/data/derived/gleif/allianzgi_ownership_chain.dsl
```

**Ownership Structure (from GLEIF):**
```
Allianz SE (DE) [UBO Terminus - publicly traded]
  └── 100% → Allianz Global Investors GmbH (DE) [LEI: OJ2TIQSVQND4IZYYK658]
        ├── 100% → ALLIANZ CAPITAL PARTNERS OF AMERICA LLC (US-DE)
        └── 100% → アリアンツ・グローバル・インベスターズ・ジャパン (JP)
```

**Load sequence:**

```bash
# Regenerate from GLEIF API (optional - already exists)
python3 scripts/gleif_extract_allianz.py

# Load ownership chain
./target/release/ob-poc-dsl < data/derived/gleif/allianzgi_ownership_chain.dsl
```

**Required verbs for ownership (check if exist):**
- `entity.ensure-limited-company` ✓ (exists)
- `ubo.add-ownership` ✓ (exists in ubo.yaml)
- `cbu.role:mark-ubo-terminus` ❌ **DOES NOT EXIST** - needs to be created

**NOTE:** The GLEIF DSL file uses `cbu.role:assign-ownership` and `cbu.role:mark-ubo-terminus` 
which may not match the actual verb definitions. The DSL file may need to be updated to use:
- `ubo.add-ownership` instead of `cbu.role:assign-ownership`
- A new verb needs to be created for marking UBO terminus, OR
- Use `entity.set-attribute` to flag the terminus status on the entity

### 4.3 Linking Trading and Ownership Views

The key connection point is:
- **Trading view:** `Allianz Global Investors GmbH` as MANAGEMENT_COMPANY role on CBU
- **Ownership view:** Same entity as child of `Allianz SE`

The entity should be the same (same LEI), allowing navigation:
- From CBU → ManCo → Parent chain → UBO terminus
- From Allianz SE → Subsidiaries → ManCo → CBUs they manage

---

## Expected Final Counts

| Entity Type | Count |
|-------------|-------|
| CBUs | 1 (Allianz Global Investors Group) |
| Management Company | 11 |
| Depositary | 2 |
| Auditor | 1 |
| Umbrella Fund | 2 (main SICAV + SICAV II) |
| Sub-fund/Compartment | 205+ |
| Share Class | 1,973+ |
| fund_structure links | ~2,200 (umbrella→subfund + subfund→shareclass) |

---

## Troubleshooting

### If DSL fails with "already exists"
The `ensure` verbs should handle upserts. If `create` verbs fail:
```bash
# Use ensure variants
sed 's/fund\.create-/fund.ensure-/g' LU_funds.dsl > LU_funds_ensure.dsl
```

### If umbrella not found
Ensure 03_load_allianzgi.dsl completed successfully before loading sub-funds.

### If share class ISINs rejected
Check ISIN format (12 chars, starts with country code). Some older funds may have invalid ISINs in source data.

---

## APPENDIX: Gaps and Action Items

### Verb Gaps

| Gap | Priority | Notes |
|-----|----------|-------|
| `ubo.mark-terminus` | HIGH | Need verb to flag UBO chain terminus (public company, no known person) |
| GLEIF DSL verb mismatch | HIGH | `allianzgi_ownership_chain.dsl` uses verbs that may not exist - needs alignment |

### Data Gaps

| Gap | Priority | Notes |
|-----|----------|-------|
| Directors/Officers | MEDIUM | No director data in current load - need entity type + role assignments |
| LEI on ManCo entities | LOW | DSL creates ManCos without LEI - could be enriched from GLEIF |
| Fund legal addresses | LOW | Share class ISINs present but no fund registered addresses |

### Schema Gaps

| Gap | Priority | Notes |
|-----|----------|-------|
| `fund_structure` vs `entity_funds.parent_fund_id` | HIGH | Two ways to track parent-child: `create-subfund` uses `entity_funds.parent_fund_id`, but `fund_structure` table is separate. Verify which graph queries use. |
| Share class → entity link | MEDIUM | kyc.share_classes vs entity with Share Class type - which is canonical? |

**Investigation needed:** The `create-subfund` verb sets `umbrella-id` → `entity_funds.parent_fund_id`. 
But `fund_structure` table is populated by `link-feeder` verb (for master-feeder only).
Graph navigation likely queries `entity_funds.parent_fund_id` OR needs both to be populated.

### Demo Requirements

For a complete Allianz demo showing Trading + Ownership views:

1. ✅ Fund hierarchy (Umbrella → Sub-fund → Share Class)
2. ✅ Service provider roles (ManCo, Depositary, Auditor)
3. ⚠️ Ownership chain (entities exist, relationships need verb fix)
4. ❌ UBO terminus marking
5. ❌ Director/Officer entities
6. ⚠️ Graph navigation (depends on fund_structure being populated)
