# Tuple → Struct Refactoring Task List

> **Goal:** Replace anonymous tuples with named structs for better code readability, maintainability, and IDE support.
> **Pattern:** Use `#[derive(Debug, sqlx::FromRow)]` for DB query results.

---

## Priority 1: Large Tuples (6+ fields) - High Impact

### 1.1 `request_ops.rs` - DocumentSubject
**Location:** `rust/src/domain_ops/request_ops.rs:1426`
**Current:**
```rust
async fn resolve_document_subject(...) -> Result<(
    String,           // subject_type
    Uuid,             // subject_id
    Option<Uuid>,     // workstream_id
    Option<Uuid>,     // case_id
    Option<Uuid>,     // cbu_id
    Option<Uuid>,     // entity_id
)>
```
**Refactor to:**
```rust
#[derive(Debug, Default)]
pub struct DocumentSubject {
    pub subject_type: String,
    pub subject_id: Uuid,
    pub workstream_id: Option<Uuid>,
    pub case_id: Option<Uuid>,
    pub cbu_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
}
```
**Call sites:** Lines 983, 1181

---

### 1.2 `board_control_rules.rs` - BoardControllerRow
**Location:** `rust/src/services/board_control_rules.rs:545`
**Current:**
```rust
sqlx::query_as::<_, (Uuid, Uuid, Option<Uuid>, Option<String>, String, String, Decimal, NaiveDate, Value)>
```
**Refactor to:**
```rust
#[derive(Debug, sqlx::FromRow)]
struct BoardControllerRow {
    id: Uuid,
    cbu_id: Uuid,
    controller_entity_id: Option<Uuid>,
    controller_name: Option<String>,
    method: String,
    confidence: String,
    score: rust_decimal::Decimal,
    as_of: NaiveDate,
    explanation: serde_json::Value,
}
```

---

### 1.3 `cbu_entity_roles_service.rs` - RoleAssignment
**Location:** `rust/src/database/cbu_entity_roles_service.rs:156, 196`
**Current:**
```rust
sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, Uuid, String)>
```
**Refactor to:**
```rust
#[derive(Debug, sqlx::FromRow)]
struct RoleAssignmentRow {
    assignment_id: Uuid,
    cbu_id: Uuid,
    entity_id: Uuid,
    role_name: String,
    assigned_by: Uuid,
    status: String,
}
```

---

### 1.4 `control_routes.rs` - ControlAnchor
**Location:** `rust/src/api/control_routes.rs:167`
**Current:**
```rust
sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, Option<String>, Option<String>)>
```
**Refactor to:**
```rust
#[derive(Debug, sqlx::FromRow)]
struct ControlAnchorRow {
    anchor_id: Uuid,
    entity_id: Uuid,
    controlled_by: Uuid,
    relationship_type: String,
    label: Option<String>,
    notes: Option<String>,
}
```

---

## Priority 2: Medium Tuples (4-5 fields) - Readability

### 2.1 `verb_service.rs` - VerbMatchResult
**Location:** `rust/src/database/verb_service.rs:99, 136`
**Current:**
```rust
sqlx::query_as::<_, (String, String, f32, f64)>
```
**Refactor to:**
```rust
#[derive(Debug, sqlx::FromRow)]
pub struct VerbMatchRow {
    pub verb_name: String,
    pub matched_phrase: String,
    pub priority: f32,
    pub distance: f64,
}
```
**Call sites:** Lines 99, 136

---

### 2.2 `verb_service.rs` - SemanticMatch
**Location:** `rust/src/database/verb_service.rs:250`
**Current:**
```rust
sqlx::query_as::<_, (String, String, f64, Option<String>)>
```
**Refactor to:**
```rust
#[derive(Debug, sqlx::FromRow)]
pub struct SemanticMatchRow {
    pub verb_name: String,
    pub phrase: String,
    pub similarity: f64,
    pub description: Option<String>,
}
```

---

### 2.3 `idempotency.rs` - IdempotencyCheck
**Location:** `rust/src/dsl_v2/idempotency.rs:229, 448`
**Current:**
```rust
sqlx::query_as::<_, (String, Option<Uuid>, Option<JsonValue>, Option<i64>)>
```
**Refactor to:**
```rust
#[derive(Debug, sqlx::FromRow)]
struct IdempotencyCheckRow {
    status: String,
    entity_id: Option<Uuid>,
    result_payload: Option<serde_json::Value>,
    version: Option<i64>,
}
```

---

### 2.4 `control_routes.rs` - EntityInfo
**Location:** `rust/src/api/control_routes.rs:351`
**Current:**
```rust
sqlx::query_as::<_, (Uuid, String, String, Option<String>)>
```
**Refactor to:**
```rust
#[derive(Debug, sqlx::FromRow)]
struct EntityInfoRow {
    entity_id: Uuid,
    name: String,
    entity_type: String,
    jurisdiction: Option<String>,
}
```

---

### 2.5 `universe_routes.rs` - UniverseEntity
**Location:** `rust/src/api/universe_routes.rs:586`
**Current:**
```rust
sqlx::query_as::<_, (Uuid, String, String, Option<String>)>
```
**Refactor to:**
```rust
#[derive(Debug, sqlx::FromRow)]
struct UniverseEntityRow {
    entity_id: Uuid,
    name: String,
    entity_type: String,
    jurisdiction: Option<String>,
}
```

---

### 2.6 `mcp/handlers/core.rs` - LearningCandidate
**Location:** `rust/src/mcp/handlers/core.rs:1794, 1887`
**Current:**
```rust
sqlx::query_as::<_, (i64, String, String, String)>
```
**Refactor to:**
```rust
#[derive(Debug, sqlx::FromRow)]
struct LearningCandidateRow {
    id: i64,
    phrase: String,
    verb: String,
    status: String,
}
```

---

## Priority 3: Function Return Tuples

### 3.1 `session.rs` - StatusCounts
**Location:** `rust/src/api/session.rs:1604, 1826`
**Current:**
```rust
pub fn count_by_status(&self) -> (usize, usize, usize, usize)
pub fn progress(&self) -> (usize, usize, usize, usize)
```
**Refactor to:**
```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct StatusCounts {
    pub pending: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub failed: usize,
}
```

---

### 3.2 `idempotency.rs` - ViewStateSnapshot  
**Location:** `rust/src/dsl_v2/idempotency.rs:392`
**Current:**
```rust
let (view_taxonomy, view_selection, view_refinements, view_stack_depth, view_snapshot) = ...
```
**Refactor to:**
```rust
#[derive(Debug, Default)]
struct ViewStateSnapshot {
    taxonomy: Option<String>,
    selection: Option<String>,
    refinements: Option<String>,
    stack_depth: Option<i32>,
    snapshot: Option<serde_json::Value>,
}
```

---

### 3.3 `capital_ops.rs` - ShareCapitalSnapshot
**Location:** `rust/src/domain_ops/capital_ops.rs:848`
**Current:**
```rust
Some((_, authorized, issued, outstanding, treasury, votes, economic, date)) => { ... }
```
**Refactor to:**
```rust
#[derive(Debug, sqlx::FromRow)]
struct ShareCapitalRow {
    id: Uuid,
    authorized: Decimal,
    issued: Decimal,
    outstanding: Decimal,
    treasury: Decimal,
    votes_per_share: Decimal,
    economic_per_share: Decimal,
    as_of_date: NaiveDate,
}
```

---

## Priority 4: Small Tuples (Keep or Convert)

These are borderline - convert if used in multiple places or if field meaning is unclear:

| File | Line | Tuple | Decision |
|------|------|-------|----------|
| `document_service.rs` | 277 | `(Uuid, JsonValue)` | Keep - obvious meaning |
| `dsl_repository.rs` | 381 | `(String, i32)` | Keep - small, local |
| `decay.rs` | 68, 143 | `(f32,)` | Keep - single value |
| `execution_audit.rs` | 306 | `(String, i64, i64)` | Convert if reused |

---

## Implementation Order

1. **Week 1:** Priority 1 items (6+ field tuples)
   - [ ] `DocumentSubject` in request_ops.rs
   - [ ] `BoardControllerRow` in board_control_rules.rs
   - [ ] `RoleAssignmentRow` in cbu_entity_roles_service.rs
   - [ ] `ControlAnchorRow` in control_routes.rs

2. **Week 2:** Priority 2 items (verb_service, idempotency)
   - [ ] `VerbMatchRow` and `SemanticMatchRow` in verb_service.rs
   - [ ] `IdempotencyCheckRow` in idempotency.rs
   - [ ] `EntityInfoRow` and `UniverseEntityRow` in routes

3. **Week 3:** Priority 3 items (function returns)
   - [ ] `StatusCounts` in session.rs
   - [ ] `ViewStateSnapshot` in idempotency.rs
   - [ ] `ShareCapitalRow` in capital_ops.rs

---

## Refactoring Template

For each conversion:

```rust
// BEFORE
let row = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
    "SELECT id, name, description FROM table WHERE id = $1"
)
.bind(id)
.fetch_optional(&pool)
.await?;

if let Some((id, name, desc)) = row {
    // use id, name, desc
}

// AFTER
#[derive(Debug, sqlx::FromRow)]
struct MyRow {
    id: Uuid,
    name: String,
    description: Option<String>,
}

let row: Option<MyRow> = sqlx::query_as(
    "SELECT id, name, description FROM table WHERE id = $1"
)
.bind(id)
.fetch_optional(&pool)
.await?;

if let Some(r) = row {
    // use r.id, r.name, r.description
}
```

---

## Testing

After each refactoring:
1. `cargo build` - Compiler catches type mismatches
2. `cargo test` - Run unit tests
3. `cargo clippy` - Check for new warnings

---

## Notes

- Structs should be defined near their use (same module) unless shared
- Use `pub` only if exported outside the module
- Consider `#[derive(Clone)]` if the struct needs to be copied
- For API responses, add `#[derive(Serialize)]` as needed
