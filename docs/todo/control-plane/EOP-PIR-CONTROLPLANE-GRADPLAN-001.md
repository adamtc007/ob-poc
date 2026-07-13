# EOP-PIR-CONTROLPLANE-GRADPLAN-001 — Adversarial review of the graduation plan

### Reviews: `docs/todo/control-plane/EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.1.md`
### Date: 2026-07-13
### Status: READ-ONLY review. The plan under review was not edited. No `git commit` was run.

---

## Phase 0 — grounding (docs read in full this session)

| Doc | Lines |
|---|---|
| `docs/todo/control-plane/EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.1.md` (plan under review) | 498 |
| `docs/todo/control-plane/EOP-RESEARCH-CONTROLPLANE-GRADUATION-001.md` | 270 |
| `docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` (v0.2) | 279 |
| `scripts/check-invariants.sh` | 538 |
| `invariants-expected.toml` | 77 |
| `.github/workflows/invariants.yml` | 52 |
| `docs/research/control-plane-ownership-ledger.md` (through T11.2 Part A) | 782 (full read: 1-50, 380-479, 592-669, 750-782, plus targeted `grep`/section reads for C-032/C-001/T9.7) |
| `docs/todo/workspace-hygiene-001.md` | 62 |
| `docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-INVARIANT-PROMOTION-001.md` | 257 |

`docs/todo/control-plane/INDEX.md` — **does not exist** (confirmed via `ls docs/todo/control-plane/`). This is a status fact, not a defect: it is exactly G0 item 3's open work, and the plan already treats it as open. Total governing-artifact tally: 2,815 lines read/grepped this session, plus live command output (`check-invariants.sh e1/e4/e5`, `rg`/`grep` code checks) captured below.

---

## Phase 1-3 findings — defect register

### GRADPLAN-D-001 — BLOCKER-adjacent, downgraded to MAJOR: E3's "per-gate source map" does not exist; G5's "NotApplicable" is a new enum variant, not a query tweak

**Claim:** Plan G1 item 2: *"E3 probe updated if AD-1(a)/(c) moves G10 off the input stack (probe already distinguishes infrastructure vs invariant failure; extend its per-gate source map)."* G2's exit gate: *"E3 probe shows G11, G12, G14 with substantive samples (G14 via its post-dispatch source, which the probe's per-gate source map must reflect)."* G5 item 2: *"`NotApplicable` recorded as a first-class outcome, not `NotEvaluated`, so the E3 probe can tell them apart."*

**Ground truth (checked this session):** `rust/src/agent/control_plane_metrics.rs:95-124` (`gate_outcome_counts`) groups strictly by `(gate, outcome_kind)` with `outcome_kind ∈ {Success, Failure, NotEvaluated, NotImplemented, Unrecognised}` — there is no `source`/`path` column anywhere in the query, the `control_plane_shadow_decisions` schema, or the Rust types (`GateOutcomeCount { gate, outcome_kind, count }`). The *only* per-run source distinction that exists anywhere in the probe is the two-way `E3_INFRASTRUCTURE_FAILURE` vs `E3_INVARIANT_FAILURE` panic-marker split in the `#[ignore]`-gated test harness (`control_plane_metrics.rs:524-586`) — a whole-probe-run distinction ("could the harness even reach the DB"), not a per-gate provenance tag ("did this gate's sample come from Path A shadow eval vs a post-dispatch capture vs a not-applicable-by-construction path"). The plan's own phrase "probe already distinguishes infrastructure vs invariant failure" is TRUE; the adjacent claim that this is (or extends to) "a per-gate source map" is not — no such map exists in any form.

Separately: `GateResult` (`crates/ob-poc-control-plane/src/gate.rs:69-77`) has exactly four variants — `Success`, `Failure(String)`, `NotEvaluated { blocked_by }`, `NotImplemented` — confirmed via direct read. `NotApplicable` does not exist anywhere in the workspace (`rg -n "NotApplicable" crates/ob-poc-control-plane/src/` → zero hits). Adding it is not a query-layer tweak; it is a new variant on the crate's most foundational, most-depended-on enum, in a codebase that enforces exhaustive matching with no wildcard arm (the same discipline `check-invariants.sh`'s own E3 compile-time half tests for `GateId`). Every `match` over `GateResult` in both `ob-poc-control-plane` and the `ob-poc` application layer would need a new arm.

**Consequence if unfixed:** G1 item 2, G2's exit gate, and G5 items 2 and 4 all budget this as incremental extension of existing machinery ("extend," "reflect," "so the probe can tell them apart" — language implying the distinguishing capability already exists in some form). An implementer picking up G2 or G5 against the plan's own text would discover mid-session that the exit gate depends on a new enum variant threaded through an exhaustively-matched core type plus a new schema column/derivation for per-gate provenance — exactly the kind of scope invention Phase 3's grind-suitability bar exists to catch, except here it hits at the *exit gate*, not the work items, meaning a session could do all of G2's listed items 1-2 and still fail to close per the letter of the exit gate.

**Severity: MAJOR.** Recommended amendment: G2 and G5 should each carry an explicit sub-item ("extend `GateResult` with a `NotApplicable` variant; extend `gate_outcome_counts`'s schema/query with a per-gate source/path dimension") sized and reviewed as its own piece of work, not folded silently into "recount" language.

---

### GRADPLAN-D-002 — MINOR: plan's "12/14 gates have real shadow inputs" miscounts the research's own table

**Claim:** G0 item 2: *"12/14 gates have real shadow inputs per R:§A1's table."*

**Ground truth:** R:§A1's table (research doc lines 46-63) marks exactly 11 gates "Real" or "Real (partial)": G1, G2, G3, G4, G5, G6, G7, G8, G9, G12 (partial), G13. G10 and G11 are "Stub"; G14 is "Not applicable to this call site at all." 11, not 12. (`invariants-expected.toml`'s own `[e3]` count of "10/14... G1-G9, G13" is a different, narrower claim — *live samples*, not *wiring presence* — and is internally consistent with the metrics-undercount bug G2 item 1 targets; it does not rescue the plan's "12" figure, which purports to cite R:§A1's wiring table directly.)

**Consequence if unfixed:** G0 item 2 instructs writing this count into the runbook as v0.3's corrected, authoritative readiness table. An off-by-one lands in a governing doc that future sessions cite without re-deriving.

**Severity: MINOR.** Amendment: change "12/14" to "11/14 (G1-G9, G12 partial, G13)" in G0 item 2's instruction text.

---

### GRADPLAN-D-003 — MAJOR: G0's exit gate smuggles an unspecified predicate ("first non-fixture row")

**Claim:** G0 exit gate: *"deployment confirmed writing real rows to `control_plane_shadow_decisions` (first non-fixture row is the evidence)."*

**Ground truth:** Per R:§A3/A5 (independently re-confirmed by this session's own reading, not re-queried live), every existing row in `control_plane_shadow_decisions` is traceable to a specific unit/integration test fixture by construction (`test.sealable-rate-<uuid>` naming, `nonexistent.verb` for a deliberate lookup-failure test, etc.) — there is no column, tag, or naming convention that structurally distinguishes a "fixture" row from a "real" row for rows that use a *real* verb FQN like `cbu.confirm` under real production conditions post-deploy. "First non-fixture row" is answerable today only by manual/prose inspection (a human recognizing "this timestamp is after deploy and this session_id isn't one of our test harness's" — itself an inference, not a query), not by a command or CI status, contradicting Phase 1 rule 2's own bar ("MACHINE-CHECKABLE... a command, a CI status, a test name").

**Mitigating factor:** the tranche's own text says "Blind review required before merge (item 5 is the one irreversible step)," which functions as an architect-sign-off exception for the *merge decision itself* — but the exit-gate clause under review here is not the merge decision, it's the "window opened" claim, which the plan elsewhere (§1, §5 risk register) treats as load-bearing for when the counted window starts. A fuzzy predicate at that specific joint is exactly the smuggled-prose-judgement shape Phase 1 rule 2 flags as a MAJOR defect by default.

**Severity: MAJOR.** Recommended amendment: name a concrete post-deploy predicate — e.g. a deploy-marker timestamp recorded at merge time, with the check being `SELECT count(*) FROM control_plane_shadow_decisions WHERE created_at > :deploy_marker AND session_id NOT IN (:known_test_harness_session_ids)`, or (simpler) a deploy-time DB marker row inserted once, with "first row after the marker" as the mechanical definition of "non-fixture."

---

### GRADPLAN-D-004 — MINOR: §3's dependency graph over-scopes AD-1's block to "G1 start" where §2's prose only blocks "G1 exit"

**Claim:** §2 states AD-1 "blocks G1 exit, G5" (title line). §3's ASCII dependency graph shows `├─► G1 [AD-1] ─┐`, i.e. the bracket annotation sits directly on G1 with no start/exit qualifier.

**Ground truth (internal-consistency check, no code needed):** of G1's four work items, only item 2 ("Resolve G10 per AD-1's ratified option...") textually depends on AD-1's resolution. Items 1 (envelope threading), 3 (live-DB test), and 4 (non-eligible-decision handling) do not reference AD-1 and are described as within-the-existing-types design work assignable to Fable/Opus independent of which AD-1 option is chosen. The graph's undifferentiated `[AD-1]` tag on the whole G1 node reads as "G1 cannot start," contradicting §2's own more precise "blocks G1 exit."

**Consequence if unfixed:** an implementer reading only §3's graph (the plan's own quick-reference summary) could wait on AD-1 ratification before starting G1 item 1's threading design at all, when the plan's prose intent (per §2) is that only item 2, and hence the tranche's *exit*, is gated.

**Severity: MINOR.** Amendment: annotate the graph node as `G1 (items 1,3,4 startable now) [AD-1 blocks item 2 / exit]`, or add a one-line footnote under the graph clarifying the exit-only scope of the bracket tags, consistent with how AD-2's `G3 = AD-2` / `G4 [needs G3]` notation already distinguishes start-blocking from a upstream design tranche.

---

### GRADPLAN-D-005 — NOTE: §4's E3 completion-mapping row is silently conditioned on AD-1(a)

**Claim:** §4 completion mapping: *"E3 | G1 (G10) + G2 (G11/G12/G14) + G5 (matrix + B/C/D) | ..."* — stated unconditionally.

**Ground truth:** G1's own per-tranche E-movement note says *"E3 (G10 gains real samples under AD-1(a))"* — explicitly conditional on option (a) being ratified. Under AD-1(b) (prior-decision-presence framing) G10 gains a *different* kind of sample (via a new `EvaluationContext.execution_envelope` field and lookup source, per AD-1's own §2 text); under AD-1(c) (retire G10 as an input gate), G10 leaves the E3 matrix entirely and E3's own gate count changes from 14 to 13+1+1, requiring an amendment to `GateId`/the E3 compile-time exhaustiveness test, not merely new samples. §4's table doesn't carry any of this conditionality, presenting "G1 (G10)" as a flat, option-independent fact.

**Consequence if unfixed:** low — the plan already recommends (a) and is internally consistent under its own recommendation. If the architect picks (b) or (c) instead, §4's E3 row silently goes stale and a future session might not re-derive the amendment G1/E3 would then need.

**Severity: NOTE.** Amendment: footnote §4's E3 row with "(shape depends on AD-1's ratified option; table assumes (a), the plan's recommendation)."

---

### GRADPLAN-D-006 — MAJOR (Phase 3 feasibility, per the task's explicit ask): G1 item 1's correlation carrier is under-scoped as "a design detail for the session"

**Claim:** G1 item 1: *"Establish the correlation carrier (sequencer entry state is the natural home; design detail for the session, within the existing types — no new crate edges expected...)."* Session tier for G1: *"the threading design (item 1) is Fable/Opus work with architect review; items 3-4 are grind-suitable."*

**Ground truth:** the correlation problem as described genuinely needs answers to: (a) where does the sealed `EnvelopeHandle` live between `decision::evaluate()`'s seal (inside `phase5_runtime_recheck`, confirmed this session at `sequencer.rs:8015` calling `ob_poc_control_plane::decision::evaluate_with_report`) and the later `execute_verb_admitting_envelope` call at `step_executor_bridge.rs:553` (confirmed this session, still passing literal `None`) — these are two different functions, potentially separated by however long the runbook step scheduler takes to reach that step; (b) what is the handle's lifetime/expiry behavior if the gap between seal and consume exceeds the envelope's validity window (5 minutes per T10.1's convention, confirmed via ledger read); (c) retry/replay: if a step is retried, does it reuse the same sealed envelope (single-use — will the second attempt hit "already consumed") or does retry require re-sealing; (d) multi-step runbooks: does each step in a runbook get its own seal-then-consume pair, or is there a plan-level envelope. None of these four are named in the plan's one-sentence treatment, and the ledger's own T10.1 entry independently flags a directly analogous unresolved item — "Owed convergence, registered MIGRATION-PENDING... `evaluate_shadow()` and `evaluate()` are now two parallel pub entry points into overlapping logic... Target for convergence: T10.2's admission-scope wrapper" — i.e. the ledger itself already identifies that the seal/consume split G1 item 1 must bridge is a known, previously-deferred structural gap, not a fresh small wiring task.

**Consequence if unfixed:** this is exactly the shape the runbook's own §8 already names for the *sibling* problem on Path B/C ("Path B/C admission-hook design... not yet designed, larger than a config flip; needs its own short plan"). G1 item 1 is the Path-A-analogue of that same class of problem, and the plan does not give it the same "needs its own short plan" treatment — it schedules it as inline session work with architect review, not as a design doc with its own review gate.

**Severity: MAJOR.** Recommended amendment: split G1 item 1 into its own short design doc (mirroring G3's treatment of AD-2, or T10.1's own "Addendum C" pattern), addressing at minimum the four sub-questions above, before any G1 grind work (items 3-4) starts — items 3-4 as currently scoped assume item 1's design exists and is stable underneath them.

---

### GRADPLAN-D-007 — MINOR/NOTE: E1's C-001 provability gap is real and current (not stale), confirms G0 item 4's premise

**Claim:** G0 item 4: *"for rows whose closing work has already landed but which fail E1's provability bar (claimed-CLOSED without hash/resolving symbol — C-001 is the known instance), add the citations."*

**Ground truth (live check, this session):** `bash scripts/check-invariants.sh e1` → `Provably CLOSED: 3` (C-022, C-030, C-037), `Not provably closed: 42`, with `C-001(claimed-closed,unproven:hash=0,symbol=1,resolves=1)` explicitly listed — i.e. C-001 has a resolving symbol but no commit-hash citation, exactly matching `invariants-expected.toml`'s own `[e1]` comment. This is a **status-fact confirmation, not a defect** — G0 item 4's premise holds exactly as stated. Recorded here per the rules of evidence (every plan claim checked against ground truth this session), not as a criticism.

**Severity: NOTE** (no action needed; included for completeness of the register).

---

### GRADPLAN-D-008 — NOTE: bpmn-lite local checkout confirmed unchanged from the research's snapshot, no drift found

**Claim (Phase 3's explicit ask):** verify both G6a carrier options against current bpmn-lite checkout state (the `[patch]` redirect and tag pin per R:§C1).

**Ground truth (live check, this session):** `~/.cargo/config.toml`'s `[patch."https://github.com/adamtc007/bpmn-lite"]` block still redirects to `~/dev/bpmn-lite/`; the local checkout is still at commit `619370d4` (2026-06-17), matching R:§C1 exactly. `rust/Cargo.toml:404` still pins `tag = "v0.2.0"`. The local checkout has minor uncommitted noise (`Cargo.lock`, `docker-compose.yml`, `scripts/test_ui_snapshot.json`, an untracked `report.txt`) unrelated to `plan_walker.rs`/`dispatch_callout` — no drift affecting either C2 option (a)/(b) found.

**Severity: NOTE.** No amendment needed; recorded as a confirmed-clean finding per the task's explicit instruction to check for drift.

---

### GRADPLAN-D-009 — NOTE: G4/G5/G6 as currently written do not touch Path A's shadow call site (window-discipline check passes)

**Claim (Phase 1.5's explicit ask):** verify nothing in G4/G5/G6 as drafted touches `phase5_runtime_recheck`, `build_evaluation_context`, `evaluate_shadow`, or divergence classification.

**Ground truth (live check, this session):** `rg -ln "phase5_runtime_recheck|build_evaluation_context|evaluate_shadow" rust/src/ --type rust` → hits only in `agent/control_plane_shadow.rs`, `sequencer.rs`, plus three files (`sem_os_context_envelope.rs`, `control_plane_envelope_store.rs`, `control_plane_metrics.rs`) that reference the terms in **doc comments only**, confirmed by direct inspection (`control_plane_envelope_store.rs:267` is a `///` comment naming `phase5_runtime_recheck` as context, not a call site). No actual call site outside `sequencer.rs`/`control_plane_shadow.rs` exists today. G4's scope (dsl_v2::executor seam) and G5's scope (B/C/D evaluation, explicitly new call sites) as textually described do not touch these symbols. This is a genuine internal-consistency pass — recorded per the plan's own risk-register item ("a session that finds it must [touch Path A's call site] is a stop-and-review event, not a judgement call").

**Severity: NOTE.** No amendment needed.

---

## Verdict

**RATIFY-WITH-AMENDMENTS.**

No BLOCKER exists — nothing found makes the plan structurally unsound or requires re-authoring before any tranche can start. GRADPLAN-D-001 and GRADPLAN-D-003 are the two MAJOR defects that should be amended before G2/G5 (D-001) and before G0's merge-step is treated as "window opened" (D-003) are executed against the plan's current text; GRADPLAN-D-006 should be amended (split into its own design doc) before G1 grind work starts. None of the three block G0 items 1-4 or G3 (architect design work), which can proceed under the plan as written.

**Amendments (apply before the corresponding tranche executes; do not apply to the plan doc in this session per the task's read-only constraint):**

1. **GRADPLAN-D-001 (MAJOR):** Add an explicit sub-item to G2 and to G5 scoping the `GateResult::NotApplicable` variant addition (with its exhaustive-match fallout across `ob-poc-control-plane` and `ob-poc`) and the `gate_outcome_counts`/schema per-gate-source extension as their own reviewed piece of work, not shorthand ("extend," "reflect") assuming existing machinery.
2. **GRADPLAN-D-002 (MINOR):** Correct G0 item 2's runbook-correction instruction from "12/14 gates have real shadow inputs" to "11/14 (G1-G9, G12 partial, G13)."
3. **GRADPLAN-D-003 (MAJOR):** Replace G0's exit-gate clause "first non-fixture row is the evidence" with a concrete, queryable predicate (a deploy-time marker row/timestamp and an explicit `WHERE` clause excluding known test-harness session ids).
4. **GRADPLAN-D-004 (MINOR):** Annotate §3's dependency graph to show AD-1 blocking only G1 item 2/exit, not G1's start, consistent with §2's own text.
5. **GRADPLAN-D-005 (NOTE):** Footnote §4's E3 completion-mapping row as conditioned on AD-1(a) being the ratified option.
6. **GRADPLAN-D-006 (MAJOR):** Split G1 item 1 (the seal→consume correlation carrier) into its own short design doc, addressing envelope lifetime across the seal/consume gap, expiry-vs-scheduling-delay behavior, retry/replay semantics, and multi-step-runbook sealing — mirroring the treatment G3/AD-2 and the runbook's own §8 already give the structurally analogous Path B/C admission-hook design gap. Items 3-4 of G1 should not start grinding until this lands.

---

## Second opinions on the architect decisions (AD-1, AD-2, AD-3)

Per the task's framing: these are second opinions for the architect, not decisions made here.

- **AD-1 (recommendation: (a), G10 grades envelope validity at consume time).** Phase 2 ground truth is **neutral-to-weakening**. The mechanism this option leans on is real (`decision::evaluate()` does seal a real `ExecutionEnvelope` at the Path A shadow call site today, confirmed via `decision.rs:313`/`sequencer.rs:8015`), which supports (a)'s premise that "the mechanism already proves this in `t4_1` tests." But GRADPLAN-D-001 and GRADPLAN-D-006 both land squarely on option (a)'s consequences: (a) requires new shadow-recording machinery at the *consume* seam (a second call site distinct from today's single pre-dispatch shadow insert) to have anything to grade, and G1 item 2's one-line treatment of "wire its real evaluation at the seam" does not currently account for that scope. This doesn't argue against (a) — it argues the plan under-costs (a)'s adoption.
- **AD-2 (recommendation: (b), add a path dimension to `EnforcedVerbs`).** Phase 2 ground truth **strengthens** the draft's recommendation. `EnforcedVerbs::is_enforced` (`control_plane_envelope_store.rs:31-45`, confirmed this session) is exactly the flat `HashSet<String>::contains` the research describes — no path-scoping exists today, and the E2 structural gate (`check-invariants.sh gate_e2`) already treats "admitting call present" per-path independently for A/B/C/D, meaning the machinery to reason about paths distinctly already exists one layer up (the gate script) even though `EnforcedVerbs` itself doesn't carry a path dimension. Adding the dimension (b) is consistent with machinery the codebase already reasons about elsewhere; staying at (a) global-per-verb would create an asymmetry between how the invariant gate reasons about the four paths and how the actual enforcement primitive does.
- **AD-3 (recommendation: (b), ship now / burn-in).** Phase 2 ground truth **strengthens** the draft's recommendation. The research's A3/A5 findings (independently re-confirmed via this session's reading of the T11.F.1 addendum, ledger lines 610-614: *"zero real production traffic... the ≥500-shadow-evaluation graduation criterion requires elapsed production time after merge/deploy, which has not happened"*) make the "calendar time, not code" framing hold up — nothing else in the plan can be evidence-grounded (A5's candidate-verb selection, §7's triage practice) until real traffic exists, and G2's own exit gate already correctly marks itself as "the last Path-A shadow-semantics change" (confirmed via Phase 1.5/GRADPLAN-D-009's clean pass — nothing in G4/G5/G6 as drafted touches Path A's call site), so AD-3(b)'s "the window that counts is the one after G2" claim holds structurally today.

---

## Confirmation

Two output files this session: this file, and (Part 2, gated on this verdict) `docs/todo/control-plane/EOP-IMPL-CONTROLPLANE-GRADUATION-G0-001.md`. The plan under review was not modified. No `git commit` was run.

---

## v0.3 re-validation addendum (2026-07-13, follow-up session)

Scope: lighter re-validation of `EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.3.md` (post-amendment, post-AD-3-resolution), not a full re-review. Read v0.3 in full including its own v0.1→v0.2 and v0.2→v0.3 changelogs; cross-checked the six amendments below against v0.3's actual text; live-checked the fastest ground-truth spot-checks from Phase 2 of the original review.

### Amendment cross-check (GRADPLAN-D-001..006)

1. **D-001 (MAJOR — GateResult::NotApplicable / per-gate source map are new work, not query tweaks).** PIR's proposed amendment: *"Add an explicit sub-item to G2 and to G5 scoping the `GateResult::NotApplicable` variant addition ... and the `gate_outcome_counts`/schema per-gate-source extension as their own reviewed piece of work."* v0.3 G2 item 4: *"E3 probe capability: per-gate source/provenance dimension (GRADPLAN-D-001 — this does not exist today; `gate_outcome_counts` groups strictly by (gate, outcome_kind) with no source column anywhere in query, schema, or types). Scope: a per-gate provenance dimension ... Sized and reviewed as its own piece of work."* v0.3 G5 item 1: *"`GateResult::NotApplicable` variant (GRADPLAN-D-001 — this is a new variant on the crate's most-depended-on, exhaustively-matched core enum, NOT a query tweak): add the variant; sweep every `match` over `GateResult`..."* — **APPLIED**, verbatim-traceable to the PIR's own language, in both tranches as prescribed.
2. **D-002 (MINOR — 12/14 → 11/14).** PIR: *"change '12/14' to '11/14 (G1-G9, G12 partial, G13)' in G0 item 2's instruction text."* v0.3 G0 item 2: *"11/14 gates — G1–G9, G12 (partial), G13 — have real shadow inputs..."* — **APPLIED** exactly.
3. **D-003 (MAJOR — unspecified "first non-fixture row" predicate).** PIR: *"Replace G0's exit-gate clause 'first non-fixture row is the evidence' with a concrete, queryable predicate (a deploy-time marker row/timestamp and an explicit WHERE clause excluding known test-harness session ids)."* v0.3 relocates the whole merge step to tranche GM (per AD-3(a)) and GM's Work section gives: `SELECT count(*) FROM "ob-poc".control_plane_shadow_decisions WHERE created_at > :deploy_marker_ts AND session_id NOT IN (<known test-harness session ids, enumerated at marker time>)`. G0's own exit gate no longer mentions "first non-fixture row" at all (checked — absent from v0.3's G0 section). — **APPLIED**, though relocated rather than amended in place (correctly so, per AD-3(a)'s restructuring, and the plan's own changelog says as much: *"deploy-marker predicate (D-003) moves with it"*). **See D-010 below** — the predicate as carried forward is not quite correct against the live schema.
4. **D-004 (MINOR — graph over-scopes AD-1 to all of G1).** PIR: *"annotate the graph node as `G1 (items 1,3,4 startable now) [AD-1 blocks item 2 / exit]`."* v0.3 §3 graph: `├─► G1 [AD-1: blocks item 2 / exit only] ─┐`, plus a prose footnote immediately below the graph: *"Bracket tags mark what the AD actually blocks (GRADPLAN-D-004): G1's design doc (item 1) and items 3–4's grind prep can start before AD-1 lands; only item 2 and the tranche exit wait on it."* — **APPLIED** (equivalent form, arguably clearer than the PIR's own suggested wording).
5. **D-005 (NOTE — §4 E3 row silently conditioned on AD-1(a)).** PIR: *"Footnote §4's E3 row as conditioned on AD-1(a) being the ratified option."* v0.3 §4 table has a `†` on the E3 row's "G1 (G10)" cell, resolving to: *"† Shape assumes AD-1(a), the plan's recommendation (GRADPLAN-D-005). Under AD-1(b), G10's samples come from a new `EvaluationContext` field + lookup source instead; under AD-1(c), G10 leaves the input matrix entirely..."* — **APPLIED**, and expanded beyond the PIR's minimum ask (documents all three AD-1 branches' consequences, not just flagging the conditionality).
6. **D-006 (MAJOR — G1 item 1 correlation carrier under-scoped as inline session work).** PIR: *"Split G1 item 1 ... into its own short design doc, addressing envelope lifetime across the seal/consume gap, expiry-vs-scheduling-delay behavior, retry/replay semantics, and multi-step-runbook sealing ... Items 3-4 of G1 should not start grinding until this lands."* v0.3 G1 item 1 is now titled *"Seal→consume design doc (`EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001`)"* and enumerates the same four sub-questions verbatim (handle lifetime location, expiry/validity-window behaviour, retry/replay, multi-step runbooks) plus the T10.1 convergence note; v0.3's G1 session-tier line states *"items 3–4 do not start grinding until item 1's doc is ratified (GRADPLAN-D-006)."* — **APPLIED** in full, including the explicit sequencing gate on items 3-4.

**Verdict on the cross-check: all six amendments APPLIED, none PARTIALLY-APPLIED or NOT-APPLIED.** No amendment text was diluted or dropped during the AD-3 restructuring.

### AD-3 restructuring consistency

G0's own text confirms the merge step is gone: *"(Item 5, merge + deploy, moved to tranche GM under AD-3(a))"* — G0's exit gate (`[e5]=pass`, runbook v0.3 committed, INDEX.md exists, `[e1]` detail recounted) contains no merge/deploy clause. GM is a new standalone tranche (§3) whose hard preconditions are G0+G1+G2 exit gates plus architect blind review — consistent with AD-3(a)'s "hold the merge" ratification. Nothing in G0/G1/G2's restructured text implicitly assumes the old window-opens-at-G0 model: G0 item 2's runbook-correction instruction now explicitly adds *"a new §1 clause recalibrating 'production evidence' for a single-operator deployment"* rather than referencing G0 as the window's start. Standing rule 3 (window discipline) and the §5 risk register's *"Window resets"* item both correctly frame the window as starting at GM, not G0. **Consistent — no residual old-model assumption found.**

### GM/GW spot-check (same defect classes PIR Phase 1 checked)

- **GM exit gate machine-checkability:** *"deployed; marker recorded; the predicate above is runnable (returning 0 is fine — GW fills it)."* The predicate itself is a runnable SQL query (machine-checkable); "deployed" and "marker recorded" are ops actions confirmed by the deploy record itself, not prose judgement — this is the same class of architect-blind-review exception the original PIR accepted for the (now-relocated) merge decision, appropriately carried to GM. No new smuggled-prose-judgement defect found here.
- **GW exit gate machine-checkability:** *"`shadow_divergence_stats` meets the §1 criterion on post-marker rows; candidate verb named in the ledger with its distribution evidence."* §1's criterion (`total_decisions >= 500 AND diverged == 0`, or fully triaged) is the same pre-existing, already-machine-checkable query the runbook and v0.1 both relied on — no new ambiguity introduced.
- **Window-discipline check on GM/GW themselves:** GM's Work section is merge + deploy + record-marker only — no code touching Path A's shadow call site. GW's Work section (items 1-4: define exercise set, run campaign, track via `shadow_divergence_stats`, pick candidate verb) is operational/data-generation, not code — it does not touch `phase5_runtime_recheck`/`build_evaluation_context`/`evaluate_shadow`/divergence classification either. Both pass the same test GRADPLAN-D-009 applied to G4/G5/G6.

### AD-1 / AD-2 unchanged

`grep`-diffed against v0.1: AD-1 is still tagged `[blocks G1 exit, G5]` and AD-2 still `[blocks G4 start; constrains G7]`, byte-identical to v0.1's tags. Their body text (options (a)/(b)/(c) for AD-1, (a)/(b) for AD-2, and the plan's recommendations) is unchanged between v0.1 and v0.3 — confirmed by direct read of v0.3 §2, which is prose-identical to v0.1's §2 apart from AD-3 now carrying a "RESOLVED" status block. **Unchanged, consistent with what this PIR reviewed.**

### Live ground-truth re-spot-checks (fast, no full rebuild)

- **`GateResult` variant count** (`rust/crates/ob-poc-control-plane/src/gate.rs:69-77`, re-read this session): still exactly 4 variants — `Success`, `Failure(String)`, `NotEvaluated { blocked_by }`, `NotImplemented`. `rg -n "NotApplicable" rust/crates/ob-poc-control-plane/src/ rust/src/` still finds zero hits in the control-plane/gate context (all hits are unrelated `ConditionEvaluation`/`DiagnosticCode` enums in other domains). `git log --oneline -5 -- rust/crates/ob-poc-control-plane/src/gate.rs` → most recent touch is `7c7397a4` (T9.7, pre-dates this whole plan/review chain). **No drift.**
- **`gate_outcome_counts` grouping** (`rust/src/agent/control_plane_metrics.rs:95-125`, re-read this session): still `GROUP BY kv.key, outcome_kind` with the same 4-way `CASE` (`Success`/`Failure`/`NotEvaluated`/`NotImplemented`, else `Unrecognised`) and no source/path/provenance column anywhere in the query or `GateOutcomeCount { gate, outcome_kind, count }`. `git log --oneline -5` on this file shows `61540c0a` (invariant-promotion review remediation) as most recent — that commit is already cited in v0.3's own Basis list (§"Basis", `check-invariants.sh` commit range `8a9b87e6`…`61540c0a`), so it predates and is already accounted for by this plan; not new drift since the PIR completed.
- **D-003 window-evidence predicate against the live schema** (`rust/migrations/20260710_control_plane_shadow_decisions.sql`, re-read this session): the table's timestamp column is named **`decided_at`**, not `created_at`. Both the PIR's own recommended amendment text and v0.3's GM tranche (and the G0-001 slicing doc's now-superseded Slice 5) write the predicate as `WHERE created_at > :deploy_marker_ts ...` — a column that does not exist on this table. This is a pre-existing error in the PIR's own D-003 recommendation, carried forward unfixed into v0.3's GM text. Logged below as **GRADPLAN-D-010**.
- **`step_executor_bridge.rs:553`** (re-read this session): still the admitting call (`execute_verb_admitting_envelope(&step.verb, args, &mut ctx, None)`), still passing literal `None` for the envelope, exactly as R:§A1/G1 describe. `git log --oneline -5` shows `6ffd2659` (T9.1b prep, an unrelated extraction) as most recent, with `5a704f4e` (the PIR-D-002 fix this plan already cites) two commits back. **No drift.**

### New defect found

**GRADPLAN-D-010 — MINOR: GM's deploy-marker predicate references a non-existent column (`created_at` vs. actual `decided_at`)**

**Claim:** GM's Work section (v0.3 §3): *"`SELECT count(*) FROM \"ob-poc\".control_plane_shadow_decisions WHERE created_at > :deploy_marker_ts AND session_id NOT IN (<known test-harness session ids, enumerated at marker time>)`."* The G0-001 slicing doc's Slice 5 (superseded but still on disk) carries the same error in its exit-evidence `psql` command.

**Ground truth:** `rust/migrations/20260710_control_plane_shadow_decisions.sql` defines `decided_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()` — there is no `created_at` column on `control_plane_shadow_decisions`. The predicate as written would fail with a Postgres "column does not exist" error if run verbatim.

**Consequence if unfixed:** low-severity but directly undermines D-003's own fix — the whole point of D-003's amendment was to replace a fuzzy prose predicate with a concrete, *runnable* SQL query; a query that references a nonexistent column is not runnable, so the machine-checkability D-003 was meant to buy is not actually delivered as currently worded. Whoever executes GM would discover this only at the moment they try to run it — a small but avoidable friction at the plan's one irreversible step.

**Severity: MINOR** (one-token fix, does not change the predicate's design or GM's structure). **Recommended amendment:** in v0.3 §3 GM's Work section, change `created_at` to `decided_at` in the SQL predicate. No other text depends on the wrong column name (the surrounding prose — "Window open is then a query, not a judgement" — is otherwise sound).

### Verdict on this addendum

No BLOCKER found. All six original amendments are correctly applied in v0.3; the AD-3 restructuring is internally consistent and does not resurrect the old window-opens-at-G0 assumption anywhere checked; GM/GW pass the same machine-checkability and window-discipline tests the original PIR applied to G0-G6; AD-1/AD-2 are unchanged. One new MINOR defect (D-010) found — a wrong column name in the carried-forward D-003 predicate, non-blocking, one-token fix recommended before GM executes (not before G0). **Part 2 (G0 implementation) may proceed.**
