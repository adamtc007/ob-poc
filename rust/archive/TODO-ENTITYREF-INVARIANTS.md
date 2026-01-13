# EntityRef Invariants: Gap Analysis & Remediation

## Current State Summary

### ✅ What Already Exists

**AST Structure** (`rust/crates/dsl-core/src/ast.rs`):
```rust
pub enum AstNode {
    EntityRef {
        entity_type: String,      // ✅ From YAML lookup.entity_type
        search_column: String,    // ✅ From YAML lookup.search_key.primary
        value: String,            // ✅ User input
        resolved_key: Option<String>, // ✅ None = unresolved, Some = resolved
        span: Span,               // ✅ Source location
    },
    // ...
}
```

**Search Key Config** (`rust/crates/dsl-core/src/config/types.rs`):
```rust
pub enum SearchKeyConfig {
    Simple(String),                    // ✅ Backwards compatible
    Composite(CompositeSearchKey),     // ✅ S-expression with discriminators
}

pub struct CompositeSearchKey {
    pub primary: String,
    pub discriminators: Vec<SearchDiscriminator>,
    pub resolution_tiers: Vec<ResolutionTier>,
    pub min_confidence: f32,
}
```

**Pipeline**:
1. ✅ Parser → Raw AST (Literals only)
2. ✅ Enrichment → EntityRef where lookup config exists
3. ✅ Validator → Resolves via EntityGateway
4. ✅ Executor

---

## Gap Analysis: Invariants vs Implementation

### Invariant 1: EntityRef is a typed node in the AST
**Status: ✅ DONE**

The AST has `AstNode::EntityRef { entity_type, keys[], where[], pk? }` structure.

---

### Invariant 2: Resolution is location-addressed, never text-matched
**Status: ❌ MISSING**

**Current**: Resolution uses `(entity_type, value)` as key, not `(stmt_idx, arg_name)`.

**Evidence** (from `semantic_validator.rs`):
```rust
// Batch resolve uses (RefType, value) as cache key
pub type RefCache = HashMap<(RefType, String), ResolveResult>;
```

**Risk**: Two identical "John Smith" strings in different args resolve to same entity.

**Fix Needed**: Add `RefLocation` to EntityRef or use span as location key.

---

### Invariant 3: "Undefined ref UUID" is first-class state
**Status: ✅ DONE**

`resolved_key: Option<String>` - None means unresolved, Some means resolved.

**Enhancement Needed**: Add generation tracking for staleness detection.

---

### Invariant 4: Keys are structured, not ad-hoc strings
**Status: ⚠️ PARTIAL**

`SearchKeyConfig` has `Simple` vs `Composite` enum, but:
- No strict enum for allowed search keys
- `search_column` in EntityRef is just `String`, not validated

**Fix Needed**: Create `SearchKey` enum with known values + custom escape hatch.

---

### Invariant 5: Resolution is pure; execution depends on resolved state
**Status: ⚠️ PARTIAL**

**Current**: Executor receives resolved AST, but no explicit gate.

**Fix Needed**: Add pre-flight check in executor:
```rust
fn validate_all_resolved(program: &Program) -> Result<(), ExecutionError> {
    for entity_ref in program.entity_refs() {
        if entity_ref.resolved_key.is_none() {
            return Err(ExecutionError::UnresolvedRef { ... });
        }
    }
    Ok(())
}
```

---

### Invariant 6: Every verb argument that can take an entity accepts EntityRef
**Status: ✅ DONE**

Enrichment pass converts strings to EntityRef based on YAML `lookup` config.

---

### Invariant 7: Search behavior is configurable via keys + where
**Status: ⚠️ PARTIAL**

**Have**:
- `search_key` s-expression in YAML
- `CompositeSearchKey` with discriminators

**Missing**:
- Discriminators not flowing through to EntityGateway search
- `resolution_tiers` not being used in resolver
- `min_confidence` not being applied

**Evidence** (from earlier analysis):
```rust
// In resolution_service.rs
discriminators: HashMap::new(), // Always empty!
```

---

### Invariant 8: Resolver emits an "explain blob" even on auto-resolve
**Status: ❌ MISSING**

No `ExplainRef` structure exists. Resolution is opaque.

**Fix Needed**:
```rust
pub struct ResolutionExplain {
    pub ref_id: String,                    // Location: "1:entity-id"
    pub search_key_used: String,           // "name" or s-expr
    pub candidates_count: usize,
    pub winner_score: f32,
    pub winner_uuid: String,
    pub generation: u64,                   // Index generation at resolution time
    pub resolution_tier: ResolutionTier,   // Which tier matched
    pub discriminators_applied: Vec<String>,
}
```

---

## Supplementary TODO: Invariant Enforcement

### Fix A: Location-Based Resolution Key

**File**: `rust/crates/dsl-core/src/ast.rs`

Add location to EntityRef:
```rust
pub enum AstNode {
    EntityRef {
        entity_type: String,
        search_column: String,
        value: String,
        resolved_key: Option<String>,
        span: Span,
        
        // NEW: Stable location for commit targeting
        /// Location: "{stmt_index}:{arg_name}"
        ref_id: String,
    },
    // ...
}
```

**File**: `rust/src/dsl_v2/enrichment.rs`

Generate ref_id during enrichment:
```rust
fn enrich_argument(&mut self, arg: Argument, verb_args: Option<&Vec<RuntimeArg>>, stmt_index: usize) -> Argument {
    // ...
    AstNode::EntityRef {
        entity_type,
        search_column: config.search_key.primary_column().to_string(),
        value: s,
        resolved_key: None,
        span: arg_span,
        ref_id: format!("{}:{}", stmt_index, arg.key),  // NEW
    }
}
```

**File**: `rust/src/dsl_v2/semantic_validator.rs`

Update cache key to use ref_id:
```rust
// OLD
pub type RefCache = HashMap<(RefType, String), ResolveResult>;

// NEW
pub type RefCache = HashMap<String, ResolveResult>;  // keyed by ref_id
```

---

### Fix B: SearchKey Enum

**File**: `rust/crates/dsl-core/src/config/types.rs`

Add well-known search keys:
```rust
/// Well-known search key types
/// Prevents ad-hoc string proliferation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchKeyType {
    // Exact match keys
    Id,              // UUID primary key
    Lei,             // Legal Entity Identifier
    Bic,             // Bank Identifier Code
    Isin,            // International Securities Identification Number
    Code,            // Generic code (role, jurisdiction)
    
    // Fuzzy match keys
    Name,            // Entity name
    SearchName,      // Normalized search name
    Alias,           // Alternative names
    
    // Custom escape hatch
    Custom(String),
}

impl SearchKeyType {
    /// Get the database column name
    pub fn column_name(&self) -> &str {
        match self {
            SearchKeyType::Id => "id",
            SearchKeyType::Lei => "lei",
            SearchKeyType::Bic => "bic",
            SearchKeyType::Isin => "isin",
            SearchKeyType::Code => "code",
            SearchKeyType::Name => "name",
            SearchKeyType::SearchName => "search_name",
            SearchKeyType::Alias => "alias",
            SearchKeyType::Custom(col) => col,
        }
    }
    
    /// Is this an exact-match key?
    pub fn is_exact(&self) -> bool {
        matches!(self, 
            SearchKeyType::Id | 
            SearchKeyType::Lei | 
            SearchKeyType::Bic | 
            SearchKeyType::Isin | 
            SearchKeyType::Code
        )
    }
}
```

---

### Fix C: Resolution Explain Blob

**File**: `rust/crates/dsl-core/src/ast.rs` (or new file)

Add explain structure:
```rust
/// Audit trail for how a reference was resolved
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionExplain {
    /// Location in AST: "{stmt_index}:{arg_name}"
    pub ref_id: String,
    
    /// Entity type searched
    pub entity_type: String,
    
    /// Search key configuration used (as s-expr string)
    pub search_key_expr: String,
    
    /// Input value that was searched
    pub input_value: String,
    
    /// How many candidates the search returned
    pub candidate_count: usize,
    
    /// The winning candidate's score
    pub winner_score: f32,
    
    /// The resolved key (UUID or code)
    pub resolved_key: String,
    
    /// Display name of resolved entity
    pub resolved_display: String,
    
    /// Which resolution tier matched (Exact, Composite, Contextual, Fuzzy)
    pub resolution_tier: ResolutionTier,
    
    /// Discriminators that were applied (if any)
    pub discriminators_applied: Vec<String>,
    
    /// Index generation at time of resolution
    pub index_generation: u64,
    
    /// Timestamp
    pub resolved_at: chrono::DateTime<chrono::Utc>,
}

impl ResolutionExplain {
    pub fn is_stale(&self, current_generation: u64) -> bool {
        self.index_generation < current_generation
    }
}
```

**File**: `rust/crates/dsl-core/src/ast.rs`

Add explain to EntityRef:
```rust
pub enum AstNode {
    EntityRef {
        entity_type: String,
        search_column: String,
        value: String,
        resolved_key: Option<String>,
        span: Span,
        ref_id: String,
        
        // NEW: Resolution audit trail
        explain: Option<ResolutionExplain>,
    },
    // ...
}
```

---

### Fix D: Pre-Flight Resolution Check

**File**: `rust/src/dsl_v2/executor.rs`

Add resolution gate:
```rust
/// Validate all EntityRefs are resolved before execution
fn validate_all_resolved(program: &Program) -> Result<(), ExecutionError> {
    let unresolved: Vec<_> = program.statements.iter()
        .filter_map(|stmt| match stmt {
            Statement::VerbCall(vc) => Some(vc),
            _ => None,
        })
        .flat_map(|vc| &vc.arguments)
        .filter_map(|arg| match &arg.value {
            AstNode::EntityRef { resolved_key: None, ref_id, value, .. } => {
                Some((ref_id.clone(), value.clone()))
            }
            _ => None,
        })
        .collect();
    
    if !unresolved.is_empty() {
        return Err(ExecutionError::UnresolvedRefs {
            refs: unresolved,
            message: format!(
                "Cannot execute: {} unresolved entity reference(s). \
                 Use disambiguation UI to resolve before execution.",
                unresolved.len()
            ),
        });
    }
    
    Ok(())
}

impl Executor {
    pub async fn execute(&mut self, program: Program) -> Result<ExecutionResult, ExecutionError> {
        // PRE-FLIGHT: Ensure all refs resolved
        validate_all_resolved(&program)?;
        
        // ... rest of execution
    }
}
```

---

### Fix E: Plumb Discriminators End-to-End

**File**: `rust/src/dsl_v2/semantic_validator.rs`

Extract discriminators from verb call context:
```rust
fn extract_discriminators(
    vc: &VerbCall,
    arg: &Argument,
    search_key: &SearchKeyConfig,
) -> HashMap<String, String> {
    let mut discriminators = HashMap::new();
    
    if let SearchKeyConfig::Composite(composite) = search_key {
        for disc in &composite.discriminators {
            // Check if sibling arg provides this discriminator
            if let Some(from_arg) = &disc.from_arg {
                if let Some(sibling) = vc.get_value(from_arg) {
                    if let Some(value) = extract_string_value(sibling) {
                        discriminators.insert(disc.field.clone(), value);
                    }
                }
            }
        }
    }
    
    discriminators
}
```

**File**: `rust/src/dsl_v2/gateway_resolver.rs`

Pass discriminators to gateway:
```rust
pub async fn resolve_with_discriminators(
    &mut self,
    entity_type: &str,
    search_key: &SearchKeyConfig,
    value: &str,
    discriminators: HashMap<String, String>,
) -> ResolveResult {
    let request = SearchRequest {
        nickname: entity_type.to_string(),
        values: vec![value.to_string()],
        search_key: Some(search_key.primary_column().to_string()),
        mode: SearchMode::Fuzzy as i32,
        limit: Some(10),
        discriminators,  // NOW POPULATED!
    };
    
    // ... rest of resolution
}
```

---

## Property Test Suite

**File**: `rust/crates/dsl-core/tests/entity_ref_properties.rs`

```rust
use proptest::prelude::*;
use dsl_core::ast::*;
use dsl_core::parser::parse_program;

// Property 1: Parse round-trip preserves EntityRef structure
proptest! {
    #[test]
    fn parse_roundtrip_preserves_entity_ref(
        entity_type in "[a-z_]+",
        value in "[A-Za-z ]+",
    ) {
        let source = format!(
            r#"(entity.create :name "{}" :type "{}")"#,
            value, entity_type
        );
        
        let ast = parse_program(&source).unwrap();
        let rendered = ast.to_dsl_string();
        let reparsed = parse_program(&rendered).unwrap();
        
        assert_eq!(ast, reparsed);
    }
}

// Property 2: Commit correctness - resolving one ref doesn't affect others
#[test]
fn commit_only_affects_targeted_ref() {
    let mut program = parse_program(r#"
        (entity.create :name "John Smith" :as @p1)
        (entity.create :name "John Smith" :as @p2)
    "#).unwrap();
    
    // Enrich to create EntityRefs
    let enriched = enrich_program(program, &registry());
    
    // Resolve only the first "John Smith" (ref_id = "0:name")
    let resolved = resolve_single_ref(&enriched, "0:name", "uuid-1234");
    
    // Second "John Smith" (ref_id = "1:name") should still be unresolved
    let ref_1 = get_entity_ref(&resolved, "1:name");
    assert!(ref_1.resolved_key.is_none());
}

// Property 3: Determinism
#[test]
fn same_inputs_same_output() {
    let program = parse_and_enrich("(entity.create :name \"Test Corp\")");
    
    let result1 = resolve_all(&program, generation: 1);
    let result2 = resolve_all(&program, generation: 1);
    
    assert_eq!(result1, result2);
}

// Property 4: Unresolved refs block execution
#[test]
fn unresolved_refs_block_execution() {
    let program = parse_and_enrich(r#"
        (entity.create :name "Unresolved Person")
    "#);
    
    // Don't resolve
    let result = executor.execute(program).await;
    
    assert!(matches!(result, Err(ExecutionError::UnresolvedRefs { .. })));
}

// Property 5: Config-only changes for new search keys
#[test]
fn new_search_key_only_needs_config() {
    // Adding "registration_number" search key should only require:
    // 1. YAML config change
    // 2. Gateway index config
    // NOT: code changes in verbs
    
    // This test verifies the resolver generically handles any search_key
    let config = SearchKeyConfig::parse("registration_number").unwrap();
    assert!(resolver.supports_search_key(&config));
}
```

---

## Definition of Done Checklist

| # | Invariant | Test | Status |
|---|-----------|------|--------|
| 1 | Parse round-trip preserves EntityRef | `parse_roundtrip_preserves_entity_ref` | ⬜ |
| 2 | Commit only affects targeted ref_id | `commit_only_affects_targeted_ref` | ⬜ |
| 3 | Same inputs + generation → same UUID | `same_inputs_same_output` | ⬜ |
| 4 | Unresolved refs block execution | `unresolved_refs_block_execution` | ⬜ |
| 5 | New search key = config only | `new_search_key_only_needs_config` | ⬜ |
| 6 | Every resolution has explain blob | Manual verification | ⬜ |

---

## Files to Modify

| File | Changes |
|------|---------|
| `rust/crates/dsl-core/src/ast.rs` | Add `ref_id`, `explain` to EntityRef |
| `rust/crates/dsl-core/src/config/types.rs` | Add `SearchKeyType` enum |
| `rust/src/dsl_v2/enrichment.rs` | Generate `ref_id` during enrichment |
| `rust/src/dsl_v2/semantic_validator.rs` | Use `ref_id` as cache key, extract discriminators |
| `rust/src/dsl_v2/gateway_resolver.rs` | Accept discriminators, return explain blob |
| `rust/src/dsl_v2/executor.rs` | Add pre-flight resolution check |
| `rust/crates/dsl-core/tests/entity_ref_properties.rs` | NEW: Property tests |

---

## Priority Order

1. **Fix A: ref_id** - Prevents silent data corruption (same as TODO-ENTITY-RESOLUTION-FIXES.md Fix 1)
2. **Fix D: Pre-flight check** - Prevents half-resolved execution
3. **Fix E: Discriminators** - Enables proper disambiguation
4. **Fix F: Key strength ordering** - Strongest keys first (id/lei before name)
5. **Fix G: Scoped selection validation** - Enforce tenant/CBU scope on lookup-by-id
6. **Fix C: Explain blob** - Auditability (optional telemetry, not correctness gate)
7. **Fix B: SearchKeyType enum** - Prevents config drift
8. **Property tests** - Regression protection

---

## Critical Design Refinements

### Refinement 1: Generation Tracking is Optional Telemetry

Generation tracking is useful for debugging stale results, but **not required for correctness**.

The real correctness guards are:
- Location-based commit (ref_id)
- Fresh reader after refresh (no cached Searcher)
- Pre-flight resolution check

**Do**: Include generation in `ResolutionExplain` for debugging.
**Don't**: Make it a hard dependency or blocking check.

```rust
pub struct ResolutionExplain {
    // ... other fields ...
    
    /// Index generation at time of resolution (TELEMETRY ONLY)
    /// Useful for debugging, not a correctness gate
    pub index_generation: Option<u64>,
}
```

---

### Refinement 2: Keep DSL Surface Syntax Crisp

**Critical distinction in the DSL representation:**

| Type | DSL Syntax | Internal Type | Example |
|------|------------|---------------|---------|
| Entity PK | `(pk <uuid>)` | `ResolvedKey::Uuid` | `(pk "550e8400-e29b-41d4-a716-446655440000")` |
| Code | `(k <key> <value>)` | `ResolvedKey::Code` | `(k :role "DIRECTOR")` |
| Code (alt) | `(code <type> <value>)` | `ResolvedKey::Code` | `(code :jurisdiction "LU")` |

**Rules:**
- `(pk ...)` is ONLY for entity UUIDs (proper_person, limited_company, fund, etc.)
- Role codes, jurisdiction codes, document types remain as `(k ...)` or literal strings
- Never put a code inside `(pk ...)` - that conflates the semantics

**Internal representation can still use:**
```rust
pub enum ResolvedKey {
    Uuid(Uuid),    // → renders as (pk <uuid>)
    Code(String),  // → renders as string literal or (k ...)
}
```

**But the DSL surface stays clean:**
```lisp
;; Entity reference - resolved to UUID
(cbu.assign-role 
  :person (pk "550e8400-e29b-41d4-a716-446655440000")  
  :role "DIRECTOR"           ;; Code stays as string, not (pk ...)
  :jurisdiction "LU")        ;; Code stays as string
```

---

### Refinement 3: Key Strength Ordering in Resolver

Resolver MUST try keys in strength order, regardless of YAML order:

**Tier 1 - Exact (deterministic, no ambiguity):**
1. `id` - UUID primary key
2. `lei` - Legal Entity Identifier (globally unique)
3. `registration_number` - Company registration (unique per jurisdiction)
4. `bic` - Bank Identifier Code
5. `isin` - Securities identifier

**Tier 2 - Composite (high confidence with discriminators):**
6. `search_name` + `date_of_birth` + `nationality`
7. `search_name` + `jurisdiction` + `registration_date`

**Tier 3 - Fuzzy (needs disambiguation):**
8. `name` / `search_name` alone
9. `alias`

**Implementation:**

```rust
impl SearchKeyConfig {
    /// Get key strength for ordering (lower = stronger = try first)
    pub fn strength(&self) -> u8 {
        match self.primary_column() {
            "id" => 0,
            "lei" => 1,
            "registration_number" | "reg_number" => 2,
            "bic" => 3,
            "isin" => 4,
            "code" => 5,  // Reference data codes
            _ if !self.discriminators().is_empty() => 10,  // Composite
            "search_name" | "name" => 20,  // Fuzzy
            "alias" => 25,
            _ => 30,  // Unknown - lowest priority
        }
    }
}

impl GatewayRefResolver {
    /// Resolve trying strongest keys first
    pub async fn resolve_ordered(
        &mut self,
        entity_type: &str,
        available_keys: &[(SearchKeyConfig, String)], // (key_config, value)
    ) -> ResolveResult {
        // Sort by strength
        let mut keys: Vec<_> = available_keys.iter().collect();
        keys.sort_by_key(|(config, _)| config.strength());
        
        for (config, value) in keys {
            let result = self.search_single(entity_type, config, value).await;
            match result {
                ResolveResult::Found { .. } => return result,
                ResolveResult::NotFound => continue,  // Try next key
                ResolveResult::Ambiguous { .. } => {
                    // If we have stronger keys left, try them
                    // Otherwise return ambiguous
                    continue;
                }
            }
        }
        
        ResolveResult::NotFound
    }
}
```

---

### Refinement 4: Scoped Selection Validation

Selection validation MUST enforce tenant/CBU scope. Otherwise "validate via gateway" still allows cross-scope resolution.

**Current (unsafe):**
```rust
// Just looks up by UUID - no scope check!
let validation = gateway.search(SearchRequest {
    nickname: entity_type,
    values: vec![uuid.to_string()],
    mode: SearchMode::Exact,
    // NO SCOPE!
});
```

**Fixed:**
```rust
async fn validate_selection(
    &self,
    resolved_key: &ResolvedKey,
    entity_type: &str,
    scope: &ResolutionScope,  // NEW: Required scope
) -> Result<ValidatedSelection, SelectionError> {
    let request = SearchRequest {
        nickname: entity_type.to_string(),
        values: vec![resolved_key.to_string()],
        search_key: Some("id".to_string()),
        mode: SearchMode::Exact as i32,
        limit: Some(1),
        discriminators: HashMap::new(),
        
        // SCOPE ENFORCEMENT
        tenant_id: scope.tenant_id.clone(),
        cbu_id: scope.cbu_id.clone(),
    };
    
    let response = self.gateway.search(request).await?;
    
    match response.matches.len() {
        0 => Err(SelectionError::NotFoundInScope {
            key: resolved_key.clone(),
            scope: scope.clone(),
            message: format!(
                "Entity {} not found in scope (tenant={:?}, cbu={:?})",
                resolved_key, scope.tenant_id, scope.cbu_id
            ),
        }),
        1 => Ok(ValidatedSelection {
            resolved_key: resolved_key.clone(),
            display: response.matches[0].display.clone(),
            entity_type: entity_type.to_string(),
            scope: scope.clone(),
        }),
        _ => Err(SelectionError::MultipleMatches),  // Shouldn't happen for id lookup
    }
}

/// Resolution scope - defines what entities are visible
#[derive(Debug, Clone)]
pub struct ResolutionScope {
    /// Tenant ID (multi-tenant isolation)
    pub tenant_id: Option<String>,
    /// CBU ID (client business unit isolation)
    pub cbu_id: Option<Uuid>,
    /// Additional scope constraints
    pub constraints: HashMap<String, String>,
}

impl ResolutionScope {
    /// Scope for a specific CBU
    pub fn for_cbu(cbu_id: Uuid) -> Self {
        Self {
            tenant_id: None,
            cbu_id: Some(cbu_id),
            constraints: HashMap::new(),
        }
    }
    
    /// Global scope (admin operations only)
    pub fn global() -> Self {
        Self {
            tenant_id: None,
            cbu_id: None,
            constraints: HashMap::new(),
        }
    }
}
```

**In Entity Gateway proto:**
```protobuf
message SearchRequest {
    string nickname = 1;
    repeated string values = 2;
    optional string search_key = 3;
    SearchMode mode = 4;
    optional int32 limit = 5;
    map<string, string> discriminators = 6;
    
    // Scope enforcement (REQUIRED for id lookups)
    optional string tenant_id = 7;
    optional string cbu_id = 8;
}
```

**In Tantivy index, add scope fields:**
```rust
// During indexing, include scope fields
doc.add_text(tenant_id_field, &record.tenant_id);
doc.add_text(cbu_id_field, &record.cbu_id.to_string());

// During search, filter by scope
if let Some(tenant_id) = &query.tenant_id {
    let term = Term::from_field_text(self.tenant_id_field, tenant_id);
    scope_queries.push(Box::new(TermQuery::new(term, IndexRecordOption::Basic)));
}
```

---

## Updated Correctness Model

```
                    ┌─────────────────────────────────────┐
                    │  CORRECTNESS (Required)             │
                    │                                     │
                    │  1. Location-based commit (ref_id)  │
                    │  2. Fresh reader (no cached search) │
                    │  3. Pre-flight resolution check     │
                    │  4. Scoped selection validation     │
                    │  5. Key strength ordering           │
                    └─────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    │  TELEMETRY (Optional)         │
                    │                               │
                    │  - Generation tracking        │
                    │  - Resolution explain blob    │
                    │  - Timing metrics             │
                    └───────────────────────────────┘
```

The system is **correct** if the top box is implemented.
The system is **debuggable** if the bottom box is also implemented.
