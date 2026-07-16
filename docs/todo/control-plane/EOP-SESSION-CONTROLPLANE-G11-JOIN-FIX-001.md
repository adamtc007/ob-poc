# EOP-SESSION-CONTROLPLANE-G11-JOIN-FIX-001 — Implementation session log

### Target: a confirmed correctness bug in G11/`AuditReplay`'s DD-4(ii) re-derivation join (`control_plane_audit.rs::replay_grade_for_decision`)
### Date: 2026-07-14
### Branch: `codex/phase-1-5-governance-closure` (not merged, not committed by this session)

---

## 0. Verdict up front

**Confirmed real, fixed.** The bug-report hypothesis was correct in
substance and is the fix landed: `control_plane_shadow_decisions` had no
column that was genuinely unique per shadow-eval *attempt* —
`entry_id` is the `RunbookEntry`/`CompiledStep`'s own stable id
(`sequencer.rs::phase5_runtime_recheck`'s `entry_id` parameter), reused
across every retry/re-check of the *same* runbook step. DD-4(ii)'s
re-derivation join (`WHERE entry_id = $1 LIMIT 1`, no `ORDER BY`/
tiebreaker) could therefore non-deterministically retrieve a *different*
attempt's `gate_results` when grading a `DecisionEvaluated` audit event,
producing a false `Success` or false `Failure` grade for the
G11/`AuditReplay` metric.

The fix does **not** need a new field on the `DecisionEvaluated` audit
payload, and does not need a new mint site. The genuinely
per-attempt-unique value already exists: `control_plane_audit`'s own
`decision_id` column (`envelope.id()` for `ApprovedStp`, a fresh
`Uuid::new_v4()` otherwise — minted once per shadow-eval attempt at
`sequencer.rs`'s two call sites) already groups exactly one attempt's
audit rows, and `replay_grade_for_decision(pool, decision_id)` already
receives it as a parameter. The fix threads that SAME value onto a new
`control_plane_shadow_decisions.decision_id` column at insert time, and
switches the DD-4(ii) join to filter on `decision_id` instead of
`entry_id`. No new UUID minting, no payload schema change — just
correlating two rows that were always minted from the same value but
never had a column to carry the correlation on the shadow-row side.

**Verified with a real RED→GREEN reproduction**, not asserted: the new
regression test (`same_entry_id_retried_attempts_each_join_to_their_own_gate_results_not_the_others`)
fails against the exact pre-fix `entry_id`-only join (reintroduced
temporarily, then reverted) and passes against the fix — see §4.

`invariants-expected.toml` is **untouched** (recommend-only, per this
program's own discipline). This fix moves nothing in `[e3]`'s pass/fail
counts on its own — confirmed, not assumed, in §5: `[e3]` stays `fail`
(`WriteSetAttestation` remains the sole zero/wrong-provenance gate,
unrelated to this fix and unchanged by it) both before and after.

---

## 1. Investigation — verifying every claim in the bug report against real source

### 1.1 The buggy join, confirmed at its current location

Read `rust/src/agent/control_plane_audit.rs`'s
`replay_grade_for_decision` (the join was at line 450 pre-fix, not 427 —
the file had shifted since the bug report was written, confirmed by
direct read, not assumed):

```rust
Some(AuditEvent::DecisionEvaluated { outcome, entry_id, .. }) if *entry_id != Uuid::nil() => {
    let gate_results: Option<(serde_json::Value,)> = sqlx::query_as(
        r#"SELECT gate_results FROM "ob-poc".control_plane_shadow_decisions WHERE entry_id = $1 LIMIT 1"#,
    )
    .bind(*entry_id)
    .fetch_optional(pool)
    .await?;
    ...
}
```

No `ORDER BY`, no tiebreaker — exactly as reported.

### 1.2 Confirming `entry_id` is not unique in `control_plane_shadow_decisions`

Read `rust/migrations/20260710_control_plane_shadow_decisions.sql`
directly: the table's PK is a bare `BIGINT GENERATED ALWAYS AS IDENTITY`
(`id`), `entry_id UUID NOT NULL` carries no `UNIQUE` constraint, and
there is no `decision_id` column at all in the original table (it was
added piecemeal by later migrations for other columns —
`execution_path` in `20260713`, `floor_rejected`/`floor_gate`/
`floor_reason` in `20260712` — none of them `decision_id`). Confirmed
directly against `psql -d data_designer -c '\d "ob-poc".control_plane_shadow_decisions'`
before this session's migration: `entry_id` present, no `decision_id`.

### 1.3 Confirming `entry_id` really is reused across retries — the real production call site

Read `rust/src/sequencer.rs::phase5_runtime_recheck` (the primary G11
emission site) end to end. `entry_id` is a function parameter set once
per call to `RunbookEntry.id`/`CompiledStep.step_id` — the SAME value
across every re-invocation of this function for the same runbook step
(retries after `RequiresHumanGate`, or any other re-check path that
calls `phase5_runtime_recheck` again for the same entry). Each call
builds and inserts a fresh `ShadowDecisionRow` via
`build_shadow_decision_row(session.id, entry_id, ...)` — confirmed no
idempotency/upsert guard exists on this insert (`insert_shadow_decision`
is a plain `INSERT`, best-effort, never an `ON CONFLICT`). So two
distinct rows sharing one `entry_id` but carrying different
`gate_results` is not a hypothetical — it is the direct, unavoidable
consequence of retrying the same step through the existing, unchanged
retry path.

### 1.4 The real per-attempt-unique value — where it already lives

Read `rust/crates/ob-poc-control-plane/src/audit.rs`'s `AuditEvent`
definition and `rust/src/agent/control_plane_audit.rs`'s
`insert_audit_event`/`audit_rows_for_decision`: `control_plane_audit`
(migration `20260713_control_plane_audit.sql`) has its own `decision_id`
column, and `audit_rows_for_decision(pool, decision_id)`'s `WHERE
decision_id = $1` is exactly how every existing G11 test already groups
one decision's audit rows. `replay_grade_for_decision`'s own signature
is `async fn replay_grade_for_decision(pool: &sqlx::PgPool, decision_id: Uuid)`
— the correct join key was **already a parameter in scope** at the join
site; the bug was using `entry_id` (extracted from the event payload)
instead of the function's own `decision_id` argument.

Read `sequencer.rs`'s two `DecisionEvaluated` construction sites (line
~8079 in `phase5_runtime_recheck`, line ~8367 in the HumanGate-resume
reseal path) and their own doc comments (`decision_id: the audit
stream's correlating key... is the envelope's own id... For
non-ApprovedStp decisions... a fresh id is used`). Confirmed: `decision_id`
is computed inline, used only to key the two `insert_audit_event` calls
inside the `tokio::spawn`, and was **never threaded back** to the
`ShadowDecisionRow`/`build_shadow_decision_row` call a few statements
earlier (which only had `entry_id` to work with). This matches the bug
report's own citation of the session doc's phrasing almost exactly
(`envelope.id()` for `ApprovedStp`, a fresh id otherwise) — the value
the doc described already exists in the code; it just wasn't persisted
on the shadow-decisions row.

### 1.5 What "fixed" actually required — narrower than the bug report's opening hypothesis

The bug report's hypothesis (§3, "the correct fix is almost certainly...")
proposed adding a `decision_id` column, populating it with the SAME
value the audit event uses, and joining on it. That is exactly what
landed — confirmed correct, not revised. One simplification found during
implementation: `replay_grade_for_decision` did not need to read
`entry_id` out of the `DecisionEvaluated` payload for the join at all,
because it already receives `decision_id` as its own function parameter
(§1.4) — no new field needed on the audit *event* payload itself, only
on the *shadow-decisions row*. `entry_id` on `DecisionEvaluated` remains
in place, unchanged, as informational correlation to the originating
`RunbookEntry` — it was simply never the right join key for this
purpose.

---

## 2. The fix

### 2.1 Migration — `rust/migrations/20260714_control_plane_shadow_decisions_decision_id.sql`

```sql
ALTER TABLE "ob-poc".control_plane_shadow_decisions
    ADD COLUMN decision_id UUID;

CREATE INDEX idx_control_plane_shadow_decisions_decision_id
    ON "ob-poc".control_plane_shadow_decisions (decision_id)
    WHERE decision_id IS NOT NULL;

COMMENT ON COLUMN ... -- full rationale, see the file
```

Nullable, no backfill — same posture as G1's `entry_id` addition to
`control_plane_envelopes` (`20260713_control_plane_envelopes_entry_id.sql`)
and G5's `execution_path` addition to this same table
(`20260713_control_plane_shadow_decisions_execution_path.sql`): existing
rows predate the fix and have no real `decision_id` to backfill (their
originating audit event, if any, cannot be retroactively identified
from the shadow row alone). Applied directly against the dev DB
(`psql -d data_designer -f migrations/20260714_...sql`) — confirmed via
`\d "ob-poc".control_plane_shadow_decisions` that the column and partial
index landed.

### 2.2 `ShadowDecisionRow` / `build_shadow_decision_row` (`control_plane_shadow.rs`)

Added `pub decision_id: Option<Uuid>` to `ShadowDecisionRow`, added
`decision_id: Option<Uuid>` as a new parameter to
`build_shadow_decision_row` (positioned right after `entry_id` — the two
are paired "which attempt" concepts), and added it to
`insert_shadow_decision`'s `INSERT` column list/binds. `#[allow(clippy::too_many_arguments)]`
added (this function already had 6 params; a 7th crosses clippy's
default threshold).

### 2.3 `sequencer.rs` — both call sites

In both `phase5_runtime_recheck` and the HumanGate-resume reseal path,
moved the `decision_id` computation (`envelope.id()` for `ApprovedStp`,
peeked via `&decision` without moving it; `Uuid::new_v4()` otherwise) to
**before** `build_shadow_decision_row` is called, and:

- Passed `Some(decision_id)` into `build_shadow_decision_row`.
- Replaced the `tokio::spawn` block's own independent
  `let decision_id = envelope.id();` / `Uuid::new_v4()` recomputations
  with reuse of the same outer binding (`Uuid` is `Copy`, so this is a
  free capture into the `async move` block, not a clone). This is not
  just tidiness: it is what actually *guarantees* the shadow row and the
  audit rows carry the identical value, rather than two independently
  correct-today computations that could silently drift apart under a
  future edit.

### 2.4 DD-4(ii) join (`control_plane_audit.rs::replay_grade_for_decision`)

```rust
Some(AuditEvent::DecisionEvaluated { outcome, .. }) => {
    let gate_results: Option<(serde_json::Value,)> = sqlx::query_as(
        r#"SELECT gate_results FROM "ob-poc".control_plane_shadow_decisions WHERE decision_id = $1 LIMIT 1"#,
    )
    .bind(decision_id)   // the function's own parameter, not a payload field
    .fetch_optional(pool)
    .await?;
    ...
}
```

`decision_id` uniquely identifies at most one `control_plane_shadow_decisions`
row by construction (minted once per shadow-eval attempt), so the
`LIMIT 1` here is now a correctness no-op (defensive, not
load-bearing) rather than the silent source of nondeterminism it was
against `entry_id`. `entry_id != Uuid::nil()` guard dropped — no longer
relevant to this join at all (kept as a payload field, just not
consulted here). A missing shadow row for this `decision_id` (never
inserted, or a pre-migration row whose shadow counterpart has
`decision_id IS NULL`) is still inconclusive, not a manufactured
failure — same posture as before, just keyed correctly now.

### 2.5 Other `build_shadow_decision_row` call sites — audited, not guessed

`ShadowDecisionRow` is `pub` (G5's disclosed widening, so `ob-poc-web`
could reach it) and `build_shadow_decision_row` had 3 production call
sites beyond `sequencer.rs`, found by grep, each individually checked
for whether it has a corresponding `control_plane_audit` emission to
correlate with:

- `rust/crates/ob-poc-web/src/bus_runtime.rs` (Path D, bus-federated) —
  grepped this file for `insert_audit_event`/`control_plane_audit`:
  zero matches. This seam shadow-evaluates but never emits a
  `DecisionEvaluated` audit event at all. `decision_id: None` — the
  honest answer, not a guess.
- `rust/src/dsl_v2/executor.rs` (Path B, DslDirect) — same check, same
  result: zero `insert_audit_event` call sites in this file.
  `decision_id: None`.
- `rust/src/agent/control_plane_metrics.rs` — 5 more call sites, all
  inside `#[cfg(test)]` fixtures unrelated to the G11 join (testing
  `shadow_divergence_stats`/`write_attestation_breach_stats`/
  `sealable_rate_by_verb`/`e3_matrix_invariant_probe` etc.) —
  `decision_id: None` for all 5; none of them insert corresponding
  audit rows either.

No call site was left un-migrated silently — `cargo build --workspace`
would not have compiled otherwise (new required positional parameter),
and a full-tree grep for `build_shadow_decision_row(` after the edit
confirms every call site (10 production + test sites total) was
updated.

---

## 3. Existing tests updated (not just the new one)

Five pre-existing live-DB tests in `control_plane_audit.rs` construct
`ShadowDecisionRow`s and assert on `replay_grade_for_decision`/
`audit_replay_outcome_counts`/`gate_outcome_counts`. All five needed
their `build_shadow_decision_row` call updated to pass
`Some(decision_id)` (the same `decision_id` already used for their
`insert_audit_event` calls) — without this, the fixed join would return
`None` (no matching shadow row) for every one of them, since the old
calls left `decision_id` unset:

- `w1_shadow_row_is_field_identical_with_and_without_audit_emission`
- `replay_grade_success_for_a_complete_consistent_approved_stp_lifecycle`
- `replay_grade_failure_for_an_outcome_rederivation_mismatch`
- `gate_outcome_counts_surfaces_audit_replay_samples_at_shadow_eval_provenance`
- (`divergence_flagged_when_shadow_and_legacy_disagree` in
  `control_plane_shadow.rs` — passes `None`, doesn't exercise the join)

All five re-run and pass post-fix (§4).

---

## 4. The regression test — RED→GREEN, not asserted

New test:
`agent::control_plane_audit::tests::live_db::same_entry_id_retried_attempts_each_join_to_their_own_gate_results_not_the_others`
in `rust/src/agent/control_plane_audit.rs`.

Constructs exactly the reported failure scenario: two shadow-eval
attempts sharing one `entry_id` (attempt 1: `Rejected`, Authority-denied
`gate_results`; attempt 2: `ApprovedStp`, all-`Success` `gate_results`),
each with its own real `decision_id` and its own real
`control_plane_audit` row sequence (attempt 2's full `Sealed →
Consumed → Committed` lifecycle included). Proves three things against
the real production functions, not a hand-rolled reimplementation:

1. **`entry_id` really is ambiguous in the persisted data** — a direct
   query confirms 2 distinct `control_plane_shadow_decisions` rows share
   one `entry_id`, with 2 *different* `gate_results` values.
2. **The fixed join resolves each attempt to its own row** —
   `replay_grade_for_decision(pool, decision_id_1)` and
   `replay_grade_for_decision(pool, decision_id_2)` both grade `Success`
   (each decision's own claim is internally consistent with its own
   `gate_results`).
3. **The old join would have been wrong for at least one attempt
   regardless of which ambiguous row it picked** — re-deriving attempt
   1's `Rejected` claim against attempt 2's `gate_results` produces
   `ApprovedStp` (a false-Failure mismatch); re-deriving attempt 2's
   `ApprovedStp` claim against attempt 1's `gate_results` produces
   `Rejected` (also a false-Failure mismatch).

**RED→GREEN, performed for real:**

1. Backed up `control_plane_audit.rs`, then temporarily reverted
   `replay_grade_for_decision`'s join to the exact pre-fix logic
   (`WHERE entry_id = $1 LIMIT 1`, bound to `*entry_id` extracted from
   the event payload, `if *entry_id != Uuid::nil()` guard restored).
2. Ran the new test in isolation:
   `cargo test -p ob-poc --lib control_plane_audit::tests::live_db::same_entry_id_retried_attempts -- --ignored --test-threads=1`
   → **FAILED**:
   ```
   thread '...same_entry_id_retried_attempts...' panicked at src/agent/control_plane_audit.rs:1229:13:
   attempt 2 (ApprovedStp, backed by its OWN all-Success gate_results) must grade Success under the fixed decision_id join
   ```
   (In this run Postgres's unordered `LIMIT 1` happened to return
   attempt 1's row for both attempts' queries — grading attempt 2 wrong.
   The point proven is that the old join CAN return the wrong row, not
   which specific row it returns on any given execution; §4's Part 3
   assertions independently prove both directions are unsafe regardless
   of physical row order.)
3. Restored the file from the backup exactly (`diff` confirmed clean).
4. Re-ran the same test → **`ok`**.

Full suite re-run post-restore, all 8 live-DB tests in this module
green (§5).

---

## 5. Verification — full build/test/clippy/invariants, executed for real

### 5.1 Build

```
$ DATABASE_URL="postgresql:///data_designer" cargo build --workspace
   Compiling ob-poc v0.1.0 ...
   Compiling ob-poc-web v0.1.0 ...
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 32.12s
```
Clean.

### 5.2 Clippy — scoped to what this session touched

`cargo clippy -p ob-poc -p ob-poc-web --lib --tests -- -D warnings`
fails on this branch **at HEAD, before any of this session's changes**
(verified by `git stash` + re-run, error set identical: `int_plus_one`
lints in `control_plane_metrics.rs` pre-existing test asserts,
`from_refs`/`compare_trace_ids`/`compare_session_traces` dead-code in
`traceability/{phase2,replay}.rs`, `expect_fun_call` in
`tests/kyc_m3_remediation.rs` or `tests/kyc_verb_coverage.rs` (whichever
compiles first — nondeterministic ordering, both pre-existing), an
`await_holding_lock` in `sem_os_runtime/verb_executor_adapter.rs`, and
an `items_after_test_module` in `ob-poc-web/src/main.rs`). None of these
are in files this session touched, and the error set is
byte-for-byte identical with and without this session's diff — **not
introduced by this fix**, out of scope per the mission brief (no
G14/write-set-attestation touching).

This session's own touched surface, verified clean independently:

```
$ cargo clippy -p ob-poc --lib -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.15s
$ cargo clippy -p ob-poc-web --bins -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.00s
```
(`ob-poc-web` has no lib target, only bins — the `--tests` failure above
is specifically the pre-existing `main.rs` test-module-ordering lint,
unrelated to this diff.)

### 5.3 Live-DB control-plane tests

```
$ DATABASE_URL="postgresql:///data_designer" cargo test -p ob-poc --lib control_plane -- --ignored --test-threads=1
...
test result: ok. 40 passed; 1 failed; ...
```
The 1 failure is `e3_invariant_probe` — expected, pre-existing,
unrelated (§5.5). All `control_plane_audit`/`control_plane_shadow`/
`control_plane_envelope_store`/`control_plane_floor` tests pass,
including the new regression test and the 5 updated pre-existing ones.

### 5.4 Full lib suite (non-ignored)

```
$ DATABASE_URL="postgresql:///data_designer" cargo test -p ob-poc --lib
test result: ok. 2187 passed; 0 failed; 219 ignored; 0 measured; 0 filtered out
```
2187/0 — matches this program's own established baseline count from the
prior session (`3b8b12e2`/`0c7c9441`), confirming no regression anywhere
else in the tree.

### 5.5 `check-invariants.sh e3` — confirms this fix changes nothing about `[e3]`'s pass/fail

```
$ DATABASE_URL="postgresql:///data_designer" ./scripts/check-invariants.sh e3
...
E3_INVARIANT_FAILURE: 0 gate(s) have zero substantive production samples anywhere: [];
1 gate(s) have samples only at the WRONG provenance (expected-provenance mismatch): ["WriteSetAttestation"]
  ** E3 live half: INVARIANT FAILURE — verified against a live DB, N/14 gates genuinely empty. **
  E3: DOES NOT HOLD (see harness output above)
```
`AuditReplay` itself now reports **85 substantive samples, all at the
correct `shadow_eval` provenance** (up from the ~57 the prior session's
G11-wiring commit reported at close, reflecting more decisions
accumulated in the shared dev DB since) — `AuditReplay` is not the
failing gate; `WriteSetAttestation` is, exactly as
`invariants-expected.toml`'s own `[e3]` comment already documents (G14,
deliberately unarmed, out of this session's scope). `[e3]` was `fail`
before this fix and is `fail` after, for the identical named reason —
confirmed, not assumed.

### 5.6 `check-invariants.sh ratchet`

```
$ DATABASE_URL="postgresql:///data_designer" ./scripts/check-invariants.sh ratchet
...
  E5: DOES NOT HOLD
  [e5] actual=fail expected=fail — MATCH
== Ratchet: 0/5 invariant(s) diverge from invariants-expected.toml ==
```
0/5 divergence — every gate's actual status matches its recorded
expectation, `[e3]` included. `invariants-expected.toml` was **not
edited** by this session (per the mission's non-negotiable rule) — no
recommendation to change it either: this fix is upstream of the G14
arming decision that would move `[e3]`, exactly like the G14
table-format-fix session's own framing.

---

## 6. Files changed

- `rust/migrations/20260714_control_plane_shadow_decisions_decision_id.sql` — new. Adds nullable `decision_id UUID` + partial index to `control_plane_shadow_decisions`.
- `rust/src/agent/control_plane_shadow.rs` — `ShadowDecisionRow.decision_id: Option<Uuid>` (new field), `build_shadow_decision_row` gained a `decision_id: Option<Uuid>` parameter, `insert_shadow_decision`'s `INSERT` extended; 2 test call sites updated (`None`).
- `rust/src/agent/control_plane_audit.rs` — DD-4(ii) join in `replay_grade_for_decision` switched from `entry_id` to `decision_id`; doc comments on `audit_replay_outcome_counts`/`replay_grade_for_decision` rewritten to describe the real (fixed) join and cite the new regression test; 5 pre-existing test call sites updated to pass `Some(decision_id)`; 1 new regression test added.
- `rust/src/sequencer.rs` — both `phase5_runtime_recheck` and the HumanGate-resume reseal path: `decision_id` now computed once, before `build_shadow_decision_row`, and reused (not recomputed) inside the `tokio::spawn` block for the `insert_audit_event` calls; doc comments updated.
- `rust/crates/ob-poc-web/src/bus_runtime.rs` — Path D's `build_shadow_decision_row` call updated with `decision_id: None` (no corresponding audit-event emission exists at this call site).
- `rust/src/dsl_v2/executor.rs` — Path B's `build_shadow_decision_row` call updated with `decision_id: None` (same reason).
- `rust/src/agent/control_plane_metrics.rs` — 5 test-only `build_shadow_decision_row` call sites updated with `None` (unrelated to the G11 join; mechanical signature-compatibility update only).
- `docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-G11-JOIN-FIX-001.md` — this doc.

Not touched: `invariants-expected.toml` (recommend-only, per rule), the
plan doc's own tranche-completion markers, the ownership ledger (both
governance bookkeeping handled separately per the mission brief), any
G14/write-set-attestation code, `bpmn-lite`, and the 5 pre-existing
dirty files called out in the mission brief
(`observatory-wasm/Cargo.lock`, `rust/cbu_mismatches.json`,
`rust/mismatches.json`, `rust/reports/phase0_confusion_matrix.json`,
`rust/reports/step0_trial_evaluation.json`) — confirmed via `git status`
before and after this session's edits: identical set, no new changes to
any of them.

---

## 7. Recommendation (not applied)

None for `invariants-expected.toml` — this fix is correctness-only for
an already-`fail`-expected gate's underlying re-derivation logic, and
does not change `[e3]`'s status. Worth noting for a future session:
`AuditReplay`'s sample count in the E3 probe output (85, up from the
G11-wiring commit's original 57) will now also be *more trustworthy*
per-sample, since a live decision that gets retried after
`RequiresHumanGate` or a fixed rejection no longer risks having its
grade silently swapped with a sibling attempt's.
