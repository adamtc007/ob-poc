# Plan: Integrate Composite Search Keys into EntityGateway

## Overview

Extend EntityGateway to support composite search keys defined in verb YAML, enabling disambiguation of entities at scale (100k+ records) using multiple fields (e.g., name + DOB + nationality for persons).

## Current State

### Verb YAML (types.rs) - Already Implemented
```rust
pub enum SearchKeyConfig {
    Simple(String),                    // "name"
    Composite(CompositeSearchKey),     // { primary, discriminators, tiers }
}

pub struct CompositeSearchKey {
    pub primary: String,               // Main search column
    pub discriminators: Vec<SearchDiscriminator>,
    pub resolution_tiers: Vec<ResolutionTier>,
    pub min_confidence: f32,
}

pub struct SearchDiscriminator {
    pub field: String,                 // DB column
    pub from_arg: Option<String>,      // DSL arg that provides value
    pub selectivity: f32,              // 0.0-1.0 disambiguation power
    pub required: bool,
}
```

### EntityGateway (entity_metadata.rs) - Current
```rust
pub struct SearchKeyConfig {
    pub name: String,      // "name", "first_name"
    pub column: String,    // DB column
    pub default: bool,
}
```

### Gap
EntityGateway has its own `SearchKeyConfig` that only supports simple single-column keys. It cannot:
1. Index multiple columns as a composite key
2. Use discriminators for disambiguation
3. Apply resolution tier strategies

---

## Implementation Plan

### Phase 1: Extend EntityGateway Config Types

**File: `rust/crates/entity-gateway/src/config/entity_metadata.rs`**

1. Add `CompositeSearchKeyConfig` struct:
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct CompositeSearchKeyConfig {
    pub name: String,                           // Composite key name
    pub columns: Vec<String>,                   // Columns to combine
    #[serde(default = "default_separator")]
    pub separator: String,                      // " " by default
    #[serde(default)]
    pub discriminators: Vec<DiscriminatorConfig>,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscriminatorConfig {
    pub column: String,
    #[serde(default = "default_selectivity")]
    pub selectivity: f32,                       // 0.0-1.0
}

fn default_separator() -> String { " ".to_string() }
fn default_selectivity() -> f32 { 0.5 }
```

2. Add to `EntityConfig`:
```rust
pub struct EntityConfig {
    // ... existing fields ...
    #[serde(default)]
    pub composite_search_keys: Vec<CompositeSearchKeyConfig>,
}
```

3. Update `all_columns()` to include composite key columns.

4. Add `get_any_search_key()` method returning enum:
```rust
pub enum SearchKeyVariant<'a> {
    Simple(&'a SearchKeyConfig),
    Composite(&'a CompositeSearchKeyConfig),
}
```

### Phase 2: Update Tantivy Index Schema

**File: `rust/crates/entity-gateway/src/index/tantivy_index.rs`**

1. Add field storage for composite keys:
```rust
pub struct TantivyIndex {
    // ... existing fields ...
    composite_fields: HashMap<String, Field>,
    composite_exact_fields: HashMap<String, Field>,
}
```

2. In `TantivyIndex::new()`:
   - Create Tantivy fields for each composite key
   - Apply same index mode (trigram/exact) as entity

3. Document indexing includes composite values computed from columns.

### Phase 3: Update Refresh Pipeline

**File: `rust/crates/entity-gateway/src/refresh/postgres.rs`**

1. Update `build_columns_list()`:
   - Include all columns needed for composite keys
   - Avoid duplicates

2. Update record building:
```rust
// Compute composite search values
for comp_key in &entity_config.composite_search_keys {
    let values: Vec<String> = comp_key.columns.iter()
        .filter_map(|col| row.try_get::<String, _>(col).ok())
        .collect();
    let combined = values.join(&comp_key.separator);
    search_values.insert(comp_key.name.clone(), combined);
}
```

### Phase 4: Update Search Logic

**File: `rust/crates/entity-gateway/src/index/tantivy_index.rs`**

1. In `search()`:
   - Check if `query.search_key` is in `composite_fields`
   - If so, use composite field for search
   - Same fuzzy/exact logic applies

2. No gRPC changes needed - `search_key: String` works for both simple and composite.

### Phase 5: Update entity_index.yaml

**File: `rust/crates/entity-gateway/config/entity_index.yaml`**

Add composite keys to high-volume entities:

```yaml
entities:
  person:
    nickname: "PERSON"
    source_table: '"ob-poc".entity_proper_persons'
    return_key: "entity_id"
    display_template: "{first_name} {last_name}"
    index_mode: trigram
    search_keys:
      - name: "name"
        column: "search_name"
        default: true
      # ... existing simple keys ...
    composite_search_keys:
      - name: "full_name"
        columns: ["first_name", "last_name"]
        separator: " "
      - name: "name_dob"
        columns: ["search_name", "date_of_birth"]
        separator: " ("
        discriminators:
          - column: "date_of_birth"
            selectivity: 0.95
          - column: "nationality"
            selectivity: 0.7

  legal_entity:
    # ... existing config ...
    composite_search_keys:
      - name: "name_jurisdiction"
        columns: ["company_name", "jurisdiction"]
        separator: " - "
        discriminators:
          - column: "jurisdiction"
            selectivity: 0.8

  share_class:
    # ... existing config ...
    composite_search_keys:
      - name: "name_isin"
        columns: ["name", "isin"]
        separator: " "
```

### Phase 6: Wire Verb YAML to EntityGateway (Future)

This is optional for MVP - EntityGateway can have its own composite key config. Future integration would:

1. Load verb YAML in EntityGateway startup
2. Extract `LookupConfig.search_key` (which is `SearchKeyConfig` enum)
3. Merge with entity_index.yaml config
4. Single source of truth for search configuration

---

## File Changes Summary

| File | Changes |
|------|---------|
| `entity-gateway/src/config/entity_metadata.rs` | Add `CompositeSearchKeyConfig`, update `EntityConfig` |
| `entity-gateway/src/index/tantivy_index.rs` | Add composite field handling in schema and search |
| `entity-gateway/src/refresh/postgres.rs` | Compute composite values during refresh |
| `entity-gateway/config/entity_index.yaml` | Add composite keys to person, legal_entity, share_class |

---

## Testing Strategy

1. **Unit tests** in `entity_metadata.rs`:
   - Parse YAML with composite_search_keys
   - `all_columns()` includes composite columns
   - `get_any_search_key()` returns correct variant

2. **Unit tests** in `tantivy_index.rs`:
   - Create index with composite keys
   - Refresh with composite values
   - Search on composite key field

3. **Integration test**:
   - Start EntityGateway with composite config
   - gRPC search with `search_key: "full_name"`
   - Verify disambiguation works

---

## Rollout Plan

1. **Phase 1-4**: Core implementation (no behavior change for existing keys)
2. **Phase 5**: Add composite keys to YAML (backwards compatible)
3. **Test**: Verify LSP autocomplete works with composite keys
4. **Phase 6**: (Future) Verb YAML integration

---

## Non-Goals (for this iteration)

- Resolution tiers (exact → composite → fuzzy) - future enhancement
- Discriminator-based scoring boost - future enhancement  
- Verb YAML as single source of truth - future enhancement
- UI changes for composite key selection - not needed (transparent)

---

## Success Criteria

1. EntityGateway accepts composite_search_keys in YAML
2. Tantivy indexes composite values correctly
3. Search on composite key returns results
4. LSP autocomplete works with composite key specified in verb YAML lookup
5. No regression for existing simple search keys
