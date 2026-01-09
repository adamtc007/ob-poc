# TODO: Consolidate DSL Execution to Single Path

> **Status:** Planning
> **Priority:** High - Data integrity issue
> **Created:** 2026-01-09
> **Related:** 014-session-scope-unified-state.md

---

## Problem

Currently two DSL execution paths exist:

| Endpoint | Session Aware | Bindings Persisted | Watch Fires |
|----------|---------------|-------------------|-------------|
| `/api/session/:id/execute` | ✓ Yes | ✓ Yes | ✓ Yes |
| `/execute` | ✗ No | ✗ No | ✗ No |

**Impact:** Bindings created via `/execute` are:
- Lost on page refresh
- Not visible in session captures panel
- Not available to subsequent DSL executions in other tabs
- Not triggering viewport updates

Batch executions are just sets of singletons - they must use the same path.

---

## Target State

**ONE execution path:** `/api/session/:id/execute`

```
┌─────────────────────────────────────────────────────────────────┐
│                     ALL DSL EXECUTION                           │
│                                                                 │
│   egui WASM ──┐                                                │
│               │                                                 │
│   Agent Chat ─┼──▶ POST /api/session/:id/execute               │
│               │              │                                  │
│   MCP Tools ──┤              ▼                                  │
│               │    ┌─────────────────────┐                     │
│   Batch Ops ──┘    │  ExecutionContext   │                     │
│                    │  + session_id       │                     │
│                    └──────────┬──────────┘                     │
│                               │                                 │
│                               ▼                                 │
│                    ┌─────────────────────┐                     │
│                    │  SessionManager     │                     │
│                    │  - persist bindings │                     │
│                    │  - update scope     │                     │
│                    │  - notify watchers  │                     │
│                    └──────────┬──────────┘                     │
│                               │                                 │
│                               ▼                                 │
│                    Watch notification → UI updates              │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Implementation Checklist

### Phase 1: Audit Current Usage

- [ ] **1.1** Search egui WASM codebase for `/execute` calls
  - Location: `rust/crates/ob-poc-ui/src/`
  - Find: `fetch.*execute` or `post.*execute`
  
- [ ] **1.2** Search for any other callers of `/execute`
  - MCP handlers
  - Test files
  - Scripts

- [ ] **1.3** Confirm egui WASM establishes session on startup
  - Must have session_id before any DSL execution
  - Check session creation in app init

### Phase 2: Update egui WASM

- [ ] **2.1** Update `api.rs` - change execute function
  ```rust
  // BEFORE
  pub async fn execute_dsl(dsl: &str, bindings: Option<HashMap<String, Uuid>>) 
      -> Result<DirectExecuteResponse, String> {
      post("/execute", &DirectExecuteRequest { dsl, bindings }).await
  }
  
  // AFTER
  pub async fn execute_dsl(
      session_id: Uuid,
      dsl: &str, 
      bindings: Option<HashMap<String, Uuid>>
  ) -> Result<ExecuteResponse, String> {
      post(
          &format!("/api/session/{}/execute", session_id),
          &ExecuteDslRequest { dsl, bindings, ... }
      ).await
  }
  ```

- [ ] **2.2** Update all call sites to pass session_id
  - `state.rs` - DSL execution triggers
  - `panels/*.rs` - any direct execution calls
  - `app.rs` - command handling

- [ ] **2.3** Update response type handling
  - `DirectExecuteResponse` → `ExecuteResponse`
  - Handle new response fields (new_state, etc.)

- [ ] **2.4** Remove local binding tracking (if any)
  - Session is now source of truth
  - Watch loop will sync bindings

### Phase 3: Update Batch Execution

- [ ] **3.1** Review `batch_executor.rs`
  - Currently creates fresh `ExecutionContext::new()`
  - Needs to accept session_id parameter

- [ ] **3.2** Update `BatchExecutor` to propagate session
  ```rust
  impl BatchExecutor {
      pub fn with_session(mut self, session_id: Uuid) -> Self {
          self.session_id = Some(session_id);
          self
      }
      
      async fn execute_one(&mut self, ...) {
          let mut ctx = if let Some(sid) = self.session_id {
              ExecutionContext::new().with_session(sid)
          } else {
              ExecutionContext::new()
          };
          // ...
      }
  }
  ```

- [ ] **3.3** Update batch API endpoints
  - `/api/batch/add-products` - accept session_id
  - Any other batch endpoints

- [ ] **3.4** Ensure batch bindings propagate to session
  - Each iteration's bindings persisted
  - Or summary bindings at end

### Phase 4: Update MCP Handlers

- [ ] **4.1** Review `mcp/handlers/core.rs`
  - Already has optional session support
  - Ensure it's used consistently

- [ ] **4.2** Make session_id required for `execute_dsl` tool
  - Or provide clear error if missing

### Phase 5: Remove Old Endpoint

- [ ] **5.1** Remove `DirectExecuteRequest` struct
  - Location: `api/agent_routes.rs:4187`

- [ ] **5.2** Remove `DirectExecuteResponse` struct
  - Location: `api/agent_routes.rs:4193`

- [ ] **5.3** Remove `direct_execute_dsl` handler
  - Location: `api/agent_routes.rs:4295`

- [ ] **5.4** Remove route registration
  - Location: `api/agent_routes.rs:698`
  - Remove: `.route("/execute", post(direct_execute_dsl))`

- [ ] **5.5** Remove from egui WASM api.rs
  - Old request/response types
  - Old function signature

### Phase 6: Update Tests

- [ ] **6.1** Update integration tests that use `/execute`
  - Change to `/api/session/:id/execute`
  - Create session in test setup

- [ ] **6.2** Add test: singleton execution persists bindings
  ```rust
  #[tokio::test]
  async fn test_singleton_persists_to_session() {
      let session = create_session().await;
      execute_dsl(session.id, "(entity.create :name 'Test' :as @test)").await;
      
      let bindings = get_session_bindings(session.id).await;
      assert!(bindings.contains_key("test"));
  }
  ```

- [ ] **6.3** Add test: batch execution persists bindings
  ```rust
  #[tokio::test]
  async fn test_batch_persists_to_session() {
      let session = create_session().await;
      batch_add_products(session.id, cbu_ids, products).await;
      
      let bindings = get_session_bindings(session.id).await;
      // Verify expected bindings exist
  }
  ```

### Phase 7: Documentation

- [ ] **7.1** Update API documentation
  - Remove `/execute` endpoint docs
  - Document session requirement

- [ ] **7.2** Update CLAUDE.md
  - Note single execution path
  - Session prerequisite for DSL execution

---

## Verification

After implementation, verify:

1. **egui WASM singleton execution:**
   ```
   Run: (entity.create :name "Test Entity" :as @test)
   Check: Session captures panel shows @test
   Check: Refresh page, re-watch session, @test still visible
   ```

2. **Batch execution:**
   ```
   Run: batch add-products for 5 CBUs
   Check: Session has bindings for created resources
   ```

3. **Watch notification:**
   ```
   Tab A: Execute DSL creating @fund
   Tab B: Watching same session
   Check: Tab B receives watch update with @fund binding
   ```

4. **Old endpoint removed:**
   ```
   POST /execute → 404 Not Found
   ```

---

## Files to Modify

| File | Changes |
|------|---------|
| `rust/src/api/agent_routes.rs` | Remove `/execute` endpoint |
| `rust/crates/ob-poc-ui/src/api.rs` | Update execute function |
| `rust/crates/ob-poc-ui/src/state.rs` | Pass session_id to execute |
| `rust/src/dsl_v2/batch_executor.rs` | Accept session_id |
| `rust/src/mcp/handlers/core.rs` | Verify session propagation |

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Breaking egui WASM | Update client before removing endpoint |
| Breaking MCP tools | MCP already has session support |
| Breaking tests | Update tests in same PR |
| Performance (extra session lookup) | Negligible - already doing session work |

---

## Estimated Effort

| Phase | Effort |
|-------|--------|
| 1. Audit | 30min |
| 2. egui WASM | 2h |
| 3. Batch | 1h |
| 4. MCP | 30min |
| 5. Remove old | 30min |
| 6. Tests | 1h |
| 7. Docs | 30min |
| **Total** | **~6h** |

---

## Notes

- This is a breaking change for any external callers of `/execute`
- Go UI is gone, so no external impact expected
- Batch operations are internal, controlled migration
- MCP already supports sessions, just needs verification

