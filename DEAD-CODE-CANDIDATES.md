# DEAD-CODE-CANDIDATES.md

> **Phase 0 Artifact** — Dead code flagged for removal during Phase A
> **Date:** 2026-02-06
> **Removal Phase:** Phase A (unless otherwise noted)

---

## 1. Structs to Delete

### 1.1 `ClientContext` — DELETE in Phase A

**File:** `rust/src/repl/session_v2.rs`

```rust
pub struct ClientContext {
    pub client_group_id: Option<Uuid>,
    pub client_group_name: Option<String>,
    pub default_cbu: Option<Uuid>,
    pub default_book: Option<String>,
}
```

**Reason:** All 4 fields are already stored on `Runbook` or derivable from runbook entries:
- `client_group_id` → `Runbook.client_group_id` (already written in `set_client_context()`)
- `client_group_name` → Derivable from `session.load-cluster` entry args
- `default_cbu` → First CBU from scope, derivable from execution results
- `default_book` → Derivable from `session.load-cluster` entry args

**Dependents to update:**
- `ReplSessionV2.client_context` field
- `ReplSessionV2::new()` — remove initialization
- `ReplSessionV2::set_client_context()` — delete method
- `orchestrator_v2.rs` — ~8 reads of `session.client_context.*`
- `repl_v2_integration.rs` — test assertions on client_context
- `repl_v2_phase6.rs` — test assertions on client_context
- `repl_v2_golden_loop.rs` — test assertions on client_context

### 1.2 `JourneyContext` — DELETE in Phase H (partial removal in Phase A)

**File:** `rust/src/repl/session_v2.rs`

```rust
pub struct JourneyContext {
    pub pack: Option<Arc<PackManifest>>,
    pub pack_manifest_hash: Option<String>,
    pub answers: HashMap<String, String>,
    pub template_id: Option<String>,
    pub progress: usize,
    pub handoff_source: Option<String>,
}
```

**Reason:** All fields derivable from runbook fold:
- `pack` → Arc loaded from `pack.select` entry's pack_id via PackRouter
- `pack_manifest_hash` → From `pack.select` entry
- `answers` → New `pack.answer` runbook entries (must be created first)
- `template_id` → From `pack.select` entry
- `progress` → Count of completed entries matching template steps
- `handoff_source` → Previous `pack.select` entry in runbook

**Phase A action:** Create `ContextStack.pack_context()` that derives this. Delete reads.
**Phase H action:** Delete the struct itself after all tests migrated.

**Dependents to update:**
- `ReplSessionV2.journey_context` field
- `ReplSessionV2::activate_pack()` — replace with `pack.select` runbook entry
- `ReplSessionV2::set_journey_answers()` — replace with `pack.answer` entries
- `ReplSessionV2::set_journey_progress()` — delete (derived from entry count)
- `ReplSessionV2::set_handoff_source()` — delete (derived from runbook)
- `ReplSessionV2::clear_journey()` — replace with pack transition entry
- `ReplSessionV2::rehydrate()` — simplify (no Arc<PackManifest> to restore)
- `orchestrator_v2.rs` — ~19 reads of `session.journey_context.*`

---

## 2. Methods to Delete

### 2.1 Session Methods (Phase A)

| Method | File | Reason |
|--------|------|--------|
| `set_client_context()` | `session_v2.rs` | Replaced by `ContextStack.from_runbook()` |
| `set_journey_progress()` | `session_v2.rs` | Derived from completed entry count |
| `set_handoff_source()` | `session_v2.rs` | Derived from runbook `pack.select` entries |

### 2.2 Session Methods (Phase H)

| Method | File | Reason |
|--------|------|--------|
| `activate_pack()` | `session_v2.rs` | Replaced by `pack.select` runbook entry |
| `set_journey_answers()` | `session_v2.rs` | Replaced by `pack.answer` runbook entries |
| `clear_journey()` | `session_v2.rs` | Pack transition = new `pack.select` entry |
| `rehydrate()` | `session_v2.rs` | Simplified — no Arc<PackManifest> to restore |

### 2.3 Orchestrator Methods (Phase C)

| Method | File | Reason |
|--------|------|--------|
| Context-free `search()` on IntentMatcher | `intent_matcher.rs` | Replaced by `search_with_context()` |

---

## 3. Duplicated State to Collapse

### 3.1 Runbook ↔ Session State Duplication

These fields exist in both places and are written simultaneously:

| Runbook Field | Session Field | Action |
|---------------|---------------|--------|
| `runbook.client_group_id` | `client_context.client_group_id` | Keep on runbook only |
| `runbook.pack_id` | `journey_context.pack.id` | Keep on runbook only |
| `runbook.pack_version` | `journey_context.pack.version` | Keep on runbook only |
| `runbook.pack_manifest_hash` | `journey_context.pack_manifest_hash` | Keep on runbook only |
| `runbook.template_id` | `journey_context.template_id` | Keep on runbook only |

**Phase A action:** Stop writing to session fields. Reads go through `ContextStack.from_runbook()`.

---

## 4. Dangerous Patterns to Fix

### 4.1 Handoff Clears Runbook (CRITICAL)

**File:** `orchestrator_v2.rs`, `handle_handoff()` (~line 2862)

The handoff path clears the entire runbook when transitioning between packs. This destroys history and violates Invariant I-1 (runbook is sole durable artifact).

**Current behavior:**
```
Pack A completes → handoff → runbook.clear() → Pack B starts fresh
```

**Required behavior:**
```
Pack A completes → (pack.select :pack-id "pack-b" :handoff-from "pack-a") → Pack B starts with full history
```

**Fix phase:** Phase B (when `pack.select` becomes a DSL verb)

### 4.2 `set_state()` Proliferation

**File:** `orchestrator_v2.rs` — ~40 direct `set_state()` calls

These are not dead code but represent a smell: state should be derivable from runbook + current interaction. Many transitions could be computed from `(runbook_status, last_input_type)` rather than imperatively set.

**Fix phase:** Phase H (full state machine simplification)

---

## 5. Unused or Low-Value Code

### 5.1 Candidates Requiring Verification

These are suspected low-usage paths. Verify with test coverage before removing.

| Code | File | Suspicion | Action |
|------|------|-----------|--------|
| `ReplCommandV2::Resume` | `types_v2.rs` | Park/resume may not have test coverage | Verify tests, keep if covered |
| `ConfirmPolicy::PackConfigured` | `runbook.rs` | Not yet wired to pack manifests | Keep — needed for Phase E |
| `SlotSource::CopiedFromPrevious` | `runbook.rs` | carry_forward not yet implemented | Keep — needed for Phase E |
| `InvocationRecord` fields beyond `correlation_key` | `runbook.rs` | Durable execution scaffolding | Keep — needed for production |

### 5.2 Feature Flag Scope

All V2 REPL code is behind `vnext-repl` feature flag. New modules in Phase A+ should also be gated behind this flag.

```rust
// In repl/mod.rs — existing pattern
#[cfg(feature = "vnext-repl")]
pub mod context_stack;  // Phase A
#[cfg(feature = "vnext-repl")]
pub mod scoring;        // Phase C
```

---

## 6. Summary

| Category | Count | Phase |
|----------|-------|-------|
| Structs to delete | 2 | A (ClientContext), H (JourneyContext) |
| Methods to delete | 3 immediate + 4 deferred | A + H |
| Duplicated state fields | 5 | A |
| Dangerous patterns | 1 (handoff clears runbook) | B |
| State proliferation | ~40 `set_state()` calls | H |
| Low-value suspects | 4 (need verification) | — |

**Total estimated dead code removal:** ~200 lines in Phase A, ~150 lines in Phase H.

---

## 7. Phase A Removal Checklist

When implementing Phase A, remove these in order:

1. [ ] Create `ContextStack.from_runbook()` that derives all session state
2. [ ] Replace all `session.client_context.*` reads with `ContextStack` accessors
3. [ ] Delete `set_client_context()` method
4. [ ] Delete `ClientContext` struct
5. [ ] Remove `client_context` field from `ReplSessionV2`
6. [ ] Update test assertions to use `ContextStack` instead of `client_context`
7. [ ] Replace `session.journey_context.pack` reads with `ContextStack.pack_context()`
8. [ ] Replace `session.journey_context.answers` reads with `ContextStack.accumulated_answers()`
9. [ ] Replace `session.journey_context.progress` reads with derived count
10. [ ] Stop writing journey_context fields (reads now go through ContextStack)
11. [ ] Wire `mod context_stack` into `repl/mod.rs` behind `vnext-repl`
