# TODO: Entity Resolution System Fixes

## Overview

Critical fixes for the entity resolution subsystem based on architecture review. The core principle: **verb YAML s-expressions define search behavior, not code**. The search profile (entity type, search key, discriminators with selectivity) flows from verb config → semantic validator → gateway resolver → Tantivy.

---

## Priority 0: Data Corruption Prevention

### Fix 1: Location-Based Resolution (Not Text Matching)

**Problem:** `ResolutionService::commit()` scans AST and matches by `entity_type + value` text. If "John Smith" appears twice (as director AND as UBO), both get resolved to same person.

**Files:**
- `rust/src/services/resolution_service.rs`
- `rust/src/api/session.rs`

**Current (dangerous):**
```rust
// Matches by text - will over-apply
if ref.entity_type == resolved.entity_type 
   && ref.value == resolved.original_search 
   && ref.resolved_key.is_none() { ... }
```

**Fix:** Use location-based ref_id:

```rust
/// Unique identifier for an unresolved reference location
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct RefLocation {
    /// Statement index in the AST
    pub stmt_index: usize,
    /// Argument name within the statement
    pub arg_name: String,
    /// Optional: byte span for sub-argument precision
    pub span: Option<(usize, usize)>,
}

impl RefLocation {
    pub fn ref_id(&self) -> String {
        format!("{}:{}", self.stmt_index, self.arg_name)
    }
}
```

**Update DisambiguationItem:**
```rust
pub enum DisambiguationItem {
    EntityMatch {
        /// Location in AST - use this for commit, NOT search_text
        location: RefLocation,
        /// The search text (for display only)
        search_text: String,
        /// Entity type from verb lookup config
        entity_type: String,
        /// Candidate matches from gateway
        matches: Vec<EntityMatchOption>,
    },
    // ...
}
```

**Update commit logic:**
```rust
fn commit(&mut self, location: &RefLocation, resolved_key: ResolvedKey) {
    // Apply to EXACTLY this location
    if let Some(stmt) = self.ast.statements.get_mut(location.stmt_index) {
        if let Some(arg) = stmt.args.get_mut(&location.arg_name) {
            if let AstValue::EntityRef(ref mut entity_ref) = arg {
                entity_ref.resolved_key = Some(resolved_key.to_string());
            }
        }
    }
}
```

---

### Fix 2: ResolvedKey Enum (UUID vs Code)

**Problem:** Everything forced into `Uuid` type. Role codes (`DIRECTOR`), jurisdiction codes (`US`), product codes (`FUND_ACCOUNTING`) get fake UUIDs via hash.

**Files:**
- `rust/src/api/session.rs`
- `rust/src/services/resolution_service.rs`
- `rust/crates/ob-poc-types/src/lib.rs`
- `rust/src/dsl_v2/gateway_resolver.rs`

**Add to `ob-poc-types/src/lib.rs`:**
```rust
/// Resolved key - either a database UUID or a code string
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ResolvedKey {
    /// UUID primary key (entities, persons, funds)
    Uuid(Uuid),
    /// Code string (roles, jurisdictions, products, attributes)
    Code(String),
}

impl ResolvedKey {
    pub fn to_string(&self) -> String {
        match self {
            ResolvedKey::Uuid(u) => u.to_string(),
            ResolvedKey::Code(c) => c.clone(),
        }
    }
    
    pub fn is_uuid(&self) -> bool {
        matches!(self, ResolvedKey::Uuid(_))
    }
    
    /// Parse from string - tries UUID first, falls back to Code
    pub fn parse(s: &str) -> Self {
        match Uuid::parse_str(s) {
            Ok(u) => ResolvedKey::Uuid(u),
            Err(_) => ResolvedKey::Code(s.to_string()),
        }
    }
}

impl std::fmt::Display for ResolvedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolvedKey::Uuid(u) => write!(f, "{}", u),
            ResolvedKey::Code(c) => write!(f, "{}", c),
        }
    }
}
```

**Update EntityMatchOption:**
```rust
pub struct EntityMatchOption {
    /// The resolved key - UUID or Code
    pub resolved_key: ResolvedKey,
    /// Display name
    pub name: String,
    /// Entity type nickname
    pub entity_type: String,
    // ... rest unchanged
}
```

**Remove fake UUID generation in `gateway_resolver.rs`:**
```rust
// DELETE THIS:
let mut hasher = DefaultHasher::new();
m.value.hash(&mut hasher);
// ...

// REPLACE WITH:
let resolved_key = match Uuid::parse_str(&m.token) {
    Ok(uuid) => ResolvedKey::Uuid(uuid),
    Err(_) => ResolvedKey::Code(m.token.clone()),
};
```

---

### Fix 3: Validate Selection Against Gateway

**Problem:** `select_resolution()` endpoint accepts any UUID without verification. Client can inject cross-tenant UUIDs.

**File:** `rust/src/api/resolution_routes.rs`

**Current (unsafe):**
```rust
async fn select_resolution(...) {
    // Just parses UUID and trusts it
    let uuid = Uuid::parse_str(&req.resolved_key)?;
    // Fabricates EntityMatchInternal with no validation
}
```

**Fix:** Re-query gateway to validate:
```rust
async fn select_resolution(
    State(state): State<AppState>,
    Path((session_id, ref_id)): Path<(Uuid, String)>,
    Json(req): Json<SelectResolutionRequest>,
) -> Result<Json<SelectResolutionResponse>, StatusCode> {
    // Parse the key
    let resolved_key = ResolvedKey::parse(&req.resolved_key);
    
    // Get the pending resolution to find entity_type
    let pending = state.resolution_service
        .get_pending(&session_id, &ref_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    // Validate against gateway - exact lookup by token
    let validation = state.gateway_client
        .search(SearchRequest {
            nickname: pending.entity_type.clone(),
            values: vec![resolved_key.to_string()],
            mode: SearchMode::Exact as i32,
            search_key: Some("id".to_string()), // or token field
            limit: Some(1),
            discriminators: HashMap::new(),
        })
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    
    // Must find exactly one match
    let validated = validation.into_inner().matches
        .into_iter()
        .next()
        .ok_or(StatusCode::NOT_FOUND)?;
    
    // Now safe to commit with gateway-validated data
    state.resolution_service.commit(
        &session_id,
        &ref_id,
        ResolvedRef {
            resolved_key,
            display: validated.display,
            entity_type: pending.entity_type,
            // ...
        },
    )?;
    
    Ok(Json(SelectResolutionResponse { success: true }))
}
```

---

## Priority 1: S-Expression Search Profile Plumbing (CONFIG NOT CODE)

### Fix 4: Verb YAML Defines Search Schema

**This is the key architectural fix.** The verb's `lookup` block defines an s-expression that specifies:
- Entity type nickname
- Primary search field
- Discriminators with selectivity weights

**Example verb YAML:**
```yaml
# config/verbs/cbu/assign-role.yaml
verb: cbu.assign-role
params:
  - name: person
    type: entity
    lookup:
      entity_type: PROPER_PERSON
      # S-expression search schema - parsed by search_expr.rs
      search_key: "(search_name (date_of_birth :selectivity 0.95) (nationality :selectivity 0.7))"
  - name: role
    type: entity
    lookup:
      entity_type: ROLE
      search_key: "(role_code)"  # Simple exact match
  - name: jurisdiction
    type: entity
    lookup:
      entity_type: JURISDICTION
      search_key: "(iso_code)"
```

**Files to modify:**
- `rust/src/dsl_v2/verb_registry.rs` - Parse lookup.search_key s-expression
- `rust/src/dsl_v2/semantic_validator.rs` - Pass parsed schema to resolver
- `rust/src/dsl_v2/gateway_resolver.rs` - Use schema for search
- `rust/crates/entity-gateway/src/server/grpc.rs` - Accept discriminators

---

### Fix 5: Parse Search Schema in Verb Registry

**File:** `rust/src/dsl_v2/verb_registry.rs`

**Add to VerbParam or create new struct:**
```rust
use crate::search_expr::SearchSchema;

#[derive(Debug, Clone)]
pub struct LookupConfig {
    /// Entity type nickname (e.g., "PROPER_PERSON", "ROLE")
    pub entity_type: String,
    /// Parsed s-expression search schema
    pub search_schema: SearchSchema,
    /// Raw s-expression string (for debugging)
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
            .unwrap_or("(name)")  // default to simple name search
            .to_string();
        
        let search_schema = SearchSchema::parse(&search_key_expr)
            .map_err(|e| VerbLoadError::InvalidSearchKey(e.message))?;
        
        Ok(LookupConfig {
            entity_type,
            search_schema,
            search_key_expr,
        })
    }
}
```

**Update VerbParam:**
```rust
pub struct VerbParam {
    pub name: String,
    pub param_type: ParamType,
    pub required: bool,
    /// Lookup configuration - populated from verb YAML
    pub lookup: Option<LookupConfig>,
    // ...
}
```

---

### Fix 6: Semantic Validator Uses Search Schema

**File:** `rust/src/dsl_v2/semantic_validator.rs`

**Current:** Hardcoded entity type, ignores search_column/discriminators

**Fix:** Extract lookup config and pass to resolver:

```rust
async fn resolve_entity_ref(
    &self,
    entity_ref: &EntityRef,
    verb_param: &VerbParam,
    resolver: &dyn EntityResolver,
) -> Result<ResolveResult, ValidationError> {
    // Get lookup config from verb parameter
    let lookup = verb_param.lookup.as_ref()
        .ok_or(ValidationError::NoLookupConfig(verb_param.name.clone()))?;
    
    // Build search query from s-expression schema
    let search_query = SearchQueryFromSchema {
        // Primary search value from the EntityRef
        primary_value: entity_ref.value.clone(),
        // Primary field from schema
        primary_field: lookup.search_schema.primary_field.clone(),
        // Entity type from lookup config
        entity_type: lookup.entity_type.clone(),
        // Discriminators extracted from context (if available)
        discriminators: self.extract_discriminators(entity_ref, &lookup.search_schema),
    };
    
    // Resolve via gateway with full schema context
    resolver.resolve_with_schema(search_query, &lookup.search_schema).await
}

/// Extract discriminator values from EntityRef or surrounding context
fn extract_discriminators(
    &self,
    entity_ref: &EntityRef,
    schema: &SearchSchema,
) -> HashMap<String, String> {
    let mut discriminators = HashMap::new();
    
    for disc in &schema.discriminators {
        // Check if EntityRef has this discriminator in its context
        // e.g., entity_ref.discriminators.get("nationality")
        if let Some(value) = entity_ref.discriminators.get(&disc.field) {
            discriminators.insert(disc.field.clone(), value.clone());
        }
        
        // Could also extract from statement context, session context, etc.
    }
    
    discriminators
}
```

---

### Fix 7: Gateway Resolver Accepts Full Schema

**File:** `rust/src/dsl_v2/gateway_resolver.rs`

**Add new method:**
```rust
impl GatewayRefResolver {
    /// Resolve using full search schema from verb config
    pub async fn resolve_with_schema(
        &self,
        query: SearchQueryFromSchema,
        schema: &SearchSchema,
    ) -> Result<ResolveResult, ResolveError> {
        let request = SearchRequest {
            nickname: query.entity_type.clone(),
            values: vec![query.primary_value.clone()],
            // Use primary field from schema, not hardcoded "name"
            search_key: Some(schema.primary_field.clone()),
            mode: SearchMode::Fuzzy as i32,
            limit: Some(10),
            // Pass discriminators through!
            discriminators: query.discriminators.clone(),
        };
        
        let response = self.client.clone().search(request).await?;
        let matches = response.into_inner().matches;
        
        match matches.len() {
            0 => Ok(ResolveResult::NotFound),
            1 => {
                let m = &matches[0];
                // Check confidence threshold from schema
                if m.score >= schema.min_confidence {
                    Ok(ResolveResult::Found {
                        resolved_key: ResolvedKey::parse(&m.token),
                        display: m.display.clone(),
                        score: m.score,
                    })
                } else {
                    Ok(ResolveResult::Ambiguous {
                        options: self.convert_matches(matches),
                    })
                }
            }
            _ => {
                // Multiple matches - check if top match is clearly best
                let top = &matches[0];
                let second = &matches[1];
                
                if top.score >= schema.min_confidence 
                   && (top.score - second.score) > 0.15 {
                    // Clear winner
                    Ok(ResolveResult::Found {
                        resolved_key: ResolvedKey::parse(&top.token),
                        display: top.display.clone(),
                        score: top.score,
                    })
                } else {
                    // Ambiguous - needs disambiguation
                    Ok(ResolveResult::Ambiguous {
                        options: self.convert_matches(matches),
                    })
                }
            }
        }
    }
}
```

---

### Fix 8: Entity Gateway Applies Discriminators

**File:** `rust/crates/entity-gateway/src/server/grpc.rs`

The gRPC service already accepts `discriminators` in `SearchRequest`. Verify it's passed through:

```rust
// In EntityGatewayService::search()
let query = SearchQuery {
    values: req.values,
    search_key,
    mode,
    limit: req.limit.unwrap_or(10) as usize,
    // THIS MUST BE PASSED THROUGH (verify not empty):
    discriminators: req.discriminators,
};
```

**File:** `rust/crates/entity-gateway/src/index/tantivy_index.rs`

Verify `calculate_discriminator_score()` is being called in `search()`:

```rust
// Around line 380 in search()
let final_score = if query.discriminators.is_empty() {
    score
} else {
    self.calculate_discriminator_score(score, &doc, &query.discriminators)
};
```

---

## Priority 2: Consolidation & Cleanup

### Fix 9: Unify Resolution Systems

**Problem:** Two parallel systems:
- `ResolutionService` (in-memory, text-based matching)
- `ResolutionSubSession` in `session.rs` (location-based)

**Action:** Delete `ResolutionService` and use `ResolutionSubSession` pattern everywhere.

**Files:**
- `rust/src/services/resolution_service.rs` - DELETE or refactor
- `rust/src/api/resolution_routes.rs` - Update to use session-based resolution
- `rust/src/api/session.rs` - Make `ResolutionSubSession` the canonical model

**Key principle:** Resolution state lives in the agent session, not a separate service.

---

### Fix 10: Reuse Gateway Channel

**Problem:** `GatewayRefResolver::connect()` called per search.

**File:** `rust/src/dsl_v2/gateway_resolver.rs`

**Fix:** Store client in AppState or create once:

```rust
// In AppState or AgentState
pub struct AppState {
    // ... other fields ...
    
    /// Reusable gateway client - tonic Channel is cheap to clone
    pub gateway_client: EntityGatewayClient<Channel>,
}

impl AppState {
    pub async fn new(config: &Config) -> Result<Self, Error> {
        let gateway_client = EntityGatewayClient::connect(
            config.entity_gateway_url.clone()
        ).await?;
        
        Ok(Self {
            gateway_client,
            // ...
        })
    }
}

// In resolver, clone the client (cheap):
impl GatewayRefResolver {
    pub fn new(client: EntityGatewayClient<Channel>) -> Self {
        Self { client }
    }
    
    // No more connect() per call
}
```

---

### Fix 11: Session TTL and Cleanup

**File:** `rust/src/api/session_manager.rs`

**Add expiry tracking:**
```rust
pub struct SessionEntry {
    pub session: AgentSession,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub ttl: Duration,
}

impl SessionManager {
    /// Spawn background cleanup task
    pub fn start_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                self.cleanup_expired().await;
            }
        });
    }
    
    async fn cleanup_expired(&self) {
        let now = Utc::now();
        let mut sessions = self.sessions.write().await;
        
        sessions.retain(|_, entry| {
            now.signed_duration_since(entry.last_activity) < entry.ttl
        });
    }
}
```

---

## Priority 3: Tenant Isolation

### Fix 12: Add CBU/Tenant Scope to Searches

**Problem:** Gateway searches aren't scoped to current CBU's entity universe.

**File:** `rust/crates/entity-gateway/proto/entity_gateway.proto`

**Add to SearchRequest:**
```protobuf
message SearchRequest {
    string nickname = 1;
    repeated string values = 2;
    optional string search_key = 3;
    SearchMode mode = 4;
    optional int32 limit = 5;
    map<string, string> discriminators = 6;
    
    // NEW: Tenant/CBU scope
    optional string tenant_id = 7;
    optional string cbu_id = 8;
}
```

**File:** `rust/crates/entity-gateway/src/index/tantivy_index.rs`

**Add tenant filtering:**
```rust
// In schema builder, add tenant field
let tenant_field = schema_builder.add_text_field("tenant_id", STRING | STORED);

// In search(), filter by tenant if provided
if let Some(tenant_id) = &query.tenant_id {
    // Add term query for tenant
    let tenant_term = Term::from_field_text(self.tenant_field, tenant_id);
    let tenant_query = TermQuery::new(tenant_term, Default::default());
    
    // Combine with main query via BooleanQuery
    combined_query = BooleanQuery::new(vec![
        (Occur::Must, main_query),
        (Occur::Must, Box::new(tenant_query)),
    ]);
}
```

---

## Testing Checklist

After implementing:

1. [ ] Create two "John Smith" refs in same DSL - verify they can resolve to different people
2. [ ] Resolve a role code (`DIRECTOR`) - verify it stores as `Code("DIRECTOR")`, not fake UUID
3. [ ] Try to select a UUID from different tenant - verify rejection
4. [ ] Search for person with DOB discriminator - verify score boost
5. [ ] Check gateway client reuse - no "connecting" logs per search
6. [ ] Verify sessions expire after TTL

---

## File Summary

| File | Changes |
|------|---------|
| `ob-poc-types/src/lib.rs` | Add `ResolvedKey` enum, `RefLocation` |
| `rust/src/api/session.rs` | Update `DisambiguationItem` with location |
| `rust/src/services/resolution_service.rs` | DELETE or refactor to location-based |
| `rust/src/api/resolution_routes.rs` | Add gateway validation on select |
| `rust/src/dsl_v2/verb_registry.rs` | Parse `lookup.search_key` s-expression |
| `rust/src/dsl_v2/semantic_validator.rs` | Pass schema to resolver |
| `rust/src/dsl_v2/gateway_resolver.rs` | `resolve_with_schema()`, remove per-call connect |
| `rust/crates/entity-gateway/proto/entity_gateway.proto` | Add tenant_id, cbu_id |
| `rust/crates/entity-gateway/src/index/tantivy_index.rs` | Tenant filtering |
| `rust/src/api/session_manager.rs` | TTL + cleanup task |

---

## Architecture After Fixes

```
┌─────────────────────────────────────────────────────────────────┐
│  Verb YAML                                                       │
│  lookup:                                                         │
│    entity_type: PROPER_PERSON                                    │
│    search_key: "(search_name (dob :selectivity 0.95))"          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ parsed at load time
┌─────────────────────────────────────────────────────────────────┐
│  VerbRegistry                                                    │
│  - LookupConfig { entity_type, search_schema: SearchSchema }    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ passed to validator
┌─────────────────────────────────────────────────────────────────┐
│  SemanticValidator                                               │
│  - Extracts discriminators from context                          │
│  - Builds SearchQueryFromSchema                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ resolve_with_schema()
┌─────────────────────────────────────────────────────────────────┐
│  GatewayRefResolver                                              │
│  - Uses schema.primary_field as search_key                       │
│  - Passes discriminators to gateway                              │
│  - Uses schema.min_confidence for auto-resolve threshold         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ gRPC SearchRequest
┌─────────────────────────────────────────────────────────────────┐
│  Entity Gateway                                                  │
│  - Tantivy search with discriminator scoring                     │
│  - Tenant isolation via cbu_id filter                           │
│  - Returns ResolvedKey::Uuid or ResolvedKey::Code               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ if ambiguous
┌─────────────────────────────────────────────────────────────────┐
│  Disambiguation UI                                               │
│  - Shows matches with RefLocation                                │
│  - User selects → commit to EXACT location in AST               │
└─────────────────────────────────────────────────────────────────┘
```

**Key principle:** Search behavior is defined in verb YAML s-expressions, not scattered through code. The `search_key` s-expression is the single source of truth for how an entity parameter should be resolved.
