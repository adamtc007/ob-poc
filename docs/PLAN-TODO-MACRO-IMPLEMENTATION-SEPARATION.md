# Macro vs Implementation Verb Separation

> **Status:** Analysis Complete - Implementation Pending
> **Created:** 2026-01-29

## Problem Statement

There is blurring between the operator-facing macro vocabulary (`structure.*`, `party.*`, `case.*`, `mandate.*`) and the implementation DSL vocabulary (`cbu.*`, `entity.*`, `kyc-case.*`, `trading-profile.*`).

**Current State:**
- Only ~10 verbs marked `internal: true` (mostly `isda.*`, `corporate-action.*`)
- Implementation verbs like `cbu.create`, `entity.create-person` are exposed to operators
- Operators see jargon like "CBU", "entity_ref", "trading-profile" instead of business terms
- Golden files use implementation verbs directly instead of macros

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  OPERATOR LAYER (What users see)                                            │
│                                                                              │
│  structure.setup     → "Set up Structure"                                   │
│  party.add-person    → "Add Person"                                         │
│  case.open           → "Open Case"                                          │
│  mandate.create      → "Create Mandate"                                     │
│                                                                              │
│  Uses business vocabulary: structure, party, case, mandate                  │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              │ expands_to
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  IMPLEMENTATION LAYER (Internal DSL)                                        │
│                                                                              │
│  cbu.create          (internal: true)                                       │
│  entity.create-person (internal: true)                                      │
│  kyc-case.create      (internal: true)                                      │
│  trading-profile.create (internal: true)                                    │
│                                                                              │
│  Uses technical vocabulary: CBU, entity, kyc-case, trading-profile          │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Current Macro Coverage

| Operator Domain | Wraps | Status |
|-----------------|-------|--------|
| `structure.*` | `cbu.*`, `cbu-role.*` | ✅ Complete |
| `case.*` | `kyc-case.*`, `document.solicit` | ✅ Complete |
| `mandate.*` | `trading-profile.*` | ✅ Complete |
| `party.*` | `entity.*`, `identifier.*`, `address.*` | ✅ Created (party.yaml) |

## Implementation Tasks

### 1. Mark Implementation Verbs as Internal

Add `internal: true` to metadata of verbs wrapped by macros:

**cbu.yaml:**
- `cbu.create` → wrapped by `structure.setup`
- `cbu.update` → wrapped by `structure.update` (if exists)
- `cbu.list` → wrapped by `structure.list`
- `cbu-role.assign` → wrapped by `structure.assign-role`
- `cbu-role.list` → wrapped by `structure.roles`

**entity.yaml:**
- `entity.create-person` → wrapped by `party.add-person`
- `entity.create-company` → wrapped by `party.add-company`
- `entity.update` → wrapped by `party.update`
- `entity.get` → wrapped by `party.details`
- `entity.search` → wrapped by `party.search`
- `entity.list` → wrapped by `party.list`

**kyc/kyc-case.yaml:**
- `kyc-case.create` → wrapped by `case.open`
- `kyc-case.add-party` → wrapped by `case.add-party`
- `kyc-case.submit` → wrapped by `case.submit`
- `kyc-case.approve` → wrapped by `case.approve`
- `kyc-case.reject` → wrapped by `case.reject`
- `kyc-case.list` → wrapped by `case.list`

**trading-profile.yaml:**
- `trading-profile.create` → wrapped by `mandate.create`
- `trading-profile.add-product` → wrapped by `mandate.add-product`
- `trading-profile.set-instruments` → wrapped by `mandate.set-instruments`
- `trading-profile.set-markets` → wrapped by `mandate.set-markets`
- `trading-profile.list` → wrapped by `mandate.list`
- `trading-profile.get` → wrapped by `mandate.details`

### 2. Update Verb Search to Filter Internal Verbs

In `rust/src/mcp/verb_search.rs`, the `HybridVerbSearcher` should:
1. Load verb metadata including `internal` flag
2. Filter out `internal: true` verbs from operator-facing search results
3. Keep internal verbs available for macro expansion (system use)

```rust
// In search results, filter internal verbs for operator queries
fn filter_for_operator(results: Vec<VerbSearchResult>) -> Vec<VerbSearchResult> {
    results.into_iter()
        .filter(|r| !r.verb_meta.internal)
        .collect()
}
```

### 3. Update Golden Files to Use Macros

Golden files should demonstrate operator vocabulary, not implementation:

**Before (implementation verbs):**
```clojure
(cbu.create :name "Fund Alpha" :jurisdiction "LU")
(entity.create-company :name "Acme Corp" :jurisdiction "DE")
```

**After (macro verbs):**
```clojure
(structure.setup :structure_type pe :name "Fund Alpha" :jurisdiction "LU")
(party.add-company :name "Acme Corp" :jurisdiction "DE")
```

### 4. Verb Search Priority Update

Current search priority doesn't distinguish operator vs system context:

```
0. Operator macros (business vocabulary)  ← Operator queries land here
1. User-specific learned (exact)
2. Global learned (exact)
3. User-specific learned (semantic)
4. [REMOVED]
5. Blocklist filter
6. Global semantic (cold start)           ← Implementation verbs filtered here
7. Phonetic fallback
```

## Files to Modify

| File | Change |
|------|--------|
| `rust/config/verbs/cbu.yaml` | Add `internal: true` to wrapped verbs |
| `rust/config/verbs/entity.yaml` | Add `internal: true` to wrapped verbs |
| `rust/config/verbs/kyc/kyc-case.yaml` | Add `internal: true` to wrapped verbs |
| `rust/config/verbs/trading-profile.yaml` | Add `internal: true` to wrapped verbs |
| `rust/src/mcp/verb_search.rs` | Filter internal verbs from operator search |
| `docs/dsl/golden/*.dsl` | Update to use macro vocabulary |

## Non-Breaking Migration

1. Implementation verbs remain callable via DSL for backwards compatibility
2. `internal: true` only affects verb discovery/search
3. Existing scripts using `cbu.create` continue to work
4. New users are guided toward macros via semantic search

## Verification

After implementation:

```bash
# Lint should pass with no macro violations
cargo x verbs lint-macros

# Verb search should return macros, not implementation verbs
cargo x test-verbs --taught

# Golden files should parse and execute via macro expansion
cargo x dsl-check docs/dsl/golden/*.dsl
```
