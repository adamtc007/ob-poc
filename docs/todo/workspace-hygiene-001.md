# Workspace hygiene ticket — pre-existing failures outside control-plane scope

> Filed 2026-07-11 (EOP-PLAN-CONTROLPLANE-001, T9.4, closes PIR-D-006).
> Linked from `docs/research/control-plane-ownership-ledger.md`'s E5 scope
> annotation. These failures predate every tranche of the control-plane
> plan, are not referenced by any C-0xx ownership-ledger row, and were
> explicitly recommended for separate tracking by the independent
> adversarial review (`docs/research/control-plane-pir-001.md`, PIR-D-006)
> rather than folded into control-plane completion criteria.

## Item 1 — `ob-poc-boundary` golden verb-count assertions

`cargo test -p ob-poc-boundary --lib --features database` fails 3 of 181
tests:

```
failures:
    acp_dag_semantic::tests::disambiguates_cbu_product_onboarding_to_add_product
    acp_registry_projection::tests::slice1_projection_includes_entity_grain_effects
    acp_registry_projection::tests::slice1_projection_includes_verb_binding_metadata
```

Sample failure (`slice1_projection_includes_verb_binding_metadata`,
`crates/ob-poc-boundary/src/acp_registry_projection.rs:2775`):

```
assertion `left == right` failed
  left: 97
 right: 74
```

These are golden-count assertions comparing an expected verb/effect count
against the live catalogue. The counts have drifted (likely from
unrelated verb catalogue growth over time) without the test fixtures
being updated. Needs someone with ACP/registry-projection context to
either regenerate the golden counts or determine whether the drift
indicates a real regression.

## Item 2 — `dsl-runtime` doctest failure

`cargo test -p dsl-runtime --doc` fails 1 of 30 doctests:

```
error[E0432]: unresolved import `dsl_runtime::compute_reducer_revision`
  --> crates/dsl-runtime/src/state_reducer/state_machine.rs:97:5
   |
97 | use dsl_runtime::compute_reducer_revision;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no `compute_reducer_revision` in the root
```

The doctest example imports `compute_reducer_revision` from the crate
root, but it's no longer re-exported there (moved or renamed at some
point without the doctest being updated). Needs the doctest's `use` path
corrected to wherever `compute_reducer_revision` actually lives now.

## Scope note

Both items were independently reproduced with fresh output on 2026-07-11
(same failures, same counts, as originally found by the PIR-001 review on
2026-07-10) — not resolved, not worsened, stable pre-existing state.
Neither is a control-plane concern; do not fold either into a future
control-plane tranche's exit criteria.
