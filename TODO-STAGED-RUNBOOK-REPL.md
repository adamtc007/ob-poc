# TODO — Staged Runbook REPL + Semantic Resolver + DAG Reorder (MCP-first, Anti-Hallucination)

> **Status:** Peer reviewed ✅ — Ready for implementation  
> **Date:** 2026-01-25  
> **Depends on:** Client Group Scope Resolution ✅ IMPLEMENTED (migrations 052-053)  
> **Reviewed by:** ChatGPT (2026-01-25) → then Claude Code implementation

---

## Peer Review Notes (ChatGPT, 2026-01-25)

### Critical Additions Applied

| Gap Identified | Fix Applied |
|----------------|-------------|
| **Agent Router Contract missing** | Added Section 1.5: mandatory tool usage, default stage path, run/edit/show/abort paths |
| **Candle integration vague** | Added Section 1.6: `IntentQuery` and `IntentResponse` schemas with scoped context |
| **No parse validation** | Added `ParseFailed` status + `StageFailed` event + server-side validation in `runbook_stage` |
| **Picker could accept invented UUIDs** | Added candidate validation in `runbook_pick` — must match `ResolutionAmbiguous` event |
| **Run gating client-side only** | Added server-side `RunbookNotReady` event + validation in `runbook_run` |
| **output_ref redundant** | Removed from schema; $N refs parsed from `dsl_raw` during DAG analysis |
| **session_id unclear** | Added note: "stable MCP conversation key" |

### What Was Excellent (kept as-is)

1. **Non-negotiable invariants** — no side-effects without explicit run
2. **Entity footprint visibility** — shows what will be touched before execution
3. **DAG reordering** — transparent diff shown to user
4. **Learning flywheel** — successful resolutions create user_confirmed tags

---

## 0) Why this exists (the gap)

Today the agent can hallucinate because it has no deterministic bridge from:
- natural language → scoped entities/CBUs → concrete UUIDs → valid DSL → safe execution order

This TODO adds a **server-side "mini runbook"** that stages DSL (no execution), resolves entity arguments into UUIDs deterministically, reorders via DAG, shows an entity footprint in MCP/egui, and executes only on explicit `run/commit`.

---

## 1) Non-negotiable invariants (anti-hallucination)

1. **No side-effects unless user explicitly says** `run/execute/commit`.
2. **No invented UUIDs**. All UUIDs must come from DB/tool resolution.
3. Staged DSL must be **canonical** before execute:
   - required entity/CBU args are UUID forms
   - no ambiguous/unresolved refs
4. DAG may **reorder** staged source. Reorder must be transparent (diff shown).
5. **No auto-insert prerequisites**. Only propose patches; user must confirm.
6. MCP must use tools (DB-backed search/resolution) for pickers and footprint; never "guess".
7. **Agent must call tools** — never answer with invented actions or prose that implies execution.
8. **Picker entity_ids must come from events** — agent cannot fabricate UUIDs for `runbook_pick`.

---

## 1.5) Agent Router Contract (CRITICAL — must use MCP tools)

This is the keystone that prevents "agent skips tools and answers in prose".

### Control Loop

For **every** user message, the agent must take exactly ONE of these paths:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  AGENT ROUTER — MANDATORY TOOL USAGE                                        │
│                                                                             │
│  User message arrives                                                       │
│         │                                                                   │
│         ▼                                                                   │
│  ┌─────────────────────────────────────────────────────────────────┐       │
│  │  1. CLASSIFY INTENT (via Candle)                                │       │
│  │                                                                 │       │
│  │  Candle returns: intent_type + dsl_draft (if applicable)       │       │
│  └─────────────────────────────────────────────────────────────────┘       │
│         │                                                                   │
│         ├── intent_type = stage_command (DEFAULT)                          │
│         │         │                                                         │
│         │         ▼                                                         │
│         │   call runbook_stage(dsl, description)                           │
│         │   server resolves/rewrites/DAG-analyzes                          │
│         │   server emits events (CommandStaged/Ambiguous/Failed)           │
│         │   agent replies with SHORT human summary (no execution)          │
│         │                                                                   │
│         ├── intent_type = run_runbook                                      │
│         │         │                                                         │
│         │         ▼                                                         │
│         │   call runbook_run()                                             │
│         │   server validates readiness (rejects if not ready)              │
│         │   server emits per-command events                                │
│         │   agent summarizes results                                       │
│         │                                                                   │
│         ├── intent_type = edit_runbook (remove/edit/pick)                  │
│         │         │                                                         │
│         │         ▼                                                         │
│         │   call runbook_remove / runbook_edit / runbook_pick              │
│         │   server emits updated state                                     │
│         │   agent confirms edit                                            │
│         │                                                                   │
│         ├── intent_type = show_runbook                                     │
│         │         │                                                         │
│         │         ▼                                                         │
│         │   call runbook_show()                                            │
│         │   agent renders current state                                    │
│         │                                                                   │
│         └── intent_type = abort_runbook                                    │
│                   │                                                         │
│                   ▼                                                         │
│             call runbook_abort()                                           │
│             agent confirms cleared                                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Default Stage Mode (CRITICAL)

**Every non-run / non-edit prompt defaults to "stage a new command".**

The agent does NOT:
- Wait for user to type "stage ..."
- Rely on invocation phrases like "then", "and"
- Answer in prose with invented actions

The agent DOES:
- Treat every actionable prompt as "stage DSL via tool"
- Use Candle to produce DSL draft
- Call `runbook_stage` immediately
- Summarize what was staged (not executed)

### Run Intent Detection

User must explicitly signal run intent:
- "run", "execute", "do it", "go", "commit"
- "run the plan", "execute all", "let's do this"

Anything else → stage path (default).

---

## 1.6) Candle Query/Response Schema

Candle's job is **upstream**: choosing verbs and args before staging.

### Intent Query (agent → Candle)

```rust
/// Scoped query object passed to Candle for intent resolution
#[derive(Debug, Serialize, Deserialize)]
pub struct IntentQuery {
    /// Current client group context (required for entity resolution)
    pub client_group_id: Option<Uuid>,
    
    /// Current persona (affects tag filtering)
    pub persona: Option<String>,
    
    /// MCP session identifier (stable conversation key)
    pub session_id: String,
    
    /// Current runbook state (so Candle can choose "new line" vs "edit")
    pub runbook_summary: Option<RunbookSummary>,
    
    /// The user's natural language prompt
    pub user_prompt: String,
}
```

### Intent Response (Candle → agent)

```rust
/// Candle's classification of user intent
#[derive(Debug, Serialize, Deserialize)]
pub struct IntentResponse {
    /// What kind of action the user wants
    pub intent_type: IntentType,
    
    /// Draft DSL if intent_type = StageCommand
    pub dsl_draft: Option<String>,
    
    /// Human-readable description of what this does
    pub description: Option<String>,
    
    /// Confidence in this classification (0.0-1.0)
    pub confidence: f32,
    
    /// If edit: which line to edit
    pub edit_target: Option<i32>,
    
    /// If pick: which command needs resolution
    pub pick_target: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentType {
    /// Stage a new DSL command (DEFAULT)
    StageCommand,
    
    /// Execute the runbook
    RunRunbook,
    
    /// Edit staged commands (remove/modify)
    EditRunbook,
    
    /// Select from ambiguous candidates
    PickCandidates,
    
    /// Show current runbook state
    ShowRunbook,
    
    /// Clear all staged commands
    AbortRunbook,
    
    /// Not an actionable request (informational only)
    Informational,
}
```

### Candle Flow

```
User: "Show me the Irish funds then check their KYC"
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────┐
│  CANDLE INTENT RESOLUTION                                       │
│                                                                 │
│  Input: IntentQuery {                                          │
│    client_group_id: "11111111-...",                            │
│    persona: "kyc",                                             │
│    runbook_summary: { command_count: 0, ... },                 │
│    user_prompt: "Show me the Irish funds then check KYC"       │
│  }                                                             │
│                                                                 │
│  Output: IntentResponse {                                      │
│    intent_type: StageCommand,                                  │
│    dsl_draft: "entity.list entity-ids=\"Irish funds\"",       │
│    description: "List Irish fund entities",                   │
│    confidence: 0.92                                            │
│  }                                                             │
│                                                                 │
│  NOTE: "then check KYC" will be a SECOND stage after first    │
│  resolves, or user can add it explicitly.                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## 1.7) Server-side Safety Gates

### runbook_stage: Parse validation

```rust
/// Stage a DSL command — validates syntax before accepting
pub async fn runbook_stage(
    dsl: &str,
    description: Option<&str>,
    ctx: &SessionContext,
) -> Result<StageResult, StageError> {
    // 1. PARSE DSL (must succeed before staging)
    let parsed = match parse_dsl(dsl) {
        Ok(p) => p,
        Err(e) => {
            // Emit diagnostic event
            ctx.emit(RunbookEvent::StageFailed {
                runbook_id: ctx.runbook_id(),
                error_kind: "parse_failed".to_string(),
                error: e.to_string(),
                dsl_raw: dsl.to_string(),
            });
            return Err(StageError::ParseFailed(e));
        }
    };
    
    // 2. Resolve entity arguments
    // 3. Insert command with appropriate status
    // ...
}
```

**Resolution status now includes `ParseFailed`:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    Pending,      // not yet resolved
    Resolved,     // all refs → UUIDs
    Ambiguous,    // needs picker
    Failed,       // resolution error (no matches)
    ParseFailed,  // DSL syntax error
}
```

### runbook_run: Readiness gate

```rust
/// Execute runbook — validates ALL commands resolved first
pub async fn runbook_run(ctx: &SessionContext) -> Result<ExecutionId, RunError> {
    let runbook = ctx.get_runbook().await?;
    
    // SERVER-SIDE GATE: reject if not ready
    let blocking: Vec<_> = runbook.commands.iter()
        .filter(|c| c.resolution_status != ResolutionStatus::Resolved)
        .collect();
    
    if !blocking.is_empty() {
        ctx.emit(RunbookEvent::RunbookNotReady {
            runbook_id: runbook.id,
            blocking_commands: blocking.iter().map(|c| BlockingCommand {
                command_id: c.id,
                source_order: c.source_order,
                status: c.resolution_status,
                error: c.resolution_error.clone(),
            }).collect(),
        });
        return Err(RunError::NotReady(blocking.len()));
    }
    
    // Proceed with execution...
}
```

**Do NOT rely on client-side button disabling.**

### runbook_pick: Candidate validation

```rust
/// Pick entities — validates entity_ids came from event candidates
pub async fn runbook_pick(
    command_id: Uuid,
    entity_ids: Vec<Uuid>,
    ctx: &SessionContext,
) -> Result<PickResult, PickError> {
    let command = ctx.get_command(command_id).await?;
    
    // SAFETY: entity_ids must be subset of candidates from ResolutionAmbiguous event
    let valid_candidates: HashSet<_> = command.entity_footprint.iter()
        .map(|e| e.entity_id)
        .collect();
    
    for id in &entity_ids {
        if !valid_candidates.contains(id) {
            return Err(PickError::InvalidCandidate {
                entity_id: *id,
                message: "Entity ID not in candidates from ResolutionAmbiguous event".to_string(),
            });
        }
    }
    
    // Proceed with pick...
}
```

**Agent cannot fabricate entity_ids for picker.**

---

## 2) Schema conventions (match repo)

Your repo uses quoted schema names like `"ob-poc"` and `"kyc"` (dash requires quoting).  
**All SQL below uses `"ob-poc"`**. Do not create `ob_poc` accidentally.

Also: `gen_random_uuid()` requires pgcrypto. If not already enabled earlier, add once:

```sql
CREATE EXTENSION IF NOT EXISTS pgcrypto;
```

---

## 3) Session-scoped staged runbook (server state)

### 3.1 Database table

```sql
-- Migration: 054_staged_runbook.sql

BEGIN;

-- ============================================================================
-- Staged Runbook: Accumulated DSL commands awaiting execution
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".staged_runbook (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Session binding
    -- NOTE: session_id is the stable MCP conversation key.
    -- If your agent layer distinguishes threads/channels, add thread_id here.
    session_id TEXT NOT NULL,
    
    -- Context (copied from session at creation time)
    client_group_id UUID REFERENCES "ob-poc".client_group(id),
    persona TEXT,
    
    -- State
    status TEXT NOT NULL DEFAULT 'building',  -- 'building' | 'ready' | 'executing' | 'completed' | 'aborted'
    
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_sr_session ON "ob-poc".staged_runbook(session_id);
CREATE INDEX idx_sr_status ON "ob-poc".staged_runbook(status) WHERE status = 'building';

-- ============================================================================
-- Staged Commands: Individual DSL lines in the runbook
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".staged_command (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    runbook_id UUID NOT NULL REFERENCES "ob-poc".staged_runbook(id) ON DELETE CASCADE,
    
    -- Ordering
    source_order INT NOT NULL,              -- user's original insertion order
    dag_order INT,                          -- computed execution order (NULL until ready)
    
    -- The DSL
    dsl_raw TEXT NOT NULL,                  -- as user/agent provided (may have shorthand)
    dsl_resolved TEXT,                      -- with UUIDs substituted (NULL until resolved)
    
    -- Metadata
    verb TEXT NOT NULL,                     -- parsed verb (e.g., 'entity.list')
    description TEXT,                       -- human-readable summary
    source_prompt TEXT,                     -- original user utterance
    
    -- Resolution state
    resolution_status TEXT NOT NULL DEFAULT 'pending',  
        -- 'pending': not yet resolved
        -- 'resolved': all refs → UUIDs
        -- 'ambiguous': needs picker
        -- 'failed': resolution error (no matches)
        -- 'parse_failed': DSL syntax error
    resolution_error TEXT,                  -- if failed/parse_failed
    
    -- DAG edges (populated during analysis)
    -- NOTE: $N references are parsed from dsl_raw; no separate output_ref column needed
    depends_on UUID[],                      -- command IDs this depends on
    
    created_at TIMESTAMPTZ DEFAULT now(),
    
    UNIQUE(runbook_id, source_order)
);

CREATE INDEX idx_sc_runbook ON "ob-poc".staged_command(runbook_id);
CREATE INDEX idx_sc_status ON "ob-poc".staged_command(resolution_status);

-- ============================================================================
-- Resolved Entities: Entity footprint for the runbook
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".staged_command_entity (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    command_id UUID NOT NULL REFERENCES "ob-poc".staged_command(id) ON DELETE CASCADE,
    
    -- The resolved entity
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- How it got there
    arg_name TEXT NOT NULL,                 -- which DSL argument (e.g., 'entity-id', 'entity-ids')
    resolution_source TEXT NOT NULL,        -- 'tag_exact' | 'tag_fuzzy' | 'tag_semantic' | 'direct_uuid' | 'picker'
    original_ref TEXT,                      -- what user said (e.g., "Irish funds")
    confidence FLOAT,                       -- for fuzzy/semantic matches
    
    UNIQUE(command_id, entity_id, arg_name)
);

CREATE INDEX idx_sce_command ON "ob-poc".staged_command_entity(command_id);
CREATE INDEX idx_sce_entity ON "ob-poc".staged_command_entity(entity_id);

-- ============================================================================
-- View: Full runbook with resolved DSL
-- ============================================================================
CREATE OR REPLACE VIEW "ob-poc".v_staged_runbook AS
SELECT
    sr.id AS runbook_id,
    sr.session_id,
    sr.client_group_id,
    sr.persona,
    sr.status AS runbook_status,
    sc.id AS command_id,
    sc.source_order,
    sc.dag_order,
    sc.dsl_raw,
    sc.dsl_resolved,
    sc.verb,
    sc.description,
    sc.resolution_status,
    sc.depends_on,
    -- Entity footprint as JSON array
    (
        SELECT jsonb_agg(jsonb_build_object(
            'entity_id', sce.entity_id,
            'entity_name', e.name,
            'arg_name', sce.arg_name,
            'source', sce.resolution_source,
            'original_ref', sce.original_ref
        ))
        FROM "ob-poc".staged_command_entity sce
        JOIN "ob-poc".entities e ON e.entity_id = sce.entity_id
        WHERE sce.command_id = sc.id
    ) AS entity_footprint
FROM "ob-poc".staged_runbook sr
JOIN "ob-poc".staged_command sc ON sc.runbook_id = sr.id
ORDER BY sr.id, COALESCE(sc.dag_order, sc.source_order);

COMMIT;
```

### 3.2 Rust types

```rust
// rust/src/repl/staged_runbook.rs

use uuid::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedRunbook {
    pub id: Uuid,
    pub session_id: String,
    pub client_group_id: Option<Uuid>,
    pub persona: Option<String>,
    pub status: RunbookStatus,
    pub commands: Vec<StagedCommand>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunbookStatus {
    Building,    // accepting new commands
    Ready,       // all resolved, DAG computed, awaiting execute
    Executing,   // currently running
    Completed,   // finished successfully
    Aborted,     // user cancelled
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedCommand {
    pub id: Uuid,
    pub source_order: i32,
    pub dag_order: Option<i32>,
    
    pub dsl_raw: String,
    pub dsl_resolved: Option<String>,
    
    pub verb: String,
    pub description: Option<String>,
    pub source_prompt: Option<String>,
    
    pub resolution_status: ResolutionStatus,
    pub resolution_error: Option<String>,
    
    /// Command IDs this depends on (computed from $N references in DSL)
    pub depends_on: Vec<Uuid>,
    
    pub entity_footprint: Vec<ResolvedEntity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    Pending,      // not yet resolved
    Resolved,     // all refs → UUIDs
    Ambiguous,    // needs picker
    Failed,       // resolution error (no matches)
    ParseFailed,  // DSL syntax error
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedEntity {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub arg_name: String,
    pub resolution_source: ResolutionSource,
    pub original_ref: String,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionSource {
    TagExact,
    TagFuzzy,
    TagSemantic,
    DirectUuid,
    Picker,       // user selected from ambiguous list
    OutputRef,    // from previous command's output ($1.result)
}
```

---

## 4) Resolution pipeline (shorthand → UUID)

### 4.1 Resolution flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  RESOLUTION PIPELINE                                                        │
│                                                                             │
│  DSL with shorthand:                                                       │
│    entity.list entity-ids="Irish funds"                                    │
│                     │                                                       │
│                     ▼                                                       │
│  ┌─────────────────────────────────────┐                                   │
│  │  1. PARSE: Extract entity args      │                                   │
│  │     arg_name: "entity-ids"          │                                   │
│  │     raw_value: "Irish funds"        │                                   │
│  └─────────────────────────────────────┘                                   │
│                     │                                                       │
│                     ▼                                                       │
│  ┌─────────────────────────────────────┐                                   │
│  │  2. DETECT: Is it already UUID?     │                                   │
│  │     - Yes → mark DirectUuid         │                                   │
│  │     - No → continue to search       │                                   │
│  └─────────────────────────────────────┘                                   │
│                     │                                                       │
│                     ▼                                                       │
│  ┌─────────────────────────────────────┐                                   │
│  │  3. SEARCH: Use client_group scope  │  ← Uses existing tag search      │
│  │     - Exact tag match               │    functions from 047/052        │
│  │     - Fuzzy trigram                 │                                   │
│  │     - Semantic embedding            │                                   │
│  └─────────────────────────────────────┘                                   │
│                     │                                                       │
│                     ▼                                                       │
│  ┌─────────────────────────────────────┐                                   │
│  │  4. EVALUATE: How many matches?     │                                   │
│  │     - 0 → Failed                    │                                   │
│  │     - 1 → Resolved                  │                                   │
│  │     - 2+ high-conf → Resolved       │                                   │
│  │     - 2+ mixed-conf → Ambiguous     │                                   │
│  └─────────────────────────────────────┘                                   │
│                     │                                                       │
│                     ▼                                                       │
│  ┌─────────────────────────────────────┐                                   │
│  │  5. SUBSTITUTE: Build dsl_resolved  │                                   │
│  │     entity.list entity-ids=[        │                                   │
│  │       uuid1, uuid2, uuid3           │                                   │
│  │     ]                               │                                   │
│  └─────────────────────────────────────┘                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Resolver implementation

```rust
// rust/src/repl/resolver.rs

use sqlx::PgPool;
use uuid::Uuid;
use anyhow::Result;

pub struct EntityArgResolver<'a> {
    pool: &'a PgPool,
    client_group_id: Uuid,
    persona: Option<&'a str>,
    embedder: Option<&'a CandleEmbedder>,
}

#[derive(Debug)]
pub struct ResolutionResult {
    pub status: ResolutionStatus,
    pub entities: Vec<ResolvedEntity>,
    pub error: Option<String>,
}

impl<'a> EntityArgResolver<'a> {
    pub fn new(pool: &'a PgPool, client_group_id: Uuid) -> Self {
        Self {
            pool,
            client_group_id,
            persona: None,
            embedder: None,
        }
    }
    
    pub fn with_persona(mut self, persona: &'a str) -> Self {
        self.persona = Some(persona);
        self
    }
    
    pub fn with_embedder(mut self, embedder: &'a CandleEmbedder) -> Self {
        self.embedder = Some(embedder);
        self
    }
    
    /// Resolve a raw argument value to entity UUIDs
    pub async fn resolve(
        &self,
        arg_name: &str,
        raw_value: &str,
    ) -> Result<ResolutionResult> {
        // 1. Check if already UUID(s)
        if let Some(uuids) = self.parse_uuid_list(raw_value) {
            return Ok(ResolutionResult {
                status: ResolutionStatus::Resolved,
                entities: uuids.into_iter().map(|id| ResolvedEntity {
                    entity_id: id,
                    entity_name: String::new(), // fetch later
                    arg_name: arg_name.to_string(),
                    resolution_source: ResolutionSource::DirectUuid,
                    original_ref: raw_value.to_string(),
                    confidence: Some(1.0),
                }).collect(),
                error: None,
            });
        }
        
        // 2. Check for output reference ($N.result)
        if raw_value.starts_with('$') {
            return Ok(ResolutionResult {
                status: ResolutionStatus::Resolved,
                entities: vec![],  // will be filled at execute time
                error: None,
            });
        }
        
        // 3. Search using client_group scope
        let matches = self.search_entities(raw_value).await?;
        
        match matches.len() {
            0 => Ok(ResolutionResult {
                status: ResolutionStatus::Failed,
                entities: vec![],
                error: Some(format!("No entities found for '{}'", raw_value)),
            }),
            
            _ => {
                // Check confidence spread
                let high_conf: Vec<_> = matches.iter()
                    .filter(|m| m.confidence.unwrap_or(0.0) >= 0.7)
                    .collect();
                
                let low_conf: Vec<_> = matches.iter()
                    .filter(|m| m.confidence.unwrap_or(0.0) < 0.7)
                    .collect();
                
                // If we have high-confidence matches, use them
                // If all matches are low-confidence, mark ambiguous
                if !high_conf.is_empty() {
                    Ok(ResolutionResult {
                        status: ResolutionStatus::Resolved,
                        entities: matches.into_iter().map(|m| ResolvedEntity {
                            entity_id: m.entity_id,
                            entity_name: m.entity_name,
                            arg_name: arg_name.to_string(),
                            resolution_source: match m.match_type {
                                MatchType::Exact => ResolutionSource::TagExact,
                                MatchType::Fuzzy => ResolutionSource::TagFuzzy,
                                MatchType::Semantic => ResolutionSource::TagSemantic,
                            },
                            original_ref: raw_value.to_string(),
                            confidence: m.confidence,
                        }).collect(),
                        error: None,
                    })
                } else if matches.len() > 1 && low_conf.len() == matches.len() {
                    // All low confidence — needs picker
                    Ok(ResolutionResult {
                        status: ResolutionStatus::Ambiguous,
                        entities: matches.into_iter().map(|m| ResolvedEntity {
                            entity_id: m.entity_id,
                            entity_name: m.entity_name,
                            arg_name: arg_name.to_string(),
                            resolution_source: match m.match_type {
                                MatchType::Exact => ResolutionSource::TagExact,
                                MatchType::Fuzzy => ResolutionSource::TagFuzzy,
                                MatchType::Semantic => ResolutionSource::TagSemantic,
                            },
                            original_ref: raw_value.to_string(),
                            confidence: m.confidence,
                        }).collect(),
                        error: Some("Multiple low-confidence matches — please select".to_string()),
                    })
                } else {
                    // Single match (any confidence)
                    Ok(ResolutionResult {
                        status: ResolutionStatus::Resolved,
                        entities: matches.into_iter().map(|m| ResolvedEntity {
                            entity_id: m.entity_id,
                            entity_name: m.entity_name,
                            arg_name: arg_name.to_string(),
                            resolution_source: match m.match_type {
                                MatchType::Exact => ResolutionSource::TagExact,
                                MatchType::Fuzzy => ResolutionSource::TagFuzzy,
                                MatchType::Semantic => ResolutionSource::TagSemantic,
                            },
                            original_ref: raw_value.to_string(),
                            confidence: m.confidence,
                        }).collect(),
                        error: None,
                    })
                }
            }
        }
    }
    
    /// Search using existing client_group tag functions
    async fn search_entities(&self, query: &str) -> Result<Vec<EntityContextMatch>> {
        // Uses search_entity_tags from migration 052
        let rows = sqlx::query!(r#"
            SELECT entity_id, entity_name, tag, confidence::FLOAT4, match_type
            FROM "ob-poc".search_entity_tags($1, $2, $3, 20)
        "#, self.client_group_id, query, self.persona)
            .fetch_all(self.pool)
            .await?;
        
        let mut matches: Vec<EntityContextMatch> = rows.into_iter().map(|r| {
            EntityContextMatch {
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                matched_tag: r.tag,
                confidence: r.confidence,
                match_type: match r.match_type.as_deref() {
                    Some("exact") => MatchType::Exact,
                    Some("fuzzy") => MatchType::Fuzzy,
                    _ => MatchType::Fuzzy,
                },
            }
        }).collect();
        
        // If no text matches and we have embedder, try semantic
        if matches.is_empty() {
            if let Some(embedder) = self.embedder {
                let embedding = embedder.embed(query).await?;
                let semantic_rows = sqlx::query!(r#"
                    SELECT entity_id, entity_name, tag, similarity::FLOAT4
                    FROM "ob-poc".search_entity_tags_semantic($1, $2::vector, $3, 10, 0.5)
                "#, self.client_group_id, &embedding, self.persona)
                    .fetch_all(self.pool)
                    .await?;
                
                matches = semantic_rows.into_iter().map(|r| {
                    EntityContextMatch {
                        entity_id: r.entity_id,
                        entity_name: r.entity_name,
                        matched_tag: r.tag,
                        confidence: r.similarity,
                        match_type: MatchType::Semantic,
                    }
                }).collect();
            }
        }
        
        Ok(matches)
    }
    
    fn parse_uuid_list(&self, s: &str) -> Option<Vec<Uuid>> {
        // Handle: single UUID, [uuid1, uuid2], or comma-separated
        // ...
    }
}
```

---

## 5) DAG analysis and reordering

### 5.1 Dependency detection

Commands can depend on each other via:
1. **Output references**: `$1.result`, `$2.entity_ids`
2. **Implicit ordering**: verb semantics (e.g., `create` before `update`)
3. **Entity overlap**: same entity touched by multiple commands

```rust
// rust/src/repl/dag.rs

use petgraph::graph::DiGraph;
use petgraph::algo::toposort;
use uuid::Uuid;

pub struct DagAnalyzer {
    graph: DiGraph<Uuid, DependencyEdge>,
    command_nodes: HashMap<Uuid, petgraph::graph::NodeIndex>,
}

#[derive(Debug, Clone)]
pub enum DependencyEdge {
    OutputRef { ref_name: String },      // $1.result
    MustPrecede { reason: String },      // verb semantics
    EntityConflict { entity_id: Uuid },  // same entity, ordering matters
}

impl DagAnalyzer {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            command_nodes: HashMap::new(),
        }
    }
    
    /// Add a command to the DAG
    pub fn add_command(&mut self, cmd: &StagedCommand) {
        let node = self.graph.add_node(cmd.id);
        self.command_nodes.insert(cmd.id, node);
    }
    
    /// Analyze dependencies and add edges
    pub fn analyze(&mut self, commands: &[StagedCommand], verb_registry: &VerbRegistry) {
        for cmd in commands {
            // 1. Output reference dependencies ($N.result)
            self.detect_output_refs(cmd, commands);
            
            // 2. Verb semantic dependencies (create before update)
            self.detect_verb_ordering(cmd, commands, verb_registry);
            
            // 3. Entity conflict dependencies (same entity, writes)
            self.detect_entity_conflicts(cmd, commands);
        }
    }
    
    /// Compute execution order via topological sort
    pub fn compute_order(&self) -> Result<Vec<Uuid>, DagError> {
        match toposort(&self.graph, None) {
            Ok(sorted) => {
                Ok(sorted.into_iter()
                    .map(|n| *self.graph.node_weight(n).unwrap())
                    .collect())
            }
            Err(cycle) => {
                Err(DagError::CycleDetected {
                    node: *self.graph.node_weight(cycle.node_id()).unwrap()
                })
            }
        }
    }
    
    /// Check if reorder changed anything
    pub fn reorder_diff(&self, original: &[Uuid], reordered: &[Uuid]) -> Option<ReorderDiff> {
        if original == reordered {
            return None;
        }
        
        Some(ReorderDiff {
            original: original.to_vec(),
            reordered: reordered.to_vec(),
            moves: self.compute_moves(original, reordered),
        })
    }
    
    fn detect_output_refs(&mut self, cmd: &StagedCommand, all: &[StagedCommand]) {
        // Parse DSL for $N references
        let refs = parse_output_refs(&cmd.dsl_raw);
        for ref_num in refs {
            if let Some(source_cmd) = all.iter().find(|c| c.source_order == ref_num) {
                self.add_edge(source_cmd.id, cmd.id, DependencyEdge::OutputRef {
                    ref_name: format!("${}", ref_num),
                });
            }
        }
    }
    
    fn detect_verb_ordering(&mut self, cmd: &StagedCommand, all: &[StagedCommand], registry: &VerbRegistry) {
        let verb_def = registry.get(&cmd.verb);
        if let Some(def) = verb_def {
            // Check if this verb requires another to run first
            if let Some(prereqs) = &def.metadata.prerequisites {
                for prereq_verb in prereqs {
                    // Find any staged command with that verb
                    for other in all {
                        if other.verb == *prereq_verb && other.id != cmd.id {
                            self.add_edge(other.id, cmd.id, DependencyEdge::MustPrecede {
                                reason: format!("{} must run before {}", prereq_verb, cmd.verb),
                            });
                        }
                    }
                }
            }
        }
    }
    
    fn detect_entity_conflicts(&mut self, cmd: &StagedCommand, all: &[StagedCommand]) {
        // If two commands write to same entity, preserve source order
        // (or flag for user confirmation)
        let cmd_entities: HashSet<_> = cmd.entity_footprint.iter()
            .map(|e| e.entity_id)
            .collect();
        
        for other in all {
            if other.id == cmd.id || other.source_order >= cmd.source_order {
                continue;
            }
            
            let other_entities: HashSet<_> = other.entity_footprint.iter()
                .map(|e| e.entity_id)
                .collect();
            
            let overlap: Vec<_> = cmd_entities.intersection(&other_entities).collect();
            
            if !overlap.is_empty() && is_write_verb(&cmd.verb) && is_write_verb(&other.verb) {
                // Preserve original order for writes to same entity
                self.add_edge(other.id, cmd.id, DependencyEdge::EntityConflict {
                    entity_id: *overlap[0],
                });
            }
        }
    }
    
    fn add_edge(&mut self, from: Uuid, to: Uuid, edge: DependencyEdge) {
        let from_node = self.command_nodes[&from];
        let to_node = self.command_nodes[&to];
        self.graph.add_edge(from_node, to_node, edge);
    }
}

#[derive(Debug)]
pub struct ReorderDiff {
    pub original: Vec<Uuid>,
    pub reordered: Vec<Uuid>,
    pub moves: Vec<ReorderMove>,
}

#[derive(Debug)]
pub struct ReorderMove {
    pub command_id: Uuid,
    pub from_position: usize,
    pub to_position: usize,
    pub reason: String,
}
```

---

## 6) MCP events and tools

### 6.1 Events (server → client)

```rust
// MCP event types for staged runbook

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunbookEvent {
    /// Command staged successfully
    CommandStaged {
        runbook_id: Uuid,
        command: StagedCommandSummary,
        runbook_summary: RunbookSummary,
    },
    
    /// Staging failed (parse error)
    StageFailed {
        runbook_id: Uuid,
        error_kind: String,   // "parse_failed" | "invalid_verb" | etc.
        error: String,
        dsl_raw: String,
    },
    
    /// Resolution needs user input (picker)
    ResolutionAmbiguous {
        runbook_id: Uuid,
        command_id: Uuid,
        arg_name: String,
        original_ref: String,
        candidates: Vec<PickerCandidate>,
    },
    
    /// Resolution failed (no matches)
    ResolutionFailed {
        runbook_id: Uuid,
        command_id: Uuid,
        arg_name: String,
        original_ref: String,
        error: String,
    },
    
    /// Runbook ready for execution
    RunbookReady {
        runbook_id: Uuid,
        summary: RunbookSummary,
        entity_footprint: Vec<EntityFootprintEntry>,
        reorder_diff: Option<ReorderDiff>,
    },
    
    /// Run rejected — runbook not ready
    RunbookNotReady {
        runbook_id: Uuid,
        blocking_commands: Vec<BlockingCommand>,
    },
    
    /// Command removed
    CommandRemoved {
        runbook_id: Uuid,
        command_id: Uuid,
        cascade_removed: Vec<Uuid>,  // dependents also removed
        runbook_summary: RunbookSummary,
    },
    
    /// Runbook aborted
    RunbookAborted {
        runbook_id: Uuid,
    },
    
    /// Execution started
    ExecutionStarted {
        runbook_id: Uuid,
        total_commands: usize,
    },
    
    /// Single command executed
    CommandExecuted {
        runbook_id: Uuid,
        command_id: Uuid,
        dag_order: i32,
        result: CommandResult,
    },
    
    /// All commands executed
    ExecutionCompleted {
        runbook_id: Uuid,
        results: Vec<CommandResult>,
        learned_tags: Vec<LearnedTag>,  // tags created from this session
    },
}

/// Command blocking execution
#[derive(Debug, Serialize)]
pub struct BlockingCommand {
    pub command_id: Uuid,
    pub source_order: i32,
    pub status: ResolutionStatus,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StagedCommandSummary {
    pub id: Uuid,
    pub source_order: i32,
    pub verb: String,
    pub description: String,
    pub resolution_status: ResolutionStatus,
    pub entity_count: usize,
}

#[derive(Debug, Serialize)]
pub struct RunbookSummary {
    pub id: Uuid,
    pub status: RunbookStatus,
    pub command_count: usize,
    pub resolved_count: usize,
    pub pending_count: usize,
    pub ambiguous_count: usize,
    pub failed_count: usize,
}

#[derive(Debug, Serialize)]
pub struct PickerCandidate {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub matched_tag: String,
    pub confidence: f32,
    pub match_type: String,
}

#[derive(Debug, Serialize)]
pub struct EntityFootprintEntry {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub commands: Vec<Uuid>,  // which commands touch this entity
    pub operations: Vec<String>,  // verbs applied to this entity
}
```

### 6.2 Tools (agent → server)

```yaml
# MCP tool definitions

tools:
  # Stage a DSL command (does NOT execute)
  - name: runbook_stage
    description: "Stage a DSL command for later execution. Resolves entity references but does NOT execute."
    parameters:
      dsl:
        type: string
        required: true
        description: "DSL command (may use shorthand like 'Irish funds')"
      description:
        type: string
        required: false
        description: "Human-readable description of what this does"
    returns:
      type: object
      properties:
        command_id: uuid
        resolution_status: string
        entities_resolved: array

  # Resolve ambiguous picker
  - name: runbook_pick
    description: "Select entities from ambiguous resolution candidates"
    parameters:
      command_id:
        type: uuid
        required: true
      entity_ids:
        type: array
        items: uuid
        required: true
        description: "Selected entity UUIDs from candidates"
    returns:
      type: object
      properties:
        resolution_status: string

  # Remove a staged command
  - name: runbook_remove
    description: "Remove a staged command (and its dependents)"
    parameters:
      line:
        type: integer
        required: true
        description: "Source order line number to remove"
    returns:
      type: object
      properties:
        removed_count: integer
        cascade_removed: array

  # Show current runbook
  - name: runbook_show
    description: "Show current staged commands and entity footprint"
    returns:
      type: object
      properties:
        summary: object
        commands: array
        entity_footprint: array

  # Clear all staged commands
  - name: runbook_abort
    description: "Clear all staged commands without executing"
    returns:
      type: object
      properties:
        aborted: boolean

  # Execute the runbook
  - name: runbook_run
    description: "Execute all staged commands in DAG order"
    returns:
      type: object
      properties:
        execution_id: uuid
        # Results come via events as each command completes
```

---

## 7) REPL verb definitions

```yaml
# session.yaml — REPL verbs

verbs:
  # ============================================================================
  # STAGING (no execution)
  # ============================================================================
  
  - verb: stage
    description: "Stage a DSL command without executing"
    behavior: plugin
    handler: ReplStageOp
    invocation_phrases:
      - "then"
      - "and"
      - "also"
      - "next"
      - "add"
    metadata:
      tier: intent
      source_of_truth: staged_runbook
      internal: false
      tags: [repl, stage]
    args:
      - name: dsl
        type: string
        required: true
        description: "DSL command (may use shorthand references)"
      - name: description
        type: string
        required: false
    returns:
      type: record
      fields: [command_id, resolution_status, source_order]
    effects:
      - "Parses DSL"
      - "Resolves entity arguments to UUIDs"
      - "Adds to staged_command table"
      - "Does NOT execute"

  # ============================================================================
  # PICKER (resolve ambiguous)
  # ============================================================================
  
  - verb: pick
    description: "Select entities from ambiguous candidates"
    behavior: plugin
    handler: ReplPickOp
    invocation_phrases:
      - "use"
      - "select"
      - "choose"
      - "pick"
    metadata:
      tier: intent
      source_of_truth: staged_command_entity
      internal: false
      tags: [repl, picker]
    args:
      - name: command-id
        type: uuid
        required: false
        description: "Which command to resolve (defaults to first ambiguous)"
      - name: entity-ids
        type: uuid_array
        required: true
        description: "Selected entity UUIDs"
    returns:
      type: record
      fields: [resolution_status, entities_selected]

  # ============================================================================
  # EDITING
  # ============================================================================
  
  - verb: remove
    description: "Remove a staged command"
    behavior: plugin
    handler: ReplRemoveOp
    invocation_phrases:
      - "remove"
      - "delete"
      - "drop"
      - "remove line"
    metadata:
      tier: intent
      source_of_truth: staged_command
      internal: false
      tags: [repl, edit]
    args:
      - name: line
        type: integer
        required: true
        description: "Source order line number"
    returns:
      type: record
      fields: [removed, cascade_removed]
    effects:
      - "Removes command and dependents"
      - "Renumbers remaining commands"

  - verb: edit
    description: "Edit a staged command's DSL"
    behavior: plugin
    handler: ReplEditOp
    invocation_phrases:
      - "edit"
      - "change"
      - "modify"
      - "update line"
    metadata:
      tier: intent
      source_of_truth: staged_command
      internal: false
      tags: [repl, edit]
    args:
      - name: line
        type: integer
        required: true
      - name: dsl
        type: string
        required: true
    returns:
      type: record
      fields: [command_id, resolution_status]
    effects:
      - "Re-parses and re-resolves the command"

  # ============================================================================
  # INSPECTION
  # ============================================================================
  
  - verb: show
    description: "Show current staged runbook"
    behavior: plugin
    handler: ReplShowOp
    invocation_phrases:
      - "show"
      - "show staged"
      - "show runbook"
      - "show plan"
      - "what's queued"
    metadata:
      tier: diagnostics
      source_of_truth: staged_runbook
      internal: false
      tags: [repl, inspect]
    returns:
      type: record
      fields: [summary, commands, entity_footprint, reorder_diff]

  # ============================================================================
  # EXECUTION
  # ============================================================================
  
  - verb: run
    description: "Execute all staged commands"
    behavior: plugin
    handler: ReplRunOp
    invocation_phrases:
      - "run"
      - "execute"
      - "do it"
      - "go"
      - "commit"
    metadata:
      tier: intent
      source_of_truth: staged_runbook
      writes_operational: true
      internal: false
      tags: [repl, execute]
    returns:
      type: record
      fields: [execution_id, status]
    effects:
      - "Validates all commands resolved"
      - "Computes DAG order"
      - "Executes in order"
      - "Emits per-command results via MCP events"

  - verb: abort
    description: "Clear all staged commands"
    behavior: plugin
    handler: ReplAbortOp
    invocation_phrases:
      - "abort"
      - "clear"
      - "cancel"
      - "reset"
      - "nevermind"
    metadata:
      tier: intent
      source_of_truth: staged_runbook
      internal: false
      tags: [repl, abort]
    returns:
      type: boolean
    effects:
      - "Deletes all staged_command rows"
      - "Sets runbook status to 'aborted'"
```

---

## 8) egui integration

### 8.1 Runbook panel

```rust
// rust/crates/ob-poc-ui/src/panels/runbook_panel.rs

pub struct RunbookPanel {
    runbook: Option<StagedRunbook>,
    show_entity_footprint: bool,
    show_dag_diff: bool,
    picker_state: Option<PickerState>,
}

impl RunbookPanel {
    pub fn render(&mut self, ui: &mut egui::Ui, ctx: &AppContext) {
        egui::Window::new("📋 Staged Runbook")
            .default_width(400.0)
            .show(ui.ctx(), |ui| {
                if let Some(runbook) = &self.runbook {
                    self.render_status_bar(ui, runbook);
                    ui.separator();
                    self.render_commands(ui, runbook, ctx);
                    ui.separator();
                    self.render_entity_footprint(ui, runbook);
                    ui.separator();
                    self.render_actions(ui, runbook, ctx);
                } else {
                    ui.label("No staged commands");
                }
            });
        
        // Picker modal
        if let Some(picker) = &mut self.picker_state {
            self.render_picker_modal(ui, picker, ctx);
        }
    }
    
    fn render_status_bar(&self, ui: &mut egui::Ui, runbook: &StagedRunbook) {
        ui.horizontal(|ui| {
            let status_color = match runbook.status {
                RunbookStatus::Building => egui::Color32::YELLOW,
                RunbookStatus::Ready => egui::Color32::GREEN,
                RunbookStatus::Executing => egui::Color32::LIGHT_BLUE,
                RunbookStatus::Completed => egui::Color32::DARK_GREEN,
                RunbookStatus::Aborted => egui::Color32::RED,
            };
            ui.colored_label(status_color, format!("{:?}", runbook.status));
            ui.label(format!("│ {} commands", runbook.commands.len()));
            
            let resolved = runbook.commands.iter()
                .filter(|c| c.resolution_status == ResolutionStatus::Resolved)
                .count();
            let ambiguous = runbook.commands.iter()
                .filter(|c| c.resolution_status == ResolutionStatus::Ambiguous)
                .count();
            
            if ambiguous > 0 {
                ui.colored_label(egui::Color32::YELLOW, format!("⚠ {} need selection", ambiguous));
            }
            
            ui.label(format!("│ {}/{} resolved", resolved, runbook.commands.len()));
        });
    }
    
    fn render_commands(&mut self, ui: &mut egui::Ui, runbook: &StagedRunbook, ctx: &AppContext) {
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            for cmd in &runbook.commands {
                self.render_command_row(ui, cmd, ctx);
            }
        });
    }
    
    fn render_command_row(&mut self, ui: &mut egui::Ui, cmd: &StagedCommand, ctx: &AppContext) {
        let status_icon = match cmd.resolution_status {
            ResolutionStatus::Resolved => "✓",
            ResolutionStatus::Pending => "⏳",
            ResolutionStatus::Ambiguous => "⚠",
            ResolutionStatus::Failed => "✗",
        };
        
        ui.horizontal(|ui| {
            // Line number
            ui.label(format!("{}.", cmd.source_order));
            
            // Status icon
            ui.label(status_icon);
            
            // Verb
            ui.strong(&cmd.verb);
            
            // Entity count
            if !cmd.entity_footprint.is_empty() {
                ui.label(format!("({} entities)", cmd.entity_footprint.len()));
            }
            
            // Remove button
            if ui.small_button("🗑").on_hover_text("Remove").clicked() {
                ctx.send_command(format!("repl.remove line={}", cmd.source_order));
            }
            
            // If ambiguous, show picker button
            if cmd.resolution_status == ResolutionStatus::Ambiguous {
                if ui.small_button("Select...").clicked() {
                    self.picker_state = Some(PickerState::new(cmd.id, &cmd.entity_footprint));
                }
            }
        });
        
        // Show DSL (resolved if available, otherwise raw)
        ui.indent(cmd.id, |ui| {
            let dsl = cmd.dsl_resolved.as_ref().unwrap_or(&cmd.dsl_raw);
            ui.code(dsl);
            
            if let Some(desc) = &cmd.description {
                ui.label(egui::RichText::new(desc).weak().italics());
            }
        });
    }
    
    fn render_entity_footprint(&self, ui: &mut egui::Ui, runbook: &StagedRunbook) {
        ui.collapsing("🏢 Entity Footprint", |ui| {
            // Aggregate all entities across commands
            let mut entity_map: HashMap<Uuid, Vec<&StagedCommand>> = HashMap::new();
            for cmd in &runbook.commands {
                for entity in &cmd.entity_footprint {
                    entity_map.entry(entity.entity_id).or_default().push(cmd);
                }
            }
            
            for (entity_id, commands) in &entity_map {
                let first = commands[0].entity_footprint.iter()
                    .find(|e| e.entity_id == *entity_id)
                    .unwrap();
                
                ui.horizontal(|ui| {
                    ui.label(&first.entity_name);
                    ui.label(format!("│ {} commands", commands.len()));
                    ui.label(format!("│ {}", first.resolution_source));
                });
            }
        });
    }
    
    fn render_actions(&self, ui: &mut egui::Ui, runbook: &StagedRunbook, ctx: &AppContext) {
        ui.horizontal(|ui| {
            let can_run = runbook.status == RunbookStatus::Building
                && runbook.commands.iter().all(|c| c.resolution_status == ResolutionStatus::Resolved);
            
            ui.add_enabled_ui(can_run, |ui| {
                if ui.button("▶ Run").clicked() {
                    ctx.send_command("repl.run");
                }
            });
            
            if ui.button("🗑 Clear All").clicked() {
                ctx.send_command("repl.abort");
            }
            
            if self.show_dag_diff {
                if ui.button("Show Reorder").clicked() {
                    // Toggle DAG diff view
                }
            }
        });
    }
    
    fn render_picker_modal(&mut self, ui: &mut egui::Ui, picker: &mut PickerState, ctx: &AppContext) {
        egui::Window::new("Select Entities")
            .collapsible(false)
            .show(ui.ctx(), |ui| {
                ui.label(&picker.original_ref);
                ui.separator();
                
                for candidate in &mut picker.candidates {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut candidate.selected, "");
                        ui.label(&candidate.entity_name);
                        ui.label(format!("({:.0}%)", candidate.confidence * 100.0));
                        ui.label(&candidate.matched_tag);
                    });
                }
                
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Confirm").clicked() {
                        let selected: Vec<_> = picker.candidates.iter()
                            .filter(|c| c.selected)
                            .map(|c| c.entity_id)
                            .collect();
                        
                        ctx.send_command(format!(
                            "repl.pick command-id={} entity-ids=[{}]",
                            picker.command_id,
                            selected.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",")
                        ));
                        
                        self.picker_state = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.picker_state = None;
                    }
                });
            });
    }
}
```

---

## 9) Implementation phases

### Phase 1: Schema + basic staging
- [ ] Migration 054: staged_runbook, staged_command, staged_command_entity
- [ ] `ReplStageOp`: parse DSL, resolve entities, stage command
- [ ] `ReplShowOp`: render staged commands
- [ ] `ReplAbortOp`: clear staging
- [ ] MCP events: CommandStaged, RunbookAborted

### Phase 2: Resolution pipeline
- [ ] `EntityArgResolver`: parse → detect → search → evaluate → substitute
- [ ] Integration with existing `search_entity_tags` functions
- [ ] Ambiguous handling → PickerCandidate list
- [ ] `ReplPickOp`: resolve ambiguous from selection
- [ ] MCP events: ResolutionAmbiguous, ResolutionFailed

### Phase 3: DAG analysis
- [ ] `DagAnalyzer`: build graph, detect dependencies
- [ ] Output reference parsing ($N.result)
- [ ] Topological sort
- [ ] Reorder diff computation
- [ ] MCP event: RunbookReady (with reorder_diff)

### Phase 4: Execution
- [ ] `ReplRunOp`: validate → DAG sort → execute sequence
- [ ] Per-command result capture
- [ ] MCP events: ExecutionStarted, CommandExecuted, ExecutionCompleted
- [ ] Learning: create user_confirmed tags from successful resolutions

### Phase 5: egui integration
- [ ] RunbookPanel: status bar, command list, entity footprint
- [ ] Picker modal for ambiguous resolution
- [ ] Remove/edit command interactions
- [ ] Run/abort buttons

### Phase 6: Edge cases + polish
- [ ] Handle empty runbook
- [ ] Handle partial resolution (some resolved, some pending)
- [ ] Handle execution errors (rollback? continue?)
- [ ] Session cleanup on disconnect
- [ ] Runbook persistence (save/load for later)

---

## 10) Testing

```rust
#[tokio::test]
async fn test_stage_resolves_shorthand() {
    let pool = test_pool().await;
    let session = test_session(&pool, ALLIANZ_GROUP).await;
    
    // Stage with shorthand
    let result = session.stage("entity.list entity-ids=\"Irish funds\"").await.unwrap();
    
    assert_eq!(result.resolution_status, ResolutionStatus::Resolved);
    assert!(!result.entities_resolved.is_empty());
    
    // DSL should have UUIDs
    let cmd = session.get_command(result.command_id).await.unwrap();
    assert!(cmd.dsl_resolved.unwrap().contains(&result.entities_resolved[0].entity_id.to_string()));
}

#[tokio::test]
async fn test_stage_no_execution() {
    let pool = test_pool().await;
    let session = test_session(&pool, ALLIANZ_GROUP).await;
    
    // Stage a write command
    session.stage("entity.update entity-id=\"main manco\" status=inactive").await.unwrap();
    
    // Verify entity NOT updated
    let entity = get_entity(&pool, ALLIANZ_MANCO).await.unwrap();
    assert_ne!(entity.status, "inactive");
    
    // Execute
    session.run().await.unwrap();
    
    // NOW entity should be updated
    let entity = get_entity(&pool, ALLIANZ_MANCO).await.unwrap();
    assert_eq!(entity.status, "inactive");
}

#[tokio::test]
async fn test_dag_reorders_dependencies() {
    let pool = test_pool().await;
    let session = test_session(&pool, ALLIANZ_GROUP).await;
    
    // Stage in wrong order (use result before it exists)
    session.stage("entity.update entity-ids=$1.result status=active").await.unwrap();  // line 1
    session.stage("entity.list filter=[status=pending]").await.unwrap();  // line 2
    
    // Run should reorder: line 2 first (produces $1), then line 1 (uses $1)
    let runbook = session.get_runbook().await.unwrap();
    
    let dag_ordered: Vec<_> = runbook.commands.iter()
        .sorted_by_key(|c| c.dag_order)
        .collect();
    
    assert_eq!(dag_ordered[0].source_order, 2);  // list first
    assert_eq!(dag_ordered[1].source_order, 1);  // update second
}

#[tokio::test]
async fn test_ambiguous_requires_picker() {
    let pool = test_pool().await;
    let session = test_session(&pool, ALLIANZ_GROUP).await;
    
    // Add two similar tags with low confidence
    add_test_tag(&pool, ALLIANZ_GROUP, ENTITY_A, "manco", 0.4).await;
    add_test_tag(&pool, ALLIANZ_GROUP, ENTITY_B, "manco", 0.4).await;
    
    // Stage with ambiguous shorthand
    let result = session.stage("entity.get entity-id=\"manco\"").await.unwrap();
    
    assert_eq!(result.resolution_status, ResolutionStatus::Ambiguous);
    assert_eq!(result.candidates.len(), 2);
    
    // Cannot run until resolved
    let run_result = session.run().await;
    assert!(run_result.is_err());
    
    // Pick one
    session.pick(result.command_id, vec![ENTITY_A]).await.unwrap();
    
    // Now can run
    session.run().await.unwrap();
}

#[tokio::test]
async fn test_abort_clears_staging() {
    let pool = test_pool().await;
    let session = test_session(&pool, ALLIANZ_GROUP).await;
    
    session.stage("entity.list entity-ids=\"Irish funds\"").await.unwrap();
    session.stage("kyc.check entity-ids=$1.result").await.unwrap();
    
    let runbook = session.get_runbook().await.unwrap();
    assert_eq!(runbook.commands.len(), 2);
    
    session.abort().await.unwrap();
    
    let runbook = session.get_runbook().await.unwrap();
    assert_eq!(runbook.commands.len(), 0);
    assert_eq!(runbook.status, RunbookStatus::Aborted);
}
```

---

## 11) Acceptance criteria

### Anti-hallucination
- [ ] No DSL executes without explicit `run/execute/commit`
- [ ] All entity UUIDs come from DB resolution (never invented)
- [ ] Ambiguous resolutions surface picker (not auto-picked)
- [ ] Failed resolutions block execution

### Staging
- [ ] Commands accumulate in session-scoped runbook
- [ ] Each command shows resolution status
- [ ] Entity footprint visible before execution
- [ ] Commands can be removed/edited before execution

### Resolution
- [ ] Shorthand resolved via client_group scope
- [ ] Exact → fuzzy → semantic fallback
- [ ] Output references ($N.result) detected and linked
- [ ] Direct UUIDs pass through unchanged

### DAG
- [ ] Dependencies detected from output refs
- [ ] Topological sort produces valid order
- [ ] Reorder diff shown to user
- [ ] Cycles detected and reported

### Execution
- [ ] Commands execute in DAG order
- [ ] Per-command results reported via MCP
- [ ] Successful resolutions create learned tags
- [ ] Errors halt execution (or continue per config)

### MCP
- [ ] All tools are DB-backed (no guessing)
- [ ] Events provide full context for UI rendering
- [ ] Picker candidates come from search functions

---

## 12) Summary

This TODO adds a **deterministic, reviewable execution model** that:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   "Show me Irish funds, then check their KYC"                              │
│                           │                                                 │
│                           ▼                                                 │
│   ┌───────────────────────────────────────────────────────────────┐        │
│   │  STAGE + RESOLVE (no execution)                                │        │
│   │                                                                │        │
│   │  1. entity.list entity-ids=[uuid1, uuid2, uuid3]  ✓ resolved  │        │
│   │  2. kyc.check entity-ids=$1.result                ✓ resolved  │        │
│   │                                                                │        │
│   │  Entity footprint: AGI Ireland Fund, Dublin SICAV, Cork...    │        │
│   │  DAG order: 1 → 2 (no reorder needed)                         │        │
│   └───────────────────────────────────────────────────────────────┘        │
│                           │                                                 │
│                           ▼                                                 │
│   ┌───────────────────────────────────────────────────────────────┐        │
│   │  REVIEW (human in loop)                                        │        │
│   │                                                                │        │
│   │  "That looks right" → run                                      │        │
│   │  "Remove line 2" → edit                                        │        │
│   │  "Nevermind" → abort                                           │        │
│   └───────────────────────────────────────────────────────────────┘        │
│                           │                                                 │
│                           ▼ (on "run")                                      │
│   ┌───────────────────────────────────────────────────────────────┐        │
│   │  EXECUTE (in DAG order)                                        │        │
│   │                                                                │        │
│   │  1. ✓ entity.list → 3 entities                                │        │
│   │  2. ✓ kyc.check → 2 complete, 1 missing                       │        │
│   │                                                                │        │
│   │  LEARN: "Irish funds" tag reinforced for all 3 entities       │        │
│   └───────────────────────────────────────────────────────────────┘        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

The agent **cannot hallucinate** because:
1. Entity resolution uses DB-backed search (never invents UUIDs)
2. Ambiguity surfaces picker (never auto-picks)
3. Nothing executes until explicit user confirmation
4. DAG ensures correct ordering (no implicit dependencies)

---

## References

- TODO-CLIENT-GROUP-SCOPE-RESOLUTION.md — entity tag search functions (prerequisite)
- migrations/047_client_group_tables.sql — client_group schema
- migrations/052_client_group_entity_context.sql — entity membership + tags
- rust/src/session — session state management
- rust/crates/ob-poc-ui — egui panels
