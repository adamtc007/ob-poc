# Control Plane Ownership Ledger

> Created by T1.1 (EOP-PLAN-CONTROLPLANE-001). Opening balance seeded verbatim
> from `docs/research/control-plane-phase0-inventory.md` §RR-3 (45 rows).
>
> Every tranche that moves, invokes, retires, or splits a check MUST update
> its row here. `status` starts at `OPEN` for every row and moves to
> `CLOSED` only when the disposition is actually executed (not merely
> planned) — per plan §0 "Single-owner ledger" and completion invariant E1
> ("every RR-3 C-0xx row is CLOSED in the ownership ledger — moved, invoked,
> retired, or split with both halves named").

## Columns

- **CID** — Phase 0 inventory control id (`C-001`..`C-045`).
- **Check** — one-line description (from RR-3).
- **Location** — `file:line` at time of Phase 0 inventory. Not re-verified
  per edit; drifts are expected and not a ledger defect.
- **Owner (opening)** — crate that owned the check at Phase 0 time.
- **Gate mapping** — G1–G14 per RR-3 (or `NONE`).
- **Disposition** — RR-3's candidate disposition (`MOVE` / `INVOKE` /
  `RETIRE` / `SPLIT`, with detail).
- **Target owner** — crate/module expected to own the check post-disposition.
  `ob-poc-control-plane::<module>` once a T2+ adapter lands; left as `TBD`
  until that tranche executes.
- **Status** — `OPEN` (opening balance, nothing executed yet) or `CLOSED`
  (disposition executed — cite the commit/tranche in a trailing note).

## Ledger

| CID | Check | Location | Owner (opening) | Gate | Disposition | Target owner | Status |
|---|---|---|---|---|---|---|---|
| C-001 | Raw execute endpoint rejects normal session-flow requests. | `ob-poc/src/api/agent_routes.rs:L1861-L1878` | `ob-poc` | G1, G8 | RETIRE after envelope path replaces raw endpoint. | TBD (T6.2) | **CLOSED** — T6.2 (verification, no new code): re-read `execute_session_dsl_legacy_raw_only`/`execute_session_dsl_raw` and confirmed the raw-DSL-in-request-body bypass was already removed by the pre-existing Slice 3.1 fix (2026-04-22, `OBPOC_ALLOW_RAW_EXECUTE` deleted, no flag can reopen it — documented in CLAUDE.md but not previously cross-checked against this ledger row). `is_raw_execute_request` returns `410 GONE` before any execution for any request with a non-empty `dsl` field; `execute_session_dsl_raw`'s own inner check independently returns `403 FORBIDDEN` for `req.dsl.is_some()` (belt-and-suspenders for the `Some("")` edge case). This row's disposition was already satisfied prior to this session — the ledger simply hadn't been updated to reflect it. **Commit hash (backfilled, EOP-IMPL-CONTROLPLANE-GRADUATION-G0-001 Slice 4, 2026-07-13):** `1a194d40` ("v1.2 Catalogue Platform Refinement: Tranche 1 + 2 + pub API surface cleanup (#2)", merged 2026-04-26, containing the 2026-04-22 Slice-3.1 sub-change) — `rust/src/api/agent_routes.rs` diff in that commit shows the `OBPOC_ALLOW_RAW_EXECUTE` flag/`can_execute_raw_dsl` method deletion and the added `// F16 fix (Slice 3.1, 2026-04-22): raw DSL bypass removed` comment at the `execute_session_dsl_raw` call site, confirmed via `git show 1a194d40 -- rust/src/api/agent_routes.rs`. |
| C-002 | AgentMode blocks runbook compilation unless session can compile. | `ob-poc/src/api/repl_routes_v2.rs:L3767-L3776` | `ob-poc` | G1, G5 | MOVE into admission/authority; UI mode can remain local. | TBD (T2.1/T2.4) | OPEN |
| C-003 | Plan approval requires existing plan and Compiled/AwaitingApproval status. | `ob-poc/src/api/repl_routes_v2.rs:L3925-L3948` | `ob-poc` | G8, G9 | INVOKE from control plane. | TBD (T3.3/T3.4) | OPEN |
| C-004 | Plan execution requires Repl mode and Approved/Executing status. | `ob-poc/src/api/repl_routes_v2.rs:L3990-L4042` | `ob-poc` | G8, G9 | MOVE status admission to control plane, keep route wrapper. | TBD (T3.3/T3.4) | OPEN |
| C-005 | PolicyGate removes raw execute feature flag and centralizes strict single-pipeline / SemReg fail-closed flags. | `ob-poc-boundary/src/policy/gate.rs:L12-L23,L50-L69` | `ob-poc-boundary` | G1, G3, G5 | SPLIT: config stays boundary, gate decision moves. | TBD (T2.1/T2.3/T2.4) | OPEN |
| C-006 | ActorResolver builds actor role/clearance/jurisdiction context from env/session. | `ob-poc-boundary/src/policy/gate.rs:L79-L121` | `ob-poc-boundary` | G5 | INVOKE as identity source, not policy owner. | `ob-poc-control-plane::authority_gate` | **PARTIALLY CLOSED** — T2.4: `AuthorityGate` adapter landed + unit tested (`AuthorityInput.actor_id`/`role` mirror `ActorContext`); no production call site wires `ActorResolver` output into it yet — OPEN for call-site wiring. |
| C-007 | SemOS context envelope partitions allowed/pruned verbs and computes fingerprint. | `ob-poc/src/agent/sem_os_context_envelope.rs:L20-L56,L163-L247` | `ob-poc` | G1, G3, G5, G6, G12 | MOVE decision ownership; keep envelope as input/adapter. | `ob-poc-control-plane::intent_admission` (G1 portion) | **PARTIALLY CLOSED** — T2.1/T2.7 (this commit): `envelope.allowed_verbs`/`pruned_verbs` now feed `IntentAdmissionGate` in production shadow mode at the Sequencer stage-6 call site (`agent::control_plane_shadow::build_evaluation_context`). G3/G5/G6/G12 portions (T2.3/T2.4/T2.5/T4.4) remain OPEN — no call site wired for those adapters. |
| C-008 | SemOS TOCTOU recheck result can block if selected verb no longer allowed. | `ob-poc/src/agent/sem_os_context_envelope.rs:L138-L161,L288-L335` | `ob-poc` | G5, G12, G13 | MOVE into snapshot/policy gate. | `ob-poc-control-plane::authority_gate` | **PARTIALLY CLOSED** — T2.4: `AuthorityInput.toctou_drifted` consumes a non-`StillAllowed` `TocTouResult` as a policy-time input (`RequiresHumanApproval`), adapter unit tested; no production call site wires a real `TocTouResult` into it yet. |
| C-009 | Session verb surface applies AgentMode, scope/workflow, SemReg CCIR, fail policy, and ranking. | `ob-poc/src/agent/verb_surface.rs:L314-L324,L337-L496` | `ob-poc` | G1, G3, G5 | SPLIT discovery ranking from admission gate. | TBD (T2.1/T2.3/T2.4) | OPEN — `IntentAdmissionInput` is shaped to accept `SessionVerbSurface::allowed_fqns()`/`contains()` output equally well as `SemOsContextEnvelope`'s, but the T2.7 call site this tranche wired uses only the latter; no call site threads `SessionVerbSurface` output through yet. |
| C-010 | Lifecycle state is tagged for discovery but not pruned. | `ob-poc/src/agent/verb_surface.rs:L451-L462,L509-L521` | `ob-poc` | G4 | INVOKE only as hint; not gate proof. | TBD (T2.2) | OPEN |
| C-011 | Macro expansion validates required args and failure maps to clarification/error. | `ob-poc/src/runbook/compiler.rs:L167-L225` | `ob-poc` | G1, G9 | INVOKE. | TBD (T2.1/T3.4) | OPEN |
| C-012 | Macro fixpoint expansion enforces depth, step limits, and per-path cycle detection. | `ob-poc/src/dsl_v2/macros/expander.rs:L250-L346` | `ob-poc` | G1, G9 | INVOKE. | TBD (T2.1/T3.4) | OPEN |
| C-013 | Expanded macro output is revalidated and rejects unknown runtime verbs. | `ob-poc/src/runbook/compiler.rs:L603-L638` | `ob-poc` | G1, G9 | MOVE admission part; keep macro validator local. | TBD (T2.1) | OPEN |
| C-014 | Plan assembler builds dependency graph, rejects cycles/empty plans, but unresolved bindings are diagnostics. | `ob-poc/src/plan_builder/plan_assembler.rs:L80-L160,L234-L266` | `ob-poc` | G4, G9 | INVOKE. | TBD (T2.2/T3.4) | OPEN |
| C-015 | Pack constraint gate blocks verbs not permitted by active pack constraints. | `ob-poc/src/runbook/constraint_gate.rs:L1-L15,L33-L100` | `ob-poc` | G3 | MOVE gate decision, invoke pack data. | `ob-poc-control-plane::pack_resolution` | **PARTIALLY CLOSED** — T2.3: `PackResolutionGate` adapter landed + unit tested (`PackResolutionInput.constraint_denies_intent` mirrors `check_pack_constraints`'s empty-intersection deadlock); no production call site wires `constraint_gate` output into it yet. |
| C-016 | SemReg allowed-set unavailable is fail-closed in compiler. | `ob-poc/src/runbook/compiler.rs:L339-L362,L468-L486` | `ob-poc` | G3, G5 | MOVE. | `ob-poc-control-plane::pack_resolution` | **PARTIALLY CLOSED** — T2.3: `PackResolutionInput.semreg_allowed_set_available` reproduces the compiler's fail-closed default (unavailable ⇒ `MissingPack`, never falls through to `Resolved`), adapter unit tested; no production call site wired yet. |
| C-017 | Raw endpoint performs separate SemOS allowed-verb policy checks. | `ob-poc/src/api/agent_routes.rs:L2101-L2177` | `ob-poc` | G3, G5 | RETIRE duplicate of C-016 after raw path removal. | TBD (T6.2) | **RECLASSIFIED, still OPEN** — T6.2: since C-001's raw-DSL-body path is confirmed already closed (nothing reaches this code with attacker-controlled raw DSL anymore — only the session's own already-staged run-sheet DSL), this check is no longer "a duplicate policy check inside a doomed bypass endpoint." It is now the sole SemOS allowed-verb validation for whatever DSL the run-sheet execution route actually runs. RETIRE is the wrong disposition for it as things stand; INVOKE (fold into a real gate call site, same as C-016) is the live option, unchanged from T2's un-wired state. Not touched this tranche — flagged so a future tranche doesn't retire load-bearing validation under a stale "it's just a duplicate of the bypass" assumption. |
| C-018 | Raw endpoint performs separate semantic validator checks. | `ob-poc/src/api/agent_routes.rs:L2180-L2257` | `ob-poc` | G4, G7 | RETIRE or INVOKE through same compiler. | TBD (T6.2) | **RECLASSIFIED, still OPEN** — T6.2: same finding as C-017 — CSG/dataflow validation on the run-sheet's DSL is load-bearing for the only DSL that reaches this handler post-Slice-3.1, not a retireable duplicate. Not touched this tranche. |
| C-019 | Write set is heuristic by default and contract-union only behind feature flag. | `ob-poc/src/runbook/write_set.rs:L1-L17,L112-L146` | `ob-poc` | G7 | SPLIT into proof plus runtime attestation. | `ob-poc-control-plane::write_set` (proof) / TBD (T5.1, attestation) | **PARTIALLY CLOSED** — T2.6: `WriteSetGate` adapter landed + unit tested; `WriteSetInput.contract_derived` refuses to bound a `WriteSetProof` from heuristic-only derivation (`CannotDerive`), so the gate itself cannot silently accept the fail-open heuristic default. Plan A4's crate-level change (delete heuristic default, remove `write-set-contract` feature flag in `ob-poc/src/runbook/write_set.rs`) is a separate call-site change **not** made this tranche — remains OPEN. Runtime attestation (G14) is T5.1, unstarted. |
| C-020 | Runbook execution rejects missing or non-executable persisted runbooks. | `ob-poc/src/runbook/executor.rs:L812-L824` | `ob-poc` | G9, G10 | MOVE admission, keep store lookup. | TBD (T3.4/T4.1) | OPEN |
| C-021 | Advisory locks are sorted, timeout-bounded, and emit events on lock paths. | `ob-poc/src/runbook/executor.rs:L682-L755,L831-L875` | `ob-poc` | G10, G13 | INVOKE. | TBD (T4.1/T3.2) | OPEN |
| C-022 | Runbook executor warns and proceeds without locks when write set exists but no pool exists. | `ob-poc/src/runbook/executor.rs:L879-L885` | `ob-poc` | G10 | RETIRE for production; keep test-only explicit path. | `ob-poc` (`UnlockedExecutionToken`) | **CLOSED** — T0.3 (`65c60006`): production path now hard-errors; test-only path requires explicit `UnlockedExecutionToken`. |
| C-023 | Step dependencies cause skipped dependent steps on prior failure. | `ob-poc/src/runbook/executor.rs:L929-L960` | `ob-poc` | G9 | INVOKE. | TBD (T3.4) | OPEN |
| C-024 | Optional pre-dispatch GatePipeline returns `Ok(())` when absent or no transition metadata. | `ob-poc/src/runbook/step_executor_bridge.rs:L202-L212` | `ob-poc` | G4 | SPLIT: missing pipeline should be admission-visible. | TBD (T2.2) | OPEN |
| C-025 | GatePipeline evaluates DAG transitions and blocks severity=`error` violations. | `ob-poc/src/runbook/step_executor_bridge.rs:L214-L293` | `ob-poc` | G4 | MOVE proof ownership; invoke checker. | `ob-poc-control-plane::dag_proof` | **PARTIALLY CLOSED** — T2.2: `DagProofGate` adapter landed + unit tested (`DagProofInput.blocking_violations` mirrors `severity=error` `GateViolation`s, mapped to `GuardFailed`); no production call site wires `GatePipeline`/`step_executor_bridge` output into it yet. |
| C-026 | GateChecker resolves source entity, reads source slot, and reports violations. | `dsl-runtime/src/cross_workspace/gate_checker.rs:L155-L190,L193-L265` | `dsl-runtime` | G4 | INVOKE as validator. | `ob-poc-control-plane::dag_proof` | **PARTIALLY CLOSED** — T2.2: same adapter as C-025 (they share one gate); `check_transition`'s `Vec<GateViolation>` is not pre-classified by failure kind, so this adapter deliberately does not invent a `IllegalFromState`/`IllegalToState`/etc. split the validator doesn't provide (see module doc). No production call site wired yet. |
| C-027 | Lifecycle `requires_states` precondition is fail-open except true mismatch. | `ob-poc/src/dsl_v2/executor.rs:L2015-L2041,L2048-L2115` | `ob-poc` | G4 | SPLIT; divergent duplicate of C-025/C-026 semantics. | `ob-poc` (`LifecycleGateMode`) / `ob-poc-control-plane::dag_proof` | **PARTIALLY CLOSED** — T0.2 (`80ce7449`): fail-open classes now configurable + always audited (`LifecycleGateMode`, `LifecycleFailOpenClass`). T2.2 (this commit): `DagProofGate` unifies both semantics in one decision — `DagProofInput.lifecycle_fail_open_class`/`lifecycle_gate_mode_fail_closed` are graded alongside `blocking_violations` in the same `decide()`, so a caller that populates both sources gets one adjudicated outcome instead of two independently-diverging checks. No production call site wires either source into the adapter yet. |
| C-028 | DslExecutor rejects durable verbs unless direct durable execution is explicitly allowed. | `ob-poc/src/dsl_v2/executor.rs:L1900-L1989` | `ob-poc` | G8, G10 | INVOKE. | `ob-poc-control-plane::stp_classifier` (G8 portion) | **PARTIALLY CLOSED** — T3.3: `StpClassifierGate`/`classify()` adapter landed + unit tested; `StpClassifierInput.is_durable_verb`/`durable_execution_explicitly_allowed` mirror the DslExecutor rule (durable + not explicitly allowed ⇒ `Rejected`), deterministic by construction (1,000-reevaluation property test). No production call site translates `DslExecutor`'s real durable-verb check into this input yet. G10 portion untouched. |
| C-029 | SemOsVerbOpRegistry startup hard-fails YAML plugin declarations without registered ops. | `ob-poc-web/src/main.rs:L892-L935` | `ob-poc-web` | G1, G9 | INVOKE as deployment guard. | TBD (T2.1/T3.4) | OPEN |
| C-030 | GatePipeline startup is soft-fail and leaves runtime ungated when config/DAG load fails. | `ob-poc-web/src/main.rs:L1400-L1440` | `ob-poc-web` | G4 | MOVE to deployment/admission hard fence. | `ob-poc-web` (`decide_gate_pipeline_startup`) | **CLOSED** — T0.1 (`b73e9cee`): production-fatal on load failure unless `OB_POC_GATES_FAIL_OPEN=1` (WARN banner). |
| C-031 | ObPocVerbExecutor wraps plugin ops in transaction and rolls back on op error. | `ob-poc/src/sem_os_runtime/verb_executor_adapter.rs:L152-L247` | `ob-poc` | G10 | INVOKE. | TBD (T4.1) | OPEN |
| C-032 | CRUD executor executes metadata-driven insert/update/delete/upsert without comparing to a WriteSetProof. | `dsl-runtime/src/crud_executor.rs:L37-L70,L230-L330` | `dsl-runtime` | G7, G14 partial | SPLIT; add attestation wrapper. | `ob-poc-control-plane::write_set_attestation` (mechanism) / `ob-poc::sequencer_tx::PgTransactionScope` (enforcement) | **PARTIALLY CLOSED** — T5: `write_set_attestation::attest()` (pure comparison, `CapturedWrite` vs `WriteSetProof`) landed with a `WriteSetAttestationGate` adapter for `evaluate_shadow` parity (8 unit tests) — but this row's own location field names the actual gap precisely: `crud_executor.rs` is a bare `&PgPool` consumer with **no `TransactionScope` at all**, so no wrapper was added there this tranche, and it remains completely untouched. The real, production-capable enforcement mechanism was instead built at the one chokepoint the plan's own architecture note ("the sole plugin-verb execution contract," CLAUDE.md) actually owns transactions: `PgTransactionScope::{record_write, set_expected_write_set, commit_attested}` (`sequencer_tx.rs`) — live-DB fault-injection proven (3 `#[ignore]`-gated tests, including the exit-criterion test: a real INSERT into an undeclared table is captured, attested, and the whole transaction rolls back with zero durable rows). `commit_attested` is additive alongside the untouched `commit()`, and persists an audit row to `"ob-poc".control_plane_write_attestations` (migration `20260710_control_plane_write_attestations.sql`) regardless of outcome. **No `SemOsVerbOp` implementation calls `record_write`/`commit_attested` in production** — every real plugin verb still calls the plain `commit()` path unchanged; this tranche proves the mechanism, not deployment. `crud_executor.rs`'s bare-`PgPool` gap (this row's original location) is unstarted follow-on work, not silently absorbed into the `sequencer_tx.rs` win. |
| C-033 | Bus handler enforces catalogue version equality. | `ob-poc-bus-handler/src/lib.rs:L108-L145` | `ob-poc-bus-handler` | G12 | INVOKE. | `ob-poc-control-plane::snapshot::PinnedVersionSet` | **PARTIALLY CLOSED** — T4.4: `PinnedVersionSet.bus_catalogue_version` folds C-033's check into the unified G12 version block on `SnapshotPins`, alongside `compiler_version`/`model_version`/`prompt_version` (none of which had any prior pin at all). No call site wires the bus handler's real `InvocationContext.catalogue_version` into it yet — `ob-poc-bus-handler` remains the enforcement point unchanged. |
| C-034 | Bus adapter calls executor with `Principal::system()` and no runbook/envelope gates. | `ob-poc-web/src/bus_runtime.rs:L122-L149` | `ob-poc-web` | G5, G10 gap | MOVE into envelope admission; bypass candidate. | `ob-poc-web::bus_runtime::ObPocVerbAdapter` (mechanism) | **PARTIALLY CLOSED** — T6.1: `ObPocVerbAdapter::execute` now calls `VerbExecutionPort::execute_verb_admitting_envelope` (the T4.1 admission entry point) instead of the bare `execute_verb` — the bus is now the first production caller of that mechanism, closing the "no envelope check reachable at all" half of this row. `Principal::system()` replaced with `Principal::in_process("bus-federated", vec!["bus".into()])` so bus-originated actions are attributable and no longer carry the `admin` role by default. **Not done**: plan §0 assumption A1 asks for "enforce mode on from day one for bus (it has no legacy users to shadow-compare)" — verified false: bpmn-lite is a real, live production bus caller today and nothing issues it a sealed `ExecutionEnvelope`, so defaulting bus to enforce would reject every live bus verb call immediately. `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` stays unset by production default for bus exactly as for every other path (`NotEnforced` — zero behaviour change from the pre-T6.1 direct-`execute_verb` call). **Flagged for architect confirmation, not decided unilaterally by this tranche**: A1's "day one enforce" premise needs either (a) explicit sign-off to accept a hard cutover that will reject bus callers until they're updated to obtain envelopes first, or (b) revision of A1 to shadow-first like every other path. |
| C-035 | Workflow dispatcher validates binding/process key before BPMN orchestration. | `ob-poc/src/bpmn_integration/dispatcher.rs:L246-L273` | `ob-poc` | G1, G9 | INVOKE. | TBD (T2.1/T3.4) | OPEN |
| C-036 | Workflow dispatcher persists/queues orchestration state and tolerates some persistence failures as logs. | `ob-poc/src/bpmn_integration/dispatcher.rs:L275-L360` | `ob-poc` | G9, G10, G11 | SPLIT. | TBD (T3.4/T4.1/T7.1) | OPEN |
| C-037 | BPMN controller op writes in `pre_fetch` before transaction-scoped `execute`. | `ob-poc/src/domain_ops/bpmn_controller_ops.rs:L283-L320` | `ob-poc` | G10, G14 gap | RETIRE/SPLIT; move write into scoped execute. | `ob-poc` (`BpmnControllerStartInstance::execute`) | **CLOSED** — T0.4 (`352b52ec`): write moved from `pre_fetch` into transaction-scoped `execute`. |
| C-038 | BPMN controller validates tenant/template/idempotency before inserting process instance. | `bpmn-controller/src/instance.rs:L100-L190` | `bpmn-controller` | G2, G10 | INVOKE external surface only. | TBD (T3.1/T4.1) | OPEN |
| C-039 | Entity relationship upsert requires percentage for ownership and applies expected-version CAS except carved-out edge classes. | `sem_os_postgres/src/ops/entity_relationship.rs:L18-L34,L96-L160,L187-L215` | `sem_os_postgres` | G10, G13 | INVOKE. | TBD (T4.1/T3.2) | OPEN |
| C-040 | KYC lexicon entries declare governing taxonomy, writes, authority, preconditions, emits, and content hash. | `ob-poc-kyc-substrate/src/lexicon.rs:L1-L6,L118-L137` | `ob-poc-kyc-substrate` | G1, G5, G6, G12 | INVOKE. | TBD (T2.1/T2.4/T2.5/T4.4) | OPEN — no adapter reads `LexiconEntry` fields directly yet; `EvidenceGate` (T2.5) consumes precondition *outcomes*, not the lexicon declarations themselves. |
| C-041 | KYC control preconditions enforce evidence-before-verify and reconcile/strategy before fold/freeze. | `ob-poc-kyc-substrate/src/fold/control.rs:L438-L478` | `ob-poc-kyc-substrate` | G4, G6 | INVOKE. | `ob-poc-control-plane::evidence_gate` | **PARTIALLY CLOSED** — T2.5: `EvidenceGate` adapter landed + unit tested; `EvidenceInput.kyc_precondition_failures` mirrors `check_control_preconditions`'s failure classes (`EvidenceNotCited`→`MissingRequiredEvidence`, `NotReconciled`→`ConflictingEvidence`, `StrategyNotSelected`→`MissingRequiredEvidence`) — `ob-poc-kyc-substrate` stays the owner of precondition semantics, this module only grades an already-evaluated result. No production call site wires `check_control_preconditions`'s real output into it yet. |
| C-042 | KYC store checks preconditions under stream lock before append and sequence bump. | `ob-poc-kyc-store/src/store.rs:L118-L203` | `ob-poc-kyc-store` | G6, G10, G13 | INVOKE. | `ob-poc-control-plane::evidence_gate` | **PARTIALLY CLOSED** — T2.5: same adapter as C-041; the store-side re-check under stream lock is the production enforcement point (unchanged, still `ob-poc-kyc-store`-owned per K-14/K-42) — this gate is a pre-execution *shadow* observation, not a replacement for the lock-scoped re-check. No production call site wired yet. |
| C-043 | KYC manifest publish persists content-addressed manifest and stamps per-verb lexicon hashes. | `ob-poc-kyc-store/src/manifest.rs:L1-L7,L57-L95` | `ob-poc-kyc-store` | G12 | INVOKE. | TBD (T4.4) | OPEN |
| C-044 | Shadow GatedVerbEnvelope construction records placeholders and explicitly does not gate dispatch. | `ob-poc-boundary/src/envelope_builder.rs:L1-L18`, `ob-poc/src/sequencer.rs:L7721-L7763` | `ob-poc-boundary` / `ob-poc` | G10, G13 partial | MOVE to production envelope admission. | `ob-poc-control-plane` (shadow) / `VerbExecutionPort::execute_verb_admitting_envelope` (mechanism) | **PARTIALLY CLOSED** — T2.7: `phase5_runtime_recheck` additionally calls `ob_poc_control_plane::evaluate_shadow`, persisting a `control_plane_shadow_decisions` row comparing shadow vs legacy outcome; dispatch is not gated by it. T4.1 (this commit): the admission *mechanism* now exists for real — `VerbExecutionPort` gained `execute_verb_admitting_envelope` (default impl degrades to legacy `execute_verb`, so every existing implementor is unchanged), `ObPocVerbExecutor::admit()` checks `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` (empty/unset by production default — nothing enforced) and, for a listed verb, calls `try_consume_by_id` against `control_plane_envelopes` (T4.2). Proven end-to-end against a live DB (`t4_1_envelope_admission_tests`) without flipping any real verb into the enforced set — no path has accumulated the plan's own graduation evidence yet (§0: ≥500 shadow evaluations, zero divergence), so none graduates this tranche. `GatedVerbEnvelope`/`envelope_builder.rs` (the earlier, Phase-0b envelope design) remain untouched, still shadow-only, still not subsumed/deleted — that half of C-044's disposition is unstarted. |
| C-045 | TOCTOU recheck scaffold can recompute row-version hash but is not production-wired. | `ob-poc-boundary/src/toctou_recheck.rs:L1-L34,L81-L139` | `ob-poc-boundary` | G10, G13 | MOVE/finish. | `ob-poc-boundary::toctou_recheck::verify_pins` | **PARTIALLY CLOSED** — T4.3: `verify_pins` compares `SnapshotPins.entity_row_version` against live DB state via the existing `RowVersionProvider`/`SqlRowVersionProvider`, live-DB-proven against a real `"ob-poc".cbus` row (`db_integration_tests::verify_pins_against_real_cbu_row_version`) — the row-version migration (`20260422_row_version_entity_tables.sql`) is confirmed already applied to the reference schema (not merely "staged" as this module's own pre-T4.3 doc comment claimed). `verify_toctou` (the original `GatedVerbEnvelope`-shaped comparison) is untouched and still has zero production call sites; `verify_pins` is the new, `ExecutionEnvelope`-aligned comparison surface — no production admission point calls either yet. |

## Running totals

- Opening balance (Phase 0 / RR-3): 45 rows. MOVE 11, INVOKE 19, RETIRE 5, SPLIT 10.
- Closed by T0: **3** (C-022, C-030, C-037 — full closure).
- Partially closed by T0: **1** (C-027 — fail-open behaviour fixed; semantics-unification half still owed to T2.2).
- Partially closed by T2 (adapters landed + unit tested over their wrapped validator; production call-site wiring beyond T2.7's one path remains open — see plan §"Sonnet execution notes": adapters-only is the tranche's own scope, not full migration): **13** (C-006, C-007 [G1 portion], C-008, C-015, C-016, C-019, C-025, C-026, C-027 [supersedes T0's note — both halves now unified in `DagProofGate`], C-041, C-042, C-044).
- Partially closed by T3 (same posture — adapter landed + unit tested, no production call site): **1** (C-028, G8 portion).
- Partially closed by T4: **3** (C-033 [G12 pin unified, bus handler untouched], C-044 [envelope-admission mechanism now real and live-DB-proven, but no path graduated to enforce], C-045 [`verify_pins` live-DB-proven against real row_version, no production admission call site]).
- Partially closed by T5: **1** (C-032 — attestation mechanism landed at `PgTransactionScope`, live-DB fault-injection proven; `crud_executor.rs`'s original bare-`PgPool` gap remains untouched; no `SemOsVerbOp` calls the new mechanism in production).
- Closed by T6: **1** (C-001 — verified already closed by pre-existing Slice 3.1 work, not new T6 code; ledger simply hadn't caught up).
- Partially closed by T6: **1** (C-034 — bus path now calls `execute_verb_admitting_envelope` with an attributable principal; enforce-by-default for bus explicitly NOT flipped, flagged for architect confirmation of plan assumption A1).
- Reclassified, still open (T6 found the RETIRE disposition is now wrong given C-001's closure, not touched further): **2** (C-017, C-018 — now load-bearing validation for the run-sheet route, not duplicate bypass checks).
- Open, untouched: 24 (C-002..005, C-009..014, C-020..024, C-029, C-031, C-035..040, C-043) — note this range still nominally includes C-022 and C-037, which the "Closed by T0" line above already marks fully closed; the pre-T6 ledger's own running-total arithmetic had this same inconsistency (its stated "23" didn't match its own listed range either), so this is a pre-existing bookkeeping gap in the totals summary, not one introduced this tranche. Not reconciled here — flagged rather than silently corrected with unverified confidence.

T2 exit-criteria status: all six adapters (G1, G3, G4, G5, G6, G7) landed with unit tests against fixtures mirroring their wrapped validators' semantics (57 new/changed tests across `ob-poc-control-plane` + 3 in `ob-poc::agent::control_plane_shadow`); `evaluate_shadow()` composes all six over the T1.3 collect-where-independent evaluator; T2.7 wires a real, production shadow call site (Sequencer stage 6 `phase5_runtime_recheck`) persisting `"ob-poc".control_plane_shadow_decisions` (migration `20260710_control_plane_shadow_decisions.sql`) with divergence-vs-legacy tracking — proving "shadow decisions visible for Path A end-to-end." Legacy dispatch outcome (`phase5_recheck_failure`) is unchanged (`zero behaviour change on legacy path` — the shadow call is additive and spawned into a detached task). The five gates *not* production-wired this tranche (G3/G4/G5/G6/G7's real validator inputs) remain fully stubbed at every call site except the direct unit-test fixtures — an honest, incremental state consistent with "NO logic moves yet — adapters only," not a claim of full C-0xx closure.

T3 exit-criteria status (missing gates, RR-8 "no full production analogue" — G2, G8, G13, plus the T3.4 `ControlPlaneProof` aggregate): G2 (`EntityBindingGate`), G13 (`DecisionSnapshotGate`), and G8 (`StpClassifierGate`/`classify()`) landed with real (not stubbed) grading logic and 23 new unit tests; `evaluate_shadow()` now runs 9 real gates (G1-G8, G13) — `fully_admitted_context_succeeds_through_g8` proves the whole chain completes for the first time (T2 could only ever show G1 succeeding in isolation, since G2 didn't exist to unblock G3/G4/G6). `ControlPlaneProof` (T3.4) expanded from a placeholder single-field struct to the full 9-proof aggregate `ExecutionEnvelope::seal` already required, matching what a `RequiresHumanGate` reviewer needs to see. Property test (**done**, code-level): `same_context_reevaluates_identically_one_thousand_times` proves determinism for both a fully-succeeding and a maximally-blocked context — every gate here is a pure function of `ctx`, so this is a genuine proof, not a tautology dressed up as a test. Replay test (**done**, code-level): `serialized_context_replays_to_the_identical_report` round-trips a full `EvaluationContext` through `serde_json` and re-evaluates to a byte-identical `EvaluationReport` — `EvaluationContext` and every `*Input` type now derive `Serialize`/`Deserialize` for exactly this purpose. Shadow divergence dashboard (**not done, explicitly deferred**): this is a visualisation/observability surface, not a crate-level guarantee a single coding session can honestly claim — `control_plane_shadow_decisions` (T2.7) has the `diverged`/`legacy_outcome_blocked` columns a dashboard would query, but no dashboard exists; recorded here rather than silently omitted. No production call site was added this tranche for G2/G8/G13's inputs (same posture as T2's un-wired gates) — `EntityBindingInput` being real now means, once a real production call site supplies it, G3/G4/G6 (which all declare `EntityBinding` as a dependency) would stop reporting `NotEvaluated` in shadow — that wiring is the natural T4 follow-on, not claimed here.

T4 exit-criteria status (envelope production wiring, G10/G12) — this tranche is materially different from T1-T3: it is the first one that touches a real dispatch admission point, so every piece was scoped to be genuinely production-capable while keeping every actual dispatch path shadow/legacy by default (§0's shadow-first strategy: a gate graduates to enforce only after ≥500 production shadow evaluations with zero divergence — nothing has accumulated that evidence yet, T2.7 shadow wiring only just landed). **T4.2** (envelope persistence, single-use, TTL): `"ob-poc".control_plane_envelopes` (migration `20260710_control_plane_envelopes.sql`) + `agent::control_plane_envelope_store::{persist_sealed, try_consume, void}`, live-DB-proven (4 `#[ignore]`-gated tests: consumed resubmission rejected, expired rejected, unknown handle not-found, voided cannot be consumed — exactly the exit criterion's first two clauses). Required a new `test-support` Cargo feature on `ob-poc-control-plane` (off by default) because `ExecutionEnvelope::seal` is intentionally `pub(crate)`-only — respecting that tollgate rather than routing around it meant `ob-poc`'s own tests needed a legitimate, feature-gated way to obtain a real sealed envelope; every `*_gate` module's `tests_support` bridge was widened from `cfg(test)` to `cfg(any(test, feature = "test-support"))` for the same reason. **T4.3** (pre-state pinning): `ob-poc-boundary::toctou_recheck::verify_pins` compares `SnapshotPins` (not `GatedVerbEnvelope` — see C-045) against live `row_version` via the existing `RowVersionProvider`; live-DB-proven against a real `"ob-poc".cbus` row, which also surfaced that the row-version migration is already applied to the reference schema despite the module's own doc comment calling it "staged, pending operator approval" (stale comment, not corrected this tranche — flagged here). **T4.4** (G12 pinned version set): `SnapshotPins`/`SnapshotInput` gained a `PinnedVersionSet` field (`bus_catalogue_version`, `compiler_version`, `model_version`, `prompt_version`) — the first three subsystems the plan named had no prior pin *at all* (compiler version = `CARGO_PKG_VERSION`, the closest existing proxy; model/prompt version = the `IntentEventRow.prompt_version` field that's hardcoded `None` at its only construction site today). **T4.1** (envelope admission at the port) is the tranche's central piece: `VerbExecutionPort` gained `execute_verb_admitting_envelope` (default impl degrades to legacy `execute_verb` — every existing implementor, including `MockVerbExecutor`, is behaviourally unchanged unless it overrides), carrying a bare `Uuid` rather than `EnvelopeHandle` so `dsl-runtime` (the pure execution-tier contract crate) needn't depend on `ob-poc-control-plane` — an explicit trade-off: `ObPocVerbExecutor::admit()` can therefore only do an id-only consume (`try_consume_by_id`, no content-hash cross-check) at this boundary, weaker than `try_consume`'s full guarantee; recorded, not hidden. `admit()` checks `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` (empty/unset — the production default — enforces nothing) via `EnforcedVerbs::from_env()`, read fresh per call. Exit criterion "enforce-mode Path A green end-to-end" is satisfied as a *mechanism* proof, not a deployment: `t4_1_envelope_admission_tests` (live DB) sets the env var to `cbu.confirm` entirely within its own process-local scope, proves an unconsumed sealed envelope admits, a resubmission is rejected, and an unlisted verb is unaffected — then restores the unset default on drop. No real verb was added to the enforced set outside that test; "shadow mode still default for untouched paths" holds for literally every path, because none has graduated. Net: the mechanism this exit criterion asks for exists and is proven; the *deployment* decision to enforce any specific path is explicitly and deliberately not made by this tranche's code.

T5 exit-criteria status (write-set attestation, G14): the plan's literal text describes instrumenting "the CRUD executor and the plugin-op transaction wrapper" to record touched `(table, entity_id, columns)` into a per-transaction `WriteCapture`. Research this tranche found that faithfully covering both named sites is out of single-session scope: `crud_executor.rs` operates on a bare `&PgPool` with no `TransactionScope` participation at all (no interception point exists today without a larger refactor), and the plugin-op wrapper (`ObPocVerbExecutor`) dispatches through ~40+ scattered call sites that each call `sqlx::query(...).execute(scope.executor())` directly with no shared capture hook. The tranche was scoped instead to the one architecturally-primary chokepoint the codebase already treats as authoritative — `PgTransactionScope`, documented in CLAUDE.md as "the sole plugin-verb execution contract" — and built genuinely production-capable capture/attestation/persistence there: `record_write()`/`set_expected_write_set()` accumulate state on the scope, `commit_attested()` runs `write_set_attestation::attest()` against the accumulated `Vec<CapturedWrite>` and either commits + persists an audit row or **rolls back the whole transaction and persists the breach** — proven, not asserted, via a real live-DB fault-injection test (`write_to_undeclared_table_aborts_with_no_durable_row`): a genuine SQL `INSERT` into an undeclared table succeeds at the statement level, is captured, is judged a breach, and the assertion is that the table has **zero** durable rows afterward — this is the exit criterion verbatim, not a proxy for it. `commit()` (the pre-existing, unattested path) is completely unchanged, so every current caller is unaffected; `commit_attested` is purely additive. `GateId::WriteSetAttestation`'s stub was replaced with `WriteSetAttestationGate` in `evaluate_shadow()` for reporting parity with every other gate (8 unit tests: within-bound, undeclared-table breach, undeclared-column breach, undeclared-entity breach, breach collects every excess write not just the first, gate success/failure/fail-closed-on-missing-input). `write_set::decide()` was widened from private to `pub` — the one API surface change in this tranche that isn't purely additive — because production code needs a real `WriteSetProof` value to attach via `set_expected_write_set`, and `WriteSetProof` (unlike `ExecutionEnvelope`) has no predecessor-gate enforcement requirement that a public constructor would weaken. **Net, honestly stated**: the mechanism is real, tested against a live database, and enforces (rolls back) when invoked — but **no `SemOsVerbOp` implementation calls `record_write`/`set_expected_write_set`/`commit_attested` in production yet**; every real plugin verb still commits via the plain, unattested `commit()`. C-032's location field (`crud_executor.rs`) names a gap this tranche did not close — that gap is unstarted, explicitly not glossed over by landing the mechanism at a different (correct, but different) call site.

T6 exit-criteria status (bypass closure, the Phase 0 hit-list): the plan names five sub-tranches (T6.1-T6.5). This session closed/advanced two, verified one was already closed, and explicitly declined the remaining two rather than fake them — the reasoning for each follows. **T6.1** (bus path, C-034): `ob-poc-web::bus_runtime::ObPocVerbAdapter::execute` now calls `execute_verb_admitting_envelope` (T4.1's admission entry point) instead of the bare `execute_verb`, and uses `Principal::in_process("bus-federated", vec!["bus".into()])` instead of `Principal::system()`. This is the first production call site for that mechanism outside its own unit tests — genuinely closes "no envelope check reachable at all" for the bus path. Plan §0 assumption A1 says bus should default to **enforce** mode "from day one... it has no legacy users to shadow-compare" — checked against the actual bus consumer (bpmn-lite, a real production caller documented in `bus_runtime.rs`'s own module doc) and found that assumption false: nothing issues bus callers a sealed envelope, so defaulting to enforce would hard-reject every live bus verb call today. Kept shadow-first (`OB_POC_CONTROL_PLANE_ENFORCE_VERBS` unset — `NotEnforced`, zero behaviour change) instead of following A1 literally, and flagged the contradiction in the ledger (C-034 row) rather than either silently overriding the architect's stated assumption or blindly implementing something that would break production. **T6.2** (raw DSL, C-001/C-017/C-018): verification, not new code — re-reading `agent_routes.rs` found the raw-DSL-body bypass this row describes was already closed by the pre-existing Slice 3.1 fix (`OBPOC_ALLOW_RAW_EXECUTE` deleted 2026-04-22); C-001 is marked CLOSED on that basis. That finding also invalidated C-017/C-018's RETIRE disposition — they're no longer "duplicate checks in a doomed bypass," they're the only validation for whatever DSL the run-sheet route actually executes now; reclassified rather than retired, left OPEN for a future INVOKE-style wiring tranche. A3's remaining literal ask (route the endpoint behind a `raw-dsl-dev` cargo feature so production builds exclude it entirely) was **not done**: this session could not establish with confidence whether `POST /api/session/:id/execute` has zero live callers beyond the raw-body path that's already closed (the handler's own routing suggests it may still serve a legitimate "run the staged run-sheet" fallback for callers outside the unified `/input` pipeline) — deleting a live HTTP route without that certainty is exactly the kind of hard-to-reverse, blast-radius action this project's operating discipline requires confirming first, not inferring. **T6.3** (RealDslExecutor + WorkflowDispatcher direct branch, RR-6 rows 3-4): investigated, not implemented. Both call sites turn out to funnel into the same underlying engine — `crate::dsl_v2::executor::DslExecutor::execute_plan`/`execute_plan_atomic_in_scope` (`repl/executor_bridge.rs`, `bpmn_integration/dispatcher.rs`) — which is a different, lower-level executor than `ObPocVerbExecutor`/`VerbExecutionPort` (the T4.1/T6.1 admission mechanism's home); it iterates verbs internally with no per-verb hook exposed to a caller. Wiring admission in here means either instrumenting inside that shared legacy engine (touching a hot path shared with the BPMN-lite integration and every runbook execution) or rerouting these two callers through `ObPocVerbExecutor` instead (a behavioural risk to a live orchestration path) — both are materially riskier and larger than a single-call-site swap, and neither was attempted this session rather than force a shortcut. **T6.4** (bus target state, envelope-only) and **T6.5** (enforce-mode graduation): not attempted — both are gated on prerequisites this plan's own text already names as unmet. T6.4 requires bus callers to be able to obtain envelopes first (no such caller-side flow exists yet for bpmn-lite). T6.5 requires "≥500 production shadow evaluations with zero divergence" per plan §0 — as of this tranche, zero production shadow evaluations have accumulated at all (T2.7's `control_plane_shadow_decisions` wiring only just landed in T2, with no observed production traffic reported back to this ledger) — flipping any path to enforce now would be fabricating graduation evidence that doesn't exist, not "faking it" in code but in the underlying safety claim, so it wasn't done. Validation this tranche: `cargo build -p ob-poc-web` clean, `cargo clippy -p ob-poc-web` clean (same 5 pre-existing `ob-poc` warnings as T5, none new), `cargo test -p ob-poc-web` (5/5 green, unaffected by T6.1's change since none exercise the bus adapter).

T7 exit-criteria status (assurance plane, V&S Phase 5) — **plan NOT completed this tranche; the "T7 COMPLETE — progress 100%" banner is deliberately not claimed.** T7 surfaced a blocker outside this session's control: the plan's own basis document, `EOP-VS-CONTROLPLANE-001 v0.3`, does not exist anywhere in this repository — every tranche has cited its §-numbers (§6.14, §6.7.1, §9.1-9.4, §12, Appendix A) as if the source were available, but a repo-wide search found no such file. This was invisible until T7.4, which asks to update "the NIST crosswalk doc... from design-crosswalk toward operating status" — there is no crosswalk doc to update, and T7's stated exit criterion ("V&S §12 success criteria walked and evidenced one by one in a closing report") cannot be honestly satisfied without that document to walk. **This is flagged to the architect per the plan's own Sonnet execution notes ("if a criterion is wrong, STOP and flag"), not silently worked around.** **T7.2** (metrics) was implemented for real: `agent::control_plane_metrics` (`gate_outcome_counts`, `shadow_divergence_stats`, `write_attestation_breach_stats`, `envelope_status_counts`) — four typed, read-only queries over the three existing `control_plane_*` tables (T2.7/T4.2/T5.3), each covered by a live-DB test (4 `#[ignore]`-gated tests, all green), and wired to a genuine production caller: `GET /api/control-plane/metrics` (`agent_routes.rs`) — this is why the functions don't trip `dead_code`, unlike T5/T6's mechanism-only additions. "Exception ageing" and "replay success" (also named in V&S §6.14) are deliberately omitted: no exception-tracking table and no decision-replay job exist (T7.3, next). **T7.1** (unified `control_plane_audit` stream) and **T7.3** (decision-replay job): not attempted. Investigated first: the three source tables (`control_plane_shadow_decisions`, `control_plane_envelopes`, `control_plane_write_attestations`) share only `session_id`/`verb_fqn` — no strict correlation key — and, more fundamentally, nothing in production currently populates all three for the same real dispatch (shadow decisions run from Sequencer's `phase5_runtime_recheck`; write attestations have zero production callers per T5's own finding; envelope admission's `envelope_id` is always `None` in production per T4.1/T6.1). Building a "unified stream" to link three things that don't co-occur in production yet would either be a schema exercise with nothing real to join, or would require fabricating cross-references — declined. T7.3's replay job is gated on T7.1 (nothing to sample) and on completion invariant E3 ("G1-G14 each evaluated in production... with metrics flowing"), which T7.2's own metrics module can now measure and confirm is still false (`gate_outcome_counts` would show `NotImplemented`/`NotEvaluated` dominating any real production sample, since most gates still have no production input source per T2/T3's own honest notes) — not attempted. **Plan completion invariants (E1-E5), checked honestly rather than asserted**: E1 (every C-0xx CLOSED) — false, ~24 rows remain open/reclassified per the running totals above. E2 (all four RR-2 paths execute only via envelope admission in enforce mode) — false, every path that now calls `execute_verb_admitting_envelope` (bus, T6.1) does so with enforce off by production default; zero paths are in enforce mode. E3 (G1-G14 each evaluated in production with metrics flowing) — false for most gates; T7.2 makes this honestly *measurable* for the first time (`gate_outcome_counts`/`shadow_divergence_stats` would show it), which is different from making it true. E4 (Mode-1 register version-pinned or human-gated) — not verified this tranche. E5 (workspace green, public-API gate green, `unreachable_pub` clean) — the subset touched this session is green (`cargo build`/`clippy`/`test` clean across `ob-poc`, `ob-poc-web`), but a full-workspace sweep was not re-run. **Recommendation to the architect, not a decision made here**: either (a) locate/attach the missing `EOP-VS-CONTROLPLANE-001` source document so T7.1/T7.4's literal asks become answerable, or (b) explicitly descope T7.1/T7.3/T7.4 from this plan's completion bar and redefine "done" as T7.2's metrics-only assurance plane plus the honest E1-E5 gap list above. Validation this tranche: `cargo build -p ob-poc --lib --features database` clean, `cargo clippy -p ob-poc --lib --features database` (same 5 pre-existing warnings as T5/T6, none new), `cargo test -p ob-poc --lib --features database` (2265/0, 173 ignored incl. the 4 new T7.2 tests), 4 live-DB T7.2 tests green, `cargo x reconcile validate` (OK), KYC substrate dep-gate (PASS). No schema/migration change this tranche (T7.2 is read-only over existing tables) and no `ob-poc-control-plane`/`dsl-runtime`/`ob-poc-boundary` public API change, so no baseline refresh was needed.

T6.1a addendum (2026-07-10, post-review, research only — no code change): the architect asked the concrete precondition question "can bpmn-lite process variables carry an opaque string today?" for C-034's remaining gap (threading `EnvelopeHandle` through a bus-dispatched BPMN process). Answer: **not via `Value`/flags** (`bpmn-lite-types/src/types.rs:46-53` — `Bool | I64 | Str(interned-id) | Ref(index)`; `Str` is compile-time-interned with no runtime string pool, so an arbitrary runtime UUID cannot go there) — **but yes via `ProcessInstance.domain_payload`** (opaque canonical JSON, BLAKE3-hashed on write, never parsed by the VM), which is compiler-routed to by construction for any `String`-typed BPMN data object (`bpmn-lite-types/src/ffi_bindings.rs:19-58`), with existing end-to-end precedent in the engine (set at process start `bpmn-lite-engine/src/engine.rs:420-421`, read/written via dotted-path JSON accessors `bpmn-lite-vm/src/json_path.rs`). No size/format constraint beyond valid JSON. This resolves the mechanism question for T6.1a; the implementation itself (declaring the data object, writing it at envelope issuance, reading it back at the bus-dispatch task, wiring `ObPocVerbAdapter` to pass the real handle through instead of the current hardcoded `None`) is not yet done — tracked as a precondition in the graduation runbook (`docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` §4 Path D).

T7 re-evidence session (2026-07-10, post-doc-commit, per architect direction) — `EOP-VS-CONTROLPLANE-001 v0.3` is now in the repo (`docs/architecture/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.3.md`; path corrected 2026-07-11, T9.4 — this line originally cited `docs/todo/control-plane/...`, its location before the T9.4 relocation); re-checking T7.1/T7.3/T7.4 against its actual text rather than the earlier "no source document" placeholder. **No new enforcement code, no schema change — this is re-evidencing, not re-scoping.**

**T7.4 regression suite — reclassified CLOSED, not open.** §13 Phase 5 lists "Control-plane regression suite" as a discrete deliverable, and §12 criterion 11 requires "replay of the decision reproduces the decision." Both are already satisfied, landed in T3, previously undercredited because this ledger had no source doc to check them against: `ob-poc-control-plane::lib.rs::tests::same_context_reevaluates_identically_one_thousand_times` (the regression suite — 1,000-iteration determinism property test) and `tests::serialized_context_replays_to_the_identical_report` (the decision-level replay proof — round-trips a full `EvaluationContext` through `serde_json`, re-evaluates, byte-identical `EvaluationReport`). This closes the "regression suite" line item of T7.4 outright. It does **not** close T7.3 (see below) — §12 criterion 11's *decision* replay is proven; nothing has yet built the *job* that samples real persisted production decisions and replays them (a different, larger thing, per V&S's own distinction between "decision replay" as a capability (§6.10.2, done) and a "decision-replay job" as an operational Phase-5 deliverable (§13, not done)).

**T7.1 unified audit stream — blocker confirmed precise, not resolved.** §6.11 specifies the exact field set a unified audit record must connect: utterance/trigger, interpreted intent, selected verb, bound entities, active pack, DAG/state proof, authority decision, evidence decision, write-set, write-set attestation result, execution result, pre/post-state, model/prompt version, interpretation attestation, human approval, correlation IDs, decision snapshot pins, timestamps, exceptions. Checked against what's actually persisted today: `control_plane_shadow_decisions` carries `gate_results` (a rendered `EvaluationReport`, G1 only in practice — see T6's precondition finding) but not the full `EvaluationContext`, not write-set attestation results, not pre/post-state. `control_plane_envelopes` carries identity/lifecycle only, no gate content. `control_plane_write_attestations` carries captured/excess writes only. None of the three, individually or joined, comes close to §6.11's field list, and — the blocker that hasn't changed — nothing populates more than one of them for the same real dispatch in production yet. The doc sharpens the target shape; it does not remove the prerequisite (real concurrent production population across at least admission + write-set + audit) that has to land before a "unified stream" is anything but an empty schema. Not attempted this session.

**T7.3 decision-replay job — blocker confirmed precise, not resolved.** §6.15 and §12 criterion 11 define what must be replayed: `(candidate intent, actor/context, snapshot)` — the full `EvaluationContext`, not just its resulting report. `control_plane_shadow_decisions.gate_results` stores only the rendered *outcome* strings (`report_to_json` in `control_plane_shadow.rs`), never the input `EvaluationContext` that produced them — so today there is nothing a replay job could feed back into `evaluate_shadow()` to reproduce a real, historical production decision. The mechanism is proven at the unit level (T3's two tests above); the gap is specifically "no production call site persists the context, only the report." A future tranche's concrete next step (not started here): add a `context_snapshot jsonb` column to `control_plane_shadow_decisions` alongside `gate_results`, serializing the real `EvaluationContext` `build_evaluation_context` already constructs — that single addition is what would make a real sampling replay job possible. Not attempted this session (schema change, and only worth doing once more of §6.11's inputs are wired at real call sites — otherwise the job would only ever replay G1).

**Appendix A NIST crosswalk — status caveat correctly stays; precise Phase-5 gap list produced instead of a vague "no doc" placeholder.** Appendix A's own text (§A, status caveat) is explicit: the mapping is a "design crosswalk, not an operating attestation" until Phase 5 (§13: metrics, control dashboard, decision-replay checks, drift/version reporting, regression suite) is complete, **and** the crosswalk has been accepted by Risk. Checked against the actual Phase-5 checklist: **metrics** — DONE (T7.2, `agent::control_plane_metrics` + `GET /api/control-plane/metrics`). **Regression suite** — DONE (T3, see above, reclassified this session). **Decision-replay checks** — NOT DONE (T7.3, blocked as above). **Control dashboard** — NOT DONE (no UI; T7.2's endpoint is a data source, not a dashboard). **Drift/version reporting** — PARTIAL (T4.4 landed the pinned version set on `SnapshotPins`, but no reporting/comparison surface exists over it). 2 of 5 Phase-5 deliverables done, 1 partial, 2 not started. This is a materially more honest status than "blocked, no source document" — the caveat in Appendix A is correctly left standing (lifting it requires the remaining 3 items plus a Risk acceptance step this session has no authority to grant), but the gap is now precise and actionable rather than opaque.

**Net effect on plan completion invariants**: E3 ("G1-G14 each evaluated in production... with metrics flowing") gains a real measurement tool (T7.2) but is not thereby made true — `gate_outcome_counts` would still show most gates `NotImplemented`/`NotEvaluated` at every real call site today. No invariant flips to green this session. Validation: no code changed this session beyond the ledger; the two cited T3 tests were re-run to confirm they still pass (`cargo test -p ob-poc-control-plane` — unchanged from T3, still green).

## Independent adversarial review response (2026-07-10)

An authorship-blind adversarial review (`docs/research/control-plane-pir-001.md`, verdict **MERGE-AFTER-BLOCKERS**, 1 BLOCKER / 2 MAJOR / 3 MINOR / 4 NOTE) was run against this branch via a fresh Agent invocation with no memory of the implementation session, per the plan's own "authorship-blind" review posture. Its findings and this repository's response:

- **PIR-D-001 (BLOCKER, public-API surface gate fails)** — CLOSED. Root cause: `ob-poc-control-plane`'s T5 baseline capture omitted the header comment line every other `audits/surface/*.txt` baseline carries; `check-public-api-surface.sh`'s `tail -n +2` comparison then stripped the file's real first API line instead, producing a permanent false trip unrelated to actual API drift. Regenerated in the documented format. The other 4 tripped baselines (`dsl-analysis`, `ob-poc`, `ob-semantic-matcher`, `sem_os_postgres`) were confirmed via `git log` to predate any T0-T7 commit — refreshed as a separate chore commit, not attributed to this plan's C-0xx scope.
- **PIR-D-002 (MAJOR, Path A never reaches the admission port)** — CLOSED. `step_executor_bridge.rs:474` (`VerbExecutionPortStepExecutor::execute_step`) now calls `execute_verb_admitting_envelope` instead of the bare `execute_verb`, mirroring T6.1's bus change exactly (`envelope_id: None`, zero dispatch-outcome change while `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` stays empty by production default). This was already the graduation runbook's documented "Path A Step 0" precondition before the independent review reproduced it independently — the review corroborated, rather than discovered, this specific gap.
- **PIR-D-003 (MAJOR, trybuild compile-fail proofs never run in CI)** — CLOSED. New workflow `.github/workflows/control-plane-proofs.yml` runs `cargo test -p ob-poc-control-plane --all-targets --features test-support` (84 unit tests + both trybuild fixtures) on every push to main and PR.
- **PIR-D-004 (MINOR, T7.2 metrics test-isolation race)** — CLOSED. Fixed before this entry (see the dedicated commit) — 3 of 4 `t7_2_metrics_tests` had before/after delta assertions racing sibling tests on shared tables; widened to monotonic `>=` floors for the two insert-only tables, rewrote the third (mutable `status` column) to verify its own inserted row directly rather than a whole-table delta. Reproduced the reviewer's exact failure scenario 3× post-fix — 12/12 green every run.
- **PIR-D-005 (MINOR, no T6.5 deployment guard / T6's own exit criterion unmet)** — CLOSED, narrowly. T6's literal exit criterion ("grep/CI gate — no call site constructs `VerbExecutionContext` outside control-plane-issued paths, allowlist file, CI-checked") is now real: `audits/surface/_verb-execution-context-allowlist.txt` + `scripts/check-verb-execution-context-allowlist.sh` (wired into `control-plane-proofs.yml`), which brace-depth-tracks every non-test `VerbExecutionContext::new(` construction site in the workspace and fails CI on any unlisted one. This is the CI-gate half of T6's criterion, not the full T6.5 runtime deployment guard the plan describes ("startup asserts every registered dispatch path is envelope-guarded") — that would require a runtime-observable proxy for a source-level property (which construction sites route through admission) that doesn't have an obvious non-fake runtime signal; a CI-time static gate is the honest mechanism for what T6.5 is actually asking, and is what was built. The allowlist currently carries 3 entries: 2 `ADMISSION-WIRED` (bus, Path A) and 1 `KNOWN-BYPASS` (`dsl_v2/executor.rs`'s `dispatch_plugin_via_sem_os_op_in_scope` — the T6.3 gap, still open, now explicitly named and CI-tracked rather than merely discoverable by grep). Verified the gate actually catches new sites, not just passes trivially: during development, an initial line-proximity heuristic incorrectly classified a probe site appended *after* an existing `#[cfg(test)]` module's closing brace as test-only (a real false-negative, caught before shipping) — replaced with brace-depth tracking and re-verified against the same probe, which now correctly fails the gate.
- **PIR-D-006 (MINOR, pre-existing unrelated test failures in `ob-poc-boundary`/`dsl-runtime`)** — NOT touched. The reviewer's own recommendation was "flag separately... out of scope for this plan" — no C-0xx row references these failures, and fixing unrelated ACP/dsl-runtime doctest failures is outside this plan's charter.
- **PIR-D-007 (NOTE, V&S doc location)** — NOT relocated to `docs/architecture/` in this pass; still at `docs/todo/control-plane/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.3.md`, pending the architect's confirmation of the intended final path. **CLOSED in T9.4** (see below) — `git mv`'d to `docs/architecture/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.3.md`.
- **PIR-D-008/009/010 (NOTE, `try_consume_by_id` weaker than `try_consume`; zero `verify_pins` production call sites; the compounding risk if graduated today)** — NOT touched. These are the T4.1/T4.3 design trade-offs already acknowledged in-code and in this ledger (see the T4 exit-criteria entry above) — closing them means threading a typed `EnvelopeHandle` through the `VerbExecutionPort` boundary and wiring real production `SnapshotPins` construction respectively, both larger, deliberate architecture pieces that were explicitly deferred when T4 landed, not oversights to patch reactively under a review-response pass. Attempting either hastily here risks exactly the "subtle bug becomes a control fiction" failure mode this plan's own review charter was written to catch.

Net: of 10 PIR findings, 5 closed this pass (1 BLOCKER, 2 MAJOR, 2 MINOR), 1 MINOR explicitly out of scope, 4 NOTE items left as documented, deliberate, larger follow-on work rather than rushed.

## Tranche T8 (Addendum A — Graduation Prerequisites, 2026-07-11)

Basis: `docs/todo/control-plane/EOP-PLAN-CONTROLPLANE-001_Addendum-A_Tranche-T8.md`, PIR-6 delta list. Six sub-tranches; three closed, three explicitly deferred with technical reasoning rather than rushed — matching this plan's own "never weaken an exit criterion to pass it" rule.

- **T8.1 (typed `EnvelopeHandle` through the port boundary, closes PIR-D-008/PIR-D-010)** — **CLOSED.** `EnvelopeHandle` moved to `ob-poc-types` (a values-only boundary crate `dsl-runtime` already depended on, with zero execution-tier logic — neither `dsl-runtime` nor `ob-poc-control-plane` now depends on the other's capability logic). `VerbExecutionPort::execute_verb_admitting_envelope` carries the typed handle instead of a bare `Uuid`; `ObPocVerbExecutor::admit()` calls `try_consume` (content-hash checked) instead of `try_consume_by_id`. `try_consume_by_id` demoted to `#[cfg(all(test, feature = "database"))]` — compiler-enforced absence from production builds, stronger than a grep gate. New live-DB test proves the actual guarantee this closes: same id, wrong content hash → `ContentHashMismatch`, rejected loudly, genuine handle still consumable afterward (no poisoning). Every `VerbExecutionPort` implementor relying on the trait's default impl (`MockVerbExecutor`, `HarnessMockExecutor`) needed zero code change, confirmed by building each. Full validation: build/test/clippy clean across every touched crate, public-API surface gate ratchet refreshed for the 4 crates whose signatures genuinely changed (`dsl-runtime`, `ob-poc`, `ob-poc-control-plane`, `ob-poc-types`), catalogue + KYC dep-gates OK.
- **T8.2 (`verify_pins` in the production admission path, closes PIR-D-009)** — **DEFERRED, not attempted.** Investigated first: `SqlRowVersionProvider` reads via a plain `&PgPool` connection, independent of both `admit()`'s own transaction and `execute_verb`'s SemOS-native fast path's own `PgTransactionScope::begin(pool)`. T8.2's actual exit criterion — pin comparison "demonstrably inside/atomically bound to the execution transaction," not a separate earlier one — requires restructuring `ObPocVerbExecutor::execute_verb` (every verb in the system dispatches through this method) so a single transaction scope opens once and covers consumption + pin verification + the write, only then committing. A version that calls `verify_pins` at some convenient point before dispatch, in its own separate transaction, would NOT satisfy this criterion — it would leave exactly the TOCTOU window pin-checking exists to close, and shipping it while claiming closure would be weakening the exit criterion to pass it (explicitly forbidden by the plan's Sonnet execution notes). This is genuine transactional-boundary surgery on the hottest dispatch path in the system, not a signature change like T8.1 — assessed as requiring its own dedicated, focused tranche.
- **T8.3 (Path B/C admission wiring, retires the `KNOWN-BYPASS` ledger line)** — **DEFERRED, not attempted.** This is T6.3, reassessed a third time (T6, the independent PIR review's P3.2 probe, and now here) with the same conclusion each time: `dsl_v2::executor::DslExecutor::execute_verb`/`dispatch_plugin_via_sem_os_op_in_scope` is a large (~600+ line), independently-hot dispatch chain used by `RealDslExecutor` (Path B, REPL/direct) and `WorkflowDispatcher`'s direct branch (Path C), sharing nothing with `ObPocVerbExecutor`. Rerouting either through `execute_verb_admitting_envelope` is a major behavioral change to two live production paths — no new information this session reduces that risk assessment.

  **Update: E-T8.3 CLOSED (2026-07-11, later same day, found during the T9.6 backlog pass, not separately reattempted).** T9.3 (see that tranche's own entry below), landed after this T8.3 deferral was written, solved the identical problem a different way — the "boundary-interposition redesign" (admit at each top-level caller's ingress via `admit_plan()`, before delegating to the unmodified shared `dsl_v2::executor::DslExecutor` engine) rather than instrumenting the shared engine itself, exactly the "major behavioral change to a live production path" this entry declined to risk. T9.3's own commit reclassified `audits/surface/_verb-execution-context-allowlist.txt`'s `src/dsl_v2/executor.rs` line from `KNOWN-BYPASS` to `ADMISSION-WIRED` and states plainly: **"zero `KNOWN-BYPASS` entries remain."** That is T8.3's exit criterion verbatim ("retires the `KNOWN-BYPASS` ledger line") — checked directly against the current allowlist file, confirmed: the line's category field reads `ADMISSION-WIRED` today, not `KNOWN-BYPASS`. Per the allowlist's own header convention ("Removing a KNOWN-BYPASS line means that site is now admission-wired — update the category, don't just delete the line silently"), recategorizing in place *is* retiring the bypass, not a lesser substitute for it. Not a redefinition of the criterion — T9.3 covers strictly more ground than T8.3 asked for (5 ingress points, not just the 2 named here).
- **T8.4 (full-gate shadow coverage on Path A, starts the graduation clock)** — **DEFERRED, not attempted.** `control_plane_shadow.rs::build_evaluation_context`'s own doc comment already names the gap precisely: G3-G7 inputs require "a resolved single pack, a proposed state transition, an `AccessDecision`/TOCTOU result, evidence gaps mapped to obligations, and a contract-derived write set" — none available at the `phase5_runtime_recheck` call site today. Wiring real inputs for even one of these (let alone all five) means integrating with a genuinely different subsystem per gate; bundling all five into one sub-tranche alongside T8.1's already-substantial work risked rushing multiple independent integrations. Not started this session — each gate's real-input wiring is better scoped as its own focused piece of work.
- **T8.5 (T7.2 metrics test isolation, closes PIR-D-004)** — **Already CLOSED prior to this tranche.** T8.5's own text asks to fix "the two before/after delta tests racing sibling inserts" — this was already done in the review-response pass (see PIR-D-004 above) and re-verified fresh at the start of this tranche: `cargo test -p ob-poc --lib --features database -- control_plane --ignored` (no `--test-threads=1`) run 3 consecutive times, 12/12 green every run — exactly T8.5's own literal exit criterion. No new work needed; flagged the addendum's stale reference rather than silently redoing already-complete work.
- **T8.6 (ledger and scope hygiene)** — **PARTIALLY CLOSED.** E5 scope annotation: parent-plan completion invariant E5 ("workspace green: `cargo build && cargo test` all crates") is hereby formally scoped to crates touched by a C-0xx disposition in this ledger. PIR-D-006's pre-existing failures (`ob-poc-boundary`'s `acp_dag_semantic`/`acp_registry_projection` golden-count assertions, `dsl-runtime`'s `state_reducer::state_machine::compute_reducer_revision` doctest) are formally out of E5's scope for this plan — no C-0xx row references them, they predate every tranche in this plan, and per the independent PIR review's own recommendation they should be tracked as a separate workspace-hygiene item, not folded into control-plane completion. **No separate ticket has actually been filed** — this ledger entry is the closest thing to one that exists; a real tracking artifact (GitHub issue or `docs/todo/` entry) is still owed if this project uses either. PIR-D-007 (V&S relocation to `docs/architecture/`): **NOT landed.** The architect requested this relocation twice; both times the actual document content was not received in the message (only a broken/empty file reference). The document remained at `docs/todo/control-plane/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.3.md` at the time this entry was written. All citing documents (this ledger, the implementation plan, the graduation runbook) still referenced that path — updating them to a `docs/architecture/` path before the file had actually moved there would have created dangling references, so this was correctly not attempted speculatively. **Superseded by T9.4** (see below): the relocation needed no new document content — it was already in the repo — so this was closed by `git mv` alone.

**T8 completion invariant status**: E-T8.1 ✅ (zero id-only consume paths reachable in production, compiler-enforced). E-T8.2 ❌ (not attempted — see above). E-T8.3 ❌ (not attempted — `KNOWN-BYPASS` line remains in the allowlist). E-T8.4 ❌ (not attempted — Path A's graduation clock has NOT started; shadow coverage remains G1-only). E-T8.5 ✅ (already satisfied). E-T8.6 partial (E5 scoped, PIR-D-006 named but not separately ticketed, PIR-D-007 explicitly still blocked). **Net: T8 is not complete.** Per the addendum's own closing statement ("on completion, Path A enters its graduation window; the next enforce-mode action is governed entirely by EOP-RB-CONTROLPLANE-001") — that condition is not met. Path A's graduation clock has not started. T8.2/T8.3/T8.4 remain open work for a future, focused tranche each.

**Update (2026-07-11, later same day): E-T8.3 and E-T8.4 both CLOSED since this paragraph was written** — see their own inline "Update" notes above (E-T8.3 via T9.3's boundary-interposition redesign; E-T8.4 via T9.1's G1-G7 shadow wiring). Revised status: E-T8.1 ✅, **E-T8.2 ❌** (confirmed still genuinely open by the T9.6 investigation below — not a stale read, freshly re-checked), **E-T8.3 ✅**, **E-T8.4 ✅** (code-readiness only — the graduation clock itself hasn't started running until this branch merges/deploys; see E-T8.4's own caveat), E-T8.5 ✅, E-T8.6 partial (unchanged). **Net: T8's own literal completion invariant is now satisfied except E-T8.2 and the two named E-T8.6 hygiene items** — closer to complete than "T8 is not complete" above suggests, but the addendum's own closing condition ("Path A enters its graduation window") still isn't met: that requires production shadow-evaluation volume (T6.5), not more code, and E-T8.2's pin-atomicity gap is real, unresolved risk, not paperwork.

**Update: E-T8.4 CLOSED (2026-07-11, later same day)** — T9.1's closure (G1–G7 real inputs at `phase5_runtime_recheck`, the exact gap this criterion named) satisfies T8.4 mechanically, not by redefinition: `report_to_json` (`control_plane_shadow.rs`) already serialises all `GateId::ALL` outcomes (not just the ones with real input) into the `gate_results` JSONB column on every dispatch, so every real Path A dispatch now persists genuine G1–G7 outcomes to `control_plane_shadow_decisions` with zero additional code — `t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch` (`0e2e5ec7`) is the direct proof. **Caveat, not glossed over:** "starts the graduation clock" is a production-traffic fact, not a code fact — the clock starts ticking only once this code is actually deployed and serving real Path A dispatches, and as of this entry the branch (`codex/phase-1-5-governance-closure`) is **not merged to main**. E-T8.4 is closed in the sense that the code no longer blocks the clock from starting; it does not mean the clock has started. T6.5's "≥500 shadow evaluations, zero divergence" threshold still requires elapsed production time after merge/deploy, not more code.

## Tranche T9.4 (Addendum B — owed hygiene, 2026-07-11)

Both items T8.6 left as "still owed" are now closed.

- **PIR-D-007 — CLOSED.** The V&S document was already in the repo (had been since the T7 re-evidence session) — the only remaining work was moving it, not obtaining it. `git mv docs/todo/control-plane/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.3.md docs/architecture/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.3.md` (history preserved as a rename). Citing paths updated: the graduation runbook's §8 open-items line (was "missing V&S source doc," now marked resolved with the new path). This ledger's own historical narrative lines (the T7 re-evidence session's opening sentence, T8.6's own "NOT landed" entry) had their path citations corrected in place (with an inline note on what the path was when originally written) rather than left dangling — T9.4's own exit criterion is "zero citations of the old path outside the PIR," and this ledger is not the PIR. `control-plane-pir-001.md` is the sole named exception and was left untouched, per this addendum's explicit instruction.
- **PIR-D-006 — CLOSED (ticketed).** `docs/todo/workspace-hygiene-001.md` filed, covering both items with fresh-reproduced evidence (both failures independently re-confirmed 2026-07-11, identical to the PIR-001 review's original findings — stable pre-existing state, neither resolved nor worsened): `ob-poc-boundary`'s 3 golden-count assertion failures (`acp_dag_semantic`/`acp_registry_projection`, expected-vs-actual counts drifted, e.g. `left: 97, right: 74`) and `dsl-runtime`'s 1 doctest failure (`compute_reducer_revision` no longer re-exported at the crate root the doctest imports from). This is that link.

E5's scope annotation (T8.6) now reads: workspace green is scoped to C-0xx-dispositioned crates, and the excluded pre-existing failures are tracked at `docs/todo/workspace-hygiene-001.md`, not merely named.

## Tranche T9.1c/T9.1d — wired, not closed; T9.1 amended (Addendum B, 2026-07-11)

**Empirical probe before trusting the dependency table (habit worth naming):** before claiming T9.1c (G5 Authority) and T9.1d (G6 Evidence) closed, `build_evaluation_context` was extended with real `AuthorityInput` (actor_id/role from the `ActorContext` already resolved at this call site for G1; `access_decision` derived from an actual `PruneReason::AbacDenied` match on `envelope.pruned_verbs`, not fabricated) and real `EvidenceInput` (`evidence_gaps` mapped directly from `SemOsContextEnvelope.evidence_gaps`). 6/6 unit tests passed. Rather than trust a reading of `GATE_DEPENDENCIES` (`crates/ob-poc-control-plane/src/gate.rs`) to conclude the wiring was reachable, a throwaway probe test called `ob_poc_control_plane::evaluate_shadow()` against a context built by the real function, shaped exactly as today's call site produces it (`entity_binding: None`, `pack_resolution: None`). Result:

```
EntityBinding = Failure("no EntityBindingInput supplied")
PackResolution = NotEvaluated { blocked_by: [EntityBinding] }
Authority = NotEvaluated { blocked_by: [PackResolution] }
Evidence = NotEvaluated { blocked_by: [EntityBinding, PackResolution] }
```

Both gates report `NotEvaluated` regardless of the real input now wired — the wiring is correct but currently unread. Landed anyway (commit `150831b3`), explicitly documented as not satisfying T9.1's exit criterion, rather than held back or, worse, claimed closed.

**What this demonstrates architecturally:** the declared-dependency, collect-where-independent evaluator design (V&S §6.16) did exactly the job it exists to do. Had gate dependencies been inferred ad hoc instead of declared and enforced — the thing §6.16.1 exists to forbid — the wired-but-unread `AuthorityInput`/`EvidenceInput` could have produced plausible-looking `Allow`/`Sufficient` results assembled from data the gates never actually consumed, and T9.1c/T9.1d could have been marked closed on fiction. The hard `NotEvaluated { blocked_by }` chain made the gap undeniable instead of silently wrong. Worth carrying forward as a design point, not just a bug catch: the dependency graph caught its own planner's scoping error.

**Root cause (architect's own characterization, recorded verbatim in spirit):** Addendum B's T9.1 framing — "complete [G3, G4, G5, G6, G7] in any order" — directly contradicts the V&S §6.16.1 dependency table the same document is built on: `G3<-G2`, `G4/G5/G6<-G3`, `G7<-G4`. The five sub-tranches were never independent. Worse, the five-way split dropped **G2 (EntityBinding)** entirely — nothing at the shadow call site supplies `EntityBindingInput` at all, not even attempted — even though T8.4's original wording ("non-`not_evaluated` results for every implemented gate") always implicitly required it. This is the addendum contradicting its own basis document, not a margin gap in the addendum.

**T9.1 is amended, effective this entry — the original "any order" clause is REVOKED:**

> **T9.1 (amended twice): six sub-tranches in dependency order, per V&S §6.16.1.**
> - **T9.1-pre — G2 EntityBinding input, reclassified from "plumbing" to "integration" (see below).** Exit: G2 non-`not_evaluated` on real Path A dispatch.
> - **T9.1a — G3 PackResolution** (unchanged scope, now unblockable once T9.1-pre lands).
> - **T9.1b — G4 DagProof**, **T9.1c — G5 Authority** (wiring already landed in `150831b3`; closure now means the gate actually reads it, not new wiring), **T9.1d — G6 Evidence** (same — wiring landed, closure is reachability) — any order among these three, since all three depend only on G3.
> - **T9.1e — G7 WriteSet** (depends on G4's legal transition — must follow T9.1b).
> - Clock-start condition unchanged in substance, sharpened in scope: all implemented gates non-`not_evaluated` on every live verb family — which now genuinely includes G2, as T8.4's original wording always required and the five-way split had silently dropped.

**Option considered and rejected:** de-scoping G5/G6 back to "deferred" alongside G3/G4/G7 was considered and rejected — it would defer the graduation clock for no saving, since the same G2/G3 prerequisite gates the whole pipeline regardless of which sub-tranches are nominally excluded. The wiring is done; it stays landed, inert-but-honestly-labeled, pending T9.1-pre/T9.1a.

**T9.1c/T9.1d status: WIRED, NOT CLOSED** (accurate when written — corrected below). Reopens as closed only once T9.1-pre and T9.1a land and a live Path A dispatch shows non-`not_evaluated` Authority/Evidence results recorded in `control_plane_shadow_decisions`.

**Update: T9.1c/T9.1d now CLOSED (2026-07-11, same day, commit `15835f7d`)** — T9.1-pre (G2) and T9.1a (G3) both landed later this session. `g3_reaches_success_and_unblocks_authority_evidence` (`agent::control_plane_shadow::tests`) proves via `evaluate_shadow` that Authority and Evidence both reach real `Success`, not `NotEvaluated`, once their declared dependencies genuinely succeed. The exit criterion this entry set is met.

## T9.1-pre reclassified: "plumbing" was wrong in principle, not just in practice (2026-07-11)

**Third scoping correction, same session, at the architect's own address.** T9.1's amendment above described T9.1-pre as surfacing "resolved entity refs... proven at compile time by dsl-resolution" — i.e. plumbing, not new logic. Investigating the actual shape of `EntityBindingInput` (`crates/ob-poc-control-plane/src/entity_binding.rs:98`) showed this framing was wrong at the root, not just optimistic: G2 grades five **point-in-time facts** per entity (`exists`, `expected_kind`/`actual_kind`, `lifecycle_state_readable`, `availability_blocked`, `in_active_pack`) — reads of current DB state, not something any compile-time artifact could carry even in principle, because dsl-resolution proves a reference *denotes something*, not what that something currently *is*. The module's own doc comment already said this ("the caller performs the lookup, per §9.1") — Addendum B's planner forgot what the crate's own author wrote down. Confirmed via grep that no existing service in the codebase already assembles this fact shape (no `EntityFacts`/`entity_lookup`/`lifecycle_state_readable` producer exists today) — this is genuine new integration, comparable in kind to the G3/G4 work already deferred, not a wiring task.

**Convergence found, not just a cost:** T9.2's atomic-admission work needs pre-state pins (`row_version` per bound entity, captured at evaluation, verified at admission) from the same versioned rows G2's facts come from. **Decision: one batched, snapshot-consistent query** (`WHERE id = ANY($1)` over the entity registry joined to state slots and pack membership) executed once at shadow-evaluation time, returning `EntityFacts` *plus* `row_version` per entity — G2 consumes the facts, `SnapshotPins` (T9.2) consumes the versions, G3 (T9.1a) gets pack membership from the same as-of read instead of a second query that could disagree. Retires the `ResolvedEntity{row_version:0}` placeholder sitting in the Mode-1 register since Phase 0. T9.1-pre stops being an unbudgeted tax on T9.1 and becomes the read path T9.2 needed at evaluation time regardless.

**Shape (per §9.1's decision-assembler law — the control plane crate does no I/O):** an `EntityFactsSource` trait, defined in the control-plane crate or `ob-poc-boundary`, implemented in `ob-poc` against the entity store, injected at the shadow call site. The gate stays pure; the lookup is a borrowed proof like every other validator in this design.

**Which args are entity refs — confirmed, not a fourth flag:** the UUID-regex heuristic (`write_set.rs::derive_write_set_heuristic`) was explicitly ruled out as a G2 input source — a UUID-shaped string isn't necessarily a bound entity, and a missed one is a silently ungraded binding. Checked whether contract-derived typing (the correct source, per A4's write-set default) is reachable at the shadow call site: **yes** — `VerbConfigIndex::entries[fqn].args` (`src/repl/verb_config_index.rs:64-75`) carries `ArgSummary { name, lookup_entity_type: Option<String>, .. }` per parameter, and `phase5_runtime_recheck` (the shadow call site, `sequencer.rs:7695`) already holds `self.verb_config_index: Arc<VerbConfigIndex>` in scope. Entity-typed args for the dispatched verb are identified by filtering `args` on `lookup_entity_type.is_some()` and resolving each matched `name` against `entry.args`, not by regexing values. This part of the design pass closes clean — no third stop-and-flag needed.

**T9.1-pre status: CLOSED (2026-07-11, commits `302b61fa`, `f3d025a7`, `b04b43d3`).**

- `EntityFactsSource` trait + `PgEntityFactsSource` (`crates/ob-poc-boundary/src/entity_facts.rs`) — batched per-kind query reusing `toctou_recheck.rs`'s tested 5-kind table mapping, returns `EntityFacts` + `row_version` (the T9.2 convergence point) in one round trip. Fixed a real NULL-vs-false SQL bug caught by the live-DB tests (cbu's `IN(...)` predicates need `COALESCE(_, false)`), not assumed correct.
- `entity_binding_requests` / `build_entity_binding_input` (`agent::control_plane_shadow`) — contract-derived entity-arg detection (`VerbConfigIndex.args[].lookup_entity_type`, confirmed reachable in the design pass) + facts-to-`EntityBindingInput` assembly, wired into `sequencer.rs::phase5_runtime_recheck`.
- **Correctness bug caught before shipping, not after:** `entity_binding` must be `Some(entities: vec![])` — not `None` — for a verb with zero entity-typed args, because `entity_binding.rs::decide()` treats an empty list as vacuous `Success`, while `None` produces a hard `Failure`. The original doc-comment draft had this backwards; corrected and locked in by `empty_entity_binding_input_is_vacuous_success_not_failure` before any wiring landed. Same failure class as the T9.1c/d catch: a plausible-looking wiring choice that would have silently made G2 fail on the common case (e.g. every `session.info` dispatch).
- `in_active_pack` defaults to `true` unconditionally — documented in the code as an open question, not a considered answer (G3/pack-resolution isn't wired yet and G2 has no declared dependency on it, so it structurally can't wait).
- **Empirical reachability, proven twice:** `g2_reaches_success_end_to_end_against_a_real_cbu_row` (evaluate_shadow reports G2 `Success` against a live cbu row) and `g3_is_now_the_sole_blocker_for_authority_and_evidence` (re-ran the T9.1c/d probe post-wiring: `PackResolution` now correctly reports its own genuine `Failure`, not `NotEvaluated{blocked_by:[EntityBinding]}`, and `Authority`/`Evidence` are now blocked *solely* by `PackResolution` — confirms T9.1a is the sole remaining prerequisite for T9.1c/d's already-landed G5/G6 wiring to start reporting real outcomes).

13/13 `control_plane_shadow` tests pass (7 new + 6 pre-existing), 4/4 `entity_facts` tests pass, all live against the DB. `cargo build`/`clippy` clean. Public-API surface baseline refreshed for `ob-poc-boundary`.

**Next in T9.1's dependency order: T9.1a (G3 PackResolution)** — needs Domain Pack registry understanding (distinct from `constellation_family`/`constellation_map`, per V&S §1.1's naming note), not yet attempted. **T9.2 and T9.3 have zero dependency on T9.1 and may proceed independently** (T9.3 already closed; T9.2 flagged, not attempted — see their own tranche entries).

## Tranche T9.3 — CLOSED, all production DSL ingress points admission-wired (2026-07-11)

**Landed (commit `3e768969`):** `RealDslExecutor::execute()`/`execute_in_scope()` (`src/repl/executor_bridge.rs`) now call a new `admit_plan()` — same admission primitive as T6's bus-path fix (`agent::control_plane_envelope_store::check_admission`), applied per verb in the compiled plan — before delegating to the (unmodified) internal `dsl_v2::executor::DslExecutor` engine. Confirmed via `git diff --stat` that `dsl_v2/executor.rs` and `bpmn_integration/dispatcher.rs` are untouched, per the redesign's own exit criterion. `WorkflowDispatcher`'s Direct branch needed no separate T9.3b edit: it calls `self.inner.execute_v2()`, and every production construction of `inner` in `crates/ob-poc-web/src/main.rs` (4 sites) is `RealDslExecutor`, which resolves `execute_v2` through the blanket `impl<T: DslExecutor> DslExecutorV2 for T` straight into the now-admission-checked `execute()`. One fix covers Path B (direct REPL, no BPMN), Path C (WorkflowDispatcher Direct), and the JobWorker durable-direct construction (`main.rs:1359`).

**NOT closed — a broader ingress surface than Addendum B named.** Before flipping the `dsl_v2/executor.rs` allowlist entry from `KNOWN-BYPASS`, checked whether `RealDslExecutor` and `WorkflowDispatcher` are actually the only production callers of the shared internal engine. They are not. Confirmed at least three more live, mounted ingress points that construct `dsl_v2::executor::DslExecutor` directly and reach the exact same `VerbExecutionContext::new(Principal::system())` construction site with no admission check at all:

1. **MCP `dsl_execute` tool** (`ToolName::DslExecute` → `src/mcp/handlers/core.rs:723`, executor built via `build_dsl_executor()` at line 336) — an external MCP-client-reachable surface, separate from the REST API's raw-DSL closure (CLAUDE.md: `OBPOC_ALLOW_RAW_EXECUTE` removal was scoped to request *bodies*, not this MCP tool).
2. **`POST /api/session/:id/execute`** (route mounted at `src/api/agent_routes.rs:98` as `execute_session_dsl_legacy_raw_only` → `execute_session_dsl_raw`, using `AgentState.dsl_v2_executor`, constructed with bare `DslExecutor::new(pool)` at `src/api/agent_state.rs:103`).
3. **Batch/sheet executors** (`src/dsl_v2/batch_executor.rs:397`, `src/dsl_v2/sheet_executor.rs:566`) — bulk template/sheet execution, `api/session.rs` references `BatchResultAccumulator`, so at least the batch path is live.

(`domain_ops/{template_ops,onboarding,gleif_ops}.rs` and `gleif/repository.rs` also construct `DslExecutor` directly — investigated below, confirmed out of scope.)

**Why the allowlist was initially left untouched:** `scripts/check-verb-execution-context-allowlist.sh` scans `VerbExecutionContext::new(` construction sites, not call-graph reachability (the same documented limitation as `lint_write_paths.sh` — "source scanning... does not prove arbitrary indirect callers are verb-mediated"). Reclassifying the one `dsl_v2/executor.rs` entry from `KNOWN-BYPASS` to closed at that point would have turned the gate green while three known paths still reached the identical unguarded construction site — gaming the scanner exactly as the T9.1c/d empirical probe exists to prevent.

**Closed same session (commit `68d2e98b`):** fixed all three. Extracted the shared per-plan admission loop from `executor_bridge.rs`'s `admit_plan()` into a single crate-shared function — `agent::control_plane_envelope_store::admit_plan()` (env-reading wrapper) / `admit_plan_checked()` (pure core, parameter-taking, testable without process-global env mutation) — so all 5 call sites (the original 2 plus these 3) delegate to one implementation:

1. **MCP `dsl_execute`** — `admit_plan()` called on the compiled plan right before the existing `build_dsl_executor()`/execute step.
2. **`POST /api/session/:id/execute`** — same, right before the existing batch-policy-routed execute step.
3. **Batch/sheet executors** — `batch_executor.rs`'s `execute_iteration` admits its per-iteration compiled plan before dispatch; `sheet_executor.rs`'s `execute_statement` parses+compiles its statement once more up front purely to admission-check (the execution parse inside `execute_dsl` is unchanged) since it doesn't otherwise hold a compiled plan at that call site.

New live-DB tests for `admit_plan_checked` (11/11 passing): a 2-verb plan is rejected on its *second* step (proves the whole plan is walked, not just the first verb), the error names the rejected verb and reason, the from-env wrapper delegates correctly.

**The previously-unconfirmed sites, investigated and confirmed out of scope, not left as an asterisk:** grepped for `impl SemOsVerbOp` and exclusive-caller chains — `template_ops.rs`'s `OnboardingAutoComplete`-style ops, `onboarding.rs`'s `OnboardingAutoComplete`, and all 9 structs in `gleif_ops.rs` (`GleifEnrich`, `GleifImportTree`, etc.) are themselves `SemOsVerbOp` plugin verbs, dispatched through the already-admission-checked `ObPocVerbExecutor`/`SemOsVerbOpRegistry` chain; the `DslExecutor::new(...)` calls inside them execute sub-DSL as part of an *already-admitted* verb's own implementation — same carve-out the allowlist file already used for Path A/D's fallthrough to this construction site. `gleif/repository.rs`'s `create_entity_from_gleif` is called exclusively from `gleif/enrichment.rs::GleifEnrichmentService`, which is used exclusively by `domain_ops/gleif_ops.rs` — same category, confirmed by grep, not asserted. `templates/harness.rs` is a "Template Test Harness" per its own doc comment — test tooling. `bin/dsl_cli.rs` is a separate Cargo `[[bin]]` target gated `required-features = ["cli"]` — not part of the running `ob-poc-web` service process.

**Allowlist updated:** `audits/surface/_verb-execution-context-allowlist.txt`'s `src/dsl_v2/executor.rs` entry reclassified `KNOWN-BYPASS` → `ADMISSION-WIRED` (construction site itself unchanged — `git diff --stat` confirms zero changes to `dsl_v2/executor.rs`, per the redesign's own exit criterion — but every production caller reaching it now pre-admits). Gate output: `ALLOWLISTED (ADMISSION-WIRED): src/dsl_v2/executor.rs` — **zero `KNOWN-BYPASS` entries remain.**

**T9.3 status: CLOSED.** All production DSL dispatch ingress points in the running service are admission-checked (mechanism-only while `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` is empty by production default — same shadow-first posture as every other T9 sub-tranche). `cargo build` clean (`ob-poc` lib with/without `database` feature, `ob-poc-web`), `cargo clippy` clean on all touched files.

## Tranche T9.2 — FLAGGED, not attempted (2026-07-11)

Addendum B's own sonnet execution note pre-authorized this outcome: *"T9.2 is the one sub-tranche where 'STOP and flag' is the expected outcome if the transaction restructure surfaces a constraint this addendum didn't anticipate... a flagged design constraint beats a shipped approximation."* It did.

**What "atomic admission" requires:** envelope consumption + `verify_pins` (`ob-poc-boundary::toctou_recheck` — has **zero production call sites today**, so even the pin-check half of T9.2 is currently entirely unwired) + the verb's own write, all inside one transaction, so nothing can commit a conflicting change between the admission check and the write.

**Why it can't be done as a scoped change:** `ObPocVerbExecutor::execute_verb` (`src/sem_os_runtime/verb_executor_adapter.rs`) routes a dispatched verb to one of three structurally different, all-live-in-production transaction strategies, selected by verb behavior at runtime:

1. **SemOS-native ops** (~line 225-289) — explicit `PgTransactionScope::begin(pool)` / `commit()` / `rollback()`.
2. **CRUD fast path** (~line 293-309, `PgCrudExecutor`, wired in production at `crates/ob-poc-web/src/main.rs:1588`) — holds a bare `pool: PgPool`; each `execute_select`/`execute_insert`/etc. (`crates/dsl-runtime/src/crud_executor.rs`) is its own implicit autocommit statement, no transaction at all today.
3. **Generic path** (~line 311-320) — delegates into `dsl_v2::executor::DslExecutor`, which opens its own transaction internally (`execute_verb_inner`/`execute_verb_in_scope`).

`execute_verb_admitting_envelope` today calls `self.admit(...)` (its own pool-acquired `try_consume` UPDATE, committed independently) then `self.execute_verb(...)` (one of the three above, its own separate transaction) — two fully independent commits with a real gap between them. Closing that gap for real means opening one outer transaction before branch selection, threading that same connection through whichever of the three branches gets used, and stopping each branch from opening its own nested transaction when a caller-supplied one is already active — a structural rewrite of the busiest shared dispatch path in the system, spanning three independently-evolved subsystems (`sem_os_ops` registry, `CrudExecutionPort`, `dsl_v2::executor::DslExecutor`). Not a scoped, reversible single-session change; a wrong move risks silently wrong commit/rollback semantics or deadlocks across every domain, not just control-plane paths.

**T9.2 status: FLAGGED, not attempted.** No code changed. Options for how to proceed, put to the architect rather than picked unilaterally: (a) a short design doc for the three-branch transaction-scope unification, reviewed before any code lands; (b) hold T9.2 open indefinitely while other T9 work proceeds; (c) architect-specified alternative mechanism.

**Update: design doc landed (2026-07-11, same day, option (a) picked), commit `be2152a5`.** `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T9.2-ATOMIC-ADMISSION-001.md` — proposes one `PgTransactionScope` opened before branch selection; per-branch changes (SemOS-native ops and the generic path are trivial swaps to already-existing scope-accepting entry points; the CRUD fast path needs a new `CrudExecutionPort::execute_crud_in_scope` trait method plus five new query-builder methods — the real work); admission primitives genericized (`impl PgExecutor<'_>`, mirroring T9.3's `admit_plan`/`admit_plan_checked` split) rather than duplicated; pin verification (`verify_pins` — zero production callers today) proposed as a new free function rather than reused as a trait impl, since a live transaction doesn't fit the existing `&self`-based `RowVersionProvider` shape. Three open questions recorded, one resolved during the design pass itself. **T9.2 status: DESIGN DOC COMPLETE, AWAITING REVIEW. Implementation not started.**

**Update: APPROVED WITH AMENDMENTS (architect review, 2026-07-11, same day), doc revised to v0.2, commit `36b437d5`.** The one-scope-before-branching principle and branch analysis were approved as-is. One BLOCKER found and resolved in the revision: a plain pin read inside the transaction doesn't stop a concurrent writer committing between the check and the write under READ COMMITTED — fixed by adding `SELECT ... FOR UPDATE` on pinned entity rows to `verify_pins_in_scope` (new §5a), with a new required concurrent-writer-during-scope probe as the actual proof of atomicity (the two originally-proposed probes alone don't cover it). One false claim corrected: §6's "behaviorally invisible" doesn't hold for Branch 2 (CRUD) — today's per-statement autocommit means partial writes survive a mid-sequence failure; under T9.2 they roll back atomically, a deliberate correctness improvement with a genuinely changed failure mode, now stated as such with a dedicated durability-on-failure test. Two additions: validity-window semantics stated explicitly (admission-time validity stands, no commit-time re-check, to head off a later "fix" that would make slow verbs nondeterministic) plus the rollback-then-retry corollary; and OQ4 (durable/park interaction in Branch 3 — a parked verb's scope commit consumes the envelope, so resume must re-admit via `EnvelopeHandle` rehydration, needs tracing, may resolve during implementation).

**Debt pre-registered, per the review's own instruction:** the pool-based `execute_crud`/`check_admission`/`try_consume`/`verify_pins` variants retained in T9.2 for staged rollout (§7) are the same "two APIs, one weaker" shape that produced **PIR-D-008** (resolved in T8.1 by demoting `try_consume_by_id` to `#[cfg(test)]`-only). Their eventual demotion to test-only (once every production caller is confirmed migrated to the scope-based variants) is owed, not optional-forever — tracked here as a follow-up item since no "FIA-4B shrink list" exists yet in this ledger to file it against; whoever owns that list when it lands should pull this item in.

**Update: assessed and closed per-primitive (2026-07-11, same day, commit `131c9de2`)** — not a blanket demotion. The debt's premise ("once every production caller is confirmed migrated") turned out to be true for one of the four named primitives and false for two others; each was checked individually via `grep` before acting, not pattern-matched against `try_consume_by_id`'s precedent:

- **`verify_pins`** — confirmed zero production callers anywhere (only its own tests). Demoted to `#[cfg(test)] pub(crate)`, the exact T8.1/PIR-D-008 pattern. `verify_pins_in_scope` (T9.2's real, locked, admission-time check) is the only variant reachable from production now.
- **`check_admission` / `try_consume`** — **not demotable; the debt's premise was incomplete.** `admit_plan_checked` (T9.3's plan-level pre-flight admission gate — four production ingress points: the REPL executor bridge, MCP `dsl_execute`, the legacy raw-execute route, batch/sheet executors) genuinely needs a per-verb check across a whole multi-step plan before any single step's transaction scope exists, a shape T9.2's in-scope variants structurally can't cover (there is no scope yet at plan-check time). These two pool-based primitives are permanent, not staged-rollout debt.
- **`execute_crud`** — a required `CrudExecutionPort` trait method (no default), so `PgCrudExecutor` must implement it regardless of reachability; can't be `#[cfg(test)]`-gated without breaking trait conformance or the legitimate test-harness callers (`discovery_ops_integration.rs`, `sem_os_harness`) that exercise the full non-scoped `VerbExecutionPort` path directly. Confirmed its only call path (`ObPocVerbExecutor::execute_verb` Branch 2) is unreachable from production today — every production caller now goes through `execute_verb_admitting_envelope`, which the T9.2 restructure routes entirely through `execute_verb_in_open_scope`/`execute_crud_in_scope`, never falling back to `self.execute_verb()`. No further mechanical action is possible or needed here; the risk the debt worried about (a silent pool-based fallback reachable in production) doesn't exist.

Also fixed three stale doc-comment references to `ObPocVerbExecutor::admit` (deleted in the T9.2 restructure, replaced by `admit_in_scope`), found while auditing these callers — would otherwise have pointed a future reader at a function that no longer exists.

**FIA-4B debt: CLOSED** (one item demoted, two items reclassified as permanent-not-debt with the reasoning recorded, one item confirmed already-safe by construction).

**Update: IMPLEMENTED, empirically proven (2026-07-11, same day).** Landed in four commits on `codex/phase-1-5-governance-closure` (not yet merged to main):

1. `f00c1755` — §5/§5a `verify_pins_in_scope` (`ob-poc-boundary::toctou_recheck`): locked (`SELECT ... FOR UPDATE`), scope-threaded pin re-check reading `SnapshotPins::entity_kinds_and_versions()` directly (the widened 3-tuple resolved OQ3 — neither the pin nor G2's `BoundEntities` proof carried entity kind before this; `entity_facts.rs`'s `KindMapping`/`kind_mapping` widened to `pub(crate)`, "one mapping, two consumers"). 4 live-DB tests, including the required concurrent-writer-during-scope probe (§5a's own proof requirement) — a second connection's `UPDATE` on the pinned row is asserted still-blocked after 200ms while the scope holds its lock, unblocks only on `rollback()`.
2. `9836402d` — §3 Branch 2 `CrudExecutionPort::execute_crud_in_scope`, no default (OQ2). Avoided duplicating the ~14 CRUD operation methods by introducing one executor abstraction (`CrudExec<'e>` — `Pool(&PgPool) | Conn(&mut PgConnection)`) threaded through every operation method and the three low-level query helpers; both `execute_crud` and `execute_crud_in_scope` now call one private `dispatch()`. 3 live-DB tests: pool/scope parity, rollback leaves no durable trace, commit is durable (with cleanup).
3. (uncommitted at ledger-write time, see below) — the restructure itself: `execute_verb_admitting_envelope` now opens ONE `PgTransactionScope` before branch selection, runs the new `admit_in_scope` (scope-threaded counterpart to the now-deleted pool-based `admit()`) and a new `execute_verb_in_open_scope` (mirrors `execute_verb`'s 3-branch routing but every branch — SemOS-native ops, CRUD via `execute_crud_in_scope`, and the generic/plugin path via the already-existing `DslExecutor::execute_verb_in_scope` sibling — runs against the one caller-supplied scope), commits on `Ok`, rolls back on `Err`. `admit()` had zero remaining callers after the swap and was deleted outright rather than left as dead code; its 4 existing live-DB tests were ported to open their own scope per call (a consume only durably persists once that scope commits, matching what production now does). Added a 5th, end-to-end test proving the actual atomicity claim: admit a real envelope, dispatch a verb guaranteed to fail (unknown FQN), assert the envelope is *still consumable afterward* — proof the whole scope, including the consume, rolled back together, closing the exact bug class T9.2 exists to prevent (a failed dispatch after successful admission permanently burning the envelope under the old two-transaction shape).
4. OQ4 resolved by inspection, not by new plumbing: durable verbs never reach `execute_verb_admitting_envelope`'s dispatch as durable-direct unless `ctx.allow_durable_direct` is set (the BPMN worker path), and that branch is entirely inside Branch 3's `DslExecutor::execute_verb_in_scope`, unaffected by the outer scope threading — no park/resume special-casing was needed.

Pin verification (`verify_pins_in_scope`) is built and tested as a standalone primitive but **not yet wired into `execute_verb_admitting_envelope`** — `EnvelopeHandle` deliberately carries only id + content hash (see its own module doc), not `SnapshotPins`; the sealed `ExecutionEnvelope` row has no production path populating pins today (G13 has zero production callers per `snapshot.rs`'s own comment). Wiring it in is real future work (T3.2's `DecisionSnapshotGate` adapter reaching production), not something this tranche's scope covers or silently skipped — flagged explicitly, not left implicit.

Verification: full workspace (`cargo build --workspace --features database --all-targets`) clean; `ob-poc`'s 2277-test non-DB lib suite green (0 regressions); the ported + new `t4_1_envelope_admission_tests` module (5 tests) green live; `dsl-runtime`'s 185-test lib suite green; `crud_executor::db_integration_tests` (3 tests) and `toctou_recheck::db_integration_tests` (5 tests, incl. the pre-existing shadow test) green live.

## Tranche T9.1a — FLAGGED, not attempted (2026-07-11)

**What G3's designed production analogue is:** `pack_resolution.rs`'s own module doc names it directly — `src/runbook/constraint_gate.rs::check_pack_constraints` against `journey::pack_manager::{PackManager, EffectiveConstraints}` — exactly what ownership-ledger rows **C-015** ("Pack constraint gate blocks verbs not permitted by active pack constraints... PARTIALLY CLOSED — no production call site wires `constraint_gate` output into it yet") and **C-016** ("SemReg allowed-set unavailable is fail-closed in compiler... PARTIALLY CLOSED — no production call site wired yet") already flagged, before this tranche, as adapter-landed-but-unwired.

**What this tranche found, going one level deeper:** it isn't just that this call site (the shadow wiring) hasn't wired `constraint_gate`'s output yet — **`PackManager` has zero production construction sites anywhere in the running service.** Grepped `PackManager::new(` across the whole tree: every hit is inside `journey/pack_manager.rs`'s own `#[cfg(test)]` module or the external `integration_tests/runbook_pipeline_test.rs` harness. `crates/ob-poc-web/src/main.rs` never builds one. `src/runbook/compiler.rs`'s only reference to `PackManager` is a doc comment (`/// * constraints — effective constraints from PackManager (Phase 2)`), not a call — the "Phase 2" wiring that comment promises was never completed. There is no live session-scoped pack-activation state anywhere to read from.

**Why this is a different, larger shape of gap than T9.1-pre's:** T9.1-pre's designated source (real DB tables — `cbus`, `entities`, etc.) already existed and was live; the gap was purely "nobody wrote the query yet," a bounded read. T9.1a's designated source (`PackManager`'s pack-activation/suspension state machine, tracking which packs are currently active for a session) has **no running instance to read** — building it means wiring session-scoped pack activation through the REPL lifecycle from scratch, a separate, substantial piece of infrastructure, not a scoped read at one call site.

**Alternative considered and rejected:** `SessionVerbSurface`'s workspace/macro-membership computation (`agent::verb_surface`) is live, real, and reachable at the shadow call site — but the T9.1-pre design pass already recorded, in the ownership ledger, the explicit caution that "Pack is reserved exclusively for SemOS domain packs" (V&S §1.1's own naming note) and that `constellation_family`/`constellation_map` are a *distinct* concept from pack identity. Substituting `SessionVerbSurface`'s workspace membership for G3's `candidate_pack_ids` would repeat exactly the dishonest-conflation mistake that caution exists to prevent — a workspace is not a SemOS pack, and grading G3 against the wrong concept would be a fabricated-looking-real signal, the same failure class the T9.1c/d empirical probe was built to catch.

**Practical effect on T9.1's dependency order:** per `GATE_DEPENDENCIES` (`crates/ob-poc-control-plane/src/gate.rs`), G4/G5/G6/G7 all transitively require G3. **T9.1 is now blocked at G2** (closed) pending T9.1a. T9.1c/d's landed wiring (Authority/Evidence) will not report real, non-`not_evaluated` outcomes until this is resolved — confirmed empirically in the T9.1-pre closure entry above (`g3_is_now_the_sole_blocker_for_authority_and_evidence`).

**T9.1a status: FLAGGED, not attempted.** No code changed. Options for the architect, not picked unilaterally: (a) scope a minimal PackManager session-activation wiring sub-project as its own tranche (size/shape TBD — needs its own investigation into where pack activation/suspension state should live: session record, a new table, or derived per-turn from something else entirely); (b) an architect-specified alternative real pack-identity source not yet identified; (c) accept T9.1 stops at G2 for the current graduation cycle and revisit the graduation-window definition (`EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001`) accordingly.

### Correction (2026-07-11, same day, option (a) picked): the "build new infrastructure" conclusion above was wrong

The finding that `PackManager` has zero production construction sites was, and remains, accurate — left as-is above rather than deleted. **The conclusion drawn from it was not:** re-investigating before starting the "build it" work turned up two already-live pieces of session state that make new session-persistent pack-activation infrastructure unnecessary:

- `ReplSessionV2::active_pack_id() -> Option<String>` (`src/repl/session_v2.rs:949`) — already computed from `self.staged_pack`/`self.runbook.pack_id`. Confirmed via a direct schema check (`grep "^id:" config/packs/*.yaml`) that REPL journey pack ids are bare strings (`"kyc-case"`, `"book-setup"`), not the SemOS-Domain-Pack dotted format (`"ob-poc.cbu"`) `pack_resolution.rs`'s own test fixtures happened to use — confirming `constraint_gate.rs`/`PackManager`'s designed analogue really is the REPL journey pack system, not SemOS Domain Packs, resolving the ambiguity the original flag didn't fully chase down.
- `ReplOrchestratorV2::pack_router: PackRouter` (`src/sequencer.rs:229`) — already holds every loaded journey-pack manifest in memory (`PackRouter::get_pack(pack_id) -> Option<&(Arc<PackManifest>, String)>`), already used extensively elsewhere in `sequencer.rs`.

`PackManager` itself is pure in-memory state (two `HashMap`s, no I/O, no async) — a fresh, throwaway instance built per shadow-recheck call (register the one active pack, activate it, call `effective_constraints()` + the already-tested `check_pack_constraints`) is cheap and correct for the REPL's real single-active-pack model, not a workaround. No new session-persistent activation tracking was needed. **T9.1a CLOSED** (commit `15835f7d`) — see the implementation entry below for the full design and the empirical proof that Authority/Evidence now reach real `Success`.

Lesson for the ledger, not just this tranche: the original flag stopped at "no live instance of the thing the doc comment points to" without checking whether a *different, already-live* piece of state could feed the same real logic without new infrastructure. Worth the extra look before flagging "needs new infrastructure" as a conclusion, not just a hypothesis.

## Tranche T9.1b — CLOSED (2026-07-11, same day)

**What G4's designed production analogue is:** the v1.3 cross-workspace gate stack (`crates/dsl-runtime/src/cross_workspace/gate_checker.rs`'s `GateChecker`, `DagRegistry`, `SlotStateProvider`) already runs in production — `VerbExecutionPortStepExecutor::pre_dispatch_gate_check` (`src/runbook/step_executor_bridge.rs`) gate-checks every step's `transition_args`-declared transition before real dispatch. Unlike T9.1a's `PackManager` (zero production instances anywhere), this mechanism is live, wired at `ob-poc-web::main` startup, and already carried on `ReplOrchestratorV2` (the same struct that owns `phase5_runtime_recheck`, the shadow call site) as `gate_pipeline: Option<GatePipeline>` — confirmed by direct investigation before writing any code, not assumed from reading the module doc.

**Design, reviewed with the architect before implementation** (per the user's explicit "concrete design" request): extract `pre_dispatch_gate_check`'s inline resolution body into a reusable `resolve_transition_probe` function returning a `DagTransitionProbe { entity_id, from_state, to_state, blocking_violations }`, so the shadow call site builds G4's `DagProofInput` from the exact same mechanism the real dispatch-path gate uses — not a re-derived approximation. Landed in two commits:

1. `6ffd2659` — the extraction. Preserves `pre_dispatch_gate_check`'s original control flow byte-for-byte: same candidate `TransitionRef` iteration order, same `GateChecker::check_transition` calls, same short-circuit-on-first-blocking-violation (a later candidate's check is never invoked once an earlier one already found a violation — matches the pre-extraction inline version exactly, not just in outcome but in call sequence). Proved with a dedicated equivalence test (`pre_dispatch_gate_check_equivalence_legal_and_violating`) exercising the real production method against an in-memory `GateChecker`/`DagRegistry` built from an inline DAG YAML fixture (temp dir, no live DB, a `FixedSlotState` in-memory `SlotStateProvider` — the same no-mock-feature-needed pattern used throughout).
2. `c92e40f2` — the shadow wiring. `build_dag_proof_input` (`control_plane_shadow.rs`) calls `resolve_transition_probe` against `self.gate_pipeline.as_ref()` — no new plumbing needed, exactly as the design predicted. `lifecycle_fail_open_class`/`lifecycle_gate_mode_fail_closed` stay at their safe defaults (`None`/`false`) — T0.2's `enforce_requires_states_precondition` needs a live `&mut dyn TransactionScope` (designed for real dispatch, not read-only shadow observation), so unifying it here would have meant opening a throwaway rollback-only scope, a second mechanism riding along on this tranche; deliberately left as flagged follow-on work, not silently absorbed.

**Empirically proven, not assumed:** `g4_reaches_success_end_to_end_against_a_fixture_dag` is this tranche's version of the by-now-standard reachability proof (matching `g2_reaches_success`/`g3_reaches_success_and_unblocks_authority_evidence`) — build a real `DagProofInput` via `build_dag_proof_input`, supply real succeeding `EntityBinding`/`PackResolution` (G4's actual `GATE_DEPENDENCIES` prerequisites — the first attempt at this test omitted them and correctly failed with `NotEvaluated { blocked_by: [EntityBinding, PackResolution] }`, caught by running the test rather than assuming the wiring was sufficient), and confirm `evaluate_shadow` reports G4 `Success`, not `NotEvaluated`.

Full `ob-poc` lib suite: 2283 passed, 0 failed (2277 → 2283, six new non-DB tests across the two commits). Full workspace build clean (`--all-targets --features database`).

**T9.1b status: CLOSED.** Only remaining gap, explicitly flagged rather than owed silently: T0.2 lifecycle-precondition unification into `DagProofInput`'s `lifecycle_fail_open_class`/`lifecycle_gate_mode_fail_closed` fields — real follow-on work requiring a throwaway transaction scope, not part of this tranche's scope.

## Tranche T9.1e — CLOSED (2026-07-11, same day)

**The module doc's stated blocker was investigated and found outdated.** `control_plane_shadow.rs` claimed G7 "needs parsed verb args this call site doesn't have (only the raw DSL string)" — but `derive_write_set_heuristic` (`src/runbook/write_set.rs`) only ever needed the resolved-args map, structurally identical to `entry.args` (already available, already used by G2/G3/G4's builders). **The real blocker was different:** `WriteSetGate::decide` (`crates/ob-poc-control-plane/src/write_set.rs:133-138`) hard-requires `contract_derived: true` and non-empty `tables: Vec<String>` — and nothing reachable from `phase5_runtime_recheck` produced table names at all. Checked three plausible already-live sources before concluding a new one was needed: `RuntimeVerb` (the registry `verb_executor_adapter.rs` dispatches through) has no `writes_to` field; `SemOsContextEnvelope`/`VerbCandidateSummary` (already computed earlier in the same function) don't carry it either; only a standalone binary (`src/bin/sem_os_footprint_audit.rs`) loaded `domain_metadata.yaml` before this tranche — the running web service never did.

**Design, reviewed with the architect before implementation** (same "concrete design" pattern as T9.1b): load `sem_os_obpoc_adapter::metadata::DomainMetadata::from_file(...)` once at `ob-poc-web::main` startup (best-effort, not production-fatal the way `GatePipeline`'s DAG registry load is — a missing/malformed `domain_metadata.yaml` just leaves G7 shadow-unwired, logged), thread it onto `ReplOrchestratorV2` as a new `domain_metadata` field the same way `gate_pipeline` already is. `build_write_set_input` (`control_plane_shadow.rs`) looks up the verb's `VerbFootprint.writes`; empty or absent → `None` (not a fabricated `CannotDerive` — a read-only verb legitimately writes nothing, same "not applicable, not a failure" posture as G4's non-transition-verb case). `state_slots`/`allowed_columns` stay empty — no production source for column-level footprint exists anywhere in the codebase yet, and `decide()` doesn't require them.

**Empirically proven, not assumed:** `g7_reaches_success_end_to_end_given_a_legal_dag_transition` follows the by-now-standard reachability pattern (g2/g3/g4) — supplies a real `DagProofInput` (G7's actual `GATE_DEPENDENCIES` prerequisite, confirmed via `gate.rs`: `(GateId::WriteSet, &[GateId::DagProof])`) and confirms `evaluate_shadow` reports G7 `Success`. Also added a regression guard the other tranches didn't need (`real_domain_metadata_yaml_loads_and_has_at_least_one_write_footprint`): loads the actual production `config/sem_os_seeds/domain_metadata.yaml` and asserts at least one non-empty `writes: [...]` entry exists — catches this wiring going silently dead (`build_write_set_input` always `None`) if the YAML drifts, without needing a live DB.

Full `ob-poc` lib suite: 2289 passed, 0 failed (2283 → 2289, six new tests). Full workspace build clean (`--all-targets`).

**T9.1e status: CLOSED. T9.1 (G1–G7) is now fully shadow-wired end to end** — every gate from `IntentAdmission` through `WriteSet` builds its `EvaluationContext` input from a real production source, not a placeholder, though several (G4's lifecycle-precondition unification, G7's column/state-slot granularity) have explicitly flagged, not silently absorbed, follow-on gaps.

## T9.1 CLOSED (2026-07-11, same day) — closure sweep against the actual amended exit criterion

Six sub-tranches, landed in this order across this session: **T9.1-pre** (G2, `302b61fa`/`f3d025a7`/`b04b43d3`) → **T9.1a** (G3, `15835f7d`) → **T9.1c/T9.1d** (G5/G6 reachability, same commit as T9.1a) → **T9.1b** (G4, `6ffd2659`/`c92e40f2`) → **T9.1e** (G7, `45f3eb6f`). Every sub-tranche's own entry above records its individual empirical reachability proof (a pairwise "this gate reaches Success once its declared dependency does" test).

**The amended T9.1 plan's actual exit criterion is stronger than any single pairwise proof:** *"all implemented gates non-`not_evaluated` on every live verb family."* Rather than infer this holds from seven separate two-gate proofs plus a reading of `GATE_DEPENDENCIES`, `t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch` (`agent::control_plane_shadow::tests`, commit `0e2e5ec7`) builds all seven gates' real inputs together — a live `cbu` row for G2, a fixture pack for G3, the fixture DAG/`GateChecker` for G4, the fixture domain-metadata write footprint for G7, with G5/G6 deriving from the same envelope/actor already assembled — and asserts `IntentAdmission`, `EntityBinding`, `PackResolution`, `DagProof`, `Authority`, `Evidence`, and `WriteSet` all report real `Success` in one `evaluate_shadow` call against one dispatch. This is the habit this session repeatedly found worth the extra step (T9.1c/T9.1d's own "empirical probe before trusting the dependency table" entry above is the precedent): a declared dependency graph proves gates *can* reach Success independently; it doesn't by itself prove they *do*, together, on a real call.

**What remains explicitly open, not silently closed:**
- G4: T0.2 lifecycle-precondition unification (`lifecycle_fail_open_class`/`lifecycle_gate_mode_fail_closed` stay at safe defaults — no live source wired; needs a throwaway transaction scope).
- G7: column/state-slot granularity (`allowed_columns`/`state_slots` stay empty — no production source exists anywhere in the codebase yet, table-level only).
- G8–G14 (STP Classification through Write-Set Attestation) are separate, later tranches — not part of T9.1's scope, and `StpClassifier`'s registered gate adapter will report `NotEvaluated`/`Failure` on any of today's shadow dispatches since no `StpClassifierInput` is built anywhere yet. This is correct, not a gap in this closure. **Update: G8 CLOSED under T9.5, same day — see that tranche's entry below.** G9–G14 remain open.
- T9.2's debt pre-registration (pool-based `execute_crud`/`check_admission`/`try_consume`/`verify_pins` variants retained for staged rollout) — **CLOSED** (commit `131c9de2`, see T9.2's own entry): `verify_pins` demoted to test-only; `check_admission`/`try_consume` reclassified as permanent (T9.3's `admit_plan_checked` genuinely needs them); `execute_crud` confirmed unreachable from production already, by construction.
- None of this is *gating* production dispatch yet — T9.1's entire scope is shadow-only observation (`control_plane_shadow_decisions`), per T2.7's original module doc. Graduating any gate from shadow to gating dispatch is out of scope for T9.1 and not implied by this closure.

## Tranche T9.5 — CLOSED (G8 StpClassifier shadow-wired, 2026-07-11, same day)

Post-T9.1-closure gap-filling pass (architect-approved backlog order: T8.4 closure check → G8/G14 input wiring → ...). G8 was assessed as genuinely small, unlike G14 (see the correction note immediately below).

**Design:** `build_stp_classifier_input` (`control_plane_shadow.rs`) reuses the exact `RuntimeBehavior` lookup `sem_os_runtime::verb_executor_adapter` already uses in production to route CRUD-vs-Plugin-vs-GraphQuery-vs-Durable dispatch (`RuntimeVerbRegistry::get(domain, verb)` against `RuntimeBehavior::Durable(_)`, the "external workflow engine, e.g. BPMN-Lite" variant) — no new registry, no new lookup mechanism. `is_durable_verb` is honestly `false` for any FQN that doesn't parse as `domain.verb` or has no registry entry. `durable_execution_explicitly_allowed` is always `false`: this function is only called from `phase5_runtime_recheck`, Path A's own REPL/runbook dispatch, never a BPMN direct-worker context — the one place the exception would legitimately apply. `has_unpinned_entities` is threaded in by the caller as `!entity_ids.is_empty()` (the same entity-request list G7 already resolves) — T4.3's `SnapshotPins`/`verify_pins` mechanism has zero production populators (confirmed in T9.2's entry above), so every bound entity is honestly unpinned by construction; this is the same conservative-default posture already established for G4's `lifecycle_fail_open_class` and G2/G7's "no source, stay at the safe default" pattern, not a new kind of shortcut.

**Wired:** `stp_classifier` param added to `build_evaluation_context` (now 9 args; `#[allow(clippy::too_many_arguments)]` added — the function was already over clippy's default threshold at 8 args before this change, per-arg allow is the established pattern elsewhere in the codebase, 43 other call sites already use it). `phase5_runtime_recheck` (`sequencer.rs`) builds `Some(build_stp_classifier_input(&entry.verb, !entity_requests.is_empty()))` and threads it through, alongside G1-G7.

**Empirically proven, not assumed:** `t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch` (the same live-DB closure test from T9.1, extended in place rather than duplicated, since G8 depends on all seven of G1-G7 per `GATE_DEPENDENCIES` — the fixture that test already assembles is exactly G8's own precondition set) now also asserts `StpClassifier` reaches real `GateResult::Success` against a live cbu row. Ran and passed against `DATABASE_URL=postgresql:///data_designer` before this entry was written. Four new unit tests cover `build_stp_classifier_input` directly: unregistered FQN → false, malformed FQN (no `.`) → false, `has_unpinned_entities` threads through from the caller in both directions, `durable_execution_explicitly_allowed` is always false regardless of durability.

**Correction made mid-investigation, not glossed over:** G14 (`WriteSetAttestation`) was originally paired with G8 in the proposed backlog as "small." It is not. G14 is a *post-execution* comparison (`write_set_attestation.rs`'s `attest()` takes `CapturedWrite`s — what a dispatch actually wrote — and compares against the declared `WriteSetInput`), fundamentally incompatible with the pre-dispatch `phase5_runtime_recheck` call site every other gate (G1-G8) was wired through this session. The real production infrastructure for this already exists — `PgTransactionScope::record_write`/`set_expected_write_set`/`commit_attested` (`src/sequencer_tx.rs`, T5.1-T5.3) — but has **zero production callers**: no `SemOsVerbOp` implementation, nor the centralized CRUD dispatch path, ever calls `record_write`. Wiring G14 for real means either instrumenting every `SemOsVerbOp` (large, cross-cutting) or at minimum the shared CRUD path (smaller but still a new post-dispatch call site, not an input-builder function). This is real, separately-scoped work, not a same-session pairing with G8. Flagged to the architect rather than either silently skipped or silently absorbed as scope creep.

**T9.5 status: CLOSED for G8.** G14 deferred, correctly sized, pending an explicit architect decision on sequencing (proceed now as its own larger tranche, or defer behind G9-G13/T8.2/T8.3).

## Tranche T9.6 — CLOSED (G13 DecisionSnapshot shadow-wired, 2026-07-11, same day)

Combined G13 + T8.2 investigation, per the approved backlog order. Split into two different-sized outcomes: **G13 closed** (small, real production source already existed); **T8.2 confirmed still genuinely open** (not attempted — investigated, not a quick win either).

**G13 design:** `snapshot.rs`'s own module doc says plainly "No production analogue exists today" for `sem_reg_snapshot_id`/`session_snapshot_id`/`kyc_manifest_hash`/`PinnedVersionSet` — but the per-entity `row_version` pin already has one: `EntityFactsRow.row_version` (`ob-poc-boundary::entity_facts`), fetched at `phase5_runtime_recheck` for G2, was added specifically for this convergence (`entity_facts.rs`'s own module doc: "T9.2's `SnapshotPins` need `row_version` from the same rows"). `build_decision_snapshot_input` (`control_plane_shadow.rs`) reads it directly — no new query. `DecisionSnapshotGate::decide` (per its own tests) succeeds on any `Some(_)`, even an all-default `SnapshotInput` — "this gate pins whatever was read, it doesn't judge it" — so `None` is reserved for the one honest failure case: the batched facts fetch itself erroring (same posture as G2/G8).

**Wiring required a small call-site restructure, not just a new input-builder:** `phase5_runtime_recheck` previously discarded the raw `HashMap<Uuid, EntityFactsRow>` after deriving G2's `EntityBindingInput` from it. Restructured to capture `entity_facts_map: Option<HashMap<Uuid, EntityFactsRow>>` first (`Some(HashMap::new())` for the zero-entity-args case, `Some(facts)` on a successful fetch, `None` on a real fetch error — the same three states `entity_binding` already had), then derive *both* `entity_binding` (G2) and `snapshot` (G13) from that one map. `build_evaluation_context` grew a 10th param (`snapshot`); `EvaluationContext.snapshot` field already existed (T3.1/T3.2 skeleton), unused until now.

**Empirically proven:** `t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch` (already carrying the live cbu-row `facts` map for G2) extended again to also assert `DecisionSnapshot` reaches real `GateResult::Success` — ran green against `DATABASE_URL=postgresql:///data_designer`. 3 new unit tests cover `build_decision_snapshot_input` directly: fetch-not-attempted → `None`; zero entities requested → `Some` with empty `entity_row_versions` (vacuous success); a real fixture row → `entity_row_versions` carries the exact `(id, kind, version)` tuple.

**T8.2 investigation (not attempted, confirmed genuinely open):** re-read `admit_in_scope`/`execute_verb_in_open_scope` (`verb_executor_adapter.rs`) — T9.2's restructure opens one `PgTransactionScope` before branch selection and calls `admit_in_scope` for the envelope-consume half, but `execute_verb_in_open_scope`'s own doc comment says outright: *"pin verification (when wired — see G13's zero-production-caller note in `ob-poc-control-plane::snapshot`)"* — i.e. T9.2 itself already documented this gap at the time, not glossed over. `verify_pins_in_scope` (`toctou_recheck.rs`, T9.2 §5/§5a, `SELECT ... FOR UPDATE`-locked) exists, is tested, and is exactly the mechanism T8.2 needs — but has zero production callers; nothing calls it from `admit_in_scope` or anywhere else in the dispatch path. Wiring it for real needs: (a) a genuine `SnapshotPins` source at admission time (an `EnvelopeHandle` is opaque — id + content_hash only, per its own module doc; the pins would have to come from the persisted envelope row's sealed content, which is a different read than anything `admit_in_scope` does today), and (b) confirming `envelope_handle` is actually `Some` in the production paths that matter (today it's `None` at most call sites — envelope minting isn't wired everywhere yet, a separate, already-flagged gap). This is the same "genuine transactional-boundary surgery, not a signature change" the original T8.2 entry (above) called it — re-confirmed, not resolved, by this investigation. **Left FLAGGED, not attempted**, consistent with the original entry; no new attempt was made to shrink or reframe the exit criterion.

**T9.6 status: CLOSED for G13. T8.2 remains its own future focused tranche** — the investigation added detail (the exact doc-comment admission that T9.2 left this open, the concrete two-part shape of what real wiring needs) but did not change its size or its FLAGGED status.

## Tranche T9.7 — CLOSED (G9 RunbookProof + G12 VersionPinning shadow-wired, 2026-07-11, same day)

The "concrete design first" pass promised at the end of T9.6, for the G9-G12 backlog item — with one correction made before any code landed: `context.rs`'s own doc comment (written at T3) claimed G9-G12 "consume this context's *outputs* rather than being graded from within it," which turned out to be an incomplete generalisation, not an architectural constraint — G8 (`StpClassifier`) already disproves it: a gate can declare real predecessors in `gate::GATE_DEPENDENCIES` *and* still grade its own small primitive fact from `EvaluationContext`, the dependency graph and the gate's own `evaluate()` are orthogonal. That correction, plus per-gate real-source investigation, reduced the actual work: **G9 and G12 close now; G10 and G11 stay stubbed because they genuinely have no production fact anywhere to grade, not because of an architecture gap.**

**Investigated per-gate, not assumed:**
- **G9 (RunbookProof)** — `proof.rs`'s `ControlPlaneProof` already has a public field for exactly eight other gates' proof types (`intent`, `binding`, `pack`, `dag`, `authority`, `evidence`, `write_set`, `snapshot`) plus `runbook: CompiledRunbookRef`. Real source for the one new fact: `entry.compiled_runbook_id.is_some()` — `try_compile_entry()` populates it before the execution loop reaches `phase5_runtime_recheck` for entries created through the current pipeline (INV-3: "raw DSL execution without a CompiledRunbookId is never permitted"); the on-the-fly fallback compile (for legacy/pre-pipeline entries) means `false` is a legitimate rare case, not a systematic false negative.
- **G12 (VersionPinning)** — `snapshot.rs`'s `PinnedVersionSet` (already defined, already embedded in G13's `SnapshotPins`) is the type this gate grades; only `compiler_version` (`env!("CARGO_PKG_VERSION")`) has a real source at the shadow call site — `bus_catalogue_version`/`model_version`/`prompt_version` stay `None`, same "pins whatever was read" posture as G13.
- **G10 (ExecutionEnvelope)** — confirmed via grep that `ExecutionEnvelope::seal()` (the sole constructor) has zero non-test call sites anywhere in the workspace; `control_plane_envelope_store.rs` explicitly documents this ("`ExecutionEnvelope::seal()` today (that requires a full G1-G14..."). **Not touched — no fact exists to grade.**
- **G11 (AuditReplay)** — still a 7-line placeholder; its owning infrastructure (T7.1's unified `control_plane_audit` append-per-decision stream) doesn't exist. **Not touched.**

**Crate/application boundary discipline (architect requirement, stated mid-tranche):** every new type is a crate-owned, primitive/value-only struct (`RunbookProofInput { has_compiled_runbook_ref: bool }`, `VersionPinningInput { versions: PinnedVersionSet }`), explicitly exported per item (no wildcard re-exports), `#![deny(unreachable_pub)]` already enforced at the crate root catches any accidental leakage. `ob-poc` (the application) does 100% of the translation from its own runtime facts into these primitives at the call site (`control_plane_shadow.rs`'s `build_runbook_proof_input`/`build_version_pinning_input`) — zero new dependency direction, nothing ob-poc-shaped crosses into `ob-poc-control-plane`.

**G9's `GATE_DEPENDENCIES` edge is real, not invented:** `RunbookProof → [IntentAdmission, EntityBinding, PackResolution, DagProof, Authority, Evidence, WriteSet, DecisionSnapshot]` — traceable directly to `ControlPlaneProof`'s own field list (the artefact G9 represents literally embeds those eight proofs), the same standard the crate already applies elsewhere; contrast with Authority/Evidence, deliberately *not* linked because §6.16.1 states that relationship as conditional. G12 keeps its existing `&[]` — nothing in the spec ties version pinning to the other gates.

**Landed:** `proof::{RunbookProofInput, RunbookProofGate}`, `versioning::{VersionPinningInput, VersionPinningGate}` (full module rewrite — was a 6-line placeholder), both registered in `evaluate_shadow` (removed from `stub_ids`, which now only lists `ExecutionEnvelope`/`AuditReplay`). `EvaluationContext` gained `runbook_proof`/`version_pinning` fields; its stale T3-era doc comment corrected in place (not silently). `control_plane_shadow.rs` gained the two builder functions, wired into `phase5_runtime_recheck` alongside G1-G8/G13.

**Empirically proven, not assumed:** crate-level `runbook_proof_blocked_when_a_declared_predecessor_is_missing` proves G9 is genuinely blocked (not silently skipped) when a predecessor is absent — the same collect-where-independent proof this crate already had for G3/G4. `fully_admitted_context_succeeds_through_every_wired_gate` extended to cover all 11 now-wired gates (was 10). The T9.1/T9.5/T9.6 application-level closure test (`t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch`) extended again — now asserts `RunbookProof` and `VersionPinning` also reach real `Success` against a live cbu row; ran green against `DATABASE_URL=postgresql:///data_designer`. 4 new unit tests for the two builder functions. `cargo test -p ob-poc-control-plane` 91/91 green (verified 84/84 before this tranche via `git stash`, so 7 new crate-level tests: 3 `RunbookProofGate`, 3 `VersionPinningGate`, 1 dependency-blocking proof) plus 4 new app-side unit tests in `control_plane_shadow.rs`. `cargo clippy` clean on every touched file (both crates).

**T9.7 status: CLOSED for G9 and G12. G10 and G11 remain genuinely stubbed** (`UnimplementedGate`) pending their own owning infrastructure (T4.2/T4.3 envelope persistence, T7.1 audit stream) — not a sizing miss like G14 was, a confirmed absence of any signal to wire.

**Backlog remaining after T9.7:** G10, G11 (blocked on other infrastructure, not schedulable as shadow-wiring), G14 (deferred, needs sequencing decision — see T9.5), enforce-mode graduation (T6.4/T6.5, not schedulable — needs elapsed production time after merge/deploy). **Superseded by Addendum C / T10 below**, architect-authored and ratified same day: sequencing (c) — "envelopes enter production the same way gates did: in shadow."

## Addendum C — Tranche T10.1 CLOSED (Shadow Sealing on Path A, 2026-07-11, same day)

Basis: `EOP-PLAN-CONTROLPLANE-001_Addendum-C_Tranche-T10.md` (architect-authored, ratified sequencing (c): T10.1 → T10.2 sequential, T10.3 parallel to both). Hard boundary rules B1-B5 apply (no pub widening beyond the one sanctioned `EnvelopeRecord`/`to_record()` addition; architecture approach frozen; `dsl_v2/executor.rs` untouched; layering proven not assumed).

**The real structural gap T10.1 had to close, found during design (not assumed from the addendum text):** every gate module already has its own internal `decide()`/`classify()`/`build_pins()` function that DOES produce the typed proof value (`AdmittedIntent`, `BoundEntities`, ...) on success — `Gate<Ctx>::evaluate` just discards it, mapping to a bare `GateResult`. Sealing an `ExecutionEnvelope` needs the actual values, which `evaluate_shadow`'s report never carried. Resolved without touching the `Gate` trait (B3: no redesign) by widening each module's `decide()` visibility from private to `pub(crate)` (not `pub` — B1-compliant) and having a new `decision::evaluate()` re-invoke each one a second time — pure, deterministic functions, so a second call on the same input is guaranteed to reproduce what `evaluate_shadow`'s report already proved, at negligible cost, without duplicating `evaluate_collect_where_independent`'s already-tested dependency-blocking walk.

**B2 pub-surface judgment call, flagged explicitly for architect review, not silently decided:** B2 names exactly one sanctioned pub addition (`EnvelopeRecord` + `to_record()`). Getting a sealed envelope's decision OUT to the application (which must persist it — this crate does no I/O, §9.1) requires *some* new pub entry point beyond that. Read `decision::evaluate(ctx, validity) -> ControlPlaneDecision` as *not* a second surface-widening violation: `ControlPlaneDecision`/`ControlPlaneRejection`/`GateFailure` have been `pub` since T1 (declared, unimplemented, exactly as `decision.rs`'s own original module doc promised — "`evaluate(...) -> ControlPlaneDecision` is the crate's conceptual core API"). Wiring real logic behind an already-pub, already-declared signature is the same class of change T9.1-T9.7 made repeatedly this session (populating an already-pub `EvaluationContext`/`EvaluationReport` with real inputs), not a new kind of surface. If the architect disagrees at review, the fallback is a field on `EvaluationReport` instead — rejected here only because it would pull `envelope.rs` into `gate.rs`'s dependency direction (a real B5 layering inversion; `gate.rs` is the foundational, most-depended-on module, `envelope.rs` is the most downstream).

**`RunbookProofInput` widened (T9.7's `has_compiled_runbook_ref: bool` → `compiled_runbook_id: Option<Uuid>`):** sealing needs a real `CompiledRunbookRef`, not a presence flag. `RunbookProofGate`'s pass/fail semantics are unchanged (`Success` iff `Some(_)`); `decision.rs` gained a `proof::decide()` mirroring every other module's shape. All T9.7 call sites (crate tests, `control_plane_shadow.rs`, `sequencer.rs`) updated to pass the real `entry.compiled_runbook_id.map(|id| id.0)`.

**`EnvelopeRecord` design:** a flattened, primitive-typed **projection**, not a serde mirror of `ExecutionEnvelope` — none of the individual proof types (`AdmittedIntent`, `BoundEntities`, ...) gained `Deserialize`; only their already-plain-data fields are copied out (`envelope_id`, `verb_fqn`, `bound_entity_ids`, `pack_id`, the full `SnapshotPins` — which already had `Deserialize`, no change needed there — and the validity window). A record read back from storage can be inspected but can never be fed back into a `seal()`-equivalent constructor to fabricate a proof (§6.10.4's "rehydration only through control-plane verification" — no such verification function exists yet; not claimed).

**Persistence:** `control_plane_envelopes` (T4.2) gained a nullable `record JSONB` column (migration `20260711_control_plane_envelope_record.sql`) — deliberately additive, not a redesign of the identity-only T4.2 philosophy (pre-T10.1 rows stay `NULL`). `persist_sealed` (T4.2, previously `#[allow(dead_code)]` with its own doc admitting "no production call site yet") now serialises and stores the record; the `#[allow(dead_code)]` is gone — it has a real caller now.

**Production call site:** `sequencer.rs`'s `phase5_runtime_recheck`, immediately after the existing G1-G14 shadow-decision insert, over the exact same `cp_ctx`. A 5-minute `ValidityWindow` (matching this crate's own test convention — no production TTL policy exists to draw from otherwise). Strictly additive to the existing `tokio::spawn` background task: shadow-decision insert first, then — only on `ApprovedStp` — `persist_sealed`. **Nothing consumes, nothing gates, nothing blocks** — `legacy_outcome` (computed earlier in the function) remains the sole return value, unchanged.

**Sealable-rate telemetry (T10.1's other named exit criterion):** `control_plane_metrics::sealable_rate_by_verb` — derived from the *already-persisted* `gate_results` JSONB on `control_plane_shadow_decisions` (no new counter, can't drift from what shadow-evaluation actually observed), grouped by `verb_fqn`, counting rows where all ten gates `decision::evaluate` requires for `ApprovedStp` (`PROOF_BEARING_GATES` plus `RunbookProof` and `StpClassifier`) report `Success`. Wired into the existing `GET` control-plane-metrics route (`agent_routes.rs`) beside `gate_outcomes`/`shadow_divergence`/`envelope_status_counts`.

**Empirically proven, not assumed:**
- `decision::tests` (6 new, crate-internal): `evaluate` seals a real envelope end-to-end given a fully-legal context; returns `RequiresHumanGate` when G8 classifies `HumanGated`; rejects naming every failed/blocked gate (not just the first) when inputs are missing; rejects when the runbook reference is absent even though everything else is STP-clean; rejects when `StpClassifierInput` itself is missing.
- `envelope::tests::to_record_copies_out_plain_data_and_round_trips_through_json` — proves the record survives an actual JSON round-trip with real pins intact, and that `ExecutionEnvelope`'s own proof fields stay non-`Deserialize` (the trybuild fixtures — `seal_is_crate_private.rs`, `envelope_not_deserializable.rs` — still pass unchanged, confirming B1/B2 held).
- `control_plane_envelope_store::tests::persist_sealed_stores_a_readable_record_with_real_pins` (live-DB) — seals a real envelope with a non-empty `entity_row_versions` pin, persists it, reads the `record` column straight back out of Postgres, deserialises it, and asserts it matches `to_record()`'s own output byte-for-byte plus the specific pinned row_version.
- `control_plane_metrics::tests::sealable_rate_by_verb_counts_only_rows_with_every_required_gate_success` (live-DB) — inserts 2 sealable + 1 non-sealable shadow-decision row under a run-unique `verb_fqn` (avoiding the shared-DB pollution issue the write-attestation test above already documented), asserts the exact 2/3 split.
- Full suite: `cargo test -p ob-poc-control-plane` 100/100 (94 unit + trybuild + compile-fail). `cargo test -p ob-poc --lib --features database` 2298 non-DB tests unaffected, plus 25/25 DB-gated control-plane tests green against `DATABASE_URL=postgresql:///data_designer` with the new migration applied. `cargo clippy` clean (both crates, all touched files). `cargo tree -p ob-poc-control-plane` unchanged (B5: no new external dependency edges); `Cargo.toml` diff is empty.

**T10.1 status: CLOSED.** E-T10.1's exit criteria (sealed-record persistence with non-placeholder pins, sealable-rate metric per verb family, zero behaviour change on dispatch outcomes, B1/B2/B5 surface discipline) all met — the B2 pub-surface interpretation is the one open item flagged above for architect confirmation, not a gap in the work itself.

**Architect ruling on the B2 flag: Option A ratified (2026-07-11, same day).** Full reasoning, recorded because it sharpens what B1/B2 actually protect (not just this instance):

> `evaluate(ctx) -> ControlPlaneDecision` isn't a surface addition at all — it's V&S §9.3, verbatim: the source document declares that exact signature as "the core API shape," and T1.2 built the return types (`ControlPlaneDecision`/`ControlPlaneRejection`/`GateFailure`) as `pub` for precisely this moment. Under a direction-of-fit reading, the conformance analysis inverts: the *unimplemented* declared API was the divergence — a model-specified door existing as a hole — and implementing it is the code catching up to the model, not the boundary moving. B1 exists to stop the boundary moving *without ratification*; this boundary was ratified in v0.3 and merely unbuilt. The G1-G13 sub-tranches (T9.1-T9.7) are the same pattern: populating declared shape is instantiation, not widening.
>
> Option C (`try_seal() -> Option<EnvelopeRecord>`, narrower surface) would have been the actual deviation despite the narrower-surface instinct: it appears nowhere in the model, diverges from the §9.3 shape, and discards the rejection detail §6.14's metrics and the audit record are specified to consume. Minimality is the right default *between unratified options* — it doesn't outrank a door the specification already names.
>
> Option B's rejection (a new field on `EvaluationReport` instead of a new function) stands on B5 grounds: it would require `gate.rs` — the crate's most foundational, most-depended-on module — to import `envelope.rs`, its most downstream module. The layering check caught the inversion at design time, which is what it's for.

**Durable B1 clarification (added to this ledger so future sessions don't have to re-derive the distinction):**

> **Implementing a `pub` signature the model declares (e.g. V&S §9.3) is conformance, not surface widening — B1's stop-and-flag applies to items the model does not name.** A type or function already `pub` since an earlier tranche, whose *logic* was simply unimplemented (a documented "future" note, a stub, an `UnimplementedGate`), can be wired for real without triggering B1/B2, provided its signature was already declared and its purpose already justified by the source spec. Genuinely new items — including narrower, better-intentioned conveniences that don't appear in the model (Option C's `try_seal()` is the worked example) — remain flag-first regardless of how small they look. The test: is there a citable model section naming this shape, or would landing it require its own amendment to justify? The former is instantiation; the latter is widening.

**Three conditions attached to the ratification, addressed same day:**

1. **Owed convergence, registered MIGRATION-PENDING (not resolved this tranche):** `evaluate_shadow()` and `evaluate()` are now two parallel pub entry points into overlapping logic (both run the same gate computation; `evaluate()` additionally re-derives proof values and seals). Per the plan's own P5 pattern, shadow/enforce must end up as a *mode on one code path*, not two functions — leaving both live indefinitely would be exactly the "two APIs, one weaker" shape PIR-D-008/FIA-4B already exist to catch elsewhere in this plan. **Target for convergence: T10.2's admission-scope wrapper** — that's the point where the mode flag (shadow: observe: enforce: gate) becomes load-bearing, so it's the natural place for `sequencer.rs`'s call site to switch from calling both `evaluate_shadow` and `evaluate` separately (as T10.1 does today) to calling `evaluate()` once and deriving the shadow-decision row from its result. Not attempted this tranche — correctly scoped as T10.2's problem, not invented as new T10.1 work.
2. **Compile-probe, not assumption — landed same day:** `tests/trybuild/decision_does_not_leak_envelope_construction.rs` proves `ControlPlaneDecision` being `pub` (carrying `ExecutionEnvelope` in `ApprovedStp`) does not transitively enable constructing an envelope from outside the crate — struct-literal construction still fails on private fields regardless of the wrapping enum's visibility. Third fixture alongside the existing `seal_is_crate_private.rs`/`envelope_not_deserializable.rs`; `cargo test -p ob-poc-control-plane` still 100/100 (compile-fail harness auto-discovers `tests/trybuild/*.rs`, no harness change needed), `cargo clippy -p ob-poc-control-plane -- -D warnings` clean.
3. **This entry** is the ledger-wording condition — both the ruling's full reasoning and the durable B1 clarification above are recorded verbatim, not paraphrased down to a bare "approved," so a future session hits the same fork with the distinction already made rather than re-deriving it.


## Addendum C — Tranche T10.3 (G14 Write-Set Capture Wiring, 2026-07-11, parallel to T10.1/T10.2)

Basis: `EOP-PLAN-CONTROLPLANE-001_Addendum-C_Tranche-T10.md`, T10.3 ("no per-`SemOsVerbOp` instrumentation pass this tranche — B4-adjacent restraint"). Scope: make the T5.1-T5.3 write-set attestation machinery (`PgTransactionScope::captured_writes`/`set_expected_write_set`/`commit_attested`, previously built with zero production callers) actually reachable from real CRUD dispatch, so G14 has something real to attest against instead of only test-constructed `CapturedWrite` vectors.

**Two structural gaps found during implementation (not assumed from the addendum text), both flagged before proceeding:**

1. `PgTransactionScope::record_write` (T5.1) was an `ob-poc`-only *inherent* method. `dsl-runtime::crud_executor` — where the actual SQL dispatch happens — only ever holds `&mut dyn TransactionScope`, which has no way to reach an inherent method on a concrete type it doesn't know about. Flagged as a stop-and-flag point per Addendum C's own pre-authorized list; architect said "proceed," read as authorizing the judgment call rather than requesting another multi-option ruling. Resolved by adding `record_write(&mut self, table: &str, entity_id: Uuid, columns: &[String])` to the `TransactionScope` trait itself (`dsl-runtime::tx`) with a **default no-op body** — every other implementor (test mocks, harness executors) is behaviourally unchanged unless it opts in; `PgTransactionScope`'s trait impl now carries the real T5.1 logic (the old inherent method was removed to avoid same-name shadowing confusion).
2. Even with `record_write` on the trait, `CrudExecutionPort::execute_crud_in_scope` (T9.2) still took `conn: &mut sqlx::PgConnection`, not `&mut dyn TransactionScope` — so the CRUD executor's operation methods still couldn't reach `record_write` at all. Flagged explicitly as "compounding beyond what the last 'proceed' covered" but characterised as a direct, necessary continuation of the already-approved trait widening (not an independent new decision) rather than re-escalated. Resolved: widened the trait method's signature to `scope: &mut dyn TransactionScope`; SQL dispatch is unchanged (still goes through `scope.executor()`), the widening only adds `scope.record_write(...)` reachability. `CrudExec` enum's `Conn(&mut PgConnection)` variant renamed to `Scope(&mut dyn TransactionScope)` throughout `dsl-runtime::crud_executor`; the one production call site (`ob-poc::sem_os_runtime::verb_executor_adapter`) updated to pass the scope directly instead of pre-extracting the connection.

**What's wired (self-reported capture, honestly partial where the SQL shape doesn't cleanly yield a single entity id):**
- `execute_insert` — full coverage: raw (unquoted) column list tracked alongside the existing quoted list; `record_write` called with the inserted row's id after success. Documented over-report on the idempotent no-op-conflict fallback path (never under-reports).
- `execute_update` — captures only when the key predicate is a `SqlValue::Uuid` (the common case) and `affected > 0`; other key-value shapes are an honest coverage gap, not silently assumed.
- `execute_delete` — captures only the single-`key_column` path; the multi-condition delete branch is explicitly NOT captured (documented PARTIAL). Column list is `["deleted_at"]` for soft-delete, `[]` for hard delete (a vacuous but honest claim — hard delete has no post-write columns to name).
- `execute_upsert` — full coverage, reuses the already-raw `insert_cols` list.
- **Deliberately NOT instrumented this tranche** (matching T10.3's own B4-adjacent restraint against a full per-op instrumentation pass): `execute_link`, `execute_unlink`, `execute_role_link`, `execute_role_unlink`, `execute_entity_create`, `execute_entity_upsert`.

**What's still open (not attempted this tranche, correctly out of scope rather than silently dropped):**
- **Coverage-graded comparison logic** (`FULL`/`PARTIAL(branch)`/`NONE` per verb, per the addendum's "grade coverage honestly" requirement) — not implemented. What exists is binary: an op either calls `record_write` on the paths listed above or it doesn't; there's no typed report of *which* branch a given dispatch took.
- **`commit_attested`/`set_expected_write_set` still have zero production callers.** `sequencer.rs`'s commit path calls plain `commit()`, not `commit_attested()` — confirmed by grep, not assumed. T10.3 makes `captured_writes` populate for real CRUD dispatches; it does NOT wire G14's actual attestation gate (compare-and-possibly-reject) into the live commit path. That remains open, and per the same reasoning as T10.1's owed-convergence note, the natural landing point is **T10.2's admission-scope wrapper** (where a mode flag becomes load-bearing for the first time) rather than inventing a second ad hoc wiring point here.
- No fault-injection live-DB test proving an excess/undeclared write during a real CRUD-path dispatch is captured as a G14 mismatch — T5.1-T5.3's existing tests exercise `commit_attested` directly with hand-built `CapturedWrite`/`WriteSetProof` values, not through the CRUD executor. Not added this tranche; would need `execute_crud_in_scope` + `commit_attested` composed in one live-DB test, which presupposes the still-open production wiring above.

**Test-isolation fix, found and closed empirically (not a T10.3 addendum requirement, but blocking the tranche's own test suite):** two existing `execute_crud_in_scope_update_*` tests (`_committed_is_durable`, `_rolled_back_leaves_no_durable_trace`) both queried `SELECT cbu_id, description FROM "ob-poc".cbus LIMIT 1` — no `ORDER BY` — to pick "the first cbu row." Under parallel `cargo test` execution both tests raced on the *same* physical row: the committed test's real, durable write could land on the row between the rollback test's pre-write read and its post-rollback verification read, making the rollback test observe the committed test's marker instead of the true original value. Bisected via `git stash` (base commit: parallel-run flaky-but-mostly-passing across a small sample; my changes: failed consistently — same underlying pre-existing race, just perturbed into reliable reproduction by different timing, not a new bug in the capture logic itself). Matches the project's own documented PIR-D-004 precedent (shared mutable test fixtures racing under parallel execution — fix the test, not the product). Fixed by giving each test a deterministic, distinct row (`ORDER BY cbu_id LIMIT 1 OFFSET 0` vs `OFFSET 1`). Verified: 3/3 repeated `cargo test -p dsl-runtime --lib --features database -- --ignored crud_executor` runs green (default parallel threads, no `--test-threads=1` workaround needed).

**Empirically proven, not assumed:**
- `cargo test -p dsl-runtime --lib --features database` — 185 passed, 0 failed, 6 ignored (the DB-gated ones, run separately below).
- `cargo test -p dsl-runtime --lib --features database -- --ignored crud_executor`, DATABASE_URL live: 3/3 tests green across 3 repeated runs (race closed).
- `cargo test -p ob-poc --lib --features database -- --ignored sequencer_tx::t5_write_set_attestation_tests`, DATABASE_URL live: 3/3 green (T5.1-T5.3 still hold after `record_write`'s trait relocation and `execute_crud_in_scope` signature widening).
- `cargo build --workspace`: clean.
- `cargo clippy -p dsl-runtime -- -D warnings`: clean (every file this tranche touched). `cargo clippy -p ob-poc --lib -- -D warnings` fails on 5 pre-existing errors in files this tranche did not touch (`gleif/types.rs` duplicated `#[allow(dead_code)]`, `domain_ops/kyc_stream_ops.rs` too-many-arguments, `sequencer_stages.rs` enum-variant-names) — confirmed pre-existing via `git stash` + re-run against the base commit (identical 5 errors), not introduced by this tranche.
- **B4**: `git diff --name-only` confirms `rust/src/dsl_v2/executor.rs` untouched.
- **B5**: `cargo tree -p dsl-runtime` carries no dependency edge onto `ob-poc` — the trait (`TransactionScope::record_write`, `CrudExecutionPort::execute_crud_in_scope`) is defined in `dsl-runtime`; `PgTransactionScope` in `ob-poc` implements it. Layering direction proven, not assumed.
- **B1**: both widenings are additive (`record_write` ships a default no-op; `execute_crud_in_scope`'s signature change is a supertype widening of what the caller already held, `&mut PgConnection` was always reachable via `scope.executor()`) and were flagged/justified in-session before landing, not made silently.

**T10.3 status: capture-wiring landed, attestation-gate wiring and coverage-grading remain open — correctly deferred to T10.2, not silently dropped.** Uncommitted at time of writing (`git status`: `tx.rs`, `port.rs`, `crud_executor.rs`, `sequencer_tx.rs`, `verb_executor_adapter.rs` all modified, unstaged).

## Addendum C — Tranche T10.2 (Pin Verification Wired to Real Envelopes, 2026-07-11, sequential after T10.1)

Basis: `EOP-PLAN-CONTROLPLANE-001_Addendum-C_Tranche-T10.md`, T10.2 — "T10.2 is the first consumer of `record`" (T10.1's own closing note, `persist_sealed`'s doc comment). Scope: `verify_pins_in_scope` (T9.2 §5/§5a — locked, scope-threaded pin re-check) was built and tested as a standalone primitive but had **zero production call sites**; `EnvelopeHandle` deliberately carries only id + content hash, not `SnapshotPins`, so nothing populated the pins `verify_pins_in_scope` needs at the one place that could actually call it (`execute_verb_admitting_envelope`). T10.1's `record` column (an `EnvelopeRecord` serialised from a real sealed envelope, including its real `SnapshotPins`) closed exactly that gap — T10.2's job is wiring the two together.

**What changed (all intra-`ob-poc`, no crate-boundary pub surface touched — B1 not implicated):**

1. `control_plane_envelope_store::try_consume_in_scope_with_pins` — wraps the existing `try_consume_in_scope`, then (only on `Consumed`) reads the same row's `record` column back over the *same connection, inside the same transaction*, against a row `consume_core`'s own `SELECT ... FOR UPDATE` already holds locked — so the follow-up read observes exactly the row just consumed, not a possibly-different concurrent write. Deserialises into `EnvelopeRecord`, returns `record.snapshot` (`SnapshotPins`). A `NULL`/missing/malformed `record` degrades to `None` — logged, not a hard failure — matching pre-T10.1 rows and `verify_pins_in_scope`'s own "empty pins never drift" posture, rather than failing dispatch over bookkeeping that predates pin capture.
2. `check_admission_in_scope` widened from returning a bare `AdmissionDecision` to `(AdmissionDecision, Option<SnapshotPins>)` — the pins ride alongside the decision so the caller (which owns the scope the write will run in) can act on them.
3. `ObPocVerbExecutor::admit_in_scope` widened to return `Result<Option<SnapshotPins>>` instead of `Result<()>` — same reasoning.
4. `execute_verb_admitting_envelope`: after a successful `admit_in_scope`, if pins were recovered, calls `ob_poc_boundary::toctou_recheck::verify_pins_in_scope(&pins, scope.executor())` — inside the same scope, before branch dispatch, per §2's one-scope-before-branching ordering (`BEGIN → consume → verify pins → dispatch → COMMIT`, exactly the design doc's §6 ordering). On drift or a vanished pinned row, rolls the whole scope back (same rollback-retry corollary T9.2 already proved for dispatch failures — a pin-drift rejection leaves the envelope reconsumable, not burned) and returns a rejection naming the cause.

**Not attempted this tranche, correctly deferred rather than dropped:**
- **The owed `evaluate_shadow()`/`evaluate()` convergence** (registered MIGRATION-PENDING at the B2 ratification, targeted at "T10.2's admission-scope wrapper" as the point where a mode flag becomes load-bearing). Re-examined at the start of this tranche: `execute_verb_admitting_envelope` is a *dispatch-time* admission wrapper (consume + pin-verify + write), not the *shadow-evaluation* call site (`sequencer.rs`'s `phase5_runtime_recheck`, which is pre-dispatch, read-only, and runs regardless of enforcement). The two remain genuinely different call sites serving different purposes (shadow observation vs real admission) — the convergence this owed item describes is about `sequencer.rs` calling `evaluate()` once instead of `evaluate_shadow()` + `evaluate()` separately, which is unrelated to *this* tranche's pin-wiring work and was not silently bundled in. Still open; re-flagging rather than closing on a technicality.
- **OQ4-adjacent durable/park interaction with pin verification specifically**: T9.2's OQ4 (durable verbs bypass the outer scope via `ctx.allow_durable_direct`) was resolved by inspection for the admission/write half; pin verification rides the same scope, so the same resolution covers it — no new park/resume gap introduced, but not re-verified with a dedicated probe this tranche (the existing OQ4 resolution reasoning transfers, not re-derived).
- Production graduation (enforcing any real verb via `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`) remains out of scope — `EnforcedVerbs::from_env()`'s production default (unset/empty) means this entire pin-verification path is presently unreachable in production, same shadow-first posture as every other T9/T10 sub-tranche.

**Test-isolation fix, found and closed empirically (blocking this tranche's own test suite, not a T10.2 requirement per se):** `t4_1_envelope_admission_tests`' `EnvGuard` mutated the process-global `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` env var with cleanup-on-`Drop` but no mutual exclusion between tests — under default parallel `cargo test`, one test's `set()` could land between another's `set()` and its own assertions, or `shadow_default_admits_every_verb_with_no_envelope` (which doesn't use `EnvGuard` at all) could run while a different test held the var set to something else. Same PIR-D-004 shape as the T10.3 race fixed earlier this session (shared mutable test fixture racing under parallel execution — fix the test, not the product). Fixed with a `static ENV_GUARD_LOCK: Mutex<()>` held for `EnvGuard`'s lifetime, and an explicit lock acquisition added to `shadow_default_admits_every_verb_with_no_envelope`. Verified: 3/3 repeated runs of `t4_1_envelope_admission_tests` + `control_plane_envelope_store` together (23 tests) green under default parallel `cargo test`.

**Empirically proven, not assumed:**
- `check_admission_in_scope_recovers_real_pins_from_the_record_column` (live-DB, new) — seals a real envelope with a real pinned entity via the full proof-type ceremony (not a hand-built fixture), persists it, admits it through `check_admission_in_scope`, and asserts the recovered `SnapshotPins::entity_kinds_and_versions()` matches exactly what was sealed.
- `execute_verb_admitting_envelope_rejects_on_pin_drift_and_leaves_envelope_reconsumable` (live-DB, new) — the actual end-to-end proof T10.2 exists for: seals a real envelope pinning a real `cbus` row at `real_row_version - 1` (deliberately stale, simulating a concurrent writer having moved the row since gate time), dispatches `cbu.confirm` through the real `execute_verb_admitting_envelope` production entry point, asserts it rejects with a pin-drift-specific message (not dispatch failure, not admission failure) *before* the verb's own write runs, and then re-admits the same handle successfully — proving the whole scope, including the consume, rolled back together rather than partially admitting.
- Full suite: `cargo test -p ob-poc --lib --features database` — 2298 non-DB tests green (0 regressions); the 23-test DB-gated cluster (`control_plane_envelope_store` + `t4_1_envelope_admission_tests`) green across 3 repeated parallel runs.
- `cargo build --workspace`: clean. `cargo clippy -p ob-poc --lib -- -D warnings`: the same 5 pre-existing errors as T10.3 (confirmed by `-->` path: `gleif/types.rs`, `domain_ops/kyc_stream_ops.rs`, `sequencer_stages.rs` — none touched by this tranche).
- **B4**: `git diff --name-only | grep dsl_v2/executor.rs` — 0 hits, untouched.
- **B5**: this tranche touched no `dsl-runtime` files; `cargo tree -p dsl-runtime` carries no `ob-poc` dependency edge (unchanged from T10.3's check).
- **B1**: every change this tranche is intra-`ob-poc` (`agent::control_plane_envelope_store`, `sem_os_runtime::verb_executor_adapter`, both `pub(crate)`/private) — no crate-boundary `pub` surface widened. `ob_poc_boundary::toctou_recheck::verify_pins_in_scope` was already `pub` (T9.2); this tranche is its first real caller, not a new export.

**T10.2 status: pin-verification wiring landed and empirically proven end-to-end. Owed `evaluate_shadow`/`evaluate` convergence remains open (correctly re-deferred, not this tranche's call site).** Uncommitted at time of writing (`git status`: `agent/control_plane_envelope_store.rs`, `sem_os_runtime/verb_executor_adapter.rs` modified, unstaged).

## Addendum C — owed `evaluate_shadow()`/`evaluate()` convergence CLOSED (2026-07-11, same day as T10.2)

Registered at the B2 ratification (T10.1's closure entry, condition 1) as MIGRATION-PENDING: `evaluate_shadow()` and `evaluate()` were two parallel `pub` entry points into overlapping logic — `sequencer.rs`'s `phase5_runtime_recheck` called `evaluate_shadow(&cp_ctx)` once (for the shadow-decision audit row) and `decision::evaluate(&cp_ctx, validity)` again (for the sealed-envelope-or-rejection), silently repeating the identical dependency-aware gate walk (`evaluate_shadow` is exactly what `evaluate`'s first line calls internally). T10.2's ledger entry re-examined and re-deferred this, on the grounds that T10.2's own call site (dispatch-time admission) was a different, unrelated function from the shadow-evaluation call site the convergence actually targets — correct, but left the debt itself still open with nowhere assigned. Closed now, as its own small unit of work, once T10.2 was safely landed and there was no risk of conflating the two.

**What changed:** `decision.rs` gained `evaluate_with_report(ctx, validity) -> (EvaluationReport, ControlPlaneDecision)` — the full body `evaluate` used to have, unchanged line-for-line except every return point now also carries the `report` it already computed. `evaluate(ctx, validity) -> ControlPlaneDecision` is now a one-line wrapper (`evaluate_with_report(ctx, validity).1`) — same public signature, same behaviour, zero breaking change for existing callers/tests (all of `decision.rs`'s own pre-existing tests, which call `evaluate` directly, needed no changes). `sequencer.rs`'s `phase5_runtime_recheck` switched to calling `evaluate_with_report` once, using its `report` for `build_shadow_decision_row` (previously a separate `evaluate_shadow` call) and its `decision` for the sealing branch — one gate-walk per verb dispatch instead of two.

**Why a new function rather than widening `evaluate`'s own return type:** widening `evaluate` itself to return a tuple would have broken every existing `ControlPlaneDecision`-only caller and test (`decision.rs`'s own ~10 unit tests, `envelope::tests`, the trybuild fixtures reasoning about `ControlPlaneDecision`'s shape). A new function is additive — B1-compliant, not requiring the same flag-first ratification the original `evaluate` addition needed (that one instantiated a §9.3-declared-but-unbuilt signature; this one is a pure implementation-detail extraction with no model section of its own to conform to or diverge from, and doesn't touch the crate's declared public API shape beyond one new function).

**Empirically proven, not assumed:**
- New test `evaluate_with_report_matches_the_separate_evaluate_shadow_and_evaluate_calls` — asserts `evaluate_with_report`'s `report` is byte-for-byte equal (`EvaluationReport: PartialEq`) to a standalone `evaluate_shadow` call, and its `decision` seals the same intent/binding as a standalone `evaluate` call — proving the convergence changed *how many times* the gate walk runs, not *what* it produces.
- `cargo test -p ob-poc-control-plane`: 100 unit (99 pre-existing + 1 new) + 1 trybuild + 0 doctests, all green — every pre-existing `evaluate`-calling test needed zero changes.
- `cargo build -p ob-poc --lib --features database` and `cargo build --workspace`: clean after the `sequencer.rs` call-site switch.
- `cargo clippy -p ob-poc --lib -- -D warnings` and `cargo clippy -p ob-poc-control-plane -- -D warnings`: both clean (this closes out the same clippy-debt pass that fixed the 5 pre-existing failures earlier the same day).
- **B4**: `git diff --name-only | grep dsl_v2/executor.rs` — 0 hits.
- **B5**: `cargo tree -p ob-poc-control-plane` carries no `ob-poc` dependency edge — the crate's layering is unaffected by this change (it's a pure intra-crate function split).

**Convergence status: CLOSED.** No further owed items remain from the T10.1 B2 ratification's three conditions.

## MCA-001 addendum — AB5 classification rule (standing) + verdict (2026-07-11, same day)

**Standing classification rule for agent-local vs. operational session-state writes** (ratified in response to MCA-001's E-3 escalation): a write is **operational** if any capability, gate, or audit-of-record path reads it — i.e. the write can influence a decision. A write is **agent-local** only if the sole consumer is the agent tier itself, for conversational continuity, with no downstream decision consuming it. Durability and DB-persistence are irrelevant to the classification; readership is the only test. This rule governs every future AB5-shaped classification, not just the instance below.

**Applying the rule — traced, not assumed:** MCA-001 (AB5) found a real Postgres-backed session checkpoint write mid-clarification-loop (`persist_session_checkpoint_inner` → `save_session_snapshot`, `src/sequencer.rs:785-845`) with no CP crossing. Traced its readers per the rule's trap-check ("does this feed anything G1's admission consumes — the session verb surface"):

1. The checkpoint persists `ReplSessionV2`'s full `state` (including the clarification loop's `ScopeGate { pending_input, candidates }`, `src/sequencer.rs:4237-4240`) into `"ob-poc".repl_sessions_v2.state` JSONB.
2. `SessionRepository::load_session` (`src/repl/session_repository.rs:235-294`) reads that row back and reconstructs `ReplSessionV2.scope`/`.stage_focus` (among other fields) on session resume.
3. Those exact fields feed `VerbSurfaceContext` at `src/agent/orchestrator.rs:435-448` (`stage_focus: ctx.stage_focus.as_deref()`, `has_group_scope`/`is_infrastructure_scope` derived from `ctx.scope`).
4. `compute_session_verb_surface(&surface_ctx)` (`src/agent/verb_surface.rs:324`) consumes that context to produce `SessionVerbSurface.allowed_fqns()`.
5. That surface directly gates dispatch-adjacent behaviour: `orchestrator.rs:1846-1865` narrows `surface_allowed` to a constrained-match verb only when `surface_allowed.contains(constrained_verb)` — the checkpoint-derived surface is load-bearing for what a subsequent turn is even allowed to search/consider.

**Verdict: AB5 is NONCONFORMANT** (not MODEL-SILENT, per MCA-001's original hedge — resolved by applying the ratified rule). The checkpoint write is decision-relevant by the readership test, therefore operational, therefore should cross the CP under AB5 as written, and currently does not. Added to the T11 mesh-retirement backlog (MCA-4) as a fourth open item, same severity class as AB4/AB7 (moderate — not L2/§9.4/§6.7.1, so not BLOCKER-tier) — no retirement path registered yet.

**Also worth noting for the T11 scoping pass**: `ob_poc_sage::session_context::load_entity_states_for_group` (the AB4 finding's own crate) is itself a *second*, independent decision-relevant read, found while tracing this rule — its output (`entity_states`) feeds `compute_valid_verb_set_for_constellations` → `resolve_constrained_hybrid`, whose resolved verb is checked against `surface_allowed` at the same `orchestrator.rs:1846` site. AB4 and this AB5 finding converge on the identical gate (`surface_allowed`/`SessionVerbSurface`) — reinforcing the architect's ruling that AB2's tier-split is the correct root remedy: once direct `sqlx` is unreachable from the agent tier (AB2's fix), both AB4's and this AB5 instance's violations close as a structural consequence, not two separate patches.

## v0.4 ratified — Amendment 1, Clearing-House Mandate (2026-07-11, same day as MCA-001)

`EOP-VS-CONTROLPLANE-001` is now **v0.4** (`docs/architecture/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.4.md`, renamed from v0.3 via `git mv` to preserve history). Deltas applied mechanically per Amendment 1's own change-log entry:

- **New §15 — Target Topology: The Clearing House.** §15.1 mediation topology (utterance-level interception, not per-verb checkpoint); §15.2 leakproof L1 (dependency-direction lock)/L2 (keyed doors)/L3 (lateral surface deletion); §15.3 coverage C1 (compile-time)/C2 (runtime provenance attestation, metric `capability_invocations_without_cp_provenance`, alert threshold zero)/C3 (audit closure); §15.4 pack universality K1–K3; §15.5 **ratified R-a** (typed read-only lenses — architect's standing recommendation, strengthened by MCA-001's AB4/AB5 findings being read-path violations with a concrete conformance target under R-a); §15.6 migration posture (checkpoint topology, T0–T10, reclassified as explicitly transitional en route to mediation, not a rival design).
- **§8 relationship directions inverted** — 8.1 (Sage invoked BY the CP, holds no capability keys), 8.5/8.6 (compiler/runtime invoked by, and only by, the hub on agent-originated flows) — each edit marked in-document as a v0.4 inversion rather than silently rewriting v0.3's original framing.
- **§12 criteria 13–15 added** (L1 graph-gate green; C2 ≡ 0 over a graduation window on all packs; K3 zero-coverage-work pack onboarding, evidenced).
- **§3/§4 blockquote strengthened**: "…and the Control Plane is the only party that can ask it to."
- Change-log entry added verbatim per Amendment 1's own specified text.

**Ratification checklist, all four items closed same day:** §15.1 mediation topology ratified as target state; §15.5 ratified R-a; §15.6 checkpoint work (T0–T10) confirmed transitional, relocating not discarded; sonnet-session authorized to apply deltas — executed.

**MCA-001's E-1 escalation is CLOSED** as a direct consequence — `docs/research/model-conformance-mca-001.md` updated to reflect v0.4 now being the checked-in precedence-1 document.

**Sequencing, per architect direction — NOT executed yet, explicitly deferred:** the full MCA run (L1–L3, C1–C3, K1–K3 crate-dependency census, beyond what AB2/AB4 touched) has not been run against v0.4. The T11 mesh-retirement plan must be cut from the *complete* MCA-4 gap register, not from MCA-001's AB-scoped slice alone — do not cut T11 before that full run. T11.0 (build the C2 provenance metric, `capability_invocations_without_cp_provenance`) is already known as the opening item regardless of what else the full run finds ("measure before retiring, retire before locking") — but per the stated order (ratify v0.4 → commit MCA report/E-3 trace → run full MCA → cut T11), this has not been started.

## MCA-002 — full topology run against v0.4, COMPLETE (2026-07-11, same day)

`docs/research/model-conformance-mca-002.md` — the full MCA run architect direction deferred behind v0.4's ratification (L1–L3 leakproof, C1–C3 coverage, K1–K3 pack universality, plus a mechanism-clause spot-check re-confirming §9.4/§6.16.1/B1–B5 still hold). Result: **9 new NONCONFORMANT findings**, all in the newly-ratified §15 topology layer (expected — v0.4 was ratified same day, nothing has yet targeted it). Combined with MCA-001's AB-family findings, the complete gap register (MCA-4, both documents) stands at **13 NONCONFORMANT + 1 MIGRATION-PENDING, zero unresolved, 22/22 clauses executed across both runs**.

**Headline findings:** L1/L2 (BLOCKER) generalize AB2's Sage-specific finding to the whole workspace — `ob-poc-control-plane` has zero resolved reverse-dependents (`cargo tree -i`), and three separate crates (`ob-poc` root, `ob-poc-agent`, `ob-poc-web`) hold direct capability-crate deps outside any CP mediation. C1 fails by necessity (no L1 gate exists to be green). C3 is the most consequential non-BLOCKER finding: the *only* CP-evaluation call site anywhere in the codebase is `src/sequencer.rs:7103`, and its own module doc states persistence is best-effort/never-propagated — meaning "every agent-originated transition has a reachable CP decision record" is not merely unbuilt but structurally contradicted by the current design's explicit posture. Filed as escalation E-4 (best-effort vs. hard-guarantee tension needs an architect ruling, not more tracing).

**T11 plan is now unblocked to cut**, per the architect's own precondition ("the mesh you retire must be the measured one, not the remembered one") — this measurement is complete. Not cut as part of this audit entry — a separate implementation-planning artifact, per instruction not to conflate audit output with tranche planning. T11.0 (build the C2 metric) remains the architect-mandated opening item of whatever tranche plan follows.

## Tranche T11.1a — Boundary Map (design pass, no code), DRAFT for ratification (2026-07-11, same day)

Basis: `EOP-PLAN-CONTROLPLANE-002_v0.1` (T11: Mesh Retirement / Leakproofing), T11.1a. Full map at `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T11.1a-BOUNDARY-MAP-001.md`.

**Hard blocker found before any T11 code, per the plan's own B7:** the v0.4.1 micro-amendment (C3's constitutive clarification, per MCA-002's escalation E-4) has not been supplied to this session — E-4 itself is still an open escalation, not a ruling. T11.0 (the mandated opener, C2 metric instrumentation) is code and therefore also blocked by B7, not started. **Only T11.1a proceeded**, since the plan itself marks it "design pass, no code."

**Census (36 top-level `ob-poc` `src/` modules, ~278K lines):** 8 modules classified agent-tier at high/medium-high confidence, ready for T11.1b extraction once ratified (`sage/`, `journey/`, `lookup/`, `navigation/`, `plan_builder/`, `research/`, `semtaxonomy_v2/`, `acp_runtime_context.rs`). 4 modules flagged as genuinely MIXED, needing their own finer-grained pass rather than force-classified this pass (`agent/` — specifically whether `orchestrator.rs`, the single largest/most load-bearing file in the census at 4,890 lines, is agent-tier-with-lensing or permanently CP-adjacent; `dsl_v2/` — split cleanly at the executor boundary, lower priority; `mcp/` — open question whether the MCP tool surface is T11 or T12 scope; `repl/` — couples directly with T11.4's AB5 remediation, same code as `SessionRepository::load_session`). 8 modules CP-adjacent (routing/glue, not extraction targets, become T11.2 keyed-door consumers instead — includes `sequencer.rs` itself, definitionally hub-shaped under v0.4 §15.1). 10 modules capability-tier (stay in place). 4 infrastructure-neutral. 1 dead/empty (`constellation/`, zero files, flagged as unrelated hygiene).

**`sage/`'s own internal finding, resolved by cross-reference rather than treated as new:** `sage/mod.rs` itself has zero real capability imports (its only `sqlx::` grep hit is a comment, independently re-verified) — the module's actual capability contact is exactly MCA-001's already-identified AB4 finding (`session_context` re-export) plus test-only scaffolding in `valid_verb_set.rs` already confirmed non-production-reachable. No new finding; existing T11.3 scope covers it.

**Three architect decisions requested, not resolved unilaterally:** (1) whether the 8-module high-confidence list is sufficient for a first T11.1b slice with the 4 flagged modules deferred to a follow-up pass (design recommends yes); (2) whether `mcp/`'s interpretation-shaped surface is T11 or T12 scope — the plan's own text doesn't name MCP; (3) `agent/orchestrator.rs`'s long-term tier classification — the highest-leverage call in the whole map, not resolvable from a directory-grain census alone.

**T11.1a status: DRAFT, awaiting architect ratification.** T11.1b (mechanical extraction) does not start until ratified, per the plan's own text. No other T11 work started, per B7.

## v0.4.1 ratified — C3 constitutive clarification, ruling on E-4 (2026-07-11, same day)

`EOP-VS-CONTROLPLANE-001` is now **v0.4.1** (`docs/architecture/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.4.1.md`, renamed from v0.4 via `git mv`). Ruling on MCA-002's escalation E-4 (best-effort persistence vs. C3's hard audit-trail guarantee — the two cannot both be true given `src/sequencer.rs:7103` is the sole CP-evaluation call site and its own module doc states persistence is best-effort/never-propagated): **option (b) — C3 is constitutive of mediation completion, not an obligation of checkpoint topology.** Best-effort, non-blocking shadow-decision persistence is the conformant posture through T11; C3 becomes a hard guarantee only once mediation (§15.1) is live and audit-trail reachability holds by construction rather than by a persistence-layer promise. §15.3's C3 clause amended in place with this clarification; change-log entry added.

**Consequence for MCA-002:** C3's verdict reclassified NONCONFORMANT → **MIGRATION-PENDING**, sharing AB1's T12/mediation terminus — passes the same two-condition test (explicitly transitional per the amended §15.3 text; tracked with a stated terminus). Not a T11 exit criterion. Complete MCA-4 gap register (MCA-001 + MCA-002 combined) now stands at **12 NONCONFORMANT + 2 MIGRATION-PENDING (AB1, C3)**, zero unresolved. `docs/research/model-conformance-mca-002.md` updated in place to reflect the ruling.

**Unblocks B7.** `EOP-PLAN-CONTROLPLANE-002_v0.1` (Tranche Series T11)'s hard rule B7 — "the v0.4.1 micro-amendment... is applied and committed BEFORE any T11 code lands" — is now satisfied. T11.0 (C2 provenance metric) and T11.1b (mechanical extraction, pending T11.1a's ratification) may proceed.

## Tranche T11.0 — C2 provenance metric, LIVE (2026-07-11, same day, unblocked by v0.4.1)

Basis: `EOP-PLAN-CONTROLPLANE-002_v0.1`, T11.0 ("mandated opener: measure before retire"). First code to land under B7 (v0.4.1 ratified same day, immediately prior).

**Mechanism:** `src/agent/capability_provenance.rs` — in-process counters (`std::sync::LazyLock<Mutex<HashMap<String, ProvenanceCounts>>>`, no new dependency), keyed by capability-entry name, split by `has_cp_provenance`. Deliberately in-process, not DB-persisted: `PgTransactionScope::begin` is a hot path (every transaction workspace-wide) and adding synchronous DB I/O purely for a counter would violate the same "never blocks the calling turn" discipline T10.1's shadow-evaluation mechanism already established. `record_capability_invocation`/`capability_invocations_without_cp_provenance` (the mutator/reader pair) wired into `PgTransactionScope::begin` and `begin_timeout` (`src/sequencer_tx.rs`) — every scope-mediated capability invocation, covering the SemOS-native-ops and CRUD-fast-path dispatch branches.

**Honest about pre-L2 limits, per the plan's own text ("instrumented, not structural"):** every recorded invocation today is `has_cp_provenance = false` — no marker exists anywhere for a CP-mediated path to set (confirmed by MCA-002: the sole CP-evaluation call site runs async, after dispatch already completed, so it structurally cannot mark a context dispatch will later see). This is the expected, correct state pre-T11.2, not a bug — "the number will be large; that is the point" (T11.0's own text). Becomes structural, not merely instrumented, the moment T11.2's keyed doors land (holding a `CapabilityInvocation` becomes the thing this module checks for).

**Known coverage gap, recorded not hidden:** direct pool-based capability access that never opens a `PgTransactionScope` — `ob_poc_sage::session_context`'s raw `sqlx::PgPool` queries (AB4), the legacy pool-based `execute_crud`/`check_admission`/`try_consume` variants (T9.2's "permanent, not debt" primitives) — is NOT counted by this first slice. Not folded into the metric as a fabricated zero; T11.2/T11.3 naturally reduce this gap as a side effect of keying/lensing those exact call sites.

**Baseline:** mechanism proven live via a real live-DB test (`begin_records_a_capability_invocation_without_cp_provenance`, `src/sequencer_tx.rs`) — a real `PgTransactionScope::begin()` call increments the counter by exactly one, verified against a before/after snapshot. A meaningful *production* baseline (the actual opening mesh-remainder number the plan's exit criteria asks the ledger to record) requires the live web service under real traffic — process-lifetime counters reset on restart, so this session's own test-process counts are not that number. Recorded honestly as **not yet captured** rather than fabricated; whoever next deploys/operates the service should read `GET` control-plane-metrics and record the first real number here.

**MCA clause C2 re-executed:** the metric now exists (`grep -rn "capability_invocations_without_cp_provenance"` — real hits in `capability_provenance.rs`, wired through `agent_routes.rs`'s `ControlPlaneMetricsResponse`), moving the finding from "metric-absent" to "metric live, baseline pending first production read" — not yet "measured," since no production baseline exists. `docs/research/model-conformance-mca-002.md` not yet updated to reflect this (the metric's *existence* changes C2's proof-method answer, but MCA-002's own re-execution is properly Tranche Series T11's own exit criterion E-T11.A, a full MCA-002 re-run at series completion — not updated piecemeal per sub-tranche).

**Verification:** `cargo build -p ob-poc --lib --features database` clean. `cargo test -p ob-poc --lib --features database`: 2300/0 (2 new unit tests). Live-DB test green. `cargo clippy -p ob-poc --lib -- -D warnings` clean. B4 confirmed (`dsl_v2/executor.rs` untouched).

**T11.0 status: mechanism live, first exit criterion (metric live) met; second (baseline captured in ledger) explicitly not met, honestly recorded as pending real deployment traffic rather than closed on a fabricated number.**

## Tranche T11.F.1 addendum — synthetic-corpus caveat, and graduation-window status (2026-07-12)

**T11.F.1's evidence check (this session) used entirely synthetic, session-local data.** The 45 rows queried from `"ob-poc".control_plane_shadow_decisions` in this dev database are exclusively this session's own test-fixture inserts (`cbu.confirm` from `decision.rs`/envelope-store tests, run-unique `test.sealable-rate-*` fixtures) — zero real production traffic. The finding itself (zero G1/G3/G4 `Failure` results anywhere; the only `Failure` in the whole dataset is `Authority: Failure("denied")`, a judgmental gate) matches T11.F's own prediction ("expected: ~zero"), but the evidentiary weight is "confirms the premise against synthetic test data," not "validated against live usage." Recorded so a future reader doesn't mistake this for a production-traffic finding.

**No graduation window has genuinely started anywhere.** Cross-referencing this session's own T8/T9 entries above: `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` is empty by production default everywhere; the branch this work lives on (`codex/phase-1-5-governance-closure`) is not merged to `main`; the ≥500-shadow-evaluation graduation criterion requires elapsed production time after merge/deploy, which has not happened. Every "shadow-first," "checkpoint topology," and now "definitional floor" claim in this ledger describes code that is real, tested, and correct — none of it describes a system currently observing real production traffic. Recorded as a standing caveat, not a new finding — the individual entries above already say this piecemeal; this line exists so it's findable in one place.

## Tranche T11.F.2 — Rejection design, DRAFT for review (2026-07-12)

Basis: `EOP-PLAN-CONTROLPLANE-002_v0.1`, T11.F.2. Design-only, per the requested sequencing ("Sonnet writes the T11.F.2 rejection design... sends it here for review. That's the single next action"). No code lands with this entry. Full document: `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T11.F.2-DEFINITIONAL-FLOOR-001.md`.

**Headline finding, reshapes the floor's scope from the plan's own one-line description:** investigating each floor gate's real `decide()` logic (not assumed from "G1/G3/G4 enforce unconditionally") found all three conflate a genuinely definitional outcome with genuinely judgmental outcomes inside the same enum — and **G1 specifically has a previously-undiscovered dead-code bug**: `intent_admission.rs::decide()` string-matches `exclusion_reasons` against literals `"unknown_intent"`/`"outside_pack"`/`"deprecated"`, but the real producer (`control_plane_shadow.rs:534-539`) Debug-formats `PruneReason`'s four actual variants (`AbacDenied`/`EntityKindMismatch`/`AgentModeBlocked`/`PolicyDenied`) — none of which ever equal those literals. Every real exclusion today falls through to `RejectedUnauthorisedSurface` regardless of true cause; **G1's own `decide()` cannot currently distinguish "verb doesn't exist" from "verb exists but is policy-denied."** Applying the floor to "G1 as coded" would have made ABAC/policy denials hard-reject unconditionally — exactly what T11.F's own text forbids for judgmental gates.

**Design response:** the floor's G1 check bypasses `decide()` entirely, using a direct `runtime_registry().get(domain, verb).is_some()` lookup (the plan's own "registration-gap fast path" language, now given a concrete mechanism) — genuinely definitional, no ABAC/pack/policy context needed. G1's existing gate (bug included) is untouched; fixing the dead code is flagged as separate, small follow-on work, not required for the floor's correctness since the floor doesn't route through it. G3 scoped to `MissingPack`/`AmbiguousPack` only (`PackDeniesIntent`/`PackDeniesEntity` stay judgmental). G4 scoped to the five topological outcomes, with `GuardFailed` conservatively treated as judgmental pending one more producer-trace on `blocking_violations` (flagged, not resolved).

**Also covered:** rejection shape per ingress (4 production ingresses + the KNOWN-BYPASS explicitly out of scope, consistent with the plan's own "floor coverage rides T11's coverage arc"); the synchronous-vs-async ordering problem at Path A (`phase5_runtime_recheck` is async/post-hoc and evaluates judgmental gates too — the floor needs its own narrower, synchronous pre-check, not a blanket change to that function); §6.13 routing recommended as synchronous-rejection-plus-audit-record (no new queue — none exists, and building one speculatively for a floor whose whole premise is "definitional failures shouldn't happen on real traffic" would be oversized); the audit record reuses `control_plane_shadow_decisions` (new `floor_rejected` boolean) but requires a real behavioural change from the existing best-effort persistence posture — failure must be visible (warning/alert), not silently swallowed, per §12's own auditability closing sentence; a 10-item fault-injection matrix including two explicit negative controls (ABAC-denied and PackDeniesIntent traffic must NOT be floor-rejected) and a grep gate proving no env/flag conditional guards the rejection branch.

**T11.F.2 status: DESIGN COMPLETE, sent for review. Implementation queued behind clean review, per the requested sequencing.**

## Defect register — G1 (Intent Admission) NONCONFORMANT: `decide()` cannot discriminate exclusion reasons (2026-07-12)

**Verdict: NONCONFORMANT against v0.4.1's own G1 clause.** Discovered by T11.F.2's design work (routing the definitional floor around G1's gate, not by an MCA audit pass) — recorded here as its own defect entry per the architect's ruling that the conflation finding "does not get to live as routed-around-and-forgotten."

**The defect:** `crates/ob-poc-control-plane/src/intent_admission.rs::decide()` selects between `RejectedUnknownIntent`/`RejectedOutsidePack`/`RejectedDeprecated`/`RejectedUnauthorisedSurface` by string-matching `input.exclusion_reasons` against the literals `"unknown_intent"`, `"outside_pack"`, `"deprecated"`. The real producer of `exclusion_reasons` (`control_plane_shadow.rs:534-539`) is `Debug`-formatted `PruneReason` — whose four actual variants (`AbacDenied {..}`, `EntityKindMismatch {..}`, `AgentModeBlocked {..}`, `PolicyDenied {..}`) never equal any of those three literals. **Every real production exclusion falls through to `RejectedUnauthorisedSurface`, regardless of true cause.** `decide()` has never, in production, correctly discriminated "verb doesn't exist" from "verb exists but is policy/ABAC-denied" — the function's stated purpose (per its own outcome enum) and its actual behavior have diverged since whichever tranche introduced this exclusion-reasons pipeline, undetected until this pass.

**Classification, per the architect's framing:** a PFA Phase-4-class defect — "a binding that type-checks and means nothing." The code compiles, the match is exhaustive, every branch is reachable in principle; but the binding between the string literals and their intended real-world referents was never actually wired to anything that produces those literals. G3/G4 (`pack_resolution.rs`/`dag_proof.rs`) do not share this specific defect — their `decide()` functions correctly produce every declared outcome from real inputs — but share the weaker, adjacent *pattern*: definitional and judgmental outcomes bundled in the same enum (a scoping/API-shape issue, lower severity, not a broken discrimination).

**Why this was never caught:** G1's `decide()` has never had a production caller that consumed anything beyond the binary `Success`/`Failure` collapse (`IntentAdmissionGate::evaluate` maps every non-`Admitted` outcome to `GateResult::Failure(format!("{other:?}"))` — the specific variant is stringified into the failure reason, but nothing downstream branches on which variant it was). The distinction this defect silently erases has never been decision-relevant to any existing consumer — exactly the shape of defect that graduating G1 to real per-reason handling would have surfaced immediately, and exactly why T11.F.2's design work surfaced it first: the floor is the first consumer that actually needs the discrimination to be correct.

**Consequence for T11.F.2:** does not block the floor — the floor's G1 check bypasses `decide()` entirely via a direct verb-registry lookup (see the T11.F.2 design doc, §2), so this defect cannot leak into the floor's correctness. **Does block G1's own future graduation** to judgmental enforcement (were `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` ever to cover G1's non-definitional outcomes) — G1 cannot honestly enforce a discrimination it cannot currently make. Recorded as a standing precondition on that graduation, not merely a nice-to-have cleanup.

**Obligations, per the architect's ruling (T11.F.2 design doc §6, full detail there):**
1. This entry (defect register, own right) — done.
2. A regression test pinning the current broken behavior as documented-known (`intent_admission_decide_currently_cannot_discriminate_exclusion_reasons_known_defect` or similar), so a future refactor that accidentally makes *some* literal matches start working produces a plausible-but-still-untested discrimination rather than silently "fixing" the conflation without anyone noticing. **Done (2026-07-12)** — `known_defect_g1_cannot_discriminate_real_prune_reasons` in `crates/ob-poc-control-plane/src/intent_admission.rs`'s test module, using the real `PruneReason::AbacDenied` Debug shape (not the bare-token case the pre-existing `pruned_verb_is_rejected_unauthorised_surface_by_default` test already covered), asserting `RejectedUnauthorisedSurface` with an explanatory panic message pointing back at this ledger entry.
3. A scoped fix ticket for `decide()` itself (structured `PruneReason` matching, not a string-literal patch — a string patch would be fragile to any future `PruneReason` Debug-format change, repeating this exact failure mode). **Filed (2026-07-12), not implemented:** `decide()` needs to accept a typed `&[PruneReason]` (or a crate-local mirror enum, to preserve `ob-poc-control-plane`'s independence from `ob-poc`'s `sem_os_context_envelope` type — B1) instead of `Vec<String>`, matching structurally rather than string-comparing; the call site (`control_plane_shadow.rs:534-539`) would pass the real `PruneReason` values through instead of pre-Debug-formatting them. Scope: `intent_admission.rs`'s `IntentAdmissionInput.exclusion_reasons` field + `decide()`'s match arms + the one call site; `IntentAdmissionDecision`'s four rejection variants stay as-is (this is a plumbing fix, not a taxonomy change). Precondition for G1's graduation to judgmental enforcement (§ above) — not scheduled against any current tranche.

**Status: PARTIALLY REMEDIATED (2026-07-12).** Obligations 1-2 done. Obligation 3 (the actual `decide()` fix) remains open by design — filed as scoped owed work, not bundled into T11.F.2's implementation (T11.F.2 slices 1-4, this session, landed the floor's G1/G3/G4 checks entirely around this defect, per §2 of the design doc — the floor's correctness does not depend on this fix landing).

## Tranche T11.1a — Boundary Map RATIFIED (2026-07-12)

Full document: `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T11.1a-BOUNDARY-MAP-001.md`. Architect ruling on the doc's three open questions (§4):

1. **Ship the 8-module high-confidence extraction slice now** (`sage/` minus the two known AB4/test-only exceptions, `journey/`, `lookup/`, `navigation/`, `plan_builder/`, `research/`, `semtaxonomy_v2/`, `acp_runtime_context.rs`). The 4 flagged mixed modules (`agent/`, `dsl_v2/`, `mcp/`, `repl/`) are **not** deferred indefinitely — their own finer-grained pass is scheduled as **Priority 1, immediately following this tranche** (the first thing T11.1's next slice does once the 8-module mechanical extraction lands).
2. **`mcp/`'s interpretation-shaped surface is T12 scope**, explicitly deferred until "the dust has settled" on T11's checkpoint-topology work — rides the full mediation cutover, not T11.
3. **`agent/orchestrator.rs` gets split, not classified as a single tier.** Ratified direction: decompose along the CP-adjacent/utterance-parsing seam the file currently conflates — the CP-adjacent half (dispatch, session-state plumbing, `compute_session_verb_surface`/`VerbSurfaceContext` calls) stays where capability access is structurally expected; the utterance-parsing/interpretation half becomes an agent-tier extraction candidate alongside `sage/`. This decomposition is now itself a design task (finding the actual seam in a 4,890-line file) — own sub-pass required before any part of `orchestrator.rs` moves, same "flagged, needs finer-grained pass" discipline the doc already applies to `dsl_v2/`/`mcp/`/`repl/`, now sharpened with a concrete split axis for this specific file.

Cross-crate items (`ob-poc-agent`'s E-5 stale "Forbidden dep: ob-poc" question, `ob-poc-web`'s startup-wiring scope) remain open, non-blocking for `ob-poc`-internal T11.1b, carried against the series completion invariant (E-T11.A).

**T11.1a status: RATIFIED, T11.1b UNBLOCKED.** Correction to this entry's earlier draft: B7 is already satisfied — v0.4.1 (ratified earlier this session) already carries the E-4/C3 constitutive clarification, and T11.F.2's code landed under that same clearance. The still-queued "step 3" batch (§7 definitional/judgmental clarification, not yet drafted; `ob-poc-agent`'s E-5 header-comment fix) is separate, smaller housekeeping — neither blocks T11.1b's mechanical extraction of the 8-module slice.

## v0.4.2 ratified — §7.1 definitional/judgmental clarification + E-5 closed (2026-07-12)

Closes the "step 3" housekeeping batch queued behind T11.F.2:

**E-5** (`ob-poc-agent`'s "Forbidden dep: ob-poc" comment): confirmed via `Cargo.toml` inspection the rule is technically satisfied (no `ob-poc` path dep) but is narrower than v0.4's L1 now requires — the crate has direct `dsl-runtime`/`sem_os_client` deps with no `ob-poc-control-plane` edge, so capability calls here are not yet CP-mediated. Fixed by inline comment clarification, not a dependency-graph change (that's T11.2/T12 scope) — a future reader can no longer mistake "no ob-poc dep" for "L1-compliant."

**§7.1** (new subsection, `EOP-VS-CONTROLPLANE-001_Control-Plane_v0.4.2.md`): drafted as a proposal (never fabricated as ratified text — this session's standing discipline), presented for review, ratified with one clarification the architect supplied directly: the 3 tollgate clauses T11.F.2's implementation actually investigated (intent recognition/G1, active pack/G3, DAG legality/G4) get the definitional/judgmental split; **the other 6 clauses are "more technical fails"** — each already fails on one uniform kind of check, not a conflation the split applies to. §7.1 states the enforcement posture explicitly: definitional cores unconditional from T11.F onward (independent of shadow-vs-enforce mode), judgmental sub-cases stay shadow-first/graduated, AB1/C3's T12 terminus unchanged. This is new model vocabulary ("definitional"/"judgmental" did not exist anywhere in v0.4.1 — T11.F's implementation plan coined it without prior model ratification) now formally backed into the model text it was implicitly relying on.

File renamed `v0.4.1` → `v0.4.2` via `git mv` (history preserved, matching this session's established rename discipline). All stale in-repo filename references updated (`check_floor_is_unconditional.sh`, T11.1a boundary map doc); historical citations in dated MCA/ledger entries left untouched (they describe what was true at time of writing, not current state).

**Step 3 housekeeping batch: CLOSED.** T11.1b may proceed.

## Tranche T11.1b — agent-tier extraction, slice 1 landed (2026-07-12)

Basis: EOP-PLAN-CONTROLPLANE-002 v0.1, Tranche T11.1, executing the T11.1a-ratified 8-module list. Full detail in the commit message (`feat(control-plane): T11.1b — agent-tier extraction slice 1`); summarised here for the ledger's own record.

**Headline finding, reshapes the T11.1a map's own confidence claim:** the design doc's directory-level "zero `sqlx::`/`dsl_runtime::`/`sem_os_postgres::` hits" census was a real signal for capability-*crate* coupling, but a materially different (and untested) question from "does this module reference other `ob-poc`-internal modules that stay behind." Per-file tracing of `use crate::X` across all 8 modules found real cross-module coupling to `mcp`, `repl`, `graph`, `runbook`, `sem_os_runtime`, `gleif` — none flagged by the original census. Landed with the user's explicit steer (align on capabilities; maintain interface/visibility discipline; no `pub` scope explosion) resolving the fork: capabilities (real I/O — `HybridVerbSearcher`, `constellation_runtime`, `GleifClient`) stay in `ob-poc` as **named T11.2 keyed-door targets**, framed per the user's own architecture point — pre-CP these were directly managed/called by agent/journey code; post-CP, the Control Plane should manage the call, with Sage/journey/repl left holding only the *result data structs*, not the capability handle. Plain data with zero capability coupling (`IntentArgValue`, `StructuredIntent`, `PackCandidate`, `assemble_dsl_string`) moved to `ob-poc-types` instead of staying duplicated or trapped behind the wrong crate boundary.

**Landed (slice 1):** `semtaxonomy_v2/` (whole), `research/` minus `sources/gleif/{loader,normalize}.rs`, `journey/{mod,pack_manager,providers,router}.rs`, `sage/{arg_assembly,deterministic,drafter,llm_sage,verb_index,verb_resolve}.rs` — merged into the existing `ob-poc-agent` crate (not a new crate — architect ruling: capability alignment, `ob-poc-agent` already has the Sage/journey/dsl-runtime dep shape, only the CP edge itself is genuinely new).

**Excluded from slice 1, each for a traced (not assumed) reason:**
- `navigation/` (whole) — `executor.rs` deeply coupled to `graph::EntityGraph` (implements `NavExecutor` for it, real mutation, not just the `ProngFilter` data type its sibling `parser.rs` needs); same parse/execute split shape T11.1a already flagged for `dsl_v2/`. Its own follow-up pass.
- `lookup/` (whole) — `service.rs` (its only real content) wraps `Arc<HybridVerbSearcher>` directly as a builder field, not just a function-call dependency.
- `plan_builder/` (whole) — `errors.rs` constructs real `OrchestratorResponse::Clarification` values (deep `runbook::response`/`runbook::types::CompiledRunbook` coupling), not a superficial doc-comment reference; its own `mod.rs` already documented `verb_classifier`/`constraint_gate` as staying in `runbook/` by original design.
- `journey/{playback,template}.rs` — `Runbook`/`SentenceGenerator` coupling, the same `repl::session_v2` inversion blocker `acp_runtime_context.rs` already self-documents (Phase 3 slice 2d.4, 2026-05-12).
- `sage/{constrained_match,valid_verb_set}.rs` — direct `HybridVerbSearcher`/`constellation_runtime` reach.
- `research/sources/gleif/{loader,normalize}.rs` — direct `GleifClient` reach; nothing else in `research/` references `GleifLoader` by name (confirmed via grep, not assumed), so the whole `gleif/` subtree stays as an orphaned-but-compiling leaf under a slim `ob-poc::research::sources` remnant.
- `acp_runtime_context.rs` (whole file) — same repl-inversion blocker as `journey::playback`/`template`, self-documented already.

**T11.2 target list (named here for the record, not yet scoped as a tranche):** `mcp::verb_search::HybridVerbSearcher`, `sem_os_runtime::constellation_runtime`, `gleif::GleifClient` — first three real capability-crate keyed-door candidates, framed per the user's CP-management principle above.

**Verified:** full workspace build clean (incl. `ob-poc-web`), `cargo clippy -p ob-poc --lib --features database -D warnings` clean, `cargo tree -p ob-poc-agent` shows no `ob-poc` edge (no cycle, L1 holds for the moved code), `dsl_v2/executor.rs` untouched (B4), 2145/0 `ob-poc` lib tests (unchanged pass count pre/post move), 3 `ob-poc-boundary` test failures and 7 `db_integration` test failures both confirmed pre-existing via `git stash` (unrelated seed/config drift — not introduced by this tranche).

**T11.1b status: SLICE 1 LANDED.** Slice 2 (the 4 MIXED modules `agent/`, `dsl_v2/`, `mcp/`, `repl/`) is the ratified Priority-1 follow-up, not yet started.

## Tranche T11.1b/slice 2 — `agent/orchestrator.rs` split, design ratified (2026-07-12)

Full text: `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T11.1b-SLICE2-ORCHESTRATOR-SPLIT-001.md`. Architect ruling on the separation law for `orchestrator.rs` (flagged MIXED in T11.1a §2, ratified answer 3): **interpretation ("what does the user mean" — linguistic, probabilistic) is agent-tier; adjudication ("is that legal for this pack/entity/state" — deterministic) is CP-tier; no code evaluates both.**

The AB5/E-3 trace showed the cost of the current entanglement: session state → `VerbSurfaceContext` → `compute_session_verb_surface` → `surface_allowed` runs legality computation inside the interpretation loop, agent-owned, no CP provenance. But the split preserves the one real reason it happened — constrained matching against the allowed surface is what stops Sage hallucinating verbs. v0.4's inverted §8 (*CP invokes Sage with a granted context*) is the resolution already in the model: legality data flows into interpretation only as a **CP-minted grant** — to Sage, an advisory, staleness-tolerant **hint** (ranks/constrains candidates, never computed by Sage); to the CP, the same data recomputed at decision time is the **verdict** (G1/G3/G4 never trust the hint). Hint drift is harmless by construction — retires the T11.F two-touchpoint drift concern.

Four rules for the file-level split: (1) utterance intake/clarification/candidate-assembly/attestation → agent tier; (2) `compute_session_verb_surface`/`surface_allowed`/every legality predicate → CP tier, full stop; (3) the surface reaches Sage only as a per-invocation CP grant, provenance-carrying, read-only, advisory, never persisted as authority (same line as AB5's `scope`/`stage_focus` CP-side field split); (4) new MCA clause — no legality predicate evaluates in agent-tier code, grants are CP-minted and advisory, provable via the L1 graph plus a verdict-type-import grep on agent crates.

**Status: design ratified, no code yet.** Next step is the file-level boundary trace over `orchestrator.rs`'s 4,890 lines plus specifying the CP-grant struct shape (new surface, not a T11.1b-slice-1-style mechanical rename) — not started, awaiting explicit "proceed."

## Tranche T11.1b/slice 2 — boundary trace complete, split re-sequenced (2026-07-12)

Full text: `docs/todo/control-plane/EOP-TRACE-CONTROLPLANE-T11.1b-SLICE2-ORCHESTRATOR-BOUNDARY-001.md`. Traced every call site of `compute_session_verb_surface`/`Phase2Service::evaluate*`/`resolve_sem_reg_verbs`/`VerbSurfaceFailPolicy` across the file's 39 top-level fns.

**Headline finding:** the legality mint is not concentrated at one or two call sites — it is **independently recomputed at 7 call sites across 5 functions**, of which **4 are independent re-mints of the SemOS envelope** (`resolve_sem_reg_verbs` at lines 385/1602/2125/2974) and **2 are independent computations of the SessionVerbSurface** (`compute_session_verb_surface` at 449/1632). A 5th independent recipe exists in the external-facing `resolve_allowed_verbs` (3400), used by callers without a full `OrchestratorContext` (e.g. MCP handlers). Same "is this legal" question, answered from scratch up to 4 times per utterance depending which internal path fires (`prepare_turn_context` vs `legacy_handle_utterance`, which itself mints twice in one function body, vs the disambiguation-menu re-entry `handle_utterance_with_forced_verb`).

**Consequence for the split plan:** a direct "these functions move, those stay" file cut is the wrong first move — it would either duplicate the mint logic across both crates or require inventing the `LegalityGrant` shape and the split simultaneously. Re-sequenced into 5 steps: (1) define a `LegalityGrant` type wrapping today's `(envelope, surface, phase2 artifacts, fail_policy)` tuple with a staleness marker; (2) collapse all 5 independent mint sites into one minting call, hoisted to the top of `handle_utterance` before intent routing; (3) thread the grant as a parameter through the consuming functions, replacing their internal recomputation with grant-field reads (pure refactor, no behavior change, existing tests cover it); (4) only then does the agent-tier/CP-tier file split become mechanical — the confirmed-clean interpretation-only functions (`route`, `run_sage_stage`, `run_coder_stage`, the `data_management_rewrite` family, `dsl_similarity`, `build_journey_pipeline_result`, etc.) move to `ob-poc-agent`, the mint call site becomes the CP-tier surface T11.2 formalizes; (5) migrate `resolve_allowed_verbs`'s external callers onto the same grant, closing the 5th mint site. Steps 1-3 satisfy design-law rule 4 mechanically (grep-provable: zero mint call sites outside the single minting function).

Also classified as CP-adjacent-not-verdict-producing (stays put either way): `emit_telemetry`, `persist_trace_scaffold`, `finalize_orchestrator_trace` — DB-backed audit/trace bookkeeping that reads already-decided outcome fields, doesn't adjudicate.

**Status: trace complete, no code moved.** Awaiting explicit "proceed" and a choice of starting step (recommended: step 1, `LegalityGrant` shape, since 2-5 all depend on it).

## Tranche T11.1b/slice 2 — steps 1-3 landed, step 5 verified clean, step 4 blocked with a named precondition (2026-07-12)

User directive mid-execution, worth recording as standing guidance: *"this split is on capability lines — so just duplicating the same function in 2 places is no good."* Confirmed the plan was already one-implementation-many-call-sites, not a copy; also surfaced that not every legality touch-point needs the same weight — user ratified keeping the TOCTOU-shaped checks on the lighter envelope-only path rather than forcing everything through the full grant ("keep the lighter envelope").

**Steps 1-3 landed** (`rust/src/agent/legality_grant.rs`, new module): `LegalityGrant` struct + `mint_legality_grant()` (the full envelope→composite_state→surface→phase2 mint — one implementation, called by `prepare_turn_context` and `legacy_handle_utterance`'s initial mint, which were previously two independent, *inconsistent* copies: `legacy_handle_utterance` never loaded `composite_state`/`entity_state`, a real drift the collapse fixes as a side effect, not just a refactor) + `verify_envelope_legality()` (a deliberately lighter envelope+phase2-only check for single-verb validation, now used by `handle_utterance_with_forced_verb`; the TOCTOU recheck stays a bare `resolve_sem_reg_verbs` call, needing even less than that). Verified: build/clippy clean, all 64 orchestrator tests pass, full lib suite 2145/0 unchanged.

**Step 5 (`resolve_allowed_verbs`) verified already correct — no code change.** Closer reading found it already shares the one real primitive (`resolve_context_internal`) with `resolve_sem_reg_verbs`; it only builds a different *request* because its callers (`sequencer.rs`, `mcp/handlers/core.rs`, `api/agent_routes.rs`) don't have a full `OrchestratorContext`. The trace doc's original "5th independent mint site" framing overstated this; corrected in the doc.

**Step 4 (move pure-interpretation functions to `ob-poc-agent`) is BLOCKED on a real, previously-undiscovered precondition, not started.** `OrchestratorContext` is itself a mixed-tier struct — capability handles (`pool`, `verb_searcher`, `policy_gate`, `sem_os_client`) and pure interpretation metadata (`session_id`, `stage_focus`, `goals`, entity signals, `sage_engine`) in one type — and every confirmed-clean interpretation function still takes `&OrchestratorContext` wholesale in its signature (verified directly against `run_sage_stage`). Moving them today would either hand agent-tier code capability handles directly (the L1 violation the whole program exists to prevent) or require inventing a second projection type splitting `OrchestratorContext` the same way `LegalityGrant` splits the legality verdict — real, unscoped design work. Per B8 (stop and flag, no convenience re-widening), this is flagged rather than improvised. **Recommend folding into T11.2** as a second named split target alongside the keyed-door capabilities.

Full detail: `docs/todo/control-plane/EOP-TRACE-CONTROLPLANE-T11.1b-SLICE2-ORCHESTRATOR-BOUNDARY-001.md` §6.

## Tranche T11.1b/slice 2 — `dsl_v2/` boundary re-check, T11.1a's split assumption corrected (2026-07-12)

`agent/orchestrator.rs` is done; `mcp/` is out of scope (T12, per T11.1a ratified answer 2); `repl/` needs T11.4's AB5 design first (not started). That leaves `dsl_v2/` as the only remaining item with no other blocker — checked it next.

T11.1a's boundary map described `dsl_v2/`'s split as "real and structurally clean": parse/validate files (`csg_linter.rs`, `semantic_validator.rs`, `applicability_rules.rs`) agent/CP-adjacent-shaped, execution engines (`executor.rs`, `graph_executor.rs`, `batch_executor.rs`, `generic_executor.rs`) unambiguously capability-tier — with a caveat that the two giant executor files (3,618/3,993 lines) "deserve a dedicated read before committing to exact file lists."

**That dedicated read now done, and the assumption doesn't hold.** A capability-import grep across all 24 files in the 22,257-line module found direct `sqlx`/`PgPool` coupling in **8 files, not 2**: alongside `executor.rs` (38 hits) and `generic_executor.rs` (42 hits), real DB coupling exists in `idempotency.rs` (9 — `sqlx::FromRow` structs, `query_as`, transaction-scoped writes to `dsl_idempotency`), `entity_deps.rs` (4 — `pub(crate) async fn init_entity_deps(pool: &PgPool)`, real load queries), `applicability_rules.rs` (4 — `sqlx::query!` against rule tables, not just types), `sheet_executor.rs` (4 — `sqlx::query!` writes plus a `Transaction` parameter), `csg_linter.rs` (2 — one is test-scaffolding, one is a real `PgPool` import), `graph_executor.rs`/`batch_executor.rs` (1 each — `PgPool` imports, need per-line confirmation before either can be called clean). Only `semantic_validator.rs`'s single hit is confirmed cosmetic (bare `use sqlx::PgPool;`, need to verify unused-vs-used, not yet checked).

**Consequence:** `dsl_v2/` is not the T11.1b-slice-1-shaped "8 zero-hit modules, ship it" case, nor is it the orchestrator.rs-shaped "one file, one clean separation law" case — it's a third shape, murkier than either: capability coupling is scattered across roughly a third of the module's files, some of it (idempotency, entity-deps) load-bearing infrastructure that other capability-tier code depends on, not a thin parse/validate layer with an occasional leak. A file-level split here needs its own dedicated per-file trace (same depth as this ledger entry, times ~8 files) before any extraction — not attempted this pass, and T11.1a's own text already called this "not urgent," which this finding reinforces rather than overturns.

**Status: correction recorded, no extraction attempted, no further T11.1b/slice-2 code target currently unblocked.** Of the 4 originally-flagged MIXED modules, `agent/` is done, `mcp/` is out of scope, `dsl_v2/` needs its own multi-file trace pass before it's actionable, `repl/` needs T11.4 first. Next un-blocked, ratifiable work is either: (a) the `dsl_v2/` per-file trace (mechanical, same shape as this entry, no new design law needed), or (b) T11.2 itself (`LegalityGrant`'s keyed-door consumers + the `OrchestratorContext` capability/metadata split flagged above) — both are legitimate next steps, neither started, no ranking between them ratified yet.

## Architect ruling — T11.2 first, `dsl_v2/` parked with conditions (2026-07-13)

**T11.2 sequenced ahead of the `dsl_v2/` trace.** Rationale, verbatim logic: "T11.2 is the next control increment... pattern-before-instances: T11.2 defines `CapabilityInvocation` and the keyed-door shape that every subsequent extraction — including whatever the `dsl_v2` trace eventually yields — must conform to. Doing the trace first would scope an extraction against a door pattern that doesn't exist yet." Within T11.2, the `OrchestratorContext` capability/metadata split goes first, since it's already flagged as blocking T11.1b/slice-2's step 4 and its shape constrains what `CapabilityInvocation` needs to carry.

**Condition 1 — the parking grep, run before parking:** `grep -rn "dsl_v2" crates/ob-poc-agent/src/` → zero hits (also checked `Cargo.toml`: `ob-poc-agent` depends on `dsl-runtime`/`dsl-core`/`dsl-analysis`, the external crates `ob-poc`'s own `dsl_v2` module partially re-exports from — not on `ob-poc`'s `dsl_v2` itself). Clean. Per the architect's own conditional: `dsl_v2/` staying in `ob-poc` root violates nothing today — it's capability-tier code resident in a capability-tier crate; its internal parse/execute split is a quality refinement, not an L1 blocker. (Had the grep found hits, those call sites would have joined T11.2's mediation targets immediately rather than waiting on the extraction — moot here.)

**Condition 2 — the trace is owed, ledgered with its trigger:** before any `dsl_v2/` extraction slice is planned, the per-file trace (same depth as the 2026-07-12 entry above, across all ~8 capability-touching files, not just the two executor files) runs first. No directory-level scoping is accepted as sufficient input to that decision, ever again, for this module specifically — see the standing rule below, which generalizes it.

**Standing planning-discipline rule, effective immediately:** T11.1a's directory-level census has now been wrong in the same direction twice — once on `agent/orchestrator.rs` (assumed a classifiable single file, was actually 7 redundant mint sites needing a new type), once on `dsl_v2/` (assumed a 2-file capability boundary, was actually 8). **Directory-level censuses (`grep -rl` per-directory, "zero hits" as a stopping condition) are retired as evidence for extraction/boundary-split scoping.** Per-file trace (every file in the candidate set individually read or grepped, not just directory-aggregated) is the only accepted input to a boundary decision going forward. This is the assumption-register discipline applied to the design docs' own claims: a design doc's classification is a claim, not a fact, until probed — "structurally clean" is exactly the kind of claim that needs a probe before anything is built on it. Applies retroactively as a caveat on any *unprobed* portion of T11.1a's map still being relied on (the CP-adjacent/capability-tier/infrastructure-neutral buckets were directory-level too — not yet re-verified per-file, treat as unconfirmed until someone actually needs to act on them).

## Tranche T11.2, Part A — `OrchestratorContext` split, design drafted (2026-07-13)

Full text: `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T11.2-CAPABILITY-INVOCATION-001.md`. Per-file field census (24 fields, `OrchestratorContext`, plus its 4 construction sites: `orchestrator.rs`'s own tests, `sequencer.rs`, `agent/harness/stub.rs`, `api/agent_service.rs`) — not a directory-level pass, per the standing rule just adopted above.

**Finding: three classes, not two.** (1) **Capability handles** (5 fields: `pool`, `verb_searcher`, `lookup_service`, `policy_gate`, `sem_os_client`) — `lookup_service` is the interesting one: despite the name, it wraps `Arc<dyn EntityLinkingService>` + `Arc<HybridVerbSearcher>` internally (verified against `src/lookup/service.rs`), so it's a capability handle, not plain data, contrary to what its field name suggests. (2) **CP-authoritative data** (4 fields: `agent_mode`, `goals`, `stage_focus`, `scope`) — plain data types, but per the legality-grant design law already ratified in T11.1b/slice 2, their *use* is CP-tier (`ScopeContext` itself is confirmed plain — `client_group_id`/`client_group_name`/`persona` — but scope's legality-determining role is CP-side). (3) **Agent-tier data** (the remaining 15 fields) — unambiguous.

**Proposed shape: a projection, not a restructure.** `OrchestratorContext` itself stays put (4 external construction sites, no behavioral reason to touch its layout — it's legitimately CP-tier-resident, constructed by code that already holds real capability handles at startup/session scope). A new `AgentTurnContext` type — `Clone`-able, built once per turn via `ctx.agent_turn_context()`, containing the 15 agent-tier fields plus the 4 CP-authoritative fields carried read-only/advisory (same posture `LegalityGrant` already established for hints) — is what the confirmed-clean interpretation functions take instead of `&OrchestratorContext`. No capability-handle field ever appears in it; that absence is the grep-provable enforcement mechanism design-law rule 4 asks for. Mirrors the `LegalityGrant` pattern exactly: one derived projection, not a source-struct rewrite.

**Deliberately deferred, not drafted this pass:** `CapabilityInvocation` proper (the *call*-request half of T11.2, as opposed to this doc's *context*-read half). Recommend `AgentTurnContext` prove itself against a real consumer first (retrofit onto the already-moved `run_sage_stage`/`run_coder_stage`) before the harder half is designed.

**Status: draft, not ratified, no code.** Awaiting review of the field census and the projection-not-restructure shape.

## Tranche T11.2, Part A — `AgentTurnContext` implemented, both recommended consumers retrofitted (2026-07-13)

Architect: "implement." `rust/src/agent/agent_turn_context.rs` (new module): `AgentTurnContext` per the design doc's §3 shape, `OrchestratorContext::agent_turn_context()` projection method. `run_sage_stage`/`run_coder_stage` — the design doc's own recommended first consumers — now take `&AgentTurnContext`; all 5 production call sites (`handle_utterance` ×3, `legacy_handle_utterance` ×2, each projecting once per turn: `let agent_turn = ctx.agent_turn_context();`) and 15 test call sites updated.

No capability handle appears in `AgentTurnContext` — and unlike the field census (a design-time claim), this is now compiler-enforced: the type literally has no `PgPool`/`Arc<HybridVerbSearcher>`/`Arc<PolicyGate>`/`Arc<dyn SemOsClient>` field, so any future accidental capability read through this projection is a compile error, not something a per-file trace has to catch later.

Verified: workspace build clean, clippy `-D warnings` clean, all 64 `agent::orchestrator::tests` pass, full lib suite 2145/0 unchanged.

`OrchestratorContext` itself untouched, as designed. Physically relocating `run_sage_stage`/`run_coder_stage` (now unblocked, signature-wise) to `ob-poc-agent` is the next mechanical step — not done this pass; this pass proves the projection type, doesn't yet move code across the crate boundary. `CapabilityInvocation` proper (Part B) remains undrafted, per the design doc's own sequencing.

Full detail: `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T11.2-CAPABILITY-INVOCATION-001.md` §5.

## Tranche T11.2, Part A — physical relocation landed, same day (2026-07-13)

"Carry on": moved `AgentTurnContext` and `run_sage_stage`/`run_coder_stage`/`SageStageOutcome`/`DraftStageOutcome`/`coder_result_from_compiler_selection`/`render_selection_dsl`/`render_dsl_string` into `ob-poc-agent` proper (`crates/ob-poc-agent/src/agent_turn_context.rs`, `crates/ob-poc-agent/src/sage/stages.rs`) — the mechanical step the same-day earlier entry flagged as not-yet-done. `ob-poc`'s `orchestrator.rs` keeps only the projection method, now building `ob_poc_agent::agent_turn_context::AgentTurnContext` and calling the relocated functions via `use ob_poc_agent::sage::stages::{run_coder_stage, run_sage_stage};`.

Two fields (`UtteranceSource`, `ScopeContext`) have no shared type across the crate boundary — both are small, behaviorless data types duplicated in `ob-poc-agent` rather than shared (no shared-crate home exists yet since `mcp/` stays in `ob-poc` per T11.1a); the projection method does the field-by-field conversion. Everything else (`ActorContext`/`sem_os_policy`, `RecentIntent`/`SageEngine`/`ob_poc_sage`, `IntentCompiler`/already-relocated `semtaxonomy_v2`, `AgentMode`/`sem_os_types`) is the identical nominal type on both sides — direct assignment, no conversion. Two new workspace deps added to `ob-poc-agent` (`sem_os_policy`, `sem_os_types`), neither with an `ob-poc` edge.

Verified: workspace build clean (incl. `ob-poc-web`), clippy `-D warnings` clean, `cargo tree -p ob-poc-agent --edges normal` shows no `ob-poc` edge (L1 holds), all 64 orchestrator tests pass, full lib suite 2145/0 unchanged.

**T11.2 Part A: COMPLETE** — design, projection type, and physical relocation all landed same day. Part B (`CapabilityInvocation` proper — the call-request half) remains the next open, undrafted decision.

## EOP-PLAN-CONTROLPLANE-GRADUATION-001 — AD-1 + AD-2 ratified (2026-07-13)

Architect ruling, same day as AD-3's resolution: **AD-1 → (a)** G10
(ExecutionEnvelope) grades envelope validity at consume time, not
prior-decision presence (b) or retirement from the input matrix (c).
Rationale of record: matches what `t4_1` already proves; the PIR's
own under-costing caveat (GRADPLAN-D-001 — the per-gate provenance
dimension G10's consume-seam samples need) is absorbed because G2
item 4 builds that dimension regardless of AD-1's outcome, so G1 item
2's consume-seam recording rides machinery already scheduled rather
than adding new scope.

**AD-2 → (b)** `EnforcedVerbs` gains a path dimension, keyed by
(verb FQN, path tag), backward-compatible (untagged = all-paths).
Rationale of record: the PIR strengthened this option independently of
the draft's own recommendation — E2's structural gate already reasons
per-path, so a path-agnostic enforcement mechanism is the one
component of the whole system that cannot express the runbook's own
A→B→C→D graduation order; that asymmetry is exactly the shape of
incident that surfaces at an operator's expense later, not at design
time. Cost: one enum tag at four ingress points.

All three architect decisions (AD-1, AD-2, AD-3) in the graduation plan
are now ratified. Plan bumped to v0.4
(`docs/todo/control-plane/EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.4.md`)
recording both resolutions in AD-1/AD-2's own sections (matching AD-3's
existing RESOLVED format) plus consequential edits to the dependency
graph, §4's completion-mapping footnote, G1/G3/G7's tranche text, and
§5's risk register (the "AD decisions pending" risk is retired,
replaced by "design docs are the critical path").

**No tranche in the plan is now blocked on an open architect decision.**
The two concrete design-doc deliverables the resolutions unblocked —
`EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001` (G1 item 1: the
seal→consume correlation-carrier design GRADPLAN-D-006 required be
split out, not inline session work) and
`EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001` (G3: the
concrete path-tag enum/keying/env-var spec AD-2(b) still needs before
G4 can start) — are in progress, not yet landed. Neither existed
before this entry.

## EOP-PLAN-CONTROLPLANE-GRADUATION-001 — G1 + G3 design docs ratified (2026-07-13)

Both design docs unblocked by AD-1/AD-2's ratification (above) landed
and were ratified by the architect the same day.

**`EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001.md` RATIFIED.** Carrier:
a new `entry_id` column on `"ob-poc".control_plane_envelopes` (not
"sequencer entry state," which the plan itself only offered as an
unverified candidate — no struct at HEAD had the right lifetime).
`HumanGate` entries (which can park indefinitely before dispatch)
re-seal at resume rather than extending or reusing a pre-park envelope.
Per-step sealing for multi-step runbooks, matching Path A/D's existing
shape. No new crate edges. **Deviation found and recorded, not
corrected here:** the plan's own citation of the T10.1
`evaluate_shadow()`/`evaluate()` MIGRATION-PENDING split as still open
is stale — that convergence was already closed 2026-07-11 (this
ledger's own "Addendum C... CLOSED" entry), two days before the plan
(v0.4) was drafted. Flagged for the plan's next correction pass.

**`EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001.md` RATIFIED.**
`ExecutionPath` enum (`RunbookSequencer`/`DslDirect`/
`WorkflowDispatched`/`BusFederated`) in `ob-poc-types`, zero new crate
edges (all four ingress crates already depend on it). `EnforcedVerbs`
reshaped to `HashMap<String, PathScope>` (`All | Only(HashSet<ExecutionPath>)`)
— deliberately NOT the plan's own `HashSet<(String, PathTag)>` framing,
which cannot express "untagged = all paths" without a non-physical
sentinel. Env-var grammar `verb[:tag(|tag)*]`, fails the WHOLE config
on any malformed entry (fail-closed, a deliberate safety call for an
admission mechanism, not fail-open or silent-partial-apply). The
Branch-3 double-admission fallthrough inside `ObPocVerbExecutor` must
carry the SAME tag as its outer admission, never a distinct
"fallthrough" tag — a distinct tag would reopen exactly the asymmetry
AD-2(b) exists to close. **Correction recorded, does not overturn
AD-2(b):** the ratification's own "one enum tag at four ingress points"
cost claim is a tag-COUNT, not a location-count — Path C is one
`RealDslExecutor` instance tagged once at construction, but Path B is
an umbrella over several distinct callers (MCP `dsl_execute`, legacy
raw-execute route, batch/sheet executors, the no-BPMN `executor_v2`
fallback) sharing one tag since none can distinguish itself from the
others today; flagged so G4's implementer isn't surprised mid-build.
Also flagged, non-blocking: durable-verb resume via `JobWorker` has no
clean tag answer yet, depending on T9.2's still-open OQ4 (park/resume
re-entry trace).

**Runbook amendment applied as part of ratification** (not deferred):
`EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` bumped v0.3 → v0.4. §5's
graduation procedure now freezes `(verb-FQN, path-tag)`, never a bare
untagged verb-FQN, as a first move (untagged entries reserved for a
verb already independently graduated on all four paths); step 3's env
var takes the `verb:path-tag` grammar; step 5's ledger record names
the path tag(s) graduated.

Plan bumped to v0.5
(`docs/todo/control-plane/EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.5.md`)
recording both ratifications; G1's dependency-graph bracket and G3's
tranche header updated from "design doc not yet written" to ratified.

**Consequence: G1 items 2-4 and G4's first line of code are now
unblocked** (G1 item 2 additionally still names G2 item 4's provenance
dimension as a named dependency — not yet landed, tracked separately).
G2b's audit-stream doc and G2 item 4 itself remain the plan's genuine
critical path.

## G2 items 3+4 implemented; E3 ruling on sample sourcing (2026-07-13)

`EOP-DESIGN-CONTROLPLANE-G2-AUDIT-PROVENANCE-001` v0.2 (RATIFIED)
implemented: `control_plane_audit` migration + `AuditEvent` enum,
`GateOutcomeProvenance` + rebuilt `gate_outcome_counts` (folding in the
G2 item-1 sentinel fix in the same rewrite), G11 completeness +
outcome re-derivation, W1-W4 window-discipline tests all green. V3/V4
findings (expected): G2 item 2's `commit_attested` and G1 items 2-4's
seal→consume wiring haven't landed, so `DispatchCommitted`/
`EnvelopeConsumed` emit at the real-but-partial call sites available
today, each degrading honestly (`attested: false`; `decision_id ==
envelope_id` correlation, documented) rather than faking completeness.
Full detail: `docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-G2-AUDIT-PROVENANCE-IMPL-001.md`.
Two hygiene items the implementing session missed (dead-code on the
G11 evaluation primitives — proven by tests, no live caller yet by
design, since wiring one is a real decision this doc doesn't make;
one clippy `collapsible_match`) fixed in the same pass, `-D warnings`
clean, full `ob-poc` suite (2160) + `ob-poc-control-plane` (115) green.

**E3 ruling:** the session flagged that its live-DB run moved E3's
gate count 10/14 → 11/14 (G10/ExecutionEnvelope now has 7 substantive
samples, correct `consume_seam` provenance) but every sample is
test-sourced, not production traffic — and asked whether E3's bar is
"substantive samples, however sourced" or specifically
production-sourced evidence. **Architect ruling: "we have no real
samples — so however sourced will have to do."** Applies the same
single-operator-deployment posture already established for AD-3(a)
(the operator IS the traffic; nothing production-sourced exists until
GM's merge+deploy opens the window) consistently to E3 — a gate
counts as evaluated once it has substantive samples at its correct
provenance, regardless of whether the run was a live-DB test or live
traffic. `invariants-expected.toml`'s `[e3]` detail updated to
11/14, ruling recorded inline; status stays `fail` (G11/G12/G14
remain genuinely zero under the same bar — this is a clarification of
what the bar was measuring, not a relaxation of it).

## G1 items 2-4 + G2 item 2 landed (2026-07-13)

**Correction (2026-07-13, later same day):** this section originally
read "G1 item 2 + G2 item 2" — an undercount. `01539938` implemented
G1 items **2, 3, and 4** in one commit, not item 2 alone; corrected
below after a follow-up verification session (see the "G1 items 3-4
verification" entry further down) independently confirmed all three
items' properties hold against current HEAD before catching the
mislabel.

G1 item 2 (seal→consume carrier): `control_plane_envelopes` gains an
`entry_id` column (migration `20260713_control_plane_envelopes_entry_id.sql`).
`step_executor_bridge.rs`'s hardcoded `envelope_id: None` replaced
with a real lookup via `lookup_sealed_handle`. Sealing changed from
fire-and-forget `tokio::spawn` to a synchronous `await` in
`sequencer.rs`, plus a new `reseal_for_human_gate_resume` method for
G1's HumanGate re-seal requirement (design doc's own bar — an
end-to-end test for this path is still owed, not written this
session).

G1 item 3 (live-DB proof from the real Path A call site): two new
tests in `step_executor_bridge.rs`'s `g1_item2_path_a_tests` module —
an enforced verb with a real sealed envelope admits, consumes, and
completes dispatch, then a same-step retry with no re-seal finds
nothing sealed and is rejected; the same enforced verb with nothing
sealed for a fresh `entry_id` is rejected with the classified
`RejectedNoEnvelope` message. Both drive `execute_step` directly, not
the adapter's own isolated `t4_1` tests.

G1 item 4 (non-eligible decisions reject with triage classification):
confirmed by construction, not a separate test — `persist_sealed`
only fires inside `phase5_runtime_recheck`'s `ApprovedStp` arm, so
`RequiresHumanGate`/`Rejected` decisions never seal anything, and
`execute_step` hits the identical no-envelope-found path item 3
already proves rejects with `RejectedNoEnvelope`, not a silent
fallthrough to allow.

G2 item 2 (write-set attestation transport): investigated per its
design doc's suggested wiring (`build_write_set_input` as the
`set_expected_write_set` source) and correctly hit a STOP-condition —
that source always produces empty `allowed_columns`, which would
misclassify every legitimate write as a breach (proven with a new
unit test). Per instruction, only the safe transport landed:
`execute_verb_admitting_envelope`'s commit call site now calls
`commit_attested(None, Some(verb_fqn))` instead of plain `commit()`
— no comparison armed, STOP-condition correctly not silently
resolved.

Both verified independently (build/test/clippy) and committed
together as `01539938`.

## G4 landed — Path B/C per-step admission, E2 structural complete (2026-07-13)

`EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001`'s ratified
design implemented in full: `ExecutionPath` enum (`ob-poc-types`,
`RunbookSequencer`/`DslDirect`/`WorkflowDispatched`/`BusFederated`);
`EnforcedVerbs` reshaped `HashSet<String>` → `HashMap<String,
PathScope>` (`PathScope::All | Only(HashSet<ExecutionPath>)`) with a
new env-var grammar (`verb:path-tag`) per the ratified runbook v0.4;
`check_admission`/`check_admission_in_scope`/`admit_plan`/
`admit_plan_checked` all gained a `path` parameter. Path B's
admission call wired into `dsl_v2::executor::execute_verb_in_scope`
(new `g4_seam_admission_tests`, 4/4 passing live) via an additive,
default-`None` `ExecutionContext.envelope_handle` field — a disclosed
deviation from G3's ratified "Paths B/C always `None`" design,
needed because G4's own item 4 requires atomicity/rollback tests
against a real consumable envelope; zero production callers set it,
so production behavior is unchanged. Path D tagged
`ExecutionPath::BusFederated` at all 4 `RealDslExecutor` construction
sites in `ob-poc-web`. `check-invariants.sh`'s `gate_e2` rewritten to
check the real seam location instead of the stale claim it used to
check. Mid-flight UUID-versioning question resolved (repo-wide count:
1443 `Uuid::new_v4()` vs 18 `Uuid::now_v7()` in Rust, 242 vs 68 in the
Postgres schema defaults — house convention leans v7 at the schema
layer, unfollowed at the app layer) and applied narrowly: new
control-plane IDs use `Uuid::now_v7()`, pre-existing call sites left
untouched. Committed as `02816414`. E2 structural leg (2/4 → 4/4 RR-2
paths reaching an admitting entry point) now complete; E2's dynamic
leg (production enforcement still defaults to `NotEnforced`) is
unchanged by design — enforcement is a deploy-time decision, not a
code-completeness one.

## G5 landed — gate applicability matrix + shadow-eval on Paths B/C/D (2026-07-13)

`EOP-DESIGN-CONTROLPLANE-G5-GATE-APPLICABILITY-MATRIX-001.md`
(new, left DRAFT pending architect ratification — status discipline
honoured, not silently absorbed) specifies which of G1-G14 apply to
which of Paths A/B/C/D. Implemented against the ratified plan v0.5's
G5 tranche, 5 items: (1) `GateResult::NotApplicable(reason)` added to
the crate's most-depended-on enum, full compiler-driven match sweep,
one genuinely ambiguous site (`decision.rs::rejection_from_report`,
a Path-A-only caller hitting a path-conditional variant it structurally
can't apply) resolved fail-closed with a disclosed rationale, not
silently guessed; (2) new `applicability.rs` module implementing the
14×4 matrix, resolving the design doc's 3 UNKNOWNs by reading the
actual gate implementations; (3-4) shadow-gate evaluation wired at
Paths B/C/D call sites using the matrix to skip inapplicable gates;
`ShadowDecisionRow` and friends widened `pub(crate)` → `pub`
(disclosed E5 surface change) plus a new direct `ob-poc-web` →
`ob-poc-control-plane` Cargo dependency, so Path D's adapter persists
rows in the same shape Path A already does rather than duplicating
the INSERT — reviewed and accepted, same "values + evaluator" edge
already used elsewhere, no cycle; (5) `control_plane_shadow_decisions`
gains an `execution_path` column (orthogonal to G2's
`GateOutcomeProvenance` — source of the outcome vs which RR-2 path
produced it), plus two new `check-invariants.sh` e3 probes
(`g5_path_a_never_produces_not_applicable` — window-discipline proof
that Path A's shadow eval never emits the new variant —  and
`e3_matrix_invariant_probe`, 2/2 passing with real per-path
substantive-sample evidence for B/C/D).

Independently re-verified end to end (not taken on the agent's
claim): forced rebuild clean after stale IDE diagnostics (4th
occurrence this session of the same stale-diagnostics pattern);
clippy clean (`-D warnings`) on the touched crates, one pre-existing
`items_after_test_module` finding in `ob-poc-web/src/main.rs`
confirmed via `git stash` to predate G5; `ob-poc` lib suite unchanged
at 2169/0 (G5's new tests are `#[ignore]`-gated live-DB); `ob-poc-
control-plane` lib suite 120/0 (+4 from G4); the literal `e3`
check-invariants run reproduced exactly matching the session doc's
claims — overall E3 verdict unchanged (`DOES NOT HOLD`), driven
entirely by the pre-existing G11/G14 gaps this tranche wasn't scoped
to close. A live-DB regression from the new `NotApplicable` sentinel
falling into a legacy query's `Unrecognised` bucket was found and
fixed within the same diff, not left latent. Committed as `79f2d27f`.
`invariants-expected.toml` left untouched (recommend-only) — E3's
underlying gaps are unchanged by this tranche.

## G1 items 3-4 verification — already landed, no new code (2026-07-13)

A session tasked with implementing G1 items 3-4 found, before writing
any code, that the branch was already 82+ commits past `01539938` —
the commit that had actually implemented items 2, 3, *and* 4 together
(this ledger's "G1 item 2 + G2 item 2 landed" entry above undercounted
it as item 2 alone; corrected in place, not left standing). Rather
than re-implementing already-landed work, the session independently
re-verified both named properties against *current* HEAD — after two
further tranches (G4, G5) had changed adjacent signatures
(`ExecutionPath`/`PathScope`, `check_admission*`'s new `path`
parameter) — rather than trusting the prior session's now-stale
citations. Both `g1_item2_path_a_tests` (item 3) and the
by-construction non-eligible-decision argument (item 4) hold
unchanged: `ob-poc` lib 2169/0 (+9 from G4/G5), `ob-poc-control-plane`
120/0 (+4), control-plane live-DB sweep 33/1 (the 1 expected
`e3_invariant_probe` failure), and the literal `check-invariants.sh
ratchet` reproduced twice (once by the session, once independently by
the reviewer after) both showing 0/5 divergence. Full detail:
`docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-G1-ITEMS-3-4-IMPL-001.md`,
committed as `202046ed`.

Repeats two still-open recommendations rather than letting them go
stale a second time: a dedicated HumanGate re-seal-at-resume live-DB
test (design doc §10 assertion 4 — the code path
(`reseal_for_human_gate_resume`) exists, the test doesn't), and an
updated `[e2]` `invariants-expected.toml` wording (still not
applied) reflecting that E2's structural leg is now 4/4 RR-2 paths
(G4 landed B/C), not the 2/4 the current comment still says — status
stays `fail` either way, since no verb is enforced by production
default on any path.

## G6b landed (RR-5 rows 2+5); G6c investigated, handed to architect (2026-07-14)

**G6b, row 2 (`toctou_entity_tables`):** the real per-entity
`row_version` populator (`build_decision_snapshot_input` → real
`SnapshotPins` → `persist_sealed`) already existed but was dead code
— `build_stp_classifier_input`'s `has_unpinned_entities` flag was
hardcoded `!entity_requests.is_empty()` at both real call sites in
`sequencer.rs`, capping every entity-bound verb at `HumanGated`
regardless of whether a real pin existed. New `has_unpinned_entities()`
(`control_plane_shadow.rs`) derives the fact honestly from the same
`entity_facts_map` G2/G13 already fetch — pinned iff
`PgEntityFactsSource` actually captured a `row_version`; a failed
batched fetch is fail-closed unpinned-for-everything. New live-DB test
proves a genuinely not-found entity still caps STP eligibility at
`HumanGated` end to end.

**G6b, row 5 (`raw_dsl_best_effort`):** RR-5's original
characterization (raw-DSL-in-request-body bypass) is stale — Slice
3.1 (2026-04-22) already closed that. What remains on Path C
(`DslDirect`) has no envelope-minting infrastructure at all — building
one is new-seam design work comparable in weight to G1's own
seal-consume doc; correctly treated as a STOP-condition and not built.
Instead, a new named test proves the honest current posture: an
enforced verb on Path C with nothing sealed is rejected
(`RejectedNoEnvelope`), never silently dispatched "best-effort" —
satisfying E4's test-existence bar without fabricating a populator
that doesn't exist.

Net: `check-invariants.sh e4` moves 1/5 → 2/5 satisfied rows (exactly
`toctou_entity_tables` and `raw_dsl_best_effort`, independently
reproduced). `[e4]` status stays `fail` (rows 1/3/4 remain open).
Full verification (build/clippy/2174-test lib suite/e4 probe/ratchet)
independently reproduced by the reviewer, matching the session's own
claims exactly; one cosmetic mislabel (a test assert message said
"Path C" while testing `ExecutionPath::DslDirect`, which is Path B)
caught and fixed during review. Committed as `9614be04`. Full detail:
`docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-G6B-G6C-IMPL-001.md`.

**G6c (RR-5 row 4, BPMN `process_instances`):** investigated,
confirmed not closeable from `ob-poc`. Live `psql` inspection
confirms no `row_version`/CAS on `process_instances`' mutable-state
columns; the actual concurrency primitive appears to be a
`lease_owner`/`lease_until` claim pattern whose write-side discipline
lives entirely inside the external `bpmn-lite-engine`/
`bpmn-lite-store` git dependencies (source not present in this
checkout, correctly not touched — bpmn-lite changes ride that repo's
own flow per the plan's standing rule 5). **Recommended architect
classification: Row 4 = Mode-1, confirmed, distinct mechanism,
unverifiable from `ob-poc`** — no fabricated symbol/test written to
force a grep-satisfying closure. A secondary, narrower finding
(`start_instance`'s idempotency check uses a non-unique index, a real
create-path TOCTOU race distinct from row 4's question) flagged, not
fixed, as out of G6c's small/standalone charter.

**G6a remains parked**, unstarted this session by design — it's
cross-repo (bpmn-lite) architect-involved work per the plan's own
tiering, not grind-suitable; needs an architect option choice (R:§C2's
carrier (a) vs (b)) before any implementation begins.

## Architect ruling — G6c row 4 is a bpmn-lite-side concern, not an ob-poc gap (2026-07-14)

Operator clarification on G6c's recommendation above: bpmn-lite is not
"a repo this session happened to lack source for" — it is a genuinely
**separate execution runtime**, spun up as a byproduct of the DSL
runtime dispatching a durable/orchestrated verb, running its own
process lifecycle outside `ob-poc`'s own admission surface. **Ruling:
RR-5 row 4's remediation path is entirely bpmn-lite's own concern,
ridden on that repo's own governance/tag-bump flow (standing rule 5),
not a lingering `ob-poc` gap to keep re-investigating from this side.**
This confirms (does not change) G6c's own recommended classification —
Mode-1, confirmed, distinct mechanism (lease_owner/lease_until, not
row-version/CAS), unverifiable from `ob-poc` — as ratified, not merely
proposed.

Applied: `scripts/check-invariants.sh`'s `gate_e4` row array gained an
explanatory comment above the `rows=(...)` block plus an inline
"SEPARATE RUNTIME, remediation is bpmn-lite-side, not ob-poc's" marker
on the `bpmn_process_instances` row's display text — annotation only,
no logic change. The row still counts "unsatisfied" (it genuinely has
no pin today; this is not a bar relaxation) and `E4`'s numeric output
is byte-for-byte unchanged (verified: `[shadow_envelope_entities]`/
`[bus_operational_writes]`/`[bpmn_process_instances]` still the 3
unsatisfied slugs, `2/5` satisfied, `E4: DOES NOT HOLD`). The point of
the annotation is that a future session reading this gate's output
does not re-open row 4 as an `ob-poc` investigation a third time —
its owner is bpmn-lite, and that's now on the record at the point
where the gate itself is read, not only in a session doc that could
go unread.

## G1 design doc §10 assertion 4 closed — HumanGate reseal-at-resume, live-DB proven (2026-07-14)

Closes a gap three prior sessions flagged as owed without attempting
(`01539938`'s own session doc, the G1-items-3-4 verification session,
the G6b/G6c session). New
`human_gate_resume_reseals_fresh_envelope_not_the_stale_pre_park_one`
(`sequencer.rs`) drives the real `ReplOrchestratorV2::execute_runbook`
→ park → `handle_human_gate_approval` → `reseal_for_human_gate_resume`
→ dispatch path end to end (not a simulation): parks a `HumanGate`
entry (shadow-sealed `ApprovedStp` before the park branch runs, per
`phase5_runtime_recheck`'s own ordering), ages the pre-park envelope
past its 5-minute validity window while parked, approves, and asserts
the pre-park envelope stays untouched (`sealed`) while a freshly-minted
`envelope_id` is what actually transitions to `consumed`.

**Disclosed deviation, verified not assumed:** the task's literal
framing ("park an entry so it seals nothing at park time") turned out
structurally unreachable — `has_unpinned_entities` and G2's
entity-binding `NotFound` share the same underlying `facts_map`
presence check, so an entity-bound verb can never be simultaneously
G2-bound and G8-unpinned. Retargeted to the design doc's own literal
§10 item 4 scenario: shadow-seal before park, stale the pre-park
envelope, prove the resume-time reseal (not the stale one) is what
gets consumed — the actual property being asked for.

**Independently reproduced, not taken on the session's claim:** the
reviewer personally reproduced the RED→GREEN proof (temporarily gated
the reseal call behind an env var, confirmed the exact predicted
`Expired` failure, reverted, confirmed green again, confirmed zero
residual diff), plus forced rebuild, clippy `-D warnings`, full lib
suite (2174/0, +1 ignored), and `check-invariants.sh ratchet` (0/5
divergence) — all matching the session doc's own claims exactly.
Committed as `29aa8df1`. Full detail:
`docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-G1-HUMANGATE-RESEAL-TEST-001.md`.

The three prior sessions' "materially larger integration-test
build-out" concern was checked by actually attempting it: accurate in
scope, not in the implied infeasibility — every needed fixture (a
`SemOsClient` stub, an in-memory `GatePipeline`, a synthetic
`DomainMetadata`, a pre-inserted `CompiledRunbook`) had a reusable
pattern already in the tree.

## `ob-poc`'s own stale E5 baseline refreshed (2026-07-14)

Closes the STOP `invariants-expected.toml`'s `[e5]` section recorded
2026-07-13 (GRADPLAN G0 item 1): `ob-poc`'s public-API surface diff was
too large/mixed (~23k lines) to safely eyeball in that grind slice and
was deliberately left un-refreshed, unlike the other 4 crates that
session did refresh.

Decomposed the real diff (17,365 raw lines against HEAD `6c6a22eb`)
exhaustively by mechanism, no unclassified residual: 6021 lines
`tower_http::follow_redirect::policy::PolicyExt` blanket-impl fanout
(traced to T11.1b making `ob-poc-agent` a real dependency of `ob-poc`,
which activates a `reqwest` rustls-tls feature chain — reproduced
absent at the exact pre-T11.1b commit in a throwaway worktree); 88
lines the same chain one hop further (`iri_string`); 4018 lines (2009/
2009, symmetric) Send/Sync auto-trait reordering, same nightly-noise
pattern already accepted for the other 4 baselines; 7238 lines genuine
already-ratified project work (95.8% of the removed side is T11.1b's
agent-tier extraction; the added side is genuine G2/G4/G6b
control-plane symbols, each traced via `git log -S` to a commit
already in this ledger). Sum reconciles exactly to the raw total.

Independently re-verified, not taken on the review's claim: regenerated
the public-API dump from scratch and diffed it byte-for-byte against
the new committed baseline (identical); independently reproduced the
6021/88 bucket line counts against the actual diff;
`check-invariants.sh ratchet` still 0/5 divergence after the refresh
(`[e5]` status stays `fail`, unaffected — detail-comment-only change).
Committed as `2c81e021`. Proposed `[e5]` wording update left in the
session doc (`invariants-expected.toml` not edited, ratchet file, per
this program's own discipline):
`docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-OBPOC-BASELINE-REVIEW-001.md`.

## G6a landed — snapshot_pin envelope carrier end-to-end (2026-07-14)

Operator ratified option (b) of R:§C2 (populate the dormant
`snapshot_pin` proto field, not the `inputs` channel) and separately
directed the bpmn-lite-side idempotency-race fix (§3.3 of the G6b/G6c
session's finding) be fixed directly in that repo, since ob-poc's own
investigation discovered it. Both landed this session, in parallel, in
two separate repos.

**G6a design** (`docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-G6A-SNAPSHOT-PIN-CARRIER-001.md`,
DRAFT — one open question in §7 flagged for architect sign-off, not
self-authorised): the architect's carrier choice left unresolved what
a bare `Uuid` can carry, since `EnvelopeHandle` is a compound `{id,
content_hash}` value. Investigation found bpmn-lite structurally
cannot mint a real `ExecutionEnvelope` at all (none of `seal`'s 8
proof inputs exist there — no pack registry, no compiled runbook
object, no entity/authority/evidence readers). Resolution:
`snapshot_pin` carries bpmn-lite's `callout_id` as a bare correlation
id; `ob-poc` is the only party that can seal, so it mints its own
envelope at admission time via a real `evaluate()` call, using the
incoming pin only as `persist_sealed`'s `entry_id` audit column —
never as a trusted foreign identity. T8.1's content-hash check (why
`try_consume_by_id` was demoted to test-only) stays untouched, does
not reopen that gap.

**Self-corrected bug, found via live-DB testing not asserted from
prose:** the design's first draft of `mint_envelope_for_bus` minted
and consumed in one call; a live-DB test against that draft caught
that this would run the consume in a separate transaction from
dispatch, breaking T9.2's rollback-together atomicity. Corrected to
mint-and-persist-only, deferring consume to the existing
`execute_verb_admitting_envelope` call — Path D now has the identical
atomicity Path A already has, not a weaker one.

**Disclosed, not a security gap — a real functional limitation** (§6/
§7): `evaluate()`'s `PROOF_BEARING_GATES` check unconditionally
requires `GateId::PackResolution`/`GateId::RunbookProof`, both already
confirmed `NotApplicable` to bus dispatch by G5's ratified matrix.
Until `evaluate`/`evaluate_with_report` becomes path-aware (a
separate, reviewed change, recommended but not self-authorised here),
an operator who enforces a bus verb gets a permanent, honest
`RejectedNoEnvelope` — proven live-DB (one of the 4 new tests), not
asserted. Landed anyway: the wire threading and mint mechanism are
real, independently tested infrastructure, immediately usable the
moment that separate change lands.

**Landed:** `ObPocVerbExecutor::mint_envelope_for_bus` +
`bus_runtime.rs` wiring + `VerbExecutor::execute` trait widened
(5th `snapshot_pin` param, every implementer updated) — ob-poc side,
committed `f1c81e72`. bpmn-lite side (`dispatch_callout` populates
`snapshot_pin`, `InvocationContext` widened + copied from the wire) —
committed locally in `~/dev/bpmn-lite` as `2379d3f`, **not pushed, not
tagged**; `rust/Cargo.toml`'s `v0.2.0` pin left untouched (a real tag
bump is the operator's own cross-repo-coordinated release step).

**Idempotency-race fix** (separate repo, same session, parallel task):
`bpmn_spawn_idempotency.idempotency_key` was already a PRIMARY KEY,
but `spawn_process_with_idempotency` did a bare pre-check SELECT
outside any lock before calling the engine's real, side-effecting
`start_process()` — two concurrent same-key callers could both pass
the check, both spawn a duplicate instance, and the loser hit an
unhandled unique-violation error instead of a graceful replay. Fixed
with a transaction-scoped `pg_advisory_xact_lock` keyed on the
idempotency key, acquired before any side-effecting work, re-checking
under the lock. RED→GREEN proven. Committed locally in `~/dev/bpmn-lite`
as `f75fd42`, not pushed/tagged.

**Concurrent-session hazard, disclosed not hidden:** both tasks worked
in the same non-isolated `~/dev/bpmn-lite` checkout at once. The
idempotency-fix session's own working-tree changes (`lib.rs` + its new
concurrency test) were overwritten mid-session by the G6a session's
own edits to adjacent files in the same tree. Caught during this
session's independent verification pass (not by either background
task): the `lib.rs` fix was recovered byte-for-byte from a
dropped-but-still-reachable `git stash` object (found via `git fsck
--dangling`); the concurrency test had never been staged/stashed and
was unrecoverable via git, so it was manually reconstructed against
the same file's existing test patterns, independently RED→GREEN
re-verified by the reviewer, and committed in `f75fd42`. **Lesson for
future sessions: do not run two agents/tasks concurrently against the
same non-isolated working tree** (this program's usual pattern —
separate ob-poc-side work, or `isolation: 'worktree'` for genuinely
parallel same-repo work — avoids this; the bpmn-lite cross-repo case
wasn't recognised as needing the same discipline until it bit).

Full verification (independently reproduced by the reviewer in both
repos): forced rebuild clean in ob-poc; clippy `-D warnings` clean on
`ob-poc`/`ob-poc-web`/`ob-poc-bus-handler`; `ob-poc` lib suite 2174/0
(+4 ignored); all 4 new `g6a_bus_envelope_mint_tests` pass in
isolation, matching the design doc's §8 plan exactly including case
3's disclosed-limitation reproduction; `ob-poc-bus-handler` 7/7;
`check-invariants.sh ratchet` 0/5 divergence. bpmn-lite side: clean
build (one pre-existing unused-imports warning, confirmed identical at
pre-fix HEAD); both new `snapshot_pin` wire tests pass; the recovered
concurrency test RED→GREEN-reproduced independently by the reviewer;
two pre-existing, unrelated test-ordering flakes from accumulated
state in the shared `bpmn_lite_test` DB confirmed (pass in isolation).

## G2 item 3 (G11) closed; item 2 (G14) real derivation, deliberately not armed (2026-07-14)

**This is the actual gate to `GM`** — the plan's own text names G2's
exit gate as GM's precondition ("G2's completion marks the last
Path-A shadow-semantics change — GM is unblocked at its close").

**G11/AuditReplay — CLOSED.** Made the "real decision" the ratified
G2-audit-provenance design doc explicitly left open: G11 is an
on-demand replay over `control_plane_audit`, called from the single
existing `gate_outcome_counts` call site (consumed by both the E3
probe and the operator-facing metrics endpoint) rather than writing
anything new into `control_plane_shadow_decisions` or the audit
stream itself. `DecisionEvaluated` gained a backward-compatible
`entry_id` join key (`#[serde(default)]`, nil for pre-existing rows)
to close a gap the prior session's own re-derivation logic needed but
didn't have. A genuine concurrency bug (unstable `ORDER BY
decision_id LIMIT 500`) was caught and fixed during implementation.
**Live-verified, independently reproduced by the reviewer: AuditReplay
moved 0 → 57 substantive samples at its correct provenance; E3 moves
11/14 → 13/14 — `WriteSetAttestation` is now the only remaining
zero/wrong-provenance gate.**

**G14/WriteSetAttestation — PARTIAL, deliberately not armed.** Built a
real, tested `allowed_columns` derivation for Insert/Update/Upsert
verbs with an explicit `returning` (mirrored line-by-line against
`crud_executor.rs`'s real column-selection logic, proven against the
real `capability-binding.draft` verb) — Delete/Link/Unlink and
`returning`-less Insert/Upsert correctly fail closed to `None` rather
than guess. Found a **second, independent, previously-undocumented
blocker** while checking whether this newly-correct subset was safe
to arm: `domain_metadata.yaml`'s bare table names never match
`record_write`'s real `"{schema}.{table}"` capture format, so
`attest()`'s exact-string table check would misclassify every
legitimate write as a breach regardless of column correctness —
proven directly against `attest()`, not asserted. Per this program's
own framing (G14 is "the plan's ONE production-behavior change" and
deserves its own dedicated review), `set_expected_write_set` stays
unwired; fixing the table-name-format mismatch is recommended as
separate, bounded follow-up work, not bundled into this diff under
time pressure.

Independently re-verified (not taken on the agent's claim): several
IDE diagnostics showing real-looking E0063 missing-field errors across
the touched files turned out to be stale mid-edit artifacts (confirmed
clean via forced rebuild); clippy clean; full lib suite 2183/0;
live-DB control-plane sweep 39/1 (the E3 ratchet failure, now naming
only `WriteSetAttestation`, counts independently reproduced matching
the commit's claim); `check-invariants.sh ratchet` 0/5 divergence.
Committed as `3b8b12e2`. `invariants-expected.toml` left untouched
(recommend-only) — `[e3]` stays `fail`.

**G2's remaining open item, precisely scoped for a future session:**
the table-name-format normalization (`domain_metadata.yaml` schema-
qualification, or a comparison-site fix) — once landed, the
Insert/Update/Upsert-with-`returning` subset becomes arming-ready, at
which point arming `set_expected_write_set` should be its own,
separately reviewed diff, not bundled with the fix that enables it.

## G14's table-name-format mismatch fixed (2026-07-14)

Closes the second blocker G2 item 2's own session (`3b8b12e2`) found
and proved but explicitly deferred. New `qualify_footprint_table`
(`control_plane_shadow.rs`) schema-qualifies `domain_metadata.yaml`'s
bare table names at the derivation site — bare names default to
`ob-poc.` (matching `crud_executor.rs`'s own convention), already-
dotted names (the `sem_reg*` family) pass through verbatim. Rejected
qualifying the YAML source itself (5 other consumers, and a real
counter-case: `team.yaml` declares a `teams` schema for 3 verbs while
`domain_metadata.yaml` gives them bare names — a naive source-side
default would be actively wrong there, not just broader blast radius)
and rejected normalizing at `attest()`'s comparison boundary (would
weaken a security-relevant exact-match check for no added benefit).
Verified operation-agnostic — confirmed all 4 `record_write` call
sites (Insert/Update/Delete/Upsert) share the identical format, not
just the 3 the prior session's column-derivation covered.

The prior session's proof-of-breakage test is superseded by one
proving the same real scenario now attests `Bounded`, not `Breach`.
Two pre-existing, narrower `domain_metadata.yaml` data-quality bugs
(a `kyc.` prefix naming a non-existent schema; the `team.*` domain's
bare names that will now default wrongly) were found during
verification and documented with real tests, not silently absorbed or
ignored — flagged as separate, narrower follow-up work.

**Scope discipline held**: `set_expected_write_set` remains unwired,
independently re-confirmed by the reviewer via grep (its only two real
call sites are test-only fault-injection scaffolding in
`sequencer_tx.rs`'s own test module) — arming the real comparison
stays the deliberately deferred, separately-reviewable decision this
program has held it as since G2 item 2's original STOP.

Independently re-verified (not taken on the agent's claim): forced
rebuild clean, clippy clean, full lib suite 2187/0, all new tests pass
in isolation, `check-invariants.sh ratchet` 0/5 divergence. Committed
as `0c7c9441`. `invariants-expected.toml` untouched — this fix moves
nothing in `[eN]` on its own (it's upstream of the arming decision
that would).

## Adversarial review pass; G11's non-unique-entry_id join bug found and fixed (2026-07-14)

Operator called for a QA review pass before deciding whether to arm
G14. Two parallel adversarial reviews (not implementation sessions —
explicitly told to hunt for real bugs, not confirm the code looks
fine) covered the write-set attestation chain and this session's
other new logic (G6a's `mint_envelope_for_bus`, G11's audit replay).

**Write-set attestation chain — verdict: not safe to arm, and the gap
is bigger than either landing session's own STOP-conditions caught.**
Two serious findings, independently spot-verified by the reviewer
(not taken on the review's claim): (1) `record_write` is called from
only 4 of 10 `CrudOperation` execute functions
(`crud_executor.rs` — Insert/Update/Delete/Upsert); `Link`/`Unlink`/
`RoleLink`/`RoleUnlink`/`EntityCreate`/`EntityUpsert` never self-report
a write at all — `EntityCreate`/`EntityUpsert` are the core of
person/company (CBU/UBO) onboarding. If armed, `attest()` would see an
empty capture list for these and vacuously pass ANY write, legitimate
or not — a structurally different and more severe gap than the
already-disclosed "empty `allowed_columns` fails closed" class. (2)
Trigger-driven columns (`row_version`, `updated_at` on `cbus`/`deals`/
`entities`/`cases`/`client_group`) are structurally invisible to
`attest()`. (3) `qualify_footprint_table`'s own safety-justification
doc comment undercounted non-`ob-poc`-schema verb YAMLs by ~4-5x (9
more files than the 2 it named) — currently inert only because
`domain_metadata.yaml` lacks footprint entries for most of them.
**Recommendation: do not arm G14.** The write-set coverage gap
(Link/Unlink/EntityCreate/EntityUpsert self-reporting) is scoped as
its own future tranche, not attempted this session.

**G11/AuditReplay — a real correctness bug found and FIXED same
session.** `replay_grade_for_decision`'s DD-4(ii) join
(`WHERE entry_id = $1 LIMIT 1`, no tiebreaker) could non-deterministically
join a `DecisionEvaluated` audit event to a DIFFERENT retry attempt's
`gate_results` — `entry_id` is the RunbookEntry's own stable id, reused
across every retry of the same runbook step, not unique per shadow-eval
attempt. Confirmed real (not hypothetical) by the reviewer directly
against the schema and `sequencer.rs`'s call sites before any fix was
attempted. Fixed: new `decision_id UUID` column on
`control_plane_shadow_decisions` (migration
`20260714_control_plane_shadow_decisions_decision_id.sql`, nullable,
no backfill), populated with the same `decision_id` value
`control_plane_audit` already uses, join switched from `entry_id` to
`decision_id`. New regression test proves two same-`entry_id` retry
attempts with disagreeing `gate_results` now each grade against their
own row. RED→GREEN independently reproduced by the reviewer (reverted
the join, confirmed the exact predicted false-grade failure, restored,
confirmed green). Committed as `73528d5c`.

**G6a's `mint_envelope_for_bus`** — reviewed, one latent (currently
dormant) gap noted: no expiry/sweep job exists for orphaned sealed
envelope rows if mint and admit are ever separated by a crash or
future refactor; not acted on this session (harmless today since §7's
gap means nothing actually mints in production yet). Everything else
checked out solid.

## G14 write-coverage tranche: record_write wired into the 6 missing CRUD ops (2026-07-14)

Operator authorized "g14 next" — closing the write-capture half of the
gap the adversarial review found above (finding (1)). `crud_executor.rs`
now calls `record_write` from all 10 `CrudOperation` execute functions,
not just the original 4 (Insert/Update/Delete/Upsert):

- `execute_link`/`execute_unlink`: one call per Uuid-typed side
  (`from`/`to`) of the junction row, gated on `affected > 0` (an
  `ON CONFLICT DO NOTHING` re-link/already-absent-unlink is a genuine
  no-op, not a write to attest against). Unlink uses an empty column
  list — hard delete of the whole junction row, matching
  `execute_delete`'s existing precedent for the same shape.
- `execute_role_link`: recorded unconditionally on success — same
  idempotent-INSERT-with-fallback-SELECT SQL shape as `execute_insert`,
  same reasoning applies verbatim — keyed by the junction row's own
  generated PK (the one of the six ops with its own row identity, unlike
  link/unlink/role_unlink which have none).
- `execute_role_unlink`: symmetric to unlink, one call per Uuid-typed
  side among `from`/`to`/`role`.
- `execute_entity_create`/`execute_entity_upsert`: two calls each (base
  `entities` table, then the extension table), keyed by `entity_id` (the
  semantic FK a caller's bound-entity-id list would actually contain),
  not the extension table's own surrogate PK when one exists. The
  idempotent early-return branch in `entity_create` (entity already
  exists) correctly records nothing — no SQL write happens on that path.

**Found and documented, not fixed (genuinely out of scope for this
tranche):** `infer_pk_column` in `crud_executor.rs` has no case for any
real entity extension table (`entity_proper_persons`,
`entity_limited_companies`, `entity_funds`, ...) — it falls back to a
literal `"id"` column that exists on none of them, so
`execute_entity_create`'s extension-table INSERT fails for every real
entity type today, independent of this change (confirmed: identical
failure with the new `record_write` calls temporarily removed). The live
production path for `entity.create` is a different, correct
implementation (`dsl_v2::generic_executor.rs`'s own `infer_pk_column`,
which has the real per-table mapping) — this `crud_executor.rs` copy of
`execute_entity_create` is not yet wired into production for that verb.
The new test proves the base-table capture (which runs and succeeds
before the broken extension step) and documents the blocked second half
rather than papering over it with a synthetic fixture.

**Still open, deliberately not attempted this tranche:** the downstream
`allowed_columns` derivation (`control_plane_shadow.rs`'s
`derive_allowed_columns_for_operation`) still only covers
Insert/Update/Upsert-with-explicit-`returning`; extending it to the same
6 operation kinds is required before G14 could ever be armed for them,
and is its own not-yet-scoped follow-up. **G14's gate remains unarmed in
production** (`set_expected_write_set` still has no real call site) —
this tranche is pure write-capture infrastructure, zero production
behavior change.

Independently re-verified (not just the landing agent's own claim):
forced rebuild (`touch` + `cargo build -p dsl-runtime`, IDE diagnostics
were stale as usual — a `cargo build --workspace` afterward was also
clean), `cargo clippy -p dsl-runtime --lib -- -D warnings` clean, full
`dsl-runtime` lib suite (185/0) and live-DB ignored suite (7/0,
`DATABASE_URL`) green, and personally reproduced RED (reverted the
base-table `record_write` call in `execute_entity_create`, got the exact
predicted panic — `expected exactly 1 captured write ... got []`) then
GREEN, confirming `git diff --stat` returned to its pre-probe size
(263 insertions, 0 deletions) after restoring. Committed as `62f460fb`.
`invariants-expected.toml` untouched — no arming decision was made.

## G14 derivation-side coverage: allowed_columns for Link/Unlink/RoleUnlink (2026-07-14)

Operator called for "derivation side — let's close this aspect," closing
out the other half of finding (1): `derive_allowed_columns_for_operation`
(`control_plane_shadow.rs`) previously covered only Insert/Update/
Upsert-with-explicit-`returning`, returning `None` for every other
`CrudOperation` behind one blanket `_ => None`. Implemented directly
(not delegated — a contained, single-file change), scoped by first
checking what's actually reachable: a full grep of `config/verbs/*.yaml`
confirmed zero real verbs use `Link`/`Unlink`/`RoleLink`; only
`RoleUnlink` (`cbu.remove-role`) and `EntityCreate`/`EntityUpsert`
(`entity.create`/`entity.ensure`, `fund.create`/`fund.ensure`) exist.

Widened `derive_crud_allowed_columns` to also thread `crud.from_col`/
`crud.to_col` through, and gave each of the 6 write-capture tranche's
operation kinds an explicit, evidenced answer instead of a catch-all:

- **`Link`**: `Some([from_col, to_col])` — deterministic straight from
  the verb's own `crud_mapping` fields (matches `execute_link`'s own
  capture exactly, no `maps_to` involved). `None` if either field is
  undeclared.
- **`Unlink`/`RoleUnlink`**: `Some(vec![])` unconditionally — both hard
  deletes always record an empty column list in `crud_executor.rs`; this
  is the known-correct answer, not a "cannot derive" placeholder.
- **`RoleLink`**: stays `None` — its PK column has no `crud.returning`
  override path at all (unlike Insert/Upsert, it's *always*
  `infer_pk_column(junction)`); reimplementing that per-table heuristic
  here would be the same cross-crate-drift risk this function already
  declines elsewhere. Currently moot (zero real verbs).
- **`EntityCreate`/`EntityUpsert`**: stay `None` — they write to a
  second table (the extension table) whose *identity itself* is resolved
  from `entity_types.table_name` at execution time, not derivable from
  static verb YAML at all; and since `allowed_columns` is one flat list
  shared across the whole write-set (not table-scoped), even a
  base-table-only partial answer would be unsound to expose as `Some`.
  (Separately, moot in practice today: the write-coverage tranche above
  already found `infer_pk_column` broken for every real extension table,
  so this half of the write never actually succeeds in production.)

Independently re-verified: forced rebuild (`touch` + real `cargo build`,
stale-diagnostics pattern again this session — confirmed clean both at
crate and full-workspace scope), `cargo clippy -p ob-poc --lib --features
database -- -D warnings` clean, the full `control_plane_shadow` test
module (54/0 unit + 5/0 live-DB ignored, `DATABASE_URL`) green,
`check-invariants.sh ratchet` 0/5 divergence, and personally reproduced
RED (disabled the `Link` branch, got the exact predicted panic) then
GREEN, confirming `git diff --stat` returned to its pre-probe size after
restoring. Committed as `a3d2b5cb`.

**G14 status after both tranches**: write-capture and derivation now
agree on the same 6 operation kinds, each with a specific justified
answer rather than a silent gap. The gate remains **unarmed** in
production (`set_expected_write_set` still has no real call site) — both
tranches are infrastructure completeness, zero production behavior
change. `invariants-expected.toml` untouched.
