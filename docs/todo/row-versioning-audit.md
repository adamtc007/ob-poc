# Row-Versioning Audit (Phase 0f / Stream 2)

> **Purpose:** ground-truth inventory of entity tables requiring `row_version bigint` per Decision D1. Input to Stream 2 batched migrations.
> **Created:** 2026-04-18
> **Decisions:** D1 (explicit `row_version bigint` everywhere), D2 (per-entity-group batched rollout over ~2 weeks), Q1 (pre-lands ahead of Phase 0 gate — parallel stream).
> **Related:** `docs/todo/three-plane-architecture-v0.3.md` §10.5 (StateGateHash input set); `docs/todo/three-plane-architecture-implementation-plan-v0.1.md` §4 (Stream 2 critical-path parallel).

---

## 1. Executive summary

- **396 entity tables** scanned across `ob-poc`, `kyc`, and `custody` schemas.
- **0 tables** have explicit `row_version bigint` (D1-mandated form).
- **1 table** (`repl_sessions_v2`) has `version bigint` — unrelated semantics; not D1-compliant.
- **All tables use `updated_at` timestamps** — close but not monotonic-version-equivalent. Clock skew and same-millisecond collisions break the invariant required by `StateGateHash`.
- **42 high-priority gate-surface tables** require D1 remediation. This is the Stream 2 backfill scope.

## 2. High-priority gate-surface tables (D1 remediation required)

These are the tables touched by verbs reachable through `VerbExecutionPort`. Every row here must carry `row_version bigint NOT NULL DEFAULT 0` plus an UPDATE trigger that bumps on every row mutation — before Phase 5d (`StateGateHash` in-txn recheck) can be wired.

| # | Schema.Table | PK | Update Freq | Gate Prio | Group |
|---|---|---|---|---|---|
| 1 | `"ob-poc".cbus` | UUID | MED | **CRITICAL** | A — Core entity |
| 2 | `"ob-poc".entities` | UUID | MED | **CRITICAL** | A — Core entity |
| 3 | `"ob-poc".entity_relationships` | UUID | MED | **CRITICAL** | A — Core entity |
| 4 | `"ob-poc".entity_workstreams` | UUID | LOW | HIGH | A — Core entity |
| 5 | `"ob-poc".entity_limited_companies` | UUID | MED | HIGH | A — Core entity |
| 6 | `"ob-poc".entity_proper_persons` | UUID | MED | HIGH | A — Core entity |
| 7 | `"ob-poc".entity_partnerships` | UUID | LOW | MED | A — Core entity |
| 8 | `"ob-poc".entity_trusts` | UUID | LOW | MED | A — Core entity |
| 9 | `"ob-poc".entity_funds` | UUID | LOW | MED | A — Core entity |
| 10 | `"ob-poc".entity_parent_relationships` | UUID | LOW | HIGH | A — Core entity |
| 11 | `"ob-poc".entity_addresses` | UUID | LOW | MED | A — Core entity |
| 12 | `"ob-poc".entity_identifiers` | UUID | LOW | MED | A — Core entity |
| 13 | `kyc.ubo_registry` | UUID | LOW | **CRITICAL** | B — KYC/UBO |
| 14 | `kyc.ubo_evidence` | UUID | MED | MED | B — KYC/UBO |
| 15 | `kyc.investors` | UUID | MED | HIGH | B — KYC/UBO |
| 16 | `kyc.board_compositions` | UUID | LOW | MED | B — KYC/UBO |
| 17 | `kyc.trust_provisions` | UUID | LOW | MED | B — KYC/UBO |
| 18 | `kyc.partnership_capital` | UUID | LOW | MED | B — KYC/UBO |
| 19 | `kyc.dilution_instruments` | UUID | MED | MED | B — KYC/UBO |
| 20 | `kyc.appointment_rights` | UUID | LOW | MED | B — KYC/UBO |
| 21 | `kyc.fund_vehicles` | UUID | MED | MED | B — KYC/UBO |
| 22 | `kyc.fund_compartments` | UUID | MED | MED | B — KYC/UBO |
| 23 | `"ob-poc".deals` | UUID | MED | HIGH | C — Deals & contracts |
| 24 | `"ob-poc".deal_products` | UUID | MED | HIGH | C — Deals & contracts |
| 25 | `"ob-poc".legal_contracts` | UUID | LOW | MED | C — Deals & contracts |
| 26 | `"ob-poc".contract_products` | UUID | LOW | MED | C — Deals & contracts |
| 27 | `"ob-poc".booking_principal` | UUID | LOW | HIGH | C — Deals & contracts |
| 28 | `"ob-poc".booking_location` | UUID | LOW | MED | C — Deals & contracts |
| 29 | `"ob-poc".client_group` | UUID | LOW | HIGH | D — Client/Group |
| 30 | `"ob-poc".client_profile` | UUID | MED | HIGH | D — Client/Group |
| 31 | `"ob-poc".cbu_entity_roles` | UUID | MED | HIGH | D — Client/Group |
| 32 | `"ob-poc".cbu_ca_preferences` | UUID | MED | MED | D — Client/Group |
| 33 | `"ob-poc".attribute_registry` | UUID | LOW | MED | E — Registry/supporting |
| 34 | `"ob-poc".ownership_snapshots` | UUID | LOW | MED | E — Registry/supporting |
| 35 | `"ob-poc".remediation_events` | UUID | LOW | MED | E — Registry/supporting |
| 36 | `"ob-poc".shared_atom_registry` | UUID | LOW | MED | E — Registry/supporting |
| 37 | `"ob-poc".fund_vehicles` | UUID | MED | MED | E — Registry/supporting |
| 38 | `"ob-poc".fund_compartments` | UUID | MED | MED | E — Registry/supporting |
| 39 | `"ob-poc".dilution_instruments` | UUID | MED | MED | E — Registry/supporting |
| 40 | `"ob-poc".role_applicable_entity_types` | UUID | LOW | LOW | E — Registry/supporting |
| 41 | `"ob-poc".entity_relationships_history` | UUID | LOW | MED | F — Temporal/history |
| 42 | `"ob-poc".cbu_entity_roles_history` | UUID | LOW | MED | F — Temporal/history |

## 3. Grouping for batched D2 rollout

Each group becomes one migration. Migrations sequence by dependency (Group A first — everything else references entity ids).

| Group | Tables | Rationale |
|---|---|---|
| **A — Core entity (12)** | entities, cbus, entity_*, entity_relationships, entity_workstreams, entity subtypes | Universal substrate. Must land before any other group because gate-surface depends on entity identity. |
| **B — KYC/UBO (10)** | ubo_registry, ubo_evidence, investors, kyc.* | KYC pipeline works against entity core. Lands after A. |
| **C — Deals & contracts (6)** | deals, deal_products, legal_contracts, contract_products, booking_principal, booking_location | Deal lifecycle works against entity + client. Lands after A + D. |
| **D — Client/Group (4)** | client_group, client_profile, cbu_entity_roles, cbu_ca_preferences | Client-side grouping tables. Lands after A. |
| **E — Registry/supporting (8)** | attribute_registry, ownership_snapshots, remediation_events, shared_atom_registry, fund_*, dilution_instruments, role_applicable_entity_types | Low-frequency supporting data. Lands anytime after A. |
| **F — Temporal/history (2)** | entity_relationships_history, cbu_entity_roles_history | History tables — append-mostly but occasionally amended. Lands last. |

**6 migrations total.** Each migration:

1. Adds `row_version bigint NOT NULL DEFAULT 0` column.
2. Creates an `UPDATE` trigger that does `NEW.row_version = OLD.row_version + 1`.
3. Backfills existing rows in batches of 10,000 using `FOR UPDATE SKIP LOCKED` where possible.

## 4. Out-of-scope tables (explicitly)

The 354 tables not in §2 are one of:

- **Append-only audit / event logs** — `audit_events`, `intent_events`, `session_events`, `changeset_events`, `outbox_events`. Never UPDATEd in normal flow. Phase 5d `StateGateHash` never gates against them.
- **Lookup / reference tables** — currencies, country_codes, jurisdiction_codes, role_codes. Immutable once seeded. No versioning needed.
- **Derived projections / materialised views** — `v_cbu_derived_values`, `cbu_attr_values` (derived rows). Consumed read-only by gate paths; versioning is on the source entities.
- **Internal infrastructure** — SQL schema metadata tables, migration_log, SeaQL / sqlx internal.

If a verb is later discovered to mutate one of these via a path not captured here, the table is added to §2 and a backfill migration issued — budgeted as "new row" work, not replan.

## 5. Backfill policy (zero-downtime per D2)

Per D2 "per-entity-group batched rollout":

1. **Prepare**: add column with `DEFAULT 0` (non-blocking DDL in Postgres 11+).
2. **Backfill**: UPDATE in batches of 10,000 with explicit commit between batches. Throttle to stay under connection-pool pressure. Monitor `pg_stat_activity` for long-running queries.
3. **Enable trigger**: only after all rows have non-null `row_version`. Trigger takes ~microseconds per UPDATE — no measurable perf impact at current ~650 ops/day verb execution rate.
4. **Verify**: a monitoring query per group asserts `MIN(row_version) >= 0` and no rows with `NULL`.

One group per day during off-hours. Full Stream 2 completes in ~1 week calendar time if sequenced A→D→B→E→C→F.

## 6. Phase 5d entry criterion

Phase 5d (`StateGateHash` in-txn recheck) must confirm the following before kickoff:

- All 42 tables in §2 have `row_version` column + UPDATE trigger.
- All triggers are enabled (not in `DISABLE ALL` state).
- Monitoring dashboard shows the version bump rate matches the UPDATE rate.
- A test harness verifies: (a) mutating a row via SQL bumps version; (b) mutating via a verb bumps version; (c) reading via `SELECT FOR UPDATE` stabilises the version for the duration of the txn.

## 7. Open items

- **`public.outbox` row versioning?** The migration 131 outbox table has `created_at` but no `row_version`. Outbox rows aren't gate-surface (they're drained, not gated), so no D1 remediation needed. Flagged here for explicitness.
- **`sem_reg.*` governance tables row versioning?** Out of scope — SemOS-side metadata, governed through changesets not direct verb mutation.
- **Entity-gateway tables** (in `entity-gateway` crate) — not scanned here. If the gateway stores entity state independently, those tables also need remediation. Flagged for Phase 5d entry-criteria review.

## 8. Gate YAML cross-reference

- Phase 0 gate YAML criterion `row-version-audit-complete` is satisfied by this document.
- Stream 2 migration work (6 migrations) is NOT part of Phase 0 closure but is the next concrete artefact, targeted for completion in parallel with Phases 1–4 per Q1 pre-land.
- Phase 5d gate YAML (to be drafted) will add criterion `row-version-coverage-complete` checking every table in §2.
