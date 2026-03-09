# Phase 0: Vocabulary Rationalization — OWNER DECISIONS (RESOLVED)

All [DECISION] items from the Phase 0 TODO have been resolved by the domain owner. This file captures the decisions. Apply these to `phase0-vocabulary-rationalization-todo.md`.

---

## Collision Decisions (Task 1)

| Phrase | Canonical Verb | Remove From | Notes |
|--------|---------------|-------------|-------|
| "who owns this entity" | `ubo.list-by-subject` | graph.ancestors, control.identify-ubos, bods.list-ownership | Differentiate each with specific phrases |
| "subscribe cbu to product" | `contract.subscribe` + `cbu.add-product` (keep both, differentiate) | **DELETE `product-subscription.subscribe` and the entire `product-subscription` domain** | Two verbs for two contexts, third is redundant |
| "trace ownership" | `control.trace-chain` | gleif.trace-ownership, bods.discover-ubos | These are research/Sage-side verbs — differentiate |
| "who are the beneficial owners" | `control.identify-ubos` | bods.list-persons, trust.identify-ubos | Broadest — differentiate others |
| "run sanctions screening" | `screening.sanctions` | **DELETE entire `case-screening` domain** | Fold all case-screening verbs into screening |
| "add ownership" | `ubo.add-ownership` | client-group.add-relationship | UBO domain owns ownership |
| "add share class" | BOTH — capital (voting/control) + fund (economic) | Neither deleted | Differentiate: "voting share class" vs "economic share class" |
| "list share classes" | BOTH — same as above | Neither deleted | Same differentiation |
| "appoint sub-advisor" | `investment-manager.assign` | delegation.add | |
| "issue new shares" | `capital.issue-shares` | **DELETE `capital.issue.new`** | Duplicate verb |
| "show sub-funds" | `fund.list-subfunds` (after merge) | fund-vehicle, fund-compartment | Both domains merging into fund |
| "unsubscribe cbu from product" | Both `contract.unsubscribe` + `cbu.remove-product` | product-subscription (deleted) | Differentiate by context |
| "what products is cbu subscribed to" | `cbu.list-subscriptions` (or create if needed) | contract.list-subscriptions, product-subscription.list | |

## Domain Merge Decisions (Task 2)

| Merge | Decision |
|-------|----------|
| 2A: `case-screening` → `screening` | **YES — DELETE case-screening entirely** |
| 2B: `lifecycle` → `service-resource` | YES |
| 2C: `fund-vehicle` + `fund-compartment` → `fund` | **YES — merge both into `fund`** |
| 2D: `doc-request` → `document` | YES |
| 2E: `product-subscription` | **DELETE entirely** |

## Type-Parameterized Merge Decisions (Task 3)

| Family | Decision |
|--------|----------|
| 3A: `entity.create-*` (4 verbs) | **MERGE → `entity.create` + `entity-type` arg** |
| 3A: `entity.ensure-*` (4 verbs) | **MERGE → `entity.ensure` + `entity-type` arg** |
| 3B: `ubo.end-*` (3 verbs) | **MERGE → `ubo.end-relationship` + `relationship-type` arg** |
| 3B: `ubo.delete-*` (3 verbs) | **MERGE → `ubo.delete-relationship` + `relationship-type` arg** |
| 3C: `sla.bind-to-*` (5 verbs) | **MERGE → `sla.bind` + `target-type` arg** |
| 3D: `trading-profile.add-*` (10 verbs) | **MERGE → `trading-profile.add-component` + `component-type` arg** |
| 3D: `trading-profile.remove-*` (7 verbs) | **MERGE → `trading-profile.remove-component` + `component-type` arg** |
| 3E: `fund.create-*` (6 verbs) | **MERGE → `fund.create` + `fund-type` arg** |
| 3E: `fund.ensure-*` (3 verbs) | **MERGE → `fund.ensure` + `fund-type` arg** |

## Reference Data Decision (Task 6)

**YES — consolidate ALL reference data CRUD into single `refdata.*` domain:**
- `refdata.ensure` (replaces jurisdiction.ensure, currency.ensure, market.ensure, etc.)
- `refdata.read` (replaces jurisdiction.read, currency.read, etc.)
- `refdata.list` (replaces jurisdiction.list, currency.list, etc.)
- `refdata.deactivate` (replaces jurisdiction.deactivate, currency.deactivate, etc.)
- Arg: `domain: string` — "jurisdiction", "currency", "market", "settlement-type", "ssi-type", "client-type", "screening-type", "risk-rating", "case-type"
- Eliminates ~50 verbs and all cross-domain refdata phrase collisions

## Estimated Impact

| Metric | Before | After |
|--------|--------|-------|
| Total verbs | 1,123 | ~600-700 |
| Domains | 134 | ~50-60 |
| Exact phrase collisions | 84 | 0 |
| Refdata verbs | ~50 | 4 |
| Type-parameterized duplicates | ~50 | 0 |
| Deleted domains | 0 | 6 (case-screening, lifecycle, fund-vehicle, fund-compartment, doc-request, product-subscription) |

## Execution Order

1. Task 2 (domain merges) — structural changes first
2. Task 3 (type-parameterized merges) — depends on Task 2 for fund merge
3. Task 6 (refdata consolidation) — independent
4. Task 1 (phrase collision fixes) — after merges reduce the collision set
5. Task 5 (description enrichment) — after structure is stable
6. Task 4 (noun_index expansion) — update to reference merged verb names
7. Task 7 (verify + re-embed) — final validation
