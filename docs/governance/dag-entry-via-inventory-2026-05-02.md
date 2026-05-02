# DAG Entry-Via Inventory - 2026-05-02

Purpose: distinguish legitimate non-verb reachability from missing writer verbs.

Scope for this tranche:
- `deal_dag.yaml`
- `cbu_dag.yaml`
- `instrument_matrix_dag.yaml`
- `lifecycle_resources_dag.yaml`

## Applied Annotations

| DAG | Slot | State | `entry_via` | Evidence |
|-----|------|-------|-------------|----------|
| deal | `deal_rate_card` | `SUPERSEDED` | `trigger { name: idx_deal_rate_cards_one_agreed }` | DAG transition already states backend creation of a new AGREED card for the same `(deal, contract, product)`. |
| deal | `deal_rate_card` | `CANCELLED` | `cascade { parent: deal.cancel }` | DAG transition already states implicit rate-card removal or parent deal cancellation. |
| cbu | `investor_kyc` | `EXPIRED` | `trigger { name: kyc_expires_at }` | DAG transition already states backend `kyc_expires_at` trigger. |

## Remaining Non-Verb Cases

These are legitimate non-direct-verb cases, but I have not annotated them without line-level evidence in the current YAML or runtime source:

- scheduler-driven archival states
- health-check / readiness signal states
- settlement-pipeline first-trade signal states
- parent-state mirror cascades outside the confirmed Phase 5 parent-child refactors

## Current Rule

No special-case writer verbs are required for the rows above. If a state is entered by a real backend trigger, scheduler, signal, or parent cascade, it should carry `entry_via` and the validator should check the named mechanism rather than requiring a direct writer verb.

