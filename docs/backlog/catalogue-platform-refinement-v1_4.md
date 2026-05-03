# Catalogue Platform Refinement — v1.4 (Carrier Completeness + State-Node Green-Switch Model)

> **Status:** DRAFT — Adam peer-review pending.
> **Date:** 2026-04-29.
> **Prior versions:** v1.0 (2026-04-18), v1.1 (2026-04-22), v1.2 (2026-04-23, consolidated authoritative as of 2026-04-26), v1.3 (2026-04-24, SUPERSEDED).
> **v1.4 changes from v1.2 (the authoritative base):** two new principles (P18, P19), one consequential schema field, one new CI gate, and the carrier-completeness migration pack.

---

## v1.4 CHANGES FROM v1.2 — at a glance

| # | Amendment | Type | Severity |
|---|---|---|---|
| V1.4-1 | **P18 — Carrier Completeness** | New principle | P0 |
| V1.4-2 | **P19 — State-node green-switch model** | New principle | P0 |
| V1.4-3 | **`green_when:` on StateDef** | Schema extension | P0 |
| V1.4-4 | **CI gates: `reconcile carrier-completeness` + `reconcile drift-check`** | CI extension | P0 |
| V1.4-5 | **Carrier-completeness migration pack** (S-1, S-3, S-7, S-9, S-12, S-25 + IN_CLEARANCE substates) | Migrations applied | P0 |

---

## P18 — Carrier Completeness

> Schema persists what DAG defines.

The DAG taxonomy is canonical for state-machine semantics: state enumerations, transition rules, guards, dual lifecycles, slot dispatch. The schema's role is to persist the current state of each entity per the DAG's declared state set.

**Invariant**: every DAG-declared stateful entity has a corresponding carrier (table + state column + CHECK constraint or FK) that persists the current state. Carrier completeness is verified by CI: for each DAG state machine, a corresponding schema carrier must exist with state values matching the DAG's enumeration.

**Permitted exceptions**: transitional windows where DAG declarations precede carrier migrations are allowed when scheduled — migration tracked, completed in a bounded window (typically one tranche). Permanent divergence between DAG and carrier is a defect.

**Enforcement**: `cargo x reconcile carrier-completeness` (CI gate, see V1.4-4).

---

## P19 — State-node Green-Switch Model

> Every state node has a switch (green/red); transitions are tests, not commands.

### The model

1. **Every state node has a switch**: green means the entity satisfies the state's entry/transit criteria; red means it doesn't.
2. **Each state declares its `green_when` predicate**: the binary test that flips the switch. It is purely binary — `true` (green) or `false` (red); no fuzzy logic, no partial credit.
3. **Verbs are agents**: a verb runs while the *source* state is green, makes changes to the entity, and the system tests whether the *destination* state's `green_when` is satisfied.
4. **The destination's `green_when` IS the postcondition** of the transition: when it holds, the transition fires; when it doesn't, the entity stays at the source state with a diagnosable reason.
5. **Transitions express possibility, not action**: `from: A, to: B, via: V` means *V can drive A→B if A is green and after V the destination's green_when holds*. The DAG isn't an imperative chain — it's a topology of testable states.

### The recursive shape of `green_when`

Every fact in a `green_when` predicate decomposes into two binary checks:

- **exists**: the entity/row/FK is present.
- **valid**: the entity is in a state from a valid set (which itself can be the entity's own state-machine green_when).

Combined: `obtained = exists AND valid`. A state is green when all its required facts are obtained.

The two failure modes are diagnostically distinct:
- **Missing**: required entity not present (no row, FK null). Runtime can say "you don't have a passport for this UBO yet".
- **Invalid**: entity present but its own state is not in the required valid set. Runtime can say "you have a passport but it failed quality check; attach a new one".

### The recursive structure means parent state machines reference child state machines:

```
cbu.READY_TO_TRADE.green_when:
  every required UBO exists
  AND every required UBO.state in {APPROVED, WAIVED}
  ...
```

The UBO's `APPROVED` state is itself defined by its own green_when — referring to evidence requirements, ownership-percentage facts, etc. — each with their own state machines. The chain bottoms out at primitive facts (a passport row exists; its OCR-quality flag is set).

### Why this matters

- **Diagnosability**: every red state can be unrolled; the runtime walks down the green_when tree to find the missing/invalid facts.
- **Composability**: parent state machines compose by referencing child green sets; no one state machine is a god object.
- **No verb-driven invariant violations**: the verb only fires when the destination's predicate holds; impossible transitions are caught at the test, not the handler.
- **Asymmetry between intent and outcome**: a verb intends to drive a transition, but the *outcome* depends on whether the substrate facts agree. "I clicked approve" doesn't mean "approved"; it means "approve was attempted; outcome depends on green_when".

### What stays as user verbs (non-tollgate transitions)

Transitions that require explicit human judgment — not derivable from substrate facts — remain as user verbs:

- **Reject**: compliance officer's explicit refusal. No predicate makes "rejected" true; it's an act.
- **Waive**: explicit override of a missing requirement. Discretionary judgment.
- **Escalate**: explicit choice to refer up.
- **Override / force**: explicit bypass of the green-switch, with audit trail.

These verbs may still have `precondition:` for guards (e.g., "user must have role X"), but they don't have a `green_when` to drive them — they're discretionary.

### Predicate language

Predicates are free-text English in v1.4 (parsed by humans; runtime-evaluated by handler logic). A v1.5+ evolution may introduce structured fields (`required:` for existence assertions, `valid_states:` for validity assertions) once patterns settle.

The convention for v1.4:

```yaml
green_when: |
  every <required_entity> exists
  AND every <required_entity>.state = <required_state>
  AND no <forbidden_entity> exists with state = <forbidden_state>
  AND <numeric_attribute>.value >= <threshold>
```

Each line is a single binary assertion, AND'd together.

---

## V1.4-3 — Schema extension: `green_when` on `StateDef`

```rust
// rust/crates/dsl-core/src/config/dag.rs:268-279
pub struct StateDef {
    pub id: String,
    #[serde(default)]
    pub entry: bool,
    #[serde(default)]
    pub description: Option<String>,
    /// v1.4 (P19): the green-switch predicate for this state.
    /// Optional: states without a green_when are permissive (no entry test).
    #[serde(default)]
    pub green_when: Option<String>,
}
```

Backward-compatible: existing state declarations without `green_when` deserialize as `None` and are permissive.

---

## V1.4-4 — CI gates

Two new `cargo x reconcile` subcommands, wired into `.github/workflows/catalogue.yml`:

- **`reconcile carrier-completeness`**: for each DAG state machine, verify a carrier (table + state column + CHECK) exists with state values matching the DAG enumeration.
- **`reconcile drift-check`**: for each verb declaration with `transition_args:`, verify the `target_workspace + target_slot` matches the DAG's authoring workspace + slot. For each DAG transition's `via:` string, verify a corresponding verb declaration exists.

Both gates are blocking; failure fails the build.

---

## V1.4-5 — Carrier-completeness migration pack (delivered)

Eight migrations applied 2026-04-29 (decision log D-005 through D-014):

| # | File | Purpose |
|---|---|---|
| 01 | `20260429_carrier_01_cbu_service_consumption.sql` | M-039 carrier (existing table extended with S-15 linkage cols) |
| 02 | `20260429_carrier_02_service_intent_comments.sql` | M-026 vs M-039 semantic distinction (Q2 (a)) |
| 03 | `20260429_carrier_03_deals_operational_status.sql` | M-046 carrier + B1 backfill |
| 04 | `20260429_carrier_04_deals_status_in_clearance.sql` | M-045 9-state CHECK |
| 05 | `20260429_carrier_05_cbus_operational_status_reassert.sql` | M-032 idempotent re-assert |
| 06 | `20260429_carrier_06_deal_slas_status.sql` | M-052 carrier |
| 07 | `20260429_carrier_07_settlement_chain_lifecycle_status.sql` | M-021 carrier |
| 08 | `20260429_carrier_08_deals_in_clearance_substates.sql` | Q21 (b) BAC + KYC parallel substate columns |

Plus DAG amendments:
- M-036 orphan states removed (D-003).
- M-045 collapsed BAC_APPROVAL + KYC_CLEARANCE into IN_CLEARANCE compound state with parallel substates (D-004).
- 9 worked-example `green_when` predicates seeded across 6 DAGs (D-016).
- Phase 4 §4.3 KYC verb-set drift partially reconciled (D-017): 5 new verbs, 7 DAG via renames.

---

## Out of scope for v1.4 (deferred)

- **Broader green_when fill**: ~280 verb-driven transitions across all 12 DAGs need green_when predicates. Each requires per-state input on the binary fact set. Separate engagement.
- **Phase 4 §4.2 reconciliation**: substrate audit's claimed workspace-ownership drift across 9 clusters — NOT actually present in current YAML (already DAG-canonical). No work needed; logged for closure (D-015).
- **Bypass-write fix**: `service_pipeline_service_impl.rs:165` — marked with FIXME, deferred to R.1 (D-002).
- **BookSetup carrier (S-5)**: deferred per Q15.
- **R.1-absorbable carriers** (S-4, S-6, S-10, S-11, S-13, S-14): per substrate audit §7.3.
- **Tier classifications + three-axis declarations**: R.4/R.5 catalogue work.
- **Tranche 3 deliverables**: Catalogue workspace, authorship verbs, Sage/REPL integration, Observatory.
- **Test coverage gaps**: 36 of 61 state machines have zero tests. Surfaced for awareness.

---

## Versioning note

v1.4 supersedes v1.2 as the working spec. v1.0/v1.1/v1.2/v1.3 remain in archive for traceability. R.1 (when it begins) operates against v1.4. Future amendments produce v1.5.
