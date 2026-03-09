# TODO: Schema Consolidation → Two Schemas Only

**Goal:** Collapse everything into `ob-poc` (business) + `sem_reg*` (SemOS). Kill all dead schemas.  
**Date:** 2026-03-08  
**Scope:** 35 tables, 25 FK constraints, ~77 Rust source files to update, 125 migration files

---

## Phase 1 — Move `stewardship` into `sem_reg` (9 tables)

Stewardship has zero FKs to `ob-poc` and 4 FKs into `sem_reg.changesets` / `sem_reg.changeset_entries`. It governs the registry — it belongs with SemOS.

### 1.1 Migration SQL

```sql
-- Move stewardship tables to sem_reg schema
ALTER TABLE stewardship.basis_claims       SET SCHEMA sem_reg;
ALTER TABLE stewardship.basis_records      SET SCHEMA sem_reg;
ALTER TABLE stewardship.conflict_records   SET SCHEMA sem_reg;
ALTER TABLE stewardship.events             SET SCHEMA sem_reg;
ALTER TABLE stewardship.focus_states       SET SCHEMA sem_reg;
ALTER TABLE stewardship.idempotency_keys   SET SCHEMA sem_reg;
ALTER TABLE stewardship.templates          SET SCHEMA sem_reg;
ALTER TABLE stewardship.verb_implementation_bindings SET SCHEMA sem_reg;
ALTER TABLE stewardship.viewport_manifests SET SCHEMA sem_reg;

DROP SCHEMA stewardship;
```

**FK resolution:** All 5 FK constraints are internal (`basis_claims → basis_records`) or point to `sem_reg.*` — `SET SCHEMA` preserves them automatically. No cross-schema rewiring needed.

### 1.2 Rust source updates (4 files)

| File | Change |
|------|--------|
| `sem_reg/stewardship/focus.rs` | `stewardship.focus_states` → `sem_reg.focus_states` |
| `sem_reg/stewardship/idempotency.rs` | `stewardship.idempotency_keys` → `sem_reg.idempotency_keys` |
| `sem_reg/stewardship/show_loop.rs` | All `stewardship.*` → `sem_reg.*` |
| `sem_reg/stewardship/store.rs` | All `stewardship.*` → `sem_reg.*` |

```bash
# Bulk find/replace
cd rust/src
grep -rn 'stewardship\.' sem_reg/stewardship/ --include="*.rs" -l
# sed -i 's/stewardship\./sem_reg./g' on each file — then verify
```

### 1.3 Update domain_metadata.yaml

In `rust/config/sem_os_seeds/domain_metadata.yaml`, the `stewardship` domain already uses `stewardship.*` prefixed table names. Change to `sem_reg.*`:

```
stewardship:
  tables:
    sem_reg.basis_claims:       # was stewardship.basis_claims
    sem_reg.basis_records:      # was stewardship.basis_records
    ...
```

- [ ] Migration SQL written and tested
- [ ] 4 Rust files updated
- [ ] domain_metadata.yaml updated
- [ ] `DROP SCHEMA stewardship;` executed
- [ ] Integration tests pass

---

## Phase 2 — Move `agent` into `ob-poc` (8 tables)

### 2.1 Migration SQL

```sql
ALTER TABLE agent.entity_aliases      SET SCHEMA "ob-poc";
ALTER TABLE agent.events              SET SCHEMA "ob-poc";
ALTER TABLE agent.invocation_phrases  SET SCHEMA "ob-poc";
ALTER TABLE agent.learning_audit      SET SCHEMA "ob-poc";
ALTER TABLE agent.learning_candidates SET SCHEMA "ob-poc";
ALTER TABLE agent.lexicon_tokens      SET SCHEMA "ob-poc";
ALTER TABLE agent.phrase_blocklist    SET SCHEMA "ob-poc";
ALTER TABLE agent.user_learned_phrases SET SCHEMA "ob-poc";

DROP SCHEMA agent;
```

**FK resolution:** `entity_aliases → entities` already points to `ob-poc` — moves cleanly. `learning_audit → learning_candidates` is internal — both move together.

**Name collision check:** No `ob-poc` table uses `agent_*` prefix — clean.

### 2.2 Rust source updates (19 files)

Heavy usage in `agent/learning/inspector.rs` (~20 queries). Bulk replace `agent\.` → `"ob-poc".` in SQL strings, but **be careful**: the Rust module path `agent::` must not be touched.

Strategy: target only SQL string literals containing `agent.entity_aliases`, `agent.events`, `agent.learning_candidates`, `agent.invocation_phrases`, `agent.learning_audit`, `agent.lexicon_tokens`, `agent.phrase_blocklist`, `agent.user_learned_phrases`.

```bash
# Safe grep — only SQL contexts
grep -rn '"agent\.\|agent\.\(entity_aliases\|events\|learning\|invocation\|lexicon\|phrase_blocklist\|user_learned\)' rust/src/ --include="*.rs"
```

Also update `agent.upsert_entity_alias()` and `agent.upsert_lexicon_token()` — these are Postgres functions that need `SET SCHEMA` too:

```sql
ALTER FUNCTION agent.upsert_entity_alias SET SCHEMA "ob-poc";
ALTER FUNCTION agent.upsert_lexicon_token SET SCHEMA "ob-poc";
```

### 2.3 Update domain_metadata.yaml

The `agent` domain uses `agent.*` prefixed names. Change to bare names (ob-poc default):

```
agent:
  tables:
    entity_aliases:        # was agent.entity_aliases
    events:                # ← NAME COLLISION risk — rename to agent_events?
    invocation_phrases:    # was agent.invocation_phrases
    ...
```

**Potential problem:** `agent.events` bare name collides with the concept of "events" broadly. Consider renaming to `agent_events` during migration, or leave as `events` if context is clear.

- [ ] Migration SQL written and tested
- [ ] Check for Postgres functions (`upsert_entity_alias`, `upsert_lexicon_token`) and move them
- [ ] 19 Rust files updated (SQL strings only, not module paths)
- [ ] domain_metadata.yaml updated
- [ ] `DROP SCHEMA agent;` executed
- [ ] Integration tests pass

---

## Phase 3 — Move `teams` into `ob-poc` (7 tables)

### 3.1 Migration SQL

```sql
ALTER TABLE teams.teams                    SET SCHEMA "ob-poc";
ALTER TABLE teams.memberships              SET SCHEMA "ob-poc";
ALTER TABLE teams.team_cbu_access          SET SCHEMA "ob-poc";
ALTER TABLE teams.team_service_entitlements SET SCHEMA "ob-poc";
ALTER TABLE teams.access_review_campaigns  SET SCHEMA "ob-poc";
ALTER TABLE teams.access_review_items      SET SCHEMA "ob-poc";
ALTER TABLE teams.access_attestations      SET SCHEMA "ob-poc";

DROP SCHEMA teams;
```

**FK resolution:**
- `teams.teams → ob-poc.entities` — moves to same schema, FK preserved
- `teams.memberships → ob-poc.clients` — same
- `teams.team_cbu_access → ob-poc.cbus` — same
- Internal FKs (`memberships → teams`, `access_review_items → campaigns`, etc.) — all move together

**Name collision check:** No existing `ob-poc.teams` table — clean.

### 3.2 Rust source updates (2 files)

| File | Change |
|------|--------|
| `domain_ops/team_ops.rs` | `teams.*` → `"ob-poc".*` |
| `domain_ops/access_review_ops.rs` | `teams.*` → `"ob-poc".*` |

- [ ] Migration SQL written and tested
- [ ] 2 Rust files updated
- [ ] domain_metadata.yaml updated
- [ ] `DROP SCHEMA teams;` executed

---

## Phase 4 — Move `feedback` into `ob-poc` (3 tables)

### 4.1 Migration SQL

```sql
-- feedback has custom ENUM types — move those first
ALTER TYPE feedback.actor_type      SET SCHEMA "ob-poc";
ALTER TYPE feedback.audit_action    SET SCHEMA "ob-poc";
ALTER TYPE feedback.error_type      SET SCHEMA "ob-poc";
ALTER TYPE feedback.issue_status    SET SCHEMA "ob-poc";
-- (check for others: remediation_path)

ALTER TABLE feedback.failures     SET SCHEMA "ob-poc";
ALTER TABLE feedback.occurrences  SET SCHEMA "ob-poc";
ALTER TABLE feedback.audit_log    SET SCHEMA "ob-poc";

DROP SCHEMA feedback;
```

**Critical:** The `feedback` schema defines **4+ custom PostgreSQL ENUMs** (`actor_type`, `audit_action`, `error_type`, `issue_status`, possibly `remediation_path`). These must move before the tables. Also update sqlx type annotations in Rust:

### 4.2 Rust source updates (3 files)

| File | Change |
|------|--------|
| `feedback/inspector.rs` | SQL: `feedback.*` → `"ob-poc".*` |
| `feedback/types.rs` | sqlx annotations: `type_name = "feedback.error_type"` → `type_name = "ob-poc.error_type"` (5 annotations) |
| `dsl_v2/planning_facade.rs` | Any feedback schema refs |

- [ ] Enum types moved first
- [ ] Migration SQL written and tested
- [ ] 3 Rust files updated (including 5+ sqlx type_name annotations)
- [ ] `DROP SCHEMA feedback;` executed

---

## Phase 5 — Move `events` + `sessions` into `ob-poc` (2 tables)

### 5.1 Migration SQL

```sql
-- sessions depends on events — move events first
ALTER TABLE events.log    SET SCHEMA "ob-poc";
ALTER TABLE sessions.log  SET SCHEMA "ob-poc";

DROP SCHEMA events;
DROP SCHEMA sessions;
```

**Name collision:** Both are called `log`. After moving to `ob-poc`, they'll conflict.

**Resolution:** Rename during move:

```sql
ALTER TABLE events.log   RENAME TO event_log;
ALTER TABLE events.event_log SET SCHEMA "ob-poc";

ALTER TABLE sessions.log RENAME TO session_log;
ALTER TABLE sessions.session_log SET SCHEMA "ob-poc";
```

**FK:** `sessions.log → events.log` becomes `session_log → event_log` — rewrite FK after rename.

### 5.2 Rust source updates (~24 files for sessions, ~18 for events)

`sessions.log` is heavily referenced (~24 files). Many of these will be Rust struct field names (`sessions`) not SQL — audit carefully to only change SQL string contexts.

- [ ] Rename `log` → `event_log` and `session_log` to avoid collision
- [ ] Migration SQL written and tested
- [ ] FK `session_log.event_id → event_log.id` rewritten
- [ ] Rust files updated (SQL contexts only)
- [ ] `DROP SCHEMA events; DROP SCHEMA sessions;` executed

---

## Phase 6 — Move `ob_ref` into `ob-poc` (5 tables) + resolve collisions

### 6.1 Resolve name collisions

**`regulators`** — both exist with different column widths and defaults:
- `ob_ref.regulators`: `varchar(50)` regulator_code, has `active` bool, `regulator_type` column
- `"ob-poc".regulators`: `varchar(20)` regulator_code, has `tier` column, no `active`

`ob_ref.regulators` is the richer table (referenced by `tollgate_evaluations` and `entity_regulatory_registrations`). **Decision needed:** merge columns into one canonical table or drop the weaker one.

Suggested approach:
```sql
-- Add missing columns from ob_ref to ob-poc version (or vice versa)
-- Migrate data from the weaker table
-- Drop the duplicate
-- Repoint FKs
```

**`role_types`** — same situation:
- `ob_ref.role_types`: UUID PK (`role_type_id`), has `category`, `active` columns
- `"ob-poc".role_types`: varchar PK (`role_code`), no UUID, no `active`

Different PK types — can't just merge. **Decision needed:** which PK strategy wins?

### 6.2 Migration SQL (non-colliding tables)

```sql
ALTER TABLE ob_ref.request_types      SET SCHEMA "ob-poc";
ALTER TABLE ob_ref.standards_mappings SET SCHEMA "ob-poc";
ALTER TABLE ob_ref.tollgate_definitions SET SCHEMA "ob-poc";

-- After collision resolution:
-- DROP TABLE ob_ref.regulators;     (or merge first)
-- DROP TABLE ob_ref.role_types;     (or merge first)
DROP SCHEMA ob_ref;
```

### 6.3 Rust source updates (6 files)

| File | Change |
|------|--------|
| `domain_ops/regulatory_ops.rs` | `ob_ref.*` → `"ob-poc".*` |
| `domain_ops/document_ops.rs` | `ob_ref.*` → `"ob-poc".*` |
| `domain_ops/skeleton_build_ops.rs` | `ob_ref.*` → `"ob-poc".*` |
| `domain_ops/request_ops.rs` | `ob_ref.*` → `"ob-poc".*` |
| `api/workflow_routes.rs` | `ob_ref.*` → `"ob-poc".*` |
| + 1 more | — |

- [ ] Collision resolution decided for `regulators` and `role_types`
- [ ] Data merged / duplicates dropped
- [ ] Non-colliding tables moved
- [ ] 6 Rust files updated
- [ ] `DROP SCHEMA ob_ref;` executed

---

## Phase 7 — Move `ob_kyc` into `ob-poc` (1 table)

```sql
ALTER TABLE ob_kyc.entity_regulatory_registrations SET SCHEMA "ob-poc";
DROP SCHEMA ob_kyc;
```

FK rewrite: `→ ob_ref.regulators` must already be resolved in Phase 6.

### Rust: 1 file (`domain_ops/regulatory_ops.rs`)

- [ ] Migration SQL
- [ ] 1 Rust file updated
- [ ] `DROP SCHEMA ob_kyc;` executed

---

## Post-migration verification

- [ ] `SELECT schema_name FROM information_schema.schemata` returns only: `ob-poc`, `sem_reg`, `sem_reg_authoring`, `sem_reg_pub`, `public`
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] All sqlx compile-time checks pass (run `cargo sqlx prepare`)
- [ ] domain_metadata.yaml: all `stewardship.*` refs → `sem_reg.*`, all other dead-schema prefixes → bare names
- [ ] Generate fresh `pg_dump` as new `master-schema.sql`

---

## Effort estimate

| Phase | Tables | FKs | Rust files | Risk |
|-------|--------|-----|------------|------|
| 1. stewardship → sem_reg | 9 | 5 (all internal/sem_reg) | 4 | Low |
| 2. agent → ob-poc | 8 | 2 | 19 | Medium (heavy inspector.rs) |
| 3. teams → ob-poc | 7 | 7 | 2 | Low |
| 4. feedback → ob-poc | 3 | 2 | 3 | Medium (enum types) |
| 5. events+sessions → ob-poc | 2 | 1 | ~30 | Medium (name collision, many refs) |
| 6. ob_ref → ob-poc | 5 | 3 | 6 | **High** (2 collisions need decisions) |
| 7. ob_kyc → ob-poc | 1 | 2 | 1 | Low (depends on Phase 6) |
