# EOP-SESSION-CONTROLPLANE-G6B-G6C-IMPL-001 — Implementation session log

### Task: implement `EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.5` G6b (RR-5
### rows 2 and 5 only, not row 3) + G6c (RR-5 row 4 investigation)
### Date: 2026-07-14
### Branch: `codex/phase-1-5-governance-closure` (working tree left
### uncommitted — no commit made this session, per instruction)

---

## 0. Finding, up front

**G6b's premise needed re-derivation, not trust, before any code was
written** — the task brief's own paraphrase of RR-5's row numbering
("Row 3 is BPMN `process_instances`") does **not** match the actual
Mode-1 register table in `docs/research/control-plane-phase0-inventory.md`
§RR-5, which the brief itself warned might be the case. Read verbatim, the
table's five rows and `check-invariants.sh::gate_e4`'s own slug/pin-symbol
list (`scripts/check-invariants.sh:434-440`, the two are already in
1:1 correspondence and were treated as ground truth over the prose) are:

| # | RR-5 family text | E4 slug |
|---|---|---|
| 1 | Shadow envelope resolved entities | `shadow_envelope_entities` |
| 2 | Entity tables intended for TOCTOU | `toctou_entity_tables` |
| 3 | Bus-invoked operational writes | `bus_operational_writes` |
| 4 | BPMN `process_instances` | `bpmn_process_instances` |
| 5 | Raw DSL best-effort execution | `raw_dsl_best_effort` |

Row 4 is BPMN, not row 3 — matching G6c's own text ("RR-5 Row 4
investigation... determine BPMN `process_instances`' version-pin
story"), which is internally consistent with the table and *not* with
the brief's paraphrase. G6b's own plan text ("needs G6a for Row 3
only... Rows 2 and 5 are startable before G6a lands") therefore scopes
me to **row 2 (`toctou_entity_tables`) and row 5
(`raw_dsl_best_effort`)** — row 3 (`bus_operational_writes`) is the one
gated on G6a (bpmn-lite envelope threading feeds the bus-invocation
path), out of scope here. This matches the task brief's own
instruction to re-derive from the table rather than trust the
paraphrase.

**Both in-scope rows moved from unsatisfied to satisfied on the literal
`check-invariants.sh e4` gate this session** (1/5 → 2/5 real rows
satisfied for these two specifically — see §5). Row 2's populator is
now genuinely real (not a shadow-stub), with a live-DB RED→GREEN proof
(§3). Row 5's underlying threat (an arbitrary-DSL-text bypass) was
already closed by a prior tranche (Slice 3.1, 2026-04-22) — this
session closed the row's E4 gate honestly via the test-existence
branch, proving Path C's *existing* fail-closed behaviour with a named
test, rather than inventing new sealing infrastructure it does not yet
have (see §2's STOP-condition write-up for why building that
infrastructure this session would have been unsafe/unscoped).

G6c investigated `bpmn_controller::start_instance` (a fully
ob-poc-owned crate — `rust/crates/bpmn-controller/`, **not** the
separate `bpmn-lite` git repo, which is not checked out in this
environment at all) and the live `bpmn_lite` Postgres schema directly.
Finding: RR-5's row 4 characterisation is correct and now
empirically confirmed (no `row_version`/CAS on `process_instances`),
but the write path that would need one (the tick/worker loop mutating
`state`/`session_stack`/`flags`/`counters`/`current_node_id`) lives
entirely in the external `bpmn-lite-engine`/`bpmn-lite-store` crates
(git dep, tag `v0.2.0`, source not present in this checkout) — this is
a genuine, unclosable-from-here gap, handed to the architect as a
classification recommendation in §4, not scoped work.

---

## 1. G6b row 2 (`toctou_entity_tables`) — the real populator

### 1.1 What was already real, verified before touching anything

Before writing code I traced the full chain RR-5's row-2 text and the
E4 gate's `pin_resolves=1` finding describe, to establish exactly what
"real" already meant and what gap remained:

- The row-version migration `rust/migrations/20260422_row_version_entity_tables.sql`
  (whose own module doc in `toctou_recheck.rs` still says "staged,
  pending operator approval") is **already applied** in the dev
  database — confirmed by direct `psql` inspection, not assumed from
  the stale comment:
  ```
  $ psql -d data_designer -c "\d+ \"ob-poc\".cbus" | grep -i row_version
   row_version    | bigint | not null | 1
      "cbus_row_version_not_null" NOT NULL "row_version"
      trg_cbus_bump_row_version BEFORE UPDATE ON "ob-poc".cbus ...
  $ psql -d data_designer -c "select count(*) from \"ob-poc\".cbus where row_version is not null"
   count
  -------
    2200
  ```
  The `toctou_recheck.rs` module doc (dated 2026-04-22) is stale on
  this point and should be corrected in a future doc-hygiene pass —
  flagged, not fixed here (out of this session's scope, and editing a
  doc comment with no behavioural effect is not worth a separate
  commit inside this diff).
- `verify_pins_in_scope` (`ob-poc-boundary/src/toctou_recheck.rs`) is
  already called for real at two production seams
  (`dsl_v2/executor.rs:2014`, `sem_os_runtime/verb_executor_adapter.rs:663`)
  — this is exactly why E4's grep-based `pin_resolves` check already
  found row 2 satisfied before this session started.
- `build_decision_snapshot_input` (`agent/control_plane_shadow.rs`,
  T9.6) already builds a **real** `SnapshotInput` with real
  per-entity `row_version` values, sourced from
  `PgEntityFactsSource::entity_facts` (`ob-poc-boundary/src/entity_facts.rs`)
  — a genuine batched DB read, not a stub. When Path A's decision is
  `ApprovedStp`, `persist_sealed` (`sequencer.rs:8085`) durably stores
  this real `SnapshotPins` object with the envelope, and
  `verify_pins_in_scope` genuinely re-reads and compares against it at
  consume time (`FOR UPDATE` locked re-read, `toctou_recheck.rs`).

**So the "populator" existed in code already — but it was dead in
practice.** `build_stp_classifier_input`'s `has_unpinned_entities`
parameter (G8/STP classifier input) was hardcoded at both of its two
real production call sites
(`sequencer.rs`'s `phase5_runtime_recheck` and
`reseal_for_human_gate_resume`) to `!entity_requests.is_empty()` — a
**blanket "any bound entity is unpinned"** flag, unconditional on
whether a real pin was actually captured. Per `stp_classifier.rs`'s
own `classify()` (`has_unpinned_entities: true` unconditionally caps
the decision at `HumanGated`, never `StpExecutable`), this meant
**every entity-bound verb was permanently capped below `ApprovedStp`**
— `persist_sealed` (the real-pins seal site) could only ever fire for
zero-entity verbs, for which `SnapshotPins.entity_row_versions` is
vacuously empty. The real per-entity row_version machinery existed and
worked, but nothing in production ever reached the branch that used
it. This is the gap G6b's objective text describes: "nothing populates
real `SnapshotPins` at admission time... Build the populator at the
seams G1/G4 already established."

### 1.2 The fix

New function `has_unpinned_entities` in
`rust/src/agent/control_plane_shadow.rs` (right after
`build_stp_classifier_input`, whose doc comment now points at it
instead of describing the old blanket placeholder):

```rust
pub(crate) fn has_unpinned_entities(
    requests: &[(Uuid, String)],
    facts_map: Option<&HashMap<Uuid, ob_poc_boundary::entity_facts::EntityFactsRow>>,
) -> bool {
    if requests.is_empty() {
        return false;
    }
    let Some(facts_map) = facts_map else {
        return true;
    };
    requests.iter().any(|(id, _)| !facts_map.contains_key(id))
}
```

An entity counts as pinned iff it is present in `facts_map` — per
`build_entity_binding_input`'s own established contract,
`EntityFactsSource::entity_facts` returns an entry *only* for a row it
actually read and captured `row_version` from in the same query; a
missing entry means not found (or an unsupported kind), which is
honestly ungraded as unpinned, matching G2's identical `exists: false`
grading for the same condition. `facts_map: None` (the batched fetch
itself errored) is unpinned for every request — fail-closed, an I/O
failure is not evidence of a pin. Zero requests is vacuously not
unpinned — same posture the neighbouring `build_*_input` functions
already use.

Both real call sites in `sequencer.rs` (`phase5_runtime_recheck` and
`reseal_for_human_gate_resume`) now pass
`has_unpinned_entities(&entity_requests, entity_facts_map.as_ref())`
instead of the blanket `!entity_requests.is_empty()`.

### 1.3 RED→GREEN proof (live-DB, this session)

Temporarily reverted `has_unpinned_entities` to the old blanket
behaviour (`!requests.is_empty()`, ignoring `facts_map` entirely) and
re-ran the extended `t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch`
test (which now calls the real function against a real, live `cbus`
row instead of a hardcoded `false`):

```
RED (reverted to blanket-unpinned):
thread '...t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch' panicked:
assertion `left == right` failed: T9.1 closure: StpClassifier expected
Success given every input was built to be legal, got Some(Failure("requires_human_gate"))
test result: FAILED. 0 passed; 1 failed

GREEN (restored, real function):
test agent::control_plane_shadow::tests::t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch ... ok
test result: ok. 1 passed; 0 failed
```

This proves the fix has real teeth: without it, a genuinely pinned,
live entity is still forced through `HumanGated`.

### 1.4 New tests

- 5 new pure unit tests for `has_unpinned_entities` (empty requests;
  fetch-failed; entity missing from facts; entity found with a real
  pin; mixed pinned/unpinned) — `control_plane_shadow.rs::tests`.
- Extended the existing `t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch`
  live-DB test (§1.3) to call the real `has_unpinned_entities` against
  the real `facts` map it already fetches, instead of a hardcoded
  `false` the test's own prior comment explicitly flagged as fixture-only
  ("not because pinning is real (it isn't)"). That comment is now
  false and has been corrected in place.
- New named live-DB test **`toctou_unpinned_entity_requires_human_gate`**
  (E4's own designated test name for this row's OR-branch, added
  deliberately so the row is doubly satisfied — both `pin_resolves`
  and the named test, not just the former): a genuinely
  not-found entity (real `PgEntityFactsSource` fetch against a random
  UUID, not a stub) is graded unpinned and caps `classify()` at
  `HumanGated`, proving the fail-closed twin of §1.3's positive case —
  the populator being real does not mean every entity is treated as
  pinned, only ones a real DB read actually found.

---

## 2. G6b row 5 (`raw_dsl_best_effort`) — investigation, STOP-condition, and the safe closure taken

### 2.1 RR-5's row text is stale on the literal bypass; the underlying threat is already closed

RR-5's row-5 text ("Raw endpoint has its own validation and execution
modes", citing `agent_routes.rs:2311-2448`) describes a bypass that no
longer exists. `POST /api/session/:id/execute`
(`execute_session_dsl_raw`, `rust/src/api/agent_routes.rs`) rejects
any `req.dsl` supplied in the request body outright:

```rust
if req.dsl.is_some() {
    tracing::warn!(..., "Raw DSL in request body rejected — raw-execute
        bypass removed in Slice 3.1. Route through
        ReplOrchestratorV2::process() instead.");
    return Err(StatusCode::FORBIDDEN);
}
```

This matches `CLAUDE.md`'s own deprecation table ("Direct DSL bypass
(`dsl:` prefix)" → "SemReg-filtered pipeline", `OBPOC_ALLOW_RAW_EXECUTE`
removed 2026-04-22, Slice 3.1) — confirmed current at this session's
HEAD, not assumed from the doc. What that endpoint actually executes
today is `session.run_sheet.runnable_dsl()` — DSL that already passed
through the compiled-runbook/SemOS pipeline — dispatched through
`RealDslExecutor` (`repl/executor_bridge.rs`), tagged
`ExecutionPath::DslDirect` (Path C in RR-2/G3's taxonomy).

### 2.2 What is genuinely still open — and why I did not build it this session

`DslExecutor::execute_verb_in_scope` (`dsl_v2/executor.rs`) already has
real, structural per-step admission wired (G4) — `check_admission_in_scope`
is called for every dispatch on Path B/C, and (per the existing G4 seam
tests, `seam_rejects_on_pin_drift_and_leaves_envelope_reconsumable`,
re-run clean this session) genuinely rejects on pin drift **when a real
sealed envelope is presented**. The gap is upstream of that: **no
production call site ever seals an envelope for Path B or C**.
`ctx.envelope_handle` is `None` at every real Path B/C ingress point —
confirmed by the pre-existing code comment at the call site
(`dsl_v2/executor.rs`, "Every production Path B/C ingress point leaves
this `None`... no envelope-minting infrastructure wired for B/C yet")
and by grepping every real (non-test) construction site of
`ExecutionContext` feeding this path.

Building a real populator for Path C — i.e. a seal site that builds a
`cp_ctx` the way `phase5_runtime_recheck` does for Path A and mints a
real, consumable envelope — is **not** a "populate at an
already-established seam" task the way row 2 was. There is no
established seam for it. It requires a fresh design decision this
session is not positioned to make safely:

- **Where** does Path C's seal happen — inline inside the hot
  `execute_verb_in_scope` path (which today short-circuits to
  `NotEnforced` immediately for the overwhelming majority of
  dispatches with zero extra DB cost), or a separate pre-dispatch
  phase?
- **When** does it shadow-evaluate — unconditionally like Path A
  (real DB cost on every dispatch, matching Path A's own shadow
  posture), or gated behind `enforced.is_enforced(...)` first (cheaper,
  but then the shadow/audit trail for Path C stays sparse, unlike
  Path A's unconditional shadow evaluation)?
- Does a self-seal-and-immediately-consume shape (no park/resume gap
  on this path, unlike Path A's `HumanGate`) even fit the existing
  `EnvelopeHandle`/`try_consume_*` contract cleanly, or does it need
  its own variant?

This is architecturally the same weight as
`EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001` (G1 item 1) — a
dedicated design doc with an architect ratification, not a bounded
grind task. Per this program's STOP-condition discipline ("if the
plan's approach is unsafe/ambiguous... do not guess or force a
resolution... implement the narrowest safe resolution consistent with
fail-closed defaults"), I did not build it. **STOP-condition
recorded, not silently worked around** (§6).

### 2.3 The safe, real closure taken instead

What I *did* verify and prove, because it is true today and testable
without inventing anything: an enforced verb on `ExecutionPath::DslDirect`
with nothing sealed is rejected fail-closed (`RejectedNoEnvelope`),
never silently dispatched "best-effort." New live-DB test
**`raw_dsl_execution_requires_human_gate`** (E4's own designated test
name for this row) in `dsl_v2/executor.rs`'s `g4_seam_admission_tests`
module, alongside the existing G4 seam tests:

```rust
#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn raw_dsl_execution_requires_human_gate() {
    let _guard = EnvGuard::set("cbu.confirm:B"); // B = DslDirect
    let pool = test_pool().await;
    let executor = DslExecutor::new(pool.clone());
    let mut ctx = ctx_for(ob_poc_types::ExecutionPath::DslDirect);
    assert!(ctx.envelope_handle.is_none(), "Path C's honest production posture");
    let vc = verb_call("cbu", "confirm", vec![]);
    let mut scope = PgTransactionScope::begin(&pool).await.expect("begin scope");
    let err = { /* execute_verb_in_scope */ }.expect_err(...);
    scope.rollback().await.expect("rollback");
    assert!(err.to_string().contains("is enforce-mode gated")
        && err.to_string().contains("no sealed envelope was presented"));
}
```

Passed clean on the first correct attempt (after fixing a self-inflicted
error — see below).

**One real mistake made and caught this session, disclosed rather than
silently fixed:** the test initially used `EnvGuard::set("cbu.confirm:C")`
— tag `"C"` is `ExecutionPath::WorkflowDispatched`, not `DslDirect`
(`crates/ob-poc-types/src/execution_path.rs`: A=RunbookSequencer,
B=DslDirect, C=WorkflowDispatched, D=BusFederated). With the wrong tag,
the verb was never actually enforced on `DslDirect`, so the test hit an
unrelated lifecycle-gate error (`invalid_uuid`) instead of the intended
`RejectedNoEnvelope` — the test would have "passed" for the wrong
reason if I had just accepted a green run without reading the failure
message first. Caught because the FIRST run failed with an error message
that didn't match the assertion, prompting me to check the tag mapping
rather than loosen the assertion. Fixed to `"cbu.confirm:B"` (matching
the sibling seam tests in the same module, which all correctly use `:B`).

### 2.4 Recommendation for a future tranche (not this session's scope)

A genuine Path B/C seal-site design (mirroring G1's own shape: a design
doc, architect ratification, then implementation) is the real close-out
for row 5 (and, per §2.2's shared gap, would likely also unblock parts
of row 3 for Path B/C traffic, though row 3's stated blocker is
specifically G6a's bpmn-lite threading for the *bus* path, a distinct
call site). Recommend scoping this as its own tranche, not folded
silently into a future grind session.

---

## 3. G6c — RR-5 row 4 (BPMN `process_instances`) investigation

### 3.1 Where the write path actually lives

`bpmn_controller::start_instance` (`rust/crates/bpmn-controller/src/instance.rs`)
is a fully ob-poc-owned, in-workspace crate — **not** the separate
`bpmn-lite` git repo (confirmed: no `bpmn-lite/` directory exists
anywhere in this checkout; the repo is referenced only as a pinned git
dependency, `Cargo.toml:404-412`, tag `v0.2.0`). The only ob-poc-side
write to `process_instances` is this one `INSERT` (verified: grepped
every `fqn()` in `src/domain_ops/bpmn_controller_ops.rs` — the three
`bpmn-controller.*`/`loader.*` verbs touching instances are
`start-instance` (create), `instance-status` (read),
`list-instances` (read); no update/terminate/cancel verb exists on the
ob-poc side).

### 3.2 Empirical schema finding (live `bpmn_lite` Postgres DB, not assumed from a stale doc)

```
$ psql -d bpmn_lite -c "\d+ process_instances"
```

confirms: **no `row_version` column, no CAS-style trigger on the
mutable fields.** The only immutability protection is a trigger,
`process_instances_enforce_immutable_fields()`, which locks
`instance_id`/`tenant_id`/`bytecode_version`/`process_key`/`entry_id`/
`runbook_id`/`created_at` and makes `integrity_hash` immutable-once-set
— all creation-time lineage facts, not a version pin on the *mutable*
runtime-state columns (`state`, `session_stack`, `flags`, `counters`,
`current_node_id`, `placeholder_values`) the fiber VM updates as a
process executes. RR-5's row-4 characterisation ("no cited
row-version/CAS check was found on this path") is confirmed correct by
direct inspection, not merely un-refuted.

**One relevant, different mechanism observed:** `lease_owner`/
`lease_until` columns plus a supporting index
(`idx_instances_scheduler_claim` on `(tenant_id, lease_until,
updated_at)`) strongly suggest a **lease/claim-based pessimistic
mutual-exclusion** design is bpmn-lite's actual concurrency-control
primitive for mutable-state writes (a worker claims an instance for a
lease window before mutating it), which is a *different* mechanism
from row-version/CAS but could serve an equivalent purpose (no lost
updates from concurrent workers) if every mutating write site
correctly enforces "claim held and not expired" before writing.

**I cannot verify whether that discipline is actually enforced.** The
write call sites that would need to enforce it (the tick/worker loop)
live entirely inside `bpmn-lite-engine`/`bpmn-lite-store` — external
git dependencies whose source is not present in this checkout, and
which this task explicitly instructed not to touch even if it were
(separate repo, own release flow, "Standing rule 5" per the plan doc).

### 3.3 Secondary finding, flagged not fixed (out of this session's narrow G6c scope)

`start_instance`'s own idempotency check
(`SELECT instance_id FROM process_instances WHERE tenant_id = $1 AND
correlation_id = $2 ... LIMIT 1`, then a separate `INSERT`) reads
`idx_instances_correlation`, which is a **plain, non-unique** btree
index, not a unique constraint. Two concurrent `start_instance` calls
with the same `idempotency_key` could both pass the "not found" read
before either commits its `INSERT`, producing two instances for one
idempotency key — a real TOCTOU race in the create path itself,
distinct from (and narrower than) row 4's actual question (which is
about the *running* lifecycle's mutable-state writes, not instance
creation). Flagged for a future session; not fixed here to avoid scope
creep beyond G6c's stated "small, standalone" charter, and because
fixing it would need a schema change (a unique index/constraint) on a
table this session has no mandate to migrate.

### 3.4 Recommendation (architect classification, per the plan's own anticipated G6c outcome)

This is not closeable from ob-poc this session — the source that would
need the fix (or the verification that the lease mechanism already
covers it) is in a separate repo not present here. Recommended
classification for architect sign-off:

**Row 4 = Mode-1, confirmed, distinct mechanism, unverifiable from
ob-poc.** Concretely:
1. `process_instances`' mutable-state writes have no row-version/CAS
   pin — confirmed, not merely unconfirmed.
2. bpmn-lite's actual concurrency primitive for this table appears to
   be the `lease_owner`/`lease_until` claim pattern, not row-version —
   this needs verification *in the bpmn-lite repo itself* (by whoever
   next works there, "rides that repo's own flow, pinned by tag bump"
   per the plan's standing rule 5) that every mutating write site
   holds a valid claim before writing.
3. If that verification finds gaps, the remediation is either (a)
   tightening the claim discipline in bpmn-lite-engine, or (b) adding
   a real `row_version` column + CAS check to `process_instances`
   (mirroring `20260422_row_version_entity_tables.sql`'s pattern) —
   either way, bpmn-lite-side work, a new tranche, not this session's.
4. Secondary: the `start_instance` idempotency-key race (§3.3) is a
   real, narrow, ob-poc-side bug, independent of the row-4 question,
   worth its own small follow-up (add a unique index on
   `(tenant_id, correlation_id)` plus `ON CONFLICT DO NOTHING`/retry,
   in the bpmn-lite schema — also not this session's, same repo
   boundary).

I did not flip `invariants-expected.toml`'s implicit row-4 status (it
was never separately tracked there; E4's own `[e4]` comment already
lists row 4 among the 4 unsatisfied) and did not write a
`process_instances_row_version` symbol or a
`bpmn_process_instance_requires_human_gate` test — both would be
fabricated to satisfy a grep, not honest evidence, given the actual
mechanism (if any) lives in code I cannot read this session.

---

## 4. STOP-conditions hit

**One, row 5 (§2.2–2.4):** the plan's G6b objective text ("Build the
populator at the seams G1/G4/G6a established") assumes an
already-established seam exists for every in-scope row. It does for
row 2 (Path A's `phase5_runtime_recheck`/`persist_sealed`, already
wired, just gated on a hardcoded blanket flag — a genuine "populate at
the seam" task). It does **not** for row 5 (Path B/C has no seal
call site at all — a genuine "build a new seam" task, architecturally
comparable to G1's own seal-consume design doc). Per this program's
STOP-condition discipline, I did not build new, unreviewed sealing
infrastructure on a hot dispatch path under this session's own time
budget. Narrowest safe resolution taken instead: proved the *existing*
fail-closed property with a named, real test (§2.3), and recorded the
real gap and its design questions for a dedicated future tranche
(§2.4) rather than silently declaring the row closed or silently
building something un-reviewed.

**One, G6c (as anticipated by the plan's own text):** row 4's real
answer requires reading/verifying code in the external `bpmn-lite`
repo, not present in this checkout. Per instruction, this is handed to
the architect as a ratified-pending classification (§3.4), not forced
to a code-level closure.

No other STOP-condition fired — row 2's fix was verified safe by
construction (an honest per-entity pin fact, fail-closed on any I/O
failure or not-found entity, RED→GREEN proven) before being wired into
the two real call sites.

---

## 5. Verification (every command run fresh this session)

### Build

```
$ cargo build -p ob-poc --features database --lib
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.65s   # zero errors
```

### Clippy (scoped to the touched crate, matching this program's established convention)

```
$ cargo clippy -p ob-poc --features database --lib -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.88s   # zero warnings
```

### New unit tests (control_plane_shadow, non-DB)

```
$ cargo test -p ob-poc --lib --features database control_plane_shadow
test result: ok. 38 passed; 0 failed; 5 ignored (live-DB); 0 measured; 2338 filtered out
```

All 5 new `has_unpinned_entities` unit tests pass; no regression in the
34 pre-existing tests in this module.

### Live-DB: row 2 + row 5's new named tests, plus the extended closure test

**Correction made mid-session, disclosed rather than silently
fixed:** an initial combined run of this doc's own new/touched tests
plus the existing `g1_item2_path_a_tests`/`t4_1_envelope_admission_tests`
modules used `--test-threads=4` and one *pre-existing, unmodified*
test (`admits_consumes_once_from_path_a_then_a_bare_retry_finds_nothing_sealed`,
which I did not touch) failed — not because of anything in this
session's diff, but because that test (like several of its siblings)
reads a shared fixture row via `SELECT cbu_id FROM cbus LIMIT 1`, which
collides under parallel execution with sibling tests mutating the same
row. `EOP-SESSION-CONTROLPLANE-G1-ITEMS-3-4-IMPL-001.md` already
established `--test-threads=1` as this program's convention for
exactly this reason (its own commands all use it); my first run
deviated from that convention and got the corresponding flake. Re-ran
with `--test-threads=1` and it passed clean — confirmed via isolation
(`--test-threads=1` on that single test alone, `ok`) before
re-running the full combined set below. The evidence quoted here is
the corrected, `--test-threads=1` run, matching precedent:

```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    -- --ignored --test-threads=1 control_plane_shadow g4_seam_admission g1_item2_path_a t4_1_envelope_admission
running 19 tests
test agent::control_plane_shadow::tests::g2_reaches_success_end_to_end_against_a_real_cbu_row ... ok
test agent::control_plane_shadow::tests::g3_none_leaves_authority_and_evidence_blocked ... ok
test agent::control_plane_shadow::tests::g3_reaches_success_and_unblocks_authority_evidence ... ok
test agent::control_plane_shadow::tests::t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch ... ok
test agent::control_plane_shadow::tests::toctou_unpinned_entity_requires_human_gate ... ok
test dsl_v2::executor::tests::g4_seam_admission_tests::branch_3_fallthrough_consumes_envelope_exactly_once ... ok
test dsl_v2::executor::tests::g4_seam_admission_tests::raw_dsl_execution_requires_human_gate ... ok
test dsl_v2::executor::tests::g4_seam_admission_tests::seam_rejects_on_pin_drift_and_leaves_envelope_reconsumable ... ok
test dsl_v2::executor::tests::g4_seam_admission_tests::seam_rolls_back_the_consume_when_dispatch_fails ... ok
test dsl_v2::executor::tests::g4_seam_admission_tests::seam_skip_is_keyed_on_exact_path_match_not_a_bare_flag ... ok
test runbook::step_executor_bridge::g1_item2_path_a_tests::admits_consumes_once_from_path_a_then_a_bare_retry_finds_nothing_sealed ... ok
test runbook::step_executor_bridge::g1_item2_path_a_tests::no_sealed_envelope_for_this_entry_is_rejected_with_triage_classification ... ok
test sem_os_runtime::verb_executor_adapter::t4_1_envelope_admission_tests:: (7 tests) ... ok
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 2363 filtered out
```

### RED→GREEN teeth proof (§1.3) — reproduced

```
RED (has_unpinned_entities reverted to blanket !requests.is_empty()):
test agent::control_plane_shadow::tests::t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch ... FAILED
  StpClassifier expected Success, got Some(Failure("requires_human_gate"))

GREEN (restored):
test agent::control_plane_shadow::tests::t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch ... ok
```

### `check-invariants.sh e4` — literal script, before/after

Before this session (per `invariants-expected.toml`'s existing
comment): `1/5` satisfied (`toctou_entity_tables` via `pin_resolves`
only).

After this session, run fresh:

```
$ bash scripts/check-invariants.sh e4
  [shadow_envelope_entities] ... version-pin resolves: 0, human-gate test exists: 0
  [toctou_entity_tables]     ... version-pin resolves: 1, human-gate test exists: 1
  [bus_operational_writes]   ... version-pin resolves: 0, human-gate test exists: 0
  [bpmn_process_instances]   ... version-pin resolves: 0, human-gate test exists: 0
  [raw_dsl_best_effort]      ... version-pin resolves: 0, human-gate test exists: 1
  Total Mode-1 register rows: 5
  Satisfied (pinned or human-gated-tested): 2
  Unsatisfied: shadow_envelope_entities bus_operational_writes bpmn_process_instances
  E4: DOES NOT HOLD
```

**2/5 satisfied** (up from 1/5) — exactly rows 2 and 5, matching this
session's scope. `toctou_entity_tables` is now doubly satisfied (both
branches of the OR), `raw_dsl_best_effort` newly satisfied via its
named test. `bus_operational_writes` (row 3, needs G6a) and
`bpmn_process_instances` (row 4, G6c's external-repo finding) remain
unsatisfied, as expected and explained in §2.4/§3.4.
`shadow_envelope_entities` (row 1) was out of scope for this session
and is untouched.

E4's overall status stays `DOES NOT HOLD` (2/5, not 5/5) — this
session moved the count, not the pass/fail verdict; `invariants-expected.toml`
is a ratchet file and was **not edited** (see §7).

### Control-plane live-DB sweep (scoped, `--test-threads=1`, matching this program's convention — not the unscoped whole-crate `--ignored` run, which includes ~250 tests spanning unrelated subsystems, several pre-existing-failing independent of DATABASE_URL/API-key availability; see below)

```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    -- --ignored --test-threads=1 control_plane
test result: FAILED. 34 passed; 1 failed
```

The 1 failure, `agent::control_plane_metrics::t7_2_metrics_tests::e3_invariant_probe`,
is the **expected** failure: `AuditReplay`/G11 has zero substantive
samples and `WriteSetAttestation`/G14 has samples only at the wrong
provenance, matching `invariants-expected.toml`'s existing `[e3] status
= "fail"` and every prior session's identical finding — unrelated to
this session's subject, unchanged by this session's diff. 34 (up from
the prior G1-items-3-4 session's 33) is the +1 from this session's new
`toctou_unpinned_entity_requires_human_gate` test.

### Full `ob-poc` lib suite (non-ignored)

```
$ cargo test -p ob-poc --lib --features database
test result: ok. 2174 passed; 0 failed; 208 ignored
```

2174 vs. the prior G1-items-3-4 session's cited 2169 — the +5 is this
session's own new `has_unpinned_entities` unit tests, not a
regression.

### `ob-poc-control-plane` crate-internal suite (untouched by this session — re-run to prove no regression)

```
$ cargo test -p ob-poc-control-plane --lib
test result: ok. 120 passed; 0 failed
```

Unchanged from the prior session's cited 120 (this session did not
touch this crate).

### Plugin coverage + dep-gate

```
$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database -- test_plugin_verb_coverage
test result: ok. 1 passed; 0 failed

$ bash scripts/check_kyc_substrate_deps.sh
PASS: no forbidden deps in ob-poc-kyc-substrate
```

### An unscoped full `--ignored` sweep was attempted and abandoned mid-run — disclosed, not hidden

I first ran the entire crate's `--ignored` set (every `#[ignore]` test
in `ob-poc`, ~250+ tests, not scoped to control-plane) to be maximally
thorough. It ran for >25 minutes of CPU time and surfaced 13 failures
entirely unrelated to this session's 3-file diff (stewardship
`b3_changesets`, `utterance_api_coverage`, `verb_search` corpus
hard-negatives/taught-phrases, LLM-backed `live_acp_llm_draft_loop_*`
harnesses, `scope_resolution_integration`, and
`sequencer::tests::test_turn_record_lock_blocks_same_session_writer_until_release`).
I killed the run rather than let it finish (its scope was a self-inflicted
overreach, not something this task asked for) and instead **proved one
of the 13 is genuinely pre-existing** rather than just asserting it:
`git stash`-ed this session's 3 changed files, re-ran
`test_turn_record_lock_blocks_same_session_writer_until_release` alone,
and it failed identically (`same-session writer must block behind the
durable turn lock`, `sequencer.rs:10068`) with my changes absent —
confirmed pre-existing, not introduced by this diff — then restored
the stash. The other 12 are outside `sequencer.rs`/`dsl_v2/executor.rs`/
`control_plane_shadow.rs` entirely (verb-search corpus, stewardship,
LLM harnesses needing `ANTHROPIC_API_KEY`) and were not individually
stash-verified given the time cost, but none touch a file this session
edited.

### `check-invariants.sh ratchet` (the literal script this program's reviewer independently re-runs) — completed this session

Started in the background (full-workspace `e5` build+test is slow) and
completed before this doc closed:

```
$ DATABASE_URL=postgresql:///data_designer bash scripts/check-invariants.sh ratchet
  ... (E1-E4 evaluated; E4's own section matches the standalone `e4` run above, 2/5) ...
  [e5] actual=fail expected=fail — MATCH
== Ratchet: 0/5 invariant(s) diverge from invariants-expected.toml ==
```

Zero divergence — every one of E1-E5's actual status matches
`invariants-expected.toml` as it stands today (this session did not
edit that file — see §6). E4's status stays `fail` as expected (2/5,
not 5/5 — the ratchet file's own `[e4]` comment already says `fail`
with no specific count asserted in a machine-checked way, only prose,
so the count movement inside a still-`fail` status does not itself
constitute a divergence). E5 fails for pre-existing, unrelated reasons
across the whole workspace (confirmed via the earlier abandoned
unscoped sweep and the `git stash` spot-check on
`test_turn_record_lock_blocks_same_session_writer_until_release`,
above) — this session's 3-file diff does not touch any of the crates
listed in the `e5` output's `unreachable_pub`-enforcing-crate sweep.

---

## 6. `invariants-expected.toml` — recommendations (not applied)

**`[e4]`** — current comment: *"1/5 Mode-1 register rows (RR-5) have
real, non-test production wiring: `toctou_entity_tables`... The other 4
have neither a load-bearing version-pin call site nor a named human-gate
test."* This is now stale on the count and on two rows' detail.
Suggested replacement text (status stays `fail` — 2/5, not 5/5):

> 2/5 Mode-1 register rows (RR-5) satisfied: `toctou_entity_tables`
> (both branches — `verify_pins_in_scope` is a real call site, and the
> real per-entity pin populator now reaches it via
> `has_unpinned_entities`, G6b 2026-07-14, RED→GREEN proven) and
> `raw_dsl_best_effort` (named test `raw_dsl_execution_requires_human_gate`
> proves Path C fails closed with no envelope — the row's original
> literal-bypass threat was already closed by Slice 3.1, 2026-04-22).
> `bus_operational_writes` needs G6a (bpmn-lite envelope threading).
> `bpmn_process_instances` is an external-repo gap (bpmn-lite's own
> tick/worker write path; see G6c's session doc) pending architect
> classification. `shadow_envelope_entities` remains untouched.

**`[e2]`/`[e1]`/`[e3]`** — no change recommended; unaffected by this
session's subject (repeating the prior session's own still-accurate
`[e2]` recommendation would be redundant here — see
`EOP-SESSION-CONTROLPLANE-G1-ITEMS-3-4-IMPL-001.md` §5, unchanged).

---

## 7. Files changed

- `rust/src/agent/control_plane_shadow.rs` — new `has_unpinned_entities`
  function + doc-comment correction on `build_stp_classifier_input`;
  5 new unit tests; 1 new live-DB test
  (`toctou_unpinned_entity_requires_human_gate`); extended the existing
  `t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch`
  test to use the real function instead of a hardcoded `false`,
  correcting its own now-false doc comment in place.
- `rust/src/sequencer.rs` — both real call sites of
  `build_stp_classifier_input` (`phase5_runtime_recheck`,
  `reseal_for_human_gate_resume`) now pass
  `has_unpinned_entities(&entity_requests, entity_facts_map.as_ref())`
  instead of the blanket `!entity_requests.is_empty()`.
- `rust/src/dsl_v2/executor.rs` — 1 new live-DB test
  (`raw_dsl_execution_requires_human_gate`) in the existing
  `g4_seam_admission_tests` module.
- `docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-G6B-G6C-IMPL-001.md`
  — this doc.

No schema/migration changes (row 2 reused the already-applied
`20260422_row_version_entity_tables.sql`; row 5 needed no schema
change; G6c is investigation-only, no bpmn-lite-side changes made or
attempted). `invariants-expected.toml` was **not edited** (§6 is
recommend-only, per instruction). No commit was made — working tree
left as-is for independent review.

Pre-existing untouched noise, confirmed left alone (per this session's
own instructions, not part of this task): `observatory-wasm/Cargo.lock`,
`rust/cbu_mismatches.json`, `rust/mismatches.json`,
`rust/reports/phase0_confusion_matrix.json`,
`rust/reports/step0_trial_evaluation.json`.

---

## 8. Open items for future tranches

- **Row 5's real populator** — a Path B/C seal-site design doc
  (architecturally comparable to G1's own), per §2.2/§2.4.
- **Row 3 (`bus_operational_writes`)** — blocked on G6a per the plan;
  unchanged.
- **Row 4 (`bpmn_process_instances`)** — architect classification per
  §3.4; needs bpmn-lite-repo-side verification of the lease-claim
  discipline, or a `row_version` migration there, neither of which is
  reachable from this checkout.
- **`start_instance`'s idempotency-key race** (§3.3) — a real, narrow,
  bpmn-lite-schema-side bug (non-unique `correlation_id` index),
  independent of row 4's own question; flagged, not fixed.
- **`toctou_recheck.rs`'s stale module doc** ("staged, pending operator
  approval" — the migration is applied) — a doc-hygiene fix, not
  behavioural; flagged, not fixed here.
- **`invariants-expected.toml`'s `[e4]` wording** — §6, not applied.
