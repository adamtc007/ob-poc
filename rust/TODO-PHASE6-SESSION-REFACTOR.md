# Phase 6: Session Verbs Refactor + MCP Wiring

## Overview

Simplify session from 20 verbs to 10. Focus/camera moves to client-side viewport.
Session state persisted to DB - survives refresh, resumable next day.

**CRITICAL: Memory is truth, DB is backup. Persistence failure = session lost on refresh. Nothing breaks.**

---

## Performance Model

```
HOT PATH (60fps, sync):              COLD PATH (background, async):
┌────────────────────────┐           ┌────────────────────────┐
│ Session in MEMORY      │           │ DB persistence         │
│                        │           │                        │
│ • load_cbu()     <1µs  │──fire────▶│ • debounced save ~2s   │
│ • unload_cbu()   <1µs  │  and      │ • tokio::spawn         │
│ • undo/redo      <1µs  │  forget   │ • errors logged, ignored│
│ • queries        <1µs  │           │                        │
│                        │◀──────────│ • load on startup only │
│ NEVER BLOCKS RENDER    │  once     │                        │
└────────────────────────┘           └────────────────────────┘
```

**Rules:**
- Mutations are sync, in-memory, instant
- `maybe_save()` spawns background task, never awaited in hot path
- DB errors logged and swallowed - session continues working
- If save fails repeatedly, session lost on refresh - user's problem, not crash
- Load from DB only at startup, with timeout

---

## Database Schema

```sql
-- Session persistence
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID,  -- NULL for anonymous, FK to users if auth exists
    name TEXT,     -- Optional friendly name
    cbu_ids UUID[] NOT NULL DEFAULT '{}',
    history JSONB NOT NULL DEFAULT '[]',   -- Undo stack (array of UUID arrays)
    future JSONB NOT NULL DEFAULT '[]',    -- Redo stack
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '7 days'
);

-- Indexes
CREATE INDEX idx_sessions_user ON sessions(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX idx_sessions_expires ON sessions(expires_at);
CREATE INDEX idx_sessions_updated ON sessions(updated_at DESC);

-- Auto-extend expiry on activity (trigger)
CREATE OR REPLACE FUNCTION extend_session_expiry()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    NEW.expires_at = NOW() + INTERVAL '7 days';
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER session_activity
    BEFORE UPDATE ON sessions
    FOR EACH ROW
    EXECUTE FUNCTION extend_session_expiry();

-- Cleanup job (run via pg_cron or app cron)
-- DELETE FROM sessions WHERE expires_at < NOW();
```

---

## New Session State Model

```rust
/// Session state is just the set of loaded CBUs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    pub cbu_ids: HashSet<Uuid>,
}

/// Session with history
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub state: SessionState,
    history: Vec<SessionState>,  // Undo stack
    future: Vec<SessionState>,   // Redo stack
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            state: SessionState::default(),
            history: Vec::new(),
            future: Vec::new(),
        }
    }
    
    fn push_history(&mut self) {
        self.history.push(self.state.clone());
        self.future.clear();
    }
    
    // === Mutations (push history first) ===
    
    pub fn load_cbu(&mut self, id: Uuid) -> bool {
        if self.state.cbu_ids.contains(&id) {
            return false;
        }
        self.push_history();
        self.state.cbu_ids.insert(id);
        true
    }
    
    pub fn load_many(&mut self, ids: impl IntoIterator<Item = Uuid>) -> usize {
        let new_ids: Vec<Uuid> = ids.into_iter()
            .filter(|id| !self.state.cbu_ids.contains(id))
            .collect();
        if new_ids.is_empty() {
            return 0;
        }
        self.push_history();
        let count = new_ids.len();
        self.state.cbu_ids.extend(new_ids);
        count
    }
    
    pub fn unload_cbu(&mut self, id: Uuid) -> bool {
        if !self.state.cbu_ids.contains(&id) {
            return false;
        }
        self.push_history();
        self.state.cbu_ids.remove(&id);
        true
    }
    
    pub fn clear(&mut self) -> usize {
        if self.state.cbu_ids.is_empty() {
            return 0;
        }
        self.push_history();
        let count = self.state.cbu_ids.len();
        self.state.cbu_ids.clear();
        count
    }
    
    // === History ===
    
    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            self.future.push(self.state.clone());
            self.state = prev;
            true
        } else {
            false
        }
    }
    
    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.future.pop() {
            self.history.push(self.state.clone());
            self.state = next;
            true
        } else {
            false
        }
    }
    
    // === Queries ===
    
    pub fn count(&self) -> usize { self.state.cbu_ids.len() }
    pub fn history_depth(&self) -> usize { self.history.len() }
    pub fn future_depth(&self) -> usize { self.future.len() }
    
    // === Persistence ===
    
    pub async fn load(id: Uuid, pool: &PgPool) -> Result<Option<Self>, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT id, cbu_ids, history, future
            FROM sessions 
            WHERE id = $1 AND expires_at > NOW()
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;
        
        Ok(row.map(|r| Self {
            id: r.id,
            state: SessionState {
                cbu_ids: r.cbu_ids.into_iter().collect(),
            },
            history: serde_json::from_value(r.history).unwrap_or_default(),
            future: serde_json::from_value(r.future).unwrap_or_default(),
        }))
    }
    
    pub async fn save(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        let cbu_ids: Vec<Uuid> = self.state.cbu_ids.iter().copied().collect();
        
        sqlx::query!(
            r#"
            INSERT INTO sessions (id, cbu_ids, history, future)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id) DO UPDATE SET
                cbu_ids = EXCLUDED.cbu_ids,
                history = EXCLUDED.history,
                future = EXCLUDED.future
            "#,
            self.id,
            &cbu_ids,
            serde_json::to_value(&self.history).unwrap(),
            serde_json::to_value(&self.future).unwrap(),
        )
        .execute(pool)
        .await?;
        Ok(())
    }
    
    pub async fn delete(id: Uuid, pool: &PgPool) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM sessions WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
    
    /// List recent sessions for a user (or anonymous)
    pub async fn list_recent(
        user_id: Option<Uuid>,
        limit: i64,
        pool: &PgPool,
    ) -> Result<Vec<SessionSummary>, sqlx::Error> {
        sqlx::query_as!(
            SessionSummary,
            r#"
            SELECT id, name, array_length(cbu_ids, 1) as cbu_count, updated_at
            FROM sessions
            WHERE ($1::uuid IS NULL AND user_id IS NULL) OR user_id = $1
            AND expires_at > NOW()
            ORDER BY updated_at DESC
            LIMIT $2
            "#,
            user_id,
            limit
        )
        .fetch_all(pool)
        .await
    }
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: Uuid,
    pub name: Option<String>,
    pub cbu_count: Option<i32>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

---

## Background Persistence (Fire and Forget)

```rust
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{warn, debug};

pub struct Session {
    // Hot state
    pub id: Uuid,
    pub state: SessionState,
    history: Vec<SessionState>,
    future: Vec<SessionState>,
    
    // Persistence tracking (not persisted itself)
    dirty: bool,
    last_saved: Instant,
}

impl Session {
    /// Call every frame - non-blocking, fire-and-forget
    pub fn maybe_save(&mut self, pool: &PgPool) {
        if !self.dirty {
            return;
        }
        if self.last_saved.elapsed() < Duration::from_secs(2) {
            return;
        }
        
        // Snapshot current state
        let snapshot = SessionSnapshot {
            id: self.id,
            cbu_ids: self.state.cbu_ids.iter().copied().collect(),
            history: self.history.clone(),
            future: self.future.clone(),
        };
        
        let pool = pool.clone();
        
        // Fire and forget - NEVER await this
        tokio::spawn(async move {
            match snapshot.persist(&pool).await {
                Ok(_) => debug!("Session {} saved", snapshot.id),
                Err(e) => warn!("Session save failed (non-fatal): {}", e),
                // ^ Logged and swallowed. Session keeps working.
            }
        });
        
        self.dirty = false;
        self.last_saved = Instant::now();
    }
    
    /// Load from DB at startup - with timeout, fallback to empty
    pub async fn load_or_new(id: Option<Uuid>, pool: &PgPool) -> Self {
        if let Some(id) = id {
            match tokio::time::timeout(
                Duration::from_secs(2),
                Self::load_from_db(id, pool)
            ).await {
                Ok(Ok(Some(session))) => {
                    debug!("Session {} loaded from DB", id);
                    return session;
                }
                Ok(Ok(None)) => debug!("Session {} not found, creating new", id),
                Ok(Err(e)) => warn!("Session load failed (non-fatal): {}", e),
                Err(_) => warn!("Session load timed out (non-fatal)"),
            }
        }
        
        // Fallback: fresh session
        Self::new()
    }
}

#[derive(Clone)]
struct SessionSnapshot {
    id: Uuid,
    cbu_ids: Vec<Uuid>,
    history: Vec<SessionState>,
    future: Vec<SessionState>,
}

impl SessionSnapshot {
    async fn persist(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            INSERT INTO sessions (id, cbu_ids, history, future)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id) DO UPDATE SET
                cbu_ids = EXCLUDED.cbu_ids,
                history = EXCLUDED.history,
                future = EXCLUDED.future
            "#,
            self.id,
            &self.cbu_ids,
            serde_json::to_value(&self.history).unwrap_or_default(),
            serde_json::to_value(&self.future).unwrap_or_default(),
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}
```

**App integration:**

```rust
// In main update loop (60fps)
fn update(&mut self, dt: f32) {
    // ... handle input, mutations (all sync, instant)
    
    // Background save - non-blocking
    self.session.maybe_save(&self.pool);
    
    // ... camera, force sim, render
}

// On startup
async fn init(session_id: Option<Uuid>, pool: &PgPool) -> App {
    let session = Session::load_or_new(session_id, pool).await;
    // ...
}
```

**Failure modes (all graceful):**

| Failure | Result | User Impact |
|---------|--------|-------------|
| DB down | Saves silently fail | Session lost on refresh |
| Timeout | Load falls back to new | Start fresh, no crash |
| Corrupt data | Load falls back to new | Start fresh, no crash |
| Network blip | Retry next save cycle | Likely recovers |

---

## Handler Implementations

### Location: `rust/src/ops/session_ops.rs` (NEW or replace existing)

```rust
use crate::session::Session;
use sqlx::PgPool;
use uuid::Uuid;

pub struct SessionLoadCbuOp;
pub struct SessionLoadJurisdictionOp;
pub struct SessionLoadGalaxyOp;
pub struct SessionUnloadCbuOp;
pub struct SessionClearOp;
pub struct SessionUndoOp;
pub struct SessionRedoOp;
pub struct SessionInfoOp;
pub struct SessionListOp;

impl SessionLoadCbuOp {
    pub async fn execute(
        session: &mut Session,
        cbu_id: Uuid,
        pool: &PgPool,
    ) -> Result<LoadCbuResult, OpError> {
        // Verify CBU exists
        let cbu = sqlx::query_as!(
            CbuRow,
            "SELECT cbu_id, name, jurisdiction FROM cbus WHERE cbu_id = $1",
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or(OpError::NotFound("CBU not found"))?;
        
        let added = session.load_cbu(cbu_id);
        
        Ok(LoadCbuResult {
            cbu_id,
            name: cbu.name,
            jurisdiction: cbu.jurisdiction,
            total_loaded: session.count(),
            was_new: added,
        })
    }
}

impl SessionLoadJurisdictionOp {
    pub async fn execute(
        session: &mut Session,
        jurisdiction: &str,
        pool: &PgPool,
    ) -> Result<LoadJurisdictionResult, OpError> {
        let cbu_ids: Vec<Uuid> = sqlx::query_scalar!(
            "SELECT cbu_id FROM cbus WHERE jurisdiction = $1",
            jurisdiction
        )
        .fetch_all(pool)
        .await?;
        
        let count_added = session.load_many(cbu_ids);
        
        Ok(LoadJurisdictionResult {
            jurisdiction: jurisdiction.to_string(),
            count_added,
            total_loaded: session.count(),
        })
    }
}

impl SessionLoadGalaxyOp {
    pub async fn execute(
        session: &mut Session,
        apex_entity_id: Uuid,
        pool: &PgPool,
    ) -> Result<LoadGalaxyResult, OpError> {
        // Get apex name
        let apex_name: String = sqlx::query_scalar!(
            "SELECT name FROM entities WHERE entity_id = $1",
            apex_entity_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or(OpError::NotFound("Apex entity not found"))?;
        
        // Find all CBUs under this apex via group edges
        let cbu_ids: Vec<Uuid> = sqlx::query_scalar!(
            r#"
            WITH RECURSIVE group_tree AS (
                SELECT entity_id FROM entities WHERE entity_id = $1
                UNION ALL
                SELECT ge.child_entity_id
                FROM group_edges ge
                JOIN group_tree gt ON ge.parent_entity_id = gt.entity_id
            )
            SELECT c.cbu_id 
            FROM cbus c
            JOIN group_tree gt ON c.legal_entity_id = gt.entity_id
            "#,
            apex_entity_id
        )
        .fetch_all(pool)
        .await?;
        
        let count_added = session.load_many(cbu_ids);
        
        Ok(LoadGalaxyResult {
            apex_name,
            count_added,
            total_loaded: session.count(),
        })
    }
}

impl SessionUnloadCbuOp {
    pub async fn execute(
        session: &mut Session,
        cbu_id: Uuid,
        pool: &PgPool,
    ) -> Result<UnloadCbuResult, OpError> {
        let name: Option<String> = sqlx::query_scalar!(
            "SELECT name FROM cbus WHERE cbu_id = $1",
            cbu_id
        )
        .fetch_optional(pool)
        .await?;
        
        let removed = session.unload_cbu(cbu_id);
        
        Ok(UnloadCbuResult {
            cbu_id,
            name: name.unwrap_or_default(),
            total_loaded: session.count(),
            was_present: removed,
        })
    }
}

impl SessionClearOp {
    pub fn execute(session: &mut Session) -> ClearResult {
        let count_removed = session.clear();
        ClearResult { count_removed }
    }
}

impl SessionUndoOp {
    pub fn execute(session: &mut Session) -> HistoryResult {
        let success = session.undo();
        HistoryResult {
            success,
            total_loaded: session.count(),
            history_depth: session.history_depth(),
            future_depth: session.future_depth(),
        }
    }
}

impl SessionRedoOp {
    pub fn execute(session: &mut Session) -> HistoryResult {
        let success = session.redo();
        HistoryResult {
            success,
            total_loaded: session.count(),
            history_depth: session.history_depth(),
            future_depth: session.future_depth(),
        }
    }
}

impl SessionInfoOp {
    pub async fn execute(
        session: &Session,
        pool: &PgPool,
    ) -> Result<SessionInfo, OpError> {
        // Get jurisdiction breakdown
        let jurisdictions: Vec<JurisdictionCount> = sqlx::query_as!(
            JurisdictionCount,
            r#"
            SELECT jurisdiction, COUNT(*) as count
            FROM cbus
            WHERE cbu_id = ANY($1)
            GROUP BY jurisdiction
            ORDER BY count DESC
            "#,
            &session.state.cbu_ids.iter().copied().collect::<Vec<_>>()
        )
        .fetch_all(pool)
        .await?;
        
        Ok(SessionInfo {
            total_cbus: session.count(),
            jurisdictions,
            history_depth: session.history_depth(),
            future_depth: session.future_depth(),
        })
    }
}

impl SessionListOp {
    pub async fn execute(
        session: &Session,
        limit: usize,
        jurisdiction_filter: Option<&str>,
        pool: &PgPool,
    ) -> Result<Vec<CbuSummary>, OpError> {
        let ids: Vec<Uuid> = session.state.cbu_ids.iter().copied().collect();
        
        let rows = sqlx::query_as!(
            CbuSummary,
            r#"
            SELECT cbu_id, name, jurisdiction
            FROM cbus
            WHERE cbu_id = ANY($1)
            AND ($2::text IS NULL OR jurisdiction = $2)
            ORDER BY name
            LIMIT $3
            "#,
            &ids,
            jurisdiction_filter,
            limit as i64
        )
        .fetch_all(pool)
        .await?;
        
        Ok(rows)
    }
}
```

---

## MCP Tool Registration

### Location: `rust/src/api/mcp_tools.rs` (update)

```rust
use crate::ops::session_ops::*;

pub fn register_session_tools(registry: &mut ToolRegistry) {
    registry.register("session/load-cbu", |params, ctx| async move {
        let cbu_id: Uuid = params.get("cbu-id")?.parse()?;
        let result = SessionLoadCbuOp::execute(
            &mut ctx.session,
            cbu_id,
            &ctx.pool,
        ).await?;
        Ok(json!(result))
    });
    
    registry.register("session/load-jurisdiction", |params, ctx| async move {
        let jurisdiction: String = params.get("jurisdiction")?;
        let result = SessionLoadJurisdictionOp::execute(
            &mut ctx.session,
            &jurisdiction,
            &ctx.pool,
        ).await?;
        Ok(json!(result))
    });
    
    registry.register("session/load-galaxy", |params, ctx| async move {
        let apex_id: Uuid = params.get("apex-entity-id")?.parse()?;
        let result = SessionLoadGalaxyOp::execute(
            &mut ctx.session,
            apex_id,
            &ctx.pool,
        ).await?;
        Ok(json!(result))
    });
    
    registry.register("session/unload-cbu", |params, ctx| async move {
        let cbu_id: Uuid = params.get("cbu-id")?.parse()?;
        let result = SessionUnloadCbuOp::execute(
            &mut ctx.session,
            cbu_id,
            &ctx.pool,
        ).await?;
        Ok(json!(result))
    });
    
    registry.register("session/clear", |_params, ctx| async move {
        let result = SessionClearOp::execute(&mut ctx.session);
        Ok(json!(result))
    });
    
    registry.register("session/undo", |_params, ctx| async move {
        let result = SessionUndoOp::execute(&mut ctx.session);
        Ok(json!(result))
    });
    
    registry.register("session/redo", |_params, ctx| async move {
        let result = SessionRedoOp::execute(&mut ctx.session);
        Ok(json!(result))
    });
    
    registry.register("session/info", |_params, ctx| async move {
        let result = SessionInfoOp::execute(&ctx.session, &ctx.pool).await?;
        Ok(json!(result))
    });
    
    registry.register("session/list", |params, ctx| async move {
        let limit: usize = params.get("limit").unwrap_or(100);
        let jur: Option<String> = params.get("jurisdiction").ok();
        let result = SessionListOp::execute(
            &ctx.session,
            limit,
            jur.as_deref(),
            &ctx.pool,
        ).await?;
        Ok(json!(result))
    });
}
```

---

## Agent Service Integration

### Location: `rust/src/api/agent_service.rs` (update)

Wire session verbs to agent command handling:

```rust
// In handle_show_command or similar
fn handle_session_command(&mut self, message: &str) -> Option<AgentChatResponse> {
    let lower = message.to_lowercase();
    
    // "load allianz lux" → load-cbu
    if lower.starts_with("load ") {
        let query = message[5..].trim();
        // Resolve CBU, then call session.load_cbu()
        return Some(self.execute_load_cbu(query));
    }
    
    // "load jurisdiction lu" → load-jurisdiction
    if lower.starts_with("load jurisdiction ") {
        let jur = message[18..].trim().to_uppercase();
        return Some(self.execute_load_jurisdiction(&jur));
    }
    
    // "undo" / "redo"
    if lower == "undo" {
        return Some(self.execute_undo());
    }
    if lower == "redo" {
        return Some(self.execute_redo());
    }
    
    // "clear session"
    if lower == "clear" || lower == "clear session" {
        return Some(self.execute_clear());
    }
    
    // "session info" / "what's loaded"
    if lower == "session info" || lower.contains("what's loaded") {
        return Some(self.execute_session_info());
    }
    
    None
}
```

---

## Files to Delete

Old handlers that are replaced:

```
rust/src/ops/session_set_galaxy_op.rs
rust/src/ops/session_set_book_op.rs
rust/src/ops/session_set_cbu_op.rs
rust/src/ops/session_set_jurisdiction_op.rs
rust/src/ops/session_set_neighborhood_op.rs
rust/src/ops/session_focus_op.rs
rust/src/ops/session_clear_focus_op.rs
rust/src/ops/session_back_op.rs
rust/src/ops/session_forward_op.rs
rust/src/ops/session_add_cbu_op.rs
rust/src/ops/session_remove_cbu_op.rs
rust/src/ops/session_clear_cbu_set_op.rs
rust/src/ops/session_list_active_cbus_op.rs
rust/src/ops/session_save_bookmark_op.rs
rust/src/ops/session_load_bookmark_op.rs
rust/src/ops/session_list_bookmarks_op.rs
rust/src/ops/session_delete_bookmark_op.rs
```

---

## Migration Checklist

### Step 0: Database Migration (30m)
- [ ] Create migration `migrations/XXXX_sessions_table.sql`
- [ ] Add `sessions` table with cbu_ids, history, future arrays
- [ ] Add expiry trigger
- [ ] Run migration: `sqlx migrate run`

### Step 1: New Session Model (1h)
- [ ] Create `rust/src/session/mod.rs` with `Session`, `SessionState`
- [ ] Implement load/unload/clear/undo/redo (in-memory)
- [ ] Implement `Session::load()`, `Session::save()`, `Session::delete()`
- [ ] Unit tests for history stack

### Step 2: New Handlers (2h)
- [ ] Create `rust/src/ops/session_ops.rs`
- [ ] Implement all 9 ops
- [ ] Call `session.save()` after mutations (debounced)
- [ ] Wire to database queries

### Step 3: Verb YAML Swap (30m)
- [ ] Backup `config/verbs/session.yaml`
- [ ] Rename `session_v2.yaml` → `session.yaml`
- [ ] Delete old `session.yaml`

### Step 4: MCP Registration (1h)
- [ ] Update `mcp_tools.rs` with new registrations
- [ ] Remove old tool registrations
- [ ] Test via MCP client

### Step 5: Agent Wiring (1h)
- [ ] Update `agent_service.rs` command handlers
- [ ] Natural language → verb mapping
- [ ] Test "load allianz lux", "undo", "session info"

### Step 6: Session Resume API (1h)
- [ ] GET `/api/session/:id` - load existing session
- [ ] POST `/api/session` - create new session
- [ ] GET `/api/sessions` - list recent sessions
- [ ] DELETE `/api/session/:id` - delete session
- [ ] Session ID in URL for bookmarkable links

### Step 7: Auto-Save (1h)
- [ ] Debounced save after mutations (~1s delay)
- [ ] Save on window beforeunload (client sends beacon)
- [ ] Handle concurrent saves gracefully

### Step 8: Cleanup (30m)
- [ ] Delete old handler files
- [ ] Remove dead imports
- [ ] Cargo build clean

---

## Success Criteria

- [ ] `load cbu "allianz lux"` → adds to session, viewport updates
- [ ] `load jurisdiction lu` → adds 47 CBUs
- [ ] `undo` → removes them
- [ ] `redo` → restores them
- [ ] `session info` → shows count, jurisdictions
- [ ] `clear` → empties session
- [ ] Viewport auto-fits to loaded content
- [ ] No old session handlers remain
- [ ] 10 verbs, not 20
- [ ] **Browser refresh → session restored**
- [ ] **Close tab, come back tomorrow → session still there**
- [ ] **Session expires after 7 days of inactivity**
- [ ] **`/session/abc-123` URL is bookmarkable**

---

## Total Effort: ~8h
