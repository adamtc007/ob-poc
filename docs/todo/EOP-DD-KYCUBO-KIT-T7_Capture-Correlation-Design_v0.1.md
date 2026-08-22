# EOP-DD-KYCUBO-KIT-T7 — Capture Correlation Design v0.1

**Status:** RATIFIED 2026-08-17 — Q1(a) binary v0 (Accepted/Rejected only; `Edited`
stays unreachable until a future tranche's proposals carry argument content),
Q2 record nothing on silent abandonment ("nothing actually happened"), Q3
sliced: **slice 1** = state map + `Stage` resolution (this implementation
pass); `Discard`/supersede handling (§3.3's "record the outgoing pending
proposal as Rejected" and §3.4's `Discard` arm) is an explicit fast-follow,
not done here.

**Slice 1 LANDED 2026-08-17.** `PendingProposal` (`kyc_workbook_surface.rs`,
`pub(crate)`), `Sequencer::kyc_pending_proposals` (same lifecycle/lock-order
convention as `kyc_workbooks`), `Propose` writes on `Select`/`Clarify` only,
`Stage` resolves+records+clears on all three of its outcomes (matched
verb → Accepted, mismatched verb → Rejected, `NotCurrentlyLegal` →
Rejected/no `staged_move_id`) and leaves the pending entry untouched on any
other `Err`. `record_capture` failures are best-effort (`tracing::warn!`,
never fail the `Stage` command — the real KYC action has already resolved by
the time capture runs). 6 new tests, all RED-proven where the logic branches
(mismatch detection, untouched-on-error); full lib suite green (1838/0/221,
one unrelated pre-existing flaky lock-contention test excluded). `Discard`
resolution and supersede-on-fresh-`Propose` remain the fast-follow.

**Closes:** the deferred wiring gap left open at the end of T7.2 (`EOP-PLAN-KYCUBO-KIT-T7_Plain-English-Ramp_v0.1.md` §2 T7.1 — "recorder built, dispatch wiring deferred") — `record_capture()` exists and is tested (`kyc_ramp_capture.rs`), `KycWorkbookCommand::Propose{utterance}` exists and is tested (`kyc_workbook_surface.rs`), but nothing yet calls the former from the latter, because `record_capture` needs a `user_action: Accepted | Edited | Rejected` that isn't knowable at the moment `Propose` returns — it's only knowable once the operator does something *next*.

This document proposes the correlation mechanism: how a proposal's fate gets tracked between the `Propose` call and whatever REPL input follows it, and how that later input resolves into a `CaptureRecord`.

---

## 1. The problem, precisely

`Propose` is a single request/response call — parse utterance, rank board, decide disposition, return text. It has no session it opens the way `Open`/`Discard` do. But per the ratified capture charter (`EOP-DD-KYCUBO-CAPTURE-CHARTER_v0.1.md` §1), every capture record needs a `user_action`, and that's a fact about a *later* turn, not this one.

So we need:
1. Somewhere to remember "session S was just shown proposal P" between one REPL turn and the next.
2. A rule for reading the operator's next action against that memory and deciding Accepted / Edited / Rejected (or: not recording at all).
3. A point where `record_capture` actually gets called, and what clears the memory once it has.

## 2. A real gap in the charter's schema, found while designing this

The capture charter's `user_action` is a 3-way enum, and T7.1's own doc comment on it says: *"an edited-then-accepted proposal is a near-miss, not a hit."* That assumes a proposal that could plausibly be *edited* — i.e., one that includes something with editable content (argument values, a target).

But T7.2's I-4 ruling (`EOP-PLAN-KYCUBO-KIT-T7_Plain-English-Ramp_v0.1.md` §1) is explicit: *"args stay resolver-owned... no fuzzy match"* — and `render_proposal` (`kyc_workbook_surface.rs`) only ever names a verb (`Select(move_id)` → one verb; `Clarify([...])` → 2+ verb candidates). There is no argument content in a T7.2 proposal to diff an "edit" against. The operator composes the entire `kyc-workbook.stage <dsl-text>` themselves regardless of what was proposed.

At the current proposal granularity, "edited" has no referent. Three ways to resolve this, not decided here:

- **(a) Binary v0.** Drop `Edited` from what T7.2 ever emits — only `Accepted`/`Rejected` are reachable until a future tranche's proposals carry argument content worth diffing. The charter's enum stays 3-way (schema unchanged, no migration), `Edited` simply has zero rows until then.
- **(b) Reinterpret `Edited` for `Clarify`.** Treat `Select`-then-matching-stage as `Accepted`, and `Clarify`-then-stage-one-of-the-offered-candidates as `Edited` (the operator resolved an ambiguity the ramp couldn't) — `Rejected` reserved for "staged something the disposition didn't even offer."
- **(c) Defer the whole question.** Don't wire capture into `Propose` at all yet; wait for a proposal surface with real argument content (a later tranche) where "edited" is unambiguous.

**Recommendation: (a).** It's the smallest change, keeps the charter's schema untouched, and doesn't manufacture a semantic distinction ((b)) that isn't really about editing. Flagging as **Q1** below rather than deciding unilaterally, since it's a reading of an already-ratified charter's field semantics.

## 3. Proposed mechanism

### 3.1 State: a second session-scoped map, same ownership shape as `kyc_workbooks`

```rust
// on Sequencer, alongside the existing kyc_workbooks field:
#[cfg(feature = "database")]
kyc_pending_proposals: Arc<RwLock<HashMap<Uuid, PendingProposal>>>,
```

```rust
// kyc_workbook_surface.rs (or a new sibling module) — NOT on KycWorkbook itself.
// KycWorkbook lives in the zero-inference-gated crate::domain_ops::kyc_workbook
// module; a PendingProposal carries ramp concepts (disposition, rendered
// proposal text) that must never leak into that module (I-5: ramp -> workbook
// types only, never the reverse). Kept as its own map at the same layer
// kyc_workbook_surface.rs already owns dispatch from.
struct PendingProposal {
    utterance_text: String,
    placement_set_hash: String,     // board.board_hash.to_hex()
    proposal: String,               // render_proposal()'s output, verbatim
    disposition: CaptureDisposition,// Select | Clarify (never Abstain — see 3.3)
    candidate_verb_fqns: Vec<String>, // 1 entry for Select, 2+ for Clarify
    created_at: DateTime<Utc>,
}
```

Same lifecycle rationale as `kyc_workbooks`: process-lifetime, in-memory, keyed by REPL session id, deliberately not part of `ReplSessionV2`/persisted session state — a proposal's pending status is exactly as ephemeral as an unstaged workbook edit.

### 3.2 `dispatch()` gains a second map parameter

```rust
pub(crate) async fn dispatch(
    cmd: KycWorkbookCommand,
    workbooks: &mut HashMap<Uuid, KycWorkbook>,
    pending_proposals: &mut HashMap<Uuid, PendingProposal>,   // NEW
    session_id: Uuid,
    pool: &sqlx::PgPool,
    principal: &RuntimePrincipal,
    as_of: DateTime<Utc>,
    utterance_embedding: Option<&[f32]>,
) -> Result<String, SurfaceError>
```

`sequencer.rs::handle_kyc_workbook_command` threads `self.kyc_pending_proposals.write().await` through, mirroring exactly how it already threads `self.kyc_workbooks.write().await`.

### 3.3 What `Propose` does differently

- `Select(move_id)` and `Clarify([...])` outcomes: build a `PendingProposal` and `.insert(session_id, ...)` — **overwriting** any prior pending proposal for this session. Per §3.5, overwriting a live pending proposal without it ever being staged records the old one as `Rejected` first (never silently dropped).
- `Abstain`: no `PendingProposal` is stored. There is no candidate to later accept or reject — per the charter, "per interaction" capture is scoped to interactions that *proposed* something. (This is the same reasoning T7.1 already applied implicitly — `Disposition` in `kyc_ramp_capture.rs` is 3-way including `Abstain`, but nothing requires every disposition to be captured; only ones a `user_action` can meaningfully attach to are.)

### 3.4 What resolves a pending proposal into a `CaptureRecord`

Only `Stage` and `Discard` are resolution points for v0 — the two commands that represent a concrete, attributable next action. (`Open`/`Show`/`Validate`/`Run`/`Commit` do not resolve or clear a pending proposal; `Commit` in particular ships whatever was already staged, which itself was already resolved at `Stage` time.)

**`Stage { text }`** (after the existing parse-and-recognise logic already in the arm today):
1. If no pending proposal exists for this session → stage proceeds exactly as it does today, nothing recorded (never gate ordinary staging on a proposal existing — I-1's "ramp is optional" would break if it did).
2. If a pending proposal exists:
   - Parse `text` far enough to read its verb_fqn (the existing `stage()` call already does this internally; this reads the same information without duplicating recognition logic — see §3.6 on avoiding a second parser call).
   - If the staged verb_fqn ∈ `candidate_verb_fqns` → `user_action = Accepted` (collapses `Select`-matched and `Clarify`-resolved into the same outcome per the Q1(a) recommendation above).
   - Else → `user_action = Rejected`.
   - Call `record_capture` with `staged_move_id = Some(event.id.0)` if the stage call succeeded (`IntentEvent.id: EventId(Uuid)` — confirmed field, `event.rs:44-46`), `None` if it landed as a `NotCurrentlyLegal` rejection (message-only reply, no `IntentEvent` exists).
   - Remove the pending proposal from the map either way — resolved, one-shot.

**`Discard`**: if a pending proposal exists for this session, record it `Rejected` with `staged_move_id = None` before clearing the workbook. Then clear the pending proposal regardless.

**A fresh `Propose` superseding an unresolved one**: per §3.3, record the outgoing pending proposal as `Rejected` (silently superseded — the operator asked again rather than acting on the first one) before installing the new one.

**Everything else (session timeout, operator navigates away, process restart):** no record. This is a deliberate v0 limitation, not an oversight — recording "rejected" for pure silence would itself be a guess (I-3's discipline: disposition/outcome facts are never inferred, only read off explicit action). The capture corpus is naturally biased toward resolved interactions; noted as a known v1 gap, not hidden.

### 3.5 Invariant: the two maps never drift apart

A pending proposal must never outlive its workbook. `workbooks.remove(session_id)` happens in exactly two places today (`Commit`, `Discard`); `Discard` already clears both under this design (§3.4). `Commit` does not need to touch `pending_proposals` because `Commit` never itself resolves a pending proposal (only `Stage` does, and `Stage` always resolves-and-clears immediately) — but this is exactly the kind of "should always be true" claim T7.1/T7.2 encoded as a structural test rather than a comment (`structural_only_commit_and_run_are_write_routes`, `workbook_module_remains_ramp_free`). Proposed teeth: a test that opens a workbook, proposes, commits without staging the proposal, and asserts the pending-proposal map still contains it afterward (proving `Commit` truly never silently resolves what only `Stage`/`Discard` are allowed to) — an explicit negative-space assertion, not just a happy-path check.

### 3.6 Avoiding a duplicate parse

`Stage`'s existing arm already calls `workbook.stage(&resolved_text, ...)`, which internally parses `text` via `dsl_parser::parse` and extracts `verb_fqn` (`kyc_workbook.rs::sexpr_to_parsed_move`) — but that's `pub(crate)` *inside* `kyc_workbook.rs`, not exposed. Two options:
- **(i)** Re-parse `text` a second time in the `Stage` dispatch arm just to read the verb_fqn for correlation (cheap — `dsl_parser::parse` is a tier-0 deterministic parse, not a network call — but literally duplicated work).
- **(ii)** Read the verb_fqn off the *result* instead: `workbook.stage(...)` returns `&StagedMove { event: IntentEvent, .. }` on success, and `IntentEvent.verb_fqn` is already there — no second parse needed, this is just reading a field off what `stage()` already handed back. The failure path (`NotCurrentlyLegal`) doesn't have an `IntentEvent` to read from, but that path is unconditionally `Rejected` regardless of which verb was attempted, so no parse is needed there either.

**Recommendation: (ii)** — zero duplicate parsing, reads only already-computed data.

## 4. Open rulings for Adam

- **Q1.** `Edited` semantics gap (§2): ratify **(a)** binary v0 (recommended), or **(b)** reinterpret for `Clarify`, or **(c)** defer capture wiring entirely until a future tranche's proposals carry argument content.
- **Q2.** Silent-abandonment non-recording (§3.4, last paragraph): ratify as a stated v0 limitation, or require some form of "never silently drop a shown proposal" (e.g. a TTL sweep that records abandoned proposals as a 4th state) — this would need a charter amendment (`user_action` currently has no "abandoned" value) and its own design; flagged, not recommended, given I-3's "never infer disposition" discipline cuts against inventing one.
- **Q3.** Scope for this pass: implement the full mechanism now (state map + `Stage`/`Discard`/re-`Propose` resolution + the negative-space structural test in §3.5), or land it in smaller slices (e.g. state map + `Stage` resolution first, `Discard`/supersede handling as a fast-follow)?

## 5. Explicitly out of scope here

- T7.3 (SLM ranker) — unaffected by this design; it consumes the capture corpus this wiring starts populating, nothing here depends on it.
- Any change to the charter's field list itself (only a reading of `user_action`'s existing 3 values is at stake, per Q1).
- Any change to `Propose`'s own recognition/ranking behavior (§3 of this doc only adds a side-channel memory + a later read of it; `Propose`'s return value to the operator is unchanged).
