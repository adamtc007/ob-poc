# CBU / Taxonomy / Verb Coverage Analysis

## Executive Summary

**CRITICAL GAP: `dsl_verbs` table is EMPTY**
- Agent RAG discovery **cannot work** - table has 0 rows
- VerbSyncService exists but is **not called on startup**
- Fix required in `dsl_api.rs` main() to call `VerbSyncService::sync_all()`

---

## 1. Entity Type → Verb Coverage

| Entity Type | Create Verb | Ensure Verb | Status |
|-------------|-------------|-------------|--------|
| LIMITED_COMPANY_PRIVATE | `entity.create-limited-company` | `entity.ensure-limited-company` | ✅ |
| LIMITED_COMPANY_PUBLIC | ❌ Missing | ❌ Missing | ⚠️ GAP |
| LIMITED_COMPANY_UNLIMITED | ❌ Missing | ❌ Missing | ⚠️ GAP |
| PARTNERSHIP_LIMITED | `entity.create-partnership-limited` | `entity.ensure-partnership-limited` | ✅ |
| PARTNERSHIP_GENERAL | ❌ Missing | ❌ Missing | ⚠️ GAP |
| PARTNERSHIP_LLP | ❌ Missing | ❌ Missing | ⚠️ GAP |
| PROPER_PERSON_NATURAL | `entity.create-proper-person` | `entity.ensure-proper-person` | ✅ |
| PROPER_PERSON_BENEFICIAL_OWNER | ❌ Uses NATURAL | ❌ Uses NATURAL | ⚠️ Alias? |
| TRUST_DISCRETIONARY | `entity.create-trust-discretionary` | `entity.ensure-trust-discretionary` | ✅ |
| TRUST_CHARITABLE | ❌ Missing | ❌ Missing | ⚠️ GAP |
| TRUST_FIXED_INTEREST | ❌ Missing | ❌ Missing | ⚠️ GAP |
| TRUST_UNIT | ❌ Missing | ❌ Missing | ⚠️ GAP |
| Umbrella Fund | `fund.create-umbrella` | `fund.ensure-umbrella` | ✅ |
| Sub-fund/Compartment | `fund.create-subfund` | `fund.ensure-subfund` | ✅ |
| Share Class | `fund.create-share-class` | `fund.ensure-share-class` | ✅ |
| Standalone Fund | `fund.create-standalone` | ❌ Missing | ⚠️ Partial |
| Master Fund | `fund.create-master` | ❌ Missing | ⚠️ Partial |
| Feeder Fund | `fund.create-feeder` | ❌ Missing | ⚠️ Partial |
| Management Company | Via role assignment | Via role assignment | ✅ (indirect) |
| Depositary | Via role assignment | Via role assignment | ✅ (indirect) |
| Fund Administrator | Via role assignment | Via role assignment | ✅ (indirect) |

### Priority Gaps for Allianz Demo

**HIGH** - Needed for complete load:
- None blocking - current verbs cover SICAV structures

**MEDIUM** - Would improve coverage:
- `entity.ensure-limited-company-public` - For public ManCo parents like Allianz SE
- `fund.ensure-standalone` / `fund.ensure-master` / `fund.ensure-feeder` - Idempotent versions

**LOW** - Edge cases:
- Other partnership types (GENERAL, LLP)
- Other trust types (CHARITABLE, FIXED_INTEREST, UNIT)

---

## 2. Relationship/Edge → Verb Coverage

| Relationship Type | Add Verb | Update Verb | End Verb | List Verb | Status |
|-------------------|----------|-------------|----------|-----------|--------|
| `ownership` | `ubo.add-ownership` | `ubo.update-ownership` | `ubo.end-ownership` | `ubo.list-owners` | ✅ |
| `control` | `control.add` | ❌ Missing | `control.end` | `control.list-controllers` | ⚠️ Partial |
| `trust_role` | `cbu.role:assign-trust-role` | ❌ Missing | ❌ Missing | ❌ Missing | ⚠️ Partial |
| `fund_structure` | `fund.link-feeder` | ❌ Missing | ❌ Missing | `fund.list-feeders` | ⚠️ Partial |
| `delegation` | `delegation.add` | ❌ Missing | `delegation.end` | `delegation.list-*` | ⚠️ Partial |

### UBO Terminus

| Verb | Exists | Notes |
|------|--------|-------|
| `ubo.mark-terminus` | ✅ EXISTS | Marks chain end (public company, no known person) |

---

## 3. Role Assignment → Verb Coverage

| Role Category | Assignment Verb | Status |
|---------------|-----------------|--------|
| OWNERSHIP_CHAIN | `cbu.role:assign-ownership` | ✅ |
| CONTROL_CHAIN | `cbu.role:assign-control` | ✅ |
| TRUST_ROLES | `cbu.role:assign-trust-role` | ✅ |
| FUND_MANAGEMENT | `cbu.role:assign-fund-role` | ✅ |
| FUND_STRUCTURE | `cbu.role:assign-fund-role` | ✅ |
| SERVICE_PROVIDER | `cbu.role:assign-service-provider` | ✅ |
| TRADING_EXECUTION | `cbu.role:assign-signatory` | ✅ |
| RELATED_PARTY | `cbu.role:assign` (generic) | ✅ |
| DISTRIBUTION | `cbu.role:assign` (generic) | ✅ |
| FINANCING | `cbu.role:assign` (generic) | ✅ |
| FUND_OPERATIONS | `cbu.role:assign` (generic) | ✅ |
| INVESTOR_CHAIN | `cbu.role:assign` (generic) | ✅ |

**Role assignment is well covered** - generic `cbu.role:assign` handles all 98 role types.

---

## 4. Agent RAG Discovery - CRITICAL GAP

### Current State

```sql
SELECT COUNT(*) FROM "ob-poc".dsl_verbs;
-- Returns: 0 rows
```

**The agent CANNOT discover verbs via RAG** - the table is empty.

### Infrastructure Exists But Not Connected

| Component | Status |
|-----------|--------|
| `dsl_verbs` table | ✅ EXISTS (24 columns) |
| `VerbSyncService` | ✅ EXISTS (verb_sync.rs) |
| `VerbDiscoveryService` | ✅ EXISTS (verb_discovery.rs) |
| Startup sync call | ❌ MISSING |
| API routes for discovery | ✅ EXISTS (verb_discovery_routes.rs) |

### Fix Required

In `/Users/adamtc007/Developer/ob-poc/rust/src/bin/dsl_api.rs`, add to main():

```rust
use ob_poc::session::VerbSyncService;
use ob_poc::dsl_v2::RuntimeVerbRegistry;

#[tokio::main]
async fn main() {
    // ... existing pool setup ...

    // === ADD THIS ===
    // Sync verbs to DB for RAG discovery
    let registry = RuntimeVerbRegistry::from_config("config/verbs")
        .expect("Failed to load verb registry");
    let sync_service = VerbSyncService::new(pool.clone());
    match sync_service.sync_all(&registry).await {
        Ok(result) => {
            println!("Verb sync: {} added, {} updated, {} unchanged",
                result.verbs_added, result.verbs_updated, result.verbs_unchanged);
        }
        Err(e) => {
            eprintln!("Verb sync failed: {}", e);
        }
    }
    // === END ADD ===

    // ... rest of main ...
}
```

### dsl_verbs Table Schema (Ready for RAG)

| Column | Purpose |
|--------|---------|
| `search_text` | Full-text search (tsvector) |
| `intent_patterns` | "add owner", "assign role", etc. |
| `workflow_phases` | "entity_collection", "screening" |
| `graph_contexts` | "cursor_on_cbu", "layer_ubo" |
| `typical_next` | Suggested next verbs |
| `produces_type` | Entity type created |
| `consumes` | Required inputs |

---

## 5. Summary: What's Blocking Demo

| Gap | Severity | Fix |
|-----|----------|-----|
| `dsl_verbs` empty | **CRITICAL** | Add sync call to startup |
| Fund hierarchy disconnect | **P0** | Already documented - graph queries use wrong table |
| GLEIF DSL verb mismatch | **P0** | Update DSL to use `ubo.add-ownership` |
| No `update-control` verb | LOW | Add if needed |
| Missing ensure variants | LOW | Add for master/feeder/standalone |
| Missing entity type variants | LOW | Add public/unlimited company if needed |

---

## 6. Verification After Fix

After adding verb sync to startup:

```sql
-- Should return ~150+ verbs
SELECT COUNT(*) FROM "ob-poc".dsl_verbs;

-- Test RAG search
SELECT full_name, description
FROM "ob-poc".dsl_verbs
WHERE to_tsvector('english', search_text) @@ plainto_tsquery('english', 'create fund')
LIMIT 5;

-- Check intent patterns populated
SELECT full_name, intent_patterns
FROM "ob-poc".dsl_verbs
WHERE intent_patterns IS NOT NULL
LIMIT 10;
```

---

## 7. Action Items

### P0 - CRITICAL

1. **Add verb sync to API startup** (dsl_api.rs main())
2. **Verify sync populates dsl_verbs** (should be ~150+ rows)

### P1 - HIGH

3. **Populate intent_patterns** for top 30 verbs (manual or generated)
4. **Add workflow_phases** for KYC lifecycle verbs
5. **Add graph_contexts** for navigation verbs

### P2 - MEDIUM

6. Add missing ensure variants (standalone, master, feeder)
7. Add update-control verb
8. Consider entity type aliases (PUBLIC → PRIVATE with flag)
