# EOP-PLAN-CONTROLPLANE-GRADUATION-001 — Control-Plane Graduation Plan

### Version: v0.5 (G1 + G3 design docs ratified)
### Date: 2026-07-13
### Status: AD-1/AD-2/AD-3 all ratified (unchanged from v0.4). Both design docs the resolutions unblocked are now ALSO ratified (architect, 2026-07-13): `EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001.md` (G1 item 1) and `EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001.md` (G3, whose §(f) runbook amendment is applied — the runbook is now v0.4). G1 items 2-4 and G4 are unblocked to start grinding. PIR verdict RATIFY-WITH-AMENDMENTS, all six amendments (GRADPLAN-D-001..006) applied as of v0.2.
### Changelog v0.4 → v0.5 (design-doc ratification):
- G1 item 1's design doc (`EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001`) RATIFIED: carrier is a new `entry_id` column on `control_plane_envelopes` (not "sequencer entry state" as this plan's own text guessed); `HumanGate` entries re-seal at resume rather than extending a stale envelope; per-step sealing for multi-step runbooks; no new crate edges. Ratification note recorded in the design doc itself: this plan's own citation of the T10.1 `evaluate_shadow`/`evaluate` MIGRATION-PENDING convergence as still-open is STALE — that split was closed 2026-07-11, before this plan (v0.4) was drafted. Not corrected in this revision (a defect in a plan is reported, not silently patched by the doc that found it) — flagged here for the plan's own next correction pass.
- G3's design doc (`EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001`) RATIFIED: `ExecutionPath` enum in `ob-poc-types`; `EnforcedVerbs` reshaped to `HashMap<String, PathScope>` (`All | Only(HashSet<ExecutionPath>)`) rather than this plan's own `HashSet<(String, PathTag)>` framing, which cannot cleanly express "untagged = all paths"; env-var grammar `verb[:tag(|tag)*]`, fail-CLOSED on any malformed entry (rejects the whole config, not just the bad entry); Branch-3 double-admission fallthrough carries the SAME tag as the outer admission, never a distinct one. Runbook §5 amendment applied (below). Correction recorded: AD-2(b)'s "one enum tag at four ingress points" cost claim is a tag-count, not a location-count — Path B is an umbrella over several distinct callers sharing one tag, Path C is a single tagged instance; doesn't overturn AD-2(b), noted so G4's implementer isn't surprised mid-build.
- `EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` bumped v0.3 → v0.4: §5's procedure now freezes `(verb-FQN, path-tag)`, not a bare verb-FQN; untagged entries reserved for verbs already graduated on all four paths independently, not as a first move; step 3's env var takes `verb:path-tag` syntax; step 5's ledger record names the path tag(s) graduated.
- G1's dependency-graph bracket and G3's tranche header updated: both design docs move from "not yet written" to ratified; G1 items 2-4 and G4's first line of code are unblocked.
### Changelog v0.3 → v0.4 (AD-1 + AD-2 resolution):
- AD-1 RESOLVED as (a): G10 grades envelope validity at consume time — matches what `t4_1` already proves, and the PIR's under-costing caveat (GRADPLAN-D-001, the per-gate provenance dimension G10's consume-seam samples need) is absorbed because G2 item 4 builds that provenance dimension regardless of AD-1's outcome; G1 item 2's consume-seam recording rides machinery already scheduled, not new scope
- AD-2 RESOLVED as (b): `EnforcedVerbs` gains a path dimension — the PIR strengthened this option independently (E2's own structural gate already reasons per-path; the enforcement mechanism being the one component that cannot express the runbook's A→B→C→D ordering is exactly the asymmetry that bites an operator later); cost is one enum tag at four ingress points
- §4's E3 completion-mapping footnote (previously conditioned on "assumes AD-1(a)") now states AD-1(a) as settled fact, not a shape assumption
- §5 risks: "AD decisions pending" risk retired — replaced with "design docs are now the critical path" (both G1's seal→consume doc and G3's enforcement-dimension doc are unblocked, not yet written)
- G1 item 1 and G3 (both previously gated on their respective AD) may now proceed to their design-doc deliverables
### Changelog v0.1 → v0.2 (per the PIR register):
- D-001 (MAJOR): E3 probe capability (per-gate source/provenance; `GateResult::NotApplicable`) scoped as explicit sub-items in G2 and G5 — it does not exist today and is core-type/schema work, not query tweaks
- D-002 (MINOR): runbook-correction count fixed to 11/14
- D-003 (MAJOR): G0 exit gate's "first non-fixture row" replaced with a machine-checkable deploy-marker predicate
- D-004 (MINOR): dependency graph now shows AD-1 blocking G1 item 2/exit only
- D-005 (NOTE): §4's E3 row footnoted as conditioned on AD-1(a)
- D-006 (MAJOR): G1 item 1 (seal→consume correlation carrier) split into its own design doc; G1 grind items blocked on it
### Changelog v0.2 → v0.3 (AD-3 resolution):
- AD-3 RESOLVED as (a): single-operator deployment — the operator IS the traffic, so option (b)'s early-real-traffic value is void; one merge, one clean window
- Critical-path framing inverted in §0: engineering (G1+G2) is the long pole, not calendar time
- Merge+deploy extracted from G0 into its own tranche **GM**, sequenced AFTER G1+G2; deploy-marker predicate (D-003) moves with it
- New stage **GW — evidence-generation campaign**: the window is filled by deliberate, ledger-logged exercise runs, not passive accumulation
- G0 item 2 gains a runbook §1 recalibration: "production evidence" defined for a single-operator deployment
- Slice 5 of EOP-IMPL-…-G0-001 inherits GM's preconditions (G1+G2 landed), superseding its v0.1 precondition list
- Promoted from the slicing doc: Slice 7's production-behavior-change STOP-condition is now named in G2 item 2's own text
### Basis (all read; citations below use R:§ for the research doc):
- `EOP-RESEARCH-CONTROLPLANE-GRADUATION-001.md` (2026-07-13) — the grounding research; every factual claim in this plan traces to a CONFIRMED finding there unless separately cited
- `EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` v0.2 — the standing authority for graduation order (A→B→C→D), window definition (§1), procedure (§5), rollback (§6), triage (§7)
- `EOP-PLAN-CONTROLPLANE-001_Implementation-Plan_v0.1.md` §"Completion invariant (E)" — E1–E5, verbatim anchor for this plan's completion
- `scripts/check-invariants.sh` + `invariants-expected.toml` + `.github/workflows/invariants.yml` — the enforcement machinery (EOP-SESSION-CONTROLPLANE-INVARIANT-PROMOTION-001, commits `8a9b87e6`…`61540c0a`)
- `docs/research/control-plane-ownership-ledger.md` (through T11.2 Part A, 2026-07-13) — current state of record
- `EOP-PIR-CONTROLPLANE-GRADPLAN-001.md` (2026-07-13) — the adversarial review of this plan; defect IDs GRADPLAN-D-0xx cited below

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

**The critical path is engineering, not calendar time** (inverted from
v0.1/v0.2 by AD-3(a)). This is a single-operator deployment: the
operator is the traffic, so nothing accumulates while nobody is
driving, and there is no early-shipping dividend to collect. The plan
is therefore shaped as *build first, merge once, then fill the window
deliberately*: G0 clears hygiene and doc debt; G1+G2 land every
Path-A shadow-semantics change pre-merge; GM merges and deploys
exactly once, opening the only window; GW fills it with a
ledger-logged exercise campaign (≥500 real decisions per runbook §1 —
"real" meaning genuine operator-driven dispatches through the full
stack, distinguished from test fixtures by GM's deploy marker, not by
who typed them); G7 is the graduation event the window makes
possible. G3–G6 remain parallel work alongside this spine.

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

### AD-1 — What does G10 (ExecutionEnvelope) grade as an *input*? [RESOLVED: (a) consume-time validity — architect, 2026-07-13]
Ratified option: **(a)** — G10 grades envelope *validity at consume
time*: real evaluation moves to the admission seam (was a sealed,
unconsumed, unexpired, content-hash-matching envelope presented?). G10
is structurally a consume-side gate, the same shape as G14's
post-dispatch posture; the E3 probe's existing per-gate-source
distinction (built regardless, in G2 item 4) accounts for it without
further amendment.
Rationale of record: matches what the mechanism already proves in the
`t4_1` test suite — no new semantics, just a real call site. The PIR's
own under-costing caveat (GRADPLAN-D-001 — G2 item 4's provenance
dimension is core-type/schema work, not a query tweak) does not weigh
against (a) specifically: that dimension is being built in G2
regardless of which AD-1 option is chosen, so G1 item 2's consume-seam
recording rides machinery already on the critical path rather than
adding new scope of its own.
Consequences: G1 item 2 (resolve G10 per AD-1, wire its real
evaluation) is unblocked, still dependent on G2 item 4 landing first
(named dependency, not new). §4's completion-mapping footnote no
longer needs the "assumes AD-1(a)" hedge (options (b)/(c)'s
alternative row-derivations are moot). The G1 seal→consume design doc
(`EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001`, item 1) can now name
G10's consume-time semantics as settled input to its own design,
rather than carrying it as an open fork.

### AD-2 — Enforcement dimension: global-per-verb or per-(verb, path)? [RESOLVED: (b) per-(verb, path) dimension — architect, 2026-07-13]
Ratified option: **(b)** — `EnforcedVerbs` gains a path dimension,
keyed by (verb FQN, path tag), path tag threaded from each of the four
ingress points (Sequencer/A, dsl_v2 seam once G4 lands/B+C, bus
adapter/D). Backward-compatible: an untagged entry means all-paths,
preserving today's semantics for any verb pinned before this lands.
Rationale of record: the PIR strengthened this option independently of
the draft's own recommendation — E2's own structural gate already
reasons per-path (its exclusivity check treats each of the four RR-2
paths as a distinct thing to prove), so an enforcement mechanism that
cannot express *which* path a graduation applies to is the one
component of the whole system asymmetric with everything around it.
That asymmetry is exactly the shape of incident that surfaces later,
at an operator's expense, not at design time. The cost side of the
ledger is small and bounded: one enum tag at four ingress points, not
an open-ended state expansion.
Consequences: **G4 must not start until G3's design doc (below)
ratifies the concrete path-tag enum, keying, and env-var syntax** —
AD-2 itself is resolved, but G3's mechanical design is still a
prerequisite for G4's first line of code. G7's graduation procedure
now expresses "graduate this verb on Path A only" literally, matching
the runbook §3 order's own meaning rather than the order becoming
"paths become capable" language option (a) would have required.

### AD-3 — Window discipline [RESOLVED: (a) hold the merge — architect, 2026-07-13]
Ratified option: **(a)** — hold the merge until G1 + G2's
shadow-affecting parts land, then deploy once for one clean window
(tranche GM).
Rationale of record: this is a **single-operator deployment**. The
draft's and the PIR's arguments for (b) — early burn-in telemetry,
§7 triage practice on live data, A5 frequency ranking — all priced
"real traffic" as something that accumulates independently of the
team. Here the operator is the traffic: nothing accrues unattended,
A5's frequency question is answerable from the operator's own usage
knowledge, and deferring the merge costs no external users anything.
Holding produces exactly one deploy, one window, and zero reset
accounting.
Consequences applied in this version: merge extracted from G0 into
GM (post-G1/G2); GW added as the deliberate window-filling stage;
§0's critical-path framing inverted; runbook §1's "production
evidence" recalibrated for a single operator (G0 item 2).

---

## 3. Tranches

Dependency shape (design-doc gates in brackets — all three ADs are now
ratified, and both G1's and G3's design docs are now ALSO ratified;
brackets mark the deliverable each tranche's grind work depends on and
its current status):

```
G0 (hygiene + docs; no merge)
 ├─► G1 [doc ratified: items 2-4 unblocked, item 2 also needs G2.4] ─┐
 ├─► G2 ────────────────────────────────────────────────────────────┼─► GM (merge+deploy, once)
 ├─► G3 [doc ratified: G4 unblocked]                                │        │
 │      └─► G4 ─────────► G5 ─► (E3 matrix                          │        ▼
 │                              ratified)                           │   GW (exercise campaign
 ├─► G6a (bpmn-lite, parallel) ─► G6b                                │       ≥500/0 window)
 │                                                                    │        │
 └────────────────────────────────────────────────────────────────────┴─► G7
```

Bracket tags mark what each design doc actually blocks (GRADPLAN-D-004
principle, now applied to the ratified ADs' successor artifacts): G1's
design doc (`EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001`) is ratified
— items 3–4's grind prep can start now; item 2 additionally waits on
G2 item 4's provenance dimension (a named dependency, not an open
design fork). G3's design doc
(`EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001`) is ratified —
G4 is unblocked to start against its mechanical spec.

G2, G3, G6a are parallel-safe with everything. G4 needs G3's doc.
G5 needs G4 (B/C must reach a per-step seam before shadow evaluation
can hang off it). GM needs G1 + G2 (every Path-A shadow-semantics
change lands pre-merge — that is AD-3(a)'s whole point). GW needs GM.
G7 needs GW's completed window and a candidate verb from the
campaign's own data. G4/G5/G6 may land before or after GM; if after,
standing rule 3 still applies (none of them may touch Path A's
shadow call site — GRADPLAN-D-009 verified none do as drafted).

---

### G0 — Ship and wait (opens the window)

**Objective:** clear the hygiene and documentation debt that must not
survive into the merge (which is now GM's, not G0's — AD-3(a)).

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
   `step_executor_bridge.rs:553`, **11/14 gates** — G1–G9, G12
   (partial), G13 — have real shadow inputs per R:§A1's table
   (GRADPLAN-D-002 correction; G10/G11 are stubs, G14 is
   not-applicable at this call site) — replace the "G1 only" claim); §4 box 1 flipped
   DONE with commit `5a704f4e`; §8 T6.1a re-scoped as a **two-repo
   change** (real producer: `bpmn-lite-engine::plan_walker::
   dispatch_callout`, R:§C3) — delete the "ob-poc-only" claim; and a
   new §1 clause recalibrating "production evidence" for a
   single-operator deployment: window rows are genuine operator-driven
   dispatches through the full stack, accumulated via deliberate
   exercise runs logged in the ledger (GW), distinguished from test
   fixtures by GM's deploy marker + session-id exclusion — not by
   passive-traffic assumptions the deployment cannot satisfy.
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
(Item 5, merge + deploy, moved to tranche GM under AD-3(a).)

**Exit gate:** `[e5] = pass` in CI; runbook v0.3 committed (including
the §1 single-operator recalibration); INDEX.md exists; `[e1]` detail
recounted.
**E-movement:** E5 → pass. E1 detail improves.
**Session tier:** grind-suitable throughout (items 1, 4 are mechanical;
items 2, 3 are doc edits against research citations). Slicing:
EOP-IMPL-CONTROLPLANE-GRADUATION-G0-001 Slices 1–4 stand as written;
its Slice 5 is superseded by tranche GM's preconditions below.

---

### G1 — Path A enforce-readiness: seal → handle → consume [design doc ratified 2026-07-13]

**Objective:** close the gap R:§A4 exposed — the admitting call at
`step_executor_bridge.rs:553` passes `envelope_id: None` while sealing
happens in a structurally separate flow inside `phase5_runtime_recheck`.
Until these are connected, flipping `ENFORCE_VERBS` on any verb is an
outage on that verb.

Work items:
1. **Seal→consume design doc**
   (`EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001`) — the correlation
   carrier is NOT inline session work (GRADPLAN-D-006): the `EnvelopeHandle`
   sealed by `decision::evaluate()` (inside `phase5_runtime_recheck`,
   `sequencer.rs:~8015`) must reach `execute_verb_admitting_envelope`
   at `step_executor_bridge.rs:553`, two functions separated by the
   step scheduler. The design doc must answer, at minimum:
   (a) where the handle lives between seal and consume (sequencer
   entry state is the candidate, not the decision);
   (b) lifetime/expiry when the seal→consume gap exceeds the
   envelope's validity window (5 min per T10.1's convention) —
   re-seal, extend, or reject-and-retriage;
   (c) retry/replay: a retried step against a single-use consumed
   envelope — reuse forbidden by construction, so does retry re-seal,
   and what distinguishes legitimate retry from replay;
   (d) multi-step runbooks: per-step seal/consume pairs or a
   plan-level envelope.
   It must also address T10.1's registered owed convergence
   (`evaluate_shadow()`/`evaluate()` as two parallel entry points,
   MIGRATION-PENDING, target T10.2's admission-scope wrapper) — the
   same structural split this design bridges; converging or
   consciously not converging is a decision the doc records. Same
   review flow as the T9.x/T10.x design docs. No new crate edges
   expected (both flows are `ob-poc`-internal) — the doc confirms or
   corrects this.
2. Resolve G10 per AD-1's ratified option; wire its real evaluation at
   the seam the decision names. Note (GRADPLAN-D-001): the probe's
   infrastructure-vs-invariant split is a whole-run distinction; **no
   per-gate source map exists today** — the capability G10's
   consume-seam samples (under AD-1(a)) would report through is built
   in G2 item 4, which this item therefore depends on.
3. Live-DB test: an enforced verb (`ENFORCE_VERBS` set in the test
   env) with a threaded real envelope admits, consumes once, rejects
   resubmission; with no envelope, rejects — i.e., the `t4_1` property
   set now proven *from the Path A call site*, not just from the
   adapter's own tests.
4. Non-eligible decisions (HumanGate/Rejected in shadow): confirm and
   test the enforce-mode behaviour is reject-with-triage-classification
   per runbook §7, not silent fallthrough.

**Exit gate:** the test in item 3 green in CI; `[e2]` detail comment
updated ("Path A enforce-capable, not yet enforced"). This IS a
shadow-semantics change — under AD-3(a) it lands pre-merge by
construction (GM depends on it), so no window accounting arises.
**E-movement:** E2 mechanism-readiness (status stays `fail` — no verb
is enforced yet, correctly); E3 (G10 gains real samples under AD-1(a)).
**Session tier:** item 1 is a Fable/Opus design doc with architect
ratification; **items 2–4 do not start grinding until item 1's doc is
ratified** (GRADPLAN-D-006) — they assume its design exists and is
stable underneath them. Item 2 no longer separately waits on an
architect decision (AD-1(a) is ratified) — only on item 1's doc and
G2 item 4's provenance dimension landing.

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
   **Named caution (promoted from the slicing doc's Slice 7):** this is
   the plan's ONE production-behavior change — a real excess/undeclared
   write gets caught and rolled back where it previously wasn't. If
   implementation finds any verb's behavior changing, stop and flag
   for architect review even with green tests; everything else in
   G0–G2 is shadow-only by posture.
3. **G11 / T7.1 audit stream** — build the `control_plane_audit`
   append-per-decision stream (the named blocker for G11), then G11's
   real evaluation against it. The largest item in G2; if it needs its
   own session, split it out as G2b rather than letting it delay
   items 1–2. G2b's design doc must state explicitly whether the audit
   stream is also the per-gate provenance source item 4 needs — named
   dependency, not later discovery.
4. **E3 probe capability: per-gate source/provenance dimension**
   (GRADPLAN-D-001 — this does not exist today; `gate_outcome_counts`
   groups strictly by (gate, outcome_kind) with no source column
   anywhere in query, schema, or types). Scope: a per-gate provenance
   dimension (Path-A shadow eval / post-dispatch attestation /
   consume-seam) through schema or derivation, `gate_outcome_counts`,
   and the probe's assertions. Sized and reviewed as its own piece of
   work — it is a prerequisite for this tranche's own exit gate and
   for G1 item 2 and G5 item 5. Since it may touch what shadow rows
   record, it lands inside G2 by definition (window discipline —
   standing rule 3).

**Exit gate:** item 4's provenance dimension merged; E3 probe shows
G11, G12, G14 with substantive samples, G14 attributed via its
post-dispatch provenance (now expressible); `[e3]` detail comment
recounted. **G2's completion marks the last Path-A shadow-semantics
change — GM (merge) is unblocked at its close (AD-3(a)).**
**E-movement:** E3 (three gates move); E1 (C-032 advances).
**Session tier:** items 1–2 grind-suitable; items 3–4 each want a
short design doc first (same review flow as T9.x designs), then grind.

---

### G3 — Enforcement-dimension mechanical design [RATIFIED 2026-07-13 — unblocks G4]

**Objective:** a short design doc turning AD-2(b)'s ratified decision
into a concrete spec, ratified before any G4 code: the path-tag enum,
its four ingress threading points (Sequencer/A, dsl_v2 seam once G4
lands/B+C, bus adapter/D), `EnforcedVerbs` keying by (verb FQN, path
tag), and env-var syntax (backward-compatible: an untagged entry means
all-paths, preserving today's semantics for any verb pinned before
this lands).

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
1. **`GateResult::NotApplicable` variant** (GRADPLAN-D-001 — this is a
   new variant on the crate's most-depended-on, exhaustively-matched
   core enum, NOT a query tweak): add the variant; sweep every `match`
   over `GateResult` in `ob-poc-control-plane` and `ob-poc` (no
   wildcard arms — the compiler enumerates the sweep); extend
   `report_to_json`/`gate_outcome_counts`/the probe. Window-discipline
   check carried in the same diff: no Path-A gate returns
   `NotApplicable`, verified by test, so Path-A shadow semantics are
   untouched (standing rule 3).
2. Resolve the three applicability UNKNOWNs left reasoned-not-confirmed
   (G3 and G9 on Paths B/C; G3 on C vs D distinctions) by code
   confirmation, completing the matrix.
3. Build the evaluation call for B/C at the G4 seam (context builders
   reusing the Path-A input sources where the matrix marks a gate
   applicable; `NotApplicable` recorded as a first-class outcome, not
   `NotEvaluated`, so the E3 probe can tell them apart).
4. Path D evaluation at `bus_runtime.rs`'s adapter (matrix column D;
   several gates are not-applicable-by-construction there — the
   ratified matrix is the authority).
5. Amend the E3 gate (`check-invariants.sh` + probe) to evaluate
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
**Session tier:** item 2 research-grade; item 1's enum sweep and
items 3–5 grind-suitable against the ratified matrix (item 1 wants
its match-sweep plan reviewed before grinding — the fallout surface
is workspace-wide).

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

### GM — Merge + deploy (once) [needs G0, G1, G2; the irreversible step]

**Objective:** the single merge of `codex/phase-1-5-governance-closure`
to `main` and deploy, with every Path-A shadow-semantics change
already underneath it (AD-3(a)).

Preconditions (all hard — these supersede the slicing doc's Slice 5
precondition list):
- G0 exit gate met (`[e5] = pass` live-verified immediately before
  merge, not believed-from-earlier).
- G1 exit gate met (enforce-capable Path A; seal→consume design
  ratified and implemented).
- G2 exit gate met (last shadow-semantics change landed, including
  item 4's provenance dimension).
- Architect blind review of the merged whole.

Work: merge; deploy; record the deploy marker — one row in a
`control_plane_deploy_markers` table (or minimally the deploy
timestamp + HEAD hash in `invariants-expected.toml` comments and the
ledger). "Window open" is then a query, not a judgement
(GRADPLAN-D-003):
`SELECT count(*) FROM "ob-poc".control_plane_shadow_decisions
WHERE decided_at > :deploy_marker_ts AND session_id NOT IN
(<known test-harness session ids, enumerated at marker time>)`.
(GRADPLAN-D-010, fixed 2026-07-13: the table's timestamp column is
`decided_at`, not `created_at` — the original predicate would fail
with a Postgres "column does not exist" error if run verbatim; see
`rust/migrations/20260710_control_plane_shadow_decisions.sql`.)

**Exit gate:** deployed; marker recorded; the predicate above is
runnable (returning 0 is fine — GW fills it).
**E-movement:** none directly; opens the window everything queues on.
**Session tier:** ops + architect. Not grind work.

---

### GW — Evidence-generation campaign [needs GM]

**Objective:** fill the runbook §1 window deliberately. In a
single-operator deployment the window does not fill itself: ≥500 real
decisions arrive because the operator drives them.

Work:
1. Define the exercise set: realistic end-to-end scenarios across the
   verbs Path A actually carries (the CBU/KYC journeys the demos
   exercise), written down as a campaign script and logged in the
   ledger as exercise-of-record — honest evidence, not synthetic
   fixtures; the deploy-marker predicate is what keeps the two
   distinguishable.
2. Run the campaign in sessions; after each, triage every divergence
   per runbook §7 (this is where the triage muscle gets built —
   the value option (b) wanted early arrives here instead).
3. Track progress via `shadow_divergence_stats` until
   `total_decisions >= 500 AND diverged == 0` (or all triaged).
4. Candidate-verb selection (A5, answerable at last): from the
   campaign's own per-verb distribution, pick the first graduation
   candidate — low-frequency, low-consequence, pinned as
   (verb, path-A) per AD-2(b)'s ratified keying.

**Exit gate:** `shadow_divergence_stats` meets the §1 criterion on
post-marker rows; candidate verb named in the ledger with its
distribution evidence.
**E-movement:** none directly; produces G7's preconditions.
**Session tier:** operator-driven; triage review is architect work.

---

### G7 — First graduation event [needs GW]

**Objective:** execute runbook §5 for the first verb, for real.

Preconditions (all hard):
- GW exit gate met (window complete on post-marker rows; candidate
  verb named with evidence).
- G1 landed (enforce-capable Path A) — without it, enforcement is an
  outage by construction (already guaranteed transitively via GM).
- Architect sign-off recorded in the ledger in the runbook §4 box-4
  form (the graduation sign-off event the ledger currently has no
  instance of, R:§A4).
- G3's design doc landed and G4 implemented it (AD-2(b) is ratified,
  but the (verb, path-tag) mechanism it specifies is what "pin this
  verb" concretely means — the decision alone does not build it).

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
| E3 | G1 (G10)† + G2 (G11/G12/G14) + G5 (matrix + B/C/D) | G2 detail; G5 status per amended definition |
| E4 | G6b/G6c (+ G6a for Row 3) | G6 |
| E5 | G0.1 | G0 |

† AD-1(a) is ratified (2026-07-13, GRADPLAN-D-005's conditioning is now
settled fact, not a shape assumption): G10's samples come from the
consume-seam recording G1 item 2 wires. This row does not need
re-deriving under (b)/(c) — those options are no longer live.

## 5. Risks and open items

- **Two of four design docs landed; two remain the critical path**
  (2026-07-13) — G1's seal→consume doc and G3's enforcement-dimension
  doc are both RATIFIED; G1 items 2-4 and G4's first line of code are
  unblocked. G2b's audit-stream doc and G2 item 4's provenance-dimension
  work remain the genuine bottleneck: G1 item 2 still names G2 item 4 as
  a dependency (the consume-seam recording rides that machinery), so
  "unblocked" for G1 item 2 is not yet "startable end-to-end" until G2
  item 4 lands too.
- **Window resets** — under AD-3(a) all Path-A shadow-semantics
  changes land pre-merge by construction, so reset risk exists only
  post-GM: G4/G5/G6 landing after GM must not touch Path A's call
  site (verified none do as drafted, GRADPLAN-D-009); a session that
  finds it must is a stop-and-review event, not a judgement call.
- **Campaign honesty (new, AD-3(a))** — GW's exercise runs are
  legitimate window evidence only if they are genuine full-stack
  operator dispatches, logged as exercise-of-record, and separated
  from test fixtures by the deploy-marker predicate. Scripting the
  scenarios is fine; scripting around the stack (direct DB writes,
  harness-driven inserts) would be gaming the plan's own gate and is
  named here so no session can do it innocently.
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
