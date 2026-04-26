# Three-Plane Correction Slice & Test Plan (2026-04-22)

**Source review:** `docs/todo/three-plane-implementation-peer-review-2026-04-22.md`
**Scope:** P0 bypass fixes + F1/F2/F3 structural corrections + F16-F22 bypass / dead-code / hygiene.
**Explicitly deferred (see `three-plane-v0.3-wiring-followon-2026-04-22.md`):** F5 envelope wiring, F6
single-dispatch invariant, F7 PendingStateAdvance pipeline, F8 TOCTOU, F9 row-versioning, F10
dual-schema YAML. Those are Phase 5c-wire / 5d / 6 work.

**Cadence:** medium slices (500-800 LOC each, themed). F2 Pattern B sweep runs file-by-file R-sweep
style because the files are heterogeneous.
**Test policy (from Q3):** unit tests + 353-case utterance-harness regression on every slice that
touches the intent/execution pipeline.

---

## Slice dependency graph

```
  0.1 Design primitive ─┐
                        ├─▶ 6.1 Implement primitive ─▶ 7.5 nested-executor migration
  0.2 Deployment audit ─┤
                        └─▶ 4.1 Delete dead feature branch

  1.1 (P0 bypass fix) ──▶ 9 Full regression gate

  2.1 (registry factory) ─▶ 2.2 (coverage check) ─▶ 3.1 ─▶ 3.2 ─▶ 4.x ─▶ 5.x ─▶ 6.1 ─▶ 7.x ─▶ 8.1 ─▶ 9
```

Slices 0.1 and 0.2 are pre-work with no production changes; they unblock 4.1, 6.1, and 7.x.
Slice 1.1 is the P0 — lands first, ships independently if everything else slips.

---

## Phase 0 — Pre-work (no production changes)

### Slice 0.1 — Design scope-aware nested-DSL primitive

**Deliverable:** ADR at `ai-thoughts/NNN-scope-aware-nested-dsl.md` + trait/fn stub (compile-only, no
callers).
**Scope:** this is an *interface design* slice, not a code slice. The purpose is to make the Phase 7
Pattern B migration produce *correct* code instead of *relocated* code.

Must answer:

- How does a nested tree receive `&mut dyn TransactionScope` without copying/cloning?
- Does the nested execution run inside the same Postgres transaction (risk: long-running txns) or
  a Postgres `SAVEPOINT` (preferred — preserves outer rollback, scopes inner failure)?
- How does the nested tree reach the canonical `SemOsVerbOpRegistry`?
- Recursion-depth limit: what's the policy and where is it enforced?

Output: Rust signature for `DslExecutor::execute_plan_in_scope` (or a free fn), a helper for
`SAVEPOINT` acquire/release/rollback, and a test-vector file describing the intended rollback
propagation.

**Files:** new `ai-thoughts/NNN-scope-aware-nested-dsl.md`, stub at
`rust/src/dsl_v2/nested_execution.rs`.
**LOC:** ~200 of docs + ~80 of unreachable stub code.
**Test plan:** none — no behaviour changes.
**Acceptance:** ADR reviewed, stub compiles, trait shape signed off before Slice 6.1 begins.
**Rollback:** delete the two files.

### Slice 0.2 — Deployment audit for `runbook-gate-vnext`

**Deliverable:** short audit doc listing every deployment surface and whether the feature is enabled.

**Files to inspect:**

- `rust/Cargo.toml` workspace features
- `rust/crates/ob-poc-web/Cargo.toml` default-features + feature chain
- All `ci/**` workflows
- `Dockerfile`, `docker-compose.yml`, any deploy scripts
- `rust/xtask/src/deploy.rs` and any `cargo x deploy`-adjacent surface

**Output:** `docs/todo/runbook-gate-vnext-deployment-audit-2026-04-22.md` — table of deployment × feature-on/off.

**Acceptance criteria for Slice 4.1 to proceed:** every production and staging deployment has
`runbook-gate-vnext` ON. If any deployment has it OFF, 4.1 is blocked until that deployment is
migrated or the finding is revisited.

**LOC:** docs only.
**Risk:** low. Worst case: find a deployment with the feature off → block 4.1, open a ticket.

---

## Phase 1 — P0: close the SemOS bypasses (F14 + F15)

### Slice 1.1 — Lift `allowed_verbs` filter to all tiers in `HybridVerbSearcher`

**User rule this enforces:** "no bypassing sem-os — that is the tollgate for all agentic DSL
discovery."

**Files:**

- `rust/src/mcp/verb_search.rs` — remove the `bypass CCIR allowed_verbs check` comment at 626-629;
  restructure so the filter runs over *all* tier results (Tier -2A scenarios, Tier -2B macros, Tier
  -0.5 constellation index, Tier 0+ fuzzy) before returning.
- `rust/src/mcp/scenario_index.rs` — if scenarios need metadata to describe their allowed-verb set
  (e.g. pack constraints), add the field to `ScenarioRoute`.
- `rust/src/mcp/macro_index.rs` — same for macros.

**LOC:** 200-400 (restructure the tier-result aggregation + add new tests + fixture updates).

**Test plan (Q3 = both):**

1. **Unit tests** against `HybridVerbSearcher::search()` with mocked envelopes:
   - Scenario match + verb not in `allowed_verbs` → scenario filtered (or rejected with
     `PruneReason::AbacDenied` / `TaxonomyNoOverlap`).
   - Macro match + one verb in expansion not in `allowed_verbs` → entire macro filtered (atomic
     policy: a macro is a compound, so partial authorization = deny whole).
   - Scenario match + all verbs in `allowed_verbs` → scenario retained with original score.
   - `allowed_verbs = None` (SemReg unavailable) → fail closed; search returns error, not fallback
     results. This lines up with Slice 3.2.
2. **New utterance-harness pack:** `rust/tests/fixtures/utterance/p0_bypass_regression.toml`.
   ~20 cases: deliberately mis-authorized scenarios and macros that MUST NOT match, plus legit
   scenarios/macros that MUST match under normal authorization. Fails any build if bypass regresses.
3. **Full 353-case utterance harness regression** — target: ±1% hit rate vs pre-slice baseline.
4. **All 48 agentic scenarios** in `rust/scenarios/suites/` — green.

**Risk:** hit rate may drop if legitimate scenarios relied on the bypass to span workspaces
incorrectly. Mitigation: the regression fixture distinguishes "authorized-and-should-match" from
"unauthorized-and-must-not-match"; a drop in the former is a real regression requiring scenario
re-authoring, not a rollback.

**Acceptance:**

- `rg 'bypass CCIR' rust/src/` returns zero hits.
- P0 regression fixture: 100% pass.
- Full harness: within ±1% of baseline hit rate.
- A manually-constructed unauthorized scenario that previously matched now returns empty result.

**Rollback:** single-file revert of `verb_search.rs`. Other files have additive changes (new field
on route structs) that are backwards-compatible.

**Ships standalone.** If every subsequent slice slips, 1.1 goes to prod on its own.

---

## Phase 2 — Registry wiring + startup coverage (F1 + F3)

### Slice 2.1 — Canonical executor factory + route all `DslExecutor::new` sites

**Files:**

- `rust/src/api/executor_factory.rs` (new) — `ExecutorFactory::build()` / `build_real()` returns an
  executor with services + `SemOsVerbOpRegistry` pre-installed. Holds the canonical registry `Arc`
  built at app startup.
- `rust/src/mcp/handlers/core.rs` — lines 1001 and 1387 rewritten to use the factory.
- `rust/src/api/agent_service.rs` — line 1398 rewritten. Dead lines 1153 (behind `cfg(not)`) left
  alone; handled in Slice 4.1.
- `rust/src/dsl_v2/sheet_executor.rs` — line 566 rewritten.
- `rust/src/dsl_v2/executor.rs` — `DslExecutor::new` visibility tightened to `pub(crate)` (or wrap
  in a `#[cfg(test)]` boundary). `RealDslExecutor::new` same.
- `rust/crates/ob-poc-web/src/main.rs` — build the factory once at startup, inject it into
  `AppState`.

**LOC:** 400-600.

**Test plan:**

1. Unit test that `ExecutorFactory::build` always produces an executor where
   `sem_os_ops.is_some()`.
2. Integration test for plugin dispatch via:
   - MCP `dsl_execute` path (was F1 broken site).
   - Agent `runbook-gate-vnext` path (was F1 broken site).
   - Sheet executor path (was F1 broken site).
3. Compile-level check: `rg 'DslExecutor::new\(' rust/src/` returns matches *only* inside
   `rust/src/dsl_v2/executor.rs` or behind `#[cfg(test)]`.
4. Full 353-case utterance harness regression.

**Risk:** sheet executor has its own flow — may need a factory variant for sheet contexts (resolvers,
template expander). Surface if discovered.

**Acceptance:**

- All 4 confirmed-broken paths from F1 now dispatch plugin verbs without the hard-fail error.
- `DslExecutor::new` not publicly callable from `src/api/` or `src/mcp/`.
- Startup builds exactly one canonical registry; factory injects it everywhere.

**Rollback:** revert commit; `DslExecutor::new` becomes `pub` again.

### Slice 2.2 — Startup fail-fast coverage enforcement (F3)

**Files:**

- `rust/src/domain_ops/coverage_check.rs` (new) — shared fn
  `check_plugin_coverage(registry: &SemOsVerbOpRegistry, manifest: &[&str]) -> Result<(), Vec<String>>`
  returning the set of manifest FQNs with no registry entry.
- `rust/crates/ob-poc-web/src/main.rs:765-773` — call `check_plugin_coverage` after registry build;
  panic with a structured error listing missing FQNs if non-empty.
- `rust/src/domain_ops/mod.rs:602` — `test_plugin_verb_coverage` calls the same shared fn.

**LOC:** ~80.

**Test plan:**

1. Unit test for `check_plugin_coverage` with a mock registry + manifest — both empty-result and
   missing-FQN cases.
2. Integration test: add a bogus YAML plugin verb without a registered op, run `cargo run -p
   ob-poc-web`, assert panic.
3. Existing `test_plugin_verb_coverage` still passes unchanged.

**Acceptance:**

- Startup fails loud when any YAML plugin verb has no `SemOsVerbOp` registration.
- Startup log still shows `registered_ops = N` on success.
- Same check used by prod startup and tests — no drift possible.

---

## Phase 3 — Other bypasses + fail-closed semantics (F16 + F17)

### Slice 3.1 — Remove `OBPOC_ALLOW_RAW_EXECUTE` escape hatch

**Files:**

- `rust/src/policy/gate.rs:46-47` — delete `can_execute_raw_dsl` + the flag read.
- `rust/src/api/agent_routes.rs:1669-1677` — delete the raw-execute branch; either remove the route
  or refactor it to route through `ReplOrchestratorV2::process()` if we want to keep a
  session-scoped operator probe.
- CLAUDE.md — update env-var list.
- Deploy scripts / docs — remove any references.

**LOC:** ~150 deletion.

**Test plan:**

1. Integration test: `POST /api/session/:id/execute` with raw DSL returns 404 (route gone) or
   routes through orchestrator and is gated by SemOS envelope.
2. `rg OBPOC_ALLOW_RAW_EXECUTE rust/` returns only in CHANGELOG-style archival.

**Acceptance:**

- No code path executes DSL without first resolving a `SemOsContextEnvelope`.
- The env var no longer branches any control flow.

**Risk:** breaks local debugging flows that rely on the raw endpoint. Mitigation: document the
replacement (e.g. `cargo x dsl-cli` local harness).

### Slice 3.2 — Fail closed in runbook compiler when envelope unavailable

**Files:** `rust/src/runbook/compiler.rs:327-340`.

**Change:** replace the `if let Some(allowed) = ...` graceful-degradation block with `let allowed =
sem_reg_allowed_verbs.ok_or(CompilationError::EnvelopeUnavailable)?;`.

**LOC:** ~30.

**Test plan:**

1. Unit test: compiler returns `EnvelopeUnavailable` when passed `None`.
2. Integration test: SemReg offline → runbook compilation errors; session surfaces the error to the
   user; no verb executes.
3. Regression: all 48 agentic scenarios still green (scenarios always have an envelope in test
   setup).

**Acceptance:**

- No code path where `sem_reg_allowed_verbs = None` produces a compiled plan.
- Error shape is structured and surfaces to the REPL.

**Risk:** SemReg downtime blocks all sessions. This is the correct behaviour — ungoverned execution
is worse than blocked execution. Add monitoring/alerting on `EnvelopeUnavailable` error rate.

---

## Phase 4 — Dead code ripout (F18 + F21 + F22)

### Slice 4.1 — Delete `runbook-gate-vnext` dead branch

**Depends on:** Slice 0.2 audit green (every deployment has feature ON).

**Files:**

- `rust/Cargo.toml` — delete `runbook-gate-vnext` feature definition.
- `rust/src/api/agent_service.rs` — delete lines 116-124 (unused imports), delete lines 1144-1300
  (legacy `execute_resolved_dsl`), delete every `#[cfg(feature = "runbook-gate-vnext")]` and
  `#[cfg(not(feature = "runbook-gate-vnext"))]` annotation.
- Any other file with the cfg annotation (grep to enumerate).

**LOC:** ~200 deletion.

**Test plan:**

1. `cargo check --all-features` green, `cargo check` green, `cargo check --no-default-features`
   green.
2. Full 353-case utterance harness regression.
3. `rg 'runbook-gate-vnext' rust/` returns zero hits.

**Acceptance:**

- Feature flag gone.
- The remaining `execute_via_runbook_gate` path is the sole DSL execution path.

**Risk:** if Slice 0.2 audit missed a deployment with feature OFF, that deployment breaks. Hence
the explicit dependency.

### Slice 4.2 — Orphan files + deprecated feature flag + dead-code allow sweep

**Files:**

- `rust/src/error_enhanced.rs` — delete after git-blame check confirms no hidden caller
  (doctest, LSP tool, proc-macro).
- `rust/Cargo.toml:210` — delete `vnext-repl` feature.
- `rust/src/` — audit 66 `#[allow(dead_code)]` annotations. For each: delete the annotation and
  let the compiler error tell you if the function is truly dead; delete-safe ones go; legitimate
  ones (e.g. "kept for WebSocket push" markers) gain a one-line justification comment.

**LOC:** ~200 deletion + ~30 annotation review.

**Test plan:** `cargo check --all-features` green; full unit test suite green.

**Risk:** `error_enhanced.rs` may have a caller that git-blame missed. Mitigation: run
`cargo check` after deletion — compile errors reveal any caller.

### Slice 4.3 — Stale references in comments and fixtures

**Files:**

- `rust/src/domain_ops/sem_os_helpers.rs:1-7` — module docstring update from "CustomOps" to
  `SemOsVerbOp`.
- `rust/src/agent/orchestrator.rs:11`, `rust/src/agent/sem_os_context_envelope.rs:3` — replace
  `SemRegVerbPolicy` references with `SemOsContextEnvelope`.
- `rust/src/api/mod.rs:75, 156` — delete comments referencing deleted routes/files.
- `rust/crates/ob-poc-types/src/galaxy.rs` — rename `manco` fixtures to `ownership`.

**LOC:** ~50.

**Test plan:** none (docs-only); `cargo test` stays green.

**Acceptance:** `rg 'CustomOp|SemRegVerbPolicy|CbuSession|\bmanco\b' rust/src/ rust/crates/ob-poc-types/src/`
returns only in truly-justified contexts (e.g. historical release notes inside code comments).

---

## Phase 5 — Consolidate overlapping tools (F19 + F20)

### Slice 5.1 — Decide `dsl_execute` vs `dsl_execute_submission`

**Pre-decision needed (peer-review question):** is there a genuine "no-session" contract?
Default assumption: no — `dsl_execute_submission` is a leak.

**Files:**

- `rust/src/mcp/handlers/core.rs:85-86` — remove the enum variant.
- `rust/src/mcp/handlers/core.rs:1267-1350` — delete the handler.
- MCP tool manifest / doc — remove the tool.

**LOC:** ~150 deletion.

**Test plan:**

1. Integration test via MCP client: `dsl_execute_submission` tool is not listed; calling it returns
   tool-not-found.
2. Every legitimate use of `dsl_execute_submission` migrated to `dsl_execute` with explicit session.

**Risk:** any caller relying on the no-session contract breaks. Mitigation: log MCP tool usage for
one release cycle before deletion; confirm zero calls.

### Slice 5.2 — Remove 410-Gone route stubs

**Files:**

- `rust/src/api/agent_routes.rs:92` — delete `/api/session/:id/chat` mount.
- `rust/src/api/agent_routes.rs:124` — delete `/api/session/:id/decision/reply` mount.
- `rust/src/api/agent_routes.rs:135-151` — delete `chat_session_legacy_blocked` +
  `decision_reply_legacy_blocked` fns.

**LOC:** ~50 deletion.

**Test plan:**

1. Integration test: `POST /api/session/:id/chat` returns 404 (was 410).
2. Observability check: no traffic to those endpoints in the last 30 days of logs (precondition
   for deletion — if traffic exists, leave the stubs for another grace period).

**Risk:** any downstream client still using those endpoints breaks with 404 instead of a structured
410. Pre-check logs.

---

## Phase 6 — Implement the scope-aware primitive (depends on 0.1)

### Slice 6.1 — Implement `execute_plan_in_scope`

**Files:**

- `rust/src/dsl_v2/executor.rs` — new fn `execute_plan_in_scope(&self, plan: CompiledRunbook, scope:
  &mut dyn TransactionScope) -> Result<ExecutionResult>`. Uses `scope.transaction()` or a Postgres
  SAVEPOINT (decided in Slice 0.1). Dispatches plugin verbs through the canonical
  `SemOsVerbOpRegistry` in the nested tree.
- `rust/crates/dsl-runtime/src/tx.rs` — add `acquire_savepoint` / `release_savepoint` /
  `rollback_to_savepoint` helpers.

**LOC:** 400-600.

**Test plan:**

1. Unit test: nested execution with inner failure rolls back the SAVEPOINT without affecting outer
   txn state.
2. Unit test: nested execution with inner success commits to outer txn; outer rollback subsequently
   removes the nested writes.
3. Plugin dispatch test: a plugin verb invoked inside a nested tree resolves through the canonical
   registry.
4. Recursion depth test: ≥ 2 levels of nesting work; a runaway recursion is bounded.
5. Property test: any atomic-equivalent result achievable with pool-based nested execution today is
   achievable with scope-based nested execution.

**Acceptance:**

- API exists, tests green.
- Ready for Phase 7.5 consumers.

**Risk:** SAVEPOINT semantics are Postgres-specific — document the Postgres-only contract
explicitly. If future backends are in scope, add a trait method that each backend implements.

---

## Phase 7 — F2 Pattern B migration (R-sweep, medium slices)

Migrates the 92 A-class write-path leaks + 4 D-class nested executors to use `scope.executor()`
(for direct SQL) or `execute_plan_in_scope` (for nested DSL). C-class service bridges handled in a
dedicated sub-slice. B-class (read-only) migrated opportunistically or reclassified.

### Slice 7.1 — `trading_profile.rs` (34 A-writes)

**Files:** `rust/src/domain_ops/trading_profile.rs`.
**Change:** every `scope.pool().clone()` + write rewritten to use `scope.executor()`. Helper fns
that took `&PgPool` refactored to take `&mut PgConnection` or parameterized on a generic Executor.
**LOC:** 500-700 delta.

**Test plan:**

1. Existing unit tests stay green.
2. NEW integration tests (one per verb family): for each migrated write, test that outer-txn
   rollback erases the write from the DB.
3. Full 353-case utterance harness regression.
4. Run the 48 agentic scenarios.

**Risk:** trading_profile is the heaviest file in the review (3295 LOC total); the high count of
internal component sub-ops means the diff will be broad. Review carefully.
**Acceptance:** `rg 'scope\.pool\(' rust/src/domain_ops/trading_profile.rs` = 0; rollback tests
green.

### Slice 7.2 — `booking_principal_ops.rs` (32 A-writes)

Same pattern as 7.1. ~500 LOC. Same test plan.

### Slice 7.3 — `request_ops.rs` (11 A-writes)

Same pattern. ~300 LOC. Same test plan.

### Slice 7.4 — `source_loader_ops.rs` (7 A) + `gleif_ops.rs` (8 A + 1 D)

Combined medium slice. ~500 LOC.

**Note on gleif_ops.rs:** has 1 nested `DslExecutor::new` — migrated to `execute_plan_in_scope`
from Slice 6.1. Also has external HTTP calls (Pattern B A1 concern, F11) — those stay for now,
flagged for Phase 5f in the v0.3 wiring follow-on plan.

### Slice 7.5 — `onboarding.rs` + `template_ops.rs` (3 D-nested-executors)

Uses the new `execute_plan_in_scope` primitive from Slice 6.1. ~300 LOC.

**Critical correctness:** after this slice, a template that contains a failing inner verb rolls back
the entire outer plan — this is a behaviour change from pre-slice (where the nested pool executor
would commit before the outer rolled back).

**Test plan:** specific rollback-semantics test — inner verb fails, outer verb succeeds, expect *no*
writes committed (whereas pre-slice would have committed the inner work).

### Slice 7.6 — B-class (42 read-only escapes) + C-class (20 service bridges) sweep

**B-class:** migrate to `scope.executor()` for consistency. Same-transaction reads are safer and
cheaper than pool reads. ~200 LOC.

**C-class:** classify per file — are these bridges genuinely transitional, or should the service
API itself be refactored to take `&mut PgConnection`? For v0.3 alignment, the latter; for this
plan's scope, document each one and defer the service refactor to its own slice.

---

## Phase 8 — Hygiene (F4)

### Slice 8.1 — Invalid cfg + unused param + stale docstring

**Files:**

- `rust/crates/dsl-runtime/src/document_bundles/service.rs:255` — fix invalid
  `#[cfg(all(test, feature = "database"))]` (the `database` feature doesn't exist in
  `dsl-runtime`).
- `rust/crates/dsl-runtime/src/crud_executor.rs:1358` — remove unused `crud: &VerbCrudMapping`
  parameter.
- (Module docstring already handled in Slice 4.3.)

**LOC:** ~30.

**Test plan:** `cargo check` emits no warnings from these locations.

---

## Phase 9 — Verification gate

After every phase, and final gate after Slice 8.1:

```bash
cd rust/
cargo x pre-commit                                           # fmt + clippy + unit
cargo x check --db                                           # DB integration
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings  # if verb YAML changed
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test intent_hit_rate -- --ignored --nocapture
cargo x harness run --all                                    # 48 scenarios
```

Plus:

- Chrome DevTools MCP smoke: tollgate flow via `tests/fixtures/ui_smoke_test.toml`.
- Final full 353-case utterance harness compared against pre-Phase-1 baseline. Target: first-attempt
  hit rate within ±1%, two-attempt within ±0.5%.
- P0 regression fixture from Slice 1.1: 100%.

---

## Blast-radius ordering (why this order, not another)

- **Slice 1.1 ships first alone** — P0 user-rule violation; all other work irrelevant if bypass
  stands.
- **Slices 2.1 + 2.2 next** — F1 is a runtime hard-fail; F3 prevents silent regression of F1.
- **Slices 3.1 + 3.2** — close remaining SemOS bypasses; cheap.
- **Slices 4.1-4.3** — dead code is free risk; ripping it simplifies every subsequent slice.
- **Slices 5.1 + 5.2** — consolidate overlap so Phase 7 doesn't migrate code that's about to be
  deleted.
- **Slice 6.1** — primitive must exist before 7.5 can be correct.
- **Slices 7.1-7.6** — highest-write-count file first; limits rollback blast radius if a slice
  fails review.
- **Slice 8.1** — hygiene last; structural fixes first so warning cleanup doesn't get redone.

---

## Estimated timeline

Assuming single-author cadence, medium slices averaging 1-2 days review + merge:

- Phase 0: 1-2 days (mostly design)
- Phase 1: 2-3 days (P0 + harness regression signal)
- Phase 2: 3-4 days
- Phase 3: 1-2 days
- Phase 4: 2-3 days (including deployment coordination for 4.1)
- Phase 5: 2-3 days (including the 30-day log-observation window for 5.2 if needed)
- Phase 6: 2-3 days (primitive + tests)
- Phase 7: 8-12 days (7.1-7.6 across 6 slices)
- Phase 8: 0.5 days
- Phase 9: 0.5 days

**Total: ~3-4 weeks** if nothing slips. Phase 7 is the long pole.

---

## Rollback strategy

- Slices 1.1, 3.1, 3.2, 8.1 — single-file reverts.
- Slices 2.1, 2.2 — revert + re-run harness baseline.
- Slice 4.1 — if any deployment runs without the feature, revert immediately. (Rare because of 0.2
  gate.)
- Slices 4.2, 4.3 — revert + `cargo check`.
- Slices 5.1, 5.2 — revert + re-add route stubs.
- Slice 6.1 — revert; Phase 7 consumers revert to pool-based nested execution (regression but
  recoverable).
- Slices 7.1-7.6 — one-file revert per slice; regression tests prove atomicity is preserved.
