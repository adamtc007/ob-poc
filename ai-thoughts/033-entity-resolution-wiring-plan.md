# 033 Entity Resolution Wiring Plan

## Problem Statement

Entity resolution infrastructure exists but is **not wired end-to-end**. The gaps:

| Issue | Current State | Required |
|-------|---------------|----------|
| **String vs UUID confusion** | LSP inserts `@KEY`, but confirmation doesn't update to resolved value | Clear separation: search by name → confirm → insert primary_key |
| **Incremental discriminator refinement** | Discriminator fields defined but UI doesn't surface them | Progressive refinement UI (nationality, DOB, etc.) |
| **Non-UUID primary keys** | Code assumes UUID everywhere | Support CODE returns (jurisdiction, role, product) |
| **AST update on confirm** | Resolution API exists but doesn't update DSL | Wire confirmation → AST mutation → editor update |

---

## Key Insight: Entity-Type Drives UI Configuration

The resolution modal must **switch behavior based on entity_type**. Currently it's hardcoded to show one "name" search field. The fix:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  CURRENT: Hardcoded single search field                                     │
│  ─────────────────────────────────────                                      │
│                                                                              │
│  resolution.rs:                                                              │
│    TextEdit::singleline(search_buffer)                                       │
│        .hint_text("Type name or add discriminators...")  ← HARDCODED        │
│                                                                              │
│  Problem: CBU has jurisdiction/client_type, person has nationality/DOB      │
│           - these are not exposed in UI                                      │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│  REQUIRED: Config-driven UI based on entity_type                            │
│  ───────────────────────────────────────────────                            │
│                                                                              │
│  1. UnresolvedRefResponse already has entity_type: "cbu" | "person" | ...   │
│  2. Add search_keys[] and discriminator_fields[] to response                │
│  3. UI renders fields dynamically based on these configs                    │
│                                                                              │
│  entity_type: "cbu"           entity_type: "person"                         │
│  ┌────────────────────┐       ┌────────────────────────────────────┐        │
│  │ Name: [Allianz    ]│       │ Name: [John Smith                 ]│        │
│  │ Jurisdiction: [LU▼]│       │ Nationality: [UK▼]  DOB: [1965    ]│        │
│  │ Type: [FUND     ▼]│       │                                    │        │
│  └────────────────────┘       └────────────────────────────────────┘        │
│                                                                              │
│  entity_type: "jurisdiction"  (reference data - small table)                │
│  ┌────────────────────────────────────────────────────────────────────┐    │
│  │ Dropdown: [Luxembourg (LU) ▼]  ← No search modal, just dropdown    │    │
│  └────────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data flow:**

```
1. Verb YAML declares:    lookup.entity_type: "cbu"
                                    │
2. Resolution start:                ▼
   Server looks up "cbu" in entity_index.yaml
   → search_keys: [id, name*, jurisdiction, client_type]
   → discriminators: []  (CBU has none)
                                    │
3. Response includes:               ▼
   UnresolvedRefResponse {
     entity_type: "cbu",
     search_keys: [...],  // NEW
     discriminator_fields: [],
   }
                                    │
4. UI renders:                      ▼
   render_entity_specific_fields(entity_type, search_keys, discriminators)
```

---

## Key Insight: Search Keys vs Discriminators

There's a critical distinction that the current UI ignores:

| Concept | Purpose | Example | UI |
|---------|---------|---------|-----|
| **Search Keys** | Columns that can be searched | `cbu.name`, `cbu.jurisdiction`, `cbu.client_type` | Multiple search fields |
| **Discriminators** | Boost scoring for disambiguation | `person.nationality`, `person.date_of_birth` | Refinement fields |

**Current gap:** CBU has 4 search keys but UI only shows name:

```yaml
# entity_index.yaml - CBU definition
cbu:
  search_keys:
    - name: "id"              # Can search by UUID
      column: "cbu_id"
    - name: "name"            # Can search by name (DEFAULT)
      column: "name"
      default: true
    - name: "jurisdiction"    # Can filter by jurisdiction
      column: "jurisdiction"
    - name: "client_type"     # Can filter by type
      column: "client_type"
```

**Required UI:**

```
┌─────────────────────────────────────────────────────────────────────────┐
│ 🔍 Allianz                                                              │
│                                                                         │
│ ┌─ Search Keys (from entity_index.yaml search_keys) ─────────────────┐ │
│ │ Name: [Allianz        ]  Jurisdiction: [LU ▼]  Type: [FUND ▼]     │ │
│ └───────────────────────────────────────────────────────────────────┘ │
│                                                                         │
│ Allianz Fund I (98%)       ← Matches name "Allianz", LU, FUND          │
│   LU | FUND | Active                                                   │
│                                                                         │
│ Allianz Fund II (95%)                                                  │
│   LU | FUND | Active                                                   │
└─────────────────────────────────────────────────────────────────────────┘
```

**Why this matters:**
- `person` has search_keys (name, first_name, last_name, id_document) + discriminators (nationality, DOB)
- `cbu` has search_keys (name, jurisdiction, client_type) but NO discriminators
- The UI should show BOTH concepts where applicable

---

## Key Insight: The Triplet Model

Every entity lookup is defined by a **triplet** in verb YAML:

```yaml
lookup:
  entity_type: person          # → EntityGateway nickname (PERSON)
  search_key: name             # → Column to search (or s-expr for composite)
  primary_key: entity_id       # → What to INSERT (UUID or CODE)
```

**return_key in entity_index.yaml must match primary_key in verb YAML.**

### Return Key Types

| entity_type | return_key | Type | Example Value |
|-------------|------------|------|---------------|
| `person` | `entity_id` | UUID | `550e8400-e29b-41d4-a716-446655440000` |
| `cbu` | `cbu_id` | UUID | `123e4567-e89b-12d3-a456-426614174000` |
| `jurisdiction` | `jurisdiction_code` | CODE | `LU`, `IE`, `US` |
| `role` | `name` | CODE | `DIRECTOR`, `SIGNATORY` |
| `product` | `product_code` | CODE | `FUND_ACCOUNTING` |
| `currency` | `iso_code` | CODE | `USD`, `EUR` |
| `instrument_class` | `class_id` | UUID | ... |
| `market` | `market_id` | UUID | ... |

**Implication:** Resolution flow must handle both UUID and CODE returns. The DSL doesn't care - it receives a string token that the executor resolves.

---

## Architecture: Config-Driven Resolution

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Verb YAML (cbu.yaml, entity.yaml, etc.)                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ lookup:                                                              │   │
│  │   entity_type: person        ──┐                                     │   │
│  │   search_key: "(name (nationality :selectivity 0.7))"                │   │
│  │   primary_key: entity_id     ──┼─ Triplet defines resolution        │   │
│  └────────────────────────────────┘                                     │   │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  EntityGateway Index Config (entity_index.yaml)                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ person:                                                              │   │
│  │   nickname: "PERSON"         ← Maps to entity_type                   │   │
│  │   return_key: "entity_id"    ← Must match primary_key                │   │
│  │   discriminators:                                                    │   │
│  │     - name: nationality      ← UI can show refinement field          │   │
│  │       selectivity: 0.7                                               │   │
│  │     - name: date_of_birth                                            │   │
│  │       selectivity: 0.95                                              │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  Resolution Flow                                                            │
│                                                                              │
│  1. User types: (entity.create :name "John Smith"                           │
│                                                                              │
│  2. Parser detects unresolved ref: EntityRef { value: "John Smith" }        │
│                                                                              │
│  3. Resolution UI shows:                                                    │
│     ┌──────────────────────────────────────────────────────────────────┐   │
│     │ 🔍 John Smith                                                     │   │
│     │                                                                   │   │
│     │ ┌─ Discriminators (from entity_index.yaml) ─────────────────┐    │   │
│     │ │ Nationality: [UK     ▼]  DOB: [1965-__-__]                │    │   │
│     │ └───────────────────────────────────────────────────────────┘    │   │
│     │                                                                   │   │
│     │ John Smith (98%)           ← Score boosted by discriminator match│   │
│     │   UK | DOB 1965-03-12 | BlackRock                                │   │
│     │                                                                   │   │
│     │ John Smith (72%)                                                 │   │
│     │   US | DOB 1980-07-22 | Vanguard                                 │   │
│     └──────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  4. User confirms → API returns: { resolved_value: "550e8400-..." }         │
│                                                                              │
│  5. AST updated: EntityRef { value: "John Smith", resolved: Some(uuid) }    │
│                                                                              │
│  6. DSL editor updated: :name "John Smith"  →  :entity-id 550e8400-...      │
│     (or keeps name if verb expects name arg)                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Type Extensions Required

**1. UnresolvedRefResponse (ob-poc-types/src/resolution.rs)**

Already has `entity_type` and `discriminator_fields`. Need to add `search_keys`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRefResponse {
    pub ref_id: String,
    pub entity_type: String,           // ← Already exists: "cbu", "person", etc.
    pub search_value: String,
    pub initial_matches: Vec<EntityMatchResponse>,
    
    // NEW: Search keys from entity_index.yaml
    pub search_keys: Vec<SearchKeyField>,
    
    // Already exists but may need enrichment
    pub discriminator_fields: Vec<DiscriminatorField>,
    
    // NEW: Resolution mode hint
    pub resolution_mode: ResolutionMode,
    
    // ... other fields
}

/// Search key definition (from entity_index.yaml search_keys)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchKeyField {
    pub name: String,           // "name", "jurisdiction", "client_type"
    pub label: String,          // "Name", "Jurisdiction", "Client Type"
    pub is_default: bool,       // true for the main text search field
    pub field_type: SearchKeyType,
    pub enum_values: Option<Vec<EnumValue>>, // For dropdowns
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchKeyType {
    Text,           // Free text search (name)
    Enum,           // Dropdown (jurisdiction, client_type)
    Uuid,           // UUID lookup (rarely user-facing)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub code: String,      // "LU"
    pub display: String,   // "Luxembourg"
}

/// How to render the resolution UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolutionMode {
    /// Full search modal with multiple fields
    SearchModal,
    /// Simple autocomplete dropdown (reference data)
    Autocomplete,
}
```

**2. ResolutionSearchRequest**

Extend to accept multiple search keys:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionSearchRequest {
    pub ref_id: String,
    
    // CHANGE: From single `query` to multi-key search
    pub search_keys: HashMap<String, String>,  // { "name": "Allianz", "jurisdiction": "LU" }
    
    // Keep discriminators for scoring boost
    pub discriminators: HashMap<String, String>,
    
    pub limit: Option<usize>,
}
```

**3. WindowData::Resolution**

Add current_ref info for UI to access entity_type:

```rust
pub enum WindowData {
    Resolution {
        parent_session_id: String,
        subsession_id: String,
        current_ref_index: usize,
        total_refs: usize,
        // NEW: Current ref being resolved (has entity_type, search_keys)
        current_ref: Option<UnresolvedRefResponse>,
    },
    // ...
}
```

**4. ResolutionUIState**

Extend to hold multi-key values:

```rust
pub struct ResolutionUIState {
    pub search_query: String,              // Keep for backward compat
    pub search_key_values: HashMap<String, String>,  // NEW: Multi-key values
    pub discriminator_values: HashMap<String, String>,
    // ...
}
```

---

## Implementation Phases

### Phase 1: Search Keys Metadata Flow ✅ COMPLETE

**Status:** Server-side wiring complete. Added:
- `GetEntityConfig` RPC to EntityGateway proto
- Handler implementation in `grpc.rs` 
- `get_entity_config` method in `GatewayRefResolver`
- `SearchKeyField`, `DiscriminatorField`, `ResolutionModeHint` types in `ob-poc-types`
- Entity config cache with enrichment in `resolution_routes.rs`
- Multi-key search method `search_multi_key` in gateway resolver

**Goal:** Surface ALL search keys (not just name) in resolution UI.

**The problem:** CBU, person, legal_entity all have multiple search keys but UI only shows name field.

**Files:**
- `rust/crates/entity-gateway/proto/gateway.proto` - Add GetIndexConfig RPC
- `rust/crates/entity-gateway/src/service.rs` - Expose search_keys metadata
- `rust/src/api/resolution_routes.rs` - Include search_keys in response
- `rust/crates/ob-poc-ui/src/panels/resolution.rs` - Render multiple search fields

**API Change:**

```rust
// GET /api/session/:id/resolution
#[derive(Serialize)]
pub struct UnresolvedRefResponse {
    pub ref_id: String,
    pub entity_type: String,
    pub search_value: String,
    pub initial_matches: Vec<EntityMatchResponse>,
    // NEW: Search keys from index config
    pub search_keys: Vec<SearchKeyInfo>,
    // NEW: Discriminator fields from index config
    pub discriminator_fields: Vec<DiscriminatorFieldInfo>,
}

#[derive(Serialize)]
pub struct SearchKeyInfo {
    pub name: String,           // "name", "jurisdiction", "client_type"
    pub display_name: String,   // "Name", "Jurisdiction", "Client Type"
    pub is_default: bool,       // true for the main search field
    pub is_enum: bool,          // true if has known values (jurisdiction, client_type)
    pub enum_values: Option<Vec<String>>, // ["LU", "IE", "US"] for jurisdictions
}
```

**UI Change:**

```rust
// resolution.rs - render search keys as fields
fn render_search_keys(
    ui: &mut Ui, 
    keys: &[SearchKeyInfo], 
    values: &mut HashMap<String, String>,
    default_query: &str,
) -> bool {
    let mut changed = false;
    
    ui.horizontal(|ui| {
        for key in keys {
            ui.label(&key.display_name);
            
            if key.is_enum {
                // Dropdown for enum fields (jurisdiction, client_type)
                let current = values.get(&key.name).cloned().unwrap_or_default();
                egui::ComboBox::from_id_salt(&key.name)
                    .selected_text(if current.is_empty() { "Any" } else { &current })
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(current.is_empty(), "Any").clicked() {
                            values.remove(&key.name);
                            changed = true;
                        }
                        for val in key.enum_values.as_ref().unwrap_or(&vec![]) {
                            if ui.selectable_label(current == *val, val).clicked() {
                                values.insert(key.name.clone(), val.clone());
                                changed = true;
                            }
                        }
                    });
            } else if key.is_default {
                // Primary text input for default search key
                let response = TextEdit::singleline(
                    values.entry(key.name.clone()).or_insert_with(|| default_query.to_string())
                )
                    .hint_text("Search...")
                    .desired_width(150.0)
                    .show(ui);
                changed |= response.response.changed();
            } else {
                // Secondary text input
                let response = TextEdit::singleline(values.entry(key.name.clone()).or_default())
                    .hint_text(&key.display_name)
                    .desired_width(80.0)
                    .show(ui);
                changed |= response.response.changed();
            }
        }
    });
    
    changed
}
```

**Example: CBU Search UI:**

```
┌────────────────────────────────────────────────────────────────────────┐
│ Search Keys:                                                           │
│ Name: [Allianz        ]  Jurisdiction: [LU ▼]  Type: [FUND ▼]         │
│                                                                        │
│ Results:                                                               │
│ ┌──────────────────────────────────────────────────────────────────┐  │
│ │ 1. Allianz Fund I              [Select]                          │  │
│ │    LU | FUND | ACTIVE                              98%           │  │
│ └──────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

---

### Phase 2: Discriminator Metadata Flow (for Person)

**Goal:** Surface discriminator fields for entity types that have them (person, legal_entity).

**Distinction from search keys:**
- **Search keys**: What columns to search in (filter)
- **Discriminators**: What columns boost scoring (rank)

**Files:**
- `rust/src/api/resolution_routes.rs` - Add discriminator_fields to response
- `rust/crates/entity-gateway/src/service.rs` - Expose discriminator metadata
- `rust/crates/ob-poc-ui/src/panels/resolution.rs` - Render discriminator inputs

**API Addition:**

```rust
#[derive(Serialize)]
pub struct DiscriminatorFieldInfo {
    pub name: String,           // "nationality", "date_of_birth"
    pub display_name: String,   // "Nationality", "Date of Birth"
    pub field_type: String,     // "string", "date", "enum"
    pub selectivity: f32,       // 0.7, 0.95 - higher = more discriminating
    pub enum_values: Option<Vec<String>>, // For dropdowns
}
```

**UI Change:**

```rust
// resolution.rs - render discriminator fields (separate from search keys)
fn render_discriminators(ui: &mut Ui, fields: &[DiscriminatorFieldInfo], values: &mut HashMap<String, String>) {
    if fields.is_empty() {
        return; // CBU has no discriminators
    }
    
    ui.add_space(4.0);
    ui.label(RichText::new("Refinement:").small().color(Color32::GRAY));
    
    ui.horizontal(|ui| {
        for field in fields {
            ui.label(&field.display_name);
            match field.field_type.as_str() {
                "enum" => {
                    // Dropdown
                    egui::ComboBox::from_id_salt(&field.name)
                        .selected_text(values.get(&field.name).unwrap_or(&String::new()))
                        .show_ui(ui, |ui| {
                            for val in field.enum_values.as_ref().unwrap_or(&vec![]) {
                                ui.selectable_value(
                                    values.entry(field.name.clone()).or_default(),
                                    val.clone(),
                                    val,
                                );
                            }
                        });
                }
                "date" => {
                    // Date input (year-only option)
                    TextEdit::singleline(values.entry(field.name.clone()).or_default())
                        .hint_text("YYYY or YYYY-MM-DD")
                        .desired_width(100.0)
                        .show(ui);
                }
                _ => {
                    // Text input
                    TextEdit::singleline(values.entry(field.name.clone()).or_default())
                        .desired_width(80.0)
                        .show(ui);
                }
            }
        }
    });
}
```

**Example: Person Search UI (with both search keys AND discriminators):**

```
┌────────────────────────────────────────────────────────────────────────┐
│ Search Keys:                                                           │
│ Name: [John Smith     ]  First: [        ]  Last: [        ]          │
│                                                                        │
│ Refinement (boost scoring):                                            │
│ Nationality: [UK ▼]  Date of Birth: [1965-__-__]                      │
│                                                                        │
│ Results:                                                               │
│ ┌──────────────────────────────────────────────────────────────────┐  │
│ │ 1. John Smith                  [Select]                          │  │
│ │    UK | DOB 1965-03-12 | BlackRock                   98%         │  │
│ └──────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

---

### Phase 3: Search Edge Cases & Key Interactions ✅ COMPLETE

**Status:** Implemented in `search_resolution` handler:
- Edge case 1: Empty search strings - early return with hint
- Edge case 2: Query too long (>100 chars) - truncation suggestion  
- Edge case 3: Sub-key narrows to zero - fallback search without filters
- Edge case 4: Filter applied but no results - show "Found elsewhere" with fallback matches
- Edge case 5: No results at all - show suggestions including "Create new"
- Added `SuggestedAction`, `SuggestedActionType`, `SearchSuggestions` types
- Extended `ResolutionSearchResponse` with `fallback_matches`, `filtered_by`, `suggestions`

**Goal:** Handle corner cases in search behavior and key interactions.

#### Edge Case 1: Empty Search Strings

```
┌────────────────────────────────────────────────────────────────────────┐
│ Scenario: User clears the name field but has jurisdiction selected     │
│                                                                        │
│ Name: [            ]  Jurisdiction: [LU ▼]  Type: [Any ▼]             │
│                                                                        │
│ Behavior:                                                              │
│ - If default key (name) is empty but sub-keys have values:            │
│   → Search ALL entities matching sub-keys (paginated)                  │
│   → Show: "Showing all LU entities (page 1 of 23)"                    │
│                                                                        │
│ - If ALL keys are empty:                                               │
│   → Don't search, show hint: "Enter a search term"                    │
│   → OR: Show recently used entities for this type                      │
└────────────────────────────────────────────────────────────────────────┘
```

#### Edge Case 2: No Results (Complex/Impossible Query)

```
┌────────────────────────────────────────────────────────────────────────┐
│ Scenario: User searches for something that will never match            │
│                                                                        │
│ Name: [XYZ Corp 12345]  Jurisdiction: [LU ▼]                          │
│                                                                        │
│ Results: No matches found                                              │
│                                                                        │
│ Suggestions:                                                           │
│ ┌──────────────────────────────────────────────────────────────────┐  │
│ │ 💡 Try:                                                          │  │
│ │   • Clear jurisdiction filter to search all [Clear filters]      │  │
│ │   • Search by partial name: "XYZ" instead of "XYZ Corp 12345"   │  │
│ │   • Check spelling                                               │  │
│ └──────────────────────────────────────────────────────────────────┘  │
│                                                                        │
│ [+ Create New Entity]                                                  │
└────────────────────────────────────────────────────────────────────────┘
```

#### Edge Case 3: Sub-Key Narrows Results to Zero

**The "abc doesn't exist in LUX" scenario:**

```
┌────────────────────────────────────────────────────────────────────────┐
│ User flow:                                                             │
│                                                                        │
│ 1. Type "Allianz" → 15 results (works)                                │
│ 2. Select Jurisdiction: LU → 3 results (works)                        │
│ 3. Change name to "ABC Corp" → 0 results                              │
│                                                                        │
│ Problem: ABC Corp doesn't exist in LU, but might exist elsewhere       │
│                                                                        │
│ Solution: Show contextual hints                                        │
│ ┌──────────────────────────────────────────────────────────────────┐  │
│ │ No "ABC Corp" found in Luxembourg                                │  │
│ │                                                                   │  │
│ │ Found elsewhere:                                                  │  │
│ │   • ABC Corp (IE) - Ireland                    [Select]          │  │
│ │   • ABC Corporation (US) - United States       [Select]          │  │
│ │                                                                   │  │
│ │ Or: [Clear jurisdiction filter]                                   │  │
│ └──────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

**Implementation:**

```rust
// In search handler
async fn search_with_fallback(
    gateway: &mut GatewayRefResolver,
    entity_type: &str,
    search_keys: &HashMap<String, String>,
    discriminators: &HashMap<String, String>,
) -> SearchResult {
    // 1. First search with all keys
    let results = gateway.search(entity_type, search_keys, discriminators).await?;
    
    if !results.is_empty() {
        return SearchResult::Found(results);
    }
    
    // 2. If no results and we have sub-key filters, try without them
    let has_sub_keys = search_keys.iter()
        .any(|(k, v)| k != "name" && !v.is_empty());
    
    if has_sub_keys {
        let name_only: HashMap<_, _> = search_keys.iter()
            .filter(|(k, _)| *k == "name")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        
        let fallback_results = gateway.search(entity_type, &name_only, &HashMap::new()).await?;
        
        if !fallback_results.is_empty() {
            return SearchResult::FoundElsewhere {
                results: fallback_results,
                filtered_by: search_keys.iter()
                    .filter(|(k, v)| *k != "name" && !v.is_empty())
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect(),
            };
        }
    }
    
    SearchResult::NotFound
}

enum SearchResult {
    Found(Vec<EntityMatch>),
    FoundElsewhere { results: Vec<EntityMatch>, filtered_by: Vec<String> },
    NotFound,
}
```

#### Edge Case 4: Very Long Search Strings

```
┌────────────────────────────────────────────────────────────────────────┐
│ Scenario: User pastes a very long string                               │
│                                                                        │
│ Name: [The Very Long Company Name That Goes On And On Incorpora...]   │
│                                                                        │
│ Behaviors:                                                             │
│ - Truncate display at ~50 chars with "..."                            │
│ - Search uses full string (up to 256 char limit)                      │
│ - If > 256 chars, truncate and warn                                   │
│ - Consider extracting key terms for fuzzy match                        │
│                                                                        │
│ API validation:                                                        │
│ - search_keys.name: max 256 chars                                     │
│ - search_keys.* (other): max 64 chars                                 │
│ - Return 400 if exceeded                                               │
└────────────────────────────────────────────────────────────────────────┘
```

#### Edge Case 5: Reset Primary Key When Sub-Key Changes

```
┌────────────────────────────────────────────────────────────────────────┐
│ Question: Should changing jurisdiction reset the name field?           │
│                                                                        │
│ Answer: NO - keep the name, just re-search with new filter            │
│                                                                        │
│ Rationale:                                                             │
│ - User typed "Allianz" for a reason                                   │
│ - Changing jurisdiction is refining, not starting over                │
│ - If no results, show fallback (see Edge Case 3)                      │
│                                                                        │
│ Exception: "Clear all filters" button DOES reset everything           │
└────────────────────────────────────────────────────────────────────────┘
```

#### API Response Extensions

```rust
#[derive(Serialize)]
pub struct ResolutionSearchResponse {
    pub matches: Vec<EntityMatchResponse>,
    pub total: usize,
    pub truncated: bool,
    
    // NEW: Fallback results when primary search fails
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_matches: Option<Vec<EntityMatchResponse>>,
    
    // NEW: What filters narrowed results to zero
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filtered_by: Option<Vec<String>>,
    
    // NEW: Suggestions for no-result scenarios
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<SearchSuggestion>>,
}

#[derive(Serialize)]
pub struct SearchSuggestion {
    pub suggestion_type: SuggestionType,
    pub message: String,
    pub action: Option<SuggestedAction>,
}

#[derive(Serialize)]
pub enum SuggestionType {
    ClearFilters,
    TryShorterQuery,
    CheckSpelling,
    CreateNew,
}

#[derive(Serialize)]
pub struct SuggestedAction {
    pub label: String,
    pub action_type: String, // "clear_filter", "set_value", etc.
    pub payload: HashMap<String, String>,
}
```

---

### Phase 4: egui Modal Switching by Entity Profile 🔲 TODO

**Goal:** Resolution modal dynamically renders search fields based on `entity_type` from `UnresolvedRefResponse`.

#### Current State

The resolution panel has a **hardcoded single search field**:

```rust
// resolution.rs line 155
TextEdit::singleline(search_buffer)
    .hint_text("Type name or add discriminators...")  // ← HARDCODED
```

But the API now returns entity-specific configuration:

```rust
// UnresolvedRefResponse (from start_resolution)
{
  "entity_type": "cbu",
  "search_keys": [
    { "name": "name", "label": "Name", "is_default": true, "field_type": "text" },
    { "name": "jurisdiction", "label": "Jurisdiction", "field_type": "enum", "enum_values": [...] },
    { "name": "client_type", "label": "Client Type", "field_type": "enum", "enum_values": [...] }
  ],
  "discriminator_fields": [],
  "resolution_mode": "search_modal"
}
```

#### Target State

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Entity Type: CBU                           Entity Type: Person              │
│ ┌─────────────────────────────┐           ┌─────────────────────────────┐  │
│ │ Name: [Allianz        ]     │           │ Name: [John Smith     ]     │  │
│ │ Jurisdiction: [LU ▼]        │           │ ─── Discriminators ───      │  │
│ │ Client Type: [FUND ▼]       │           │ Nationality: [UK ▼]         │  │
│ └─────────────────────────────┘           │ DOB: [1965-__-__]           │  │
│                                           └─────────────────────────────┘  │
│                                                                             │
│ Entity Type: Jurisdiction (autocomplete)                                    │
│ ┌─────────────────────────────┐                                            │
│ │ [Luxembourg (LU)        ▼]  │  ← Simple dropdown, no search modal        │
│ └─────────────────────────────┘                                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Implementation Plan

**1. Extend `ResolutionPanelUi` State (`state.rs`)**

```rust
pub struct ResolutionPanelUi {
    // EXISTING
    pub selected_ref_id: Option<String>,
    pub search_query: String,
    pub chat_buffer: String,
    pub search_results: Option<ResolutionSearchResponse>,
    pub show_discriminators: bool,
    pub discriminator_values: HashMap<String, String>,
    pub show_panel: bool,
    pub messages: Vec<(String, String)>,
    pub current_ref_name: Option<String>,
    pub dsl_context: Option<String>,
    pub voice_active: bool,
    pub last_voice_transcript: Option<String>,
    
    // NEW: Entity-specific config from UnresolvedRefResponse
    pub current_entity_type: Option<String>,
    pub search_keys: Vec<SearchKeyField>,
    pub search_key_values: HashMap<String, String>,  // Multi-key search values
    pub resolution_mode: ResolutionModeHint,
}
```

**2. Extend `WindowData::Resolution` (`state.rs`)**

```rust
pub enum WindowData {
    Resolution {
        parent_session_id: String,
        subsession_id: String,
        current_ref_index: usize,
        total_refs: usize,
        // NEW: Current unresolved ref being resolved (has entity_type, search_keys)
        current_ref: Option<UnresolvedRefResponse>,
    },
    // ...
}
```

**3. Extend `ResolutionPanelData` (`resolution.rs`)**

```rust
pub struct ResolutionPanelData<'a> {
    // EXISTING
    pub window: Option<&'a WindowEntry>,
    pub matches: Option<&'a [EntityMatchDisplay]>,
    pub searching: bool,
    pub current_ref_name: Option<String>,
    pub dsl_context: Option<String>,
    pub messages: Vec<(String, String)>,
    pub voice_active: bool,
    
    // NEW: Entity-specific config
    pub entity_type: Option<String>,
    pub search_keys: &'a [SearchKeyField],
    pub search_key_values: &'a HashMap<String, String>,
    pub discriminator_fields: &'a [DiscriminatorField],
    pub resolution_mode: ResolutionModeHint,
    
    // NEW: Fallback/suggestions from search response
    pub fallback_matches: Option<&'a [EntityMatchDisplay]>,
    pub filtered_by: Option<&'a HashMap<String, String>>,
    pub suggestions: Option<&'a SearchSuggestions>,
}
```

**4. Extend `ResolutionPanelAction` (`resolution.rs`)**

```rust
pub enum ResolutionPanelAction {
    // EXISTING
    Search { query: String },
    Select { index: usize, entity_id: String },
    Skip,
    CreateNew,
    Complete { apply: bool },
    Close,
    SendMessage { message: String },
    ToggleVoice,
    
    // NEW: Multi-key search with filters
    SearchMultiKey {
        search_key_values: HashMap<String, String>,
        discriminators: HashMap<String, String>,
    },
    
    // NEW: Clear filter actions (from suggestions)
    ClearFilter { key: String },
    ClearAllFilters,
    
    // NEW: Select from fallback matches
    SelectFallback { index: usize, entity_id: String },
}
```

**5. New Rendering Functions (`resolution.rs`)**

```rust
/// Render search fields based on entity profile
fn render_search_keys(
    ui: &mut Ui,
    search_keys: &[SearchKeyField],
    values: &mut HashMap<String, String>,
) -> bool {
    let mut changed = false;
    
    ui.horizontal_wrapped(|ui| {
        for key in search_keys {
            ui.vertical(|ui| {
                ui.label(RichText::new(&key.label).small());
                
                match key.field_type {
                    SearchKeyFieldType::Text => {
                        let response = TextEdit::singleline(
                            values.entry(key.name.clone()).or_default()
                        )
                            .hint_text(if key.is_default { "Search..." } else { "" })
                            .desired_width(if key.is_default { 150.0 } else { 100.0 })
                            .show(ui);
                        changed |= response.response.changed();
                    }
                    SearchKeyFieldType::Enum => {
                        let current = values.get(&key.name).cloned().unwrap_or_default();
                        egui::ComboBox::from_id_salt(&key.name)
                            .selected_text(if current.is_empty() { "Any" } else { &current })
                            .width(100.0)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(current.is_empty(), "Any").clicked() {
                                    values.remove(&key.name);
                                    changed = true;
                                }
                                if let Some(enum_values) = &key.enum_values {
                                    for ev in enum_values {
                                        let label = format!("{} ({})", ev.display, ev.code);
                                        if ui.selectable_label(current == ev.code, &label).clicked() {
                                            values.insert(key.name.clone(), ev.code.clone());
                                            changed = true;
                                        }
                                    }
                                }
                            });
                    }
                    SearchKeyFieldType::Uuid => {
                        // Rare - just text input
                        let response = TextEdit::singleline(
                            values.entry(key.name.clone()).or_default()
                        )
                            .hint_text("UUID")
                            .desired_width(250.0)
                            .show(ui);
                        changed |= response.response.changed();
                    }
                }
            });
            ui.add_space(8.0);
        }
    });
    
    changed
}

/// Render discriminator fields for scoring refinement
fn render_discriminators(
    ui: &mut Ui,
    fields: &[DiscriminatorField],
    values: &mut HashMap<String, String>,
) -> bool {
    if fields.is_empty() {
        return false;
    }
    
    let mut changed = false;
    
    ui.add_space(4.0);
    ui.collapsing("Refinement (optional)", |ui| {
        ui.horizontal_wrapped(|ui| {
            for field in fields {
                ui.vertical(|ui| {
                    ui.label(RichText::new(&field.label).small().color(Color32::GRAY));
                    
                    match field.field_type {
                        DiscriminatorFieldType::Enum => {
                            let current = values.get(&field.name).cloned().unwrap_or_default();
                            egui::ComboBox::from_id_salt(&field.name)
                                .selected_text(if current.is_empty() { "—" } else { &current })
                                .width(80.0)
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(current.is_empty(), "—").clicked() {
                                        values.remove(&field.name);
                                        changed = true;
                                    }
                                    if let Some(enum_values) = &field.enum_values {
                                        for ev in enum_values {
                                            if ui.selectable_label(current == ev.code, &ev.display).clicked() {
                                                values.insert(field.name.clone(), ev.code.clone());
                                                changed = true;
                                            }
                                        }
                                    }
                                });
                        }
                        DiscriminatorFieldType::Date => {
                            let response = TextEdit::singleline(
                                values.entry(field.name.clone()).or_default()
                            )
                                .hint_text("YYYY or YYYY-MM-DD")
                                .desired_width(100.0)
                                .show(ui);
                            changed |= response.response.changed();
                        }
                        DiscriminatorFieldType::String => {
                            let response = TextEdit::singleline(
                                values.entry(field.name.clone()).or_default()
                            )
                                .desired_width(80.0)
                                .show(ui);
                            changed |= response.response.changed();
                        }
                    }
                });
                ui.add_space(8.0);
            }
        });
    });
    
    changed
}

/// Render autocomplete dropdown for reference data (jurisdiction, role, etc.)
fn render_autocomplete_resolution(
    ui: &mut Ui,
    matches: &[EntityMatchDisplay],
    current_value: &str,
) -> Option<ResolutionPanelAction> {
    let mut action = None;
    
    ui.label("Select:");
    egui::ComboBox::from_id_salt("ref_autocomplete")
        .selected_text(if current_value.is_empty() { "Select..." } else { current_value })
        .width(250.0)
        .show_ui(ui, |ui| {
            for (idx, m) in matches.iter().enumerate() {
                let label = format!("{} ({})", m.name, m.id);
                if ui.selectable_label(false, &label).clicked() {
                    action = Some(ResolutionPanelAction::Select {
                        index: idx,
                        entity_id: m.id.clone(),
                    });
                }
            }
        });
    
    action
}

/// Render suggestions when no results found
fn render_suggestions(
    ui: &mut Ui,
    suggestions: &SearchSuggestions,
) -> Option<ResolutionPanelAction> {
    let mut action = None;
    
    ui.add_space(8.0);
    egui::Frame::default()
        .fill(Color32::from_rgb(50, 45, 40))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.label(RichText::new("💡").small());
            ui.label(&suggestions.message);
            
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for suggested in &suggestions.actions {
                    match &suggested.action {
                        SuggestedActionType::ClearFilters => {
                            if ui.button(&suggested.label).clicked() {
                                action = Some(ResolutionPanelAction::ClearAllFilters);
                            }
                        }
                        SuggestedActionType::ClearFilter { key } => {
                            if ui.button(&suggested.label).clicked() {
                                action = Some(ResolutionPanelAction::ClearFilter { key: key.clone() });
                            }
                        }
                        SuggestedActionType::CreateNew => {
                            if ui.button(&suggested.label).clicked() {
                                action = Some(ResolutionPanelAction::CreateNew);
                            }
                        }
                        _ => {}
                    }
                }
            });
        });
    
    action
}

/// Render fallback matches (found elsewhere)
fn render_fallback_matches(
    ui: &mut Ui,
    matches: &[EntityMatchDisplay],
    filtered_by: &HashMap<String, String>,
) -> Option<ResolutionPanelAction> {
    let mut action = None;
    
    ui.add_space(4.0);
    ui.label(RichText::new("Found elsewhere:").small().color(Color32::YELLOW));
    
    // Show what filters were applied
    let filter_text: Vec<_> = filtered_by.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    ui.label(RichText::new(format!("(not in {})", filter_text.join(", "))).small().color(Color32::GRAY));
    
    for (idx, m) in matches.iter().enumerate() {
        egui::Frame::default()
            .fill(Color32::from_rgb(55, 50, 45))
            .inner_margin(6.0)
            .rounding(4.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.small_button("Select").clicked() {
                        action = Some(ResolutionPanelAction::SelectFallback {
                            index: idx,
                            entity_id: m.id.clone(),
                        });
                    }
                    ui.label(&m.name);
                    if let Some(ref details) = m.details {
                        ui.label(RichText::new(details).small().color(Color32::LIGHT_GRAY));
                    }
                });
            });
    }
    
    action
}
```

**6. Update Main Render Function (`resolution.rs`)**

```rust
fn render_resolution_content(
    ui: &mut Ui,
    search_key_values: &mut HashMap<String, String>,  // NEW: replaces search_buffer
    discriminator_values: &mut HashMap<String, String>,
    chat_buffer: &mut String,
    data: &ResolutionPanelData<'_>,
    _subsession_id: &str,
    current_ref_index: usize,
    total_refs: usize,
) -> Option<ResolutionPanelAction> {
    let mut action: Option<ResolutionPanelAction> = None;

    // ... header code unchanged ...

    // SWITCH ON RESOLUTION MODE
    match data.resolution_mode {
        ResolutionModeHint::Autocomplete => {
            // Simple dropdown for reference data
            if let Some(matches) = data.matches {
                let current = search_key_values.get("name").cloned().unwrap_or_default();
                action = render_autocomplete_resolution(ui, matches, &current);
            }
        }
        ResolutionModeHint::SearchModal => {
            // Full search modal with multiple fields
            
            // 1. Render search keys (multi-field)
            let keys_changed = render_search_keys(ui, data.search_keys, search_key_values);
            
            // 2. Render discriminators (if any)
            let discrim_changed = render_discriminators(ui, data.discriminator_fields, discriminator_values);
            
            // 3. Trigger search on change (debounced in app.rs)
            if keys_changed || discrim_changed {
                action = Some(ResolutionPanelAction::SearchMultiKey {
                    search_key_values: search_key_values.clone(),
                    discriminators: discriminator_values.clone(),
                });
            }
            
            ui.separator();
            
            // 4. Results area
            if let Some(matches) = data.matches {
                if matches.is_empty() {
                    // No direct matches - check for suggestions/fallback
                    if let Some(suggestions) = data.suggestions {
                        if let Some(a) = render_suggestions(ui, suggestions) {
                            action = Some(a);
                        }
                    }
                    if let (Some(fallback), Some(filtered_by)) = (data.fallback_matches, data.filtered_by) {
                        if !fallback.is_empty() {
                            if let Some(a) = render_fallback_matches(ui, fallback, filtered_by) {
                                action = Some(a);
                            }
                        }
                    }
                } else {
                    // Show matches
                    for (idx, m) in matches.iter().enumerate() {
                        if let Some(select_action) = render_match_row(ui, idx, m) {
                            action = Some(select_action);
                        }
                    }
                }
            }
        }
    }

    // ... rest of chat, buttons unchanged ...
    
    action
}
```

**7. Wire Action Handling in `app.rs`**

```rust
// In handle_resolution_action()
fn handle_resolution_action(&mut self, action: ResolutionPanelAction, state: &mut AsyncState) {
    match action {
        ResolutionPanelAction::SearchMultiKey { search_key_values, discriminators } => {
            // Get current ref_id
            if let Some(ref_id) = &self.resolution_ui.selected_ref_id {
                // Debounce: only search if changed (300ms timer)
                let request = ResolutionSearchRequest {
                    ref_id: ref_id.clone(),
                    query: search_key_values.get("name").cloned().unwrap_or_default(),
                    search_key: None, // Use default
                    filters: search_key_values.iter()
                        .filter(|(k, v)| *k != "name" && !v.is_empty())
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                    discriminators,
                    limit: Some(10),
                };
                // Spawn search request
                self.spawn_resolution_search(request, state);
            }
        }
        ResolutionPanelAction::ClearFilter { key } => {
            self.resolution_ui.search_key_values.remove(&key);
            // Re-trigger search
            self.trigger_resolution_search(state);
        }
        ResolutionPanelAction::ClearAllFilters => {
            // Keep only the name field
            let name = self.resolution_ui.search_key_values.get("name").cloned();
            self.resolution_ui.search_key_values.clear();
            if let Some(n) = name {
                self.resolution_ui.search_key_values.insert("name".to_string(), n);
            }
            // Re-trigger search
            self.trigger_resolution_search(state);
        }
        ResolutionPanelAction::SelectFallback { index, entity_id } => {
            // Same as Select but clears filters first
            self.resolution_ui.search_key_values.retain(|k, _| k == "name");
            self.handle_resolution_select(index, entity_id, state);
        }
        // ... existing handlers ...
    }
}
```

**8. Populate State from API Response (`app.rs`)**

```rust
// When resolution session starts, populate UI state from first unresolved ref
fn on_resolution_start(&mut self, response: ResolutionSessionResponse) {
    if let Some(first_unresolved) = response.unresolved.first() {
        // Populate entity-specific config
        self.resolution_ui.current_entity_type = Some(first_unresolved.entity_type.clone());
        self.resolution_ui.search_keys = first_unresolved.search_keys.clone();
        self.resolution_ui.resolution_mode = first_unresolved.resolution_mode.clone();
        
        // Pre-populate name from search_value
        self.resolution_ui.search_key_values.clear();
        self.resolution_ui.search_key_values.insert(
            "name".to_string(), 
            first_unresolved.search_value.clone()
        );
        
        // Clear discriminators
        self.resolution_ui.discriminator_values.clear();
    }
}

// When moving to next ref, update UI state
fn on_next_ref(&mut self, ref_response: &UnresolvedRefResponse) {
    self.resolution_ui.current_entity_type = Some(ref_response.entity_type.clone());
    self.resolution_ui.search_keys = ref_response.search_keys.clone();
    self.resolution_ui.resolution_mode = ref_response.resolution_mode.clone();
    
    self.resolution_ui.search_key_values.clear();
    self.resolution_ui.search_key_values.insert(
        "name".to_string(), 
        ref_response.search_value.clone()
    );
    self.resolution_ui.discriminator_values.clear();
}
```

#### Files to Modify

| File | Changes |
|------|---------|
| `ob-poc-ui/src/state.rs` | Extend `ResolutionPanelUi` with multi-key fields |
| `ob-poc-ui/src/panels/resolution.rs` | New render functions, mode switching |
| `ob-poc-ui/src/app.rs` | Action handlers, state population |
| `ob-poc-ui/src/api.rs` | Update API call to use multi-key request |

#### Debounce Strategy

To avoid excessive API calls:

```rust
// In app.rs state
pub last_search_trigger: Option<Instant>,

// In update loop
if let Some(trigger_time) = self.last_search_trigger {
    if trigger_time.elapsed() >= Duration::from_millis(300) {
        self.execute_pending_search(state);
        self.last_search_trigger = None;
    }
}

// When search key changes
self.last_search_trigger = Some(Instant::now());
```

---

### Phase 5: Incremental Search with Keys + Discriminators

**Goal:** Re-search as user changes search keys or discriminator values.

**Files:**
- `rust/src/api/resolution_routes.rs` - Accept search_keys + discriminators in search request
- `rust/crates/entity-gateway/src/index/tantivy_index.rs` - Already supports discriminator scoring
- `rust/crates/ob-poc-ui/src/panels/resolution.rs` - Debounced search on any field change

**API Change:**

```rust
// POST /api/session/:id/resolution/search
#[derive(Deserialize)]
pub struct ResolutionSearchRequest {
    pub ref_id: String,
    // NEW: Search key values (filter)
    pub search_keys: HashMap<String, String>,  // { "name": "Allianz", "jurisdiction": "LU" }
    // NEW: Discriminator values (boost scoring)
    pub discriminators: HashMap<String, String>, // { "nationality": "UK" }
    pub limit: Option<usize>,
}
```

**Flow:**

```
User selects Jurisdiction dropdown: "LU"
    │
    ▼ (debounced 300ms)
POST /api/session/:id/resolution/search
{
    "ref_id": "ref_0",
    "search_keys": { "name": "Allianz", "jurisdiction": "LU" },
    "discriminators": {}
}
    │
    ▼
EntityGateway.search() with multi-key filter
    │
    ▼ 
Results filtered: Only LU jurisdictions returned
```

```
User types in Nationality field: "UK" (for person search)
    │
    ▼ (debounced 300ms)
POST /api/session/:id/resolution/search
{
    "ref_id": "ref_0",
    "search_keys": { "name": "John Smith" },
    "discriminators": { "nationality": "UK" }
}
    │
    ▼
EntityGateway.search() with discriminator boost
    │
    ▼ 
Results re-ranked: UK persons score higher
```

---

### Phase 4: Confirmation → AST Update

**Goal:** Wire confirmation to update AST with resolved primary_key.

**Files:**
- `rust/src/api/resolution_routes.rs` - Confirm endpoint updates session AST
- `rust/src/api/session.rs` - AST mutation methods
- `rust/crates/ob-poc-ui/src/panels/resolution.rs` - Handle confirm action

**Key Logic:**

```rust
// POST /api/session/:id/resolution/confirm
pub async fn confirm_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ConfirmResolutionRequest>,
) -> Result<Json<ConfirmResolutionResponse>, StatusCode> {
    let mut sessions = state.session_store.sessions.write().await;
    let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;
    
    // Get the resolved value (UUID or CODE)
    let resolved_value = req.selected_id.clone();
    
    // Update AST - find the EntityRef and set resolved field
    if let Some(ref mut ast) = session.pending_ast {
        update_entity_ref_in_ast(ast, &req.ref_id, &resolved_value)?;
    }
    
    // Mark ref as resolved in resolution state
    if let Some(ref mut resolution) = session.resolution {
        resolution.resolved.insert(req.ref_id.clone(), resolved_value.clone());
    }
    
    Ok(Json(ConfirmResolutionResponse {
        ref_id: req.ref_id,
        resolved_value,
        remaining_count: resolution.unresolved.len() - resolution.resolved.len(),
    }))
}
```

---

### Phase 5: DSL Editor Update

**Goal:** Reflect resolved values in the DSL source.

**Options:**

| Approach | Pros | Cons |
|----------|------|------|
| **A: Replace in source** | Simple, visible | Loses original name context |
| **B: Annotation comment** | Preserves name | Clutters source |
| **C: Symbol binding** | Clean, declarative | Requires parser support |

**Recommendation: C - Symbol binding**

```dsl
; Original (unresolved)
(entity.create :name "John Smith" :as @john)

; After resolution (symbol bound to UUID)
; @john is now bound to 550e8400-e29b-41d4-a716-446655440000
(entity.create :name "John Smith" :as @john)

; Subsequent usage automatically resolves
(cbu.add-role :entity-id @john :role DIRECTOR)
```

The symbol table already exists. Resolution just needs to pre-bind symbols.

**Alternative for non-symbol cases:**

```dsl
; Original
(entity.create :entity-id "John Smith")

; After resolution - replace with resolved ID
(entity.create :entity-id "550e8400-e29b-41d4-a716-446655440000")
```

---

### Phase 5: Reference Data Mode

**Goal:** Handle CODE lookups (jurisdiction, role, etc.) with dropdown UI.

**Key Difference:**

| Entity Type | Lookup Mode | UI | Return |
|-------------|-------------|-----|--------|
| `person`, `cbu`, `entity` | Search modal | Fuzzy search + results | UUID |
| `jurisdiction`, `role`, `currency` | Autocomplete dropdown | Filtered list | CODE |

**Detection:**

```rust
// In resolution_routes.rs
fn resolution_mode_for_entity_type(entity_type: &str) -> ResolutionMode {
    match entity_type {
        "jurisdiction" | "role" | "currency" | "product" | 
        "client_type" | "case_type" | "screening_type" | 
        "risk_rating" | "settlement_type" | "ssi_type" => ResolutionMode::Autocomplete,
        _ => ResolutionMode::Search,
    }
}
```

**UI:**

```rust
// For Autocomplete mode - simple dropdown
fn render_autocomplete_resolution(
    ui: &mut Ui,
    entity_type: &str,
    current_value: &str,
    matches: &[EntityMatchResponse],
) -> Option<ResolutionPanelAction> {
    let mut action = None;
    
    egui::ComboBox::from_id_salt("ref_autocomplete")
        .selected_text(current_value)
        .show_ui(ui, |ui| {
            for m in matches {
                if ui.selectable_label(false, &m.display).clicked() {
                    action = Some(ResolutionPanelAction::Select {
                        index: 0,
                        entity_id: m.id.clone(), // This is the CODE, not UUID
                    });
                }
            }
        });
    
    action
}
```

---

## Verification Checklist

### Phase 1: Discriminator Metadata
- [ ] EntityGateway exposes discriminator config via gRPC
- [ ] Resolution API includes discriminator_fields in response
- [ ] UI renders discriminator input fields
- [ ] Fields match entity_index.yaml config

### Phase 2: Incremental Search
- [ ] Search request accepts discriminators
- [ ] EntityGateway applies discriminator boost
- [ ] UI debounces discriminator changes (300ms)
- [ ] Results re-rank on discriminator input

### Phase 3: AST Update
- [ ] Confirm endpoint updates session AST
- [ ] EntityRef.resolved field populated
- [ ] Symbol table updated with binding
- [ ] Remaining count accurate

### Phase 4: Editor Update
- [ ] DSL editor reflects resolved state
- [ ] Symbol bindings visible
- [ ] can_execute transitions to true when all resolved

### Phase 5: Reference Data
- [ ] Detection of CODE vs UUID return types
- [ ] Dropdown UI for reference data
- [ ] Codes inserted correctly in DSL

---

## Files to Modify

| File | Changes |
|------|---------|
| `entity-gateway/src/service.rs` | Expose index config metadata |
| `entity-gateway/proto/gateway.proto` | Add GetIndexConfig RPC |
| `src/api/resolution_routes.rs` | Add discriminator_fields, accept discriminators in search |
| `src/api/session.rs` | AST mutation methods |
| `ob-poc-ui/src/panels/resolution.rs` | Discriminator UI, autocomplete mode |
| `ob-poc-ui/src/state.rs` | Discriminator values in resolution state |
| `ob-poc-ui/src/api.rs` | Resolution API client updates |

---

## Test Cases

```rust
#[test]
fn test_person_resolution_with_discriminators() {
    // Search "John Smith" → multiple matches
    // Add nationality "UK" → results re-ranked
    // Add DOB "1965" → single high-confidence match
    // Confirm → UUID inserted in AST
}

#[test]
fn test_jurisdiction_resolution_autocomplete() {
    // Type "Lu" → dropdown shows Luxembourg (LU)
    // Select → CODE "LU" inserted in DSL
}

#[test]
fn test_resolution_updates_can_execute() {
    // DSL with 2 unresolved refs → can_execute: false
    // Resolve first → can_execute: false (1 remaining)
    // Resolve second → can_execute: true
}
```

---

## Summary

The entity resolution system has strong foundations (EntityGateway, Tantivy, discriminator scoring). The gaps are in **wiring**:

1. **Config → UI**: Discriminator fields defined in YAML but not surfaced
2. **UI → Search**: Incremental refinement not connected
3. **Confirm → AST**: Selection doesn't update the pending DSL
4. **CODE vs UUID**: Assumes UUID everywhere

This plan addresses each gap with minimal new infrastructure - mostly connecting existing pieces.
