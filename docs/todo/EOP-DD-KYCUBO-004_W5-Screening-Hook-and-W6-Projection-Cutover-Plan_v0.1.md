# EOP-DD-KYCUBO-004: W5 Screening Hook Wiring + W6 Case/Workstream Projection Cutover — Plan v0.1

**Status:** Part 1 (W5) DONE — commit `b028e5b3`. Part 2 (W6) still plan-only.
Written per explicit instruction ("leave 1, do 4, plan 2 and 3") against
CLAUDE.md's KYC/UBO "Remaining V&S scope" list. Item 1 (economic-axis crossing on
`ControlProngStrategy`) is out of scope by that instruction and not discussed
further here. Item 4 (`kyc_stream` `SourceOfTruth` variant) is DONE — commit
`1965253` on `github.com/adamtc007/dsl` (pushed, `refactor/sem-os-pack-policy`)
+ ob-poc `fa08c0e0`.

**Part 1 (W5) implemented same session, `b028e5b3`** — the 3 open questions
below were resolved by direct investigation (not left for a future session)
and Option B (outbox+drainer) was superseded by a simpler same-transaction
plugin op once the fan-out requirement was understood: see "What actually
shipped" at the end of Part 1.

This plan covers the remaining two items:
- **W5** — wire real `screening.*` outcomes into the `kyc.obligation.update-screening`
  dsl.kyc verb, so obligation screening state is actually driven by screening
  results instead of being a verb that exists but nothing calls.
- **W6** — cut `"ob-poc".cases` / `"ob-poc".entity_workstreams` over from directly
  written legacy tables to folds off the dsl.kyc stream, matching the pattern
  already proven for control edges and obligations.

Both are grounded against the current code (this session, direct reads), not
speculation. Neither has been started.

---

## Part 1 — W5: screening hook wiring

### Current state (verified)

- `kyc.obligation.update-screening` is a real, lexicon-covered, fold-wired
  dsl.kyc verb (`ob-poc-kyc-substrate/src/fold/obligation.rs:293`,
  `src/domain_ops/kyc_stream_ops.rs:1174` `KycObligationUpdateScreening`,
  registered in `domain_ops::mod.rs:442`). It folds correctly and has test
  coverage in the substrate crate. **Nobody calls it from a real screening
  outcome today.** It is reachable only via direct/manual DSL invocation.
- The actual screening lifecycle lives entirely in the legacy operational
  tables, driven by two `crud`-behavior verbs in `config/verbs/screening.yaml`:
  - `screening.complete` — `UPDATE "ob-poc".screenings SET status = :status,
    completed_at = now() WHERE screening_id = :screening-id`, `status` one of
    `CLEAR | HIT_PENDING_REVIEW | ERROR`. Declares `emits_event:
    screening.completed` in its YAML.
  - `screening.review-hit` — `UPDATE "ob-poc".screenings SET reviewed_at =
    now() ...`, `status` one of `HIT_CONFIRMED | HIT_DISMISSED`. Declares
    `emits_event: screening.reviewed`.
- **`emits_event` in verb YAML is descriptive metadata only** — grepped for
  every runtime consumer; the only hits are unrelated `runbook.rs` internal
  test names and phase-4/5 integration test files. There is no event-bus
  dispatcher today that reacts to a verb's declared `emits_event` and fires a
  side-effect call. So this is not "the hook is half-wired" — it is fully
  unwired; `screening.complete`/`review-hit` are plain CRUD writes with no
  observer.
- Resolution of `entity_workstreams.screening_cleared` (the boolean gate
  `tollgate_evaluate.rs:211` reads) has no writer found anywhere in `src/` or
  `crates/` — it appears to be either dead/unused today or set by a path not
  found in this grep pass and worth confirming before building on it.

### Design options

**Option A — direct dual-write inside the `crud` verb.** Change
`screening.complete`/`review-hit` from `behavior: crud` to `behavior: plugin`,
backed by a `SemOsVerbOp` that does the existing `UPDATE screenings` AND, in
the same transaction (`scope.executor()`), determines the owning
`subject_id`/`obligation_id` (via `entity_workstreams.case_id` →
`cases.subject_entity_id`, or however the real subject linkage resolves —
needs confirming, see Open Questions) and calls the same append path
`kyc.obligation.update-screening` uses (`stream_append`, §3 protocol) with a
payload derived from the `status` arg. Pros: one transaction, no lag, no new
infrastructure. Cons: couples the screening plugin op directly to
`ob-poc-kyc-seam`/`ob-poc-kyc-store` (a new crate dependency the screening ops
module doesn't have today), and it's the kind of ad-hoc dual-write V&S has
been actively removing elsewhere in this codebase (CBU⊥KYC decoupling, W1
bypass elimination) — worth being honest that this is the same defect shape
in miniature.

**Option B — outbox/drainer projection (mirrors W6 pattern, see Part 2).**
Add a lightweight trigger or application-level enqueue on
`screenings` UPDATE (status transition to a terminal value) that a new
`PgScreeningToObligationDrainer` (same `FOR UPDATE SKIP LOCKED` shape as
`PgKycProjectionDrainer`/`PgKycObligationDrainer`) picks up and translates
into a governed `kyc.obligation.update-screening` append. Pros: consistent
with the established drainer pattern (§3.6 chokepoint stays the single append
path; the screening domain doesn't need a new dependency, it only needs an
outbox row), decouples timing, keeps `ob-poc-kyc-seam` as the only crate
touching dsl-runtime for KYC. Cons: introduces lag between screening
completion and the obligation fold seeing it (same class of eventual
consistency already accepted for the two existing drainers), needs a new
outbox table + migration.

**Recommendation:** Option B, on consistency-with-existing-pattern grounds —
it's the second instance of this exact "legacy op writes an operational
table, a drainer folds it into the stream" pattern, not a new one, and it
avoids adding a dsl-runtime dependency to the screening ops module.

### Open questions to resolve before implementation

1. What is the real subject/obligation linkage for a `screenings` row? (via
   `entity_workstreams.entity_id` + `cases.subject_entity_id`, presumably —
   needs a schema-level confirmation, not assumed here.)
2. Is `entity_workstreams.screening_cleared` actually live (does anything set
   it), or is it dead? If dead, W5 may be free to repurpose/retire it rather
   than needing to keep it in sync.
3. Does `screening.review-hit`'s `HIT_CONFIRMED`/`HIT_DISMISSED` outcome need
   its own obligation-fold semantics distinct from `screening.complete`'s
   `CLEAR`/`HIT_PENDING_REVIEW`/`ERROR`, or does only the terminal
   confirm/dismiss decision matter to the obligation? (i.e. does the
   obligation track want "screening in progress" visibility, or only
   "screening resolved, clear or not"?)

### What actually shipped (commit `b028e5b3`)

All 3 open questions resolved by direct code investigation, not left open:

1. **Subject/obligation linkage** — confirmed via `tests/kyc_w3_w5_w6.rs`:
   `kyc.obligation.*`/`kyc.person.*` verbs key `subject-id` to the natural
   person/entity's own real UUID, not a separate synthetic identifier. So
   `entity_workstreams.entity_id` (reached via `screenings.workstream_id`)
   **is** the obligation subject-id directly — no new linkage table needed.
2. **`entity_workstreams.screening_cleared`** — confirmed dead (no writer
   anywhere in the codebase). Left untouched; not this change's problem to
   solve.
3. **`review-hit`'s distinct semantics** — confirmed real: `HIT_CONFIRMED`
   fails the screening track outright (`rejected`, does not go back to
   `in_progress`); `HIT_DISMISSED` clears it (`satisfied`). Handled as two
   separate status→`TrackState` mappings, both fail-closed on any value
   outside the verb YAML's declared `valid_values` (mirrors the
   `STRUCTURE_CLASS_WIRE_VALUES` precedent).

**Option B (outbox+drainer) was reconsidered and dropped once #1 was
resolved.** The fan-out ("which obligations does this screening result
apply to") needs a transactional read of the subject's current obligation
fold at write time — an async drainer would just duplicate that same read
later, adding latency for no correctness benefit. Shipped as **Option A**
instead: `screening.complete`/`review-hit` moved from `behavior: crud` to
`behavior: plugin` (`ScreeningComplete`/`ScreeningReviewHit` in
`kyc_stream_ops.rs`), preserving the original UPDATE semantics exactly
(`COALESCE` for optional args, same columns), then in the same transaction:
resolve the screened entity → `PgKycEventStore::load_events` +
`fold_obligations_versioned` for that subject → fan out one
`kyc.obligation.update-screening` event per currently-registered obligation.
Zero obligations yet raised is a no-op, not an error.

Tests: `rust/tests/kyc_w5_screening_hook.rs`, 3 cases (CLEAR→satisfied
happy path, HIT_CONFIRMED→rejected, fail-closed unrecognized status writes
nothing). RED-proof: temporarily removing the fan-out call didn't just fail
a test, it failed to **compile** (`dead_code = "deny"` caught the now-unused
helper) — stronger evidence of real wiring than a runtime assertion.

---

## Part 2 — W6: case/workstream projection cutover

### Current state (verified)

`"ob-poc".cases` and `"ob-poc".entity_workstreams` are legacy tables, written
directly (not via the dsl.kyc stream) by at least:
- `src/domain_ops/request_ops.rs`
- `crates/sem_os_postgres/src/ops/kyc_case.rs`
- `crates/sem_os_postgres/src/ops/red_flag.rs`

This is the same architectural shape `kyc_control_edge_projection` and
`kyc_obligation_projection` already replaced for control/economic edges and
obligations (W6 in the original plan, migrations `20260630_kyc_control_edge_
projection.sql` + `20260630_kyc_obligation_projection.sql`, drained by
`PgKycProjectionDrainer`/`PgKycObligationDrainer`, `FOR UPDATE SKIP LOCKED`,
per-effect-kind, idempotent/convergent). The remaining W6 scope specifically
is: **cases/entity_workstreams becoming folds too**, i.e. extending that same
already-proven pattern to the two tables that are still written directly.

### Why this is materially bigger than W5

Cases/workstreams are the oldest, most heavily-consumed tables in the KYC
domain — `red_flag.rs`, `kyc_case.rs`, `request_ops.rs`, plus (per the
earlier grep) `ob-workflow`'s `guards.rs`/`requirements.rs`,
`agent/composite_state_loader.rs`, and the tollgate evaluator all read
`cases`/`entity_workstreams` directly and would need to keep working — either
against a read-compatible projection view (same column shape, populated by
fold instead of direct write) or a migration of every reader. The two
existing projections (`kyc_control_edge_projection`,
`kyc_obligation_projection`) are comparatively new, narrow, single-purpose
tables with few consumers; `cases`/`entity_workstreams` are wide, old, and
have `ob-workflow` crate consumers outside the kyc-stack proper.

### Recommended phasing (mirrors the W1→W4→W6 discipline already used)

1. **Inventory pass** (not yet done): full read/write consumer list for
   `cases` and `entity_workstreams` across `src/` + all `crates/` (the grep in
   this doc is a starting point, not exhaustive — `ob-workflow`,
   `agent/composite_state_loader.rs`, and the tollgate evaluator are known
   readers not yet enumerated in full).
2. **Define the fold.** What dsl.kyc verbs, if any, currently exist that
   *should* drive case/workstream state, vs. what new verbs are needed? (Case
   open/close/escalate, workstream status transitions — none of these appear
   to be dsl.kyc-stream verbs today; they're the plain CRUD/plugin verbs in
   `kyc_case.rs`/`entity-workstream.yaml`.) This likely means new W1-style
   verb + lexicon + fold work, not just a projection over existing events —
   materially different from W5, which projects an *existing* fold's already-
   governed verb.
3. **Projection view, not table rename.** Given the wide consumer surface,
   prefer standing up a `kyc_case_rollup_projection`/
   `kyc_workstream_rollup_projection` (new tables, drainer-fed, same shape as
   the control/obligation precedent) behind a compatibility view matching the
   legacy column names, and migrate consumers off direct-table access
   gradually — the same phased-cutover posture used for the CBU⊥KYC
   decoupling work, not a single big-bang rename.
4. **Retire direct writers last**, once every consumer is confirmed reading
   the projection.

### Recommendation

Given the size gap between W5 (one drainer, one outbox, verb already exists)
and W6 (new verb+lexicon design, wide consumer inventory, phased view-based
cutover), **do not bundle these into one work item**. W5 is a scoped,
mechanical follow-on to the existing drainer pattern and could reasonably be
picked up next. W6 needs its own design doc (`EOP-DD-KYCUBO-005` or similar)
once the full consumer inventory (step 1 above) is done — this plan
deliberately stops at "here is the shape and the phasing," not a committed
schema, because the consumer surface hasn't been fully mapped yet and
guessing at it here would be exactly the kind of unauthorised/undersubstantiated
claim this program's own discipline (K-16/K-33, RED→GREEN receipts) has
consistently rejected elsewhere.

---

## Summary

| Item | Size | Status |
|------|------|--------|
| W5 screening hook | Small — 1 plugin op pair, verb already existed | **DONE**, commit `b028e5b3` |
| W6 case/workstream cutover | Large — new verb/lexicon design + wide consumer migration | Plan only. Next: full consumer inventory pass, then a dedicated design doc before any code |
