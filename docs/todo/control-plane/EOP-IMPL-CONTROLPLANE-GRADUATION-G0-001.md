# EOP-IMPL-CONTROLPLANE-GRADUATION-G0-001 — G0 implementation slicing (+ G2 prep)

### Basis: `EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.1.md` (RATIFIED-WITH-AMENDMENTS per `EOP-PIR-CONTROLPLANE-GRADPLAN-001.md`, this session)
### Date: 2026-07-13
### Status: slicing only. No implementation work performed in this session.

Amendments carried from the PIR that touch G0/G2 scope: GRADPLAN-D-002 (runbook
count correction, folded into Slice 2 below) and GRADPLAN-D-003 (G0's exit-gate
"first non-fixture row" predicate, folded into Slice 5 below). GRADPLAN-D-001
(the `NotApplicable`/per-gate-source gap) belongs to G2 item 3's design doc
and G5, not this doc's slices — flagged in Slice 4/G2-prep below where it
first becomes relevant.

---

## G0 — sliced into grind sessions

G0 items 1-4 may be sliced in any order (plan's own constraint); item 5
(merge+deploy) is its own final slice, blocked on AD-3 ratification and
architect review of Slices 1-4. Five slices below, one per plan work item
plus the merge slice.

### Slice 1 — E5 to expected-pass (plan G0 item 1)

**Scope:** `audits/surface/*.txt` baselines for 5 crates (`dsl-runtime`,
`ob-poc`, `ob-poc-boundary`, `ob-poc-control-plane`, `ob-poc-types`);
root-cause of `ob-poc-agent`'s `cargo public-api` measurement error;
`Cargo.toml` feature-declaration fix for 4 crates
(`ob-poc-derived-attributes`, `ob-poc-entity-linking`, `ob-poc-taxonomy`,
`ob-poc-trading-profile`) failing `--no-default-features` builds.
`invariants-expected.toml`'s `[e5]` block, flipped to `status = "pass"` in
the same diff once all sub-items are green.

**Files touched:** `audits/surface/{dsl-runtime,ob-poc,ob-poc-boundary,ob-poc-control-plane,ob-poc-types}.txt` (regenerated); `rust/crates/{ob-poc-derived-attributes,ob-poc-entity-linking,ob-poc-taxonomy,ob-poc-trading-profile}/Cargo.toml`; whatever `ob-poc-agent` config/tooling file the root-cause points to (unknown until investigated — see STOP-condition below); `invariants-expected.toml`.

**Exit evidence (exact commands):**
```
bash scripts/check-invariants.sh e5
```
must print `E5: HOLDS` and exit 0. Sub-checks whose output should be pasted into the tranche summary as corroborating evidence:
```
cd rust && cargo build --workspace --features database
cd rust && cargo test -p ob-poc --lib --features database
bash scripts/check-public-api-surface.sh
```

**Expectation flips carried:** `invariants-expected.toml` `[e5].status` → `"pass"`, comment rewritten to state the baselines are current as of the closing commit hash.

**STOP-conditions (end session for review, do not handle inline):**
- Refreshing a crate's `audits/surface/*.txt` baseline reveals a **public-API surface change beyond noise** — i.e. the diff isn't just "recorded a symbol that was already real and shipped in an earlier tranche" (the documented case for all 5 crates per the invariant-promotion session) but includes a genuinely new/widened `pub` item nobody flagged. Stop; this is a B1-class surface-widening question needing its own review, not a baseline refresh.
- `ob-poc-agent`'s `cargo public-api` root cause turns out to be a **code defect** (e.g. a genuine compile/feature-resolution problem) rather than a tooling/measurement-only gap as R:§D2 currently characterizes it. Stop; this reclassifies the item from "hygiene refresh" to "bug fix," which needs its own scoping.
- Any of the 4 `--no-default-features` fixes requires **more than a `Cargo.toml` feature-declaration change** (e.g. the crate's code genuinely doesn't compile without a default-only dependency, requiring source changes). Stop; this is larger than the "pre-existing Cargo.toml feature gap" R:§D2/the invariant-promotion session characterizes it as.

**Session tier:** grind-suitable (mechanical baseline refresh + Cargo.toml edits), except the `ob-poc-agent` root-cause investigation, which is grind-suitable only until/unless a STOP-condition fires.

---

### Slice 2 — Runbook corrections (plan G0 item 2)

**Scope:** doc-only edits to `docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md`, producing v0.3:
- §2 readiness table: replace the "G1 (IntentAdmission) only" claim with the corrected reading — Path A calls `execute_verb_admitting_envelope` at `step_executor_bridge.rs:553` (not the stale `:474` bare `execute_verb`); **11/14 gates** (G1-G9, G12 partial, G13) have real shadow inputs at that call site, not "12/14" as the plan's own G0 item 2 text states — GRADPLAN-D-002's correction, apply here.
- §4 Path A checklist box 1: flip to DONE, citing commit `5a704f4e`.
- §8 open items: re-scope T6.1a as a two-repo change (real producer `bpmn-lite-engine::plan_walker::dispatch_callout`, per R:§C3), delete the "This is an ob-poc-only change" sentence and its supporting paragraph (runbook v0.2 lines ~156-161).

**Files touched:** `docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` only.

**Exit evidence:**
```
git log -1 --format=%H -- docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md
grep -c "12/14 gates" docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md   # expect 0
grep -n "v0.3" docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md | head -1  # expect a version-header hit
grep -c "ob-poc-only change" docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md  # expect 0
```
The commit hash from the first command is the exit evidence cited in the tranche's ledger entry (standing rule 2).

**Expectation flips carried:** none directly (doc-only; no `invariants-expected.toml` row is scoped to runbook prose).

**STOP-conditions:**
- Re-checking `step_executor_bridge.rs`'s admitting call at correction time finds it has moved off line 553 or changed shape (e.g. no longer passes `None`) — re-verify against Slice-time HEAD before writing the number into v0.3; a stale line number in a "corrected" doc defeats the point.
- Re-checking R:§A1's gate table against `control_plane_shadow.rs`/`sequencer.rs` at correction time shows a *different* real-gate count than 11/14 (i.e., new gate wiring landed on another branch/tranche since this session) — recount from code, don't copy either this doc's or the plan's number blind.

**Session tier:** grind-suitable (doc edits against already-cited code loci; each claim in this doc's Phase 2 checks already re-verified the citations this session).

---

### Slice 3 — Governance index (plan G0 item 3)

**Scope:** create `docs/todo/control-plane/INDEX.md` listing every live governing artifact: plan 001 (`EOP-PLAN-CONTROLPLANE-001_Implementation-Plan_v0.1.md`), the 002 track's scope note, the graduation plan (this review's subject), the graduation runbook, `control-plane-pir-001.md` (marked historical), the invariant-promotion session doc + its evidence file, the research doc (`EOP-RESEARCH-CONTROLPLANE-GRADUATION-001.md`), this PIR (`EOP-PIR-CONTROLPLANE-GRADPLAN-001.md`) and this slicing doc, the T9.2/T11.1a/T11.1b/T11.2/T11.F.2 design docs, MCA-001/MCA-002, `workspace-hygiene-001.md`. Each entry: one line, path, one-sentence purpose, status (live/historical).

**Files touched:** `docs/todo/control-plane/INDEX.md` (new file).

**Exit evidence:**
```
test -f docs/todo/control-plane/INDEX.md && echo EXISTS
ls docs/todo/control-plane/*.md docs/research/control-plane-*.md docs/architecture/EOP-VS-CONTROLPLANE-001*.md 2>/dev/null | wc -l
```
Manual cross-check (documented in the tranche summary, not automatable): every file the second command lists appears as an entry in INDEX.md, and every INDEX.md entry resolves to a real path (`test -f` per line).

**Expectation flips carried:** none.

**STOP-conditions:**
- A discovered artifact's status is ambiguous (neither clearly live nor clearly historical, e.g. a design doc whose ratification status isn't stated in its own text) — flag for architect classification rather than guessing "live" or "historical."

**Session tier:** grind-suitable.

---

### Slice 4 — Ledger provability backfill, bounded (plan G0 item 4)

**Scope:** strictly citation-adding, no new engineering. Confirmed this session (`bash scripts/check-invariants.sh e1`): 3/45 rows provably CLOSED (C-022, C-030, C-037); C-001 is claimed CLOSED but fails only on the commit-hash sub-check (`hash=0,symbol=1,resolves=1` in the gate's own output) — its ledger row already names "Slice 3.1 (2026-04-22)" in prose but does not backtick-quote a commit hash. This slice's job: find the actual commit hash for the Slice 3.1 fix referenced in C-001's row text (`OBPOC_ALLOW_RAW_EXECUTE` deletion) via `git log`, add it to the row. Re-run E1 and record the new provable count in `[e1]`'s detail comment. **Do not** touch any of the other 41 non-closed rows — they are genuinely open per the gate's own output, not backfill candidates.

**Files touched:** `docs/research/control-plane-ownership-ledger.md` (C-001's row only); `invariants-expected.toml` (`[e1]` comment, count only — status stays `"fail"`, 42 rows remain genuinely open).

**Exit evidence:**
```
bash scripts/check-invariants.sh e1
```
Expect `Provably CLOSED: 4` (up from 3), with C-001 no longer appearing in the `Non-closed/unproven IDs` list.

**Expectation flips carried:** `invariants-expected.toml` `[e1]` comment only (status stays `"fail"` — standing rule 1 forbids flipping the top-level status here since 41/45 rows remain open; only the detail comment's count changes, and only because this slice's own diff earned it).

**STOP-conditions:**
- No commit hash can actually be found for the `OBPOC_ALLOW_RAW_EXECUTE` deletion referenced in C-001's prose (e.g. it predates the ledger, or the deletion was part of a squashed/rebased history with no clean single-commit attribution) — do not fabricate or approximate a hash; report the row as genuinely unprovable and leave it open, flag for architect review of whether the row's disposition text itself needs revision.

**Session tier:** grind-suitable (this is exactly a `git log`/`git blame` lookup plus a one-line edit).

---

### Slice 5 — Merge + deploy (plan G0 item 5)

**Superseded (2026-07-13):** this slice's merge step is now the standalone GM tranche in plan v0.3 (per AD-3's resolution, "(a) hold the merge"); it is retained below for its exit-evidence text only, corrected for GRADPLAN-D-010.

**Scope:** merge `codex/phase-1-5-governance-closure` to `main`, deploy, confirm real Path A traffic reaches `control_plane_shadow_decisions`. Per GRADPLAN-D-003's amendment: before merging, define and record a concrete deploy-marker predicate (e.g. insert a single sentinel row or record a wall-clock deploy timestamp in the ledger) so "first non-fixture row" has a queryable definition — `decided_at > :deploy_marker_timestamp AND session_id NOT IN (:known_test_harness_session_ids)` — rather than relying on manual inspection. (GRADPLAN-D-010, fixed 2026-07-13: the table's timestamp column is `decided_at`, not `created_at`.)

**Files touched:** none beyond the merge itself (this is a git/ops action, not a code change) plus a ledger entry recording the deploy marker and merge commit hash.

**Exit evidence (exact commands/status, run post-deploy):**
```
git log -1 --format=%H main   # merge commit hash, cited in the ledger entry
psql "$DATABASE_URL" -c "SELECT verb_fqn, count(*) FROM \"ob-poc\".control_plane_shadow_decisions WHERE decided_at > '<deploy_marker_timestamp>' GROUP BY verb_fqn ORDER BY 2 DESC;"
```
Non-empty result with a real verb FQN (not `nonexistent.verb`/`test.*`) after the deploy marker is the evidence; record the query output verbatim in the tranche summary and the ledger entry.

**Expectation flips carried:** none directly (this slice does not itself flip any `invariants-expected.toml` row; it is the precondition for G1/G2's work and the window's start, which the *next* tranches' diffs will cite).

**STOP-conditions (hard, per the plan's own text — "the one irreversible step"):**
- Any of Slices 1-4 have not been architect-reviewed and merged first — do not merge Slice 5 ahead of or bundled with 1-4.
- AD-3 has not been explicitly ratified (even though the plan's recommendation is (b)/ship-now, the plan states AD-3 "blocks G0's merge step" — ratification is a precondition, not a formality to skip because the recommendation seems obviously right).
- `bash scripts/check-invariants.sh e5` does not show `HOLDS` immediately before merge (Slice 1 must be genuinely closed, not just believed closed from an earlier run).

**Session tier:** ops + architect. Not grind work. Blind-review required before this slice executes (matches the plan's own text verbatim).

---

## G2 items 1-2 — pre-sliced (AD-free, parallel-safe)

### Slice 6 — G12 metrics undercount fix (plan G2 item 1)

**Scope:** `gate_outcome_counts`'s SQL (`rust/src/agent/control_plane_metrics.rs:95-124`) currently classifies `report_to_json`'s historical `"missing"` sentinel (rows predating a gate's registration in `evaluate_shadow`'s map) as `'Unrecognised'` — the same bucket as a genuinely-unrecognised value, undercounting G12 specifically. Fix: distinguish the `"missing"` sentinel string from other unrecognised values in the `CASE` expression (add a `WHEN kv.value = 'missing' THEN 'HistoricalMissing'` arm or equivalent, decision left to the implementing session but must be a **named, distinct** outcome_kind, not silently folded back into `'Unrecognised'`). Stretch, optional, flag if taken: populate `bus_catalogue_version` where reachable (per the plan's own "stretch" framing — do not treat as required for this slice's exit).

**Files touched:** `rust/src/agent/control_plane_metrics.rs` (the SQL `CASE` expression + `GateOutcomeCount`/test assertions if the outcome_kind enum needs a new label).

**Exit evidence:**
```
DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database gate_outcome_counts_classifies_by_variant_prefix_not_full_debug_string -- --ignored --nocapture
DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database e3_invariant_probe -- --ignored --nocapture
```
The second command's output should show G12's substantive-sample count increase versus the pre-fix baseline (paste both before/after runs in the tranche summary — this is the kind of before/after evidence `git stash` + re-run already established as this ledger's convention for T10.3's isolation fix).

**Expectation flips carried:** `invariants-expected.toml` `[e3]` detail comment only (count of gates with substantive samples may move from 10/14 toward 11/14 if G12's undercounted rows surface; top-level status stays `"fail"` — G10/G11/G14 remain genuinely zero regardless of this fix).

**STOP-conditions:**
- The `"missing"` sentinel turns out to be ambiguous in the live data (i.e. some `"missing"` rows are genuinely pre-registration-gap historical artifacts and others are a different, currently-undiagnosed zero-value case) — stop and re-characterize before writing a `CASE` arm that conflates two different things under one new label.

**Session tier:** grind-suitable.

---

### Slice 7 — G14 post-dispatch call site (plan G2 item 2)

**Scope:** wire `set_expected_write_set` + `commit_attested` into the sequencer's commit path. Confirmed this session: `sequencer.rs:7549` calls plain `scope.commit()`; `set_expected_write_set` (`sequencer_tx.rs:138`) has zero production callers (only test-module call sites at `sequencer_tx.rs:321/373/414`, confirmed inside `mod t5_write_set_attestation_tests`). This slice replaces (or adds a mode-gated branch alongside) the plain `commit()` call with `commit_attested`, sourcing the `WriteSetProof` from whatever `VerbFootprint.writes`-derived expectation is already available at that call site (T9.1e's table-level `WriteSet` gate input is the nearest existing real source, per R:§A1's G7 row — confirm at implementation time whether it's directly reusable or needs its own small adapter). Advances ledger row C-032 (currently PARTIALLY CLOSED).

**Files touched:** `rust/src/sequencer.rs` (the commit call site, ~line 7549); `docs/research/control-plane-ownership-ledger.md` (C-032 row update, citing the new call site + commit hash).

**Exit evidence:**
```
cd rust && rg -n "commit_attested" src/sequencer.rs   # expect at least one non-comment, non-test hit
DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database sequencer_tx::t5_write_set_attestation_tests -- --ignored --nocapture
DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database e3_invariant_probe -- --ignored --nocapture
```
G14 showing a non-zero substantive-sample count in the third command's output (it was 0/14 pre-fix, alongside G10/G11) is the E3-side exit evidence.

**Expectation flips carried:** `invariants-expected.toml` `[e3]` detail comment (G14 moves off the zero-sample list); ownership ledger's C-032 row status text updated with the new commit hash and call site (standing rule 2 — disposition class, hash, resolving symbol).

**STOP-conditions:**
- No real `WriteSetProof` source is cleanly available at the `sequencer.rs:7549` commit call site without restructuring beyond a call-site swap (i.e., building the proof requires information not already in scope at that point in the function) — stop; this reclassifies from "wire an existing mechanism" to "extend the mechanism," needing its own design pass, mirroring T10.3's own experience discovering two structural gaps mid-implementation (recorded in the ledger, Addendum C / T10.3 entry) — if this slice hits the same shape of surprise, follow the same discipline: flag before proceeding, don't silently absorb scope.
- Wiring `commit_attested` here changes behavior for any verb currently succeeding via plain `commit()` (i.e. a real excess/undeclared write gets caught and rolled back where it previously wasn't) — this is a **production behavior change**, not a shadow-only addition like every other G-tranche item in this plan. Stop and flag for architect review before merging even if tests are green; this is qualitatively different from the plan's "shadow-only, zero behavior change" posture elsewhere and the plan itself does not explicitly call this out.

**Session tier:** grind-suitable against the two STOP-conditions; likely to trip the first one given T10.3's own precedent of finding structural gaps here.

---

## G2 item 3 — scope statement only (not sliced)

**G11 / T7.1 audit stream** (plan G2 item 3): per the plan's own text, "the largest item in G2; if it needs its own session, split it out as G2b." Confirmed via ledger (T9.7 entry, line 388: *"G11 (AuditReplay) — still a 7-line placeholder; its owning infrastructure (T7.1's unified `control_plane_audit` append-per-decision stream) doesn't exist"*) that this is greenfield infrastructure, not a wiring task — no existing table, no existing adapter beyond the 7-line stub. **Scope statement, not a slice:** a short design doc (same review flow as the T9.x/T10.x "Addendum" design docs already in this ledger) must answer: (a) what does `control_plane_audit` persist per decision that `control_plane_shadow_decisions` doesn't already capture (is this a new table or a derived view over the existing shadow-decision rows?); (b) append-only guarantees and retention; (c) what "replay" means operationally (G11's own name is `AuditReplay` — does the gate need to actually replay a decision against the audit stream, or merely confirm an audit record exists?); (d) relationship to GRADPLAN-D-001's finding that no per-gate source/`NotApplicable` machinery exists yet — if G11's audit stream is meant to also serve as the source of per-gate provenance for G2's own exit-gate language ("G14 via its post-dispatch source, which the probe's per-gate source map must reflect"), that dependency should be named explicitly in the design doc rather than discovered later. This design doc is G2b's own first deliverable; implementation slicing for G2b follows once it's ratified, not in this document.

---

## Explicitly not sliced (with reasons)

| Tranche | Why not sliced here |
|---|---|
| G1 | Blocked in part on AD-1 (item 2/exit only, per GRADPLAN-D-004's correction — items 1/3/4 could in principle be sliced once GRADPLAN-D-006's design doc lands, but that design doc doesn't exist yet; slicing G1 grind work ahead of it would just re-invent the design inline, the exact anti-pattern the amendment exists to prevent). |
| G3 | Is AD-2 itself — architect + Fable design work, not a grind slice by the plan's own text. |
| G4 | Needs G3 (AD-2 ratified) first; also inherits GRADPLAN-D-001's exit-gate ambiguity indirectly only through G5, not directly — G4's own exit gate (E2 structural exclusivity + atomicity tests) is machine-checkable as written and could be sliced once G3 lands, but is out of scope for *this* doc (G0/G2-only per the task's Phase 5 instruction). |
| G5 | Needs G4; additionally inherits GRADPLAN-D-001 directly (its exit gate depends on the `NotApplicable` variant and per-gate source map that do not exist yet) — should not be sliced until that amendment is applied and G4 lands. |
| G6a | Two-repo, architect-involved (bpmn-lite is not ob-poc's to merge; GRADPLAN-D-008 found no drift, so the two carrier options in the plan remain valid, but the choice itself is explicitly architect-ratified, not a grind slice). |
| G7 | Needs the counted window (starts at G2's deploy) plus G1 plus AD-2; ops + architect event, not grind work, per the plan's own text. |

---

## Confirmation

Two output files this session: `EOP-PIR-CONTROLPLANE-GRADPLAN-001.md` (Part 1) and this file (Part 2). The plan under review, the runbook, and the ownership ledger were read but not edited. No `git commit` was run.
