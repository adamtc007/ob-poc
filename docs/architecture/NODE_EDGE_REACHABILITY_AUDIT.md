# Node/Edge Reachability Audit

## CRITICAL FINDING: Dynamic Verbs Not Expanding

**The `from_config_with_db()` method exists but is NEVER called.**

This means entity types in the database are NOT being expanded to verbs. The agent can ONLY create entity types that have explicit verb definitions.

| Entity Types in DB | Explicit Create Verbs | Gap |
|--------------------|----------------------|-----|
| 22 | 11 | **11 unreachable** |

---

## 1. Entity Type → Verb Mapping

### ✅ REACHABLE (Have Explicit Verbs)

| Entity Type Code | Create Verb | Ensure Verb |
|------------------|-------------|-------------|
| `limited_company` (generic) | `entity.create-limited-company` | `entity.ensure-limited-company` |
| `PROPER_PERSON_NATURAL` | `entity.create-proper-person` | `entity.ensure-proper-person` |
| `TRUST_DISCRETIONARY` | `entity.create-trust-discretionary` | `entity.ensure-trust-discretionary` |
| `PARTNERSHIP_LIMITED` | `entity.create-partnership-limited` | `entity.ensure-partnership-limited` |
| `fund_umbrella` | `fund.create-umbrella` | `fund.ensure-umbrella` |
| `fund_subfund` | `fund.create-subfund` | `fund.ensure-subfund` |
| `fund_share_class` | `fund.create-share-class` | `fund.ensure-share-class` |
| `fund_standalone` | `fund.create-standalone` | ❌ |
| `fund_master` | `fund.create-master` | ❌ |
| `fund_feeder` | `fund.create-feeder` | ❌ |

### ❌ UNREACHABLE (No Verbs)

| Entity Type Code | Name | Gap Type |
|------------------|------|----------|
| `LIMITED_COMPANY_PRIVATE` | LIMITED_COMPANY_PRIVATE | Uses generic `limited_company`? |
| `LIMITED_COMPANY_PUBLIC` | LIMITED_COMPANY_PUBLIC | **BLOCKED** |
| `LIMITED_COMPANY_UNLIMITED` | LIMITED_COMPANY_UNLIMITED | **BLOCKED** |
| `PARTNERSHIP_GENERAL` | PARTNERSHIP_GENERAL | **BLOCKED** |
| `PARTNERSHIP_LLP` | PARTNERSHIP_LLP | **BLOCKED** |
| `PROPER_PERSON_BENEFICIAL_OWNER` | PROPER_PERSON_BENEFICIAL_OWNER | Uses `NATURAL`? |
| `TRUST_CHARITABLE` | TRUST_CHARITABLE | **BLOCKED** |
| `TRUST_FIXED_INTEREST` | TRUST_FIXED_INTEREST | **BLOCKED** |
| `TRUST_UNIT` | TRUST_UNIT | **BLOCKED** |
| `management_company` | Management Company | Created via role? |
| `depositary` | Depositary | Created via role? |
| `fund_administrator` | Fund Administrator | Created via role? |

---

## 2. Relationship/Edge Type → Verb Mapping

### ✅ REACHABLE

| Relationship Type | Add Verb | List Verb |
|-------------------|----------|-----------|
| `ownership` | `ubo.add-ownership` | `ubo.list-owners`, `ubo.list-owned` |
| `control` | `control.add` | `control.list-controllers` |
| `trust_role` | `cbu.role:assign-trust-role` | `cbu.role:list` |
| `fund_structure` (feeder→master) | `fund.link-feeder` | `fund.list-feeders` |
| `delegation` | `delegation.add` | `delegation.list-*` |

### ⚠️ PARTIAL (Edge exists but no direct verb)

| Edge Type | Current State |
|-----------|---------------|
| `fund_structure` (umbrella→subfund) | Written to `entity_funds.parent_fund_id`, no `fund_structure` table entry |
| `fund_structure` (subfund→shareclass) | Written to `entity_funds.parent_fund_id`, no `fund_structure` table entry |

---

## 3. Role Type → Verb Mapping

### ✅ ALL 98 ROLES REACHABLE

The `cbu.role:assign` verb accepts any role from the `roles` table:

```dsl
(cbu.role:assign :cbu-id @cbu :entity-id @entity :role "SHAREHOLDER")
(cbu.role:assign :cbu-id @cbu :entity-id @entity :role "DIRECTOR")
(cbu.role:assign :cbu-id @cbu :entity-id @entity :role "TRUSTEE")
;; ... any of 98 roles
```

Specialized assignment verbs also exist:
- `cbu.role:assign-ownership` - Ownership roles with percentage
- `cbu.role:assign-control` - Control roles (director, officer)
- `cbu.role:assign-trust-role` - Trust roles (settlor, trustee, beneficiary)
- `cbu.role:assign-fund-role` - Fund management roles
- `cbu.role:assign-service-provider` - Service provider roles
- `cbu.role:assign-signatory` - Trading/signatory roles

---

## 4. Graph Navigation → Verb Mapping

### ✅ ALL GRAPH OPERATIONS REACHABLE

| Operation | Verb |
|-----------|------|
| View graph | `graph.view` |
| Focus on node | `graph.focus` |
| Filter by type | `graph.filter` |
| Group nodes | `graph.group-by` |
| Find path | `graph.path` |
| Find connected | `graph.find-connected` |
| Walk up | `graph.ancestors` |
| Walk down | `graph.descendants` |
| Compare | `graph.compare` |

---

## 5. Fixes Required

### P0 - CRITICAL: Enable Dynamic Verb Expansion

File: `/Users/adamtc007/Developer/ob-poc/rust/src/bin/dsl_api.rs`

Change startup to use `from_config_with_db`:

```rust
// Current (broken):
let registry = RuntimeVerbRegistry::from_config(&verb_config_path)?;

// Fixed:
let config = VerbsConfig::load(&verb_config_path)?;
let registry = RuntimeVerbRegistry::from_config_with_db(&config, &pool).await?;
```

This will auto-generate verbs for ALL entity types:
- `entity.create-limited-company-private`
- `entity.create-limited-company-public`
- `entity.create-partnership-general`
- etc.

### P1 - Add Missing Explicit Verbs (if dynamic not used)

If dynamic expansion is not enabled, add explicit verbs:

```yaml
# entity.yaml additions
create-limited-company-public:
  description: Create a public limited company
  behavior: crud
  crud:
    operation: entity_create
    type_code: LIMITED_COMPANY_PUBLIC
    # ...

create-partnership-general:
  description: Create a general partnership
  # ...

create-trust-charitable:
  description: Create a charitable trust
  # ...
```

### P2 - Fund Structure Edge Fix

Ensure `fund.create-subfund` also writes to `fund_structure` table, not just `entity_funds.parent_fund_id`.

---

## 6. Verification Queries

After fix, verify all entity types are reachable:

```sql
-- Count verbs per entity type
SELECT et.type_code, et.name,
       CASE WHEN v.full_name IS NOT NULL THEN '✅' ELSE '❌' END as has_verb
FROM "ob-poc".entity_types et
LEFT JOIN "ob-poc".dsl_verbs v 
  ON v.full_name LIKE 'entity.create-' || REPLACE(LOWER(et.type_code), '_', '-') || '%'
  OR v.full_name LIKE 'fund.create-%'
ORDER BY has_verb DESC, et.type_code;
```

```sql
-- All verbs that create entities
SELECT full_name, produces_type, produces_subtype
FROM "ob-poc".dsl_verbs
WHERE produces_type = 'entity'
ORDER BY full_name;
```

---

## 7. Agent Workflow Impact

| Scenario | Current State | After Fix |
|----------|---------------|-----------|
| "Create Allianz SE as public company" | ❌ No verb for PUBLIC | ✅ `entity.create-limited-company-public` |
| "Create a general partnership" | ❌ No verb | ✅ `entity.create-partnership-general` |
| "Create a charitable trust" | ❌ No verb | ✅ `entity.create-trust-charitable` |
| "Show umbrella→subfund hierarchy" | ⚠️ fund_structure empty | ✅ After edge fix |

---

## Summary

| Category | Reachable | Unreachable | Notes |
|----------|-----------|-------------|-------|
| Entity Types | 10 | **12** | Dynamic expansion disabled |
| Relationship Types | 5 | 0 | All covered |
| Role Types | 98 | 0 | Generic assign works |
| Graph Operations | 9 | 0 | All covered |
| Fund Hierarchy | ⚠️ | - | Wrong table used |

**Critical fix:** Call `from_config_with_db()` instead of `from_config()` at startup.
