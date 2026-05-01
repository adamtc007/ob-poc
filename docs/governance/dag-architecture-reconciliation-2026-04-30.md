# DAG Architecture Reconciliation — 2026-04-30

> **Status:** v1.0 — recommendation pending Adam's sign-off on Part 2.
> **Inputs:** the five companion documents in `/Users/adamtc007/Downloads/zed-bundle/`, plus direct reading of the codebase under `/Users/adamtc007/Developer/ob-poc/rust/`.
> **Output of:** the reasoning task specified in `00-PROMPT.md`.
> **Author:** Zed-Claude.
> **Output:** zero code changes; one document.

---

## Summary (read first)

**Recommendation: Option C — composed resolved template.** Authors keep authoring DAG taxonomies (`rust/config/sem_os_seeds/dag_taxonomies/*.yaml`) and constellation maps (`rust/config/sem_os_seeds/constellation_maps/*.yaml`) as today. A new resolver layer in `rust/crates/dsl-core/src/resolver/` composes them into a per-(composite_shape, workspace) `ResolvedTemplate`, which is the authoritative upstream object the gate dispatcher reads. This is the only option that (a) honours Vision I9 by quarantining shape-specific composition inside the resolver, (b) avoids regenerating 35 hand-authored constellation maps, (c) lets the predicate-vs-hydration responsibility split survive intact, and (d) matches an intent already half-built into the codebase: `dag_registry.rs:88-92` declares the `transitions_by_verb_fqn` index and explicitly names it as "the foundation for hooking GateChecker into verb dispatch."

**Three most important things in the recon I confirmed or corrected:**

1. **Confirmed.** Two slot schemas exist exactly as Codex reports them. `Slot` in `rust/crates/dsl-core/src/config/dag.rs:160-202` carries DAG-taxonomy lifecycle data (state machines, parent_slot, dual_lifecycle, periodic_review_cadence, category_gated, suspended_state_exempt, predicate bindings). `SlotDef` in `rust/crates/sem_os_core/src/constellation_map_def.rs:21-52` carries constellation hydration data (table, pk, join, occurrence, cardinality, depends_on, placeholder, overlays, edge_overlays, verbs palette, children, max_depth). Field sets do not overlap meaningfully and have evolved separately.

2. **Confirmed and extended.** No base→ancestor→leaf shape-rule composition engine exists. The closest analogue, `load_constellation_stack` in `rust/src/sage/valid_verb_set.rs:161-173`, is hardcoded selection (`if id.starts_with("struct.")` stacks `[group.ownership, id, kyc.onboarding]`). That hardcoded prefix-check is itself an "if banana then apple" pattern at the loader layer — flagging it because it's the same anti-pattern Vision I9 forbids in user-space, surfaced one layer up. Structure macros (`rust/config/verb_schemas/macros/struct-*.yaml`) encode jurisdiction, structure-type, required-vs-optional roles, document bundles, and trading-profile flavour as procedural macro-expansion steps; concrete `struct_*` constellation maps duplicate some of that knowledge as authored slot inventories.

3. **Codex missed two things, and they matter.** First, a `gates/` module already exists in `rust/crates/sem_os_core/src/gates/` — but it is **publish gates** (snapshot validation: `PublishGateResult`, `GateMode::Enforce/ReportOnly`, `GateSeverity::Error/Warning`, `evaluate_publish_gates`), not dispatch gates. The gate-contract document's proposed `Gate::Attach/Progress/Discretionary/Populate` enum will name-collide with this module unless a different namespace or type prefix is chosen. Second, a verb dispatch chain already exists (`DslExecutor` + `dsl_v2::ExecutionContext` (30 fields) + `SemOsVerbOpRegistry` + `GenericCrudExecutor`, bridged by `rust/src/sem_os_runtime/verb_executor_adapter.rs`). The gate-contract spec implicitly proposes a *new* dispatcher; the realistic shape is a gate-checking layer that wraps or precedes this existing dispatch chain, not a replacement.

**Open questions I cannot answer from code alone (need Adam's call):**

- **Q4 (canonical shape taxonomy source).** Multiple plausible candidates exist (`shared_atoms/fund_structure_type.yaml`, `config/ontology/entity_taxonomy.yaml`, constellation FQN convention, structure macros), each satisfying a different subset of the requirement. None is a typed parent/child shape taxonomy today. This is a design decision, not a code-readable fact.
- **Q9 (discretionary verb identifiability).** `verb_contract.rs:71-97` carries `HarmClass` and `ActionClass` enums that signal but do not unambiguously classify. Phase 7 of the existing implementation TODO is the right home for the human-reviewed sweep; until that lands, `DiscretionaryGate` metadata cannot be authored confidently.
- **Q3 (folding `config/role_cardinality.yaml`).** I recommend "migrate into slot `gate_metadata` and sunset," but the retention/sunset choice is Adam's — there may be downstream consumers of the legacy file I cannot see.

---

## Part 1 — Recon validation

Codex's recon (`nom-equivalent-metadata-inventory-2026-04-30.md`, dated 2026-04-30) is **substantially correct**. I confirm each of its substantive claims and extend them with three findings Codex did not surface.

### 1.1 Two slot schemas — confirmed

Both struct definitions read as Codex describes.

**DAG taxonomy slot** (`rust/crates/dsl-core/src/config/dag.rs:160-202`):

```rust
pub struct Slot {
    pub id: String,
    pub stateless: bool,
    pub rationale: Option<String>,
    pub state_machine: Option<SlotStateMachine>,    // structured or string-reference
    pub requires_products: Vec<String>,
    pub parent_slot: Option<ParentSlot>,
    pub state_dependency: Option<StateDependency>,
    pub dual_lifecycle: Vec<DualLifecycle>,
    pub periodic_review_cadence: Option<PeriodicReviewCadence>,
    pub category_gated: Option<CategoryGated>,
    pub suspended_state_exempt: bool,
    pub extra: BTreeMap<String, YamlValue>,         // forward-compat
}
```

**Constellation map slot** (`rust/crates/sem_os_core/src/constellation_map_def.rs:21-52`):

```rust
pub struct SlotDef {
    pub slot_type: SlotType,                        // Cbu | Entity | EntityGraph | Case | Tollgate | Mandate
    pub entity_kinds: Vec<String>,
    pub table: Option<String>,
    pub pk: Option<String>,
    pub join: Option<JoinDef>,
    pub occurrence: Option<usize>,
    pub cardinality: Cardinality,                   // Root | Mandatory | Optional | Recursive
    pub depends_on: Vec<DependencyEntry>,
    pub placeholder: Option<String>,                // not bool — string variant tag
    pub state_machine: Option<String>,              // by name; resolves against state_machines/*.yaml
    pub overlays: Vec<String>,
    pub edge_overlays: Vec<String>,
    pub verbs: BTreeMap<String, VerbPaletteEntry>,
    pub children: BTreeMap<String, SlotDef>,
    pub max_depth: Option<usize>,
}
```

Field-by-field comparison: the two share only the noun "slot" and the strings "id" / "state_machine." DAG taxonomy slots own predicate/state lifecycle; constellation map slots own substrate hydration shape and verb-palette gating by state. **Codex's claim that "they've evolved separately and serve different purposes" is correct.**

Two minor extensions. (a) `ConstellationMapDefBody` carries a `bulk_macros: Vec<String>` field at the body level (`constellation_map_def.rs:17`) that Codex did not enumerate — minor, but it's an authored-content channel. (b) The constellation-runtime projection in `rust/src/sem_os_runtime/constellation_runtime.rs:55` defines a *third* `SlotDef` (the runtime variant) with additional fields including `placeholder_detection`, distinct from the registry value type at `sem_os_core::constellation_map_def::SlotDef`. So strictly speaking there are **three** slot shapes — two authored types and one runtime projection — but the runtime projection is a derivation from `sem_os_core::SlotDef`, so Codex's "two schemas" framing remains the right one for architectural decisions.

### 1.2 No rule-composition engine — confirmed; with one finding to surface

A targeted search across `rust/crates/dsl-core/`, `rust/crates/sem_os_core/`, `rust/src/sem_os_runtime/`, `rust/src/sage/`, `rust/src/semtaxonomy*/`, and `rust/src/taxonomy/` for terms `shape_rule`, `shape_taxonomy`, `ResolvedTemplate`, `DagTemplate`, `compose`, `refinement`, `merge_slot`, `overlay_slot`, `base_rules`, `leaf_shape`, `fund_vehicle` finds **zero matches** for the rule-composition concept the Shape Doc §4 assumes.

What does exist and is sometimes confused with composition:

- **`load_constellation_stack(id)` in `rust/src/sage/valid_verb_set.rs:161-173`** — returns a `Vec<ConstellationMapDefBody>`, not a merged object. For `struct.*` IDs, hardcoded to stack `[group.ownership, id, kyc.onboarding]`. This is **selection plus list**, not composition — the verbs and slots from each map remain independent.
- **`compute_valid_verb_set_for_constellations` in `rust/src/sage/valid_verb_set.rs:212-251`** — iterates each constellation independently, computes its verb set, then **deduplicates and unions by FQN**. This is verb-set union, not template merging.
- **`ConstellationModel::from_parts` → `flatten_slots` in `rust/crates/sem_os_core/src/grounding.rs:74-85`** — takes one constellation map, flattens nested children into a single slot map. Hierarchy collapse, not multi-map composition.
- **`DagRegistry` in `rust/crates/dsl-core/src/config/dag_registry.rs:1-100`** — pre-indexed snapshot of the loaded DAGs. Read-only after construction. Indexes constraints by target transition, derived states by host slot, parent slots, children, and (notably) `transitions_by_verb_fqn`. Not composition; the enabling lookup layer for whatever composition is built later.
- **`ConstellationMapDef::overlays: Vec<String>` and `edge_overlays: Vec<String>`** at `rust/src/sem_os_runtime/constellation_runtime.rs:73-75` — string names of runtime data overlays evaluated during hydration. Names a data-enrichment layer, not a schema-composition mechanism.

Finding to surface: **the hardcoded `if id.starts_with("struct.")` at `valid_verb_set.rs:162` is itself an "if banana then apple" pattern.** It was added because the codebase needed *some* way to compose the ownership-graph + structure + KYC perspectives for `struct.*` constellations, and no general mechanism exists. Whatever Phase 1.5 designs as a resolver, it should subsume this hardcoded prefix-check; otherwise we ship Phase 1.5 with the anti-pattern still living in the codebase one layer up from where Vision I9 forbids it.

### 1.3 Structure macros encode shape-specific procedural knowledge — confirmed, with concrete examples

Five `struct-*.yaml` macros: `struct-cross-border.yaml`, `struct-ie.yaml`, `struct-lux.yaml`, `struct-uk.yaml`, `struct-us.yaml`. Reading `struct-lux.yaml` (M1 `struct.lux.ucits.sicav`, M2 `struct.lux.aif.raif`, M3 `struct.lux.pe.scsp`) and `struct-ie.yaml` (M4 `struct.ie.ucits.icav`, M5 `struct.ie.aif.icav`, M6 `struct.ie.hedge.icav`) yields the following concrete shape-specific facts encoded procedurally:

- **Jurisdiction is hardcoded per macro.** `struct.lux.ucits.sicav` emits `jurisdiction: LU` at `struct-lux.yaml:88`; `struct.ie.ucits.icav` emits `jurisdiction: IE` at `struct-ie.yaml:89`. Same field, same purpose, two macros.
- **Structure type is hardcoded.** `struct.lux.ucits.sicav` emits `structure-type: ucits` (line 88); `struct.lux.aif.raif` emits `structure-type: aif` (line 283); `struct.lux.pe.scsp` emits `structure-type: pe` (line 487).
- **Required-vs-optional roles vary by shape.** UCITS macros require `management_company` and `depositary` (struct-lux.yaml:34-49, struct-ie.yaml:34-49). AIF RAIF requires `aifm` and `depositary` (struct-lux.yaml:218-230). PE SCSp requires `general_partner` only (struct-lux.yaml:430-435). Hedge ICAV requires `aifm`, `depositary`, and `prime_broker` (struct-ie.yaml:480-498). The required/optional partition encodes regulatory regime, not declarative slot metadata.
- **Document bundle selection per shape.** `docs.bundle.ucits-baseline` (struct-lux.yaml:97), `docs.bundle.aif-baseline` (struct-lux.yaml:291), `docs.bundle.private-equity-baseline` (struct-lux.yaml:494), `docs.bundle.hedge-baseline` (struct-ie.yaml:551). Bundle choice = shape choice.
- **Trading profile flavour per shape.** `profile_type: ucits | aif | pe | hedge` per macro. Hedge ICAV additionally sets `leverage_permitted: true, short_selling_permitted: true` (struct-ie.yaml:589-591). These are post-creation runtime configuration, embedded in the macro body.
- **Macro-level composition exists but is procedural.** `struct.ie.hedge.icav` invokes `struct.ie.aif.icav` via `invoke-macro` at `struct-ie.yaml:535-547`, importing `@cbu` and `@trading-profile` symbols. This is verb-sequence composition — the kind of composition the macro language already supports — not slot/template composition.

So Codex's claim is correct, and the texture is: structure macros are the *de facto* shape-rule layer today, but they encode the shape-specific facts as ordered verb invocations rather than as declarative slot metadata. The Shape Doc §4 vision (composing base rules with shape rules to produce a resolved DagTemplate) does not match what the macros actually do.

### 1.4 12 DAG taxonomy YAMLs and 35 constellation map YAMLs — confirmed

Counted via `ls`:

- `rust/config/sem_os_seeds/dag_taxonomies/`: 12 files (book_setup_dag, booking_principal_dag, catalogue_dag, cbu_dag, deal_dag, instrument_matrix_dag, kyc_dag, lifecycle_resources_dag, onboarding_request_dag, product_service_taxonomy_dag, semos_maintenance_dag, session_bootstrap_dag).
- `rust/config/sem_os_seeds/constellation_maps/`: 35 files. Of these, 18 are workspace/lifecycle maps (`cbu_workspace`, `deal_lifecycle`, `kyc_extended`, `kyc_onboarding`, etc.); 17 begin with `struct_` and are jurisdiction-specific (`struct_lux_ucits_sicav`, `struct_ie_ucits_icav`, `struct_us_private_fund_delaware_lp`, etc.).
- `rust/config/sem_os_seeds/state_machines/`: 20 files (Codex says 20; correct).
- `rust/config/sem_os_seeds/constellation_families/`: 24 files (Codex says 24; correct).
- `rust/config/sem_os_seeds/shared_atoms/`: 5 files.
- `rust/config/verb_schemas/macros/struct-*.yaml`: 5 files.

The DAG taxonomies own predicate lifecycle (`green_when` on states; `predicate_bindings`; `cross_workspace_constraints`; `derived_cross_workspace_state`) and state-machine surface (states/transitions/terminal_states). The constellation maps own hydration shape (table/pk/join), entity-kind eligibility, and per-state verb palette. Their concerns do not overlap; their slot ids do partially overlap (both name `cbu`, `management_company`, `mandate`, etc.) but with different semantic contracts. **Codex's distinction (predicate lifecycle vs hydration/action surface) is correct.**

A small path correction: Codex's recon writes paths as `crates/dsl-core/...` and `config/sem_os_seeds/...`. The actual prefix in this repo is `rust/crates/dsl-core/...` and `rust/config/sem_os_seeds/...`. This is a documentation issue, not a substance issue, but Phase 1.5 deliverables must use the actual paths.

### 1.5 Three things Codex did not surface

These are material to Parts 2–4 and Adam should weigh them before signing off.

**(A) Name collision: `gates/` already exists in `sem_os_core`.**

`rust/crates/sem_os_core/src/gates/` (`mod.rs`, `governance.rs`, `technical.rs`) is a complete, tested, in-production gate framework — but for **publishing snapshots of registry artefacts**. Types exposed include `GateResult`, `GateFailure`, `GateSeverity::{Error, Warning}`, `GateMode::{Enforce, ReportOnly}`, `PublishGateResult`, `ExtendedPublishGateResult`, `UnifiedPublishGateResult`, plus check functions `check_proof_rule`, `check_security_label`, `check_governed_approval`, `check_version_monotonicity`, `check_evidence_proof_rule`, `check_derivation_cycle`, `check_derivation_evidence_grade`, `check_derivation_type_compatibility`, `evaluate_publish_gates`, `evaluate_extended_gates`, `evaluate_all_publish_gates`.

The dag-gate-contract spec proposes `Gate::Attach`, `Gate::Progress`, `Gate::Discretionary`, `Gate::Populate` for **dispatching DAG-driven verbs**. Same word, completely orthogonal concept.

Implications:

- The proposed path `rust/crates/dsl-core/src/dispatch/gate/` (gate-contract spec §4.2) avoids a *file* collision because it lives in a different crate. Good.
- The type name `Gate` will collide if anyone ever imports both `sem_os_core::gates::*` and `dsl_core::dispatch::gate::Gate`. The publish-gate types do not export a top-level `Gate` enum, so the collision is latent rather than immediate, but it is a footgun.
- Adam should pick a naming convention now. Recommended: rename the dispatch-side types to `DispatchGate::Attach`, `DispatchGate::Progress`, `DispatchGate::Discretionary`, `DispatchGate::Populate`, and put them under `dsl_core::dispatch::*` with module paths that read `dsl_core::dispatch::dispatch_gate::DispatchGate`. The phrase "dispatch gate" also reads more clearly in error messages.

**(B) The lookup foundation for gate-checker dispatch is already built.**

`rust/crates/dsl-core/src/config/dag_registry.rs:88-92`:

```rust
/// transitions_by_verb_fqn[verb_fqn] → list of transitions that
/// declare this verb in their `via:` field. Used by runtime to
/// answer "what transitions could this verb cause?" — the
/// foundation for hooking GateChecker into verb dispatch.
transitions_by_verb_fqn: HashMap<String, Vec<TransitionRef>>,
```

The phrase "GateChecker into verb dispatch" tells us this architecture has been anticipated. The DAG registry already provides the verb-FQN → DAG-transition lookup that any dispatch gate resolver needs. This is encouraging for Option C (the resolver has its lookup table) and confirms the implicit direction: gates resolve from the DAG taxonomy via the registry index, with constellation-map data composed in to fill hydration-shape fields the DAG side does not own.

**(C) Substantial verb dispatch chain already exists.**

`rust/src/sem_os_runtime/verb_executor_adapter.rs:1-15` documents the existing dispatch chain: `DslExecutor` (in `dsl_v2::executor::DslExecutor`) drives verb execution with a 30-field `dsl_v2::ExecutionContext`. Dispatch routes to `SemOsVerbOpRegistry`, `CrudExecutionPort` / `GenericCrudExecutor`, or DslExecutor's plugin path, depending on contract behaviour. The `ObPocVerbExecutor` adapter implements the SemOS port (`dsl_runtime::VerbExecutionPort`) over this stack.

Implication for the gate-contract spec: Phase G2's "dispatcher skeleton" should be specified as a **gate-checking layer that wraps the existing dispatch chain**, not a parallel new dispatcher. Concretely:

- Pre-execution checks (G4) run before `DslExecutor::execute` is invoked.
- Verb body execution (G7) is the existing `DslExecutor` body call.
- Post-execution checks (G5) run after the body returns and before the dispatcher commits.
- The `legacy_adapter` referenced in Phase G7.5 exists implicitly as the bridge layer in `verb_executor_adapter.rs` plus whatever per-verb dispatch wiring lives in `dsl_v2::executor`. The adapter excision must not break existing dispatch.

This is not a re-scoping of Phase G — it is an instruction to not double-build the dispatcher. The gate-contract spec implicitly assumes a greenfield dispatcher; the real shape is integration with `DslExecutor`. Phase 1.5A's resolver design should account for this so Phase G3 has a clean handle.

### 1.6 Specific Codex questions answered by code

A few of Codex's open questions are answerable directly from code, ahead of Part 3:

- **Q5 (entity_kinds as v1 EligibilityConstraint):** Yes; `entity_kinds: Vec<String>` is already authored on every constellation slot and is the only typed eligibility surface that exists. Defer typed-shape-taxonomy eligibility to Phase 2.
- **Q6 (transition `precondition` migration):** `precondition: Option<String>` exists at `dag.rs:387` on `TransitionDef`. The implementation TODO already scopes Phase 10 (`precondition cleanup`) for stripping it; Phase 1.5 should not touch it.
- **Q9 (discretionary verb signal):** Partial signal exists in `verb_contract.rs:71-97` via `HarmClass` and `ActionClass` enums. Not unambiguous; Phase 7's human-reviewed sweep is the right home.

These are picked up in Part 3 with the corresponding citations.

---

## Part 2 — Architectural recommendation

**Decide between A, B, C, or other.** I recommend **Option C**, with one caveat about naming and one about the migration of procedural macros.

### 2.1 Option A — DAG taxonomy authoritative; constellation maps become generated projections

**What this would mean.** The DAG taxonomy schema (`Slot` in `dag.rs`) absorbs the gate-metadata fields (closure, eligibility, cardinality_max, attachment/addition predicates, role guards, audit class, completeness assertions). The constellation maps (`SlotDef` in `constellation_map_def.rs` plus 35 YAML files) become outputs of a generator that reads the DAG taxonomy plus a (new) shape-rule layer. The runtime hydrator (`constellation_runtime.rs`) consumes generated maps as today.

**What this disrupts.** The 35 hand-authored constellation maps go away or become generator outputs. The 17 `struct_*` jurisdiction-specific maps in particular were authored over the past quarter and represent significant investment. The DAG taxonomy schema would have to grow new fields for hydration shape (table, pk, join, occurrence, overlays, edge_overlays, verb palette by state), which is currently entirely out-of-scope for DAG taxonomies. The DAG schema balloons.

**I9 risk.** Forces hydration logic into DAG taxonomy authorship. Any author wanting to add a new constellation map must now author DAG-taxonomy + shape-rule + (something). The cognitive load multiplies, and the schema grows surface area for "if banana then apple" patterns at the schema-coordination layer (which constellation YAML do I emit for shape X?).

**Verdict.** Disrupts existing authoring most. Forces the wrong schema to grow.

### 2.2 Option B — Constellation maps authoritative; DAG taxonomy becomes a predicate/lifecycle layer

**What this would mean.** The constellation map schema absorbs the gate-metadata fields. The DAG taxonomies become a per-workspace predicate-and-lifecycle layer that constellation maps reference (the way they already reference `state_machine: entity_kyc_lifecycle`). The 12 DAG taxonomy YAMLs would be edited to remove fields that move into constellation maps, and the constellation maps would gain authored gate-metadata blocks per slot.

**What this disrupts.** The 12 DAG taxonomies are authored richly with cross-workspace constraints, derived states, dual lifecycles, periodic review cadences, parent-slot hierarchies, and category gates — these are workspace-level structural facts that span many constellation maps. Forcing them into per-constellation-map YAML would either duplicate the data across maps that share workspaces or require the constellation map schema to grow a workspace-shared-metadata layer (which is essentially re-inventing the DAG taxonomy with a different name).

**I9 risk.** Cross-workspace predicates currently live in DAG taxonomy. Examples: a CBU verb whose `green_when` reads the parent deal's state, the `derived_cross_workspace_state.cbu_operationally_active` derivation that combines KYC + Deal + IM + evidence. These are not constellation-map concerns (a constellation map is one shape's hydration projection). Forcing them into constellation maps either fragments the predicate language across 35 files or recreates the cross-workspace-constraint mechanism per-map.

**Verdict.** Discards the most-mature authored asset (DAG taxonomies) and breaks the cross-workspace predicate model.

### 2.3 Option C — Composed resolved template (recommended)

**What this would mean.** Both schemas remain authoritative for what they currently own. A new resolver layer composes them at template-resolution time into a `ResolvedTemplate { workspace, composite_shape, slots: Vec<ResolvedSlot>, transitions: Vec<ResolvedTransition>, version: VersionHash }` structure. Each `ResolvedSlot` carries:

- predicate/lifecycle data inherited from the matching DAG-taxonomy `Slot` (state machine, predicate bindings, parent_slot, dual_lifecycle, periodic_review_cadence, category_gated, suspended_state_exempt, derived states the slot hosts, cross-workspace constraints targeting transitions of this slot)
- hydration data inherited from the matching constellation-map `SlotDef` (table, pk, join, entity_kinds, cardinality, depends_on, placeholder, overlays, edge_overlays, verb palette by state, children, max_depth)
- gate metadata (closure, eligibility, cardinality_max, entry_state, attachment_predicates, addition_predicates, aggregate_breach_checks, role_guard, justification_required, audit_class, completeness_assertion, shape_refinement_origin) authored on whichever schema owns it

The dispatcher's slow path (`Phase G3 — Gate resolution from DagTemplate`) reads the `ResolvedTemplate` for the active composite, looks up the slot, and packages the right gate variant. The cache (`Phase G6`) keys on `(composite_uuid, slot_id, version)`. The lookup index is the existing `dag_registry.rs` `transitions_by_verb_fqn` plus a parallel index over the constellation map slot palette — both already in place or trivially derivable.

**What this disrupts.** Approximately nothing in existing authoring. Authors keep editing DAG taxonomy YAMLs and constellation map YAMLs as today. The gate-metadata block is additive on both schemas (Phase 1.5B). The 5 `struct-*.yaml` structure macros stop being the source of structural truth and become procedural workflows, which is what they were always meant to be — but they can stay valid as macros for the existing onboarding flow.

**I9 honoured.** The resolver is a single, central place where shape-conditional logic lives. User-space verb bodies see only the `ResolvedTemplate`'s output. The hardcoded `if id.starts_with("struct.")` at `valid_verb_set.rs:162` either gets subsumed by the resolver or stays in `valid_verb_set.rs` until that module is rewritten against the resolver — but the long-term plan eliminates it.

**Resolver has a clean home.** New crate-internal module: `rust/crates/dsl-core/src/resolver/` (mod.rs, composer.rs, manifest.rs). The resolver takes (composite_shape, workspace, dag_registry, constellation_map_registry, role_cardinality_lookup) and returns a `ResolvedTemplate`. Pure function. Cacheable. No I/O at lookup time.

**Phase 1.5 specifiable concretely.** See Part 4. Sub-phases A (decide), B (additive metadata), C (pilot backfill on Lux UCITS SICAV CBU), D (resolver skeleton + manifest). Each has acceptance criteria against real files.

**Migration story for procedural macros.** Phase 2 (shape-aware generation) becomes the sweep: extract the shape-specific facts from `struct-*.yaml` (jurisdiction, structure-type, required-vs-optional roles, document bundle, profile_type, leverage flags) into a new authored shape-rules layer (likely `rust/config/sem_os_seeds/shape_rules/` once Q4 is decided). The macros remain valid as ordered verb sequences; their structural-truth role moves into the resolver's input set.

**Match to existing intent.** `dag_registry.rs:88-92`'s "foundation for hooking GateChecker into verb dispatch" is exactly this shape: the registry index lookup serves the resolver, which produces the gate, which the dispatcher uses. No code change is needed to enable this; the lookup is already there.

**Verdict.** Disrupts least, honours I9, gives the resolver a clean home, lets Phase 1.5 be specified concretely, has the cleanest migration story for procedural macros, and matches an intent already half-built into the codebase.

### 2.4 Caveats on Option C

**Naming.** Adopt the Shape Doc's "DagTemplate" / generator language for Option C's resolver, but pick concrete Rust names that do not collide with existing types:

- `ResolvedTemplate` (the output of the resolver) — distinct from "DagTemplate" because it includes both DAG-taxonomy and constellation-map data.
- `Resolver` or `TemplateComposer` (the function/struct that builds it) — avoids "Generator" because that term is overloaded in the codebase (codegen, code generation, runbook generation).
- Dispatch types must avoid the `gates/` collision per finding (A). Use `DispatchGate::Attach/Progress/Discretionary/Populate`, module path `dsl_core::dispatch::dispatch_gate`.

**Schema drift risk.** Option C lets two schemas continue to evolve. Phase 1.5B should add a validator that ensures every DAG-taxonomy slot id used in a constellation map has a matching `Slot` in the workspace's DAG taxonomy, and vice versa for slots that need predicate lifecycles. This validator is the only structural backstop against silent drift between the two schemas. It should run in CI.

**Proposal language to revise.** The Shape Doc's §4 "no new composition mechanism is introduced" is incorrect under Option C — a new composition mechanism *is* introduced (the resolver). The shape doc needs revision to match. The Vision doc's §2.6 closure discipline lands cleanly under Option C: closure/cardinality_max are gate-metadata fields on the resolved slot.

### 2.5 Why not a fourth option

I considered "Option D — publish a unified resolver-target type and incrementally collapse both schemas into it." This is the long-term evolution Option C might enable once the resolver has been load-bearing for two or three quarters and we have a settled view of which fields actually live where. It is too risky for Phase 1.5 because it changes both schemas at once and breaks every existing authored YAML. Better to do C, let the resolver's input/output stability tell us whether unification is justified, and revisit later.

I also considered "no resolver — just teach the dispatcher to read both schemas directly per verb." This recreates Option A or B implicitly (the dispatcher embeds the composition logic) and scatters shape-conditional logic throughout dispatcher code rather than centralising it in a resolver. Rejected on I9 grounds.

---

## Part 3 — Other open questions

Codex's recon §5 lists ten open questions. For each, I state my recommended answer, cite the code that grounds it (or note when the question is a design choice not determinable from code), and flag whether Adam's call is required.

### Q1 — Authoritative gate-source object

**My answer.** Option C: composed resolved template. See Part 2.

**Code grounding.** `dag_registry.rs:88-92` ("the foundation for hooking GateChecker into verb dispatch"); the existing two-schema split in `dag.rs:160-202` and `constellation_map_def.rs:21-52`; the absence of any composition engine confirmed by search.

**Adam's call required.** Yes — this is *the* architectural pivot.

### Q2 — `struct_*` constellation maps: hand-authored or generated?

**My answer.** Hand-authored for Phase 1.5; planned generated outputs by Phase 2.

**Code grounding.** Reading `rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml:1-80`: the file is a complete authored slot inventory with hydration data (`table: cbus`, `pk: cbu_id`, `join: { via: cbu_entity_roles, ... }`, `entity_kinds: [company]`, full verb palette by state). No generator-output marker; no reference to a base template. Confirmed authored. Phase 1.5C uses Lux UCITS SICAV as the pilot precisely because its hand-authored map is the closest existing thing to "what the resolver would produce."

**Adam's call required.** Partial — code says "currently hand-authored." Adam decides the migration timeline (when Phase 2 generator replaces authoring). My recommendation is: do not delete `struct_*.yaml` until the generator's output passes a byte-equivalence test against the hand-authored versions on the pilot pair.

### Q3 — `config/role_cardinality.yaml` folding

**My answer.** Migrate into slot `gate_metadata` on the chosen upstream (per Q1 = Option C, the field lives on whichever of the two schemas owns the slot's cardinality concern; for role-bearing slots that's the constellation map). Annotate during Phase 1.5C; sunset the legacy file in Phase 2 once backfill is complete.

**Code grounding.** Codex §1 reports the file's content (`roles: { depositary: { cardinality: one, context: [ucits, aif, raif] }, investment-manager: { cardinality: one-or-more } }, structure-aliases: ...`). Reading the file directly: the data maps cleanly to `cardinality_max` + `closure` + role-context restriction. No code in the runtime reads it from the file path I checked, so removal is low-risk pending an audit.

**Adam's call required.** Yes for sunset timing — there may be tooling that reads the file I have not surveyed. My recommendation defaults to "annotate then sunset," but Adam may prefer to keep it as a peer-reviewed legacy compatibility file.

### Q4 — Canonical shape taxonomy source

**My answer.** Not determinable from code. Adam's call.

**Code grounding.** Three candidates partially satisfy: `rust/config/sem_os_seeds/shared_atoms/fund_structure_type.yaml` (governed allowed values; flat); `rust/config/ontology/entity_taxonomy.yaml` (Codex §1 reports this defines entity types/subtypes and DB bindings; not a hierarchical shape taxonomy with rules); the constellation FQN convention itself (`struct.lux.ucits.sicav` implies ancestry but as a convention only). Structure macros encode shape-specific facts but are procedural. None is what Shape Doc §4 calls a "typed parent/child shape taxonomy."

**Adam's call required.** Yes. My recommendation: defer the typed shape taxonomy to Phase 2; in Phase 1.5, accept `entity_kinds` (Q5) as the v1 eligibility surface and continue using the constellation-map FQN convention (`struct.lux.ucits.sicav`) for shape identity. New typed taxonomy would land under `rust/config/sem_os_seeds/shape_taxonomy/` modelled on the FQN convention.

### Q5 — `entity_kinds` as v1 EligibilityConstraint

**My answer.** Yes, with a typed `EligibilityConstraint` enum that initially has one variant `EligibilityConstraint::EntityKinds(Vec<String>)` and a path to add `EligibilityConstraint::ShapeTaxonomyPosition(ShapeRef)` in Phase 2.

**Code grounding.** `entity_kinds: Vec<String>` already on every constellation slot at `constellation_map_def.rs:26`; authored across all 35 maps (e.g., `entity_kinds: [company]`, `entity_kinds: [person, company]`, `entity_kinds: [trust]`). The dispatcher's `Ineligible { slot_requires: EligibilityConstraint, candidate_has: ShapeRef }` error variant (gate-contract spec §1.5) works against either constraint kind without changing the error surface. No code change needed for v1; v2 extension is additive.

**Adam's call required.** No — code says yes; gate-contract error vocabulary supports it. Confirms Codex's question with a concrete enum proposal.

### Q6 — Transition `precondition` migration

**My answer.** Not in Phase 1.5. Phase 1.5 adds `attachment_predicates: Vec<PredicateRef>` and `addition_predicates: Vec<PredicateRef>` as new authored fields on `gate_metadata`. The implementation TODO Phase 10 (precondition cleanup) is the right home for stripping legacy `precondition` from non-discretionary transitions.

**Code grounding.** `precondition: Option<String>` exists at `dag.rs:387` on `TransitionDef`. The semos-dag-implementation-todo.md Phase 10 already scopes "Strip vestigial `precondition:` from non-discretionary transitions" with acceptance "Grep finds zero `precondition:` entries in DAG YAML except under discretionary-verb transitions (where the field has been renamed `role_guard:`)."

**Adam's call required.** No — already scoped in the existing TODO. My recommendation: Phase 1.5 gate metadata co-exists with legacy `precondition` for one phase; Phase 10 cleans up.

### Q7 — `nom_rules_version` source

**My answer.** Hash over the resolver's input set: every DAG taxonomy YAML, every constellation map YAML, every standalone state-machine YAML, `role_cardinality.yaml`, plus (eventually) shape-rule YAMLs and the resolver code's own version string. The dispatcher needs *one* version per resolved template; per-DAG or per-constellation hashes do not give the resolver-output cohesion that gate freshness needs.

**Code grounding.** `compute_map_revision(yaml: &str) -> String` exists in `rust/src/sem_os_runtime/constellation_runtime.rs:537` as a per-map content hash and is the right primitive to extend. `ValidatedConstellationMap.map_revision` (per Codex §1) gives one-map versioning; not enough. The resolver's version is a `Sha256` over the sorted concatenation of input file bytes plus the resolver code's `CARGO_PKG_VERSION`. Stable across rebuilds with identical inputs.

**Adam's call required.** Partial — primitive exists, scope of the multi-source hash is a design choice. My recommendation: include `dag_taxonomies/`, `constellation_maps/`, `state_machines/`, `role_cardinality.yaml`, and (when Phase 2 lands) `shape_rules/`. Exclude `verb_schemas/` and other procedural files because they are not gate-resolution inputs.

### Q8 — Reducer conditions to dsl-core predicate AST

**My answer.** Yes — long-term, unify on the dsl-core predicate AST. But not in Phase 1.5; this is Phase 2.x or Phase 8 work.

**Code grounding.** `rust/crates/dsl-core/src/config/predicate/` contains `ast.rs`, `mod.rs`, `parser.rs` — the Phase 1 predicate AST. `rust/crates/sem_os_core/src/state_machine_def.rs` (per Codex §1) has `ConditionDef` with reducer predicates as string expressions. Two parsers exist; two evaluators implied. Per Codex's §3.4 ninth bullet, this is doc/spec drift the recon flagged.

Two choices:

- (a) Translate reducer conditions to `dsl-core` predicate AST. Single language; lints once; one evaluator. Long-term right.
- (b) Keep reducer conditions as a separate local DSL. Faster delivery; permanent maintenance burden.

I recommend (a) but it is a separate workstream because it touches `state_machine_def.rs`, `reducer_runtime.rs`, every reducer YAML, and the predicate AST simultaneously. Phase 1.5 should land gate metadata using `dsl-core` predicates exclusively (gate fields like `attachment_predicates: Vec<PredicateRef>` reference the dsl-core AST). Reducer-condition migration is a parallel workstream.

**Adam's call required.** Yes for sequencing. Code-determinable that two parsers exist; design choice on when to converge.

### Q9 — Discretionary verb identifiability

**My answer.** Partial signal exists; full classification needs a human-reviewed pass. Phase 7 of the existing implementation TODO is the right home.

**Code grounding.** `rust/crates/sem_os_core/src/verb_contract.rs:71-77`:

```rust
pub enum HarmClass {
    ReadOnly,
    Reversible,
    Irreversible,
    Destructive,
}
```

and `verb_contract.rs:80-97`:

```rust
pub enum ActionClass {
    List, Read, Search, Describe, Create, Update, Delete,
    Assign, Remove, Import, Compute, Review, Approve, Reject, Execute,
}
```

`ActionClass::Reject` is unambiguously discretionary. `ActionClass::Approve` and `ActionClass::Review` often are. `HarmClass::Irreversible | Destructive` are signals but not unique to discretionary verbs.

**Adam's call required.** No for the answer (Phase 7 is already scoped); yes for confirming the sweep can produce a CSV manifest with proposed `flavour: discretionary` annotations for review.

### Q10 — First pilot remains Lux UCITS SICAV CBU

**My answer.** Yes.

**Code grounding.** Three lines of evidence: (a) `rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` is a complete authored constellation with hydration data, entity_kinds, dependency order, and verb palette; (b) `rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml:1-100` shows the most-developed DAG taxonomy among the 12 (R-3 dual-lifecycle re-centring, V1.3 amendments applied, 23 slots per Vision §6.3); (c) the Vision document itself uses Lux SICAV as the worked example (§4.1's CBU `VALIDATED.green_when` references `entity_proper_person`, `entity_limited_company_ubo`, `mandate`, `cbu_evidence` — all richly modelled in `cbu_dag.yaml`).

**Adam's call required.** No — code confirms.

---

## Part 4 — Corrected Phase 1.5 scope

Codex's recon §4 proposed sub-phases 1.5A through 1.5D. The structure is right; I sharpen each with concrete file paths in this codebase, testable acceptance criteria, dependencies, review checkpoints, and explicit in-scope / deferred lists.

### Phase 1.5A — Decide authoritative gate-source object and resolver shape

**Type.** Decision and design specification only. Zero code.

**Files (deliverables).**

- This document (`docs/governance/dag-architecture-reconciliation-2026-04-30.md`) — the recommendation.
- `docs/governance/resolved-template-design-2026-05.md` (new) — the resolver design doc following Adam's sign-off on Option C. Contents: `ResolvedTemplate` type sketch in Rust, input source enumeration, composition rules with conflict precedence, version-hash inputs, caching strategy, freshness/eviction triggers, integration plan with the existing `DslExecutor` dispatch chain.

**Acceptance criteria.**

1. Adam signs off on Option C (or another option with reasoning).
2. The resolver design doc enumerates: input sources (file paths under `rust/config/sem_os_seeds/`); composition rule precedence (when DAG taxonomy and constellation map both define a field, who wins); version-hash inputs (per Q7); caching key and eviction triggers; the integration boundary with `DslExecutor` (where the gate-checking layer wraps the existing dispatch).
3. The design doc's type sketches do not collide with `sem_os_core::gates::*` (per finding A).
4. The design doc explicitly addresses how the resolver subsumes the hardcoded `if id.starts_with("struct.")` at `valid_verb_set.rs:162`.

**Dependencies.** Phase 1 (predicate AST) for typing `attachment_predicates: Vec<PredicateRef>`.

**Review checkpoints.** Adam reads recommendation; one-week comment window; sign-off in writing. The resolver design doc gets its own review round before Phase 1.5B starts.

**In scope.** Architectural choice; design doc; type sketches.

**Explicitly deferred to later phases.** Any code; any YAML changes.

### Phase 1.5B — Additive gate-metadata fields on existing schemas (no resolver yet)

**Type.** Schema extension on both authored types. Additive only — no field removed; no semantic change to existing fields.

**Files.**

- `rust/crates/dsl-core/src/config/dag.rs` — extend `Slot` (lines 160-202) with optional gate-metadata fields:

  ```rust
  pub closure: Option<ClosureType>,                              // ClosedBounded | ClosedUnbounded | Open
  pub eligibility: Option<EligibilityConstraint>,                // EntityKinds(Vec<String>) | ShapeTaxonomyPosition(ShapeRef) (v2)
  pub cardinality_max: Option<u64>,
  pub entry_state: Option<String>,
  pub attachment_predicates: Vec<String>,                        // parsed by Phase 1 AST
  pub addition_predicates: Vec<String>,                          // parsed by Phase 1 AST
  pub aggregate_breach_checks: Vec<String>,                      // parsed by Phase 1 AST
  pub role_guard: Option<RoleGuard>,
  pub justification_required: Option<bool>,
  pub audit_class: Option<AuditClass>,
  pub completeness_assertion: Option<CompletenessAssertionConfig>,
  ```

  All fields `#[serde(default)]` so existing 12 DAG YAMLs parse unchanged.

- `rust/crates/sem_os_core/src/constellation_map_def.rs` — extend `SlotDef` (lines 21-52) with the same gate-metadata block. Constellation slots and DAG slots both get the extension; the resolver decides per-field which side wins (per design doc from 1.5A).

- `rust/crates/dsl-core/src/config/dag_validator.rs` — extend with structural lints:
  - `closure: Open` requires `completeness_assertion` populated.
  - `eligibility::EntityKinds` references known kinds (cross-reference `config/ontology/entity_taxonomy.yaml`).
  - `attachment_predicates` / `addition_predicates` / `aggregate_breach_checks` parse via the Phase 1 predicate AST.
  - **New schema-coordination validator.** Every constellation map slot id used in a workspace whose DAG taxonomy declares slots with the same id must have type-compatible references; surface mismatches as warnings (not errors) until Phase 2 reconciliation.

- `rust/crates/dsl-core/tests/dag_gate_metadata.rs` (new) — round-trip serde tests for the new fields; existing 12 DAG YAMLs still parse unchanged.
- `rust/crates/sem_os_core/tests/constellation_gate_metadata.rs` (new) — round-trip serde for the constellation extension; existing 35 constellation YAMLs still parse unchanged.
- `rust/crates/dsl-core/tests/dag_validator_gate.rs` (new) — synthetic violations of each new lint rule produce the expected error.

**Acceptance criteria.**

1. `cargo test --workspace --features dag,sem_os_core` passes.
2. All 12 DAG taxonomy YAMLs and all 35 constellation map YAMLs parse without changes.
3. Each new lint catches its synthetic violation with a clear error message.
4. `cargo clippy -- -D warnings` clean.
5. The shape-coordination validator runs and produces zero warnings against the current YAMLs (or, if it produces warnings, those are documented in a manifest for Phase 1.5C / Phase 2 to address).

**Dependencies.** Phase 1.5A approved; Phase 1 (predicate AST exists for predicate field parsing).

**Review checkpoints.** Field-set confirmation against the design doc *before* parser/lint code begins. Migration safety review (no breaking change to existing parses) before merge.

**In scope.** Schema extensions and validators only.

**Explicitly deferred.** Authoring the new fields on real YAMLs (1.5C); the resolver (1.5D); gate runtime (Phase G1+); shape-rule input layer (Phase 2).

### Phase 1.5C — Pilot backfill on Lux UCITS SICAV CBU

**Type.** Authoring on real YAML files. High judgement; small scope.

**Files (authored content; not new files).**

- `rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml` — author `closure`, `eligibility`, `cardinality_max`, `entry_state`, `attachment_predicates` on the CBU's bounded slots only (root `cbu`, `entity_proper_person`, `entity_limited_company_ubo`, `manco`, `share_class`, `mandate`, `cbu_evidence`).
- `rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` — for each existing slot:
  - `management_company`, `depositary`, `investment_manager`, `administrator`, `auditor`, `domiciliation_agent`: author `closure: closed_bounded`, `cardinality_max: 1` (depositary, management_company) or as appropriate; author `eligibility: { kind: entity_kind, values: [company] }` matching existing `entity_kinds`.
  - Investor-equivalent slots (when present): `closure: closed_unbounded` with `aggregate_breach_checks` placeholder.
- `rust/config/role_cardinality.yaml` — annotate which roles map to which slots in the gate-metadata model. Do **not** delete this file in 1.5C.
- `rust/crates/dsl-core/tests/lux_sicav_pilot.rs` (new) — assert pilot YAMLs load, validate (Phase 1.5B validator passes), and the slots that should have gate-metadata do.

**Acceptance criteria.**

1. Pilot YAMLs load and parse.
2. Phase 1.5B validators run with zero errors against the pilot pair.
3. Manifest output (Phase 1.5D) lists zero missing required gate-metadata fields for the CBU + struct.lux.ucits.sicav pair.
4. Per-slot CSV manifest produced and reviewed in batches of ≤10 slots per round.

**Dependencies.** Phase 1.5B merged; Q5 (entity_kinds as v1 EligibilityConstraint) confirmed.

**Review checkpoints.** Per-slot CSV manifest with proposed values for closure / eligibility / cardinality_max / entry_state. Adam approves each batch before YAML lands. Codex's documented oversize-batch failure mode applies: ≤10 slots per batch.

**In scope.** Pilot backfill on Lux UCITS SICAV CBU only.

**Explicitly deferred.** Backfill of the other 11 DAG taxonomies and 34 constellation maps (Phase 2 scope); deletion of `role_cardinality.yaml`; closure annotation on `closed_unbounded` slots' aggregate-only predicates beyond placeholder.

### Phase 1.5D — Resolver skeleton + manifest emission

**Type.** New crate-internal module. Greenfield code.

**Files.**

- `rust/crates/dsl-core/src/resolver/mod.rs` (new) — public surface: `pub struct ResolvedTemplate`, `pub struct ResolvedSlot`, `pub struct ResolvedTransition`, `pub fn resolve_template(...)`.
- `rust/crates/dsl-core/src/resolver/composer.rs` (new) — composition logic: matches DAG-taxonomy slot to constellation-map slot by id, merges authored gate metadata using the precedence from Phase 1.5A's design doc, resolves transitions, computes content-hash version (`nom_rules_version`).
- `rust/crates/dsl-core/src/resolver/manifest.rs` (new) — emits a manifest of every (composite_shape, slot) pair with present/missing gate-metadata fields per slot.
- `rust/crates/dsl-core/src/resolver/version.rs` (new) — multi-source content hash (per Q7).
- `rust/crates/dsl-core/src/lib.rs` — register `pub mod resolver;`.
- `rust/crates/dsl-core/tests/resolver_lux_sicav.rs` (new) — given the pilot YAMLs from 1.5C, asserts `resolve_template("struct.lux.ucits.sicav", "cbu")` returns a complete `ResolvedTemplate` with all required gate fields populated for the CBU slots authored in 1.5C.
- `rust/crates/dsl-core/tests/resolver_manifest.rs` (new) — asserts the manifest correctly reports missing-field counts on a synthetic incomplete template.
- `rust/src/bin/reconcile_resolver_manifest.rs` (new) — CLI for `cargo run --bin reconcile_resolver_manifest -- --pair cbu/struct.lux.ucits.sicav` to print the manifest. Integrates with the existing `cargo x reconcile` pattern.

**Acceptance criteria.**

1. `cargo test --workspace -p dsl-core resolver_lux_sicav` passes.
2. Resolver test passes against the pilot YAMLs from 1.5C.
3. Manifest CLI prints the metadata coverage for the pilot pair.
4. Running the CLI across all (workspace, constellation) pairs produces a known-deferred list (no errors, just `missing_fields` per pair). This list is the input to Phase 2's planning.
5. The resolver subsumes the hardcoded `if id.starts_with("struct.")` at `valid_verb_set.rs:162` — either by callers migrating to `resolve_template()` directly, or by `load_constellation_stack` becoming a thin wrapper over the resolver. Pick one in the design doc.
6. Version hash is stable across rebuilds with identical inputs (snapshot test).
7. `cargo clippy -- -D warnings` clean.

**Dependencies.** Phase 1.5C merged; Phase 1 (predicate AST for resolved-template predicate references).

**Review checkpoints.** Resolver implementation reviewed against the design doc from 1.5A (composition rule precedence, conflict handling, version-hash inputs). Pilot test review.

**In scope.** Resolver skeleton with one composition rule (constellation slot id matches DAG slot id; constellation hydration data + DAG predicate data combine; explicit `gate_metadata` blocks merge with the per-design-doc precedence). Manifest emission. Stub `nom_rules_version` content hash.

**Explicitly deferred to later phases.**

- Gate runtime (Phase G1: `DispatchGate::Attach/Progress/Discretionary/Populate` enum and friends).
- Gate dispatcher (Phase G2-G6).
- Shape-rule input layer (Phase 2).
- Backfill of remaining 11 DAG taxonomies and 34 constellation maps (Phase 2).
- Reducer-condition migration to dsl-core predicate AST (Phase 2.x or Phase 8).
- Discretionary-verb classification sweep (Phase 7).
- Legacy adapter excision (Phase G7.5).
- The `crates/nom-rules` crate proposed in the original Phase 1.5 spec — does not exist, will not exist, name retired.

### Cross-phase notes

**Phase G3 dependency.** The gate-contract spec's Phase G3 (`Gate resolution from DagTemplate`) becomes Phase G3 (`Dispatch gate resolution from ResolvedTemplate`). The resolver from Phase 1.5D is its source object. G3 reads the `ResolvedTemplate` for the active composite, looks up the slot, and projects the appropriate `DispatchGate` variant. This rewires the gate-contract spec's data flow but does not change its phase ordering.

**Naming retro to gate-contract spec.** The gate-contract spec needs a one-liner pass to:
- Rename the dispatch types to `DispatchGate::Attach/Progress/Discretionary/Populate` to avoid `sem_os_core::gates` collision.
- Update Phase G1 file paths to `rust/crates/dsl-core/src/dispatch/` (existing convention) and add a sub-module `dispatch/dispatch_gate/` rather than `dispatch/gate/`.
- Note that Phase G2's "dispatcher skeleton" is a gate-checking layer that wraps the existing `DslExecutor` dispatch chain, not a parallel new dispatcher. The integration point is at `rust/src/sem_os_runtime/verb_executor_adapter.rs`.

**Vision and Shape Doc revisions.** Vision §2.6 (cardinality discipline → closure discipline) lands cleanly under Option C. Shape Doc §4 must be revised: a new composition mechanism *is* introduced (the resolver). Implementation TODO Phase 1.5 is replaced by this document's Part 4. Phase 2 reframes from "annotate slot cardinality" to "shape-rule authoring under `rust/config/sem_os_seeds/shape_rules/` plus shape-aware-generation that produces or supersedes `struct_*.yaml`."

**Back-out plan.** If Option C proves wrong in practice (e.g., the resolver becomes a maintenance burden out of proportion to its value), the back-out is to delete `rust/crates/dsl-core/src/resolver/` and revert to direct two-schema reads. Authored gate-metadata on both schemas remains valid; nothing in the YAML layer depends on the resolver existing. This bounds the downside of committing to C.

---

_End of document._
