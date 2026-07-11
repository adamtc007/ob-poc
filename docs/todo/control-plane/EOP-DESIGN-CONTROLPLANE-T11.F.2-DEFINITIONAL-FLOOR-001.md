# EOP-DESIGN-CONTROLPLANE-T11.F.2 — The Definitional Floor: Rejection Design

### Basis: EOP-PLAN-CONTROLPLANE-002 v0.1, Tranche T11.F ("hard stop; runs immediately after T11.0")
### Status: DRAFT for architect review. No code changes ride with this document, per the requested sequencing ("Sonnet writes the design doc... sends it here for review. That's the single next action.").
### Contains one finding that changes the floor's scope from what the plan text literally says — flagged prominently in §2, not buried.

---

## 0. Summary of the finding that reshapes this design

T11.F's own text names the floor as "G1 (intent admission vs. the DSL taxonomy), G3 (pack resolution), G4 (DAG legality)" — implying each gate, as a whole, enforces unconditionally. **Investigating each gate's actual `decide()` logic (not assumed from the plan's one-line description) found that all three gates conflate a genuinely definitional outcome with genuinely judgmental outcomes inside the same enum, and G1 specifically has a dead-code bug that makes the definitional/judgmental split currently unrecoverable from its own decision function.** This is why §2 below is the load-bearing section of this document — the floor cannot safely be "G1/G3/G4 as coded," only a precisely-scoped subset of each.

---

## 1. Rejection shape per ingress

Four production ingress points reach dispatch today (per the ownership ledger's T9.3 entry, re-verified this pass):

| Ingress | Entry point | Current admission mechanism | Floor insertion point (proposed) |
|---|---|---|---|
| **Path A** — REPL/runbook, primary | `RealDslExecutor::execute()`/`execute_in_scope()` (`src/repl/executor_bridge.rs`) → `admit_plan()` (plan-level pre-flight) → step dispatch → `phase5_runtime_recheck` (per-step shadow eval, async/post-hoc) | `admit_plan_checked` (enforcement-only, keyed by `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`) + async shadow observation, never blocking | **Both**: fast-path registration check at `admit_plan_checked` (plan-level, cheap); full G3/G4 floor check must move from `phase5_runtime_recheck`'s current async/post-hoc position to a synchronous pre-dispatch call (see §1.1) |
| **Path B/C** — Direct REPL / `WorkflowDispatcher` Direct branch | Same `RealDslExecutor.execute()`/`execute_v2()` per T9.3's fix — resolves through Path A's mechanism | Same as Path A | Same as Path A (no separate insertion needed — T9.3 already unified these) |
| **Path D — "the bus"** — durable/BPMN-dispatched steps | `VerbExecutionPortStepExecutor::execute_step` (`src/runbook/step_executor_bridge.rs:513`) → `execute_verb_admitting_envelope(&step.verb, args, &mut ctx, None)` | Per-step envelope admission (T9.2's atomic-admission scope), no plan-level pre-flight equivalent | Registration fast-path + G3/G4 floor check inside `execute_verb_admitting_envelope`'s scope, before `execute_verb_in_open_scope` dispatches (mirrors T10.2's pin-verification insertion point — same scope, same "before branch dispatch" ordering) |
| **MCP `dsl_execute`** | `ToolName::DslExecute` (`src/mcp/handlers/core.rs:723`) → `admit_plan()` before `build_dsl_executor()`/execute | Same plan-level pre-flight as Path A | Same as Path A |
| **KNOWN-BYPASS** — `dsl_v2/executor.rs`'s `dispatch_plugin_via_sem_os_op_in_scope` (T6.3 gap) | Not admission-wired at all, CI-tracked as a named exception (`audits/surface/_verb-execution-context-allowlist.txt`) | None | **Out of scope for T11.F.2** — the floor can only cover what the admission port already reaches; this path reaches neither today. Not silently left uncovered: T11.F.3's own text says floor coverage "rides T11's coverage arc," so this bypass staying uncovered is consistent with the plan, not a gap this document invents a fix for. |

**Rejection response shape**, uniform across all covered ingresses: a floor rejection returns the same shape a legacy validation failure already returns at that ingress today (`Err(...)` propagated to the caller — a REST `4xx`, an MCP tool error, a BPMN step failure) — **not a new error type**. The floor's job is to make an already-possible failure mode (this dispatch cannot proceed) trigger reliably and unconditionally for the three definitional reasons; it does not invent a new response contract callers must learn. What's new is *why* the rejection fires (deterministically, on a dictionary-lookup fact) and that it fires *before* any write, not after a partial one.

### 1.1 The synchronous-vs-async ordering problem at Path A

`phase5_runtime_recheck` (Path A's real G1/G3/G4-evaluating call site) is currently invoked, and its shadow result persisted, via `tokio::spawn` — genuinely after the calling turn's own dispatch decision is already made (confirmed this session, MCA-002's C3 finding). Making the floor block Path A therefore cannot mean "make `phase5_runtime_recheck` synchronous" as a blanket change — that call site evaluates *all* gates (including the judgmental ones, which must stay shadow-first) in one pass. **Proposed: a new, narrower synchronous pre-check** — call it `floor_check` — that runs only the three definitional sub-checks (§2) before Path A's step dispatch, structurally separate from `phase5_runtime_recheck`'s full shadow evaluation (which keeps running exactly as it does today, unchanged, for the judgmental gates' continued shadow observation). This avoids retrofitting blocking behaviour onto a function whose entire existing contract (and every caller's assumption) is "never blocks."

---

## 2. Registration-gap fast path — the conflation finding, in detail

### G1 (Intent Admission): a real, previously-undiscovered dead-code bug

`intent_admission.rs::decide()` branches on `input.exclusion_reasons` containing the literal strings `"unknown_intent"`, `"outside_pack"`, or `"deprecated"` to select between `RejectedUnknownIntent`/`RejectedOutsidePack`/`RejectedDeprecated`, falling through to `RejectedUnauthorisedSurface` otherwise. But `exclusion_reasons` is built (`control_plane_shadow.rs:534-539`) from `Debug`-formatting `PruneReason` — whose four real variants are `AbacDenied {...}`, `EntityKindMismatch {...}`, `AgentModeBlocked {...}`, `PolicyDenied {...}`. **None of these four Debug strings ever equal `"unknown_intent"`, `"outside_pack"`, or `"deprecated"`.** Every real production exclusion today falls through to `RejectedUnauthorisedSurface`, regardless of whether the true cause was "this verb doesn't exist" or "this verb exists but ABAC denies this actor." **G1's `decide()` function, as coded, cannot currently distinguish a definitional failure from a judgmental one.** This is not a hypothetical edge case — it is the only path the function has, confirmed by reading both sides of the string comparison against their actual producers, not assumed from either module's own doc comments.

**Consequence for the floor:** the floor cannot be "G1's existing gate, made blocking." Doing so today would make `AbacDenied`/`PolicyDenied` exclusions — genuinely judgmental, policy-driven denials — hard-reject unconditionally, exactly the outcome T11.F's own text says must NOT happen ("Judgmental gates... remain shadow-first/graduated... divergence data is meaningful for them; it is not for the floor").

**Proposed fix, the "registration-gap fast path" itself:** bypass G1's `decide()` for the floor's purposes entirely. Use a direct, cheap, synchronous lookup — `runtime_registry().get(domain, verb).is_some()` (`crate::dsl_v2::runtime_registry`, already the production dispatch-routing lookup, already proven fast/synchronous at the real dispatch site per this session's own T9.5 work) — as the floor's G1 check: **does this verb_fqn exist in the runtime verb registry at all?** This is genuinely definitional (a dictionary-membership fact, structurally identical to "is this word in the language"), requires no `SessionVerbSurface`/ABAC/pack context, and is honestly distinct from every one of G1's judgmental exclusion reasons. G1's existing gate (and its string-matching bug) is untouched by this design — it continues shadow-observing exactly as today; **fixing the dead code is out of scope for T11.F.2** (a separate, small, correctness-only fix, flagged as follow-on work in §5) and not required for the floor to be correct, since the floor doesn't route through `decide()` at all.

### G3 (Pack Resolution): same pattern, less severe

`pack_resolution.rs::decide()`'s `MissingPack`/`AmbiguousPack` outcomes (no pack candidate / more than one) are genuinely definitional — a structural fact about the world (SemOS pack-resolution state), not a policy judgment. `PackDeniesIntent`/`PackDeniesEntity` are pack-authored business rules — judgmental. Unlike G1, there is no string-matching bug here; the four outcomes are cleanly produced by real, distinct branches (`semreg_allowed_set_available`, `constraint_denies_intent`, and the candidate-count match). **Floor scope: `MissingPack` and `AmbiguousPack` only.** `PackDeniesIntent`/`PackDeniesEntity` stay judgmental/shadow.

### G4 (DAG Legality): same pattern, needs one more precondition traced before commit

`dag_proof.rs::decide()`'s `IllegalFromState`/`IllegalToState`/`Unreachable`/`WrongLifecycleAxis`/`TransitionUnimplemented` are topology facts about the DAG (does this transition exist in the graph at all) — definitional. `GuardFailed { reason }` is produced by two different upstream conditions bundled into one variant: `blocking_violations` (transition-slot-scoped structural preconditions — arguably part of "is this transition legal in this DAG," i.e. definitional) and `lifecycle_fail_open_class`/`lifecycle_gate_mode_fail_closed` (an explicitly policy-flavored fail-open/fail-closed mode setting — judgmental). **This split needs one more trace before T11.F.2 implementation** — specifically, whether `blocking_violations` can ever contain a judgmental (non-topological) reason in practice (its producer, `GateChecker::check_transition`, wasn't re-read this pass). Flagged, not resolved here — recommend either (a) tracing `blocking_violations`' producer before committing to "GuardFailed via blocking_violations is floor-eligible," or (b) the conservative default: treat the entire `GuardFailed` variant as judgmental for T11.F.2 (only `IllegalFromState`/`IllegalToState`/`Unreachable`/`WrongLifecycleAxis`/`TransitionUnimplemented` are floor), revisiting once (a) is done. This document recommends (b) — safer default, narrower floor, no risk of a policy check accidentally hard-rejecting.

### Summary: the floor's real, precise scope

| Gate | Floor-eligible outcomes | Stays judgmental/shadow |
|---|---|---|
| G1 | A verb_fqn absent from `runtime_registry()` (new fast-path check, bypasses `decide()`'s buggy string-match entirely) | `RejectedOutsidePack`/`RejectedDeprecated`/`RejectedUnauthorisedSurface`/`RejectedAttestationInsufficient` (everything `decide()` currently actually produces) |
| G3 | `MissingPack`, `AmbiguousPack` | `PackDeniesIntent`, `PackDeniesEntity` |
| G4 | `IllegalFromState`, `IllegalToState`, `Unreachable`, `WrongLifecycleAxis`, `TransitionUnimplemented` | `GuardFailed` (conservative default — see above) |

---

## 3. §6.13 exception routing + queue ownership

**No exception-routing or work-item infrastructure exists anywhere in the codebase today** — grepped for `exception_routing`/`ExceptionRouting`/`controlled_work_item`/`work_item`: zero hits. §6.13's full exception-class taxonomy (16 classes, including several this floor doesn't touch — stale state, envelope replay, write-set breach, lock conflict) is entirely unbuilt. This document does not propose building general §6.13 infrastructure — that is out of T11.F's scope (T11.F names only "route to exception handling as controlled work items," not "build the work-item system"). **Minimal viable routing for the floor specifically:**

- A floor rejection is NOT queued anywhere new. It is returned synchronously to the caller at the ingress point (§1's table) — the same "the turn fails, the caller sees why" shape every existing validation failure already has. There is no queue to own because there is no queue.
- The "controlled work item, not an uncontrolled failure" requirement (§6.13's own text) is satisfied by §4's audit record below, not by a queue: the rejection is durably recorded with enough structure (which gate, which floor-eligible outcome, which verb/entity/transition) that an operator can later query "how many floor rejections, of what kind, on what verb families" — the same shape `sealable_rate_by_verb`/`gate_outcome_counts` already give for shadow data, extended to cover real (not merely shadow) rejections.
- **If/when general §6.13 infrastructure is built** (a future, separate tranche — not proposed here), the floor's rejection path should emit into it rather than return synchronously — but building that speculatively now, for a floor whose entire premise is "definitional failures should never happen on real traffic" (T11.F.1's own text), would be infrastructure sized for a volume this floor doesn't expect to generate. Recommend: ship the synchronous-rejection-plus-audit-record version now; revisit only if T11.F.1-style evidence after deployment shows real volume needing a queue.

---

## 4. Audit record

**Reuse `"ob-poc".control_plane_shadow_decisions`, do not create a new table** — its shape (`session_id`, `entry_id`, `verb_fqn`, `gate_results` JSONB, `legacy_outcome_blocked`, `shadow_intent_admission_blocked`, `diverged`) already carries everything a floor-rejection record needs; adding a fourth boolean column, `floor_rejected: bool` (default `false`), distinguishes a real floor-triggered hard rejection from a shadow-only observation in the same table, same query surface, same existing metrics functions extended rather than duplicated.

**The one required behavioural change to persistence, not merely a new column:** today, `insert_shadow_decision` is explicitly best-effort/fire-and-forget (T10.1's own documented posture, re-affirmed by v0.4.1's C3 ruling as conformant for shadow observation). **A floor rejection's audit record cannot inherit that posture** — §12's own closing sentence ("a rejected AI action should be a normal, auditable control outcome, not an exception to the architecture") requires the rejection and its record to be atomic, or at minimum the caller must know if the audit write failed (rather than silently proceeding to reject the user with no durable trace). Proposed: the floor-rejection path calls a new, narrower `insert_floor_rejection` function that is **not** best-effort — its failure is logged AND propagated as a warning attached to the rejection response (the user still sees "rejected: unknown verb," but a 500-class signal or structured log alert fires separately if the audit write itself failed), rather than either (a) blocking the rejection on a successful audit write (would make a DB hiccup turn a correct rejection into a hung request) or (b) silently swallowing the audit failure (would violate §12's own auditability requirement for exactly the case that requirement exists to protect).

---

## 5. Fault-injection matrix (acceptance criteria for T11.F.2's implementation)

| # | Floor gate | Scenario | Path | Expected outcome |
|---|---|---|---|---|
| 1 | G1 (registry-absence) | Dispatch a verb_fqn that does not exist in `runtime_registry()` (e.g. `"nonexistent.verb"`) | Path A | Hard rejection before any write; `control_plane_shadow_decisions` row with `floor_rejected=true`, audit record readable |
| 2 | G1 (registry-absence) | Same, via the bus (`VerbExecutionPortStepExecutor::execute_step`) | Path D | Same — hard rejection, audit record, BPMN step fails cleanly (not a worker crash) |
| 3 | G3 (`MissingPack`) | Dispatch a verb whose entity resolves to no active SemOS pack | Path A | Hard rejection; audit record distinguishes G3/MissingPack from G1 |
| 4 | G3 (`AmbiguousPack`) | Dispatch where entity resolution yields >1 candidate pack | Path A | Hard rejection |
| 5 | G4 (illegal transition) | Dispatch a transition not present in the entity's DAG (e.g. `VALIDATED → VALIDATION_PENDING` reversal where no such edge exists) | Path A | Hard rejection |
| 6 | Negative control — G1 judgmental | Dispatch a verb that exists in the registry but is ABAC-denied for the acting principal | Path A | **NOT** hard-rejected by the floor (must still flow through the existing judgmental/shadow path unchanged — this is the test that proves the floor didn't accidentally widen to cover `RejectedUnauthorisedSurface`) |
| 7 | Negative control — G3 judgmental | Dispatch where the resolved pack explicitly denies this intent (`PackDeniesIntent`) | Path A | **NOT** hard-rejected by the floor |
| 8 | Negative control — legitimate traffic | Every existing green integration/live-DB test in the current suite | Path A, Path D | Zero new failures — the floor must not reject anything that passes today (this is the regression backstop; T11.F.1's evidence check already showed zero real G1/G3/G4 failures on the (synthetic, session-local) accumulated data, so a regression here would be a real defect in the floor's own implementation, not an expected side effect) |
| 9 | Grep gate | `grep` across the floor's implementation for any `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`/env-var/feature-flag conditional guarding the floor's rejection branch | n/a (static check) | Zero hits — the floor's own exit criterion, "structurally independent of enforce-mode flags," proven by absence, not by testing every flag state |
| 10 | Audit durability | Force the audit-record insert to fail (e.g. a constraint violation or connection drop) during a real floor rejection | Path A | The user still receives the rejection (not a hung request); a separate warning/alert signal fires; no silent swallow |

---

## 6. Open items carried forward, not resolved by this document

- **G4's `GuardFailed`/`blocking_violations` producer trace** (§2) — needed before implementation locks in the conservative default, or to widen the floor if the trace shows `blocking_violations` is purely topological.
- **G1's dead-code string-match bug** (§2) is a real, standalone defect independent of T11.F.2 — recommend a small, separate fix ticket (not bundled into this tranche, since the floor's fast-path design deliberately routes around it rather than depending on it being fixed).
- **The two small ledger lines requested alongside this document** (synthetic-corpus caveat on T11.F.1, no graduation window genuinely started) are recorded in the ownership ledger, not repeated here — see the ledger entry accompanying this document's commit.

---

## 7. What this document does NOT do

No code changes ride with this document. `floor_check`, `insert_floor_rejection`, the `floor_rejected` column, and every fault-injection test in §5 are proposed, not implemented — implementation is explicitly queued behind "clean review" per the requested sequencing.
