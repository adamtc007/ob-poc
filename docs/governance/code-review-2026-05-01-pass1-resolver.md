# Code Review — Pass 1: Resolver core

**Commit:** `2642126b "Implement SemOS DAG architecture phases"`
**Files in scope:**
- `rust/crates/dsl-core/src/resolver/mod.rs` (112 LOC)
- `rust/crates/dsl-core/src/resolver/composer.rs` (772 LOC)
- `rust/crates/dsl-core/src/resolver/shape_rule.rs` (238 LOC)
- `rust/crates/dsl-core/src/resolver/manifest.rs` (165 LOC)
- `rust/crates/dsl-core/src/resolver/version.rs` (29 LOC)
- (cross-references) `rust/crates/dsl-core/src/config/dag.rs:205–302`, `rust/crates/sem_os_core/src/constellation_map_def.rs:90–136`, `rust/src/sage/valid_verb_set.rs:149–164`

**LOC reviewed:** 1,316 (resolver) + ~250 LOC of cross-referenced schema
**Build status:** `cargo check -p dsl-core` clean. `cargo test -p dsl-core` 256 passed / 0 failed / 2 ignored.
Resolver-specific suites: `resolver_lux_sicav` (2/2), `resolver_manifest` (2/2), `shape_rule_composition` (10/10).
**Reviewer:** Zed-Claude
**Date:** 2026-05-01

## Summary

17 findings: 6 MUST-FIX, 7 SHOULD-FIX, 3 CONSIDER, 1 NOTE. The Resolver's typed shape (`ResolvedTemplate`, `ResolvedSlot`, `SlotProvenance`, `VersionHash`), the leaf-shape-rule application path, and the `+`-sigil error path for same-rule mixing all work as advertised — the Lux SICAV pilot resolves and the manifest reports zero missing gate metadata. However, several decisions ratified in §11 are not enforced in code: D-016 state-machine primitives are deserialized but never composed; D-015 shape-rule constraint directives likewise; the 4-tier cascade has DAG ranked above constellation-map for gate-metadata fields, inverting D-018's table; D-018's same-level `AmbiguousShapeRefinement` does not exist as a type or check; D-008's "no shape conditionals in the wrapper" is honoured at the verb-set call site (the `starts_with("struct.")` is gone) but a structurally equivalent shape-conditional (`is_cbu_business_plane` + hardcoded constellation IDs) was reintroduced inside the resolver. SlotProvenance has multiple seed-step bugs that violate D-018's "missing provenance is a Resolver defect" stance. Confidence in the *shipped* leaf-shape-rule path is medium-high; confidence that the Resolver enforces the full §9/§11 contract is low — Phase 2 work that the spec explicitly schedules has not been performed (which is consistent), but several pieces of Phase 1.5/Phase 2-adjacent enforcement that the commit message claims as Phase 2 substrate are also absent.

## Findings

### MUST-FIX

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P1-001 — 4-tier cascade ranks DAG above constellation map for gate-metadata fields

**File:** `rust/crates/dsl-core/src/resolver/composer.rs`
**Lines:** 381–425, 459–473, 697–712
**Spec reference:** §9.3 (steps 3–4), D-018 (per-field table for `closure`, `eligibility`, `cardinality_max`, `entry_state`, `role_guard`, etc.); D-018 ratified rule 1.

**Observation:** for every gate-metadata field, the seed step prefers DAG over constellation map:

```rust
closure: dag_slot
    .and_then(|slot| slot.closure.clone())
    .inspect(|_| { source(&mut provenance, "closure", ResolvedSource::DagTaxonomy); })
    .or_else(|| { constellation_slot.and_then(|slot| { ... convert_closure(slot.closure.as_ref()) }) }),
```

`gate_vec` (lines 697–712) does the same for `attachment_predicates` / `addition_predicates` / `aggregate_breach_checks`: DAG wins, constellation only fills if DAG is empty.

**Issue:** D-018's ratified table and §9.3 cascade are unambiguous: for gate-metadata fields, the order is **leaf shape rule → ancestor shape rule → constellation map → DAG taxonomy → default**. Constellation map BEATS DAG when both are set; the code reverses these. D-011 says collisions across both schemas should warn (1.5B) / error (Phase 2), but until then `§9.3` step 3 is the tie-breaker. Today it is silently inverted, with no warning. Any constellation map that disagrees with the DAG on `closure` / `eligibility` / `cardinality_max` etc. will see the DAG win, contrary to the spec.

The pilot tests pass because the Lux SICAV pilot does not currently exercise a DAG-vs-constellation collision on a gate-metadata field — every collision-eligible field on a relevant slot is set in only one schema. The bug is latent rather than visible.

**Disposition (suggested):** invert the seed-step precedence so constellation map wins, then fall through to DAG, then default. Re-run `shape_rule_composition_applies_leaf_gate_metadata` and the manifest test to confirm no regression. Add a fixture that authors the same gate field on both DAG and constellation with different values and asserts constellation wins.

---

#### [MUST-FIX] [COMPLIANCE] P1-002 — Same-level shape-rule conflict detection (`AmbiguousShapeRefinement`) is not implemented

**File:** `rust/crates/dsl-core/src/resolver/composer.rs`, `rust/crates/dsl-core/src/resolver/mod.rs`
**Lines:** composer.rs:490–494 (apply loop), 499–528 (ancestor walk); mod.rs:14–17 (no `AmbiguousShapeRefinement` re-export)
**Spec reference:** D-018 rule 1; D-015 (`AmbiguousConstraintRefinement`); D-016 (`AmbiguousStateMachineRefinement`).

**Observation:** `apply_slot_refinement` walks `shape_chain` linearly and unconditionally overwrites each gate-metadata field with the current rule's value. `push_shape_rule_ancestors` traverses `extends:` recursively in YAML list order, with leaf appended last. There is no comparison of values authored at the same taxonomic depth. A grep across the workspace for `AmbiguousShapeRefinement`, `AmbiguousConstraintRefinement`, and `AmbiguousStateMachineRefinement` returns zero matches:

```
$ rg AmbiguousShapeRefinement crates/
(no results)
```

**Issue:** D-018 rule 1 requires `ResolveError::AmbiguousShapeRefinement { slot, field, sources: Vec<ShapeRef> }` to fire when two ancestor shape rules at the same taxonomic depth set different values for the same gate-metadata field. The current code silently lets the YAML-listed-last sibling win. This contract is the design's only defence against shape-rule authoring errors that look correct in isolation; without it, the resolver has no safety net for additive ambiguity across siblings.

The lack of a typed shape taxonomy (OQ-1) is a related concern — "same taxonomic depth" is not expressible without a parent/child shape model — but the trivial form ("two entries in a single rule's `extends:` list both setting the same field") is detectable today and is also missing.

**Disposition (suggested):** add `ResolveError::AmbiguousShapeRefinement { slot, field, sources: Vec<String> }` (and the analogous `AmbiguousConstraintRefinement` / `AmbiguousStateMachineRefinement` per D-015 / D-016). Implement the trivial-sibling check first (siblings = entries appearing as siblings in any single rule's `extends:` list); harden to the full taxonomic-depth check when the shape taxonomy lands. Add tests that author conflicting siblings and assert the error fires.

---

#### [MUST-FIX] [COMPLIANCE] P1-003 — State-machine refinement primitives (D-016) are deserialized but never composed into transitions

**File:** `rust/crates/dsl-core/src/resolver/composer.rs`, `rust/crates/dsl-core/src/resolver/shape_rule.rs`
**Lines:** composer.rs:665–695 (`compose_transitions`); shape_rule.rs:42–59 (primitive fields); composer.rs (no consumers)
**Spec reference:** D-016 (the five primitives + raw escape hatch); D-018 table rows for `insert_between`, `add_branch`, `add_terminal`, `refine_reducer`, raw edits.

**Observation:** `ShapeRule` carries typed fields for `insert_between`, `add_branch`, `add_terminal`, `refine_reducer`, `raw_add`, `raw_remove`. Grepping the composer for any read of these fields returns nothing:

```
$ rg "tighten_constraint|add_constraint|replace_constraint|insert_between|add_branch|add_terminal|refine_reducer|raw_add|raw_remove" crates/dsl-core/src/resolver/composer.rs
(no results)
```

`compose_transitions(dag)` reads exclusively from `dag.slots[*].state_machine.Structured.transitions`. The shape-rule primitives are pure dead deserialization.

**Issue:** D-016 ratifies named primitives as the resolver's mechanism for state-machine refinement. The schema parses, the primitives can be authored, but the resolver produces a transition vector that ignores them entirely. Authors writing `insert_between` in a shape rule today get a passing resolver run (because the primitive deserializes fine) and zero effect on the resolved template (because the composer never reads it). This is a silent contract violation.

D-016 is described in §10.8 as Phase 2 work, but the primitives' presence in `ShapeRule` (Phase 2 substrate, per D-016 "Constraints recorded") means the typed surface has been authored without enforcement. The escape-hatch's mandatory `rationale:` is enforced structurally by serde at the field level (no `#[serde(default)]`); however, D-016's other ResolveError contracts (referencing nonexistent edges, unreachable states post-composition, `AmbiguousStateMachineRefinement`) are absent.

**Disposition (suggested):** either (a) implement primitive composition in `compose_transitions` plus a post-composition validator, with `ResolveError::AmbiguousStateMachineRefinement` and the per-primitive error variants, OR (b) if Phase 2 timing is the correct constraint, delete the unused primitive fields from `ShapeRule` and document that authoring is gated on Phase 2 landing. Either is acceptable; carrying typed-but-unread fields is not.

---

#### [MUST-FIX] [COMPLIANCE] P1-004 — Shape-rule cross-workspace constraint directives (D-015) are deserialized but never applied

**File:** `rust/crates/dsl-core/src/resolver/composer.rs`, `rust/crates/dsl-core/src/resolver/shape_rule.rs`, `rust/crates/dsl-core/src/resolver/mod.rs`
**Lines:** shape_rule.rs:33–40, 145–168 (typed directives); mod.rs:53–62 (`ResolvedTemplate` has no cross-workspace-constraint field); composer.rs (no consumers)
**Spec reference:** D-015 (the three rules + the four conflict-and-validation rules); D-018 table rows for `tighten_constraint`, `add_constraint`, `replace_constraint`.

**Observation:** `ShapeRule` carries `tighten_constraint: Vec<TightenConstraint>`, `add_constraint: Vec<AddConstraint>`, `replace_constraint: Vec<ReplaceConstraint>`. None is read in `composer.rs`. `ResolvedTemplate` (mod.rs:52–62) has fields for `slots`, `transitions`, `version`, `generated_at`, `generated_from`, `structural_facts` — but **no field for cross-workspace constraints**. Even if a directive were applied, there is no surface to expose the result.

```rust
pub struct ResolvedTemplate {
    pub workspace: WorkspaceId,
    pub composite_shape: ShapeRef,
    pub structural_facts: StructuralFacts,
    pub slots: Vec<ResolvedSlot>,
    pub transitions: Vec<ResolvedTransition>,
    pub version: VersionHash,
    pub generated_at: String,
    pub generated_from: ResolverProvenance,
}
```

The `replaceable_by_shape: bool` flag is added on `CrossWorkspaceConstraint` (`dag.rs:555`) and `PredicateBinding` (`dag.rs:405`), defaulting to `false`. Grepping the resolver for reads of `replaceable_by_shape` returns zero:

```
$ rg replaceable_by_shape crates/dsl-core/src/resolver
(no results)
```

**Issue:** D-015 declares cross-workspace constraints DAG-authoritative with shape-rule tightening allowed. The resolver does not surface cross-workspace constraints at all in `ResolvedTemplate`, does not apply tighten/add/replace directives, and does not enforce `replaceable_by_shape: false` — because there is nothing to enforce against. The DAG-side `replaceable_by_shape` field is dead schema.

This is partially consistent with the spec (D-015 work is largely Phase 2), but the partial implementation is misleading: the schema declares the contract, the YAML parses, the runtime ignores it. An author who authors a `tighten_constraint:` today gets no error and no effect.

**Disposition (suggested):** add a `cross_workspace_constraints: Vec<ResolvedCrossWorkspaceConstraint>` field on `ResolvedTemplate`, port the DAG-side constraints into it, then implement `tighten_constraint`/`add_constraint`/`replace_constraint` per D-015's rules including `AmbiguousConstraintRefinement`. If Phase 2 timing forbids that, delete the dead shape-rule directive types and the unused `replaceable_by_shape` field and document in `legacy-compat-tracker.md` that the schema field is reserved.

---

#### [MUST-FIX] [QUALITY] P1-005 — Version hash (D-006) excludes state-machine YAMLs and shared-atom YAMLs

**File:** `rust/crates/dsl-core/src/resolver/version.rs`, `rust/crates/dsl-core/src/resolver/composer.rs`
**Lines:** version.rs:13–29 (hash function); composer.rs:185–194 (caller)
**Spec reference:** D-006; §9.4.

**Observation:** `compute_version_hash` is invoked with a `paths` vec built solely from:

```rust
let mut paths = vec![loaded_dag.source_path.as_path(), leaf.source_path.as_path()];
for loaded in &legacy_stack {
    if let Some(found) = inputs.constellation_maps.get(&loaded.constellation) {
        paths.push(found.source_path.as_path());
    }
}
for rule in &shape_chain {
    paths.push(rule.source_path.as_path());
}
let version = compute_version_hash(&paths, &composite_shape, &workspace);
```

`ResolverInputs` contains only `dag_taxonomies`, `constellation_maps`, `shape_rules`, and `seed_root` (composer.rs:46–50). State machines (`rust/config/sem_os_seeds/state_machines/*.yaml`) and shared atoms (`rust/config/sem_os_seeds/shared_atoms/*.yaml`) are not loaded into `ResolverInputs` and not threaded into the hash.

**Issue:** D-006 lists the canonical hash inputs:
> SHA-256 over the canonical-serialised concatenation of (DAG taxonomies for the workspace) + (constellation maps relevant to the shape) + (shape rules in ancestor-walk order) + (state machines referenced) + (`role_cardinality.yaml`) + (relevant shared atoms) + (Resolver code's `CARGO_PKG_VERSION`).

State machines and shared atoms are missing. A change to `state_machines/*.yaml` for a state machine referenced by a workspace's DAG will not flip the version hash, so the `nom_rules_version` invariant ("any input change changes version" — §9.4) is violated. The dispatcher's gate-cache invalidation logic (downstream) depends on this; stale gates will not be evicted when reducer-condition YAMLs change.

`role_cardinality.yaml` was deleted in this same commit (per D-007's eventual sunset, though D-007 explicitly gates deletion on Phase 2 completion + consumer audit — see also the Pass 4 scope). With the file gone, including it in the hash is moot, but the omission of state machines and shared atoms is not.

**Disposition (suggested):** extend `ResolverInputs` to load `state_machines/` and `shared_atoms/`. Have `compute_version_hash` accept structured input slices (DAG paths, constellation paths, shape-rule paths, state-machine paths, shared-atom paths) rather than a flat `&[&Path]`, so the contract that all categories are present is enforced at the type level. Add a snapshot test that flips a state-machine YAML and asserts the hash changes.

---

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P1-006 — D-008 violation: shape conditionals reintroduced inside the resolver

**File:** `rust/crates/dsl-core/src/resolver/composer.rs`
**Lines:** 77–96 (`legacy_constellation_stack`), 758–772 (`is_cbu_business_plane`, `push_if_present`)
**Spec reference:** D-008 #4 ("the wrapper's implementation must not reintroduce shape conditionals"); §9.8.

**Observation:**

```rust
pub fn legacy_constellation_stack(
    &self,
    constellation_id: &str,
) -> Result<Vec<core_map::ConstellationMapDefBody>, ResolveError> {
    let target = self.constellation_maps.get(constellation_id)
        .ok_or_else(|| ResolveError::ConstellationNotFound(constellation_id.to_string()))?;
    let mut stack = Vec::new();
    if is_cbu_business_plane(&target.body) {
        push_if_present(&mut stack, &self.constellation_maps, "group.ownership");
    }
    stack.push(target.body.clone());
    if is_cbu_business_plane(&target.body) {
        push_if_present(&mut stack, &self.constellation_maps, "kyc.onboarding");
    }
    Ok(stack)
}

fn is_cbu_business_plane(body: &core_map::ConstellationMapDefBody) -> bool {
    body.slots.get("cbu")
        .is_some_and(|slot| slot.cardinality == core_map::Cardinality::Root)
}
```

The `id.starts_with("struct.")` prefix check at the call-site is genuinely gone (`rg "starts_with(\"struct\\.\""` confirms `valid_verb_set.rs` no longer carries it). However, the same conditional now lives inside the resolver as a slot-shape inspection plus two hardcoded constellation IDs (`"group.ownership"`, `"kyc.onboarding"`).

**Issue:** D-008 #4 is explicit: *"If provenance alone is insufficient and the projection requires `if shape == ...` reconstruction, that's a defect in the Resolver's provenance surface, not a license for shape conditionals; the Resolver's provenance is extended instead."* The current implementation is a renamed-not-removed conditional. Whether you check `id.starts_with("struct.")` (string match) or `body.slots.get("cbu").is_some_and(...)` (structure match), the substantive logic is the same: "for cbu-business-plane shapes, attach group.ownership and kyc.onboarding."

The fix path the spec mandates is to extend the Resolver's provenance/authoring such that the legacy stack is reconstructable from authored data — e.g., authoring `legacy_stack_peers: [group.ownership, kyc.onboarding]` somewhere on the constellation, or expressing the cbu-business-plane membership through the structural-facts surface. The current implementation is the antipattern the architecture exists to remove.

**Disposition (suggested):** author the legacy-stack peers as data on the constellation map (or via a workspace-level convention surfaced through `WorkspaceConfig`) and replace `is_cbu_business_plane` + the hardcoded constellation IDs with a data read. Until that lands, document the deviation explicitly in `legacy-compat-tracker.md` so the cleanup is tracked, and demote `is_cbu_business_plane`'s heuristic from "reproduced quietly inside the resolver" to "intentional, scheduled-for-removal escape hatch."

---

### SHOULD-FIX

#### [SHOULD-FIX] [QUALITY] P1-007 — `SlotProvenance` recording has multiple seed-step bugs

**File:** `rust/crates/dsl-core/src/resolver/composer.rs`
**Lines:** 268–275 (state_machine), 289–380 (constellation-only field block), 474–486 (justification_required, audit_class, completeness_assertion)
**Spec reference:** D-018 ("Missing provenance for a non-default field is a Resolver defect, surfaced in tests, never reaches runtime"); D-018 table row for `SlotProvenance`.

**Observation:** several field-seed paths record provenance incorrectly:

(a) **DAG-provided `state_machine` records no provenance.**

```rust
state_machine: dag_state_machine_id(dag_slot).or_else(|| {
    source(&mut provenance, "state_machine", ResolvedSource::ConstellationMap);
    constellation_slot.and_then(|slot| slot.state_machine.clone())
}),
```

`Option::or_else` only invokes the closure when the original is `None`. If the DAG provides the state machine (the common case), the closure is not invoked and no `state_machine` entry is added to `provenance.field_sources`.

(b) **False positives for absent fields.** Where the DAG path returns `None` and the closure runs, `source(...)` is called *before* `constellation_slot.and_then(...)` is evaluated. If both DAG and constellation are absent, provenance is recorded as `ConstellationMap` for an absent field. The same pattern bites `table`, `pk`, `join`, `placeholder`, `max_depth`:

```rust
table: constellation_slot.and_then(|slot| {
    source(&mut provenance, "table", ResolvedSource::ConstellationMap);
    slot.table.clone()
}),
```

If `slot.table` is `None`, provenance is still recorded.

(c) **Three fields record no provenance at all in the seed step.**

```rust
justification_required: dag_slot.and_then(|slot| slot.justification_required)
    .or_else(|| constellation_slot.and_then(|slot| slot.justification_required)),
audit_class: dag_slot.and_then(|slot| slot.audit_class.clone())
    .or_else(|| constellation_slot.and_then(|slot| slot.audit_class.clone())),
completeness_assertion: dag_slot.and_then(|slot| {
        slot.completeness_assertion.as_ref().map(convert_dag_completeness)
    })
    .or_else(|| constellation_slot.and_then(|slot| slot.completeness_assertion.clone())),
```

No `source(...)` calls. If the shape rule does not override these fields, provenance is empty regardless of which schema set them.

**Issue:** D-018 makes provenance-population a Resolver defect class. The current recording is unreliable: missing for some authored fields, falsely set for some absent fields. The Lux SICAV pilot tests do not assert on these specific fields' provenance, so the bugs are silent.

**Disposition (suggested):** rework the seed-step to use a uniform helper: for each field, given `dag_value: Option<T>` and `constellation_value: Option<T>`, return `(Option<T>, Option<ResolvedSource>)` and only insert into `provenance.field_sources` when the returned source is `Some`. Add provenance assertions to `resolver_lux_sicav_composes_pilot_template`.

---

#### [SHOULD-FIX] [COMPLIANCE] P1-008 — `+`-sigil fields exist on DAG and constellation schemas, where D-018 forbids them

**File:** `rust/crates/dsl-core/src/config/dag.rs`, `rust/crates/sem_os_core/src/constellation_map_def.rs`, `rust/crates/dsl-core/src/resolver/composer.rs`
**Lines:** dag.rs:245–264; constellation_map_def.rs:110–127; composer.rs:697–712 (does not read `additive_*`)
**Spec reference:** D-018 final constraint: *"the `+` sigil convention for vector composition needs typed support in `shape_rule.rs` (Phase 2; §10.8) and in `dag_validator.rs` (Phase 1.5B, for parse-time validation that vectors with `+` prefix are present in shape-rule files only, not in DAG taxonomy or constellation map files where the sigil is meaningless)."*

**Observation:** the additive `+` fields are declared on three structs:

- `Slot` (dag.rs:245-264): `additive_attachment_predicates`, `additive_addition_predicates`, `additive_aggregate_breach_checks`
- `core_map::SlotDef` (constellation_map_def.rs:110-127): same three fields
- `SlotGateMetadataRefinement` (shape_rule.rs:105-130): same three fields

The composer's `gate_vec` reads only the bare-name fields from DAG and constellation:

```rust
fn gate_vec(provenance: &mut SlotProvenance, field: &str,
            dag: Option<&Vec<String>>, constellation: Option<&Vec<String>>) -> Vec<String> { ... }
```

It does not consume the `additive_*` fields from the DAG or constellation slot.

**Issue:** if a DAG taxonomy or constellation map YAML authors `+attachment_predicates: [foo]`, serde silently parses it into the `additive_attachment_predicates` field, the composer ignores it, and the data is lost without warning. D-018 says this should be a parse-time validation error in `dag_validator.rs`. That validator's coverage is Pass 3's scope; for Pass 1 the contained finding is that the resolver's input contract accepts data the spec forbids and produces a silent drop.

**Disposition (suggested):** delete the three additive fields from `Slot` and `core_map::SlotDef`. Cross-link to Pass 3's review of `dag_validator.rs` for the parse-time guard.

---

#### [SHOULD-FIX] [QUALITY] P1-009 — `compose_transitions` round-trips `from`/`via` through `serde_yaml::to_string` to render state IDs

**File:** `rust/crates/dsl-core/src/resolver/composer.rs`
**Lines:** 665–695
**Spec reference:** D-016 (transitions surface); none directly on representation.

**Observation:**

```rust
out.push(ResolvedTransition {
    slot_id: slot.id.clone(),
    from: serde_yaml::to_string(&transition.from)
        .unwrap_or_default()
        .trim()
        .to_string(),
    to: transition.to.clone(),
    via: transition.via.as_ref().map(|via| {
        serde_yaml::to_string(via).unwrap_or_default().trim().to_string()
    }),
    destination_green_when,
});
```

`TransitionDef.from` is `YamlValue` because the schema accepts `STATE` or `(STATE_A, STATE_B)` or list. The implementation here reserialises the YAML value back to text and trims one trailing newline.

**Issue:** for a multi-state form (list or sequence), `serde_yaml::to_string` produces a YAML document like `- A\n- B\n`, which `.trim()` reduces to `"- A\n- B"`. That is not a usable state identifier, and downstream consumers comparing `from` to a state ID will fail silently. The pattern is fragile and depends on the actual authored YAML being scalar-only — which is true today for the DAGs used in tests (no test exercises multi-state forms), so the bug is latent.

**Disposition (suggested):** parse `transition.from` and `transition.via` into a typed enum (`StateRef::Single(String)` | `StateRef::Multi(Vec<String>)`) at deserialize time, expose that on `ResolvedTransition`, and let consumers decide how to render. If a single string is required at the resolved-template surface, expand multi-state transitions into one `ResolvedTransition` per state.

---

#### [SHOULD-FIX] [COMPLIANCE] P1-010 — `predicate_bindings` has no shape-rule path, no tighten/extend mechanism, and `replaceable_by_shape` is dead schema

**File:** `rust/crates/dsl-core/src/resolver/composer.rs`, `rust/crates/dsl-core/src/resolver/shape_rule.rs`, `rust/crates/dsl-core/src/config/dag.rs`
**Lines:** composer.rs:276–288 (DAG-only); shape_rule.rs:88–143 (no `predicate_bindings` field); dag.rs:405 (`replaceable_by_shape: bool` on `PredicateBinding`)
**Spec reference:** D-018 rule 3 ("`predicate_bindings` follows tighten-only discipline"); D-018 table row for `predicate_bindings`.

**Observation:** `predicate_bindings` on `ResolvedSlot` is sourced exclusively from `dag_slot.state_machine.Structured.predicate_bindings`. `SlotGateMetadataRefinement` has no `predicate_bindings` field. There is no resolver path by which a shape rule can tighten or extend predicate bindings. Grepping the resolver for `replaceable_by_shape` returns zero results; the field is dead schema.

**Issue:** D-018 rule 3 specifies tighten-only with explicit `replaceable_by_shape: true` opt-in. None of this is implemented — neither tightening, nor extension, nor the `replaceable_by_shape` enforcement. Per D-018 the implementation is Phase 2 territory, but the schema-side flag has been added (Phase 1.5B substrate) without any test to mark it as deferred. A future author could reasonably read the field and assume it works.

**Disposition (suggested):** either implement Phase 2 work, or add a TODO comment + an explicit deferred-field annotation, plus an entry in `legacy-compat-tracker.md` that records `replaceable_by_shape` (on both `PredicateBinding` and `CrossWorkspaceConstraint`) as added-but-unread.

---

#### [SHOULD-FIX] [QUALITY] P1-011 — D-008 wrapper re-parses every seed YAML on every call

**File:** `rust/src/sage/valid_verb_set.rs`
**Lines:** 159–164
**Spec reference:** §9.5 (caching); D-008 (wrapper exists for production caller `agent/orchestrator.rs:1753-1768`).

**Observation:**

```rust
#[deprecated(note = "transitional; see D-008")]
pub fn load_constellation_stack(id: &str) -> Result<Vec<ConstellationMapDefBody>> {
    let inputs = ResolverInputs::default_from_cargo_manifest()?;
    let template = resolve_template(id.to_string(), "cbu".to_string(), &inputs)?;
    Ok(template.generated_from.legacy_constellation_stack)
}
```

`ResolverInputs::default_from_cargo_manifest()` reads, on every call, every YAML in `dag_taxonomies/`, `constellation_maps/`, and `shape_rules/`. A production caller exists at `agent/orchestrator.rs:1753-1768`; on every Sage valid-verb-set computation, the entire seed tree is re-parsed.

**Issue:** §9.5 puts ResolvedTemplate caching at G6 (deferred, acceptable to be absent), but Layer 1 — the `ResolverInputs` cache — is described as built once and reused. The wrapper has neither layer, so every call is O(disk-read of all seeds). On a session with multiple verb invocations this is a meaningful per-call cost.

**Disposition (suggested):** lift `ResolverInputs` to a process-level `OnceLock<ResolverInputs>` (or pass it in via an injected handle from the orchestrator) so the YAMLs are read once. Even without the Layer 2 ResolvedTemplate cache, the input cache is the §9.5 baseline.

---

#### [SHOULD-FIX] [QUALITY] P1-012 — Version-hash inputs include legacy-stack constellations, conflating stack heuristic with version surface

**File:** `rust/crates/dsl-core/src/resolver/composer.rs`
**Lines:** 156, 185–194
**Spec reference:** D-006 ("constellation maps relevant to the shape"); cross-reference P1-006.

**Observation:**

```rust
let legacy_stack = inputs.legacy_constellation_stack(&composite_shape)?;
...
let mut paths = vec![loaded_dag.source_path.as_path(), leaf.source_path.as_path()];
for loaded in &legacy_stack {
    if let Some(found) = inputs.constellation_maps.get(&loaded.constellation) {
        paths.push(found.source_path.as_path());
    }
}
```

When `is_cbu_business_plane` returns true for the composite, `kyc.onboarding` and `group.ownership` constellation paths are folded into the version-hash input. The leaf shape's hash now depends on the contents of two unrelated constellations whose presence is dictated by a shape conditional in `is_cbu_business_plane`.

**Issue:** if `is_cbu_business_plane` is the wrong predicate (P1-006), the version hash inherits the wrongness. More substantively, D-006 says the hash inputs are "constellation maps relevant to the shape." The relevance of `kyc.onboarding` to `struct.lux.ucits.sicav` is a runtime composition convention, not an authored truth about the shape. Mixing the two means edits to `kyc.onboarding` flip the version hash for every CBU shape — over-broad invalidation — and conversely, if the convention changes, hashes for the same authored content change without any input change.

**Disposition (suggested):** treat the legacy-stack constellations as a separate provenance category (carried only on `ResolverProvenance.legacy_constellation_stack`) and exclude them from `compute_version_hash`. The version reflects authored truth; the legacy stack is downstream presentation.

---

#### [SHOULD-FIX] [QUALITY] P1-013 — D-008 wrapper hardcodes `workspace = "cbu"`

**File:** `rust/src/sage/valid_verb_set.rs`
**Lines:** 162
**Spec reference:** D-008.

**Observation:** the wrapper passes the literal `"cbu"` workspace to `resolve_template`:

```rust
let template = resolve_template(id.to_string(), "cbu".to_string(), &inputs)?;
```

There is no documentation in the function's doc comment or D-008's text that records this assumption, and no validation that `id` is in fact a CBU-workspace shape.

**Issue:** if a non-CBU constellation ID is passed (e.g., a future caller that doesn't realise the wrapper is CBU-only), the resolver will compose against the CBU DAG, producing a template whose state machine, predicate bindings, and gate metadata are wrong for the requested constellation. The wrapper is documented as transitional (`#[deprecated]`), but the workspace-binding behaviour is undocumented.

**Disposition (suggested):** either (a) restrict the wrapper's signature to accept only CBU shapes and panic/error on non-CBU, (b) accept a workspace argument so the caller is forced to supply it, or (c) document the assumption in the doc comment with a runtime guard. The current silent assumption is the worst of the three.

---

### CONSIDER

#### [CONSIDER] [STYLE] P1-014 — `apply_vector_refinement` detects mixed `+`/replacement only within a single rule, not across same-level siblings

**File:** `rust/crates/dsl-core/src/resolver/composer.rs`
**Lines:** 626–651
**Spec reference:** D-018 rule 2 ("Mixing additive and replacement behaviour at the same level on the same field → ResolveError (`AmbiguousVectorComposition`)").

**Observation:** the `AmbiguousVectorComposition` error fires only when one shape rule sets both `attachment_predicates: ...` and `+attachment_predicates: ...` on the same field. Two same-level sibling shape rules where one uses replacement and the other uses additive are not detected — they apply in YAML order with the second silently winning.

**Issue:** D-018 rule 2 says the conflict is "at the same level." The current implementation only detects the within-single-rule case. Whether sibling-level detection is in scope for Phase 1.5D or deferred to Phase 2 is unclear from the spec; flagging because the named error type's scope is narrower than the rule it claims to enforce.

**Disposition (suggested):** add sibling-level detection in concert with P1-002 once same-level depth is defined. Until then, document the limitation in `apply_vector_refinement`'s doc comment.

---

#### [CONSIDER] [STYLE] P1-015 — `compute_version_hash` uses `to_string_lossy()` on paths

**File:** `rust/crates/dsl-core/src/resolver/version.rs`
**Lines:** 20, 22

**Observation:** `path.to_string_lossy()` substitutes `U+FFFD` for non-UTF-8 bytes. On macOS/Linux, build paths under `rust/config/` are virtually always UTF-8, so the risk is theoretical. But `to_string_lossy()` allows two distinct path bytes to hash to the same value if they both contain non-UTF-8 bytes that lossily decode the same way.

**Disposition (suggested):** use `path.as_os_str().as_encoded_bytes()` (Rust 1.74+) for the hashed path key; on Unix, the platform-specific `OsStr::as_bytes()`.

---

#### [CONSIDER] [STYLE] P1-016 — `ResolvedTemplate.generated_at` is the literal string `"compute-on-read"`

**File:** `rust/crates/dsl-core/src/resolver/composer.rs`
**Lines:** 203

**Observation:** `generated_at: "compute-on-read".to_string()` is a placeholder string, not a timestamp. The field's type is `String`, and the value never changes.

**Issue:** §3.3 implies `generated_at` is provenance metadata. The current value is misleading — it claims compute-on-read semantics that the resolver does not actually implement, and a reviewer of a manifest dump cannot use the field to date the composition. Either set a real timestamp at composition time, or remove the field.

**Disposition (suggested):** populate with `chrono::Utc::now().to_rfc3339()` or remove the field. If "compute-on-read" is an intentional sentinel meaning "no timestamp because we don't cache," express it as `Option<DateTime<Utc>>` with `None`.

---

### NOTE

#### [NOTE] P1-017 — `push_shape_rule_ancestors` has no cycle protection

**File:** `rust/crates/dsl-core/src/resolver/composer.rs`
**Lines:** 514–528

**Observation:** the recursive walker calls itself on each ancestor's `extends:` entries with no `visited:` set. A cycle in the `extends:` graph (rule A extends B, B extends A) would stack-overflow.

**Issue:** no current shape rules form cycles, but the resolver has no defence. A YAML typo could crash the resolver process at startup.

**Disposition (suggested):** add a `BTreeSet<&str>` of visited shape names threaded through the recursion; on a re-visit, return `ResolveError::ShapeRuleCycle { path: Vec<String> }`.

---

## Coverage notes

**What this pass covered:**
- All five resolver source files end-to-end.
- D-006 (version hash inputs), D-008 (wrapper subsumption), D-015 (cross-workspace constraint precedence), D-016 (state-machine refinement primitives), D-017 (gate-checking integration point — schema only; runtime-side wrapper-port is downstream of Pass 1's scope), D-018 (per-field merge precedence + 4-tier cascade + `+` sigil + tighten-only) verified against composer/shape_rule/version code.
- Schemas referenced from Pass 1 for `+`-sigil and `replaceable_by_shape` enforcement (`dag.rs`, `constellation_map_def.rs`).
- `resolver_lux_sicav` (2 tests), `resolver_manifest` (2 tests), `shape_rule_composition` (10 tests) — all green.

**What this pass deliberately did not cover:**
- `dag_validator.rs` enforcement of the `+` sigil parse-time guard (Pass 3 scope).
- The Frontier hydrator's consumption of `ResolvedTemplate` (Pass 2 scope).
- The wrapper-port `GateCheckingVerbExecutor` (D-017 runtime integration; not present in this commit).
- Authored YAML quality of the 21 shape rules (Pass 4 scope).
- Test quality across the resolver tests' ability to detect the bugs above (Pass 5 scope).

**Inconclusive / verified by code only:**
- Layer 1 / Layer 2 caching (§9.5) — verified absent by reading; no fixture exists to assert presence.
- D-016 primitives — verified deserialized but unused by reading and by `rg`; no test asserts they apply, because they don't.
- D-015 directives — same as above.
- D-008 wrapper invocation surface — only one production caller verified (`agent/orchestrator.rs:1753-1768` per D-008's recorded constraints, not directly inspected in Pass 1).

## Recommended next steps

In priority order — every item ties to a finding above, with severity inherited.

1. **MUST-FIX** P1-001 (cascade order inversion): correct the seed-step precedence; this is silent DAG-wins-over-constellation, the easiest finding to misdiagnose downstream.
2. **MUST-FIX** P1-006 (D-008 shape conditionals reintroduced): author the legacy-stack peers as data; this is the architectural antipattern the workstream exists to remove.
3. **MUST-FIX** P1-002 (same-level conflict detection): add `AmbiguousShapeRefinement` plus sibling detection. Without it, multi-ancestor shape graphs are unsafe to author.
4. **MUST-FIX** P1-005 (version hash inputs): include state machines and shared atoms; close the silent stale-cache window.
5. **MUST-FIX** P1-003 / P1-004 (D-016 / D-015 primitives + directives unused): decide whether to implement (Phase 2 territory in §10.8) or delete the dead schema; carrying typed-but-unread fields invites silent author errors.
6. **SHOULD-FIX** P1-007 (provenance bugs): rework the seed-step helper; add provenance assertions to the pilot tests.
7. **SHOULD-FIX** P1-008 (`+` fields on DAG/constellation schemas): delete from `Slot` and `core_map::SlotDef`, then re-verify under Pass 3's parse-time guard.
8. **SHOULD-FIX** P1-009–P1-013 (transitions YAML round-trip, `predicate_bindings` shape-rule path, wrapper caching, version-hash leakage, hardcoded workspace): individual fixes scoped per finding.
9. **CONSIDER** P1-014–P1-016: low-priority polish.
10. **NOTE** P1-017: cycle guard; cheap to add now, expensive to debug later.
