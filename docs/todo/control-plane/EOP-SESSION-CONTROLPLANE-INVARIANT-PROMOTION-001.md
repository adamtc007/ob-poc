# Session: EOP-PLAN-CONTROLPLANE-001 — Invariant Promotion (pre-T0)

### Status: COMPLETE (2026-07-13). All 6 phases landed; final gate green. Evidence: `EOP-SESSION-CONTROLPLANE-INVARIANT-PROMOTION-001-evidence-2026-07-13.txt` (verbatim `check-invariants.sh all` output, exit 5 — all 5 invariants correctly report DOES NOT HOLD today).

## Summary (for blind review)

**1. Per-invariant status, mechanism, and today's result** — see §1 below for
the full predicate restatement with ground-truth citations. Short form:

| Invariant | Mechanism | Today | Detail |
|---|---|---|---|
| E1 | `scripts/check-invariants.sh e1` — enumerates all 45 RR-3 CIDs, requires ledger status literally `**CLOSED**` (not partial) + commit hash + a symbol that resolves via `rg` | **FAIL** | 3/45 provably closed (C-022, C-030, C-037); C-001 claimed CLOSED but missing a commit-hash citation, correctly counted unproven |
| E2 | same script `e2` — structural: grep each of RR-2's 4 paths for a call to `execute_verb_admitting_envelope`; dynamic: runs the existing live-DB `t4_1_envelope_admission_tests` (7 tests) | **FAIL** | 2/4 paths (A, D) wired structurally, B/C have no admitting call at all; dynamic evidence proves Path D's mechanism works but is `NotEnforced` by production default |
| E3 | new `#[test] e3_gate_label_match_is_exhaustive` (compile-time, no `_` arm — a 15th `GateId` breaks it) + new `#[ignore] e3_invariant_probe` (live query over `gate_outcome_counts`) in `rust/src/agent/control_plane_metrics.rs` | **FAIL** | 10/14 gates have substantive production samples; G10 (ExecutionEnvelope), G11 (AuditReplay), G12 (VersionPinning), G14 (WriteSetAttestation) have zero |
| E4 | same script `e4` — 5 RR-5 Mode-1 rows (schema imposed by this gate, flagged §"schema" below), each needs a real (non-test, non-comment) call site for its pin symbol OR a named human-gate test | **FAIL** | 1/5 rows satisfied (`toctou_entity_tables`, `verify_pins_in_scope` genuinely called in `verb_executor_adapter.rs:582`); the other 4 have neither |
| E5 | same script `e5` — `cargo build`/`cargo test`, `scripts/check-public-api-surface.sh`, `#![deny(unreachable_pub)]` crate-declaration census | **FAIL** | build/test green; surface ratchet trips on 5 crates (unrefreshed baselines spanning several already-landed tranches), `ob-poc-agent` surface measurement errors, 4 crates fail `--no-default-features` builds (pre-existing feature gap) |

**2. Production-code changes, individually, with why the gate required them:**

- `rust/src/agent/control_plane_metrics.rs` — added `e3_gate_label_match_is_exhaustive` and `e3_invariant_probe` inside the existing `#[cfg(all(test, feature = "database"))] mod t7_2_metrics_tests`. **Test code only, not production logic** — required because E3's own spec text demands an exhaustive-match compile-time proof plus a live query against the already-existing (T7.2) `gate_outcome_counts` function, and `gate_outcome_counts` is `pub(crate)` — only reachable from an internal test in this exact module, per the repo's own test-boundary rule (external `rust/tests/` harnesses can't see `pub(crate)` items).
- No other production code was touched. E1/E2(structural)/E4/E5 needed no instrumentation point — they're pure ground-truth cross-references (ledger text, RR-2/RR-5 doc content, existing `execute_verb_admitting_envelope`/`verify_pins_in_scope` call sites, existing CI scripts) reachable via `rg`/`awk`/existing test suites. E2's dynamic half reuses the pre-existing `t4_1_envelope_admission_tests` live-DB suite (T4.1/T10.2) unmodified.

**3. Schema imposed on the ledger/registers (flagged, not silently assumed to pre-exist):**

- **E4 only.** RR-5's Mode-1 register (`docs/research/control-plane-phase0-inventory.md` §RR-5) is 5 rows of free-text prose with no id column. This gate assigns a short slug to each row (`shadow_envelope_entities`, `toctou_entity_tables`, `bus_operational_writes`, `bpmn_process_instances`, `raw_dsl_best_effort`) plus a proposed `(pin-symbol, human-gate-test-name)` pair per row, hardcoded in `scripts/check-invariants.sh::gate_e4`. This is new structure imposed on an artifact that didn't have it — RR-5 itself is untouched. Not applied to E1 (RR-3 already has a real `C-0xx` id column, no schema needed) or E3 (`GateId` is already a real enum, `gate.rs`'s own doc comment is the closest thing to a legend and was used as-is, not re-invented).

**4. Dependencies added:** none. `cargo-public-api` (used by the pre-existing `check-public-api-surface.sh`, not newly required by this session) was already installed locally; CI's `public-api-surface.yml` already installs it explicitly for that job. `scripts/check-invariants.sh` uses only `bash`, `awk`, `rg`, `cargo` — all already present in this repo's toolchain.

**5. Open judgement calls deferred to review:**

- **E5's premise was wrong, corrected empirically (Phase 1).** The session brief assumed E5 passes today ("workspace hygiene already holds"). Running the real gate found it fails — 5 crates have public-API baseline drift from already-landed tranches (T4/T5/T9.7, plus this session's own earlier T11.1b `ob-poc-types::intent` addition), `ob-poc-agent`'s surface measurement errors, and 4 crates fail isolated builds. **Not fixed here** — refreshing baselines and closing Cargo.toml feature gaps is real engineering, out of a gate-authoring session's scope per its own constraints. `invariants-expected.toml`'s `[e5]` is set to `fail`, not the originally-assumed `pass` — this itself is the kind of expectation-flip decision the ratchet's own design says should be visible in a diff, and it is, right here.
- **"T0 as next tranche" is stale** (§0). T0-T7 (the v0.1 plan) and a further T8-T11.2 (a second, referenced-but-textually-absent `EOP-PLAN-CONTROLPLANE-002` track) are already landed. Whoever picks up next should choose the real next increment from the ledger's own open items, not assume T0.
- **E2's dynamic half only drives Path D.** Paths A/B/C either have no admitting entry point at all (B, C — proven by absence, no dynamic test needed to prove a negative that's structurally absent) or (Path A) have the call site but no dedicated dynamic harness was written this session to drive it end-to-end live — deferred rather than standing up 3 more per-path integration harnesses in a gate-authoring session. Flagged, not hidden.
- **Phase 4(a)'s visibility-tightening was correctly not attempted**, per the architect's own preemptive note: the structural check landed as a script-only check (grep for the admitting call), with `pub(in)` visibility restriction on the RR-2 path entry points deferred to whichever tranche reroutes the non-compliant callers (B, C) — attempting it now would break current callers.
- **A pre-existing metrics-classification gap surfaced, not fixed:** `gate_outcome_counts`'s SQL (T7.2) classifies `report_to_json`'s `"missing"` sentinel (written when a gate wasn't yet registered in `evaluate_shadow`'s map at persist time — e.g. historical rows predating T9.7's G9/G12 addition) as `'Unrecognised'`, not distinguished from a genuine unrecognised value. This under-counts G12 specifically in the local dev DB sample (VersionPinning shows 0 substantive rows even though the adapter itself is real) — flagged in §1's E3 detail, not corrected (modifying T7.2's existing SQL is out of this session's scope).

## Review response (2026-07-13, remediation commit)

Reviewer verdict: accept, with four findings, two required before the next
tranche relies on these gates. All four addressed in this remediation pass.

**Corrections accepted, no action needed** — the stale-plan finding and E5's
premise correction were both endorsed as the governing principle working as
intended; no changes made in response to those beyond what already stood.

**Finding 1 — E2 proved presence, not exclusivity (addressed).** A path
could pass by having an admitting call in one branch while a bare
`execute_verb()` call stood open in another. `gate_e2` in
`scripts/check-invariants.sh` now checks both: the admitting call's
file:line locus (also closes the verifiability gap the reviewer named —
E1/E4 already printed resolve loci, E2's structural half didn't), AND zero
bare `execute_verb(` call sites in the same files (comment lines and `fn
execute_verb(` trait-mock definitions excluded from the bare-call count, so
a doc reference or test-double impl can't trip a false fail). Re-run
confirms Path A and D still pass on the exclusivity bar (no bare call sites
found in either), Paths B/C still fail outright (no admitting call at all —
unaffected by this change).

**Finding 2 — Path A's pass, unexplained (addressed).** Confirmed by `git
log -L` on the admitting call site: commit `5a704f4e` ("PIR-D-002 — Path A
now reaches the admission port"), governed by a graduation runbook this
session's ground-truth pass had not surfaced —
`docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` v0.2 —
which independently establishes a per-path graduation order (Path A first,
then B, then C) and records Path A's coverage today as G1-only (not all 14
gates). The match is a real production call site, not a comment/test
artifact (`rust/src/runbook/step_executor_bridge.rs:553`, inside the live
dispatch method, not the test module's mock at line 861). `gate_e2` now
cites this provenance inline. This also means the recommended next-tranche
sequencing in the original summary was working from an incomplete picture
— the graduation runbook already has its own documented order and open
items (§8), which should be consulted ahead of any fresh sequencing
recommendation.

**Finding 3 — E3's expected-fail could mask a broken probe (addressed).**
`e3_invariant_probe` (`rust/src/agent/control_plane_metrics.rs`) now panics
with a distinct `E3_INFRASTRUCTURE_FAILURE` marker for DB-connect/query
failures, versus `E3_INVARIANT_FAILURE` for a real, verified, substantive
result. `gate_e3` in the script greps captured test output for these
markers and prints an explicit, loud distinction — an infra failure is
flagged as "NOT proof the invariant fails" and as something needing
immediate attention if it ever shows up once a live DB is wired into CI.
Verified both branches locally: a bad `DATABASE_URL` produces the
infrastructure-failure banner; the real dev DB produces the
invariant-failure banner with the same 4/14-gate detail as before.
`invariants-expected.toml`'s `[e3]` comment now carries this caution
explicitly, since CI has no live DB today and this distinction only bites
once one is added.

**Finding 4 — E4's mapping is now normative by accident (addressed,
option: marked provisional).** Chose "mark provisional" over "ratify now"
— ratifying five invented names as the target contract in a gate-authoring
session would itself be the kind of scope creep the session's own
constraints warn against (this session's mandate is gates, not designing
the pin mechanism). `scripts/check-invariants.sh`'s `gate_e4` comment now
states explicitly that renaming a row's target is a spec change requiring
the same review visibility as an `invariants-expected.toml` status flip —
not a routine script edit.

**Re-verification after remediation:** `cargo build --workspace --features
database` clean; `cargo clippy -p ob-poc --lib --features database -- -D
warnings` clean; `cargo test -p ob-poc --lib --features database` 2146
passed/0 failed/194 ignored (unchanged); `DATABASE_URL=... check-invariants.sh
all` exit 5 — all 5 invariants still correctly report DOES NOT HOLD,
confirming the exclusivity and infra/invariant-distinction changes didn't
flip any status. Evidence file overwritten with the post-remediation run.

## 0. Headline finding — corrects the premise this session was framed against

The plan (`EOP-PLAN-CONTROLPLANE-001_Implementation-Plan_v0.1.md`) is **not**
"authored-but-unexecuted." Tranches T0 through T7 are landed (T0: 4 hard
fences; T1: crate skeleton, proof types, dependency-declared evaluator; T2:
six gate adapters G1/G3/G4/G5/G6/G7; T3: three missing-analogue gates
G2/G8/G13 plus the `ControlPlaneProof` aggregate; T4: envelope admission
mechanism (G10) + pin verification + G12 version set; T5: write-set
attestation mechanism (G14) at `PgTransactionScope`; T6: two of five
bypass-closure sub-tranches; T7: metrics-only, explicitly incomplete),
and beyond the v0.1 plan, an entire second track (T8-T9.7, T10.1-T10.3,
T11.0-T11.2) has also landed under a referenced-but-absent
`EOP-PLAN-CONTROLPLANE-002` (mesh-retirement/leakproofing scope).

This does not change this session's task (E1-E5 promotion is still
correct, orthogonal work — the invariants measure the *whole* plan's
completion, regardless of how much has landed), but it means "T0 as a
pilot tranche" is no longer available as a next step: T0-T7 are done, and
the ownership ledger (2026-07-10, T7 entry, line ~103) already contains an
honest, prose, by-hand check of E1-E5 against the codebase as it stood
then: **E1 false (~24 rows open), E2 false (zero paths in enforce mode),
E3 false (most gates `NotImplemented`/`NotEvaluated` at every real call
site), E4 not verified, E5 green for the touched subset only.** This
session's job is to make that same check re-runnable and sentinel-proof,
not to re-discover it by hand.

Flagged for the architect, not decided here: the real next tranche once
this session closes is whichever of T2.7-adjacent call-site wiring,
T6.3-T6.5 bypass closure, or T9/T11's own continuation makes sense next —
not "T0."

## 1. Per-invariant restatement (checkable predicates)

**E1** — *every RR-3 C-0xx row is CLOSED in the ownership ledger.*
Ground truth: `docs/research/control-plane-phase0-inventory.md` §RR-3 (45
rows, `C-001`..`C-045`, the canonical row-id set) cross-referenced against
`docs/research/control-plane-ownership-ledger.md` §Ledger (the same 45
CIDs, each with a `Status` cell). Checkable predicate: for every CID in
RR-3, a ledger row exists AND that row's status cell literally contains
`**CLOSED**` (not `**PARTIALLY CLOSED**`) AND the status text names a
disposition class (moved/invoked/retired/split) AND cites a commit hash
AND cites at least one destination symbol that resolves in the current
workspace. Per the ledger's own running totals (line 82/88): 4 rows meet
the CLOSED bar today (C-001, C-022, C-030, C-037) of 45 — expect FAIL.

**E2** — *all four RR-2 paths execute only via envelope admission in
enforce mode.* Ground truth: `phase0-inventory.md` §RR-2 defines exactly
four paths (A: utterance→Sage/SemOS→runbook→runtime→DB write via
`ObPocVerbExecutor::execute_verb`, `sem_os_runtime/verb_executor_adapter.rs:L117-L286`;
B: raw DSL via `agent_routes.rs` execute handler; C: BPMN/workflow via
`bpmn_integration/dispatcher.rs` + `bpmn_controller_ops.rs`; D: bus via
`ob-poc-web/src/bus_runtime.rs:L122-L149`). Checkable predicate:
structurally, every path's entry point resolves to
`VerbExecutionPort::execute_verb_admitting_envelope` (not the bare
`execute_verb`) as its only route to the mutation terminus; dynamically,
`OB_POC_CONTROL_PLANE_ENFORCE_VERBS` (`sem_os_runtime/verb_executor_adapter.rs`)
is non-empty for the verb under test and admission without a consumed
envelope is rejected. Per the ledger (T6, T7 entries): only Path D (bus)
calls the admitting entry point at all (T6.1), and `ENFORCE_VERBS` is
unset by production default everywhere — expect FAIL, and expect the
structural half to fail for Paths A/B/C outright (no admitting-entry-point
call site exists for them yet).

**E3** — *G1-G14 each evaluated in production (not `NotImplemented`) with
metrics flowing.* Ground truth: `GateId::ALL` (`ob-poc-control-plane/src/gate.rs:L44-L59`,
14 variants — this is the canonical registry; no separate G1-G14 legend
exists elsewhere in the docs, `gate.rs`'s own doc comment at L12-L23 is
the closest thing to one and matches RR-3/RR-8's usage exactly) is the
exhaustive gate set. `agent::control_plane_metrics::gate_outcome_counts`
(`rust/src/agent/control_plane_metrics.rs`) is the real, live-DB-backed,
already-wired metrics query the T7.2 tranche built for exactly this
question. Checkable predicate: for every `GateId`, at least one production
shadow evaluation (`control_plane_shadow_decisions` row, via
`agent::control_plane_shadow::build_evaluation_context` at the Sequencer
stage-6 call site) produced a `Success` or `Failure` (not
`NotImplemented`/`NotEvaluated`) for that gate, AND `gate_outcome_counts`
returns a non-zero sample for it. Per the ledger (T7 entry): most gates
have no real call-site input source, so most would show
`NotImplemented`/`NotEvaluated` dominating — expect FAIL.

**E4** — *Mode-1 register (RR-5) rows either version-pinned or permanently
classified human-gated with the classification tested.* Ground truth:
`phase0-inventory.md` §RR-5 Mode-1 register, 5 rows (shadow envelope
resolved entities; entity tables intended for TOCTOU; bus-invoked
operational writes; BPMN `process_instances`; raw DSL best-effort
execution). Checkable predicate: for each of the 5 rows, either (a) a
named lockfile/register entry the gate can verify pins a real version, or
(b) a named `#[test]` function exercising a human-gate code path for that
family, enumerated from the register text itself (row name → expected
test name mapping), with an unmatched row on either side failing. No
existing test inventory maps 1:1 to these 5 rows today (T7's own entry:
"E4 not verified this tranche") — expect FAIL.

**E5** — *workspace green: `cargo build && cargo test` all crates; public
API surface gate green; `unreachable_pub` clean.* Ground truth:
`scripts/check-public-api-surface.sh` (existing, wired into
`.github/workflows/public-api-surface.yml`) plus the two cargo commands.
Checkable predicate: all three exit 0. Per this session's own most recent
work (T11.2 commits) and the T7 ledger entry's own subset check: expect
PASS.

**E5 correction, found empirically in Phase 1 (2026-07-13): the PASS
premise above is wrong.** `scripts/check-public-api-surface.sh` was run
for real and fails today, on all three of its own checks: (1) RATCHET —
5 crates have public-API drift against their committed
`audits/surface/*.txt` baselines that was never refreshed when the
underlying tranche landed: `dsl-runtime` (T5 write-set attestation:
`execute_crud_in_scope`, `TransactionScope::record_write`),
`ob-poc-control-plane` (T9.7: `RunbookProofGate`, `VersionPinningGate`,
`EnvelopeRecord`), `ob-poc-boundary` (T4.3: `verify_pins_in_scope`),
`ob-poc` (pre-existing `tower_http::PolicyExt` blanket-impl noise plus
this session's own T11.1b `pub use ob_poc::semtaxonomy_v2`), and
`ob-poc-types` (this session's own T11.1b `intent` module — the one
baseline gap actually introduced by prior work *in this same session*,
not a different tranche). (2) MEMBRANE — `ob-poc-agent`'s surface
measurement errors outright (root cause not dug into further, out of
scope for a gate-only session); 4 unrelated crates
(`ob-poc-derived-attributes`, `ob-poc-entity-linking`, `ob-poc-taxonomy`,
`ob-poc-trading-profile`) fail to build at `--no-default-features` — a
pre-existing Cargo.toml feature-declaration gap, not new drift. (3) the
test-double-leak check never runs because the membrane check aborts
first. None of this is caused by anything in this invariant-promotion
session's own gate code — it is real, pre-existing drift the honest gate
correctly surfaces. Per the session's own scope constraints ("this
session produces gates only... does NOT make the invariants pass"),
**not fixed here** — refreshing 5 baselines and 4 crates' feature
declarations is real engineering work belonging to whichever tranche (or
a dedicated housekeeping pass) owns it, not a gate-authoring session.
**Corrected expectation: E5 = FAIL today, not PASS** — carried into
Phase 6's `invariants-expected.toml` as the honest baseline. This is the
same class of premise-correction as §0's "T0 pilot" finding: the session
brief's assumption about current state was wrong, caught by actually
running the check rather than assuming.

## 2. Ground-truth gaps found (flagged, not silently resolved)

- No single canonical "G1 = Intent Admission" legend exists as a
  standalone table anywhere in the docs; the mapping is reconstructed from
  `gate.rs`'s own doc comment (which is accurate and already used as the
  canonical registry by this session, not re-derived from RR-3/RR-8 prose
  independently).
- RR-9's own "Completion invariant check" paragraph (phase0-inventory.md
  line 230) maps E1-E5 to different claims than the plan's actual §
  "Completion invariant (E)" text — a pre-existing doc-drift, noted, not
  corrected (out of this session's scope per the constraint against
  redesigning inventoried artifacts).
- The running-totals arithmetic in the ledger (line 91) already flags its
  own inconsistency ("this range still nominally includes C-022 and
  C-037... a pre-existing bookkeeping gap"). E1's gate does not trust this
  prose arithmetic — it recomputes from the row statuses directly.
