# TODO: Composite Search Key & Entity Resolution at Scale

## Problem Statement

Current `LookupConfig.search_key` is a single column name (e.g., `"name"`). This doesn't scale to 100k+ persons where "John Smith" matches thousands of records.

**Current triplet:** `(entity_type, search_key, primary_key)` → `("person", "name", "entity_id")`

**Needed:** S-expression composite search key with partial population and tiered resolution strategies.

---

## Target Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      DSL Verb Layer                             │
│  entity.lookup (person (name "John") (dob "1975-03-15"))        │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                   SearchKey (S-Expression)                      │
│  SearchKey::Person {                                            │
│    name: Some("John Smith"),                                    │
│    dob: Some(1975-03-15),                                       │
│    nationality: None,         // partial population OK          │
│    source_id: None,                                             │
│    within_cbu: Some($cbu_ref),                                  │
│  }                                                              │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                 Resolution Strategy Router                       │
│  match search_key.resolution_tier():                            │
│    Exact      → source_index.get(source_id)        // O(1)      │
│    Composite  → composite_index.get(name,dob,nat)  // O(log n)  │
│    Contextual → context_index.get(cbu).filter(name) // O(log n) │
│    Fuzzy      → phonetic + trigram search          // O(n)      │
│    Insufficient → return error with missing fields              │
└─────────────────────────────────────────────────────────────────┘
```

---

## PHASE 1: Data Model Changes

### 1.1 New Rust Types (`rust/src/dsl_v2/config/types.rs`)

```rust
// Replace simple search_key: String with structured SearchKeyConfig

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SearchKeyConfig {
    /// Legacy: single column name (backwards compatible)
    Simple(String),
    /// New: composite search key definition
    Composite(CompositeSearchKey),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompositeSearchKey {
    /// Primary search field (always required)
    pub primary: String,
    
    /// Discriminator fields that narrow the search
    #[serde(default)]
    pub discriminators: Vec<SearchDiscriminator>,
    
    /// Resolution tiers in priority order
    #[serde(default)]
    pub resolution_tiers: Vec<ResolutionTier>,
    
    /// Minimum confidence threshold (0.0-1.0)
    #[serde(default = "default_confidence")]
    pub min_confidence: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchDiscriminator {
    /// Field name in the entity
    pub field: String,
    /// DSL argument that provides this value
    pub from_arg: String,
    /// How much this field narrows the search (0.0-1.0)
    pub selectivity: f32,
    /// Is this field required for resolution?
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionTier {
    /// Source system ID - exact match, O(1)
    Exact,
    /// Name + DOB + Nationality - composite index, O(log n)
    Composite,
    /// Name within CBU scope - contextual, O(log n) 
    Contextual,
    /// Phonetic/trigram search - fuzzy, O(n)
    Fuzzy,
}
```

### 1.2 Update LookupConfig

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LookupConfig {
    pub table: String,
    #[serde(default)]
    pub schema: Option<String>,
    pub entity_type: Option<String>,
    
    // CHANGED: from String to SearchKeyConfig
    pub search_key: SearchKeyConfig,
    
    pub primary_key: String,
    #[serde(default)]
    pub resolution_mode: Option<ResolutionMode>,
}
```

### 1.3 Runtime SearchKey Struct (`rust/src/dsl_v2/resolution/search_key.rs` - NEW FILE)

```rust
/// Runtime search key with partially populated fields
#[derive(Debug, Clone)]
pub enum SearchKey {
    Person(PersonSearchKey),
    Company(CompanySearchKey),
    Fund(FundSearchKey),
    Cbu(CbuSearchKey),
    Generic(GenericSearchKey),
}

#[derive(Debug, Clone, Default)]
pub struct PersonSearchKey {
    pub name: Option<String>,
    pub dob: Option<NaiveDate>,
    pub nationality: Option<CountryCode>,
    pub source_system: Option<String>,
    pub source_id: Option<String>,
    pub within_cbu: Option<Uuid>,
    pub role_hint: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CompanySearchKey {
    pub name: Option<String>,
    pub registration_number: Option<String>,
    pub jurisdiction: Option<CountryCode>,
    pub lei: Option<String>,
    pub source_system: Option<String>,
    pub source_id: Option<String>,
}

impl SearchKey {
    /// Determine which resolution tier to use based on populated fields
    pub fn resolution_tier(&self) -> ResolutionTier { ... }
    
    /// List fields that would improve resolution confidence
    pub fn missing_discriminators(&self) -> Vec<&'static str> { ... }
    
    /// Compute confidence score based on populated fields
    pub fn confidence_score(&self) -> f32 { ... }
}
```

---

## PHASE 2: Database Schema Changes

### 2.1 Entity Resolution Indexes (`migrations/YYYYMMDD_entity_resolution_indexes.sql`)

```sql
-- Source system index (exact match)
CREATE TABLE IF NOT EXISTS "ob-poc".entity_source_ids (
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    source_system VARCHAR(50) NOT NULL,
    source_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (source_system, source_id)
);

CREATE INDEX idx_entity_source_ids_entity ON "ob-poc".entity_source_ids(entity_id);

-- Composite search index for persons
CREATE INDEX idx_persons_composite ON "ob-poc".entity_proper_persons (
    LOWER(last_name),
    LOWER(first_name),
    date_of_birth,
    nationality
);

-- Normalized name index for fuzzy matching
CREATE TABLE IF NOT EXISTS "ob-poc".entity_name_index (
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    name_normalized VARCHAR(500) NOT NULL,  -- lowercase, sorted tokens
    name_soundex VARCHAR(50),               -- phonetic code
    entity_type VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_entity_name_normalized ON "ob-poc".entity_name_index(name_normalized);
CREATE INDEX idx_entity_name_soundex ON "ob-poc".entity_name_index(name_soundex);

-- Context index: entities within CBU
CREATE INDEX idx_cbu_entity_roles_lookup ON "ob-poc".cbu_entity_roles(cbu_id, entity_id);

-- Trigram index for fuzzy search (requires pg_trgm extension)
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX idx_entity_name_trgm ON "ob-poc".entity_name_index 
    USING gin (name_normalized gin_trgm_ops);
```

### 2.2 Name Normalization Function

```sql
CREATE OR REPLACE FUNCTION "ob-poc".normalize_name(raw_name TEXT)
RETURNS TEXT AS $$
BEGIN
    -- Lowercase, remove accents, sort tokens alphabetically
    RETURN (
        SELECT string_agg(token, '|' ORDER BY token)
        FROM unnest(
            regexp_split_to_array(
                unaccent(lower(trim(raw_name))),
                '\s+'
            )
        ) AS token
        WHERE token != ''
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Trigger to maintain name index
CREATE OR REPLACE FUNCTION "ob-poc".update_entity_name_index()
RETURNS TRIGGER AS $$
BEGIN
    -- Delete old entry
    DELETE FROM "ob-poc".entity_name_index WHERE entity_id = NEW.entity_id;
    
    -- Insert new entry (name column varies by entity type)
    INSERT INTO "ob-poc".entity_name_index (entity_id, name_normalized, name_soundex, entity_type)
    SELECT 
        NEW.entity_id,
        "ob-poc".normalize_name(NEW.name),
        soundex(NEW.name),
        (SELECT type_code FROM "ob-poc".entities WHERE entity_id = NEW.entity_id);
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
```

---

## PHASE 3: Resolution Engine (`rust/src/dsl_v2/resolution/engine.rs` - NEW FILE)

### 3.1 Core Resolution Types

```rust
pub enum ResolutionResult {
    /// Single match with high confidence
    Resolved { entity_id: Uuid, confidence: f32 },
    /// Multiple matches - need more discriminators
    Ambiguous { candidates: Vec<Candidate>, suggestion: String },
    /// No match found - safe to create new
    NotFound,
    /// Not enough fields to attempt resolution
    InsufficientKeys { missing: Vec<String>, message: String },
}

pub struct Candidate {
    pub entity_id: Uuid,
    pub display_name: String,
    pub match_score: f32,
    pub matched_fields: Vec<String>,
}
```

### 3.2 Resolution Engine Interface

```rust
pub struct EntityResolutionEngine {
    pool: PgPool,
    // In-memory caches for hot paths
    source_cache: HashMap<(String, String), Uuid>,
    // Bloom filter for fast "definitely not exists"
    existence_filter: BloomFilter,
}

impl EntityResolutionEngine {
    pub async fn resolve(&self, key: &SearchKey) -> Result<ResolutionResult> {
        // 1. Check bloom filter for fast negative
        if !self.maybe_exists(key) {
            return Ok(ResolutionResult::NotFound);
        }
        
        // 2. Route to appropriate resolver based on tier
        match key.resolution_tier() {
            ResolutionTier::Exact => self.resolve_exact(key).await,
            ResolutionTier::Composite => self.resolve_composite(key).await,
            ResolutionTier::Contextual => self.resolve_contextual(key).await,
            ResolutionTier::Fuzzy => self.resolve_fuzzy(key).await,
        }
    }
    
    async fn resolve_exact(&self, key: &SearchKey) -> Result<ResolutionResult>;
    async fn resolve_composite(&self, key: &SearchKey) -> Result<ResolutionResult>;
    async fn resolve_contextual(&self, key: &SearchKey) -> Result<ResolutionResult>;
    async fn resolve_fuzzy(&self, key: &SearchKey) -> Result<ResolutionResult>;
}
```

---

## PHASE 4: DSL Grammar Extension

### 4.1 S-Expression Search Key Parser (`rust/src/dsl_v2/parser/search_key.rs` - NEW FILE)

```rust
/// Parse S-expression search key
/// 
/// Examples:
///   (person (name "John Smith"))
///   (person (name "John") (dob "1975-03-15") (nationality "GB"))
///   (company (reg-number "12345678") (jurisdiction "GB"))
///   (person (source "BLOOMBERG" "BBG_123"))
///   (person (name "John") (within (cbu $my_cbu)))

pub fn parse_search_key(input: &str) -> IResult<&str, SearchKey> {
    delimited(
        tag("("),
        alt((
            parse_person_key,
            parse_company_key,
            parse_fund_key,
            parse_cbu_key,
        )),
        tag(")")
    )(input)
}

fn parse_person_key(input: &str) -> IResult<&str, SearchKey> {
    let (input, _) = tag("person")(input)?;
    let (input, fields) = many0(parse_search_field)(input)?;
    
    let mut key = PersonSearchKey::default();
    for (field_name, value) in fields {
        match field_name {
            "name" => key.name = Some(value.as_string()?),
            "dob" => key.dob = Some(value.as_date()?),
            "nationality" => key.nationality = Some(value.as_country()?),
            "source" => {
                let (sys, id) = value.as_source_pair()?;
                key.source_system = Some(sys);
                key.source_id = Some(id);
            }
            "within" => key.within_cbu = Some(value.as_cbu_ref()?),
            "role" => key.role_hint = Some(value.as_string()?),
            _ => return Err(/* unknown field */),
        }
    }
    
    Ok((input, SearchKey::Person(key)))
}
```

### 4.2 Verb YAML Schema Extension

```yaml
# In entity.yaml - updated lookup config

lookup-person:
  description: Resolve a person entity by composite key
  behavior: plugin
  handler: entity_resolution
  args:
    - name: key
      type: search_key  # NEW type
      required: true
      search_key_config:
        entity_type: person
        primary: name
        discriminators:
          - field: date_of_birth
            from_arg: dob
            selectivity: 0.95
            required: false
          - field: nationality
            from_arg: nationality
            selectivity: 0.7
            required: false
          - field: source_id
            from_arg: source-id
            selectivity: 1.0
            required: false
        resolution_tiers:
          - exact      # source_id present
          - composite  # name + dob + nationality
          - contextual # name + within-cbu
          - fuzzy      # name only (returns candidates)
        min_confidence: 0.8
  returns:
    type: uuid
    name: entity_id
    capture: true

ensure-proper-person:
  # ... existing config ...
  args:
    - name: first-name
      type: string
      required: true
    - name: last-name  
      type: string
      required: true
    - name: dob
      type: date
      required: false
    - name: nationality
      type: string
      required: false
    - name: source-system
      type: string
      required: false
    - name: source-id
      type: string
      required: false
    - name: within-cbu
      type: uuid
      required: false
      lookup:
        table: cbus
        entity_type: cbu
        search_key: name
        primary_key: cbu_id
  # Resolution uses these args to build SearchKey automatically
  resolution:
    entity_type: person
    key_mapping:
      name: "concat(first-name, ' ', last-name)"
      dob: dob
      nationality: nationality
      source_system: source-system
      source_id: source-id
      within_cbu: within-cbu
```

---

## PHASE 5: Verb Self-Discovery

### 5.1 Update VerbProduces/VerbConsumes

```rust
// In types.rs

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbProduces {
    pub produced_type: String,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub resolved: bool,
    #[serde(default)]
    pub initial_state: Option<String>,
    
    // NEW: How to build search key for this entity type
    #[serde(default)]
    pub search_key_template: Option<SearchKeyTemplate>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchKeyTemplate {
    /// Which args map to which search key fields
    pub field_mappings: HashMap<String, String>,
    /// Expression to compute complex fields (e.g., full name from first+last)
    #[serde(default)]
    pub computed_fields: HashMap<String, String>,
}
```

### 5.2 Runtime Key Builder

```rust
impl UnifiedVerbDef {
    /// Build a search key from verb arguments for resolution
    pub fn build_search_key(&self, args: &HashMap<String, Value>) -> Option<SearchKey> {
        let produces = self.produces.as_ref()?;
        let template = produces.search_key_template.as_ref()?;
        
        match produces.produced_type.as_str() {
            "entity" => {
                match produces.subtype.as_deref() {
                    Some("proper_person") => {
                        let mut key = PersonSearchKey::default();
                        // Map args to search key fields
                        if let Some(first) = args.get("first-name") {
                            if let Some(last) = args.get("last-name") {
                                key.name = Some(format!("{} {}", first, last));
                            }
                        }
                        if let Some(dob) = args.get("dob") {
                            key.dob = dob.as_date();
                        }
                        // ... etc
                        Some(SearchKey::Person(key))
                    }
                    Some("limited_company") => {
                        let mut key = CompanySearchKey::default();
                        // ... map fields
                        Some(SearchKey::Company(key))
                    }
                    _ => None
                }
            }
            _ => None
        }
    }
}
```

---

## PHASE 6: Implementation Tasks

### 6.1 Rust Code Changes

| File | Change |
|------|--------|
| `rust/src/dsl_v2/config/types.rs` | Add `SearchKeyConfig`, `CompositeSearchKey`, `ResolutionTier` |
| `rust/src/dsl_v2/resolution/mod.rs` | NEW: Module declaration |
| `rust/src/dsl_v2/resolution/search_key.rs` | NEW: `SearchKey` enum and impls |
| `rust/src/dsl_v2/resolution/engine.rs` | NEW: `EntityResolutionEngine` |
| `rust/src/dsl_v2/resolution/indexes.rs` | NEW: Index management |
| `rust/src/dsl_v2/parser/search_key.rs` | NEW: S-expression parser |
| `rust/src/dsl_v2/verb_registry.rs` | Update `ArgDef` to use new search key |
| `rust/src/dsl_v2/executor/mod.rs` | Integrate resolution engine |

### 6.2 Database Migrations

| Migration | Purpose |
|-----------|---------|
| `YYYYMMDD_001_entity_source_ids.sql` | Source system ID table |
| `YYYYMMDD_002_entity_name_index.sql` | Normalized name index |
| `YYYYMMDD_003_resolution_indexes.sql` | Composite + trigram indexes |
| `YYYYMMDD_004_name_normalization.sql` | Normalization functions + triggers |

### 6.3 YAML Config Updates

| File | Change |
|------|--------|
| `rust/config/verbs/entity.yaml` | Add `search_key_config` to lookups |
| `rust/config/verbs/cbu.yaml` | Update CBU lookups |
| `rust/config/verbs/fund.yaml` | Update fund lookups |
| `rust/config/verbs/_meta.yaml` | Document new search key schema |

---

## PHASE 7: Backwards Compatibility

### 7.1 Simple String Keys Still Work

```yaml
# OLD (still works)
lookup:
  table: cbus
  search_key: name
  primary_key: cbu_id

# NEW (composite)
lookup:
  table: entities  
  search_key:
    primary: name
    discriminators:
      - field: date_of_birth
        from_arg: dob
  primary_key: entity_id
```

### 7.2 Migration Path

1. `SearchKeyConfig::Simple(String)` handles legacy configs
2. Parser auto-upgrades simple keys to single-field composite
3. No breaking changes to existing verb YAML files

---

## Testing Strategy

### Unit Tests
- [ ] `SearchKey` tier detection with various field combinations
- [ ] Name normalization (unicode, accents, ordering)
- [ ] S-expression parser for all entity types
- [ ] Backwards compat with simple string keys

### Integration Tests
- [ ] Exact match via source_id
- [ ] Composite match (name + dob + nationality)
- [ ] Contextual match (name within CBU)
- [ ] Fuzzy match returning ranked candidates
- [ ] Ambiguous result handling

### Scale Tests
- [ ] 100k person records - resolution < 50ms
- [ ] Bulk load bypass (pre-assigned IDs)
- [ ] Index maintenance under load

---

## Files to Create

```
rust/src/dsl_v2/resolution/
├── mod.rs                 # Module exports
├── search_key.rs          # SearchKey types + impls
├── engine.rs              # EntityResolutionEngine
├── indexes.rs             # Index management
├── normalization.rs       # Name normalization
└── tests.rs               # Unit tests

rust/src/dsl_v2/parser/
└── search_key.rs          # S-expression parser (nom)

migrations/
├── YYYYMMDD_entity_source_ids.sql
├── YYYYMMDD_entity_name_index.sql
└── YYYYMMDD_resolution_indexes.sql
```

---

## Dependencies

```toml
# Cargo.toml additions
bloom = "0.3"           # Bloom filter for fast negative lookups
rust-strsim = "0.10"    # String similarity (Levenshtein, Jaro-Winkler)
```

---

## Success Criteria

1. **Exact match** (source_id): < 1ms, 100% confidence
2. **Composite match** (name+dob+nat): < 5ms, 95% confidence  
3. **Contextual match** (name within CBU): < 5ms, 85% confidence
4. **Fuzzy match** (name only): < 100ms, returns top-5 candidates
5. **Ambiguous handling**: Clear error with missing field suggestions
6. **Backwards compat**: All existing verb YAML files work unchanged
