# Tranche 3 — Design + Phase 3.A — 2026-04-26

> **Spec reference:** v1.2 §8 (Tranche 3 — Governed authorship mechanism).
> **Authority:** Adam-as-architectural-authority per `tier-assignment-authority-provisional.md`.
> **Status:** Phase 3.A design + Phase 3.B core implementation in progress.
> **Goal:** Drift becomes architecturally impossible. Sage autonomy bounded by effective consequence tier. REPL friction proportional to effective consequence. Catalogue authorship agentic. The Catalogue workspace exercises the three-plane architecture.

---

## 1. Tranche 3 design overview

Tranche 3 closes the loop on v1.2's vision: every catalogue change must pass through a governed workspace whose own verbs declare three-axis tiers and whose ABAC gate ensures only authorised principals can author the catalogue.

The ergonomics goal: a catalogue author's experience inside the Catalogue workspace is identical to a CBU operator's experience inside the CBU workspace — same Sage proposal flow, same REPL confirmation policy, same Observatory canvas. The architecture is uniform; what changes is the *consequences* the workspace's verbs control.

## 2. P9 hypothesis test — "Catalogue workspace is a normal SemOS workspace"

v1.2 P9 frames this as a hypothesis to test in Phase 3.A. Verdict: **the hypothesis holds.**

**Evidence in favour:**

1. The Catalogue workspace's primary entity is a **proposal** — a row in a `catalogue_proposals` table with a state machine (`DRAFT` → `STAGED` → `COMMITTED` / `ROLLED_BACK`). Identical shape to other domain workspaces' primary entities (deal, cbu, kyc-case).

2. The four authorship verbs (`propose-verb-declaration`, `commit-verb-declaration`, `rollback-verb-declaration`, `list-proposals`) fit cleanly into the v1.2 three-axis model:
   - `propose` — `state_effect: transition` (DRAFT entry) + `[]` + `reviewable`.
   - `commit` — `state_effect: transition` (STAGED → COMMITTED) + `[emitting]` (audit + downstream notify) + `requires_explicit_authorisation`.
   - `rollback` — `state_effect: transition` (STAGED → ROLLED_BACK) + `[]` + `requires_confirmation`.
   - `list-proposals` — `state_effect: preserving` + `[observational]` + `benign`.

3. ABAC gating is identical to other workspaces — the catalogue-author role is a SemOS principal-level attribute checked at gate-time. No new ABAC primitives required.

4. The Sage proposal flow (semantic-utterance → verb match → preview → confirm) works without modification. The author types "propose a tier change for cbu.suspend" and Sage routes to the `propose-verb-declaration` verb.

**Evidence against (rebutted):**

1. **"The catalogue is itself code that defines verbs — meta-circular."** True but doesn't violate workspace shape. The same way a CBU's verbs operate on CBU rows, catalogue verbs operate on `catalogue_proposals` rows. The proposal *content* references verb FQNs, but that's data, not meta-circular structure.

2. **"Authorship needs richer ergonomics than other workspaces."** The Phase 3.D Observatory integration adds catalogue-specific UX (diff preview, validator output rendering), but those are presentation-layer concerns, not workspace-shape concerns.

**Conclusion:** Catalogue workspace is implemented as a standard SemOS workspace. No bespoke model required. v1.2 P9 hypothesis confirmed.

## 3. Catalogue workspace shape

### 3.1 Slots

```
catalogue_dag.yaml
├── workspace_root  (stateless coordination root)
├── proposal        (stateful — primary entity)
│   ├── states: DRAFT, STAGED, COMMITTED, ROLLED_BACK, REJECTED
│   ├── transitions:
│   │   ├── (entry) → DRAFT          via propose-verb-declaration
│   │   ├── DRAFT → STAGED           via stage-proposal (auto-stage on validator-clean)
│   │   ├── STAGED → COMMITTED       via commit-verb-declaration  (requires_explicit_authorisation)
│   │   ├── STAGED → ROLLED_BACK     via rollback-verb-declaration  (requires_confirmation)
│   │   ├── DRAFT → REJECTED         via reject-proposal           (requires_confirmation)
│   │   └── REJECTED → DRAFT         via reopen-proposal           (reviewable)
│   └── carrier table: catalogue_proposals
└── verb            (stateless reference — points at config/verbs/<domain>/<verb>.yaml)
```

### 3.2 Cross-workspace constraints

The Catalogue workspace gates **into other workspaces:**

```
catalogue_proposal.COMMITTED → triggers seed reload across all 11 domain workspaces
```

This is the *forward-discipline activation point* (Phase 3.F) — once Tranche 3 ships, any change to the verb catalogue MUST flow through `catalogue_proposal.COMMITTED`. Direct YAML edits become uneditable (the YAML files are write-locked at the filesystem level; reads come from a runtime-managed store).

### 3.3 ABAC gate

```yaml
catalogue-author-role:
  description: Permits creating, staging, committing, and rolling back verb declaration proposals
  permissions:
    - catalogue.propose-verb-declaration
    - catalogue.list-proposals
    - catalogue.rollback-verb-declaration
  separation_of_duties:
    # Per v1.2 §8.4 DoD item 4: catalogue-author may NOT commit their own proposals.
    # commit-verb-declaration requires a different principal than the proposal author.
    - rule: cannot_commit_own_proposal
      enforcement: ABAC + audit
```

`commit-verb-declaration` requires explicit ABAC review by a *different* catalogue-author. This gives the catalogue a "two-eye" rule — important because committing is the architectural drift gate.

## 4. Authorship verb specs (v1.2 §8.3 Phase 3.A)

### 4.1 `catalogue.propose-verb-declaration`

```yaml
catalogue.propose-verb-declaration:
  description: Stage a new or updated verb declaration for review
  behavior: plugin
  args:
    - name: verb-fqn          # the verb being authored / amended
      type: string
      required: true
    - name: proposed-declaration
      type: json              # the full verb YAML fragment, JSON-serialised
      required: true
    - name: rationale
      type: string
      required: false
  three_axis:
    state_effect: transition
    external_effects: []
    consequence:
      baseline: reviewable
  transition_args:
    entity_id_arg: proposal-id
    target_workspace: catalogue
    target_slot: proposal
  returns:
    type: uuid
    name: proposal_id
    capture: true
```

### 4.2 `catalogue.commit-verb-declaration`

```yaml
catalogue.commit-verb-declaration:
  description: Promote a staged proposal to the authoritative catalogue (irreversible)
  behavior: plugin
  args:
    - name: proposal-id
      type: uuid
      required: true
    - name: approver
      type: string
      required: true       # ABAC-checked: must be a different catalogue-author than the proposer
  three_axis:
    state_effect: transition
    external_effects: [emitting]
    consequence:
      baseline: requires_explicit_authorisation
  transition_args:
    entity_id_arg: proposal-id
    target_workspace: catalogue
    target_slot: proposal
```

### 4.3 `catalogue.rollback-verb-declaration`

```yaml
catalogue.rollback-verb-declaration:
  description: Roll back a staged proposal (returns to DRAFT)
  behavior: plugin
  args:
    - name: proposal-id
      type: uuid
      required: true
    - name: reason
      type: string
      required: true
  three_axis:
    state_effect: transition
    external_effects: []
    consequence:
      baseline: requires_confirmation
  transition_args:
    entity_id_arg: proposal-id
    target_workspace: catalogue
    target_slot: proposal
```

### 4.4 `catalogue.list-proposals`

```yaml
catalogue.list-proposals:
  description: List proposals filtered by status, author, or date range
  behavior: plugin
  args:
    - name: status
      type: string
      required: false
      valid_values: [DRAFT, STAGED, COMMITTED, ROLLED_BACK, REJECTED]
    - name: author
      type: string
      required: false
    - name: since
      type: date
      required: false
  three_axis:
    state_effect: preserving
    external_effects: [observational]
    consequence:
      baseline: benign
```

## 5. Authoring macros (Phase 3.B evidence-based)

Per v1.2 §8.2 P26: macros are evidence-based from Tranche 2 patterns. The patterns Tranche 2 surfaced:

### 5.1 `catalogue.tier-tightening` macro

Most common authorship pattern from Phase 2.G.4: tighten one verb's tier to align with its cluster.

```yaml
- macro_id: catalogue.tier-tightening
  description: Tighten a verb's baseline tier (only ever upward — monotonic floor)
  args:
    - verb-fqn
    - new-tier
    - rationale
  expansion:
    - catalogue.propose-verb-declaration
    - (auto-stage if validator-clean)
    - catalogue.commit-verb-declaration  # requires different approver
  composition_tier: requires_explicit_authorisation
```

### 5.2 `catalogue.add-escalation-rule` macro

Pattern from R25: add a context-dependent escalation rule to a verb that needs it.

```yaml
- macro_id: catalogue.add-escalation-rule
  description: Add an escalation rule to a verb's consequence block
  args:
    - verb-fqn
    - rule-name
    - predicate         # arg_eq / arg_in / entity_attr_eq / etc.
    - escalated-tier
  expansion:
    - catalogue.propose-verb-declaration
    - catalogue.commit-verb-declaration
  composition_tier: requires_explicit_authorisation
```

### 5.3 `catalogue.migrate-to-transition-args` macro

Pattern from Phase 2.C revisit: migrate a grandfathered verb from inline `transitions:` to `transition_args:`.

```yaml
- macro_id: catalogue.migrate-to-transition-args
  description: Migrate a v1.1 verb (legacy transitions: block) to v1.2 canonical transition_args:
  args:
    - verb-fqn
    - target-workspace
    - target-slot
    - entity-id-arg
  expansion:
    - catalogue.propose-verb-declaration
    - catalogue.commit-verb-declaration
  composition_tier: requires_confirmation
```

### 5.4 `catalogue.declare-new-verb` macro

```yaml
- macro_id: catalogue.declare-new-verb
  description: Declare a brand-new verb with full three-axis from a shape template
  args:
    - verb-fqn
    - shape-template       # pointer to one of the 11 templates in verb-shape-templates.md
    - domain-overrides     # tier override + escalation rules if needed
  expansion:
    - catalogue.propose-verb-declaration
    - catalogue.commit-verb-declaration
  composition_tier: requires_explicit_authorisation
```

### 5.5 `catalogue.bulk-tier-tightening` macro

Used for mass-fixes like Phase 2.G.4's 6 self-consistency fixes.

```yaml
- macro_id: catalogue.bulk-tier-tightening
  description: Tighten multiple verbs to align with a cluster pattern
  args:
    - verb-fqns       # list
    - new-tier
    - cluster-rationale
  expansion:
    - foreach verb-fqn:
        - catalogue.propose-verb-declaration
    - catalogue.commit-verb-declaration  # bulk commit requires explicit auth + audit log
  composition_tier: requires_explicit_authorisation
  composition_rules:
    - aggregation: cardinality > 5 → escalate one tier  # >5 verb changes = larger blast radius
```

## 6. Sage integration spec (Phase 3.C)

Sage consumes the **effective tier** at proposal-time:

```rust
let effective_tier = compute_effective_tier(
    verb,
    runtime_args,
    entity_attributes,
    context_flags,
);

// For a runbook (macro expansion):
let runbook_effective_tier = compute_runbook_tier(
    runbook_steps,
    composition_rules,
);
```

Both functions exist already in `dsl-core::config::escalation` and `dsl-core::config::runbook_composition` from Tranche 1. Sage just needs to call them when proposing actions and gate its autonomy per `docs/policies/sage_autonomy.md`.

**Phase 3.C deliverable:** wiring code in the orchestrator that:
1. Computes effective tier before Sage proposes a verb / runbook.
2. Gates Sage execution per the four tiers.
3. Surfaces the escalation chain in Sage's preview text.

The runtime helpers exist; this is glue code.

## 7. REPL integration spec (Phase 3.C)

REPL consumes the same `compute_effective_tier` / `compute_runbook_tier` and applies `docs/policies/repl_confirmation.md` gates:

| Effective tier | REPL behaviour |
|----------------|----------------|
| `benign` | execute on submit, no prompt |
| `reviewable` | execute on submit, brief preview line |
| `requires_confirmation` | blocking `[y/N]` confirmation |
| `requires_explicit_authorisation` | blocking typed-paraphrase confirmation |

For runbooks, the COMPOSED effective tier gates the whole runbook with one prompt; per-step gates only fire if a step's tier exceeds the composed tier (rare).

**Phase 3.C deliverable:** REPL orchestrator changes that:
1. Compute effective tier for the matched verb / runbook.
2. Branch on tier to decide prompt shape.
3. Surface escalation rule names + composition reasons in the prompt UX.

## 8. Observatory integration (Phase 3.D)

Phase 3.D adds Catalogue-workspace-specific UX to the Observatory canvas:

- **Proposal diff preview** — when a proposal is in DRAFT, render the YAML diff between current and proposed declarations.
- **Validator output rendering** — surface structural / well-formedness errors and policy warnings in-context.
- **ABAC two-eye visualisation** — show which catalogue-author proposed, which is reviewing, and whose signature is pending.
- **Tier-distribution heatmap** — Phase 2.G.2's heatmap rendered in real-time so the catalogue's shape is visible to authors.

Per CLAUDE.md the Observatory has Phases 1-7 complete (egui canvas embedded in ChatPage). Phase 8 was deferred. **Phase 3.D = Observatory Phase 8 + Catalogue-workspace bindings.** Out of scope for this session.

## 9. Forward-discipline activation (Phase 3.F)

This is the architectural payoff: once activated, **direct YAML edits become impossible**. All catalogue changes flow through `catalogue.commit-verb-declaration`.

**Activation steps (sequenced for safety):**

1. **Stage 1 — Pilot.** Add the Catalogue workspace + authorship verbs. Direct YAML still works; the workspace is opt-in. (This session's deliverable.)

2. **Stage 2 — Soft enforcement.** Add a CI check that fails if `git log --diff-filter=M -- rust/config/verbs/` has commits not authored by `catalogue.commit-verb-declaration`. Surface as a warning in PR descriptions. (1-2 sessions out.)

3. **Stage 3 — Read-only filesystem.** Mount `rust/config/verbs/` read-only at runtime. Catalogue load now reads from a runtime store seeded from YAML at boot. (Half-week of work.)

4. **Stage 4 — Hard enforcement.** Remove YAML loading entirely. Catalogue is loaded exclusively from a database table populated by `commit-verb-declaration`. (Architectural commitment; ~1 week.)

**Phase 3.F is OUT OF SCOPE for this session.** The Catalogue workspace + authorship verbs (this session) are Stage 1. Stages 2-4 are architectural commitments that need separate planning.

## 10. Tranche 3 DoD checklist (per v1.2 §8.4)

| DoD item | Description | Status |
|---:|-----------|--------|
| 1 | Catalogue workspace implemented as SemOS workspace | **Implemented (this session)** |
| 2 | Authorship verbs implemented with three-axis declarations including own consequence tiers and transition_args | **Implemented (this session)** |
| 3 | Authoring macros implemented evidence-based from Tranche 2; macros carry runbook composition rules | **5 macros designed; YAMLs landed (this session)** |
| 4 | Catalogue-author ABAC gate active | **Spec landed; ABAC gate documented (this session); enforcement is Phase 3.F Stage 2** |
| 5 | Sage honours effective-tier-aware autonomy policy | **Helpers exist (Tranche 1); wiring deferred** |
| 6 | REPL honours effective-tier-aware confirmation policy | **Helpers exist (Tranche 1); wiring deferred** |
| 7 | Observatory UI supports Catalogue workspace | **Phase 3.D — out of scope this session** |
| 8 | Sage integration enables agentic catalogue authorship | **Spec landed; runtime wiring deferred** |
| 9 | xtask extended with commit / rollback / macro subcommands | **Stretch — see §11** |
| 10 | Forward discipline active | **Phase 3.F Stage 4 — out of scope** |
| 11 | Ergonomics validated including effective-tier UX | **Phase 3.E — smoke test in this session** |
| 12 | Documentation updated | **This document + supporting governance docs** |

**Verdict:** Tranche 3 is **partially complete** after this session. Items 1-3 fully landed; 4 design + spec + initial implementation; 11 smoke-tested; 12 documented. Items 5-10 are deferred to a follow-up session — they're substantial engineering investments rather than designable in one pass.

## 11. xtask catalogue subcommands (stretch)

Optional Phase 3.B item:

```bash
cargo x catalogue propose <verb-fqn> --rationale "..."  # opens $EDITOR
cargo x catalogue commit <proposal-id> --approver "<email>"
cargo x catalogue rollback <proposal-id> --reason "..."
cargo x catalogue list [--status <S>] [--since <date>]
```

These wrap the four authorship verbs as command-line ergonomics. Not architectural; ergonomic. **Stretch goal — implement if time permits.**

## 12. What ships from this session

**Concrete deliverables** (Phase 3.A + 3.B core):

- This design document.
- `rust/config/sem_os_seeds/dag_taxonomies/catalogue_dag.yaml` — Catalogue workspace DAG taxonomy.
- `rust/config/verbs/catalogue.yaml` — 4 authorship verbs (already exist as stubs from earlier work; upgraded to real declarations).
- `rust/src/domain_ops/catalogue_ops.rs` — real Rust implementations replacing the stubs.
- `rust/migrations/<date>_catalogue_proposals.sql` — `catalogue_proposals` carrier table migration.
- `rust/config/verb_schemas/macros/catalogue.yaml` — 5 authoring macros from Tranche 2 patterns.
- `docs/governance/catalogue-author-abac-spec.md` — ABAC role specification.
- `docs/governance/tranche-3-implementation-report-2026-04-26.md` — closing report.

**Out of scope** (Phase 3.C / 3.D / 3.E / 3.F follow-ups):

- Sage / REPL runtime wiring of effective tier.
- Observatory Catalogue-workspace UX.
- Forward-discipline activation Stages 2-4.
- xtask catalogue subcommands (stretch — TBD).

---

**End of Tranche 3 design + Phase 3.A — 2026-04-26.**
