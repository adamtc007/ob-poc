# NOM-Equivalent Metadata Inventory — 2026-04-30

Status: reconnaissance for `[gate-contract-recon]`

Purpose: document where SemOS currently keeps NOM-equivalent declarative metadata, how that metadata is shaped, and what must be reconciled before the gate-contract workstream can safely define gate variants.

This document reports the codebase as found. It intentionally does not treat the missing `crates/nom-rules` crate as an implementation defect; that crate name was documentation drift.

## 1. Current Metadata Locations

### `crates/dsl-core/src/config/dag.rs`

This is the strongest current equivalent to the Phase 1.5 "NOM metadata schema" for DAG taxonomies. It loads `config/sem_os_seeds/dag_taxonomies/*.yaml` into typed Rust structs:

- `Dag`
- `Slot`
- `SlotStateMachine`
- `StateMachine`
- `PredicateBinding`
- `StateDef`
- `TransitionDef`
- cross-workspace constraints
- derived cross-workspace states
- parent-slot/state-dependency metadata
- dual lifecycles
- periodic review cadence
- evidence type validity windows
- category-gated slots

The loader is strict at the top level (`Dag` has `deny_unknown_fields`) but deliberately forward-compatible inside slots and state machines via flattened `extra` maps.

### `crates/dsl-core/src/config/dag_registry.rs`

This is the runtime index over loaded DAG taxonomy YAML. It materialises lookup tables for:

- cross-workspace constraints by target transition
- derived states by host slot
- parent slot lookup
- children by parent slot
- verb FQN to transition references

This is relevant to gates because it is already the place that answers "which DAG transition is this verb connected to?"

### `crates/dsl-core/src/config/dag_validator.rs`

This is the current structural validation layer for DAG taxonomies. It validates:

- cross-workspace constraint references
- derived-state references and cycles
- parent-slot references
- state dependency consistency
- dual-lifecycle junctions
- mutually exclusive category gates
- `green_when` parseability and predicate binding coverage
- long-lived suspended-state convention
- periodic review cadence shape
- evidence validity-window shape

It does not currently validate closure discipline, eligibility constraints, attachment/addition predicates, role guards, or gate-ready metadata.

### `crates/dsl-core/src/config/predicate/*`

This is the current Phase 1 predicate AST/parser. `StateDef.green_when: Option<String>` is parsed into `Predicate`, with support for:

- `Exists`
- `StateIn`
- `AttrCmp`
- `Every`
- `NoneExists`
- `AtLeastOne`
- `Count`
- `Obtained`

The parser treats authored `green_when` as a free-text convention and converts it into typed AST. Predicate bindings live on `StateMachine`, not on slots directly.

### `config/sem_os_seeds/dag_taxonomies/`

There are 12 DAG taxonomy YAML files. These are the authored DAG/state-machine source for workspaces such as CBU, KYC, Deal, catalogue, onboarding, and lifecycle resources.

The shape is list-oriented:

```yaml
version: "1.0"
workspace: cbu
dag_id: cbu_dag
overall_lifecycle: ...
slots:
  - id: cbu
    stateless: false
    parent_slot: ...
    state_dependency: ...
    state_machine:
      id: cbu_discovery_lifecycle
      source_entity: '"ob-poc".cbus'
      state_column: status
      states:
        - id: DISCOVERED
          entry: true
        - id: VALIDATED
          green_when: ...
      transitions:
        - from: DISCOVERED
          to: VALIDATION_PENDING
          via: cbu.submit-for-validation
      terminal_states: [...]
```

CBU examples show current shape-like mechanisms under existing names:

- `category_gated` gates slots by `cbu_category`, e.g. investor/holding/share-class only for `FUND_MANDATE`.
- `parent_slot` and `state_dependency` model hierarchy/cascade.
- `dual_lifecycle` models discovery and operational lifecycles on the same slot.
- `periodic_review_cadence` and `evidence_types.validity_window` model review/expiry discipline.
- `predicate_bindings` bind `green_when` entity names to DAG/substrate sources.
- `precondition` still exists on transitions in some places, e.g. `investor.activate` requires `investor_kyc.status = APPROVED`.

### `config/sem_os_seeds/constellation_maps/`

There are 35 constellation map YAML files. These are the closest existing shape-specific slot inventories. Examples include:

- `struct_lux_ucits_sicav.yaml`
- `struct_ie_ucits_icav.yaml`
- `struct_us_private_fund_delaware_lp.yaml`
- `cbu_workspace.yaml`
- `kyc_extended.yaml`

The shape is map-oriented:

```yaml
constellation: struct.lux.ucits.sicav
description: Luxembourg UCITS SICAV onboarding constellation
jurisdiction: LU
slots:
  management_company:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: management-company
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: true
    state_machine: entity_kyc_lifecycle
    overlays: [...]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
```

These files carry slot-shaped metadata required by the current hydrated constellation runtime:

- slot identity
- slot type
- accepted `entity_kinds`
- substrate table/primary key
- join path
- cardinality, currently `root | mandatory | optional | recursive`
- dependency order and `min_state`
- placeholder handling
- state machine reference
- overlays and edge overlays
- verb palette and state-based availability
- nested children
- recursive max depth

They do not appear to be generated from a base-plus-shape rule system. Each `struct_*` map is currently an authored concrete constellation.

### `crates/sem_os_core/src/constellation_map_def.rs`

This is the pure value type for authored constellation maps in the SemOS registry. It largely mirrors the runtime `src/sem_os_runtime/constellation_runtime.rs` structs:

- `ConstellationMapDefBody`
- `SlotDef`
- `SlotType`
- `Cardinality`
- `JoinDef`
- `DependencyEntry`
- `VerbPaletteEntry`
- `VerbAvailability`

This is important because `config/sem_os_seeds/constellation_maps/*.yaml` are scanned into SemOS registry seeds, not only consumed by the local runtime.

### `src/sem_os_runtime/constellation_runtime.rs`

This is the hydration/runtime projection for constellation maps. It defines:

- `ConstellationMapDef`
- `SlotDef`
- `RuntimeStateMachine`
- `RuntimeStateTransition`
- `RuntimeOverlaySource`
- `ValidatedConstellationMap`
- `ResolvedSlot`
- `HydratedConstellation`
- `HydratedSlot`

The runtime validates and flattens constellation slots, loads referenced state machines, computes map revisions, compiles query plans, hydrates slots, and computes action surfaces.

### `config/sem_os_seeds/state_machines/`

There are 20 standalone state machine YAML files. These are reducer-style state machines used by constellation slots:

```yaml
state_machine: entity_kyc_lifecycle
states: [approved, verified, evidence_collected, ...]
initial: empty
transitions:
  - from: empty
    to: placeholder
    verbs: [entity.ensure-or-placeholder]
reducer:
  overlay_sources:
    screenings:
      table: screenings
      join: workstream_id
      provides: [screening_type, status, result_summary]
      cardinality: many
  conditions:
    all_screenings_clear:
      expr: "all_screenings_run AND NOT ANY(screenings WHERE status IN (...))"
  rules:
    - state: approved
      requires: [workstream_closed_verified, case_approved, no_blocking_flags]
```

This is separate from DAG taxonomy state machines. It drives hydrated slot state/reducer behaviour for constellation maps.

### `crates/sem_os_core/src/state_machine_def.rs`

This is the pure registry value type for the standalone state machine YAML. It contains:

- `StateMachineDefBody`
- `TransitionDef`
- `ReducerDef`
- `OverlaySourceDef`
- `ConditionDef`
- `RuleDef`
- `ConsistencyCheckDef`

Reducer predicates are string expressions, not the `dsl-core` predicate AST.

### `config/sem_os_seeds/constellation_families/`

There are 24 constellation family YAML files. These select or narrow to concrete constellation maps. Shape:

```yaml
fqn: family.cbu_workspace
family_id: cbu_workspace
domain_id: cbu
selection_rules:
  - condition: "true"
    target_constellation: cbu.workspace
    priority: 60
constellation_refs:
  - constellation_id: cbu.workspace
    jurisdiction: ALL
    entity_kind: cbu
candidate_jurisdictions: [ALL]
candidate_entity_kinds: [cbu]
grounding_threshold: ...
```

This is selection/grounding metadata, not slot metadata, but it is a cross-reference layer between user/session context and concrete constellation maps.

### `config/sem_os_seeds/shared_atoms/`

There are 5 shared atom YAML files. They carry governed value sets, e.g. `fund_structure_type` allowed values and `regulatory_classification` allowed values. These are candidates for typed shape attributes, but today they are not wired into shape-rule inheritance or gate eligibility.

### `config/role_cardinality.yaml`

This file carries role-level cardinality and context metadata outside the SemOS seed tree:

```yaml
roles:
  depositary:
    cardinality: one
    context: [ucits, aif, raif]
  investment-manager:
    cardinality: one-or-more
structure-aliases:
  ucits: [sicav, icav-ucits, oeic-ucits, fcp-ucits]
```

This is semantically close to gate `cardinality_max` and shape-specific role-slot rules, but it is not integrated into DAG taxonomies or constellation maps.

### `config/verb_schemas/macros/`

There are 26 macro files. The `struct-*` macro files encode jurisdiction/structure-specific setup flows. Example: `struct-lux.yaml` expands `struct.lux.ucits.sicav` to `cbu.create`, document bundle application, role assignments, placeholder creation, etc.

These macros carry shape-like procedural knowledge:

- required and optional structure arguments
- internal acceptable party kinds
- jurisdiction constants
- structure type constants
- expansion steps
- document bundle choices
- role assignment sequence

This is not declarative slot metadata in the gate-contract sense. It is procedural macro expansion metadata. It is currently one of the places where shape-specific behaviour lives.

### `config/verbs/`

There are 149 verb YAML files. These carry verb definitions, arguments, handlers, metadata, returns, and newer `three_axis` declarations. They also contain older lifecycle blocks in some files.

Relevant shape:

- `three_axis.state_effect`: `preserving | transition`
- `three_axis.transitions`: optional transition edges for some verbs
- `metadata.tags`, `phase_tags`, `side_effects`
- verb arguments and lookup metadata
- handler/CRUD metadata

This surface is useful for verb classification and migration, but it is not currently the source of slot closure, eligibility, or gate pre/postconditions.

### `config/stategraphs/`

There are 9 stategraph YAML files. These declare graph-style nodes, edges, gates, and satisfied signals for higher-level workflow/navigation. Example shape:

```yaml
nodes:
  - node_id: cbu.documents
    node_type: gate
    satisfied_when:
      - signal: pending_document_count
edges:
  - edge_id: cbu.create
    from_node: cbu.entry
    to_node: cbu.active
    verb_ids: [...]
gates:
  - gate_node: cbu.active
    required_nodes: [...]
```

These are not the same as gate-contract gates, but the name collision is worth noting. They are navigation/workflow gates, not dispatcher pre-execution contracts.

### `src/semtaxonomy*`

`src/semtaxonomy_v2` is not the NOM metadata schema. It is the deterministic utterance/intent compiler pipeline:

- structured intent extraction
- semantic IR
- surface object resolution
- operation resolution
- binding resolution
- candidate selection
- discrimination
- composition

It consumes action surfaces and verb metadata, and it references hydrated state, but it does not own slot definitions, state-machine definitions, or shape-rule metadata.

`src/semtaxonomy` is the legacy/adjacent semantic pipeline. It is relevant to verb-surface selection, not to gate-source metadata.

### `src/taxonomy`

This is a visual/entity taxonomy and membership-rule builder. It defines tree/graph rendering-oriented concepts such as `TaxonomyContext`, `MembershipRules`, `NodeType`, dimensions, grouping, traversal and terminus. It is not a shape-rule inheritance engine for DAG generation.

It does, however, demonstrate existing concepts that overlap with shape-aware needs:

- dimensions such as jurisdiction and fund type
- context-specific membership rules
- maximum depth and traversal rules
- cycle/visited handling in tree construction

## 2. Structural Shape by Location

### Slot definition shape

There are two active slot shapes:

1. DAG taxonomy slots in `crates/dsl-core/src/config/dag.rs`:
   - `id`
   - `stateless`
   - `rationale`
   - optional inline/reference `state_machine`
   - `requires_products`
   - `parent_slot`
   - `state_dependency`
   - `dual_lifecycle`
   - `periodic_review_cadence`
   - `category_gated`
   - `suspended_state_exempt`
   - flattened `extra`

2. Constellation slots in `crates/sem_os_core/src/constellation_map_def.rs` and `src/sem_os_runtime/constellation_runtime.rs`:
   - `type`
   - `entity_kinds`
   - `table`
   - `pk`
   - `join`
   - `occurrence`
   - `cardinality`
   - `depends_on`
   - `placeholder`
   - `placeholder_detection` in runtime type
   - `state_machine`
   - `overlays`
   - `edge_overlays`
   - `verbs`
   - `children`
   - `max_depth`

The DAG taxonomy slot shape has richer state/predicate lifecycle data. The constellation slot shape has richer hydration/action-surface data.

### State machine shape

There are also two active state-machine shapes:

1. DAG taxonomy inline state machines:
   - `id`
   - `source_entity`
   - `state_column`
   - `scope`
   - `description`
   - `predicate_bindings`
   - `states`
   - `transitions`
   - `terminal_states`
   - `expected_lifetime`
   - `owner`
   - `note`
   - flattened `extra`

   States have `id`, `entry`, `description`, and optional `green_when`.

   Transitions have `from`, `to`, optional `via`, optional `precondition`, and optional `args`.

2. Standalone constellation reducer state machines:
   - `state_machine`
   - `description`
   - `states`
   - `initial`
   - `transitions`
   - optional `reducer`

   Reducers have `overlay_sources`, `conditions`, and `rules`.

These shapes are not unified. A gate resolver would need to know which source is authoritative for a given verb/slot: DAG taxonomy transition, constellation reducer transition, or both.

### Predicate / `green_when` shape

`green_when` currently lives on DAG taxonomy `StateDef`, as free-text parsed by `dsl-core` into the predicate AST. Predicate bindings live on the surrounding `StateMachine`.

Reducer conditions in standalone `state_machines/*.yaml` are string expressions and do not use the `dsl-core` predicate AST.

Transition `precondition` still exists in DAG taxonomy transitions. That disagrees with the Vision/Gate direction where destination `green_when` is the postcondition and gate preconditions should be explicit attachment/addition predicates or role guards, not ad hoc transition text.

### Existing eligibility-like fields

Current equivalents are partial and fragmented:

- `constellation_maps.*.slots.*.entity_kinds` constrains acceptable entity kind for a slot, e.g. `[company]`, `[person, company]`, `[trust]`.
- macro args use internal kind constraints, e.g. `internal.kinds: [company]`.
- `config/ontology/entity_taxonomy.yaml` defines entity types/subtypes and DB bindings.
- `config/verbs/registry/investor-role.yaml` contains a textual "UBO eligibility" note, but not a general typed eligibility schema.

There is no `EligibilityConstraint` equivalent that references a governed shape taxonomy position.

### Existing closure/cardinality-like fields

Current equivalents:

- DAG taxonomy Vision-era language uses `category_gated`, not closure.
- Constellation maps have `cardinality: root | mandatory | optional | recursive`.
- Standalone reducer overlay sources may have `cardinality: many`.
- `config/role_cardinality.yaml` has role multiplicity: `one`, `zero-or-one`, `one-or-more`, `zero-or-more`, with context lists.

Absent:

- `closed_bounded | closed_unbounded | open`
- `completeness_assertion`
- explicit `cardinality_max`
- aggregate-only discipline attached to unbounded slots

### Existing shape-rule-like fields

Current equivalents:

- Shape-specific concrete constellation maps (`struct_lux_ucits_sicav`, etc.).
- Constellation family selection rules choose a target constellation.
- `category_gated` in DAG taxonomies activates/deactivates slots by CBU category.
- `entity_kinds` in constellation slots conditionally hydrates slots for entity type, e.g. KYC extended trust/partnership slots.
- Structure macros encode jurisdiction/shape-specific procedural expansions.
- Shared atoms define some governed value sets such as fund structure type and regulatory classification.

Absent:

- base DAG template plus ancestor-to-leaf shape-rule composition
- deterministic resolved `DagTemplate` output for a shape
- field-level shape refinement provenance
- conflict detection for same-level shape refinements
- derived input rules such as shape -> jurisdiction/regulator

### Composition / inheritance / refinement

Current composition mechanisms are real but not the Shape Doc mechanism:

- `DagRegistry` composes cross-reference indexes from all loaded DAG YAML.
- `ConstellationModel::from_parts` flattens nested constellation slots.
- `load_constellation_stack` in `src/sage/valid_verb_set.rs` suggests a stack of constellation maps can be loaded for action-surface purposes.
- Constellation family `selection_rules` choose a target constellation.
- Macro expansion composes procedural verb sequences.
- State-machine reducers compose overlay sources, conditions, and rules.

There is no evidence of base -> ancestor -> leaf rule refinement for DAG templates. The current `struct_*` constellation maps are concrete authored maps, not generated leaves from inherited rules.

## 3. Gap Analysis

### Vision §2 closure discipline

Vision needs every slot to have an authoring-time closure/cardinality discipline:

- bounded slots permit universal child predicates
- unbounded slots only permit existence/aggregate forms
- runtime never iterates unbounded child populations in user-space

Current codebase gaps:

- No `ClosureType` equivalent exists in DAG taxonomy or constellation map types.
- Existing `cardinality` enums are not semantically equivalent: `mandatory` and `optional` are presence/requirement concepts, not bounded/unbounded/open closure.
- `recursive` exists in constellation maps, but that is traversal shape, not closure discipline.
- `role_cardinality.yaml` has multiplicity, but is not integrated into DAG validation or gate resolution.
- Predicate validator parses `Every`, `Count`, `NoneExists`, etc., but does not check the target entity set against slot closure.
- No `completeness_assertion` exists for open slots.

Significant doc/spec drift: the Phase 1.5 TODO assumed a new `SlotMetadataV2` in `nom-rules`, but the actual repo has two slot schemas and neither carries closure as needed.

### Shape Doc §4 shape-rule refinement

Shape Doc §4 needs shape as a governed taxonomy position, with rules that compose base rules, ancestor shape rules, and leaf shape rules. It also needs slot presence, slot configuration, and state-machine refinements.

Current codebase gaps:

- Concrete shape-specific `struct_*` constellation maps exist, but no inherited shape taxonomy/rule engine was found.
- `config/sem_os_seeds/shared_atoms/fund_structure_type.yaml` and `config/ontology/entity_taxonomy.yaml` provide useful governed values/entity taxonomy, but not a shape taxonomy with parent/child rule inheritance.
- `constellation_families` select target constellations; they do not compose refinements.
- `category_gated` can activate slots by CBU category but is not general shape-rule refinement.
- Structure macros contain shape-specific setup logic procedurally. That is the opposite of the Shape Doc's "runtime never resolves shape; resolved template carries it" target.
- There is no resolved `DagTemplate` object containing all shape-resolved slots, state machines, predicates, closure, eligibility, and provenance.

Significant doc/spec drift: the Shape Doc says no new composition mechanism is introduced and relies on existing NOM composition semantics. In this codebase, the relevant composition semantics either do not exist or are not represented as the Shape Doc assumes.

### Gate Contract §1.2 upstream needs

Gate variants require upstream metadata for:

- `composite_shape`
- `slot_id`
- `eligibility`
- `cardinality_max`
- `current_population_count`
- `closure`
- `entry_state`
- `attachment_predicates`
- `source_state`
- `destination_state`
- `destination_green_when`
- `flavour`
- `role_guard`
- `justification_required`
- `audit_class`
- `addition_predicates`
- `aggregate_breach_checks`
- `nom_rules_version`
- `generated_at`

Current upstream support:

- `slot_id`: present in DAG taxonomy slots and constellation slot names.
- `source_state` / `destination_state`: present in DAG taxonomy transitions and standalone state-machine transitions.
- `destination_green_when`: present on DAG taxonomy states, but sparse and still free-text before parsing.
- `entry_state`: inferable from `StateDef.entry`, but not explicitly tied to attach/populate gates.
- `current_population_count`: computable at runtime from constellation join metadata, not authored metadata.
- `generated_at`: runtime can provide, not authored metadata.
- `map_revision`: exists on `ValidatedConstellationMap`, but is not a NOM rules version.

Partial/fragmented support:

- `composite_shape`: constellation map name and `jurisdiction` imply shape, but there is no typed `ShapeRef`.
- `eligibility`: `entity_kinds` constrains coarse entity type, not taxonomic eligibility.
- `cardinality_max`: maybe derivable from `role_cardinality.yaml` for role slots, but not generally linked.
- `closure`: absent.
- `flavour`: `three_axis.state_effect` and metadata tags help, but do not classify attach/progress/discretionary/populate/tollgate as required.
- `role_guard`: no general DAG/gate metadata; some verb/macro text mentions justification/override, but not a typed role guard.
- `justification_required`: appears as verb arguments in some macros/verbs, not as discretionary gate metadata.
- `audit_class`: no unified gate field source found.
- `attachment_predicates` / `addition_predicates` / `aggregate_breach_checks`: absent as first-class fields.
- `nom_rules_version`: registry snapshots and map revisions exist, but no unified version hash for the resolved metadata set.

Hard gap:

- There is not yet one upstream object from which all four gate variants can be resolved without guessing across multiple metadata surfaces.

## 4. Proposed Corrected Phase 1.5 Scope

Do not create `crates/nom-rules`. Reformulate Phase 1.5 as "SemOS metadata schema convergence for gate resolution" against the actual codebase.

### Proposed files / integration points

Primary DAG taxonomy schema:

- Extend `crates/dsl-core/src/config/dag.rs`.
- Extend `crates/dsl-core/src/config/dag_validator.rs`.
- Extend `crates/dsl-core/src/config/dag_registry.rs` only for read indexes needed by later gate resolution.
- Add/extend tests under `crates/dsl-core/tests/` or existing module tests.
- Update authored YAML under `config/sem_os_seeds/dag_taxonomies/` only after field-set approval.

Constellation map schema:

- Extend `crates/sem_os_core/src/constellation_map_def.rs`.
- Mirror necessary runtime fields in `src/sem_os_runtime/constellation_runtime.rs`.
- Extend validation/loading in `src/sem_os_runtime/constellation_runtime.rs`.
- Extend SemOS registry seed scan only if new fields need explicit seed normalisation; scanner currently preserves payload shape.

State-machine/reducer schema:

- Extend `crates/sem_os_core/src/state_machine_def.rs` only if standalone reducer state machines need to carry gate-relevant metadata. Prefer not to duplicate gate metadata here unless the constellation runtime is the chosen upstream for a field.

Shape/structure support:

- Decide whether `config/sem_os_seeds/constellation_maps/struct_*.yaml` remain concrete authored maps or become generated/resolved outputs.
- If they remain concrete for now, add gate-ready fields directly to constellation slot definitions as an interim explicit metadata layer.
- If they become generated outputs, add a new authored source location for base/shape refinements before modifying gate types. Candidate path: `config/sem_os_seeds/shape_rules/` or a subdirectory under `config/sem_os_seeds/constellation_maps/`, but this needs Adam approval.

Existing ancillary metadata:

- Integrate or retire `config/role_cardinality.yaml`. If retained, map `one`, `zero-or-one`, `one-or-more`, `zero-or-more` to explicit gate fields in the chosen upstream schema, with context resolution.
- Treat `config/verb_schemas/macros/struct-*.yaml` as procedural workflow macros, not authoritative slot metadata. Extract durable structural facts from them into the chosen declarative metadata layer.

### Proposed field additions, subject to approval

Add a gate-source metadata block to the chosen slot schema rather than immediately defining runtime gate variants:

```yaml
gate_metadata:
  closure: closed_bounded | closed_unbounded | open
  completeness_assertion: ...
  eligibility:
    kind: entity_kind | shape_taxonomy
    values: [...]
  cardinality_max: <integer | null>
  entry_state: <state id>
  attachment_predicates: [...]
  addition_predicates: [...]
  aggregate_breach_checks: [...]
  role_guard: ...
  justification_required: true | false
  audit_class: ...
  shape_refinement_origin: ...
```

For DAG taxonomy slots, this could live directly on `Slot`. For constellation map slots, it could live directly on `SlotDef`. The right home depends on whether gate resolution should start from DAG taxonomy transitions or from hydrated constellation maps.

### Proposed acceptance criteria for corrected Phase 1.5

- Identify and approve the authoritative upstream source for gate resolution: DAG taxonomy, constellation map, or a composed object built from both.
- Add typed Rust structs for the approved metadata fields in the existing crate(s), not a new `nom-rules` crate.
- Parse round-trip YAML for the new fields.
- Validate closure presence for active slots, initially warning if strict authoring is too broad for one phase.
- Validate `open` closure requires `completeness_assertion`.
- Validate eligibility references known entity kinds or approved shape-taxonomy positions.
- Validate `cardinality_max` is present where `closure == closed_bounded` and absent or explicitly justified where not applicable.
- Validate predicate fields parse using the existing predicate parser or explicitly document why a separate predicate expression language is required.
- Document how `map_revision` / registry content hashes become `nom_rules_version` for gate freshness.
- Produce a manifest of existing concrete constellation slots and DAG slots that still lack gate-source metadata.

### Proposed sequencing correction

1. Phase 1.5A: Decide authoritative gate-source object and schema home.
2. Phase 1.5B: Add additive typed metadata fields to existing schema(s), without gate runtime code.
3. Phase 1.5C: Backfill a small pilot set, likely `struct_lux_ucits_sicav` plus CBU DAG slots it depends on.
4. Phase 1.5D: Add validators and manifest output for missing metadata.
5. Phase 2: Shape-aware generation/refinement, if still desired, should either generate concrete constellation maps/DagTemplates or explicitly annotate concrete maps as resolved outputs.
6. Gate G1: Only after 1.5A is approved, propose gate runtime field types using real upstream names and conversion paths.

## 5. Open Questions for Adam

1. Which object should be authoritative for gate resolution: DAG taxonomy slots, constellation map slots, or a composed "resolved template" built from both?

2. Are `config/sem_os_seeds/constellation_maps/struct_*.yaml` intended to remain hand-authored concrete resolved maps, or should they become generated outputs from base plus shape-rule inputs?

3. Should `config/role_cardinality.yaml` be folded into SemOS seeds, or treated as legacy side metadata to be migrated into slot `gate_metadata`?

4. What is the canonical shape taxonomy source? Existing candidates are `shared_atoms/fund_structure_type.yaml`, `config/ontology/entity_taxonomy.yaml`, constellation names, and structure macros, but none is currently a typed parent/child shape taxonomy.

5. Should `entity_kinds` be accepted as the first version of `EligibilityConstraint`, or must eligibility wait for a proper shape taxonomy?

6. Should transition `precondition` be migrated into gate `attachment_predicates` / `addition_predicates` / role guards, or retained temporarily as a compatibility field with warnings?

7. What should provide `nom_rules_version`: SemOS registry snapshot content hash, `ValidatedConstellationMap.map_revision`, a hash over DAG taxonomy YAML plus constellation YAML plus state-machine YAML, or something else?

8. Should standalone reducer state-machine conditions be converted to the `dsl-core` predicate AST, or are they a separate local reducer language that gates should not consume?

9. Are discretionary verbs currently identifiable from metadata, or do we need a human-reviewed classification pass before adding `DiscretionaryGate` metadata?

10. Does the first pilot target remain Lux UCITS SICAV CBU, given that it has both a concrete constellation map and rich CBU DAG taxonomy coverage?

