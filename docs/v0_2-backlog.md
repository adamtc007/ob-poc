# ob-poc Unified DSL v0.2 Backlog

Generated from v0.1 `[GAP: ...]` markers and surfaced issues.

## Language extensions

| Item | Origin | Priority |
|---|---|---|
| `for-each` template combinator — generate N atoms from a list parameter | S3-patch §8.1.3 | P0 — blocks 7 of 12 packs from supporting variable-arity |
| Variable-arity atom generation in pack templates | All packs with fixed-arity GAP markers | P0 |
| Full type lattice and subtyping rules | S1 §3.8 GAP | P1 |
| Conditional events (require external condition monitoring) | S3 §12 | P2 |
| Parallel multi-instance with dynamic expected count | S3 §12 | P2 |

## Runtime extensions

| Item | Origin | Priority |
|---|---|---|
| `PostgresJourneyStore` — production-grade Postgres implementation | Tranche 6 deferral | P0 — required for any deployment |
| Timer worker — polls `dsl_pending_timer`, fires expired timers | S2 §6.10 | P0 |
| Message correlation delivery | S2 §6.10 | P0 |
| Full BPMN compensation beyond transaction-subprocess scope | S2 §6.9 GAP | P2 |
| Timer cycle support | S2 §6.10 GAP | P2 |
| Cross-process async verb invocation | S2 §6.5 | P2 |

## Catalogue extensions

| Item | Origin | Priority |
|---|---|---|
| Variable-arity pack templates (complete packs 3, 4, 5, 6, 7, 8, 10) | S3-patch §8.1.3 | P0 — follows `for-each` |
| Additional decision packs (common patterns surfaced in practice) | Domain discovery | P1 |

## Tooling

| Item | Origin | Priority |
|---|---|---|
| Production Sage pack-matching (confidence-ranked, embeddings-based) | S3-patch §8.4 | P1 |
| BPMN/DMN XML migration tooling | S3 §12 | P1 |
| Pack catalogue browser UI | Product decision | P2 |

## SemOS integration

| Item | Origin | Priority |
|---|---|---|
| Tranche 1: Pre-refactor SemOS regression baseline | Plan §Tranche 1 | P0 — required before Tranche 3 |
| Tranche 3: SemOS verb reshape (~1,098 verbs to `(verb ...)` atoms) | Plan §Tranche 3 | P0 — highest-risk tranche |

## Type system

| Item | Origin | Priority |
|---|---|---|
| Full type lattice (base types, subtyping, refinements) | S1 §3.8 | P2 |
| Type-checked condition expressions | Parser limitation | P1 |

## Rejected (out of scope permanently)

- **Ad-hoc subprocess** — no Camunda 8 support, no custody banking requirement
- **Complex gateway** — expressible through inclusive + predicate; rejected in design
- **BPMN/DMN XML as primary authoring surface** — migration input only
