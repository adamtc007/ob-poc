# TODO: Allianz Fund Import via Agent Template Batch Execution

## ✅ COMPLETED - All Phases Done

**Status**: All phases complete. The batch import pipeline is fully functional.

---

## Summary of Achievements

1. **178 Allianz fund entities** seeded to database
2. **ManCo entity** created (Allianz Global Investors GmbH)
3. **`onboard-fund-cbu` template** works correctly with DSL expansion
4. **`batch_test_harness` CLI** created and tested
5. **177 CBUs created** from Allianz funds in 1.64s
6. **Each CBU has 3 roles**: ASSET_OWNER, MANAGEMENT_COMPANY, INVESTMENT_MANAGER
7. **xtask commands** added for repeatable testing

---

## xtask Commands Available

```bash
# Import Allianz funds as CBUs using template
cargo x batch-import [--limit N] [--dry-run] [--verbose]

# Delete Allianz CBUs (cascade delete via DSL)
cargo x batch-clean [--limit N] [--dry-run]
```

---

## Phase 0: Gap Analysis & Remediation ✅

### Completed Items
- [x] Reviewed session.rs, expander.rs, agent_routes.rs
- [x] Identified batch template expansion gap (dotted property access)
- [x] Fixed `$param.property` substitution in template expander

---

## Phase 1: Seed Allianz Entity Data ✅

### Completed Items
- [x] Created `data/allianzgi_seed/seed.sql` with all fund entities
- [x] Created ManCo entity: Allianz Global Investors GmbH
- [x] Ran seed scripts against database
- [x] Verified: 178 Allianz entities in database

---

## Phase 2: Template System Verification ✅

### Completed Items
- [x] Template registry loads from `config/verbs/templates/`
- [x] `onboard-fund-cbu` template accessible and expandable
- [x] Fixed template expansion for dotted property access (`$fund_entity.name`)

---

## Phase 3: Batch Execution Infrastructure ✅

### Completed Items
- [x] Template expansion with shared and batch params works
- [x] DSL execution via GenericCrudExecutor (YAML-driven, no direct DB)
- [x] `cbu.delete-cascade` plugin handler for cleanup

---

## Phase 4: Test Harness ✅

### Completed Items
- [x] Created `rust/src/bin/batch_test_harness.rs`
- [x] Added to `Cargo.toml` with required features
- [x] Features: `--template`, `--fund-query`, `--shared`, `--limit`, `--dry-run`, `--verbose`, `--json`

### Usage
```bash
# Dry run with limit
cargo run --features database,cli --bin batch_test_harness -- \
  --template onboard-fund-cbu \
  --fund-query \
  --shared manco_entity=MANCO_UUID \
  --shared im_entity=IM_UUID \
  --shared jurisdiction=LU \
  --limit 5 \
  --dry-run

# Full execution
cargo run --features database,cli --bin batch_test_harness -- \
  --template onboard-fund-cbu \
  --fund-query \
  --shared manco_entity=MANCO_UUID \
  --shared im_entity=IM_UUID \
  --shared jurisdiction=LU
```

---

## Phase 5: Agent Chat Integration (Deferred)

This phase was deferred as the CLI batch harness meets current testing needs.
The infrastructure is in place for future agent chat integration.

---

## Phase 6: Full Integration Test ✅

### Test Results

**5-Fund Test:**
```
Found 5 Allianz fund entities to process

[1/5] Processing: ALLIANZ SECURICASH SRI... OK (1 binding)
[2/5] Processing: Allianz AI Income... OK (1 binding)
[3/5] Processing: Allianz ActiveInvest Balanced... OK (1 binding)
[4/5] Processing: Allianz ActiveInvest Defensive... OK (1 binding)
[5/5] Processing: Allianz ActiveInvest Dynamic... OK (1 binding)

Completed in 0.40s
Success: 5, Failed: 0
```

**Full 177-Fund Test:**
```
Found 177 Allianz fund entities to process
...
Completed in 1.64s
Success: 177, Failed: 0
```

---

## Success Criteria - All Met ✅

| Criterion | Status |
|-----------|--------|
| All Allianz funds seeded as entities | ✅ 178 entities |
| ManCo entity created | ✅ Allianz Global Investors GmbH |
| Template loads and expands correctly | ✅ Fixed dotted property access |
| Batch test harness runs (dry-run) | ✅ Works perfectly |
| Batch test harness creates CBUs | ✅ 177 CBUs created |
| Each CBU has required roles | ✅ ASSET_OWNER, MANAGEMENT_COMPANY, INVESTMENT_MANAGER |
| Full import completes successfully | ✅ 1.64s for 177 funds |

---

## Key Files Created/Modified

| File | Action | Purpose |
|------|--------|---------|
| `data/allianzgi_seed/seed.sql` | Created | Seed fund + ManCo entities |
| `rust/src/bin/batch_test_harness.rs` | Created | CLI test harness for batch template execution |
| `rust/src/templates/expander.rs` | Modified | Fixed `$param.property` substitution |
| `rust/xtask/src/main.rs` | Modified | Added `batch-import` and `batch-clean` commands |
| `rust/Cargo.toml` | Modified | Added batch_test_harness binary |

---

## Reference Files

- Scraped data: `scrapers/allianz/output/allianz-lu-2025-12-17.json`
- Template: `rust/config/verbs/templates/fund/onboard-fund-cbu.yaml`
- Template expander: `rust/src/templates/expander.rs`
- Batch harness: `rust/src/bin/batch_test_harness.rs`
