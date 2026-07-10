# IMPLEMENTATION PLAN — ob-poc-control-plane
### EOP-PLAN-CONTROLPLANE-001 v0.1
### Basis: EOP-VS-CONTROLPLANE-001 v0.3 + docs/research/control-plane-phase0-inventory.md
### Executor: zed.sonnet (tranche-by-tranche) · Reviewer: Claude (adversarial, authorship-blind)

---

## 0. Plan assumptions (architect decisions baked in — override before starting)

| # | Question (RR-8) | Decision |
|---|---|---|
| A1 | Bus invocation service | Envelope-primary. Interim (T6.1): bus handler must call `evaluate()` before executor dispatch. Target (T6.4): bus accepts only `EnvelopeHandle`. |
| A2 | GatePipeline config/DAG load failure | Production-fatal. Dev/test may opt into fail-open via explicit `OB_POC_GATES_FAIL_OPEN=1` + startup banner. |
| A3 | Raw DSL endpoint | Dev-only behind a cargo feature `raw-dsl-dev`; even then routed through `evaluate()`. Production build excludes the route entirely. |
| A4 | Write-set derivation | Contract-union (currently feature-gated, C-019) becomes the default; heuristic mode retired. Prerequisite for G14. |
| A5 | Row-version migration (C-045 / RR-5) | Assumed to land before T4.3 (pre-state pinning). Until then, unpinnable entities are STP-ineligible per V&S §6.10.1 — they run human-gated. |

Shadow-first strategy: the control plane runs in **shadow mode** (evaluate + record + compare, never block) from T1 until each gate individually graduates to **enforce mode** via config. Graduation criterion per gate: ≥ N production evaluations (suggest N=500 or 2 weeks) with zero divergence between shadow decision and legacy path outcome, or every divergence triaged as a legacy defect.

Single-owner ledger: every tranche that moves/retires a check MUST update `docs/research/control-plane-ownership-ledger.md` (created in T1.1) mapping each C-0xx → current owner → target owner → status. Phase 0's inventory is the opening balance.

---

## Tranche T0 — Hard fences (pre-crate risk closure)

Small, independent, immediately shippable diffs. No new crate yet.

### T0.1 GatePipeline startup fail-closed (closes C-030)
- `ob-poc-web/src/main.rs:L1400-L1440`: config/DAG registry load failure → process exit with actionable error, unless `OB_POC_GATES_FAIL_OPEN=1` (log a WARN banner every 60s while running fail-open).
- Exit criteria: unit test proving startup aborts on missing DAG registry; test proving env override runs with banner; `cargo test -p ob-poc-web` green.

### T0.2 requires_states fail-open → configurable fail-closed (closes C-027 divergence)
- `ob-poc/src/dsl_v2/executor.rs:L2015-L2115`: introduce `LifecycleGateMode { FailOpen, FailClosed }` config, default `FailClosed` in production profile. The five fail-open classes (no lifecycle, no entity arg, invalid UUID, no slot mapping, absent/NULL/unreadable state) each emit a structured audit event naming the class, in BOTH modes.
- Reconcile semantics with `GateChecker` (C-025/C-026): document in code comments which check owns what; divergences recorded in the ownership ledger for T2.2 unification.
- Exit criteria: table-driven test covering all five classes in both modes; no silent pass path remains (grep gate: no `Ok(())` return in that function without a preceding event emission).

### T0.3 No-pool lock skip → production error (closes C-022)
- `ob-poc/src/runbook/executor.rs:L879-L885`: write-set present + no pool → hard error in production; explicit `allow_unlocked_execution(test_only_token)` for test paths.
- Exit criteria: test proving error; existing in-memory tests migrated to the explicit token.

### T0.4 BPMN pre_fetch write moved into scoped execute (closes C-037)
- `ob-poc/src/domain_ops/bpmn_controller_ops.rs:L273-L320`: relocate `start-instance` side effect from `pre_fetch` into `execute` so the write participates in the op transaction scope. Idempotency check (already in `bpmn-controller/src/instance.rs:L100-L190`) protects retries.
- Exit criteria: integration test: op failure after execute-start leaves no `process_instances` row (or a compensated one); `cargo test -p ob-poc -p bpmn-controller` green.

PRINT: `T0 COMPLETE — progress ~12% — → IMMEDIATELY proceed to T1`

---

## Tranche T1 — Crate skeleton: ob-poc-control-plane

### T1.1 Crate + ledger
- New crate `rust/crates/ob-poc-control-plane` with `#![deny(unreachable_pub)]`, registered in workspace members, wired into `scripts/check-public-api-surface.sh` baseline.
- Create `docs/research/control-plane-ownership-ledger.md` seeded from RR-3 (45 rows, dispositions as recorded).
- Modules per V&S §9.2: `intent_admission, entity_binding, pack_resolution, dag_proof, authority_gate, evidence_gate, write_set, stp_classifier, snapshot, proof, envelope, audit, metrics, exceptions, versioning`.

### T1.2 Proof types and decision model
- Success-form proof types with module-private constructors: `AdmittedIntent, BoundEntities, ResolvedPack, LegalTransition, Authorised, EvidenceSufficient, WriteSetProof, CompiledRunbookRef, SnapshotPins, ValidityWindow`.
- `ControlPlaneDecision { ApprovedStp(ExecutionEnvelope), RequiresHumanGate(ControlPlaneProof), Rejected(ControlPlaneRejection) }`.
- `ExecutionEnvelope`: private fields, `pub(crate) fn seal(...)` taking every success proof by signature (V&S §9.4), NO `Deserialize` impl. `EnvelopeHandle` newtype (opaque id + hash) IS serializable.
- `ControlPlaneRejection` aggregates per-gate failures incl. `not_evaluated { blocked_by }`.

### T1.3 Declared dependency graph + evaluator
- Gate dependency declaration as a const table exactly per V&S §6.16.1; collect-where-independent evaluator generic over gate trait `Gate<Ctx> { fn evaluate(&self, ctx) -> GateResult }`.
- All 14 gates stubbed returning `GateResult::NotImplemented` (maps to `not_evaluated` in shadow mode).
- Exit criteria (whole tranche): `cargo build -p ob-poc-control-plane` green; `cargo test -p ob-poc-control-plane` green with tests proving (a) seal unreachable from any failure form (compile-fail test via trybuild), (b) evaluator honours dependency declaration, (c) envelope not deserializable (compile-fail test); public API surface snapshot committed.

PRINT: `T1 COMPLETE — progress ~25% — → IMMEDIATELY proceed to T2`

---

## Tranche T2 — Gate adapters over existing validators (INVOKE dispositions)

Each sub-tranche wraps an existing validator behind a gate trait impl, returning proof types. NO logic moves yet — adapters only. Each cites its C-0xx rows and updates the ledger.

- **T2.1 G1 intent admission** over `SessionVerbSurface` + `SemOsContextEnvelope` (C-007, C-009; SPLIT: discovery/ranking stays, admission decision becomes the gate). Interpretation attestation: adapt existing Sage pre-classification + intent telemetry fields (`ob-poc/src/agent/telemetry/store.rs`) into an `InterpretationAttestation` input; absent attestation on AI-originated intent → deterministic rejection.
- **T2.2 G4 DAG proof** over `GateChecker::check_transition` (C-025/C-026) producing `LegalTransition`/failure with violations. The T0.2 lifecycle check becomes a second input source; unify the two semantics here — `requires_states` logic is subsumed and marked RETIRE-pending in the ledger.
- **T2.3 G3 pack resolution** over constraint gate + SemReg fail-closed (C-015, C-016).
- **T2.4 G5 authority** over `AccessDecision` partitioning + `ActorResolver` (C-006, C-007, C-008); TOCTOU recheck result consumed as snapshot evidence, not re-implemented.
- **T2.5 G6 evidence** over SemOS governance evidence gaps + KYC preconditions via adapter trait so `ob-poc-kyc-substrate` stays the owner (C-040, C-041, C-042).
- **T2.6 G7 write-set** over `derive_write_set` with contract-union promoted to default (A4, C-019). Heuristic mode deleted; feature flag removed.
- **T2.7 Shadow wiring**: replace the shadow `GatedVerbEnvelope` builder call in Sequencer stage 6 (`ob-poc/src/sequencer.rs:L7721-L7765`, C-044) with `control_plane.evaluate()` in shadow mode; record `ControlPlaneDecision` + divergence-vs-legacy-outcome to a new `control_plane_shadow_decisions` table (design beside `sem_reg.decision_records`).
- Exit criteria: all six adapters unit-tested against fixtures from existing validator tests; shadow decisions visible for Path A end-to-end in integration test; zero behaviour change on legacy path (assert legacy outcomes unchanged in integration suite); ledger updated.

PRINT: `T2 COMPLETE — progress ~45% — → IMMEDIATELY proceed to T3`

---

## Tranche T3 — Missing gates (G2, G8, G13 — no analogues exist, RR-8)

- **T3.1 G2 EntityBindingReport**: real binding gate — existence, kind, lifecycle state readability, availability (locked/archived), pack membership. Reads through snapshot handles (T3.3). Replaces the placeholder `ResolvedEntity{row_version:0}` (RR-5 Mode-1 row 1).
- **T3.2 G13 SnapshotPins**: one object pinning every gate read: SemReg snapshot set id (`ob-poc/src/sem_reg/store.rs`), session version/current_snapshot_id (`ob-poc/src/repl/session_repository.rs`), KYC manifest hash + subject next_seq where KYC entities bound, entity row_versions where available. Gates from T2 refactored to read through a `SnapshotCtx` that records pins as it reads. Live-read gates (GateChecker slot reads) record the sequence observed at read time as the pin.
- **T3.3 G8 STP classifier**: pure function over aggregated gate results + config policy → `StpEligibilityDecision`. Encodes A5: any bound entity lacking a comparable version pin → cap at `RequiresHumanGate`. Durable verbs map existing C-028 rule into classifier policy.
- **T3.4 G9 ControlPlaneProof**: aggregate artefact bundling all gate outputs + pins + compiled runbook ref; persisted beside `sem_reg.decision_records` (reuse `snapshot_manifest` pattern, C-044/RR-4).
- Exit criteria: property test — same (intent, ctx, pins) → identical decision across 1,000 randomized re-evaluations; replay test — persisted proof re-evaluated against pinned snapshot reproduces decision; shadow divergence dashboarded.

PRINT: `T3 COMPLETE — progress ~60% — → IMMEDIATELY proceed to T4`

---

## Tranche T4 — Envelope production wiring (G10, G12)

- **T4.1 Envelope admission at the port**: `VerbExecutionPort::execute_verb` (`dsl-runtime/src/port.rs`) gains an envelope-bearing entry; `VerbExecutionPortStepExecutor` presents the envelope; `ObPocVerbExecutor` rejects envelope-less dispatch when enforce mode is on (config per-path graduation).
- **T4.2 Envelope persistence + single-use + TTL**: `control_plane_envelopes` table beside runbook store (RR-6 recommendation): envelope hash, status (Sealed/Consumed/Expired/Voided), validity window, consumed_at. `EnvelopeHandle` rehydration ONLY through control plane verification (re-check single-use, window, pins) per V&S §6.10.4 — parked/human/durable states reuse session invocation-record pattern (`ob-poc/src/repl/session_repository.rs:L469-L499`).
- **T4.3 Pre-state pinning enforcement**: envelope carries per-entity expected versions from `SnapshotPins`; port admission re-reads and compares; mismatch → void + `stale_state` exception. Depends on row-version migration (A5); entities not yet covered stay human-gated per T3.3. Finish and productionise the TOCTOU scaffold (`ob-poc-boundary/src/toctou_recheck.rs`, C-045) as the comparison implementation; `ob-poc-types/src/gated_envelope.rs` types either subsumed or deleted (ledger decision, C-044).
- **T4.4 G12 pinned version set**: unify subsystem pins (SemReg snapshot ids, KYC manifest hash, bus catalogue version C-033, DSL/compiler crate versions, model/prompt version from attestation) into the envelope's version block.
- Exit criteria: integration tests — consumed envelope resubmission rejected; expired envelope rejected; stale pin voids and routes exception; enforce-mode Path A green end-to-end; shadow mode still default for untouched paths.

PRINT: `T4 COMPLETE — progress ~75% — → IMMEDIATELY proceed to T5`

---

## Tranche T5 — Write-set attestation (G14 — nothing exists, RR-4)

- **T5.1 Write capture**: CRUD executor (`dsl-runtime/src/crud_executor.rs`) and plugin-op transaction wrapper (`ob-poc/src/sem_os_runtime/verb_executor_adapter.rs:L117-L286`) record touched (table, entity_id, columns) into a per-transaction `WriteCapture` — implementation choice for sonnet: thread a capture handle through the executor context (preferred) rather than pg event triggers.
- **T5.2 Pre-commit attestation**: before commit, runtime compares capture ⊆ `WriteSetProof`; excess → **abort transaction** + control-breach event (V&S §6.7.1 default posture). Post-durability check for async projection paths → quarantine + exception route.
- **T5.3 Attestation record**: persisted per execution, referenced from audit stream; breach counter in metrics.
- Exit criteria: fault-injection test — op writing an undeclared table aborts with breach event, no durable row; projection-side breach quarantines; `cargo test` workspace green.

PRINT: `T5 COMPLETE — progress ~85% — → IMMEDIATELY proceed to T6`

---

## Tranche T6 — Bypass closure (the Phase 0 hit-list)

- **T6.1 Bus path** (C-034, A1 interim): `ObPocBusHandler` → adapter calls `evaluate()` with a `BusPrincipal` context (replaces bare `Principal::system()`); enforce mode on from day one for bus (it has no legacy users to shadow-compare — confirm with architect if false).
- **T6.2 Raw DSL** (C-001, C-017, C-018, A3): route behind `raw-dsl-dev` cargo feature; internal flow replaced by `evaluate()` + envelope execution; duplicate SemOS/validator checks (C-017/C-018) retired in ledger.
- **T6.3 RealDslExecutor + WorkflowDispatcher direct branch** (RR-6 rows 3–4): both acquire envelopes via `evaluate()`; `execute_in_scope` accepts an envelope-scoped variant.
- **T6.4 Bus target state** (A1 final): bus accepts `EnvelopeHandle` only; direct verb invocation over bus removed from catalogue.
- **T6.5 Enforce-mode graduation**: flip Path A (Sequencer/runbook) to enforce once shadow criteria met; startup asserts every registered dispatch path is envelope-guarded (deployment guard in the C-029 pattern).
- Exit criteria: grep/CI gate — no call site constructs `VerbExecutionContext` outside control-plane-issued paths (allowlist file, CI-checked); all four RR-2 paths traced in integration tests through envelope admission; ledger shows zero checks with ambiguous ownership.

PRINT: `T6 COMPLETE — progress ~95% — → IMMEDIATELY proceed to T7`

---

## Tranche T7 — Assurance plane (V&S Phase 5)

- **T7.1** Unified audit stream: one `control_plane_audit` append per decision linking utterance hash → attestation → gate results → pins → envelope → attestation record → outcome (existing stores become sources, not replaced — C-040..C-043, telemetry, traces).
- **T7.2** Metrics per V&S §6.14 (per-gate rejection rates meaningful thanks to collect-where-independent), exception ageing, breach count, replay success.
- **T7.3** Decision-replay job: sample N daily decisions, re-evaluate against pinned snapshots, alert on divergence.
- **T7.4** Regression suite over the gate dependency declaration + NIST crosswalk doc updated from design-crosswalk toward operating status (Appendix A caveat lift is a Risk decision, not a code change).
- Exit criteria: replay job green over a seeded week of decisions; dashboard renders; V&S §12 success criteria walked and evidenced one by one in a closing report.

PRINT: `T7 COMPLETE — progress 100%`

---

## Completion invariant (E) — plan level

- E1: every RR-3 C-0xx row is CLOSED in the ownership ledger (moved, invoked, retired, or split with both halves named).
- E2: all four RR-2 paths execute only via envelope admission in enforce mode.
- E3: G1–G14 each evaluated in production (not `NotImplemented`) with metrics flowing.
- E4: Mode-1 register (RR-5) rows either version-pinned or permanently classified human-gated with the classification tested.
- E5: workspace green: `cargo build && cargo test` all crates; public-API surface gate green; `unreachable_pub` clean.

## Sonnet execution notes

- One tranche per session; do not start T(n+1) in the same session as T(n) unless T(n) exit criteria are demonstrably green in that session.
- Never weaken an exit criterion to pass it; if a criterion is wrong, STOP and flag to the architect — that is the only sanctioned stop.
- Cite C-0xx and file:line from the Phase 0 inventory in every commit message touching an inventoried check.
- If a needed row-version/migration dependency (A5) is not landed when T4.3 starts, implement behind the pin-availability check and proceed — do not block the tranche.
