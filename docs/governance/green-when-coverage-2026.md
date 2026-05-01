# Green-When Coverage Manifest — 2026-04-30

Phase 8 introduced machine-readable coverage accounting for authored DAG
`green_when` predicates. The detailed row-level manifest lives at
`docs/governance/green-when-coverage-2026.csv`.

The sweep is intentionally diagnostic. It reports missing predicates; it does
not invent domain predicates where no architecture-backed predicate exists.

| Workspace | Total states | Candidate states | Covered | Missing |
|---|---:|---:|---:|---:|
| book_setup | 8 | 6 | 0 | 6 |
| booking_principal | 7 | 2 | 0 | 2 |
| catalogue | 5 | 3 | 0 | 3 |
| cbu | 69 | 37 | 4 | 33 |
| deal | 54 | 34 | 1 | 33 |
| instrument_matrix | 55 | 27 | 2 | 25 |
| kyc | 87 | 56 | 4 | 52 |
| lifecycle_resources | 11 | 7 | 0 | 7 |
| onboarding_request | 0 | 0 | 0 | 0 |
| product_maintenance | 10 | 3 | 0 | 3 |
| semos_maintenance | 26 | 21 | 1 | 20 |
| session_bootstrap | 0 | 0 | 0 | 0 |
| **Total** | **332** | **196** | **12** | **184** |

Exclusions are entry/source-only states and destinations reached only by
discretionary verbs, matching the Phase 8 scope of non-discretionary
destination-state postconditions.
