# Entity Linking Service Implementation Plan (073)

**Status:** Ready for implementation  
**Approach:** Adapt TODO v2 to existing schema, leverage lexicon patterns  
**Companion:** `ai-thoughts/072-lexicon-service-implementation-plan.md`

---

## Executive Summary

The TODO v2 spec proposes creating new `entity`, `entity_alias`, `entity_concept_link`, and `entity_feature` tables. However, the codebase **already has**:

| TODO v2 Table | Existing Equivalent | Notes |
|---------------|---------------------|-------|
| `entity` | `ob-poc.entities` | 1,452 rows, full schema |
| `entity_alias` | `agent.entity_aliases` | Exists but empty (0 rows) |
| `entity_alias` | `ob-poc.entity_names` | 462 rows, name_type enum |
| `entity_concept_link` | None | Need to create |
| `entity_feature` | None | Need to create |

**Key decision:** Adapt to existing schema rather than create parallel tables.

---

## Phase 1: Schema Additions (Minimal)

Only add what's missing. Don't duplicate existing tables.

### 1.1 Migration: `073_entity_linking_support.sql`

```sql
-- Entity concept links (for disambiguation via industry/domain)
CREATE TABLE IF NOT EXISTS "ob-poc".entity_concept_link (
    entity_id   UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    concept_id  TEXT NOT NULL,
    relation    TEXT NOT NULL DEFAULT 'related',
    weight      REAL NOT NULL DEFAULT 1.0 CHECK (weight >= 0.0 AND weight <= 1.0),
    provenance  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (entity_id, concept_id, relation)
);

CREATE INDEX IF NOT EXISTS idx_ecl_concept ON "ob-poc".entity_concept_link(concept_id);
CREATE INDEX IF NOT EXISTS idx_ecl_entity ON "ob-poc".entity_concept_link(entity_id);

-- Token features for fuzzy matching (populated by compiler)
CREATE TABLE IF NOT EXISTS "ob-poc".entity_feature (
    entity_id   UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    token_norm  TEXT NOT NULL,
    weight      REAL NOT NULL DEFAULT 1.0 CHECK (weight >= 0.0 AND weight <= 1.0),
    source      TEXT NOT NULL DEFAULT 'canonical_name',
    PRIMARY KEY (entity_id, token_norm)
);

CREATE INDEX IF NOT EXISTS idx_ef_token ON "ob-poc".entity_feature(token_norm);

-- Add normalized name column to entities if missing
ALTER TABLE "ob-poc".entities 
ADD COLUMN IF NOT EXISTS name_norm TEXT;

-- Populate name_norm from name
UPDATE "ob-poc".entities 
SET name_norm = LOWER(TRIM(REGEXP_REPLACE(name, '[^a-zA-Z0-9 ]', ' ', 'g')))
WHERE name_norm IS NULL;

-- Create index on name_norm
CREATE INDEX IF NOT EXISTS idx_entities_name_norm ON "ob-poc".entities(name_norm);

-- Trigger to maintain name_norm
CREATE OR REPLACE FUNCTION "ob-poc".update_entity_name_norm()
RETURNS TRIGGER AS $$
BEGIN
    NEW.name_norm = LOWER(TRIM(REGEXP_REPLACE(NEW.name, '[^a-zA-Z0-9 ]', ' ', 'g')));
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_entity_name_norm ON "ob-poc".entities;
CREATE TRIGGER trg_entity_name_norm
    BEFORE INSERT OR UPDATE OF name ON "ob-poc".entities
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_entity_name_norm();
```

---

## Phase 2: Module Structure

Mirror the lexicon module pattern for consistency.

```
rust/src/entity_linking/
├── mod.rs              # Re-exports
├── normalize.rs        # Text normalization (shared with lexicon)
├── snapshot.rs         # EntitySnapshot (in-memory)
├── mention.rs          # MentionExtractor (n-gram scanning)
├── resolver.rs         # EntityLinkingService trait + impl
└── compiler.rs         # Snapshot compiler from DB
```

### 2.1 Key Differences from Lexicon

| Aspect | Lexicon | Entity Linking |
|--------|---------|----------------|
| Source | Verb YAML | DB tables |
| Snapshot file | `lexicon.snapshot.bin` | `entity.snapshot.bin` |
| Hot path | `search_verbs()` | `resolve_mentions()` |
| Index type | Label→verbs, token→verbs | Alias→entities, token→entities |
| Disambiguation | Target type matching | Kind constraint + concepts |

### 2.2 Reuse from Lexicon

- `normalize.rs` - Can share normalization functions
- Snapshot pattern - Bincode serialization, version field, content hash
- Evidence enum pattern - Serializable, tagged enum
- Compiler pattern - Async DB load → build indexes → serialize

---

## Phase 3: Core Types

### 3.1 Snapshot (`snapshot.rs`)

```rust
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::HashMap;
use uuid::Uuid;

pub type EntityId = Uuid;

pub const SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub version: u32,
    pub hash: String,
    pub entities: Vec<EntityRow>,
    
    // Primary lookup: normalized alias → entity IDs
    pub alias_index: HashMap<String, SmallVec<[EntityId; 4]>>,
    
    // Canonical name lookup (unique)
    pub name_index: HashMap<String, EntityId>,
    
    // Token overlap index: token → entity IDs
    pub token_index: HashMap<String, SmallVec<[EntityId; 8]>>,
    
    // Concept links for disambiguation
    pub concept_links: HashMap<EntityId, SmallVec<[(String, f32); 8]>>,
    
    // Entity type lookup
    pub kind_index: HashMap<String, SmallVec<[EntityId; 16]>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRow {
    pub entity_id: EntityId,
    pub entity_kind: String,       // From entity_types.name
    pub canonical_name: String,    // entities.name
    pub canonical_name_norm: String, // entities.name_norm
}
```

### 3.2 Evidence Types (`resolver.rs`)

Directly from TODO v2 - these are well-designed:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Evidence {
    #[serde(rename = "alias_exact")]
    AliasExact { alias: String },
    
    #[serde(rename = "alias_token_overlap")]
    AliasTokenOverlap { tokens: Vec<String>, overlap: f32 },
    
    #[serde(rename = "kind_match_boost")]
    KindMatchBoost { expected: String, actual: String, boost: f32 },
    
    #[serde(rename = "kind_mismatch_penalty")]
    KindMismatchPenalty { expected: String, actual: String, penalty: f32 },
    
    #[serde(rename = "concept_overlap_boost")]
    ConceptOverlapBoost { concepts: Vec<String>, boost: f32 },
}
```

### 3.3 MentionExtractor (`mention.rs`)

The n-gram mention extraction from TODO v2 is critical. Key changes:
- Use existing normalization from lexicon
- Return `MentionSpan` with character positions for UI highlighting

---

## Phase 4: Compiler

### 4.1 Data Sources

| Index | Source Table | Query |
|-------|--------------|-------|
| `entities` | `ob-poc.entities` | Active entities with type join |
| `alias_index` | `ob-poc.entity_names` + `agent.entity_aliases` | Union of all name variants |
| `token_index` | `ob-poc.entity_feature` + derived | Tokens from names/aliases |
| `concept_links` | `ob-poc.entity_concept_link` | Industry/domain concepts |
| `kind_index` | `ob-poc.entities` | Group by entity_type |

### 4.2 Compiler SQL

```rust
async fn load_entities(pool: &PgPool) -> Result<Vec<EntityRow>> {
    sqlx::query_as!(EntityRow,
        r#"SELECT 
            e.entity_id,
            et.name as entity_kind,
            e.name as canonical_name,
            COALESCE(e.name_norm, LOWER(e.name)) as canonical_name_norm
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        ORDER BY e.entity_id"#
    ).fetch_all(pool).await
}

async fn load_aliases(pool: &PgPool) -> Result<Vec<(EntityId, String, f32)>> {
    // Union entity_names + entity_aliases
    sqlx::query!(
        r#"SELECT entity_id, LOWER(name) as alias_norm, 1.0 as weight
           FROM "ob-poc".entity_names
           UNION ALL
           SELECT entity_id, LOWER(alias) as alias_norm, confidence as weight
           FROM agent.entity_aliases
           WHERE entity_id IS NOT NULL"#
    ).fetch_all(pool).await
}
```

---

## Phase 5: Integration

### 5.1 Where Entity Linking Fits

```
User: "Set up Goldman Sachs for OTC trading"
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│  VERB SEARCH (existing)                                  │
│  HybridVerbSearcher.search() → "trading-profile.create" │
└─────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│  ENTITY LINKING (new)                                    │
│  EntityLinkingService.resolve_mentions()                │
│  → Extract "Goldman Sachs" at positions [8, 21]         │
│  → Match to entity_id with evidence                     │
└─────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│  DSL GENERATION (existing)                               │
│  LLM generates: (trading-profile.create                 │
│                   :entity-id <Goldman Sachs>)           │
└─────────────────────────────────────────────────────────┘
```

### 5.2 LookupService (Unified Layer)

The TODO v2 `LookupService` is the right abstraction:

```rust
pub struct LookupService {
    entity_linker: Arc<dyn EntityLinkingService>,
    lexicon: Arc<dyn LexiconService>,
}

impl LookupService {
    /// Verb-first ordering: verbs → expected_kinds → entities
    pub fn analyze(&self, utterance: &str) -> LookupResult {
        // 1. Extract concepts
        let concepts = self.lexicon.extract_concepts(utterance);
        
        // 2. Verb search
        let verbs = self.lexicon.search_verbs(utterance, None, 10);
        
        // 3. Derive expected kinds from verb target_types
        let expected_kinds: Vec<String> = verbs.iter()
            .take(3)
            .flat_map(|v| self.lexicon.verb_target_types(&v.dsl_verb))
            .collect();
        
        // 4. Entity resolution with kind constraint
        let entities = self.entity_linker.resolve_mentions(
            utterance,
            Some(&expected_kinds),
            Some(&concepts),
            10,
        );
        
        LookupResult { entities, verbs, concepts, expected_kinds }
    }
}
```

---

## Phase 6: xtask Commands

```bash
# Compile entity snapshot from DB
cargo x entity-compile

# Lint entity data quality
cargo x entity-lint

# Show entity snapshot stats
cargo x entity-stats
```

---

## Phase 7: Tests

Adapt TODO v2 tests but use existing data:

```rust
#[tokio::test]
async fn test_exact_alias() {
    let svc = EntityLinkingServiceImpl::load_default().unwrap();
    let results = svc.resolve_mentions("Allianz", None, None, 5);
    assert!(!results.is_empty());
    assert!(results[0].selected.is_some());
}

#[tokio::test]
async fn test_multi_mention() {
    let svc = EntityLinkingServiceImpl::load_default().unwrap();
    let results = svc.resolve_mentions("Allianz and BlackRock", None, None, 5);
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_kind_constraint() {
    let svc = EntityLinkingServiceImpl::load_default().unwrap();
    
    // Without constraint - might match person or company
    let r1 = svc.resolve_mentions("John Smith", None, None, 5);
    
    // With person constraint
    let r2 = svc.resolve_mentions("John Smith", Some(&["proper_person".to_string()]), None, 5);
    
    // r2 should have higher confidence if it's actually a person
}
```

---

## File Checklist

```
rust/
├── migrations/073_entity_linking_support.sql   # Schema additions
├── src/
│   ├── entity_linking/
│   │   ├── mod.rs
│   │   ├── normalize.rs      # Text normalization
│   │   ├── snapshot.rs       # EntitySnapshot type
│   │   ├── mention.rs        # MentionExtractor
│   │   ├── resolver.rs       # EntityLinkingService
│   │   └── compiler.rs       # DB → snapshot compiler
│   ├── lookup/
│   │   ├── mod.rs
│   │   └── service.rs        # Unified LookupService
│   └── lib.rs                # Add pub mod entity_linking, lookup
├── xtask/src/
│   ├── entity.rs             # compile/lint/stats commands
│   └── main.rs               # Add entity subcommand
├── assets/
│   └── entity.snapshot.bin   # Compiled snapshot
└── tests/
    └── entity_linking_test.rs
```

---

## Implementation Order

1. **Migration** - Add entity_concept_link, entity_feature, name_norm
2. **normalize.rs** - Share/extend from lexicon
3. **snapshot.rs** - EntitySnapshot struct
4. **compiler.rs** - DB → snapshot (can test early)
5. **mention.rs** - N-gram extraction
6. **resolver.rs** - EntityLinkingService impl
7. **xtask commands** - entity-compile, entity-lint
8. **Tests** - Unit + integration
9. **lookup/service.rs** - Unified layer (optional, can defer)

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Empty entity_aliases table | Populate from entity_names first |
| Missing concept links | Start with empty, add later |
| Performance regression | Same snapshot pattern as lexicon (<100µs) |
| Breaking existing EntityGateway | This is additive, doesn't replace Gateway |

---

## Not Implemented (Deferred)

- **Embedding-based semantic matching** - Token overlap is sufficient for v1
- **Entity deduplication/merging** - Out of scope
- **Real-time learning** - Use existing agent.entity_aliases pattern
- **UI disambiguation picker** - Existing picker works with new candidates
