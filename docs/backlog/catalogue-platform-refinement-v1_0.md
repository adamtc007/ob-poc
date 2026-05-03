# Catalogue Platform Refinement — Vision & Scope (v1.0)

> **Status:** Final. Approved as architecture specification.
> **Date:** 2026-04-18
> **Prior versions:** v0.1 through v0.9 (see revision history in companion log).
> **v1.0 changes from v0.9:** two final edits. (1) Resolved an internal contradiction around P-G and Tranche 1 completion — the strict position is adopted: P-G (named authority for tier-assignment governance) is part of Tranche 1 DoD item 11 and §11 stopping points, so Tranche 1 is not complete until P-G is resolved. The §13 escape hatch ("Tranche 1 can still exit with the other eight artefacts") is removed. (2) Effective-tier pseudocode in P11 now explicitly handles the empty-escalation-rules case.
> **Decision level:** final architectural specification.

---

## 1. Purpose

Refine the ob-poc platform by establishing a formally reconciled verb-DAG model, a DB-free catalogue-mode in SemOS, and a governed authorship mechanism that prevents drift. Define the consequence-tier mechanism that enables principled Sage / REPL collaboration, with proper handling of context-dependent consequence and runbook-level composition across any runbook shape.

Standalone platform enhancement producing lasting capability:

- A catalogue where every verb's semantics are declaratively expressed across three axes: state effect, external effects, and consequence severity (with monotonic escalation rules).
- A formal distinction between semantic declarations (structural, mechanically-validatable) and policy declarations (governance, judgement-based).
- A mechanical validator enforcing structural consistency, with conservative warnings for policy sanity.
- SemOS startup path validating catalogue correctness before database connection.
- Reconciled catalogue and DAG model across the four primary workspaces.
- A uniform runbook composition model computing effective consequence tier for any runbook, whether macro-produced or ad-hoc.
- Governed authorship mechanism making drift architecturally impossible.
- Principled Sage / REPL collaboration grounded in tier-aware autonomy policy honouring escalation and composition.

---

## 2. Vision

One sentence:

> **Establish a three-axis declarative semantic model (state effect, external effects, consequence tier with monotonic escalation) plus a uniform runbook-level tier composition rule, with a validator enforcing structural consistency and a governance review pass handling tier-assignment judgement; reconcile the existing catalogue through this mechanism; build the Catalogue workspace as the permanent governance layer; and use the effective (context-escalated, runbook-composed) consequence tier to define bounded Sage autonomy and tier-aware REPL confirmation — delivered in three tranches, each with standalone value.**

Expanded:

- **Tranche 1 — Semantic model and validator.** Three-axis schema including escalation rules; validator; DB-free catalogue-load; declared DAG taxonomies; runbook composition spec; xtask client; Sage / REPL policy documented; governance process for tier-assignment review documented **including named authority**.
- **Tranche 2 — Estate reconciliation.** 1,500-verb audit. Every verb declares all three axes with escalation rules where context-dependent. Orphans resolved. Tier distribution reviewed as a first-class governance judgement pass. Runtime triage.
- **Tranche 3 — Governed authorship mechanism.** Catalogue workspace, authorship verbs, macros (evidence-based), access control, Sage integration honouring effective tier, REPL tier-aware confirmation, Observatory UI, forward discipline.

---

## 3. Overarching principles

### P1 — Declarative semantic model (three axes)

Every verb declares:
- **State effect:** `transition` | `preserving`.
- **External effects:** set of {`observational`, `emitting`, `navigating`}.
- **Consequence tier:** one of `benign` | `reviewable` | `requires_confirmation` | `requires_explicit_authorisation`, with optional escalation rules.

Plus precondition (always declared). No arbitrary predicates.

### P2 — Mechanical validation for structural declarations

Catalogue-vs-DAG structural consistency is verified by a mechanical validator. Structural checks are enforced; policy sanity warnings are conservative (see §6.2 and P13). [Tranche 1]

### P3 — DB-free catalogue-mode

Catalogue operations run without DB connectivity. Catalogue-load validation runs before SemOS establishes its DB pool. [Tranche 1]

### P4 — Verb declarations are the authoritative semantic source

The catalogue is canonical; declared DAG taxonomy is a governed projection artefact.

### P5 — Reconciliation is directional

Phase A (DAG-side sweep) → Phase B (orphan classification) → Phase C (stability). [Tranche 2]

### P6 — Orphan verbs drive DAG creation where clearly implied

Missing DAG structure revealed by orphan verbs is created during reconciliation, where the orphan set clearly implies the structure.

### P7 — Runtime alignment is semantic, not comprehensive

Alignment at the state-transition semantic layer; incidental divergences queue for follow-up. [Tranche 2]

### P8 — Governance is by architectural enforcement

After Tranche 3, the only catalogue-modification path is the Catalogue workspace. [Tranche 3]

### P9 — The Catalogue workspace is a normal SemOS workspace (hypothesis)

Treated as a design hypothesis in Tranche 3 Phase 3.A. If workspace semantics don't fit, bespoke model with documented rationale. [Tranche 3]

### P10 — Consequence severity is orthogonal to state effect and external effects

A verb's consequence severity is independent of whether it transitions DAG state and independent of whether it produces external effects.

- A state-preserving verb can be highly consequential (send external notification, produce attestation, execute export).
- A state-transition verb can be `benign` in user terms (reorder within a collection).
- **A state-transition verb with no external effects can still be highly consequential** — transitioning sanctions state, settlement readiness, legal status, approval state, or controlling designation may warrant `requires_confirmation` or `requires_explicit_authorisation` even with `external_effects: []`. The consequence lives in the state transition itself, not in an emitted signal.

The three axes require independent declaration. No axis is derivable from another.

### P11 — Consequence tier declaration is a baseline floor with monotonic escalation

The declared consequence tier is the **baseline tier** — the minimum authority required to invoke the verb. Context (arguments, entity attributes, session state) can escalate the effective tier upward but never de-escalate below the baseline.

Every verb declares:
- A **baseline tier** — the minimum (one of the four tier values from P1).
- Optional **escalation rules** — declarative conditionals that raise the tier when specific conditions apply. May be absent.

At runtime, effective tier is computed:
```
matched_rule_tiers = [rule.tier for rule in escalation_rules if rule.matches]

if matched_rule_tiers is empty:
    effective_tier = baseline_tier
else:
    effective_tier = max(baseline_tier, max(matched_rule_tiers))
```

Equivalently, effective tier is the maximum of the baseline tier and the tiers of all matching escalation rules, where the empty-match case reduces to baseline_tier. Monotonic in the max direction. The baseline floor is architecturally binding; runtime can only tighten the gate, never relax it.

Escalation rules are declarative — a restricted DSL over argument values, entity attributes, and named context flags. Not arbitrary code. The validator checks rules are well-formed. Sage and REPL compute effective tier at invocation time, after entities are resolved.

### P12 — Runbook consequence tier composes uniformly across any runbook

A runbook's effective tier is computed from its constituent verbs' effective tiers and its structure. This computation applies uniformly to **any runbook**, regardless of origin:

- **Macro-produced runbooks** — derived from a catalogue-declared macro; the macro may carry additional declared composition rules specific to its shape.
- **Ad-hoc REPL runbooks** — user-assembled sequences typed in the REPL or proposed by the Sage; compute effective tier from the same three components without macro-declared extras.

The three composition components apply in all cases:

- **Component A — max step tier:** the maximum effective tier of any constituent verb (each verb's tier after its own escalation rules applied).
- **Component B — aggregation escalation:** rules that raise the runbook's tier based on patterns across the sequence (bulk cardinality thresholds, repeated external effects, high-volume updates).
- **Component C — cross-scope escalation:** rules that escalate based on the runbook spanning multiple workspaces, touching entities across multiple DAGs, or producing systemic effects no individual verb captures.

Effective runbook tier:
```
runbook_tier = max(
  max(step.effective_tier for step in runbook),           // Component A
  aggregation_tier_if_applicable(runbook),                // Component B, defaults to benign if no rule matches
  cross_scope_tier_if_applicable(runbook)                 // Component C, defaults to benign if no rule matches
)
```

Pure function of runbook structure plus constituent verbs' declarations. Sage and REPL treat the runbook's effective tier identically to an individual verb's. The user sees the honest effective tier whether they assembled the runbook themselves in the REPL or invoked a macro.

Composition rules for Components B and C are catalogue-declared patterns. Macros may additionally carry macro-specific composition rules, but the base patterns apply to any runbook through the same pure-function computation.

### P13 — Semantic vs policy declaration axes are different kinds of claim

Not all declaration axes carry the same kind of truth:

**Semantic declarations** describe what the verb *is* — structural facts:
- `state_effect`, `external_effects`, `pre`, `transitions`.

Mechanically validatable. Missing DAG reference is wrong; state-transition verb without declared transitions is wrong. The validator can prove inconsistency.

**Policy declarations** describe how the verb *should be treated* — governance decisions:
- `consequence` (baseline tier, escalation rules).
- Composition rules for runbooks.
- Authorship access requirements.

Partially validatable. The validator enforces schema conformance (tier field present with valid value), well-formedness (escalation rules have valid syntax, reference known fields), and *conservative* structural sanity warnings — only combinations that are **internally inconsistent in a mechanical sense** raise warnings, not combinations that are merely unusual. See §6.2 for the warning model.

Tier correctness is a governance judgement, not a theorem. Tranche 2 Phase 2.C is a first-class judgement pass with documented authority.

### P14 — Sage and REPL are two access patterns over the same catalogue

Both operate on the same verb catalogue with different authority models:
- **Sage (proactive, bounded-autonomous):** executes within an envelope defined by effective consequence tier.
- **REPL (reactive, user-authoritative):** user is ultimate authority; REPL's execution policy is tier-aware.

Both respect the same catalogue. The effective tier is what lets them collaborate principled rather than arbitrarily.

### P15 — Tranches are independently valuable

Each tranche delivers a coherent capability at its exit.

---

## 4. Context

### 4.1 What exists

- Workspace DAG model with ESPER navigation and four primary workspaces (Deal, CBU, KYC, Instrument Matrix).
- ~1,500 DSL verbs across 134 domains.
- 625 `CustomOperation` implementations.
- ~306 database tables with entity-state columns.
- SemOS-Scoped Verb Resolution (83% accuracy litmus).
- Semantic Traceability Kernel and Loopback Calibration.
- Prior remediation pack: 23 broken macro refs, 845 missing SemOS footprints, 69 phrase collisions, 34 orphan entries.
- Motivated Sage architecture conceptually in place.

### 4.2 What doesn't yet exist

- A formal three-axis declaration schema with escalation rules.
- A mechanical validator with conservative policy-sanity warnings.
- Catalogue-load validation at SemOS startup; DB-free operating mode.
- Declared DAG taxonomies as explicit artefacts.
- Uniform runbook composition rules for consequence tier.
- `cargo xtask reconcile` as a catalogue-operation client.
- The Catalogue workspace.
- Authorship verbs, macros, access control.
- Forward-discipline enforcement.
- Tier-aware Sage autonomy policy.
- Tier-aware REPL confirmation policy.
- A governance judgement process for tier assignments with named authority (P-G — §13).

### 4.3 Ground-truth assumptions

**Tranche 1:**
- Every verb can have three-axis declaration articulated.
- Catalogue-time operations can be DB-free.
- The four-tier consequence enumeration with escalation DSL is expressive enough.
- The escalation DSL can be designed restrictive-enough-to-validate while expressive-enough-for-real-verbs.
- Runbook composition rules capture the patterns real runbooks produce — whether macro-expanded or ad-hoc.
- **The organisational partnership required to name tier-assignment authority (P-G) is available during Phase 1.1.** If organisational blockers prevent this, Tranche 1 cannot complete.

**Tranche 2:**
- Declared DAG taxonomies are substantially correct.
- Runtime behaviour at the state-transition layer matches declarations on fixtures once articulated.
- Orphan verbs primarily resolve as A or D.
- Consequence tier assignment is mostly mechanical with a minority requiring judgement; the judgement pass has clear authority (established by P-G in Tranche 1).
- Escalation rules are declarable for context-dependent verb families without contortion.

**Tranche 3:**
- Catalogue workspace fits normal SemOS workspace semantics (hypothesis).
- Authoring macros emerge from Tranche 2 patterns.
- Sage autonomy bounded by effective tier is a sufficient autonomy model.
- Runtime tier computation is fast enough at invocation time not to introduce perceptible latency.

---

## 5. The three tranches

Per-tranche vision, scope, phases, DoD, exit value.

**A note on tranche weight.** Tranche 1 is foundational and necessarily the heaviest of the three. It establishes the semantic model, the escalation DSL, the runbook composition rules, the validator, the startup wiring, the declared taxonomies, the xtask client, the Sage / REPL policies, and the governance process for tier review (including named authority). This is a deliberate concentration — all downstream work depends on it, and splitting it across more tranches would fragment the schema work. The risk is not that Tranche 1 is *wrongly* heavy, but that Tranche 3 thinking may leak into Tranche 1 scope. See R24.

---

## 6. Tranche 1 — Semantic model and validator

### 6.1 Tranche 1 vision

Establish the three-axis semantic model including escalation rules and uniform runbook composition, implement the validator, wire DB-free catalogue-load, deliver xtask, and document the Sage / REPL policies plus the governance process for tier review with named authority. Platform gains formal catalogue reasoning capability.

### 6.2 Tranche 1 scope

In scope:

- Define the three-axis declaration schema:
  - **state_effect:** `transition` | `preserving`.
  - **external_effects:** set of `observational`, `emitting`, `navigating`.
  - **consequence:** baseline tier (one of `benign` | `reviewable` | `requires_confirmation` | `requires_explicit_authorisation`) plus optional escalation rules.
  - Plus `pre`, `transitions` (if state_effect = transition), `conditional` flag, `narration_template`.
- Define the four-tier consequence taxonomy formally: semantics, examples, edge-case guidance per tier.
- Define the escalation DSL — declarative conditional rules over argument values, entity attributes, and named context flags. Restricted expressiveness: no arbitrary code. Monotonic: rules can only raise tier.
- Define runbook composition rules — the three components applied uniformly to any runbook:
  - **Component A (max step tier)** — mandatory.
  - **Component B (aggregation escalation)** — declarative pattern rules; catalogue-declared.
  - **Component C (cross-scope escalation)** — declarative pattern rules; catalogue-declared.
- Implement the validator as a pure function library in `sem_os_core`. Checks:
  - **Structural declarations (errors):**
    - `state_effect: transition` without a `transitions` block.
    - `state_effect: preserving` with a non-empty `transitions` block.
    - `pre` referencing a DAG state not declared in the referenced workspace taxonomy.
    - `transitions` referencing DAG states not declared in the referenced workspace taxonomy.
    - `conditional: false` with multiple post-condition branches declared.
  - **Well-formedness (errors):**
    - Tier field absent or not one of the four valid values.
    - Escalation rule references an argument name not in the verb's arg schema.
    - Escalation rule references an entity attribute not declared on the referenced entity.
    - Escalation rule tier value invalid.
    - Composition rule references a nonexistent verb, macro, or named pattern.
    - Narration template references tokens not declared in the verb's argument or context schema.
  - **Policy sanity warnings (conservative, narrow):** warnings raised only for combinations that are mechanically internally inconsistent — not for combinations that are merely unusual. P10's orthogonality means many "unusual" combinations are legitimate: state-preserving verbs with `requires_explicit_authorisation` (exports, attestations, disclosures); state-transition verbs with `benign` (cosmetic reorderings); state-transition verbs with `external_effects: []` and `requires_explicit_authorisation` (sanctions-state transitions, settlement-readiness advances, approval-state changes). These pass silently. The validator does not warn based on opinion about what "should" be consequential.
- Implement catalogue-load validation in SemOS startup **before DB pool initialisation**.
- Produce declared DAG taxonomy artefacts for Deal, CBU, KYC, Instrument Matrix.
- Implement `cargo xtask reconcile --validate / --batch / --status`.
- Implement a hand-curated 20-verb fixture covering all schema combinations, including verbs with escalation rules, and exercising runbook composition through both a macro-produced runbook and an ad-hoc REPL-assembled runbook.
- Document Sage autonomy policy and REPL confirmation policy as tier-keyed rules consuming *effective tier* (after escalation and composition).
- **Document the governance process for tier-assignment review — authority, sign-off protocol, audit trail — including named authority (P-G resolved as part of Phase 1.1 deliverable 9).**
- CI integration: catalogue validation as a CI gate.

Out of scope:

- Catalogue workspace mechanism (Tranche 3).
- Authorship verbs, macros (Tranche 3).
- Forward-discipline enforcement (Tranche 3).
- Access control as SemOS ABAC gate (Tranche 3).
- xtask commit / rollback / macro subcommands (Tranche 3).
- Reconciliation of the 1,500-verb estate (Tranche 2).
- Sage and REPL code integration — policies are documented in Tranche 1; integration is Tranche 3.

### 6.3 Tranche 1 phases

**Phase 1.1 — Schema design.** Three-axis declaration schema. Consequence tier taxonomy with examples. Escalation DSL spec. Runbook composition rule spec with uniform application to macro and ad-hoc runbooks. Declared DAG taxonomy schema. Validator error taxonomy including the narrow warning set. Sage autonomy policy. REPL confirmation policy. **Governance process for tier-assignment review including named authority (resolves P-G).** Review.

**Phase 1.2 — Validator implementation.** Pure function library. Structural errors, well-formedness errors, conservative policy-sanity warnings. Unit tests on fixtures covering escalation and runbook composition (both macro and ad-hoc forms).

**Phase 1.3 — Declared taxonomy production.** YAML artefacts for four workspaces.

**Phase 1.4 — SemOS startup wiring.** Catalogue-load validation before DB pool init. CI verification.

**Phase 1.5 — xtask client.** `--validate`, `--batch`, `--status`.

**Phase 1.6 — Fixture and exercise.** 20-verb fixture. Both macro-produced runbook and ad-hoc REPL runbook exercised through composition rules. Simulate Sage / REPL decisions to validate policy mechanics.

### 6.4 Tranche 1 DoD

1. Three-axis declaration schema specified, documented, reviewed.
2. Consequence tier taxonomy formally defined with the four tier values.
3. Escalation DSL specified and validator-checkable.
4. Runbook composition rules specified and applicable uniformly to macro and ad-hoc runbooks.
5. Validator implemented and passes unit tests; structural errors and well-formedness errors enforced; policy-sanity warnings are conservative per §6.2.
6. Declared DAG taxonomies exist for all four workspaces.
7. Catalogue-load validation runs before DB pool init; verified in CI.
8. `cargo xtask reconcile` functional for --validate / --batch / --status.
9. 20-verb fixture validates end-to-end including escalation and composed runbooks (macro + ad-hoc).
10. Sage and REPL policies documented with tier-keyed rules consuming effective tier.
11. **Governance process for tier-assignment review documented with named authority (P-G resolved). This is an absolute DoD requirement — Tranche 1 is not complete until P-G is resolved.**
12. CI gate enforces catalogue validation.
13. Documentation updated.

### 6.5 Tranche 1 exit value

At Tranche 1 exit (DoD satisfied, including P-G), the platform has:

- Formal three-axis semantic model with escalation and uniform runbook composition.
- Validator catching structural bugs and well-formedness bugs at author-time and startup.
- DB-free catalogue validation.
- Declared DAG taxonomies for four workspaces.
- Documented escalation DSL and runbook composition rules.
- Documented Sage / REPL policies ready for voluntary honouring until Tranche 3 makes them architectural.
- Documented governance process for tier-assignment review with named authority.
- CI enforcement.
- xtask tool for local validation.

Because P-G is part of Tranche 1 DoD, Tranche 1 exit is simultaneously the gate for Tranche 2 kickoff. There is no "technical completion" vs "Tranche 2-ready completion" split — Tranche 1 completes once, and its completion implies readiness for Tranche 2.

---

## 7. Tranche 2 — Estate reconciliation

### 7.1 Tranche 2 vision

Reconcile 1,500-verb estate through Tranche 1's mechanism. Every verb declares three axes including escalation rules where context-dependent. Orphans resolved. Tier distribution reviewed as a **first-class governance judgement pass** under the named authority established in Tranche 1 (P-G). Runtime triage categorises Bucket 3 for fix, Bucket 2 for follow-up.

### 7.2 Tranche 2 prerequisite

Tranche 2 kickoff requires Tranche 1 DoD satisfied, which by DoD item 11 requires P-G resolved. Tranche 1 incomplete ⇒ Tranche 2 does not begin. There is no intermediate state.

### 7.3 Tranche 2 scope

In scope:

- Audit all ~1,500 verbs via xtask bulk batches.
- Every verb declares three axes:
  - state_effect, external_effects, pre, transitions as applicable.
  - Baseline consequence tier plus escalation rules where context varies.
- Execute orphan-verb phased flow (Phase 2.A DAG-side sweep, Phase 2.B orphan classification).
- Resolve orphan verbs into categories A–E.
- **Phase 2.C — consequence-tier governance judgement pass.** Review tier assignments across the estate as a first-class governance activity under the named authority (P-G). Cluster similar verbs. Surface anomalies. Resolve inconsistencies with documented rationale. Escalation rules also reviewed for coverage and well-formedness.
- Run runtime-consistency check; triage into Buckets 1, 2, 3.
- Resolve Bucket 3.
- Hand off Bucket 2 to follow-up activity with scope / owner / schedule.
- Produce exhaustive reconciliation report: declaration coverage, tier distribution, escalation rule inventory, tier-assignment decisions from Phase 2.C with rationale, Bucket 2 queue.

Out of scope:

- Structural refactor.
- Bucket 2 fixes.
- Forward-discipline enforcement.
- Workspace DAG redesign.

### 7.4 Tranche 2 phases

**Phase 2.A — DAG-side sweep.** Enumerate verbs declaring transitions per state. Non-orphan verb declarations produced via xtask batches with baseline tier and escalation rules where context-dependent.

**Phase 2.B — Orphan-verb classification and resolution.** Classify orphans into A–E. Every resolution declares all three axes.

**Phase 2.C — Consequence-tier governance judgement pass.** First-class review under named authority (P-G). Cluster similar verbs for consistent treatment. Resolve ambiguous tier assignments by governance decision with documented rationale. Review escalation rules for coverage and well-formedness. Produce the tier-assignment decision record.

**Phase 2.D — Runtime triage.** Runtime-consistency check; bucket categorisation. Resolve Bucket 3. Document Bucket 2.

**Phase 2.E — Final validation.** Validator against full reconciled catalogue; structural errors zero; policy-sanity warnings reviewed (most should pass silently given the conservative warning model).

**Phase 2.F — Reporting and handoff.** Exhaustive report with tier distribution, escalation inventory, Phase 2.C decision record, Bucket 2 queue.

### 7.5 Tranche 2 DoD

1. Every verb has a three-axis declaration or explicit exception.
2. Every orphan verb classified and resolved.
3. Every workspace DAG taxonomy consistent with catalogue-derived DAG.
4. Runtime-consistency check complete; Bucket 3 resolved; Bucket 2 handed off.
5. Consequence tier populated for every verb; escalation rules declared where context-dependent.
6. Phase 2.C governance review completed under named authority: tier-assignment decisions documented with rationale; anomalies resolved with explicit authority signature.
7. Reconciliation report produced including tier distribution and escalation inventory.
8. Documentation updated.

### 7.6 Tranche 2 exit value

Reconciled catalogue with three-axis coverage. Tier populated and reviewed with audit trail. Declared DAG taxonomies matching estate. Bucket 2 queue bounded and scheduled. Sage / REPL can honour tier-keyed policy voluntarily against the reconciled catalogue.

---

## 8. Tranche 3 — Governed authorship mechanism

### 8.1 Tranche 3 vision

Build the Catalogue workspace as a SemOS workspace. Authorship verbs and macros live inside. Access gated. Sage integrates with **effective-tier-aware** autonomy policy. REPL integrates with tier-aware confirmation policy at verb and runbook level. Forward discipline activated.

### 8.2 Tranche 3 scope

In scope:

- Design the Catalogue workspace DAG informed by Tranche 2 patterns.
- Design and implement authorship verbs with full three-axis declarations. Authorship verbs' own consequence tiers — `commit_catalogue_change` likely `requires_explicit_authorisation`; `propose_verb` likely `reviewable`; observational verbs `benign`.
- Design and implement authoring macros evidence-based from Tranche 2. Macros carry their own runbook composition rules declaring what tier the full expansion produces.
- Implement access control (catalogue-author role) as SemOS ABAC gate.
- Integrate Sage with effective-tier-aware autonomy: compute effective tier at invocation (baseline + escalation + runbook composition); honour tier policy; enforce monotonic floor.
- Integrate REPL with effective-tier-aware confirmation: per-verb and per-runbook confirmation proportional to effective tier.
- Integrate Catalogue workspace with Observatory UI.
- Integrate Sage with Catalogue workspace for agentic authorship.
- Extend xtask with commit / rollback / macro-invocation subcommands.
- Activate forward discipline.
- One-time direct-YAML bootstrap for authorship verbs and initial macros.

Out of scope:

- Re-running Tranche 2.
- Structural refactor of ob-poc.
- Runbook consequence preview (separate feature, enabled here but out of scope).
- Bucket 2 runtime alignment.

### 8.3 Tranche 3 phases

**Phase 3.A — Design.** Catalogue workspace DAG. Authorship verbs with three-axis declarations. Macros from Tranche 2 evidence. Access control. Sage integration spec. REPL integration spec. Observatory design. xtask extensions.

**Phase 3.B — Implementation.** Build workspace. Bootstrap. Implement access control. Extend xtask.

**Phase 3.C — Sage and REPL integration.** Wire effective-tier computation. Honour policy. Test against reconciled catalogue.

**Phase 3.D — Observatory integration.** UI for workspace operations. Sage-assisted catalogue authorship flows.

**Phase 3.E — Ergonomics validation.** Exercise end-to-end with realistic operations. Validate Sage isn't over-blocked; REPL flows aren't over-interrupted; tier escalation produces honest UX messaging; runbook composition tier visible to user.

**Phase 3.F — Forward-discipline activation.** Remove direct-YAML paths. Enforce architecturally.

**Phase 3.G — Handoff.** Documentation. Final architecture review.

### 8.4 Tranche 3 DoD

1. Catalogue workspace implemented as SemOS workspace.
2. Authorship verbs implemented with three-axis declarations including their own consequence tiers.
3. Authoring macros implemented evidence-based from Tranche 2; macros carry runbook composition rules.
4. Catalogue-author ABAC gate active.
5. Sage honours effective-tier-aware autonomy policy.
6. REPL honours effective-tier-aware confirmation policy.
7. Observatory UI supports Catalogue workspace.
8. Sage integration enables agentic catalogue authorship.
9. xtask extended with commit / rollback / macro subcommands.
10. Forward discipline active.
11. Ergonomics validated including effective-tier UX.
12. Documentation updated.

### 8.5 Tranche 3 exit value

Drift architecturally impossible. Sage autonomy bounded by effective consequence tier. REPL friction proportional to effective consequence. Catalogue authorship agentic. Catalogue workspace exercises three-plane architecture.

---

## 9. Cross-tranche risks and mitigations

### R1 — Declaration schema inadequate

*Mitigation:* 20-verb fixture covers representative cases; schema gaps resolve in Tranche 1.

### R2 — DB-free claim false

*Mitigation:* Phase 1.4 explicitly verifies.

### R3 — Tier taxonomy too coarse

*Mitigation:* Phase 1.6 fixture tests edge cases; extend if needed.

### R4 — Tier taxonomy too fine-grained

*Mitigation:* Phase 1.1 taxonomy includes examples and edge-case guidance.

### R5 — Escalation DSL too restrictive

*Mitigation:* Phase 1.6 fixture includes varied context-dependencies; extend DSL before Tranche 2 if gaps.

### R6 — Escalation DSL too permissive

*Mitigation:* restrict to conditional equality, set-membership, thresholds over declared typed inputs. No loops, no recursion, no arbitrary code.

### R7 — Runbook composition rules too simple

*Mitigation:* Component B (aggregation) captures common patterns. Phase 2.C review surfaces real patterns; rules may extend during Tranche 2 evidence.

### R8 — Runbook composition rules too complex

*Mitigation:* start with three components; resist additions. Prefer transparent escalation — UX shows *why* a runbook is at tier T.

### R9 — Tranche 2 volume

*Mitigation:* parallelise via subagents; xtask aggregates.

### R10 — Tranche 2 conditional verbs

*Mitigation:* soft 10% / hard 20% threshold.

### R11 — Bucket 2 queue unmanageable

*Mitigation:* >200 items or >20% of fixture triggers pause.

### R12 — Tier assignment inconsistency

*Mitigation:* Phase 2.C governance review clusters and resolves. Documented canonical examples per tier. Named authority (P-G) for decisions.

### R13 — Tier assignment governance authority unavailable

P-G requires organisational partnership to name the authority. If that partnership is not available during Phase 1.1, Tranche 1 cannot complete (DoD item 11 unmet), and Tranche 2 does not begin. The paper treats this as a binding constraint, not a mitigation target.

*Mitigation:* begin organisational conversations about P-G before Phase 1.1 starts, so the authority question is in motion alongside schema and validator design. If organisational resolution proves genuinely blocked, the platform refinement as a whole pauses rather than splitting Tranche 1 into partial completions.

### R14 — Catalogue workspace design doesn't fit SemOS workspace semantics

*Mitigation:* Phase 3.A honest evaluation; adapt or document bespoke model.

### R15 — Sage policy too restrictive

*Mitigation:* Phase 1.1 policy design uses realistic scenarios; Phase 3.E ergonomics validation tests.

### R16 — Sage policy too permissive

*Mitigation:* err on visible-action. `reviewable` shows "doing X". Audit trails. User review available.

### R17 — Effective tier computation expensive at invocation

*Mitigation:* escalation rules are simple predicates; composition is pure function. Phase 1.2 benchmarks include effective-tier computation.

### R18 — Macros designed speculatively

*Mitigation:* Tranche 2 produces patterns artefact.

### R19 — Tranche 3 ergonomics poor

*Mitigation:* Phase 3.E before 3.F activates forward discipline.

### R20 — Bootstrap circularity

*Mitigation:* Phase 3.B one-time direct-YAML; strictly scoped.

### R21 — Scope creep

*Mitigation:* strict tranche DoDs.

### R22 — Tranche decoupling violated

*Mitigation:* Tranche 1 design reviews explicit about downstream implications.

### R23 — REPL integration breaks existing UX

*Mitigation:* Phase 3.E tests existing flows; fix tier not policy if tier is wrong.

### R24 — Tranche 3 thinking leaks into Tranche 1 scope

Tranche 1 is the heaviest tranche. The risk is that design discussions drift into Tranche 3 territory — speculating about Catalogue workspace DAG shape, designing macros before Tranche 2 evidence, or specifying Observatory UX.

*Mitigation:* Tranche 1 design reviews explicitly check against Tranche 3 territory. Tranche 3 considerations captured as notes, not integrated into Tranche 1 deliverables. Items to watch for:
- Catalogue workspace DAG states / transitions.
- Authorship verb pre/post declarations.
- Macro library designs.
- Observatory UI patterns.
- Access control implementation details.

---

## 10. Decision gates

### Tranche 1 gates

1. ✅ / ❌ — three-axis declaration schema is the right formal model.
2. ✅ / ❌ — consequence tier is a first-class axis, orthogonal to state_effect **and** to external_effects.
3. ✅ / ❌ — four-tier taxonomy is the right granularity.
4. ✅ / ❌ — consequence tier is a baseline floor with monotonic escalation (P11).
5. ✅ / ❌ — escalation is declarative (restricted DSL), not arbitrary code.
6. ✅ / ❌ — runbook composition applies uniformly to macro-produced and ad-hoc runbooks via the three components (P12).
7. ✅ / ❌ — semantic vs policy distinction (P13) is recognised: structural axes validator-enforceable, policy axes validator-checkable for structure and well-formedness but governance-decisioned for correctness.
8. ✅ / ❌ — validator policy-sanity warnings are conservative; only mechanically-inconsistent combinations raise warnings (not merely unusual ones).
9. ✅ / ❌ — P3 DB-free catalogue-mode achievable.
10. ✅ / ❌ — catalogue-load validation runs before DB pool init.
11. ✅ / ❌ — declared DAG taxonomies produced for four workspaces.
12. ✅ / ❌ — xtask is a validator client.
13. ✅ / ❌ — Tranche 1 excludes Catalogue workspace.
14. ✅ / ❌ — Sage and REPL policies documented consuming effective tier.
15. ✅ / ❌ — **governance process for tier-assignment review documented with named authority (P-G resolved) — absolute requirement for Tranche 1 DoD**.

### Tranche 2 gates

16. ✅ / ❌ — Tranche 1 complete (which by DoD item 11 includes P-G).
17. ✅ / ❌ — orphan-verb phased flow is the right structure.
18. ✅ / ❌ — orphan verbs drive DAG creation where clearly implied.
19. ✅ / ❌ — runtime alignment scoped to Bucket 3.
20. ✅ / ❌ — parallelisation via subagents + xtask.
21. ✅ / ❌ — Phase 2.C consequence-tier review is a first-class governance judgement pass under named authority.
22. ✅ / ❌ — escalation rules declared for context-dependent verbs.

### Tranche 3 gates

23. ✅ / ❌ — Tranche 2 complete.
24. ✅ / ❌ — Catalogue workspace is a normal SemOS workspace (or bespoke documented).
25. ✅ / ❌ — Forward discipline activated only after ergonomics validation.
26. ✅ / ❌ — Macros evidence-based from Tranche 2.
27. ✅ / ❌ — Sage honours effective tier.
28. ✅ / ❌ — REPL honours effective tier at verb and runbook level.
29. ✅ / ❌ — Observatory and Sage integrations first-class.
30. ✅ / ❌ — Bootstrap scoped strictly to Phase 3.B.

---

## 11. Definition of done (platform-level)

Three tranche DoDs satisfied.

Stopping points:
- **Tranche 1 complete** (all 13 DoD items including P-G): formal model, CI validation, documented policies, named governance authority — useful.
- **Tranche 1 + 2 complete**: reconciled estate, tier-populated with escalation, governance audit trail — useful with code-review discipline.
- **All three tranches complete**: drift impossible, tier-enforced Sage autonomy, tier-aware REPL, agentic authorship.

Each stopping point is a valid end-state. The platform can stop after any tranche and retain the capability that tranche delivered.

---

## 12. What this enables downstream

**After Tranche 1:**
- SemOS startup safety.
- CI gate on catalogue changes.
- Formal vocabulary for verbs, DAGs, consequence severity, escalation, runbook composition.
- Sage / REPL policies documented and ready for voluntary honouring.
- Governance process for tier review documented with named authority.

**After Tranche 2:**
- Reconciled catalogue with three-axis coverage including escalation.
- Tier distribution reviewed via governance judgement under named authority; decisions audited.
- Bucket 2 queue bounded.
- Phase 0 ownership matrix (if three-plane refactor proceeds) can reference reconciled verbs with tier.
- Runbook consequence preview becomes implementable: composition rules plus declared DAG transitions give everything needed for DAG-level projection without shadow execution.
- Sage / REPL can honour tier-keyed policy against the real catalogue voluntarily.

**After Tranche 3:**
- Drift architecturally impossible.
- Sage autonomy bounded by effective tier at verb and runbook level.
- REPL friction proportional to effective consequence.
- Catalogue authorship agentic.
- All downstream capabilities unlocked.

Three-plane refactor is a downstream consumer; timing independent.

---

## 13. Prerequisite resolution and open questions

### Prerequisite P-G — Named authority for tier-assignment governance

**Status: required for Tranche 1 DoD (item 11). Tranche 1 is not complete until P-G is resolved.**

Phase 2.C of Tranche 2 is a first-class governance judgement pass, not a tidy-up step. It reviews consequence-tier assignments across the estate and resolves ambiguous cases as governance decisions with documented rationale. This requires a named authority — a body or role with the standing to make tier-assignment decisions.

Candidate authority models (to be decided during Phase 1.1):

- **Architecture committee** — a small standing group (platform lead, compliance lead, security lead, domain architect) that convenes for tier decisions, produces a documented decision record.
- **Platform lead** — single individual with authority, consulting subject-matter experts as needed, signing off decisions.
- **Per-workspace ownership** — workspace owners own tier decisions within their workspaces, with cross-workspace escalation to a central authority.
- **Tier-tiered authority** — lower tiers (benign/reviewable) assigned by author with validator sanity check; higher tiers (requires_confirmation/requires_explicit_authorisation) require committee approval.

The decision is organisational, not purely architectural. It must be made in partnership with whoever has organisational standing to delegate the authority. P-G resolution is included in Phase 1.1 deliverable 9 and is part of Tranche 1 DoD item 11.

If organisational resolution is blocked during Phase 1.1, Tranche 1 remains incomplete. The paper does not accommodate a partial-Tranche-1 exit — Tranche 1 is atomic with respect to P-G. This is by design: P-G is as load-bearing as the validator or the schema, because without it the mechanism that ingests those artefacts (Tranche 2's governance pass) cannot execute.

### Open questions

Architectural or process, not blocking:

1. Tranche sequencing — design thinking can overlap, implementation strict.
2. Tranche independence — explicit cross-tranche implication review during Tranche 1.
3. Catalogue-mode as first-class SemOS property — formalise in future three-plane document.
4. Tranche 2 without Tranche 3 valid long-term — yes with code-review discipline.
5. Bucket 2 follow-up scope — sized during Tranche 2.
6. Macro governance post-Tranche 3 — via workspace normal flow.
7. Agentic authorship aggression — Sage suggests, user confirms.
8. Relationship to three-plane refactor — independent.
9. Consequence tier granularity — four tiers; adjust after Phase 1.6 if needed.
10. Reversibility signal separate from tier — tier bakes it in implicitly; separate signal out of scope.
11. Sage autonomy customisation — out of scope Tranche 3; future session-preference extension.
12. REPL confirmation UX — co-design in Phase 3.E.
13. Escalation DSL expressiveness — start restrictive; extend only on demonstrated need.
14. Runbook composition rule completeness — start with three components; review during Tranche 2.
15. Effective-tier UX transparency — honest UX shows the escalation chain.

---

## 14. Next artefact

Tranche 1 Phase 1.1 deliverables:
1. Declaration schema document (three axes with escalation).
2. Consequence tier taxonomy with examples and edge-case guidance.
3. Escalation DSL specification.
4. Runbook composition rule specification (uniform for macro and ad-hoc).
5. Declared DAG taxonomy schema.
6. Validator error taxonomy (structural errors, well-formedness errors, conservative policy-sanity warnings).
7. Sage autonomy policy spec (consuming effective tier).
8. REPL confirmation policy spec (consuming effective tier).
9. **Governance process for tier-assignment review, including named authority (resolves P-G and satisfies Tranche 1 DoD item 11).**

All nine deliverables are required for Tranche 1 DoD. Produced by a downstream Claude/Zed session reviewing this document.

---

**End of Catalogue Platform Refinement v1.0 — Final.**
