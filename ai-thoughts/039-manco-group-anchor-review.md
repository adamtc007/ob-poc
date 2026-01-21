# ManCo Group Anchor Model - Peer Review Summary

**Reviewer:** Claude Code
**Date:** 2026-01-20
**Status:** Ready for ChatGPT peer review

---

## Files Under Review

| File | Purpose | Lines |
|------|---------|-------|
| `ai-thoughts/039-manco-group-anchor-design.md` | Design summary and rationale | ~200 |
| `migrations/030_manco_group_anchor.sql` | Schema + functions | ~400 |
| `rust/crates/ob-poc-types/src/manco_group.rs` | Rust types | ~350 |
| `rust/config/verbs/manco-group.yaml` | DSL verb definitions | ~250 |

---

## Summary of Proposal

The ManCo Group Anchor model introduces:

1. **`cbu_groups`** - Collections of CBUs anchored to a ManCo entity (e.g., "AllianzGI Lux Book")
2. **`cbu_group_members`** - Links CBUs to groups with source tracking
3. **`kyc.holding_control_links`** - Materialized shareholding control relationships

### Key Use Cases

- Find all CBUs managed by a ManCo
- Find which ManCo manages a specific CBU
- Trace shareholding control chain from ManCo → Ultimate Parent (e.g., AllianzGI → Allianz SE)

---

## Schema Analysis

### 1. `cbu_groups` Table

**Location:** `"ob-poc".cbu_groups`

```sql
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_groups (
    group_id UUID PRIMARY KEY,
    manco_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    group_name VARCHAR(255) NOT NULL,
    group_code VARCHAR(50),
    group_type VARCHAR(30) NOT NULL DEFAULT 'MANCO_BOOK',
    jurisdiction VARCHAR(10),
    ultimate_parent_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    ...
);
```

**Assessment:** ✅ Good design
- Correctly references `entities` table for ManCo anchor
- Supports multiple group types (MANCO_BOOK, CORPORATE_GROUP, etc.)
- Has temporal columns (effective_from/to) for history
- Auto-derived flag distinguishes computed vs manual groups

**Potential Issue:** The unique constraint `UNIQUE NULLS NOT DISTINCT (manco_entity_id, jurisdiction, effective_to)` may not handle the case where a ManCo has multiple active groups in the same jurisdiction with different names. Consider if this is intentional.

### 2. `cbu_group_members` Table

**Location:** `"ob-poc".cbu_group_members`

```sql
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_group_members (
    membership_id UUID PRIMARY KEY,
    group_id UUID NOT NULL REFERENCES "ob-poc".cbu_groups(group_id),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    source VARCHAR(30) NOT NULL DEFAULT 'MANCO_ROLE',
    ...
);
```

**Assessment:** ✅ Good design
- Properly tracks source of membership (MANCO_ROLE, GLEIF_MANAGED, SHAREHOLDING, MANUAL)
- Has temporal columns for history
- Cascades on group delete

**Question:** Should there be a constraint preventing a CBU from being in multiple active groups simultaneously? Current design allows it.

### 3. `kyc.holding_control_links` Table

**Location:** `kyc.holding_control_links`

```sql
CREATE TABLE IF NOT EXISTS kyc.holding_control_links (
    link_id UUID PRIMARY KEY,
    holder_entity_id UUID NOT NULL,
    issuer_entity_id UUID NOT NULL,
    share_class_id UUID REFERENCES kyc.share_classes(id),
    voting_pct NUMERIC(8,4),
    economic_pct NUMERIC(8,4),
    control_type VARCHAR(30) NOT NULL,
    ...
);
```

**Assessment:** ⚠️ Needs clarification on overlap with existing tables

---

## Critical Review: Overlap with Existing Schema

### Does `holding_control_links` duplicate `ownership_snapshots`?

**`kyc.ownership_snapshots`** (from migration 013):
- Stores computed ownership positions from register OR imported from BODS/GLEIF
- Has `derived_from` column: REGISTER, BODS, GLEIF, PSC, MANUAL, INFERRED
- Stores `percentage`, `numerator`, `denominator`
- Has `is_direct` and `is_aggregated` flags
- Purpose: Bridge for reconciliation between sources

**`kyc.holding_control_links`** (proposed):
- Stores materialized control relationships from holdings
- Has `control_type`: CONTROLLING, SIGNIFICANT_INFLUENCE, MATERIAL, NOTIFIABLE, MINORITY
- Stores `voting_pct`, `economic_pct`
- Has `chain_depth` for indirect control
- Purpose: Enable efficient graph traversal for control chains

### Verdict: NOT a duplicate, but related

| Aspect | ownership_snapshots | holding_control_links |
|--------|--------------------|-----------------------|
| Primary purpose | Reconciliation (compare sources) | Control chain traversal |
| Data source | REGISTER, BODS, GLEIF, etc. | Holdings only |
| Control classification | None (just percentages) | CONTROLLING, SIGNIFICANT, etc. |
| Chain depth | `is_direct` boolean | `chain_depth` integer |
| Aggregation | Per share class or aggregated | Per share class or aggregated |

**Recommendation:** These tables serve different purposes. `ownership_snapshots` is for reconciling multiple data sources. `holding_control_links` is for efficient control graph queries. However, consider:

1. **Should `holding_control_links` be derived FROM `ownership_snapshots` instead of directly from holdings?** This would ensure consistency between reconciliation and control analysis.

2. **Add a `source_snapshot_ids` column** to `holding_control_links` to trace back to the ownership_snapshots used for computation.

---

## SQL Function Analysis

### `fn_compute_control_links`

**Issue:** The function queries `kyc.holdings` and `kyc.share_class_supply` directly, but:
- `share_class_supply` may not be populated for all share classes
- Falls back to `sc.issued_shares` which is good
- Uses `issuer_control_config` for thresholds (✅ correct)

**Potential bug in denominator calculation:**
```sql
SUM(h.units * COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1)) / 
    NULLIF((SELECT SUM(scs.issued_units * COALESCE(sc2.votes_per_unit, 1)) 
            FROM kyc.share_class_supply scs
            JOIN kyc.share_classes sc2 ON sc2.id = scs.share_class_id
            WHERE sc2.issuer_entity_id = sc.issuer_entity_id), 0) * 100 AS voting_pct
```

This subquery:
1. Only counts share classes that have entries in `share_class_supply`
2. May miss share classes that only have `issued_shares` set directly

**Recommendation:** The denominator calculation should match `fn_holder_control_position` from migration 013, which handles both cases.

### `fn_manco_group_control_chain`

**Assessment:** ✅ Well-designed recursive CTE

The function correctly:
- Starts from ManCo and traverses upward via `holding_control_links`
- Respects `control_type IN ('CONTROLLING', 'SIGNIFICANT_INFLUENCE')`
- Has configurable `max_depth` to prevent infinite loops
- Identifies `is_ultimate_controller` when no one controls an entity

---

## Rust Types Review

### `manco_group.rs`

**Assessment:** ✅ Well-structured

Good practices observed:
- Proper use of `#[serde(rename_all = ...)]` for DB mapping
- `ControlType` has correct ordering (`Ord` derive)
- `default_threshold()` method provides configurable defaults
- Query builder pattern (`MancoGroupQuery`) for flexible filtering

**Minor suggestion:** Consider adding `FromStr` implementations for `GroupType` and `ControlType` to support DSL parsing.

---

## DSL Verbs Review

### `manco-group.yaml`

**Assessment:** ✅ Comprehensive coverage

Good:
- All SQL functions have corresponding verbs
- Examples provided for each verb
- Composite verb `manco.book.summary` aggregates related data
- Aliases for convenience (`manco.cbus` → `manco.group.cbus`)

**Issue:** The YAML structure doesn't match the standard verb YAML format in the codebase. Compare with existing verbs in `config/verbs/`:

```yaml
# Expected format (from other verb files):
domains:
  manco:
    verbs:
      group.derive:
        description: "..."
        behavior: plugin
        ...

# Proposed format:
verbs:
  - domain: manco
    verb: group.derive
    ...
```

**Recommendation:** Align with existing verb YAML format or document the new format.

---

## Data Integrity Considerations

### 1. Deriving groups from roles

`fn_derive_cbu_groups()` creates groups from `MANAGEMENT_COMPANY` role assignments. This assumes:
- Every CBU has at most one MANAGEMENT_COMPANY role
- The role is current (not terminated)

**Question:** What happens if a CBU has multiple ManCos assigned (transitional period)?

### 2. Control links computation

`fn_compute_control_links()` DELETEs existing links before recomputing:
```sql
DELETE FROM kyc.holding_control_links
WHERE as_of_date = p_as_of_date
    AND (p_issuer_entity_id IS NULL OR issuer_entity_id = p_issuer_entity_id);
```

This is safe for recomputation but means:
- Historical control links are lost unless computed with different `as_of_date`
- No audit trail of changes to control links over time

**Recommendation:** Consider adding a `superseded_at` pattern like `ownership_snapshots` instead of DELETE.

---

## File Location Review

| File | Proposed Location | Assessment |
|------|-------------------|------------|
| Design doc | `ai-thoughts/039-...` | ✅ Correct for design docs |
| Migration | `migrations/030_...` | ⚠️ Check sequence - there's already a `030_fund_vehicles.sql` |
| Rust types | `ob-poc-types/src/manco_group.rs` | ✅ Correct crate |
| Verb YAML | `config/verbs/manco-group.yaml` | ✅ Correct location |

**Critical issue:** Migration file naming collision:
```
migrations/030_fund_vehicles.sql       # Already exists
migrations/030_manco_group_anchor.sql  # Proposed
```

**Recommendation:** Rename to `031_manco_group_anchor.sql` or higher.

---

## Integration with Solar Navigation

The design doc mentions integration with the Solar Navigation metaphor:
- Galaxy Level: ManCo groups as orbital clusters
- System Level: CBUs as planets orbiting ManCo sun

**Assessment:** The data model supports this, but implementation details are not specified.

**Recommendation:** Add a section to the design doc showing:
1. How `nav_service.rs` would query `fn_get_manco_group_cbus()`
2. How the control chain feeds into the UBO drill-down view
3. Example of zooming from Galaxy → System → Planet using this data

---

## Summary: Recommendations

### Must Fix Before Implementation

1. **Migration filename collision** - Rename from `030` to unused number
2. **Denominator calculation in `fn_compute_control_links`** - Align with existing logic in `fn_holder_control_position`
3. **Verb YAML format** - Align with existing verb file structure or update parser

### Should Consider

4. **Link `holding_control_links` to `ownership_snapshots`** - Add `source_snapshot_ids` for traceability
5. **Historical audit for control links** - Use superseded pattern instead of DELETE
6. **Constraint on CBU multi-group membership** - Decide if CBU can be in multiple groups

### Nice to Have

7. **Add `FromStr` to Rust enums** - For DSL parsing flexibility
8. **Document Solar Navigation integration** - Show concrete query patterns

---

## Files for ChatGPT Review

The following files should be reviewed together:

1. `ai-thoughts/039-manco-group-anchor-design.md` - Design rationale
2. `migrations/030_manco_group_anchor.sql` - Schema + SQL functions
3. `rust/crates/ob-poc-types/src/manco_group.rs` - Rust type definitions
4. `rust/config/verbs/manco-group.yaml` - DSL verb definitions
5. `migrations/013_capital_structure_ownership.sql` - Existing schema (for comparison)
6. `migrations/009_kyc_control_schema.sql` - Existing holdings/share_classes schema

**Key questions for peer review:**

1. Is the separation between `ownership_snapshots` and `holding_control_links` justified?
2. Is the verb YAML format compatible with the existing parser?
3. Are the SQL functions correct for the denominator calculations?
4. Should the control chain traversal include GLEIF relationships as well as shareholdings?
