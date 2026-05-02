# DAG Reachability Remediation Plan

> **Companion to:** `docs/governance/dag-reachability-audit-2026-05-02.md`. Read the audit first.
> **Status:** Plan only. No code/DSL/DB changes have been implemented.
> **Authorship:** Drafted 2026-05-02. Awaiting peer review and Adam sign-off on D1-D5.
> **Each slice below is independently shippable** — they can be sequenced in priority order without cross-dependencies.

---

## Goal

Close the gaps surfaced by the 2026-05-02 reachability audit so that:

- Every state declared in the four audited DAGs (Deal, CBU, InstrumentMatrix, LifecycleResources) is either reachable through a direct verb OR explicitly documented as backend-/trigger-/cascade-only.
- No SimpleStatusOp registration targets an enum value that the carrier table's CHECK constraint rejects.
- Substate gating (deal `bac_status` / `kyc_clearance_status`) has direct-verb writers for every enum value the DAG references in preconditions.
- Plugin side-effects that produce DAG-relevant state mutations are either promoted to first-class verbs or explicitly documented as side-effect-only.

This document enumerates the executable changes — **code (Rust), DSL (verb YAML + DAG YAML), and DB (migrations)** — that close each gap. It does not attempt to choose between the design alternatives; those are listed at the bottom of the audit (D1-D5) and need Adam sign-off before implementation begins.

---

## Concern Being Addressed

The reachability audit found that the four primary onboarding flows have a mix of:

1. **Production-blocking bugs** — two SimpleStatusOp deal verbs target enum values that no longer exist after the D-004 IN_CLEARANCE collapse. They will violate the `deals_status_check` constraint at runtime.
2. **Substate gating gaps** — the DAG's `IN_CLEARANCE → CONTRACTED` precondition reads `bac_status='approved' AND kyc_clearance_status='approved'`, but no verb writes `bac_status='approved'`. Direct-verb deal flow is stuck at IN_CLEARANCE indefinitely.
3. **Plugin side-effect blind spots** — child-lifecycle entry states (trading_profile DRAFT, service_intent active) and the IM template→instance clone happen as side-effects of parent verbs with no FQN to bind to. The DAG cannot model them and direct utterances cannot drive them.
4. **Side-effect-only command verbs** — the entire 16-verb service-pipeline cluster (discovery, provisioning, readiness) emits no state transitions. Discovery progress is invisible to the DAG.
5. **Backend/trigger transitions undocumented as such** — some are intentional (CBU `archived`, application_instance `DEGRADED`), some are gaps (`cbu_lifecycle_instances.DECOMMISSIONED` has no verb at all).

The remediation has to land each fix in the right plane — code for executor/op changes, DSL for verb authoring, DB for migrations — and keep the canonical `SemOsVerbOpRegistry` + DAG YAML in sync.

---

## Slice P0-A — Fix the broken deal SimpleStatusOp registrations

**Audit refs:** F-1, §3.1.

### Problem

`rust/src/domain_ops/simple_status_op.rs:271-296` registers two verbs that write states the DB no longer accepts:

```rust
SimpleStatusConfig {
    fqn: "deal.submit-for-bac",
    target_state: "BAC_APPROVAL",   // not in deals_status_check
    state_col: "deal_status",
    ...
},
SimpleStatusConfig {
    fqn: "deal.bac-approve",
    target_state: "KYC_CLEARANCE",  // not in deals_status_check
    state_col: "deal_status",
    ...
},
```

Both states were collapsed into `IN_CLEARANCE` by `rust/migrations/20260429_carrier_08_deals_in_clearance_substates.sql`. Calling either verb crashes at the DB.

### Code

1. **Remove** both `SimpleStatusConfig` entries from `STATUS_FLIP_VERBS` in `rust/src/domain_ops/simple_status_op.rs`.
2. **Add** dedicated `SemOsVerbOp` impls for `deal.submit-for-bac` and `deal.bac-approve` in `rust/crates/sem_os_postgres/src/ops/deal.rs`. Both must:
   - Validate the deal is currently in `IN_CLEARANCE` (or, for `submit-for-bac`, in `NEGOTIATING` and transition `deal_status` to `IN_CLEARANCE` AND seed `bac_status='in_review'` AND `kyc_clearance_status='pending'` in a single atomic write).
   - Emit `PendingStateAdvance` to `deal:in_clearance` (for submit-for-bac) or to a substate-aware DAG node (for bac-approve — see slice P0-B).
3. **Register** the new ops in `sem_os_postgres::ops::build_registry()` (`rust/crates/sem_os_postgres/src/ops/mod.rs`). Remove from `STUB_VERBS` if present.

### DSL

Update `rust/config/verbs/deal.yaml` for both verbs:

- `transition_args.target_state: IN_CLEARANCE` (or remove `target_state` and rely on `target_workspace + target_slot` if substate-aware verbs declare a different shape).
- Confirm `behavior: plugin` (not `crud`), since the substate write needs custom logic.
- Description must reflect the substate semantics: "writes deal_status=IN_CLEARANCE AND seeds bac_status='in_review' / kyc_clearance_status='pending' atomically" for submit-for-bac.

### DB

No migration — schema already supports the substates. **Verify** there are no in-flight `deal.submit-for-bac` calls in the wild that depend on the old behaviour (search audit logs / outbox for the FQNs).

### Files affected

- `rust/src/domain_ops/simple_status_op.rs` (delete 2 entries)
- `rust/crates/sem_os_postgres/src/ops/deal.rs` (add 2 ops + result types)
- `rust/crates/sem_os_postgres/src/ops/mod.rs` (register both)
- `rust/config/verbs/deal.yaml` (update 2 verb definitions)

### Verification

- `cargo test -p ob-poc --lib -- domain_ops::tests::test_plugin_verb_coverage` — green.
- `cargo run -p xtask -- verbs lint` — green.
- New unit test in `db_integration.rs`: end-to-end `deal.create → deal.update-status (NEGOTIATING) → deal.submit-for-bac → SELECT deal_status, bac_status, kyc_clearance_status FROM deals WHERE deal_id=...` proves the substate seed.
- New unit test: invoking the old behaviour (writing `BAC_APPROVAL` to `deal_status`) is rejected by the constraint.

---

## Slice P0-B — Add direct-verb writers for the deal substates

**Audit refs:** F-2, §3.2. **Depends on:** P0-A (since `bac-approve` is being rewritten anyway).

### Problem

The DAG precondition `IN_CLEARANCE → CONTRACTED` requires `bac_status='approved' AND kyc_clearance_status='approved'`. Of the 8 substate enum values, only `kyc_clearance_status='approved'` and `kyc_clearance_status='rejected'` have a direct-verb writer (`deal.update-kyc-clearance`). No verb writes `bac_status` to any value.

### Code

Add four `SemOsVerbOp` impls in `rust/crates/sem_os_postgres/src/ops/deal.rs`:

| FQN | What it writes |
|-----|---------------|
| `deal.bac-mark-in-review` | `deals.bac_status = 'in_review'` (preserving on `deal_status`) |
| `deal.bac-approve` | `deals.bac_status = 'approved'` (preserving). Replaces the slice-P0-A version which also handles the deal_status side. |
| `deal.bac-reject` | `deals.bac_status = 'rejected'`. Cascades to `deal_status='REJECTED'` only if business policy requires terminal cascade — confirm with Adam. |
| `deal.kyc-mark-in-review` | `deals.kyc_clearance_status = 'in_review'` (preserving) |

Each is a small preserving update — could be a `SimpleStatusOp` but only if `SimpleStatusConfig` is extended to support **column-only writes** (no `deal_status` change). Today `SimpleStatusOp` always writes the named `state_col`; we can either:
- (a) Extend `SimpleStatusConfig` with `preserves_other_columns: bool` flag — not very clean.
- (b) Just write dedicated SemOsVerbOp impls — preferred. ~30 LOC each.

### DSL

Add 4 verbs to `rust/config/verbs/deal.yaml`:

- Same arg shape as `deal.update-kyc-clearance`: `deal-id (uuid, required)`, optional `notes`, optional `decided-by`.
- `behavior: plugin`. `returns.type: affected`.
- `metadata.flavour: attribute_mutating` (not `transition`, since `deal_status` is unchanged).
- `transition_args` not required (preserving update). Or use a substate-aware DAG node id like `deal:in_clearance:bac_approved`.

### DB

No migration — substates and constraints already exist (D-004 migration).

### Files affected

- `rust/crates/sem_os_postgres/src/ops/deal.rs` (4 ops)
- `rust/crates/sem_os_postgres/src/ops/mod.rs` (4 registrations)
- `rust/config/verbs/deal.yaml` (4 new verb declarations)

### Verification

- `cargo test -p ob-poc --lib -- domain_ops::tests::test_plugin_verb_coverage` green.
- New end-to-end integration test: drive deal from PROSPECT through to CONTRACTED using only direct verbs (no macros), proving the gating chain is now expressible. Specifically: `deal.create → deal.update-status NEGOTIATING → deal.submit-for-bac → deal.bac-mark-in-review → deal.bac-approve → deal.kyc-mark-in-review → deal.update-kyc-clearance approved → deal.update-status CONTRACTED`.

---

## Slice P1-A — Make trading-profile DRAFT and service_intent active reachable

**Audit refs:** F-3, §5.1. **Decision required:** D2(a) vs D2(b).

### Problem

Two child-lifecycle entry states are written only as side-effects:
- `trading_profile.DRAFT` — created by `cbu.add-product` (plugin) or `trading-profile.create-draft` (plugin, not in `SimpleStatusOp`). The latter exists as a verb but has no `transition_args.target_state`, so it's invisible to the DAG.
- `service_intent.active` — `cbu.add-product` inserts the row in `active`. No verb writes `active`.

### If D2(a) — promote to first-class verbs

#### Code

1. Confirm `sem_os_postgres::ops::trading_profile_ca::CreateDraft` (or wherever `trading-profile.create-draft` lives) exists. If not, add it. Currently the code path goes through `cbu::add-product`'s plugin body.
2. Add `service-intent.activate` SemOsVerbOp that writes `cbu_service_intent.status = 'active'` for a given `intent-id`. Likely fits `SimpleStatusOp` (single-column flip).

#### DSL

- `rust/config/verbs/trading-profile.yaml` — add `transition_args.target_state: DRAFT` to `trading-profile.create-draft`. Already a verb, just missing the DAG link.
- `rust/config/verbs/service-intent.yaml` — add `service-intent.activate` declaration if missing.

#### DB

None.

### If D2(b) — document as side-effect-only

#### DSL

Add a top-of-file comment in `instrument_matrix_dag.yaml` for the trading_profile and service_intent slots: "DRAFT/active entry state is plugin-driven via `cbu.add-product`; no direct verb. See `dag-reachability-audit-2026-05-02.md` §5."

Add a new field `entry_via: plugin` (or similar) on the slot's state-machine spec so `xtask reconcile validate` can recognise the documented exception and not flag it as unreachable.

#### Code

Extend `xtask reconcile` to skip "unreachable" warnings for states with `entry_via: plugin` declared.

### Files affected (D2(a) path)

- `rust/crates/sem_os_postgres/src/ops/trading_profile_ca.rs` (or wherever) — confirm/add `CreateDraft`
- `rust/src/domain_ops/simple_status_op.rs` — add `service-intent.activate`
- `rust/config/verbs/trading-profile.yaml` — add `transition_args` to existing verb
- `rust/config/verbs/service-intent.yaml` — add new verb

### Verification

- `xtask reconcile validate` shows DRAFT and active as reachable (or as documented exceptions).
- New integration test: invoke `trading-profile.create-draft` directly and verify a row appears with `status='DRAFT'`.

---

## Slice P1-B — Decide and act on the IM template→instance clone

**Audit refs:** F-4, §5.2. **Decision required:** D2.

### Problem

The IM has explicit two-stage instances (`cbu_trading_profiles.cbu_id IS NULL` for templates, `IS NOT NULL` for cloned instances). The clone happens inside `cbu.add-product` as a side-effect. No FQN binds to the clone step from a direct utterance.

### If D2(a) — add `trading-profile.clone-from-template` verb

#### Code

Add `SemOsVerbOp` in `rust/crates/sem_os_postgres/src/ops/trading_profile_ca.rs`:
- Args: `template-profile-id (uuid)`, `target-cbu-id (uuid)`.
- Validates template has `cbu_id IS NULL`.
- Inserts a new row with `cbu_id = target-cbu-id`, copying all template fields, status `DRAFT`.
- Emits `PendingStateAdvance trading-profile:draft / instrument-matrix/trading-profile`.

#### DSL

`rust/config/verbs/trading-profile.yaml` — declare `trading-profile.clone-from-template` with `behavior: plugin`, `returns.type: uuid` (the new instance id), `transition_args.target_state: DRAFT`.

#### DB

None.

#### Refactor `cbu.add-product`

Once the clone verb exists, refactor `cbu.add-product` to invoke it via the SemOsVerbOpRegistry instead of duplicating the clone SQL. Optional but recommended for single-source-of-truth.

### If D2(b) — document as derived projection

Add a comment to `instrument_matrix_dag.yaml` for the trading_profile slot's two-stage model: "Clone is a derived side-effect of `cbu.add-product`; not a verb. State machine sees the cloned row at DRAFT; the clone itself is invisible."

### Files affected (D2(a) path)

- `rust/crates/sem_os_postgres/src/ops/trading_profile_ca.rs` — add `CloneFromTemplate`
- `rust/crates/sem_os_postgres/src/ops/mod.rs` — register
- `rust/config/verbs/trading-profile.yaml` — declare verb
- `rust/crates/sem_os_postgres/src/ops/cbu.rs` — `add-product` op refactored to call clone via registry
- New integration test in `db_integration.rs`

---

## Slice P1-C — Discovery pipeline: state visibility

**Audit refs:** F-5, §6.2. **Decision required:** D3.

### Problem

The 16 service-pipeline verbs (`discovery.run`, `discovery.explain`, `attributes.{rollup,populate}`, `provisioning.run`, `readiness.compute`, `pipeline.full`, etc.) execute as plugin ops with NO `transition_args`. Discovery progress is invisible to the DAG.

### If D3(a) — add discovery_state slot

#### DB

New migration `rust/migrations/20260503_discovery_state_slot.sql`:

```sql
ALTER TABLE "ob-poc".cbus
  ADD COLUMN IF NOT EXISTS discovery_state text;

ALTER TABLE "ob-poc".cbus
  ADD CONSTRAINT cbus_discovery_state_check CHECK (
    discovery_state IS NULL
    OR discovery_state IN ('PENDING', 'EXECUTING', 'COMPLETE', 'FAILED')
  );
```

#### DSL

Add a `discovery_state` slot to `lifecycle_resources_dag.yaml` (or to the CBU dag if scoped per-CBU rather than per-resource):

- States: PENDING, EXECUTING, COMPLETE, FAILED
- Transitions: PENDING → EXECUTING (via `discovery.run`), EXECUTING → COMPLETE / FAILED (via discovery completion side-effect or via `discovery.mark-complete` / `discovery.mark-failed` verbs)

#### Code

Update `discovery.run` op (in `service_pipeline.rs` dispatch or extract into a dedicated `sem_os_postgres::ops::discovery::Run`):
- On entry: write `discovery_state='EXECUTING'`.
- On success: write `discovery_state='COMPLETE'`.
- On failure: write `discovery_state='FAILED'` (in a separate transaction or via a defer).

Add `transition_args.target_state` declarations to `discovery.run` in `rust/config/verbs/discovery.yaml`.

### If D3(b) — keep observability row-count based

#### DSL

Add `outputs:` declarations to all 16 service-pipeline verbs in their YAML so the response Records include structured progress info (e.g. `discovery.run` outputs `{srdef_count, intent_count, status: "complete|partial|failed"}`). The DAG remains state-machine-blind, but downstream consumers can read structured outputs.

### Files affected (D3(a) path)

- `rust/migrations/20260503_discovery_state_slot.sql` (new)
- `rust/config/sem_os_seeds/dag_taxonomies/lifecycle_resources_dag.yaml`
- `rust/config/verbs/discovery.yaml`
- `rust/crates/sem_os_postgres/src/ops/discovery.rs` (or wherever discovery.run lives)

---

## Slice P1-D — Add `service-resource.decommission` verb

**Audit refs:** F-6, §6.1. **Decision required:** D5(a) vs D5(b).

### Problem

`cbu_lifecycle_instances.status` enum allows DECOMMISSIONED but no verb writes it. Only `service-resource.reactivate` is registered.

### If D5(a) — add the verb

#### Code

Add to `rust/src/domain_ops/simple_status_op.rs::STATUS_FLIP_VERBS`:

```rust
SimpleStatusConfig {
    fqn: "service-resource.decommission",
    table: "cbu_lifecycle_instances",
    pk_col: "instance_id",
    state_col: "status",
    target_state: "DECOMMISSIONED",
    entity_arg: "instance-id",
    timestamp: Some(TimestampColumn { column: "decommissioned_at" }),
},
```

(Verify a `decommissioned_at` column exists; if not, the timestamp goes via `updated_at` only.)

#### DSL

`rust/config/verbs/service-resource.yaml` — declare `service-resource.decommission` with `behavior: plugin`, `transition_args.target_state: DECOMMISSIONED`.

#### DB

No migration unless `decommissioned_at` is wanted (small optional migration).

### If D5(b) — drop DECOMMISSIONED from the enum

#### DB

New migration removing DECOMMISSIONED from `cbu_lifecycle_instances_status_check`. **Risk:** any existing rows in that state become invalid; need a backfill query first.

---

## Slice P2-A — Add `application-instance.mark-degraded` (or document)

**Audit refs:** F-6, §6.1.

### Problem

DEGRADED is declared as a state but specified as `via "(backend: health-check signal)"`. No verb. If a developer wants to mark an instance as DEGRADED for testing/manual ops, there's no path.

### Option A — add the verb

Trivial: add to `STATUS_FLIP_VERBS` and declare in `rust/config/verbs/application-instance.yaml`. Same pattern as Slice P1-D.

### Option B — document as backend-only

Add an `entry_via: backend_signal` annotation on the DAG state and update the validator to recognise it.

---

## Slice P2-B — Decide on `cbu.decide REFERRED`

**Audit refs:** F-7, §4.2. **Decision required:** D4.

### If D4(a) — add REFERRED to the DAG

#### DSL

Add REFERRED state to `cbu_dag.yaml` primary lifecycle:
- States: append `referred`
- Transitions: `validation_pending → referred via cbu.decide`
- Outbound: `referred → validation_pending via cbu.reopen-validation` (or similar)

#### DB

Verify `cbus` has a `status` column with constraint allowing REFERRED. If not, ALTER the constraint.

#### Code

`cbu.decide` op (in `cbu.rs`) — when `decision = 'REFERRED'`, write the new status. Currently this branch is audit-only.

### If D4(b) — document REFERRED as audit-only

Update `cbu.decide` description in the YAML to state "REFERRED is recorded in the audit trail and emits an event but does not change cbus.status. See `dag-reachability-audit-2026-05-02.md` §4.2."

---

## Sequencing

| Slice | Priority | Depends on | Effort | Risk |
|-------|----------|------------|--------|------|
| P0-A — fix broken deal SimpleStatusOp | P0 | none | M (2 new ops, refactor 2 registrations, tests) | Low — schema already supports |
| P0-B — substate writers | P0 | P0-A | M (4 new ops, tests) | Low |
| P1-A — DRAFT / active entry verbs | P1 | D2 decision | S | Low |
| P1-B — clone-from-template verb | P1 | D2 decision | M (new op + cbu.add-product refactor) | Med (refactor risk) |
| P1-C — discovery state visibility | P1 | D3 decision | M (migration + DAG + 16 verbs touched if outputs path; smaller if state-slot path) | Med |
| P1-D — service-resource.decommission | P1 | D5 decision | XS | Low |
| P2-A — application-instance.mark-degraded | P2 | none | XS | Low |
| P2-B — cbu.decide REFERRED | P2 | D4 decision | XS-S | Low |

P0-A and P0-B should ship together as a single PR (they share the deal.rs file and the substate-aware verb model). P1-A through P2-B are independent slices.

---

## Verification gate (applies to every slice)

Before merge:

1. `cargo run -p xtask -- verbs lint` — 0 errors.
2. `cargo run -p xtask -- reconcile validate` — 0 errors / 0 warnings.
3. `cargo test -p ob-poc --lib -- domain_ops::tests` — all 4 invariant tests pass (registry coverage in both directions).
4. `cargo clippy --workspace --all-targets -- -D warnings` — clean.
5. `DATABASE_URL=... cargo run -p xtask -- check --db` — no new failures (24/29 → at minimum 24/29; ideally improves).
6. New per-slice integration test landed alongside the change, demonstrating the now-reachable state is hit by the new verb.
7. **Reachability re-audit** (Adam-run): re-run the matrix from `dag-reachability-audit-2026-05-02.md` for the affected flow and confirm the formerly-unreachable states are now ✓.

---

## Out of scope for this remediation plan

- Macro audit (Adam doing separately).
- Cross-workspace constraint completeness — covered by `dag-coherence-review-2026-04-26.md`.
- Test infrastructure: the 5 remaining `db_integration` failures (entity-create polymorphic arg mapping) — separate slice.
- Outbox / event emission alignment for the new verbs — should follow existing patterns (see `helpers::emit_pending_state_advance` calls in existing kyc_case ops).
- Performance review of the 4 new substate writer verbs (P0-B) — they're single-row UPDATEs, expect to be cheap.
