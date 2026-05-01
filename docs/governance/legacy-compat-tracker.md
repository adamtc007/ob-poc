# Legacy Compatibility Tracker

## D-008 — `load_constellation_stack`

- Status: transitional compatibility wrapper.
- Owner: Phase 1.5D.
- Sunset target: OQ-6 replacement of `compute_valid_verb_set_for_constellations` with a
  `ResolvedTemplate`-native action-surface function.
- Constraint: wrapper reads legacy projection data from `ResolvedTemplate.generated_from`;
  it must not reconstruct constellation stacks with shape-specific conditionals.

## D-007 — `role_cardinality.yaml`

- Status: retired in Phase 2 after explicit approval.
- Owner: Phase 2.
- Removed:
  - `rust/config/role_cardinality.yaml`
  - `rust/src/dsl_v2/cardinality.rs`
  - `dsl_v2` public re-exports for `CardinalityRegistry`, `CardinalityValidator`,
    and related file-backed cardinality types.
- Replacement: role multiplicity is carried by authored gate metadata and composed through
  the Resolver shape-rule layer.
