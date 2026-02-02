# Entity Scope DSL: Consolidated Implementation Plan

## Document Metadata

| Field | Value |
|-------|-------|
| **Status** | APPROVED FOR IMPLEMENTATION |
| **Source Docs** | `ARCH-PROPOSAL-entity-lineage-and-semantic-scope-v5.md`, `TODO-entity-scope-implementation.md` |
| **Created** | 2026-02-02 |
| **Approach** | **Incremental Extension** (not rip-and-replace) |

---

## Executive Summary

This plan consolidates the v5 architecture proposal and TODO into a single implementation roadmap. The key insight from codebase analysis:

> **80% of the required infrastructure already exists.** This is an integration and extension project, not a greenfield build.

### Approach Decision: Incremental Extension

| Criteria | Rip-and-Replace | Incremental Extension ✓ |
|----------|-----------------|-------------------------|
| Stage 0 scope resolution | Rebuild from scratch | ✅ Reuse `scope_resolution.rs` |
| Entity membership model | New tables | ✅ Extend `client_group_entity` (already slim) |
| Attribute provenance | New `entity_attribute_sources` | ✅ Use existing `attribute_values_typed` |
| AST/Compiler/DAG | Major rewrite | ✅ Add scope node variants |
| Session state | New session model | ✅ Extend `UnifiedSession` |
| Risk | High - regression risk | Low - additive changes |
| Timeline | 4-6 weeks | **2-3 weeks** |

**Rationale**: The existing codebase has excellent separation of concerns. Adding scope DSL operands requires ~20% new code, 80% integration.

---

## Implementation Alignment Matrix

| v5 Proposal Concept | Already Exists | Location | Gap |
|---------------------|----------------|----------|-----|
| Stage 0 scope hard gate | ✅ | `rust/src/mcp/scope_resolution.rs` | None |
| Entity membership (slim) | ✅ | `migrations/052_*` | None |
| Entity tags + embeddings | ✅ | `migrations/052_*` | Semantic search not wired |
| Typed roles + lineage | ✅ | `migrations/055_*` | None |
| Attribute provenance | ✅ | `attribute_values_typed` | None |
| Constraint cascade | ✅ | `rust/src/session/constraint_cascade.rs` | None |
| Scope DSL operands (@s1) | ❌ | — | **Needs AST/parser extension** |
| `scope.resolve/narrow/commit` | ❌ | — | **Needs compiler ops** |
| `scope_snapshots` table | ❌ | — | **New migration 064** |
| `resolution_events` table | ❌ | — | **New migration 065** |
| Linter rules S001-S007 | ❌ | — | **New linter module** |
| Agent pipeline integration | ⚠️ Partial | `agent_service.rs` | Wire `ScopeResolver` |

---

## Phase 0: Pre-Validation (Day 1)

> **CRITICAL**: Validate existing implementations before writing any code.

### Checklist

```bash
# Run these commands and verify output matches expected behavior

# 1. Verify Stage 0 hard gate exists
grep -n "SCOPE_PREFIXES\|TARGET_INDICATORS" rust/src/mcp/scope_resolution.rs

# 2. Verify slim membership model (no duplicate fact columns)
psql -d data_designer -c "\d \"ob-poc\".client_group_entity" | grep -v "lei\|jurisdiction\|is_fund"

# 3. Verify entity tag embeddings table exists
psql -d data_designer -c "SELECT COUNT(*) FROM \"ob-poc\".client_group_entity_tag_embedding"

# 4. Verify constraint cascade implementation
grep -n "set_client\|set_structure" rust/src/session/constraint_cascade.rs

# 5. Review AST for extension points
grep -n "enum Expr\|enum ArgValue" rust/crates/dsl-core/src/ast.rs
```

### Validation Outcomes

- [ ] Stage 0 hard gate confirmed (returns early for scope phrases)
- [ ] `client_group_entity` has NO `lei`, `jurisdiction`, `is_fund` columns
- [ ] `client_group_entity_tag_embedding` has >0 rows
- [ ] `UnifiedSession.client` field exists
- [ ] `Expr` enum is extensible (no `#[non_exhaustive]` blocker)

---

## Phase 1: Database Layer (Day 2-3)

### 1.1 Create `066_scope_snapshots.sql`

> Note: Using 066 to avoid collision with any pending migrations

```sql
-- Location: migrations/066_scope_snapshots.sql

CREATE TABLE "ob-poc".scope_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Descriptor (what was requested)
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id),
    description TEXT,
    filter_applied JSONB,
    limit_requested INTEGER NOT NULL,
    mode TEXT NOT NULL DEFAULT 'interactive' 
        CHECK (mode IN ('strict', 'interactive', 'greedy')),
    
    -- Resolved list (deterministic output)
    selected_entity_ids UUID[] NOT NULL,
    entity_count INTEGER GENERATED ALWAYS AS (array_length(selected_entity_ids, 1)) STORED,
    
    -- Scoring summary
    top_k_candidates JSONB NOT NULL DEFAULT '[]',
    resolution_method TEXT NOT NULL,
    overall_confidence DECIMAL(3,2),
    
    -- Index fingerprints (drift detection)
    embedder_version TEXT,
    
    -- Lineage
    parent_snapshot_id UUID REFERENCES "ob-poc".scope_snapshots(id),
    
    -- Audit
    session_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by TEXT NOT NULL DEFAULT 'agent',
    
    CONSTRAINT chk_ss_nonempty CHECK (array_length(selected_entity_ids, 1) > 0)
);

-- Immutability trigger
CREATE OR REPLACE FUNCTION "ob-poc".prevent_snapshot_update()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'scope_snapshots are immutable after creation';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER snapshot_immutable
    BEFORE UPDATE ON "ob-poc".scope_snapshots
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".prevent_snapshot_update();

-- Indexes
CREATE INDEX idx_ss_session ON "ob-poc".scope_snapshots(session_id);
CREATE INDEX idx_ss_group ON "ob-poc".scope_snapshots(group_id);
CREATE INDEX idx_ss_created ON "ob-poc".scope_snapshots(created_at DESC);
```

**Tests**:
- [ ] Immutability trigger fires on UPDATE
- [ ] `entity_count` generated correctly
- [ ] FK to `client_group` enforced

### 1.2 Create `067_resolution_events.sql`

```sql
-- Location: migrations/067_resolution_events.sql

CREATE TABLE "ob-poc".resolution_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID,
    snapshot_id UUID REFERENCES "ob-poc".scope_snapshots(id),
    
    -- Event classification
    event_type TEXT NOT NULL CHECK (event_type IN (
        'scope_created',      -- New scope resolved
        'scope_narrowed',     -- Scope refined
        'scope_committed',    -- User confirmed
        'scope_rejected',     -- User rejected suggestion
        'verb_overridden',    -- User changed verb
        'group_changed'       -- User changed group anchor
    )),
    
    -- Payload (event-specific data)
    payload JSONB NOT NULL DEFAULT '{}',
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Learning query indexes
CREATE INDEX idx_re_session ON "ob-poc".resolution_events(session_id);
CREATE INDEX idx_re_type ON "ob-poc".resolution_events(event_type);
CREATE INDEX idx_re_snapshot ON "ob-poc".resolution_events(snapshot_id) 
    WHERE snapshot_id IS NOT NULL;

-- Hit-rate analysis view
CREATE OR REPLACE VIEW "ob-poc".v_scope_hit_rate AS
SELECT
    DATE_TRUNC('day', created_at) AS day,
    COUNT(*) FILTER (WHERE event_type = 'scope_created') AS resolutions,
    COUNT(*) FILTER (WHERE event_type = 'scope_committed') AS commits,
    COUNT(*) FILTER (WHERE event_type = 'scope_rejected') AS rejections,
    COUNT(*) FILTER (WHERE event_type = 'scope_narrowed') AS narrows,
    ROUND(
        COUNT(*) FILTER (WHERE event_type = 'scope_committed')::NUMERIC /
        NULLIF(COUNT(*) FILTER (WHERE event_type = 'scope_created'), 0),
        3
    ) AS hit_rate
FROM "ob-poc".resolution_events
GROUP BY DATE_TRUNC('day', created_at)
ORDER BY day DESC;
```

**Tests**:
- [ ] Event types validated by CHECK constraint
- [ ] Hit-rate view returns correct calculations

---

## Phase 2: AST Extensions (Day 4-5)

### 2.1 Add Scope Nodes to AST

**File**: `rust/crates/dsl-core/src/ast.rs`

```rust
// Add to Expr enum (after existing variants):

/// Anchor session to a client group
ScopeAnchor {
    group: String,
    span: Span,
},

/// Resolve entity scope (creates candidate set)
ScopeResolve {
    desc: String,
    limit: u32,
    mode: ResolutionMode,
    as_symbol: String,
    span: Span,
},

/// Narrow an existing scope
ScopeNarrow {
    scope_symbol: String,
    filter: FilterExpr,
    as_symbol: String,
    span: Span,
},

/// Commit a scope (freeze to snapshot)
ScopeCommit {
    scope_symbol: String,
    as_symbol: String,
    span: Span,
},

/// Refresh a committed scope (re-resolve against current index)
ScopeRefresh {
    old_symbol: String,
    as_symbol: String,
    span: Span,
},

// Add to ArgValue enum:
/// Reference to a committed scope
ScopeRef(String),

// Add new types:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionMode {
    Strict,      // Fail if ambiguous
    Interactive, // Prompt for clarification
    Greedy,      // Take top-1 (audit logged)
}

impl Default for ResolutionMode {
    fn default() -> Self { Self::Interactive }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilterExpr {
    HasRole(String),
    HasType(String),
    InJurisdiction(String),
    HasTag(String),
    And(Box<FilterExpr>, Box<FilterExpr>),
    Or(Box<FilterExpr>, Box<FilterExpr>),
    Not(Box<FilterExpr>),
}
```

**Tests**:
- [ ] All new variants serialize/deserialize correctly
- [ ] `FilterExpr` composes correctly (And/Or/Not)
- [ ] Display implementations work

### 2.2 Update Expr Display

```rust
impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // ... existing matches
            
            Expr::ScopeAnchor { group, .. } => {
                write!(f, "(scope.anchor :group \"{}\")", group)
            }
            Expr::ScopeResolve { desc, limit, mode, as_symbol, .. } => {
                write!(f, "(scope.resolve :desc \"{}\" :limit {} :mode {:?} :as @{})", 
                    desc, limit, mode, as_symbol)
            }
            Expr::ScopeNarrow { scope_symbol, filter, as_symbol, .. } => {
                write!(f, "(scope.narrow @{} :filter {} :as @{})", 
                    scope_symbol, filter, as_symbol)
            }
            Expr::ScopeCommit { scope_symbol, as_symbol, .. } => {
                write!(f, "(scope.commit :scope @{} :as @{})", scope_symbol, as_symbol)
            }
            Expr::ScopeRefresh { old_symbol, as_symbol, .. } => {
                write!(f, "(scope.refresh @{} :as @{})", old_symbol, as_symbol)
            }
        }
    }
}
```

---

## Phase 3: Parser Extensions (Day 6-7)

### 3.1 Add Scope Verb Parsing

**File**: `rust/crates/dsl-core/src/parser.rs`

```rust
/// Parse scope.* verbs
fn parse_scope_verb(&mut self, verb: &str) -> Result<Expr, ParseError> {
    let span_start = self.current_span();
    
    match verb {
        "scope.anchor" => {
            self.expect_keyword(":group")?;
            let group = self.parse_string()?;
            Ok(Expr::ScopeAnchor { 
                group, 
                span: self.span_from(span_start) 
            })
        }
        "scope.resolve" => {
            self.expect_keyword(":desc")?;
            let desc = self.parse_string()?;
            
            self.expect_keyword(":limit")?;
            let limit = self.parse_integer()? as u32;
            
            let mode = if self.try_keyword(":mode") {
                self.parse_resolution_mode()?
            } else {
                ResolutionMode::default()
            };
            
            self.expect_keyword(":as")?;
            let as_symbol = self.parse_symbol()?;
            
            Ok(Expr::ScopeResolve {
                desc,
                limit,
                mode,
                as_symbol,
                span: self.span_from(span_start),
            })
        }
        "scope.narrow" => {
            let scope_symbol = self.parse_symbol()?;
            self.expect_keyword(":filter")?;
            let filter = self.parse_filter_expr()?;
            self.expect_keyword(":as")?;
            let as_symbol = self.parse_symbol()?;
            
            Ok(Expr::ScopeNarrow {
                scope_symbol,
                filter,
                as_symbol,
                span: self.span_from(span_start),
            })
        }
        "scope.commit" => {
            self.expect_keyword(":scope")?;
            let scope_symbol = self.parse_symbol()?;
            self.expect_keyword(":as")?;
            let as_symbol = self.parse_symbol()?;
            
            Ok(Expr::ScopeCommit {
                scope_symbol,
                as_symbol,
                span: self.span_from(span_start),
            })
        }
        "scope.refresh" => {
            let old_symbol = self.parse_symbol()?;
            self.expect_keyword(":as")?;
            let as_symbol = self.parse_symbol()?;
            
            Ok(Expr::ScopeRefresh {
                old_symbol,
                as_symbol,
                span: self.span_from(span_start),
            })
        }
        _ => Err(ParseError::UnknownScopeVerb(verb.to_string())),
    }
}

/// Parse @symbol syntax
fn parse_symbol(&mut self) -> Result<String, ParseError> {
    self.expect_token(Token::At)?;
    self.parse_identifier()
}

/// Parse filter expressions
fn parse_filter_expr(&mut self) -> Result<FilterExpr, ParseError> {
    self.expect_token(Token::LParen)?;
    
    let kind = self.parse_identifier()?;
    let filter = match kind.as_str() {
        "has-role" => FilterExpr::HasRole(self.parse_string()?),
        "has-type" => FilterExpr::HasType(self.parse_string()?),
        "in-jurisdiction" => FilterExpr::InJurisdiction(self.parse_string()?),
        "has-tag" => FilterExpr::HasTag(self.parse_string()?),
        "and" => {
            let left = self.parse_filter_expr()?;
            let right = self.parse_filter_expr()?;
            FilterExpr::And(Box::new(left), Box::new(right))
        }
        "or" => {
            let left = self.parse_filter_expr()?;
            let right = self.parse_filter_expr()?;
            FilterExpr::Or(Box::new(left), Box::new(right))
        }
        "not" => {
            let inner = self.parse_filter_expr()?;
            FilterExpr::Not(Box::new(inner))
        }
        _ => return Err(ParseError::UnknownFilterKind(kind)),
    };
    
    self.expect_token(Token::RParen)?;
    Ok(filter)
}

/// Parse :scope @sX in verb arguments
fn parse_arg_value(&mut self) -> Result<ArgValue, ParseError> {
    if self.peek_token() == Some(&Token::At) {
        self.advance();
        let symbol = self.parse_identifier()?;
        Ok(ArgValue::ScopeRef(symbol))
    } else {
        // ... existing parsing
    }
}
```

**Tests**:
- [ ] `(scope.anchor :group "allianz")` parses correctly
- [ ] `(scope.resolve :desc "Irish funds" :limit 50 :as @s1)` parses
- [ ] `(scope.narrow @s1 :filter (has-role "custodian") :as @s2)` parses
- [ ] Nested filter expressions work: `(and (has-role "x") (in-jurisdiction "IE"))`
- [ ] `(verb :scope @s1)` parses to `ArgValue::ScopeRef`

---

## Phase 4: Compiler Extensions (Day 8-9)

### 4.1 Add Scope Ops

**File**: `rust/crates/dsl-core/src/ops.rs`

```rust
// Add to Op enum:

/// Resolve a scope descriptor to candidate set
ResolveScopeOp {
    desc: String,
    limit: u32,
    mode: ResolutionMode,
    output_symbol: String,
    group_id: Option<Uuid>,  // None = use session anchor
},

/// Narrow an existing scope
NarrowScopeOp {
    source_symbol: String,
    filter: FilterExpr,
    output_symbol: String,
},

/// Commit scope to immutable snapshot
CommitScopeOp {
    source_symbol: String,
    output_symbol: String,
    snapshot_id: Uuid,  // Pre-generated for determinism
},

/// Expand scope to entity tuples (macro expansion)
ExpandScopeOp {
    snapshot_id: Uuid,
    target_verb: String,
    target_args: HashMap<String, ArgValue>,
},
```

### 4.2 Symbol Table Extension

**File**: `rust/crates/dsl-core/src/compiler.rs`

```rust
/// Scope binding states
#[derive(Debug, Clone)]
pub enum ScopeBinding {
    /// Unresolved candidates (pre-commit)
    Candidates {
        desc: String,
        entity_ids: Vec<Uuid>,
        scores: Vec<f32>,
    },
    /// Committed snapshot (frozen)
    Committed {
        snapshot_id: Uuid,
        entity_ids: Vec<Uuid>,
    },
}

/// Extended compilation context
pub struct CompilationContext {
    pub symbol_table: HashMap<String, EntityKey>,
    pub scope_symbols: HashMap<String, ScopeBinding>,
    pub session_group_id: Option<Uuid>,
}

impl CompilationContext {
    pub fn bind_scope_candidates(
        &mut self,
        symbol: &str,
        desc: &str,
        entity_ids: Vec<Uuid>,
        scores: Vec<f32>,
    ) {
        self.scope_symbols.insert(
            symbol.to_string(),
            ScopeBinding::Candidates { 
                desc: desc.to_string(), 
                entity_ids, 
                scores 
            },
        );
    }
    
    pub fn commit_scope(
        &mut self,
        source_symbol: &str,
        target_symbol: &str,
        snapshot_id: Uuid,
    ) -> Result<(), CompileError> {
        let binding = self.scope_symbols.get(source_symbol)
            .ok_or(CompileError::UndefinedScopeSymbol(source_symbol.to_string()))?;
        
        let entity_ids = match binding {
            ScopeBinding::Candidates { entity_ids, .. } => entity_ids.clone(),
            ScopeBinding::Committed { .. } => {
                return Err(CompileError::ScopeAlreadyCommitted(source_symbol.to_string()));
            }
        };
        
        self.scope_symbols.insert(
            target_symbol.to_string(),
            ScopeBinding::Committed { snapshot_id, entity_ids },
        );
        
        Ok(())
    }
    
    pub fn get_committed_scope(&self, symbol: &str) -> Option<&ScopeBinding> {
        self.scope_symbols.get(symbol).filter(|b| matches!(b, ScopeBinding::Committed { .. }))
    }
}
```

### 4.3 Scope Expansion Logic

```rust
/// Expand (verb :scope @sX) to tuple form
fn expand_scope_verb_call(
    &mut self,
    verb: &str,
    args: &HashMap<String, ArgValue>,
    ctx: &CompilationContext,
) -> Result<Vec<Op>, CompileError> {
    // Find scope reference in args
    let scope_ref = args.iter()
        .find_map(|(k, v)| {
            if let ArgValue::ScopeRef(sym) = v {
                Some((k.clone(), sym.clone()))
            } else {
                None
            }
        });
    
    let Some((arg_key, scope_symbol)) = scope_ref else {
        // No scope ref, compile normally
        return self.compile_verb_call_normal(verb, args);
    };
    
    // Get committed scope
    let binding = ctx.get_committed_scope(&scope_symbol)
        .ok_or(CompileError::ScopeNotCommitted(scope_symbol.clone()))?;
    
    let ScopeBinding::Committed { entity_ids, .. } = binding else {
        unreachable!("get_committed_scope filters for Committed");
    };
    
    // Check if verb is set_native
    let verb_schema = self.registry.get_verb(verb)?;
    if verb_schema.set_native.unwrap_or(false) {
        // Keep scope reference, don't expand
        return self.compile_verb_call_normal(verb, args);
    }
    
    // Expand to N tuples
    let mut ops = vec![];
    for entity_id in entity_ids {
        let mut expanded_args = args.clone();
        expanded_args.remove(&arg_key);
        expanded_args.insert(
            "entity-id".to_string(),
            ArgValue::Uuid(*entity_id),
        );
        ops.extend(self.compile_verb_call_normal(verb, &expanded_args)?);
    }
    
    Ok(ops)
}
```

---

## Phase 5: DAG Extensions (Day 10)

### 5.1 Scope Dependencies

**File**: `rust/crates/dsl-core/src/dag.rs`

```rust
/// Extract scope dependencies from ops
fn scope_dependencies(op: &Op) -> ScopeDeps {
    match op {
        Op::ResolveScopeOp { output_symbol, .. } => {
            ScopeDeps::Produces(output_symbol.clone())
        }
        Op::NarrowScopeOp { source_symbol, output_symbol, .. } => {
            ScopeDeps::Both {
                consumes: vec![source_symbol.clone()],
                produces: output_symbol.clone(),
            }
        }
        Op::CommitScopeOp { source_symbol, output_symbol, .. } => {
            ScopeDeps::Both {
                consumes: vec![source_symbol.clone()],
                produces: output_symbol.clone(),
            }
        }
        Op::ExpandScopeOp { .. } => {
            // Already expanded at compile time
            ScopeDeps::None
        }
        // For regular ops, check for ScopeRef in args
        _ => extract_scope_refs_from_args(op),
    }
}

/// Add scope edges to DAG
fn add_scope_edges(dag: &mut Dag, ops: &[Op]) {
    let mut scope_producers: HashMap<String, usize> = HashMap::new();
    
    for (idx, op) in ops.iter().enumerate() {
        match scope_dependencies(op) {
            ScopeDeps::Produces(sym) => {
                scope_producers.insert(sym, idx);
            }
            ScopeDeps::Consumes(syms) => {
                for sym in syms {
                    if let Some(&producer_idx) = scope_producers.get(&sym) {
                        dag.add_edge(producer_idx, idx);
                    }
                }
            }
            ScopeDeps::Both { consumes, produces } => {
                for sym in consumes {
                    if let Some(&producer_idx) = scope_producers.get(&sym) {
                        dag.add_edge(producer_idx, idx);
                    }
                }
                scope_producers.insert(produces, idx);
            }
            ScopeDeps::None => {}
        }
    }
}
```

**Tests**:
- [ ] `scope.resolve @s1` → `scope.commit @s1` has edge
- [ ] `scope.narrow @s1 :as @s2` → `scope.commit @s2` has edge
- [ ] Circular dependency detected and rejected

---

## Phase 6: Runtime Integration (Day 11-13)

### 6.1 Extend `ScopeResolver`

**File**: `rust/src/mcp/scope_resolution.rs`

```rust
impl ScopeResolver {
    /// Resolve scope to candidates (not committed yet)
    pub async fn resolve_scope(
        &self,
        pool: &PgPool,
        group_id: Uuid,
        desc: &str,
        limit: u32,
        mode: ResolutionMode,
    ) -> Result<ScopeCandidates, ScopeError> {
        // 1. Search entity tags within group
        let candidates = search_entity_tags(pool, group_id, desc, limit as i64).await?;
        
        // 2. Check ambiguity
        if candidates.len() > 1 && mode == ResolutionMode::Strict {
            return Err(ScopeError::AmbiguousInStrictMode {
                candidates: candidates.len(),
                desc: desc.to_string(),
            });
        }
        
        Ok(ScopeCandidates {
            desc: desc.to_string(),
            entity_ids: candidates.iter().map(|c| c.entity_id).collect(),
            scores: candidates.iter().map(|c| c.score).collect(),
            resolution_method: "tag_search".to_string(),
        })
    }
    
    /// Commit scope to immutable snapshot
    pub async fn commit_scope(
        &self,
        pool: &PgPool,
        group_id: Uuid,
        candidates: &ScopeCandidates,
        session_id: Option<Uuid>,
    ) -> Result<Uuid, ScopeError> {
        let snapshot_id = Uuid::new_v4();
        
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".scope_snapshots (
                id, group_id, description, selected_entity_ids,
                top_k_candidates, resolution_method, session_id, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'agent')
            "#,
            snapshot_id,
            group_id,
            candidates.desc,
            &candidates.entity_ids,
            serde_json::to_value(&candidates.top_k())?,
            candidates.resolution_method,
            session_id,
        )
        .execute(pool)
        .await?;
        
        Ok(snapshot_id)
    }
    
    /// Expand committed scope to entity list
    pub async fn expand_scope(
        &self,
        pool: &PgPool,
        snapshot_id: Uuid,
    ) -> Result<Vec<Uuid>, ScopeError> {
        let row = sqlx::query!(
            r#"
            SELECT selected_entity_ids 
            FROM "ob-poc".scope_snapshots 
            WHERE id = $1
            "#,
            snapshot_id,
        )
        .fetch_one(pool)
        .await?;
        
        Ok(row.selected_entity_ids)
    }
    
    /// Record resolution event for learning
    pub async fn record_resolution_event(
        &self,
        pool: &PgPool,
        session_id: Option<Uuid>,
        snapshot_id: Option<Uuid>,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<(), ScopeError> {
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".resolution_events (
                session_id, snapshot_id, event_type, payload
            ) VALUES ($1, $2, $3, $4)
            "#,
            session_id,
            snapshot_id,
            event_type,
            payload,
        )
        .execute(pool)
        .await?;
        
        Ok(())
    }
}
```

### 6.2 Wire into Agent Pipeline

**File**: `rust/src/api/agent_service.rs`

```rust
impl AgentService {
    pub async fn process_chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        // EXISTING: Stage 0 scope detection (already implemented)
        let scope_outcome = self.scope_resolver
            .is_scope_phrase(&request.message)
            .then(|| self.scope_resolver.resolve(&self.pool, &request.message))
            .transpose()
            .await?;
        
        // Handle scope anchoring
        if let Some(ScopeResolutionOutcome::Resolved { group_id, group_name, .. }) = &scope_outcome {
            // Update session anchor
            self.update_session_anchor(request.session_id, *group_id, group_name).await?;
            
            // Record event
            self.scope_resolver.record_resolution_event(
                &self.pool,
                Some(request.session_id),
                None,
                "group_anchored",
                json!({ "group_name": group_name }),
            ).await?;
            
            return Ok(ChatResponse {
                message: format!("Session anchored to {} group", group_name),
                scope_set: true,
                ..Default::default()
            });
        }
        
        // Continue with verb discovery...
        // (existing flow)
    }
}
```

---

## Phase 7: Linter Rules (Day 14)

### 7.1 Create Scope Linter Module

**File**: `rust/crates/dsl-core/src/linter/scope_rules.rs`

```rust
use crate::ast::{Expr, Program, ResolutionMode};

pub struct ScopeLinter;

impl ScopeLinter {
    pub fn lint(program: &Program) -> Vec<LintDiagnostic> {
        let mut diagnostics = vec![];
        
        diagnostics.extend(Self::check_s001_limit_required(program));
        diagnostics.extend(Self::check_s002_cross_group(program));
        diagnostics.extend(Self::check_s003_commit_before_use(program));
        diagnostics.extend(Self::check_s006_defined_before_use(program));
        diagnostics.extend(Self::check_s007_no_cycles(program));
        
        diagnostics
    }
    
    /// S001: :limit REQUIRED on scope.resolve/define, max 100
    fn check_s001_limit_required(program: &Program) -> Vec<LintDiagnostic> {
        const MAX_LIMIT: u32 = 100;
        let mut diags = vec![];
        
        for expr in &program.expressions {
            if let Expr::ScopeResolve { limit, span, .. } = expr {
                if *limit > MAX_LIMIT {
                    diags.push(LintDiagnostic {
                        code: "S001",
                        severity: Severity::Error,
                        message: format!("scope.resolve :limit {} exceeds maximum of {}", limit, MAX_LIMIT),
                        span: span.clone(),
                    });
                }
            }
        }
        
        diags
    }
    
    /// S002: Cross-group scope forbidden unless :global true
    fn check_s002_cross_group(program: &Program) -> Vec<LintDiagnostic> {
        // Requires session context - check if group in scope.resolve differs from anchor
        // This is enforced at runtime, emit warning in linter
        vec![]
    }
    
    /// S003: scope.commit REQUIRED before scope consumption in :strict mode
    fn check_s003_commit_before_use(program: &Program) -> Vec<LintDiagnostic> {
        let mut diags = vec![];
        let mut committed_scopes: HashSet<String> = HashSet::new();
        let mut candidate_scopes: HashSet<String> = HashSet::new();
        
        for expr in &program.expressions {
            match expr {
                Expr::ScopeResolve { as_symbol, .. } => {
                    candidate_scopes.insert(as_symbol.clone());
                }
                Expr::ScopeCommit { scope_symbol, as_symbol, span, .. } => {
                    if !candidate_scopes.contains(scope_symbol) {
                        diags.push(LintDiagnostic {
                            code: "S006",
                            severity: Severity::Error,
                            message: format!("scope.commit references undefined scope @{}", scope_symbol),
                            span: span.clone(),
                        });
                    }
                    committed_scopes.insert(as_symbol.clone());
                }
                Expr::VerbCall { args, span, .. } => {
                    // Check for ScopeRef in args
                    for (_, arg_value) in args {
                        if let ArgValue::ScopeRef(sym) = arg_value {
                            if !committed_scopes.contains(sym) {
                                diags.push(LintDiagnostic {
                                    code: "S003",
                                    severity: Severity::Error,
                                    message: format!("scope @{} used before scope.commit", sym),
                                    span: span.clone(),
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        
        diags
    }
    
    /// S006: Scope symbol must be defined before use
    fn check_s006_defined_before_use(program: &Program) -> Vec<LintDiagnostic> {
        // Handled in S003
        vec![]
    }
    
    /// S007: Circular scope dependencies forbidden
    fn check_s007_no_cycles(program: &Program) -> Vec<LintDiagnostic> {
        // Build scope dependency graph and check for cycles
        // Use Tarjan's algorithm or simple DFS
        vec![]
    }
}
```

---

## Phase 8: Testing & Integration (Day 15-17)

### 8.1 Unit Tests

```rust
// tests/scope_ast_tests.rs
#[test]
fn test_scope_resolve_parse() {
    let input = r#"(scope.resolve :desc "Irish funds" :limit 50 :as @s1)"#;
    let ast = parse_program(input).unwrap();
    
    assert_matches!(&ast.expressions[0], Expr::ScopeResolve { 
        desc, limit, as_symbol, .. 
    } => {
        assert_eq!(desc, "Irish funds");
        assert_eq!(*limit, 50);
        assert_eq!(as_symbol, "s1");
    });
}

#[test]
fn test_scope_narrow_parse() {
    let input = r#"(scope.narrow @s1 :filter (has-role "custodian") :as @s2)"#;
    let ast = parse_program(input).unwrap();
    
    assert_matches!(&ast.expressions[0], Expr::ScopeNarrow { 
        scope_symbol, filter, as_symbol, .. 
    } => {
        assert_eq!(scope_symbol, "s1");
        assert_matches!(filter, FilterExpr::HasRole(r) => assert_eq!(r, "custodian"));
        assert_eq!(as_symbol, "s2");
    });
}

#[test]
fn test_scope_ref_in_verb() {
    let input = r#"(ubo.trace-chain :scope @s1)"#;
    let ast = parse_program(input).unwrap();
    
    // Check that :scope @s1 parses to ScopeRef
}
```

### 8.2 Integration Tests

```rust
// tests/scope_integration.rs
#[tokio::test]
#[ignore] // Requires DB
async fn test_full_scope_flow() {
    let pool = test_pool().await;
    let resolver = ScopeResolver::new();
    
    // 1. Resolve scope
    let candidates = resolver.resolve_scope(
        &pool,
        test_group_id(),
        "Irish funds",
        50,
        ResolutionMode::Interactive,
    ).await.unwrap();
    
    assert!(!candidates.entity_ids.is_empty());
    
    // 2. Commit scope
    let snapshot_id = resolver.commit_scope(
        &pool,
        test_group_id(),
        &candidates,
        Some(test_session_id()),
    ).await.unwrap();
    
    // 3. Expand scope
    let entity_ids = resolver.expand_scope(&pool, snapshot_id).await.unwrap();
    
    assert_eq!(entity_ids, candidates.entity_ids);
    
    // 4. Verify immutability
    let result = sqlx::query!(
        r#"UPDATE "ob-poc".scope_snapshots SET description = 'hacked' WHERE id = $1"#,
        snapshot_id
    ).execute(&pool).await;
    
    assert!(result.is_err()); // Trigger should block
}

#[tokio::test]
#[ignore]
async fn test_replay_uses_snapshot() {
    // Verify replay mode uses stored entity_ids, not re-resolution
}
```

### 8.3 Golden Tests

```rust
// tests/golden/scope_expansion.rs
#[test]
fn test_scope_expansion_golden() {
    let input = r#"
        (scope.resolve :desc "Irish funds" :limit 10 :as @s1)
        (scope.commit :scope @s1 :as @s_irish)
        (ubo.trace-chain :scope @s_irish)
    "#;
    
    // Compile with mock scope resolution returning 3 entities
    let ops = compile_with_mock_scope(input, vec![uuid1, uuid2, uuid3]);
    
    // Verify expansion to 3 tuples
    assert_eq!(ops.len(), 5); // resolve + commit + 3 trace-chain
    assert_matches!(&ops[2], Op::TraceChain { entity_id, .. } => assert_eq!(entity_id, &uuid1));
    assert_matches!(&ops[3], Op::TraceChain { entity_id, .. } => assert_eq!(entity_id, &uuid2));
    assert_matches!(&ops[4], Op::TraceChain { entity_id, .. } => assert_eq!(entity_id, &uuid3));
}
```

---

## Files to Create/Modify Summary

### New Files

| File | Purpose | Lines (est) |
|------|---------|-------------|
| `migrations/066_scope_snapshots.sql` | Snapshot table + trigger | 60 |
| `migrations/067_resolution_events.sql` | Learning events + view | 50 |
| `rust/crates/dsl-core/src/linter/scope_rules.rs` | S001-S007 rules | 200 |
| `rust/config/verbs/scope.yaml` | Scope verb definitions | 100 |

### Modified Files

| File | Changes | Lines (est) |
|------|---------|-------------|
| `rust/crates/dsl-core/src/ast.rs` | Add scope nodes, FilterExpr, ResolutionMode | +150 |
| `rust/crates/dsl-core/src/parser.rs` | Parse scope verbs and @symbols | +200 |
| `rust/crates/dsl-core/src/compiler.rs` | Scope ops, symbol tracking, expansion | +250 |
| `rust/crates/dsl-core/src/dag.rs` | Scope dependencies | +80 |
| `rust/crates/dsl-core/src/ops.rs` | Add scope ops | +50 |
| `rust/src/mcp/scope_resolution.rs` | Snapshot creation, expand_scope | +150 |
| `rust/src/api/agent_service.rs` | Wire scope anchor to session | +50 |

**Total New Code**: ~1,340 lines

---

## Acceptance Criteria

| # | Criterion | Test |
|---|-----------|------|
| 1 | **Determinism**: Same program produces identical results on replay | Golden test with fixed snapshot |
| 2 | **Immutability**: `scope_snapshots` rows cannot be updated | Integration test with UPDATE attempt |
| 3 | **Cross-Group Protection**: S002 linter rule fires | Linter test with cross-group scope |
| 4 | **Learning Integration**: Events recorded and queryable | Integration test with event verification |
| 5 | **Pipeline Order**: Parse → Resolve → Lint → DAG → Execute | E2E test with each stage |
| 6 | **Minimum Diff**: Existing machinery reused | Code review - no duplicate search logic |

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| AST changes break existing tests | Run full test suite after each phase |
| Parser ambiguity with `@` symbol | Explicit tokenizer rule for `@identifier` |
| Snapshot table grows unbounded | Add retention policy (keep 30 days) |
| Semantic search performance | Reuse existing pg_trgm + vector indexes |
| Session state complexity | Extend `UnifiedSession`, don't replace |

---

## Timeline Summary

| Phase | Days | Deliverable |
|-------|------|-------------|
| 0: Pre-validation | 1 | Checklist complete |
| 1: Database | 2 | Migrations 066-067 |
| 2: AST | 2 | Scope nodes, FilterExpr |
| 3: Parser | 2 | Scope verb parsing |
| 4: Compiler | 2 | Scope ops, expansion |
| 5: DAG | 1 | Scope dependencies |
| 6: Runtime | 3 | ScopeResolver integration |
| 7: Linter | 1 | S001-S007 rules |
| 8: Testing | 3 | Unit + integration + golden |
| **Total** | **17 days** | Full implementation |

---

## Next Steps

1. Review and approve this plan
2. Create feature branch: `feature/entity-scope-dsl`
3. Execute Phase 0 validation
4. Proceed with Phase 1 migration
