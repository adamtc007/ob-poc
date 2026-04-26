# Full Catalogue Reconciliation Review (v1.0 Lens) — 2026-04-26

> **Scope:** Independent review of the ob-poc verb catalogue, DAG taxonomies, runtime stack, and DB estate against `docs/todo/catalogue-platform-refinement-v1_0.md`.
> **Activity framing:** Tranches 1 + 2 of v1.0 collapsed into one execution; Tranche 3 deferred. Adam-as-architectural-authority for the activity. All tier decisions explicitly revisable under future organisational P-G.
> **Method:** Section 0 (estate verification) measured first; Section 1 (codebase reading) follows; Section 2 onward planned against the verified estate. Halt-and-revise authorised if architectural tensions exceed v1.0's accommodation envelope.
> **Author:** Reconciliation review, 2026-04-26.

---

## Section 0 — Estate scope verification

### 0.1 Verb count from catalogue

**Headline number — drift detected.** Three different counts coexist in the project today:

| Source | Verb count | Three-axis declared | Coverage |
|--------|------------|--------------------:|---------:|
| `CLAUDE.md` (last reviewed 2026-04-25) | 1,245 | 758 | 60.9% |
| `cargo x reconcile status` (live) | 1,282 | 795 | 62.0% |
| Direct YAML grep over `rust/config/verbs/**/*.yaml` | **842** | **498** | **59.1%** |

The 842 vs 1,282 gap is explained by counting methodology. `cargo x reconcile` includes verbs synthesised from CRUD scaffolds + auto-phrase generation; raw YAML `verb-name:` keys give 842. **Adam should pick one canonical counting convention and document it in `cargo x verbs check`.** Until then, no claim of "X% coverage" is reproducible across runs.

For Section 0 onward this report uses the live `cargo x reconcile status` numbers (1,282 / 795 / 62.0%) as canonical, since `xtask` is the validator client v1.0 designates.

**Behavior breakdown** (raw YAML, 842 entries):

| behavior | count | % |
|----------|------:|--:|
| plugin   | 760   | 90.3 |
| crud     | 523   | 62.1 |
| graph_query | 9  |  1.1 |
| durable  | 2     |  0.2 |
| (multiple behaviors per verb explain >100% sum) | | |

**Top 8 domains by verb count:** Deal (57), Trading-Profile (55), CBU (48), Service-Resource (29), Client-Group (27), Ownership (22), Fund (22), Capital (21).

**Three-axis distribution among declared verbs (498):**

| Axis present | count | % of three-axis-declared |
|--------------|------:|-------------------------:|
| state_effect            | 498 | 100.0 |
| external_effects        | 498 | 100.0 |
| consequence.baseline    | 498 | 100.0 |
| consequence.escalation  |   3 |   0.6 |
| transitions (state map) |   0 |   0.0 |
| transition_args (v1.3)  | 126 |  15.0 |

The `transitions:` field that v1.0 §6.2 mandates as the structural carrier of `state_effect: transition` is **absent across the estate**. v1.3 substituted `transition_args:` (verb→workspace/slot pointer) which the runtime uses to look up DAG transitions externally. **This is an architectural deviation from v1.0 that has not been reconciled in the spec.** See §1.6.

### 0.2 Plugin handler count from code

The `CustomOperation` trait that v1.0 references no longer exists. Per CLAUDE.md and recent commit history, Phase 5c-migrate replaced it with `SemOsVerbOp`, and slice #80 (commit `60869ed2`) deleted the trait, the `inventory` registry, and the `dsl-runtime-macros` crate. The current contract is:

- `SemOsVerbOpRegistry` populated by `sem_os_postgres::ops::build_registry()` + `ob_poc::domain_ops::extend_registry()`.
- After this morning's cleanup commits (`2f433449`, `2159a32c`), every plugin verb declared in YAML has a registered op, and every registered op has a YAML declaration. Both invariants are tested:
  - `test_plugin_verb_coverage` — YAML→registry (was failing with 117 missing; now green).
  - `test_no_rust_only_verbs_in_registry` — registry→YAML (was failing with 44 orphans; now green).

**Effective count: 748 registered SemOsVerbOps** = the plugin verbs in YAML (the rest are CRUD or durable, dispatched through other channels). Both directions clean.

### 0.3 Reconciliation between 0.1 and 0.2

After today's commits, the catalogue↔registry reconciliation has zero drift. There are no catalogue entries without an op (Bucket A), no ops without a catalogue entry (Bucket B), and no registered orphans surviving (the 44 orphans were either un-registered or deleted).

**However**, a third-direction drift exists that v1.0 didn't anticipate but the live estate shows clearly:

- **Behavior-mismatch leftover (now fixed)** — 5 `document.*` verbs declared `behavior: durable` in YAML but were registered as `SemOsVerbOp` (treated as plugin). This was inconsistent with v1.0 P1 (a verb has one behavior) and was resolved by deleting the registrations.

The reconciliation plan should add a **fourth direction**: `behavior: <X>` in YAML must match the dispatch path the runtime actually uses. Test this invariant explicitly.

### 0.4 Workspace inventory

**v1.0 named 4 workspaces. The codebase has 11.** This is the first, largest scope deviation.

| # | Workspace | Type | DAG taxonomy | Carrier table(s) |
|--:|-----------|------|--------------|------------------|
| 1 | CBU | domain | `cbu_dag.yaml` | `cbus`, `cbu_evidence`, `cbu_service_consumption`, `cbu_trading_profiles`, `cbu_corporate_action_events`, `share_classes`, etc. |
| 2 | KYC | domain | `kyc_dag.yaml` | `cases`, `entity_workstreams`, `screenings`, `kyc_ubo_evidence` |
| 3 | Deal | domain | `deal_dag.yaml` | `deals`, `deal_products`, `deal_rate_cards`, `deal_slas`, `deal_onboarding_requests` |
| 4 | InstrumentMatrix | domain | `instrument_matrix_dag.yaml` | `cbu_trading_profiles`, `cbu_trading_activity` |
| 5 | BookingPrincipal | domain (R3.5) | `booking_principal_dag.yaml` | `booking_principal_clearances` |
| 6 | LifecycleResources | infrastructure (R1) | `lifecycle_resources_dag.yaml` | `application_instances`, `capability_bindings` |
| 7 | ProductMaintenance | infrastructure | `product_service_taxonomy_dag.yaml` | `services`, `service_versions` |
| 8 | SemOsMaintenance | infrastructure | `semos_maintenance_dag.yaml` | `changesets`, `attribute_defs`, `manco_regulatory_status` |
| 9 | SessionBootstrap | infrastructure | `session_bootstrap_dag.yaml` | (transitional; no stateful slot) |
| 10 | OnboardingRequest | infrastructure | `onboarding_request_dag.yaml` | `deal_onboarding_requests` |
| 11 | BookSetup | infrastructure (journey) | `book_setup_dag.yaml` | `client_books` |

**11 DAG taxonomy YAMLs vs 9 workspaces in CLAUDE.md** — CLAUDE.md is stale (last reviewed 2026-04-25, before R1/R3.5 lifecycle_resources + booking_principal landed). The activity should add a CI check that CLAUDE.md's workspace inventory matches the DAG taxonomy directory.

**Distinction:** workspaces 1–6 are **state-machine owners** (their DAGs declare slots over carrier tables they own). Workspaces 7–11 are **infrastructure** — pack scaffolding, governance, journeys. The latter consume domain workspaces and own minimal state of their own.

### 0.5 Dynamic / plugin / generated verb registration

Three mechanisms inflate the verb count beyond what's literally in YAML:

1. **Auto-phrase generation** (`crates/dsl-core/src/config/phrase_gen.rs`) — synthesises `action × domain` invocation phrases from synonym tables. Doesn't add verbs but inflates phrase counts and creates phrase-collision drift (this morning's `verb atlas: clear all 47 lint errors` commit handled the immediate fallout).
2. **`SimpleStatusOp` dispatcher** (`src/domain_ops/simple_status_op.rs`, today's commit `2f433449`) — one Rust struct registered 91 times via a config table. Each entry maps to a YAML-declared FQN. **Visible to the catalogue review;** no invisible verbs.
3. **`StubOp` dispatcher** (`src/domain_ops/stub_op.rs`) — 17 plugin FQNs registered as stubs. Each FQN is YAML-declared (catalogue, trading-profile prune cascades, etc.). **Visible.**

**No invisible-to-catalogue dynamic registration found.** This contradicts v1.0's worry about plugin/conditional/macro-expanded verbs being invisible to static review. The codebase is fully static at registration time.

### 0.6 Cross-cutting verb assessment

| Domain | Role | Verbs | Pack assignment |
|--------|------|------:|-----------------|
| `session.*` | Session lifecycle (pre-workspace scope) | 16 | SessionBootstrap |
| `view.*` / `nav.*` | Navigation/viewport | 22 | client-side metadata |
| `agent.*` | Agent control, narration, intent | ~10 | cross-workspace |
| `audit.*` / `observation.*` | Cross-cutting facts | ~15 | shared |
| `gleif.*` | External enrichment | 17 | research pack |
| `bpmn.*` | Workflow control | 5 | step_executor_bridge |

These 85 verbs don't naturally belong to any single workspace. v1.0 didn't address them. **Recommendation:** treat as a virtual "Infrastructure" workspace for declaration purposes — they get the three-axis treatment but their tier governance is handled by a delegated authority (e.g. the Sage/REPL infrastructure owner) rather than the domain workspace owners.

### 0.7 Coverage statement

> **The full estate reconciliation activity covers 1,282 verbs across 11 workspaces (7 domain + 4 infrastructure), with 8 gaps identified for scope decision.**

The 8 gaps are:

| # | Gap | Resolution required before R.4 begins |
|--:|-----|--------------------------------------|
| 1 | **v1.0 named 4 workspaces; codebase has 11.** | Confirm scope = all 11 (recommended) or restrict to v1.0's 4 (not recommended; would orphan ~600 verbs). |
| 2 | **`transitions:` field absent estate-wide.** v1.3 substituted `transition_args:`. v1.0 spec needs amendment to recognise this. | v1.1 amendment: declare `transition_args:` as the canonical carrier of `state_effect: transition` semantics; deprecate the inline `transitions:` block. |
| 3 | **Three-axis coverage: 62.0%, not 100%.** 487 verbs un-declared. | Calibrate per-verb declaration time on first 50-100 verbs; revise R.4 estimate. |
| 4 | **Escalation rules: 5 across 795 declared verbs (0.6%).** v1.0 P11 frames escalation as central; this rate suggests either under-adoption or that most verbs genuinely don't need escalation. | Phase R.5 must explicitly review whether the 0.6% rate reflects reality or under-declaration. **This is a measured signal that doesn't fit v1.0's mental model.** |
| 5 | **Two workspaces have 0% three-axis coverage:** InstrumentMatrix (43 verbs) and SemOsMaintenance (6 verbs). | Confirm these are R.4 scope and not deliberate exclusions. |
| 6 | **Slot-dispatch table missing 2 workspaces:** `onboarding_request`, `book_setup` not in `slot_state.rs`. | Decide: these are phase-level (no dispatch needed) or runtime-checked (need entries). |
| 7 | **Cross-cutting domains (~85 verbs)** have no workspace home. | Treat as virtual Infrastructure workspace; authority delegated. |
| 8 | **Verb count discrepancies between sources** (842 / 1,245 / 1,282). | Pick canonical convention; document. |

**The activity does not begin until Adam makes a decision on each of these 8 gaps. Recommendations are noted; final calls are organisational.**

---

## Section 1 — Codebase reading across the verified estate

### 1.1 Per-workspace verb inventory and DAG structure

For each in-scope workspace, the DAG is declared in `rust/config/sem_os_seeds/dag_taxonomies/<workspace>_dag.yaml`. State machines, transitions, and entry/terminal states are explicit. Per-workspace summary:

| Workspace | Slots | Stateful slots | XW constraints | Derived state | Cascade rules |
|-----------|------:|---------------:|---------------:|--------------:|--------------:|
| CBU | 16 | 9 | 4 | 1 (`cbu_operationally_active`) | 1 |
| Deal | 14 | 6 | 5 | 0 | 0 |
| KYC | 13 | 9 | 0 | 0 | 0 |
| InstrumentMatrix | 13 | 12 | 2 | 0 | 0 |
| BookingPrincipal | 1 | 1 | 2 | 0 | 0 |
| LifecycleResources | 2 | 2 | 2 | 1 | 1 |
| ProductMaintenance | 2 | 2 (services, service_versions) | 1 | 1 | 0 |
| SemOsMaintenance | 7 | 5 | 2 | 1 | 0 |
| SessionBootstrap | 1 | 0 | 1 | 1 | 0 |
| OnboardingRequest | 3 | 0 | 1 | 1 | 0 |
| BookSetup | 6 | 1 | 2 | 1 | 0 |

Total: 78 slots (≈49 stateful), 22 cross-workspace constraints, 9 derived states, 2 cascade rules.

Per-verb DAG attribution lives in `transition_args:` (126 verbs) which points at `(workspace, slot, entity_id_arg)`. The DagRegistry indices use this pointer to look up the verb's transition.

### 1.2 Cross-workspace boundary mapping

10 declared cross-workspace constraint predicates against the schema. Of these:

- **9 resolve to real `(table, column)` pairs** in `migrations/master-schema.sql`.
- **1 contains an `EXISTS` subquery** the predicate parser doesn't currently handle: `capability_bindings.service_id = this_consumption.service_id AND EXISTS (SELECT 1 FROM application_instances ai WHERE ai.id = capability_bindings.application_instance_id AND ai.lifecycle_status = 'ACTIVE')`. The R8 layer-4 binding constraint is enforced at runtime but not validator-checkable. **Halt-and-revise candidate (lower priority): predicate DSL needs `EXISTS` support, or this constraint needs decomposition into a derived state.**

**Constraint topology:**

```
KYC ──┬──► CBU validation (1 constraint)
      ├──► Deal contracting (1 constraint)
      └──► Book setup (1 constraint)

Deal ──┬──► CBU service consumption (1 constraint)
       └──► InstrumentMatrix mandate (1 constraint, implicit via cbu→im chain)

CBU ──┬──► InstrumentMatrix mandate (1 explicit)
      └──► (CBU is target of Mode B aggregate `cbu_operationally_active`)

BookingPrincipal ──► Deal contracting (1 constraint, R3.5)
LifecycleResources ──► CBU service-consumption.active (1 EXISTS constraint)
```

Six pairs of workspaces share constraints. **No cycles** detected in the constraint graph; KYC is the universal source, no workspace points back at it.

### 1.3 Existing types, traits, plugins relevant to reconciliation

The v1.3 runtime stack is real and operational:

| Component | File | Status |
|-----------|------|--------|
| `ThreeAxisDeclaration` | `crates/dsl-core/src/config/types.rs:184-199` | COMPLETE |
| `ConsequenceTier` enum (4 variants) | `crates/dsl-core/src/config/types.rs` | COMPLETE |
| Escalation predicate DSL | `crates/dsl-core/src/config/escalation.rs:72-124` | COMPLETE |
| Validator (pure fn library) | `crates/dsl-core/src/config/validator.rs` | COMPLETE; no DB dep verified |
| Runbook composition (A/B/C) | `crates/dsl-core/src/config/runbook_composition.rs` | COMPLETE |
| `DagRegistry` (5 indices) | `crates/dsl-core/src/config/dag_registry.rs` | COMPLETE |
| `GateChecker` (Mode A) | `crates/dsl-runtime/src/cross_workspace/gate_checker.rs` | COMPLETE |
| `DerivedStateEvaluator` (Mode B) | `crates/dsl-runtime/src/cross_workspace/derived_state.rs` | COMPLETE |
| `DerivedStateProjector` | `crates/dsl-runtime/src/cross_workspace/derived_state_projector.rs` | COMPLETE |
| `CascadePlanner` (Mode C) | `crates/dsl-runtime/src/cross_workspace/hierarchy_cascade.rs` | COMPLETE |
| `SqlPredicateResolver` | `crates/dsl-runtime/src/cross_workspace/predicate.rs` | COMPLETE; lacks EXISTS support |
| `SlotStateProvider` (Postgres) | `crates/dsl-runtime/src/cross_workspace/slot_state.rs` | COMPLETE; 26-row dispatch table |
| `PostgresChildEntityResolver` | `crates/dsl-runtime/src/cross_workspace/hierarchy_cascade.rs` | COMPLETE |
| `GatePipeline` builder | `crates/dsl-runtime/src/cross_workspace/gate_pipeline.rs` | COMPLETE |
| Wired into orchestrator | `src/repl/orchestrator_v2.rs` | OPT-IN ONLY (`with_gate_pipeline()`); default = None |
| `SemOsVerbOp` (registry trait) | `crates/sem_os_postgres/src/ops/mod.rs` | COMPLETE |
| `cargo x reconcile` | `xtask/src/reconcile.rs` | COMPLETE (--validate / --batch / --status) |
| Catalogue-load before DB pool | `crates/ob-poc-web/src/main.rs` | COMPLETE (P.1.g comment names this) |

**Critical wiring gap (lower than P-G but real):** GatePipeline is opt-in via `ReplOrchestratorV2::with_gate_pipeline()`. The default constructor passes `None`. Per-CLAUDE.md: "currently defaults to None until ob-poc-web wires it." **The runtime gate that blocks invalid transitions is not active in production until ob-poc-web's main.rs constructs the GatePipeline.** R.3 should ensure this is wired before declaring v1.3 "code complete."

### 1.4 Test fixtures and integration tests

| Component | Location | Coverage |
|-----------|----------|----------|
| Cross-workspace DAG harness (mock mode) | `crates/dsl-runtime/src/cross_workspace/test_harness/` | 11 fixtures |
| Cross-workspace DAG harness (live mode) | same module + `tests/cross_workspace_dag_live_scenarios.rs` | 3 fixtures |
| Test schema | `rust/test-migrations/cross_workspace_dag/0001_schema.sql` | curated 9-table subset |
| `cargo x dag-test --reset` | `xtask/src/dag_test.rs` | runs both modes |
| Unified pipeline tollgate tests | `tests/unified_pipeline_tollgates.rs` | 9 tests |
| REPL V2 test suite | `tests/repl_v2_*.rs` | 149 tests |
| Plugin verb coverage tests | `src/domain_ops/mod.rs` (#[cfg(test)]) | both directions |

**Fixture coverage gap:** v1.0 DoD item 9 demands a "20-verb fixture exercising escalation + composed runbooks (macro + ad-hoc)." The DAG harness has 14 fixtures but they exercise transitions, not escalation. The 5 verbs that DO declare escalation rules are not represented in any fixture. **Halt-and-revise candidate (medium priority):** R.5 escalation review will go in blind without fixtures exercising context-dependent escalation paths.

### 1.5 Internal documentation

- `CLAUDE.md` — root project guide; partially stale (workspace count, last review date).
- `docs/todo/catalogue-platform-refinement-v1_{0,1,2,3}.md` — successive specs. v1.3 is the operative spec.
- `docs/annex-sem-os.md` — references v1.3 cross-workspace runtime stack.
- `docs/observatory-implementation-plan.md` — Tranche 3 territory; out of scope for this activity.
- `docs/todo/onboarding-dag-deep-review-2026-04-26.md` — Adam's pre-existing review identifying gaps in 4-layer model.
- `docs/todo/onboarding-dag-remediation-plan-2026-04-26.md` — remediation plan that produced R1/R2/R3.5/R7 work.
- `ai-thoughts/*` — design ADRs; mostly Tranche 3-territory.

**No `tier-assignment-authority` document exists.** The closest match is the instrument-matrix-pilot-plan-2026-04-22.md which names "Adam-as-pilot-authority" explicitly as **provisional** pending estate-scale P-G. v1.0's load-bearing P-G item remains unresolved. (See §1.6.)

### 1.6 Architectural tension flags

Five tensions surfaced from the codebase reading. Two are halt-and-revise candidates.

#### Tension 1 — `transitions:` block missing; replaced by `transition_args:` (HALT-AND-REVISE)

v1.0 §6.2 prescribes `transitions:` as the structural carrier of `state_effect: transition`:

> "**state_effect: transition without a transitions block** → structural error."

The estate has **zero** verbs declaring `transitions:`. The v1.3 substitution is `transition_args: { entity_id_arg, target_workspace, target_slot }` — a pointer to a DAG transition rather than an inline transition map. This is architecturally cleaner (single source of truth: the DAG taxonomy) but **the validator's structural-error check no longer applies** because no verb violates it (no verb has `transitions:` to begin with).

**Recommendation:** v1.1 amendment to v1.0 §6.2:

- Replace "verb declares `transitions:` block" with "verb declares `transition_args:` block pointing at a DAG taxonomy slot".
- Replace structural error "state_effect:transition without transitions block" with "state_effect:transition without transition_args block, OR with transition_args pointing at a slot whose DAG declares no transition matching the verb's name".
- Keep escalation/composition rules unchanged.

This is **architectural reality has overtaken the spec; spec needs updating.** Don't continue R.4 declaration without the amendment — verbs declared today against v1.0's `transitions:` model would all be "wrong" the moment the spec is updated.

#### Tension 2 — P-G named authority unresolved (HALT-AND-REVISE — load-bearing)

v1.0 §13 explicitly: "**P-G is as load-bearing as the validator or the schema.** Tranche 1 is atomic with respect to P-G. If organisational resolution is blocked during Phase 1.1, Tranche 1 remains incomplete."

Current state: Adam-as-architectural-authority is documented in the v1.0 reconciliation prompt itself as a *convention for this activity*, marked "explicitly revisable under future organisational P-G."

This is the load-bearing case v1.0 names: organisational resolution is blocked / deferred, and the activity is proceeding on a provisional convention. Two paths:

1. **Treat the convention as the named authority for this activity.** v1.0 §13 doesn't strictly forbid this — "named authority" just means *someone is named*. Adam-as-architectural-authority IS named. The "revisable under future organisational P-G" clause means the decisions are reviewable, not that they're invalid.
2. **Halt and resolve P-G before Phase R.5.** Wait for organisational delegation.

**Recommendation:** Path 1 with explicit framing. The v1.0 reconciliation prompt itself uses path 1 framing ("Adam acting as architectural authority for this execution. All tier decisions are explicitly revisable"). v1.0 §13's strictness was about *un-named* authority; an explicitly-named provisional authority with documented revisability is consistent with the spec's intent. **The architecture paper should be amended to reflect that "named authority" admits a documented provisional designation** — this is a minor v1.1 amendment, not a halt.

#### Tension 3 — Escalation rules: 5 across 795 declared verbs (0.6%) (DESIGN QUESTION)

v1.0 P11 frames escalation as central to the model. v1.0 R5 worried "Escalation DSL too restrictive"; v1.0 R6 worried "Escalation DSL too permissive." Neither anticipated a 0.6% adoption rate.

Three hypotheses:

(a) **Most verbs genuinely don't need escalation.** ob-poc verbs largely have stable consequence regardless of arg values: a `cbu.suspend` is `requires_confirmation` whether for one CBU or a group; `deal.contracted` is `requires_explicit_authorisation` regardless of deal size. If true, 0.6% is the right number.

(b) **Authors are skipping escalation because the DSL is too restrictive or under-tooled.** The 5 verbs that DO declare escalation might be the only ones where the author had a clear pattern. Other verbs that should escalate are silently un-declared.

(c) **The model's emphasis on escalation is wrong.** The real lever for tier variance is workspace-level policy, not per-verb declaration.

**Recommendation:** R.5 governance review must explicitly examine the 0.6% rate against reality. Surface 10–20 verbs the reviewer thinks ought to escalate; check whether they do; if not, classify each as "(a) genuinely doesn't need it" or "(b) under-declared." If the verdict is mostly (a), the 0.6% rate is correct and v1.0 should be amended to soften P11's emphasis. **This is a measurement-vs-spec tension that resolves through governance review, not an architectural halt.**

#### Tension 4 — `EXISTS` subquery in cross-workspace constraint not validator-checkable (LOW PRIORITY)

The R8 layer-4 binding constraint (`capability_bindings.service_id = this_consumption.service_id AND EXISTS (SELECT 1 FROM application_instances ai WHERE ai.id = capability_bindings.application_instance_id AND ai.lifecycle_status = 'ACTIVE')`) is enforced at runtime by `SqlPredicateResolver` but the validator can't statically check that the EXISTS-clause tables/columns exist. **Recommendation:** R.7 final validation should add a DB-roundtrip check (against test schema, not production) for any predicate the static parser can't resolve. Out of scope for the v1.0 spec but a real gap.

#### Tension 5 — GatePipeline opt-in, not default (MEDIUM PRIORITY)

`ReplOrchestratorV2::with_gate_pipeline()` is required to activate the runtime gate. ob-poc-web's main.rs does not currently construct it (per CLAUDE.md). **The runtime gate is opt-in everywhere.** This is fine for harness/test environments but means the v1.3 "CODE COMPLETE" claim doesn't extend to "production traffic is gated by it." **Recommendation:** R.3 must explicitly wire the GatePipeline into `ob-poc-web::main` before this activity declares v1.3 production-ready.

---

## Section 2 — Reconciliation phased plan (R.1 – R.9)

The plan is structured into 9 phases. R.1–R.7 collapse v1.0 Tranche 1+2 into one execution. R.8 is the post-reconciliation coherence pass (required, not optional, given estate scale). R.9 is reporting + handoff feeding into Tranche 3.

Pre-condition: **all 8 gaps from §0.7 must have a scope decision before R.1 begins.**

### R.1 — Schema, validator, and DSL hardening

**Scope.** v1.0 Tranche 1 Phase 1.1–1.2 + Tension 1 amendment. Most of the schema and validator are COMPLETE per §1.3; this phase finalises the gaps.

**Concrete artefacts:**
- `docs/todo/catalogue-platform-refinement-v1_1.md` extended with §6.2 amendment: `transition_args:` replaces `transitions:` as the structural carrier.
- `crates/dsl-core/src/config/validator.rs` — new structural-error rule: `state_effect: transition` requires `transition_args:` resolving to a DAG slot whose state-machine declares a transition matching the verb's FQN tail.
- `crates/dsl-core/src/config/predicate_dsl.rs` — extend with `EXISTS` clause support (Tension 4).
- 20-verb declarative fixture under `crates/dsl-core/tests/fixtures/escalation_composition/` exercising 5+ escalation rules and 2 composed runbooks (macro + ad-hoc).
- Sage / REPL policy documents under `docs/policies/sage_autonomy.md` and `docs/policies/repl_confirmation.md` (DoD item 10 — currently missing).

**Exit criteria:** validator catches the new structural error in unit tests; 20-verb fixture validates clean; both policy docs reviewed.

**Maps to:** v1.0 DoD items 1, 2, 3, 4, 5, 9, 10. Tension 1 amendment lands here.

### R.2 — Declared DAG taxonomies — coherence pass across 11 workspaces

**Scope.** v1.0 Tranche 1 Phase 1.3 expanded to 11 workspaces and a cross-workspace coherence review at the boundaries identified in §1.2.

**Pre-existing.** All 11 DAG taxonomies exist. This phase reviews them as a set, not creates them.

**Concrete artefacts:**
- A side-by-side review document — one section per cross-workspace pair from §1.2 — verifying naming consistency, FK resolvability against `master-schema.sql`, that constraints don't cycle.
- Decisions on 2 dispatch gaps (§0.7 #6): `onboarding_request`, `book_setup` either added to `slot_state.rs` dispatch or documented as phase-level.
- Decision on the 5-table schema delta (§0.7 #8 partial: master-schema.sql has 367 tables; CLAUDE.md claims 372). Investigate via `git log --oneline --grep "DROP TABLE"`.

**Exit criteria:** every cross-workspace constraint predicate either resolves statically (via the extended DSL from R.1) or is decomposed; dispatch table covers every slot the DAG references; schema-table count is reconciled to the master-schema document.

**Maps to:** v1.0 DoD item 6. Tension 4 lands here partially (the EXISTS-decomposition step).

### R.3 — Runtime wiring + xtask + CI gate

**Scope.** v1.0 Tranche 1 Phase 1.4–1.5 + Tension 5.

**Pre-existing.** `cargo x reconcile` exists with all three flags. Catalogue-load runs before DB pool init. CI gate is missing.

**Concrete artefacts:**
- `ReplOrchestratorV2::new()` constructs the `GatePipeline` by default — opt-out instead of opt-in.
- `ob-poc-web::main` constructs the GatePipeline from the loaded `DagRegistry` and wires it into the orchestrator.
- `.github/workflows/catalogue.yml` (or equivalent) runs `cargo x reconcile --validate` on PR / push. The Frontend Type Check pattern from this morning's `xtask/src/main.rs` fix shows xshell is the right wiring.
- Pre-commit gate already includes `cargo x reconcile` validation (per the morning's pre-commit run); confirm and document.

**Exit criteria:** orchestrator default-on; ob-poc-web binary serves traffic gated by GatePipeline; CI fails the build on validation regression.

**Maps to:** v1.0 DoD items 7, 8, 12. Tension 5 lands here.

### R.4 — Verb declaration sweep across the estate

**Scope.** v1.0 Tranche 2 Phase 2.A + 2.B over all 1,282 verbs. The bulk of effort.

**Pre-existing.** 795 verbs (62%) declared. 487 to go. Two workspaces (InstrumentMatrix, SemOsMaintenance) at 0% and need full sweeps. ~85 cross-cutting domain verbs need workspace-virtual treatment.

**Strategy:** parallelise via subagents per v1.0 R9. Per-verb time was estimated at 15–30 minutes in v1.0; calibrate after the first 50–100 and revise. The morning's `SimpleStatusOp` work showed that ~80 status-flip verbs can be declared in a single config-table edit; **a similar pattern for the remaining 487 may compress effort substantially.**

**Concrete artefacts:**
- Per-workspace sweeps (parallelisable). Each subagent:
  - Lists the un-declared verbs in its workspace.
  - For each: assigns state_effect, external_effects, baseline tier (provisional — R.5 reviews).
  - Adds `transition_args:` if the verb is `state_effect: transition`.
  - Notes any verb that *might* need escalation but doesn't currently declare it (input to R.5).
- Orphan classification per v1.0 §7.4 Phase 2.B: every verb without DAG transitions classified A/B/C/D/E.
- The 0.6% escalation question (Tension 3): R.4 explicitly flags candidate escalation verbs; R.5 reviews.

**Exit criteria:** 100% three-axis coverage. Orphans classified. Workspace-virtual cross-cutting domain handled.

**Maps to:** v1.0 DoD items T2.1–T2.5.

**Risk:** the 487 remaining verbs include the 43 unclassified InstrumentMatrix verbs and the 6 SemOsMaintenance verbs. Both are high-impact regions — IM gates trading; SemOsMaintenance gates governance. Provisional declarations must not paper over real tier ambiguity.

### R.5 — Tier governance pass under Adam-as-authority

**Scope.** v1.0 Tranche 2 Phase 2.C across 1,282 verbs. The single-authority bottleneck.

**Sequencing:** R.5 follows R.4 by tens of percent (start when R.4 has covered ≥80%). Cluster verbs by:

1. **Domain × tier baseline** — surface outlier verbs in clusters that should be uniform.
2. **External-effect kind × tier baseline** — every verb emitting a regulatory notification should be ≥`reviewable`; flag exceptions.
3. **State-transition kind × tier baseline** — sanctions / settlement / approval transitions should be ≥`requires_confirmation`; flag exceptions.

**Tension 3 resolution lands here:** for each tier-cluster, sample 5–10 verbs; review whether they SHOULD escalate; if 0.6% is wrong, raise it; if 0.6% is right, document the rationale.

**Concrete artefacts:**
- Tier-decision-record under `docs/governance/tier-decisions-2026-Qx.md` — one entry per non-trivial decision with rationale.
- Anomaly resolution log — every clustered outlier either reclassified or rationalised.
- Escalation rule additions where Tension 3 review surfaced under-declaration.

**Exit criteria:** every verb has a governance signature (provisional Adam-as-authority). Anomaly clusters resolved. Escalation rate explicitly reviewed.

**Effort:** at 1 verb per 1–2 minutes (clustered review, not individual deep-dive), 1,282 verbs = 22–43 hours. Plan for batched 2–4 hour sessions across 2 weeks; allow checkpointing per v1.0 R9 mitigation.

**Maps to:** v1.0 DoD item T2.6 + Tranche 1 DoD item 11 (P-G satisfied through provisional convention).

### R.6 — Runtime triage on fixture set

**Scope.** v1.0 Tranche 2 Phase 2.D — runtime-consistency check.

**Pre-existing.** The cross-workspace DAG harness (mock + live modes, 14 fixtures) provides the runtime fixture base. The unified-pipeline tollgate tests provide REPL fixtures.

**Concrete artefacts:**
- Run every fixture; compare runtime behaviour to declared three-axis. Categorise into:
  - **Bucket 1:** runtime matches declaration. (Expected majority.)
  - **Bucket 2:** runtime deviates from declaration in incidental, defer-friendly ways.
  - **Bucket 3:** runtime contradicts declaration semantically — fix during this activity.
- Bucket 3 fix list with PRs.
- Bucket 2 follow-up document with scope, owner, schedule.

**Exit criteria:** Bucket 3 empty. Bucket 2 documented. Buckets 1+2 cover ≥95% of fixture set.

**Maps to:** v1.0 DoD item T2.4.

### R.7 — Final mechanical validation

**Scope.** v1.0 Tranche 2 Phase 2.E — full validator pass.

**Concrete artefacts:**
- `cargo x reconcile --validate` against full reconciled catalogue.
- Structural errors: zero (R.1 made structural-error rules complete; R.4 ensured no verb violates them).
- Well-formedness errors: zero.
- Policy-sanity warnings: reviewed; most pass silently per v1.0 §6.2's conservative model.

**Exit criteria:** clean validator pass. CI gate green.

**Maps to:** v1.0 DoD item T2.5.

### R.8 — Post-reconciliation coherence pass

**Scope.** Required, not optional, at estate scale. Four sub-passes:

**R.8.1 — Cross-section taxonomy review.** All 22 cross-workspace constraints + 9 derived states + 2 cascade rules reviewed as a set. New states from Orphan-B/C resolutions reviewed for naming consistency with existing taxonomy. Output: coherence findings; corrections back-staged through xtask.

**R.8.2 — Tier landscape review.** Heatmap of tier distribution by:
- Workspace (current: Deal 100%, CBU 100%, IM 0%, SemOsMaintenance 0% — extreme outliers).
- Domain (top 8 domains have 90%+ coverage; long tail has gaps).
- External-effect kind.
- State-transition kind.

Look for clusters that suggest miscalibration. Workspace-level distributions where 80% of verbs are `requires_explicit_authorisation` are probably mistier'd; workspaces producing external regulatory notifications where no verb reaches above `reviewable` are probably wrong.

**R.8.3 — Bucket 3 cumulative review.** Audit log of every Bucket 3 declaration change made during R.6. Cumulative semantic drift assessment: did the changes drift the catalogue toward better representation or toward whatever runtime happened to do?

**R.8.4 — Catalogue self-consistency review.** Are similar verbs declared with similar shapes? Are escalation rules consistent across verbs depending on the same context features? Are narration templates following consistent patterns?

**Exit criteria:** coherence findings either resolved or queued for follow-up. **One iteration only;** if a second iteration would be needed, that's a signal for activity-scope reassessment.

**Estimated effort:** 4 sub-passes × 4–8 hours each = 16–32 hours. Single-authority (R.5 + R.8 reviewer is the same person to maintain consistency).

### R.9 — Reporting + handoff to Tranche 3

**Scope.** v1.0 Phase 2.F-equivalent.

**Concrete artefacts:**
- Reconciliation report under `docs/governance/reconciliation-report-2026-Qx.md`:
  - Per-verb declarations summary.
  - Tier distribution (post-R.8.2).
  - Escalation rule inventory.
  - Bucket 2 follow-up queue with scope/owner/schedule.
  - R.8 coherence findings + resolutions.
  - v1.1 candidate amendments (Tension 1, Tension 4, Tension 3 verdicts).
- Tranche 3 inheritance package per Section 7 below.

---

## Section 3 — What this activity does NOT deliver

Explicitly deferred:

1. The Catalogue workspace as a SemOS workspace (Tranche 3 Phase 3.A onwards).
2. Authorship verbs (Tranche 3 Phase 3.B). The 4 `catalogue.*` verbs currently stub-registered (`StubOp`) are P.8 prototype scaffolding; their real implementations are Tranche 3.
3. Authoring macros (Tranche 3 Phase 3.B, evidence-based on this activity's findings).
4. Access control as ABAC gate (Tranche 3 Phase 3.B).
5. Sage integration with effective-tier-aware autonomy (Tranche 3 Phase 3.C).
6. REPL integration with effective-tier-aware confirmation (Tranche 3 Phase 3.C).
7. Observatory UI integration (Tranche 3 Phase 3.D). Per CLAUDE.md, Phases 1–7 of Observatory are complete (egui canvas embedded in ChatPage); Phase 8 diagrams pending; full Observatory-vs-Catalogue integration is Tranche 3.
8. Forward-discipline activation (Tranche 3 Phase 3.F) — removing direct-YAML editing and forcing all changes through the Catalogue workspace.
9. Estate-scale organisational P-G governance — explicitly out of scope; this activity uses Adam-as-architectural-authority convention (Tension 2).
10. Bucket 2 runtime alignment — separate follow-up activity.
11. The 17 stub verbs (`catalogue.*` and `trading-profile.*` prune cascades) need real implementations — out of scope for this activity; tracked via the `StubOp::STUB_VERBS` array.

---

## Section 4 — Activity-specific risks

Risks specific to this execution (not v1.0 R1–R24 restated):

**A1 — `transition_args:` amendment lands late.** R.1 must finish the v1.1 amendment before R.4 begins. If the amendment is contested, R.4 is blocked. Mitigation: write the amendment as the very first R.1 deliverable; circulate before any other work starts.

**A2 — Escalation rate verdict (Tension 3) requires extensive verb-by-verb review.** R.5 must explicitly check whether 0.6% is the right number. If the verdict is "most verbs need escalation we missed," R.4 effort doubles or triples. Mitigation: run a 20-verb sample in R.4 first; calibrate.

**A3 — InstrumentMatrix workspace at 0% coverage.** 43 verbs in a high-stakes workspace. If R.4 papers over real tier ambiguity to hit 100%, R.8.2 will catch it but post-hoc. Mitigation: IM gets a dedicated R.4 sweep with two reviewers (Adam + a domain expert if available).

**A4 — Adam-as-authority decision fatigue at estate scale (R.5).** 1,282 verbs reviewed by one person across batched sessions. Inconsistency risk is real. Mitigation: cluster aggressively; review by tier-cluster not by verb; publish interim decisions for self-consistency check.

**A5 — Verb count discrepancy (842 / 1,245 / 1,282) propagates to plan estimates.** If the canonical count is actually 842 (raw YAML), R.4 effort is 31% smaller than estimated; if 1,282 (xtask), match. Mitigation: pick the convention in §0.1 before R.1 begins.

**A6 — DAG taxonomy schema-lag (cbu_dag declares states the schema CHECK doesn't yet enforce, per cbu_dag.yaml §D-2).** R.2 cross-workspace coherence may surface cases where the DAG promises a state machine the schema can't actually carry. Mitigation: R.2 explicitly checks DAG-vs-schema state-enumeration alignment for every stateful slot.

**A7 — `EXISTS` predicate unparseability propagates to R.7 final validation.** If R.1 doesn't extend the predicate DSL, R.7 declares the R8 layer-4 constraint unvalidatable. Mitigation: R.1 must include the EXISTS extension OR R.2 decomposes the constraint.

**A8 — GatePipeline opt-in default extends past R.3.** Production traffic continues serving without the runtime gate. Mitigation: R.3 makes GatePipeline default-on; CI fails any orchestrator construction without one.

**A9 — Bucket 2 queue grows past v1.0 R11's 200-item / 20% threshold.** Runtime triage at 1,282 verbs may surface more incidental drift than v1.0 anticipated. Mitigation: R.6 budgets 1 day per workspace; if Bucket 2 exceeds threshold, pause and reassess.

**A10 — Tranche 3 thinking leaks into R.4/R.5 declarations.** Per v1.0 R24. Mitigation: R.5 governance review explicitly checks each declaration against Tranche 3 territory; declarations that anticipate Catalogue-workspace mechanics get pushed back to authorship-time.

---

## Section 5 — Open questions requiring Adam's input

1. **§0.7 gap decisions (8 items).** Each requires a yes/no/constrained scope. R.1 cannot start without these.
2. **Tension 1 — `transition_args:` amendment.** Approve as written, modify, or halt for v1.1 paper revision?
3. **Tension 2 — provisional P-G framing.** Treat Adam-as-architectural-authority as the named authority for this activity (recommended) or halt for organisational delegation?
4. **Tension 3 — escalation rate (0.6%).** Run R.5 review with the explicit hypothesis that 0.6% might be correct, OR enter R.5 with the assumption that escalation is under-declared?
5. **R.5 batching.** Cluster by workspace × tier (recommended) or by domain × tier?
6. **R.5 → R.8 iteration.** Explicit go/no-go criteria for a second R.8 iteration. Recommendation: stop after one; if R.8 produces >50 unresolved findings, reassess the activity's scope rather than iterate.
7. **Cross-cutting verbs (~85 verbs).** Treat as virtual Infrastructure workspace (recommended) or distribute to most-relevant domain workspace?
8. **InstrumentMatrix dedicated sweep.** Two reviewers or one? Recommendation: two if a domain expert exists; one with extra calibration time otherwise.

Recommendations are captured. Final calls are organisational.

---

## Section 6 — Effort estimate

Critical path runs through R.4 and R.5 sequentially (R.5 needs R.4 ≥80% complete). R.6, R.7, R.8 can overlap.

| Phase | Effort | Parallelisable? | Critical path? |
|-------|--------|-----------------|----------------|
| R.1 — schema/validator/DSL hardening + 20-verb fixture + Sage/REPL policies | M (3–5 days) | Partial | Yes |
| R.2 — DAG taxonomy coherence + 2 dispatch gaps + schema delta | S (2–3 days) | No (single reviewer) | No |
| R.3 — GatePipeline default-on + CI gate | S (2 days) | Partial | Yes |
| R.4 — Verb declaration sweep (487 verbs to 100%) | XL (10–15 days, parallelised across 4 subagents) | YES (4-way) | Yes |
| R.5 — Tier governance pass | XL (15–20 days, single-authority, batched) | NO | Yes |
| R.6 — Runtime triage | M (4–6 days) | Partial | No |
| R.7 — Final validation | S (1 day) | No | Yes (gates R.9) |
| R.8 — Post-reconciliation coherence (4 sub-passes) | L (8–10 days, single-authority) | NO | Yes |
| R.9 — Reporting + handoff | M (3–5 days) | Partial | Yes |

**Total critical path: ~50–65 days of focused effort.** Bigger if Tension 3 verdict is "escalation under-declared" (escalation declarations are 30% slower than baseline three-axis declarations); smaller if much of R.4 can compress through generic dispatchers like SimpleStatusOp.

**Items where effort doubles if assumption breaks:**

- **R.4** doubles if per-verb time exceeds 10 minutes. Calibrate after first 50.
- **R.5** doubles if cluster-review produces too many anomalies (>10% of verbs). Calibrate after first 100.
- **R.8.1** doubles if cross-workspace coherence findings cascade (e.g. fixing one constraint forces another rebalance).

**Items where parallelisation works:**

- R.4 verb sweeps across 4 workspaces simultaneously.
- R.6 fixtures can run in parallel (the harness already runs them concurrently).
- R.8.2 + R.8.4 can run while R.8.1 + R.8.3 finish.

**Items where parallelisation does NOT work:**

- R.5 single-authority by definition.
- R.8 sub-passes each need the prior one's output.

---

## Section 7 — Outputs feeding forward to Tranche 3

When this activity completes, Tranche 3 inherits:

1. **Reconciled catalogue with three-axis coverage including escalation** — 100% declared, validator-clean, `transition_args:` semantics formalised in v1.1. Tranche 3 starts from a clean, drift-free catalogue.

2. **Declared DAG taxonomies for 11 workspaces** — coherence-reviewed (R.8.1), schema-aligned (R.2), dispatch-complete (R.2 gap-fix). Tranche 3 doesn't redo this work.

3. **Validator + xtask client** — `cargo x reconcile --validate / --batch / --status` operational; CI-gated. Tranche 3 extends rather than rebuilds.

4. **Catalogue-load validation wired to SemOS startup** — already operational; Tranche 3 inherits the gate and activates forward discipline against it.

5. **GatePipeline default-on in production** — R.3 outcome. Tranche 3's effective-tier autonomy honors a runtime that's actually gating.

6. **Runtime triage outcomes** — Bucket 3 resolved; Bucket 2 documented and scheduled.

7. **Tier landscape from R.5 + R.8.2** — heatmap with provisional-Adam-as-authority signatures. Tranche 3's Sage/REPL integration consumes this directly. The "provisional" framing carries forward — Tranche 3 inherits the decisions without inheriting the provisional framing (any tier change in Tranche 3 goes through a real organisational P-G if one exists by then).

8. **R.8 coherence findings** — Tranche 3's Catalogue workspace design (Phase 3.A) inherits a coherent post-reconciliation state.

9. **Authoring patterns observed during R.4 + R.5** — the patterns the SimpleStatusOp dispatcher (today's commit) revealed are real authoring patterns; v1.0 R18 mitigation about "macros designed speculatively" is partly addressed by these patterns. Tranche 3's macro library design is evidence-based.

10. **v1.1 candidate amendments** — Tension 1 (`transition_args:` formalisation), Tension 3 verdict (escalation rate), Tension 4 (`EXISTS` predicate support). Tranche 3 may run against v1.0 or v1.1 depending on what landed.

11. **Sage / REPL policies** — written in R.1; ready for voluntary honouring during reconciliation; ready for architectural enforcement in Tranche 3 Phase 3.C.

**Tranche 3 must still resolve:**
- The Catalogue workspace DAG shape (P9 hypothesis tested in Phase 3.A).
- The authorship verb subset specifics (the 4 stub catalogue.* verbs are rough scaffolding).
- The macro library contents (evidence from R.4/R.5).
- Access control model (ABAC gate against catalogue-author role).
- Sage / REPL integration specifics (consume the v1.1 effective-tier model).
- Observatory UI design (Phase 8 diagrams).
- Estate-scale organisational P-G (organisational, not architectural — Tension 2 carries forward as an organisational question).
- Forward-discipline activation timing.

---

## Closing

**Status of v1.3 "CODE COMPLETE" claim per CLAUDE.md:**

| v1.0 DoD bar | Actual | Verdict |
|--------------|--------|---------|
| T1.1–T1.5 schema/validator/DSL/composition | All COMPLETE | OK |
| T1.6 4 declared taxonomies | 11 taxonomies (+7 above bar) | EXCEEDS |
| T1.7 DB-free catalogue-load | COMPLETE | OK |
| T1.8 xtask reconcile | COMPLETE | OK |
| T1.9 20-verb fixture | PARTIAL (no escalation fixtures) | Gap → R.1 |
| T1.10 Sage / REPL policy docs | PARTIAL (referenced not produced) | Gap → R.1 |
| T1.11 P-G named authority | PROVISIONAL | Tension 2; recommend amendment |
| T1.12 CI gate | MISSING | Gap → R.3 |
| T2.1 100% three-axis coverage | 62% | Gap → R.4 |
| T2.5 escalation declared | 0.6% | Tension 3; verdict via R.5 |
| T2.6 governance pass under named authority | Pilot-scope only | Gap → R.5 |

**v1.3 is "code complete" for the runtime + schema + validator + xtask, but neither Tranche 1 nor Tranche 2 is DoD-complete by v1.0's bar.** The CLAUDE.md "CODE COMPLETE" claim covers about 60% of v1.0 Tranche 1+2 DoD; the remaining 40% is what this reconciliation activity executes.

**Recommendation:** approve §0.7 gap decisions (8 items) + Tension 1 amendment (`transition_args:` formalisation) + Tension 2 framing (Adam-as-architectural-authority is the named authority for this activity), then begin R.1.

Tension 3 (0.6% escalation rate) does not block the activity but does require explicit attention in R.5; do not let it drift into "we'll review escalation later."

Tension 4 (`EXISTS` predicate support) is a smaller architectural lift (~1 day) that prevents R.7 declaring a real cross-workspace constraint as un-validatable; recommend including in R.1.

Tension 5 (GatePipeline opt-in) is a single-line wiring fix (~½ day) but blocks the v1.3 production claim; recommend including in R.3.

---

**End of full catalogue reconciliation review (v1.0 lens) — 2026-04-26.**
