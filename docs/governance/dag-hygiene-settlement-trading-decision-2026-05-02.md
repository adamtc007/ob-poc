# DAG Hygiene Decision Packet: Settlement Chain and Trading Profile Drift

Date: 2026-05-02

Scope: Phase 2.c of `docs/todo/DAG_REACHABILITY_SEMOS_HYGIENE_PLAN_2026-05-02.md`.

## Summary

The settlement-chain and trading-profile findings should not be handled as pure rips. Both sets are referenced by `instrument_matrix_dag.yaml` and both appear to be intended lifecycle topology, not abandoned declarations.

Recommended path:

1. Preserve settlement-chain topology by switching the six SimpleStatus configs from `status` to the existing `lifecycle_status` carrier introduced by `rust/migrations/20260429_carrier_07_settlement_chain_lifecycle_status.sql`.
2. Preserve trading-profile topology by aligning the `cbu_trading_profiles.status` constraint with the DAG/YAML lifecycle vocabulary before keeping the affected SimpleStatus configs executable.
3. Refresh `migrations/master-schema.sql` before promoting hard-fail validator checks, because the root schema export is behind the standalone migrations.

## Settlement Chain

Current runtime configs in `rust/src/domain_ops/simple_status_op.rs`:

| FQN | Table | PK | Current state column | Target |
| --- | --- | --- | --- | --- |
| `settlement-chain.request-review` | `cbu_settlement_chains` | `chain_id` | `status` | `reviewed` |
| `settlement-chain.enter-parallel-run` | `cbu_settlement_chains` | `chain_id` | `status` | `parallel_run` |
| `settlement-chain.go-live` | `cbu_settlement_chains` | `chain_id` | `status` | `live` |
| `settlement-chain.abort-parallel-run` | `cbu_settlement_chains` | `chain_id` | `status` | `reviewed` |
| `settlement-chain.suspend` | `cbu_settlement_chains` | `chain_id` | `status` | `suspended` |
| `settlement-chain.reactivate` | `cbu_settlement_chains` | `chain_id` | `status` | `live` |

Current schema evidence:

- `migrations/master-schema.sql` still shows no `status` or `lifecycle_status` column for `cbu_settlement_chains`.
- `rust/migrations/20260429_carrier_07_settlement_chain_lifecycle_status.sql` adds `lifecycle_status text` with CHECK values `draft`, `configured`, `reviewed`, `parallel_run`, `live`, `suspended`, `deactivated`.
- The six SimpleStatus target values all fit that `lifecycle_status` CHECK.

DAG/YAML evidence:

- `rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_dag.yaml` references all six FQNs in `settlement_pattern_template` transitions.
- `rust/config/verbs/custody/settlement-chain.yaml` declares all six as `behavior: plugin`, with transition edges matching the `lifecycle_status` vocabulary.

Recommended action:

- Change these six SimpleStatus configs to `state_col: "lifecycle_status"`.
- Keep the FQNs and YAML declarations.
- Re-export `migrations/master-schema.sql` so validator evidence no longer sees a stale table shape.

Do not rip these verbs unless Adam explicitly chooses to remove the settlement chain lifecycle from the DAG.

## Trading Profile

Current runtime configs in `rust/src/domain_ops/simple_status_op.rs`:

| FQN | Table | PK | State column | Target |
| --- | --- | --- | --- | --- |
| `trading-profile.enter-parallel-run` | `cbu_trading_profiles` | `profile_id` | `status` | `PARALLEL_RUN` |
| `trading-profile.go-live` | `cbu_trading_profiles` | `profile_id` | `status` | `ACTIVE` |
| `trading-profile.abort-parallel-run` | `cbu_trading_profiles` | `profile_id` | `status` | `APPROVED` |
| `trading-profile.suspend` | `cbu_trading_profiles` | `profile_id` | `status` | `SUSPENDED` |
| `trading-profile.reactivate` | `cbu_trading_profiles` | `profile_id` | `status` | `ACTIVE` |
| `trading-profile.supersede` | `cbu_trading_profiles` | `profile_id` | `status` | `SUPERSEDED` |

Current schema evidence:

- `migrations/master-schema.sql` constrains `cbu_trading_profiles.status` to `DRAFT`, `VALIDATED`, `PENDING_REVIEW`, `ACTIVE`, `SUPERSEDED`, `ARCHIVED`.
- `PARALLEL_RUN`, `APPROVED`, and `SUSPENDED` are therefore currently non-executable target states.

DAG/YAML evidence:

- `rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_dag.yaml` models trading profile states as `DRAFT`, `SUBMITTED`, `APPROVED`, `PARALLEL_RUN`, `ACTIVE`, `SUSPENDED`, `REJECTED`, `SUPERSEDED`, `ARCHIVED`.
- `rust/config/verbs/trading-profile.yaml` declares the affected lifecycle verbs as `behavior: plugin`, with transitions matching the DAG vocabulary.

Decision needed:

Option A, preserve topology:

- Add a migration aligning `cbu_trading_profiles.status` to the DAG vocabulary.
- Keep the SimpleStatus configs for `enter-parallel-run`, `abort-parallel-run`, `suspend`, `go-live`, `reactivate`, and `supersede`.
- Then resolve how existing `VALIDATED` and `PENDING_REVIEW` rows map into the DAG vocabulary.

Option B, rip dead lifecycle:

- Delete the non-executable SimpleStatus configs and corresponding DAG/YAML transitions in the same slice.
- This would shrink the instrument matrix lifecycle and should only be done if the parallel-run/suspension states are no longer product requirements.

Recommended action:

- Choose Option A. The DAG and YAML agree on the richer lifecycle, and the drift appears to be schema lag rather than dead topology.
- Before implementation, Adam should confirm the backfill mapping for existing rows:
  - `PENDING_REVIEW` likely maps to `SUBMITTED`.
  - `VALIDATED` likely maps to `APPROVED`.

## Stop Point

Implementation should pause here for Adam's decision on the trading-profile schema alignment and backfill mapping. Settlement-chain repair is mechanically clear if preserving topology is approved.
