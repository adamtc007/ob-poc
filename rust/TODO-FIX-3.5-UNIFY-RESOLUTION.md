# Fix 3.5: Unify Resolution Systems - Rip and Replace Plan

## Executive Summary

**DELETE** `ResolutionService` (1500 lines). **REPLACE** with direct session state access using `ResolutionSubSession`.

The old system duplicates state. The new system keeps everything in the session. Rip and replace is cleaner than migration.

---

## The Two Systems

| Aspect | OLD: `ResolutionService` | NEW: `ResolutionSubSession` |
|--------|--------------------------|----------------------------|
| Location | `src/services/resolution_service.rs` | `src/api/session.rs` |
| Storage | Separate `DashMap<Uuid, SessionEntry>` | Inside `AgentSession.sub_session` |
| Key format | Text: `"{entity_type}:{value}"` | Location: `"{stmt}:{span.start}-{span.end}"` |
| Used by | `resolution_routes.rs` | `agent_routes.rs`, MCP handlers |
| Lines | 1513 | ~50 |

**Problem**: Two sources of truth → drift, bugs, maintenance burden.

---

## Rip and Replace Strategy

### Phase A: Enhance ResolutionSubSession (ADD)

Add the missing methods to `session.rs` that resolution_routes needs:

```rust
// In src/api/session.rs

impl ResolutionSubSession {
    /// Start resolution - extract unresolved refs from AST
    pub fn from_ast(ast: &[Statement], gateway: &GatewayRefResolver) -> Self {
        // ... extract unresolved refs, pre-fetch matches
    }
    
    /// Search for matches (re-search with different query/discriminators)
    pub async fn search(
        &self,
        ref_id: &str,
        query: &str,
        discriminators: &HashMap<String, String>,
        limit: usize,
        gateway: &GatewayRefResolver,
    ) -> Vec<EntityMatchInfo> {
        // ... search via gateway
    }
    
    /// Select a resolution (with gateway validation)
    pub fn select(&mut self, ref_id: &str, resolved_key: &str) -> Result<(), SelectionError> {
        self.resolutions.insert(ref_id.to_string(), resolved_key.to_string());
        Ok(())
    }
    
    /// Check if all refs are resolved
    pub fn is_complete(&self) -> bool {
        self.unresolved_refs.iter().all(|r| self.resolutions.contains_key(&r.ref_id))
    }
    
    /// Apply resolutions to AST (commit)
    pub fn apply_to_ast(&self, ast: &mut [Statement]) -> Result<(), CommitError> {
        // ... walk AST, apply resolved_key by ref_id
    }
}
```

### Phase B: Rewrite resolution_routes.rs (REPLACE)

Delete the entire file and rewrite from scratch. New version uses session state directly:

```rust
// src/api/resolution_routes.rs - NEW VERSION

use axum::{extract::{Path, State}, http::StatusCode, Json, Router, routing::{get, post}};
use uuid::Uuid;
use crate::api::session::{SessionStore, SubSessionType, ResolutionSubSession};
use crate::dsl_v2::gateway_resolver::GatewayRefResolver;

/// Shared state - just needs session store and gateway
#[derive(Clone)]
pub struct ResolutionState {
    pub session_store: SessionStore,
    pub gateway: GatewayRefResolver,
}

/// POST /api/session/:session_id/resolution/start
pub async fn start_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ResolutionResponse>, (StatusCode, String)> {
    let mut sessions = state.session_store.write().await;
    let session = sessions.get_mut(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".into()))?;
    
    // Create ResolutionSubSession from current AST
    let resolution = ResolutionSubSession::from_ast(&session.context.ast, &state.gateway).await;
    
    // Store in session
    session.sub_session = SubSessionType::Resolution(resolution.clone());
    
    Ok(Json(resolution.to_response()))
}

/// GET /api/session/:session_id/resolution
pub async fn get_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ResolutionResponse>, (StatusCode, String)> {
    let sessions = state.session_store.read().await;
    let session = sessions.get(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".into()))?;
    
    match &session.sub_session {
        SubSessionType::Resolution(res) => Ok(Json(res.to_response())),
        _ => Err((StatusCode::NOT_FOUND, "No active resolution".into())),
    }
}

/// POST /api/session/:session_id/resolution/search
pub async fn search_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, (StatusCode, String)> {
    let sessions = state.session_store.read().await;
    let session = sessions.get(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".into()))?;
    
    let resolution = match &session.sub_session {
        SubSessionType::Resolution(res) => res,
        _ => return Err((StatusCode::NOT_FOUND, "No active resolution".into())),
    };
    
    let matches = resolution.search(
        &body.ref_id,
        &body.query,
        &body.discriminators,
        body.limit.unwrap_or(10),
        &state.gateway,
    ).await;
    
    Ok(Json(SearchResponse { matches }))
}

/// POST /api/session/:session_id/resolution/select
pub async fn select_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<SelectRequest>,
) -> Result<Json<SelectResponse>, (StatusCode, String)> {
    // SECURE: Validate against gateway first
    state.gateway.validate_exists(&body.resolved_key, &body.entity_type).await
        .map_err(|_| (StatusCode::NOT_FOUND, "Entity not found in gateway".into()))?;
    
    let mut sessions = state.session_store.write().await;
    let session = sessions.get_mut(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".into()))?;
    
    let resolution = match &mut session.sub_session {
        SubSessionType::Resolution(res) => res,
        _ => return Err((StatusCode::NOT_FOUND, "No active resolution".into())),
    };
    
    resolution.select(&body.ref_id, &body.resolved_key)?;
    
    Ok(Json(SelectResponse { success: true }))
}

/// POST /api/session/:session_id/resolution/commit
pub async fn commit_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<CommitResponse>, (StatusCode, String)> {
    let mut sessions = state.session_store.write().await;
    let session = sessions.get_mut(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".into()))?;
    
    let resolution = match &session.sub_session {
        SubSessionType::Resolution(res) => res.clone(),
        _ => return Err((StatusCode::NOT_FOUND, "No active resolution".into())),
    };
    
    // Check all resolved
    if !resolution.is_complete() {
        return Err((StatusCode::BAD_REQUEST, "Not all refs resolved".into()));
    }
    
    // Apply to AST
    resolution.apply_to_ast(&mut session.context.ast)?;
    
    // Clear sub-session
    session.sub_session = SubSessionType::Root;
    
    Ok(Json(CommitResponse { success: true }))
}

pub fn routes() -> Router<ResolutionState> {
    Router::new()
        .route("/api/session/:session_id/resolution/start", post(start_resolution))
        .route("/api/session/:session_id/resolution", get(get_resolution))
        .route("/api/session/:session_id/resolution/search", post(search_resolution))
        .route("/api/session/:session_id/resolution/select", post(select_resolution))
        .route("/api/session/:session_id/resolution/commit", post(commit_resolution))
}
```

### Phase C: Delete ResolutionService (DELETE)

```bash
# Files to DELETE entirely
rm src/services/resolution_service.rs

# In src/services/mod.rs - remove:
# pub mod resolution_service;
# pub use resolution_service::*;
```

### Phase D: Update Imports (CLEANUP)

Any remaining imports of `ResolutionService` or `ResolutionStore` → remove or redirect to session-based approach.

---

## API Contract Preservation

The HTTP API stays the same. Only the implementation changes:

| Endpoint | Before | After |
|----------|--------|-------|
| `POST /resolution/start` | `ResolutionService::start_resolution()` | `ResolutionSubSession::from_ast()` |
| `GET /resolution` | `resolution_store.get()` | `session.sub_session` |
| `POST /resolution/search` | `ResolutionService::search()` | `ResolutionSubSession::search()` |
| `POST /resolution/select` | `ResolutionService::validate_and_select()` | `gateway.validate()` + `ResolutionSubSession::select()` |
| `POST /resolution/commit` | `ResolutionService::commit()` | `ResolutionSubSession::apply_to_ast()` |

---

## Execution Order for Claude Code

1. **FIRST**: Add methods to `ResolutionSubSession` in `session.rs` (~100 lines)
   - `from_ast()` - extract unresolved refs
   - `search()` - gateway search
   - `select()` - store resolution
   - `is_complete()` - check all resolved
   - `apply_to_ast()` - commit to AST
   - `to_response()` - serialize for API

2. **SECOND**: Delete and rewrite `resolution_routes.rs` (~200 lines)
   - New file, clean implementation
   - Uses session state directly
   - Maintains same HTTP API

3. **THIRD**: Delete `resolution_service.rs` (1513 lines deleted!)

4. **FOURTH**: Update `services/mod.rs` - remove exports

5. **FIFTH**: Update any remaining imports (grep for `resolution_service`)

---

## Key Implementation Notes

### Gateway Validation on Select

The old system had `validate_and_select()`. The new system should:

```rust
// In resolution_routes.rs select handler
pub async fn select_resolution(...) {
    // FIRST: Validate against gateway (security!)
    let scope = ResolutionScope::for_cbu(session.cbu_id);
    state.gateway.validate_selection(&body.resolved_key, &body.entity_type, &scope).await?;
    
    // THEN: Store in session
    resolution.select(&body.ref_id, &body.resolved_key)?;
}
```

### Location-Based ref_id

The new system uses span-based ref_id: `"{stmt}:{span.start}-{span.end}"`. This is already defined in TODO-ENTITY-RESOLUTION-CONSOLIDATED.md Fix 4.1.

### Pre-fetch Matches

`ResolutionSubSession::from_ast()` should pre-fetch initial matches for each unresolved ref, just like the old system did. This provides immediate disambiguation options in the UI.

---

## Testing After Refactor

1. [ ] `POST /resolution/start` returns unresolved refs with initial matches
2. [ ] `GET /resolution` returns current state
3. [ ] `POST /resolution/search` returns gateway matches
4. [ ] `POST /resolution/select` validates against gateway, stores selection
5. [ ] `POST /resolution/select` with fake UUID returns 404
6. [ ] `POST /resolution/commit` applies all resolutions to AST
7. [ ] `POST /resolution/commit` with unresolved refs returns 400
8. [ ] Two "John Smith" refs can be resolved to different entities (location-based)

---

## Lines of Code Impact

| Action | Lines |
|--------|-------|
| DELETE `resolution_service.rs` | -1513 |
| ADD to `session.rs` | +100 |
| REWRITE `resolution_routes.rs` | ±200 (net ~same) |
| **NET** | **~-1400** |

Clean win. Less code, single source of truth, correct architecture.
