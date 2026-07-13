# EOP-SESSION-CONTROLPLANE-G1-ITEMS-3-4-IMPL-001 — Verification session log

### Task: implement `EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.5` G1 items 3–4
### Date: 2026-07-13
### Branch: `codex/phase-1-5-governance-closure` (working tree left clean — no code changes made this session)

---

## 0. Finding, up front

**G1 items 3 and 4, as literally scoped by this session's own brief, are
already implemented, committed, and — as of this session — independently
re-verified against current HEAD.** They landed in commit `01539938`
("feat(control-plane): wire G1 item 2 seal->consume + G2 item 2 commit
transport"), together with G1 item 2, and are documented in
`docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-G1-ITEM2-G2-ITEM2-IMPL-001.md`.
Two further tranches (G4 — Path B/C per-step admission, commit `02816414`;
G5 — gate applicability matrix + shadow-eval on B/C/D, commit `79f2d27f`)
landed on top of that, changing several signatures the G1 tests touch
(`ExecutionPath`/`PathScope`, `check_admission*`'s new `path` parameter).
This session's job, given that discovery, was **not** to re-implement
already-landed work, but to verify — line by line, against the actual
brief's own two named properties, and against the *current* HEAD (not the
HEAD the prior session ran against) — that the claim holds and nothing in
the two subsequent tranches silently broke or hollowed it out. It does
hold; evidence is below. This doc records that verification, not a new
implementation.

This is being disclosed prominently, per this program's own house rule
about STOP-conditions and not silently working around a premise mismatch:
the task brief I was given assumed only G1 items 1–2 were done and items
3–4 were open. That assumption was stale relative to `git log` at the
time I started — `codex/phase-1-5-governance-closure` was already 82+
commits past `01539938` (through `77aa2f0d`, ledger entries for
G1item2+G2item2/G4/G5) before this session began. I verified this by
reading `git log --oneline`, `git show --stat 01539938`, and the prior
session's own doc, before writing a single line — not by assuming my
brief was current.

---

## 1. Verification against the task's own two named properties

### Item 3 — "(a) admits, consumes exactly once, rejects resubmission; (b) no envelope threaded is rejected"

Both live in `rust/src/runbook/step_executor_bridge.rs`'s
`g1_item2_path_a_tests` module (`#[cfg(all(test, feature = "database"))]`,
each `#[ignore = "requires DATABASE_URL"]`), driving
`VerbExecutionPortStepExecutor::execute_step` — the exact call site named
in this session's brief (`step_executor_bridge.rs:601-610`, confirmed by
direct read this session, quoted in §3 below) — not
`execute_verb_admitting_envelope`/`admit_in_scope` called in isolation.

- **(a)** `admits_consumes_once_from_path_a_then_a_bare_retry_finds_nothing_sealed`
  (lines 1176–1269): seals a real `control_plane_envelopes` row keyed
  `(session_id, entry_id)`, dispatches a real `cbu.confirm` against a real
  `cbus` row through `execute_step` — first call admits, consumes, and
  **actually completes** the dispatch (`StepOutcome::Completed`), and the
  row durably transitions to `status = 'consumed'` (asserted by a direct
  `SELECT`). A second `execute_step` call against the *same compiled
  step*, no intervening re-seal — i.e. exactly "a resubmission attempt
  against the same (now-consumed) envelope" — is rejected with the real
  `RejectedNoEnvelope` message (`lookup_sealed_handle`'s own
  `WHERE status = 'sealed'` filter excludes the now-consumed row, so the
  resubmission genuinely finds nothing to present, which is what
  "rejects" means at this call site — see §2 below for why this, not a
  literal same-handle POST, is the correct shape of "resubmission" from
  Path A's own call pattern).
- **(b)** `no_sealed_envelope_for_this_entry_is_rejected_with_triage_classification`
  (lines 1132–1160): enforced `cbu.confirm`, nothing sealed for a fresh
  `entry_id`, `execute_step` called directly — rejected with the same
  `RejectedNoEnvelope` message.

Both assert on the real `admit_in_scope` message text
(`"{verb_fqn} is enforce-mode gated (OB_POC_CONTROL_PLANE_ENFORCE_VERBS)
but no sealed envelope was presented"`), confirmed to still match the
live source verbatim this session (`sem_os_runtime/verb_executor_adapter.rs:230-233`,
grepped directly, not assumed from the prior session's citation).

### Item 4 — "non-eligible decisions (HumanGate/Rejected in shadow) reject with triage classification, not silent fallthrough"

**By construction, this is the same mechanism proven by (b) above, and I
verified the construction claim directly against `sequencer.rs` rather
than taking it on the prior session's word.** `phase5_runtime_recheck`
only calls `persist_sealed` inside the `ApprovedStp` arm of the decision
match (`sequencer.rs:8035`, `matches!(decision,
ControlPlaneDecision::ApprovedStp(_))`, confirmed by direct read this
session, both at the original seal site and at its `~8282` mirror used by
`reseal_for_human_gate_resume`). For `RequiresHumanGate` or `Rejected`
decisions, **no row is ever inserted** for that `entry_id` — `execute_step`
subsequently finds nothing via `lookup_sealed_handle`
(`WHERE status = 'sealed'`), which is *exactly* the scenario
`no_sealed_envelope_for_this_entry_is_rejected_with_triage_classification`
drives and asserts rejects with the classified `RejectedNoEnvelope`
message, not a bare/opaque dispatch error and not a silent fallthrough to
allow. There is no separate code path in `admit_in_scope` for "enforced,
decision was non-eligible, allow anyway" — `AdmissionDecision` is a
2-variant enum (`NotEnforced` / and the consume outcome), confirmed by
direct read of `control_plane_envelope_store.rs`'s admission types; a
missing handle is unconditionally `RejectedNoEnvelope` once the verb is in
`OB_POC_CONTROL_PLANE_ENFORCE_VERBS`, regardless of *why* nothing was
sealed. This satisfies the runbook §7 framing this session's brief cites
("reject-with-triage-classification, not silent fallthrough") in the
sense that matters at this call site: the rejection carries a stable,
distinguishable reason (`RejectedNoEnvelope`, not a generic `Err`), which
is what makes downstream triage against §7's three buckets (control-plane
bug / legacy defect / ambiguous) possible in the first place — §7 itself
is about classifying *shadow-divergence* rows post hoc, not about the
rejection error shape; the two are related but distinct, and I want that
distinction on the record rather than silently equating them.

**What is not additionally tested, and is out of this session's literal
scope:** the design doc's own §10 test plan (assertion 4) separately asks
for a dedicated live-DB test of the `HumanGate` re-seal-at-resume path
(park an entry, let its pre-park envelope go stale, approve, assert the
fresh re-seal — not the pre-park envelope — is what gets consumed). The
prior session implemented `reseal_for_human_gate_resume` but explicitly
did not write that test ("an end-to-end test for this path is still owed,
not written this session" — both the session doc §2 and the ownership
ledger's "G1 item 2 + G2 item 2 landed" entry say this). **This session's
own brief does not ask for that test** — its item 3 text is scoped to the
admit/consume-once/no-envelope triad (satisfied above), and its item 4
text is scoped to non-eligible-decision rejection (satisfied above,
same mechanism as the no-envelope case by construction). I did not
attempt to add the HumanGate-resume test this session: building it
requires a real `ReplSessionV2` with a wired `sem_os_client` and a full
park→approve flow through `Sequencer::handle_human_gate_approval`, which
the design doc's own §10/§11 already characterises as "a materially
larger integration-test build-out" than a bounded addition — attempting
it under this session's time budget risked producing a rushed harness
that doesn't meet this program's evidence bar, which is a worse outcome
than leaving it honestly flagged, as the prior session did. **Recommend a
follow-up session scoped specifically to that harness** (unchanged
recommendation from the prior session — repeating it here rather than
letting it go stale a second time).

---

## 2. Why "resubmission" at Path A is "same-step, no re-seal," not "same raw handle POSTed twice"

Flagging this distinction explicitly since a literal reading of this
session's brief ("rejects a resubmission attempt against the same
(now-consumed) envelope") could be read as asking for a raw-handle replay
test. That exact property — presenting the identical, already-consumed
`EnvelopeHandle` a second time and getting `AlreadyConsumed` — is already
proven, live-DB, at the adapter level by
`t4_1_envelope_admission_tests::enforced_verb_with_consumed_envelope_admits_then_rejects_resubmission`
(re-run this session, §4 below) and is not something Path A's own call
pattern ever manufactures: Path A never holds onto a raw handle across two
`execute_step` calls to hand it back in verbatim — it always re-derives
the handle via `lookup_sealed_handle` per call
(`step_executor_bridge.rs:577-599`). The G1 design doc's own §10 test
plan (item 3) anticipated exactly this and phrased its assertion
accordingly ("assert the *system's* behaviour is 'fresh seal per
attempt'... that raw-resubmission property is already proven at the
adapter level"). `admits_consumes_once_from_path_a_then_a_bare_retry_finds_nothing_sealed`
proves the Path-A-shaped version of "resubmission is rejected": a second
`execute_step` on the same step, no re-seal, finds the row `lookup_sealed_handle`
would need already consumed and excluded by its own `status = 'sealed'`
filter — the practical, at-this-call-site meaning of "rejects
resubmission." Both properties (raw-handle replay at the adapter, and
Path A's own no-re-seal retry pattern) are covered; between them they
satisfy the brief's item 3(a) without gap.

---

## 3. Re-verification against current HEAD (independent, this session — every command run fresh, not copied from the prior session's doc)

### Build

```
$ cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.67s   # zero errors
```

### Clippy (scoped to the touched crates + ob-poc, matching this program's established convention — full-workspace `--all-targets` has pre-existing unrelated failures per every prior session's own note, re-confirmed still true, not re-litigated here)

```
$ cargo clippy -p ob-poc --lib --features database -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 42.70s   # zero warnings

$ cargo clippy -p ob-poc-control-plane --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.25s    # zero warnings
```

### The two G1-item-3/4 live-DB tests, from the real Path A call site

```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    -- --ignored --test-threads=1 g1_item2_path_a_tests
running 2 tests
test runbook::step_executor_bridge::g1_item2_path_a_tests::admits_consumes_once_from_path_a_then_a_bare_retry_finds_nothing_sealed ... ok
test runbook::step_executor_bridge::g1_item2_path_a_tests::no_sealed_envelope_for_this_entry_is_rejected_with_triage_classification ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 2373 filtered out; finished in 0.23s
```

### The adapter-level `t4_1` property set (unaffected by G1/G2/G4/G5 wiring — re-confirmed, not assumed)

```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    -- --ignored --test-threads=1 sem_os_runtime::verb_executor_adapter::
running 7 tests
test sem_os_runtime::verb_executor_adapter::t4_1_envelope_admission_tests::enforced_verb_with_consumed_envelope_admits_then_rejects_resubmission ... ok
test sem_os_runtime::verb_executor_adapter::t4_1_envelope_admission_tests::enforced_verb_without_envelope_is_rejected ... ok
test sem_os_runtime::verb_executor_adapter::t4_1_envelope_admission_tests::envelope_with_wrong_content_hash_is_rejected_loudly ... ok
test sem_os_runtime::verb_executor_adapter::t4_1_envelope_admission_tests::execute_verb_admitting_envelope_floor_rejects_an_unregistered_verb_before_any_scope_or_consume ... ok
test sem_os_runtime::verb_executor_adapter::t4_1_envelope_admission_tests::execute_verb_admitting_envelope_rejects_on_pin_drift_and_leaves_envelope_reconsumable ... ok
test sem_os_runtime::verb_executor_adapter::t4_1_envelope_admission_tests::execute_verb_admitting_envelope_rolls_back_the_consume_when_dispatch_fails ... ok
test sem_os_runtime::verb_executor_adapter::t4_1_envelope_admission_tests::shadow_default_admits_every_verb_with_no_envelope ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 2368 filtered out; finished in 0.13s
```

### Full lib suite (proves G4/G5's subsequent changes didn't regress anything, including these two tests' non-ignored neighbours)

```
$ cargo test -p ob-poc --lib --features database
test result: ok. 2169 passed; 0 failed; 206 ignored; 0 measured; 0 filtered out; finished in 26.08s
```

(2169 vs. the prior session's cited 2160 — the +9 is G4/G5's own new
tests landing on top, not a regression; confirmed by the intervening
commits' own stat summaries, `02816414`/`79f2d27f`.)

### `ob-poc-control-plane` crate-internal suite

```
$ cargo test -p ob-poc-control-plane --lib
test result: ok. 120 passed; 0 failed
```

(120 vs. the prior session's cited 116 — again G4/G5 additions, e.g.
`applicability.rs`'s new matrix tests.)

### Control-plane live-DB sweep

```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    control_plane -- --ignored --test-threads=1
test result: FAILED. 33 passed; 1 failed
```

The 1 failure is `agent::control_plane_metrics::t7_2_metrics_tests::e3_invariant_probe`,
panicking with `E3_INVARIANT_FAILURE: 1 gate(s) have zero substantive
production samples anywhere: ["AuditReplay"]; 1 gate(s) have samples only
at the WRONG provenance: ["WriteSetAttestation"]` — this is the **expected**
failure, matching `invariants-expected.toml`'s ratcheted `[e3] status =
"fail"` exactly (AuditReplay/G11 and WriteSetAttestation/G14 are
independently confirmed still-zero/wrong-provenance gates per the ledger's
existing G2-audit-provenance entry, unrelated to this session's subject).
33 (up from the prior session's 31) is consistent with G4/G5 adding more
live-DB control-plane tests.

### Plugin coverage + dep-gate

```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database -- test_plugin_verb_coverage
test result: ok. 1 passed; 0 failed

$ bash scripts/check_kyc_substrate_deps.sh
PASS: no forbidden deps in ob-poc-kyc-substrate
```

### `check-invariants.sh` ratchet (the literal script this program's reviewer independently re-runs)

```
$ DATABASE_URL=postgresql:///data_designer bash scripts/check-invariants.sh ratchet
  ... (E1-E5 evaluated) ...
  E5: DOES NOT HOLD
  [e5] actual=fail expected=fail — MATCH
== Ratchet: 0/5 invariant(s) diverge from invariants-expected.toml ==
```

Zero divergence — every one of E1–E5's actual status matches what
`invariants-expected.toml` records, both before and after this session
(this session made no code changes, so this is expected, but it was run
for real rather than assumed).

### Confirmation that the Path A call site still tags `ExecutionPath::RunbookSequencer` (G4's addition, checked because it changes the signature this session's tests dispatch through)

```
$ grep -n "ExecutionPath::RunbookSequencer\|admit_in_scope(" \
    src/sem_os_runtime/verb_executor_adapter.rs src/runbook/step_executor_bridge.rs
rust/src/runbook/step_executor_bridge.rs:608:                ob_poc_types::ExecutionPath::RunbookSequencer,
rust/src/sem_os_runtime/verb_executor_adapter.rs:639:            .admit_in_scope(verb_fqn, envelope_handle, path, scope.executor())
```

Confirms G4's `path` parameter reached the real Path A call site correctly
and the G1-item-3/4 tests are exercising the `RunbookSequencer` tag (the
same tag `EnforcedVerbs::from_env()`'s backward-compatible untagged-entry
semantics admit under, per G3's ratified grammar — the tests' bare
`"cbu.confirm"` env-var value, unchanged from the prior session, still
means "all paths" and continues to work after G4's reshape).

---

## 4. STOP-conditions hit

**One, of a different shape than the program's usual "the plan's approach
is unsafe" STOP**: the task brief's premise (only G1 items 1–2 landed) was
stale relative to `git log` at session start. Per this program's own
rule ("if you discover... the plan is contradicted by real code... do
not silently work around it or guess. Stop, document the finding
precisely... implement the narrowest safe resolution consistent with
fail-closed defaults"), the narrowest safe resolution here was: verify the
existing implementation independently and rigorously (§1–§3 above) rather
than (a) blindly re-implementing already-landed work, which would either
produce duplicate/conflicting test names and a spurious second migration,
or (b) silently doing nothing and reporting "done" without having checked
anything myself. I did neither — I re-ran every command fresh against
current HEAD (not copied from the prior doc) and independently confirmed
both the mechanism and the two named test properties hold, including
after two further tranches (G4, G5) changed adjacent signatures.

No other STOP-condition fired — no unsafe/ambiguous approach was
discovered in the existing implementation; the design doc's own
reasoning (§2–§8 of `EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001`) holds
up against the current code, re-checked line by line in §1 above rather
than trusted.

---

## 5. `invariants-expected.toml` — recommendations (not applied)

No new recommendation from this session — this session made no code
changes and the actual/expected status is a proven 0/5 ratchet match
(§3 above). Repeating, not re-deriving, the prior session's own still-
unapplied recommendation since it remains accurate at current HEAD:

**`[e2]`** — current detail text: *"Structural: 2/4 RR-2 paths (A, D) call
`execute_verb_admitting_envelope` at all; B, C have no admitting entry
point. Dynamic (live DB): Path D's admission mechanism works when enabled
but `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` is unset by production default
(NotEnforced) everywhere."* This is now doubly stale: it predates G1's
own seal→consume wiring (superseded by this session's re-confirmation)
**and** predates G4's landing (Paths B and C now *also* have an admitting
entry point at the `dsl_v2::executor::execute_verb_in_scope` seam — E2's
structural leg is 4/4, per the ledger's "G4 landed" entry, not 2/4 as the
comment still says). Status should **stay `fail`** (no verb is enforced
by production default on any path — pure capability, not enforcement).
Suggested wording, updated for both G1 and G4 (my own phrasing, not
copy-pasted from the prior session, since the prior session's suggested
text predates G4 and would itself be an undercount if adopted verbatim
now): *"Structural: 4/4 RR-2 paths (A, B, C, D) reach an admitting entry
point (G4 landed the B/C seam). Path A's seal→consume correlation is real
and `entry_id`-keyed (G1 items 2–4, live-DB proven, this doc's own
re-verification). Dynamic (live DB): every path's admission mechanism
works when enabled, but `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` is unset by
production default (NotEnforced) everywhere — enforce-capable on all
paths, not yet enforced on any."*

**`[e1]`/C-032** — unchanged from the prior session's recommendation
(G2 item 2's transport-wired-comparison-not-armed nuance); not re-derived
here since this session did not touch G2 item 2's subject.

**`[e3]`** — no change recommended; G11/AuditReplay and G14/WriteSetAttestation
remain at the same zero/wrong-provenance state this session's own sweep
re-confirmed (§3), consistent with the existing ledger entries.

---

## 6. Files changed

**None.** This session made no source, schema, or config edits — the
properties the task brief asked to be implemented were already
implemented, committed (`01539938`), and (as of this session)
independently re-verified against current HEAD, including after two
further tranches landed on top. The only artifact this session produces
is this doc.

Pre-existing untouched noise, confirmed left alone (per this session's
own instructions, not part of this task): `observatory-wasm/Cargo.lock`,
`rust/cbu_mismatches.json`, `rust/mismatches.json`,
`rust/reports/phase0_confusion_matrix.json`,
`rust/reports/step0_trial_evaluation.json`.

---

## 7. Open items (unchanged from the prior session, repeated so they don't silently drop)

- **HumanGate re-seal end-to-end live-DB test** (design doc §10 assertion
  4) — `reseal_for_human_gate_resume` is implemented and reuses the real
  seal helpers, but has no dedicated test proving the pre-park envelope
  is not the one consumed and a fresh one is. Recommend a follow-up
  session scoped specifically to building the park→approve harness.
- **Design doc §11 open question 1** — whether a `Durable` mid-dispatch
  park (distinct from `HumanGate`'s pre-dispatch park) ever re-enters a
  not-yet-consumed envelope on resume. Reasoned as correct-by-construction
  in the design doc, not yet asserted by a test.

Neither is required by this session's own literal brief (G1 items 3–4 as
worded above), and I did not attempt either given the time-budget
reasoning in §1.
