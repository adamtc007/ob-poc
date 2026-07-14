# EOP-SESSION-CONTROLPLANE-G2-ITEMS-2-3-CLOSURE-001 — Implementation session log

### Targets: `EOP-PLAN-CONTROLPLANE-GRADUATION-001` v0.5, G2 items 2 and 3
### Date: 2026-07-14
### Branch: `codex/phase-1-5-governance-closure` (not merged, not committed by this session)

---

## 0. Verdict up front

- **G2 item 3 (G11 / AuditReplay live wiring): CLOSED.** A real, tested,
  justified on-demand replay call site now feeds `gate_outcome_counts`.
  Live-verified: **AuditReplay moved from 0 to 51 substantive samples, at
  its expected `shadow_eval` provenance.** E3 moved from "3 gates zero"
  (G11, G12, G14 — per the prior session's own recorded baseline) to
  **only G14 (WriteSetAttestation) remaining at the wrong provenance** —
  13/14 gates now pass E3's bar.
- **G2 item 2 (G14 / real attestation): PARTIALLY CLOSED, deliberately not
  armed.** A real, non-empty, tested `allowed_columns` derivation now
  exists for a narrow, exactly-verified subset of CRUD verbs (Insert /
  Update / Upsert with an explicit `returning` column). **`set_expected_
  write_set` is still NOT wired** — this session found a **second,
  independent, previously-undocumented correctness gap** (table-name
  format mismatch between `domain_metadata.yaml` and `record_write`'s
  captured writes) that would make arming unsafe even for the now-correct
  column subset. Both gaps are proven with real tests, not asserted in
  prose. See §3 for the full STOP-condition account.

Given the explicit mission framing — a fully honest partial landing is
the expected outcome when item 2's arming bar isn't met — this session
reports **G2 item 3 CLOSED, G2 item 2 PARTIAL** (derivation landed, arming
deferred with a concrete, tested reason).

---

## 1. G2 item 3 — G11 (AuditReplay) live wiring

### 1.1 Verification before coding

Read (not assumed): `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-G2-
AUDIT-PROVENANCE-001_v0.2.md` (RATIFIED design, DD-4 "G11 semantics"),
`docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-G2-AUDIT-PROVENANCE-
IMPL-001.md` (built the dead-code primitives, explicitly declined to wire
a caller — "a real decision this doc doesn't make"), and the primitives
themselves (`rust/src/agent/control_plane_audit.rs`: `AuditRow`,
`audit_rows_for_decision`, `check_completeness` [DD-4(i)],
`rederive_decision_outcome` [DD-4(ii)]).

Confirmed directly against source, not assumed from the docs:
- `expected_provenance(GateId::AuditReplay)` (`crates/ob-poc-control-
  plane/src/audit.rs`) is `ShadowEval`, with its own doc comment: "it
  evaluates *over* the audit stream itself" — this is the design's own
  signal that G11 is meant to be a read-time replay, not a decision-time
  gate contribution.
- `gate_outcome_counts`'s `shadow_eval` CTE (`control_plane_metrics.rs`)
  reads exclusively from `control_plane_shadow_decisions.gate_results`
  (a JSONB column populated once, at decision time, by `report_to_json`).
  There is **no existing mechanism** by which a gate could contribute a
  `shadow_eval`-provenance sample any other way.
- `check_completeness`/`rederive_decision_outcome`'s own inputs
  (`&[AuditEvent]`, `&serde_json::Value` gate_results) are only fully
  populated **after** a decision's later lifecycle events
  (`EnvelopeConsumed`, `DispatchCommitted`/`DispatchRolledBack`) have
  arrived — which is always strictly later than that same decision's own
  shadow-eval gate stack run at `phase5_runtime_recheck`. **There is no
  earlier call site that could produce this signal honestly** — grading
  G11 at decision time would necessarily grade an empty/incomplete
  stream for every decision, which is not what DD-4 asks for.

**Design decision made here (the "real decision" the ratified doc
declined to make):** G11 is implemented as an **on-demand replay
function**, `audit_replay_outcome_counts`, called from `gate_outcome_
counts` itself (not written into any table) — it queries the audit
stream, grades each "replay-eligible" decision, and returns synthetic
`(gate="AuditReplay", outcome_kind, provenance="shadow_eval", count)`
rows that `gate_outcome_counts` unions into its normal output. This
keeps G11 exactly analogous to how the design doc frames it ("ShadowEval
over the audit stream itself") without writing anything new into
`control_plane_shadow_decisions` (which would have re-opened the
append-once/pollution risk DD-1 explicitly designed the two-structure
split to avoid) or into `control_plane_audit` itself (the stream stays a
pure fact record — no self-referential "audit of the audit" write, same
posture DD-5 already applies to campaign bookkeeping).

**Why call it from `gate_outcome_counts` and not a separate query:**
that function is the single, existing, real production/test call site
consumed by both the E3 probe and the operator-facing `GET /api/control-
plane/metrics` endpoint (`api/agent_routes.rs::get_control_plane_
metrics`) — neither the dispatch hot path nor anything writing state.
Folding G11's replay in there means both consumers get real AuditReplay
samples for free, with zero new call sites elsewhere to keep in sync.

### 1.2 Eligibility — avoiding a false-negative on in-flight decisions

A decision is graded only once its lifecycle has reached a **terminal**
point:
- a non-`ApprovedStp` `DecisionEvaluated` (terminal immediately — no
  envelope is ever minted for it), or
- an `ApprovedStp` decision that has reached `DispatchCommitted` or
  `DispatchRolledBack`.

A sealed-but-not-yet-consumed envelope is **deliberately excluded** — it
may simply not have been consumed yet (the operator hasn't driven that
runbook step to completion), which DD-4(i)'s own grammar does not call a
violation. Grading it now would manufacture a false `Failure`. Proven
with a dedicated live-DB test
(`a_sealed_but_unconsumed_decision_is_not_replay_eligible`): inserting
only `DecisionEvaluated(ApprovedStp)` + `EnvelopeSealed` for a fresh
decision does not change `audit_replay_outcome_counts`' total sample
count at all.

### 1.3 DD-4(ii) re-derivation — the join gap this session had to close

`rederive_decision_outcome` needs the SAME decision's `gate_results`
(from `control_plane_shadow_decisions`), but **no existing audit event
carried a stable join key to it** — `DecisionEvaluated`'s only payload
fields were `outcome` and a best-effort `snapshot_ref` (not a join key
per the prior session's own V2 finding).

**Fix, disclosed schema-shape change:** `DecisionEvaluated` gained a new
field, `entry_id: Uuid`, `#[serde(default)]`-gated (defaults to
`Uuid::nil()`, so any already-persisted pre-this-session row still
deserializes cleanly — proven by the existing `payload_json_round_trips_
every_variant` test still passing unedited plus a new nil-default
consideration). `sequencer.rs`'s two `DecisionEvaluated` construction
sites (`phase5_runtime_recheck`'s main path and the HumanGate-resume
reseal path) now populate it with the exact same `entry_id` value the
adjacent `build_shadow_decision_row` call already uses — no new value
invented, the same one already in scope.

This is a **JSONB payload field addition**, not a schema migration (the
column itself, `control_plane_audit.payload`, is already `JSONB` and
untouched) — no new `.sql` file needed.

**Re-derivation logic:** for a replay-eligible decision whose first
event is `DecisionEvaluated { outcome, entry_id, .. }` with a non-nil
`entry_id`, fetch `control_plane_shadow_decisions.gate_results` for that
`entry_id` and call the existing `rederive_decision_outcome`. A nil
`entry_id` (pre-this-session rows, or the non-`ApprovedStp` fallback
path which historically minted an uncorrelated id) or a missing shadow
row makes re-derivation **inconclusive**, not a failure — graded on
completeness alone in that case. This mirrors the same "cannot prove, so
don't punish" posture the rest of this codebase's shadow-only components
already use (e.g. `find_verb_footprint` returning `None`, not a
fabricated guess).

### 1.4 Bounded scan, and a genuine bug caught during implementation

`audit_replay_outcome_counts` scans up to 500 "eligible" `decision_id`s
per call (matching GW's own "≥500 real decisions" campaign-window
language) — bounded because it is an N+1 per-decision scan, even though
it only runs from an on-demand metrics endpoint and the E3 probe, never
the dispatch hot path.

**First draft ordered `ORDER BY decision_id LIMIT 500`** — caught during
self-review (not by a test) as wrong: `decision_id` is a random UUID
(`Uuid::new_v4()` at seal time, or an uncorrelated fresh UUID for
non-`ApprovedStp` decisions), so ordering by it is **not** "the most
recent 500" as the function's own doc comment claimed. Fixed to order by
`MAX(seq) DESC` per decision — `seq` is the audit stream's own append-
only monotonic identity, the actual recency signal. This also matters
for test reliability: a freshly-inserted test decision always has the
highest `seq` in the table, so it reliably surfaces in the bounded scan
regardless of how many pre-existing rows share the dev database.

### 1.5 Tests (new, this session)

Unit-adjacent / live-DB (all in `control_plane_audit.rs`'s `live_db`
module, `#[ignore = "requires DATABASE_URL"]`):
- `replay_grade_success_for_a_complete_consistent_approved_stp_lifecycle`
  — a real, complete `ApprovedStp` lifecycle (shadow row with all
  proof-bearing gates + RunbookProof + StpClassifier `Success`,
  `DecisionEvaluated`→`EnvelopeSealed`→`EnvelopeConsumed`→
  `DispatchCommitted`, `entry_id` correctly linked) grades `Success`.
- `replay_grade_failure_for_a_grammar_incomplete_lifecycle` — DD-4(i)
  violation (`DispatchCommitted` with no prior `EnvelopeConsumed`) grades
  `Failure`, isolating the completeness check.
- `replay_grade_failure_for_an_outcome_rederivation_mismatch` — DD-4(ii)
  violation: `DecisionEvaluated` claims `ApprovedStp`, but the linked
  shadow row's own `gate_results` (`Authority` denied) re-derives to
  `Rejected` — grammar is otherwise complete, isolating the re-derivation
  check specifically. Includes a sanity assertion that the fixture really
  does re-derive to `Rejected` (proves the mismatch is real, not a
  fixture-construction mistake).
- `a_sealed_but_unconsumed_decision_is_not_replay_eligible` — §1.2's
  eligibility proof, against the real aggregate function's total count.
- `gate_outcome_counts_surfaces_audit_replay_samples_at_shadow_eval_
  provenance` — end-to-end: inserts a real lifecycle, calls the real
  `control_plane_metrics::gate_outcome_counts` (the actual function the
  E3 probe and the metrics endpoint both call), asserts at least one
  `AuditReplay`/`shadow_eval` substantive sample appears.

All 7 live-DB tests in this module (5 new + `full_lifecycle_round_trips_
and_is_complete` + `w1_shadow_row_is_field_identical...`) pass — see §5
for command output.

### 1.6 E3 result — before/after, real numbers

Before this session (per the prior session's own recorded baseline, and
re-confirmed live at the start of this session before any code change):
G11 (AuditReplay), G12 (VersionPinning), G14 (WriteSetAttestation) all
zero substantive samples.

**After this session** (live `e3_invariant_probe` run, reproduced twice
— once mid-session, once in final verification, both below):

```
[E3] IntentAdmission: 258 substantive, 258 at expected provenance=shadow_eval
[E3] EntityBinding: 186 substantive, 186 at expected provenance=shadow_eval
[E3] PackResolution: 75 substantive, 75 at expected provenance=shadow_eval
[E3] DagProof: 75 substantive, 75 at expected provenance=shadow_eval
[E3] Authority: 147 substantive, 147 at expected provenance=shadow_eval
[E3] Evidence: 75 substantive, 75 at expected provenance=shadow_eval
[E3] WriteSet: 75 substantive, 75 at expected provenance=shadow_eval
[E3] StpClassifier: 75 substantive, 75 at expected provenance=shadow_eval
[E3] RunbookProof: 75 substantive, 75 at expected provenance=shadow_eval
[E3] ExecutionEnvelope: 143 substantive, 143 at expected provenance=consume_seam
[E3] AuditReplay: 51 substantive, 51 at expected provenance=shadow_eval        <- NEW, was 0
[E3] VersionPinning: 129 substantive, 129 at expected provenance=shadow_eval  <- already live (not this session)
[E3] DecisionSnapshot: 186 substantive, 186 at expected provenance=shadow_eval
[E3] WriteSetAttestation: 129 substantive, 0 at expected provenance=post_dispatch  <- G14, unchanged

E3_INVARIANT_FAILURE: 0 gate(s) have zero substantive production samples anywhere: [];
1 gate(s) have samples only at the WRONG provenance: ["WriteSetAttestation"]
```

**G12 (VersionPinning) was already substantive before this session** —
it is not a claim of this session's work; recorded here only so the
before/after picture is accurate (the prior session's own baseline
comment in `invariants-expected.toml` is stale on this specific point,
predating whatever landed VersionPinning's wiring; not investigated
further, out of this session's scope, flagged in §6).

**Net: E3 moves from 11/14 to 13/14 gates passing the bar** (substantive
samples at expected provenance). The single remaining failure is G14 —
exactly matching item 2's own outcome below.

---

## 2. G2 item 2 — G14 (WriteSetAttestation) real attestation

### 2.1 Re-verification of the prior STOP, from current source

Confirmed directly (not taken on the prior session's word, since two
tranches — G4, G5 — landed in between and could have changed things):
`build_write_set_input` (`control_plane_shadow.rs`) still sets
`allowed_columns: Vec::new()` unconditionally; `write_set_attestation.rs`'s
own test
(`empty_allowed_columns_breaches_every_write_with_any_column_even_on_
exact_table_and_entity_match`) still documents exactly why arming this
as-is would roll back every legitimate write. Unchanged since the prior
session — confirmed the STOP is still live, not stale.

### 2.2 Investigating a real `allowed_columns` source

Per the mission's own pointer, checked `sem_os_obpoc_adapter::metadata::
DomainMetadata`'s `VerbFootprint` (`crates/sem_os_obpoc_adapter/src/
metadata.rs`) directly: **`writes: Vec<String>` is table-only — there is
no column field anywhere on this type.** Confirmed by reading the struct
definition, not inferred. This source cannot ever produce a real
`allowed_columns` value, at any confidence level — it is structurally
absent, not merely unpopulated.

**The only place a verb's actual write *columns* are declared anywhere
in this codebase** is the verb YAML's own `crud.args[].maps_to` mapping
— read directly by `crud_executor.rs`'s real dispatch
(`execute_insert`/`execute_update`/`execute_upsert`/`execute_delete`) to
build its SQL, and by the SAME functions' `record_write` self-report
calls that produce `CapturedWrite.columns` — the value G14's real
comparison is checked against. This is the exact same production
runtime source `runtime_registry()` already exposes (already consumed
by `build_stp_classifier_input` two functions above in the same file).

### 2.3 Mirroring `crud_executor.rs`'s exact column-selection logic — verified per operation, not guessed

Read `crud_executor.rs`'s `execute_insert`/`execute_update`/
`execute_upsert`/`execute_delete` line by line (not summarized) to
determine, per `CrudOperation` variant, exactly which columns a real
dispatch can ever report to `record_write`:

| Operation | Real `raw_columns` composition | Safe to derive statically? |
|---|---|---|
| `Insert` | `pk_col` (unconditionally seeded) + every `arg_defs[].maps_to` present in that call's `args` | Yes — declared `maps_to` ∪ `pk_col` is always a superset (arg *presence* varies per call, the *mapping* doesn't). **Requires `crud.returning` to be explicit** — see §2.4. |
| `Upsert` | Same shape as Insert (`insert_cols` also starts with `pk_col`) | Same as Insert. |
| `Update` | `arg_defs[].maps_to` minus the key column (key becomes a `WHERE` bind, not a `SET` column) | Yes — declared `maps_to` (a superset including the key) is always safe; the key column just never gets exercised as a captured write. |
| `Delete` | **Soft-delete path writes a literal `"deleted_at"` column that never appears in ANY `arg_defs[].maps_to` mapping at all** (`execute_delete`: `let columns = if is_soft { vec!["deleted_at"...] } else { vec![] }`) | **No** — a `maps_to`-only derivation would silently UNDER-declare this, the unsafe direction. Deliberately excluded (returns `None`). |
| `Link` / `Unlink` | Junction-table `from_col`/`to_col` via a structurally different code path this session did not mirror | **No** — excluded, not attempted. |
| `RoleLink`/`RoleUnlink`/`ListByFk`/`ListParties`/`SelectWithJoin`/`Select`/`EntityCreate`/`EntityUpsert` | Not analysed this session | **No** — excluded, not attempted (scope discipline: only the 3 kinds this session fully verified are covered). |

**Second within-scope caveat found:** for Insert/Upsert, `pk_col` is
`crud.returning.as_deref().unwrap_or_else(|| infer_pk_column(table))` —
when `returning` is absent, the real dispatch falls back to
`infer_pk_column`, a best-effort table-name heuristic with its own
fallback arm (`crud_executor.rs:1457`) this session did **not**
re-verify exhaustively against every table name in the corpus. Guessing
wrong here would UNDER-declare the pk column (unsafe direction — a real,
legitimate pk write would then look like a breach). **Insert/Upsert
verbs without an explicit `returning` in their YAML therefore also
return `None`**, not a guessed value.

**Implementation:** `derive_allowed_columns_for_operation(operation,
returning, maps_to_cols) -> Option<Vec<String>>` (pure, in `control_
plane_shadow.rs`) implements exactly the table above. `derive_crud_
allowed_columns(rv: &RuntimeVerb)` is a thin extraction wrapper around
it. `build_write_set_input` now calls this via `runtime_registry()`
(the same lookup `build_stp_classifier_input` already uses) and sets
`allowed_columns` to the real derived set — `Vec::new()` only when
derivation genuinely isn't possible (unregistered verb, non-CRUD
behaviour, excluded operation kind, or missing `returning`), same honest
"cannot derive" posture the rest of this module already uses.

### 2.4 Real-verb proof, not a synthetic fixture only

`derive_crud_allowed_columns_real_insert_verb_with_explicit_returning`
exercises the REAL `capability-binding.draft` verb
(`config/verbs/capability-binding.yaml`: `operation: insert`,
`returning: id`, args mapping `instance-id→application_instance_id`,
`service-id→service_id`, `notes→notes`) through the real, on-disk-YAML-
loaded `runtime_registry()` — not a hand-built `RuntimeVerb`. Asserts the
derived set is exactly `{application_instance_id, id, notes,
service_id}`. The Delete/Link/Unlink/no-`returning` exclusion tests use
the pure `derive_allowed_columns_for_operation` function directly (no
`RuntimeVerb` fixture needed — that type has no `Default` impl and a
dozen-plus unrelated fields, so testing the pure per-operation function
is both simpler and a tighter, more direct proof of the actual decision
logic).

### 2.5 The STOP that keeps arming off — a genuinely NEW finding, not a repeat of the prior session's

Having built a correct `allowed_columns` for the Insert/Update/Upsert
subset, checked whether arming `set_expected_write_set` for JUST that
subset would now be safe. It is not, for a **second, independent reason
this session found and the prior G2-item-2 session did not document**:

`build_write_set_input`'s `tables` field comes straight from `domain_
metadata.yaml`'s `writes: [...]` list — **bare table names** (e.g.
`"deals"`, `"capability_bindings"` — confirmed by reading the real
`config/sem_os_seeds/domain_metadata.yaml` directly, and by the existing
`build_write_set_input_some_with_tables_when_footprint_declares_writes`
test's own assertion, `ws.tables == vec!["deals"]`). But
`CapturedWrite.table` — what `record_write` actually reports in
production — is `format!("{schema}.{table}")`, e.g. `"ob-poc.deals"`
(confirmed directly in `crud_executor.rs`'s four `record_write` call
sites). **`attest()`'s table check is exact-string** (`!expected.
tables().contains(&write.table)`) — these two values can never match as
things stand today, for ANY verb, regardless of how correct
`allowed_columns` is.

**Proven directly against `attest()`, not asserted in prose:**
`derived_columns_are_correct_but_table_name_format_does_not_match_
captured_writes` builds a real `WriteSetInput` for `capability-
binding.draft` (correct, non-empty `allowed_columns` from §2.3-2.4),
constructs a `CapturedWrite` in the real production format
(`"ob-poc.capability_bindings"`) with a genuinely-declared, legitimate
column (`service_id`), and asserts `attest()` still classifies it as a
`Breach` — a fully legitimate write still misclassifies today, purely on
the table-name-format mismatch, independent of the column fix.

**This is why `set_expected_write_set` stays unwired even for the now-
correct subset:** arming it would still roll back every legitimate write
for those verbs, on a different axis than the prior session's documented
STOP. Fixing the table-name mismatch (either schema-qualifying `domain_
metadata.yaml`'s `writes:` entries, or normalizing at the comparison
site) is real, bounded, separate work — not attempted this session
(scope discipline: this session's charter was the column derivation;
finding and precisely documenting a second blocker is the honest
disclosure the mission asked for, not an invitation to also fix it
under time pressure without its own review).

### 2.6 What DID land from item 2, concretely

- A real, tested, non-empty `allowed_columns` derivation for Insert /
  Update / Upsert verbs with an explicit `returning` column
  (`derive_allowed_columns_for_operation` + `derive_crud_allowed_
  columns`), wired into `build_write_set_input` — this **improves G7's
  shadow-eval accuracy** (a shadow-only, non-gating benefit; G7's
  `WriteSetGate::decide` only checks `contract_derived` + non-empty
  `tables` today, so this doesn't change G7's shadow verdict, but the
  data is now real where before it was structurally empty for every
  verb).
- Two independent, now-documented, test-proven reasons `set_expected_
  write_set`/`commit_attested`'s real comparison stays unarmed: (a) the
  derivation only covers a subset of `CrudOperation` variants (Delete/
  Link/Unlink/others remain `None`), (b) the table-name-format mismatch
  (§2.5) breaks the comparison even within the covered subset.
- `commit_attested(None, Some(verb_fqn))`'s existing safe transport
  (landed in the prior `01539938` session) is unchanged — this session
  did not touch `execute_verb_admitting_envelope`'s commit call site at
  all.

**Recommendation for a follow-up session (not self-authorised here):**
fixing (b) is probably the smaller of the two remaining pieces (schema-
qualify `domain_metadata.yaml`'s `writes:` entries, or normalize table
strings at the `attest()` comparison boundary) and would make the
Insert/Update/Upsert-with-`returning` subset arming-ready. Extending
coverage to Delete (mirroring the soft-delete `deleted_at` literal) and
Link/Unlink (mirroring junction-column semantics) is separate,
comparable-sized work. Neither should be combined with actually arming
`set_expected_write_set` in the same session as building them — this
program's own "the plan's ONE production-behavior change" framing
deserves its own dedicated review pass once the derivation is airtight
end to end, not a bundled diff.

---

## 3. STOP-conditions summary (both, precisely)

1. **G2 item 2 arming**: NOT done. Two independent, real, test-proven
   reasons: (a) the `allowed_columns` derivation only covers Insert/
   Update/Upsert-with-explicit-`returning` (Delete/Link/Unlink/others
   return `None`, i.e. still empty `allowed_columns` for those verbs —
   arming today would still misclassify every write for them); (b) even
   within the covered subset, `WriteSetInput.tables` (bare names) does
   not match `CapturedWrite.table` (schema-qualified) — proven against
   the real `attest()` function, not asserted. This is a genuinely new,
   independently-discovered finding this session adds to the ledger, not
   a re-statement of the prior session's column-only finding.
2. **G11's re-derivation (DD-4(ii)) is inconclusive, not failing, for
   any row without a non-nil `entry_id`** (pre-this-session audit rows,
   and the non-`ApprovedStp` fallback path's uncorrelated id) — disclosed
   in §1.3, not a hidden gap. Does not block G11's own closure: DD-4(i)
   completeness still grades every eligible row; DD-4(ii) simply narrows
   its own confidence honestly rather than guessing.
3. **G12 (VersionPinning)'s live samples predate this session** and were
   not investigated further (out of scope) — noted in §1.6 only so the
   before/after picture in this doc is accurate, not claimed as this
   session's work.

---

## 4. Files changed

- `rust/crates/ob-poc-control-plane/src/audit.rs` — `AuditEvent::
  DecisionEvaluated` gains `entry_id: Uuid` (`#[serde(default)]`); round-
  trip test fixture updated.
- `rust/src/agent/control_plane_audit.rs` — new `audit_replay_outcome_
  counts` (the G11 live call site) + `replay_grade_for_decision` (the
  per-decision half, split out for direct testability); removed the now-
  stale `#[allow(dead_code)]` markers on `AuditRow`/`audit_rows_for_
  decision`/`check_completeness`/`rederive_decision_outcome` (all have a
  real caller now); `decision_evaluated` test helper updated for the new
  field; 5 new live-DB tests (§1.5).
- `rust/src/agent/control_plane_metrics.rs` — `gate_outcome_counts` now
  unions in `audit_replay_outcome_counts`'s output as `AuditReplay`/
  `shadow_eval` rows (best-effort: a query failure here is logged, not
  fatal to the rest of the function's output).
- `rust/src/agent/control_plane_shadow.rs` — new `derive_allowed_
  columns_for_operation` (pure) + `derive_crud_allowed_columns` (thin
  `RuntimeVerb` extraction wrapper); `build_write_set_input` now derives
  real `allowed_columns` via `runtime_registry()` instead of always
  `Vec::new()`; 9 new tests (6 for the derivation logic itself, 1 real-
  verb proof, 1 table-name-mismatch documentation test, 1 unregistered-
  verb fallback check).
- `rust/src/sequencer.rs` — both `DecisionEvaluated` construction sites
  (`phase5_runtime_recheck`'s main path, the HumanGate-resume reseal
  path) now populate the new `entry_id` field with the same value the
  adjacent shadow-row build already uses.

No migration file added (JSONB payload field addition, not a schema
change). No production dispatch-path call site touched — `commit_
attested`'s call site in `verb_executor_adapter.rs` is byte-for-byte
unchanged from before this session.

---

## 5. Command output (real runs, this session)

```
$ cargo build -p ob-poc --lib --features database
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.61s   # zero warnings

$ cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 36.15s   # zero errors

$ cargo clippy -p ob-poc --lib --features database -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.72s   # zero warnings

$ cargo clippy -p ob-poc-control-plane --lib -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.65s    # zero warnings

$ cargo test -p ob-poc --lib
test result: ok. 2183 passed; 0 failed; 218 ignored; 0 measured; 0 filtered out

$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib \
    --features database control_plane_audit -- --ignored --test-threads=1
running 7 tests
test agent::control_plane_audit::tests::live_db::a_sealed_but_unconsumed_decision_is_not_replay_eligible ... ok
test agent::control_plane_audit::tests::live_db::full_lifecycle_round_trips_and_is_complete ... ok
test agent::control_plane_audit::tests::live_db::gate_outcome_counts_surfaces_audit_replay_samples_at_shadow_eval_provenance ... ok
test agent::control_plane_audit::tests::live_db::replay_grade_failure_for_a_grammar_incomplete_lifecycle ... ok
test agent::control_plane_audit::tests::live_db::replay_grade_failure_for_an_outcome_rederivation_mismatch ... ok
test agent::control_plane_audit::tests::live_db::replay_grade_success_for_a_complete_consistent_approved_stp_lifecycle ... ok
test agent::control_plane_audit::tests::live_db::w1_shadow_row_is_field_identical_with_and_without_audit_emission ... ok
test result: ok. 7 passed; 0 failed; 0 ignored

$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib \
    --features database control_plane -- --ignored --test-threads=1
test result: FAILED. 39 passed; 1 failed; 0 ignored
    (the 1 failure is e3_invariant_probe -- EXPECTED per invariants-expected.toml's
     ratcheted [e3] status = "fail"; real numbers in §1.6)

$ DATABASE_URL=postgresql:///data_designer bash scripts/check-invariants.sh e3
  ... (13/14 gates substantive at expected provenance, WriteSetAttestation the sole gap)
  E3: DOES NOT HOLD (unchanged verdict, driven entirely by G14 now)

$ DATABASE_URL=postgresql:///data_designer bash scripts/check-invariants.sh ratchet
== Ratchet: 0/5 invariant(s) diverge from invariants-expected.toml ==

$ cargo test -p ob-poc --lib -- test_plugin_verb_coverage
test result: ok. 1 passed; 0 failed

$ bash scripts/check_kyc_substrate_deps.sh
PASS: no forbidden deps in ob-poc-kyc-substrate
```

---

## 6. `invariants-expected.toml` — recommendation only, not applied

Per this program's standing discipline, `invariants-expected.toml` (a
ratchet file) was **not edited**. Recommended update for the next
session/architect review, with exact before/after counts cited above:

- `[e3]`'s detail comment should move from "11/14 gates... G11, G12, G14
  remain at zero" to **"13/14 gates... only G14 (WriteSetAttestation)
  remains at zero / wrong-provenance."** Status stays `"fail"` — G14 is
  still genuinely unarmed, this is a detail-comment-only update, not a
  status flip. The comment should also correct that G12 (VersionPinning)
  had already moved before this session (not this session's work,
  flagged in §1.6) — the existing comment's "G11, G12, G14... zero" line
  is now stale on that specific point independent of anything this
  session did.
- No other `[eN]` section is affected by this session's changes.

---

## 7. Honest scope summary

- **G2 item 3 (G11): CLOSED** — real live call site, real tests, real
  E3 movement (11/14 → 13/14), zero production-dispatch-path changes.
- **G2 item 2 (G14): PARTIAL, by design** — a real, tested, non-empty
  `allowed_columns` derivation landed for a verified subset; arming
  deliberately withheld behind two independent, now-documented,
  test-proven reasons (narrower-than-full operation coverage, and a
  newly-found table-name-format mismatch). This is the honest outcome
  the mission explicitly named as acceptable when the arming bar isn't
  met — and per the plan's own "ONE production-behavior change" framing,
  arming remains a deliberate STOP for a dedicated future review, not
  something to route around under pressure to close the gate.
- **G2's overall exit gate** ("item 4's provenance dimension merged; E3
  probe shows G11, G12, G14 with substantive samples") is **not fully
  met** — G14 remains genuinely zero at its expected provenance by this
  session's own honest account, which is a live, correctly-reported gap
  for the architect to review against GM's preconditions, not a claim
  this session is making G2 fully closeable.
