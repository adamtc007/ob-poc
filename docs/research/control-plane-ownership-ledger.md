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
| C-001 | Raw execute endpoint rejects normal session-flow requests. | `ob-poc/src/api/agent_routes.rs:L1861-L1878` | `ob-poc` | G1, G8 | RETIRE after envelope path replaces raw endpoint. | TBD (T6.2) | OPEN |
| C-002 | AgentMode blocks runbook compilation unless session can compile. | `ob-poc/src/api/repl_routes_v2.rs:L3767-L3776` | `ob-poc` | G1, G5 | MOVE into admission/authority; UI mode can remain local. | TBD (T2.1/T2.4) | OPEN |
| C-003 | Plan approval requires existing plan and Compiled/AwaitingApproval status. | `ob-poc/src/api/repl_routes_v2.rs:L3925-L3948` | `ob-poc` | G8, G9 | INVOKE from control plane. | TBD (T3.3/T3.4) | OPEN |
| C-004 | Plan execution requires Repl mode and Approved/Executing status. | `ob-poc/src/api/repl_routes_v2.rs:L3990-L4042` | `ob-poc` | G8, G9 | MOVE status admission to control plane, keep route wrapper. | TBD (T3.3/T3.4) | OPEN |
| C-005 | PolicyGate removes raw execute feature flag and centralizes strict single-pipeline / SemReg fail-closed flags. | `ob-poc-boundary/src/policy/gate.rs:L12-L23,L50-L69` | `ob-poc-boundary` | G1, G3, G5 | SPLIT: config stays boundary, gate decision moves. | TBD (T2.1/T2.3/T2.4) | OPEN |
| C-006 | ActorResolver builds actor role/clearance/jurisdiction context from env/session. | `ob-poc-boundary/src/policy/gate.rs:L79-L121` | `ob-poc-boundary` | G5 | INVOKE as identity source, not policy owner. | TBD (T2.4) | OPEN |
| C-007 | SemOS context envelope partitions allowed/pruned verbs and computes fingerprint. | `ob-poc/src/agent/sem_os_context_envelope.rs:L20-L56,L163-L247` | `ob-poc` | G1, G3, G5, G6, G12 | MOVE decision ownership; keep envelope as input/adapter. | TBD (T2.1/T2.3/T2.4/T2.5/T4.4) | OPEN |
| C-008 | SemOS TOCTOU recheck result can block if selected verb no longer allowed. | `ob-poc/src/agent/sem_os_context_envelope.rs:L138-L161,L288-L335` | `ob-poc` | G5, G12, G13 | MOVE into snapshot/policy gate. | TBD (T2.4/T3.2/T4.4) | OPEN |
| C-009 | Session verb surface applies AgentMode, scope/workflow, SemReg CCIR, fail policy, and ranking. | `ob-poc/src/agent/verb_surface.rs:L314-L324,L337-L496` | `ob-poc` | G1, G3, G5 | SPLIT discovery ranking from admission gate. | TBD (T2.1/T2.3/T2.4) | OPEN |
| C-010 | Lifecycle state is tagged for discovery but not pruned. | `ob-poc/src/agent/verb_surface.rs:L451-L462,L509-L521` | `ob-poc` | G4 | INVOKE only as hint; not gate proof. | TBD (T2.2) | OPEN |
| C-011 | Macro expansion validates required args and failure maps to clarification/error. | `ob-poc/src/runbook/compiler.rs:L167-L225` | `ob-poc` | G1, G9 | INVOKE. | TBD (T2.1/T3.4) | OPEN |
| C-012 | Macro fixpoint expansion enforces depth, step limits, and per-path cycle detection. | `ob-poc/src/dsl_v2/macros/expander.rs:L250-L346` | `ob-poc` | G1, G9 | INVOKE. | TBD (T2.1/T3.4) | OPEN |
| C-013 | Expanded macro output is revalidated and rejects unknown runtime verbs. | `ob-poc/src/runbook/compiler.rs:L603-L638` | `ob-poc` | G1, G9 | MOVE admission part; keep macro validator local. | TBD (T2.1) | OPEN |
| C-014 | Plan assembler builds dependency graph, rejects cycles/empty plans, but unresolved bindings are diagnostics. | `ob-poc/src/plan_builder/plan_assembler.rs:L80-L160,L234-L266` | `ob-poc` | G4, G9 | INVOKE. | TBD (T2.2/T3.4) | OPEN |
| C-015 | Pack constraint gate blocks verbs not permitted by active pack constraints. | `ob-poc/src/runbook/constraint_gate.rs:L1-L15,L33-L100` | `ob-poc` | G3 | MOVE gate decision, invoke pack data. | TBD (T2.3) | OPEN |
| C-016 | SemReg allowed-set unavailable is fail-closed in compiler. | `ob-poc/src/runbook/compiler.rs:L339-L362,L468-L486` | `ob-poc` | G3, G5 | MOVE. | TBD (T2.3/T2.4) | OPEN |
| C-017 | Raw endpoint performs separate SemOS allowed-verb policy checks. | `ob-poc/src/api/agent_routes.rs:L2101-L2177` | `ob-poc` | G3, G5 | RETIRE duplicate of C-016 after raw path removal. | TBD (T6.2) | OPEN |
| C-018 | Raw endpoint performs separate semantic validator checks. | `ob-poc/src/api/agent_routes.rs:L2180-L2257` | `ob-poc` | G4, G7 | RETIRE or INVOKE through same compiler. | TBD (T6.2) | OPEN |
| C-019 | Write set is heuristic by default and contract-union only behind feature flag. | `ob-poc/src/runbook/write_set.rs:L1-L17,L112-L146` | `ob-poc` | G7 | SPLIT into proof plus runtime attestation. | TBD (T2.6/T5.1) | OPEN |
| C-020 | Runbook execution rejects missing or non-executable persisted runbooks. | `ob-poc/src/runbook/executor.rs:L812-L824` | `ob-poc` | G9, G10 | MOVE admission, keep store lookup. | TBD (T3.4/T4.1) | OPEN |
| C-021 | Advisory locks are sorted, timeout-bounded, and emit events on lock paths. | `ob-poc/src/runbook/executor.rs:L682-L755,L831-L875` | `ob-poc` | G10, G13 | INVOKE. | TBD (T4.1/T3.2) | OPEN |
| C-022 | Runbook executor warns and proceeds without locks when write set exists but no pool exists. | `ob-poc/src/runbook/executor.rs:L879-L885` | `ob-poc` | G10 | RETIRE for production; keep test-only explicit path. | `ob-poc` (`UnlockedExecutionToken`) | **CLOSED** — T0.3 (`65c60006`): production path now hard-errors; test-only path requires explicit `UnlockedExecutionToken`. |
| C-023 | Step dependencies cause skipped dependent steps on prior failure. | `ob-poc/src/runbook/executor.rs:L929-L960` | `ob-poc` | G9 | INVOKE. | TBD (T3.4) | OPEN |
| C-024 | Optional pre-dispatch GatePipeline returns `Ok(())` when absent or no transition metadata. | `ob-poc/src/runbook/step_executor_bridge.rs:L202-L212` | `ob-poc` | G4 | SPLIT: missing pipeline should be admission-visible. | TBD (T2.2) | OPEN |
| C-025 | GatePipeline evaluates DAG transitions and blocks severity=`error` violations. | `ob-poc/src/runbook/step_executor_bridge.rs:L214-L293` | `ob-poc` | G4 | MOVE proof ownership; invoke checker. | TBD (T2.2) | OPEN |
| C-026 | GateChecker resolves source entity, reads source slot, and reports violations. | `dsl-runtime/src/cross_workspace/gate_checker.rs:L155-L190,L193-L265` | `dsl-runtime` | G4 | INVOKE as validator. | TBD (T2.2) | OPEN |
| C-027 | Lifecycle `requires_states` precondition is fail-open except true mismatch. | `ob-poc/src/dsl_v2/executor.rs:L2015-L2041,L2048-L2115` | `ob-poc` | G4 | SPLIT; divergent duplicate of C-025/C-026 semantics. | `ob-poc` (`LifecycleGateMode`) / TBD (T2.2 unification) | **PARTIALLY CLOSED** — T0.2 (`80ce7449`): fail-open classes now configurable + always audited (`LifecycleGateMode`, `LifecycleFailOpenClass`); semantics unification with C-025/C-026 still owed to T2.2. |
| C-028 | DslExecutor rejects durable verbs unless direct durable execution is explicitly allowed. | `ob-poc/src/dsl_v2/executor.rs:L1900-L1989` | `ob-poc` | G8, G10 | INVOKE. | TBD (T3.3) | OPEN |
| C-029 | SemOsVerbOpRegistry startup hard-fails YAML plugin declarations without registered ops. | `ob-poc-web/src/main.rs:L892-L935` | `ob-poc-web` | G1, G9 | INVOKE as deployment guard. | TBD (T2.1/T3.4) | OPEN |
| C-030 | GatePipeline startup is soft-fail and leaves runtime ungated when config/DAG load fails. | `ob-poc-web/src/main.rs:L1400-L1440` | `ob-poc-web` | G4 | MOVE to deployment/admission hard fence. | `ob-poc-web` (`decide_gate_pipeline_startup`) | **CLOSED** — T0.1 (`b73e9cee`): production-fatal on load failure unless `OB_POC_GATES_FAIL_OPEN=1` (WARN banner). |
| C-031 | ObPocVerbExecutor wraps plugin ops in transaction and rolls back on op error. | `ob-poc/src/sem_os_runtime/verb_executor_adapter.rs:L152-L247` | `ob-poc` | G10 | INVOKE. | TBD (T4.1) | OPEN |
| C-032 | CRUD executor executes metadata-driven insert/update/delete/upsert without comparing to a WriteSetProof. | `dsl-runtime/src/crud_executor.rs:L37-L70,L230-L330` | `dsl-runtime` | G7, G14 partial | SPLIT; add attestation wrapper. | TBD (T5.1) | OPEN |
| C-033 | Bus handler enforces catalogue version equality. | `ob-poc-bus-handler/src/lib.rs:L108-L145` | `ob-poc-bus-handler` | G12 | INVOKE. | TBD (T4.4) | OPEN |
| C-034 | Bus adapter calls executor with `Principal::system()` and no runbook/envelope gates. | `ob-poc-web/src/bus_runtime.rs:L122-L149` | `ob-poc-web` | G5, G10 gap | MOVE into envelope admission; bypass candidate. | TBD (T6.1) | OPEN |
| C-035 | Workflow dispatcher validates binding/process key before BPMN orchestration. | `ob-poc/src/bpmn_integration/dispatcher.rs:L246-L273` | `ob-poc` | G1, G9 | INVOKE. | TBD (T2.1/T3.4) | OPEN |
| C-036 | Workflow dispatcher persists/queues orchestration state and tolerates some persistence failures as logs. | `ob-poc/src/bpmn_integration/dispatcher.rs:L275-L360` | `ob-poc` | G9, G10, G11 | SPLIT. | TBD (T3.4/T4.1/T7.1) | OPEN |
| C-037 | BPMN controller op writes in `pre_fetch` before transaction-scoped `execute`. | `ob-poc/src/domain_ops/bpmn_controller_ops.rs:L283-L320` | `ob-poc` | G10, G14 gap | RETIRE/SPLIT; move write into scoped execute. | `ob-poc` (`BpmnControllerStartInstance::execute`) | **CLOSED** — T0.4 (`352b52ec`): write moved from `pre_fetch` into transaction-scoped `execute`. |
| C-038 | BPMN controller validates tenant/template/idempotency before inserting process instance. | `bpmn-controller/src/instance.rs:L100-L190` | `bpmn-controller` | G2, G10 | INVOKE external surface only. | TBD (T3.1/T4.1) | OPEN |
| C-039 | Entity relationship upsert requires percentage for ownership and applies expected-version CAS except carved-out edge classes. | `sem_os_postgres/src/ops/entity_relationship.rs:L18-L34,L96-L160,L187-L215` | `sem_os_postgres` | G10, G13 | INVOKE. | TBD (T4.1/T3.2) | OPEN |
| C-040 | KYC lexicon entries declare governing taxonomy, writes, authority, preconditions, emits, and content hash. | `ob-poc-kyc-substrate/src/lexicon.rs:L1-L6,L118-L137` | `ob-poc-kyc-substrate` | G1, G5, G6, G12 | INVOKE. | TBD (T2.1/T2.4/T2.5/T4.4) | OPEN |
| C-041 | KYC control preconditions enforce evidence-before-verify and reconcile/strategy before fold/freeze. | `ob-poc-kyc-substrate/src/fold/control.rs:L438-L478` | `ob-poc-kyc-substrate` | G4, G6 | INVOKE. | TBD (T2.2/T2.5) | OPEN |
| C-042 | KYC store checks preconditions under stream lock before append and sequence bump. | `ob-poc-kyc-store/src/store.rs:L118-L203` | `ob-poc-kyc-store` | G6, G10, G13 | INVOKE. | TBD (T2.5/T4.1/T3.2) | OPEN |
| C-043 | KYC manifest publish persists content-addressed manifest and stamps per-verb lexicon hashes. | `ob-poc-kyc-store/src/manifest.rs:L1-L7,L57-L95` | `ob-poc-kyc-store` | G12 | INVOKE. | TBD (T4.4) | OPEN |
| C-044 | Shadow GatedVerbEnvelope construction records placeholders and explicitly does not gate dispatch. | `ob-poc-boundary/src/envelope_builder.rs:L1-L18`, `ob-poc/src/sequencer.rs:L7721-L7763` | `ob-poc-boundary` / `ob-poc` | G10, G13 partial | MOVE to production envelope admission. | TBD (T2.7/T4.1) | OPEN |
| C-045 | TOCTOU recheck scaffold can recompute row-version hash but is not production-wired. | `ob-poc-boundary/src/toctou_recheck.rs:L1-L34,L81-L139` | `ob-poc-boundary` | G10, G13 | MOVE/finish. | TBD (T4.3) | OPEN |

## Running totals

- Opening balance (Phase 0 / RR-3): 45 rows. MOVE 11, INVOKE 19, RETIRE 5, SPLIT 10.
- Closed by T0: **3** (C-022, C-030, C-037 — full closure).
- Partially closed by T0: **1** (C-027 — fail-open behaviour fixed; semantics-unification half still owed to T2.2).
- Open: 41 (40 untouched + C-027's remaining half).
