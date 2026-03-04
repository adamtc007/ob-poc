# Schema Consolidation Migration Plan (`kyc` + `custody` + `client_portal` -> `ob-poc`)

Last updated: March 4, 2026

## 1. Objective

Consolidate runtime business schemas into a single application schema:

- Move all `kyc.*` tables into `"ob-poc".*`
- Move all `custody.*` tables into `"ob-poc".*`
- Move all `client_portal.*` tables into `"ob-poc".*`
- Remove legacy schemas after cutover:
  - `DROP SCHEMA kyc CASCADE`
  - `DROP SCHEMA custody CASCADE`
  - `DROP SCHEMA client_portal CASCADE`

`sem_reg`, `sem_reg_authoring`, and `stewardship` are out of scope for this consolidation.

## 2. Current State Snapshot

Observed base table counts:

- `"ob-poc"`: 206
- `kyc`: 37
- `custody`: 31
- `client_portal`: 6

Important collision risks in target schema (`"ob-poc"` already has these names):

- `client_portal.sessions` collides with `"ob-poc".sessions`
- `kyc.ubo_registry` collides with `"ob-poc".ubo_registry`
- `kyc.ubo_evidence` collides with `"ob-poc".ubo_evidence`

High FK density exists from `kyc`/`custody`/`client_portal` into `"ob-poc"` and within source schemas; migration must preserve constraints and sequence/default behavior.

## 3. Design Decisions

1. Target schema remains `"ob-poc"` (quoted, hyphenated schema name).
2. Use **rename-on-collision** for moved tables that collide with existing `"ob-poc"` table names.
3. Use a **compatibility window** with legacy views in old schemas for a safe code cutover.
4. Migrate by `ALTER TABLE ... SET SCHEMA` (metadata move) where possible, not row copy.
5. Recreate/repair dependent objects (views, functions, triggers, grants) deterministically.

## 4. Canonical Rename Map (Collision Handling)

Recommended target names for colliding tables:

- `kyc.ubo_registry` -> `"ob-poc".kyc_ubo_registry`
- `kyc.ubo_evidence` -> `"ob-poc".kyc_ubo_evidence`
- `client_portal.sessions` -> `"ob-poc".client_portal_sessions`

Non-colliding tables keep current table names when moved to `"ob-poc"`.

## 5. Migration Phases

## Phase A: Preflight Inventory + Freeze

1. Create machine-readable inventory artifacts:
   - tables, indexes, constraints, triggers, views, functions, sequences
   - grants and ownership
2. Generate dependency graph:
   - FKs across `kyc`, `custody`, `client_portal`, `"ob-poc"`
3. Enforce a deploy freeze window:
   - no DDL in affected schemas
   - write traffic drained for cutover

Deliverables:

- `docs/todo/schema-consolidation-inventory.json`
- `docs/todo/schema-consolidation-fk-graph.csv`

## Phase B: Add Compatibility Layer (Forward)

1. Keep current app working while preparing SQL cutover.
2. Add migration-safe resolution helpers (if needed) and feature flags.
3. Introduce compatibility views in source schemas only after table moves (Phase D), not before.

## Phase C: Move Tables + Sequences to `"ob-poc"`

For each table in `kyc`, `custody`, `client_portal`:

1. If collision:
   - rename table in source schema first (or set destination renamed table name post-move)
2. Move table:
   - `ALTER TABLE <schema>.<table> SET SCHEMA "ob-poc";`
3. Move owned sequences:
   - `ALTER SEQUENCE <schema>.<seq> SET SCHEMA "ob-poc";`
4. Rewire defaults:
   - ensure `nextval('"ob-poc".<seq>'::regclass)` is set
5. Recreate missing FKs/index dependencies if any reference breaks

Notes:

- PostgreSQL updates OIDs/references for many dependencies automatically, but check all constraints and defaults explicitly.
- Triggers and trigger functions that embed fully-qualified SQL need review.

## Phase D: Legacy Schema Compatibility Views

After table moves, create read/write-compatible views in old schemas to keep old SQL temporarily functional:

- `CREATE VIEW kyc.<old_name> AS SELECT * FROM "ob-poc".<new_name>;`
- `CREATE VIEW custody.<old_name> AS SELECT * FROM "ob-poc".<new_name>;`
- `CREATE VIEW client_portal.<old_name> AS SELECT * FROM "ob-poc".<new_name>;`

For mutable operations, add `INSTEAD OF` triggers only where direct updatable views are insufficient.

Compatibility window target: 1-2 releases max.

## Phase E: Application Code Cutover

Update SQL references in runtime code:

- `src/` and active crates (`ob-poc-web`, `ob-workflow`, `ob-semantic-matcher`, etc.)
- Replace `kyc.<table>`/`custody.<table>`/`client_portal.<table>` with `"ob-poc".<table>`
- Apply collision rename map in queries and DTO bindings

Verification gates:

- `cargo check`
- `cargo clippy -- -D warnings`
- targeted integration suites touching KYC/custody/client portal routes
- API smoke flows for chat + execute + client portal endpoints

## Phase F: Hard Cutover + Schema Removal

Prerequisites:

- No runtime SQL references to `kyc.`/`custody.`/`client_portal.`
- Compatibility view access metrics show zero legacy hits for agreed soak period

Steps:

1. Drop compatibility views
2. Drop schemas:
   - `DROP SCHEMA kyc CASCADE;`
   - `DROP SCHEMA custody CASCADE;`
   - `DROP SCHEMA client_portal CASCADE;`
3. Re-run schema inventory and ensure only `"ob-poc"` + Semantic OS schemas remain for app workloads

## 6. Migration File Plan

Suggested migration sequence:

1. `104_schema_consolidation_preflight.sql`
   - audit tables, collision checks, fail-fast guards
2. `105_move_kyc_tables_to_ob_poc.sql`
3. `106_move_custody_tables_to_ob_poc.sql`
4. `107_move_client_portal_tables_to_ob_poc.sql`
5. `108_create_legacy_schema_compat_views.sql`
6. `109_code_cutover_cleanup_constraints.sql`
7. `110_drop_legacy_schemas.sql`

## 7. SQL Patterns (Templates)

### Move table

```sql
ALTER TABLE kyc.case_events SET SCHEMA "ob-poc";
```

### Handle collision with deterministic rename

```sql
ALTER TABLE kyc.ubo_registry RENAME TO kyc_ubo_registry;
ALTER TABLE kyc.kyc_ubo_registry SET SCHEMA "ob-poc";
```

### Move sequence and repair default

```sql
ALTER SEQUENCE kyc.case_events_event_id_seq SET SCHEMA "ob-poc";
ALTER TABLE "ob-poc".case_events
  ALTER COLUMN event_id
  SET DEFAULT nextval('"ob-poc".case_events_event_id_seq'::regclass);
```

### Compatibility view

```sql
CREATE VIEW kyc.case_events AS
SELECT * FROM "ob-poc".case_events;
```

## 8. Validation Checklist

Database-level:

- all moved tables exist only in `"ob-poc"`
- all PK/FK constraints valid
- no defaults point to dropped-schema sequences
- no functions/views depend on dropped schemas

Application-level:

- chat API flows pass
- KYC workflow flows pass
- custody/trading profile flows pass
- client portal login/submission/escalation flows pass

AffinityGraph/semantic:

- rerun `registry.governance-gaps` and compare baseline
- rerun utterance coverage harness after code cutover

## 9. Rollback Plan

Rollback boundary: before Phase F schema drop.

Rollback method:

1. Keep full DB backup/snapshot before Phase C.
2. If cutover fails:
   - keep compatibility views active
   - revert application release
   - optionally move tables back schema-by-schema using reverse `ALTER TABLE ... SET SCHEMA`
3. Do not drop source schemas until post-soak signoff.

## 10. Risks and Mitigations

1. Hidden SQL references in app/tests/migrations
   - Mitigation: exhaustive grep + runtime query audit logs
2. Name collisions causing silent behavior drift
   - Mitigation: explicit rename map + contract tests
3. Sequence/default breakage after schema move
   - Mitigation: post-move sequence/default verification script
4. Updatable view incompatibilities during compatibility phase
   - Mitigation: targeted `INSTEAD OF` triggers or keep write paths on new schema only

## 11. Ownership + Execution Model

Recommended execution split:

1. DBA/Platform: Phase A-C-F SQL and backup/restore controls
2. App Engineering: Phase E code cutover + test harness updates
3. Domain/SME: approve collision rename semantics (`ubo_*`, portal session naming)

