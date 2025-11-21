# OB-POC Implementation Plan: DSL State Machine Improvements

**Generated:** November 20, 2025  
**Based on:** Agent Review Analysis  
**Goal:** Fully functional DSL state machine

---

## Executive Summary

The agent review identified a **70% complete** system with solid foundations but critical gaps. This plan prioritizes fixes to achieve a production-ready DSL execution engine.

---

## Phase 1: Critical Fixes (Week 1)

### 1.1 Fix AST Value Type Duplication
**Priority:** CRITICAL | **Effort:** 2 hours | **File:** `src/parser/ast.rs`

**Problem:** Duplicate variants cause ambiguity (String, Integer, Array duplicate Literal types)

**Action:** Remove duplicates, add helper methods (as_string, as_number, as_bool)

### 1.2 Fix Attribute Reference Parsing
**Priority:** CRITICAL | **Effort:** 30 min | **File:** `src/parser/idiomatic_parser.rs`

**Problem:** Requires `@attr{"uuid"}` instead of `@attr{uuid}`

**Action:** Parse UUID directly without quotes

### 1.3 Add Stack Effect Validation
**Priority:** CRITICAL | **Effort:** 4 hours | **File:** `src/forth_engine/compiler.rs`

**Problem:** Stack effects defined but never validated

**Action:** Add validate_stack_effects() in compile_sheet()

### 1.4 Add Validation Phase to Parser
**Priority:** CRITICAL | **Effort:** 1 day | **Files:** `src/parser/validation.rs` (new)

**Problem:** No validation of verbs, attributes, or required parameters

**Action:** Create parse_and_validate() that validates verbs, params, and attribute refs

---

## Phase 2: High Priority (Week 2-3)

### 2.1 Implement Typed Word Functions
**Priority:** HIGH | **Effort:** 1 week | **File:** `src/forth_engine/kyc_vocab.rs`

**Problem:** generic_word consumes entire stack, loses type safety

**Action:** Replace with strongly-typed functions using WordSignature and ParamSpec

### 2.2 Add Transaction Support
**Priority:** HIGH | **Effort:** 3 days | **Files:** `src/forth_engine/mod.rs`, `src/database/`

**Problem:** No atomic transaction support for DSL execution

**Action:** Wrap execution in database transaction, rollback on error

### 2.3 Unify Error Types
**Priority:** HIGH | **Effort:** 1 week | **File:** `src/error.rs`

**Problem:** Too many error types scattered across modules

**Action:** Create unified ObPocError with Parse, Validation, Compile, Runtime, Database variants

### 2.4 Add Comprehensive Tests
**Priority:** HIGH | **Effort:** 1 week | **Files:** `src/tests/`, `tests/`

**Action:** Add parser, compiler, VM unit tests; integration tests; database tests

---

## Phase 3: Medium Priority (Month 2)

### 3.1 Implement HIR (High-level IR)
**Effort:** 2 weeks | **Files:** `src/hir/` (new)

Typed intermediate representation for optimization passes

### 3.2 Add Control Flow Support
**Effort:** 1 week | **Files:** VM, Compiler

Branch, BranchIfFalse, Return opcodes; word definitions calling other words

### 3.3 Implement Optimizer
**Effort:** 2 weeks | **Files:** `src/optimizer/` (new)

Constant folding, dead code elimination, inlining

### 3.4 Write Documentation
**Effort:** 1 week | **Files:** `docs/`

ARCHITECTURE.md, LANGUAGE.md, TUTORIAL.md, rustdoc

---

## Phase 4: Low Priority (Month 3+)

- Add Debugger (step-through, breakpoints, stack inspection)
- Performance Optimization (arena allocation, constant pools)
- Property-Based Tests (proptest for parser round-trip)

---

## Implementation Checklist

### Week 1 (Critical)
- [ ] Fix AST Value duplication
- [ ] Fix attribute ref parsing
- [ ] Add stack effect validation
- [ ] Add validation phase

### Week 2-3 (High Priority)
- [ ] Implement typed word functions
- [ ] Add transaction support
- [ ] Unify error types
- [ ] Add comprehensive tests

### Month 2 (Medium Priority)
- [ ] Implement HIR
- [ ] Add control flow support
- [ ] Implement optimizer
- [ ] Write documentation

---

## Key Issues from Review

1. **Parser/VM Mismatch** - S-expressions parsed but postfix expected (currently works by accident)
2. **Type Safety Lost** - generic_word consumes all stack items without validation
3. **Return Stack Unused** - No support for recursion or nested word calls
4. **No Transaction Support** - DSL executes before DB saves, partial failures possible
5. **Duplicate AST Types** - Value enum has overlapping variants

---

## Success Criteria

- All critical issues resolved
- 80%+ test coverage
- Type-safe word execution
- Atomic database transactions
- Compile-time error detection
- Unified error handling

