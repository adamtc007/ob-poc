# Entity Resolution System - Consolidated TODO

> **Peer Reviewed**: Architecture refinements applied per review feedback.
> Key changes: span-based ref_id, strength ordering in resolver (not config), 
> (pk ...) validation enforced, scope enforcement end-to-end, explicit (where ...) support.

## Status Summary

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 1 | Tantivy Stale Index Fix | ✅ DONE |
| Phase 2 | Data Corruption Prevention (Fixes 1-3) | ✅ DONE |
| Phase 3 | S-Expression Search Profile Plumbing (7 fixes) | 🔲 TODO |
| Phase 4 | Architectural Invariants (6 fixes) | 🔲 TODO |

---

## Completed Work (Reference Only)

<details>
<summary>✅ Phase 1: Tantivy Stale Index (DONE)</summary>

- ReloadPolicy on reader creation
- `wait_merging_threads()` after commit
- Generation tracking for debugging
- Force reload method

**CAVEAT**: `wait_merging_threads()` is correctness for "refresh rebuild" but can introduce latency spikes. Avoid calling on every small commit - use on explicit refresh/rebuild operations only.

See: `rust/crates/entity-gateway/src/index/tantivy_index.rs`
</details>

<details>
<summary>✅ Phase 2: Data Corruption Prevention (DONE)</summary>

**Fix 1**: Location-based resolution with `RefLocation` struct
**Fix 2**: `ResolvedKey` enum (UUID vs Code)
**Fix 3**: Gateway validation on selection

See: `rust/src/services/resolution_service.rs`, `rust/src/api/session.rs`
</details>

---

## Phase 3: S-Expression Search Profile Plumbing

**Core Principle**: Verb YAML s-expressions define search behavior, not code.

```yaml
# config/verbs/cbu/assign-role.yaml
params:
  - name: person
    lookup:
      entity_type: PROPER_PERSON
      search_key: "(search_name (date_of_birth :selectivity 0.95) (nationality :selectivity 0.7))"
```

This flows: **YAML → VerbRegistry → SemanticValidator → GatewayResolver → Tantivy**

---

### Fix 3.1: Parse Search Schema in Verb Registry

**File:** `rust/src/dsl_v2/verb_registry.rs`

```rust
use crate::search_expr::SearchSchema;

#[derive(Debug, Clone)]
pub struct LookupConfig {
    pub entity_type: String,
    pub search_schema: SearchSchema,
    pub search_key_expr: String,
}

impl LookupConfig {
    pub fn from_yaml(yaml: &serde_yaml::Value) -> Result<Self, VerbLoadError> {
        let entity_type = yaml["entity_type"]
            .as_str()
            .ok_or(VerbLoadError::MissingField("entity_type"))?
            .to_string();
        
        let search_key_expr = yaml["search_key"]
            .as_str()
            .unwrap_or("(name)")
            .to_string();
        
        let search_schema = SearchSchema::parse(&search_key_expr)
            .map_err(|e| VerbLoadError::InvalidSearchKey(e.message))?;
        
        Ok(LookupConfig { entity_type, search_schema, search_key_expr })
    }
}
```

---

### Fix 3.2: Semantic Validator Extracts Discriminators

**File:** `rust/src/dsl_v2/semantic_validator.rs`

Discriminators come from TWO sources (merged):
1. **Literal `(where ...)` inside EntityRef** - explicit, portable
2. **Sibling args per verb schema** - inferred from context

**Enterprise-grade pattern**: Support explicit discriminators in EntityRef:

```lisp
;; Portable reference with explicit discriminators
(entity-ref person
  (k search_name "John Smith")
  (where (dob "1978-04-12") (nationality "GB"))
)
```

**Implementation:**

```rust
fn extract_discriminators(
    vc: &VerbCall,
    entity_ref: &EntityRef,
    search_key: &SearchKeyConfig,
) -> HashMap<String, String> {
    let mut discriminators = HashMap::new();
    
    // 1. FIRST: Explicit (where ...) from EntityRef itself (takes precedence)
    for (field, value) in &entity_ref.explicit_where {
        discriminators.insert(field.clone(), value.clone());
    }
    
    // 2. THEN: Inferred from sibling args per verb schema
    if let SearchKeyConfig::Composite(composite) = search_key {
        for disc in &composite.discriminators {
            // Don't override explicit discriminators
            if discriminators.contains_key(&disc.field) {
                continue;
            }
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

**Benefits:**
- Portable references: entity-ref can travel between contexts
- Verb-context driven: still works without explicit where
- Override capability: explicit beats inferred

async fn resolve_entity_ref(
    &self,
    entity_ref: &EntityRef,
    verb_param: &VerbParam,
    vc: &VerbCall,
) -> Result<ResolveResult, ValidationError> {
    let lookup = verb_param.lookup.as_ref()
        .ok_or(ValidationError::NoLookupConfig(verb_param.name.clone()))?;
    
    let discriminators = extract_discriminators(vc, &arg, &lookup.search_key);
    
    self.resolver.resolve_with_schema(
        &lookup.entity_type,
        &entity_ref.value,
        &lookup.search_schema,
        discriminators,
    ).await
}
```

---

### Fix 3.3: Gateway Resolver Uses Full Schema

**File:** `rust/src/dsl_v2/gateway_resolver.rs`

```rust
impl GatewayRefResolver {
    pub async fn resolve_with_schema(
        &self,
        entity_type: &str,
        value: &str,
        schema: &SearchSchema,
        discriminators: HashMap<String, String>,
    ) -> Result<ResolveResult, ResolveError> {
        let request = SearchRequest {
            nickname: entity_type.to_string(),
            values: vec![value.to_string()],
            search_key: Some(schema.primary_field.clone()),
            mode: SearchMode::Fuzzy as i32,
            limit: Some(10),
            discriminators,  // NOW POPULATED!
        };
        
        let response = self.client.clone().search(request).await?;
        let matches = response.into_inner().matches;
        
        match matches.len() {
            0 => Ok(ResolveResult::NotFound),
            1 if matches[0].score >= schema.min_confidence => {
                Ok(ResolveResult::Found {
                    resolved_key: ResolvedKey::parse(&matches[0].token),
                    display: matches[0].display.clone(),
                    score: matches[0].score,
                })
            }
            _ => Ok(ResolveResult::Ambiguous {
                options: self.convert_matches(matches),
            }),
        }
    }
}
```

---

### Fix 3.4: Entity Gateway Applies Discriminators

**File:** `rust/crates/entity-gateway/src/server/grpc.rs`

Verify discriminators pass through to Tantivy:

```rust
// In EntityGatewayService::search()
let query = SearchQuery {
    values: req.values,
    search_key,
    mode,
    limit: req.limit.unwrap_or(10) as usize,
    discriminators: req.discriminators,  // MUST NOT BE EMPTY
};
```

**File:** `rust/crates/entity-gateway/src/index/tantivy_index.rs`

Verify `calculate_discriminator_score()` is called in `search()`.

---

### Fix 3.5: Unify Resolution Systems

**Problem:** Two parallel systems exist:
- `ResolutionService` (in-memory, text-based)
- `ResolutionSubSession` in `session.rs` (location-based)

**Action:** Delete `ResolutionService`, use session-based resolution everywhere.

**Files:**
- `rust/src/services/resolution_service.rs` - DELETE or refactor
- `rust/src/api/resolution_routes.rs` - Update to use session
- `rust/src/api/session.rs` - `ResolutionSubSession` becomes canonical

---

### Fix 3.6: Reuse Gateway Channel

**File:** `rust/src/dsl_v2/gateway_resolver.rs`

```rust
// In AppState - create once, clone cheaply
pub struct AppState {
    pub gateway_client: EntityGatewayClient<Channel>,
    // ...
}

impl GatewayRefResolver {
    pub fn new(client: EntityGatewayClient<Channel>) -> Self {
        Self { client }  // No more connect() per call
    }
}
```

---

### Fix 3.7: Session TTL (Low Priority)

Resolution sessions currently live forever in memory. Add cleanup:

```rust
impl ResolutionSessionManager {
    /// Purge sessions older than TTL
    pub fn cleanup_stale(&mut self, ttl: Duration) {
        let cutoff = Instant::now() - ttl;
        self.sessions.retain(|_, session| session.last_accessed > cutoff);
    }
}

// Call periodically (e.g., every 5 minutes)
```

---

## Phase 4: Architectural Invariants

### Correctness Model

```
CORRECTNESS (Required)              TELEMETRY (Optional)
─────────────────────────────────   ────────────────────────
1. Location-based commit (ref_id)   - Generation tracking
2. Fresh reader after refresh       - Resolution explain blob
3. Pre-flight resolution check      - Timing metrics
4. Scoped selection validation
5. Key strength ordering
```

---

### Fix 4.1: Add ref_id to EntityRef AST

**File:** `rust/crates/dsl-core/src/ast.rs`

**IMPORTANT**: Use span-based ref_id, NOT arg_name. Arg names are not stable under renames.

```rust
pub enum AstNode {
    EntityRef {
        entity_type: String,
        search_column: String,
        value: String,
        resolved_key: Option<String>,
        span: Span,
        
        /// Location: "{stmt_index}:{span.start}-{span.end}" - stable under arg renames
        ref_id: String,
    },
    // ...
}
```

**File:** `rust/src/dsl_v2/enrichment.rs`

Generate ref_id from span (NOT arg_name):

```rust
fn enrich_argument(&mut self, arg: Argument, verb_args: Option<&Vec<RuntimeArg>>, stmt_index: usize) -> Argument {
    // ...
    let span = arg.span();
    AstNode::EntityRef {
        entity_type,
        search_column: config.search_key.primary_column().to_string(),
        value: s,
        resolved_key: None,
        span: span.clone(),
        // CORRECT: span-based, stable under arg renames
        ref_id: format!("{}:{}-{}", stmt_index, span.start, span.end),
    }
}
```

---

### Fix 4.2: Pre-Flight Resolution Check

**File:** `rust/src/dsl_v2/executor.rs`

```rust
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
                "Cannot execute: {} unresolved entity reference(s)",
                unresolved.len()
            ),
        });
    }
    
    Ok(())
}

impl Executor {
    pub async fn execute(&mut self, program: Program) -> Result<ExecutionResult, ExecutionError> {
        validate_all_resolved(&program)?;  // PRE-FLIGHT
        // ... rest of execution
    }
}
```

---

### Fix 4.3: Key Strength Ordering

Resolver MUST try strongest keys first.

**IMPORTANT**: Keep strength ordering in resolver, NOT in config crate. Config should be pure data.

**File:** `rust/src/dsl_v2/gateway_resolver.rs` (NOT config/types.rs)

```rust
impl GatewayRefResolver {
    /// Get key strength for ordering (lower = stronger = try first)
    /// Policy lives HERE in resolver, not in config crate
    fn key_strength(column: &str, has_discriminators: bool) -> u8 {
        match column {
            // Tier 1: Exact identifiers
            "id" => 0,
            "lei" => 1,
            "registration_number" | "reg_number" => 2,
            "bic" => 3,
            "isin" => 4,
            "code" => 5,
            // Tier 2: Composite with discriminators
            _ if has_discriminators => 10,
            // Tier 3: Fuzzy
            "search_name" | "name" => 20,
            "alias" => 25,
            _ => 30,
        }
    }
    
    /// Resolve trying strongest keys first
    pub async fn resolve_ordered(
        &mut self,
        entity_type: &str,
        available_keys: &[(SearchKeyConfig, String)],
    ) -> ResolveResult {
        let mut keys: Vec<_> = available_keys.iter().collect();
        keys.sort_by_key(|(config, _)| {
            Self::key_strength(
                config.primary_column(),
                !config.discriminators().is_empty()
            )
        });
        // ... rest of resolution
    }
}
```

**Strength tiers**:

```
Tier 1 (Exact):     id → lei → reg_number → bic → isin
Tier 2 (Composite): name + dob + nationality
Tier 3 (Fuzzy):     name → alias
```

**File:** `rust/crates/dsl-core/src/config/types.rs`

```rust
impl SearchKeyConfig {
    pub fn strength(&self) -> u8 {
        match self.primary_column() {
            "id" => 0,
            "lei" => 1,
            "registration_number" | "reg_number" => 2,
            "bic" => 3,
            "isin" => 4,
            "code" => 5,
            _ if !self.discriminators().is_empty() => 10,
            "search_name" | "name" => 20,
            "alias" => 25,
            _ => 30,
        }
    }
}
```

---

### Fix 4.4: Scoped Selection Validation

**File:** `rust/src/api/resolution_routes.rs`

Selection MUST enforce tenant/CBU scope:

```rust
#[derive(Debug, Clone)]
pub struct ResolutionScope {
    pub tenant_id: Option<String>,
    pub cbu_id: Option<Uuid>,
}

async fn validate_selection(
    gateway: &EntityGatewayClient<Channel>,
    resolved_key: &ResolvedKey,
    entity_type: &str,
    scope: &ResolutionScope,
) -> Result<ValidatedSelection, SelectionError> {
    let request = SearchRequest {
        nickname: entity_type.to_string(),
        values: vec![resolved_key.to_string()],
        search_key: Some("id".to_string()),
        mode: SearchMode::Exact as i32,
        limit: Some(1),
        discriminators: HashMap::new(),
        tenant_id: scope.tenant_id.clone(),
        cbu_id: scope.cbu_id.map(|u| u.to_string()),
    };
    
    let response = gateway.clone().search(request).await?;
    
    match response.into_inner().matches.len() {
        0 => Err(SelectionError::NotFoundInScope { ... }),
        1 => Ok(ValidatedSelection { ... }),
        _ => Err(SelectionError::MultipleMatches),
    }
}
```

**File:** `rust/crates/entity-gateway/proto/entity_gateway.proto`

```protobuf
message SearchRequest {
    string nickname = 1;
    repeated string values = 2;
    optional string search_key = 3;
    SearchMode mode = 4;
    optional int32 limit = 5;
    map<string, string> discriminators = 6;
    optional string tenant_id = 7;
    optional string cbu_id = 8;
}
```

**CRITICAL**: Proto fields are not enough - entity-gateway MUST actually enforce:

**File:** `rust/crates/entity-gateway/src/index/tantivy_index.rs`

```rust
// During indexing, include scope fields
doc.add_text(tenant_id_field, &record.tenant_id);
doc.add_text(cbu_id_field, &record.cbu_id.to_string());

// During search, filter by scope (NOT optional - this is the enforcement)
fn build_scoped_query(&self, base_query: Box<dyn Query>, scope: &SearchScope) -> Box<dyn Query> {
    let mut must_clauses: Vec<Box<dyn Query>> = vec![base_query];
    
    if let Some(tenant_id) = &scope.tenant_id {
        let term = Term::from_field_text(self.tenant_id_field, tenant_id);
        must_clauses.push(Box::new(TermQuery::new(term, IndexRecordOption::Basic)));
    }
    
    if let Some(cbu_id) = &scope.cbu_id {
        let term = Term::from_field_text(self.cbu_id_field, &cbu_id.to_string());
        must_clauses.push(Box::new(TermQuery::new(term, IndexRecordOption::Basic)));
    }
    
    Box::new(BooleanQuery::new(must_clauses.into_iter().map(|q| (Occur::Must, q)).collect()))
}
```

Without this enforcement, proto fields become "security theater".

---

### Fix 4.5: DSL Surface Syntax (Design Rule + Enforcement)

Keep DSL representation crisp - entities vs codes are distinct:

```lisp
;; CORRECT
(cbu.assign-role 
  :person (pk "550e8400-e29b-41d4-a716-446655440000")  ;; Entity → UUID
  :role "DIRECTOR"                                      ;; Code → string
  :jurisdiction "LU")                                   ;; Code → string

;; WRONG - don't conflate codes into pk
(cbu.assign-role 
  :role (pk "DIRECTOR"))  ;; ❌ Codes are NOT pks!
```

**ACTION ITEM**: Enforce at parser/validator level:

**File:** `rust/crates/dsl-core/src/parser.rs` or `rust/src/dsl_v2/semantic_validator.rs`

```rust
fn validate_pk_is_uuid(node: &AstNode) -> Result<(), ValidationError> {
    if let AstNode::Pk(value) = node {
        Uuid::parse_str(value).map_err(|_| ValidationError::PkMustBeUuid {
            value: value.clone(),
            message: format!(
                "(pk \"{}\") is invalid: pk must contain a UUID, not a code. \
                 Use string literals for codes like roles and jurisdictions.",
                value
            ),
        })?;
    }
    Ok(())
}
```

Internal `ResolvedKey::{Uuid, Code}` is fine, but:
- `(pk ...)` renders ONLY for `ResolvedKey::Uuid`
- `ResolvedKey::Code` renders as string literal

---

### Fix 4.6: Resolution Explain (Optional Telemetry)

**File:** `rust/crates/dsl-core/src/ast.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionExplain {
    pub ref_id: String,
    pub entity_type: String,
    pub search_key_expr: String,
    pub input_value: String,
    pub candidate_count: usize,
    pub winner_score: f32,
    pub resolved_key: String,
    pub resolved_display: String,
    pub resolution_tier: ResolutionTier,
    pub discriminators_applied: Vec<String>,
    pub index_generation: Option<u64>,  // OPTIONAL - telemetry only
    pub resolved_at: chrono::DateTime<chrono::Utc>,
}
```

Add to EntityRef (optional):

```rust
pub enum AstNode {
    EntityRef {
        // ... existing fields ...
        explain: Option<ResolutionExplain>,  // For debugging
    },
}
```

---

## Testing Checklist

After implementation:

1. [ ] Two "John Smith" refs in same DSL resolve to different people
2. [ ] Role code (`DIRECTOR`) stores as `Code("DIRECTOR")`, not fake UUID
3. [ ] Selection of UUID from different tenant/CBU is rejected
4. [ ] Search with DOB discriminator boosts score
5. [ ] Gateway client reused (no "connecting" logs per search)
6. [ ] Unresolved refs block execution with clear error
7. [ ] LEI search tried before fuzzy name search
8. [ ] Parse round-trip preserves EntityRef structure

**Note**: Detailed proptest code examples available in `archive/TODO-ENTITYREF-INVARIANTS.md` lines 450-550.

---

## File Summary

| File | Phase | Changes |
|------|-------|---------|
| `rust/src/dsl_v2/verb_registry.rs` | 3.1 | Parse lookup.search_key s-expression |
| `rust/src/dsl_v2/semantic_validator.rs` | 3.2 | Extract discriminators (explicit + inferred) |
| `rust/src/dsl_v2/gateway_resolver.rs` | 3.3, 3.6, 4.3 | resolve_with_schema(), reuse channel, key strength |
| `rust/crates/entity-gateway/src/server/grpc.rs` | 3.4 | Pass discriminators through |
| `rust/src/services/resolution_service.rs` | 3.5 | DELETE (unify to session) |
| `rust/src/api/session.rs` | 3.7 | Session TTL cleanup |
| `rust/crates/dsl-core/src/ast.rs` | 4.1, 4.6 | Add ref_id (span-based), explain to EntityRef |
| `rust/src/dsl_v2/enrichment.rs` | 4.1 | Generate span-based ref_id |
| `rust/src/dsl_v2/executor.rs` | 4.2 | Pre-flight resolution check |
| `rust/crates/dsl-core/src/parser.rs` | 4.5 | Validate (pk ...) contains UUID |
| `rust/src/api/resolution_routes.rs` | 4.4 | Scoped selection validation |
| `rust/crates/entity-gateway/proto/entity_gateway.proto` | 4.4 | Add tenant_id, cbu_id |
| `rust/crates/entity-gateway/src/index/tantivy_index.rs` | 4.4 | Scope enforcement in queries |

---

## Archived TODOs

The following files are superseded by this consolidated TODO:
- `TODO-ENTITY-RESOLUTION-FIXES.md` - Phases 1-2 done, 3+ merged here
- `TODO-ENTITYREF-INVARIANTS.md` - Merged into Phase 4
- `TODO-FIX-STALE-TANTIVY-INDEX.md` - Phase 1 done
