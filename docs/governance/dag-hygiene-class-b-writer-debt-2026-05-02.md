# DAG Hygiene Class B Writer Debt Packet - 2026-05-02

Purpose: close Phase 5 / Tranche C without inventing new business verbs.

Inputs:
- `docs/governance/dag-reachability-semos-reconciliation-2026-05-02.md` §3
- current DAG predicates in `rust/config/sem_os_seeds/dag_taxonomies/{deal,cbu,instrument_matrix,lifecycle_resources}_dag.yaml`
- current registered verbs from `sem_os_postgres::ops::build_registry()` plus `ob_poc::domain_ops::extend_registry()`
- current `writes:` metadata in `rust/config/verbs/`

## Current Classification

The original reconciliation found 18 missing closure tuples. After Phases 0-4 and the Phase 5 cascade work, the remaining true Class B surface is mostly declaration debt, not missing execution.

| Row | DAG predicate / transition | Tuple | Current writer | Action |
|-----|----------------------------|-------|----------------|--------|
| C-1 | `deal.IN_CLEARANCE -> CONTRACTED` | `deal_rate_cards.status = AGREED` | `deal.agree-rate-card` | Add `writes:` metadata. |
| C-2 | `deal.ONBOARDING -> ACTIVE` | `deal_onboarding_requests.request_status = COMPLETED` | `deal.update-onboarding-status` | Add `writes:` metadata for the terminal value. |
| C-3 | `cbu.investor.ELIGIBLE -> ACTIVE` | `investors.kyc_status = APPROVED` | `investor.approve-kyc` | Add `writes:` metadata. |
| C-4 | `deal.IN_CLEARANCE -> CONTRACTED` | `deals.kyc_clearance_status = approved` | `deal.update-kyc-clearance` | Already declared. |
| C-5 | `billing_profile.DRAFT -> ACTIVE` | `deal_rate_cards.status = AGREED` | `deal.agree-rate-card` | Covered by C-1 declaration. |
| C-6 | `deal.IN_CLEARANCE -> REJECTED` | `deals.kyc_clearance_status = rejected` | `deal.update-kyc-clearance` | Already declared. |
| C-7 | `cbu.VALIDATED green_when` | `mandate.state = active` | Cross-workspace handoff | Not Class B writer debt; represent as cross-workspace / entry signal. |
| C-8 | booking principal clearance approval gate | `booking_principal_clearances.clearance_status in APPROVED/ACTIVE` | `booking-principal-clearance.approve`, `.activate` | Existing SimpleStatus configs; master schema export must stay fresh for validator accuracy. |
| C-9 | one agreed rate card per tuple | `deal_rate_cards.status = SUPERSEDED` | DB uniqueness/trigger plus `deal.counter-rate-card` | Add `writes:` for explicit counter path; non-verb path belongs to `entry_via: trigger`. |
| C-10 | `investor_kyc` lifecycle | `investors.kyc_status = APPROVED/REJECTED/IN_PROGRESS/REFRESH_REQUIRED` | `investor.approve-kyc`, `.reject-kyc`, `.start-kyc`, `.request-documents` | Add `writes:` metadata. |

## Applied Closure

No new writer verbs are required from this packet.

Applied metadata updates:
- `deal.propose-rate-card` writes `deal_rate_cards.status = PROPOSED`
- `deal.counter-rate-card` writes `deal_rate_cards.status = COUNTER_OFFERED` and `SUPERSEDED`
- `deal.agree-rate-card` writes `deal_rate_cards.status = AGREED`
- `deal.update-onboarding-status` writes `deal_onboarding_requests.request_status = COMPLETED`
- `investor.request-documents` writes `investors.kyc_status = REFRESH_REQUIRED`
- `investor.start-kyc` writes `investors.kyc_status = IN_PROGRESS`
- `investor.approve-kyc` writes `investors.kyc_status = APPROVED`
- `investor.reject-kyc` writes `investors.kyc_status = REJECTED`
- `investor.mark-eligible` writes `investors.lifecycle_state = ELIGIBLE_TO_SUBSCRIBE`

## Deferred To Entry-Via

These are not true verbification debt:
- `deal_rate_cards.status = SUPERSEDED` when produced by the unique-agreed backend mechanism.
- `deal_rate_cards.status = CANCELLED` when produced by parent deal cancellation or rate-card removal.
- `investors.kyc_status = EXPIRED` from the `kyc_expires_at` backend trigger.
- `mandate.state = active` as a cross-workspace handoff into the CBU green predicate.

They should be represented with `entry_via` annotations, not new special-case writer verbs.

