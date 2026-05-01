# Refactor Inventory — 2026-04-29 Carrier Completeness + Reconciliation + v1.3

> **Phase 1 deliverable** for `refactor-todo-2026-04-29.md`.
> **Date:** 2026-04-29.
> **Workspace root:** `/Users/adamtc007/Developer/ob-poc/rust/`.

---

## 1. Migration sequence

**Naming convention.** `YYYYMMDD_<snake_case_name>.sql`. Multiple migrations on the same date are allowed and ordered alphabetically by the trailing name. Examples in tree:

- `20260402_*`: 4 files (compensation_records, external_call_log, provider_capabilities, remediation_events — *correction:* dates split across `20260402` and `20260403`)
- `20260427_*`: 2 files (catalogue_workspace, lifecycle_resources_workspace)

**Latest migration:** `20260429_booking_principal_clearance.sql` (already on disk).

**Next available filenames** (will sort after the existing 2026-04-29 file):

| # | Proposed filename | TODO ref | Notes |
|---|---|---|---|
| 1 | `20260429_cbu_service_consumption_carrier.sql` | §2.1 | NEW — M-039 carrier |
| 2 | `20260429_service_intent_semantic_comments.sql` | §2.2 | NEW — comment-only |
| 3 | `20260429_deal_status_in_clearance.sql` | §2.3 (post §5.1) | **needs backfill — see Blocker B1** |
| 4 | `20260429_cbus_operational_status.sql` | §2.4 | **Already done — see Blocker B2** |
| 5 | `20260429_deals_operational_status.sql` | §2.5 | NEW — M-046 carrier |
| 6 | `20260429_deal_slas_status_carrier.sql` | §2.6 | NEW — M-052 carrier |
| 7 | `20260429_settlement_chain_lifecycle_status.sql` | §2.7 | NEW — M-021 carrier |
| 8 | `20260429_deals_in_clearance_substates.sql` | §5.2 | NEW — bac_status + kyc_clearance_status |

The directory enforces no explicit `_NNN` suffix; alphabetical-after-date is the ordering convention.

## 2. Verb YAML directory structure

**Root:** `rust/config/verbs/`.

**Layout:** mixed — top-level domain `.yaml` files (one per domain) plus 11 subdirectories that hold further-subdivided domains:

- Top-level: 90 `.yaml` files (e.g., `cbu.yaml`, `deal.yaml`, `kyc.yaml`, `attribute.yaml`, `screening.yaml`, etc.) plus `_meta.yaml`.
- Subdirectories:
  - `admin/`, `custody/`, `kyc/`, `observation/`, `refdata/`, `reference/`, `registry/`, `research/`, `sem-reg/`, `templates/`, `verification/`.

For Phase 4 reconciliation, the verb declarations to amend are scattered across both top-level files and subdirectories. Cluster mapping (preview):

- `registry.investor.*`, `registry.holding.*`, `registry.share-class.*` → `registry/` subdir.
- `screening.*` → `screening.yaml` (top-level).
- `service.*` → `service.yaml`-family (top-level).
- `manco-group.*` / `manco.*` → ownership/manco yaml family.
- `service-consumption.*` → likely a top-level file (TBD when Phase 4 starts).
- `custody.trade-gateway.*`, `custody.settlement-chain.*` → `custody/` subdir.

## 3. DAG taxonomy directory

**Root:** `rust/config/sem_os_seeds/dag_taxonomies/`.

**File count:** 12 (TODO §1 said "twelve files" — confirmed):

| # | File | Workspace |
|---|---|---|
| 1 | `book_setup_dag.yaml` | BookSetup |
| 2 | `booking_principal_dag.yaml` | BookingPrincipal |
| 3 | `catalogue_dag.yaml` | Catalogue |
| 4 | `cbu_dag.yaml` | CBU |
| 5 | `deal_dag.yaml` | Deal |
| 6 | `instrument_matrix_dag.yaml` | InstrumentMatrix |
| 7 | `kyc_dag.yaml` | KYC |
| 8 | `lifecycle_resources_dag.yaml` | LifecycleResources |
| 9 | `onboarding_request_dag.yaml` | OnboardingRequest |
| 10 | `product_service_taxonomy_dag.yaml` | ProductMaintenance |
| 11 | `semos_maintenance_dag.yaml` | SemOsMaintenance |
| 12 | `session_bootstrap_dag.yaml` | SessionBootstrap |

## 4. CI configuration

**File holding the new gate:** `.github/workflows/catalogue.yml` — already wired with three pre-existing checks. Two more checks to add per TODO §8:

Existing in `catalogue.yml`:
- `cargo run --package xtask -- verbs compile`
- `cargo run --package xtask -- reconcile validate`
- `cargo run --package xtask -- verbs atlas --lint-only`
- `cargo test -p ob-poc --lib --features database -- domain_ops::tests::test_plugin_verb_coverage domain_ops::tests::test_no_rust_only_verbs_in_registry`

To add (TODO §8):
- `cargo run --package xtask -- reconcile carrier-completeness` — NEW.
- `cargo run --package xtask -- reconcile drift-check` — NEW.

Trigger paths already include `rust/config/verbs/**`, `rust/config/sem_os_seeds/**`, `rust/xtask/src/**` — so additions to those locations will trip the gate without new path filters.

A second workflow `forward-discipline.yml` exists; not relevant to this refactor.

## 5. Existing carrier-load validation

**Startup gate (binary):** `rust/crates/ob-poc-web/src/main.rs:166-298` — DB-free pre-startup gate.

```rust
// rust/crates/ob-poc-web/src/main.rs:194
let loader_for_validation = ConfigLoader::from_env();
let verbs_config_for_validation = match loader_for_validation.load_verbs() {
    Ok(c) => c,
    Err(e) => return Err(format!("catalogue-load failed pre-DB: {}", e).into()),
};
// + validate_verbs_config + validate_dags + warning + error tally
```

**xtask CLI:** `rust/xtask/src/reconcile.rs::validate()` — same code path (`load_verbs` + `validate_verbs_config` + `validate_dags`). Currently exposes three subcommands: `Validate`, `Status`, `Batch`.

**Validator code:** `rust/crates/dsl-core/src/config/`:
- `validator.rs` — verb-level structural + well-formedness checks.
- `dag_validator.rs` — DAG taxonomy checks.
- `dag_registry.rs` — DagRegistry + cross-workspace state machinery (v1.3).
- `loader.rs` — file-tree loader.

**For Phase 8 we'll extend `reconcile.rs` with two new `ReconcileAction` variants** (`CarrierCompleteness`, `DriftCheck`) and corresponding handler functions; the validators they call live in `dsl-core/src/config/`.

## 6. `deals_status_check` constraint name

**Live name:** `deals_status_check` (default).

**Current state set (15 states, observed live 2026-04-29):**

```
PROSPECT, QUALIFYING, NEGOTIATING, BAC_APPROVAL, KYC_CLEARANCE,
CONTRACTED, ONBOARDING, ACTIVE, SUSPENDED, WINDING_DOWN, OFFBOARDED,
CANCELLED, LOST, REJECTED, WITHDRAWN
```

Source: `pg_constraint.consrc` for `deals_status_check`.

**TODO §2.3 target state set (9 states):**

```
PROSPECT, QUALIFYING, NEGOTIATING, IN_CLEARANCE, CONTRACTED,
LOST, REJECTED, WITHDRAWN, CANCELLED
```

**Delta:**
- Drop: `BAC_APPROVAL`, `KYC_CLEARANCE` (collapse into `IN_CLEARANCE`).
- Drop: `ONBOARDING`, `ACTIVE`, `SUSPENDED`, `WINDING_DOWN`, `OFFBOARDED` (move to new `operational_status` column per §2.5).
- Add: `IN_CLEARANCE`.

`master-schema.sql:10588` has a stale 9-state set without `BAC_APPROVAL`/`KYC_CLEARANCE`/`SUSPENDED`/`LOST`/`REJECTED`/`WITHDRAWN` — that file is out of date relative to live DB.

---

## Live-DB facts (relevant to migration safety)

Captured via `psql -d data_designer` on 2026-04-29.

| Table | Row count | Notes |
|---|---|---|
| `cbus` | 1,794 | Has `status`, `operational_status`, `disposition_status` columns. CHECK `chk_cbu_operational_status` already enforces TODO §2.4's 8-state set. |
| `deals` | 3 | 2 in `ONBOARDING`, 1 in `PROSPECT`. **§2.3 backfill needed for the two ONBOARDING rows.** |
| `deal_slas` | 0 | §2.6 add-column is data-safe. |
| `service_intents` | 0 | §2.2 comment-only is trivially safe. |
| `cbu_settlement_chains` | 1 | §2.7 backfill (`is_active=true → 'live'`) trivially safe. |

---

## Blockers surfaced from Phase 1

### B1 — `deals` data backfill missing from §2.3

The two `ONBOARDING` deal rows would violate the new 9-state CHECK if §2.3 lands as written. Options:

- **(a)** Backfill before §2.3: `UPDATE "ob-poc".deals SET deal_status = 'CONTRACTED', operational_status = 'ONBOARDING' WHERE deal_status = 'ONBOARDING'`. Requires §2.5 (operational_status column) to land first.
- **(b)** Add `ONBOARDING` (and `ACTIVE`/`SUSPENDED`/`WINDING_DOWN`/`OFFBOARDED`) to the new commercial CHECK as transitional states until backfill completes.
- **(c)** Backfill to `CONTRACTED` only, leave operational_status NULL.

Recommendation: **(a)** — smallest, cleanest, matches the architectural separation. Sequence becomes §2.5 (add operational_status column) → backfill → §2.3 (new commercial CHECK).

### B2 — §2.4 already done

`cbus.operational_status` column + `chk_cbu_operational_status` CHECK constraint already exist with the exact 8-state set TODO §2.4 defines. The migration would be a no-op via `IF NOT EXISTS` and `DROP CONSTRAINT IF EXISTS` / `ADD CONSTRAINT`. Ship the file anyway for the comment trailer (`-- Materialises: M-032 …`) or drop §2.4 entirely.

Recommendation: ship the migration as a **comment-only / annotation migration** — re-asserting the existing CHECK and adding the M-032 audit trailer comment. Idempotent, traceable.

### B3 — v1.3 spec versioning conflict

`docs/todo/catalogue-platform-refinement-v1_3.md` already exists (52KB, dated 2026-04-24, marked SUPERSEDED on 2026-04-26 when v1.2 absorbed its amendments). Its existing `P16` is about three-layer state stratification — different from TODO §7's proposed `P16 = Carrier Completeness`.

Per the project's CLAUDE.md: "Catalogue spec: `catalogue-platform-refinement-v1_2.md` (consolidated authoritative spec, 2026-04-26 — supersedes v1.0/v1.1/v1.3)."

Options:

- **(a)** Bump the carrier-completeness amendment to **v1.4** — adds new file `catalogue-platform-refinement-v1_4.md`, leaves v1.3 untouched (preserves the SUPERSEDED archival state). Cleanest archival; explicit version bump.
- **(b)** Repurpose existing v1.3 file — strip SUPERSEDED banner, replace P16 content. Loses the prior superseded snapshot.
- **(c)** Write `catalogue-platform-refinement-v1_3-amendment.md` as a delta — unusual for this repo's pattern.

Recommendation: **(a) v1.4** — preserves the prior version history and matches the doc's existing semver-like cadence (v1.0 → v1.1 → v1.2 → v1.3 → v1.4).

### B4 — Phase 2 / Phase 5 ordering

TODO §2.3 says "depends on §5 producing the substate model first." Need to execute **§5.1 (DAG amendment for IN_CLEARANCE) before §2.3 (deals_status_check schema migration)**. The natural section ordering in the doc is misleading.

Proposed actual sequence:

1. §1 inventory (this doc).
2. §6 deferral marker (independent, trivial).
3. §3 M-036 orphan removal (independent, trivial).
4. §5.1 IN_CLEARANCE DAG amendment.
5. §2.1 cbu_service_consumption.
6. §2.2 service_intent comments.
7. §2.5 deals.operational_status.
8. **B1 backfill** for ONBOARDING deals.
9. §2.3 deals_status_check (new 9-state CHECK).
10. §2.4 cbus.operational_status (re-assert / no-op annotation).
11. §2.6 deal_slas.sla_status.
12. §2.7 cbu_settlement_chains.lifecycle_status.
13. §5.2 deals.bac_status / kyc_clearance_status.
14. §5.3 verb amendments for IN_CLEARANCE.
15. §4 reconciliation (clusters 1–9 + KYC families).
16. §7 v1.4 spec amendment (assuming B3 (a)).
17. §8 CI gate.
18. §9 final verification + summary.

---

**End of inventory.**
