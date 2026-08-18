# EOP-PLAN-GAMEBOARD-001 — R2 Stage 2 Design (report-only → enforce cutover)

> **Status:** DESIGN ONLY. Nothing in this document is wired to live dispatch.
> Stage 1 (report-only census, `src/cross_slot_census.rs`) remains the only
> code that touches `cross_slot_constraints` at runtime, and it is called
> from nowhere in the dispatch path (proven by
> `nothing_outside_this_module_calls_evaluate_all` /
> `cross_slot_constraints_have_a_report_only_evaluator_not_yet_wired_to_dispatch`).
> Activating any part of this design is a separate, explicit decision per
> rule — see Gate 4 below.

## 0. Why this can't be "flip a global switch"

The Stage 1 census (run live, 2026-08-18) makes a blanket cutover
irresponsible:

- Of 49 declared `cross_slot_constraints`, only 6 have a real hand-verified
  check at all; 41 are still `NotYetImplemented`, 2 are `SchemaMismatch`
  (reference a column/join that doesn't resolve against the live schema —
  see the census module's own doc comments for
  `investor_active_requires_kyc_approved`/`holding_active_requires_investor_active`).
- Of the 3 rules with the longest-standing real checks,
  **`cbu_validated_requires_evidence_set_verified` and
  `cbu_validated_requires_commercial_client_entity` are currently violated
  by 100% of live `VALIDATED` CBUs** (93/93 and 78/93 respectively) —
  enforcing either at dispatch today would not "catch drift," it would halt
  the entire `cbu.confirm` pathway as currently practiced.
- `deal_contracted_requires_bac_approved` and
  `case_cannot_approve_without_workstreams_complete` are on much smaller,
  cleaner cohorts (2 and 5 violations) but still non-zero.
- `case_cannot_approve_with_unresolved_red_flags` and
  `case_cannot_approve_with_unresolved_screening_hits` are the only two
  rules currently `Clean` against live data — the only two that could be
  enforced today without touching a single existing row.

The design below therefore treats enforcement as **per-rule, opt-in, and
gated on a live-clean census run** — never a single flag that activates all
49 (or even all 6 implemented) at once.

## 1. Reused precedent: `control_plane_shadow.rs`

The Control-Plane Graduation program already solved "run a new evaluator
alongside the real dispatch gate, log divergence, never block" for G1–G4
(`src/agent/control_plane_shadow.rs`, `T2.7`). Its shape:

- Shadow evaluation happens at the same point the real gate
  (`GateChecker::check_transition`, via `pre_dispatch_gate_check`) already
  runs, reusing that gate's own resolution (`resolve_transition_probe`) —
  not a second independent derivation.
- The shadow verdict is persisted to a dedicated table
  (`"ob-poc".control_plane_shadow_decisions`) via a **best-effort insert**
  — failure to persist is logged, never propagated, never blocks dispatch.
- The shadow module has zero authority: nothing reads its verdict to
  decide whether to allow or refuse the real transition.

Stage 2 reuses this exact shape for `cross_slot_constraints` rather than
inventing a second shadow mechanism. Concretely:

- New table `"ob-poc".cross_slot_constraint_shadow_decisions` (rule_id,
  workspace, entity_id, verdict, detail, observed_at, dispatch_context —
  same column shape as `control_plane_shadow_decisions`, adjusted for
  per-entity rather than per-verb-dispatch granularity since a single
  constraint touches N entities per evaluation, not one).
- New function `cross_slot_census::shadow_check_at_dispatch(rule_ids: &[&str], pool: &PgPool) -> ...`
  — calls `evaluate_one` for exactly the rules passed in (never all 49),
  persists best-effort, returns nothing that gates anything. Call site:
  same place `pre_dispatch_gate_check` already runs, immediately after
  (not instead of) the real `GateChecker` call, so it observes the same
  transition attempt without altering its outcome.
- `rule_ids` passed in per verb dispatch, not global — e.g. `cbu.confirm`
  passes `["cbu_validated_requires_evidence_set_verified",
  "cbu_validated_requires_commercial_client_entity"]`, `kyc-case.approve`
  passes the 3 case-approval cluster rule ids. This mapping (verb FQN →
  relevant rule ids) doesn't exist anywhere yet — it must be authored
  explicitly, not derived, since `CrossSlotConstraint` carries no
  transition-addressing fields (confirmed in the census module's own doc:
  "unlike `cross_workspace_constraints`, `CrossSlotConstraint` carries no
  target-transition addressing to index by").

## 2. From shadow to enforce: per-rule promotion gate

A rule may move from **shadow** (Stage 2a, logs only) to **enforce**
(Stage 2b, `GateChecker`-equivalent — returns a real refusal) only when
**all** of the following hold, checked and recorded per rule, not assumed:

1. **Real check exists** — not `NotYetImplemented`/`SchemaMismatch`.
2. **Zero live violations** at the moment of promotion, confirmed by a
   fresh `evaluate_one` run against production, not a stale census.
   (`case_cannot_approve_with_unresolved_red_flags` and
   `case_cannot_approve_with_unresolved_screening_hits` clear this bar
   today; nothing else does.)
3. **Verb → rule mapping authored** (§1) so the enforce check only runs on
   the actual transition(s) the rule concerns, not every dispatch.
4. **Explicit sign-off recorded** (this is a business/product decision for
   rules like the evidence/commercial-entity ones, not a mechanical
   check — e.g. "is the 100%-violation rate because the rule is right and
   the workflow is missing a step, or because the rule doesn't match how
   CBUs are actually validated" is exactly the kind of question this
   design deliberately does not answer on the codebase's behalf).

Rules failing #2 need either a backfill (fix the existing violating rows)
or a rule-text correction (if the rule itself is wrong) before they can
ever reach #4 — Stage 2 doesn't pick between those for them.

## 3. What Stage 2 does NOT do

- Does not touch the 41 `NotYetImplemented` rules — those need individual
  hand-translation (same as the 3 KYC case-approval rules added
  2026-08-18) before they're even shadow-eligible.
- Does not touch the 2 `SchemaMismatch` rules — both need a ruling on what
  "investor active" should mean against the live `investor_entity_id`
  join shape (see the census module's own doc comments) before any rule
  text change.
- Does not activate enforcement for any rule. Stage 2a (shadow) is itself
  gated on this design being reviewed, not self-authorizing.
- Does not invent a generic rule-text interpreter — `CrossSlotConstraint.rule`
  stays hand-translated per rule (§1 fact 2 of the parent plan), same
  discipline as Stage 1.

## 4. Activation checklist (for whoever flips Stage 2a on for a given rule)

- [ ] Confirm `pre_dispatch_gate_check`'s call site is still the single
      chokepoint (re-check R0 P1's bypass trace hasn't been invalidated).
- [ ] Author the verb → rule_id mapping for the specific verb(s) this rule
      concerns.
- [ ] Add the shadow-persistence table via a normal forward migration.
- [ ] Wire `shadow_check_at_dispatch` immediately after (not replacing) the
      real `GateChecker` call for those verbs only.
- [ ] Run for an agreed observation window; review
      `cross_slot_constraint_shadow_decisions` for false positives before
      considering Stage 2b (enforce) for that rule.
