# EOP-SESSION-CONTROLPLANE-G1-HUMANGATE-RESEAL-TEST-001 — Implementation session log

### Task: build the owed live-DB test for `EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001` §10, test-plan assertion 4 —
### "HumanGate re-seal, not reuse" — flagged as an open gap by three prior sessions without being closed.
### Date: 2026-07-14
### Branch: `codex/phase-1-5-governance-closure` (working tree left uncommitted — no commit made this session, per instruction)

---

## 0. Outcome, up front

**Outcome (a): a real live-DB test was built, is GREEN, and RED→GREEN
proven against the actual `reseal_for_human_gate_resume` production
code.** This closes the specific gap three prior sessions named without
attempting: `01539938` (the original G1 item 2 landing, "an end-to-end
test for this path is still owed, not written this session"),
`EOP-SESSION-CONTROLPLANE-G1-ITEMS-3-4-IMPL-001` (2026-07-13,
independently re-verified item 3's existing tests but repeated the
flag rather than attempting it, reasoning it "requires a real
`ReplSessionV2` with a wired `sem_os_client` and a full park→approve
flow through `Sequencer::handle_human_gate_approval`... a materially
larger integration-test build-out"), and the G6b/G6c session
(2026-07-14, same day, repeated the same flag a third time without
attempting it).

The prior sessions' own reasoning ("a materially larger
integration-test build-out") was **checked, not inherited** — see §1.
It turned out to be correct in spirit (this genuinely needed the real
`ReplOrchestratorV2`, a real park→approve flow, and real fixtures for
essentially every proof-bearing gate `evaluate_with_report` walks) but
tractable in practice: every fixture needed already had a reusable
in-repo pattern (a `SemOsClient` stub, an in-memory `GatePipeline`, a
synthetic single-verb `DomainMetadata`, a hand-built `VerbConfigIndex`
entry) from adjacent test modules in this same program
(`step_executor_bridge.rs::g1_item2_path_a_tests`,
`control_plane_shadow.rs`'s own G2/G3/G4 fixture tests, and
`control_plane_audit.rs`'s `rederivation_matches_evaluate_with_report_
on_a_fully_admitted_context` — the single most load-bearing reference:
a known-working, field-by-field `ApprovedStp` `EvaluationContext`
fixture). No new test-harness *type* was invented; this session
assembled existing patterns around the one path (`Sequencer::
execute_runbook` → HumanGate park → `handle_human_gate_approval` →
`reseal_for_human_gate_resume` → dispatch) that had never been driven
end to end before.

**New test:** `sequencer::g1_humangate_reseal_tests::
human_gate_resume_reseals_fresh_envelope_not_the_stale_pre_park_one`
(`rust/src/sequencer.rs`, `#[cfg(all(test, feature = "database"))]`,
`#[ignore = "requires DATABASE_URL"]`, matching this program's
established live-DB test convention).

**Deviation from the task brief's literal framing, disclosed not
silently absorbed:** the brief described the target scenario as "park
an entry as `RequiresHumanGate` (so it seals nothing at park time)".
§1.2 below shows this specific shape is **not reachable** from the
real production call sites as currently wired — not a hedge, a derived
fact from reading `stp_classifier.rs`'s actual dependency structure.
The test instead drives the design doc's own literal §9/§10-item-4
scenario: a `HumanGate` entry gets **shadow-sealed `ApprovedStp`**
*before* park (§4's own framing — "a `HumanGate` entry gets
shadow-sealed before... parking it"), that pre-park envelope is aged
past its validity window while parked, and the assertions are exactly
the design doc's own (i) and (ii). This is the stronger and more
literal reading of the design doc; §1.2 records why the brief's
paraphrase doesn't match reachable code.

---

## 1. Verification before coding (claims re-derived against current HEAD, not trusted)

### 1.1 The two methods exist and are wired as the design doc describes

Confirmed by direct read, `rust/src/sequencer.rs`:

- `reseal_for_human_gate_resume` (`fn` at line 8166, `#[cfg(feature =
  "database")]`) — re-runs `evaluate_with_report` + synchronous
  `persist_sealed`, exactly `phase5_runtime_recheck`'s own shadow-seal
  logic minus the T11.F.2 floor checks (by design, per its own doc
  comment).
- `handle_human_gate_approval` (line 3479) calls it at line 3556,
  **awaited**, immediately before `execute_entry_via_gate` (line
  3563) — the same synchronous-before-dispatch shape
  `phase5_runtime_recheck` itself uses for `Sync`/`Durable` entries.
- `phase5_runtime_recheck` (line 7726) runs unconditionally, before
  the `match execution_mode` block (line 7158) that parks a
  `HumanGate` entry — confirmed by reading the loop directly
  (`execute_runbook_from`, line 6858 onward): `runtime_recheck =
  phase5_runtime_recheck(...)` happens at line 7112-7115, the
  `ExecutionMode::HumanGate => { ... park_entry(...); }` arm at
  7159-7187, strictly after.
- `lookup_sealed_handle` (`agent/control_plane_envelope_store.rs:419`)
  is `ORDER BY created_at DESC LIMIT 1` with **no `not_after` filter**
  — an expired-but-still-`status='sealed'` row is still *found*; its
  expiry is graded at consume time (`consume_core`,
  `control_plane_envelope_store.rs:471-513`: `Utc::now() > not_after`
  flips the row to `'expired'` and returns `ConsumeOutcome::Expired`).
  This is the exact mechanism the test's RED proof (§3) exercises.

### 1.2 The brief's literal "park as RequiresHumanGate" scenario is unreachable — verified, not assumed

Traced the only two real call sites that ever build a
`StpClassifierInput.has_unpinned_entities` value
(`sequencer.rs:7976`, `sequencer.rs:8246`, both calling
`control_plane_shadow::has_unpinned_entities`):

```rust
// agent/control_plane_shadow.rs:291-302
pub(crate) fn has_unpinned_entities(
    requests: &[(Uuid, String)],
    facts_map: Option<&HashMap<Uuid, EntityFactsRow>>,
) -> bool {
    if requests.is_empty() { return false; }
    let Some(facts_map) = facts_map else { return true; };
    requests.iter().any(|(id, _)| !facts_map.contains_key(id))
}
```

and the matching `build_entity_binding_input` (`control_plane_shadow.rs:431-452`):
an entity id **absent from `facts_map`** becomes an honest
`EntityFacts { exists: false, .. }` for G2 (entity binding), which
`entity_binding::decide` (`ob-poc-control-plane/src/entity_binding.rs:106-110`)
grades `NotFound` — and `EntityBinding` **is** one of the 8
`PROOF_BEARING_GATES` (`decision.rs:75-84`). `rejection_from_report`
runs before the `stp_classifier`/`ApprovedStp`/`RequiresHumanGate`
branch is even reached (`decision.rs:191-195`).

Concretely: `has_unpinned_entities` and `EntityBinding`'s `NotFound`
outcome are driven by **the same underlying predicate** (presence of
an id in the same `facts_map`, fetched once and shared by both G2 and
G8's caller) — a `facts_map`-absent entity is *simultaneously*
"unpinned" (would cap G8 at `HumanGated`) *and* "not found" (fails G2,
which short-circuits to `Rejected` before G8's `stp_classifier` branch
runs at all). A `facts_map`-*present* entity is never unpinned by this
function's own logic (`contains_key` is `true`). So, as wired today,
`ControlPlaneDecision::RequiresHumanGate` cannot be produced by an
entity-bound verb through either real call site — it is either
`Rejected` (entity missing/wrong-kind/etc, G2 fails first) or, when
every entity is genuinely bound, `has_unpinned_entities` is `false` and
the verb is `StpExecutable` if the other seven gates hold. (The other
route to `RequiresHumanGate`, `is_durable_verb`, is likewise foreclosed:
`build_stp_classifier_input`'s `durable_execution_explicitly_allowed`
is hardcoded `false` at this call site, so a durable verb classifies
`Rejected`, not `HumanGated`.)

This is recorded as an **incidental finding**, not this session's
primary deliverable — flagged in §8 as a possible follow-up (is `G8`
`HumanGated` dead code on Path A by construction, or is this a gap that
should be fixed?), not investigated further here. It does, however,
directly explain why the brief's literal scenario could not be built as
described, and why the test instead targets the design doc's own
literal §9/§10-item-4 wording (a genuinely-`ApprovedStp`-at-park entry
that goes stale while parked) — which the design doc's own author
evidently intended as the primary shape of assertion 4 in the first
place (see the quoted assertion text in §10 of the design doc: "the
entry's **pre-park envelope** is not the one consumed... its row is
still **sealed**/now expired" — this presupposes a pre-park envelope
exists, which only the `ApprovedStp`-at-park shape produces).

### 1.3 Every proof-bearing gate's minimal-success shape — checked against real code, not guessed

Before writing the test, read `evaluate_with_report`
(`ob-poc-control-plane/src/decision.rs:187-360`), all 8
`PROOF_BEARING_GATES`' `decide()` functions, and the two real
call-site builder functions (`agent/control_plane_shadow.rs`,
`phase5_runtime_recheck` and `reseal_for_human_gate_resume`
themselves) to derive the field-by-field fixture requirements in §2.
Cross-checked against `control_plane_audit.rs`'s
`rederivation_matches_evaluate_with_report_on_a_fully_admitted_context`
test fixture (lines 509-622) — a known-working, all-gates-`Success`
`EvaluationContext` literal already proven (by that test's own
assertion) to reach `ApprovedStp` through the real
`evaluate_with_report` function. This was the single most valuable
piece of prior art: it confirmed field-for-field which of the ~10
input structs need to be genuinely populated (not merely `Some(_)`
with placeholder-shaped data) for each gate to reach `Success`, most
notably that **`DagProof` is proof-bearing and a `None` value (the
routine, no-`GatePipeline`-wired, no-`transition_args` case) is a hard
`Failure`, not a vacuous pass** — meaning a synthetic `GatePipeline`
had to be constructed for this test (§2.4), not skipped.

---

## 2. What was built

`rust/src/sequencer.rs`, new module `g1_humangate_reseal_tests`
(`#[cfg(all(test, feature = "database"))]`), appended after the
existing `mod tests` block. Net diff: **+563 lines, 0 removed, one
file touched.**

### 2.1 `SingleVerbSemOsClient` — G1 (intent admission)

A `SemOsClient` stub granting exactly one candidate verb
(`access_decision: Allow`), same shape as the existing
`agent::harness::semos_stub::HarnessSemOsClient` but parameterised on
the verb under test instead of hardcoded to `"harness.no-op"`. All
other trait methods return `unsupported()`/empty, matching
`HarnessSemOsClient`'s own posture (not exercised by this path).

### 2.2 Real `cbus` row, reset to an unblocked state — G2 (entity binding)

Selects a real, existing `cbus` row (`LIMIT 1`, matching this file's
established `g1_item2_path_a_tests` convention) and resets
`deleted_at = NULL`, `disposition_status = 'active'`,
`operational_status = 'actively_trading'` — the exact three columns
`ob-poc-boundary::entity_facts::kind_mapping("cbu")`'s
`availability_sql` checks. (First attempt used
`'operationally_active'`, which is not a member of
`chk_cbu_operational_status`'s check constraint — caught by a real
constraint violation, fixed to `'actively_trading'`; see §5.) Original
`name` captured and restored at the end of the test.

### 2.3 Unconstrained test pack — G3 (pack resolution)

A minimal `PackManifest` YAML (`id`/`name`/`version`/`description`
only — every other field, including `allowed_verbs`, defaults per
`ob-poc-types::journey::pack_types::PackManifest`'s `#[serde(default)]`
annotations) registered in a real `PackRouter`, with
`session.runbook.pack_id` set to match. `check_pack_constraints`
short-circuits `Ok(())` when `EffectiveConstraints::is_constrained()`
is `false` (`constraint_gate.rs:38-40`), which holds here because
`allowed_verbs` is empty — no allowlist to violate, `cbu.rename`
resolves.

### 2.4 Synthetic in-memory `GatePipeline` — G4 (DAG proof)

The one genuinely load-bearing new fixture. Duplicated locally (not
cross-module-exposed) from the identical `FixedSlotState`/`FixedLookup`
pattern already used by `control_plane_shadow.rs`'s own
`g4_reaches_success_end_to_end_against_a_fixture_dag` test and
`step_executor_bridge.rs`'s `pre_dispatch_gate_check_equivalence_*`
tests: an in-memory `SlotStateProvider` (no Postgres query), a
`VerbTransitionLookup` that returns the same `TransitionArgs`
regardless of `verb_fqn` (decoupled from whatever `transition_args`
the real verb's production YAML does or doesn't declare — `cbu.rename`
declares none), and a tiny synthetic DAG YAML
(`FROM -> TO via: cbu.rename`) loaded through the real
`DagRegistry::from_dir`. Wired as `ReplOrchestratorV2::gate_pipeline`,
so the **same pipeline instance** feeds both
`phase5_runtime_recheck`'s/`reseal_for_human_gate_resume`'s shadow G4
evaluation and the real dispatch-time `pre_dispatch_gate_check` — the
two agree by construction.

### 2.5 Synthetic single-verb `DomainMetadata` — G7 (write set)

`DomainMetadata::from_yaml` (already-`pub`, already used by
`control_plane_shadow.rs`'s own `test_domain_metadata` helper) with one
entry, `cbu.rename: writes: [cbus]` — matches `WriteSetGate::decide`'s
requirement (`contract_derived: true` and non-empty `tables`).

### 2.6 Hand-built `VerbConfigIndex` entry — feeds G2/G7's arg lookup

`VerbConfigIndex::insert_test_entry` (existing `#[cfg(test)]`-gated
method) with one entry for `cbu.rename`: `cbu-id` (entity-typed,
`lookup_entity_type: "cbu"`) and `name` (plain string). This is
**only** consulted by the shadow-evaluation call sites
(`entity_binding_requests`, `build_write_set_input`) — real dispatch
(`ObPocVerbExecutor::from_pool`, unmodified) reads the real production
`config/verbs/cbu.yaml` independently, so this synthetic index cannot
mask a real-dispatch bug.

### 2.7 Pre-inserted `CompiledRunbook` — G9 (runbook proof)

`phase5_runtime_recheck` reads `entry.compiled_runbook_id` directly and
runs **before** `execute_entry_via_gate_impl`'s on-the-fly-compile
fallback populates it — confirmed by reading both functions. Without a
pre-populated `CompiledRunbook`/`CompiledRunbookId` on the entry, the
shadow evaluate always Rejects on `"no compiled runbook reference
available"` (`decision.rs:303-311`), regardless of every other gate.
The test builds a `CompiledStep`/`CompiledRunbook` matching the entry
(`step_id == entry.id`, per `runbook/types.rs:139`'s own doc comment)
and inserts it into a real `RunbookStore` via the existing
`insert_sync` method, then sets `entry.compiled_runbook_id` before
`add_entry`.

### 2.8 The real orchestrator, the real park→approve path

`build_orchestrator` assembles a real `ReplOrchestratorV2` with the
above wired via the existing production builder methods
(`.with_pool`, `.with_verb_execution_port`, `.with_sem_os_client`,
`.with_verb_config_index`, `.with_domain_metadata`,
`.with_gate_pipeline`, `.with_runbook_store`) — no new orchestrator
construction path, no test-only shortcuts to `handle_human_gate_
approval` or `phase5_runtime_recheck` themselves. The test calls
`orch.execute_runbook(&mut session)` (parks) then
`orch.handle_human_gate_approval(&mut session, entry_id,
Some("g1-test-approver".to_string()))` (resumes) — both are the
orchestrator's own real, private methods, reachable because
`g1_humangate_reseal_tests` is declared inside `sequencer.rs` itself
(same privacy rule the file's pre-existing `mod tests` block already
relies on: a child module of `sequencer` can see the module's private
items).

### 2.9 Assertions

1. After `execute_runbook`: entry status is `Parked`.
2. Exactly one `control_plane_envelopes` row exists for
   `(session_id, entry_id)`, `status = 'sealed'` — the park-time seal
   really happened (proves every proof-bearing gate really reached
   `Success`, not an assumption).
3. That row's `not_after` is pushed to `now() - 10 minutes` directly
   via SQL — "let time pass" per the design doc's own suggested
   technique (§10 item 4: "sleep... or lower the test's
   `ValidityWindow`... or age the row directly").
4. After `handle_human_gate_approval`: entry status is `Completed`.
5. Exactly **two** rows now exist for `(session_id, entry_id)`, ordered
   by `created_at`:
   - row 0 == the pre-park `envelope_id`, `status` still `'sealed'`
     (never itself consumed — the design doc's own assertion (i)).
   - row 1 has a **different** `envelope_id`, `status = 'consumed'`
     (the fresh reseal was the one actually consumed — assertion (ii)).

---

## 3. RED→GREEN proof

Per this program's evidence bar ("proven," not "written"): temporarily
reverted `handle_human_gate_approval`'s reseal call to a no-op
(`if false { self.reseal_for_human_gate_resume(...).await; }`,
`sequencer.rs:3555-3557`) and re-ran the new test.

**RED** (reseal disabled):

```
thread 'sequencer::g1_humangate_reseal_tests::human_gate_resume_reseals_fresh_envelope_not_the_stale_pre_park_one'
panicked at src/sequencer.rs:11968:9:
assertion `left == right` failed: resumed HumanGate entry must complete via the freshly
resealed envelope, not fail on the (now-expired) pre-park one: Some(Object {"error":
String("invalid input: cbu.rename envelope admission rejected: Expired")})
  left: Failed
 right: Completed
test result: FAILED. 0 passed; 1 failed
```

This is the exact failure mode the design predicts: with no reseal,
`lookup_sealed_handle` still finds the pre-park envelope (its status
column is still literally `'sealed'` — nothing rewrote it), threads it
into `execute_verb_admitting_envelope`, and `consume_core` grades it
`Expired` against the artificially-aged `not_after` — `RejectedConsumeFailed(Expired)`,
surfaced as the `AdmissionDecision::RejectedConsumeFailed` error string
match exactly.

**GREEN** (reseal call restored, byte-for-byte back to the pre-session
code):

```
test sequencer::g1_humangate_reseal_tests::human_gate_resume_reseals_fresh_envelope_not_the_stale_pre_park_one ... ok
test result: ok. 1 passed; 0 failed
```

Diff confirms the revert is exact — `git diff --stat src/sequencer.rs`
after the revert shows only the additive `+563` from the new test
module, no residual edit to `handle_human_gate_approval`.

---

## 4. STOP-conditions hit

**None that blocked the deliverable.** One STOP-condition-shaped fork
was hit and resolved without escalation: §1.2's finding that the
brief's literal "park as `RequiresHumanGate`" scenario is unreachable
from real code. This was investigated to a definite, code-cited
conclusion (not assumed, not asserted without derivation) and the test
was retargeted to the design doc's own literal wording instead of
either (a) forcing a synthetic/fabricated `RequiresHumanGate` path
that doesn't correspond to any real production call shape, or (b)
declaring the whole task infeasible. This is disclosed in §0 and §1.2,
not silently substituted.

The "materially larger integration-test build-out" characterisation
from the two prior sessions was checked by actually attempting the
build, per this task's own instruction not to inherit that
conclusion without trying. It turned out accurate in scope (every
proof-bearing gate genuinely needed a real, non-trivial fixture) but
not accurate in the implied "shouldn't attempt" framing — every
fixture had a directly reusable pattern already in the tree, so the
actual net new code is one self-contained ~560-line test module, no
new production code, no new shared test infrastructure exposed
cross-module.

---

## 5. Verification (every command run fresh this session, real output)

```
$ cargo test --lib --features database -p ob-poc g1_humangate_reseal_tests --no-run
   Finished `test` profile [unoptimized + debuginfo] target(s) in 21.12s
```

```
$ DATABASE_URL="postgresql:///data_designer" cargo test --lib --features database -p ob-poc \
    g1_humangate_reseal_tests -- --ignored --nocapture
running 1 test
test sequencer::g1_humangate_reseal_tests::human_gate_resume_reseals_fresh_envelope_not_the_stale_pre_park_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2382 filtered out; finished in 0.16s
```

Re-ran 3 more times back-to-back — deterministic, no flakiness (each
run mints fresh `session_id`/`entry_id` UUIDs, so there is no
cross-run row collision in `control_plane_envelopes`):

```
running 1 test
test ... ok
test result: ok. 1 passed; 0 failed ... finished in 0.09s
running 1 test
test ... ok
test result: ok. 1 passed; 0 failed ... finished in 0.11s
running 1 test
test ... ok
test result: ok. 1 passed; 0 failed ... finished in 0.11s
```

**RED→GREEN** — see §3 verbatim output.

**No regression to the pre-existing G1 item 2/3 live-DB tests**
(same-area tests, same env var, ran alongside to check for
interference):

```
$ DATABASE_URL="postgresql:///data_designer" cargo test --lib --features database -p ob-poc \
    g1_item2_path_a_tests -- --ignored --nocapture
running 2 tests
test runbook::step_executor_bridge::g1_item2_path_a_tests::no_sealed_envelope_for_this_entry_is_rejected_with_triage_classification ... ok
test runbook::step_executor_bridge::g1_item2_path_a_tests::admits_consumes_once_from_path_a_then_a_bare_retry_finds_nothing_sealed ... ok
test result: ok. 2 passed; 0 failed
```

**Full `ob-poc` lib suite (non-ignored), unchanged pass count** (the
new test is correctly bucketed into `ignored`, not run by default):

```
$ cargo test --lib --features database -p ob-poc
test result: ok. 2174 passed; 0 failed; 209 ignored; 0 measured; 0 filtered out; finished in 27.70s
```

(2174/0 — same figure the G1-items-3-4 and G5 sessions cite as their
own baseline-plus-additions; +0 here since this session added only an
`#[ignore]`-gated test.)

**Plugin-verb coverage invariant, unaffected** (sanity check — this
session touched no verb registration):

```
$ cargo test -p ob-poc --lib -- test_plugin_verb_coverage
test domain_ops::tests::test_plugin_verb_coverage ... ok
test result: ok. 1 passed; 0 failed
```

**Clippy** — `-D warnings` on `--lib --tests` surfaces 7 pre-existing
errors, **none in this session's new code**; confirmed pre-existing by
`git stash` + re-run on unmodified HEAD (identical 7 errors, byte-for-
byte, same files/lines: `control_plane_metrics.rs` (3×
`int_plus_one`), `tests/kyc_m3_remediation.rs` /
`tests/kyc_verb_coverage.rs` (`expect_fun_call` — note: which of these
two test targets clippy reaches first is nondeterministic across runs,
both pre-existing), `traceability/phase2.rs` +
`traceability/replay.rs` (2× `dead_code`), and
`sem_os_runtime/verb_executor_adapter.rs:1473`
(`await_holding_lock` — the **existing** `EnvGuard` pattern this
session's own `EnvGuard` duplicates, per this program's established
"duplicate locally rather than widen visibility" convention; this
session's copy has the identical shape and would trip the identical
lint if `-D warnings` clippy were ever run scoped to include it, but
it wasn't reached in either the stashed or unstashed run — the crate
compile aborts at the first `-D`-promoted error per target, so later
files/targets in the dependency graph are not reliably reached either
way). Both stashed (base) and unstashed (with this session's diff)
runs produce the identical 7-error set — **this session introduces
zero new clippy findings**, but also does not itself clear the
pre-existing 7 (out of scope; not this task's job, not silently
claimed as fixed).

**`check-invariants.sh all`** — ran to completion, `5/5 do not hold`,
matching `invariants-expected.toml`'s ratchet (all of E1-E5 recorded
`status = "fail"`). This session made zero pub-surface changes and zero
crate-dependency changes (one new `#[cfg(test)]` module, private to
`sequencer.rs`), so no invariant-status shift was expected or observed.
`E5` (`unreachable_pub`) output confirms the same 52-crate enforcement
list as before — unaffected by this session's diff (test code is not
`pub`).

**Diff scope**: `git diff --stat -- src/sequencer.rs` → `1 file
changed, 563 insertions(+)`. `git status --porcelain` shows no other
tracked file touched by this session (the five pre-existing dirty
files named in the task brief — `observatory-wasm/Cargo.lock`,
`rust/cbu_mismatches.json`, `rust/mismatches.json`,
`rust/reports/phase0_confusion_matrix.json`,
`rust/reports/step0_trial_evaluation.json` — remain exactly as they
were at session start, untouched by this session).

---

## 6. `invariants-expected.toml` — recommendations (not applied)

None. This session added test coverage only; it does not close any of
E1-E5's underlying gaps (no `RR-*` row moved to provably-closed, no new
`RR-2` admitting path, no `G1-G14` gate newly evaluated-with-evidence
on a path that previously lacked it, no Mode-1 register row
reclassified, no `unreachable_pub` crate added or removed). No ratchet
flip recommended.

---

## 7. Files changed

- `rust/src/sequencer.rs` — new module `g1_humangate_reseal_tests`
  appended after the file's existing `mod tests` block (+563 lines, 0
  removed). No other section of the file modified (the RED-proof edit
  to `handle_human_gate_approval` was reverted before this diff was
  finalised — confirmed via `git diff` showing a pure addition).
- `docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-G1-HUMANGATE-
  RESEAL-TEST-001.md` — this document (new).

No production code changed. No migration, no YAML, no Cargo.toml
touched. No commit made — working tree left as-is for independent
review per instruction.

---

## 8. Open items for future tranches

1. **Is G8 `RequiresHumanGate` reachable at all on Path A, or is it
   dead code by construction?** §1.2's finding (`has_unpinned_entities`
   and `EntityBinding`'s `NotFound` share one predicate over one
   `facts_map`, so an entity-bound verb is never simultaneously
   G2-`Bound` and G8-unpinned) was derived to explain why this
   session's test couldn't target the brief's literal scenario, not
   investigated as its own question. If this is a real gap (not an
   intentional design choice — plan A5's "unpinned-entity cap" reads
   as though it expects to be reachable), it's a `RR-*`-shaped finding
   worth a dedicated look: either a genuine production path exists
   that this session didn't find, or the STP classifier's `HumanGated`
   branch is currently unreachable from Path A's own two call sites,
   which would itself be worth recording in the ownership ledger.
2. The design doc's own §11 open item 1 (whether `Durable`'s
   mid-dispatch park ever re-enters a not-yet-consumed envelope) is
   still unaddressed — explicitly out of scope for this session per
   the task brief, not attempted here.
3. This test's fixture-assembly pattern (real orchestrator + minimal
   synthetic `GatePipeline`/`DomainMetadata`/`VerbConfigIndex`/
   `SemOsClient`) is now the third instance of essentially the same
   assembly (after `control_plane_audit.rs`'s fixture and
   `control_plane_shadow.rs`'s own G2/G3/G4 tests) — if a fourth
   Path-A-driven-end-to-end test is ever needed, factoring a shared
   (still test-only, still not cross-module-`pub`) builder would cut
   real duplication; not done here per this program's "duplicate
   locally rather than widen visibility" convention, which trades a
   small amount of duplication for zero pub-surface growth.
