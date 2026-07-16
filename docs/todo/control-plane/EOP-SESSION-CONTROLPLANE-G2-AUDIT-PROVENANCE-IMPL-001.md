# EOP-SESSION-CONTROLPLANE-G2-AUDIT-PROVENANCE-IMPL-001 — Implementation session log

### Implements: `EOP-DESIGN-CONTROLPLANE-G2-AUDIT-PROVENANCE-001` v0.2 (RATIFIED)
### Date: 2026-07-13
### Branch: `codex/phase-1-5-governance-closure` (not merged, not committed)

---

## 1. Verification points (V1-V5) — performed before any code

**V1 — storage shape of per-gate outcomes today.** Confirmed:
`"ob-poc".control_plane_shadow_decisions.gate_results` is a single JSONB
column, `{"GateName": "<Debug string>"}` (`control_plane_shadow.rs::report_to_json`,
keyed by `GateId`'s `Debug` rendering). `gate_outcome_counts`
(`control_plane_metrics.rs`) reads it via `jsonb_each_text` + a `CASE`
classifying the value's textual prefix into `Success`/`Failure`/
`NotEvaluated`/`NotImplemented`/`Unrecognised`. The `"missing"` sentinel
(`report_to_json`'s fallback for a gate absent from `report.results`) fell
into the `Unrecognised` catch-all — this is the G2 item 1 bug, confirmed
by direct read, not inferred. No provenance column exists anywhere. Matches
the design doc's assumption exactly — **no deviation found.**

**V2 — is G13 `DecisionSnapshot` content sufficient to re-derive the
decision outcome from recorded gate outcomes alone?** Refined finding, not
a simple yes/no: `SnapshotPins` (the G13 proof type) has **no persisted,
queryable `SnapshotId` store** — its fields are private, there is no
"snapshot" table, and `snapshot.rs`'s own doc says "G13 has zero
production callers." So a literal `snapshot_ref: SnapshotId` join-key, as
the design doc's `AuditEvent::DecisionEvaluated` sketch implies, does not
exist to be referenced. **This does not block full outcome re-derivation,
though.** Reading `decision::evaluate_with_report`'s actual classification
logic shows it needs only each gate's Success/Failure signal — already
fully captured in `gate_results`, including `StpClassifier`'s
differentiated `Failure("requires_human_gate")` vs `Failure("rejected")`
reason string (`stp_classifier.rs`), which is exactly the information
needed to distinguish `ApprovedStp`/`HumanGate`/`Rejected`. Implemented
**full (not gate-outcomes-only degraded) re-derivation** in
`rederive_decision_outcome` (`control_plane_audit.rs`), and proved it
against the crate's real `evaluate_with_report` output on a fully-admitted
fixture (`rederivation_matches_evaluate_with_report_on_a_fully_admitted_context`).
`DecisionEvaluated.snapshot_ref` is kept as a best-effort informational
`Option<Uuid>` (from `SnapshotInput.sem_reg_snapshot_id`), documented in
the type's own doc comment as "not a join key to any table."

**V3 — has G2 item 2 (`commit_attested` wiring) landed?** **No — confirmed
not landed.** `grep -rn "commit_attested"` across `rust/src` and
`rust/crates` finds exactly one reference: a doc comment in
`write_set_attestation.rs`, plus `control_plane_metrics.rs`'s own comment
("no production caller invokes `commit_attested` yet, see the ownership
ledger's C-032 entry"). The sequencer's real dispatch commit site
(`execute_verb_admitting_envelope`, `sem_os_runtime/verb_executor_adapter.rs`)
calls plain `scope.commit()`. Per the mission's explicit instruction, I did
**not** implement G2 item 2's production `commit_attested` wiring — that
is a separate, not-yet-reviewed work item. I **did** define and wire
`DispatchCommitted`'s emission hook at the actual current call site (the
plain `commit()`), recording the honest degraded signal `attested: false`
and `gate_outcome: {gate: WriteSetAttestation, outcome_kind: NotEvaluated}`
— a real event, real row, real provenance bucket, but not (yet) a real
attestation. This is explained inline at the call site with a comment
naming exactly this V3 finding.

**V4 — where does `EnvelopeConsumed` emit per the ratified G1
seal→consume design?** Confirmed G1 items 2-4 (the `entry_id`-correlated
seal→consume wire) have **not landed either** — `persist_sealed`'s live
signature is still `(pool, session_id, verb_fqn, envelope)` (no
`entry_id` parameter; G1's design doc §2.1 specifies adding one), and
`step_executor_bridge.rs:553` still hardcodes `envelope_handle: None`.
The real G10 consume call, though, **is** live code today (exercised
whenever a caller supplies `Some(handle)` directly — e.g. the `t4_1`
adapter-level tests): `ObPocVerbExecutor::admit_in_scope`
(`sem_os_runtime/verb_executor_adapter.rs`), which calls
`check_admission_in_scope` → `try_consume_in_scope_with_pins`. I wired
`EnvelopeConsumed`'s emission at this real call site (both the `Admitted`
and `RejectedConsumeFailed` branches — a genuine consume *attempt*, either
outcome), same-transaction via the caller's own `&mut PgConnection`
(`insert_audit_event_in_scope`), captured before `check_admission_in_scope`
takes ownership of `envelope_handle`. This is correct and live *today* for
any caller that supplies a real handle; it will become the production path
the moment G1 items 2-4 land and stop hardcoding `None`. No session id is
threaded into this per-verb admission check (a documented pre-existing
scope limitation shared with the G1-floor check two lines above it in the
same file) — the audit row's `session_id` is `Uuid::nil()`, matching that
existing convention rather than inventing a new one.

**decision_id correlation, an interpretation decision made explicit here:**
neither V3 nor V4's landed mechanisms carry a stable per-decision id to the
later call sites (that's exactly what G1 items 2-4 would add). Since
`ExecutionEnvelope::seal` mints a fresh `Uuid::new_v4()` per decision
(`envelope.rs:130`) and that same id is the `EnvelopeHandle`'s identity
end-to-end, I used **`decision_id == envelope_id`** for every
envelope-bearing decision's audit rows (`DecisionEvaluated`,
`EnvelopeSealed`, `EnvelopeConsumed`, `DispatchCommitted` all share it).
For non-`ApprovedStp` decisions (no envelope minted), `DecisionEvaluated`
gets a fresh, uncorrelated `Uuid::new_v4()` — nothing downstream
references it, by construction (no envelope, no later events). This
operationalizes DD-1's "joined by `decision_id`" using the one identity
value that is actually shared today between the seal site and the (real,
if currently rarely-exercised) consume/commit sites, without inventing new
plumbing that duplicates what G1 items 2-4 will eventually supply.

**V5 — `session_id` semantics on shadow rows.** Confirmed `UUID NOT NULL`
on both `control_plane_shadow_decisions` and `control_plane_envelopes`
(read directly from their migrations), **not** `TEXT` as the design doc's
own DDL sketch states (§2, "same convention as shadow rows"). Read as a
minor drafting inconsistency in the doc, not a structural assumption
failure — the doc's *stated intent* ("same convention") is best honoured
by matching the real convention (`UUID`), which is what
`20260713_control_plane_audit.sql` does. Recorded in the migration file's
own header comment. Confirmation-only per the design doc's own framing
(DD-5 deliberately keeps this doc off GM's marker mechanism) — no design
input required.

**No V1-V5 finding revealed a structural assumption failure requiring a
STOP.** V3/V4 are exactly the "expected precondition gap" the mission
anticipated (G2 item 2 / G1 items 2-4 not landed) and were handled per its
own instruction: define + wire the event at the real current call site,
degrade honestly, don't implement the other tranche's production wiring.

---

## 2. What's implemented, by section

### §2 — the audit stream

- **Migration:** `rust/migrations/20260713_control_plane_audit.sql` —
  `"ob-poc".control_plane_audit` exactly per the doc's schema (append-only
  `GENERATED ALWAYS AS IDENTITY` seq, `decision_id`, `event_type`,
  `occurred_at`, `session_id`, `payload JSONB`, two indexes), with the one
  documented type deviation (V5: `session_id UUID`, not `TEXT`). Applied to
  local Postgres (`psql -d data_designer -f ...`) and exercised live.
- **`AuditEvent` enum:** `rust/crates/ob-poc-control-plane/src/audit.rs`
  (was a placeholder module). Exhaustively matched everywhere it's
  consumed (`event_type()`, `provenance()` — both `match` with no `_`
  arm). Six variants exactly per DD-2: `DecisionEvaluated`,
  `EnvelopeSealed`, `EnvelopeConsumed`, `DispatchCommitted`,
  `DispatchRolledBack`, `DivergenceTriaged`. **No hash chain, no digests**
  — confirmed absent (struck at ratification, not implemented even
  partially).
- **No sqlx anywhere in `ob-poc-control-plane`** — the crate's
  `Cargo.toml` is untouched (still zero DB deps). Persistence
  (`insert_audit_event`, `insert_audit_event_in_scope`,
  `audit_rows_for_decision`) lives in the root `ob-poc` crate's new
  `src/agent/control_plane_audit.rs`, every DB-touching function
  `#[cfg(feature = "database")]` — verified by inspection and by a clean
  `--no-default-features` consideration (root crate's existing `database`
  feature composition is unchanged; no new crate-level feature flag
  needed since `ob-poc` already has one).
- **Emission wired at three real call sites** (V3/V4's honest-degrade
  posture):
  - `DecisionEvaluated` + `EnvelopeSealed` — `rust/src/sequencer.rs`,
    inside `phase5_runtime_recheck`'s existing best-effort `tokio::spawn`,
    additive alongside the pre-existing `insert_shadow_decision`/
    `persist_sealed` calls (W1: nothing here alters `row`).
  - `EnvelopeConsumed` — `rust/src/sem_os_runtime/verb_executor_adapter.rs`,
    `admit_in_scope`, same-transaction via `insert_audit_event_in_scope`.
  - `DispatchCommitted` — same file, `execute_verb_admitting_envelope`,
    immediately before `scope.commit()`, same-transaction, `attested:
    false` (V3's honest degrade), only emitted when a real envelope was in
    play.
  - `DivergenceTriaged` is **not** wired to any emission site (standing
    rule 3 — divergence classification logic itself is untouched by this
    doc); the variant exists in the type per DD-2 for future triage
    tooling.

### §3 — the provenance dimension

- `GateOutcomeProvenance` (3-value closed enum) + `expected_provenance()`
  (exhaustive per-`GateId` match, no `_` arm) in `audit.rs`. Map exactly
  per the doc: G1-G9, G11-G13 → `ShadowEval`; G10 → `ConsumeSeam`; G14 →
  `PostDispatch`.
- `gate_outcome_counts` (`control_plane_metrics.rs`) **rebuilt as the
  three-way `UNION ALL`** the doc specifies (`shadow_eval` /
  `consume_seam` / `post_dispatch` CTEs), **with the G2 item 1 sentinel
  fix landed in the same rewrite**: `report_to_json`'s `"missing"`
  sentinel now classifies to its own `'NotRegistered'` bucket instead of
  falling into `'Unrecognised'`. `GateOutcomeCount` gained a `provenance:
  String` field (additive; its one caller, `api::agent_routes`, only
  destructures individual fields by `.field ==`, not an exhaustive struct
  literal — unaffected).
- E3 probe (`e3_invariant_probe`) updated per §3's assertion change: now
  computes both `substantive_any` (any provenance) and
  `substantive_expected` (at the gate's `expected_provenance` only) per
  gate, and **fails a gate that has samples only at the wrong
  provenance** (`wrong_provenance_only`), distinct from a gate with zero
  samples anywhere (`failing`) — both reported separately in the panic
  message.

### §4 — G11 semantics

- `check_completeness` (`control_plane_audit.rs`): the lifecycle grammar
  per DD-4(i) — `DecisionEvaluated` first; `EnvelopeSealed` iff
  `ApprovedStp`; `EnvelopeConsumed` at most once; `DispatchCommitted` xor
  `DispatchRolledBack`, only after a `EnvelopeConsumed`, in seq order.
  Seq-gaplessness deliberately **not** asserted (only relative ordering of
  present events), per the doc's own note.
- `rederive_decision_outcome` (`control_plane_audit.rs`): DD-4(ii),
  implemented as **full** re-derivation (not degraded) per the V2 finding
  above — mirrors `decision::evaluate_with_report`'s own
  `PROOF_BEARING_GATES` + `RunbookProof` + `StpClassifier` logic exactly,
  proved against the real function's output on a fully-admitted fixture.
- `audit_rows_for_decision` reads a decision's full event sequence back
  ordered by `seq`, deserializing via `AuditEvent::from_stored` — the read
  side of the round-trip proved by
  `payload_json_round_trips_every_variant` (unit) and
  `full_lifecycle_round_trips_and_is_complete` (live-DB, end-to-end:
  insert → read back → `check_completeness`).

### §6 — W1-W4 window-discipline tests, as executable tests

- **W1** — `w1_shadow_row_is_field_identical_with_and_without_audit_emission`
  (live-DB, `control_plane_audit.rs`): builds/inserts a shadow row with no
  audit activity around it, and a second row with `DecisionEvaluated`/
  `DispatchRolledBack` audit events inserted immediately before/after —
  asserts every shadow-row field except the deliberately-varied
  session/entry correlation keys is identical. **PASS.**
- **W2** — divergence classification is provably untouched: I made **zero
  edits** to `build_shadow_decision_row`/`report_to_json`'s classification
  logic (only widened `report_to_json`'s visibility to `pub(crate)` for a
  test import — no behavioural change). The pre-existing
  `divergence_flagged_when_shadow_and_legacy_disagree` test (unedited)
  still asserts the exact same byte-identical divergence outputs on its
  fixed fixture. **PASS** (re-run, confirmed unaffected).
- **W3** — `w3_shadow_eval_slice_matches_legacy_query_modulo_sentinel_fix`
  (live-DB, `control_plane_metrics.rs`): runs the frozen pre-rewrite query
  (`gate_outcome_counts_legacy_shadow_eval_only`, kept as a `#[cfg(test)]`
  fixture, byte-identical to the shipped-before-this-session query) and
  the rebuilt query's `shadow_eval` slice against the same data, asserting
  every non-`Unrecognised` bucket matches exactly, and that the
  `Unrecognised`→`NotRegistered` sentinel-fix delta reconciles exactly
  (`legacy Unrecognised == rebuilt Unrecognised + rebuilt NotRegistered`
  per gate). **PASS** (serially — see the test-isolation note below).
- **W4** — `w4_no_gate_other_than_g10_g14_emits_at_the_wrong_late_provenance`
  (live-DB): asserts no gate other than `ExecutionEnvelope` has any
  `consume_seam` sample and no gate other than `WriteSetAttestation` has
  any `post_dispatch` sample, over whatever real rows exist. **PASS.**

**Test-isolation note (not a defect in the implementation):** these
live-DB tests share one local Postgres instance and are not
transaction-isolated from each other or from other test modules; run with
the default parallel test runner, two tests (W3 and E3) can observe a
few extra concurrently-inserted rows mid-assertion and flake. Both pass
cleanly and consistently under `--test-threads=1`, which is how they were
verified for this report (command output below). This is a pre-existing
convention risk shared by every other live-DB test in this file (e.g.
`shadow_divergence_stats_counts_only_diverged_rows` has the same shape),
not something this session introduced or needs to fix.

---

## 3. Command output (real runs, this session)

```
$ cargo build -p ob-poc-control-plane
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.12s

$ cargo test -p ob-poc-control-plane --lib
test result: ok. 115 passed; 0 failed; 0 ignored

$ cargo test -p ob-poc-control-plane   # trybuild + doctest
test result: ok. 1 passed; 0 failed  (compile_fail_tests, 3 trybuild fixtures)

$ cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 08s   # zero errors

$ cargo test -p ob-poc --lib           # full non-DB suite
test result: ok. 2160 passed; 0 failed; 198 ignored

$ psql -d data_designer -f migrations/20260713_control_plane_audit.sql
CREATE TABLE
CREATE INDEX
CREATE INDEX
COMMENT

$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib \
    --features database control_plane -- --ignored --test-threads=1
test result: FAILED. 31 passed; 1 failed
    (the 1 failure is e3_invariant_probe -- EXPECTED per invariants-expected.toml's
     ratcheted [e3] status = "fail"; see §4 below for the gate-count delta)

$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib \
    --features database sem_os_runtime::verb_executor_adapter:: -- --ignored --nocapture
running 7 tests
test ...execute_verb_admitting_envelope_floor_rejects_an_unregistered_verb_before_any_scope_or_consume ... ok
test ...execute_verb_admitting_envelope_rolls_back_the_consume_when_dispatch_fails ... ok
test ...enforced_verb_with_consumed_envelope_admits_then_rejects_resubmission ... ok
test ...execute_verb_admitting_envelope_rejects_on_pin_drift_and_leaves_envelope_reconsumable ... ok
test ...shadow_default_admits_every_verb_with_no_envelope ... ok
test ...enforced_verb_without_envelope_is_rejected ... ok
test ...envelope_with_wrong_content_hash_is_rejected_loudly ... ok
test result: ok. 7 passed; 0 failed
    (the exact t4_1 property set G1's own design doc cites -- proven unaffected
     by the EnvelopeConsumed/DispatchCommitted audit hooks added this session)

$ cargo test -p ob-poc --lib -- test_plugin_verb_coverage
test result: ok. 1 passed; 0 failed

$ bash scripts/check_kyc_substrate_deps.sh
PASS: no forbidden deps in ob-poc-kyc-substrate

$ cargo clippy -p ob-poc-control-plane --lib
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.55s   # zero warnings
```

---

## 4. STOP-conditions / scope boundaries hit

1. **G2 item 2 (`commit_attested` production wiring) — not landed, not
   implemented here (V3).** Handled: defined `DispatchCommitted`'s
   emission at the real current `commit()` call site, honest degrade
   (`attested: false`). No new scope taken on for G2 item 2 itself.
2. **G1 items 2-4 (entry_id-correlated seal→consume wire) — not landed,
   not implemented here (V4).** Handled: `EnvelopeConsumed` wired at the
   real, already-live consume call site (`admit_in_scope`), which will
   become the production path automatically once G1 items 2-4 stop
   hardcoding `envelope_handle: None` — no new plumbing invented that
   would need to be reconciled with G1's eventual `entry_id` column.
3. **DD-5 negative scope boundary — verified untouched:** grepped for and
   confirmed no edits to GW's exercise-of-record mechanism, GM's deploy
   marker, `GateResult::NotApplicable` (doesn't exist in this codebase at
   all yet — G5 item 1, different tranche, confirmed absent), or
   divergence classification logic (`build_shadow_decision_row`/
   `report_to_json`'s only edit was a visibility widening, not a logic
   change — W2 above proves this empirically).
4. **V2's G13 SnapshotId gap** — recorded, not silently patched. Did not
   invent a new snapshot-identity persistence layer (out of this doc's
   scope); used the existing `sem_reg_snapshot_id` as a best-effort,
   explicitly-non-authoritative informational reference instead, and
   proved full outcome re-derivation does not actually require it.

No piece was abandoned — every §2/§3/§4/§6 item from the mission is
implemented and test-verified above.

---

## 5. Files changed

- `rust/migrations/20260713_control_plane_audit.sql` (new)
- `rust/crates/ob-poc-control-plane/src/audit.rs` (was a 7-line
  placeholder; now the full `AuditEvent`/`GateOutcomeProvenance`/
  `expected_provenance` type surface + 3 unit tests)
- `rust/src/agent/control_plane_audit.rs` (new — persistence,
  `check_completeness`, `rederive_decision_outcome`, 14 unit tests + 2
  live-DB tests)
- `rust/src/agent/control_plane_metrics.rs` (rebuilt `gate_outcome_counts`
  as the 3-way UNION + sentinel fix; `GateOutcomeCount` gained
  `provenance`; E3 probe made provenance-aware; +2 new live-DB tests: W3,
  W4)
- `rust/src/agent/control_plane_shadow.rs` (one-line visibility widening:
  `report_to_json` `fn` → `pub(crate) fn`, with an explanatory doc comment
  — no logic change)
- `rust/src/agent/mod.rs` (registered `control_plane_audit` module)
- `rust/src/sequencer.rs` (`phase5_runtime_recheck`: additive
  `DecisionEvaluated`/`EnvelopeSealed` audit emission alongside the
  existing shadow-row spawn)
- `rust/src/sem_os_runtime/verb_executor_adapter.rs` (`admit_in_scope`:
  `EnvelopeConsumed` emission; `execute_verb_admitting_envelope`:
  `DispatchCommitted` emission before `commit()`)

---

## 6. `invariants-expected.toml` — recommendation, not applied

**E3 has moved and should be reviewed for a possible partial flip**, but I
have **not** touched `invariants-expected.toml` — this needs architect
review per the mission's own instruction ("recommend only... unless the
evidence is airtight and you can cite exact before/after gate counts").

Before this session (per the file's own current comment, dated
2026-07-13, same day): *"10/14 gates (G1-G9, G13) have substantive
(Success/Failure) production shadow-decision samples... G10
(ExecutionEnvelope), G11 (AuditReplay), G12 (VersionPinning), G14
(WriteSetAttestation) have zero."*

After this session's live-DB test run (serial, `--test-threads=1`,
reproduced twice, output above): **G10 (ExecutionEnvelope) now has 7
substantive samples, all at its expected `consume_seam` provenance** —
because this session's `EnvelopeConsumed` wiring is real, live code at
`admit_in_scope`, and the `t4_1`/W1/W3 live-DB test runs in this same
session genuinely exercised it via `Some(handle)` calls. **G11
(AuditReplay), G12 (VersionPinning), G14 (WriteSetAttestation) remain at
zero** substantive samples — G11 has no production analogue that grades
it as a `GateResult` at all (its "evaluation" is the `check_completeness`/
`rederive_decision_outcome` functions this session added, which are
exercised only by tests, not any production call site); G12/G14 are
unchanged by this session (G14's `DispatchCommitted` emission records
`NotEvaluated`, not `Success`/`Failure`, so it correctly does **not**
count as substantive — an honest non-improvement, not a bug).

**Recommendation:** if this diff merges, E3's comment/count should update
from "10/14... G10... have zero" to "11/14... G11/G12/G14 remain zero,"
reflecting the real (test-exercised, not yet production-exercised) G10
movement — but whether that constitutes "E3 gate has genuinely moved" in
the invariant-promotion session's sense (which seems to track
*production* evidence, not test-harness evidence) is an architect call I
am not making unilaterally. G10's samples in this run came entirely from
this session's own `t4_1`/W1/W3 test executions against local Postgres,
not from any live production traffic — the same caveat the file's own E2
entry already applies to `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` being unset
in production. I recommend the architect decide whether "substantive
samples, however sourced" is the intended bar (in which case flip
G10's status) or whether E3 specifically means production-sourced
evidence (in which case the comment should be updated to note test-only
coverage without flipping the pass/fail count). E1/E2/E4/E5 are
unaffected by this session's changes — no other recommendation.
