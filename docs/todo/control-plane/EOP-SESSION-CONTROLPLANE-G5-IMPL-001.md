# EOP-SESSION-CONTROLPLANE-G5-IMPL-001 — Implementation session log

### Implements: G5 — Shadow-gate evaluation on B/C/D + E3 matrix ratification
### (`EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.5` §3, needs G4)
### Date: 2026-07-13
### Branch: `codex/phase-1-5-governance-closure` (not merged, not committed)

---

## 1. Verification against the plan/research/G4 session's own claims, before any code

Per the mission's "rule zero," everything below was checked against live code
before writing item 1's sweep plan or touching anything.

1. **`GateResult`'s current shape** (`rust/crates/ob-poc-control-plane/src/
   gate.rs:69-77`, read in full): `Success | Failure(String) |
   NotEvaluated{blocked_by} | NotImplemented`. Confirmed no `NotApplicable`
   variant exists yet — G5 item 1 is real, not already-done work.
2. **The G4 seam's exact current shape** (re-verified, not assumed from the
   G4 session doc's prose): `dsl_v2::executor::DslExecutor::
   execute_verb_in_scope` (`rust/src/dsl_v2/executor.rs:1955`), admission
   block at `:1976-2035`, `runtime_verb` resolved at `:2037-2039`. Confirmed
   `ExecutionContext` carries `execution_path: ExecutionPath`,
   `already_admitted_for: Option<ExecutionPath>`,
   `envelope_handle: Option<EnvelopeHandle>` exactly as the G4 session doc
   describes — no drift found.
3. **`bus_runtime.rs`'s adapter shape** (re-verified):
   `ObPocVerbAdapter::execute` (`rust/crates/ob-poc-web/src/
   bus_runtime.rs`), admitting call `execute_verb_admitting_envelope(...,
   ExecutionPath::BusFederated)` at the end of the function. Confirmed the
   struct had only an `executor` field (no pool) before this session —
   needed to add one for shadow-decision persistence (§4 below).
4. **`report_to_json`/`gate_outcome_counts`/`GateOutcomeProvenance` current
   shape** (third session in a row touching this surface — re-verified, not
   trusted from memory): `report_to_json` (`control_plane_shadow.rs:618`)
   is fully generic (`format!("{result:?}")`) — confirmed it needs **no**
   code change for a new `GateResult` variant, Debug derive covers it
   automatically. `gate_outcome_counts`'s SQL `CASE` (`control_plane_
   metrics.rs`) is NOT generic — confirmed it needed a new `WHEN` branch
   (done, §2 below). `GateOutcomeProvenance`/`expected_provenance`
   (`ob-poc-control-plane/src/audit.rs`) is a **per-gate** map, confirmed
   orthogonal to the per-path dimension G5 needed — no change required
   there; G5 added a **new, additive** per-path dimension via a new DB
   column + a new sibling query function, not a redesign of G2's provenance
   machinery (§5 below explains the coordination).

No STOP-conditions were hit in verification itself — all four checks
confirmed the plan's own assumptions hold. STOP-conditions were hit inside
item 1's own sweep (§2) and during live-DB regression testing (§7) — both
disclosed below, not silently absorbed.

---

## 2. Item 1 — `GateResult::NotApplicable`, the sweep plan (written before editing), and its outcome

**Sweep plan (written before any edit, per the mission's explicit
requirement):**

1. Grep every file referencing `GateResult` in `ob-poc-control-plane` and
   `ob-poc` (`rg -l GateResult`). Result: 15 files matched by name, of
   which **9 were false positives** — `mcp/scenario_index.rs`
   (`ScenarioGateResult`), `mcp/macro_index.rs` (`GateResult`, an unrelated
   macro-gate-check type), `sem_reg/{gates,registry,mod}.rs`
   (`sem_reg::gates::GateResult`, an unrelated SemReg governance-gate type)
   — confirmed by reading each hit; none of these are
   `ob_poc_control_plane::gate::GateResult`.
2. Of the 6 real hits (`ob-poc-control-plane`'s own 12 gate-adapter/core
   files that reference the type, plus `ob-poc`'s
   `control_plane_{metrics,audit,shadow}.rs`): every one of the 12 gate
   adapters (`pack_resolution.rs`, `evidence_gate.rs`,
   `intent_admission.rs`, `snapshot.rs`, `versioning.rs`,
   `write_set_attestation.rs`, `stp_classifier.rs`, `entity_binding.rs`,
   `write_set.rs`, `proof.rs`, `dag_proof.rs`, `authority_gate.rs`) only
   **constructs** `GateResult::Success`/`Failure(...)` from its own
   domain-typed outcome enum — none pattern-matches an existing
   `GateResult` value, so adding a variant cannot break any of them
   (confirmed by direct inspection, not assumption).
3. The only genuine exhaustive **match** over an existing `GateResult`
   value in the whole sweep set is `decision.rs::rejection_from_report`
   (used by `evaluate`/`evaluate_with_report`, Path A's real sealing path).
   Predicted this would be the sole compile-break site.
4. **Decision, stated before editing**: proceed to add the variant and let
   the compiler enumerate the sweep (`cargo build`'s non-exhaustive-match
   errors), per the mission's own instruction that the compiler IS the
   sweep. If the predicted single site turned out to have ambiguous
   correct behaviour for `NotApplicable` (not just "add an arm"), treat it
   as a STOP-condition, not a guess.

**Outcome**: the variant was added
(`gate.rs`, doc-commented — see the code for the full rationale, including
"no gate adapter's own `Gate::evaluate` impl constructs this variant;
`NotApplicable` is applied by path-aware *callers*, never inside this
crate's own evaluation loop"). `cargo build --workspace --features
database` then produced **exactly one** compile error, exactly the
predicted site:

```
error[E0004]: non-exhaustive patterns: `Some(&GateResult::NotApplicable(_))` not covered
   --> crates/ob-poc-control-plane/src/decision.rs
```

**STOP-condition hit and disclosed, per the sweep plan's own trigger
condition**: `rejection_from_report`'s correct behaviour for
`NotApplicable` among `PROOF_BEARING_GATES` is genuinely ambiguous, not
mechanical. Should a `NotApplicable` proof-bearing gate be treated as
vacuously satisfied (like `Success`, since "the concept doesn't apply here"
could reasonably mean "nothing to check") or as a hard block? Tracing
further: even if "vacuously satisfied" were chosen, the immediately
following proof-derivation code in `evaluate_with_report` unconditionally
re-derives a **typed proof value** for every proof-bearing gate from
`ctx.<gate>` — and a `NotApplicable` gate's `Input` was never built (no
proof value exists to derive), so `ExecutionEnvelope`'s sealed-proof shape
itself would need a design change to represent "this gate doesn't apply to
this envelope." That is a real product decision about the envelope's own
structure, out of a mechanical sweep's scope.

**Resolution taken** (documented in-code, at the exact site, per the STOP
discipline): fail-CLOSED. `NotApplicable` in `rejection_from_report` is
treated as `GateFailure::Failed`, with a message naming exactly why (this
function's only two callers, `evaluate`/`evaluate_with_report`, are
Path-A-only, and Path A never applies a path-conditional `NotApplicable`
override — so this arm is compile-reachable but not reachable with any
real Path A data today). This is the smallest, safest, most conservative
fix that satisfies the compiler without inventing an envelope-shape design
decision. Flagged here for architect review, not silently resolved as if
it were a mechanical arm.

**Extended per item 1's own text**:
- `report_to_json`: confirmed needs no change (Debug-generic).
- `gate_outcome_counts`'s SQL `CASE`: added a `WHEN kv.value LIKE
  'NotApplicable%' THEN 'NotApplicable'` branch (both the production query
  and the new `gate_outcome_counts_by_path` sibling — see item 3-5).
- The probe: extended, see §6.
- **Window-discipline check, same diff**: `applicability.rs`'s own matrix
  data makes Path A `Applicable` for every one of the 14 gates
  (`path_a_is_the_identity_matrix` test) — `apply_matrix` called on Path A
  data is provably the identity function, so window discipline holds *by
  construction*, not by convention (no Path A call site calls
  `apply_matrix` at all — confirmed by grep: only the new dsl_v2 seam and
  bus adapter call it). Verified LIVE against the real accumulated
  `control_plane_shadow_decisions` table via
  `g5_path_a_never_produces_not_applicable` — see §7.

**Sweep site count: 1 (predicted), 1 (actual). Zero missed sites.**

---

## 3. Item 2 — the three UNKNOWNs, resolved by code

Full resolution with file:line citations lives in the new design doc
(`docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-G5-GATE-APPLICABILITY-
MATRIX-001.md` §3). Summary:

1. **G3 (PackResolution) on Path B → NotApplicable.** `RealDslExecutor`'s
   struct (`repl/executor_bridge.rs:29-46`) has zero pack/session field;
   `main.rs`'s 4 construction sites confirm the same. Resolves R:§B6's
   "not independently confirmed" hedge to a definite verdict.
2. **G3 on Path C → NotApplicable.** Path C is the *same*
   `RealDslExecutor`/`dsl_v2::executor::DslExecutor` engine, wrapped by
   `WorkflowDispatcher` (`main.rs:1338-1343`) — identical evidence to B.
3. **"G3 on C vs D distinctions" → there is no distinction.** All three
   non-A paths share the identical structural absence; the plan's own
   framing of this as a "distinction to resolve" resolves to "uniform, not
   distinct."
4. **G9 (RunbookProof) on B and C → NotApplicable (both).** Path B/C's
   plan object, `dsl_v2::execution_plan::ExecutionPlan`
   (`execution_plan.rs:47-52`), has exactly two fields (`steps`, `dag`) —
   no `compiled_runbook_id` and no equivalent reachable anywhere in the
   `execute_plan`/`execute_verb_in_scope` call chain.

---

## 4. Items 3-4 — the new evaluation call sites

**Path B/C** (`dsl_v2::executor::DslExecutor::execute_verb_in_scope`,
`rust/src/dsl_v2/executor.rs`, inserted right after `runtime_verb` resolves
and before any dispatch branch): a `#[cfg(feature = "database")]`,
`tokio::spawn`-backed block that, only for `path` in
`{DslDirect, WorkflowDispatched}`, builds a bounded `EvaluationContext`,
calls `evaluate_shadow` → `applicability::apply_matrix` →
`build_shadow_decision_row` → `insert_shadow_decision`. Best-effort, never
blocks real dispatch (same posture as Path A's own shadow insert).

**Path D** (`ObPocVerbAdapter::execute`, `rust/crates/ob-poc-web/src/
bus_runtime.rs`, inserted immediately before the admitting call): the same
shape, `ExecutionPath::BusFederated` only, no `#[cfg(feature = "database")]`
gate needed (`ob-poc-web` declares no such feature — sqlx is
unconditionally available there, confirmed via `Cargo.toml`).

**What's genuinely wired** (independently substantive under
`gate::GATE_DEPENDENCIES`'s real collect-where-independent semantics — see
the matrix doc §4/§5 for the full accounting):
- **G1 IntentAdmission** — real, but a WEAKER signal than Path A's SemOS
  ABAC/pack-pruning grade: `is_admitted: true` here means "the verb
  resolved in the runtime registry / the bus already mapped a
  `local_verb_id`," not "SemOS's context envelope admitted it." Disclosed
  in-code, not presented as equivalent evidence.
- **G12 VersionPinning** — real, identical reuse of Path A's own
  `env!("CARGO_PKG_VERSION")` logic, zero session dependency.
- **G3/G9** — the four ratified `NotApplicable` cells (§3), verified live.

**What's built but NOT independently substantive — a genuine finding, not
an oversight**: **G8 StpClassifier**'s input IS constructed at both new
call sites (same `is_durable_verb` derivation Path A uses), but
`GATE_DEPENDENCIES` declares it depends on 7 other gates
(IntentAdmission, EntityBinding, PackResolution, DagProof, Authority,
Evidence, WriteSet) — none of which are wired at these call sites this
session, so StpClassifier correctly reports `NotEvaluated{blocked_by:...}`,
never `Success`/`Failure`, under the evaluator's real blocking semantics.
**This was discovered live**: the E3 matrix probe's first run (§6/§7)
initially claimed G8 as a "wired" gate and failed with 0 substantive
samples at all three of B/C/D — the probe's own gate list was corrected in
response (not the underlying semantics), and the doc comments at both call
sites were corrected to state this precisely. Left as-is (not force-wired
further) — wiring G2/G4-G7 at these call sites is exactly the
generalization-gap work named in §5 below and in the matrix doc, out of
this session's bounded scope.

**Generalization gaps found and documented, not forced** (full accounting
in the matrix doc §5): G2 (needs `VerbConfigIndex`+batched entity-facts
I/O), G4 (needs a `GatePipeline`, owned by `ReplOrchestratorV2`, not
reachable from either B/C's or D's engine), G5/G6 (need
`SemOsContextEnvelope`, not built at either call site), G7 (needs
`DomainMetadata` lookup, plumbable but not wired), G13 (needs the same
batched entity-facts as G2). None of these were forced through with a
fabricated input — every gate either has a real value or stays `None`
("not observed here," matching this codebase's own established "no
signal means no fabricated pass" doctrine throughout `control_plane_
shadow.rs`).

**A necessary pub-surface widening, disclosed**: Path D's call site lives
in a *different crate* (`ob-poc-web`) from `control_plane_shadow`'s
`build_shadow_decision_row`/`insert_shadow_decision`/`ShadowDecisionRow`
(`ob-poc`, `pub(crate)`-scoped before this session). Rather than
duplicating the row shape and `INSERT` statement inside `ob-poc-web` (the
"parallel, redundant tracking mechanism" the mission said not to build),
these three items — and the `agent::control_plane_shadow` module itself —
were widened from `pub(crate)` to `pub`. Every OTHER item in that module
(the input builders, `build_evaluation_context`, etc.) stays at its
existing `pub(crate)` visibility; only the three genuinely-needed items are
now part of `ob-poc`'s public surface. `ob-poc-web` gained a new direct
dependency on `ob-poc-control-plane` (Cargo.toml) — confirmed no cycle
(`ob-poc-control-plane` already has zero reverse-dependents beyond
`ob-poc`/`ob-poc-boundary`/`ob-poc-agent`; adding `ob-poc-web` is the same
"values + evaluator" edge shape). **This changes `ob-poc`'s public-API
surface** — flagged explicitly for the blind-review summary; the E5
public-API-surface baseline for `ob-poc` will need a refresh (recommended
in §9, not applied — per the established pattern of recommending
`invariants-expected.toml`/baseline changes rather than silently rolling
them into this diff).

---

## 5. Item 5 — the E3 gate/probe amendment, coordinated with G2's provenance machinery

**Coordination, not duplication**: G2's `GateOutcomeProvenance`/
`expected_provenance` (`ob-poc-control-plane/src/audit.rs`) is a **per-gate**
map (ShadowEval / ConsumeSeam / PostDispatch) — orthogonal to the
**per-path** dimension G5 needed. G5 did not touch that map, did not add a
path dimension to it, and did not rebuild `gate_outcome_counts`'s existing
provenance logic. Instead:

1. **New migration** (`rust/migrations/20260713_control_plane_shadow_
   decisions_execution_path.sql`): adds `execution_path TEXT NOT NULL
   DEFAULT 'A'` (+ a CHECK constraint + index) to
   `control_plane_shadow_decisions`. `DEFAULT 'A'` is a correct historical
   backfill, not a guess — every row inserted before this migration came
   from exactly one call site (Path A's `phase5_runtime_recheck`).
2. **`ShadowDecisionRow`/`build_shadow_decision_row`/
   `insert_shadow_decision`** all gained an `execution_path` parameter/
   field. Every existing call site (Path A's two production call sites in
   `sequencer.rs`, plus every test fixture across `control_plane_audit.rs`/
   `control_plane_metrics.rs`/`control_plane_shadow.rs`) was updated to
   pass `ExecutionPath::RunbookSequencer` explicitly — no implicit
   default, no silent behavior change (compiler-enforced: this was a
   function-signature change, the compiler found every call site).
3. **New sibling query function** `gate_outcome_counts_by_path`
   (`control_plane_metrics.rs`), additive beside the existing path-blind
   `gate_outcome_counts` — same table, same `shadow_eval` provenance,
   grouped by the new column too. Scoped honestly: it does NOT attempt to
   add a path dimension to G10 (`consume_seam`)/G14 (`post_dispatch`)
   samples, which come from `control_plane_audit` — that table has no
   `execution_path` column and giving it one is G1/G2-scoped work, not
   named in G5's item list. Documented in the function's own doc comment.
4. **New probe tests**: `e3_matrix_invariant_probe` (asserts, per §2's
   ratified matrix, that G1/G12 show substantive samples and G3/G9 show
   `NotApplicable` samples at each of B/C/D — exercising the real
   production functions with synthetic-but-real traffic, per the plan's own
   "synthetic acceptable for B/C/D initially" exit-gate allowance) and
   `g5_path_a_never_produces_not_applicable` (the window-discipline live
   proof, §2). Both wired into `scripts/check-invariants.sh`'s `gate_e3`
   as a new "G5 per-(gate, path) matrix probe" block, run alongside (not
   replacing) the existing path-blind `e3_invariant_probe`.
5. **A real production consumer added for `gate_outcome_counts_by_path`**
   (not left `#[allow(dead_code)]`-tolerated): the existing
   `GET .../control-plane-metrics` REST endpoint
   (`src/api/agent_routes.rs::get_control_plane_metrics`) now also returns
   `gate_outcomes_by_path` — the operator-facing surface for "is the
   ratified matrix holding on real B/C/D traffic," matching the shape of
   every other field on that response.

**`[e3]` carried in the same diff — recommendation only, per §9** (not
applied, per the established pattern on this branch).

---

## 6. Live-DB regression found and fixed mid-session (disclosed, not silent)

Running the full `control_plane*` test module together (not the two new
tests in isolation) surfaced a real regression this session's own diff
caused: `w3_shadow_eval_slice_matches_legacy_query_modulo_sentinel_fix`
started failing. Root cause: the legacy (pre-rewrite) query has no `CASE`
branch for `NotApplicable(...)` values either, so they fell into its
`Unrecognised` bucket — exactly the same shape of gap the G2 session's own
`"missing"` → `NotRegistered` sentinel fix addressed, but for a NEW
sentinel this session introduced. Confirmed via `git stash` that this test
passed before this session's diff and failed after — a genuine regression,
not a pre-existing flake. **Fixed** (not merely documented): the test's own
assertion was extended from a two-way split (`rebuilt_unrecognised +
rebuilt_not_registered == legacy_count`) to a three-way split (`+
rebuilt_not_applicable`), matching the new sentinel the rewritten query now
distinguishes. Verified green after the fix (§7).

---

## 7. Real command output — every claim below, run for real this session

**Item 1's sweep (single compile error, predicted site):**
```
$ cd rust && cargo build --workspace --features database 2>&1 | grep -E "^error" | sort -u
error: could not compile `ob-poc-control-plane` (lib) due to 1 previous error
error[E0004]: non-exhaustive patterns: `Some(&GateResult::NotApplicable(_))` not covered
```
(One site, `decision.rs::rejection_from_report` — resolved per §2. Zero
errors after the fix.)

**Full workspace build, forced (not trusting stale incremental cache):**
```
$ cargo build --workspace --features database              # clean
$ cargo build --workspace --all-targets --features database  # clean
$ cargo build -p ob-poc-types --no-default-features         # clean
$ cargo build -p ob-poc-control-plane                       # clean
```

**Applicability matrix unit tests** (`ob-poc-control-plane`):
```
$ cargo test -p ob-poc-control-plane --lib applicability
running 4 tests
test applicability::tests::the_three_named_unknowns_are_resolved_not_applicable ... ok
test applicability::tests::path_a_is_the_identity_matrix ... ok
test applicability::tests::every_cell_is_covered ... ok
test applicability::tests::apply_matrix_only_overrides_not_applicable_cells ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 116 filtered out
```

**New live-DB tests, standalone (first correct run, post-StpClassifier fix):**
```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    -- --ignored --nocapture e3_matrix_invariant_probe g5_path_a_never_produces_not_applicable
running 2 tests
test agent::control_plane_metrics::t7_2_metrics_tests::g5_path_a_never_produces_not_applicable ... ok
[E3-matrix] path=B gate=IntentAdmission: 2 substantive samples
[E3-matrix] path=B gate=VersionPinning: 2 substantive samples
[E3-matrix] path=B gate=PackResolution: 2 NotApplicable samples
[E3-matrix] path=B gate=RunbookProof: 2 NotApplicable samples
[E3-matrix] path=C gate=IntentAdmission: 2 substantive samples
[E3-matrix] path=C gate=VersionPinning: 2 substantive samples
[E3-matrix] path=C gate=PackResolution: 2 NotApplicable samples
[E3-matrix] path=C gate=RunbookProof: 2 NotApplicable samples
[E3-matrix] path=D gate=IntentAdmission: 2 substantive samples
[E3-matrix] path=D gate=VersionPinning: 2 substantive samples
[E3-matrix] path=D gate=PackResolution: 2 NotApplicable samples
[E3-matrix] path=D gate=RunbookProof: 2 NotApplicable samples
test agent::control_plane_metrics::t7_2_metrics_tests::e3_matrix_invariant_probe ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 2373 filtered out
```

**Full `control_plane*` module together (W3 regression found, fixed, re-run green):**
```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    -- --ignored --nocapture control_plane
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 2341 filtered out
```
(One pre-existing, expected-fail test — `e3_invariant_probe` — excluded
from that "34 passed" count and discussed separately below; it fails on
`AuditReplay`/`WriteSetAttestation`, both G1/G2-scoped gaps, confirmed
pre-existing via `git stash` — same failure, same reason, before this
session's diff.)

**G4's own atomicity/regression suites — re-verified for a real
regression, found a test-isolation flake, confirmed pre-existing:**
```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    -- --ignored --nocapture g4_seam_admission_tests t4_1_envelope_admission_tests
...
FAILED: t4_1_envelope_admission_tests::envelope_with_wrong_content_hash_is_rejected_loudly
FAILED: t4_1_envelope_admission_tests::execute_verb_admitting_envelope_rejects_on_pin_drift_and_leaves_envelope_reconsumable
```
Investigated rather than dismissed: both suites select `cbus` rows by
`ORDER BY cbu_id LIMIT 1 OFFSET N` against the same shared live dev
database, and `cargo test`'s default parallel threads run both suites'
tests concurrently. Re-ran `t4_1_envelope_admission_tests` **standalone**
(same tree state, same session): **7/7 pass.** Re-ran both suites together
with `--test-threads=1`: **11/11 pass.** This isolates the cause to
row-offset contention under parallel execution against a shared table, not
a G5 regression — confirmed further via `git stash`: `t4_1_*` alone passes
7/7 both before and after this session's diff.

**Full `ob-poc` lib suite:**
```
$ cargo test -p ob-poc --lib --features database
test result: ok. 2169 passed; 0 failed; 206 ignored
```
(2169 passed matches G4's own session-end baseline exactly.)

**Plugin coverage:**
```
$ cargo test -p ob-poc --lib -- test_plugin_verb_coverage
test result: ok. 1 passed; 0 failed
```

**Clippy, touched crates, `-D warnings`** — the identical pre-existing
finding set (14 findings: `control_plane_metrics.rs` `int_plus_one` x2,
`traceability/{phase2,replay}.rs` dead-code x3, `kyc_verb_coverage.rs`/
`kyc_m3_remediation.rs` `expect_fun_call`/`doc_lazy_continuation` x4,
`main.rs` `items_after_test_module`, `verb_executor_adapter.rs`
`await_holding_lock`) was reproduced via `git stash` BEFORE this session's
diff, matching G4's own session doc's disclosed pre-existing list almost
verbatim (line-number drift only). **Zero new clippy findings from this
session's own diff** — verified by comparing the stashed (before) and
unstashed (after) `-D warnings` error sets directly, not by eyeballing a
single run (per the mission's explicit caution about trusting a
possibly-stale incremental build).

**The literal `check-invariants.sh e3` run, this session, after all fixes
above (full output, not paraphrased):**
```
$ DATABASE_URL=postgresql:///data_designer bash scripts/check-invariants.sh e3
== E3: G1-G14 evaluated in production with metrics flowing ==
  Compile-time half (exhaustive GateId match, no _ arm):

running 1 test
test agent::control_plane_metrics::t7_2_metrics_tests::e3_gate_label_match_is_exhaustive ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2374 filtered out; finished in 0.00s


  Live half (gate_outcome_counts over real control_plane_shadow_decisions rows, Path A):

running 1 test
[E3] IntentAdmission: 102 substantive (Success/Failure) production samples total, 102 at expected provenance=shadow_eval
[E3] EntityBinding: 52 substantive (Success/Failure) production samples total, 52 at expected provenance=shadow_eval
[E3] PackResolution: 36 substantive (Success/Failure) production samples total, 36 at expected provenance=shadow_eval
[E3] DagProof: 36 substantive (Success/Failure) production samples total, 36 at expected provenance=shadow_eval
[E3] Authority: 86 substantive (Success/Failure) production samples total, 86 at expected provenance=shadow_eval
[E3] Evidence: 36 substantive (Success/Failure) production samples total, 36 at expected provenance=shadow_eval
[E3] WriteSet: 36 substantive (Success/Failure) production samples total, 36 at expected provenance=shadow_eval
[E3] StpClassifier: 36 substantive (Success/Failure) production samples total, 36 at expected provenance=shadow_eval
[E3] RunbookProof: 36 substantive (Success/Failure) production samples total, 36 at expected provenance=shadow_eval
[E3] ExecutionEnvelope: 58 substantive (Success/Failure) production samples total, 58 at expected provenance=consume_seam
[E3] AuditReplay: 0 substantive (Success/Failure) production samples total, 0 at expected provenance=shadow_eval
[E3] VersionPinning: 16 substantive (Success/Failure) production samples total, 16 at expected provenance=shadow_eval
[E3] DecisionSnapshot: 52 substantive (Success/Failure) production samples total, 52 at expected provenance=shadow_eval
[E3] WriteSetAttestation: 16 substantive (Success/Failure) production samples total, 0 at expected provenance=post_dispatch

thread 'agent::control_plane_metrics::t7_2_metrics_tests::e3_invariant_probe' panicked at src/agent/control_plane_metrics.rs:773:9:
E3_INVARIANT_FAILURE: 1 gate(s) have zero substantive production samples anywhere: ["AuditReplay"]; 1 gate(s) have samples only at the WRONG provenance (expected-provenance mismatch): ["WriteSetAttestation"]
test agent::control_plane_metrics::t7_2_metrics_tests::e3_invariant_probe ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2374 filtered out; finished in 0.01s
  ** E3 live half: INVARIANT FAILURE — verified against a live DB, N/14 gates genuinely empty. **

  Live half — G5 per-(gate, path) matrix probe (Path B/C/D + window discipline):

running 2 tests
test agent::control_plane_metrics::t7_2_metrics_tests::g5_path_a_never_produces_not_applicable ... ok
[E3-matrix] path=B gate=IntentAdmission: 9 substantive samples
[E3-matrix] path=B gate=VersionPinning: 9 substantive samples
[E3-matrix] path=B gate=PackResolution: 9 NotApplicable samples
[E3-matrix] path=B gate=RunbookProof: 9 NotApplicable samples
[E3-matrix] path=C gate=IntentAdmission: 5 substantive samples
[E3-matrix] path=C gate=VersionPinning: 5 substantive samples
[E3-matrix] path=C gate=PackResolution: 5 NotApplicable samples
[E3-matrix] path=C gate=RunbookProof: 5 NotApplicable samples
[E3-matrix] path=D gate=IntentAdmission: 5 substantive samples
[E3-matrix] path=D gate=VersionPinning: 5 substantive samples
[E3-matrix] path=D gate=PackResolution: 5 NotApplicable samples
[E3-matrix] path=D gate=RunbookProof: 5 NotApplicable samples
test agent::control_plane_metrics::t7_2_metrics_tests::e3_matrix_invariant_probe ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 2373 filtered out; finished in 0.02s
  E3: DOES NOT HOLD (see harness output above)
```

**Interpretation of this literal output**: G5's own scope — the per-(gate,
path) matrix probe — is **fully green** (2/2, including the window-
discipline proof). The overall `E3: DOES NOT HOLD` verdict is driven
entirely by the **pre-existing** `AuditReplay`/`WriteSetAttestation` gaps
(G1/G2-scoped, confirmed unrelated to this session's diff via the
`git stash` comparison above) — not by anything G5 was asked to close.
This matches the plan's own §4 completion-mapping row for E3: *"G1 (G10) +
G2 (G11/G12/G14) + G5 (matrix + B/C/D)"* — E3's top-level `status` was
never going to flip to `pass` from G5 alone; G5's contribution is real and
verified, the remaining gap belongs to G1/G2.

---

## 8. STOP-conditions hit and what was done

1. **Item 1's `decision.rs::rejection_from_report` ambiguity** (§2) —
   genuinely undetermined product behaviour (what should sealing do when a
   proof-bearing gate is not-applicable). Resolved fail-closed, disclosed
   in-code and here, not silently guessed through. Flagged for architect
   review as part of this session's blind-review summary.
2. **G8 StpClassifier's dependency-blocking discovery** (§4) — not a STOP
   in the sense of halting work, but a genuine finding that corrected this
   session's own initial (wrong) assumption that G8 "generalizes cleanly
   with no dependency" — caught live by the probe itself, fixed in the
   probe's own gate list and in both call sites' doc comments, not
   silently smoothed over.
3. **The W3 live-DB regression** (§6) — found via full-module regression
   testing, root-caused, and fixed in the same diff (not merely
   documented) per the "adapt and note the deviation" latitude, since it
   was a small, mechanical extension of an existing test's own comparison
   logic, not a design question.
4. **The t4_1/g4_seam parallel-execution flake** (§7) — investigated to
   ground truth (isolated via standalone + `--test-threads=1` reruns,
   confirmed via `git stash` to predate this session), NOT fixed (out of
   scope — pre-existing test-isolation fragility in shared-row-offset
   fixtures, not a G5 concern) — flagged here so a future session doesn't
   waste time rediscovering it.

None of these reached a genuine "halt this piece of work" bar — each was
either a small, disclosed, conservative code decision, or a finding that
corrected this session's own plan mid-flight without changing its overall
shape.

---

## 9. `invariants-expected.toml` recommendations (not applied)

Per the established pattern — recommending only:

- **`[e3]`**: detail comment should record: "G5 landed the ratified
  gate-applicability matrix (`EOP-DESIGN-CONTROLPLANE-G5-GATE-
  APPLICABILITY-MATRIX-001`, DRAFT pending architect ratification) and
  extended shadow-gate evaluation to Paths B/C/D at the dsl_v2 seam and
  `bus_runtime.rs`'s adapter. Per-path: G1/G12 substantive at B/C/D; G3/G9
  ratified `NotApplicable` at B/C/D (verified live,
  `e3_matrix_invariant_probe`); G8's input is built but blocked on its own
  declared predecessors (G2/G4-G7 unwired at B/C/D) — a disclosed
  generalization gap, not a defect. `status` should STAY `fail` — the
  pre-existing G11 (AuditReplay, zero samples anywhere) and G14
  (WriteSetAttestation, samples only at the wrong provenance) gaps are
  G1/G2-scoped, untouched by this tranche, and remain the sole blockers to
  a top-level pass."
- **New key recommendation, `[e3-g5-matrix]` or a G5-specific sub-note**:
  "G5's own per-(gate, path) matrix probe (`e3_matrix_invariant_probe` +
  `g5_path_a_never_produces_not_applicable`) is fully green as of this
  session — worth its own tracked line so a future E3 status flip can cite
  it independently of the G11/G14-blocked top-level status."
- **E5 public-API surface baseline**: `ob-poc`'s surface changed this
  session (three items in `agent::control_plane_shadow` widened
  `pub(crate)` → `pub`; the module itself widened too). If `[e5]`'s
  baseline-refresh tracking treats this as a live drift source, it should
  be refreshed to include this session's diff — not applied here (E5 was
  not this session's scope, flagged only because this session's own diff
  is a genuine, disclosed contributor to that baseline's next refresh).

---

## 10. Full file-changed list

Production code:
- `rust/crates/ob-poc-control-plane/src/gate.rs` — `GateResult::
  NotApplicable(String)` variant + doc.
- `rust/crates/ob-poc-control-plane/src/decision.rs` —
  `rejection_from_report`'s new arm (the item 1 STOP-condition site).
- `rust/crates/ob-poc-control-plane/src/lib.rs` — registers the new
  `applicability` module.
- `rust/crates/ob-poc-control-plane/src/applicability.rs` (new) — the
  ratified matrix (`applicability`, `apply_matrix`) + 4 tests.
- `rust/crates/ob-poc-web/Cargo.toml` — new `ob-poc-control-plane`
  dependency.
- `rust/crates/ob-poc-web/src/bus_runtime.rs` — `ObPocVerbAdapter` gains a
  `pool` field; Path D's new shadow-eval call site.
- `rust/src/agent/mod.rs` — `control_plane_shadow` module widened
  `pub(crate)` → `pub`.
- `rust/src/agent/control_plane_shadow.rs` — `ShadowDecisionRow`/
  `build_shadow_decision_row`/`insert_shadow_decision` widened to `pub`
  and gained `execution_path`; migration-column INSERT.
- `rust/src/agent/control_plane_metrics.rs` — `gate_outcome_counts`'s SQL
  gains a `NotApplicable` branch; new `GateOutcomeCountByPath`/
  `gate_outcome_counts_by_path`; new `e3_matrix_invariant_probe`/
  `g5_path_a_never_produces_not_applicable` tests; W3 test's 2-way split
  extended to 3-way (regression fix, §6); all pre-existing
  `build_shadow_decision_row` call sites updated for the new parameter.
- `rust/src/agent/control_plane_audit.rs` — 2 test call sites updated for
  the new parameter.
- `rust/src/dsl_v2/executor.rs` — Path B/C's new shadow-eval call site in
  `execute_verb_in_scope`.
- `rust/src/sequencer.rs` — Path A's 2 real call sites updated to pass
  `ExecutionPath::RunbookSequencer` explicitly.
- `rust/src/api/agent_routes.rs` — `ControlPlaneMetricsResponse` gains
  `gate_outcomes_by_path` (the production consumer for item 5's new query
  function).

Schema:
- `rust/migrations/20260713_control_plane_shadow_decisions_execution_path.sql`
  (new) — `execution_path` column + CHECK + index on
  `control_plane_shadow_decisions`. Applied to the local dev database this
  session (`psql -d data_designer -f ...`).

Tooling:
- `scripts/check-invariants.sh` — `gate_e3` gains the G5 per-(gate, path)
  matrix probe block, run alongside the existing path-blind probe.

Docs (this tranche's deliverables):
- `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-G5-GATE-APPLICABILITY-MATRIX-001.md` (new, DRAFT)
- `docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-G5-IMPL-001.md` (this file, new)

No changes to `EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.5.md`,
`EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md`, or the ownership ledger —
out of this session's scope (a future session should record G5's closure
in the ledger per standing rule 2, once this doc's matrix is ratified).

---

## 11. Build/test/clippy summary

- `cargo build --workspace --features database` — clean.
- `cargo build --workspace --all-targets --features database` — clean.
- `cargo build -p ob-poc-types --no-default-features` — clean.
- `cargo build -p ob-poc-control-plane` (default features) — clean.
- `cargo test -p ob-poc-control-plane --lib applicability` — 4/4 passed.
- `cargo test -p ob-poc --lib --features database` (full suite) — 2169
  passed, 0 failed, 206 ignored (matches G4's own session-end baseline).
- `cargo test -p ob-poc --lib -- test_plugin_verb_coverage` — 1 passed.
- Live-DB (`DATABASE_URL=postgresql:///data_designer`):
  - `e3_matrix_invariant_probe` + `g5_path_a_never_produces_not_applicable`
    — 2/2 passed, standalone and combined with the full `control_plane*`
    module.
  - Full `control_plane*` module — 34/0 (excluding the one pre-existing,
    confirmed-unrelated `e3_invariant_probe` expected-fail).
  - `g4_seam_admission_tests`/`t4_1_envelope_admission_tests` — 11/0 under
    `--test-threads=1` (parallel-execution row-offset flake investigated
    and confirmed pre-existing, not fixed — §7/§8).
  - `scripts/check-invariants.sh e2` — structural 0/4 failures (unaffected
    by this session).
  - `scripts/check-invariants.sh e3` — literal output in §7; G5's own
    matrix probe green, overall verdict blocked by pre-existing,
    unrelated G1/G2-scoped gaps.
- Clippy (`ob-poc`, `ob-poc-types`, `ob-poc-web`, `ob-poc-control-plane`,
  `--all-targets --features database -- -D warnings`) — same 14
  pre-existing findings reproduced via `git stash` both before and after
  this session's diff (compared directly, not eyeballed); zero new
  findings from this session's own changes.

No commit, no merge, no deploy, no push were performed.
