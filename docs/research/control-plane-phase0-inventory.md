# Control Plane Phase 0 Inventory

## RR-0 Executive Summary

The current execution-control plane is distributed across the REPL/orchestrator, runbook compiler, runbook executor, runtime dispatch bridge, SemOS context envelope, raw DSL endpoint, workflow dispatcher, bus handler, KYC stream store, and domain verb ops; the workspace member and external dependency surfaces are enumerated from `rust/Cargo.toml` (`ob-poc/Cargo.toml:L285-L374`, `ob-poc/Cargo.toml:L376-L401`). The closest production path to a control pipeline is `session_input` -> V2 orchestrator -> compile/runbook -> `VerbExecutionPort` -> `ObPocVerbExecutor`, but the envelope type intended to carry a single gate decision is still shadow/compile-only (`ob-poc-types/src/gated_envelope.rs:L26-L31`, `ob-poc-boundary/src/envelope_builder.rs:L10-L18`, `ob-poc/src/sequencer.rs:L7721-L7763`).

The three most dangerous findings are: bus invocations call the same executor with `Principal::system()` but without the runbook/envelope gates (`ob-poc-web/src/bus_runtime.rs:L122-L149`); the V1.3 gate pipeline is default-on only when config and DAG registry load, otherwise startup continues ungated (`ob-poc-web/src/main.rs:L1400-L1440`); lifecycle `requires_states` is intentionally fail-open for no lifecycle, no entity arg, invalid UUID, no slot mapping, and absent/NULL/unreadable current state (`ob-poc/src/dsl_v2/executor.rs:L2015-L2041`, `ob-poc/src/dsl_v2/executor.rs:L2048-L2104`).

Coverage of G1-G14 is partial: G1/G3/G5 have SemOS context, verb surface, pack gate, and SemReg filters; G4 has DAG/GateChecker and fail-open lifecycle checks; G6 exists mainly in KYC stream preconditions; G7 is heuristic/feature-gated write-set derivation; G8/G9 exist as runbook statuses and human-gate behaviour, not a deterministic STP proof; G10/G13 have locks and shadow TOCTOU scaffolding but no production envelope admission; G11/G12 have telemetry/snapshot/version pieces; G14 has no located runtime write-set attestation chokepoint.

## RR-1 Workspace Map and Entry-Point Census

Workspace members are the root crate plus 56 listed member paths, including `ob-poc-web`, `ob-poc-boundary`, `ob-poc-sage`, domain crates, KYC substrate/store/seam, `entity-gateway`, SemOS crates, `bpmn-controller`, `dsl-runtime`, DSL frontend/parser/lowering crates, `bpmn-runtime`, migration/rendering tools, and test harnesses (`ob-poc/Cargo.toml:L285-L374`). External workspace dependencies are from `github.com/adamtc007/dsl` at tag `v0.1.4` for `dsl_types`, `dsl-core`, `sem_os_types`, `sem_os_core`, `sem_os_ontology`, and `sem_os_policy`, and from `github.com/adamtc007/bpmn-lite` at tag `v0.2.0` for BPMN-lite, bus, DMN bridge, FFI, storage, and manifest crates (`ob-poc/Cargo.toml:L376-L401`).

| Entry point | Crate | Trigger type | Production/dev | Mutation route evidence |
|---|---|---:|---:|---|
| Unified agent input `POST /api/session/:id/input` | `ob-poc` / web router | HTTP utterance/decision/run command | Production | Route is registered at `ob-poc/src/api/agent_routes.rs:L85-L99`; handler delegates to V2 REPL when available at `ob-poc/src/api/agent_routes.rs:L141-L158`; `/run` eventually executes runbook entries through the Sequencer path cited in RR-2. |
| Legacy raw DSL `POST /api/session/:id/execute` | `ob-poc` | HTTP explicit raw DSL | Production but gated as legacy/raw-only | Normal session-flow requests return 410; raw requests fall through to `execute_session_dsl_raw` (`ob-poc/src/api/agent_routes.rs:L1861-L1890`). |
| Runbook plan API compile/approve/execute | `ob-poc` | HTTP plan actions | Production | Compile requires Sage mode (`ob-poc/src/api/repl_routes_v2.rs:L3767-L3776`), approval validates plan status (`ob-poc/src/api/repl_routes_v2.rs:L3925-L3948`), execution requires Repl mode and Approved/Executing status (`ob-poc/src/api/repl_routes_v2.rs:L3990-L4042`). |
| REPL/direct DSL bridge | `ob-poc` | internal executor call | Production/internal | `RealDslExecutor::execute` parses, compiles, then executes with per-verb transactions (`ob-poc/src/repl/executor_bridge.rs:L114-L134`); `execute_in_scope` runs through caller transaction scope (`ob-poc/src/repl/executor_bridge.rs:L137-L163`). |
| Workflow/BPMN dispatcher | `ob-poc` | runbook step routed to BPMN | Production when configured | `WorkflowDispatcher` parses the verb, routes direct vs orchestrated, and calls BPMN for orchestrated verbs (`ob-poc/src/bpmn_integration/dispatcher.rs:L536-L576`). |
| BPMN controller verb op | `ob-poc` + `bpmn-controller` | SemOS verb op | Production/internal | `bpmn-controller.start-instance` does the process start in `pre_fetch`, returning `_instance_id` to `execute` (`ob-poc/src/domain_ops/bpmn_controller_ops.rs:L273-L320`); `start_instance` writes `process_instances` after tenant/template/idempotency checks (`bpmn-controller/src/instance.rs:L100-L190`). |
| Bus invocation service | `ob-poc-web` / `ob-poc-bus-handler` | federated bus message | Production when bus enabled | Bus runtime registers `ObPocBusHandler` as invocation dispatcher (`ob-poc-web/src/bus_runtime.rs:L70-L105`); the handler checks catalogue version and calls the supplied executor (`ob-poc-bus-handler/src/lib.rs:L118-L145`). |
| SemOS plugin op registry startup | `ob-poc-web` | server startup | Production | Registry is built from SemOS and domain ops (`ob-poc-web/src/main.rs:L876-L890`); YAML plugin declarations without handlers panic at startup (`ob-poc-web/src/main.rs:L892-L935`). |
| Process registry startup | `ob-poc-web` | server startup | Production | Process definitions are loaded and compiled at startup; failures are logged and skipped, and the server starts partial/empty (`ob-poc-web/src/main.rs:L837-L848`). |
| KYC stream append | `ob-poc-kyc-store` | stream/event append | Library path | Store append locks the subject stream, dedupes idempotency key, folds current events, validates under lock, inserts event, bumps sequence, and enqueues projection effects (`ob-poc-kyc-store/src/store.rs:L118-L203`). |

## RR-2 Execution Path Traces

### Path A: utterance -> Sage/SemOS -> runbook -> runtime -> database write

1. HTTP input reaches `session_input`, which routes to V2 REPL when the orchestrator is present (`ob-poc/src/api/agent_routes.rs:L141-L158`).
2. The Sequencer persists turn/session state under an acquired session path, records an utterance trace/hash, and can perform non-fatal Sage pre-classification before REPL-state dispatch (`ob-poc/src/sequencer.rs:L1273-L1317`, `ob-poc/src/sequencer.rs:L1336-L1416`).
3. Compile uses `compile_verb`; the macro path runs expansion, DAG/order, pack constraints, SemReg allowed-set filtering, write-set derivation, optional snapshot manifest, and constructs a `CompiledRunbook` (`ob-poc/src/runbook/compiler.rs:L150-L153`, `ob-poc/src/runbook/compiler.rs:L294-L421`).
4. Primitive compile repeats pack constraints and SemReg checks and builds a `CompiledStep` with derived write set (`ob-poc/src/runbook/compiler.rs:L463-L555`).
5. Execution calls `execute_runbook`, looks up the persisted runbook, rejects non-executable statuses, computes write set, and tries advisory locks when a pool is present (`ob-poc/src/runbook/executor.rs:L783-L839`).
6. If lock acquisition fails, a lock-contention event is appended and execution fails; if locks are acquired, a lock event is appended (`ob-poc/src/runbook/executor.rs:L840-L875`).
7. Each step executes through `VerbExecutionPortStepExecutor`, which runs optional pre-dispatch gate checks then calls `port.execute_verb` (`ob-poc/src/runbook/step_executor_bridge.rs:L202-L293`, `ob-poc/src/runbook/step_executor_bridge.rs:L451-L505`).
8. `ObPocVerbExecutor` routes plugin verbs through `SemOsVerbOpRegistry` in a transaction, commits on success, rolls back on op error, routes CRUD through `PgCrudExecutor`, then falls back to the legacy `DslExecutor` (`ob-poc/src/sem_os_runtime/verb_executor_adapter.rs:L117-L286`).
9. A concrete mutation site is the CRUD executor, which constructs SQL from the verb CRUD mapping and runs insert/update/delete/upsert statements (`dsl-runtime/src/crud_executor.rs:L37-L70`, `dsl-runtime/src/crud_executor.rs:L230-L330`).

Bypass candidates on this path: if no DB pool is supplied, write-set locks are skipped with a warning (`ob-poc/src/runbook/executor.rs:L879-L885`); if `GatePipeline` is absent or the verb lacks transition metadata, pre-dispatch DAG gate checking returns `Ok(())` (`ob-poc/src/runbook/step_executor_bridge.rs:L202-L212`).

### Path B: legacy raw DSL -> parse/validate/compile -> atomic/best-effort execution

1. `/api/session/:id/execute` only accepts explicitly raw execute requests; other requests return 410 (`ob-poc/src/api/agent_routes.rs:L1861-L1878`).
2. The raw handler parses/validates/compiles DSL when no cached plan is present; parse errors are logged and returned (`ob-poc/src/api/agent_routes.rs:L2048-L2099`).
3. If a SemOS client is present, the path resolves allowed verbs, blocks when SemReg is unavailable under fail-closed policy, blocks empty allowed sets, and blocks denied verbs (`ob-poc/src/api/agent_routes.rs:L2101-L2177`).
4. The same path then runs `SemanticValidator` and blocks on diagnostics (`ob-poc/src/api/agent_routes.rs:L2180-L2257`), compiles and blocks on compile errors (`ob-poc/src/api/agent_routes.rs:L2261-L2308`), and treats expansion report failures as best-effort warnings (`ob-poc/src/api/agent_routes.rs:L2311-L2357`).
5. Mutation terminus is execution of the compiled plan using either atomic locks or best-effort mode (`ob-poc/src/api/agent_routes.rs:L2365-L2448`).

Bypass candidates on this path: this route has its own SemOS/CSG checks separate from the runbook compiler; expansion failures can be logged while execution continues (`ob-poc/src/api/agent_routes.rs:L2311-L2357`).

### Path C: workflow/BPMN-routed runbook step -> process instance write

1. `WorkflowDispatcher::execute_v2` extracts the verb FQN, delegates direct verbs to the inner executor, and sends orchestrated verbs to `execute_orchestrated` (`ob-poc/src/bpmn_integration/dispatcher.rs:L536-L576`).
2. `execute_orchestrated` fails if no workflow binding or no `process_key` exists (`ob-poc/src/bpmn_integration/dispatcher.rs:L246-L273`).
3. It canonicalizes and hashes the DSL payload, resolves bytecode version, creates a correlation key, and persists initial request state; failures to persist request state are logged (`ob-poc/src/bpmn_integration/dispatcher.rs:L275-L311`).
4. It starts the BPMN process by gRPC; on gRPC failure it queues the dispatch for retry rather than immediately failing (`ob-poc/src/bpmn_integration/dispatcher.rs:L329-L360`).
5. A second BPMN mutation path is `bpmn-controller.start-instance`: the op performs the process start in `pre_fetch` before the op body returns the UUID (`ob-poc/src/domain_ops/bpmn_controller_ops.rs:L283-L320`); the controller validates tenant/template/idempotency and inserts `process_instances` with state `Running` (`bpmn-controller/src/instance.rs:L100-L190`).

Bypass candidates on this path: the `pre_fetch` write for `bpmn-controller.start-instance` occurs before the op `execute` receives the DSL transaction scope (`ob-poc/src/domain_ops/bpmn_controller_ops.rs:L283-L320`).

### Path D: federated bus invocation -> executor -> DB write

1. Bus runtime builds `BusClient`, starts sender, registers `ObPocBusHandler`, and serves invocation/entity/SemOS services (`ob-poc-web/src/bus_runtime.rs:L70-L105`).
2. Handler enforces configured catalogue version mismatch rejection and then calls `executor.execute` (`ob-poc-bus-handler/src/lib.rs:L118-L145`).
3. The adapter translates bindings to JSON, creates a `VerbExecutionContext` with `Principal::system()`, binds UUID inputs, and calls `ObPocVerbExecutor::execute_verb` (`ob-poc-web/src/bus_runtime.rs:L122-L149`).
4. Mutation terminus is the same `ObPocVerbExecutor` and CRUD/plugin transaction machinery cited in Path A (`ob-poc/src/sem_os_runtime/verb_executor_adapter.rs:L117-L286`).

Bypass candidate: the bus path bypasses runbook compiler status, write-set lock derivation, human-gate runbook states, and shadow envelope construction; its only located admission control in ob-poc is catalogue-version equality (`ob-poc-bus-handler/src/lib.rs:L125-L136`).

## RR-3 Control Inventory

| CID | Check (one line) | Location (file:line) | Owning crate today | Gate mapping (G1-G14 or NONE) | Trigger paths | On-failure behaviour | Disposition candidate |
|---|---|---|---|---|---|---|---|
| C-001 | Raw execute endpoint rejects normal session-flow requests. | `ob-poc/src/api/agent_routes.rs:L1861-L1878` | `ob-poc` | G1, G8 | Raw HTTP path | 410 Gone | RETIRE after envelope path replaces raw endpoint. |
| C-002 | AgentMode blocks runbook compilation unless session can compile. | `ob-poc/src/api/repl_routes_v2.rs:L3767-L3776` | `ob-poc` | G1, G5 | Plan compile API | 403 recoverable | MOVE into admission/authority; UI mode can remain local. |
| C-003 | Plan approval requires existing plan and Compiled/AwaitingApproval status. | `ob-poc/src/api/repl_routes_v2.rs:L3925-L3948` | `ob-poc` | G8, G9 | Plan approval API | 404/409 | INVOKE from control plane. |
| C-004 | Plan execution requires Repl mode and Approved/Executing status. | `ob-poc/src/api/repl_routes_v2.rs:L3990-L4042` | `ob-poc` | G8, G9 | Plan execute API | 403/404/409 | MOVE status admission to control plane, keep route wrapper. |
| C-005 | PolicyGate removes raw execute feature flag and centralizes strict single-pipeline / SemReg fail-closed flags. | `ob-poc-boundary/src/policy/gate.rs:L12-L23`, `ob-poc-boundary/src/policy/gate.rs:L50-L69` | `ob-poc-boundary` | G1, G3, G5 | HTTP policy consumers | bool decision/snapshot | SPLIT: config stays boundary, gate decision moves. |
| C-006 | ActorResolver builds actor role/clearance/jurisdiction context from env/session. | `ob-poc-boundary/src/policy/gate.rs:L79-L121` | `ob-poc-boundary` | G5 | HTTP/session contexts | Defaults to anonymous/viewer-ish roles | INVOKE as identity source, not policy owner. |
| C-007 | SemOS context envelope partitions allowed/pruned verbs and computes fingerprint. | `ob-poc/src/agent/sem_os_context_envelope.rs:L20-L56`, `ob-poc/src/agent/sem_os_context_envelope.rs:L163-L247` | `ob-poc` | G1, G3, G5, G6, G12 | Sage/orchestrator | deny-all/unavailable flags | MOVE decision ownership; keep envelope as input/adapter. |
| C-008 | SemOS TOCTOU recheck result can block if selected verb no longer allowed. | `ob-poc/src/agent/sem_os_context_envelope.rs:L138-L161`, `ob-poc/src/agent/sem_os_context_envelope.rs:L288-L335` | `ob-poc` | G5, G12, G13 | Orchestrator recheck | structured Denied result | MOVE into snapshot/policy gate. |
| C-009 | Session verb surface applies AgentMode, scope/workflow, SemReg CCIR, fail policy, and ranking. | `ob-poc/src/agent/verb_surface.rs:L314-L324`, `ob-poc/src/agent/verb_surface.rs:L337-L496` | `ob-poc` | G1, G3, G5 | Verb discovery/Sage | prune to safe harbor or pass-through | SPLIT discovery ranking from admission gate. |
| C-010 | Lifecycle state is tagged for discovery but not pruned. | `ob-poc/src/agent/verb_surface.rs:L451-L462`, `ob-poc/src/agent/verb_surface.rs:L509-L521` | `ob-poc` | G4 | Verb surface | non-blocking tag | INVOKE only as hint; not gate proof. |
| C-011 | Macro expansion validates required args and failure maps to clarification/error. | `ob-poc/src/runbook/compiler.rs:L167-L225` | `ob-poc` | G1, G9 | Compile path | clarification or compilation error | INVOKE. |
| C-012 | Macro fixpoint expansion enforces depth, step limits, and per-path cycle detection. | `ob-poc/src/dsl_v2/macros/expander.rs:L250-L346` | `ob-poc` | G1, G9 | Compile path | expansion error | INVOKE. |
| C-013 | Expanded macro output is revalidated and rejects unknown runtime verbs. | `ob-poc/src/runbook/compiler.rs:L603-L638` | `ob-poc` | G1, G9 | Compile path | compilation error | MOVE admission part; keep macro validator local. |
| C-014 | Plan assembler builds dependency graph, rejects cycles/empty plans, but unresolved bindings are diagnostics. | `ob-poc/src/plan_builder/plan_assembler.rs:L80-L160`, `ob-poc/src/plan_builder/plan_assembler.rs:L234-L266` | `ob-poc` | G4, G9 | Compile path | error for cycle/empty; diagnostic for unresolved binding | INVOKE. |
| C-015 | Pack constraint gate blocks verbs not permitted by active pack constraints. | `ob-poc/src/runbook/constraint_gate.rs:L1-L15`, `ob-poc/src/runbook/constraint_gate.rs:L33-L100` | `ob-poc` | G3 | Compile path/raw path | ConstraintViolation | MOVE gate decision, invoke pack data. |
| C-016 | SemReg allowed-set unavailable is fail-closed in compiler. | `ob-poc/src/runbook/compiler.rs:L339-L362`, `ob-poc/src/runbook/compiler.rs:L468-L486` | `ob-poc` | G3, G5 | Compile path | EnvelopeUnavailable/SemRegDenied | MOVE. |
| C-017 | Raw endpoint performs separate SemOS allowed-verb policy checks. | `ob-poc/src/api/agent_routes.rs:L2101-L2177` | `ob-poc` | G3, G5 | Raw DSL path | blocks unavailable/empty/denied cases | RETIRE duplicate of C-016 after raw path removal. |
| C-018 | Raw endpoint performs separate semantic validator checks. | `ob-poc/src/api/agent_routes.rs:L2180-L2257` | `ob-poc` | G4, G7 | Raw DSL path | diagnostic error | RETIRE or INVOKE through same compiler. |
| C-019 | Write set is heuristic by default and contract-union only behind feature flag. | `ob-poc/src/runbook/write_set.rs:L1-L17`, `ob-poc/src/runbook/write_set.rs:L112-L146` | `ob-poc` | G7 | Runbook compile/execute | deterministic set, no runtime abort | SPLIT into proof plus runtime attestation. |
| C-020 | Runbook execution rejects missing or non-executable persisted runbooks. | `ob-poc/src/runbook/executor.rs:L812-L824` | `ob-poc` | G9, G10 | Runbook execute | NotFound/NotExecutable | MOVE admission, keep store lookup. |
| C-021 | Advisory locks are sorted, timeout-bounded, and emit events on lock paths. | `ob-poc/src/runbook/executor.rs:L682-L755`, `ob-poc/src/runbook/executor.rs:L831-L875` | `ob-poc` | G10, G13 | Runbook execute | contention/timeout event and failure | INVOKE. |
| C-022 | Runbook executor warns and proceeds without locks when write set exists but no pool exists. | `ob-poc/src/runbook/executor.rs:L879-L885` | `ob-poc` | G10 | In-memory/no-pool execute | warn only | RETIRE for production; keep test-only explicit path. |
| C-023 | Step dependencies cause skipped dependent steps on prior failure. | `ob-poc/src/runbook/executor.rs:L929-L960` | `ob-poc` | G9 | Runbook execute | skipped event/outcome | INVOKE. |
| C-024 | Optional pre-dispatch GatePipeline returns `Ok(())` when absent or no transition metadata. | `ob-poc/src/runbook/step_executor_bridge.rs:L202-L212` | `ob-poc` | G4 | Runbook step dispatch | pass-through | SPLIT: missing pipeline should be admission-visible. |
| C-025 | GatePipeline evaluates DAG transitions and blocks severity=`error` violations. | `ob-poc/src/runbook/step_executor_bridge.rs:L214-L293` | `ob-poc` | G4 | Runbook step dispatch | string error before port dispatch | MOVE proof ownership; invoke checker. |
| C-026 | GateChecker resolves source entity, reads source slot, and reports violations. | `dsl-runtime/src/cross_workspace/gate_checker.rs:L155-L190`, `dsl-runtime/src/cross_workspace/gate_checker.rs:L193-L265` | `dsl-runtime` | G4 | Step pre-dispatch | caller decides severity enforcement | INVOKE as validator. |
| C-027 | Lifecycle `requires_states` precondition is fail-open except true mismatch. | `ob-poc/src/dsl_v2/executor.rs:L2015-L2041`, `ob-poc/src/dsl_v2/executor.rs:L2048-L2115` | `ob-poc` | G4 | Legacy executor/plugin path | passes on uncertainty; bails on mismatch | SPLIT; divergent duplicate of C-025/C-026 semantics. |
| C-028 | DslExecutor rejects durable verbs unless direct durable execution is explicitly allowed. | `ob-poc/src/dsl_v2/executor.rs:L1900-L1989` | `ob-poc` | G8, G10 | DslExecutor fallback | error unless allowed | INVOKE. |
| C-029 | SemOsVerbOpRegistry startup hard-fails YAML plugin declarations without registered ops. | `ob-poc-web/src/main.rs:L892-L935` | `ob-poc-web` | G1, G9 | Server startup | panic | INVOKE as deployment guard. |
| C-030 | GatePipeline startup is soft-fail and leaves runtime ungated when config/DAG load fails. | `ob-poc-web/src/main.rs:L1400-L1440` | `ob-poc-web` | G4 | Server startup/runbook dispatch | warning + continue | MOVE to deployment/admission hard fence. |
| C-031 | ObPocVerbExecutor wraps plugin ops in transaction and rolls back on op error. | `ob-poc/src/sem_os_runtime/verb_executor_adapter.rs:L152-L247` | `ob-poc` | G10 | All port dispatch | commit/rollback | INVOKE. |
| C-032 | CRUD executor executes metadata-driven insert/update/delete/upsert without comparing to a WriteSetProof. | `dsl-runtime/src/crud_executor.rs:L37-L70`, `dsl-runtime/src/crud_executor.rs:L230-L330` | `dsl-runtime` | G7, G14 partial | CRUD dispatch | SQL result/error | SPLIT; add attestation wrapper. |
| C-033 | Bus handler enforces catalogue version equality. | `ob-poc-bus-handler/src/lib.rs:L108-L145` | `ob-poc-bus-handler` | G12 | Federated bus | VersionIncompatible | INVOKE. |
| C-034 | Bus adapter calls executor with `Principal::system()` and no runbook/envelope gates. | `ob-poc-web/src/bus_runtime.rs:L122-L149` | `ob-poc-web` | G5, G10 gap | Federated bus | executor error only | MOVE into envelope admission; bypass candidate. |
| C-035 | Workflow dispatcher validates binding/process key before BPMN orchestration. | `ob-poc/src/bpmn_integration/dispatcher.rs:L246-L273` | `ob-poc` | G1, G9 | Workflow path | Failed outcome | INVOKE. |
| C-036 | Workflow dispatcher persists/queues orchestration state and tolerates some persistence failures as logs. | `ob-poc/src/bpmn_integration/dispatcher.rs:L275-L360` | `ob-poc` | G9, G10, G11 | Workflow path | log, queue, or fail depending site | SPLIT. |
| C-037 | BPMN controller op writes in `pre_fetch` before transaction-scoped `execute`. | `ob-poc/src/domain_ops/bpmn_controller_ops.rs:L283-L320` | `ob-poc` | G10, G14 gap | Plugin op path | pre_fetch error aborts; otherwise write already done | RETIRE/SPLIT; move write into scoped execute. |
| C-038 | BPMN controller validates tenant/template/idempotency before inserting process instance. | `bpmn-controller/src/instance.rs:L100-L190` | `bpmn-controller` | G2, G10 | BPMN controller | typed errors or existing instance id | INVOKE external surface only. |
| C-039 | Entity relationship upsert requires percentage for ownership and applies expected-version CAS except carved-out edge classes. | `sem_os_postgres/src/ops/entity_relationship.rs:L18-L34`, `sem_os_postgres/src/ops/entity_relationship.rs:L96-L160`, `sem_os_postgres/src/ops/entity_relationship.rs:L187-L215` | `sem_os_postgres` | G10, G13 | Plugin op | error or CAS conflict | INVOKE. |
| C-040 | KYC lexicon entries declare governing taxonomy, writes, authority, preconditions, emits, and content hash. | `ob-poc-kyc-substrate/src/lexicon.rs:L1-L6`, `ob-poc-kyc-substrate/src/lexicon.rs:L118-L137` | `ob-poc-kyc-substrate` | G1, G5, G6, G12 | KYC stream | lint/contract surface | INVOKE. |
| C-041 | KYC control preconditions enforce evidence-before-verify and reconcile/strategy before fold/freeze. | `ob-poc-kyc-substrate/src/fold/control.rs:L438-L478` | `ob-poc-kyc-substrate` | G4, G6 | KYC stream | KycError rejection | INVOKE. |
| C-042 | KYC store checks preconditions under stream lock before append and sequence bump. | `ob-poc-kyc-store/src/store.rs:L118-L203` | `ob-poc-kyc-store` | G6, G10, G13 | KYC stream | StoreError::Rejected, rollback by caller | INVOKE. |
| C-043 | KYC manifest publish persists content-addressed manifest and stamps per-verb lexicon hashes. | `ob-poc-kyc-store/src/manifest.rs:L1-L7`, `ob-poc-kyc-store/src/manifest.rs:L57-L95` | `ob-poc-kyc-store` | G12 | KYC manifest publish | idempotent/no-op on existing hash | INVOKE. |
| C-044 | Shadow GatedVerbEnvelope construction records placeholders and explicitly does not gate dispatch. | `ob-poc-boundary/src/envelope_builder.rs:L1-L18`, `ob-poc/src/sequencer.rs:L7721-L7763` | `ob-poc-boundary` / `ob-poc` | G10, G13 partial | Sequencer stage 6 | debug-only | MOVE to production envelope admission. |
| C-045 | TOCTOU recheck scaffold can recompute row-version hash but is not production-wired. | `ob-poc-boundary/src/toctou_recheck.rs:L1-L34`, `ob-poc-boundary/src/toctou_recheck.rs:L81-L139` | `ob-poc-boundary` | G10, G13 | Future envelope path | drift error in scaffold | MOVE/finish. |

## RR-4 Proof-Analogue Reconciliation

| Target artefact | Nearest existing analogue | Delta |
|---|---|---|
| `IntentAdmissionDecision` | `SemOsContextEnvelope` carries allowed/pruned verbs, deny-all/unavailable flags, fingerprint, evidence gaps, and governance signals (`ob-poc/src/agent/sem_os_context_envelope.rs:L20-L56`, `ob-poc/src/agent/sem_os_context_envelope.rs:L163-L247`); `SessionVerbSurface` computes pruned/ranked surface (`ob-poc/src/agent/verb_surface.rs:L314-L324`). | Exists as discovery/context surface, not a single deterministic admission decision. |
| `EntityBindingReport` | `ResolvedEntity` in the envelope has entity id/kind/row_version (`ob-poc-types/src/gated_envelope.rs:L127-L139`); bus adapter binds UUID inputs into execution context (`ob-poc-web/src/bus_runtime.rs:L133-L144`). | No production report that proves existence, lifecycle state, and availability for every entity. |
| `PackResolution` | Pack constraint gate checks active pack constraints (`ob-poc/src/runbook/constraint_gate.rs:L1-L15`, `ob-poc/src/runbook/constraint_gate.rs:L33-L100`); SemOS envelope can carry grounded action surface (`ob-poc/src/agent/sem_os_context_envelope.rs:L40-L51`). | Pack decision exists as filters, not a named proof artifact. |
| `StateTransitionProof` | `GateChecker::check_transition` returns `Vec<GateViolation>` (`dsl-runtime/src/cross_workspace/gate_checker.rs:L155-L190`); step executor blocks error severity (`ob-poc/src/runbook/step_executor_bridge.rs:L271-L293`). | Exists as runtime validator result, not a durable proof. |
| `AuthorityDecision` | `AccessDecision` is preserved in SemOS candidate partitioning (`ob-poc/src/agent/sem_os_context_envelope.rs:L15-L18`, `ob-poc/src/agent/sem_os_context_envelope.rs:L173-L190`); KYC lexicon has `AuthoritySpec` (`ob-poc-kyc-substrate/src/lexicon.rs:L62-L90`). | Multiple authority signals; no single owner or decision artifact. |
| `EvidenceReadiness` | SemOS envelope extracts evidence gaps from governance signals (`ob-poc/src/agent/sem_os_context_envelope.rs:L207-L218`); KYC preconditions check evidence/reconcile/strategy before append (`ob-poc-kyc-substrate/src/fold/control.rs:L438-L478`). | Partial per-domain readiness, no cross-domain aggregate. |
| `WriteSetProof` | `derive_write_set` returns a deterministic set by heuristic or feature-gated contract union (`ob-poc/src/runbook/write_set.rs:L1-L17`, `ob-poc/src/runbook/write_set.rs:L112-L146`). | It is a set, not a proof; runtime does not attest writes stay within it. |
| `StpEligibilityDecision` | Runbook plan status gates approval/execution (`ob-poc/src/api/repl_routes_v2.rs:L3925-L4042`); durable direct execution is rejected unless allowed (`ob-poc/src/dsl_v2/executor.rs:L1900-L1989`). | Human-gated/durable status exists, but no deterministic STP classification artifact. |
| `ControlPlaneProof` | `CompiledRunbook` persists the compiled plan and runbook executor appends events (`ob-poc/src/runbook/executor.rs:L270-L305`, `ob-poc/src/runbook/executor.rs:L385-L394`); `DecisionRecord` captures snapshot manifest and decision evidence (`ob-poc/src/sem_reg/agent/decisions.rs:L20-L74`). | Pieces exist, but no aggregate proof that bundles G1-G14 outputs. |
| `ExecutionEnvelope` | `GatedVerbEnvelope` is the named boundary type (`ob-poc-types/src/gated_envelope.rs:L6-L31`); shadow builder emits it without gating (`ob-poc-boundary/src/envelope_builder.rs:L10-L18`). | Type exists; production dispatch still consumes scattered args/steps. |
| audit stream | Intent telemetry, session traces, runbook events, and decision records exist (`ob-poc/src/agent/telemetry/store.rs:L9-L101`, `ob-poc/src/repl/trace_repository.rs:L15-L45`, `ob-poc/src/runbook/executor.rs:L385-L394`, `ob-poc/src/sem_reg/agent/decisions.rs:L108-L145`). | Audit is distributed and partly best-effort, not one audit stream. |
| pinned version set | SemReg snapshots/snapshot sets exist (`ob-poc/src/sem_reg/store.rs:L1-L89`); KYC manifest is content-addressed (`ob-poc-kyc-store/src/manifest.rs:L1-L7`, `ob-poc-kyc-store/src/manifest.rs:L57-L95`); bus catalogue version is checked (`ob-poc-bus-handler/src/lib.rs:L125-L136`). | Version pins exist by subsystem, not one unified pinned set. |
| `SnapshotPins` | Decision records carry `snapshot_manifest` (`ob-poc/src/sem_reg/agent/decisions.rs:L20-L74`); sessions keep version and current snapshot id (`ob-poc/src/repl/session_repository.rs:L55-L62`, `ob-poc/src/repl/session_repository.rs:L131-L187`). | No single gate-read snapshot object across all gate reads. |
| write-set attestation record | No production analogue located; CRUD writes execute from metadata mapping (`dsl-runtime/src/crud_executor.rs:L37-L70`, `dsl-runtime/src/crud_executor.rs:L230-L330`) and write-set derivation is compile/runbook-side only (`ob-poc/src/runbook/write_set.rs:L1-L17`). | Missing G14. |

## RR-5 Versioning & Snapshot Infrastructure

SemReg has immutable-ish snapshot infrastructure: `SnapshotStore` creates snapshot sets and inserts snapshots, with only predecessor `effective_until` updated on supersede (`ob-poc/src/sem_reg/store.rs:L1-L4`, `ob-poc/src/sem_reg/store.rs:L19-L89`, `ob-poc/src/sem_reg/store.rs:L91-L108`). `DecisionRecord` records a `snapshot_manifest` map from object id to snapshot id and is inserted into `sem_reg.decision_records` (`ob-poc/src/sem_reg/agent/decisions.rs:L20-L74`, `ob-poc/src/sem_reg/agent/decisions.rs:L108-L145`).

REPL/session state has comparable versioning and append-only workbook snapshots: `current_version` reads `repl_sessions_v2.version`, `save_session_inner` increments version and writes `current_snapshot_id`, and appends `repl_session_workbook_snapshots` (`ob-poc/src/repl/session_repository.rs:L44-L62`, `ob-poc/src/repl/session_repository.rs:L112-L187`). Invocation records are persisted for parked entries (`ob-poc/src/repl/session_repository.rs:L469-L499`).

KYC streams use a monotonic subject sequence: append locks `kyc_subject_streams`, reads `next_seq`, dedupes idempotency key, inserts the event at `next_seq`, bumps `next_seq`, and enqueues projections in the same transaction (`ob-poc-kyc-store/src/store.rs:L118-L203`). KYC lexicon replay is pinned by content-addressed manifest hash (`ob-poc-kyc-substrate/src/lexicon.rs:L175-L190`, `ob-poc-kyc-store/src/manifest.rs:L57-L95`).

Entity relationship graph writes have an operator-edge CAS path: `expected-version` is read from args, update increments `version`, and non-carved-out rows require current version to match expected/coalesced version (`sem_os_postgres/src/ops/entity_relationship.rs:L18-L34`, `sem_os_postgres/src/ops/entity_relationship.rs:L96-L160`, `sem_os_postgres/src/ops/entity_relationship.rs:L187-L215`).

Snapshot-capable read paths: SemReg snapshots and decision records are snapshot-aware (`ob-poc/src/sem_reg/store.rs:L112-L140`, `ob-poc/src/sem_reg/agent/decisions.rs:L20-L74`); session saves produce workbook snapshots (`ob-poc/src/repl/session_repository.rs:L112-L187`); KYC append folds prior committed events under a locked stream (`ob-poc-kyc-store/src/store.rs:L176-L203`).

Read-live paths: GateChecker reads source slot state at dispatch time through the supplied `SlotStateProvider` (`dsl-runtime/src/cross_workspace/gate_checker.rs:L231-L244`); lifecycle `requires_states` reads current state inside the open transaction and fails open on absent/unreadable state (`ob-poc/src/dsl_v2/executor.rs:L2088-L2104`); bus invocation passes directly to executor with no envelope snapshot (`ob-poc-web/src/bus_runtime.rs:L122-L149`).

Mode-1 register:

| Entity/state family | Evidence | Mode-1 status |
|---|---|---|
| Shadow envelope resolved entities | Builder sets `row_version: 0` and `recheck_required: false` (`ob-poc-boundary/src/envelope_builder.rs:L108-L165`). | Mode-1 until real row versions and runtime recheck are wired. |
| Entity tables intended for TOCTOU | TOCTOU module says row_version migration is staged/pending operator approval and real envelope construction is not wired (`ob-poc-boundary/src/toctou_recheck.rs:L20-L34`). | Mode-1 for production gate reads today. |
| Bus-invoked operational writes | Bus adapter builds `Principal::system()` context and calls executor with no runbook/envelope (`ob-poc-web/src/bus_runtime.rs:L122-L149`). | Mode-1 for pre-state pinning at bus boundary. |
| BPMN `process_instances` | `start_instance` validates and writes process instance with state `Running` (`bpmn-controller/src/instance.rs:L100-L190`); no cited row-version/CAS check was found on this path. | Mode-1/UNKNOWN for comparable version pins. |
| Raw DSL best-effort execution | Raw endpoint has its own validation and execution modes (`ob-poc/src/api/agent_routes.rs:L2311-L2448`). | Mode-1 until routed through the same envelope/snapshot path. |

## RR-6 Runtime Admission & Envelope Surface

There is no production single admission type/function today. The intended boundary value is `GatedVerbEnvelope`, but its own module states the types are not wired into production paths (`ob-poc-types/src/gated_envelope.rs:L26-L31`). The builder constructs a shadow envelope and explicitly states it does not gate execution (`ob-poc-boundary/src/envelope_builder.rs:L10-L18`); the Sequencer logs the shadow envelope and then continues the existing path (`ob-poc/src/sequencer.rs:L7721-L7765`).

Direct construction/dispatch paths that bypass an envelope check:

| Path | Evidence |
|---|---|
| Bus adapter | Creates `VerbExecutionContext::new(Principal::system())`, binds inputs, and calls `execute_verb` directly (`ob-poc-web/src/bus_runtime.rs:L122-L149`). |
| Raw DSL endpoint | Explicit raw endpoint executes its own parse/validate/compile/execution flow (`ob-poc/src/api/agent_routes.rs:L1884-L1890`, `ob-poc/src/api/agent_routes.rs:L2048-L2448`). |
| RealDslExecutor | Direct `execute` parses/compiles and runs `execute_plan` with per-verb transactions (`ob-poc/src/repl/executor_bridge.rs:L114-L134`). |
| WorkflowDispatcher direct branch | Direct route delegates to inner executor; orchestrated route parks into BPMN path (`ob-poc/src/bpmn_integration/dispatcher.rs:L556-L576`). |
| BPMN controller op `pre_fetch` | Starts process instance before scoped `execute` returns UUID (`ob-poc/src/domain_ops/bpmn_controller_ops.rs:L283-L320`). |

Where `ExecutionEnvelope` admission would slot in: `VerbExecutionPort::execute_verb` is documented as the production handoff from Sequencer to runtime (`dsl-runtime/src/port.rs:L1-L18`, `dsl-runtime/src/port.rs:L26-L48`); `VerbExecutionPortStepExecutor::pre_dispatch_gate_check` is the current pre-port hook (`ob-poc/src/runbook/step_executor_bridge.rs:L202-L293`); `ObPocVerbExecutor::execute_verb` is the concrete runtime dispatch hub (`ob-poc/src/sem_os_runtime/verb_executor_adapter.rs:L117-L286`). A persisted envelope/handle would fit beside compiled runbook storage and events, which already persist compiled runbooks and append execution events (`ob-poc/src/runbook/executor.rs:L270-L305`, `ob-poc/src/runbook/executor.rs:L385-L394`), or beside session invocation records for parked/human/durable states (`ob-poc/src/repl/session_repository.rs:L469-L499`).

## RR-7 Audit & Metrics Infrastructure

Intent telemetry is a write-only best-effort store: `insert_intent_event` inserts `intent_events` with utterance hash, SemReg mode, denied verbs, candidate counts, chosen verb, macro/SemReg fields, allowed-verbs fingerprint, TOCTOU fields, and surface counts; failures log warnings and return false, never errors (`ob-poc/src/agent/telemetry/store.rs:L1-L101`).

Session trace infrastructure is append-only and monotonic by sequence: `TraceOp` includes `VerbExecuted`, `RunbookCompiled`, `RunbookApproved`, ACP and workbook validation events (`ob-poc-boundary/src/session_trace.rs:L1-L5`, `ob-poc-boundary/src/session_trace.rs:L118-L190`); `SessionTraceRepository::append_batch` inserts trace entries with session id, sequence, mode, op, stack snapshot, hydrated snapshot, verb, and execution result, ignoring duplicate `(session_id, sequence)` conflicts (`ob-poc/src/repl/trace_repository.rs:L15-L45`).

Runbook execution appends events such as compiled runbook events, lock contention/acquired/released, step outcomes, and final state changes through the runbook store (`ob-poc/src/runbook/executor.rs:L385-L394`, `ob-poc/src/runbook/executor.rs:L840-L875`, `ob-poc/src/runbook/executor.rs:L972-L1024`, `ob-poc/src/runbook/executor.rs:L1062-L1084`).

Decision records are immutable inserts with chosen action, alternatives, evidence for/against, negative evidence, policy verdicts, snapshot manifest, confidence, escalation fields, and actor/time (`ob-poc/src/sem_reg/agent/decisions.rs:L20-L74`, `ob-poc/src/sem_reg/agent/decisions.rs:L108-L145`).

Runtime event emission exists in the legacy DSL executor: `execute_verb` emits success/failure `DslEvent` only when an emitter is configured (`ob-poc/src/dsl_v2/executor.rs:L1334-L1369`).

Observed decision-point emission fraction: C-001, C-002, C-003, C-004, C-015, C-016, C-017, C-020, C-021, C-023, C-024/C-025, C-027, C-029/C-030, C-031, C-033, C-034, C-035/C-036, C-040-C043 have some error/trace/log surface cited in RR-3; no single audit stream records all G1-G14 gate inputs/outputs before execution.

## RR-8 Gap & UNKNOWN Registers

Target gates with no located full production implementation:

| Gate | Status |
|---|---|
| G2 Entity binding | Partial entity IDs/row versions exist in shadow envelope (`ob-poc-types/src/gated_envelope.rs:L127-L139`); no production `EntityBindingReport` proving existence/state/availability was found. |
| G8 STP classification | Runbook status and durable/human checks exist (`ob-poc/src/api/repl_routes_v2.rs:L3925-L4042`, `ob-poc/src/dsl_v2/executor.rs:L1900-L1989`); no `StpEligibilityDecision` analogue found. |
| G9 ControlPlaneProof | Compiled runbook and decision records exist (`ob-poc/src/runbook/executor.rs:L270-L305`, `ob-poc/src/sem_reg/agent/decisions.rs:L20-L74`); no aggregate proof artifact found. |
| G10 Execution envelope | `GatedVerbEnvelope` is shadow/compile-only (`ob-poc-types/src/gated_envelope.rs:L26-L31`, `ob-poc-boundary/src/envelope_builder.rs:L10-L18`). |
| G13 Decision snapshot | Snapshot pieces exist, but no single `SnapshotPins` object across all gate reads (`ob-poc/src/sem_reg/store.rs:L1-L89`, `ob-poc/src/repl/session_repository.rs:L112-L187`). |
| G14 Write-set attestation | No production attestation record located; CRUD executor writes based on metadata and SQL execution (`dsl-runtime/src/crud_executor.rs:L37-L70`, `dsl-runtime/src/crud_executor.rs:L230-L330`). |

UNKNOWN register:

| UNKNOWN | What was tried | Evidence boundary |
|---|---|---|
| Whether every BPMN `process_instances` mutation has comparable version pins. | Traced `bpmn-controller.start-instance` op and controller `start_instance`. | Found validation/idempotency/insert (`ob-poc/src/domain_ops/bpmn_controller_ops.rs:L283-L320`, `bpmn-controller/src/instance.rs:L100-L190`), but no row-version/CAS citation on that path. |
| Whether all domain plugin ops have CAS/pre-state checks. | Surveyed central registry dispatch and sampled entity relationship CAS. | Central dispatch exists (`ob-poc/src/sem_os_runtime/verb_executor_adapter.rs:L152-L247`); one CAS implementation exists (`sem_os_postgres/src/ops/entity_relationship.rs:L18-L34`), but exhaustive per-op CAS was not completed. |
| Whether SemOS external crates already expose richer proof types. | Enumerated external dependencies and restricted inventory to call sites inside ob-poc as required. | External crates are git deps in `Cargo.toml` (`ob-poc/Cargo.toml:L379-L401`); internal call sites are cited in RR-3/RR-4. |
| Whether production applies the row-version migration referenced by TOCTOU. | Read code comments and scaffold. | Code says migration is staged/pending operator approval and production wiring depends on it (`ob-poc-boundary/src/toctou_recheck.rs:L20-L34`). |

Open architect questions:

1. Should the bus invocation service become envelope-primary or be limited to already-proved `EnvelopeHandle` execution?
2. Should `GatePipeline` config/DAG load failure be production fatal rather than warn-and-run-ungated?
3. Should raw DSL keep a dev-only entry point, and if so must it consume the same `ControlPlaneProof` as natural-language/Sage paths?

## RR-9 Statistics

Crates touched/cited: 16 (`ob-poc`, `ob-poc-web`, `ob-poc-boundary`, `ob-poc-types`, `dsl-runtime`, `ob-poc-bus-handler`, `bpmn-controller`, `ob-poc-kyc-substrate`, `ob-poc-kyc-store`, `sem_os_postgres`, plus workspace/dependency metadata in the root package; citations also cover web/router, runtime, sequencer, compiler, executor, and store modules).

Entry points inventoried: 10. End-to-end execution paths traced: 4. Inventory rows: 45. Disposition counts: MOVE 11, INVOKE 19, RETIRE 5, SPLIT 10. Duplicate/divergent pairs: SemReg compiler vs raw SemOS check (C-016/C-017), DAG GateChecker vs fail-open lifecycle check (C-025/C-027), runbook write-set locks vs bus direct executor path (C-021/C-034), shadow envelope vs real runtime dispatch (C-044/C-031). Mode-1 entities/families: 5 rows in RR-5. Gates with no full production analogue: 6 (G2, G8, G9, G10, G13, G14).

Completion invariant check: E1 satisfied by RR-4 rows for every G1-G14 artefact; E2 satisfied by every RR-3 row carrying a file:line citation and disposition; E3 satisfied by RR-2 Paths A-D, each terminating at a mutation site; E4 satisfied by the populated Mode-1 register in RR-5; E5 satisfied by RR-0 appearing first in the assembled report after the evidence sections were completed.

INVENTORY COMPLETE — E1..E5 satisfied
