# P3-A: Core Entity Schema — Structural Integrity Audit

> **Auditor:** Claude (automated)
> **Date:** 2026-03-16
> **Scope:** 16 core entity tables in `"ob-poc"` schema + `cbu_structure_links`
> **Method:** Live DB introspection via `information_schema`, `pg_indexes`, `pg_constraint`, `pg_class`

---

## Executive Summary

The core entity schema is **structurally sound** for a rapidly evolving POC — primary keys, natural-key uniqueness, and trigram search indexes are consistently applied. However, the audit surfaces **3 critical issues** (duplicate FKs with conflicting cascade policies, a stale FK target, and universally nullable temporal columns) and **12 minor/flag-level issues** that should be addressed before production hardening.

| Severity | Count | Description |
|----------|-------|-------------|
| CRITICAL | 3 | Conflicting CASCADE, stale FK target, duplicate FK constraints |
| FLAG | 5 | Missing FKs, missing CHECKs, PK function inconsistency |
| MINOR | 7 | Redundant indexes, nullable timestamps, default inconsistency |
| CLEAN | 2 | Well-designed tables with no issues |

---

## Per-Table Scorecards

### 1. `cbus`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `cbu_id UUID DEFAULT uuidv7()` — correct pattern |
| **FK** | CLEAN | 3 FKs to `entities`, `products`, `entity_types` — all RESTRICT |
| **Indexes** | CLEAN | 9 indexes: PK, UQ(name,jurisdiction), trigram on name, IVFFlat on embedding, partial on status |
| **Constraints** | CLEAN | CHECK on `status` (8 values), CHECK on `cbu_category` (5 values) |
| **Naming** | CLEAN | Consistent snake_case, descriptive column names |
| **Temporal** | MINOR | `created_at`/`updated_at` both NULLABLE with `DEFAULT now()` |

**Overall: CLEAN** — Best-designed table in the schema. Good CHECK constraints model the status FSM.

---

### 2. `entities`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `entity_id UUID DEFAULT uuidv7()` |
| **FK** | FLAG | **DUPLICATE FK** on `entity_type_id` — two constraints (`entities_entity_type_id_fkey` + `fk_entities_entity_type`) both referencing `entity_types(entity_type_id)`, both CASCADE |
| **Indexes** | CLEAN | PK, UQ(entity_type_id,name), trigram on name, btree on name_norm, btree on entity_type_id |
| **Constraints** | CLEAN | UQ(entity_type_id, name) enforces natural key |
| **Naming** | CLEAN | Consistent |
| **Temporal** | MINOR | `created_at`/`updated_at` NULLABLE, `DEFAULT now()` |

**Overall: MINOR** — Duplicate FK is harmless but indicates migration layering debt.

---

### 3. `entity_funds`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `entity_fund_id UUID DEFAULT uuidv7()` |
| **FK** | CLEAN | FK to `entities(entity_id)` CASCADE — correct 1:1 extension pattern |
| **Indexes** | MINOR | **Redundant unique index** `entity_funds_entity_id_key` duplicates the UNIQUE constraint on `entity_id` |
| **Constraints** | CLEAN | UNIQUE on entity_id enforces 1:1 |
| **Naming** | CLEAN | |
| **Temporal** | MINOR | NULLABLE `created_at`/`updated_at` |

**Overall: MINOR** — Redundant index wastes space but causes no correctness issues.

---

### 4. `entity_parent_relationships`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `relationship_id UUID DEFAULT uuidv7()` |
| **FK** | CLEAN | 2 FKs to `entities` (parent, child) — both CASCADE |
| **Indexes** | CLEAN | PK + btree on parent_entity_id, child_entity_id |
| **Constraints** | CLEAN | |
| **Naming** | CLEAN | |
| **Temporal** | MINOR | NULLABLE `created_at` |

**Overall: CLEAN**

---

### 5. `cbu_entity_roles`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `cbu_entity_role_id UUID DEFAULT uuidv7()` |
| **FK** | CRITICAL | **DUPLICATE FKs x3**: `cbu_id` has 2 FKs (both CASCADE), `entity_id` has 2 FKs (both CASCADE), `role_id` has 2 FKs (both CASCADE). Six total constraints for three columns. |
| **Indexes** | MINOR | **Redundant unique index** `idx_cbu_entity_roles_unique` duplicates the UNIQUE constraint `cbu_entity_roles_cbu_id_entity_id_role_id_key` on (cbu_id, entity_id, role_id) |
| **Constraints** | CLEAN | UQ(cbu_id, entity_id, role_id) enforces natural key |
| **Naming** | CLEAN | |
| **Temporal** | MINOR | `created_at` default `(now() AT TIME ZONE 'utc')`, `updated_at` default `now()` — **inconsistent within same table** |

**Overall: FLAG** — Duplicate FKs are harmless but the temporal default inconsistency within one table is sloppy.

---

### 6. `ubo_registry`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `ubo_registry_id UUID DEFAULT uuidv7()` |
| **FK** | CRITICAL | **DUPLICATE FKs x3** on `cbu_id`, `entity_id`, `ultimate_parent_entity_id`. Worse: **CONFLICTING CASCADE on `cbu_id`** — `ubo_registry_cbu_id_fkey` uses `SET NULL` while `fk_ubo_registry_cbu_id` uses `CASCADE`. PostgreSQL behavior is undefined when two FK constraints on the same column have different actions. |
| **Indexes** | CLEAN | PK + btree on cbu_id, entity_id |
| **Constraints** | FLAG | **Missing CHECK** constraints on `screening_result`, `workflow_type`, `qualifying_reason`, `relationship_type` — all free-text VARCHAR columns that should be enum-constrained |
| **Naming** | CLEAN | |
| **Temporal** | MINOR | NULLABLE `created_at`/`updated_at` |

**Overall: CRITICAL** — The conflicting CASCADE policies on `cbu_id` are a data integrity risk. If a CBU is deleted, one FK says "set ubo_registry.cbu_id to NULL" while the other says "delete the ubo_registry row." PostgreSQL will execute both and the result depends on firing order.

---

### 7. `kyc_ubo_evidence`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | FLAG | `evidence_id UUID DEFAULT gen_random_uuid()` — uses `gen_random_uuid()` instead of project-standard `uuidv7()` |
| **FK** | CRITICAL | FK references `kyc_ubo_registry(ubo_registry_id)` — this is a **stale FK target**. The live table is `ubo_registry`, and `kyc_ubo_registry` is a separate (older) table. Evidence rows cannot reference entries in the current `ubo_registry`. |
| **Indexes** | CLEAN | PK + btree on ubo_registry_id |
| **Constraints** | CLEAN | |
| **Naming** | CLEAN | |
| **Temporal** | FLAG | **Missing `updated_at` column entirely**. Only has `created_at` (also NULLABLE). |

**Overall: CRITICAL** — The stale FK target means this table is operationally disconnected from the live UBO registry. The `gen_random_uuid()` PK default breaks the project-wide `uuidv7()` convention (which provides time-ordering).

---

### 8. `cases`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `case_id UUID DEFAULT uuidv7()` |
| **FK** | FLAG | **Missing FKs**: `client_group_id` has no FK to `client_group`, `assigned_analyst_id` and `assigned_reviewer_id` have no FK to any user/entity table, `priority` has no CHECK |
| **Indexes** | CLEAN | PK + btree on cbu_id, status |
| **Constraints** | CLEAN | Good CHECK on `status` (12-value FSM), CHECK on `case_type` (4 values) |
| **Naming** | CLEAN | |
| **Temporal** | MINOR | NULLABLE `created_at`/`updated_at` |

**Overall: FLAG** — The status FSM is well-modeled but dangling `client_group_id` without FK allows orphan references.

---

### 9. `case_import_runs`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `import_run_id UUID DEFAULT uuidv7()` |
| **FK** | CLEAN | FK to `cases(case_id)` CASCADE |
| **Indexes** | CLEAN | PK + btree on case_id |
| **Constraints** | CLEAN | |
| **Naming** | CLEAN | |
| **Temporal** | MINOR | NULLABLE `created_at` |

**Overall: CLEAN**

---

### 10. `client_group`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `group_id UUID DEFAULT uuidv7()` |
| **FK** | CLEAN | No FKs (root entity) |
| **Indexes** | CLEAN | PK + UQ on canonical_name |
| **Constraints** | CLEAN | UQ(canonical_name) enforces natural key |
| **Naming** | CLEAN | |
| **Temporal** | MINOR | NULLABLE `created_at`/`updated_at` |

**Overall: CLEAN**

---

### 11. `client_group_alias`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `alias_id UUID DEFAULT uuidv7()` |
| **FK** | CLEAN | FK to `client_group(group_id)` CASCADE |
| **Indexes** | CLEAN | PK + UQ(group_id, alias_normalized), partial index on is_primary |
| **Constraints** | CLEAN | Good partial unique index for primary alias |
| **Naming** | CLEAN | |
| **Temporal** | MINOR | NULLABLE `created_at` |

**Overall: CLEAN**

---

### 12. `client_group_anchor`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `anchor_id UUID DEFAULT uuidv7()` |
| **FK** | CLEAN | FKs to `client_group(group_id)` CASCADE and `entities(entity_id)` RESTRICT |
| **Indexes** | CLEAN | PK + UQ(group_id, role, jurisdiction_code) |
| **Constraints** | CLEAN | Composite UQ enforces one anchor per role per jurisdiction |
| **Naming** | CLEAN | |
| **Temporal** | MINOR | NULLABLE `created_at` |

**Overall: CLEAN**

---

### 13. `client_group_entity`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `id UUID DEFAULT uuidv7()` |
| **FK** | CLEAN | FKs to `client_group(group_id)` CASCADE and `entities(entity_id)` CASCADE |
| **Indexes** | CLEAN | PK + UQ(group_id, entity_id) |
| **Constraints** | CLEAN | |
| **Naming** | CLEAN | |
| **Temporal** | MINOR | NULLABLE `created_at` |

**Overall: CLEAN**

---

### 14. `trading_profiles`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `profile_id UUID DEFAULT uuidv7()` |
| **FK** | CLEAN | FK to `cbus(cbu_id)` CASCADE |
| **Indexes** | CLEAN | PK + btree on cbu_id |
| **Constraints** | CLEAN | |
| **Naming** | CLEAN | |
| **Temporal** | MINOR | NULLABLE `created_at`/`updated_at` |

**Overall: CLEAN**

---

### 15. `products`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `product_id UUID DEFAULT uuidv7()` |
| **FK** | CLEAN | No FKs (reference data) |
| **Indexes** | CLEAN | PK + UQ on product_code |
| **Constraints** | CLEAN | UQ(product_code) enforces natural key |
| **Naming** | CLEAN | |
| **Temporal** | MINOR | NULLABLE `created_at` |

**Overall: CLEAN**

---

### 16. `cbu_structure_links`

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `link_id UUID DEFAULT uuidv7()` |
| **FK** | CLEAN | FKs to `cbus` for both parent and child — CASCADE |
| **Indexes** | CLEAN | PK + UQ(parent_cbu_id, child_cbu_id) + btree on child_cbu_id |
| **Constraints** | CLEAN | CHECK `parent_cbu_id <> child_cbu_id` prevents self-links, CHECK on `link_type` and `status` |
| **Naming** | CLEAN | |
| **Temporal** | CLEAN | `created_at NOT NULL DEFAULT now()`, `updated_at NOT NULL DEFAULT now()` — **best temporal pattern in the schema** |

**Overall: CLEAN** — Newest table, demonstrates the target pattern. NOT NULL temporal columns with defaults.

---

## Cross-Table Consistency Findings

### C-1: Duplicate Foreign Key Constraints (CRITICAL)

Six duplicate FK constraint pairs exist across three tables, created by successive migrations adding the same FK:

| Table | Column | Constraint 1 | Constraint 2 | Risk |
|-------|--------|--------------|--------------|------|
| `entities` | `entity_type_id` | `entities_entity_type_id_fkey` (CASCADE) | `fk_entities_entity_type` (CASCADE) | Low — same action |
| `cbu_entity_roles` | `cbu_id` | `cbu_entity_roles_cbu_id_fkey` (CASCADE) | `fk_cbu_entity_roles_cbu_id` (CASCADE) | Low — same action |
| `cbu_entity_roles` | `entity_id` | `cbu_entity_roles_entity_id_fkey` (CASCADE) | `fk_cbu_entity_roles_entity_id` (CASCADE) | Low — same action |
| `cbu_entity_roles` | `role_id` | `cbu_entity_roles_role_id_fkey` (CASCADE) | `fk_cbu_entity_roles_role_id` (CASCADE) | Low — same action |
| `ubo_registry` | `entity_id` | `ubo_registry_entity_id_fkey` (CASCADE) | `fk_ubo_registry_entity_id` (CASCADE) | Low — same action |
| `ubo_registry` | `ultimate_parent_entity_id` | `ubo_registry_ultimate_parent_entity_id_fkey` (CASCADE) | `fk_ubo_registry_ult_parent` (CASCADE) | Low — same action |

**Special case — CONFLICTING:**

| Table | Column | Constraint 1 | Constraint 2 | Risk |
|-------|--------|--------------|--------------|------|
| `ubo_registry` | `cbu_id` | `ubo_registry_cbu_id_fkey` (**SET NULL**) | `fk_ubo_registry_cbu_id` (**CASCADE**) | **HIGH — undefined delete behavior** |

**Remediation:** Drop the duplicate constraints. For `ubo_registry.cbu_id`, decide on one policy (likely CASCADE given custody domain semantics) and drop the other.

### C-2: Stale FK Target — `kyc_ubo_evidence` → `kyc_ubo_registry` (CRITICAL)

`kyc_ubo_evidence.ubo_registry_id` references `kyc_ubo_registry(ubo_registry_id)`, but the live operational table is `ubo_registry`. Both tables exist as real heap tables (`relkind='r'`), indicating an incomplete schema migration/split. Evidence rows inserted against `ubo_registry` entries will fail the FK check.

**Remediation:** Migrate the FK to point to `ubo_registry(ubo_registry_id)`, or consolidate the two tables if `kyc_ubo_registry` is truly legacy.

### C-3: Universally Nullable Temporal Columns (MINOR — systemic)

Every table except `cbu_structure_links` has `created_at` and `updated_at` (where present) as **NULLABLE** columns. While defaults are set, any INSERT that explicitly passes `NULL` will succeed, creating rows with no audit timestamp.

**Target pattern** (from `cbu_structure_links`):
```sql
created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
```

**Remediation:** Add `NOT NULL` constraints in a forward migration. Safe because defaults ensure no existing rows have NULLs (verify first with `SELECT count(*) WHERE created_at IS NULL`).

### C-4: Inconsistent Timestamp Default Functions (MINOR)

| Pattern | Tables |
|---------|--------|
| `DEFAULT now()` | Most tables (cbus, entities, cases, etc.) |
| `DEFAULT (now() AT TIME ZONE 'utc')` | `cbu_entity_roles.created_at` |
| Mixed within same table | `cbu_entity_roles`: `created_at` uses TZ cast, `updated_at` uses plain `now()` |

Since `TIMESTAMPTZ` stores UTC internally regardless of session timezone, the `AT TIME ZONE 'utc'` cast is redundant but not harmful. The inconsistency is cosmetic.

**Remediation:** Standardize on `DEFAULT now()` (simpler, equivalent for TIMESTAMPTZ).

### C-5: PK Default Function Inconsistency (FLAG)

| Pattern | Tables |
|---------|--------|
| `DEFAULT uuidv7()` | All tables except one |
| `DEFAULT gen_random_uuid()` | `kyc_ubo_evidence.evidence_id` |

`uuidv7()` provides time-ordered UUIDs (sortable, monotonic, index-friendly). `gen_random_uuid()` is random, breaking this property.

**Remediation:** Alter `kyc_ubo_evidence.evidence_id` default to `uuidv7()`.

### C-6: Missing `updated_at` Column (FLAG)

`kyc_ubo_evidence` has no `updated_at` column at all. If evidence records are ever updated (e.g., verification status changes), there is no audit trail of when.

**Remediation:** Add `updated_at TIMESTAMPTZ NOT NULL DEFAULT now()` with an update trigger.

---

## Severity-Tagged Issue List

### CRITICAL

| ID | Table | Issue | Impact | Remediation |
|----|-------|-------|--------|-------------|
| **CR-1** | `ubo_registry` | Conflicting CASCADE policies on `cbu_id`: `SET NULL` vs `CASCADE` | On CBU delete, behavior is undefined — may NULL the column OR delete the row depending on constraint evaluation order | Drop one FK. Recommend keeping CASCADE (custody domain: UBO records belong to the CBU). |
| **CR-2** | `kyc_ubo_evidence` | FK references `kyc_ubo_registry` (old table) instead of `ubo_registry` (live table) | Evidence rows cannot be inserted for current UBO registry entries; FK violation on any new evidence | Alter FK to reference `ubo_registry(ubo_registry_id)`. |
| **CR-3** | `entities`, `cbu_entity_roles`, `ubo_registry` | 6 duplicate FK constraint pairs across 3 tables | Wasted catalog space; confusing for schema introspection; masks the CR-1 conflict | Drop duplicate constraints (keep the one with the desired cascade policy). |

### FLAG

| ID | Table | Issue | Impact | Remediation |
|----|-------|-------|--------|-------------|
| **FL-1** | `cases` | `client_group_id` has no FK to `client_group` | Orphan group references possible | Add FK with RESTRICT or SET NULL. |
| **FL-2** | `cases` | `assigned_analyst_id`/`assigned_reviewer_id` have no FK | Dangling analyst/reviewer UUIDs possible | Add FK if a user/entity table exists for analysts. |
| **FL-3** | `ubo_registry` | Missing CHECK on `screening_result`, `workflow_type`, `qualifying_reason`, `relationship_type` | Free-text values where enum constraint expected | Add CHECK constraints matching the domain's valid values. |
| **FL-4** | `kyc_ubo_evidence` | PK uses `gen_random_uuid()` instead of `uuidv7()` | Breaks time-ordering convention; less index-friendly | Alter default to `uuidv7()`. |
| **FL-5** | `kyc_ubo_evidence` | Missing `updated_at` column | No mutation timestamp audit trail | Add column with trigger. |

### MINOR

| ID | Table | Issue | Impact | Remediation |
|----|-------|-------|--------|-------------|
| **MI-1** | All except `cbu_structure_links` | `created_at`/`updated_at` are NULLABLE | Explicit NULL inserts bypass audit | Add `NOT NULL` after verifying no existing NULLs. |
| **MI-2** | `cbu_entity_roles` | Redundant unique index `idx_cbu_entity_roles_unique` duplicates the UNIQUE constraint | Wasted storage + write amplification | Drop the redundant index. |
| **MI-3** | `entity_funds` | Redundant unique index `entity_funds_entity_id_key` duplicates the UNIQUE constraint on `entity_id` | Wasted storage | Drop the redundant index. |
| **MI-4** | `cbu_entity_roles` | Inconsistent timestamp defaults within same table (`AT TIME ZONE 'utc'` vs plain `now()`) | Cosmetic inconsistency | Standardize to `DEFAULT now()`. |
| **MI-5** | `entity_parent_relationships` | No `updated_at` column | Minor — relationships may be immutable by design | Add if relationships are mutable. |
| **MI-6** | `client_group_alias` | No `updated_at` column | Minor — aliases may be immutable by design | Add if aliases are mutable. |
| **MI-7** | `client_group_entity` | No `updated_at` column | Minor | Add if entity-group links are mutable. |

---

## Index Recommendations

### Drop Redundant Indexes

| Index | Table | Reason | Estimated Savings |
|-------|-------|--------|-------------------|
| `idx_cbu_entity_roles_unique` | `cbu_entity_roles` | Duplicates UNIQUE constraint `cbu_entity_roles_cbu_id_entity_id_role_id_key` | ~same size as UQ index |
| `entity_funds_entity_id_key` | `entity_funds` | Duplicates UNIQUE constraint (if both exist as separate B-tree indexes) | Small (1:1 table) |

### Consider Adding

| Table | Columns | Type | Rationale |
|-------|---------|------|-----------|
| `cases` | `client_group_id` | btree | Supports lookup by client group (common query pattern) |
| `cases` | `assigned_analyst_id` | btree | Supports analyst workload queries |
| `ubo_registry` | `(cbu_id, entity_id)` | btree composite | Supports the common "all UBOs for a CBU" query |
| `kyc_ubo_evidence` | `(ubo_registry_id, evidence_type)` | btree composite | Supports evidence lookup by type within a UBO entry |

### Existing Indexes — No Changes Needed

The following index patterns are well-designed and should be preserved:

- **Trigram indexes** on `cbus.name` and `entities.name` — essential for fuzzy search
- **IVFFlat vector index** on `cbus.embedding` — required for semantic search
- **Partial indexes** on `client_group_alias(group_id) WHERE is_primary` — elegant pattern for "one primary per group"
- **Self-link CHECK** on `cbu_structure_links(parent_cbu_id <> child_cbu_id)` — prevents graph cycles at the single-hop level

---

## Recommended Migration Priority

1. **Immediate (CR-1):** Fix `ubo_registry.cbu_id` conflicting cascade — this is a latent data corruption risk
2. **Immediate (CR-2):** Fix `kyc_ubo_evidence` FK target — this blocks evidence insertion for current UBO entries
3. **Soon (CR-3):** Drop duplicate FK constraints — cleanup that reduces confusion
4. **Soon (FL-3):** Add CHECK constraints to `ubo_registry` enum columns
5. **Backlog (MI-1):** Add NOT NULL to temporal columns (safe, high-volume migration)
6. **Backlog (MI-2, MI-3):** Drop redundant indexes
7. **Backlog (FL-1, FL-2):** Add missing FKs to `cases`

---

## Appendix: Table Coverage

| # | Table | Rows (approx) | Rating |
|---|-------|----------------|--------|
| 1 | `cbus` | 595 | CLEAN |
| 2 | `entities` | 1,962 | MINOR |
| 3 | `entity_funds` | 619 | MINOR |
| 4 | `entity_parent_relationships` | 1,286 | CLEAN |
| 5 | `cbu_entity_roles` | 3,259 | FLAG |
| 6 | `ubo_registry` | 1,371 | CRITICAL |
| 7 | `kyc_ubo_evidence` | 0 | CRITICAL |
| 8 | `cases` | 17 | FLAG |
| 9 | `case_import_runs` | 7 | CLEAN |
| 10 | `client_group` | 4 | CLEAN |
| 11 | `client_group_alias` | 51 | CLEAN |
| 12 | `client_group_anchor` | 9 | CLEAN |
| 13 | `client_group_entity` | 1,605 | CLEAN |
| 14 | `trading_profiles` | 52 | CLEAN |
| 15 | `products` | 4 | CLEAN |
| 16 | `cbu_structure_links` | 2 | CLEAN |
