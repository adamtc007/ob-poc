# EOP-SESSION-CONTROLPLANE-G1-ITEM2-G2-ITEM2-IMPL-001 — Implementation session log

### Implements: G1 items 2–4 (`EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001`, RATIFIED) and G2 item 2 (`EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.5` §3)
### Date: 2026-07-13
### Branch: `codex/phase-1-5-governance-closure` (not merged, not committed)

---

## 1. Verification against the design doc's own claims, before any code

**G1 design doc's central claim (§2.1, §3): `CompiledStep.step_id` already equals the originating `RunbookEntry.id`, so the consume site needs zero `StepExecutor`/`CompiledStep` signature changes.** Confirmed against current code, unchanged since the doc was written: `runbook/types.rs:139-140`'s own doc comment ("Stable step ID (from the originating `RunbookEntry.id`)"), and `VerbExecutionPortStepExecutor::execute_step(&self, step: &CompiledStep)` (`step_executor_bridge.rs`) already has `step.step_id` in scope. **Confirmed correct — no deviation.** The doc's "no `StepExecutor` trait signature change" claim held; what the doc did NOT anticipate (a genuinely new finding, not a deviation from something claimed) is that `VerbExecutionPortStepExecutor` itself (the concrete struct, not the trait) had no `pool` field to run the `lookup_sealed_handle` query — added one (`pool: Option<sqlx::PgPool>` + `with_pool()` builder), which is exactly the class of change §3's own text anticipates ("touches or reasons about... `rust/src/runbook/step_executor_bridge.rs` (consume site)") — not a deviation from the doc's design, an implementation-time detail the doc's own §11 flagged as unspecified.

**§2.1's schema claim (new `entry_id UUID` column on `control_plane_envelopes`, nullable, no backfill needed since the table is shadow-sealing-only in production).** Confirmed: `SELECT count(*) FROM "ob-poc".control_plane_envelopes` and the migration history show no production consumer read `entry_id` before this diff (G1 items 2-4 were the first to need it) — no backfill attempted or needed, matching the doc.

**§4's HumanGate re-seal decision (re-seal at resume, never extend/reuse a pre-park envelope).** Implemented per the doc's decision, but via a **narrower re-derivation than the doc's own pseudocode literally suggests** — see §3 below for why, and why this is a deliberate, reasoned deviation rather than a shortcut.

**G2 item 2's premise ("Capture is already live in the CRUD executors").** Confirmed true and more specific than the plan's own text implies: `record_write` is called by `crud_executor.rs`'s real `execute_insert`/`execute_update` paths (T10.3) whenever dispatch runs through the scope-based `CrudExec::Scope` variant — which is exactly what `execute_verb_admitting_envelope`'s Branch 2/3 use. `sequencer_tx.rs`'s own doc comment ("No `SemOsVerbOp` calls `record_write` today") is accurate only for the `SemOsVerbOp` plugin-op path specifically, not for CRUD dispatch generally — a narrower claim than its wording suggests, verified by direct grep before relying on either.

---

## 2. G1 item 2 — what's implemented

### Schema

`rust/migrations/20260713_control_plane_envelopes_entry_id.sql` (new, applied to local Postgres): `ALTER TABLE "ob-poc".control_plane_envelopes ADD COLUMN entry_id UUID` (nullable, no backfill) + `CREATE INDEX idx_control_plane_envelopes_session_entry_status ON ... (session_id, entry_id, status)`, per §2.1 exactly.

### Carrier wiring

- `agent::control_plane_envelope_store::persist_sealed` gained an `entry_id: Uuid` parameter (inserted between `session_id` and `verb_fqn`), written into the new column.
- New `agent::control_plane_envelope_store::lookup_sealed_handle(pool, session_id, entry_id) -> anyhow::Result<Option<EnvelopeHandle>>` — exactly §2.1's SQL: `SELECT envelope_id, content_hash FROM control_plane_envelopes WHERE session_id = $1 AND entry_id = $2 AND status = 'sealed' ORDER BY created_at DESC LIMIT 1`, reconstructing an `EnvelopeHandle` from the stored hex content hash.
- `sequencer.rs`'s `phase5_runtime_recheck`: the `persist_sealed` call moved from inside the existing best-effort `tokio::spawn` to a **synchronous `.await`** immediately after the decision is computed (before the spawn), per §2/§4's requirement that the row genuinely exist by the time this same loop iteration reaches the consume site a few calls later. The shadow-row insert and `DecisionEvaluated`/`EnvelopeSealed` audit events stay in the existing fire-and-forget spawn — additive-only, W1-shaped (window-discipline: nothing about the shadow row itself changed).
- `runbook/step_executor_bridge.rs`: `VerbExecutionPortStepExecutor` gained a `pool: Option<sqlx::PgPool>` field + `with_pool()` builder. `execute_step`'s hardcoded `envelope_handle: None` (line 553 in the design doc's citation) is replaced with a real `lookup_sealed_handle(pool, session_id, step.step_id)` call; `None` (no pool attached — the two in-crate test constructors — or nothing sealed for this entry) degrades to the exact pre-G1 behaviour, dispatch-outcome-neutral while `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` stays empty (production default), matching the existing comment's own claim.
- `sequencer.rs`'s real Path A construction site (`~8405`, `execute_step_v13`) now calls `.with_pool(pool.clone())` when `self.pool()` is `Some`.

### §4 — HumanGate re-seal

New `Sequencer::reseal_for_human_gate_resume(&self, session, entry_id)`, called from `handle_human_gate_approval` immediately before `execute_entry_via_gate` (synchronously awaited, same requirement as the Sync/Durable path).

**Deliberate, reasoned narrowing vs. the design doc's pseudocode:** the doc's §4/§8 suggest factoring the *entire* seal step out of `phase5_runtime_recheck` (including its T11.F.2 G1/G3/G4 definitional-floor checks, which can themselves return `Some(StepOutcome::Failed)`) so both call sites share one implementation. I did **not** do this. `reseal_for_human_gate_resume` re-derives and re-persists a fresh sealed envelope using the same real helper functions `phase5_runtime_recheck` uses (`entity_binding_requests`, `build_entity_binding_input`, `build_pack_resolution_input`, `build_dag_proof_input`, `build_write_set_input`, `build_stp_classifier_input`, `build_decision_snapshot_input`, `build_runbook_proof_input`, `build_version_pinning_input`, `build_evaluation_context`, `evaluate_with_report`) but **deliberately omits** the T11.F.2 floor-check early-returns and the legacy `shadow_envelope`/`legacy_outcome` machinery. Rationale: those are real gating mechanisms with no re-seal-at-resume call site today; wiring them in here would be a **second, unreviewed production-behaviour change** at HumanGate resume (a resume that could newly fail for a reason it never could before) — outside this session's sanctioned scope (only G2 item 2 is named as a production-behaviour change; everything else is meant to stay shadow-only). The re-seal itself is provably shadow-only: it only ever calls `persist_sealed` (never blocks), matching `phase5_runtime_recheck`'s own "sealing is shadow-only — never gates dispatch" invariant.

**Consequence, honestly flagged:** this duplicates roughly 90 lines of context-derivation logic against `phase5_runtime_recheck`'s inline block rather than sharing one implementation, which is a real quality gap against the doc's own preference (not a functional gap — the new function is directly derived from, and calls the same real helpers as, the block it mirrors). A future pass could extract a shared private helper taking the already-resolved `(entity_binding, pack_resolution, dag_proof, ...)` inputs; not done here given this session's time budget.

### G1 item 3 — live-DB tests, from the real Path A call site

New module `runbook::step_executor_bridge::g1_item2_path_a_tests` (`#[cfg(all(test, feature = "database"))]`, `#[ignore = "requires DATABASE_URL"]`), driving `VerbExecutionPortStepExecutor::execute_step` directly (not `execute_verb_admitting_envelope`/`admit_in_scope` in isolation, which `t4_1_envelope_admission_tests` already proves):

1. `no_sealed_envelope_for_this_entry_is_rejected_with_triage_classification` — enforced `cbu.confirm`, nothing sealed for a fresh `entry_id`: `execute_step` returns `StepOutcome::Failed` with the real `admit_in_scope` message (`"is enforce-mode gated... but no sealed envelope was presented"`). Covers item 3's assertion 2 and item 4 together (non-eligible/no-envelope reject-with-classification, asserted by stable substring per this module's existing convention).
2. `admits_consumes_once_from_path_a_then_a_bare_retry_finds_nothing_sealed` — seals a real envelope row keyed to `(session_id, entry_id)`, dispatches a real `cbu.confirm` against a real (test-reset-to-`VALIDATION_PENDING`) `cbus` row through `execute_step`: first attempt admits, consumes, and **actually completes** (proving the whole chain, not just the admission check); the row durably transitions to `status = 'consumed'`; a second `execute_step` call against the *same compiled step* (no re-seal, simulating a caller bug) finds nothing sealed (`lookup_sealed_handle`'s own `WHERE status = 'sealed'` excludes it) and is rejected with the same classified message. Covers item 3's assertions 1 and 3 ("the *system's* behaviour is fresh-seal-per-attempt", not a raw handle resubmission, which is already proven at the adapter level per §1.5 of the design doc).

**Item 3's assertion 4 (HumanGate re-seal, own dedicated live-DB test) and open question 1 (§11: does a `Durable` mid-dispatch park ever re-enter a not-yet-consumed envelope) are NOT covered by a new test this session** — `reseal_for_human_gate_resume` is implemented, compiles, and reuses the exact real helper functions `phase5_runtime_recheck`'s own (tested) seal path uses, but proving it end-to-end requires driving a full parked `ReplSessionV2`/`HumanGate` approval flow, which is a materially larger integration-test build-out than this session's remaining budget allowed. Flagged honestly as open, not silently claimed done — recommend a follow-up session scoped specifically to that harness.

Command output (live-DB, this session):

```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    -- --ignored --test-threads=1 g1_item2_path_a_tests
running 2 tests
test runbook::step_executor_bridge::g1_item2_path_a_tests::admits_consumes_once_from_path_a_then_a_bare_retry_finds_nothing_sealed ... ok
test runbook::step_executor_bridge::g1_item2_path_a_tests::no_sealed_envelope_for_this_entry_is_rejected_with_triage_classification ... ok
test result: ok. 2 passed; 0 failed

$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    -- --ignored --test-threads=1 sem_os_runtime::verb_executor_adapter::
running 7 tests   (t4_1 property set, unaffected by G1/G2 wiring)
test result: ok. 7 passed; 0 failed
```

---

## 3. G2 item 2 — what's implemented, and the STOP-condition

### The finding

The plan's own instruction for item 2 is: "wire `set_expected_write_set` + `commit_attested` into the sequencer's commit path... populated from the verb's declared write footprint (already used for G7's shadow evaluation)." The only existing production source of that footprint is `agent::control_plane_shadow::build_write_set_input` (`control_plane_shadow.rs:195-215`), which builds a `WriteSetInput` with `tables: footprint.writes.clone()` (real, from `domain_metadata.yaml`) but **`allowed_columns: Vec::new()` always** — the function has no per-column knowledge, `domain_metadata.yaml`'s footprint is table-level only.

`write_set_attestation::attest`'s column check is `write.columns.iter().all(|c| expected.allowed_columns().contains(c))`. With an empty `allowed_columns`, this is `false` for **any** write reporting a nonempty column list — regardless of table/entity match. `crud_executor.rs`'s real `record_write` calls (T10.3, the scope-based CRUD dispatch `execute_verb_admitting_envelope`'s Branch 2/3 actually use) always report real, nonempty columns for a genuine INSERT/UPDATE.

**Consequence: wiring `set_expected_write_set` from `build_write_set_input`'s output at this call site would misclassify every real, legitimate CRUD write as a breach and roll it back, for any verb with a declared write footprint dispatched through Path A/D — not "a real excess write gets caught," but "every write gets rejected."** This is exactly the plan's own named STOP-condition ("a real excess/undeclared write gets caught and rolled back where it previously wasn't... if implementation finds any verb's behavior changing, stop and flag for architect review even with green tests").

**Proven empirically, not just reasoned:** `ob-poc-control-plane::write_set_attestation::tests::empty_allowed_columns_breaches_every_write_with_any_column_even_on_exact_table_and_entity_match` — a table-match, entity-match, single-column write against an empty-`allowed_columns` proof still classifies as `Breach`. Added to the existing test module alongside the other `attest()` fixtures; run:

```
$ cargo test -p ob-poc-control-plane --lib write_set_attestation
running 9 tests
test write_set_attestation::tests::empty_allowed_columns_breaches_every_write_with_any_column_even_on_exact_table_and_entity_match ... ok
... (8 more, all ok)
test result: ok. 9 passed; 0 failed
```

### The STOP fired — what I did instead

**Did not call `set_expected_write_set`.** The `execute_verb_admitting_envelope` commit call site (`sem_os_runtime/verb_executor_adapter.rs`) now calls `scope.commit_attested(None, Some(verb_fqn))` **instead of** plain `scope.commit()` — this genuinely wires `commit_attested` into the sequencer's commit path (the transport half of the instruction), but with no `WriteSetProof` attached. `PgTransactionScope::commit_attested`'s own early return (`let Some(expected) = self.expected_write_set.clone() else { self.tx.commit().await...; return Ok(()); }`) means the `attest()` comparison never runs when no expectation is set — `Breach` is **structurally unreachable** from this call site as wired. This is provably behaviour-identical to plain `commit()` for every real call today (the same property the crate's own pre-existing test `no_expected_write_set_commits_unconditionally_like_plain_commit` establishes for the mechanism in general).

This is exactly the mission's own sanctioned fallback: "wire the mechanism in a way that logs/records without actually rejecting." The transport is real and live; the compare-and-attest half stays open, explicitly, pending a correctly column-aware `WriteSetProof` derivation (a separate, reviewed follow-up — `build_write_set_input` needs real per-column knowledge, which `domain_metadata.yaml`'s current table-only footprint schema does not carry).

`DispatchCommitted`'s audit event (from the prior G2-items-3+4 session) is **unchanged**: `attested: false`, `gate_outcome: NotEvaluated` — still honest, since no comparison genuinely ran.

### Confirmation: no verb's behaviour changed

- `PgTransactionScope::commit_attested` with `expected_write_set: None` is provably identical to `commit()` by inspection of its own source (the early-return path, quoted above) — no live-DB test needed to prove a code path that is structurally never reached is inert, but the pre-existing `t5_write_set_attestation_tests::no_expected_write_set_commits_unconditionally_like_plain_commit` test (unedited by this session) already covers exactly this claim at the `PgTransactionScope` level.
- All 7 `t4_1_envelope_admission_tests` (the property set this exact call site is graded against) pass unchanged, live-DB, after the `commit()` → `commit_attested(None, ...)` swap — see command output in §2 above (the same run exercises this call site, since `execute_verb_admitting_envelope` is what those tests dispatch through).
- The two new `g1_item2_path_a_tests` (§2 above) include a **real successful CRUD dispatch** (`cbu.confirm`, real writes, real `record_write` capture via `crud_executor.rs`) through this exact commit call site — it completes and durably commits, proving the swap doesn't newly reject a real write.
- Full `ob-poc` lib suite: 2160 passed, 0 failed (unchanged from before this session's diff, confirming no regression anywhere else).

**No production verb's dispatch outcome changed.** The STOP-condition fired on the *comparison* half (which is not wired), not on the *transport* half (which is wired and proven inert).

---

## 4. Command output (real runs, this session)

```
$ cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.52s   # zero errors

$ cargo clippy -p ob-poc --lib --features database -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.22s   # zero warnings

$ cargo clippy -p ob-poc-control-plane --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.75s    # zero warnings
    # One pre-existing (unrelated-file) lint fixed en route: decision.rs:479
    # (`evaluate(&ctx, window.clone())` on a `Copy` type, `clone_on_copy`) —
    # only surfaced under `--all-targets` (test target), zero behaviour
    # change, `decision.rs` is otherwise untouched by this session's own
    # work (confirmed via `git status --porcelain` before editing: no prior
    # diff on that file). Fixed rather than left, per this session's own
    # house rule about not repeating the prior session's missed-lint pattern.

$ cargo clippy --workspace --all-targets -- -D warnings
    error: could not compile `ob-poc-kyc-substrate` (test "kyc_slice") due to 9 previous errors
    error: could not compile `ob-poc` (test "kyc_verb_coverage") due to 3 previous errors
    error: could not compile `ob-poc` (lib test) due to 7 previous errors  # await_holding_lock etc.
    # PRE-EXISTING, unrelated to this session, confirmed via `git diff`
    # region inspection for each: `kyc_slice.rs` (useless_vec, crate this
    # session never touched), `kyc_verb_coverage.rs` (expect_fun_call, file
    # this session never touched), and an `await_holding_lock` finding at
    # `verb_executor_adapter.rs:1430` — outside both of this session's own
    # diff hunks in that file (lines 657-735ish and 1682+ only; confirmed
    # via `git diff -- src/sem_os_runtime/verb_executor_adapter.rs`). Not
    # fixed here — real pre-existing debt, out of this session's scope, and
    # (for the lock-across-await case) not a safe one-line fix the way
    # decision.rs's was. `cargo clippy --all-targets -D warnings` across
    # the FULL workspace has evidently never been clean; every prior
    # session's own verification (including this one's primary checks
    # above) scoped to `--lib`, matching the project's established
    # convention (`cargo test -p ob-poc --lib`, CLAUDE.md's own `cargo x
    # pre-commit`/`check` commands) rather than `--all-targets`.

$ cargo test -p ob-poc --lib --features database
test result: ok. 2160 passed; 0 failed; 200 ignored

$ cargo test -p ob-poc-control-plane --lib
test result: ok. 116 passed; 0 failed

$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    control_plane -- --ignored --test-threads=1
test result: FAILED. 31 passed; 1 failed
    # the 1 failure is e3_invariant_probe -- EXPECTED per invariants-expected.toml's
    # ratcheted [e3] status = "fail"; gate counts unchanged from before this session
    # (G14/WriteSetAttestation correctly still zero substantive samples -- this
    # session's STOP-condition means no Success/Failure outcome was ever produced,
    # matching the "honest non-improvement, not a bug" framing the prior G2
    # items-3+4 session already established for this exact gate)

$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    -- --ignored --test-threads=1 g1_item2_path_a_tests sem_os_runtime::verb_executor_adapter::
test result: ok. 9 passed; 0 failed

$ cargo test -p ob-poc --lib --features database -- test_plugin_verb_coverage
test result: ok. 1 passed; 0 failed

$ bash scripts/check_kyc_substrate_deps.sh
PASS: no forbidden deps in ob-poc-kyc-substrate
```

---

## 5. Files changed

- `rust/migrations/20260713_control_plane_envelopes_entry_id.sql` (new) — `entry_id UUID` column + `(session_id, entry_id, status)` index on `control_plane_envelopes`.
- `rust/crates/ob-poc-control-plane/src/write_set_attestation.rs` — one new unit test proving the G2 item 2 STOP-condition finding (`empty_allowed_columns_breaches_every_write_with_any_column_even_on_exact_table_and_entity_match`). No production code changed in this crate.
- `rust/src/agent/control_plane_envelope_store.rs` — `persist_sealed` gained `entry_id: Uuid`; new `lookup_sealed_handle`; 11 existing test call sites updated (`entry_id` arg added, `Uuid::now_v7()` per the coordinator's mid-session UUID-generation guidance for these high-insert, no-DB-default control-plane tables).
- `rust/src/sequencer.rs` — `phase5_runtime_recheck`'s `persist_sealed` call made synchronous (moved out of the detached spawn, `entry_id` threaded); new `reseal_for_human_gate_resume` (§4); `handle_human_gate_approval` calls it before `execute_entry_via_gate`; the real Path A construction site attaches `.with_pool(...)` to `VerbExecutionPortStepExecutor`.
- `rust/src/runbook/step_executor_bridge.rs` — `VerbExecutionPortStepExecutor` gained `pool: Option<sqlx::PgPool>` + `with_pool()`; `execute_step`'s hardcoded `None` envelope handle replaced with a real `lookup_sealed_handle` call; new `g1_item2_path_a_tests` module (2 live-DB tests).
- `rust/src/sem_os_runtime/verb_executor_adapter.rs` — `execute_verb_admitting_envelope`'s commit call site: `scope.commit()` → `scope.commit_attested(None, Some(verb_fqn))` (G2 item 2, no `set_expected_write_set` — STOP-condition, §3 above); one existing test call site updated for `persist_sealed`'s new `entry_id` param (`Uuid::now_v7()`).
- `rust/crates/ob-poc-control-plane/src/decision.rs` — one-line pre-existing lint fix (`clone()` on `Copy` type, `--all-targets`-only, unrelated to this session's own subject) — see §4.

---

## 6. `invariants-expected.toml` — recommendations, not applied

Per this doc's own established pattern (the prior two sessions in this program): recommend only, architect decides. No flips applied to `invariants-expected.toml` in this diff.

**`[e2]` — recommend updating the detail comment, per G1's own exit-gate text ("`[e2]` detail comment updated ('Path A enforce-capable, not yet enforced')")**. Current text: *"Structural: 2/4 RR-2 paths (A, D) call `execute_verb_admitting_envelope` at all; B, C have no admitting entry point. Dynamic (live DB): Path D's admission mechanism works when enabled but `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` is unset by production default (NotEnforced) everywhere."* This remains accurate as a structural/dynamic split, but should gain a third clause: **Path A's seal→consume wiring is now real** (this session's diff) — a real `EnvelopeHandle`, not a hardcoded `None`, reaches `execute_verb_admitting_envelope` whenever `phase5_runtime_recheck` sealed one for the dispatching entry — proven end-to-end live-DB (§2 above, `g1_item2_path_a_tests`). Status should **stay `fail`**: no verb is in `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` by production default, so this is capability, not enforcement — exactly G1's own exit-gate framing ("Path A enforce-capable, not yet enforced"). Suggested wording: *"Structural: 2/4 RR-2 paths (A, D) call `execute_verb_admitting_envelope`... Path A's seal→consume correlation is now real (G1 items 2-4, `entry_id`-keyed lookup, live-DB proven) — Path A is enforce-**capable**, not yet enforced (`OB_POC_CONTROL_PLANE_ENFORCE_VERBS` unset by production default)."*

**`[e1]` — C-032 advancement (G2 item 2).** C-032 ("CRUD executor executes metadata-driven insert/update/delete/upsert without comparing to a WriteSetProof") is **partially, not fully, advanced**: the *mechanism* now has a real, live call site (`commit_attested` genuinely runs on every Path A/D commit), but the *comparison* itself never activates (no `WriteSetProof` attached, by this session's own STOP-condition finding). Recommend the ledger/C-032 entry note this precisely as "transport wired, comparison not yet safe to enable — `build_write_set_input`'s `allowed_columns` gap must close first" rather than either "C-032 closed" (overclaiming — no write has ever actually been compared) or "no change" (underclaiming — the call site is real and tested, not a no-op). I did not flip C-032's status in `invariants-expected.toml` myself; `[e1]`'s existing text (4/45 RR-3 rows closed, citation-only) doesn't currently enumerate C-032 by number, so there's no existing line to edit — recommend the architect decide whether this warrants a new named row or stays folded into the general count.

**`[e3]` — G14/WriteSetAttestation.** No change recommended. G14 remains at zero substantive samples, correctly — this session's `DispatchCommitted` event still records `NotEvaluated` (never `Success`/`Failure`), matching the honest-non-improvement framing the prior G2 items-3+4 session already established. G10/ExecutionEnvelope's existing 11/14-of-14 count (from that same prior session) is unaffected by this session's changes; this session's own `g1_item2_path_a_tests` add further live-DB `consume_seam`-provenance samples to G10 but do not change which gates count as substantive.

---

## 7. Rules-of-evidence notes

- No `git commit` run. No merge, deploy, or `git push` run.
- Every sqlx-touching function added or edited in `ob-poc-control-plane` — there are none; that crate's only change is a `#[cfg(test)]` unit test, matching its existing zero-DB-deps posture. Every sqlx-touching function in `ob-poc` (root) is either inside an existing `#[cfg(feature = "database")]` module/function or was already unconditional in a file where sqlx is already an unconditional dependency throughout (`runbook/step_executor_bridge.rs` — confirmed via grep before adding the new `pool` field without a `cfg` gate, to match the file's own existing convention rather than introduce a locally-inconsistent gate).
- New UUID-generation call sites introduced by this session's own work (the `entry_id` carrier and its test fixtures) use `Uuid::now_v7()`, per the coordinator's mid-session guidance — these are new rows in `control_plane_envelopes`, a high-insert, append-heavy, no-DB-side-UUID-default table exactly matching the stated rationale. Pre-existing `Uuid::new_v4()` call sites in the same files (e.g. `session_id` generation in existing test fixtures) were left untouched, per the same guidance's explicit scope boundary.
