# Session: EOP-PLAN-CONTROLPLANE-GRADPLAN-001-v0.3 — v0.3 re-validation + G0 implementation

### EOP-SESSION-CONTROLPLANE-GRADPLAN-G0-IMPL-001
### Date: 2026-07-13
### Status: COMPLETE. No commit, merge, deploy, or push performed.

---

## Part 1 — v0.3 re-validation (addendum, not a new PIR)

Full addendum appended to `docs/todo/control-plane/EOP-PIR-CONTROLPLANE-GRADPLAN-001.md` (its own "## v0.3 re-validation addendum" section — read there for the full text; summarized here).

- All six PIR amendments (GRADPLAN-D-001..006) confirmed **APPLIED** in v0.3, verbatim-traceable — none PARTIALLY-APPLIED or NOT-APPLIED.
- AD-3's "(a) hold the merge" resolution is consistently reflected: G0 no longer contains a merge step (moved to standalone tranche GM); nothing in the restructured G0/G1/G2 text implicitly assumes the old window-opens-at-G0 model.
- GM/GW spot-checked against the same defect classes the original PIR applied to G0-G6: both pass (machine-checkable exit gates modulo D-010 below; neither touches Path A's shadow call site).
- AD-1/AD-2 confirmed byte-identical to v0.1 (unchanged, as expected — only AD-3 was resolved this round).
- Live ground-truth re-checks (GateResult variant count, gate_outcome_counts grouping, step_executor_bridge.rs:553, git log on the touched files): **no drift** since the original PIR session.
- **One new defect found: GRADPLAN-D-010 (MINOR).** GM's deploy-marker predicate (and the G0-001 slicing doc's now-superseded Slice 5) references a column `created_at` that does not exist on `control_plane_shadow_decisions` — the actual column is `decided_at` (`rust/migrations/20260710_control_plane_shadow_decisions.sql`). This is a pre-existing error carried from the original PIR's own D-003 recommendation text, not something v0.3 introduced. One-token fix recommended in GM's text before that tranche executes (not before G0). **Not fixed by me** — GM is out of scope for this session (held by AD-3) and the plan doc itself is not mine to amend outside the read-only review convention.
- **Verdict: no BLOCKER. Part 2 (G0 implementation) proceeded.**

---

## Part 2 — G0 implementation

Reconciliation against v0.3: v0.3's G0 has 4 work items (merge/deploy moved out to GM). These map 1:1 to the G0-001 slicing doc's Slices 1-4; Slice 5 (merge) is out of scope per the mission's explicit exclusion and per v0.3's own restructuring.

### Slice 1 — E5 workspace hygiene (plan G0 item 1) — **PARTIAL, 2 STOP-conditions fired**

**What ran:** `bash scripts/check-invariants.sh e5` (full run, confirmed `E5: DOES NOT HOLD`, same 4 `GateResult` variants, same crate list as the invariant-promotion session's original characterization). A first attempt to run the full `scripts/check-public-api-surface.sh` (which iterates the ~90-crate workspace) was started in the background, found to be taking far too long for the 5-crate scope this slice actually needs (it had reached crate #70+ after 8+ minutes, mostly re-confirming a pre-existing, out-of-scope, workspace-wide Send/Sync trait-bound-ordering diff — a toolchain-output-formatting artifact unrelated to this slice), and was killed. Replaced with targeted `cargo +nightly public-api -p <crate> --all-features` runs against exactly the 5 named crates.

**Refreshed (4/5), each diff traced to a real already-landed commit before overwriting:**

| Crate | Diff size | Traced to | Verdict |
|---|---|---|---|
| `dsl-runtime` | 4 lines | `c072f3e8` (T10.3, 2026-07-11) | noise-only (already-shipped `execute_crud_in_scope`/`record_write`) — refreshed |
| `ob-poc-control-plane` | 230 lines | `7c7397a4` (T9.7, 2026-07-11) | noise-only (already-shipped RunbookProof/VersionPinning gates + `decision::evaluate`) — refreshed |
| `ob-poc-boundary` | 2 lines | `131c9de2`/`f00c1755` (T9.2, 2026-07-11) | noise-only (already-shipped `verify_pins_in_scope` rename) — refreshed |
| `ob-poc-types` | 185 lines | `7821ecb7` (T11.1b, 2026-07-12) | noise-only (already-shipped `intent::IntentArgValue`) — refreshed |

Exit evidence (per-crate, run this session):
```
$ diff <(tail -n +2 audits/surface/dsl-runtime.txt) /tmp/pubapi_dsl-runtime.txt | wc -l    # 0 after refresh
$ diff <(tail -n +2 audits/surface/ob-poc-control-plane.txt) /tmp/pubapi_ob-poc-control-plane.txt | wc -l   # 0 after refresh
$ diff <(tail -n +2 audits/surface/ob-poc-boundary.txt) /tmp/pubapi_ob-poc-boundary.txt | wc -l   # 0 after refresh
$ diff <(tail -n +2 audits/surface/ob-poc-types.txt) /tmp/pubapi_ob-poc-types.txt | wc -l   # 0 after refresh
```
All 4 baseline files now carry `HEAD=61540c0a5a03b25ed57a3e2a0117612e0f3fde6d` (this session's starting HEAD).

**STOP #1 — `ob-poc` itself (not refreshed).** `cargo +nightly public-api -p ob-poc --all-features` vs `audits/surface/ob-poc.txt` produces a **23,445-line diff** — orders of magnitude larger than the other 4, and not attributable to a single clean commit on inspection. It contains a genuinely new blanket impl, `impl<T> tower_http::follow_redirect::policy::PolicyExt for <almost every public type>` (2,006 occurrences), which reads as a `tower_http`/`tower` dependency-version bump surfacing a new blanket trait across the crate's entire public API — a real surface widening, not "a symbol that was already real and shipped." Mixed in are the same Send/Sync-ordering toolchain-noise lines seen workspace-wide, plus one new re-export (`pub use ob_poc::semtaxonomy_v2`, traced to the T11.1b/T11.2 agent-tier-extraction commits, 2026-07-12 — itself probably legitimate, but not separable from the rest of the diff without a proper per-hunk review). Per Slice 1's own STOP-condition ("a diff that ... includes a genuinely new/widened `pub` item nobody flagged ... this is a B1-class surface-widening question needing its own review, not a baseline refresh"): **stopped, not refreshed.** `ob-poc`'s baseline stays as-is; `[e5]`'s detail comment documents why.

**STOP #2 — `ob-poc-agent`'s measurement error is a code defect, not tooling.** Direct root-cause: `cargo +nightly public-api -p ob-poc-agent --all-features` fails because it must build `ob-poc-boundary` as a dependency, and `ob-poc-boundary::toctou_recheck::verify_pins_in_scope` (`toctou_recheck.rs:244-272`) uses `sqlx::PgConnection`/`sqlx::query`/`sqlx::Row` **unconditionally — no `#[cfg(feature = "database")]` gate** — so the build fails whenever the feature-resolution graph doesn't happen to activate `ob-poc-boundary`'s `database` feature. This reclassifies the item from "hygiene/tooling gap" to "code defect" per the slice's own STOP text. Exit evidence:
```
$ cd rust && cargo +nightly public-api -p ob-poc-agent --all-features 2>&1 | tail -15
error[E0432]: unresolved import `sqlx`
   --> crates/ob-poc-boundary/src/toctou_recheck.rs:248:9
    |
248 |     use sqlx::Row;
error: could not compile `ob-poc-boundary` (lib) due to 3 previous errors
Error: Failed to build rustdoc JSON (see stderr)
```
**Not fixed** — out of this grind slice's scope per its own STOP-condition text; needs a scoped source-level cfg-gating session.

**STOP #3 — the 4 `--no-default-features` failures are the same defect shape, not a Cargo.toml gap.** Direct build of each of the 4 crates confirms real `sqlx::` compile errors at `--no-default-features`, e.g.:
```
$ cd rust && cargo build -p ob-poc-derived-attributes --no-default-features 2>&1 | tail -8
error[E0433]: cannot find module or crate `sqlx` in this scope
   --> crates/ob-poc-derived-attributes/src/derived_attributes/repository.rs:638:19
error: could not compile `ob-poc-derived-attributes` (lib) due to 20 previous errors
```
Confirmed the same shape (20-46 `sqlx`-unresolved errors, no `#[cfg(feature = "database")]` on the offending module) for all 4: `ob-poc-derived-attributes` (20 errors), `ob-poc-entity-linking` (25), `ob-poc-taxonomy` (46), `ob-poc-trading-profile` (39). Each crate's `[features]` block declares `default = ["database"]` with `database = ["dep:sqlx"]`, but the sqlx-using modules (e.g. `derived_attributes/repository.rs`) are not gated behind `#[cfg(feature = "database")]` at all — so `--no-default-features` genuinely fails to compile. This is a source-code change (cfg-gating a whole module or its sqlx-using items), not a `Cargo.toml` feature-declaration edit. Per Slice 1's STOP text: **not fixed**, reclassified as a bug-fix item.

**`[e5]` disposition:** status stays `"fail"` (correctly — no fabrication). Detail comment rewritten (see `invariants-expected.toml` diff) to record: 4/5 named baselines refreshed and verified noise-only; `ob-poc` itself stopped (surface-widening, needs its own review); `ob-poc-agent` + the 4 `--no-default-features` crates reclassified from hygiene to bug-fix with the exact defect (unconditional `sqlx::` use, no `#[cfg(feature = "database")]`) named.

### Slice 2 — Runbook corrections (plan G0 item 2) — **DONE**

`docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` bumped v0.2 → v0.3 with a changelog entry. Edits:
- §2 readiness table's Path A row corrected: `step_executor_bridge.rs:553` calls `execute_verb_admitting_envelope` (commit `5a704f4e`), not the stale `:474` bare `execute_verb`; gate count corrected to **11/14** (G1-G9, G12 partial, G13), replacing the plan draft's miscounted "12/14" (GRADPLAN-D-002).
- §3's Path A graduation-order note and the "Reading this table" paragraph updated to match (both previously described the wiring as "not yet done").
- §4 Path A checklist box 1 flipped to `[x] DONE`, citing `5a704f4e`.
- §8 + §4 Path D: the incorrect "this is an ob-poc-only change" claim (and its supporting paragraph) deleted and replaced with the corrected two-repo framing — real producer is `bpmn-lite-engine::plan_walker::dispatch_callout` (a platform crate ob-poc does not own), per `EOP-RESEARCH-CONTROLPLANE-GRADUATION-001.md` Q-block C.
- New §1 clause: "production evidence" for a single-operator deployment (AD-3(a) recalibration) — window opens once at GM, filled deliberately by GW's exercise campaign, distinguished from fixtures by the deploy marker + session-id exclusion.

Exit evidence (run this session, exactly as specified):
```
$ grep -c "12/14 gates" docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md
0
$ grep -n "v0.3" docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md | head -1
2:### EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001 v0.3
$ grep -c "ob-poc-only change" docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md
0
```
All three pass exactly as the slicing doc specified. (Note: the phrase "ob-poc-only change" needed rewording twice during drafting — an early correction draft that *quoted* the old wrong sentence for clarity itself tripped the same grep; reworded to describe the error without repeating the literal banned string, so the exit evidence is genuinely satisfied, not gamed.)

No STOP-conditions fired (re-checked `step_executor_bridge.rs:553` and the gate count live this session — both match what was written).

### Slice 3 — Governance index (plan G0 item 3) — **DONE**

Created `docs/todo/control-plane/INDEX.md` — 6 sections (Vision & Scope, MCA, Plan 001, "002 track" T11.x design docs, the graduation program, invariant-enforcement machinery), each entry with path/status/one-line purpose.

Exit evidence:
```
$ test -f docs/todo/control-plane/INDEX.md && echo EXISTS
EXISTS
$ ls docs/todo/control-plane/*.md docs/research/control-plane-*.md docs/architecture/EOP-VS-CONTROLPLANE-001*.md 2>/dev/null | wc -l
19
```
Manual cross-check (per the slicing doc, not automatable): every one of the 18 non-INDEX files that command lists, plus the MCA-001/002 docs, `workspace-hygiene-001.md`, the invariant-promotion evidence `.txt`, `invariants-expected.toml`, `scripts/check-invariants.sh`, and `.github/workflows/invariants.yml` (none matched by that `ls` glob but named in the plan's own item-3 text) — all verified present as INDEX.md entries via `grep -q "$f" INDEX.md` for each path, all `OK`. No STOP-condition fired (no artifact's live/historical status was ambiguous).

### Slice 4 — Ledger provability backfill, bounded (plan G0 item 4) — **DONE**

Found the commit that deleted `OBPOC_ALLOW_RAW_EXECUTE` (referenced in C-001's row as "Slice 3.1, 2026-04-22" but previously uncited): `1a194d40` ("v1.2 Catalogue Platform Refinement: Tranche 1 + 2 + pub API surface cleanup (#2)", merged 2026-04-26). Confirmed via `git show 1a194d40 -- rust/src/api/agent_routes.rs`, which shows both the flag/method deletion and the `// F16 fix (Slice 3.1, 2026-04-22)` comment added at the call site. Added the citation to C-001's ledger row. No other rows touched (per the slice's own strict scope — the other 41 open rows are genuinely open, not backfill candidates).

Exit evidence:
```
$ bash scripts/check-invariants.sh e1
  Provably CLOSED: 4
  Not provably closed: 41
  Non-closed/unproven IDs: C-002 C-003 ... (C-001 no longer present)
  E1: DOES NOT HOLD
```
Matches the slice's expected outcome exactly ("Provably CLOSED: 4 (up from 3), with C-001 no longer appearing in the Non-closed/unproven IDs list"). No STOP-condition fired (the hash was found cleanly, no fabrication needed).

---

## Files changed this session

**Tracked by git (`git status --short` shows these):**
- `audits/surface/dsl-runtime.txt` — Slice 1 baseline refresh
- `audits/surface/ob-poc-boundary.txt` — Slice 1 baseline refresh
- `audits/surface/ob-poc-control-plane.txt` — Slice 1 baseline refresh
- `audits/surface/ob-poc-types.txt` — Slice 1 baseline refresh
- `docs/research/control-plane-ownership-ledger.md` — Slice 4 (C-001 hash citation)
- `docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` — Slice 2 (v0.2 → v0.3)
- `invariants-expected.toml` — `[e1]` and `[e5]` detail-comment updates (status unchanged on both: `fail`/`fail`)

**NOT tracked by git — see "Environmental finding" below:**
- `docs/todo/control-plane/INDEX.md` — new file, Slice 3
- `docs/todo/control-plane/EOP-PIR-CONTROLPLANE-GRADPLAN-001.md` — Part 1 addendum appended (D-010)
- `docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-GRADPLAN-G0-IMPL-001.md` — this file

**Pre-existing modifications at session start, NOT touched by this session** (per the git status snapshot given at session open — listed here only so the "files changed" list isn't misread as mine): `observatory-wasm/Cargo.lock`, `rust/Cargo.lock`, `rust/cbu_mismatches.json`, `rust/mismatches.json`, `rust/reports/phase0_confusion_matrix.json`, `rust/reports/step0_trial_evaluation.json`.

### Environmental finding (flag for architect)

`.git/info/exclude` (this local checkout only, not shared/committed) contains a blanket `*.md` exclude pattern with only a short allowlist (`AGENTS.md`, `CLAUDE.md`, `ARCHITECTURE_FOR_LLM_REVIEW.md`, `docs/architecture/*.md`, `docs/semos_arhitecture.md`, `docs/backlog/*.md`). `docs/todo/control-plane/*.md` is **not** on that allowlist. Files already tracked in git (e.g. the runbook, the T11.x design docs) are unaffected — gitignore/exclude rules don't hide modifications to already-tracked files — but several load-bearing governance docs this whole program has been producing are **untracked**: `EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.1.md`, `EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.3.md`, `EOP-PIR-CONTROLPLANE-GRADPLAN-001.md`, `EOP-RESEARCH-CONTROLPLANE-GRADUATION-001.md`, `EOP-IMPL-CONTROLPLANE-GRADUATION-G0-001.md`, and now this session's new `INDEX.md`. Confirmed via `git ls-files docs/todo/control-plane/EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.3.md` (etc.) returning nothing. A plain `git add .`/`git add -A` will silently skip all of them; they need `git add -f` (or the exclude rule removed/amended with a `!docs/todo/control-plane/*.md` negation) before any commit. This may partly explain G0 item 3's own stated motivation ("two governing docs were invisible to grounding phases this week") — if a prior session's `git add .` silently dropped a doc, later sessions cloning fresh would never see it. **Flagging, not fixing** — editing `.git/info/exclude` or force-adding files is a repo-hygiene/commit decision outside this session's "no commit" mandate.

---

## Expectation-flip recommendations for `invariants-expected.toml` (recommend only — not applied)

None of this session's slices genuinely close an E-gate to `pass`. Both edits I made to `invariants-expected.toml` this session are **detail-comment-only**, with `status` left at `"fail"` on both `[e1]` and `[e5]`, matching standing rule 1 (an invariant going green without its expectation flipped in the same diff is a CI failure by design; conversely, a detail comment may be updated by any tranche that earns the update, without touching status). No flip recommendation is being smuggled into the file itself — recording here instead per the mission's instruction:

- **No recommended flip.** `[e1]` remains correctly `fail` (41/45 rows still genuinely open — 1 row's citation backfill doesn't move the needle on the gate's pass/fail line). `[e3]` and `[e4]` are untouched by any G0 work (out of scope). `[e2]` is untouched (G1/G4 work, out of scope). `[e5]` **cannot** be flipped to `pass` yet — 1 of 5 named crates (`ob-poc`) is still stale (STOP), plus `ob-poc-agent` and the 4 `--no-default-features` crates need actual source fixes (STOPs, reclassified as bug-fix scope) before the gate's own script would report `HOLDS`. If a future session lands those fixes, `[e5]`'s flip to `pass` is earned in that diff, not this one.

---

## Confirmation

No `git commit` was run. No merge, deploy, or `git push` was performed. All changes listed above are on-disk, uncommitted, for the user/architect's own review and staging.
