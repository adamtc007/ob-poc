# EOP-SESSION-CONTROLPLANE-G4-IMPL-001 — Implementation session log

### Implements: G4 — Path B/C per-step admission (`EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.5` §3, per `EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001`, RATIFIED)
### Date: 2026-07-13
### Branch: `codex/phase-1-5-governance-closure` (not merged, not committed)

---

## 1. Verification against the plan/research/design docs' own claims, before any code

1. **`execute_verb_in_scope`'s location.** The plan's G4 text and R:§B2 cite `dsl_v2/executor.rs:1914`. Re-verified this session: `1914` at session start (before my ExecutionContext field additions shifted it to `1937`, then to `1962` after the double-admission-guard block was inserted above it). Small, mechanical line drift — same function, same signature, same body — noted, not a blocker.
2. **B1/B2 caller enumeration.** Re-ran `rg` for `execute_plan\(` / `execute_plan_atomic_in_scope\(` and `admit_plan\(`. Confirmed both `execute_plan` (`executor.rs:2270`) and `execute_plan_atomic_in_scope` (`executor.rs:2600`) call `execute_verb_in_scope` per-step — one seam, as the research/design doc claimed. `admit_plan` (T9.3 plan-level pre-flight) has exactly the same 4 call sites the design doc's own `check_admission`/`admit_plan` doc comments already name: `src/repl/executor_bridge.rs` (`RealDslExecutor`), `src/dsl_v2/sheet_executor.rs`, `src/dsl_v2/batch_executor.rs`, `src/api/agent_routes.rs` (raw-execute route), `src/mcp/handlers/core.rs` (MCP `dsl_execute`) — **5 call sites, not 4** (the design doc's own text undercounted by one in its "four ingress points" framing at §2.3, which itself already flagged that count as approximate/tag-level, not a location-level claim). No caller found that the research doc's §B1 didn't already enumerate — no new findings to report there.
3. **`EnforcedVerbs`'s current shape at session start.** Confirmed still the flat `HashSet<String>` from `control_plane_envelope_store.rs:27-45` the research found — G3's `HashMap<String, PathScope>` reshape had NOT landed from any prior session. Built it here, exactly per the design doc's §3(b) spec (rejected-alternative reasoning included in the code comment).
4. **`ObPocVerbExecutor`'s Branch-3 fallthrough.** Confirmed unchanged from the design doc's citation: `execute_verb_in_open_scope`'s Branch 3 (`verb_executor_adapter.rs`, around line 360 pre-edit) calls `self.executor.execute_verb_in_scope(&vc, &mut exec_ctx, scope)` — the exact seam G4 instruments. `to_dsl_context` (line ~1057 pre-edit) is confirmed the only conversion function on this path, matching the design doc's finding. G1 item 2's prior-session edits (seal→consume wiring) touched `admit_in_scope`'s body (the G1-floor + audit-event blocks) but not its signature shape in a way that conflicted with G3's planned parameter addition — no interaction/overlap found beyond both sessions adding parameters to the same function, which composed cleanly (verified by the final signature: `verb_fqn, envelope_handle, path, conn`).

No STOP-conditions were hit in verification — all four checks confirmed the plan/design's own assumptions hold, modulo the two small, already-flagged-in-the-design-doc line-number/count drifts above.

---

## 2. Item 1 — the admission call at the dsl_v2 seam

`rust/src/dsl_v2/executor.rs`, `DslExecutor::execute_verb_in_scope` (now at line 1962's admission block, function starting line 1937): a new block runs immediately after the `ENTER` trace and before the `requires_states` precondition check / any dispatch branch. It:

1. Reads `ctx.execution_path` (new field, see item 3).
2. Skips (see item 2) or calls `agent::control_plane_envelope_store::EnforcedVerbs::from_env()` (now fallible — malformed env var fails loudly, matching G3 §3(c)) and `check_admission_in_scope(scope.executor(), &enforced, &fqn, ctx.execution_path, ctx.envelope_handle)` — the **same primitives** Path A/D use (`check_admission_in_scope`, `pub(crate)` inside `ob-poc`; `dsl_v2::executor::DslExecutor` is a crate-sibling module in the same `ob-poc` crate). Confirmed same-crate-only visibility change, zero new crate edges, per R:§B3/B4's own claim.
3. On `Admitted` with real pins, runs `ob_poc_boundary::toctou_recheck::verify_pins_in_scope` — **a deliberate small extension beyond G3's literal table** (see "Deviations" below).
4. Rejects (`bail!`) on `RejectedNoEnvelope`/`RejectedConsumeFailed`/pin-drift, naming the path in the error.

`ctx.envelope_handle` is always `None` at every real production Path B/C ingress point today (T9.3's established posture, unchanged) — this block is real, live code, not a no-op, but it only bites once a verb is both listed in `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` for that path AND a real caller starts minting envelopes for B/C (future work, not this tranche's scope, matching G1/G4's shared "shadow-first" posture for every path that hasn't graduated).

---

## 3. Item 2 — the double-admission guard

Implemented exactly per G3 §3(e), which the session brief said to treat as ratified and build, not redesign:

- `ExecutionContext` (`dsl_v2/executor.rs`) gained `already_admitted_for: Option<ob_poc_types::ExecutionPath>` (default `None`).
- The seam's skip condition: `if ctx.already_admitted_for != Some(ctx.execution_path) { /* run the check */ }` — a **value match**, not a bare boolean.
- `ObPocVerbExecutor::execute_verb_in_open_scope` (Branch 3) now takes a `path: ExecutionPath` parameter (threaded from its sole caller, `execute_verb_admitting_envelope`, which received `path` from item 3's signature change) and, when converting to the dsl_v2 context, sets **both** `exec_ctx.execution_path = path` **and** `exec_ctx.already_admitted_for = Some(path)` — the fallthrough carries the SAME tag as the outer admission, never a distinct "fallthrough" tag, per the design doc's own reasoning.

**Hard test, real (not a comment):** `dsl_v2::executor::tests::g4_seam_admission_tests::seam_skip_is_keyed_on_exact_path_match_not_a_bare_flag` — directly exercises the seam with (a) matching tags (`already_admitted_for == Some(execution_path)`) and asserts the seam's own admission re-check did NOT fire (no "enforce-mode gated" error despite the verb being enforced with no envelope in `ctx`), and (b) mismatched tags (`already_admitted_for = Some(WorkflowDispatched)` while checking under `RunbookSequencer`) and asserts the seam DID re-check and reject. A second, end-to-end test — `branch_3_fallthrough_consumes_envelope_exactly_once` — drives the real production call chain (`ObPocVerbExecutor::execute_verb_admitting_envelope` → Branch 3 → the seam) and confirms the envelope's consume state stays single-valued (query the row directly, then confirm a follow-up admission attempt still sees it consumable after rollback) — "neither double-consume nor reject a properly admitted dispatch," proven both at the unit level and the real call chain.

---

## 4. Item 3 — the enforcement dimension

Built from scratch per G3's ratified mechanical spec (confirmed via verification step 3 that it hadn't landed yet):

- **New crate module** `rust/crates/ob-poc-types/src/execution_path.rs` — `ExecutionPath` enum (`RunbookSequencer` / `DslDirect` / `WorkflowDispatched` / `BusFederated`), `Copy + Hash + Eq + Serialize + Deserialize`, `as_letter()`/`from_letter()` for the env-var grammar, `ALL` const array. Re-exported from `ob-poc-types::lib.rs`. Zero new crate edges (confirmed: `dsl-runtime`, `ob-poc`, `ob-poc-web`, `ob-poc-control-plane` all already depend on `ob-poc-types`).
- **`EnforcedVerbs` reshaped**: `HashMap<String, PathScope>` where `PathScope::All | PathScope::Only(HashSet<ExecutionPath>)`. `from_env() -> Result<Self, EnforcedVerbsParseError>` — fails the WHOLE config on any malformed entry (unrecognised letter, empty tag after `:`), per G3 §3(c)'s fail-closed rule; every caller of `from_env()` propagates the error and rejects the dispatch it would otherwise have to guess about (never falls back to "nothing enforced").
- **Env-var grammar**: `entry (',' entry)*`, `entry = verb-fqn (':' path-tag ('|' path-tag)*)?`, `path-tag = 'A'|'B'|'C'|'D'` — implemented exactly per §3(c), with the worked example from §4 of the design doc (`cbu.confirm:A`) covered by a dedicated unit test.
- **Signature propagation**, matching the design doc's §3(d) table exactly: `check_admission`/`check_admission_in_scope` gained `path: ExecutionPath`; `admit_plan`/`admit_plan_checked` gained `path: ExecutionPath` (a genuine new function parameter, not context-derived, matching the doc's own correction that the plan-level loop needs it as a real param); `VerbExecutionPort::execute_verb_admitting_envelope` (trait, `dsl-runtime::port.rs`) gained `path: ExecutionPath`, default impl ignores it (same degrades-safely precedent as `envelope_handle`); `ObPocVerbExecutor::admit_in_scope` and its `execute_verb_admitting_envelope` override both gained `path`; `step_executor_bridge.rs:603` passes `ExecutionPath::RunbookSequencer`; `bus_runtime.rs:170` passes `ExecutionPath::BusFederated`.
- **`ExecutionContext` gained `execution_path: ExecutionPath`** (default `DslDirect`, matching the umbrella-default reasoning), threaded through `child_for_iteration` and the one hand-built initializer (`domain_ops/template_ops.rs`'s batch-template parent context) so a template-expansion child dispatch inherits its parent's path rather than resetting to the default.
- **`RealDslExecutor`** (`repl/executor_bridge.rs`) gained `execution_path: ExecutionPath` (default `DslDirect`) + `.with_execution_path(path)` builder; `build_executor_and_ctx()` sets `ctx.execution_path` from it; `admit_plan()` passes `self.execution_path` into the shared function.
- **`main.rs` construction-site tagging**, per §3(d)'s table: `inner` (`main.rs:1339`, wrapped exclusively by `WorkflowDispatcher`) → `WorkflowDispatched`; `worker_executor` (JobWorker durable-resume, `main.rs:1374`), `legacy_executor` (`main.rs:1481`), and the bare no-BPMN `executor_v2` fallback (`main.rs:1652`) → all `.with_execution_path(DslDirect)`, matching the design doc's explicit umbrella-treatment finding and its §6 open-question-1 caveat (JobWorker's tag is a "least-surprising default pending T9.2 OQ4's park/resume re-entry trace," not a final answer — carried forward verbatim as a code comment, not silently resolved).
- **Test-double implementors checked** (`grep -n "impl.*VerbExecutionPort for"`): `step_executor_bridge.rs`'s `UnusedPort`, `dsl-runtime::port.rs`'s `MockVerbExecutor`, `sem_os_harness::HarnessMockExecutor` — **none override `execute_verb_admitting_envelope`**, confirmed unchanged from the design doc's own finding; no code changes needed there.

---

## 5. Item 4 — atomicity tests on the dsl_v2 seam

New module `dsl_v2::executor::tests::g4_seam_admission_tests` (`#[cfg(feature = "database")]`, live-DB, `#[ignore]`-gated, same convention as every other live-DB test in this file/crate):

1. `seam_rolls_back_the_consume_when_dispatch_fails` — seals a real envelope, dispatches `cbu.confirm` with no args (admission succeeds, dispatch fails on `requires_states`), asserts the failure is NOT an admission-rejection message, rolls the caller's scope back, then re-runs `check_admission_in_scope` against the same handle in a fresh scope and asserts `Admitted` — the envelope is still consumable, proving the whole scope (including the consume) rolled back together. Direct analogue of `execute_verb_admitting_envelope_rolls_back_the_consume_when_dispatch_fails` (Path A/D), now proven from the dsl_v2 seam itself.
2. `seam_rejects_on_pin_drift_and_leaves_envelope_reconsumable` — seals an envelope pinning a stale `row_version` on a real `cbus` row, asserts the seam's own pin-verification block rejects with "pinned entity state drifted," rolls back, and re-confirms the envelope is still `Admitted`-consumable. Direct analogue of the Path A/D pin-drift test.
3. `seam_skip_is_keyed_on_exact_path_match_not_a_bare_flag` — item 2's hard test (above).
4. `branch_3_fallthrough_consumes_envelope_exactly_once` — item 2's end-to-end proof (above).

**Deviation from G3's literal table, disclosed:** the ratified design doc's §2.3/§3(d) establishes that every real production Path B/C ingress point passes `envelope_handle: None` to `admit_plan`/`check_admission` (no envelope infrastructure wired for B/C yet) — meaning the *production* seam call in `execute_verb_in_scope` also always sees `None` today. G4 item 4 explicitly asks for atomicity tests proving "rollback-of-consume on dispatch failure" and "pin-drift rejection leaving the envelope reconsumable" — properties that require a real, consumable envelope to exist at all. These two requirements are in tension: the ratified design's `envelope_handle: None`-always posture for B/C makes item 4's own named test scenarios structurally unexercisable through the literal production call shape. Resolution taken: added `ExecutionContext.envelope_handle: Option<EnvelopeHandle>` (default `None`, threaded into the seam's admission call in place of a hardcoded `None`) — every production `RealDslExecutor`/`admit_plan` caller still never sets it (verified: no construction site in `main.rs` or any of the 5 `admit_plan` callers sets this field), so **zero behavior change for any real caller today** — but it makes item 4's own named tests real, live-DB-proven tests against the actual seam rather than either (a) fabricated tests against a different, lower-level function, or (b) silently skipping item 4's own named scenarios. Flagged here explicitly per the session's own STOP-condition discipline: this is a small, additive, non-production-affecting field, not a redesign of G3's ratified admission shape for B/C — B/C's `admit_plan` chain (the T9.3 pre-flight) is untouched and still always passes `None`.

Also added, as a consequence of wiring `ctx.envelope_handle` for real: the seam now runs `ob_poc_boundary::toctou_recheck::verify_pins_in_scope` on `Admitted` decisions with real pins — mirroring `ObPocVerbExecutor::execute_verb_admitting_envelope`'s T10.2 behavior exactly. Not explicitly named in G3's §3(d) table (which only says "admission call reads `ctx.execution_path`"), but required for item 4's pin-drift test to be meaningful at this seam rather than merely at the lower-level `check_admission_in_scope` (already covered by pre-existing tests). Same reasoning as above: additive, `None`-by-default, no change to any real caller's behavior today.

Live-DB command output, this session:

```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    -- --ignored --nocapture g4_seam_admission_tests
running 4 tests
test dsl_v2::executor::tests::g4_seam_admission_tests::seam_rejects_on_pin_drift_and_leaves_envelope_reconsumable ... ok
test dsl_v2::executor::tests::g4_seam_admission_tests::seam_rolls_back_the_consume_when_dispatch_fails ... ok
test dsl_v2::executor::tests::g4_seam_admission_tests::branch_3_fallthrough_consumes_envelope_exactly_once ... ok
test dsl_v2::executor::tests::g4_seam_admission_tests::seam_skip_is_keyed_on_exact_path_match_not_a_bare_flag ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 2369 filtered out; finished in 0.16s
```

Regression check — every pre-existing Path A/D `t4_1` test still passes (proving item 5's defence-in-depth doesn't conflict with the new seam-level check):

```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    t4_1_envelope_admission_tests -- --ignored --nocapture
running 7 tests
... (all 7) ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 2366 filtered out; finished in 0.10s
```

And the pre-existing pool-level `control_plane_envelope_store::tests` (17 tests, including `admit_plan_checked`/`check_admission`/`check_admission_in_scope` signature-compat regressions) + the new `enforced_verbs_tests` (11 pure unit tests, §5's backward-compat plan):

```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    -- --ignored --nocapture control_plane_envelope_store::tests
running 17 tests ... test result: ok. 17 passed; 0 failed

$ cargo test -p ob-poc --lib --features database enforced_verbs_tests
running 11 tests ... test result: ok. 11 passed; 0 failed
```

---

## 6. Item 5 — T9.3's plan-level pre-flight retained

`admit_plan`/`admit_plan_checked` are unchanged in role — still called first, before `execute_plan`/`execute_plan_atomic_in_scope` ever runs, by all 5 ingress points. Only their signature changed (gained `path`). No caller removed this call. Verified no double-reject/double-count conflict: in production, when a verb IS enforced on a B/C path with no envelope, the OUTER `admit_plan` call rejects first (whole plan never begins executing), so the seam's own (redundant, defence-in-depth) check inside `execute_verb_in_scope` is never actually reached for that dispatch in the real flow — the two checks are provably non-conflicting because the outer one always fires first and is strictly stricter (same `EnforcedVerbs`, same fqn, same path, no envelope in either place). The dsl_v2 seam's own check only becomes independently load-bearing for callers that bypass `admit_plan` and call `execute_plan`/`execute_verb_in_scope` directly (e.g., this session's own tests) — exactly the atomicity-proof role G4 item 1 exists for.

---

## 7. Exit gate — `check-invariants.sh e2`, real run

**Finding, disclosed before the fix:** the FIRST run of `scripts/check-invariants.sh e2` after landing items 1-5 showed Path B/C structurally FAILING — 2/4 paths. Root cause: the script's `gate_e2`'s Path B/C checks were written before G3/G4's design was ratified, pointing at `rust/src/api/agent_routes.rs` and `rust/src/bpmn_integration/dispatcher.rs`/`rust/src/domain_ops/bpmn_controller_ops.rs`, grepping those files for a literal `execute_verb_admitting_envelope` call — a shape that never matched the ratified architecture (B/C's real admission call lives inside the shared `dsl_v2::executor::execute_verb_in_scope` seam, not in those ingress files, and doesn't call a function literally named `execute_verb_admitting_envelope`). This is a genuine drift between an earlier draft of the checker and the since-ratified G3 design doc (§2.3), not a defect in the implementation. Per the session brief's own permission ("if it's a small, mechanical drift... adapt and note the deviation, don't treat every drift as a blocker"), I updated `scripts/check-invariants.sh`'s `gate_e2` to add a dedicated `_e2_check_path`-sibling, `_e2_check_seam`, for Path B/C: it confirms `check_admission_in_scope` is called inside `execute_verb_in_scope`, before the function's first dispatch-branch anchor (`let runtime_verb = runtime_registry()...`), printing file:line for reviewer verification — same rigor as the original per-file check, adapted to the seam's actual shape (no per-ingress-file "bare execute_verb( bypass" check is meaningful for B/C anymore, since every ingress function funnels through `execute_plan`/`execute_verb_in_scope`, confirmed by tracing `execute_verb` → `execute_verb_inner` → `execute_verb_in_scope`, i.e. even the "bare" `self.execute_verb(...)` call inside `execute_plan_best_effort` is NOT a bypass — it still reaches the same seam). Also added a new dynamic-evidence block running `g4_seam_admission_tests` (item 4/2's tests), alongside the pre-existing Path D `t4_1` block.

Real command output, this session, after the script fix:

```
$ DATABASE_URL=postgresql:///data_designer bash scripts/check-invariants.sh e2
== E2: execution only via envelope admission (structural + dynamic) ==
  -- structural (per RR-2 path: admitting-call locus + bare-call exclusivity) --
    Path A (runbook/step_executor_bridge.rs): admitting entry point present — rust/src/runbook/step_executor_bridge.rs:603: .execute_verb_admitting_envelope(
    Path A (runbook/step_executor_bridge.rs): no bare execute_verb() call sites — admitting call is the sole route
    Path B (dsl_v2 seam, umbrella: agent_routes.rs raw-execute + batch/sheet executors + MCP dsl_execute + no-BPMN executor_v2 fallback): admitting entry point present — rust/src/dsl_v2/executor.rs:1986 (execute_verb_in_scope, before dispatch branches at rust/src/dsl_v2/executor.rs:2037)
    Path B ...: single shared seam — every execute_plan/execute_plan_atomic_in_scope step reaches this same admission call, no bare-bypass check applicable to this shape
    Path C (dsl_v2 seam, WorkflowDispatcher-wrapped RealDslExecutor instance): admitting entry point present — rust/src/dsl_v2/executor.rs:1986 (execute_verb_in_scope, before dispatch branches at rust/src/dsl_v2/executor.rs:2037)
    Path C ...: single shared seam — every execute_plan/execute_plan_atomic_in_scope step reaches this same admission call, no bare-bypass check applicable to this shape
    Path D (ob-poc-web/src/bus_runtime.rs): admitting entry point present — rust/crates/ob-poc-web/src/bus_runtime.rs:170: .execute_verb_admitting_envelope(
    Path D (ob-poc-web/src/bus_runtime.rs): no bare execute_verb() call sites — admitting call is the sole route
  -- dynamic (live DB, Path D's shared admission mechanism) --
running 7 tests ... test result: ok. 7 passed; 0 failed
  -- dynamic (live DB, Path B/C dsl_v2 seam atomicity + double-admission guard) --
running 4 tests ... test result: ok. 4 passed; 0 failed
  Structural failures: 0 / 4 paths
  E2: structural half HOLDS; dynamic evidence shows Path D NotEnforced by default -> DOES NOT HOLD
```

**Exit gate confirmed met**: "admitting call present at the seam, zero bare `execute_verb(` bypasses" — 0/4 structural failures. "atomicity tests green" — 4/4. Overall `E2: DOES NOT HOLD` is the CORRECT/expected outcome (enforce-mode is still pending — `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` is empty by production default; the script's own dynamic section is designed to show this, matching G1's analogous "structural HOLDS, dynamic shows NotEnforced" result for Path A/D).

---

## 8. STOP-conditions hit and what was done

None reached the session's actual STOP bar (redesigning a ratified decision, or a plan assumption proving structurally false in a way that blocks all further work). Two items surfaced that needed a documented, bounded, non-blocking adaptation rather than a full stop:

1. **check-invariants.sh's Path B/C shape** (§7 above) — mechanical drift in a checker script predating the ratified design; fixed in place, documented.
2. **Item 4's tension with the `envelope_handle: None`-always posture** (§5 above) — resolved with a small, additive, `None`-by-default `ExecutionContext` field; documented as a disclosed deviation, not a silent scope expansion.

Both are reported here per the "small, mechanical drift → adapt and note" instruction, not treated as blockers.

---

## 9. Full file-changed list

Production code:
- `rust/crates/ob-poc-types/src/execution_path.rs` (new) — `ExecutionPath` enum
- `rust/crates/ob-poc-types/src/lib.rs` — module registration + re-export
- `rust/crates/dsl-runtime/src/port.rs` — `execute_verb_admitting_envelope` gains `path: ExecutionPath`
- `rust/crates/ob-poc-web/src/bus_runtime.rs` — passes `ExecutionPath::BusFederated`
- `rust/crates/ob-poc-web/src/main.rs` — 4 `RealDslExecutor` construction sites tagged
- `rust/src/agent/control_plane_envelope_store.rs` — `EnforcedVerbs`/`PathScope`/`EnforcedVerbsParseError`, `check_admission`/`check_admission_in_scope`/`admit_plan`/`admit_plan_checked` gain `path`; test modules rewritten/extended
- `rust/src/api/agent_routes.rs`, `rust/src/dsl_v2/batch_executor.rs`, `rust/src/dsl_v2/sheet_executor.rs`, `rust/src/mcp/handlers/core.rs` — `admit_plan(...)` calls pass `ExecutionPath::DslDirect`
- `rust/src/domain_ops/template_ops.rs` — child `ExecutionContext` inherits `execution_path`/`already_admitted_for`/`envelope_handle`
- `rust/src/dsl_v2/executor.rs` — `ExecutionContext` gains 3 fields; `execute_verb_in_scope` gains the admission block (items 1-3); new `g4_seam_admission_tests` module (item 4)
- `rust/src/repl/executor_bridge.rs` — `RealDslExecutor` gains `execution_path` field + builder; `admit_plan`/`build_executor_and_ctx` thread it
- `rust/src/runbook/step_executor_bridge.rs` — passes `ExecutionPath::RunbookSequencer`
- `rust/src/sem_os_runtime/verb_executor_adapter.rs` — `admit_in_scope`/`execute_verb_in_open_scope`/`execute_verb_admitting_envelope` gain `path`; Branch 3 sets `already_admitted_for`; test helper gains a path-parameterised sibling; 3 existing test call sites updated for the new trait signature

Tooling:
- `scripts/check-invariants.sh` — `gate_e2`'s Path B/C checks replaced with the seam-aware `_e2_check_seam`; new dynamic block for `g4_seam_admission_tests`

Docs (this file):
- `docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-G4-IMPL-001.md` (new)

No migrations. No schema changes.

---

## 10. `invariants-expected.toml` recommendations (not applied)

Per the established pattern, recommending only — not flipping:

- **`[e2]`**: detail comment should be updated to record: "Structural: 4/4 RR-2 paths now call an admitting/admission-checked entry point as their sole route (Path A/D via `execute_verb_admitting_envelope`, Path B/C via the shared `dsl_v2::executor::execute_verb_in_scope` seam — G4, `EOP-SESSION-CONTROLPLANE-G4-IMPL-001`). Dynamic: all four paths' admission mechanisms proven live (Path A/D via `t4_1_envelope_admission_tests`, Path B/C via `g4_seam_admission_tests`, including the double-admission-guard hard test). `status` should STAY `fail` — `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` remains empty by production default on every path; no verb is enforced yet. This is G4's own exit gate's explicit expectation ("enforce-mode still pending, correctly fail")."
- **`[e4]` Row 5** (admission shape supporting per-step pins): recommend noting "G4 confirms the per-step admission shape (including pin-verification via `verify_pins_in_scope`) now exists uniformly across all 4 RR-2 paths, not just A/D — closure still needs a real B/C envelope-minting populator (none exists; `ctx.envelope_handle` is test-only-populated today), tracked as the same open item G4's own plan text names (\"admission shape now supports per-step pins — closure still needs G6b's populator\")."

---

## 11. Build/test/clippy summary

- `cargo build --workspace --features database` — clean.
- `cargo build --workspace --all-targets --features database` — clean.
- `cargo build --workspace` (default features) — clean.
- `cargo build -p ob-poc-types --no-default-features` / `cargo build -p dsl-runtime --no-default-features` — clean (both touched crates' feature-gating discipline holds).
- `cargo clippy -p ob-poc -p ob-poc-types -p ob-poc-web -p dsl-runtime --all-targets --features database -- -D warnings` — 2 findings introduced by this session's first draft (`clone_on_copy` on the new `Copy` `EnvelopeHandle` field, 2 call sites) were found and fixed in-session. Remaining clippy findings on this branch (`control_plane_metrics.rs` `int_plus_one` x3, `main.rs` `items_after_test_module`, `verb_executor_adapter.rs` pre-existing `await_holding_lock` at a DIFFERENT line than any G4 edit, `traceability/{phase2,replay}.rs` dead-code x3, `kyc_verb_coverage.rs`/`kyc_m3_remediation.rs` `doc_lazy_continuation`/`expect_fun_call`) are **confirmed pre-existing** — reproduced identically via `git stash` against this same branch before any G4 change, in files this session never touched. Zero new clippy findings remain from this session's diff.
- `cargo test -p ob-poc --lib --features database` (full lib suite, DB-backed) — 2169 passed, 0 failed, 204 ignored.
- `cargo test -p ob-poc --lib -- test_plugin_verb_coverage` — 1 passed.
- `cargo test -p dsl-runtime --lib` — 185 passed, 0 failed, 6 ignored.
- Live-DB (`DATABASE_URL=postgresql:///data_designer`, real local Postgres with production-shaped data — 2200 `cbus` rows, 391 pre-existing `control_plane_envelopes` rows):
  - `g4_seam_admission_tests` (new, item 2/4): 4/4 passed.
  - `t4_1_envelope_admission_tests` (pre-existing, Path A/D regression): 7/7 passed.
  - `control_plane_envelope_store::tests` (pre-existing, pool-level regression): 17/17 passed.
  - `enforced_verbs_tests` (new, §5 backward-compat plan): 11/11 passed.
  - `dsl_v2::executor::tests::{c1_requires_states_precondition, c5_discoverable_but_not_executable_dod}` (pre-existing, unrelated-module regression check): 2/2 passed.
  - `scripts/check-invariants.sh e2`: structural 0/4 failures (exit gate met); dynamic confirms enforce-mode correctly still off by default.

No commit, no merge, no deploy, no push were performed.
