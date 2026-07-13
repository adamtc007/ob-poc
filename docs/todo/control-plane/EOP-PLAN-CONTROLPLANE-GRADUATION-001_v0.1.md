# EOP-PLAN-CONTROLPLANE-GRADUATION-001 — Control-Plane Graduation Plan

### Version: v0.1 (DRAFT for architect ratification)
### Date: 2026-07-13
### Status: UNRATIFIED. Two architect decisions (AD-1, AD-2) and one sequencing decision (AD-3) are open and block specific tranches as marked.
### Basis (all read; citations below use R:§ for the research doc):
- `EOP-RESEARCH-CONTROLPLANE-GRADUATION-001.md` (2026-07-13) — the grounding research; every factual claim in this plan traces to a CONFIRMED finding there unless separately cited
- `EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` v0.2 — the standing authority for graduation order (A→B→C→D), window definition (§1), procedure (§5), rollback (§6), triage (§7)
- `EOP-PLAN-CONTROLPLANE-001_Implementation-Plan_v0.1.md` §"Completion invariant (E)" — E1–E5, verbatim anchor for this plan's completion
- `scripts/check-invariants.sh` + `invariants-expected.toml` + `.github/workflows/invariants.yml` — the enforcement machinery (EOP-SESSION-CONTROLPLANE-INVARIANT-PROMOTION-001, commits `8a9b87e6`…`61540c0a`)
- `docs/research/control-plane-ownership-ledger.md` (through T11.2 Part A, 2026-07-13) — current state of record

---

## 0. What this plan is, and is not

This plan covers the remaining work to take the control plane from
"structurally wired, shadow-only, synthetic-data-only" to "genuinely
enforcing on real traffic, per the graduation runbook, with E1–E5 green."
It supersedes nothing: plan 001's tranches are landed; the 002 track
(mesh-retirement / T8–T11) continues in parallel and is **out of scope
here** except where both tracks touch the same crate (noted per tranche).
The graduation runbook remains the standing authority for path order and
procedure; this plan sequences the engineering that makes the runbook's
procedure executable.

**The critical path is calendar time, not code.** The runbook §1 window
(≥500 real shadow decisions, 0 undertriaged divergences) cannot start
until the branch is merged and deployed, and nothing shortens it
(R:§A3, R:§A5 — all 59 current shadow rows are synthetic fixtures).
The plan is therefore shaped as *ship first, build during the window*:
G0 opens the window; G1–G6 execute inside it; G7 is the graduation
event the window makes possible.

### Completion invariant (E) — restated as this plan's exit criteria

Verbatim from plan 001, unmodified; this plan completes when
`check-invariants.sh all` exits 0 with `invariants-expected.toml` at
all-pass:

> E1: every RR-3 C-0xx row is CLOSED in the ownership ledger (moved, invoked, retired, or split with both halves named).
> E2: all four RR-2 paths execute only via envelope admission in enforce mode.
> E3: G1–G14 each evaluated in production (not `NotImplemented`) with metrics flowing.
> E4: Mode-1 register (RR-5) rows either version-pinned or permanently classified human-gated with the classification tested.
> E5: workspace green: `cargo build && cargo test` all crates; public-API surface gate green; `unreachable_pub` clean.

One amendment is required for E3 to be satisfiable at all and is ratified
by this plan (G5 carries it): **E3 is evaluated per the gate-applicability
matrix (R:§B6) — "each evaluated in production" means evaluated on every
path where the matrix marks the gate applicable, with
not-applicable-by-construction cells (e.g. RunbookProof on raw DSL,
PackResolution on bus dispatch) excluded by ratified justification, not
by omission.** Until G5 ratifies the matrix, E3's expected status remains
`fail` and its detail comment tracks per-gate progress on Path A only.

---

## 1. Standing rules (apply to every tranche)

1. **Gates are law.** Every tranche's diff carries its own
   `invariants-expected.toml` flips (status or detail comments). An
   invariant going green without its expectation flipped in the same
   diff is a CI failure by design — do not "fix" this by flipping the
   expectation in a follow-up; the flip belongs in the diff that earns it.
2. **Ledger provability.** Every tranche that closes or advances a
   C-0xx row records: disposition class, commit hash, destination
   symbol(s) that resolve in the workspace — the E1 gate's bar, not the
   prose bar. "Done in code, unproven in ledger" is not done.
3. **Window discipline** (see AD-3). Any change that alters Path A's
   shadow-evaluation semantics (gate set, input population, divergence
   classification) resets the runbook §1 window. Such changes must be
   batched per AD-3's resolution; a tranche that resets the window must
   say so in its summary.
4. **Evidence rules** as per the research session: claims cite
   file:line checked in-session; UNKNOWN is a valid answer; design
   forks return options, and only the two named architect decisions
   (AD-1, AD-2) resolve them.
5. **Two-repo work** (G6 only): bpmn-lite changes follow that repo's
   own review/versioning flow; ob-poc pins advance by explicit tag
   bump, never by floating the `[patch]` redirect into a release path.
6. **Blind review.** Each tranche produces the 5-item summary format
   (per-item status with citations; production-code touches
   individually; schema imposed; dependencies; deferred judgement
   calls) for architect review before merge.

---

## 2. Architect decisions (open; each blocks the tranches marked)

### AD-1 — What does G10 (ExecutionEnvelope) grade as an *input*? [blocks G1 exit, G5]
The envelope is the *output* of a successful evaluation
(`decision::evaluate()` seals), yet G10 sits in the input-grading gate
stack (R:§A1). Options, per the research:
- **(a) G10 grades envelope *validity at consume time*** — moves G10's
  real evaluation to the admission seam (was a sealed, unconsumed,
  unexpired, content-hash-matching envelope presented?). Cleanest
  semantics; makes G10 structurally a consume-side gate like G14 is a
  post-dispatch gate, and the E3 probe must account for that (it
  already distinguishes per-gate sources).
- **(b) G10 grades *prior-decision presence*** — at evaluation time,
  "does a still-valid envelope from a prior decision exist for this
  intent," useful for re-entry/resubmission flows. Requires an
  `EvaluationContext.execution_envelope` field and a lookup source
  that does not exist today.
- **(c) Retire G10 as an input gate** and re-scope it as the admission
  mechanism's own health metric (consume outcomes already persisted per
  T8.1). Shrinks the matrix to 13 input gates + 1 admission gate + 1
  post-dispatch gate; requires an E3 amendment naming it.
Recommendation carried in this draft: **(a)** — it matches what the
mechanism already proves in `t4_1` tests and gives G10 real samples the
moment G1's threading lands. Decision is the architect's.

### AD-2 — Enforcement dimension: global-per-verb or per-(verb, path)? [blocks G4 start; constrains G7]
`EnforcedVerbs` is per-verb-FQN and path-agnostic (R:§A2). Today the
runbook's per-path graduation order works *by accident* — B/C cannot
reach `admit_in_scope`. G4 ends the accident: once B/C share the
admitting seam, a verb graduated on Path A is enforced everywhere it
dispatches, mid-plan, undeclared. Options:
- **(a) Accept global-per-verb.** Simpler mechanism; graduation of a
  verb is graduation on all paths that dispatch it. Consequence: G7's
  first candidate must be a verb dispatched *only* via Path A in real
  traffic (checkable from window data), and every later graduation
  must check the verb's path distribution first. The runbook §3 order
  becomes "order in which paths become *capable* of enforcement," not
  independent per-path switches.
- **(b) Add a path dimension** — `EnforcedVerbs` keyed by
  (verb FQN, path tag), path tag threaded from each ingress. More
  state, one more thing to misconfigure, but per-path graduation
  becomes a first-class expressible operation and the runbook §3
  order means what it says.
Recommendation carried in this draft: **(b)**, on the argument that a
graduation *runbook* whose order the mechanism cannot express is a
standing incident waiting for an operator; the added state is one enum
tag at four ingress points. Decision is the architect's; **G4 must not
start until this is ratified.**

### AD-3 — Window discipline: hold the merge, or ship and accept resets? [blocks G0's merge step]
G1 and parts of G2 alter Path A's shadow-evaluation semantics and will
reset the §1 window when they land (R:§A4's reset rule). Options:
- **(a) Hold the merge** until G1 + G2's shadow-affecting parts land,
  then deploy once for one clean window. Delays real-traffic exposure
  by the duration of G1/G2; everything before deploy is still
  synthetic-only.
- **(b) Ship now (G0 as written), treat the first window as burn-in.**
  Real traffic starts populating `control_plane_shadow_decisions`
  immediately; the divergence triage muscle (§7) gets exercised on
  real data weeks earlier; the window that *counts* is the one after
  the last shadow-semantics change (G2 exit), and the burn-in rows
  remain valuable for A5's candidate-verb frequency ranking, which
  does not require a clean window — only real traffic.
Recommendation carried in this draft: **(b)**. A5 (candidate selection)
and §7 (triage practice) both want real data early and neither needs
window cleanliness; only the final ≥500/0-diverged count does.

---

## 3. Tranches

Dependency shape (AD gates in brackets):

```
G0 ──────────────► window opens (burn-in under AD-3(b))
 ├─► G1 [AD-1] ─┐
 ├─► G2 ────────┼─► counting window opens (last shadow-semantics change)
 ├─► G3 = AD-2  │
 │      └─► G4 ─┴─► G5 ─► (E3 matrix ratified)
 ├─► G6a (bpmn-lite, parallel) ─► G6b (pins populator)
 └───────────────────────────────► G7 [window ≥500/0 + G1 + AD-2]
```

G2, G3, G6a are parallel-safe with everything. G4 needs G3(=AD-2).
G5 needs G4 (B/C must reach a per-step seam before shadow evaluation
can hang off it). G7 needs G1, the counted window, and a candidate
verb from real data.

---

### G0 — Ship and wait (opens the window)

**Objective:** merge `codex/phase-1-5-governance-closure` to `main`,
deploy, and let real Path A traffic reach the shadow table — plus the
documentation and hygiene debt that must not survive the merge.

Work items:
1. **E5 to expected-pass** (already ticketed,
   `docs/todo/workspace-hygiene-001.md`): refresh the 5 stale
   public-API baselines (dsl-runtime, ob-poc, ob-poc-boundary,
   ob-poc-control-plane, ob-poc-types); root-cause and fix
   `ob-poc-agent`'s `cargo public-api` measurement error; fix the 4
   crates failing `--no-default-features` (feature-declaration gaps).
   Flip `[e5] = pass` in the same diff.
2. **Runbook corrections** (doc-only, but preconditions per R:§C3):
   §2 readiness table (Path A calls the admitting entry point at
   `step_executor_bridge.rs:553`, 12/14 gates have real shadow inputs
   per R:§A1's table — replace the "G1 only" claim); §4 box 1 flipped
   DONE with commit `5a704f4e`; §8 T6.1a re-scoped as a **two-repo
   change** (real producer: `bpmn-lite-engine::plan_walker::
   dispatch_callout`, R:§C3) — delete the "ob-poc-only" claim.
3. **Governance index**: `docs/todo/control-plane/INDEX.md` listing
   every live governing artifact (plan 001, the 002 track's scope
   note, this plan, the runbook, PIR-001 as historical, the invariant
   session doc + evidence, the research doc, the T9.2/T11.x design
   docs, MCA-001/002, workspace-hygiene-001). Every future session's
   Phase 0 reads this first. Two governing docs were invisible to
   grounding phases this week; a third instance is not acceptable.
4. **Ledger provability backfill (bounded):** for rows whose closing
   work has already landed but which fail E1's provability bar
   (claimed-CLOSED without hash/resolving symbol — C-001 is the known
   instance), add the citations. Strictly no new engineering: rows
   that are genuinely open stay open. Update `[e1]`'s detail comment
   with the new provable count.
5. **Merge + deploy.** Window opens (burn-in, under AD-3(b)).

**Exit gate:** `[e5] = pass` in CI; runbook v0.3 committed; INDEX.md
exists; deployment confirmed writing real rows to
`control_plane_shadow_decisions` (first non-fixture row is the
evidence).
**E-movement:** E5 → pass. E1 detail improves.
**Session tier:** grind-suitable throughout (items 1, 4 are mechanical;
items 2, 3 are doc edits against research citations). Blind review
required before merge (item 5 is the one irreversible step).

---

### G1 — Path A enforce-readiness: seal → handle → consume [AD-1]

**Objective:** close the gap R:§A4 exposed — the admitting call at
`step_executor_bridge.rs:553` passes `envelope_id: None` while sealing
happens in a structurally separate flow inside `phase5_runtime_recheck`.
Until these are connected, flipping `ENFORCE_VERBS` on any verb is an
outage on that verb.

Work items:
1. Thread the envelope: the `EnvelopeHandle` produced when
   `decision::evaluate()` seals (ApprovedStp-eligible decisions) must
   reach the subsequent `execute_verb_admitting_envelope` call for the
   same intent as `Some(handle)`. Establish the correlation carrier
   (sequencer entry state is the natural home; design detail for the
   session, within the existing types — no new crate edges expected,
   both flows are `ob-poc`-internal).
2. Resolve G10 per AD-1's ratified option; wire its real evaluation at
   the seam the decision names; E3 probe updated if AD-1(a)/(c) moves
   G10 off the input stack (probe already distinguishes
   infrastructure vs invariant failure; extend its per-gate source map).
3. Live-DB test: an enforced verb (`ENFORCE_VERBS` set in the test
   env) with a threaded real envelope admits, consumes once, rejects
   resubmission; with no envelope, rejects — i.e., the `t4_1` property
   set now proven *from the Path A call site*, not just from the
   adapter's own tests.
4. Non-eligible decisions (HumanGate/Rejected in shadow): confirm and
   test the enforce-mode behaviour is reject-with-triage-classification
   per runbook §7, not silent fallthrough.

**Exit gate:** the test in item 3 green in CI; `[e2]` detail comment
updated ("Path A enforce-capable, not yet enforced"); no window
implications beyond AD-3's accounting (this IS a shadow-semantics
change — it lands before the counted window opens).
**E-movement:** E2 mechanism-readiness (status stays `fail` — no verb
is enforced yet, correctly); E3 (G10 gains real samples under AD-1(a)).
**Session tier:** the threading design (item 1) is Fable/Opus work with
architect review; items 3–4 are grind-suitable against the ratified
design.

---

### G2 — Bounded single-gate work (parallel-safe)

**Objective:** the three remaining zero/undercounted gates, each a
bounded, independent piece (R:§A1's per-gate analysis).

Work items:
1. **G12 metrics undercount** — fix `gate_outcome_counts`' SQL
   classification of `report_to_json`'s `"missing"` sentinel
   (historical rows predating a gate's registration) so it is
   distinguished from genuinely unrecognised values. Small,
   T7.2-scoped. G12's wiring is already real (`compiler_version`);
   this makes the count truthful. Stretch (optional, flag if taken):
   populate `bus_catalogue_version` where reachable.
2. **G14 post-dispatch call site** — wire `set_expected_write_set` +
   `commit_attested` into the sequencer's commit path (currently plain
   `commit()`, R:§A1/T10.3). Capture is already live in the CRUD
   executors; this closes the compare-and-attest half. Advances C-032.
3. **G11 / T7.1 audit stream** — build the `control_plane_audit`
   append-per-decision stream (the named blocker for G11), then G11's
   real evaluation against it. The largest item in G2; if it needs its
   own session, split it out as G2b rather than letting it delay
   items 1–2.

**Exit gate:** E3 probe shows G11, G12, G14 with substantive samples
(G14 via its post-dispatch source, which the probe's per-gate source
map must reflect); `[e3]` detail comment recounted. **G2's completion
marks the last Path-A shadow-semantics change — the counted window
opens at its deploy (AD-3).**
**E-movement:** E3 (three gates move); E1 (C-032 advances).
**Session tier:** items 1–2 grind-suitable; item 3's stream design
wants a short design doc first (same review flow as T9.x designs).

---

### G3 — Enforcement-dimension ratification (= AD-2)

**Objective:** a short design doc resolving AD-2, ratified before any
G4 code. If AD-2(b): specify the path-tag enum, its four ingress
threading points, `EnforcedVerbs` keying, and env-var syntax
(backward-compatible: an untagged entry means all-paths, preserving
today's semantics). If AD-2(a): specify the path-distribution check
G7 and all later graduations must run against window data before
pinning a verb, as a documented runbook §5 step.

**Exit gate:** ratified doc committed; runbook §5 amended accordingly.
**E-movement:** none directly; unblocks G4.
**Session tier:** architect + Fable; not grind work.

---

### G4 — Path B/C per-step admission [needs G3]

**Objective:** strengthen B/C from T9.3's plan-level pre-flight
admission to the per-step atomic consume property A/D have — B3
option (i), as researched.

Work items:
1. Admission call inside `dsl_v2::executor::execute_verb_in_scope`
   (the confirmed single seam, `executor.rs:1914`, R:§B2), same-crate
   visibility changes only (R:§B4 — zero new crate edges).
2. **Double-admission guard**: `ObPocVerbExecutor`'s Branch-3
   fallthrough reaches this same seam after already admitting
   (R:§B3(i)). The seam's check must be skippable-with-proof —
   admission context carried in scope/ctx marks "already admitted,
   handle consumed by outer call." Design detail with a hard test:
   Branch-3 fallthrough must neither double-consume nor reject a
   properly admitted dispatch.
3. Enforcement dimension per G3's ratified shape wired at this seam.
4. **Atomicity tests on the dsl_v2 seam** — the `t4_1` equivalents
   (rollback-of-consume on dispatch failure; pin-drift rejection
   leaving the envelope reconsumable) which R:§B5 confirms the scope
   shape supports without restructuring, and which do not exist today.
5. Retain T9.3's plan-level pre-flight as the outer check (defence in
   depth; it already carries the whole-plan-walk rejection property).

**Exit gate:** E2's structural check (exclusivity form) passes for
Paths B and C — admitting call present at the seam, zero bare
`execute_verb(` bypasses; atomicity tests green; `[e2]` detail flipped
to 4/4 structural (enforce-mode still pending, correctly `fail`).
**E-movement:** E2 structural completes; E4 Row 5 advances (admission
shape now supports per-step pins — closure still needs G6b's
populator).
**Session tier:** item 2's guard design wants review before grind;
the rest is grind-suitable with the atomicity tests as the gate.

---

### G5 — Shadow-gate evaluation on B/C/D + E3 matrix ratification [needs G4]

**Objective:** the research's sleeper finding (R:§B6) — the G1–G14
shadow-evaluation pipeline runs only at Path A's
`phase5_runtime_recheck`. Extend evaluation to B/C/D and ratify the
applicability matrix as E3's normative per-path definition.

Work items:
1. Resolve the three applicability UNKNOWNs left reasoned-not-confirmed
   (G3 and G9 on Paths B/C; G3 on C vs D distinctions) by code
   confirmation, completing the matrix.
2. Build the evaluation call for B/C at the G4 seam (context builders
   reusing the Path-A input sources where the matrix marks a gate
   applicable; `NotApplicable` recorded as a first-class outcome, not
   `NotEvaluated`, so the E3 probe can tell them apart).
3. Path D evaluation at `bus_runtime.rs`'s adapter (matrix column D;
   several gates are not-applicable-by-construction there — the
   ratified matrix is the authority).
4. Amend the E3 gate (`check-invariants.sh` + probe) to evaluate
   against the ratified matrix: per-(gate, path) applicable cells need
   substantive samples; NA cells need the ratified justification
   string present in the matrix doc. Carry the `[e3]` expectation/
   detail flips in the same diff.

**Exit gate:** matrix doc ratified; E3 probe green per-matrix on all
shadow-wired cells for whatever traffic exists (synthetic acceptable
for B/C/D initially — their windows are a later graduation concern,
per runbook order).
**E-movement:** E3 becomes satisfiable as amended; substantial detail
movement.
**Session tier:** item 1 research-grade; items 2–4 grind-suitable
against the ratified matrix.

---

### G6 — Path D completion (two repos) + SnapshotPins populator

**G6a — bpmn-lite envelope threading (T6.1a, corrected scope).**
Under the runbook's corrected framing (G0 item 2): choose and implement
one of R:§C2's carriers —
- (a) `EnvelopeHandle` as a named `ResolvedBinding` in `inputs`,
  sourced from `ProcessInstance`, extracted in
  `ObPocVerbAdapter::execute` before the admitting call; or
- (b) populate the dormant `snapshot_pin` proto field end-to-end
  (`plan_walker.rs` → `dsl-bus-server::InvocationContext` widening →
  `VerbExecutor::execute` trait signature → `bus_runtime.rs`).
Option choice is the session's to propose with the architect ratifying
— (b) touches three components' interfaces but uses the field designed
for the job; (a) is smaller but overloads the inputs channel with
control-plane material. Standing rule 5 applies: bpmn-lite changes ride
that repo's own flow, pinned by tag bump. Per R:§C3, `admit()` itself
is confirmed context-free — once a real handle arrives,
no D-specific admission work remains.

**G6b — SnapshotPins production populator** [needs G6a for Row 3 only].
The common root blocker for RR-5 Rows 2/3/5 (R:§B7): nothing populates
real `SnapshotPins` at admission time outside Path A's shadow flow.
Build the populator at the seams G1/G4/G6a established; wire
`verify_pins_in_scope` live per path. Rows 2 and 5 are startable
before G6a lands.

**G6c — RR-5 Row 4 investigation** (small, standalone): determine BPMN
`process_instances`' version-pin story (UNKNOWN, no ledger tranche —
R:§B7). Outcome is either scoped work appended to this tranche or a
ratified human-gated classification with its named test (E4's bar).

**Exit gate:** E4 gate passes for Rows 2/3/5 with the provisional
slug→symbol mapping *ratified or corrected* (the invariant session
marked it provisional; this tranche is where it becomes contract);
Row 4 resolved either way; `[e4]` flipped per outcome.
**E-movement:** E4 → pass (all five rows); E2/E3 for Path D become
real rather than vacuous.
**Session tier:** G6a is coordinated two-repo work — architect-involved,
not pure grind; G6b/G6c grind-suitable.

---

### G7 — First graduation event

**Objective:** execute runbook §5 for the first verb, for real.

Preconditions (all hard):
- Counted window complete: `shadow_divergence_stats` shows
  `total_decisions >= 500 AND diverged == 0` (or every divergence
  triaged per §7) on real traffic accumulated since G2's deploy.
- G1 landed (enforce-capable Path A) — without it, enforcement is an
  outage by construction.
- Candidate verb chosen from **real** window data (A5 is answerable
  only now): low-frequency, low-consequence, and — under AD-2(a) —
  dispatched via Path A only in observed traffic; under AD-2(b) —
  pinned as (verb, path-A).
- Architect sign-off recorded in the ledger in the runbook §4 box-4
  form (the graduation sign-off event the ledger currently has no
  instance of, R:§A4).

Procedure: runbook §5 verbatim (pin, observe, widen or roll back per
§6). The diff that sets the first `ENFORCE_VERBS` pin carries `[e2]`'s
detail flip ("1 verb enforced on Path A") — E2's *status* flips to
pass only when all four paths are enforce-mode per its text, which is
the plan's completion, not this event.

**Exit gate:** first verb enforced in production, observed stable per
§5's own criteria; ledger records the graduation event with hash.
**E-movement:** E2's first real movement; the template for every
subsequent graduation.
**Session tier:** ops + architect. Not grind work.

---

## 4. Completion mapping

| Invariant | Reaches pass via | Expected flip carried by |
|---|---|---|
| E1 | G0.4 backfill + per-tranche standing rule 2 + final sweep when the last row's work lands | each closing tranche; final `[e1]=pass` by whichever tranche closes the last row |
| E2 | G1 (capable) → G4 (structural 4/4) → G7 onward (enforce per path, widened per runbook order until all four) | G4 detail; G7+ detail; `pass` at full enforce coverage |
| E3 | G1 (G10) + G2 (G11/G12/G14) + G5 (matrix + B/C/D) | G2 detail; G5 status per amended definition |
| E4 | G6b/G6c (+ G6a for Row 3) | G6 |
| E5 | G0.1 | G0 |

## 5. Risks and open items

- **AD decisions pending** — G1 exit, G4 start, and G0's merge step
  are each blocked on their AD; ratify AD-3 first (it gates the merge),
  AD-1/AD-2 can follow within days without delaying G0.
- **Window resets** — any post-G2 change to Path A shadow semantics
  restarts the count. G5's B/C/D evaluation is designed not to touch
  Path A's call site; a session that finds it must is a stop-and-review
  event, not a judgement call.
- **Two-repo coordination (G6a)** — bpmn-lite is not ob-poc's to merge;
  sequence G6a early enough that its review latency doesn't put G6b's
  Row 3 on the critical path (Rows 2/5 are startable regardless).
- **Parallel 002 track** — mesh-retirement/T11 work shares
  `ob-poc-control-plane` and `ob-poc-agent`; the E5 surface gates
  (green after G0) are the collision detector. T11.2 Part B
  (`CapabilityInvocation`) remains deferred-until-consumer per its own
  ruling; nothing in this plan is that consumer.
- **Known-stale docs** — PIR-001 is historical; the ledger is current
  state; the runbook is authoritative for procedure only after G0
  item 2's corrections land.

## 6. Non-goals

Mesh-retirement/agent-tier extraction (T11.x), definitional-floor work
(T11.F.x), and any redesign of the gate stack, ledger, or registers
beyond the amendments explicitly carried here (E3 matrix, E4 mapping
ratification, G10 per AD-1).
