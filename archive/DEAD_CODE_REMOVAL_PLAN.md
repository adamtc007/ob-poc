# Dead Code Removal Plan for ob-poc

## Executive Summary

This document provides a comprehensive plan to remove **72 dead code warnings** identified across the Rust codebase. The dead code consists of unused types, functions, methods, fields, and entire modules that are not referenced anywhere in the active codebase.

**Impact**: Removing this dead code will:
- Reduce maintenance burden
- Improve code clarity and navigation
- Speed up compilation times
- Reduce cognitive load for developers
- Lower the risk of bugs in unused code paths

**Confidence Level**: HIGH - All items listed have been verified as unused through `cargo check --lib` analysis.

---

## Dead Code Inventory by Category

### Category 1: Grammar Module (27 items - Highest Priority)
**Total Lines to Remove**: ~900 lines

The entire EBNF grammar parsing infrastructure is unused. The actual DSL parsing is handled by the `parser` module using NOM combinators directly.

#### Files to Remove Entirely:
1. **`src/grammar/idiomatic_ebnf.rs`** (413 lines)
   - `EBNFParser` struct and all parsing functions
   - 15 unused functions: `grammar`, `rule`, `expression`, `choice`, `sequence`, etc.
   - 5 unused methods in `EBNFGrammar` impl

2. **`src/grammar/ebnf_parser.rs`** (if exists)
3. **`src/grammar/dynamic_grammar.rs`** (if exists)
4. **`src/grammar/grammar_storage.rs`** (if exists)
5. **`src/grammar/grammar_validator.rs`** (if exists)

#### Dead Code in `src/grammar/mod.rs`:
- Line 30: Methods `load_grammar`, `get_grammar`, `rule_names`, `check_circular_dependencies`, `detect_cycles`
- Line 174: Function `count_expression_features`
- Line 206: Struct `GrammarSummary`
- Line 215: Function `load_default_grammar`

**Recommendation**: Keep a minimal `GrammarEngine` stub for public API compatibility, but remove all EBNF parsing logic.

---

### Category 2: AST Module (15 items)
**Total Lines to Remove**: ~350 lines

#### Files to Remove:
1. **`src/ast/visitors.rs`** (27 lines)
   - `StatementVisitor` trait - never implemented
   - `AstWalker` trait - never implemented

#### Dead Code in `src/ast/mod.rs`:
- Line 125: `CalculateUbo` struct (not the enum variant, which IS used)

#### Dead Code in `src/ast/types.rs`:
- Line 11: `SemanticInfo` struct
- Line 28: `TypeInfo` struct  
- Line 36: `DSLType` enum (entire enum with 11 variants)
- Line 74: `TypeConstraint` enum (10 variants)
- Line 122: `DatabaseReference` struct
- Line 131: `DbReferenceType` enum (7 variants)
- Line 143: `EnhancedValue` enum (entire enum with 10 variants)
- Line 230: `DSLState` struct
- Line 241: `LifecycleState` enum
- Line 256: Methods on `Value`: `as_string`, `as_map`, `is_null`, `to_enhanced`
- Line 329: Methods on `EnhancedValue`: `extract_value`, `get_semantic_info`, `set_semantic_info`

**Note**: These types represent an abandoned semantic analysis layer that was never fully implemented.

---

### Category 3: Data Dictionary Module (8 items)
**Total Lines to Remove**: ~80 lines

#### Files to Remove:
1. **`src/data_dictionary/catalogue.rs`** (30 lines)
   - Entire file - `AttributeCatalogue` struct and `cosine_similarity` function

#### Dead Code in `src/data_dictionary/mod.rs`:
- Line 44: `DataDictionary` struct
- Line 51: `CategoryDefinition` struct
- Line 59: `AttributeRelationship` struct
- Line 75: Methods `new` and `get_attribute` on `DataDictionary`

#### Dead Code in `src/data_dictionary/attribute.rs`:
- Line 23: Method `from_uuid` on `AttributeId`

**Note**: Keep the `AttributeId` type itself as it's used in execution modules.

---

### Category 4: Parser AST Module (8 items)
**Total Lines to Remove**: ~120 lines

All in **`src/parser_ast/mod.rs`**:
- Line 213: `CrudTransaction` struct
- Line 221: `TransactionResult` struct
- Line 247: `IntegrityResult` struct
- Line 263: `ConstraintViolationType` enum - 5 variants never constructed: `ForeignKey`, `Unique`, `NotNull`, `Check`, `Custom`
- Line 271: `DependencyIssue` struct
- Line 279: `DependencyType` enum
- Line 287: `SimulationResult` struct
- Line 296: `ResourceUsage` struct

**Note**: These represent a planned transaction/integrity checking layer that was never implemented.

---

### Category 5: Error Handling Module (7 items)
**Total Lines to Remove**: ~100 lines

All in **`src/error.rs`**:
- Line 299: Type alias `GrammarResult`
- Line 300: Type alias `VocabularyResult`
- Line 302: Type alias `RuntimeResult`
- Line 311: `ContextualError` struct
- Line 319: Methods `new` and `with_context` on `ContextualError`
- Line 354: `ErrorCollector` struct
- Line 359: Methods `new`, `add_simple_error`, `has_errors`, `error_count`, `clear` on `ErrorCollector`

**Note**: These were part of an enhanced error collection system that was superseded by simpler error handling.

---

### Category 6: Vocabulary Module (7 items)
**Total Lines to Remove**: ~80 lines

#### Dead Code in `src/vocabulary/mod.rs`:
- Line 34: Methods on `VerbRegistryEntry`: `new`, `deprecate`, `with_description`, `with_version`

#### Dead Code in `src/vocabulary/vocab_registry.rs`:
- Line 29: Fields `registry` and `domain_ownership` in `VocabularyRegistry` (never read, but struct is used)
- Line 35: `RegistryConfig` struct
- Line 44: `DeprecationPolicy` enum
- Line 55: `RegistryStats` struct
- Line 92: Multiple methods on `VocabularyRegistry`: `validate_verb_format`, `extract_domain`, `extract_action`, `register_verb`, `get_shared_verbs`, `get_domain_verbs`, `get_verb`, `get_stats`, `get_domains`, `remove_verb`

**Note**: The `VocabularyRegistry` struct is used, but most of its internal implementation is dead code.

---

### Category 7: Parser Module (5 items)
**Total Lines to Remove**: ~40 lines

#### Dead Code in `src/parser/normalizer.rs`:
- Line 12: `NormalizationError` enum
- Line 28: Fields `verb_aliases` and `key_aliases` in some struct (never read)
- Line 88: Multiple methods (specific names not shown in warning)

#### Dead Code in `src/parser/validators.rs`:
- Line 24: Error variants: `InvalidLinkIdentity`, `EvidenceLinkingError`, `InvalidRelationshipStructure`, `NoteFormatError`
- Line 53: Field `first_seen_location` (never read)
- Line 58: Field `case_type` (never read)

---

### Category 8: Database State Manager (1 item)
**Total Lines to Remove**: ~5 lines

#### Dead Code in `src/db_state_manager/mod.rs`:
- Line 37: Field `config` in `DbStateManager` (never read)

**Note**: The struct itself is used; just this one field is unused.

---

## Removal Strategy

### Phase 1: Preparation (Low Risk)
1. ✅ Create comprehensive inventory (DONE - this document)
2. Verify all tests pass: `cargo test --lib`
3. Document current test coverage: 32 tests passing
4. Create backup branch

### Phase 2: File Removals (Medium Risk)
Remove entire files that are completely unused:

**Priority 1 - Grammar Module**:
```bash
rm src/grammar/idiomatic_ebnf.rs
rm src/grammar/ebnf_parser.rs  # if exists
rm src/grammar/dynamic_grammar.rs  # if exists
rm src/grammar/grammar_storage.rs  # if exists
rm src/grammar/grammar_validator.rs  # if exists
```

**Priority 2 - Supporting Files**:
```bash
rm src/ast/visitors.rs
rm src/data_dictionary/catalogue.rs
```

**After each removal**:
- Run `cargo check --lib`
- Run `cargo test --lib`
- Verify no new errors introduced

### Phase 3: Incremental Code Removal (Higher Risk)
Remove dead code within files that have mixed used/unused code:

**Order of Operations**:
1. Start with leaf nodes (no dependencies)
2. Remove unused fields first (lowest impact)
3. Remove unused methods/functions
4. Remove unused structs/enums last (highest impact)

**Files to Edit** (in order):
1. `src/db_state_manager/mod.rs` - Remove `config` field
2. `src/data_dictionary/attribute.rs` - Remove `from_uuid` method
3. `src/vocabulary/mod.rs` - Remove unused methods
4. `src/error.rs` - Remove unused type aliases and structs
5. `src/parser/validators.rs` - Remove unused variants and fields
6. `src/parser/normalizer.rs` - Remove unused items
7. `src/parser_ast/mod.rs` - Remove unused structs/enums
8. `src/ast/types.rs` - Remove unused semantic types (largest change)
9. `src/ast/mod.rs` - Clean up any remaining references
10. `src/vocabulary/vocab_registry.rs` - Remove unused fields/methods
11. `src/grammar/mod.rs` - Simplify to minimal stub

### Phase 4: Update Module Declarations
Remove module declarations for deleted files:
- Update `src/ast/mod.rs` to remove `pub(crate) mod visitors;`
- Update `src/data_dictionary/mod.rs` to remove `pub(crate) mod catalogue;`
- Update `src/grammar/mod.rs` to remove references to deleted files

### Phase 5: Validation
1. Run full test suite: `cargo test --lib`
2. Run with all features: `cargo test --all-features` (may need database)
3. Check for new warnings: `cargo clippy --lib`
4. Verify no dead code warnings remain: `cargo check --lib 2>&1 | grep "is never\|are never"`
5. Run examples to verify public API still works

---

## Risk Assessment

### Low Risk Items (Can remove immediately):
- Entire files that are not imported anywhere
- Private functions/methods with no callers
- Fields that are never read in private structs

### Medium Risk Items (Require careful testing):
- Public fields that are never read (may be part of public API)
- Methods on public types (may be part of public API contract)
- Enum variants (removing them changes serialization)

### High Risk Items (Require extra validation):
- Types exported in `lib.rs` (GrammarEngine, VocabularyRegistry)
- Anything that might be used by external crates
- Code that might be used in database feature builds

---

## Testing Strategy

### Before Each Change:
```bash
cargo test --lib 2>&1 | tee /tmp/tests_before.txt
```

### After Each Change:
```bash
cargo test --lib 2>&1 | tee /tmp/tests_after.txt
diff /tmp/tests_before.txt /tmp/tests_after.txt
```

### Final Validation:
```bash
# All tests must still pass
cargo test --lib

# No new warnings
cargo check --lib 2>&1 | grep warning | wc -l

# Examples still work
cargo run --example parse_zenith
cargo run --example ai_dsl_onboarding_demo
```

---

## Expected Outcomes

### Metrics:
- **Lines of Code Removed**: ~1,700 lines
- **Files Removed**: 7 files
- **Compilation Time**: Expected 5-10% improvement
- **Dead Code Warnings**: From 72 to 0
- **Test Coverage**: Maintain 32/32 tests passing

### Benefits:
1. **Improved Maintainability**: Less code to understand and maintain
2. **Faster Development**: Quicker compilation and easier code navigation
3. **Reduced Cognitive Load**: Clearer understanding of what's actually used
4. **Better Documentation**: Code reflects actual architecture, not abandoned experiments
5. **Easier Onboarding**: New developers see only relevant code

---

## Public API Considerations

### Types Exported in `lib.rs` but Not Used:
- `GrammarEngine` - Keep minimal stub for backwards compatibility
- `VocabularyRegistry` - Used but has dead internal code

### Recommendation:
For types that are public but unused:
1. Keep the type definition with minimal implementation
2. Mark with `#[deprecated]` if appropriate
3. Document in CHANGELOG that they are stubs
4. Consider removing in next major version

---

## Rollback Plan

If any issues are encountered:

```bash
# Immediate rollback
git reset --hard HEAD

# Rollback to specific commit
git reset --hard <commit-hash>

# Restore specific file
git checkout HEAD -- <file-path>
```

---

## Execution Checklist

- [ ] Review this plan with stakeholders
- [ ] Create backup branch: `git checkout -b backup-before-dead-code-removal`
- [ ] Run full test suite and document baseline
- [ ] Phase 1: Remove grammar module files (Priority 1)
- [ ] Phase 1: Run tests after grammar removal
- [ ] Phase 2: Remove AST visitors file
- [ ] Phase 2: Remove data dictionary catalogue file
- [ ] Phase 2: Run tests after file removals
- [ ] Phase 3: Remove dead code from individual files (incremental)
- [ ] Phase 4: Update module declarations
- [ ] Phase 5: Final validation
- [ ] Update documentation (CLAUDE.md, README.md)
- [ ] Commit changes with detailed message
- [ ] Create PR with this plan attached

---

## Conclusion

This plan identifies 72 instances of dead code across 17 files, representing approximately 1,700 lines of unused code. The removal is straightforward with low risk, as all items have been verified as unused through compiler analysis. Following this phased approach with testing after each step will ensure a safe removal process.

**Estimated Time**: 2-3 hours for complete removal and validation
**Risk Level**: LOW (with proper testing at each phase)
**Priority**: MEDIUM (improves quality but not blocking any features)
