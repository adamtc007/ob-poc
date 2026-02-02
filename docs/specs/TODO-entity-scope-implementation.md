# TODO: Entity Scope DSL Implementation

## Reference Document

`docs/specs/ARCH-PROPOSAL-entity-lineage-and-semantic-scope-v5.md`

---

## Pre-Implementation Validation

> **CRITICAL**: Before writing any code, validate the existing implementations match the spec.

### Checklist

- [ ] Read `rust/src/mcp/scope_resolution.rs` - confirm Stage 0 hard gate exists
- [ ] Read `migrations/052_client_group_entity_context.sql` - confirm slim membership model
- [ ] Read `migrations/055_client_group_research.sql` - confirm typed roles/lineage
- [ ] Read `rust/crates/dsl-core/src/ast.rs` - understand current AST structure
- [ ] Read `rust/crates/dsl-core/src/compiler.rs` - understand compile_to_ops flow
- [ ] Read `rust/crates/dsl-core/src/dag.rs` - understand DAG construction

---

## Phase 1: Database Layer (Migrations)

### 1.1 Create `064_scope_snapshots.sql`

```sql
-- Location: migrations/064_scope_snapshots.sql

CREATE TABLE "ob-poc".scope_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id),
    description TEXT NOT NULL,               -- "the Irish funds"
    selected_entity_ids UUID[] NOT NULL,     -- Ordered array
    top_k_candidates JSONB NOT NULL,         -- [{entity_id, name, score}...]
    resolution_method TEXT NOT NULL,         -- 'exact'|'fuzzy'|'semantic'
    overall_confidence DECIMAL(3,2),         -- 0.00-1.00
    persona TEXT,                            -- 'kyc'|'trading'|etc or NULL
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by TEXT,                         -- session/user
    
    -- CONSTRAINT: Array must be non-empty
    CONSTRAINT chk_ss_nonempty CHECK (array_length(selected_entity_ids, 1) > 0)
);

-- Immutability trigger
CREATE OR REPLACE FUNCTION "ob-poc".prevent_snapshot_update()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'scope_snapshots are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER snapshot_immutable
    BEFORE UPDATE ON "ob-poc".scope_snapshots
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".prevent_snapshot_update();

-- Indexes
CREATE INDEX idx_ss_group ON "ob-poc".scope_snapshots(group_id);
CREATE INDEX idx_ss_created ON "ob-poc".scope_snapshots(created_at DESC);
```

- [ ] Create migration file
- [ ] Test immutability trigger
- [ ] Verify deterministic ordering (score DESC, entity_id ASC)

### 1.2 Create `065_resolution_events.sql`

```sql
-- Location: migrations/065_resolution_events.sql

CREATE TABLE "ob-poc".resolution_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_id UUID REFERENCES "ob-poc".scope_snapshots(id),
    event_type TEXT NOT NULL,                -- 'created'|'selected'|'rejected'|'refined'
    session_id UUID,
    user_id TEXT,
    payload JSONB NOT NULL DEFAULT '{}',     -- Type-specific data
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT chk_re_event_type CHECK (event_type IN ('created', 'selected', 'rejected', 'refined'))
);

CREATE INDEX idx_re_snapshot ON "ob-poc".resolution_events(snapshot_id);
CREATE INDEX idx_re_type ON "ob-poc".resolution_events(event_type);
CREATE INDEX idx_re_session ON "ob-poc".resolution_events(session_id) WHERE session_id IS NOT NULL;
```

- [ ] Create migration file
- [ ] Define payload schemas for each event_type
- [ ] Create view for hit-rate analysis

---

## Phase 2: AST Extensions

### 2.1 Add Scope Nodes to AST

**File**: `rust/crates/dsl-core/src/ast.rs`

```rust
// Add to Expr enum:
ScopeAnchor {
    group: String,  // "allianz" | group_id
},
ScopeResolve {
    desc: String,           // "the Irish funds"
    limit: u32,             // top-k
    mode: ResolutionMode,   // Exact|Fuzzy|Semantic
    as_symbol: String,      // "@s1"
},
ScopeNarrow {
    scope_symbol: String,   // "@s1"
    filter: FilterExpr,
    as_symbol: String,      // "@s2"
},
ScopeCommit {
    scope_symbol: String,   // "@s1"
    as_symbol: String,      // "@s_committed"
},
ScopeRefresh {
    old_symbol: String,     // "@s_committed"
    as_symbol: String,      // "@s3"
},

// Add to ArgValue enum:
ScopeRef(String),  // "@s1"

// Add new enum:
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionMode {
    Exact,
    Fuzzy,
    Semantic,
}
```

- [ ] Add `ResolutionMode` enum
- [ ] Add scope expression variants to `Expr`
- [ ] Add `ScopeRef` to `ArgValue`
- [ ] Update `Display` implementations
- [ ] Add tests for new variants

### 2.2 Add FilterExpr for scope.narrow

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum FilterExpr {
    HasRole(String),              // has-role "custodian"
    HasType(String),              // has-type "fund"
    InJurisdiction(String),       // in-jurisdiction "IE"
    And(Box<FilterExpr>, Box<FilterExpr>),
    Or(Box<FilterExpr>, Box<FilterExpr>),
    Not(Box<FilterExpr>),
}
```

- [ ] Add `FilterExpr` enum
- [ ] Add parser support for filter expressions
- [ ] Add tests

---

## Phase 3: Parser Extensions

### 3.1 Add Scope Verb Parsing

**File**: `rust/crates/dsl-core/src/parser.rs`

Add parsing for:
```lisp
(scope.anchor :group "allianz")
(scope.resolve :desc "the Irish funds" :limit 5 :mode :fuzzy :as @s1)
(scope.narrow :scope @s1 :filter (has-role "custodian") :as @s2)
(scope.commit :scope @s1 :as @s_committed)
(scope.refresh :scope @s_committed :as @s3)
```

- [ ] Add `parse_scope_verb()` function
- [ ] Add filter expression parser
- [ ] Handle `@symbol` syntax for ScopeRef
- [ ] Add tests for all scope verb forms

### 3.2 Add Scope Argument Parsing

When parsing verb arguments:
```lisp
(kyc.refresh :scope @s1)
```

The `:scope @s1` should parse to `ScopeRef("s1")`.

- [ ] Update argument parser to recognize `@` prefix
- [ ] Validate scope symbols exist before use
- [ ] Add tests

---

## Phase 4: Compiler Extensions

### 4.1 Add Scope Ops

**File**: `rust/crates/dsl-core/src/ops.rs`

```rust
// Add to Op enum:
ResolveScope {
    descriptor: ScopeDescriptor,
    output_symbol: String,
},
CommitScope {
    candidate_symbol: String,
    snapshot_id: Uuid,
},
ExpandScope {
    snapshot_id: Uuid,
    output_entity_ids: Vec<Uuid>,
},
```

- [ ] Add `ScopeDescriptor` struct
- [ ] Add scope ops to `Op` enum
- [ ] Add compilation logic in `compiler.rs`

### 4.2 Symbol Binding in Compilation Context

```rust
struct CompilationContext {
    // ... existing fields
    scope_symbols: HashMap<String, ScopeBinding>,
}

enum ScopeBinding {
    Candidates(Vec<EntityCandidate>),  // Pre-commit
    Committed(Uuid),                    // snapshot_id
}
```

- [ ] Add scope symbol tracking
- [ ] Validate scope symbol references
- [ ] Error on undefined scope symbols

---

## Phase 5: DAG Extensions

### 5.1 Scope Dependencies

**File**: `rust/crates/dsl-core/src/dag.rs`

Rules:
1. `scope.resolve @s1` → produces `@s1`
2. `scope.narrow @s1 :as @s2` → consumes `@s1`, produces `@s2`
3. `scope.commit @s1` → consumes `@s1`, produces snapshot
4. `(verb :scope @s1)` → consumes committed scope

- [ ] Track scope symbol producers/consumers
- [ ] Add edges for scope dependencies
- [ ] Validate no cycles in scope graph
- [ ] Test DAG ordering with scope ops

---

## Phase 6: Runtime Integration

### 6.1 Scope Resolution at Runtime

**File**: `rust/src/mcp/scope_resolution.rs`

Extend existing `search_entities_in_scope()` to support snapshot creation:

```rust
pub async fn resolve_scope_to_snapshot(
    pool: &PgPool,
    scope: &ScopeContext,
    descriptor: &ScopeDescriptor,
) -> Result<Uuid> {
    // 1. Call existing search_entity_tags / search_entity_tags_semantic
    // 2. Create scope_snapshots row
    // 3. Return snapshot_id
}

pub async fn expand_scope(
    pool: &PgPool,
    snapshot_id: Uuid,
) -> Result<Vec<Uuid>> {
    // Return selected_entity_ids from snapshot (deterministic)
}
```

- [ ] Add `resolve_scope_to_snapshot()` function
- [ ] Add `expand_scope()` function
- [ ] Integrate with existing entity search
- [ ] Add confidence calculation

### 6.2 Replay Semantics

When replaying a program with committed scopes:
- Use stored `snapshot_id` from program
- Call `expand_scope()` instead of re-resolving
- Never hit Candle/DB for resolution during replay

- [ ] Add `replay_mode` flag to execution context
- [ ] Skip resolution in replay mode
- [ ] Validate snapshot exists

---

## Phase 7: Linter Integration

### 7.1 Implement Scope Linter Rules

**Location**: New file `rust/crates/dsl-core/src/linter/scope_rules.rs`

| Rule | Description |
|------|-------------|
| S001 | Scope must be set before entity-dependent verbs |
| S002 | Cross-group scope access forbidden (machine-enforced) |
| S003 | Scope symbol must be defined before use |
| S004 | Scope must be committed before verb execution |
| S005 | Re-resolve only allowed in interactive mode |
| S006 | Semantic resolution requires active Candle embeddings |
| S007 | Scope refresh must reference committed scope |

- [ ] Create `scope_rules.rs` module
- [ ] Implement S001-S007 rules
- [ ] Add to linter pipeline
- [ ] Add tests for each rule

### 7.2 Cross-Group Protection (S002)

```rust
fn check_cross_group_access(
    program: &Program,
    current_group: Option<Uuid>,
) -> Vec<LintError> {
    // For each scope.resolve:
    // - Extract group reference
    // - Compare to current session group
    // - Error if different and not :global flag
}
```

- [ ] Implement cross-group check
- [ ] Support `:global` flag for explicit cross-group
- [ ] Add test for S002 violation

---

## Phase 8: Learning Loop Integration

### 8.1 Record Resolution Events

Integrate with existing `ScopeResolver::record_selection()`:

```rust
// When user confirms scope resolution:
record_resolution_event(
    snapshot_id,
    "selected",
    session_id,
    json!({ "selected_from_position": 0 })
);

// When user rejects and refines:
record_resolution_event(
    snapshot_id,
    "rejected",
    session_id,
    json!({ "reason": "wrong jurisdiction" })
);
```

- [ ] Add event recording to scope resolution flow
- [ ] Integrate with existing alias learning
- [ ] Add analytics queries

### 8.2 Hit-Rate Analysis

```sql
CREATE VIEW "ob-poc".v_scope_resolution_stats AS
SELECT
    DATE_TRUNC('day', re.created_at) as day,
    COUNT(*) FILTER (WHERE re.event_type = 'created') as resolutions,
    COUNT(*) FILTER (WHERE re.event_type = 'selected') as selections,
    COUNT(*) FILTER (WHERE re.event_type = 'rejected') as rejections,
    ROUND(
        COUNT(*) FILTER (WHERE re.event_type = 'selected')::NUMERIC /
        NULLIF(COUNT(*) FILTER (WHERE re.event_type = 'created'), 0),
        2
    ) as hit_rate
FROM "ob-poc".resolution_events re
GROUP BY DATE_TRUNC('day', re.created_at)
ORDER BY day DESC;
```

- [ ] Create stats view
- [ ] Add daily hit-rate tracking
- [ ] Alert on hit-rate degradation

---

## Testing Requirements

### Unit Tests

- [ ] AST: New node types serialize/deserialize correctly
- [ ] Parser: All scope verb forms parse correctly
- [ ] Compiler: Scope ops generated correctly
- [ ] DAG: Scope dependencies ordered correctly
- [ ] Linter: All S00X rules fire on violations

### Integration Tests

- [ ] Full scope resolution flow (resolve → narrow → commit → use)
- [ ] Replay with committed scope (no re-resolution)
- [ ] Cross-group protection (should fail)
- [ ] Learning event recording
- [ ] Snapshot immutability

### Golden Tests

- [ ] Sample program with scope ops compiles to expected ops
- [ ] Sample program replays identically
- [ ] Deterministic ordering across runs

---

## Files to Modify

| File | Changes |
|------|---------|
| `rust/crates/dsl-core/src/ast.rs` | Add scope nodes, ResolutionMode, FilterExpr |
| `rust/crates/dsl-core/src/parser.rs` | Parse scope verbs and @symbols |
| `rust/crates/dsl-core/src/compiler.rs` | Compile scope ops, track symbols |
| `rust/crates/dsl-core/src/dag.rs` | Scope dependencies |
| `rust/crates/dsl-core/src/ops.rs` | Add scope ops |
| `rust/src/mcp/scope_resolution.rs` | Snapshot creation, expand_scope |
| `migrations/064_scope_snapshots.sql` | NEW |
| `migrations/065_resolution_events.sql` | NEW |
| `rust/crates/dsl-core/src/linter/mod.rs` | Add scope rules module |
| `rust/crates/dsl-core/src/linter/scope_rules.rs` | NEW |

---

## Acceptance Criteria

1. **Determinism**: Same program with committed scope produces identical results on replay
2. **Immutability**: `scope_snapshots` rows cannot be updated after creation
3. **Cross-Group Protection**: S002 linter rule prevents unauthorized scope access
4. **Learning Integration**: Resolution events recorded and queryable
5. **Pipeline Order**: Parse → Resolve → Lint → DAG → Execute enforced for scope ops
6. **Minimum Diff**: Existing `scope_resolution.rs` and search functions reused

---

## Notes

- **Do NOT duplicate** existing scope phrase detection logic
- **Do NOT create** parallel attribute tracking (use `attribute_values_typed`)
- **Do NOT bypass** existing Stage 0 hard gate
- Reuse `search_entity_tags()` and `search_entity_tags_semantic()` for resolution
- Test with existing client groups and entity tags before creating new test data
