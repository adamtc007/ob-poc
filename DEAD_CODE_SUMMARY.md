# Dead Code Removal - Quick Reference

## Overview
**Total Dead Code Warnings**: 72 items  
**Estimated Lines to Remove**: ~1,700 lines  
**Files to Remove Completely**: 7 files  
**Current Test Status**: ✅ All 32 tests passing  

## Quick Stats by Category

| Category | Items | Lines | Risk Level |
|----------|-------|-------|------------|
| Grammar Module | 27 | ~900 | LOW |
| AST Module | 15 | ~350 | LOW |
| Data Dictionary | 8 | ~80 | LOW |
| Parser AST | 8 | ~120 | LOW |
| Error Handling | 7 | ~100 | LOW |
| Vocabulary | 7 | ~80 | MEDIUM |
| Parser | 5 | ~40 | LOW |
| DB State Manager | 1 | ~5 | LOW |

## Files to Remove (Phase 1)

```bash
# Grammar module - entire EBNF parsing infrastructure unused
rm rust/src/grammar/idiomatic_ebnf.rs
rm rust/src/grammar/ebnf_parser.rs
rm rust/src/grammar/dynamic_grammar.rs
rm rust/src/grammar/grammar_storage.rs
rm rust/src/grammar/grammar_validator.rs

# AST visitors - never implemented
rm rust/src/ast/visitors.rs

# Data dictionary catalogue - never used
rm rust/src/data_dictionary/catalogue.rs
```

## Top 10 Dead Code Items by Impact

1. **`grammar/idiomatic_ebnf.rs`** - Entire EBNF parser (413 lines)
2. **`ast/types.rs`** - EnhancedValue enum system (~200 lines)
3. **`ast/types.rs`** - DSLType and TypeConstraint enums (~100 lines)
4. **`parser_ast/mod.rs`** - Transaction/integrity structs (~120 lines)
5. **`grammar/mod.rs`** - GrammarEngine methods (~150 lines)
6. **`error.rs`** - ErrorCollector and ContextualError (~100 lines)
7. **`vocabulary/vocab_registry.rs`** - Internal registry methods (~80 lines)
8. **`ast/types.rs`** - Semantic analysis types (~100 lines)
9. **`data_dictionary/mod.rs`** - DataDictionary and related structs (~50 lines)
10. **`ast/visitors.rs`** - Visitor pattern traits (~27 lines)

## Execution Time Estimate

- **Analysis & Planning**: ✅ Complete (1 hour)
- **Phase 1 - File Removals**: 15 minutes
- **Phase 2 - Testing**: 15 minutes  
- **Phase 3 - Code Edits**: 45 minutes
- **Phase 4 - Module Updates**: 15 minutes
- **Phase 5 - Final Validation**: 30 minutes
- **Total**: ~2 hours

## Safety Checks

Before starting:
```bash
# Verify baseline
cargo test --lib          # Should show: 32 passed
cargo check --lib         # Should show: 72 warnings about "is never"/"are never"
```

After each phase:
```bash
# Verify still working
cargo check --lib         # Should compile successfully
cargo test --lib          # Should show: 32 passed
```

Final validation:
```bash
# Should show 0 dead code warnings
cargo check --lib 2>&1 | grep -c "is never\|are never"
```

## Risk Mitigation

**Backup Plan**:
```bash
# Create safety branch first
git checkout -b backup-before-dead-code-removal
git push origin backup-before-dead-code-removal
```

**Rollback if needed**:
```bash
git reset --hard backup-before-dead-code-removal
```

## Expected Benefits

✅ **Faster Compilation**: 5-10% improvement  
✅ **Clearer Codebase**: Remove ~1,700 lines of confusion  
✅ **Zero Dead Code Warnings**: Clean `cargo check` output  
✅ **Easier Navigation**: Less code to search through  
✅ **Better Documentation**: Code reflects actual architecture  

## When to Execute

**Prerequisites**:
- [ ] Plan reviewed and approved
- [ ] Backup branch created
- [ ] Tests verified passing (32/32)
- [ ] 2-hour time window available

**Not Safe to Execute If**:
- Active PRs modifying the same files
- Database feature tests failing
- Production deployment in progress

---

**Full Details**: See `DEAD_CODE_REMOVAL_PLAN.md` for complete analysis and step-by-step instructions.

**Status**: ✅ ANALYSIS COMPLETE - Ready for execution when approved
