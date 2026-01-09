# Session Scope Management - Implementation Status

> **Status:** ✅ Core E2E Wiring Complete
> **Created:** 2026-01-09
> **Updated:** 2026-01-09

---

## Implementation Summary

### ✅ Completed (Phases 1-9)

| Phase | Component | Key Changes |
|-------|-----------|-------------|
| 1 | Database Schema | `012_session_scope_management.sql` - session_scopes + history |
| 2 | DSL Verbs | `rust/config/verbs/session.yaml` - 14 verbs |
| 3 | Plugin Handlers | `rust/src/dsl_v2/custom_ops/session_ops.rs` |
| 4 | Handler Registration | `custom_ops/mod.rs` - all registered |
| 5 | ExecutionContext | `pending_scope_change: Option<GraphScope>` side-door pattern |
| 6 | Session Handlers | All ops call `ctx.set_pending_scope_change()` |
| 7 | API Propagation | MCP + agent_routes propagate to SessionContext |
| 8 | SessionManager | `scope_definition`, `scope_loaded` in SessionSnapshot |
| 9 | UI Reactivity | WatchResponse triggers viewport rebuild |

### 📍 Remaining Work

| Phase | Description | Effort | Blocked By |
|-------|-------------|--------|------------|
| 10 | Agent context uses scope for entity disambiguation | 3h | - |
| 11 | Lexicon patterns for NL scope commands | 3h | - |
| 12 | REPL prompt shows current scope | 1h | - |
| 13 | Visual breadcrumb + scope badge in viewport | 2h | - |

---

## E2E Data Flow (Working)

```
User: (session.set-galaxy :apex-entity-id <uuid>)
         │
         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  SessionSetGalaxyOp.execute()                                               │
│  1. UPDATE session_scopes SET scope_type = 'GALAXY'...                      │
│  2. ctx.set_pending_scope_change(GraphScope::Book {...})                    │
└─────────────────────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  API Route (post-execution)                                                 │
│  1. ctx.take_pending_scope_change()                                         │
│  2. session_context.set_scope(SessionScope::from_graph_scope(...))          │
└─────────────────────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  SessionManager.notify_watchers()                                           │
│  1. SessionSnapshot includes scope_type, scope_loaded                       │
│  2. WatchResponse sent to UI                                                │
└─────────────────────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  UI Viewport (state.rs)                                                     │
│  1. Receives WatchResponse with scope change                                │
│  2. Triggers graph refetch for new scope                                    │
│  3. Viewport rebuilds with scoped entities                                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## GraphScope Mappings

| Session Verb | GraphScope Variant |
|--------------|-------------------|
| `session.set-galaxy` | `GraphScope::Book` |
| `session.set-book` | `GraphScope::Book` |
| `session.set-cbu` | `GraphScope::SingleCbu` |
| `session.set-jurisdiction` | `GraphScope::Jurisdiction` |
| `session.set-entity-neighborhood` | `GraphScope::EntityNeighborhood` |
| `session.clear` | `GraphScope::Empty` |

---

## Key Files Modified

| File | Changes |
|------|---------|
| `migrations/012_session_scope_management.sql` | Session scopes table + history |
| `rust/config/verbs/session.yaml` | 14 session verbs defined |
| `rust/src/dsl_v2/custom_ops/session_ops.rs` | Plugin handlers |
| `rust/src/dsl_v2/custom_ops/mod.rs` | Handler registration |
| `rust/src/dsl_v2/executor.rs` | `pending_scope_change` field + accessors |
| `rust/src/mcp/handlers/core.rs` | Propagate scope to session |
| `rust/src/api/agent_routes.rs` | Propagate scope to SessionContext |
| `rust/src/api/session.rs` | `scope` field + `SessionScope::from_graph_scope()` |
| `rust/src/session/manager.rs` | `scope_definition`, `scope_loaded` in snapshot |
| `rust/crates/ob-poc-ui/src/api.rs` | WatchSessionResponse scope fields |
| `rust/crates/ob-poc-ui/src/state.rs` | Trigger graph refetch on scope change |

---

## Remaining TODO Details

### Phase 10: Agent Context Integration

Agent should use current scope for entity disambiguation:

```rust
// In agent context building (rust/src/session/agent_context.rs):
let scope = session_context.scope();
match scope {
    SessionScope::SingleCbu { cbu_id, .. } => {
        // Constrain entity resolution to this CBU
        gateway.set_cbu_filter(cbu_id);
    }
    SessionScope::Book { apex_entity_id, jurisdictions, .. } => {
        // Constrain to CBUs under apex, filtered by jurisdictions
        gateway.set_book_filter(apex_entity_id, jurisdictions);
    }
    // etc.
}
```

**Files to modify:**
- `rust/src/session/agent_context.rs`
- `rust/src/entities/gateway.rs` (add scope filters)

### Phase 11: Lexicon Natural Language Patterns

Add scope command patterns to lexicon:

```yaml
# rust/config/agent/lexicon.yaml
patterns:
  - pattern: "show me {entity}"
    intent: set_scope
    verb: session.set-galaxy
    args:
      apex-entity-id: "{entity}"
      
  - pattern: "focus on {jurisdiction}"
    intent: set_scope  
    verb: session.set-jurisdiction
    args:
      jurisdiction: "{jurisdiction}"
      
  - pattern: "zoom into {cbu}"
    intent: set_scope
    verb: session.set-cbu
    args:
      cbu-id: "{cbu}"
      
  - pattern: "go back"
    intent: navigate
    verb: session.back
```

**Files to modify:**
- `rust/config/agent/lexicon.yaml`
- `rust/src/agent/intent.rs` (handle set_scope intent)

### Phase 12: REPL Prompt

Show current scope in REPL prompt:

```
[Empty] > (session.set-galaxy :apex-entity-id "Allianz SE")
[Allianz SE (177 CBUs)] > (session.set-jurisdiction :jurisdiction "LU")
[Allianz SE / LU (47 CBUs)] > (session.set-cbu :cbu-id "Acme Fund")
[Acme Fund] > 
```

**Files to modify:**
- `rust/src/repl/mod.rs` (or wherever prompt is built)
- Read from SessionContext or SharedSession

### Phase 13: Visual Breadcrumb

Add breadcrumb component to viewport showing:
- Current scope type icon (🌐 Galaxy, 📚 Book, 📁 CBU, 🗺 Jurisdiction)
- Scope hierarchy: "Allianz SE > Luxembourg > Acme Fund"
- CBU count: "(47 CBUs)"
- Back/Forward buttons

**Files to modify:**
- `rust/crates/ob-poc-ui/src/panels/` (new breadcrumb panel)
- `rust/crates/ob-poc-ui/src/app.rs` (add to layout)

---

## Architecture Decisions Made

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Change notification | Side-door pattern via ExecutionContext | Simple, no channels needed |
| Scope propagation | API routes extract and propagate | Clean separation |
| UI notification | WatchResponse polling | Works with existing session watch |
| Graph refresh | UI triggers refetch on scope change | Viewport owns graph lifecycle |

---

## Success Criteria

| # | Criterion | Status |
|---|-----------|--------|
| 1 | User types `(session.set-cbu :cbu-id "Acme Fund")` | ✅ |
| 2 | Database updated with new scope | ✅ |
| 3 | ExecutionContext holds pending change | ✅ |
| 4 | API route propagates to SessionContext | ✅ |
| 5 | Viewport receives WatchResponse | ✅ |
| 6 | Viewport rebuilds graph for new scope | ✅ |
| 7 | Breadcrumb shows "Acme Fund" | ⏳ Phase 13 |
| 8 | Agent uses scope for disambiguation | ⏳ Phase 10 |
| 9 | Natural language "show me Allianz" works | ⏳ Phase 11 |
| 10 | REPL prompt shows scope | ⏳ Phase 12 |

---

## Notes

- Core wiring is complete - scope changes flow E2E
- Remaining work is UX polish (prompts, breadcrumbs) and agent integration
- Test with Allianz data: galaxy (177 funds), book (LU subset), single CBU
