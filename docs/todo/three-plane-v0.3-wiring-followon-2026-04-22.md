# Three-Plane v0.3 Wiring Follow-On Plan (2026-04-22)

**Prerequisite:** `three-plane-correction-slice-plan-2026-04-22.md` complete.
**Source review:** `docs/todo/three-plane-implementation-peer-review-2026-04-22.md` findings F5-F10.
**Spec:** `docs/todo/three-plane-architecture-v0.3.md` §§8.4, 10.3, 10.5, 10.7, 17.

## Phase-by-phase status (updated 2026-04-22 late-session, post-rollout)

| Phase | v0.3 mapping | State at end of session | Commits this session |
|-------|--------------|-------------------------|---------------------|
| **A** — envelope wiring (F5) | 5c-wire | **COMPLETE** (A.1-A.4). Shadow envelope + trace_id threading + real BLAKE3 StateGateHash all shipped. Placeholder fields tagged `<phase_a_todo>` for future concretization (DagNodeId, WorkspaceSnapshotId) but hash is deterministic and cross-run byte-identical. | 44585376, 9a1f76c6, 0b60d9df, d57a91be |
| **B** — single-dispatch (F6) | 5c-migrate final | **PRIMITIVES COMPLETE** (B.1, B.2a). `dispatch_plugin_via_sem_os_op_in_scope`, `execute_verb_in_scope`, `execute_plan_atomic_in_scope` all exist, tested, `#[allow(dead_code)]`-gated until B.2b migrates the Sequencer. **B.2b remaining: 1-2 weeks dedicated** to hoist Sequencer outer scope. | 00033390, bf49dd2b, c3942912 |
| **C** — PendingStateAdvance (F7) | 5c-migrate state-advance | **PATTERN COMPLETE + ROLLOUT SUBSTANTIVELY COMPLETE: 72 verbs emitting across 15 domains.** Shared `emit_pending_state_advance` + `emit_pending_state_advance_batch` (multi-entity fan-out) + `peek/take_pending_state_advance` accessors ready. Families with full coverage: cbu, entity, kyc_case, deal, screening, client_group, investor (11), document, tollgate, cbu_role, billing, capital, ownership, partnership, outreach, custody, remediation, trading_profile_ca (10 via save wrapper). Remaining verbs = edge cases (cron ticks, recompute sweeps, cache-only mutations) that need case-by-case evaluation, not mechanical rollout. **Apply-in-txn (C.2 main) needs B.2b first.** | 12cc4dc1, e632270a, 2d174449, 607a3b80, 0fdeab11, ea612b31, 4ed2dd65, 1f5f235e, b6c697b8, 7429f46d, 5adc441f |
| **D** — TOCTOU + row-version (F8, F9) | 5d | **MIGRATION STAGED** (D.1). SQL ready at `rust/migrations/20260422_row_version_entity_tables.sql`; NOT yet applied (needs operator approval). D.2 backfill + D.3 recheck require D.1 applied first. | 91aea971 |
| **E** — dual-schema YAML (F10) | Phase 6 | **NOT STARTED.** 4-8 week effort per original plan: new `round-trip-harness` crate + YAML schema extensions + per-verb effect-equivalence proofs. | — |
| **F** — Pattern B A1 (F11) | 5f | **GUARDRAIL + 3 LEDGER ROWS CLOSED.** L4 lint enforces no new A1 violations; current grandfathered-hits = **zero**. Closed: §2.1 MaintenanceReindexEmbeddings, §2.2 ActivateTeaching, §3.1 BpmnSignal, §3.1 BpmnCancel (verb + consumer + drainer registration). Open: §3.1 compile/start/inspect; §3.2-§3.4 source_loader/gleif/request (helper-indirect, needs Phase F.4 call-graph lint). | 66b6398b, d57a91be, 0cc4c834, 3760c60f, 94fc11b6 |

## Scope

This plan wires the v0.3 destination-state invariants that are out-of-scope for the main
correction plan. Each phase here corresponds to a named v0.3 phase:

- Phase A (F5) → v0.3 Phase 5c-wire (envelope construction)
- Phase B (F6) → v0.3 Phase 5c-migrate final (single dispatch site)
- Phase C (F7) → v0.3 Phase 5c-migrate state-advance pipeline
- Phase D (F8, F9) → v0.3 Phase 5d (TOCTOU + row-versioning)
- Phase E (F10) → v0.3 Phase 6 (dual-schema YAML + CRUD dissolution)
- Phase F (F11) → v0.3 Phase 5f (Pattern B A1 remediation)

Each phase is a multi-slice effort with its own gate; phases can ship independently but must ship in
order within a phase.

**Cadence:** medium slices. Where a phase has heavy fan-out (B, D) it extends to R-sweep per-op or
per-table.

## What genuinely blocks v0.3 §17 Definition-of-Done

After the 2026-04-22 session, three distinct work items remain and none can honestly land in a day:

1. **Phase B.2b — Sequencer outer-scope migration.** Touches `execute_runbook_from`, `execute_runbook_with_pool`, the runbook executor's per-step txn management, and the downstream WorkflowDispatcher. ~1-2 week dedicated work. Blocks C.2 main (apply-in-txn) and D.3 (TOCTOU recheck inside txn).

2. **Phase D.2 + D.3 — row-version backfill under live traffic + StateGateHash recheck.** Migration 20260422_row_version_entity_tables.sql ready; application + zero-downtime backfill for the 5 entity tables (cbu, entities, kyc_cases, deals, client_groups) takes 2-3 weeks. Blocks the real TOCTOU guarantee.

3. **Phase E — dual-schema YAML + round-trip harness.** New crate (`round-trip-harness/`) + YAML schema extensions + per-verb effect-equivalence proofs (N ≥ 50 fixtures per dissolved CRUD verb). Blocks CRUD dissolution, which is 50-60% of the 625-op surface. 4-8 weeks.

These three are the genuine remaining work. Everything else in the plan is either mechanical rollout (C.3 remainder, F.1 inspect/compile/start) or documented-and-tracked (F.2-F.4 Pattern B remediation for source_loader/gleif/request).

---

---

## Phase A — Envelope wiring (F5)

**Prerequisite status:** all envelope types exist in `rust/crates/ob-poc-types/src/gated_envelope.rs`.
Only missing piece is construction + propagation.

### Slice A.1 — Construct `GatedVerbEnvelope` at Sequencer stage 6

**Change:** in `ReplOrchestratorV2::process()` (or the Sequencer stage-6 helper), after verb
selection and gate-decision, build a `GatedVerbEnvelope` carrying:

- `verb`, `dag_position`, `resolved_entities`, `args`
- `authorisation: AuthorisationProof { issued_at, session_scope, state_gate_hash, recheck_required }`
- `discovery_signals` (phrase bank entry, narration hint)
- `closed_loop_marker` (`writes_since_push` at gate time)

`trace_id` and `catalogue_snapshot_id` are Slice A.2 work.

**Files:** `rust/src/repl/orchestrator_v2.rs`, `rust/src/sequencer_stages.rs`.

**LOC:** 200-400.

**Test plan:**

1. Unit test that stage-6 produces a `GatedVerbEnvelope` with correct fields populated from
   resolution + gate decision.
2. Envelope-shape property test: given a fixed session state + utterance, the envelope is
   byte-identical on two runs (determinism obligation).

**Acceptance:** every verb invocation in the orchestrator produces an envelope; stage-6 helper
(`GateDecisionOutput::from_envelope`) consumes it.

### Slice A.2 — Propagate `trace_id` + `catalogue_snapshot_id` across stages

**Change:** `trace_id` generated at stage 1 (utterance receipt); threaded through every stage output
type; `catalogue_snapshot_id` read from `sem_reg` at stage 4 (surface disclosure) and stamped on the
envelope.

**Files:** `rust/src/sequencer_stages.rs`, `rust/src/repl/orchestrator_v2.rs`, `rust/crates/sem_os_core/src/catalogue.rs`.

**LOC:** 200-300.

**Test plan:** trace-id search test — one utterance → grep all log lines by trace_id → expect
complete stage timeline.

**Acceptance:** every log event emitted by the sequencer carries a `trace_id` field; catalogue
reload invalidates in-flight envelopes with a structured error (not silent drift).

### Slice A.3 — Wire envelope into stage 8 dispatch

**Change:** `DslExecutor::execute_verb` signature augmented to accept `&GatedVerbEnvelope` alongside
(or instead of) raw args. Executor validates `envelope_version` on receipt; fails with
`DispatchError::UnknownEnvelopeVersion` if mismatch.

**Files:** `rust/src/dsl_v2/executor.rs`, every `SemOsVerbOp::execute` signature if we decide to
pass the envelope through (v0.3 §8.3).

**LOC:** 500-800.

**Test plan:** envelope-version mismatch test; trace_id round-trip from stage 6 through stage 8.

**Acceptance:** envelope is the sole value crossing the bi-plane boundary; stage 6 → stage 8
carries nothing else.

---

## Phase B — Single-dispatch-site invariant (F6)

### Slice B.1 — Move txn ownership from `DslExecutor` to Sequencer

**Change:** the Sequencer opens a transaction at the start of stage 8 inner loop; passes a
`PgTransactionScope` handle into each dispatch; commits at stage 9a. `DslExecutor::execute_verb`
no longer opens its own `PgTransactionScope`.

**Files:** `rust/src/sequencer_stages.rs`, `rust/src/dsl_v2/executor.rs`,
`rust/src/dsl_v2/executor.rs:1503` (the `PgTransactionScope` open site).

**LOC:** 400-600.

**Test plan:**

1. Rollback-on-stage-9a-fail: a runbook step fails during state-advance application → outer
   transaction rolls back; verb writes are not committed.
2. Multi-step runbook atomicity: step 3 of a 5-step runbook fails → steps 1-2 also roll back.

**Acceptance:** multi-step runbooks commit atomically or not at all (§10.7 durability invariant).

### Slice B.2 — Migrate every plugin dispatch through `sequencer::stage_8::dispatch_envelope`

**Change:** every non-test caller of `VerbExecutionPort::execute_json` routes through a single
function in the Sequencer. R-sweep if needed — enumerate all call sites and migrate one-by-one.

**LOC:** 200-400 (most of the fan-out is in B.1).

**Test plan:** grep-level lint: `rg 'VerbExecutionPort::execute_json' rust/src/` returns exactly one
non-test caller.

**Acceptance:** L2 workspace lint can be enabled — `deny(multiple_dispatch_sites)`.

### Slice B.3 — Enable L2 lint

**Change:** add the custom workspace lint per v0.3 Appendix B.L2. Either a clippy plugin, a
cargo-deny rule, or a pre-commit grep assertion.

**Files:** workspace lint config.

**LOC:** ~50.

**Acceptance:** CI fails if a second dispatch site is introduced.

---

## Phase C — `PendingStateAdvance` pipeline (F7)

### Slice C.1 — Pilot: one verb returns non-empty `PendingStateAdvance`

**Change:** pick a high-value verb (suggestion: `cbu.create` or `kyc-case.open`) — populate
`pending_state_advance.state_transitions`, `constellation_marks`, `writes_since_push_delta`.

**Files:** the picked verb's op file in `rust/crates/sem_os_postgres/src/ops/` or
`rust/src/domain_ops/`.

**LOC:** ~100.

**Test plan:** unit test that the verb's outcome contains populated state-advance fields; trace
test that stage 9a applies the advance.

### Slice C.2 — Apply `PendingStateAdvance` in stage 9a

**Change:** in the Sequencer, between dispatch-return and commit, invoke
`SemOsContextResolver::apply_state_advance(scope, pending)` to mutate SemOS state inside the same
transaction.

**Files:** `rust/src/sequencer_stages.rs`, `rust/crates/sem_os_postgres/src/state_advance.rs` (new).

**LOC:** 300-500.

**Test plan:** end-to-end test: verb execution + state-advance commit atomically; rollback on any
error removes both the verb writes AND the state mutation.

### Slice C.3 — Rollout to remaining verbs

R-sweep — one verb at a time. Can be done in parallel with Phase D. ~50 LOC per verb × N verbs.

---

## Phase D — TOCTOU + row-versioning (F8, F9)

### Slice D.1 — Add `version bigint` column to entity tables

**Change:** migration adding `row_version bigint NOT NULL DEFAULT 1` to entity tables used by the
gate surface. List per v0.3 R13 audit: `cbu`, `entity`, `deal`, `kyc_case`, ... (full list from
Phase 0a ownership matrix).

**Files:** `rust/migrations/YYYYMMDD_row_version.sql`.

**LOC:** migration-only (~200).

**Test plan:** migration idempotency; trigger-backfill test (concurrent updates).

**Acceptance:** every entity table used by `StateGateHash` has a monotonic version column.

### Slice D.2 — Backfill `row_version` under live traffic

**Change:** background backfill job that increments `row_version` on any UPDATE trigger; zero-downtime
deploy per v0.3 R13 mitigation.

**Files:** SQL triggers + a backfill job in `rust/src/maintenance/backfill_row_version.rs`.

**LOC:** ~200.

**Risk:** long-running backfill lock contention. Mitigation: chunked backfill + progress telemetry.

### Slice D.3 — Implement `StateGateHash` recheck inside txn

**Change:** inside `DslExecutor::execute_verb` (or its post-B.1 sequencer equivalent), after
acquiring row locks, recompute `StateGateHash` using the BLAKE3 spec in v0.3 §10.5. Compare against
`envelope.authorisation.state_gate_hash`. Mismatch → `DispatchError::ToctouMismatch`; outer
rollback.

**Files:** `rust/src/dsl_v2/executor.rs`, `rust/crates/ob-poc-types/src/gated_envelope.rs`.

**LOC:** 300-500.

**Test plan:** TOCTOU integration test — mutate row between gate and dispatch; assert dispatch
fails with `ToctouMismatch`.

**Acceptance:** v0.3 §17 item 8 PASS; drainer-kill-replay + TOCTOU-recheck tests green.

---

## Phase E — Dual-schema YAML + CRUD dissolution (F10, v0.3 Phase 6)

**Prerequisite:** Phase A-D complete. This is the longest-tail phase because it requires
effect-equivalence round-trip proofs per op.

### Slice E.1 — Define `runtime_schema:` + `catalogue_schema:` YAML keys

**Change:** extend verb YAML schema to accept dual-schema blocks. Parser validates both at startup.

**Files:** `rust/crates/dsl-core/src/config/types.rs`, verb YAML schema spec.

**LOC:** 200-300.

### Slice E.2 — Build round-trip harness (effect-equivalence per v0.3 §14)

**Change:** new crate `rust/crates/round-trip-harness/` — for each (args, pre-state) fixture, run
both the current Rust op impl and `PgCrudExecutor` interpreting YAML; compare effects byte-by-byte.

**Files:** new crate + fixtures.

**LOC:** 800-1200 (harness framework).

### Slice E.3 — Dissolve CRUD verbs that pass round-trip (R-sweep)

R-sweep through ~100 candidate CRUD verbs. For each: verify dual-schema YAML, run round-trip
(100% pass required), delete Rust impl. ~50-100 LOC reduction per verb × N verbs.

**Gate per verb:** 100% effect-equivalence across ≥ 50 fixtures.

**Expected outcome:** 50-60% dissolve; 5-10% reclassify as plugin; remainder already plugin.

---

## Phase F — Pattern B A1 remediation (F11)

**Source ledger:** `docs/todo/pattern-b-a1-remediation-ledger.md` already exists. Use it as the
authoritative work list.

### Slice F.1 — `bpmn_lite_ops` (5 ops, gRPC)

**Change:** refactor each op from in-body gRPC call to either (a) two-phase fetch-then-persist or
(b) outbox deferral.

**Files:** `rust/src/domain_ops/bpmn_lite_ops.rs`.

**LOC:** 300-500.

### Slice F.2 — `source_loader_ops` (16 ops, HTTP)

Same pattern. ~400-600 LOC.

### Slice F.3 — `gleif_ops` (17 ops, HTTP)

Same pattern. Note: `gleif_ops` also has an `ob-poc-adapter` destination flag — confirm destination
before migrating.

### Slice F.4 — Enable L4 workspace lint `forbid-external-effects-in-verb`

**Change:** workspace lint that greps for `reqwest`, `tonic::`, `Command` imports inside
`rust/src/domain_ops/` and `rust/crates/sem_os_postgres/src/ops/`.

**Acceptance:** ledger §2 CLOSED; v0.3 §11.2 A1 holds.

---

## Verification gate (final)

After all phases complete:

- v0.3 §17 Definition-of-Done — every item PASS.
- Effect-equivalence round-trip green for every dissolved verb.
- Determinism harness byte-identical across ≥ 100 fixtures.
- Intent hit rate within ±1% of pre-refactor baseline.
- Workspace lints L1-L4 enforced.
- Transaction-abort-on-stage-9a-fail test, drainer-kill-replay test, TOCTOU-recheck test — all
  green.

---

## Estimated timeline

- Phase A: 2-3 weeks
- Phase B: 2-3 weeks
- Phase C: 2-4 weeks (depends on verb count)
- Phase D: 3-4 weeks (row-version backfill is the long pole)
- Phase E: 4-8 weeks (round-trip harness + R-sweep over CRUD verbs)
- Phase F: 2-3 weeks

**Total: 4-6 months** sequential, potentially 3-4 months if B and D parallelize (they share
transaction-scope infrastructure).

---

## Why this plan is separate from the main correction plan

- Every phase here is a named v0.3 phase with its own decision gate (§16 items 13-19).
- The main correction plan restores invariants that were *already intended to hold* post-slice-#80.
  This plan builds invariants that were *always scoped as later phases*.
- Merging the two would conflate "fix what regressed" with "build the destination state" and make
  review impractical.

**Do not start Phase A until the main correction plan Phase 9 gate is green.** Premature start
means rebuilding fixtures twice.
