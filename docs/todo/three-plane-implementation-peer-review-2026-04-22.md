# Three-Plane Implementation Peer Review TODO

Date: 2026-04-22

## Purpose

Capture the code-review findings for the landed three-plane implementation work and turn them into a concrete remediation plan suitable for peer review before follow-on code changes.

This document focuses on:

- correctness regressions introduced by the post-slice-`#80` plugin-dispatch cutover
- transaction-boundary violations against the `TransactionScope` contract
- startup drift-detection regressions
- residual code-hygiene issues left in the tree after cleanup

## Reviewed scope

Primary reviewed range:

- `3ae55358^..HEAD`
- slice `#66` through slice `#80`
- docs follow-up commit `8303e1d9`

Primary architectural seams reviewed:

- `rust/src/dsl_v2/executor.rs`
- `rust/src/sem_os_runtime/verb_executor_adapter.rs`
- `rust/src/repl/executor_bridge.rs`
- `rust/crates/ob-poc-web/src/main.rs`
- `rust/src/domain_ops/mod.rs`
- Pattern B op files still living in `rust/src/domain_ops/*`

Verification performed during review:

- `env RUSTC_WRAPPER= cargo check`
- result: passes, but emits warnings

## Findings summary

**At-a-glance (2026-04-22 extended sweep):** 23 findings total. P0 × 2 (SemOS bypass on Tier -2A/-2B
verb search), P1 × 9 (F1 registry, F2 txn leaks, F5 envelope, F6 single-dispatch, F7 PendingStateAdvance,
F16 raw-execute flag, F17 fail-open runbook compiler, F18 dead feature-gate branch, F19 duplicate MCP
tool), P2 × 5 (F8 TOCTOU, F9 row-version, F11 Pattern B ledger, F20 410 stubs, F21 orphan/feature-flag
cleanup), P3 × 4 (F4 hygiene, F10 dual-schema YAML, F12 session lock, F22 stale refs),
positive-verification × 2 (F13 outbox, F23 multiple clean audits).

### F1. Plugin dispatch is now opt-in on `DslExecutor`, but many live call sites were not updated

Severity: High

Impact:

- any path constructing `DslExecutor::new(...)` without `.with_sem_os_ops(...)` will fail on plugin verbs
- failure mode is runtime-only, not startup-time
- regression affects recursive DSL execution from migrated Pattern B ops as well as some app entrypoints

Root cause:

- `DslExecutor` now stores `sem_os_ops: Option<Arc<SemOsVerbOpRegistry>>`
- `DslExecutor::new(...)` initializes it to `None`
- plugin execution hard-fails if the registry is absent or missing the FQN

Primary implementation references:

- `rust/src/dsl_v2/executor.rs`
  - `DslExecutor` struct
  - `DslExecutor::new`
  - `DslExecutor::with_sem_os_ops`
  - `DslExecutor::execute_verb`

Relevant locations:

- `rust/src/dsl_v2/executor.rs:1139`
- `rust/src/dsl_v2/executor.rs:1183`
- `rust/src/dsl_v2/executor.rs:1204`
- `rust/src/dsl_v2/executor.rs:1311`

Error path introduced:

- `Plugin <domain>.<verb> has no SemOsVerbOp registered`

Known live call sites missing registry injection (classified 2026-04-22):

- **Confirmed broken (production, plugin-capable path):**
  - `rust/src/mcp/handlers/core.rs:1001` (`dsl_execute` handler)
  - `rust/src/mcp/handlers/core.rs:1387` (`dsl_execute_submission` handler)
  - `rust/src/api/agent_service.rs:1398` (`RealDslExecutor::new` inside
    `#[cfg(feature = "runbook-gate-vnext")]` — production enabled)
  - `rust/src/dsl_v2/sheet_executor.rs:566`

- **Dead code (downgrade to cleanup candidate; see F18):**
  - `rust/src/api/agent_service.rs:1153` — behind `#[cfg(not(feature = "runbook-gate-vnext"))]`;
    feature always on in prod
  - `rust/src/api/agent_state.rs:86` — constructed but not observed to be used anywhere

- **False positive (reviewer-missed injection — counter-example of the desired pattern):**
  - `rust/src/repl/executor_bridge.rs` is NOT broken. Lines 93-96 conditionally call
    `.with_sem_os_ops(ops.clone())` when `self.sem_os_ops.is_some()`.

Existing canonical injection path (reference for remediation):

- `rust/crates/ob-poc-web/src/main.rs:765-773` builds `sem_os_ops` once via
  `sem_os_postgres::ops::build_registry() + ob_poc::domain_ops::extend_registry()`.
- Executors at `ob-poc-web/main.rs:855, 882, 988, 1017, 1035` chain
  `.with_sem_os_ops(sem_os_ops.clone())`.
- The pattern exists; the problem is that ad-hoc `DslExecutor::new(pool)` calls in request handlers
  and Pattern B ops do not use it.

Known recursive Pattern B op sites missing registry injection:

- `rust/src/domain_ops/template_ops.rs`
  - `template_invoke_impl`
  - `DslExecutor::new(pool.clone())`
  - location:
    - `rust/src/domain_ops/template_ops.rs:167`
- `rust/src/domain_ops/onboarding.rs`
  - onboarding auto-complete flow
  - `DslExecutor::new(pool.clone())`
  - location:
    - `rust/src/domain_ops/onboarding.rs:122`
- `rust/src/domain_ops/gleif_ops.rs`
  - multiple helper flows creating nested DSL executors
  - locations observed:
    - `rust/src/domain_ops/gleif_ops.rs:99`
    - `rust/src/domain_ops/gleif_ops.rs:707`
    - `rust/src/domain_ops/gleif_ops.rs:741`
    - `rust/src/domain_ops/gleif_ops.rs:762`

Additional non-reviewed-for-liveness call sites found by grep and likely needing classification:

- `rust/src/dsl_v2/batch_executor.rs:397`
- `rust/src/templates/harness.rs:533`
- `rust/src/gleif/repository.rs:593`
- `rust/tests/*`
- `rust/xtask/*`

### F2. Migrated ops still bypass ambient transaction scope by using `scope.pool().clone()`

Severity: High

Impact:

- escaped queries do not participate in `self.transaction()`
- outer rollback will not revert work performed through fresh pool connections
- violates the explicit contract of the three-plane transaction model
- especially dangerous inside nested DSL recursion or multi-step plugin flows

Architectural contract reference:

- `rust/crates/dsl-runtime/src/tx.rs`
  - `TransactionScope::pool()` is explicitly transitional
  - docs state pooled queries do not participate in `self.transaction()`

Concrete contract lines:

- `rust/crates/dsl-runtime/src/tx.rs:66`
- `rust/crates/dsl-runtime/src/tx.rs:72`
- `rust/crates/dsl-runtime/src/tx.rs:73`
- `rust/crates/dsl-runtime/src/tx.rs:74`

Dispatch implementation that assumes scoped atomicity:

- `rust/src/dsl_v2/executor.rs`
  - `dispatch_plugin_via_sem_os_op`
  - opens `PgTransactionScope`
  - commits on `Ok`
  - rolls back on `Err`
  - locations:
    - `rust/src/dsl_v2/executor.rs:1503`
    - `rust/src/dsl_v2/executor.rs:1567`

Observed Pattern B violations:

- `rust/src/domain_ops/template_ops.rs`
  - `TemplateInvoke::execute`
  - clones `scope.pool()`
  - location:
    - `rust/src/domain_ops/template_ops.rs:103`
  - `template_invoke_impl`
  - creates nested `DslExecutor` from pool
  - location:
    - `rust/src/domain_ops/template_ops.rs:167`
- `rust/src/domain_ops/onboarding.rs`
  - `OnboardingAutoComplete::execute`
  - clones `scope.pool()`
  - location:
    - `rust/src/domain_ops/onboarding.rs:78`
  - nested executor creation
  - location:
    - `rust/src/domain_ops/onboarding.rs:122`
- `rust/src/domain_ops/gleif_ops.rs`
  - multiple `scope.pool().clone()` usages at:
    - `rust/src/domain_ops/gleif_ops.rs:59`
    - `rust/src/domain_ops/gleif_ops.rs:230`
    - `rust/src/domain_ops/gleif_ops.rs:283`
    - `rust/src/domain_ops/gleif_ops.rs:433`
    - `rust/src/domain_ops/gleif_ops.rs:842`
    - `rust/src/domain_ops/gleif_ops.rs:1068`
    - `rust/src/domain_ops/gleif_ops.rs:1128`
    - `rust/src/domain_ops/gleif_ops.rs:1188`
    - `rust/src/domain_ops/gleif_ops.rs:1351`
  - nested executors at:
    - `rust/src/domain_ops/gleif_ops.rs:99`
    - `rust/src/domain_ops/gleif_ops.rs:707`
    - `rust/src/domain_ops/gleif_ops.rs:741`
    - `rust/src/domain_ops/gleif_ops.rs:762`
- `rust/src/domain_ops/request_ops.rs`
  - repeated pool clones inside scoped ops
  - observed at:
    - `rust/src/domain_ops/request_ops.rs:70`
    - `rust/src/domain_ops/request_ops.rs:254`
    - `rust/src/domain_ops/request_ops.rs:348`
    - `rust/src/domain_ops/request_ops.rs:423`
    - `rust/src/domain_ops/request_ops.rs:491`
    - `rust/src/domain_ops/request_ops.rs:607`
    - `rust/src/domain_ops/request_ops.rs:693`
    - `rust/src/domain_ops/request_ops.rs:769`
    - `rust/src/domain_ops/request_ops.rs:831`
    - `rust/src/domain_ops/request_ops.rs:979`
    - `rust/src/domain_ops/request_ops.rs:1096`
- `rust/src/domain_ops/source_loader_ops.rs`
  - repeated scoped pool clones at:
    - `rust/src/domain_ops/source_loader_ops.rs:192`
    - `rust/src/domain_ops/source_loader_ops.rs:349`
    - `rust/src/domain_ops/source_loader_ops.rs:389`
    - `rust/src/domain_ops/source_loader_ops.rs:507`
    - `rust/src/domain_ops/source_loader_ops.rs:633`
    - `rust/src/domain_ops/source_loader_ops.rs:673`
    - `rust/src/domain_ops/source_loader_ops.rs:772`
- `rust/src/domain_ops/booking_principal_ops.rs`
  - repeated scoped pool clones across many verbs
  - first examples:
    - `rust/src/domain_ops/booking_principal_ops.rs:134`
    - `rust/src/domain_ops/booking_principal_ops.rs:171`
    - `rust/src/domain_ops/booking_principal_ops.rs:233`
    - `rust/src/domain_ops/booking_principal_ops.rs:269`
- `rust/src/domain_ops/trading_profile.rs`
  - repeated scoped pool clones across many verbs
  - first examples:
    - `rust/src/domain_ops/trading_profile.rs:89`
    - `rust/src/domain_ops/trading_profile.rs:171`
    - `rust/src/domain_ops/trading_profile.rs:218`
    - `rust/src/domain_ops/trading_profile.rs:289`

Observed same smell in SemOS-native ops as well:

- `rust/crates/sem_os_postgres/src/ops/entity.rs`
  - `PlaceholderResolver::new(scope.pool().clone())`
  - examples:
    - `rust/crates/sem_os_postgres/src/ops/entity.rs:227`
    - `rust/crates/sem_os_postgres/src/ops/entity.rs:260`
- `rust/crates/sem_os_postgres/src/ops/document.rs`
  - file-level docs already admit transitional pool usage
  - examples:
    - `rust/crates/sem_os_postgres/src/ops/document.rs:25`
    - `rust/crates/sem_os_postgres/src/ops/document.rs:171`
- `rust/crates/sem_os_postgres/src/ops/docs_bundle.rs:64`
- `rust/crates/sem_os_postgres/src/ops/bods.rs:48`
- `rust/crates/sem_os_postgres/src/ops/tollgate.rs:19`

This is broader than the reviewed slice, but the Pattern B migration left the same hazard active in newly touched code.

Whole-tree classification (2026-04-22 sweep):

| Class | Count | Meaning |
|-------|-------|---------|
| (A) Write-path leak | 92 | INSERT/UPDATE/DELETE via fresh pool connection inside op body |
| (B) Read-only escape | 42 | SELECT-only; no correctness loss but same anti-pattern |
| (C) Service-bridge | 20 | Acknowledged transitional (module docstrings admit it) |
| (D) Nested DSL executor | 4 | Compounds F1: inner tree opens fresh transaction root |
| **Total** | **160** | — |

**(A + D) / total = 60%.** The majority of migrated Pattern B ops silently bypass the ambient scope.

Worst offenders not individually enumerated above:

- `rust/src/domain_ops/trading_profile.rs` — 34 A-writes
- `rust/src/domain_ops/booking_principal_ops.rs` — 32 A-writes
- `rust/src/domain_ops/request_ops.rs` — 11 A-writes
- `rust/src/domain_ops/source_loader_ops.rs` — 7 A-writes (all via `log_research_action`)
- `rust/src/domain_ops/gleif_ops.rs` — 8 A-writes + 1 D-nested-executor

Per-verb transaction scope complication: plugin dispatch at
`rust/src/dsl_v2/executor.rs:1503` opens a fresh `PgTransactionScope` **per verb**. Each (A) site
leaks out of *that* per-verb scope (which is itself not the Sequencer-owned scope v0.3 §8.4 requires
— see F6).

Missing primitive (spec gap, not migration gap): no scope-aware nested-DSL entrypoint exists anywhere
in the tree. No `DslExecutor::execute_plan_in_scope(…)`, no `VerbExecutionPort` method that accepts
`&mut dyn TransactionScope`. Phase 2 of the remediation plan needs this as a prerequisite.

Clean reference point: `rust/crates/dsl-runtime/src/cross_workspace/` (10 files, 2026-04-02 addition)
has zero `scope.pool()` hits. It is the working example of what a scope-respecting module looks like.

Exact hard-fail error literal from F1 (`rust/src/dsl_v2/executor.rs:1328-1334`):

> "Plugin {}.{} has no SemOsVerbOp registered (SemOsVerbOpRegistry is either absent on this executor
>  or missing the FQN). Wire `DslExecutor::with_sem_os_ops` at host startup."

### F3. Coverage enforcement moved from runtime startup to tests only

Severity: Medium

Impact:

- missing YAML/plugin registrations are no longer fatal at startup
- broken production deploy is possible if tests are skipped or a gap is introduced outside CI coverage
- runtime failure now occurs on first verb invocation instead of at boot

Before:

- startup performed a strict coverage check after registry construction

Now:

- startup only builds and logs the registry
- coverage assertion exists only in test code

Primary locations:

- startup registry construction:
  - `rust/crates/ob-poc-web/src/main.rs:765`
- startup log only:
  - `rust/crates/ob-poc-web/src/main.rs:770`
- coverage test:
  - `rust/src/domain_ops/mod.rs:602`

Gap:

- there is no longer a production fail-fast equivalent to the removed strict check

### F4. Residual hygiene debt remains after cleanup slice `#80`

Severity: Low

Impact:

- warning noise obscures meaningful regressions
- deleted-architecture terminology remains in comments/docs inside code
- weakens trust in “cleanup complete” claim

Observed `cargo check` warnings:

- invalid cfg feature:
  - `rust/crates/dsl-runtime/src/document_bundles/service.rs:255`
  - `#[cfg(all(test, feature = "database"))]`
- unused parameter:
  - `rust/crates/dsl-runtime/src/crud_executor.rs:1358`
  - `resolve_entity_type_code(crud: &VerbCrudMapping, ...)`
- ~~dead helpers in `rust/src/domain_ops/sem_os_helpers.rs`~~ **Retracted 2026-04-22.**
  Re-verification found `get_string_arg`, `get_bool_arg`, `extract_args_as_json`,
  `build_actor_from_ctx`, `delegate_to_tool*`, `convert_tool_result*` all have active callers across
  `dsl-core/compiler.rs`, `dsl_v2/graph_executor.rs`, and within the module itself (70+ call sites).
  Only the stale module docstring is a valid hygiene item (handled below + F22).

Additional smell:

- `rust/src/domain_ops/sem_os_helpers.rs` module docs still say `CustomOps`
- this is stale terminology post-slice-`#80`

### F5. Envelope types compiled but not wired (v0.3 §10.3, §17 items 3/5/6)

Severity: High — blocks stage-6 gate decision and TOCTOU recheck; §17 Definition-of-Done items 3, 5,
and 6 cannot be satisfied.

Status:

- `GatedVerbEnvelope`, `PendingStateAdvance`, `OutboxDraft`, `StateGateHash` (with BLAKE3),
  `AuthorisationProof`, `CatalogueSnapshotId`, `TraceId`, `EnvelopeVersion`, `TransactionScopeId`
  exist in `rust/crates/ob-poc-types/src/gated_envelope.rs`.
- `TransactionScope` trait correctly located in `rust/crates/dsl-runtime/src/tx.rs` per the §10.3
  2026-04-20 correction.
- No orchestrator path constructs a `GatedVerbEnvelope`. Stage 6 (gate decision) is unwired.
- `Sequencer` stage-6 typed helper (`GateDecisionOutput::from_envelope`) is ready but has no
  `GatedVerbEnvelope` to consume.

Impact:

- No TOCTOU recheck possible.
- No catalogue-snapshot-id propagation — mid-refactor catalogue reloads can invalidate in-flight
  gating silently.
- No `trace_id` correlation across stages.

### F6. Single-dispatch-site invariant violated (v0.3 §8.4, L2 lint)

Severity: High — direct violation of §8.4 transaction-scope ownership.

Actual state:

- Plugin dispatch happens inside `DslExecutor::execute_verb` at `rust/src/dsl_v2/executor.rs:1317` and
  `:1351`.
- Each plugin op opens its own per-verb `PgTransactionScope` at `dsl_v2/executor.rs:1503`.
- Not a single dispatch site at the Sequencer stage-8 inner loop; not a single Sequencer-owned
  transaction spanning a runbook.

Consequences:

- Multi-step runbooks commit verb-by-verb. Stage 9a "everything commits or nothing" does not hold.
- Workspace lint L2 `forbid multiple_dispatch_sites` cannot be enabled yet.
- Combined with F2's 60% pool-leak ratio: even the per-verb scope that *does* exist is bypassed by
  most writes.

### F7. `PendingStateAdvance` pipeline is empty (v0.3 §10.7, §17 item 6)

Severity: Medium — stage 9a atomic state advance cannot fire.

Status: MEMORY.md records: "no current plugin op produces non-empty values" for
`PendingStateAdvance`. Verified — no op returns populated `state_transitions`, `constellation_marks`,
`writes_since_push_delta`, or `catalogue_effects`. Phase 5c-migrate is tagged complete but the
state-advance outbound channel is silent.

### F8. TOCTOU recheck not implemented (v0.3 §10.5, Phase 5d)

Severity: Medium — Phase 5d explicitly NOT STARTED; surface for visibility only.

Infrastructure present:

- `SemOsContextEnvelope::toctou_recheck()` exists and is unit-tested.

Missing:

- No runtime recheck of `StateGateHash` inside a per-verb transaction after row-locking.
- No recheck between successive statements of a multi-statement DSL program.

### F9. Row-versioning missing on entity tables (v0.3 §10.5, R13)

Severity: Medium — F8 prerequisite.

Status: `version bigint` exists only on `shared_fact_versions` and `dsl_instance_versions`
(`migrations/master-schema.sql:9964, 15322, 21045`). Entity tables used by the gate surface (cbu,
entity, deal, kyc_case, …) lack it. `StateGateHash` cannot be computed against locked state until
these columns are backfilled.

### F10. Dual-schema YAML not started (v0.3 §10.4, Phase 6)

Severity: Low — Phase 6 is intentionally last; surface for visibility only.

Status: `rust/config/verbs/**/*.yaml` has zero `runtime_schema:` or `catalogue_schema:` keys. CRUD
dissolution cannot begin.

### F11. Pattern B external-I/O inside `execute_json` bodies (v0.3 §11.2, A1)

Severity: Medium — existing ledger; surface the link.

Status: Direct-import grep in `rust/src/domain_ops/` finds `reqwest`/`tonic::`/`Command` imports in
`gleif_ops.rs`, `request_ops.rs`, `source_loader_ops.rs`, `bpmn_lite_ops.rs` (indirect paths via
service helpers also exist). `docs/todo/pattern-b-a1-remediation-ledger.md` scopes Phase 5f.
Remediation Phase 2 should explicitly defer these files, not rewrite them.

### F12. Session serialization must be preserved (v0.3 §8.7)

Severity: Low — informational.

Current `ReplOrchestratorV2::process()` holds `sessions.write().await` for the whole turn. §8.7 calls
this intentional. Phase 2 of the remediation (scope-aware nested execution) must not accidentally
relax this lock.

### F13. Outbox foundation confirmed wired (positive verification)

Not a remediation item; record so future reviews do not re-audit.

- `OutboxDrainerImpl` at `rust/src/outbox/drainer.rs:59-72`.
- Instantiated at `rust/crates/ob-poc-web/src/main.rs:748`.
- Registers `MaintenanceSpawnConsumer` and `NarrateConsumer`.
- `NarrateConsumer` runs as dual-write (outbox + inline `ReplResponseV2.narration`). UX cutover to
  WebSocket push is deferred outside this review's scope.
- Migration `rust/migrations/20260421_public_outbox.sql` present.

## Execution-path audit (added 2026-04-22 at user request)

The user's explicit directive: "trace execution paths — sniff out legacy dual path 'leaks'. I want
one path and all dead code ripped out — and no bypassing sem-os — that is the tollgate for all
agentic DSL discovery."

Three parallel sweeps (dual-path leaks, SemOS-bypass hunt, dead-code hunt) surfaced the findings
below. The two **P0** findings (F14 and F15) directly violate the user's rule and should lead any
remediation.

### F14. (BYPASS-1) ScenarioIndex Tier -2A bypasses CCIR allowed_verbs — **P0**

File: `rust/src/mcp/verb_search.rs:626-629`. The code has an explicit comment:
`// Scenarios are compound intents — bypass CCIR allowed_verbs check.`

Any verb routed via a scenario (Tier -2A, score 1.05) skips the `allowed_verbs` pre-constraint
filter. Flow: orchestrator → IntentPipeline → VerbSearchIntentMatcher → HybridVerbSearcher.search()
→ ScenarioIndex.resolve() → verb returned WITHOUT SemOS membership check. Scenarios have no pack
constraint, so the downstream "pack scoring" the comment gestures at never runs. CCIR filter at
line 1237 only applies to Tier 0+ fuzzy results; Tier -2 is already in `results` before that
filter.

Why this is a violation: scenarios are user-authored YAML routes. Without CCIR validation, the
scenario file becomes a side channel that elevates any verb into the selection set regardless of
whether the session has authorization for it.

Remediation: either (a) require scenarios to have a pack constraint enforced at resolve time, or
(b) run the allowed_verbs filter across *all* tier results before returning, not just fuzzy-tier
results.

### F15. (BYPASS-2) MacroIndex Tier -2B has the same defect — **P0**

File: `rust/src/mcp/verb_search.rs:685-702`. Same root cause as F14 — the comment at 626-629 applies
to Tier -2B too. When a macro matches, it's added to `results` before line 1237's allowed_verbs
filter. Severity equal to F14 because macros expand to *multiple* verbs; a single unchecked macro
can inject a runbook of ungoverned verbs.

Remediation: same as F14.

### F16. (BYPASS-3) `OBPOC_ALLOW_RAW_EXECUTE=true` enables a SemOS-less path — P1

Files: `rust/src/api/agent_routes.rs:1669` + `rust/src/policy/gate.rs:46-47`. When the flag is true
and the actor is `operator`/`admin`, `POST /api/session/:id/execute` accepts raw DSL text, parses
it, and runs it through the normal runtime WITHOUT re-invoking SemOS context resolution. Default is
`false` (safe), but the escape hatch is in the code.

Remediation: remove the flag and the code path. If an operator-only DSL probe is genuinely needed,
it should still route through `ReplOrchestratorV2.process()` so SemOS gates every statement.

### F17. (BYPASS-4) Runbook compiler skips SemReg filtering when envelope unavailable — P1

File: `rust/src/runbook/compiler.rs:327-340`. "Graceful degradation" when `sem_reg_allowed_verbs` is
`None`: the macro-expansion step skips the CCIR check entirely. When SemReg is down or
unconfigured, runbooks execute ungoverned.

Remediation: fail closed. If envelope is unavailable, compilation errors. "Graceful degradation" on
an authorization path is a bug.

### F18. (LEAK-1) `runbook-gate-vnext` branch is dead — P1 dead code

`rust/src/api/agent_service.rs:1144-1300` (~150 LOC) + `:116-124` imports (~8 LOC) live under
`#[cfg(not(feature = "runbook-gate-vnext"))]`. The `runbook-gate-vnext` feature is enabled in
production (confirmed via `ob-poc-web` Cargo feature set), so the `cfg(not)` branch is never
compiled into the prod binary. Rip it out.

Also remove the feature-gate `#[cfg(feature = "runbook-gate-vnext")]` lines themselves — once the
other branch is deleted, the gate is no longer meaningful.

Remediation: delete lines 116-124 and 1144-1300. Remove the feature definition from Cargo.toml.

### F19. (LEAK-2) `dsl_execute` vs `dsl_execute_submission` are two MCP tools for overlapping work — P1

Files: `rust/src/mcp/handlers/core.rs:85-86` (enum variants), `:683-755` (`dsl_execute` full impl),
`:1267-1350+` (`dsl_execute_submission` full impl). Both accept DSL source, validate via SemReg,
and execute. The docstring does not clarify the semantic difference. Line 1288 notes
`dsl_execute_submission` has "no session context" — but it can still write to the database.

Remediation: pick one. If there is a genuine "no-session" contract, name it explicitly and guard it
harder; if not, delete `dsl_execute_submission`.

### F20. (LEAK-3) Legacy HTTP routes mount a 410-Gone stub instead of not existing — P2

`rust/src/api/agent_routes.rs:92` mounts `/api/session/:id/chat` → `chat_session_legacy_blocked()`;
`:124` mounts `/api/session/:id/decision/reply` → `decision_reply_legacy_blocked()`. Both return
`StatusCode::GONE`. Better than a silent 404, but still keeps dead route matchers in the router.

Remediation: remove the route mounts entirely once any downstream client observably stops hitting
them (can verify via telemetry or a short grace period with logging).

### F21. Dead files / orphan modules

- `rust/src/error_enhanced.rs` — not referenced by any `mod` declaration in `lib.rs`. Orphan file,
  ~50 LOC. Investigate git blame to confirm no hidden caller, then delete.
- `vnext-repl` feature in `rust/Cargo.toml:210` — marked "DEPRECATED: REPL V2 always enabled". No
  conditional compilation depends on it. Delete the feature line.
- ~66 `#[allow(dead_code)]` annotations across `rust/src/`. Audit-and-delete pass: most will be
  delete-safe, a few will be legitimate "kept for WebSocket push" type markers (e.g. narrate.rs).

### F22. Stale references that should be cleaned when adjacent code changes

- `rust/src/api/mod.rs:75, 156` — comments still reference deleted `CbuSession`,
  `cbu_session_routes.rs`, `agent_dsl_routes.rs`, `agent_learning_routes.rs`. Comment-only; remove.
- `rust/src/agent/orchestrator.rs:11`, `rust/src/agent/sem_os_context_envelope.rs:3` —
  `SemRegVerbPolicy` mentioned in module docs even though it's been replaced by
  `SemOsContextEnvelope`. Update.
- `rust/src/domain_ops/sem_os_helpers.rs:1-7` — module docstring opens
  `//! Shared delegation helpers for Semantic Registry CustomOps.` Update to `SemOsVerbOp`
  terminology.
- `rust/crates/ob-poc-types/src/galaxy.rs` — `manco` domain name appears in test fixtures even
  though the domain was renamed to `ownership`. Rename the fixtures.

### F23. Positive audit results (record these so future reviews don't re-audit)

Path-tracing AUDITED CLEAN:

- Plugin dispatch: single path through `dispatch_plugin_via_sem_os_op` at
  `dsl_v2/executor.rs:1503`. No `CustomOperation` residue, no `inventory::collect`, no alternate
  registry, no hardcoded dispatch tables.
- Verb registration: single `build_registry()` + `extend_registry()` chain in
  `ob-poc-web::main:765-773`. No alternate registry construction.
- V1 REPL: gone. `ReplState` mentions refer to the V2 session-state enum in `session_v2.rs`, not
  the V1 type.
- Response adapter: `ChatResponse { ... }` only constructed in
  `rust/src/api/response_adapter.rs:26`.

SemOS-bypass AUDITED CLEAN:

- `test_with_verbs` in `sem_os_context_envelope.rs:343-362` is `#[cfg(test)]`-gated; no production
  callers.
- `SemOsContextEnvelope::from_resolution()` is the only production constructor.
- No runtime reads of `config/verbs/*.yaml` (all access via registry/embeddings).
- No LLM-selected verbs. Sage emits `OutcomeIntent` (domain/action/subject); verb selection always
  runs through the allowed_verbs-constrained searcher.
- TOCTOU recheck infrastructure exists (`toctou_recheck()`); just not used on every
  multi-statement DSL boundary yet (F8).

Dead-code claims AUDITED CLEAN (genuinely gone, not residue):

- `ob-poc-ui` / `esper_*` / `ob-poc-graph` / `viewport` / `dsl-runtime-macros` /
  `ob-execution-types` crates — directories absent.
- ECIR / NounIndex / noun_index.rs / noun_index.yaml — files absent.
- `execute_json_via_legacy()` — function deleted (one comment reference remains in
  `verb_executor_adapter.rs:415` explaining the post-deletion mutation-sync shim).
- `CustomOperation`, `CustomOpFactory`, `CustomOperationRegistry`, `dispatch_plugin_via_execute_json`,
  `#[register_custom_op]`, `verify_plugin_verb_coverage` — all zero active-code hits (comments
  only; addressed in F22).

## Remediation plan

### Phase 1. Re-establish a single canonical registry wiring path

Goal:

- every production `DslExecutor` and `RealDslExecutor` gets the canonical registry automatically

Recommended changes:

- introduce a shared constructor or factory for the canonical SemOS registry
  - candidate location:
    - `ob-poc-web` bootstrap helper if app-only
    - or a library-level helper if other app code needs it
- introduce a higher-level executor constructor that wires:
  - `DslExecutor::new(pool)`
  - `.with_services(...)`
  - `.with_sem_os_ops(...)`
- stop directly calling raw `DslExecutor::new(...)` in production code

Suggested implementation options:

1. Preferred:
   add an explicit app-level executor factory and route all live call sites through it
2. Acceptable:
   make `ObPocVerbExecutor::with_sem_os_ops(...)` also rebuild or replace the inner `DslExecutor` with the same registry
3. Stronger:
   make missing registry impossible in production by changing constructors, not by convention

Files/functions to update first:

- `rust/src/mcp/handlers/core.rs`
- `rust/src/api/agent_service.rs`
- `rust/src/api/agent_state.rs`
- `rust/src/dsl_v2/sheet_executor.rs`
- `rust/src/repl/executor_bridge.rs`
- `rust/src/sem_os_runtime/verb_executor_adapter.rs`

Specific follow-up check:

- audit every result from:
  - `rg -n "DslExecutor::new\\(|RealDslExecutor::new\\(" rust -g '!target'`

Acceptance criteria:

- no production path constructs a plugin-capable executor without the canonical registry
- missing-registry failure becomes unreachable in normal host execution

### Phase 2. Remove transaction escape hatches from migrated ops

Goal:

- stop doing scoped work through fresh `PgPool` connections
- keep plugin execution inside the ambient `TransactionScope`

Recommended changes:

- migrate Pattern B ops to use `scope.executor()` where direct SQL is involved
- where nested DSL execution is needed, add a scope-aware execution entrypoint instead of constructing a pool-based `DslExecutor`
- where service APIs still require `&PgPool`, classify them explicitly as transitional and prioritize conversion

Recommended first tranche:

- `rust/src/domain_ops/template_ops.rs`
- `rust/src/domain_ops/onboarding.rs`
- `rust/src/domain_ops/gleif_ops.rs`

Reason:

- these combine scoped execution with nested DSL recursion and are the highest-risk correctness sites

Needed design work:

- define a scope-aware nested execution path for DSL recursion
- likely candidates:
  - a `DslExecutor::execute_plan_in_scope(...)` or equivalent
  - or move nested DSL invocation back through the `VerbExecutionPort`/Sequencer-owned boundary

Broader follow-up sweep:

- classify all `scope.pool().clone()` usages into:
  - acceptable transitional service bridges
  - true transaction leaks requiring rewrite now

Acceptance criteria:

- no newly migrated Pattern B op performs write-path execution via `scope.pool()` where transactional atomicity is expected
- explicit tests prove rollback of nested plugin flows

### Phase 3. Restore fail-fast coverage enforcement at startup

Goal:

- detect YAML/registry drift before serving traffic

Recommended changes:

- restore a startup assertion after the canonical registry is built
- keep the test in `rust/src/domain_ops/mod.rs`
- optionally reuse the same implementation from both runtime startup and tests

Potential implementation shape:

- small pure function:
  - inputs:
    - runtime registry
    - canonical SemOS manifest
  - output:
    - missing plugin FQNs
- startup:
  - panic or return startup error if missing is non-empty
- tests:
  - assert the same function returns empty

Files to update:

- `rust/crates/ob-poc-web/src/main.rs`
- `rust/src/domain_ops/mod.rs`
- possibly a new shared helper module if needed

Acceptance criteria:

- host fails fast when a YAML plugin verb has no SemOs registration
- startup and test coverage use the same logic

### Phase 4. Clean remaining warning and hygiene debt

Goal:

- return the tree to a warning-free baseline and remove stale architecture residue

Recommended changes:

- fix invalid cfg in:
  - `rust/crates/dsl-runtime/src/document_bundles/service.rs`
- remove or rename unused parameter in:
  - `rust/crates/dsl-runtime/src/crud_executor.rs`
- delete or integrate unused helpers in:
  - `rust/src/domain_ops/sem_os_helpers.rs`
- update stale comments mentioning `CustomOp`/`CustomOperation` where no longer true

Acceptance criteria:

- `cargo check` emits no warnings from the reviewed slice
- no stale “CustomOp” terminology remains in active code comments for the migrated paths

## Proposed execution order

0. **Fix the P0 SemOS bypasses (F14 + F15) FIRST.** Both are a single-file change in
   `rust/src/mcp/verb_search.rs` — lift the `allowed_verbs` filter to apply across all tiers,
   including Tier -2A scenarios and Tier -2B macros. This is the user's hard rule ("no bypassing
   sem-os") and blocks shipping even if F1/F2/F3 fixes are in flight.
1. Fix canonical registry injection (F1 confirmed-broken sites only; defer dead code to a cleanup
   pass).
2. Restore fail-fast coverage enforcement at startup (F3).
3. Add scope-aware nested-DSL entrypoint as prerequisite — no op can be scope-migrated until this
   primitive exists (F2 spec gap).
4. Convert highest-risk transaction-leaking Pattern B ops (trading_profile, booking_principal_ops,
   request_ops, source_loader_ops, gleif_ops) using the new entrypoint.
5. Sweep remaining `scope.pool()` A-class + D-class sites (F2 quantified).
6. Close the other SemOS bypasses: F16 (remove `OBPOC_ALLOW_RAW_EXECUTE` escape hatch) and F17
   (fail closed in the runbook compiler when envelope is unavailable).
7. Rip out dead code: F18 dead feature-gate branch, F21 orphan file + deprecated feature flag,
   F22 stale references.
8. Consolidate overlapping tools: F19 (`dsl_execute` vs `dsl_execute_submission`), F20 (410-Gone
   stubs).
9. Clean confirmed hygiene items only — invalid cfg, unused param, stale docstring (F4 retracted
   sub-bullet dropped).
10. **Defer (not in this remediation pass):** F5 envelope wiring, F6 single-dispatch invariant,
    F7 PendingStateAdvance pipeline, F8 TOCTOU recheck, F9 row-versioning backfill, F10 dual-schema
    YAML, F11 Pattern B A1 ledger. These are Phase 5c-wire / 5d / 5e-finish / 5f / 6 work and belong
    in dedicated slices with their own peer reviews.

Rationale:

- Step 0 is the user's hard rule. It must land before any other remediation because it gates whether
  the rest of the pipeline can even be trusted.
- Steps 1 + 2 reduce immediate runtime breakage risk (F1 hard-fail on plugin verbs; F3 drift
  detection).
- Step 3 is structural — it's the missing primitive Pattern B ops need in order for step 4 to
  produce correct code rather than just relocated code.
- F5-F11 outrank F3 in severity but belong to named downstream phases with their own gates. Pulling
  them into this remediation conflates scope and invites a regression in F1/F2 fixes.

## Suggested peer-review questions

- Should `DslExecutor::new(...)` remain public if a registry-free instance is invalid for production plugin execution?
- Should nested DSL execution be allowed inside Pattern B ops at all, or should it be routed back through a scope-aware composition-plane path?
- Do we want `TransactionScope::pool()` to remain available on write-capable paths, given the explicit escape semantics?
- Should startup registry drift checks be mandatory in all binaries, not just `ob-poc-web`?
- Should scenarios (Tier -2A) and macros (Tier -2B) be required to declare a pack constraint before they can be indexed at all, so the "bypass CCIR" comment at `verb_search.rs:626-629` becomes impossible to write?
- Is `OBPOC_ALLOW_RAW_EXECUTE` still a needed operator-only escape hatch, or can F16 simply delete the flag and the route?
- Should the runbook compiler's "graceful degradation when SemReg is down" in F17 be replaced with a hard fail? What use case, if any, justifies ungoverned execution?

## v0.3 Definition-of-Done alignment

Snapshot vs. three-plane v0.3 §17 checklist (at `8303e1d9`):

| # | DoD item | Status |
|---|----------|--------|
| 1 | No `sem_os_*` crate contains `execute_*` functions outside metadata loading | PASS (grep clean) |
| 2 | `dsl-runtime` owns `VerbExecutionPort`, `CustomOperation`, etc. | PARTIAL — `CustomOperation` deleted by slice #80; replacement contract is `SemOsVerbOp` in `sem_os_postgres::ops`, not `dsl-runtime`. Interpret as: spec wording is stale post-slice-#80; the structural invariant holds differently. |
| 3 | Envelope + state-advance + outbox types in `ob-poc-types` | PASS for types; FAIL for wiring (F5) |
| 4 | `ob-poc/domain_ops` contains only app-coupled ops behind service traits | PARTIAL — Pattern B 119 ops still in `rust/src/domain_ops/` per CLAUDE.md |
| 5 | `ob-poc::sequencer` is a named module with nine-stage contract | PARTIAL — module exists, stages 1/2a/2b/3/4/5/7/8/9a/9b wired as shadow extractions; stage 6 unwired (F5) |
| 6 | Transaction scope: Sequencer opens at stage 8, commits at 9a | FAIL (F6) |
| 7 | Outbox + drainer + at-least-once semantics | PASS (F13) |
| 8 | `StateGateHash` recheck inside txn after row lock | FAIL (F8, F9) |
| 9 | `execute_json_via_legacy()` deleted | PASS |
| 10 | Every runtime-executable verb catalogued; startup validation enforces | FAIL (F3 — coverage check gone) |
| 11 | Dissolved-CRUD dual-schema YAML + round-trip | FAIL (F10) |
| 12 | Determinism harness green | PASS (crate exists at `rust/crates/determinism-harness`) |
| 13 | `cargo test --workspace` green; intent hit rate ±1% | Not audited by this review |
| 14 | Workspace lints enforce one-way deps + single dispatch site | FAIL (F6 blocks L2) |
| 15 | Transaction-abort test + drainer-kill replay + TOCTOU recheck test | PARTIAL — drainer-kill replay test green (MEMORY.md); other two blocked on F6/F8 |
| 16 | CLAUDE.md updated | PASS (commit `8303e1d9`) |
| 17 | `sem_os_lift_out_plan.md` archived | Not audited |
| 18 | No `sem_os_*` ↔ `dsl-runtime` dep-graph crossings | PASS |

## Verification plan after remediation

Required:

- `cargo check`
- `cargo fmt`
- `cargo clippy -- -D warnings`

Strongly recommended:

- targeted tests for plugin dispatch through:
  - MCP path
  - agent path
  - recursive template path
  - onboarding auto-complete path
  - GLEIF import path
- transactional rollback tests proving that nested plugin flows do not leave committed writes behind after a later failure

## Appendix: grep commands used during review

```bash
rg -n "DslExecutor::new\\(|RealDslExecutor::new\\(" rust -g '!target'
rg -n "with_sem_os_ops\\(" rust -g '!target'
rg -n "scope\\.pool\\(\\)\\.clone\\(" rust/src/domain_ops rust/crates/sem_os_postgres/src/ops -g '!target'
rg -n "build_registry\\(\\)" rust -g '!target'
```
