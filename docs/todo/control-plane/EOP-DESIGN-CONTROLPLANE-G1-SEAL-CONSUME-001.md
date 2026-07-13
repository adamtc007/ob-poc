# EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001

### Design doc, not implementation. Review before any code lands.
### Basis: `EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.4.md` §3 tranche G1, item 1 (GRADPLAN-D-006 — this is a design doc's worth of decisions, not inline session work). AD-1(a) is ratified (2026-07-13): G10 grades envelope *validity at consume time*.
### Status: RATIFIED (architect, 2026-07-13). Nothing in this doc has been implemented yet — ratification unblocks G1 items 2-4's grind work (GRADPLAN-D-006), it does not itself constitute implementation. §5 (retry/replay) and §6 (multi-step) rest on properties already proven by `t4_1_envelope_admission_tests` and T9.2/T10.2 — cited, not re-derived. Note recorded at ratification: §1.4's finding that the plan's own citation of the `evaluate_shadow`/`evaluate` MIGRATION-PENDING convergence is stale (closed 2026-07-11, before the plan was drafted) should be corrected in the plan's next revision — not done as part of this ratification, since defects in a plan are reported, not silently fixed by a design doc that depends on it.

---

## 0. What this doc resolves

Path A (the Sequencer's runbook step loop) seals a real, proof-carrying
`ExecutionEnvelope` and separately calls the admission entry point that
is supposed to consume it — but the two are wired to nothing. Concretely:

- `phase5_runtime_recheck` (`rust/src/sequencer.rs:7715`) calls
  `ob_poc_control_plane::decision::evaluate_with_report(&cp_ctx, validity)`
  at `rust/src/sequencer.rs:8015`. When the decision is
  `ApprovedStp(envelope)`, it spawns a **detached, fire-and-forget**
  `tokio::spawn` (`rust/src/sequencer.rs:8026-8038`) that calls
  `crate::agent::control_plane_envelope_store::persist_sealed(&pool,
  session_id, &entry_verb, &envelope)` — inserting a `status = 'sealed'`
  row into `"ob-poc".control_plane_envelopes`, keyed by `envelope_id`
  (a fresh `Uuid::new_v4()` minted inside `ExecutionEnvelope::seal`,
  `rust/crates/ob-poc-control-plane/src/envelope.rs:130`).
- `VerbExecutionPortStepExecutor::execute_step`
  (`rust/src/runbook/step_executor_bridge.rs:553`) calls
  `self.port.execute_verb_admitting_envelope(&step.verb, args, &mut ctx,
  None)` — the fourth argument, `envelope_handle: Option<EnvelopeHandle>`,
  is a **hardcoded `None`**. Nothing at this call site knows the
  `envelope_id` the spawned task above minted, let alone its content
  hash.

These two call sites execute inside the **same loop iteration, over the
same `RunbookEntry`**, a few function calls apart
(`rust/src/sequencer.rs:7096-7245`, `execute_entry_via_gate_impl` at
`rust/src/sequencer.rs:8151`) — but nothing carries a value between them.
Today this is inert: `EnforcedVerbs::from_env()`'s production default is
empty, so `admit_in_scope` (`rust/src/sem_os_runtime/
verb_executor_adapter.rs:136`) always returns `AdmissionDecision::
NotEnforced` regardless of the `None`. The moment any verb is added to
`OB_POC_CONTROL_PLANE_ENFORCE_VERBS`, `admit_in_scope` (line 169-192)
converts a missing handle into `AdmissionDecision::RejectedNoEnvelope`
(`control_plane_envelope_store.rs:98-100`), which
`execute_verb_admitting_envelope` turns into a hard `Err`
(`verb_executor_adapter.rs:182-186`) — **every dispatch of that verb via
Path A fails**, unconditionally. This is confirmed by code, not inferred:
`AdmissionDecision::NotEnforced` and `RejectedNoEnvelope` are distinct
enum variants (`control_plane_envelope_store.rs:52-64`) with different
downstream handling — there is no silent-pass branch for "enforced but no
handle." Graduating any verb before this gap closes is an outage on that
verb, per the plan's own framing (§3, G1 header) — this doc verifies that
framing against the actual code path, not just the plan's prose.

---

## 1. Current state (verified against code, this session)

### 1.1 The two call sites and what sits between them

`phase5_runtime_recheck` runs once per runbook entry inside the
Sequencer's per-index loop (`rust/src/sequencer.rs:7052`, `for idx in
start_index..session.runbook.entries.len()`):

```
for idx in start_index..entries.len():
    runtime_recheck = phase5_runtime_recheck(session, idx, ...)   # L7103 — seals, spawns persist (detached)
    if runtime_recheck.is_some(): break/fail
    match execution_mode:
        HumanGate  => park BEFORE dispatch, no seal-adjacent call    # L7146-7166
        Durable    => execute_entry_via_gate_impl(...)                # L7183
        Sync       => execute_entry_via_gate_impl(...)  (same shape)
```

`execute_entry_via_gate_impl` (`rust/src/sequencer.rs:8151`) resolves or
compiles the `CompiledStep`, then dispatches through
`execute_runbook_with_pool`/`execute_runbook_in_scope`
(`rust/src/sequencer.rs:8267` onward), which eventually calls
`VerbExecutionPortStepExecutor::execute_step`/`execute_step_in_scope`
(`rust/src/runbook/step_executor_bridge.rs:45,55,513`) — the site that
hardcodes `None`.

**The data structure between them is not a persistent struct — it's the
loop's own call stack**, for `Sync`/`Durable` entries: `phase5_runtime_
recheck` and the eventual `execute_step` call happen in the same `idx`
iteration, separated by ordinary function calls, not an event loop hop
or a queue. The plan's candidate ("sequencer entry state") does not
name an existing type — `RunbookEntry` (`session.runbook.entries[idx]`,
the only per-entry state that outlives one iteration) is a durable,
serialized, persisted struct (checkpointed via `persist_session_
required`, `rust/src/sequencer.rs:8422`) with no field for a live,
short-lived, single-use security handle, and adding one would mean a
sealed envelope's identity gets checkpointed to the session snapshot —
undesirable (see §2).

**`CompiledStep` cannot carry it either.** `CompiledStep`
(`rust/src/runbook/types.rs:136`) is content-addressed
(`content_addressed_id(&steps, &envelope)`, `types.rs` constructor) and
cached in a `RunbookStoreBackend` for reuse across retries/replay
(`execute_entry_via_gate_impl`'s `compiled_id` resolution,
`sequencer.rs:8189-8206`) — baking a single-use envelope handle into it
would make a cached/replayed step carry a stale or already-consumed
handle by construction.

### 1.2 Sealing is fire-and-forget, not synchronous

`persist_sealed` runs inside `tokio::spawn` (`sequencer.rs:8026-8038`),
not awaited by `phase5_runtime_recheck` before it returns. Its own doc
comment states the posture explicitly: "Best-effort... failures are
logged, never propagated" (`control_plane_envelope_store.rs:260-263`).
This means, as written today, even if `execute_step` somehow knew the
envelope id, the row might not exist yet in `control_plane_envelopes`
when the consume-side query runs — a genuine race, not a hypothetical
one, since the very next thing the same loop iteration does is call
`execute_entry_via_gate_impl`.

### 1.3 No entry-level correlation column exists

`"ob-poc".control_plane_envelopes` (`rust/migrations/
20260710_control_plane_envelopes.sql`) has columns `envelope_id`
(PK), `content_hash`, `session_id`, `verb_fqn`, `status`, `not_before`,
`not_after`, `consumed_at`, `void_reason`, `created_at` — **no column
correlates a sealed row to the specific `RunbookEntry`/`CompiledStep`
that produced it.** `persist_sealed`'s own signature only takes
`session_id` and `verb_fqn` (`control_plane_envelope_store.rs:271-276`).
A runbook with two entries dispatching the same verb FQN (e.g. two
`cbu.assign-role` steps) would have two `sealed` rows indistinguishable
by `(session_id, verb_fqn)` alone — a lookup by that pair is ambiguous
today, by construction.

### 1.4 The `evaluate_shadow`/`evaluate` split named in the plan is already closed — not this doc's problem

The plan's item-1 text asks this design doc to address "T10.1's
registered owed convergence (`evaluate_shadow()`/`evaluate()` as two
parallel entry points, MIGRATION-PENDING, target T10.2's admission-scope
wrapper)." Checked against the ownership ledger and the current code:
**this was closed on 2026-07-11**, two days before the plan (v0.4,
2026-07-13) was written, via `decision::evaluate_with_report`
(`rust/crates/ob-poc-control-plane/src/decision.rs:156-220`) — see the
ledger's "Addendum C — owed `evaluate_shadow()`/`evaluate()` convergence
CLOSED" entry (`docs/research/control-plane-ownership-ledger.md:512-528`).
`sequencer.rs:8015` already calls `evaluate_with_report` once, using its
`report` for the shadow-decision audit row and its `decision` for the
sealing branch (confirmed by reading the call site directly, `sequencer.
rs:7990-8015`) — not two separate `evaluate_shadow` + `evaluate` calls.
`evaluate` itself is now a one-line wrapper over `evaluate_with_report`
(`decision.rs:134`). This is a **deviation from the plan's own framing**:
the plan asks this doc to "converge or consciously not converge" a split
that no longer exists at HEAD. §7 records this as the answer to
question (e) — there is nothing left to converge on that axis. The
actual owed-convergence-shaped gap this doc *does* need to resolve is a
different one: the seal call (`phase5_runtime_recheck`) and the consume
call (`step_executor_bridge.rs:553`) are two separate, uncorrelated
call sites into the *same underlying admission mechanism* — see §1.1.
That is this doc's actual subject.

### 1.5 The atomicity mechanism this design must not re-invent

`t4_1_envelope_admission_tests` (`rust/src/sem_os_runtime/
verb_executor_adapter.rs:1224` onward) and T9.2/T10.2's landed code
(same file, `execute_verb_admitting_envelope`, lines 540-600ish) already
prove, live-DB:

- Single-use: a second consume attempt against the same handle returns
  `AlreadyConsumed` (`enforced_verb_with_consumed_envelope_admits_then_
  rejects_resubmission`, line ~1315).
- Content-hash binding: a handle with the right id but wrong hash is
  rejected, not silently admitted (`envelope_with_wrong_content_hash_is_
  rejected_loudly`, line ~1360).
- Rollback-together: a dispatch failure after successful admission rolls
  the whole `PgTransactionScope` back, including the consume — the
  envelope is reconsumable afterward, not burned
  (`execute_verb_admitting_envelope_rolls_back_the_consume_when_dispatch_
  fails`, line ~1402).
- Pin-drift rejection leaves the envelope reconsumable too
  (`execute_verb_admitting_envelope_rejects_on_pin_drift_and_leaves_
  envelope_reconsumable`, line ~1535).

**This design's whole job is getting a real `EnvelopeHandle` to this
already-correct mechanism from Path A's seal site — not building a new
consume mechanism.** Every decision below is scoped to the carrier, not
the consume-time semantics (already settled by AD-1(a) + T9.2/T10.2).

---

## 2. Design principle

**The correlation carrier is the persistence layer already built for
this purpose, extended with the one correlation dimension it's
missing — not a new in-process struct.** `EnvelopeHandle` is
deliberately serializable specifically so it can "cross a persistence...
boundary" (`envelope_handle.rs`'s own module doc, lines 1-19) and
`EnvelopeRecord`/`persist_sealed`/`try_consume` already implement
exactly that pattern for the id+hash+status+window. What's missing is
not a new transport mechanism — it's (1) a column correlating a sealed
row to the specific step that produced it, and (2) making the seal
synchronous with respect to the loop iteration that will immediately
try to consume it, so the DB row genuinely exists by consume time. This
keeps `StepExecutor`'s trait signature (`execute_step`/
`execute_step_in_scope`, `rust/src/runbook/executor.rs:1518,1526`)
untouched — no ripple through its three implementations
(`VerbExecutionPortStepExecutor` plus the two test stubs in `executor.
rs`'s own test module) — and keeps `CompiledStep` untouched (§1.1's
caching concern).

```
phase5_runtime_recheck(session, idx, entry_id, ...):
    ...
    (report, decision) = evaluate_with_report(&cp_ctx, validity)   # unchanged
    if let ApprovedStp(envelope) = decision:
        AWAIT persist_sealed(pool, session_id, entry_id, verb_fqn, &envelope)   # was: tokio::spawn (detached)
        # entry_id is NEW — see §2.1 schema change
    ...

VerbExecutionPortStepExecutor::execute_step(step: &CompiledStep):
    handle = AWAIT lookup_sealed_handle(pool, self.session_id, step.step_id)  # NEW — step.step_id == entry_id (types.rs:139)
    self.port.execute_verb_admitting_envelope(&step.verb, args, &mut ctx, handle)
```

### 2.1 Schema change (stated now, not deferred to implementation)

Add `entry_id UUID` to `"ob-poc".control_plane_envelopes`
(new migration; forward-only per CLAUDE.md's migration discipline — this
is a new column, not an edit to `20260710_control_plane_envelopes.sql`),
nullable for backward compatibility with any pre-existing row (none in
production today — the table is shadow-sealing-only, per §1.4's closed
convergence note, so a backfill is not required), with a supporting
index `(session_id, entry_id, status)` for the lookup in `execute_step`.
`persist_sealed` gains an `entry_id: Uuid` parameter; the new
`lookup_sealed_handle(pool, session_id, entry_id) -> Option<EnvelopeHandle>`
function is a straightforward `SELECT envelope_id, content_hash FROM
control_plane_envelopes WHERE session_id = $1 AND entry_id = $2 AND
status = 'sealed' ORDER BY created_at DESC LIMIT 1` — `ORDER BY... LIMIT
1` as defence-in-depth against a genuine double-seal (§4/§5 below
establish this shouldn't happen by construction, but the query should
not panic on multiple rows if it ever does).

`CompiledStep.step_id` (`rust/src/runbook/types.rs:139-140`, doc
comment: "Stable step ID (from the originating `RunbookEntry.id`)") is
already the entry's UUID — `VerbExecutionPortStepExecutor::execute_step`
receiving `step: &CompiledStep` already has everything needed
(`step.step_id`, `step.verb`, and `self.session_id`, the field the
bridge is already constructed with — confirmed by its existing use in
`execute_step`, `step_executor_bridge.rs:517`) **without any new
parameter threaded through the `StepExecutor` trait.**

---

## 3. (a) Where does the handle live between seal and consume?

**In `"ob-poc".control_plane_envelopes`, correlated by
`(session_id, entry_id)` — a DB row, not an in-process value.** The
plan's candidate, "sequencer entry state," does not name an existing
type at HEAD (§1.1); the closest such type, `RunbookEntry`, is a
durable/checkpointed struct where a live security handle does not
belong (checkpointing a consumable-once handle risks a stale replay of
the session snapshot resurrecting a handle whose underlying row has
already changed status — a self-inflicted version of the exact race
this design exists to close). The DB-mediated design also matches the
crate's own stated intent for `EnvelopeHandle` (§2) and requires zero
signature changes to `StepExecutor`, `CompiledStep`, or any of their
existing call sites beyond the two touched directly (§2's pseudocode).

**Rejected alternative:** thread `Option<EnvelopeHandle>` as an explicit
parameter through `execute_entry_via_gate_impl` → `run_through_gate` →
`execute_runbook_with_pool`/`execute_runbook_in_scope` →
`StepExecutor::execute_step`/`execute_step_in_scope`. This is a wider
signature change (the trait is implemented by `VerbExecutionPortStepExecutor`
plus test stubs in `executor.rs`'s own module, and `execute_runbook_
with_pool`/`execute_runbook_in_scope` iterate over *multiple* steps per
call — the parameter would need to become per-step-in-a-loop state
inside those functions too, not a single argument). Rejected because
it's a larger blast radius for the same outcome the DB-mediated design
achieves with one new column and one new query.

## 4. (b) Lifetime/expiry when the seal→consume gap exceeds the validity window

The 5-minute window is confirmed at the seal call site itself:
`ValidityWindow::new(chrono::Utc::now(), chrono::Utc::now() +
chrono::Duration::minutes(5))` (`sequencer.rs:8009-8012`), with the
inline comment "matches this crate's own test convention (no production
TTL policy exists yet to draw from)."

**For `Sync`/`Durable` entries, the gap is sub-second by construction**
once §2's synchronous-await fix lands: seal and the eventual consume
happen in the same `idx` loop iteration, separated only by
`execute_entry_via_gate_impl`'s compile-resolution and gate-store setup
(no network I/O beyond the DB itself, no user-facing wait). The 5-minute
window is generous headroom for this path, not a tight budget — no
special-casing needed.

**For `HumanGate` entries, the gap is unbounded by construction, and
this is the real case this question is about.** `phase5_runtime_recheck`
runs unconditionally before the `match execution_mode` block
(`sequencer.rs:7096-7245`), so a `HumanGate` entry gets shadow-sealed
*before* the "Park BEFORE execution — DSL is NOT called" branch
(`sequencer.rs:7146-7147`) parks it — potentially for hours or days
awaiting human approval. On approval, `handle_human_gate_approval`
(`sequencer.rs:3500-3552` — confirmed by direct read) calls
`session.runbook.resume_entry(...)` then `self.execute_entry_via_gate(
entry_ref, ...)` **directly** — it does not call `phase5_runtime_recheck`
again. Any envelope sealed at park time would be 5-minutes-or-more stale
by the time this path reaches `execute_step`.

**Decision: re-seal at resume, never extend or reuse a
pre-park envelope.** Concretely, `handle_human_gate_approval` must run
the shadow-evaluate-and-seal step (a call to `evaluate_with_report` +
`persist_sealed`, i.e. the same logic `phase5_runtime_recheck` runs,
factored so both call sites share it — see §8 rather than duplicating
`phase5_runtime_recheck`'s body) **immediately before** calling
`execute_entry_via_gate`, so the seal-to-consume gap for a resumed
`HumanGate` entry is the same sub-second shape as the `Sync`/`Durable`
path, not the wall-clock duration the approval took. This is chosen
over the plan's other two named options for a concrete reason each:
- **Extend** (push `not_after` forward on the existing sealed row) is
  rejected because the envelope's proof values (`Authorised`,
  `EvidenceSufficient`, `WriteSetProof`, `SnapshotPins`, ...) were
  derived from state read at original-seal time — extending validity
  without re-deriving those proofs would silently authorise dispatch
  against facts that may have gone stale during the human-approval
  wait (the exact TOCTOU shape T9.2's §5a already exists to close one
  layer down — extending here would reopen an equivalent gap one layer
  up).
- **Reject-and-retriage** (fail the resumed dispatch outright if the
  window lapsed, routing to runbook §7 triage) is rejected as the
  *default* because it would make ordinary human-approval latency
  (minutes to days, by design — that's what `HumanGate` is for) into a
  routine failure mode; it is, however, exactly what should happen if
  re-sealing itself fails (see §5) — not a design fork, a fallback.

No separate "TTL policy" work is needed to accompany this: re-sealing at
resume means the 5-minute window's origin — a test-convention default,
not a load-bearing production number — never actually has to survive
a human-timescale gap in practice.

## 5. (c) Retry/replay: distinguishing a legitimate retry from a replay attack

**Mechanism: the `entry_id`-correlated `control_plane_envelopes` row
plus per-attempt re-sealing (fresh `Uuid::new_v4()` id, fresh content
hash) — no separate idempotency key is needed beyond what already
exists.** Two cases:

- **Legitimate retry** (a runbook step is re-dispatched because a prior
  attempt failed, or a user explicitly re-runs `/run` after fixing an
  entry): this always re-enters `phase5_runtime_recheck` for that
  `entry_id` (either via the ordinary per-idx loop on a fresh
  `execute_runbook_from` call, or via §4's re-seal-at-resume for
  `HumanGate`). Each call to `ExecutionEnvelope::seal` mints a new
  `Uuid::new_v4()` (`envelope.rs:130`) — a retry is, by construction, a
  **new admission decision derived from current facts**, not a replay of
  an old one. `lookup_sealed_handle`'s `ORDER BY created_at DESC LIMIT 1`
  (§2.1) picks up the freshest sealed row for that `entry_id`, so a
  retry naturally consumes its own fresh envelope, never a stale one.
  Note also the T9.2 rollback-together property (§1.5): if a prior
  attempt's dispatch failed *after* admission consumed its envelope, the
  whole scope rolled back, so that specific envelope is reconsumable —
  a same-envelope retry (not just a same-entry retry) would also
  legitimately succeed, though the re-seal-per-iteration design above
  means this case is not actually exercised on Path A's own retry path
  (a fresh seal always happens first).
- **Replay attack** (a captured, previously-*successfully*-consumed
  `EnvelopeHandle`, id+hash, re-submitted to force a second dispatch):
  rejected by the already-proven `AlreadyConsumed` outcome
  (`try_consume`/`try_consume_in_scope`'s status check,
  `enforced_verb_with_consumed_envelope_admits_then_rejects_resubmission`,
  §1.5) — this is exactly the property T4.2/T9.2 built and proved; this
  design doesn't add anything new here, it only makes sure Path A's real
  handle (not a hardcoded `None`) reaches that check.

**What would need to be added if this were insufficient (explicitly not
recommended, named for completeness):** a distinct idempotency key
(e.g. `(session_id, entry_id, attempt_number)`) stamped into the
envelope's own proof-bearing content so a stolen handle could be
distinguished from a legitimate retry even if content-hash binding were
somehow defeated. Not recommended: content-hash binding is already a
SHA-256 over the full serialized envelope (`envelope.rs:189-195`),
which already includes `intent`/`binding`/`pack`/... — forging a
content-hash-valid handle without having genuinely sealed that exact
envelope is not a gap this design needs to additionally guard against;
that would be re-litigating T8.1's already-closed PIR-D-008/PIR-D-010
finding, not new scope for G1.

## 6. (d) Multi-step runbooks: per-step or plan-level envelope?

**Recommendation: per-step, matching the shape already established by
Paths A and D (per G4's own text: "strengthen B/C from... plan-level
pre-flight admission to the per-step atomic consume property A/D
have").** This is not a new choice this doc is introducing — it is
already the architecture `phase5_runtime_recheck`'s per-`idx` loop
implements today (one `EvaluationContext`/`seal` call per
`RunbookEntry`, `sequencer.rs:7715` runs once per entry) and what §2's
design threads through unchanged.

Tradeoff, stated per the plan's own framing:
- **Per-step** (this doc's recommendation): each step's admission
  decision is derived from facts current *at that step's own dispatch
  time* — later steps in the same runbook see whatever state earlier
  steps' writes produced, which is the correct semantics for a runbook
  where step N+1's preconditions may depend on step N's effects (the
  DAG-taxonomy cascade/gate machinery this codebase already has,
  `docs/annex-sem-os.md`'s GatePipeline). Partial-plan rollback (some
  steps committed, a later step fails) is exactly the shape the
  Sequencer's existing per-entry `EntryStatus`/`StepResult` bookkeeping
  and the B.2b-ζ "halt + rollback on failure" behaviour
  (`sequencer.rs:7135-7139`, `7452-7458`) already handle — no new
  partial-rollback design is needed.
- **Plan-level** (one envelope covering the whole compiled runbook,
  sealed once before any step dispatches): would reduce sealing
  overhead (one `evaluate_with_report` call instead of one per step),
  but requires either (i) deriving every step's proofs against
  pre-execution facts only — wrong for a runbook whose later steps
  depend on earlier steps' writes — or (ii) re-deriving a fresh
  plan-level envelope after every step commits anyway, which is just
  per-step sealing with extra bookkeeping. T9.3's existing plan-level
  `admit_plan`/`admit_plan_checked` (`control_plane_envelope_store.
  rs:180-230`) already covers the "reject early, whole-plan-walk"
  defence-in-depth case for Paths B/C — this design doesn't need a
  second plan-level mechanism to get that property on Path A; the
  per-step atomic consume this doc wires is additive to, not a
  replacement for, that outer check.

No architect fork here — the existing per-entry loop shape is already
the only architecturally sound choice given later steps may legitimately
depend on earlier steps' writes; this section records the reasoning for
review, not a genuinely open decision.

## 7. (e) T10.1's registered owed convergence (`evaluate_shadow()`/`evaluate()`)

**Already converged — closed 2026-07-11, two days before the plan
that asks this doc to address it was written (v0.4, 2026-07-13).** See
§1.4 for the full citation trail: `decision::evaluate_with_report`
(`decision.rs:156-220`), the ledger's "Addendum C... CLOSED" entry
(`docs/research/control-plane-ownership-ledger.md:512-528`), and
`sequencer.rs:8015`'s single call site confirmed by direct read. This
doc records the correction rather than silently no-op'ing the plan's
instruction, per the rules of evidence (verify, don't trust a prior
doc's summary) — the plan's own citation of this as open is stale
relative to current HEAD, not wrong at the time the underlying ledger
entries were being written (T10.1's B2 ratification long predates the
plan; the plan's author appears to have cited the *ratification
condition* text without checking whether Addendum C had already
resolved it by the time v0.4 was drafted).

**This doc's actual subject — the seal-site/consume-site correlation
gap described in §1.1-§1.3 — is a different, still fully open item.**
It is not itself a "two parallel entry points into overlapping logic"
shape (the PIR-D-008-family pattern the plan's evaluate_shadow/evaluate
language describes); it's a missing wire between two call sites that
both, correctly, use the single `evaluate_with_report` path. §2-§6
above are this doc's resolution of that actual gap.

## 8. (f) "No new crate edges expected" — confirmed

**Confirmed correct.** Every file this design touches or reasons about
lives in the `ob-poc` root crate (`rust/Cargo.toml`'s package, not a
sub-crate under `crates/`):

- `rust/src/sequencer.rs` (seal site, `phase5_runtime_recheck`;
  `handle_human_gate_approval`'s new re-seal call, §4)
- `rust/src/runbook/step_executor_bridge.rs` (consume site)
- `rust/src/agent/control_plane_envelope_store.rs` (`persist_sealed`,
  new `lookup_sealed_handle`)
- `rust/src/runbook/types.rs` (`CompiledStep.step_id`, read-only — no
  change needed, §2.1)
- `rust/src/runbook/executor.rs` (`StepExecutor` trait — read-only, no
  signature change, §3)

The two external types this design threads (`EnvelopeHandle` from
`ob_poc_types`, `ExecutionEnvelope`/`ValidityWindow` from
`ob_poc_control_plane`) are already direct dependencies of the `ob-poc`
crate — confirmed in `rust/Cargo.toml`: `ob-poc-types = { path =
"crates/ob-poc-types" }` (line 22) and `ob-poc-control-plane = { path =
"crates/ob-poc-control-plane" }` (line 25), and both are already
imported and used directly in `sequencer.rs` and `verb_executor_adapter.
rs` today (`ob_poc_control_plane::decision::evaluate_with_report`,
`ob_poc_types::EnvelopeHandle`). No `Cargo.toml` of any crate needs a
new `[dependencies]` line for this design. The one new migration
(§2.1) is schema, not a crate edge.

---

## 9. Worked example / sequence trace

A two-step runbook, `[cbu.confirm, cbu.assign-role]`, both `Sync`,
`cbu.confirm` enforced via `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`:

```
idx=0, entry_id=E1, verb=cbu.confirm
  phase5_runtime_recheck(session, 0, E1, ...)
    evaluate_with_report(cp_ctx, validity=[now, now+5m]) -> (report, ApprovedStp(env1))
    AWAIT persist_sealed(pool, session_id, E1, "cbu.confirm", env1)
      INSERT control_plane_envelopes(envelope_id=env1.id, entry_id=E1,
             session_id, verb_fqn="cbu.confirm", status='sealed', ...)
  execute_entry_via_gate_impl(entry=E1, ...)
    -> execute_runbook_with_pool(...) -> VerbExecutionPortStepExecutor::execute_step(step: step_id=E1, verb="cbu.confirm")
         handle = lookup_sealed_handle(pool, session_id, E1)   # finds env1's row, still < 1s old
         port.execute_verb_admitting_envelope("cbu.confirm", args, ctx, Some(handle))
           admit_in_scope: EnforcedVerbs contains "cbu.confirm" -> try_consume_in_scope_with_pins(handle)
             row found, status='sealed', hash matches, not_after not lapsed -> Consumed
           verify_pins_in_scope(...) -> ok
           dispatch cbu.confirm's write -> COMMIT (consume + write atomic, T9.2)
         -> StepOutcome::Completed

idx=1, entry_id=E2, verb=cbu.assign-role (not enforced)
  phase5_runtime_recheck(session, 1, E2, ...)
    evaluate_with_report(...) -> (report, decision)   # sealed or not, doesn't matter — not enforced
    AWAIT persist_sealed(...) if ApprovedStp             # shadow bookkeeping only
  execute_entry_via_gate_impl(entry=E2, ...)
    -> execute_step(step_id=E2, verb="cbu.assign-role")
         handle = lookup_sealed_handle(pool, session_id, E2)   # may be Some or None, irrelevant
         port.execute_verb_admitting_envelope("cbu.assign-role", args, ctx, handle)
           admit_in_scope: EnforcedVerbs does NOT contain "cbu.assign-role" -> NotEnforced
           dispatch proceeds regardless of handle presence
```

`HumanGate` variant (entry E3, `kyc-case.approve`, enforced):

```
idx=2, entry_id=E3, verb=kyc-case.approve, execution_mode=HumanGate
  phase5_runtime_recheck(session, 2, E3, ...)     # seals env3, persists (entry_id=E3) — may go stale while parked
  match HumanGate: park_entry(E3, ...); break      # DSL NOT called; execute_step never reached here

... hours later, human approves ...

handle_human_gate_approval(session, E3, approved_by)
  resume_entry(...)
  [NEW] re-seal: evaluate_with_report(cp_ctx', validity=[now', now'+5m]) -> ApprovedStp(env3b)
        AWAIT persist_sealed(pool, session_id, E3, "kyc-case.approve", env3b)   # fresh envelope_id, entry_id=E3
  execute_entry_via_gate(entry_ref=E3, ...)
    -> execute_step(step_id=E3, verb="kyc-case.approve")
         handle = lookup_sealed_handle(pool, session_id, E3)   # ORDER BY created_at DESC picks env3b, not the stale env3
         port.execute_verb_admitting_envelope(..., Some(env3b handle))   # sub-second gap, well inside 5m window
```

---

## 10. Test plan (for G1 item 3's live-DB test)

The plan's item 3 asks for "the `t4_1` property set now proven *from
the Path A call site*, not just from the adapter's own tests." Concrete
assertions, all live-DB (`#[ignore = "requires DATABASE_URL"]`, matching
existing convention):

1. **End-to-end admit-and-consume from Path A**: construct a real
   `ReplSessionV2` with a one-entry runbook dispatching an
   `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`-enforced verb; drive it through
   `phase5_runtime_recheck` + `execute_entry_via_gate_impl` (or the
   full `process()` path if that's a cleaner harness seam); assert the
   verb dispatches successfully and `control_plane_envelopes` shows the
   sealed row transitioned to `status = 'consumed'`.
2. **No-envelope rejection, from the real call site**: with the verb
   enforced but `phase5_runtime_recheck` prevented from running (or its
   seal deliberately made to fail/return non-`ApprovedStp`, e.g. by
   forcing a `RequiresHumanGate`/`Rejected` decision), assert the
   subsequent dispatch attempt fails with the `RejectedNoEnvelope`-shaped
   error — proving the outage framing (§0) is real, not hypothetical,
   and stays real until a verb graduates.
3. **Single-use from Path A**: dispatch the same compiled step twice in
   the same session/entry without an intervening re-seal (simulate a
   caller bug); assert the second attempt's `lookup_sealed_handle` finds
   the now-`consumed` row unusable and the retry path (§5) re-seals
   rather than reusing it — i.e., assert the *system's* behaviour is
   "fresh seal per attempt," not that a raw resubmission of the same
   handle is even attempted from this call site (that raw-resubmission
   property is already proven at the adapter level, §1.5 — this test is
   about Path A's actual call pattern, which never manufactures that
   scenario by construction).
4. **HumanGate re-seal, not reuse**: park a `HumanGate` entry, sleep or
   otherwise simulate wall-clock time exceeding 5 minutes (or lower the
   test's `ValidityWindow` to make this practical — e.g. inject a short
   window via a test-only seam, matching `test_pool()`'s existing
   pattern of environment-gated test setup), approve it, and assert (i)
   the entry's pre-park envelope is not the one consumed (its row is
   still `sealed`/now `expired`, untouched) and (ii) a fresh envelope
   row (same `entry_id`, new `envelope_id`) was sealed at resume and is
   the one that transitions to `consumed`.
5. **Non-eligible decisions reject with triage classification (item 4
   of G1's own list)**: force `evaluate_with_report` to return
   `RequiresHumanGate`/`Rejected` for an enforced verb (no envelope ever
   sealed); assert `execute_step` surfaces a rejection classified per
   runbook §7 (not a bare dispatch error indistinguishable from an
   unrelated failure) — the error message shape from `admit_in_scope`
   (`"{verb_fqn} is enforce-mode gated... but no sealed envelope was
   presented"`, `verb_executor_adapter.rs:184`) should be asserted
   verbatim or by a stable substring, matching this module's existing
   test convention (`assert!(err.to_string().contains(...))`).

---

## 11. Open questions for architect ratification

None genuinely undetermined at this pass — §3-§8 each name a concrete
decision with rationale. Two items are flagged as **implementation-time
verification, not open design forks**, since they were reasoned about
but not executed against a live DB this session:

1. Whether `execute_entry_via_gate_impl`'s `Durable` branch (which can
   itself return `StepOutcome::Parked` *mid-dispatch*, distinct from
   `HumanGate`'s *pre-dispatch* park — §4/`sequencer.rs:7183` vs.
   `7146`) ever re-enters a *not-yet-consumed* envelope on resume.
   Reasoned in §4/§9 that `Durable` park happens *after* admission
   (the envelope is already consumed by the time a durable verb's own
   logic decides to park), so `continue_execution`'s `execute_runbook_
   from(session, idx + 1)` (`sequencer.rs:8375-8389`, resumes at the
   *next* index, never re-dispatching the parked entry) is correct as
   read — but this should get its own assertion in the test plan
   alongside item 4, not just this doc's citation-based reasoning,
   before G1 item 2 is considered proven.
2. The exact wording/placement of the shared seal-and-persist helper
   §4 references (factored out of `phase5_runtime_recheck` so
   `handle_human_gate_approval` can call it without duplicating the
   `EvaluationContext`-building logic) is an implementation detail this
   doc does not need to pre-specify — the shape (evaluate_with_report +
   persist_sealed, parameterised by entry/session) is what matters, not
   the function boundary.
