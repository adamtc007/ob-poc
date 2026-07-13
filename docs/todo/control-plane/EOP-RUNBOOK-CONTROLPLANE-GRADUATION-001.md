# GRADUATION RUNBOOK — ob-poc-control-plane enforce-mode rollout
### EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001 v0.3
### Basis: EOP-PLAN-CONTROLPLANE-001 v0.1 §0 (shadow-first strategy) + docs/research/control-plane-ownership-ledger.md
### Status: DRAFT — precondition state verified against code 2026-07-10, §2/§3/§4/§8 corrections applied 2026-07-13; no path has graduated yet.

v0.2 (2026-07-10, architect review): added §1's graduation-window
definition (a gate's window resets when its shadow inputs go from
partial to full-pipeline coverage, or from the last CP-DEFECT fix,
whichever is later — the general form of Path A's Step 0 finding) and
§4 Path D's trust-boundary design note for T6.1a's rehydration path.

v0.3 (2026-07-13, EOP-IMPL-CONTROLPLANE-GRADUATION-G0-001 Slice 2 /
EOP-PLAN-CONTROLPLANE-GRADUATION-001 v0.3 G0 item 2): §2's readiness
table corrected — Path A now calls `execute_verb_admitting_envelope`
at `step_executor_bridge.rs:553` (commit `5a704f4e`, PIR-D-002), not
the stale `:474` bare `execute_verb`; gate count corrected from the
plan draft's "12/14" to **11/14** (G1-G9, G12 partial, G13 —
GRADPLAN-D-002). §3's Path A ordering note updated to match. §4 Path A
checklist box 1 flipped to DONE, citing `5a704f4e`. §8's T6.1a entry
re-scoped as a **two-repo change**: the real bus-invocation producer
is `bpmn-lite-engine::plan_walker::dispatch_callout`, a platform crate
ob-poc does not own (`EOP-RESEARCH-CONTROLPLANE-GRADUATION-001.md`
Q-block C) — the prior single-repo-scope claim and its
supporting paragraph are deleted as wrong. New §1 clause added
recalibrating "production evidence" for a single-operator deployment
(AD-3(a) resolution, `EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.3.md`).

---

## 0. What "graduate" means, mechanically

Every gate/path in this system is **shadow by default**. The only production
switch that exists is `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`
(`ob-poc/src/agent/control_plane_envelope_store.rs::EnforcedVerbs::from_env`),
a comma-separated verb-FQN allowlist, empty/unset in production. Read fresh
per call — no restart needed to flip.

For a verb FQN in that set, `ObPocVerbExecutor::admit()`
(`sem_os_runtime/verb_executor_adapter.rs:124`) requires a sealed, unconsumed
`ExecutionEnvelope` (via `control_plane_envelopes`, T4.2) to be presented
before dispatch; absent or already-consumed → reject. **Graduating a path
means adding its verbs to this set once its shadow evidence justifies it —
nothing else changes.** No code ships at graduation time; only config.

This runbook exists so a graduation decision is a checklist, not a judgement
call made under pressure by whoever is on call that day.

---

## 1. Graduation criterion (plan §0, unchanged, non-negotiable)

> A gate graduates to enforce mode only after **≥500 production evaluations
> with zero divergence** between shadow decision and legacy path outcome,
> **or** every divergence in that window triaged as a legacy defect (not a
> control-plane bug).

Measured via `agent::control_plane_metrics::shadow_divergence_stats`
(T7.2, live: `GET /api/control-plane/metrics` → `shadow_divergence`,
`shadow_divergence_rate`). `total_decisions >= 500 AND diverged == 0` is
the query that answers "is this gate ready" — run it, don't estimate it.

**A gate, not a path, is the unit of graduation.** A "path" (Sequencer,
REPL, workflow, bus) can have some gates ready and others not, because
shadow evidence is per-gate — see §2's per-path gate coverage table before
assuming "graduate Path A" means all 14 gates at once.

**The graduation window is defined against the pipeline that will actually
enforce, not against whatever telemetry happens to already exist.**
Concretely: `total_decisions >= 500` only counts decisions produced by the
*same* evaluation the enforce flip will make binding. Shadow data collected
while a call site wires only a subset of gates (Path A wired G1 alone at
first — v0.2 of this table — before the 2026-07-13 correction below moved
it to 11/14 real gates) is evidence about that subset alone; it says
nothing about the full G1-G14 decision a verb would be bound to once
enforced. Adding a gate's real inputs to
an already-shadow-collecting call site (the precondition work in §4)
**resets that gate's window to zero** — it does not inherit the prior
partial-coverage telemetry, even if the surrounding path has been
shadow-collecting for months. Formally: **a gate's graduation window is
measured from the later of (a) the commit that wired full-pipeline shadow
evaluation on that path, or (b) the last CP-DEFECT fix for that gate on
that path** — whichever is more recent. This is the general form of §4's
Path A Step 0 precondition; recording it here so the same trap (assuming
partial shadow coverage counts toward a full-pipeline enforce decision)
doesn't recur when Path B or Path C reach this stage.

**"Production evidence" for a single-operator deployment (added v0.3,
AD-3(a)).** This deployment has exactly one operator, and the operator
*is* the traffic — nothing accumulates in the window unattended, unlike a
multi-user service where ≥500 decisions arrive passively over calendar
time. Per `EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.3.md` (AD-3, tranches
GM/GW): the window opens exactly once, at tranche GM's single merge +
deploy (not at G0, and not incrementally per tranche), marked by a
deploy-time marker (timestamp + HEAD hash, recorded in the ledger and/or
a `control_plane_deploy_markers` row). "Real" decisions counted toward
this criterion means genuine operator-driven dispatches through the full
production stack post-marker, accumulated deliberately via a
ledger-logged exercise campaign (tranche GW) — not passive traffic (there
is none to wait for) and not synthetic test fixtures. The deploy marker
plus session-id exclusion (`created_at`/`decided_at` after the marker
timestamp, `session_id NOT IN` the known test-harness session ids
enumerated at marker time — see GM's exit-gate predicate) is what
structurally distinguishes an exercise-campaign row from a fixture row;
scripting the exercise *scenarios* is fine, scripting *around* the stack
(direct DB writes, harness-driven inserts standing in for real dispatches)
would be gaming this criterion and is explicitly out of bounds.

---

## 2. Current readiness state per path (verified against code, 2026-07-10)

| Path | Calls `execute_verb_admitting_envelope`? | Shadow evidence source | Gates with real (non-stub) shadow input at that call site |
|---|---|---|---|
| **A — Sequencer/runbook** | **Yes (since commit `5a704f4e`, PIR-D-002).** `runbook/step_executor_bridge.rs:553` calls `execute_verb_admitting_envelope` (with `envelope_id: None` — production-default empty `ENFORCE_VERBS` makes this behaviourally identical to the prior plain `execute_verb` call; zero dispatch-outcome change). | `sequencer.rs::phase5_runtime_recheck` → `control_plane_shadow.rs::build_evaluation_context` | **11/14 gates (G1-G9, G12 partial, G13).** G10 and G11 are stubs; G14 is not-applicable at this call site (v0.3 correction, GRADPLAN-D-002 — the plan draft's "12/14" figure miscounted the research's own gate table). |
| **B — REPL/direct (`RealDslExecutor`)** | **No — no admission mechanism reachable at all.** Runs through `dsl_v2::executor::DslExecutor::execute_plan`/`execute_plan_atomic_in_scope` (`repl/executor_bridge.rs`), a different, lower-level engine than `ObPocVerbExecutor`/`VerbExecutionPort` with no per-verb hook (T6.3 finding). | None. | None — not wired. |
| **C — Workflow-dispatched (`WorkflowDispatcher` direct branch)** | **No — same engine as B.** `bpmn_integration/dispatcher.rs`'s direct branch delegates to the same `dsl_v2::executor::DslExecutor` (T6.3 finding). | None. | None — not wired. |
| **D — Bus (`ObPocVerbAdapter`)** | **Yes (T6.1).** Calls `execute_verb_admitting_envelope` with `envelope_id: None` always — nothing issues bus callers an envelope yet (T6.1a). | None — bus has no shadow-decision call site at all yet. | None. |

**Reading this table**: no path is graduation-ready today. Path A is
*closest* — the `execute_verb` → `execute_verb_admitting_envelope` swap
(the exact one-line change T6.1 already proved out on bus) landed at
commit `5a704f4e`, and 11/14 gates now have real shadow input at that call
site; what remains for Path A is the seal→consume wiring (plan
`EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.3.md` tranche G1) so that an
enforced verb can actually consume a threaded envelope, plus the ≥500
real-decision window (§1, GM/GW). Paths B and C need materially more work
(instrumenting or rerouting the legacy `DslExecutor` engine — T6.3 declined
this as higher-risk than a single-session change). Path D needs T6.1a
(envelope handle threading through bpmn-lite process variables, a
two-repo change — see §8) before an envelope can ever be non-`None`
there.

---

## 3. Graduation order (per the architect's direction, 2026-07-10)

1. **Path A (Sequencer/runbook)** — richest shadow data (11/14 gates real
   per §2, corrected v0.3); the `execute_verb_admitting_envelope` wiring
   landed at commit `5a704f4e` (Step 0 is DONE — see §4 box 1). What
   remains is the seal→consume threading (plan tranche G1) before enforce
   mode is safe on any Path A verb, plus the window itself (§1).
2. **Path B (REPL/direct)** — after A.
3. **Path C (workflow-dispatched)** — after B.
4. **Path D (bus)** — last, gated on T6.1a landing `EnvelopeHandle`-through-
   process-variables threading (bus's admission call is otherwise
   permanently `envelope_id: None`, so it can shadow-collect but never
   meaningfully enforce until that lands).

---

## 4. Per-path preconditions (must all be true before starting §5's procedure)

### Path A
- [x] **DONE (commit `5a704f4e`, PIR-D-002).** `step_executor_bridge.rs:553`
      calls `execute_verb_admitting_envelope` instead of `execute_verb`
      (mirrors T6.1's bus change exactly — same risk profile: default-`None`
      envelope_id, `NotEnforced` while the env var is empty, zero behaviour
      change until a verb is added to the set).
- [ ] `control_plane_shadow.rs::build_evaluation_context` wired with real
      inputs for the specific gate being graduated (G1 already real; G3-G7
      need their own call-site wiring per gate before *those* gates can
      graduate — grep the ledger's T2/T3 "no production call site" notes
      for exactly which).
- [ ] `shadow_divergence_stats` (scoped to the gate + verb set in question)
      shows `total_decisions >= 500 AND diverged == 0` over the window, OR
      every divergence in the window has a linked triage note (§6) marking
      it a legacy defect.
- [ ] Reviewer sign-off recorded in the ledger row(s) for the C-0xx checks
      this gate subsumes.

### Path B / Path C
- [ ] Either (a) an admission hook is added inside
      `dsl_v2::executor::DslExecutor::execute_plan`/`execute_plan_atomic_in_scope`,
      or (b) `RealDslExecutor`/`WorkflowDispatcher`'s direct branch is
      rerouted through `ObPocVerbExecutor` instead. This is new engineering
      work, not a config flip — do not start §5 until one of these lands
      and has its own shadow-divergence evidence window.
- [ ] Same shadow-divergence criterion as Path A, once evidence exists.

### Path D (bus)
- [ ] T6.1a lands: `EnvelopeHandle` (or an equivalent opaque token) threads
      through bpmn-lite process variables from issuance to the point the
      process dispatches a verb invocation over the bus, so
      `ObPocVerbAdapter::execute` can pass a real `Some(envelope_id)`.
      **Mechanism question answered (2026-07-10, research, not yet
      implemented)**: bpmn-lite's `Value`/flags type
      (`bpmn-lite-types/src/types.rs:46-53` — `Bool | I64 | Str(interned-id)
      | Ref(index)`) cannot carry an arbitrary runtime string (`Str` is
      compile-time-interned, no runtime pool). The correct carrier is
      `ProcessInstance.domain_payload` (opaque canonical JSON,
      BLAKE3-hashed, never parsed by the VM) — a `String`-typed BPMN data
      object is compiler-routed to `DataObjectStorage::DomainPayload` by
      construction (`bpmn-lite-types/src/ffi_bindings.rs:19-58`), with
      existing end-to-end precedent (set at process start
      `bpmn-lite-engine/src/engine.rs:420-421`, read/written via dotted-path
      JSON accessors `bpmn-lite-vm/src/json_path.rs`). No size/format
      constraint beyond valid JSON. Remaining work: declare the
      `EnvelopeHandle` as a `String` data object, write it at process start
      from wherever the envelope is issued, read it back at the task that
      dispatches over the bus and pass it through to `ObPocVerbAdapter`.
      **CORRECTED (v0.3, 2026-07-13, `EOP-RESEARCH-CONTROLPLANE-GRADUATION-001.md`
      Q-block C): this is NOT scoped to ob-poc alone.** This note
      previously claimed the whole of T6.1a was containable within
      ob-poc, with nothing in the bpmn-lite platform crate needing to
      change — that was reasoned from the
      `ExecFfi`/`domain_payload`/`json_path` mechanism (real, and
      independently re-verified), but that mechanism does not feed the
      `InvocationRequest`s `bus_runtime.rs::ObPocVerbAdapter` actually
      admits. The real producer, traced this session, is
      `bpmn-lite-engine::plan_walker::dispatch_callout`
      (`plan_walker.rs:289-368`), which builds inputs via
      `build_inputs(static_args, placeholder_vals)` from
      `instance.placeholder_values` — a different `ProcessInstance` field
      from `domain_payload` — and never touches `domain_payload` at all.
      `dispatch_callout` sets `authority: None` and `snapshot_pin: None` on
      the constructed request; the proto (`dsl-bus-protocol/proto/
      dsl_bus.proto:137-153`) has unused `AuthorityContext authority=4` and
      `Uuid snapshot_pin=7` fields, but `plan_walker.rs` never populates
      `snapshot_pin` today. `plan_walker.rs` lives in the **bpmn-lite
      platform crate**, which ob-poc does not own (its own git repo; ob-poc
      does not track it as a submodule — CLAUDE.md). **This is therefore a
      two-repo change**: either (a) a new `ProcessInstance`/
      `placeholder_values`-sourced binding in `dispatch_callout` itself, or
      (b) populate the dormant `snapshot_pin` proto field end-to-end
      (`plan_walker.rs` → `dsl-bus-server::services.rs::InvocationContext`
      widening → `ob-poc-bus-handler`'s `VerbExecutor::execute` trait
      signature → `bus_runtime.rs`) — both require a bpmn-lite-side change,
      riding that repo's own review/versioning flow (standing rule 5, plan
      tranche G6a), not an ob-poc-only session.
      **Design note for whoever writes this (architect, 2026-07-10):** the
      handle riding in `domain_payload` is data crossing a trust
      boundary — a string in a process variable is not proof of anything
      on its own. Rehydration at the bus handler must treat it as **a
      claim to verify**, not an authorization: full lookup against
      `control_plane_envelopes`, content-hash check, single-use check,
      validity-window check, and pre-state-pin check (§6.10.4 of the V&S),
      exactly as any other envelope-handle rehydration path would. A
      forged or replayed string in that field must void loudly (routed to
      exception handling, not silently accepted) — this failure path
      needs its own dedicated test alongside the happy-path threading
      test when T6.1a is implemented.
- [ ] A shadow-decision call site is added for the bus path (currently
      none exists — bus shadow-collects nothing today).
- [ ] Same divergence criterion as Path A/B/C, once evidence exists — and
      per §1's graduation-window rule, that window starts when this
      shadow-decision call site goes live, not before.

---

## 5. Graduation procedure (once all of §4's boxes are checked for a gate/path/verb-set)

1. Freeze the exact verb-FQN set being graduated (e.g. `cbu.confirm`) —
   never graduate `*` in one move.
2. Re-run the divergence query (§1) immediately before the flip — evidence
   can go stale between planning and execution.
3. Set `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` to include the new verb(s),
   additive to whatever's already enforced (comma-separated).
4. Watch `shadow_divergence_stats` AND application error rates for the
   enforced verb(s) for a defined soak window (suggest 24h minimum,
   matching an ordinary deploy soak — this plan doesn't specify one, so
   pick conservatively and record the choice here once decided).
5. Record the graduation in the ownership ledger: which C-0xx rows this
   flip closes (moves from "mechanism proven" to "enforced in production"),
   dated, with the divergence evidence window cited.

---

## 6. Rollback trigger and procedure

**Trigger**: any of —
- A rejected dispatch traced to a control-plane false-positive (the gate
  blocked something that should have been allowed) rather than a genuine
  legacy defect the gate correctly caught.
- Divergence rate for the enforced verb(s) goes non-zero post-graduation
  (shadow comparison should still run under enforce — if it stops, that's
  its own defect to fix before continuing).
- Any production incident where an enforced verb's users report
  unexpected rejection.

**Procedure**: remove the verb FQN from `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`
(same mechanism as graduation, in reverse — no code deploy needed, no data
migration, `admit()` immediately returns `NotEnforced` again for that verb).
Record the rollback in the ledger with the trigger and root cause once
known. Re-graduation requires re-satisfying §4's preconditions from
scratch, not just re-flipping the flag.

---

## 7. Divergence triage classification (§4/§1's "or every divergence
triaged" clause)

Every divergence recorded by `control_plane_shadow_decisions.diverged =
true` gets classified as exactly one of:

- **Control-plane bug**: the shadow gate's decision is wrong relative to
  the intended policy (a gap between what the gate *should* grade and what
  it *does* grade, per the gate's own adapter doc comment). Fix in
  `ob-poc-control-plane`, re-open the evidence window.
- **Legacy defect**: the shadow gate is correct and the *legacy* path's
  outcome was wrong (the new gate caught something the old check-scatter
  missed or was inconsistent about). Record which C-0xx row this proves
  was under-enforcing; the divergence counts as "handled" for graduation
  purposes per §1, but the underlying legacy gap should still get its own
  fix ticket independent of this plan.
- **Ambiguous / needs architect input**: neither is clear from the trace
  alone. Does not count toward the "zero divergence" bar until resolved —
  block graduation on it rather than assume either direction.

No triage classification is currently automated; this is a manual review
of `control_plane_shadow_decisions` rows filtered `WHERE diverged`, cross-
referenced against the specific gate's `gate_results` entry and whatever
legacy check it's being compared to.

---

## 8. Open items this runbook depends on (not yet resolved)

- **T6.1a** (bus envelope threading) — **re-scoped v0.3 (2026-07-13,
  `EOP-RESEARCH-CONTROLPLANE-GRADUATION-001.md` Q-block C)**: the real
  bus-invocation producer is `bpmn-lite-engine::plan_walker::
  dispatch_callout`, in the bpmn-lite platform crate ob-poc does not own —
  see §4 Path D's corrected design note for the two carrier options and why
  the earlier "ob-poc-only" framing (reasoned from the unrelated
  `domain_payload`/`ExecFfi` mechanism) was wrong. Implementation is a
  two-repo change (plan tranche G6a), not yet done.
- ~~**EOP-VS-CONTROLPLANE-001 v0.3** (missing V&S source doc)~~ — RESOLVED
  (T9.4, EOP-PLAN-CONTROLPLANE-001 Addendum B): the document was in the
  repo since the T7 re-evidence session, just at
  `docs/todo/control-plane/`; `git mv`'d to
  `docs/architecture/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.3.md`. The
  NIST crosswalk / "conformance is a property of the execution path"
  closing claim can now be walked against the actual document at its
  permanent location.
- **Soak window length** (§5 step 4) — placeholder value, not yet decided.
- **Path B/C admission-hook design** (§4) — not yet designed, larger than
  a config flip; needs its own short plan before Path B graduation can
  even begin queuing evidence.
- **G1-only state at Path A (§2/§3) is still accurate as of this line**,
  but the path off it is now dependency-ordered, not five independent
  sub-tranches. EOP-PLAN-CONTROLPLANE-001 Addendum B's T9.1c/T9.1d wiring
  landed (`150831b3`, real `AuthorityInput`/`EvidenceInput`), but an
  empirical probe against `evaluate_shadow()` showed both gates still
  report `NotEvaluated` — they transitively depend on G3 PackResolution,
  which depends on G2 EntityBinding, which nothing at the shadow call site
  supplies at all. T9.1's original "any order" framing is REVOKED (it
  contradicted its own basis document, V&S §6.16.1's dependency table);
  see the ownership ledger's "Tranche T9.1c/T9.1d" entry for the amended
  six-tranche dependency-ordered sequence, starting with the previously
  omitted G2 (T9.1-pre). §2/§3 of this runbook will need a follow-up
  update once T9.1-pre/T9.1a land and Path A's real gate coverage moves
  past G1.
