# Three-Plane v0.3 Wiring Follow-On Plan (2026-04-22)

**Prerequisite:** `three-plane-correction-slice-plan-2026-04-22.md` complete.
**Source review:** `docs/todo/three-plane-implementation-peer-review-2026-04-22.md` findings F5-F10.
**Spec:** `docs/todo/three-plane-architecture-v0.3.md` §§8.4, 10.3, 10.5, 10.7, 17.

## Phase-by-phase status (updated 2026-04-22 late-session, post-rollout)

| Phase | v0.3 mapping | State at end of session | Commits this session |
|-------|--------------|-------------------------|---------------------|
| **A** — envelope wiring (F5) | 5c-wire | **COMPLETE** (A.1-A.4). Shadow envelope + trace_id threading + real BLAKE3 StateGateHash all shipped. Placeholder fields tagged `<phase_a_todo>` for future concretization (DagNodeId, WorkspaceSnapshotId) but hash is deterministic and cross-run byte-identical. | 44585376, 9a1f76c6, 0b60d9df, d57a91be |
| **B** — single-dispatch (F6) | 5c-migrate final | **COMPLETE.** Entire transaction plumbing landed: α (dispatch wrapper collapse) → β (plan-atomic canonical in-scope) → γ (DslExecutor::execute_in_scope) → δ (StepExecutor::execute_step_in_scope) → ε (execute_runbook_in_scope + 2 proof tests) → ζ-1/2/3 (Sequencer opens outer scope, acquires locks on it, commits on clean/park, rolls back on fail). Atomic runbook contract (Q1.1a commit-on-park, Q1.2a rollback-on-fail, Q1.3 single-txn locks) is now live in production code. | 00033390, bf49dd2b, c3942912, f662b74d, 46bcfdcc, def658d0, 4db19367, a96e80c4, 3a1857fe, f87b145b, f170bbc2, 6d72a978 |
| **C** — PendingStateAdvance (F7) | 5c-migrate state-advance | **PATTERN COMPLETE + ROLLOUT SUBSTANTIVELY COMPLETE: 72 verbs emitting across 15 domains.** Shared `emit_pending_state_advance` + `emit_pending_state_advance_batch` (multi-entity fan-out) + `peek/take_pending_state_advance` accessors ready. Families with full coverage: cbu, entity, kyc_case, deal, screening, client_group, investor (11), document, tollgate, cbu_role, billing, capital, ownership, partnership, outreach, custody, remediation, trading_profile_ca (10 via save wrapper). Remaining verbs = edge cases (cron ticks, recompute sweeps, cache-only mutations) that need case-by-case evaluation, not mechanical rollout. **Apply-in-txn (C.2 main) needs B.2b first.** | 12cc4dc1, e632270a, 2d174449, 607a3b80, 0fdeab11, ea612b31, 4ed2dd65, 1f5f235e, b6c697b8, 7429f46d, 5adc441f |
| **D** — TOCTOU + row-version (F8, F9) | 5d | **D.1 STAGED + D.3 SCAFFOLD LANDED (2026-04-22 late-session).** D.1: SQL ready at `rust/migrations/20260422_row_version_entity_tables.sql` with real table names (`cases`, `client_group`); NOT yet applied (needs operator approval). D.3: `rust/src/toctou_recheck.rs` landed with `verify_toctou(envelope, workspace_snapshot_id, provider)` + `RowVersionProvider` trait + feature-gated SQL impl + 4 unit tests green. Activates the moment D.2 is applied AND real envelopes are constructed at stage 6. | 91aea971, 4ad61f29 |
| **E** — dual-schema YAML (F10) | Phase 6 | **SCAFFOLD PRESENT; R-SWEEP NOT STARTED.** `rust/crates/round-trip-harness/` + `rust/crates/determinism-harness/` both exist with meta-tests green (6 + 4). YAML schema extensions (`runtime_schema:` / `catalogue_schema:`) + per-verb effect-equivalence proofs remain the 4-8 week effort. | — |
| **F** — Pattern B A1 (F11) | 5f | **CLOSED 2026-04-22 late-session (37/37 ops across 6 files).** `SemOsVerbOp::pre_fetch(&args, &mut ctx, &sqlx::PgPool)` trait hook added — HTTP/gRPC/subprocess calls that need to produce read-only results now run OUTSIDE the txn scope and merge their result into args. Phase-by-phase: F.1 (§3.1 bpmn_lite) 5/5 CLOSED (BpmnInspect + BpmnCompile + BpmnStart via pre_fetch; BpmnSignal + BpmnCancel via outbox); F.2 (§3.2 source_loader) 12/12 CLOSED; F.3 (§3.3 gleif) 17/17 CLOSED (10 read ops + 1 dispatcher via pre_fetch, 6 write-interleaved ops via GleifEnrichmentService fetch/persist split). L4 workspace lint Layer 1 + `--taint` mode green with empty GRANDFATHERED array. **BpmnStart saga reaper design doc + prototype scaffolded** (`docs/todo/bpmn-start-saga-reaper-design-2026-04-22.md` + `rust/src/bpmn_integration/saga_reaper.rs`, 5 tests green) — not gating; production wiring is the next ~1 week of work. | 66b6398b, d57a91be, 0cc4c834, 3760c60f, 94fc11b6, fcc28bdd, 7ee94150, 19ef5035, 3228458f, 6cdbbda1, fddaf9ab |

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

After the 2026-04-22 session (late, post-backlog-sweep), the remaining work is narrowed to:

1. ~~Phase B.2b — Sequencer outer-scope migration.~~ **LANDED 2026-04-22** (sub-slices α through ζ). Sequencer opens one outer `PgTransactionScope` per batch, acquires advisory locks on that same scope, commits/rolls back per Q1.1a / Q1.2a. Atomic-runbook contract is live.

2. ~~Phase F — Pattern B A1 remediation.~~ **CLOSED 2026-04-22** (37/37 ops across 6 files). L4 lint Layer 1 + `--taint` mode green with empty GRANDFATHERED array. Saga-reaper follow-up for `bpmn.start` has its design doc + prototype scaffold; production wiring is ~1 week, not gating.

3. **Phase D.2 + D.3 activation** — row-version backfill under live traffic + wiring the `verify_toctou` scaffold into the Sequencer. Migration SQL ready at `rust/migrations/20260422_row_version_entity_tables.sql` with correct table names. Scaffold ready at `rust/src/toctou_recheck.rs` (4 tests green). Zero-downtime backfill across the 5 entity tables (cbus, entities, cases, deals, client_group) is the long pole (2-3 weeks under live traffic). Real envelope construction at stage 6 gates the recheck call site.

4. **Phase E — R-sweep.** Harness scaffolds present (`round-trip-harness/` + `determinism-harness/`, meta-tests green). YAML schema extensions (`runtime_schema:` / `catalogue_schema:`) + per-verb effect-equivalence proofs (N ≥ 50 fixtures per dissolved CRUD verb) remain. 4-8 weeks.

Items 3 + 4 are the genuine remaining work. Everything else in the plan is either COMPLETE or tracked to a follow-up design doc.

---

## Phase B.2b-ζ — RESOLVED (2026-04-22 late-session)

All three open questions were answered by user (verbatim decisions captured below) and implemented:

- **Q1.1 (a) — commit on park.** Scope covers "entries since last park"; park commits the current scope; resume opens a new scope for the continuation. Matches current UX on park boundaries; breaks "whole runbook atomic" claim across parks. **Implemented** in `execute_runbook_from` post-loop commit path (ζ-3).

- **Q1.2 (a) — atomic runbook is the right contract.** On any step failure the whole batch rolls back. **Implemented** via `runbook_failed` flag + break on Failed outcomes and Phase-5 recheck block; rollback executed in the post-loop cleanup.

- **Q1.3 — locks on outer scope.** Advisory locks acquired via new `acquire_advisory_locks_on_scope` helper using `scope.transaction()` instead of a parallel `_lock_tx`. **Implemented** in ζ-2. Locks and data writes now share one txn — release atomically with commit/rollback.

### Surface area landed

| Sub-slice | Commit | What it did |
|-----------|--------|-------------|
| ζ-1 | f87b145b | `execute_entry_via_gate` accepts `Option<&mut dyn TransactionScope>`; 4 bridge branches pick scope-aware vs pool-based path via a `run_through_gate` helper. |
| ζ-2 | f170bbc2 | `acquire_advisory_locks_on_scope` helper: acquires `pg_advisory_xact_lock` on the caller's scope's transaction. |
| ζ-3 | 6d72a978 | `execute_runbook_from` opens outer scope, aggregates write_set, acquires locks, threads scope into the 2 in-loop dispatch sites (Durable + Sync), sets `runbook_failed = true; break;` on Failed / Phase-5-recheck-block, commits on clean/park, rolls back on fail. |

### Follow-up opportunities (not blockers)

- **F18 legacy branch cleanup.** With atomic-runbook the contract, the pre-B.2b "per-entry commits" are no longer the production path. Removing `execute_runbook_with_pool` call sites from the Sequencer (and ultimately the function itself) is follow-up housekeeping. Kept for now as the fallback when `PgTransactionScope::begin` fails.

- **Sequencer-level atomicity integration tests.** The existing `sequencer_cross_step_atomicity.rs` exercises `execute_runbook_in_scope` directly. A fuller test harness would drive `ReplOrchestratorV2.process()` end-to-end across multiple entries and assert the atomic-on-failure / commit-on-park contract at that layer. Nice-to-have; not gating.

- **WorkflowDispatcher + durable verbs under outer scope.** Durable verbs route through the WorkflowDispatcher (BPMN signals). When a durable verb's outer txn commits on clean completion, any signal-processing logic on the other end sees the committed state. When the outer txn rolls back on a later failure, any signals already dispatched to the BPMN worker are NOT undone — this is the classic saga problem. Currently documented by the park semantics (durable verbs that await callback park the runbook, which commits-then-reopens), so a durable verb's dispatch is its own park boundary. Non-parking durable-direct verbs (`ctx.allow_durable_direct`) inside BPMN workers are inside the worker's own scope anyway. No additional change needed.

---

## Previous Phase B.2b-ζ open questions (historical, now resolved)

The original framing of the three open questions is kept below for traceability:

Phase B.2b-ζ is the final step: point `Sequencer::execute_entry_via_gate`
at `execute_runbook_in_scope` instead of `execute_runbook_with_pool`, with
the Sequencer opening an outer `PgTransactionScope` around the whole
runbook execution. Before that lands, three semantic decisions are needed:

### 1. Park atomicity

**Current behavior:** when entry N parks (e.g. awaits a BPMN signal),
entries 1..N-1 are already committed because each opens its own
transaction. User resumes later; entries N..end execute in fresh txns.

**Outer-scope problem:** a single `PgTransactionScope` cannot stay open
across a park. Txns don't survive process restarts, connection pool
recycling, or multi-hour user delays.

**Options:**
- (a) **Commit on park.** Scope covers "entries since last park"
  — park commits the current scope; resume opens a new scope for
  the continuation. Closest to current behavior; breaks "whole
  runbook atomic" claim but keeps user experience intact.
- (b) **Savepoints per entry.** Outer scope wraps the whole
  runbook; each entry gets a SAVEPOINT; park releases the
  savepoint. Complex to reason about; doesn't survive process
  restart either.
- (c) **Non-durable runbooks atomic, durable ones NOT.** Split
  the path: only runbooks with NO durable steps use
  `execute_runbook_in_scope`; anything touching a durable verb
  falls back to `execute_runbook_with_pool`. Narrow win, explicit
  carve-out.

**Recommendation:** option (a) at first, since it matches current
UX. Document that runbooks spanning parks are NOT atomic across
the park boundary.

### 2. Error recovery UX

**Current behavior:** entry 5 fails → entries 1-4 stay committed; user
sees "entry 5 failed" and can fix and retry.

**Outer-scope behavior:** entry 5 fails → whole scope rolls back →
entries 1-4 un-commit. User sees "runbook failed, all steps undone."

This is a **real UX change**. Users may have expected the intermediate
CBUs or entities to persist. Needs product sign-off before rolling out
— possibly an opt-in feature flag initially.

### 3. Advisory lock lifetime

**Current behavior:** `execute_runbook_with_pool` opens a SEPARATE
`_lock_tx` that holds advisory locks for the runbook's duration.
Data mutations happen in per-entry txns, locks in the parallel txn.

**Outer-scope behavior:** locks should be acquired ON the outer
scope (same txn as data). `execute_runbook_in_scope` leaves lock
acquisition to the caller; the Sequencer in B.2b-ζ must:

```rust
let mut scope = PgTransactionScope::begin(&pool).await?;
// acquire advisory locks on scope.transaction()
let tx = scope.transaction();
acquire_locks(tx, &lock_keys, LockMode::Timeout(...)).await?;
// run runbook
execute_runbook_in_scope(store, id, None, &bridge, &mut scope).await?;
// commit or rollback (releases locks on the same txn)
```

Lock acquisition helper needs adapting — `acquire_advisory_locks`
currently opens a NEW transaction from the pool; the B.2b-ζ version
takes the scope's existing transaction.

### Summary

α through ε landed in one session (2026-04-22); ζ is a ~2-3 day
slice once (1), (2), (3) are answered. The public
`execute_runbook_in_scope` API is available for any caller ready
to commit to outer-scope semantics — so a feature-flagged rollout
is viable: V2 REPL gets `execute_runbook_in_scope` behind a flag,
legacy paths keep `execute_runbook_with_pool` until confidence
accrues.

---

## Phase C.2-main prerequisite — token-to-DagNodeId resolver

**Surfaced 2026-04-22 late-session** while writing the shape-contract tests in
`rust/crates/dsl-runtime/src/domain_ops/helpers.rs`.

**Gap:** the 72 C.3 emit sites all write pre-resolution taxonomy tokens
(e.g. `"cbu:onboarded"`, `"entity:ghost"`, `"capital:transferred_out"`).
`ob_poc_types::PendingStateAdvance.state_transitions[].to_node` is typed
`DagNodeId(Uuid)`. Direct deserialisation fails (test:
`emit_shape_is_not_directly_deserializable_into_typed_advance`). C.2-main
is where the resolver lives — it must run BEFORE the typed advance is
constructed and applied via SemOS.

**Token inventory (emitted across 72 verbs, 15 domains):**

| Namespace | Count | Registered in `node_state_registry.yaml`? |
|-----------|-------|-------------------------------------------|
| `billing-period:*` (5) | 5 | **No** |
| `billing-profile:*` (3) | 3 | **No** |
| `capital:*` (9) | 9 | **No** |
| `cbu-role:*` (6) | 6 | **No** |
| `cbu:onboarded` | 1 | Yes (registry has `cbu`) |
| `client-group-membership:*` (2) | 2 | **No** |
| `custody:ssi_configured` | 1 | **No** |
| `deal:*` (2) | 2 | Yes (registry has `deal`) |
| `document-version:*` (2) | 2 | Registry has `document`, not `document-version` |
| `entity:*` (4) | 4 | Registry has `entity_kyc`/`entity_workstream`, not plain `entity` |
| `investor:*` (8) | 8 | **No** |
| `kyc-case:intake` | 1 | Registry has `kyc_case` (UNDERSCORE) — punctuation mismatch |
| `ownership:*` (2) | 2 | **No** |
| `partnership:*` (2) | 2 | **No** |
| `remediation:*` (3) | 3 | **No** |
| `requirement:requested` | 1 | **No** |
| `research:*` (2) | 2 | **No** |
| `tollgate-evaluation:overridden` | 1 | **No** |
| `trading:profile_ca_updated` | 1 | Registry has `trading_profile`, not `trading` |

**What C.2-main must resolve:**

1. **Registry coverage.** ~13 of 15 namespaces have no entry in
   `node_state_registry.yaml`. Either extend the YAML or carry the
   taxonomy elsewhere. Decision needed before the resolver can be
   implemented.
2. **Punctuation normalisation.** Emitted tokens mix hyphen (`kyc-case`,
   `cbu-role`, `billing-period`) with registry underscore
   (`kyc_case`, `entity_kyc`). Resolver must either canonicalise or the
   YAML must add kebab aliases.
3. **Namespace alignment.** Several emitted namespaces are sub-levels
   the registry doesn't yet model (`document-version` vs `document`,
   `trading` vs `trading_profile`, `entity` vs `entity_kyc`). Decide
   whether these are distinct state machines or sub-states of
   registered ones.
4. **UUID assignment.** Registry currently stores names only; `DagNodeId`
   is `(Uuid)`. A node-id assignment step (either deterministic hash-of-
   `<namespace>:<name>` or explicit UUIDs in the YAML) must land before
   resolution can produce real `DagNodeId` values.

**Priority:** this is a **pre-requisite for Phase C.2-main**, not a new
blocker — C.2-main was already gated on B.2b. But it means C.2-main
is NOT simply "plumbing apply into the Sequencer"; it's also a
taxonomy-bridging design step that needs sign-off before code goes in.

Until C.2-main lands, the emit sites work correctly as observed-and-
logged side effects (shadow-logged at
`rust/src/dsl_v2/executor.rs:1583-1591`). They produce no incorrect
advance — just no advance at all. So downstream consumers (narration,
constellation rehydration, progress UI) still see the legacy
refresh-after-write path and not the declarative PendingStateAdvance
union. This is intentional and safe; a regression only occurs if
someone removes the resolver step after it lands.

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
