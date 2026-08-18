# EOP-DD-KYCUBO-005: W6 Case/Workstream Fold Design v0.1

**Status: design only, not started.** Written per `EOP-DD-KYCUBO-004`'s own
next step ("W6 needs its own design doc ... once the full consumer inventory
is done"). That inventory (`EOP-DD-KYCUBO-004` Part 2 step 1) is corrected
and extended below — it under-counted the writer surface — and this doc adds
the two pieces the prior plan deliberately deferred: **the fold shape** and
**the projection/cutover mechanics**. No code has been written against this
design; it is grounded against the current schema and verb YAML (this
session, direct reads), not speculation.

**Headline finding that overturns `EOP-DD-KYCUBO-004`'s Part 2 recommendation:**
`cases`/`entity_workstreams` are FK-referenced by ~25 other tables, several
`ON DELETE CASCADE`. A view cannot be an FK target, so "projection view,
not table rename" (the prior plan's step 3) is not viable as written. See
§3.

---

## 1. Corrected writer inventory

`EOP-DD-KYCUBO-004` found 3 Rust modules with hand-written SQL
(`kyc_case.rs`, `red_flag.rs`, `request_ops.rs`) and called that "the real
production writers." That undercounts: `config/verbs/kyc/kyc-case.yaml` and
`config/verbs/kyc/entity-workstream.yaml` also declare direct writes via
`behavior: crud` — generic-executor verbs with no hand-written SQL to grep
for, since `dsl-runtime`'s CRUD executor builds the INSERT/UPDATE from the
YAML `crud:` block at runtime. These are just as real a write path as the
plugin ops; a hand-written-SQL grep structurally cannot see them.

**Full verb-level writer inventory (14 verbs, case+workstream):**

| Verb (today) | Behavior | Table | Trigger domain(s) |
|---|---|---|---|
| `kyc-case.create` | plugin | `cases` | kyc-case |
| `kyc-case.update-status` | plugin | `cases` | kyc-case |
| `kyc-case.close` | plugin | `cases` | kyc-case |
| `kyc-case.approve` | plugin | `cases` | kyc-case |
| `kyc-case.reject` | plugin | `cases` + `entity_workstreams` (cascade→PROHIBITED) | kyc-case |
| `kyc-case.approve-with-conditions` | plugin | `cases` | kyc-case |
| `kyc-case.refer` | plugin | `cases` + `entity_workstreams` (cascade→REFERRED) | kyc-case |
| `kyc-case.escalate` | plugin | `cases` | kyc-case |
| `kyc-case.assign` | **crud** | `cases` | kyc-case |
| `kyc-case.set-risk-rating` | **crud** | `cases` | kyc-case |
| `kyc-case.reopen` | **crud** | `cases` | kyc-case |
| `entity-workstream.create` | **crud** (upsert) | `entity_workstreams` | entity-workstream |
| `entity-workstream.update-status` | **crud** | `entity_workstreams` | entity-workstream |
| `entity-workstream.block` | **crud** | `entity_workstreams` | entity-workstream |
| `entity-workstream.complete` | **crud** | `entity_workstreams` | entity-workstream |
| `entity-workstream.escalate-dd` | **crud** | `entity_workstreams` | entity-workstream |
| `entity-workstream.set-ubo` | **crud** | `entity_workstreams` | entity-workstream |
| — (no discoverable verb; `request.create`'s internal cascade) | plugin | `entity_workstreams` (status→BLOCKED) | request |
| — (no discoverable verb; `try_unblock_workstream` internal helper) | plugin | `entity_workstreams` (status→COLLECT, clear blocker) | request |
| `red-flag.escalate` | plugin | `entity_workstreams` (status→ENHANCED_DD, cascade) | red-flag |

Bold (**crud**) rows are the ones `EOP-DD-KYCUBO-004`'s inventory missed.
Two more rows (the request-domain block/unblock) aren't even discoverable
verbs today — they're internal Rust helpers with no `verb_fqn`, invisible to
any verb-surface enumeration, only findable by reading `request_ops.rs`
directly.

**Reads are unaffected by count** — `EOP-DD-KYCUBO-004`'s ~20-module read
inventory stands; nothing here revises it.

**A pre-existing correctness bug found in passing, not this design's job to
fix:** `try_unblock_workstream` (`request_ops.rs:863`) takes `&PgPool`, not
`scope.executor()` — it writes outside the caller's transaction. Any W6
verb standing in for it must fix this as a side effect of touching the code
(the fold-based version necessarily goes through `scope.executor()`, so the
bug can't survive the migration), but it's worth flagging now: today, a
rolled-back request-resolution can leave a workstream unblocked when the
resolution itself didn't commit.

---

## 2. Subject model: `case_id` as `subject_root`

The existing dsl.kyc stream already has two ungoverned meanings for
`subject_root` coexisting in one flat `uuid` column with no kind
discriminator: the UBO determination root entity (`ubo.edge.*`) and the
obligation subject entity (`kyc.obligation.*`). Both are `entity_id`s, so
they at least share a domain even if nothing stops a caller from crossing
them.

**Decision: case/workstream events use `subject_root = case_id`, a third,
disjoint meaning.** One stream per case; every workstream belonging to that
case is a sub-object inside the same stream, disambiguated by
`workstream_id` in the event's `target`/payload — exactly the same shape
already proven for obligations (`subject_root = entity_id`, `obligation_id`
disambiguates within the payload).

**Why this is the right choice, not just the available one:**
- A case's own writes are already serialized against each other today —
  `kyc-case.update-status`'s B6 comment explicitly does an optimistic
  concurrency check (`WHERE status = $4`) to guard exactly the race a
  per-subject stream lock (§3 `FOR UPDATE` on `kyc_subject_streams`) gives
  for free.
- The cascade writes (`reject`→workstreams PROHIBITED, `refer`→workstreams
  REFERRED) need to read "every non-terminal workstream in this case" at
  fold time. If workstreams lived in their own per-workstream streams (or
  worse, per-entity streams shared with the unrelated obligation subject),
  that cascade would need a cross-stream fan-out — the same shape W5 needed
  for screening→obligations, and for the same reason: because the trigger
  and the target live in different subjects. Here they don't have to.
  One case, one stream, one fold, cascade is pure same-fold logic.
- `EdgeId`/`ObligationId` are precedent for adding a new
  `TargetBinding` sub-field per new object kind rather than overloading
  `subject_root`/`entity_id`. This design adds `CaseId(Uuid)` and
  `WorkstreamId(Uuid)` newtypes and two new `Option` fields on
  `TargetBinding` (additive, no existing field changes).

**Open risk, not solved here:** the `subject_root` column still has no kind
discriminator. This design is the *third* domain leaning on "trust the
caller passed the right kind of UUID." A `subject_kind` column (or a
prefix/tag scheme) would close this for all three domains at once, but it's
a cross-cutting substrate change outside W6's scope — flagged as a
follow-up candidate, not a blocker.

---

## 3. The FK-topology finding (why "compatibility view" doesn't work)

`EOP-DD-KYCUBO-004`'s Part 2 step 3 proposed: "prefer standing up a
`kyc_case_rollup_projection`/`kyc_workstream_rollup_projection` (new
tables, drainer-fed, same shape as the control/obligation precedent) behind
a compatibility view matching the legacy column names."

Checked against the real schema (`migrations/master-schema.sql`) this
session: **`cases.case_id` and `entity_workstreams.workstream_id` are
FK-referenced by ~25 other tables**, including `red_flags`,
`outstanding_requests`, `screenings`, `case_events`,
`case_evaluation_snapshots`, `kyc_decisions`, `kyc_ubo_registry`,
`tollgate_evaluations`, `ubo_registry`, `ubo_snapshots`,
`verification_challenges`, `deal_onboarding_requests`,
`deal_ubo_assessments`, and more — several `ON DELETE CASCADE`
(`case_evaluation_snapshots`, `case_events`, `entity_workstreams` itself,
`red_flags`, `screenings`, `doc_requests`).

This is fatal to the "new table + compatibility view" shape:
- **A view cannot be the target of a foreign key.** Postgres requires a
  real table (or at least a unique index on one) on the referenced side.
  Retargeting ~25 tables' FK constraints at new projection tables is a
  large, high-risk migration nobody has scoped, and is a categorically
  bigger job than "add a view."
- The two existing precedents (`kyc_control_edge_projection`,
  `kyc_obligation_projection`) have **zero inbound FKs** — nothing
  references an `edge_id` or `obligation_id` from outside the KYC stack.
  That's exactly why the existing "new table, full DELETE+re-INSERT per
  subject" projector shape was safe for them and is **not** safe here.

**Revised approach: the drainer becomes the sole writer of the existing
`cases`/`entity_workstreams` tables, in place.** No new projection tables,
no rename, no view. `cases`/`entity_workstreams` keep their names, their
columns, their FK relationships, and every one of the ~20 read-only
consumers keeps working completely unmodified — they were never touching
anything but `SELECT`, so they don't care whether the row in front of them
was written by a verb or by a drainer.

This also means the projector **cannot** reuse the existing
DELETE-then-re-INSERT-per-subject shape (safe only because nothing
references those rows). It must be an **idempotent UPSERT keyed on the
natural primary key** (`case_id` / `workstream_id`), inserting a row once
(on the fold's `kyc.case.open` / `kyc.workstream.open`) and updating it in
place on every subsequent event — never deleting. This is a real, load-bearing
difference from the two existing projectors, not a cosmetic one: a
DELETE+INSERT projector that raced a concurrent `red_flags` insert
referencing the about-to-be-deleted `case_id` would hit an FK violation
(or, worse, silently cascade-delete every dependent row if the constraint
is `ON DELETE CASCADE` — which several are).

---

## 4. `case_ref` determinism

`cases.case_ref` is generated by a `BEFORE INSERT` trigger
(`"ob-poc".generate_case_ref()`) that does
`'KYC-' || LPAD(nextval('case_ref_seq')::text, 4, '0')` — a
non-deterministic, monotonically-consumed Postgres sequence, called exactly
once per legacy `INSERT`.

Once `cases` is written by the projector instead of a verb's `INSERT`, this
trigger either double-fires per replay (wrong — a fold rebuild would burn a
new sequence value and produce a different `case_ref` than the one already
handed to the user) or must be bypassed. **The correct fix: `case_ref` is a
captured effect (`kyc_intent_events.captured_effects`), not a projector
derivation.** `kyc.case.open`'s verb op calls `nextval('case_ref_seq')`
itself, once, at append time (same transaction, same pattern the schema
comment for `captured_effects` already documents: "External-lookup results
captured at first apply; replay reads these, never re-calls the external
service"), formats it, and stores it in the event payload. The fold reads
`case_ref` back out of the opening event on every replay — deterministic by
construction, and the trigger is dropped (or left in place but made a
no-op via `INSERT ... case_ref` always non-null from the projector, since
the trigger only fires `IF NEW.case_ref IS NULL`).

This is exactly the same class of subtlety W5 already ran into and
resolved cleanly (screening linkage), surfaced here before any code is
written rather than found by a failing replay test later.

---

## 5. Fold state shape

New module, `ob-poc-kyc-substrate/src/fold/case.rs`, structurally parallel
to `fold/obligation.rs`:

```rust
pub struct CaseFacts {
    pub case_id: Uuid,
    pub case_ref: String,               // captured effect, see §4
    pub cbu_id: Uuid,
    pub deal_id: Option<Uuid>,
    pub client_group_id: Option<Uuid>,
    pub case_type: String,
    pub status: String,                 // trusts the stream; verb pre-validates (§6)
    pub escalation_level: String,
    pub risk_rating: Option<String>,
    pub assigned_analyst_id: Option<Uuid>,
    pub assigned_reviewer_id: Option<Uuid>,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub sla_deadline: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub originating_event_id: EventId,
}

pub struct WorkstreamFacts {
    pub workstream_id: Uuid,
    pub case_id: Uuid,
    pub entity_id: Uuid,
    pub status: String,
    pub discovery_source_workstream_id: Option<Uuid>,
    pub discovery_reason: Option<String>,
    pub discovery_depth: i32,
    pub is_ubo: bool,
    pub ownership_percentage: Option<BigDecimal>,
    pub risk_rating: Option<String>,
    pub requires_enhanced_dd: bool,
    pub blocker_type: Option<String>,
    pub blocker_request_id: Option<Uuid>,
    pub blocker_message: Option<String>,
    pub blocked_at: Option<DateTime<Utc>>,
    pub identity_verified: bool,        // no writer found anywhere (see §6, note on dead columns)
    pub ownership_proved: bool,         // same
    pub screening_cleared: bool,        // confirmed dead by EOP-DD-KYCUBO-004 W5 investigation
    pub evidence_complete: bool,        // no writer found anywhere
    pub originating_event_id: EventId,
}

pub struct CaseState {
    pub case: CaseFacts,
    pub workstreams: BTreeMap<Uuid, WorkstreamFacts>,  // keyed by workstream_id
}
```

`identity_verified`/`ownership_proved`/`evidence_complete` join
`screening_cleared` (already confirmed dead by the W5 investigation) as
columns with no writer anywhere in `src/`/`crates/` found by this session's
greps. The fold carries them (schema parity — a consumer reading the
compat-preserved `entity_workstreams` row must still see the column) but no
new verb sets them; they stay at their column default. If they matter,
that's a separate, out-of-scope gap, not something W6 should invent
semantics for.

Dispatch mirrors the existing `fold_control_versioned`/
`fold_obligations_versioned` shape: a `fold_case_versioned(&[&IntentEvent],
&FoldRegistry) -> Result<CaseState, KycError>` that matches on
`event.verb_fqn` (case events) vs. workstream target-carrying variants of
the same verbs, folding left-to-right, deterministic, no I/O.

---

## 6. New verb → event mapping

**Case-level** (all append with `subject = SubjectId(case_id)`,
`target = TargetBinding::for_case(case_id)`):

| New event kind | Replaces | Payload | Notes |
|---|---|---|---|
| `kyc.case.open` | `kyc-case.create` | cbu_id, deal_id, client_group_id, case_type, sla_deadline, assigned_analyst_id, notes, **case_ref** (captured, §4) | first event in a case's stream |
| `kyc.case.transition-status` | `kyc-case.update-status` | to_status, notes | `is_valid_transition`/`is_terminal_status` (LifecycleCatalog) stay as pre-append verb-side checks — the fold trusts the stream, exactly like every existing dsl.kyc verb (§7) |
| `kyc.case.close` | `kyc-case.close` + `.approve` + `.reject` + `.approve-with-conditions` + `.refer` | target_status, reason/notes, do_not_onboard: bool, cascades_workstreams_to: Option\<String\> | **dispatching-fold pattern** (CLAUDE.md precedent): 5 discoverable verb FQNs stay, each a thin arg-shaping wrapper building the same event kind — mirrors the existing `close_with_status()` Rust helper that already unifies 4 of the 5 today. `reject`/`refer` set `cascades_workstreams_to`; the fold applies it to every currently-non-terminal workstream in the same `CaseState.workstreams` map — pure same-fold logic, no second append (see §2) |
| `kyc.case.escalate` | `kyc-case.escalate` | escalation_level, notes | BOARD level cascades to case status REFER_TO_REGULATOR — same-fold, no workstream cascade (matches current code) |
| `kyc.case.assign` | `kyc-case.assign` (was crud) | analyst_id, reviewer_id | |
| `kyc.case.set-risk-rating` | `kyc-case.set-risk-rating` (was crud) | risk_rating | |
| `kyc.case.reopen` | `kyc-case.reopen` (was crud) | reopen_reason, new_case_type, new_status | |

**Workstream-level** (subject = `SubjectId(case_id)` — same stream as the
case; `target = TargetBinding::for_workstream(case_id, workstream_id)`):

| New event kind | Replaces | Payload | Trigger domain(s) |
|---|---|---|---|
| `kyc.workstream.open` | `entity-workstream.create` (was crud upsert) | entity_id, discovery_source_workstream_id, discovery_reason, discovery_depth, ownership_percentage, is_ubo | entity-workstream. Idempotent: the verb reads its own case's current fold before minting a new `workstream_id`, so a repeat call for the same (case,entity) resolves to the existing workstream rather than duplicating — same idempotent-ensure contract the `crud upsert` had via `conflict_keys` |
| `kyc.workstream.transition-status` | `entity-workstream.update-status` | to_status | entity-workstream |
| `kyc.workstream.block` | `entity-workstream.block` + `request.create`'s cascade | blocker_type, blocker_request_id, blocker_message | **two trigger domains** — entity-workstream (manual) and request (auto-block-on-create). Same shape as W5's screening→obligation fan-out: one canonical event, multiple external callers each supplying their own reason |
| `kyc.workstream.unblock` | `try_unblock_workstream` (was an undiscoverable internal helper) | — | request. Becomes a real, named event for the first time; also fixes the out-of-transaction bug noted in §1 |
| `kyc.workstream.complete` | `entity-workstream.complete` | risk_rating | entity-workstream |
| `kyc.workstream.escalate-dd` | `entity-workstream.escalate-dd` + `red-flag.escalate`'s cascade | — | **two trigger domains** — entity-workstream (manual, role-gated) and red-flag (auto-escalate). Same shape again |
| `kyc.workstream.set-ubo` | `entity-workstream.set-ubo` | ownership_percentage | entity-workstream |

**Reads are unchanged.** `kyc-case.summarize`, `kyc-case.read`,
`kyc-case.list-by-cbu`, `entity-workstream.state`, `entity-workstream.read`,
`entity-workstream.list-by-case` keep their FQNs and `behavior` — they just
end up reading rows the drainer wrote instead of rows a verb wrote. Because
of the §3 revision (drainer writes the *same* tables in place), this is
**zero read-verb changes**, not a re-point to a new table/view. The
asymmetry is worth naming: 14 write verbs need redesigning, 0 read verbs
do — the entire ~20-module external consumer surface from
`EOP-DD-KYCUBO-004`'s inventory needs zero changes for the same reason.

---

## 7. No seam API change needed

The §3 append chokepoint's precondition hook is hardcoded to two fold
types: `append_in_scope`'s `validate` closure is
`FnOnce(&ControlState, &ObligationState) -> Result<(), KycError>`
(`ob-poc-kyc-seam/src/seam.rs:107`). Adding `CaseState` as a third
precondition-checked fold would mean widening that signature at all
existing call sites (22, per `stream_append`'s own comment) — a real cost.

**It isn't needed.** Every existing case/workstream business-rule check
(`is_valid_transition`, `is_terminal_status`,
`case_cannot_approve_without_workstreams_complete`'s incomplete-workstream
count, the request-domain's remaining-blockers count) is already, today, a
plain `SELECT` the verb op runs itself before mutating — not something
routed through the generic `validate` hook. W5 already established this
exact pattern for its own precondition (`apply_screening_outcome_to_obligations`
does its own `load_events` + `fold_obligations_versioned` read, not a
`validate` closure). The new `kyc.case.*`/`kyc.workstream.*` verbs do the
same: read the case's own current fold (or the still-live legacy table,
during the dual-write burn-in window — see §9) before calling
`stream_append`, exactly mirroring what `kyc_case.rs` already does today.
No seam-crate change, no widening of the 22 existing call sites.

---

## 8. Projector + drainer

Structurally parallel to `PgKycObligationProjector`/`PgKycObligationDrainer`
(`ob-poc-kyc-store/src/projection.rs`), with the one change §3 forces:

```rust
pub struct PgKycCaseProjector;
impl PgKycCaseProjector {
    /// UPSERT, not delete+re-insert (§3 — cases/workstreams are FK-referenced).
    pub async fn rebuild_case(
        conn: &mut PgConnection,
        registry: &FoldRegistry,
        subject_root: SubjectId,   // = CaseId
    ) -> Result<CaseProjectionStats, StoreError> {
        let state = fold_case_versioned(&events, registry)?;
        // INSERT ... ON CONFLICT (case_id) DO UPDATE SET ... (never DELETE)
        // one UPSERT for the case row, one UPSERT per workstream row
    }
}
```

New outbox effect-kind: `kyc.projection.case`, added to
`PROJECTION_EFFECT_KINDS`. New drainer `PgKycCaseProjectionDrainer`, same
`FOR UPDATE SKIP LOCKED` per-effect-kind claim shape as the two existing
drainers — no contention with them, claims only its own effect-kind rows.

`ProjectionStats` gains an equivalent (`cases_written`,
`workstreams_written`) for observability parity with the existing two.

---

## 9. Phased cutover (revises `EOP-DD-KYCUBO-004`'s step 3/4)

The prior plan's phasing ("projection view, retire direct writers last")
assumed a new-table-behind-a-view shape this doc has ruled out (§3). Revised:

1. **Additive**: new `kyc.case.*`/`kyc.workstream.*` verbs, fold, projector,
   drainer land. Legacy verbs (`kyc-case.*`, `entity-workstream.*`,
   `red-flag.escalate`'s cascade, `request_ops.rs`'s cascades) are
   **untouched** and keep writing `cases`/`entity_workstreams` directly, as
   today. Nothing observes the new stream yet; this phase is pure
   plumbing, fully reversible, zero consumer risk.
2. **Dual-write burn-in**: each of the 14 legacy write verbs is changed
   (like W5's `screening.complete`/`review-hit`) from `behavior: crud`/its
   existing plugin body to a plugin op that does the **same legacy
   UPDATE/INSERT it does today** AND, same transaction, appends the
   matching `kyc.case.*`/`kyc.workstream.*` event — but does **not** yet
   let the drainer's projector touch the row (the projector only writes if
   the legacy write didn't already happen this txn — a feature flag or a
   `WHERE NOT EXISTS`-guarded UPSERT works). A verification job compares
   the fold's derived row against the legacy row per case, on a schedule,
   flagging drift. This is the burn-in that earns confidence the fold
   matches 14 verbs' worth of real business logic (including the two
   multi-trigger-domain events) before anything stops writing directly.
3. **Flip**: once burn-in shows zero drift for a bake period (length TBD —
   not decided here), each legacy verb's direct
   `UPDATE/INSERT "ob-poc".cases|entity_workstreams` is deleted; the verb
   becomes append-only, and the drainer's UPSERT becomes the sole writer.
   One verb at a time is safer than a big-bang flip given 14 verbs across
   3 trigger domains — order by risk, e.g. the two multi-trigger-domain
   events (`kyc.workstream.block`, `kyc.workstream.escalate-dd`) last,
   since they're the ones two independent domains must agree to call
   identically.
4. **Retire the dual-write scaffolding** (the `WHERE NOT EXISTS` guard, the
   drift-check job) once every verb has flipped.

This is a bigger, more cautious sequence than `EOP-DD-KYCUBO-004` sketched,
proportional to what §1/§3 found: 14 real writers across 3 domains, into
FK-anchored tables with ~25 dependents, not a narrow new-table projection
like the two precedents this pattern was proven on.

---

## 10. Explicitly out of scope / open questions

- **Bake-period length for step 3's burn-in** — not decided; needs an
  owner call once step 2 is actually running against real data volume.
- **`row_version`** (`cases.row_version bigint DEFAULT 1`) — was presumably
  for optimistic-lock support on the direct-write path. Once the drainer is
  the sole writer, its meaning is unclear (the drainer's UPSERT is already
  idempotent/convergent by construction, so it doesn't need optimistic
  locking the way concurrent verb writers did). Not resolved here — flagged
  for whoever implements step 3 to check whether anything still reads it.
- **`subject_root` kind-collision risk** (§2) — real, pre-existing, not
  W6's to fix, but W6 is the third domain deepening reliance on it.
- **The 4 dead workstream columns** (`identity_verified`, `ownership_proved`,
  `screening_cleared`, `evidence_complete`) — carried through unchanged,
  not given real semantics by this design.
- **`case_events`/`case_evaluation_snapshots`** — separate audit tables that
  FK to `cases`/`entity_workstreams` but are written by different code
  paths entirely (not touched by any of the 14 verbs in §1's inventory).
  Out of scope; noted only because they showed up in the FK sweep (§3).

---

## Summary

| Question | Answer |
|---|---|
| How many real writers? | 14 discoverable verbs (7 already plugin, 7 currently plain `crud`) + 2 undiscoverable internal helpers, across 3 domains (kyc-case, entity-workstream, request) plus one cascade from a 4th (red-flag) |
| What's the stream subject? | `case_id` — one stream per case, workstreams disambiguated by `workstream_id` inside it |
| Can we reuse the control/obligation projection shape? | No — those have zero inbound FKs; `cases`/`entity_workstreams` have ~25. Projector must UPSERT the existing tables in place, never delete/replace |
| Does the seam need to change? | No — case/workstream verbs do their own pre-append reads, same as every existing dsl.kyc verb |
| What changes for the ~20 read-only consumers? | Nothing — same tables, same columns, different writer |
| Ready to implement? | No — this is the design step `EOP-DD-KYCUBO-004` called for. Cutover sequencing (§9) still needs a bake-period decision and per-verb implementation, each with its own RED→GREEN proof, before any of this ships |
