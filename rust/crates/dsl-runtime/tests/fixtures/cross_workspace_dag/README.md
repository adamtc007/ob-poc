# Cross-workspace DAG scenario fixtures

YAML scenarios consumed by `tests/cross_workspace_dag_scenarios.rs` via
`dsl_runtime::cross_workspace::test_harness::ScenarioRunner`.

Each fixture loads the **real** DAG taxonomy YAMLs from
`rust/config/sem_os_seeds/dag_taxonomies/` — only state lookups,
predicate resolution, and child resolution are mocked. So the constraints,
derived-state aggregates, and cascade rules under test are exactly what
production runs.

## Adding a new fixture

1. Drop `your_scenario.yaml` in this directory.
2. Add one line to `tests/cross_workspace_dag_scenarios.rs`:

   ```rust
   scenario_test!(
       your_scenario,
       "tests/fixtures/cross_workspace_dag/your_scenario.yaml"
   );
   ```

3. Run: `cargo test -p dsl-runtime --features harness --test cross_workspace_dag_scenarios your_scenario`

## Schema

```yaml
name: "Human-readable scenario name"
suite_id: "snake_case_id"
description: |
  What this scenario verifies.

# Optional: override the DAG dir (default: rust/config/sem_os_seeds/dag_taxonomies).
# Use for fixture-only DAGs; production scenarios should leave this null.
dag_taxonomies_dir: null

# Symbolic alias → UUID. Refer to entities by alias name in the rest of the file.
entity_aliases:
  alias-name: "00000000-0000-0000-0000-000000000001"

# Initial in-memory state.
initial_state:
  - { workspace: <ws>, slot: <slot>, entity: "alias", state: "STATE_NAME" }
  # state can be `null` to represent "row exists, state column NULL"

# Mock predicate truth table. Predicate strings must match the constraint
# declaration verbatim (whitespace + newlines from `|` block scalars
# included). Each entry maps a target_id → source_id resolution.
predicates:
  "cases.client_group_id = this_deal.primary_client_group_id":
    - { target: "deal-1", source: "case-1" }

# Mock parent → children for cascade tests.
# Outer key: "parent_workspace.parent_slot"
# Inner key: parent entity alias
children:
  cbu.cbu:
    parent-cbu-1:
      - { workspace: cbu, slot: cbu, entity: "child-1" }

# Steps execute in order. Each step has at most one operation kind:
# check_transition / evaluate_derived / plan_cascade / mutate.
steps:
  - name: "step description"
    check_transition:
      workspace: <ws>
      slot: <slot>
      entity: "alias"
      from: "FROM_STATE"
      to: "TO_STATE"
    expect:
      violations:
        - constraint_id: "<id from DAG>"
          severity: "error"  # optional
          required_state: ["APPROVED"]  # optional
          actual_state: "REVIEW"  # optional, can be null

  - name: "evaluate derived state"
    evaluate_derived:
      workspace: <ws>
      slot: <slot>
      derived_id: "<id from DAG derived_cross_workspace_state>"
      host_entity: "alias"
    expect:
      derived:
        satisfied: false
        conditions:
          - { satisfied: true, description_contains: "raw" }  # both fields optional

  - name: "plan cascade"
    plan_cascade:
      parent_workspace: <ws>
      parent_slot: <slot>
      parent_entity: "alias"
      parent_new_state: "STATE"
    expect:
      cascade_actions:
        - child_workspace: <ws>
          child_slot: <slot>
          child_entity: "alias"
          target_state: "STATE"
          severity: "error"  # optional

  # Test scaffolding: directly mutate state. No assertion.
  - mutate:
      - { workspace: <ws>, slot: <slot>, entity: "alias", state: "NEW_STATE" }
```

## Why no Postgres?

The cross-workspace runtime takes `&PgPool` because production providers
hit Postgres. The harness wires mocks (`MockSlotStateProvider`,
`MockPredicateResolver`, `MockChildEntityResolver`) that ignore the pool
parameter. The runner constructs a `PgPool::connect_lazy(...)` that defers
connection until first query — no SQL ever runs ⇒ no connection ever
opens.

This means scenarios are **fast** (single-millisecond per step) and
**deterministic** (no DB seeding or test isolation concerns).

## Caveat: predicate string matching

The mock `PredicateResolver` compares predicate strings by **exact string
equality** with the declaration in the DAG YAML. Multi-line `|` block
scalars in the production DAG produce strings with embedded newlines;
the fixture must reproduce them verbatim if testing those constraints.

Best practice: copy the predicate string directly out of the production
DAG YAML using your editor's "yank with whitespace" command. If the
fixture's predicate map doesn't match, the resolver returns `Ok(None)`
which the GateChecker scores as a violation — easy to confuse with
"the constraint actually failed".

## Current fixtures

| Fixture | Mode | Coverage |
|---------|------|----------|
| `deal_contracted_compound_tollgate.yaml` | A | BAC+KYC+BP compound gate on Deal CONTRACTED |
| `cbu_validated_requires_kyc_approved.yaml` | A | CBU VALIDATED requires KYC APPROVED |
| `im_mandate_requires_validated_cbu.yaml` | A | IM trading_profile DRAFT→SUBMITTED requires CBU VALIDATED |
| `cbu_operationally_active_aggregate.yaml` | B | cbu_operationally_active aggregate (unsatisfied path) |
| `four_layer_chain_end_to_end.yaml` | A | Full Deal→CBU→Service→Capability binding chain |
