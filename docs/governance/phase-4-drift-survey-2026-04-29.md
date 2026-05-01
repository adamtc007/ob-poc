# Phase 4 Drift Survey — Actual State vs Substrate Audit Claims

> **Date:** 2026-04-29.
> **Source:** live YAML in `rust/config/verbs/` + DAG taxonomies in `rust/config/sem_os_seeds/dag_taxonomies/`.
> **Validator status:** `cargo x reconcile validate` reports 0 cross-DAG errors.

## §4.2 Workspace-ownership drift (9 clusters) — STATUS: ALREADY RESOLVED

All 9 clusters claimed in the substrate audit (dated 2026-04-29) describe a state that no longer exists in the live YAML. Every cluster declares the workspace/slot the audit recommends as the DAG-canonical target.

| # | Cluster | Audit claim | Actual current state | Drift? |
|---|---|---|---|---|
| 1 | `investor.*` (registry/investor.yaml) | Claims session_bootstrap | All 10 declarations target `cbu` (slot `investor` or `investor_kyc`) | ✅ none |
| 2 | `holding.*` (registry/holding.yaml) | Claims session_bootstrap | All 6 declarations target `cbu` | ✅ none |
| 3 | `share-class.*` (registry/share-class.yaml) | Claims session_bootstrap | All 7 declarations target `cbu` | ✅ none |
| 4 | `screening.*` (screening.yaml) | Claims instrument_matrix | All 3 declarations target `kyc` | ✅ none |
| 5 | `service.*` (service.yaml) | Claims instrument_matrix | All 4 declarations target `product_maintenance` | ✅ none |
| 6 | `manco-group.*` | Claims instrument_matrix slot manco_group | All 8 target `cbu` slot `manco` | ✅ none |
| 7 | `service-consumption.*` | Claims onboarding_request | All 6 target `cbu` slot `service_consumption` | ✅ none |
| 8 | `custody/trade-gateway.*` | Claims onboarding_request | All 3 target `instrument_matrix` slot `trade_gateway` | ✅ none |
| 9 | `custody/settlement-chain.*` | Claims cbu slot settlement_chain | All 8 target `instrument_matrix` slot `settlement_pattern_template` | ✅ none |

**Conclusion:** The audit was based on a stale snapshot. All 9 clusters were already DAG-canonical at the start of this refactor. **No §4.2 verb edits needed.**

## §4.3 KYC verb-set drift (4 families) — STATUS: REAL DRIFT, NEEDS DECISIONS

Real drift exists in all 4 families. Naming is single-prefix (e.g., `kyc-case.*` not `kyc.kyc-case.*`) — the TODO's double-prefix concern is moot.

### Family A — M-007 `kyc_case`

DAG via refs (kyc_dag.yaml lines 198–225, 340–343, 668–674):
- `kyc-case.update-status` ✓ exists
- `kyc-case.escalate` ✓ exists
- `kyc-case.close` ✓ exists
- `kyc-case.reopen` ✓ exists
- `case.approve` ❌ no verb (audit recommendation: amend DAG → `kyc-case.approve` and add verb)
- `case.reject` ❌ no verb (same)
- `case.approve-with-conditions` ❌ no verb (same)

**Proposed actions:**
1. Add 3 verbs to `rust/config/verbs/kyc/kyc-case.yaml`: `approve`, `reject`, `approve-with-conditions` (state_effect=transition; transition_args target_workspace=kyc, target_slot=kyc_case).
2. Amend kyc_dag.yaml DAG via refs: `case.approve` → `kyc-case.approve`, `case.reject` → `kyc-case.reject`, `case.approve-with-conditions` → `kyc-case.approve-with-conditions`.

### Family B — M-011/13/15 `evidence` (multi-machine)

DAG via refs: `evidence.link`, `evidence.verify`, `evidence.reject`, `evidence.waive`, `evidence.require`.
YAML verbs (evidence.yaml): `create-requirement`, `attach-document`, `mark-verified`, `mark-rejected`, `mark-waived`.

| DAG via | Likely YAML match | Decision needed |
|---|---|---|
| `evidence.link` | `attach-document` | rename one to match the other |
| `evidence.verify` | `mark-verified` | rename one to match the other |
| `evidence.reject` | `mark-rejected` | rename one to match the other |
| `evidence.waive` | `mark-waived` | rename one to match the other |
| `evidence.require` | (none) | add new verb (re-collection) |

**Open question:** which side is canonical? DAG uses imperative one-word (`verify`); YAML uses `mark-*` prefix (`mark-verified`). Per CLAUDE.md memory "VerbSelector route type" and "imperative-form rename" — imperative one-word matches the project's recent convention. **Default recommendation: rename YAML to DAG names** (drop `mark-` prefix).

### Family C — M-014 `red_flag`

DAG via refs: `red-flag.escalate`, `red-flag.resolve`, `red-flag.waive`, `red-flag.update-rating`.
YAML verbs (red-flag.yaml): `raise`, `mitigate`, `waive`, `close`, `dismiss`, `set-blocking`, `list-by-case`, `list-by-workstream`.

| DAG via | YAML candidate | Decision needed |
|---|---|---|
| `red-flag.escalate` | (none — closest is `raise`?) | add `escalate` verb |
| `red-flag.resolve` | `close` (different name, similar semantic) | rename `close` → `resolve` OR keep both with differentiation |
| `red-flag.waive` | `waive` ✓ | none |
| `red-flag.update-rating` | (none) | add `update-rating` verb |

`mitigate`, `dismiss`, `set-blocking` are YAML-only — preserved as YAML extensions.

### Family D — M-012 `ubo_registry`

DAG via refs: `ubo-registry.verify`, `ubo-registry.approve`, `ubo-registry.reject`, `ubo-registry.expire`, `ubo-registry.discover`.
YAML verbs (ubo-registry.yaml): `create`, `promote`, `advance`, `waive`, `reject`, `expire`.

| DAG via | YAML candidate | Decision needed |
|---|---|---|
| `ubo-registry.verify` | (none) | add `verify` verb |
| `ubo-registry.approve` | `promote` or `advance`? (semantic ambiguity) | needs business decision |
| `ubo-registry.reject` | `reject` ✓ | none |
| `ubo-registry.expire` | `expire` ✓ | none |
| `ubo-registry.discover` | `create` (semantic mismatch — discover ≠ create) | add `discover`, keep `create` separate |

`promote`, `advance`, `waive` are YAML-only — preservation status TBD pending Adam's call on `approve` mapping.

---

## Summary

- **§4.2:** 0 verb edits needed (stale audit).
- **§4.3:** ~12 verbs to add/rename + ~3 DAG via amendments. Two genuine business-decision points: (a) `mark-*` prefix policy in evidence family; (b) `ubo-registry.approve` ↔ `promote` / `advance` mapping.

Without the drift-check (Phase 8) yet implemented, the validator does NOT catch these. Phase 4 work depends on either:
1. Adam adjudicating the open questions, then implementing.
2. Implementing the drift-check first (Phase 8.1) so the work is verifiable.
3. Treating §4.3 as out of scope for this refactor and surfacing for R.1.
