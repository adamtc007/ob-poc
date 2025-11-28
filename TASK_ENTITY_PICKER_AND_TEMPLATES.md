# Task: Entity Picker Service & Template-Driven Forms

## Objective

Replace free-form entity references with a typed, searchable entity picker system. CBU operations become template-driven forms with smart typeahead for all entity slots.

**Core Insight**: CBU = Bundle of FKs. Every FK slot needs a typed entity picker.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              TEMPLATE FORM                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  Template: attach-beneficial-owner                                           │
│                                                                              │
│  CBU:        [Apex Capital     ] ← EntityRef(CBU) - from session context    │
│                                                                              │
│  Entity:     [john sm________🔍] ← EntityRef(PERSON|COMPANY) - TYPEAHEAD    │
│              ┌──────────────────────────────────────────┐                   │
│              │ John Smith (PERSON) GB • 1985-03-15     │                   │
│              │ John Smythe (PERSON) US • 1990-01-01    │                   │
│              │ Smith Holdings Ltd (COMPANY) LU         │                   │
│              │ ──────────────────────────────────────── │                   │
│              │ + Create New Person                      │                   │
│              │ + Create New Company                     │                   │
│              └──────────────────────────────────────────┘                   │
│                                                                              │
│  Ownership:  [25.5    ]% ← Percentage                                       │
│  Role:       [BENEFICIAL_OWNER ▼] ← Enum                                    │
│                                                                              │
│  [Submit]                                                                    │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SERVER PIPELINE                                      │
│  Template + Filled Slots → Validate → Assemble DSL → Execute → Update Context│
└─────────────────────────────────────────────────────────────────────────────┘
```

## Part 1: PostgreSQL pg_trgm Setup

### Migration: `migrations/20251127_add_entity_search_indexes.sql`

```sql
-- Enable pg_trgm extension for fuzzy text search
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- =============================================================================
-- Person search index (combined name)
-- =============================================================================
-- Add computed column for full name search
ALTER TABLE "ob-poc".proper_persons 
ADD COLUMN IF NOT EXISTS search_name TEXT 
GENERATED ALWAYS AS (
    COALESCE(given_name, '') || ' ' || COALESCE(family_name, '')
) STORED;

-- Trigram index for fuzzy matching
CREATE INDEX IF NOT EXISTS idx_persons_search_name_trgm 
ON "ob-poc".proper_persons 
USING gin (search_name gin_trgm_ops);

-- Also index individual name parts for prefix search
CREATE INDEX IF NOT EXISTS idx_persons_given_name_trgm 
ON "ob-poc".proper_persons 
USING gin (given_name gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_persons_family_name_trgm 
ON "ob-poc".proper_persons 
USING gin (family_name gin_trgm_ops);

-- =============================================================================
-- Company search index
-- =============================================================================
CREATE INDEX IF NOT EXISTS idx_companies_name_trgm 
ON "ob-poc".limited_companies 
USING gin (company_name gin_trgm_ops);

-- Also registration number for exact lookups
CREATE INDEX IF NOT EXISTS idx_companies_reg_number 
ON "ob-poc".limited_companies (registration_number);

-- =============================================================================
-- CBU search index
-- =============================================================================
CREATE INDEX IF NOT EXISTS idx_cbu_name_trgm 
ON "ob-poc".cbu 
USING gin (cbu_name gin_trgm_ops);

-- =============================================================================
-- Document search index
-- =============================================================================
CREATE INDEX IF NOT EXISTS idx_documents_title_trgm 
ON "ob-poc".document_catalog 
USING gin (title gin_trgm_ops);

-- =============================================================================
-- Trust search index (if table exists)
-- =============================================================================
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'ob-poc' AND table_name = 'trusts') THEN
        CREATE INDEX IF NOT EXISTS idx_trusts_name_trgm 
        ON "ob-poc".trusts 
        USING gin (trust_name gin_trgm_ops);
    END IF;
END $$;

-- =============================================================================
-- Unified entity view for cross-type search
-- =============================================================================
CREATE OR REPLACE VIEW "ob-poc".entity_search_view AS

-- Persons
SELECT 
    entity_id as id,
    'PERSON' as entity_type,
    given_name || ' ' || family_name as display_name,
    nationality as subtitle_1,
    date_of_birth::text as subtitle_2,
    search_name as search_text
FROM "ob-poc".proper_persons
WHERE entity_id IS NOT NULL

UNION ALL

-- Companies
SELECT 
    entity_id as id,
    'COMPANY' as entity_type,
    company_name as display_name,
    jurisdiction as subtitle_1,
    registration_number as subtitle_2,
    company_name as search_text
FROM "ob-poc".limited_companies
WHERE entity_id IS NOT NULL

UNION ALL

-- CBUs
SELECT 
    cbu_id as id,
    'CBU' as entity_type,
    cbu_name as display_name,
    client_type as subtitle_1,
    jurisdiction as subtitle_2,
    cbu_name as search_text
FROM "ob-poc".cbu
WHERE cbu_id IS NOT NULL;

-- Index the view via materialized view for performance (optional, refresh periodically)
-- For now, query the view directly - it's fast enough with underlying indexes
```

### Verify pg_trgm Works

```sql
-- Test query - should return results with similarity scores
SELECT 
    display_name, 
    entity_type,
    similarity(search_text, 'john sm') as score
FROM "ob-poc".entity_search_view
WHERE search_text % 'john sm'  -- % is the similarity operator (default threshold 0.3)
ORDER BY score DESC
LIMIT 10;
```

## Part 2: Entity Search Service (Rust)

### Create `rust/src/services/entity_search.rs`

```rust
//! Generic Entity Search Service
//!
//! Provides fast, fuzzy search across all entity types using PostgreSQL pg_trgm.
//! Supports typeahead with debounce-friendly response times (<10ms typical).

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// =============================================================================
// Types
// =============================================================================

/// Entity types that can be searched
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityType {
    Cbu,
    Person,
    Company,
    Trust,
    Document,
    Product,
    Service,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Cbu => "CBU",
            EntityType::Person => "PERSON",
            EntityType::Company => "COMPANY",
            EntityType::Trust => "TRUST",
            EntityType::Document => "DOCUMENT",
            EntityType::Product => "PRODUCT",
            EntityType::Service => "SERVICE",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "CBU" => Some(EntityType::Cbu),
            "PERSON" => Some(EntityType::Person),
            "COMPANY" => Some(EntityType::Company),
            "TRUST" => Some(EntityType::Trust),
            "DOCUMENT" => Some(EntityType::Document),
            "PRODUCT" => Some(EntityType::Product),
            "SERVICE" => Some(EntityType::Service),
            _ => None,
        }
    }
}

/// Search request parameters
#[derive(Debug, Clone, Deserialize)]
pub struct EntitySearchRequest {
    /// Search query (e.g., "john sm")
    pub query: String,
    
    /// Entity types to search (empty = all types)
    #[serde(default)]
    pub types: Vec<EntityType>,
    
    /// Maximum results to return
    #[serde(default = "default_limit")]
    pub limit: u32,
    
    /// Minimum similarity threshold (0.0 - 1.0)
    #[serde(default = "default_threshold")]
    pub threshold: f32,
    
    /// Optional: limit search to entities related to this CBU
    pub cbu_id: Option<Uuid>,
}

fn default_limit() -> u32 { 10 }
fn default_threshold() -> f32 { 0.2 }

/// A single search result
#[derive(Debug, Clone, Serialize)]
pub struct EntityMatch {
    /// Entity UUID
    pub id: Uuid,
    
    /// Entity type (PERSON, COMPANY, etc.)
    pub entity_type: EntityType,
    
    /// Primary display name
    pub display_name: String,
    
    /// Secondary info (nationality, jurisdiction, etc.)
    pub subtitle: Option<String>,
    
    /// Tertiary info (DOB, reg number, etc.)
    pub detail: Option<String>,
    
    /// Similarity score (0.0 - 1.0)
    pub score: f32,
}

/// Search response
#[derive(Debug, Clone, Serialize)]
pub struct EntitySearchResponse {
    /// Matching entities
    pub results: Vec<EntityMatch>,
    
    /// Total matches (before limit)
    pub total: u32,
    
    /// Whether results were truncated
    pub truncated: bool,
    
    /// Search time in milliseconds
    pub search_time_ms: u64,
}

// =============================================================================
// Service
// =============================================================================

pub struct EntitySearchService {
    pool: PgPool,
}

impl EntitySearchService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Search entities by name with fuzzy matching
    pub async fn search(&self, req: &EntitySearchRequest) -> Result<EntitySearchResponse, sqlx::Error> {
        let start = std::time::Instant::now();
        
        // Clean and validate query
        let query = req.query.trim();
        if query.is_empty() {
            return Ok(EntitySearchResponse {
                results: vec![],
                total: 0,
                truncated: false,
                search_time_ms: 0,
            });
        }
        
        // Build type filter
        let type_filter = if req.types.is_empty() {
            None
        } else {
            Some(req.types.iter().map(|t| t.as_str()).collect::<Vec<_>>())
        };
        
        // Execute search
        let results = self.search_internal(query, type_filter.as_deref(), req.limit, req.threshold).await?;
        
        let search_time_ms = start.elapsed().as_millis() as u64;
        let total = results.len() as u32;
        let truncated = total >= req.limit;
        
        Ok(EntitySearchResponse {
            results,
            total,
            truncated,
            search_time_ms,
        })
    }
    
    async fn search_internal(
        &self,
        query: &str,
        types: Option<&[&str]>,
        limit: u32,
        threshold: f32,
    ) -> Result<Vec<EntityMatch>, sqlx::Error> {
        // Use the unified view with pg_trgm similarity
        // Set the similarity threshold for the % operator
        sqlx::query_scalar::<_, ()>(&format!("SET pg_trgm.similarity_threshold = {}", threshold))
            .execute(&self.pool)
            .await
            .ok(); // Ignore errors, use default
        
        let rows = sqlx::query_as::<_, EntitySearchRow>(
            r#"
            SELECT 
                id,
                entity_type,
                display_name,
                subtitle_1,
                subtitle_2,
                similarity(search_text, $1) as score
            FROM "ob-poc".entity_search_view
            WHERE search_text % $1
                AND ($2::text[] IS NULL OR entity_type = ANY($2))
            ORDER BY score DESC, display_name ASC
            LIMIT $3
            "#
        )
        .bind(query)
        .bind(types)
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }
    
    /// Search for entities attached to a specific CBU
    pub async fn search_cbu_entities(
        &self,
        cbu_id: Uuid,
        query: Option<&str>,
        types: Option<&[EntityType]>,
        limit: u32,
    ) -> Result<Vec<EntityMatch>, sqlx::Error> {
        // Search entities that are already attached to this CBU
        let rows = sqlx::query_as::<_, EntitySearchRow>(
            r#"
            WITH cbu_entities AS (
                SELECT 
                    ce.entity_id as id,
                    CASE 
                        WHEN pp.entity_id IS NOT NULL THEN 'PERSON'
                        WHEN lc.entity_id IS NOT NULL THEN 'COMPANY'
                        ELSE 'UNKNOWN'
                    END as entity_type,
                    COALESCE(
                        pp.given_name || ' ' || pp.family_name,
                        lc.company_name
                    ) as display_name,
                    COALESCE(pp.nationality, lc.jurisdiction) as subtitle_1,
                    ce.role as subtitle_2,
                    COALESCE(
                        pp.given_name || ' ' || pp.family_name,
                        lc.company_name,
                        ''
                    ) as search_text
                FROM "ob-poc".cbu_entities ce
                LEFT JOIN "ob-poc".proper_persons pp ON ce.entity_id = pp.entity_id
                LEFT JOIN "ob-poc".limited_companies lc ON ce.entity_id = lc.entity_id
                WHERE ce.cbu_id = $1
            )
            SELECT 
                id,
                entity_type,
                display_name,
                subtitle_1,
                subtitle_2,
                CASE 
                    WHEN $2::text IS NULL THEN 1.0
                    ELSE similarity(search_text, $2)
                END as score
            FROM cbu_entities
            WHERE $2::text IS NULL OR search_text % $2
            ORDER BY score DESC, display_name ASC
            LIMIT $3
            "#
        )
        .bind(cbu_id)
        .bind(query)
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }
    
    /// Get a single entity by ID and type
    pub async fn get_entity(&self, id: Uuid, entity_type: EntityType) -> Result<Option<EntityMatch>, sqlx::Error> {
        let row = sqlx::query_as::<_, EntitySearchRow>(
            r#"
            SELECT 
                id,
                entity_type,
                display_name,
                subtitle_1,
                subtitle_2,
                1.0 as score
            FROM "ob-poc".entity_search_view
            WHERE id = $1 AND entity_type = $2
            "#
        )
        .bind(id)
        .bind(entity_type.as_str())
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(row.map(|r| r.into()))
    }
}

// =============================================================================
// Internal Types
// =============================================================================

#[derive(Debug, sqlx::FromRow)]
struct EntitySearchRow {
    id: Uuid,
    entity_type: String,
    display_name: String,
    subtitle_1: Option<String>,
    subtitle_2: Option<String>,
    score: f32,
}

impl From<EntitySearchRow> for EntityMatch {
    fn from(row: EntitySearchRow) -> Self {
        EntityMatch {
            id: row.id,
            entity_type: EntityType::from_str(&row.entity_type).unwrap_or(EntityType::Person),
            display_name: row.display_name,
            subtitle: row.subtitle_1,
            detail: row.subtitle_2,
            score: row.score,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_type_serde() {
        assert_eq!(EntityType::Person.as_str(), "PERSON");
        assert_eq!(EntityType::from_str("PERSON"), Some(EntityType::Person));
        assert_eq!(EntityType::from_str("person"), Some(EntityType::Person));
    }
    
    #[test]
    fn test_search_request_defaults() {
        let json = r#"{"query": "john"}"#;
        let req: EntitySearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.limit, 10);
        assert_eq!(req.threshold, 0.2);
        assert!(req.types.is_empty());
    }
}
```

### Update `rust/src/services/mod.rs`

```rust
pub mod entity_search;
pub use entity_search::{EntitySearchService, EntitySearchRequest, EntitySearchResponse, EntityMatch, EntityType};
```

## Part 3: Template System

### Create `rust/src/templates/mod.rs`

```rust
//! Template System for Structured DSL Generation
//!
//! Templates define form structures with typed slots.
//! EntityRef slots use the EntitySearchService for typeahead.

pub mod registry;
pub mod slot_types;
pub mod renderer;

pub use registry::TemplateRegistry;
pub use slot_types::{SlotType, SlotDefinition, FormTemplate};
pub use renderer::TemplateRenderer;
```

### Create `rust/src/templates/slot_types.rs`

```rust
//! Slot type definitions for template forms

use serde::{Deserialize, Serialize};
use crate::services::EntityType;

/// Types of form slots
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SlotType {
    /// Free text input
    Text {
        #[serde(default)]
        max_length: Option<u32>,
        #[serde(default)]
        multiline: bool,
    },
    
    /// Date picker (YYYY-MM-DD)
    Date,
    
    /// Country selector (ISO 2-letter)
    Country,
    
    /// Currency selector (ISO 3-letter)
    Currency,
    
    /// Money amount (links to currency slot)
    Money {
        currency_slot: String,
    },
    
    /// Percentage (0-100)
    Percentage,
    
    /// Integer input with optional range
    Integer {
        #[serde(default)]
        min: Option<i64>,
        #[serde(default)]
        max: Option<i64>,
    },
    
    /// Decimal number
    Decimal {
        #[serde(default)]
        precision: Option<u32>,
    },
    
    /// Boolean toggle
    Boolean,
    
    /// Dropdown/select from fixed options
    Enum {
        options: Vec<EnumOption>,
    },
    
    /// Entity reference with search
    EntityRef {
        /// Which entity types are allowed
        allowed_types: Vec<EntityType>,
        /// Search scope
        #[serde(default)]
        scope: RefScope,
        /// Allow creating new entity inline
        #[serde(default)]
        allow_create: bool,
    },
    
    /// UUID (auto-generated or manual)
    Uuid {
        #[serde(default)]
        auto_generate: bool,
    },
}

/// Options for enum slot type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumOption {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Scope for entity reference search
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefScope {
    /// Search all entities in database
    #[default]
    Global,
    /// Search only entities attached to current CBU
    WithinCbu,
    /// Search only entities created in this session
    WithinSession,
}

/// Definition of a single slot in a template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotDefinition {
    /// Slot identifier (maps to DSL param name)
    pub name: String,
    
    /// Human-readable label
    pub label: String,
    
    /// The slot type
    pub slot_type: SlotType,
    
    /// Is this slot required?
    #[serde(default)]
    pub required: bool,
    
    /// Default value (JSON)
    #[serde(default)]
    pub default_value: Option<serde_json::Value>,
    
    /// Help text shown below input
    #[serde(default)]
    pub help_text: Option<String>,
    
    /// Placeholder text
    #[serde(default)]
    pub placeholder: Option<String>,
    
    /// DSL param name (if different from slot name)
    #[serde(default)]
    pub dsl_param: Option<String>,
}

impl SlotDefinition {
    /// Get the DSL parameter name for this slot
    pub fn dsl_param_name(&self) -> &str {
        self.dsl_param.as_deref().unwrap_or(&self.name)
    }
}

/// A complete form template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormTemplate {
    /// Unique template identifier
    pub id: String,
    
    /// Human-readable name
    pub name: String,
    
    /// Description of what this template does
    pub description: String,
    
    /// DSL verb this template generates
    pub verb: String,
    
    /// DSL domain
    pub domain: String,
    
    /// Slot definitions
    pub slots: Vec<SlotDefinition>,
    
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

impl FormTemplate {
    /// Get a slot by name
    pub fn get_slot(&self, name: &str) -> Option<&SlotDefinition> {
        self.slots.iter().find(|s| s.name == name)
    }
    
    /// Get all required slots
    pub fn required_slots(&self) -> impl Iterator<Item = &SlotDefinition> {
        self.slots.iter().filter(|s| s.required)
    }
    
    /// Get all EntityRef slots
    pub fn entity_ref_slots(&self) -> impl Iterator<Item = &SlotDefinition> {
        self.slots.iter().filter(|s| matches!(s.slot_type, SlotType::EntityRef { .. }))
    }
}
```

### Create `rust/src/templates/registry.rs`

```rust
//! Template Registry - Built-in templates for common operations

use super::slot_types::*;
use crate::services::EntityType;
use std::collections::HashMap;

pub struct TemplateRegistry {
    templates: HashMap<String, FormTemplate>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            templates: HashMap::new(),
        };
        registry.register_builtins();
        registry
    }
    
    pub fn get(&self, id: &str) -> Option<&FormTemplate> {
        self.templates.get(id)
    }
    
    pub fn list(&self) -> Vec<&FormTemplate> {
        self.templates.values().collect()
    }
    
    pub fn list_by_domain(&self, domain: &str) -> Vec<&FormTemplate> {
        self.templates.values()
            .filter(|t| t.domain == domain)
            .collect()
    }
    
    fn register(&mut self, template: FormTemplate) {
        self.templates.insert(template.id.clone(), template);
    }
    
    fn register_builtins(&mut self) {
        // CBU templates
        self.register(self.create_cbu_template());
        self.register(self.attach_entity_template());
        self.register(self.attach_beneficial_owner_template());
        
        // Entity templates
        self.register(self.create_person_template());
        self.register(self.create_company_template());
        
        // Document templates
        self.register(self.request_document_template());
    }
    
    // =========================================================================
    // CBU Templates
    // =========================================================================
    
    fn create_cbu_template(&self) -> FormTemplate {
        FormTemplate {
            id: "cbu.create".into(),
            name: "Create CBU".into(),
            description: "Create a new Client Business Unit".into(),
            verb: "cbu.ensure".into(),
            domain: "cbu".into(),
            tags: vec!["cbu".into(), "create".into()],
            slots: vec![
                SlotDefinition {
                    name: "cbu_name".into(),
                    label: "CBU Name".into(),
                    slot_type: SlotType::Text { max_length: Some(200), multiline: false },
                    required: true,
                    placeholder: Some("Apex Capital Partners".into()),
                    dsl_param: Some("cbu-name".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "client_type".into(),
                    label: "Client Type".into(),
                    slot_type: SlotType::Enum {
                        options: vec![
                            EnumOption { value: "COMPANY".into(), label: "Company".into(), description: Some("Limited company or corporation".into()) },
                            EnumOption { value: "INDIVIDUAL".into(), label: "Individual".into(), description: Some("Natural person".into()) },
                            EnumOption { value: "TRUST".into(), label: "Trust".into(), description: Some("Trust or foundation".into()) },
                            EnumOption { value: "PARTNERSHIP".into(), label: "Partnership".into(), description: Some("Partnership or LP".into()) },
                        ],
                    },
                    required: true,
                    default_value: Some(serde_json::json!("COMPANY")),
                    dsl_param: Some("client-type".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "jurisdiction".into(),
                    label: "Jurisdiction".into(),
                    slot_type: SlotType::Country,
                    required: true,
                    placeholder: Some("GB".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "nature_purpose".into(),
                    label: "Nature & Purpose".into(),
                    slot_type: SlotType::Text { max_length: Some(1000), multiline: true },
                    required: false,
                    placeholder: Some("Hedge fund managing high net worth client assets".into()),
                    dsl_param: Some("nature-purpose".into()),
                    ..Default::default()
                },
            ],
        }
    }
    
    fn attach_entity_template(&self) -> FormTemplate {
        FormTemplate {
            id: "cbu.attach-entity".into(),
            name: "Attach Entity to CBU".into(),
            description: "Link an existing entity to a CBU with a specific role".into(),
            verb: "cbu.attach-entity".into(),
            domain: "cbu".into(),
            tags: vec!["cbu".into(), "entity".into(), "relationship".into()],
            slots: vec![
                SlotDefinition {
                    name: "cbu_id".into(),
                    label: "CBU".into(),
                    slot_type: SlotType::EntityRef {
                        allowed_types: vec![EntityType::Cbu],
                        scope: RefScope::WithinSession,
                        allow_create: false,
                    },
                    required: true,
                    help_text: Some("Select the CBU to attach to".into()),
                    dsl_param: Some("cbu-id".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "entity_id".into(),
                    label: "Entity".into(),
                    slot_type: SlotType::EntityRef {
                        allowed_types: vec![EntityType::Person, EntityType::Company, EntityType::Trust],
                        scope: RefScope::Global,
                        allow_create: true,
                    },
                    required: true,
                    help_text: Some("Search for an entity or create new".into()),
                    dsl_param: Some("entity-id".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "role".into(),
                    label: "Role".into(),
                    slot_type: SlotType::Enum {
                        options: vec![
                            EnumOption { value: "PRINCIPAL".into(), label: "Principal".into(), description: None },
                            EnumOption { value: "DIRECTOR".into(), label: "Director".into(), description: None },
                            EnumOption { value: "SHAREHOLDER".into(), label: "Shareholder".into(), description: None },
                            EnumOption { value: "BENEFICIAL_OWNER".into(), label: "Beneficial Owner".into(), description: Some("Person with >25% ownership".into()) },
                            EnumOption { value: "SIGNATORY".into(), label: "Signatory".into(), description: None },
                            EnumOption { value: "AUTHORIZED_PERSON".into(), label: "Authorized Person".into(), description: None },
                        ],
                    },
                    required: true,
                    ..Default::default()
                },
            ],
        }
    }
    
    fn attach_beneficial_owner_template(&self) -> FormTemplate {
        FormTemplate {
            id: "cbu.attach-beneficial-owner".into(),
            name: "Attach Beneficial Owner".into(),
            description: "Link a beneficial owner (>25% ownership) to a CBU".into(),
            verb: "cbu.attach-entity".into(),
            domain: "cbu".into(),
            tags: vec!["cbu".into(), "beneficial-owner".into(), "compliance".into()],
            slots: vec![
                SlotDefinition {
                    name: "cbu_id".into(),
                    label: "CBU".into(),
                    slot_type: SlotType::EntityRef {
                        allowed_types: vec![EntityType::Cbu],
                        scope: RefScope::WithinSession,
                        allow_create: false,
                    },
                    required: true,
                    dsl_param: Some("cbu-id".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "entity_id".into(),
                    label: "Beneficial Owner".into(),
                    slot_type: SlotType::EntityRef {
                        allowed_types: vec![EntityType::Person, EntityType::Company],
                        scope: RefScope::Global,
                        allow_create: true,
                    },
                    required: true,
                    dsl_param: Some("entity-id".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "ownership_percentage".into(),
                    label: "Ownership %".into(),
                    slot_type: SlotType::Percentage,
                    required: true,
                    help_text: Some("Must be >25% for beneficial owner".into()),
                    dsl_param: Some("ownership-percentage".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "role".into(),
                    label: "Role".into(),
                    slot_type: SlotType::Enum {
                        options: vec![
                            EnumOption { value: "BENEFICIAL_OWNER".into(), label: "Beneficial Owner".into(), description: None },
                        ],
                    },
                    required: true,
                    default_value: Some(serde_json::json!("BENEFICIAL_OWNER")),
                    ..Default::default()
                },
            ],
        }
    }
    
    // =========================================================================
    // Entity Templates
    // =========================================================================
    
    fn create_person_template(&self) -> FormTemplate {
        FormTemplate {
            id: "entity.create-person".into(),
            name: "Create Person".into(),
            description: "Create a new natural person entity".into(),
            verb: "entity.create-proper-person".into(),
            domain: "entity".into(),
            tags: vec!["entity".into(), "person".into(), "create".into()],
            slots: vec![
                SlotDefinition {
                    name: "given_name".into(),
                    label: "Given Name".into(),
                    slot_type: SlotType::Text { max_length: Some(100), multiline: false },
                    required: true,
                    placeholder: Some("John".into()),
                    dsl_param: Some("given-name".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "family_name".into(),
                    label: "Family Name".into(),
                    slot_type: SlotType::Text { max_length: Some(100), multiline: false },
                    required: true,
                    placeholder: Some("Smith".into()),
                    dsl_param: Some("family-name".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "nationality".into(),
                    label: "Nationality".into(),
                    slot_type: SlotType::Country,
                    required: false,
                    ..Default::default()
                },
                SlotDefinition {
                    name: "date_of_birth".into(),
                    label: "Date of Birth".into(),
                    slot_type: SlotType::Date,
                    required: false,
                    dsl_param: Some("date-of-birth".into()),
                    ..Default::default()
                },
            ],
        }
    }
    
    fn create_company_template(&self) -> FormTemplate {
        FormTemplate {
            id: "entity.create-company".into(),
            name: "Create Company".into(),
            description: "Create a new limited company entity".into(),
            verb: "entity.create-limited-company".into(),
            domain: "entity".into(),
            tags: vec!["entity".into(), "company".into(), "create".into()],
            slots: vec![
                SlotDefinition {
                    name: "company_name".into(),
                    label: "Company Name".into(),
                    slot_type: SlotType::Text { max_length: Some(200), multiline: false },
                    required: true,
                    placeholder: Some("Acme Holdings Ltd".into()),
                    dsl_param: Some("company-name".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "registration_number".into(),
                    label: "Registration Number".into(),
                    slot_type: SlotType::Text { max_length: Some(50), multiline: false },
                    required: false,
                    placeholder: Some("12345678".into()),
                    dsl_param: Some("registration-number".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "jurisdiction".into(),
                    label: "Jurisdiction".into(),
                    slot_type: SlotType::Country,
                    required: true,
                    ..Default::default()
                },
                SlotDefinition {
                    name: "incorporation_date".into(),
                    label: "Incorporation Date".into(),
                    slot_type: SlotType::Date,
                    required: false,
                    dsl_param: Some("incorporation-date".into()),
                    ..Default::default()
                },
            ],
        }
    }
    
    // =========================================================================
    // Document Templates
    // =========================================================================
    
    fn request_document_template(&self) -> FormTemplate {
        FormTemplate {
            id: "document.request".into(),
            name: "Request Document".into(),
            description: "Request a document from an entity".into(),
            verb: "document.request".into(),
            domain: "document".into(),
            tags: vec!["document".into(), "request".into()],
            slots: vec![
                SlotDefinition {
                    name: "entity_id".into(),
                    label: "From Entity".into(),
                    slot_type: SlotType::EntityRef {
                        allowed_types: vec![EntityType::Person, EntityType::Company],
                        scope: RefScope::WithinCbu,
                        allow_create: false,
                    },
                    required: true,
                    dsl_param: Some("entity-id".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "document_type".into(),
                    label: "Document Type".into(),
                    slot_type: SlotType::Enum {
                        options: vec![
                            EnumOption { value: "PASSPORT".into(), label: "Passport".into(), description: None },
                            EnumOption { value: "ID_CARD".into(), label: "ID Card".into(), description: None },
                            EnumOption { value: "PROOF_OF_ADDRESS".into(), label: "Proof of Address".into(), description: None },
                            EnumOption { value: "CERT_OF_INCORP".into(), label: "Certificate of Incorporation".into(), description: None },
                            EnumOption { value: "FINANCIAL_STATEMENT".into(), label: "Financial Statement".into(), description: None },
                            EnumOption { value: "SOURCE_OF_WEALTH".into(), label: "Source of Wealth".into(), description: None },
                        ],
                    },
                    required: true,
                    dsl_param: Some("document-type".into()),
                    ..Default::default()
                },
            ],
        }
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SlotDefinition {
    fn default() -> Self {
        SlotDefinition {
            name: String::new(),
            label: String::new(),
            slot_type: SlotType::Text { max_length: None, multiline: false },
            required: false,
            default_value: None,
            help_text: None,
            placeholder: None,
            dsl_param: None,
        }
    }
}
```

### Create `rust/src/templates/renderer.rs`

```rust
//! Template Renderer - Convert filled slots to DSL

use super::slot_types::{FormTemplate, SlotType};
use crate::api::intent::ParamValue;
use serde_json::Value;
use std::collections::HashMap;

pub struct TemplateRenderer;

impl TemplateRenderer {
    /// Render a template with filled slot values to DSL
    pub fn render(
        template: &FormTemplate,
        values: &HashMap<String, Value>,
    ) -> Result<String, RenderError> {
        // Validate required slots
        for slot in template.required_slots() {
            if !values.contains_key(&slot.name) {
                return Err(RenderError::MissingRequired(slot.name.clone()));
            }
        }
        
        // Build DSL s-expression
        let mut parts = vec![format!("({}", template.verb)];
        
        for slot in &template.slots {
            if let Some(value) = values.get(&slot.name) {
                let dsl_param = slot.dsl_param_name();
                let dsl_value = Self::value_to_dsl(value, &slot.slot_type)?;
                parts.push(format!(":{} {}", dsl_param, dsl_value));
            }
        }
        
        parts.push(")".to_string());
        
        Ok(parts.join(" "))
    }
    
    /// Convert a JSON value to DSL string based on slot type
    fn value_to_dsl(value: &Value, slot_type: &SlotType) -> Result<String, RenderError> {
        match (value, slot_type) {
            // String types
            (Value::String(s), SlotType::Text { .. }) |
            (Value::String(s), SlotType::Date) |
            (Value::String(s), SlotType::Country) |
            (Value::String(s), SlotType::Currency) |
            (Value::String(s), SlotType::Enum { .. }) => {
                Ok(format!("\"{}\"", s.replace('\"', "\\\"")))
            }
            
            // UUID (entity references)
            (Value::String(s), SlotType::EntityRef { .. }) |
            (Value::String(s), SlotType::Uuid { .. }) => {
                Ok(format!("\"{}\"", s))
            }
            
            // Numbers
            (Value::Number(n), SlotType::Integer { .. }) => {
                Ok(n.to_string())
            }
            (Value::Number(n), SlotType::Decimal { .. }) |
            (Value::Number(n), SlotType::Percentage) |
            (Value::Number(n), SlotType::Money { .. }) => {
                Ok(n.to_string())
            }
            
            // Boolean
            (Value::Bool(b), SlotType::Boolean) => {
                Ok(if *b { "true" } else { "false" }.to_string())
            }
            
            // Type mismatches - try to coerce
            (Value::String(s), SlotType::Integer { .. }) => {
                s.parse::<i64>()
                    .map(|n| n.to_string())
                    .map_err(|_| RenderError::TypeMismatch {
                        slot: String::new(),
                        expected: "integer".into(),
                        got: "string".into(),
                    })
            }
            
            _ => Err(RenderError::TypeMismatch {
                slot: String::new(),
                expected: format!("{:?}", slot_type),
                got: format!("{:?}", value),
            }),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("Missing required slot: {0}")]
    MissingRequired(String),
    
    #[error("Type mismatch for slot '{slot}': expected {expected}, got {got}")]
    TypeMismatch {
        slot: String,
        expected: String,
        got: String,
    },
    
    #[error("Invalid value: {0}")]
    InvalidValue(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::TemplateRegistry;
    
    #[test]
    fn test_render_create_cbu() {
        let registry = TemplateRegistry::new();
        let template = registry.get("cbu.create").unwrap();
        
        let mut values = HashMap::new();
        values.insert("cbu_name".into(), Value::String("Apex Capital".into()));
        values.insert("client_type".into(), Value::String("COMPANY".into()));
        values.insert("jurisdiction".into(), Value::String("GB".into()));
        
        let dsl = TemplateRenderer::render(template, &values).unwrap();
        
        assert!(dsl.starts_with("(cbu.ensure"));
        assert!(dsl.contains(":cbu-name \"Apex Capital\""));
        assert!(dsl.contains(":client-type \"COMPANY\""));
        assert!(dsl.contains(":jurisdiction \"GB\""));
    }
}
```

## Part 4: API Endpoints

### Create `rust/src/api/entity_routes.rs`

```rust
//! Entity Search API endpoints

use crate::services::{EntitySearchService, EntitySearchRequest, EntitySearchResponse, EntityType};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::Deserialize;
use sqlx::PgPool;
use std::sync::Arc;

// State for entity routes
pub struct EntityState {
    search_service: Arc<EntitySearchService>,
}

impl EntityState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            search_service: Arc::new(EntitySearchService::new(pool)),
        }
    }
}

// Query params for search
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Search query string
    pub q: String,
    
    /// Comma-separated entity types (e.g., "PERSON,COMPANY")
    #[serde(default)]
    pub types: Option<String>,
    
    /// Max results
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 { 10 }

/// GET /api/entities/search
async fn search_entities(
    State(state): State<EntityState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<EntitySearchResponse>, StatusCode> {
    let types: Vec<EntityType> = query.types
        .as_deref()
        .map(|t| {
            t.split(',')
                .filter_map(|s| EntityType::from_str(s.trim()))
                .collect()
        })
        .unwrap_or_default();
    
    let req = EntitySearchRequest {
        query: query.q,
        types,
        limit: query.limit.min(50),
        threshold: 0.2,
        cbu_id: None,
    };
    
    let response = state.search_service
        .search(&req)
        .await
        .map_err(|e| {
            eprintln!("Search error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(response))
}

/// Create router for entity endpoints
pub fn create_entity_router(pool: PgPool) -> Router {
    let state = EntityState::new(pool);
    
    Router::new()
        .route("/api/entities/search", get(search_entities))
        .with_state(state)
}
```

### Create `rust/src/api/template_routes.rs`

```rust
//! Template API endpoints

use crate::templates::{FormTemplate, TemplateRegistry, TemplateRenderer};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub struct TemplateState {
    registry: Arc<TemplateRegistry>,
}

impl TemplateState {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(TemplateRegistry::new()),
        }
    }
}

// Response types
#[derive(Debug, Serialize)]
pub struct TemplateListResponse {
    pub templates: Vec<TemplateSummary>,
}

#[derive(Debug, Serialize)]
pub struct TemplateSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RenderRequest {
    pub values: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct RenderResponse {
    pub dsl: String,
    pub verb: String,
}

/// GET /api/templates
async fn list_templates(
    State(state): State<TemplateState>,
) -> Json<TemplateListResponse> {
    let templates = state.registry
        .list()
        .iter()
        .map(|t| TemplateSummary {
            id: t.id.clone(),
            name: t.name.clone(),
            description: t.description.clone(),
            domain: t.domain.clone(),
            tags: t.tags.clone(),
        })
        .collect();
    
    Json(TemplateListResponse { templates })
}

/// GET /api/templates/:id
async fn get_template(
    State(state): State<TemplateState>,
    Path(id): Path<String>,
) -> Result<Json<FormTemplate>, StatusCode> {
    state.registry
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// POST /api/templates/:id/render
async fn render_template(
    State(state): State<TemplateState>,
    Path(id): Path<String>,
    Json(req): Json<RenderRequest>,
) -> Result<Json<RenderResponse>, StatusCode> {
    let template = state.registry
        .get(&id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    let dsl = TemplateRenderer::render(template, &req.values)
        .map_err(|e| {
            eprintln!("Render error: {}", e);
            StatusCode::BAD_REQUEST
        })?;
    
    Ok(Json(RenderResponse {
        dsl,
        verb: template.verb.clone(),
    }))
}

/// Create router for template endpoints
pub fn create_template_router() -> Router {
    let state = TemplateState::new();
    
    Router::new()
        .route("/api/templates", get(list_templates))
        .route("/api/templates/:id", get(get_template))
        .route("/api/templates/:id/render", post(render_template))
        .with_state(state)
}
```

### Update `rust/src/api/mod.rs`

```rust
// Add to existing mod.rs:

#[cfg(feature = "server")]
pub mod entity_routes;

#[cfg(feature = "server")]
pub mod template_routes;

#[cfg(feature = "server")]
pub use entity_routes::create_entity_router;

#[cfg(feature = "server")]
pub use template_routes::create_template_router;
```

### Update `rust/src/bin/agentic_server.rs`

```rust
// Add to imports:
use ob_poc::api::{create_agent_router, create_attribute_router, create_entity_router, create_template_router};

// Update app creation:
let app = create_agent_router(pool.clone())
    .merge(create_attribute_router(pool.clone()))
    .merge(create_entity_router(pool.clone()))
    .merge(create_template_router())
    .nest_service("/", ServeDir::new("static").append_index_html_on_directories(true))
    // ... rest of layers
```

## Part 5: TypeScript Types & UI Components

### Update `rust/ui/src/types.ts`

Add to existing types:

```typescript
// =============================================================================
// Entity Search Types
// =============================================================================

export type EntityType = 
  | "CBU" 
  | "PERSON" 
  | "COMPANY" 
  | "TRUST" 
  | "DOCUMENT" 
  | "PRODUCT" 
  | "SERVICE";

export interface EntitySearchRequest {
  query: string;
  types?: EntityType[];
  limit?: number;
  threshold?: number;
  cbu_id?: string;
}

export interface EntityMatch {
  id: string;
  entity_type: EntityType;
  display_name: string;
  subtitle?: string;
  detail?: string;
  score: number;
}

export interface EntitySearchResponse {
  results: EntityMatch[];
  total: number;
  truncated: boolean;
  search_time_ms: number;
}

// =============================================================================
// Template Types
// =============================================================================

export type SlotType = 
  | { type: "text"; max_length?: number; multiline?: boolean }
  | { type: "date" }
  | { type: "country" }
  | { type: "currency" }
  | { type: "money"; currency_slot: string }
  | { type: "percentage" }
  | { type: "integer"; min?: number; max?: number }
  | { type: "decimal"; precision?: number }
  | { type: "boolean" }
  | { type: "enum"; options: EnumOption[] }
  | { type: "entity_ref"; allowed_types: EntityType[]; scope: RefScope; allow_create: boolean }
  | { type: "uuid"; auto_generate?: boolean };

export interface EnumOption {
  value: string;
  label: string;
  description?: string;
}

export type RefScope = "global" | "within_cbu" | "within_session";

export interface SlotDefinition {
  name: string;
  label: string;
  slot_type: SlotType;
  required: boolean;
  default_value?: unknown;
  help_text?: string;
  placeholder?: string;
  dsl_param?: string;
}

export interface FormTemplate {
  id: string;
  name: string;
  description: string;
  verb: string;
  domain: string;
  slots: SlotDefinition[];
  tags: string[];
}

export interface TemplateSummary {
  id: string;
  name: string;
  description: string;
  domain: string;
  tags: string[];
}

export interface RenderRequest {
  values: Record<string, unknown>;
}

export interface RenderResponse {
  dsl: string;
  verb: string;
}
```

### Create `rust/ui/src/components/EntityPicker.ts`

```typescript
import { api } from '../api';
import type { EntityMatch, EntityType, EntitySearchResponse } from '../types';

export interface EntityPickerConfig {
  container: HTMLElement;
  allowedTypes: EntityType[];
  allowCreate: boolean;
  placeholder?: string;
  onSelect: (entity: EntityMatch | null) => void;
  onCreate?: (type: EntityType) => void;
}

export class EntityPicker {
  private config: EntityPickerConfig;
  private input: HTMLInputElement;
  private dropdown: HTMLDivElement;
  private selectedEntity: EntityMatch | null = null;
  private debounceTimer: number | null = null;
  private isOpen = false;

  constructor(config: EntityPickerConfig) {
    this.config = config;
    this.input = this.createInput();
    this.dropdown = this.createDropdown();
    this.render();
    this.bindEvents();
  }

  private createInput(): HTMLInputElement {
    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'entity-picker-input';
    input.placeholder = this.config.placeholder || 'Search...';
    return input;
  }

  private createDropdown(): HTMLDivElement {
    const dropdown = document.createElement('div');
    dropdown.className = 'entity-picker-dropdown';
    dropdown.style.display = 'none';
    return dropdown;
  }

  private render(): void {
    const wrapper = document.createElement('div');
    wrapper.className = 'entity-picker';
    wrapper.appendChild(this.input);
    wrapper.appendChild(this.dropdown);
    this.config.container.appendChild(wrapper);
  }

  private bindEvents(): void {
    this.input.addEventListener('input', () => this.handleInput());
    this.input.addEventListener('focus', () => this.handleFocus());
    this.input.addEventListener('blur', () => {
      // Delay to allow click on dropdown
      setTimeout(() => this.close(), 200);
    });
    this.input.addEventListener('keydown', (e) => this.handleKeydown(e));
  }

  private handleInput(): void {
    const query = this.input.value.trim();
    
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
    }

    if (query.length < 2) {
      this.close();
      return;
    }

    this.debounceTimer = window.setTimeout(() => {
      this.search(query);
    }, 200);
  }

  private handleFocus(): void {
    if (this.input.value.trim().length >= 2) {
      this.search(this.input.value.trim());
    }
  }

  private handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      this.close();
    } else if (e.key === 'ArrowDown' && this.isOpen) {
      e.preventDefault();
      this.focusNextItem();
    } else if (e.key === 'ArrowUp' && this.isOpen) {
      e.preventDefault();
      this.focusPrevItem();
    } else if (e.key === 'Enter' && this.isOpen) {
      e.preventDefault();
      this.selectFocusedItem();
    }
  }

  private async search(query: string): Promise<void> {
    try {
      const response = await api.searchEntities({
        q: query,
        types: this.config.allowedTypes,
        limit: 10,
      });
      this.renderResults(response);
    } catch (err) {
      console.error('Search failed:', err);
      this.renderError('Search failed');
    }
  }

  private renderResults(response: EntitySearchResponse): void {
    this.dropdown.innerHTML = '';

    if (response.results.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'entity-picker-empty';
      empty.textContent = 'No results found';
      this.dropdown.appendChild(empty);
    } else {
      response.results.forEach((entity, index) => {
        const item = this.createResultItem(entity, index);
        this.dropdown.appendChild(item);
      });

      if (response.truncated) {
        const more = document.createElement('div');
        more.className = 'entity-picker-more';
        more.textContent = `${response.total - response.results.length} more results...`;
        this.dropdown.appendChild(more);
      }
    }

    // Add create options
    if (this.config.allowCreate) {
      const divider = document.createElement('div');
      divider.className = 'entity-picker-divider';
      this.dropdown.appendChild(divider);

      this.config.allowedTypes.forEach(type => {
        if (type !== 'CBU') { // Can't create CBU inline
          const createItem = document.createElement('div');
          createItem.className = 'entity-picker-item entity-picker-create';
          createItem.innerHTML = `<span class="create-icon">+</span> Create New ${type.toLowerCase()}`;
          createItem.addEventListener('click', () => {
            if (this.config.onCreate) {
              this.config.onCreate(type);
            }
            this.close();
          });
          this.dropdown.appendChild(createItem);
        }
      });
    }

    this.open();
  }

  private createResultItem(entity: EntityMatch, index: number): HTMLDivElement {
    const item = document.createElement('div');
    item.className = 'entity-picker-item';
    item.dataset.index = String(index);
    item.dataset.id = entity.id;

    const typeLabel = document.createElement('span');
    typeLabel.className = `entity-type entity-type-${entity.entity_type.toLowerCase()}`;
    typeLabel.textContent = entity.entity_type;

    const name = document.createElement('span');
    name.className = 'entity-name';
    name.textContent = entity.display_name;

    const subtitle = document.createElement('span');
    subtitle.className = 'entity-subtitle';
    subtitle.textContent = [entity.subtitle, entity.detail].filter(Boolean).join(' • ');

    item.appendChild(typeLabel);
    item.appendChild(name);
    item.appendChild(subtitle);

    item.addEventListener('click', () => this.selectEntity(entity));

    return item;
  }

  private renderError(message: string): void {
    this.dropdown.innerHTML = '';
    const error = document.createElement('div');
    error.className = 'entity-picker-error';
    error.textContent = message;
    this.dropdown.appendChild(error);
    this.open();
  }

  private selectEntity(entity: EntityMatch): void {
    this.selectedEntity = entity;
    this.input.value = entity.display_name;
    this.config.onSelect(entity);
    this.close();
  }

  private open(): void {
    this.dropdown.style.display = 'block';
    this.isOpen = true;
  }

  private close(): void {
    this.dropdown.style.display = 'none';
    this.isOpen = false;
  }

  private focusNextItem(): void {
    const items = this.dropdown.querySelectorAll('.entity-picker-item');
    const focused = this.dropdown.querySelector('.entity-picker-item.focused');
    const currentIndex = focused ? parseInt(focused.getAttribute('data-index') || '-1') : -1;
    const nextIndex = Math.min(currentIndex + 1, items.length - 1);
    
    items.forEach((item, i) => {
      item.classList.toggle('focused', i === nextIndex);
    });
  }

  private focusPrevItem(): void {
    const items = this.dropdown.querySelectorAll('.entity-picker-item');
    const focused = this.dropdown.querySelector('.entity-picker-item.focused');
    const currentIndex = focused ? parseInt(focused.getAttribute('data-index') || '0') : 0;
    const prevIndex = Math.max(currentIndex - 1, 0);
    
    items.forEach((item, i) => {
      item.classList.toggle('focused', i === prevIndex);
    });
  }

  private selectFocusedItem(): void {
    const focused = this.dropdown.querySelector('.entity-picker-item.focused');
    if (focused) {
      (focused as HTMLElement).click();
    }
  }

  // Public methods
  getValue(): EntityMatch | null {
    return this.selectedEntity;
  }

  setValue(entity: EntityMatch | null): void {
    this.selectedEntity = entity;
    this.input.value = entity?.display_name || '';
  }

  clear(): void {
    this.selectedEntity = null;
    this.input.value = '';
    this.config.onSelect(null);
  }
}
```

### Add API methods to `rust/ui/src/api.ts`

```typescript
// Add to api object:

/** Search entities by name */
searchEntities(params: { q: string; types?: EntityType[]; limit?: number }): Promise<EntitySearchResponse> {
  const searchParams = new URLSearchParams();
  searchParams.set('q', params.q);
  if (params.types?.length) {
    searchParams.set('types', params.types.join(','));
  }
  if (params.limit) {
    searchParams.set('limit', String(params.limit));
  }
  return request('GET', `/api/entities/search?${searchParams}`);
},

/** List all templates */
listTemplates(): Promise<{ templates: TemplateSummary[] }> {
  return request('GET', '/api/templates');
},

/** Get a specific template */
getTemplate(id: string): Promise<FormTemplate> {
  return request('GET', `/api/templates/${id}`);
},

/** Render template to DSL */
renderTemplate(id: string, values: Record<string, unknown>): Promise<RenderResponse> {
  return request('POST', `/api/templates/${id}/render`, { values });
},
```

### Add CSS to `rust/ui/src/style.css`

```css
/* =============================================================================
   Entity Picker Styles
   ============================================================================= */

.entity-picker {
  position: relative;
  width: 100%;
}

.entity-picker-input {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid #ddd;
  border-radius: 6px;
  font-size: 14px;
}

.entity-picker-input:focus {
  outline: none;
  border-color: #2196f3;
  box-shadow: 0 0 0 2px rgba(33, 150, 243, 0.1);
}

.entity-picker-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  background: white;
  border: 1px solid #ddd;
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  max-height: 300px;
  overflow-y: auto;
  z-index: 1000;
  margin-top: 4px;
}

.entity-picker-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  cursor: pointer;
  transition: background 0.15s;
}

.entity-picker-item:hover,
.entity-picker-item.focused {
  background: #f5f5f5;
}

.entity-picker-item.entity-picker-create {
  color: #2196f3;
}

.entity-type {
  font-size: 10px;
  font-weight: 600;
  padding: 2px 6px;
  border-radius: 3px;
  text-transform: uppercase;
}

.entity-type-person { background: #e3f2fd; color: #1565c0; }
.entity-type-company { background: #f3e5f5; color: #7b1fa2; }
.entity-type-cbu { background: #e8f5e9; color: #2e7d32; }
.entity-type-trust { background: #fff3e0; color: #ef6c00; }

.entity-name {
  font-weight: 500;
  color: #333;
}

.entity-subtitle {
  font-size: 12px;
  color: #666;
  margin-left: auto;
}

.entity-picker-empty,
.entity-picker-error {
  padding: 20px;
  text-align: center;
  color: #999;
}

.entity-picker-error {
  color: #c62828;
}

.entity-picker-divider {
  height: 1px;
  background: #eee;
  margin: 4px 0;
}

.entity-picker-more {
  padding: 8px 12px;
  font-size: 12px;
  color: #666;
  text-align: center;
}

.create-icon {
  font-weight: bold;
  margin-right: 4px;
}
```

## Part 6: Fix @reference Resolution Bug

### Update `rust/src/api/dsl_assembler.rs`

Add validation that refs can be resolved:

```rust
/// Validate a single intent against the verb registry
pub fn validate_intent(&self, intent: &VerbIntent, context: &SessionContext) -> IntentValidation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // ... existing verb check ...

    // Validate refs can be resolved
    for (key, ref_name) in &intent.refs {
        if !ref_name.starts_with('@') {
            errors.push(IntentError {
                code: "E002".into(),
                message: format!("Invalid reference '{}' - must start with @", ref_name),
                param: Some(key.clone()),
            });
        } else if context.resolve_ref(ref_name).is_none() {
            errors.push(IntentError {
                code: "E003".into(),
                message: format!("Cannot resolve reference '{}' - no entity exists yet", ref_name),
                param: Some(key.clone()),
            });
        }
    }

    IntentValidation {
        valid: errors.is_empty(),
        intent: intent.clone(),
        errors,
        warnings,
    }
}
```

## Files to Create

| File | Purpose |
|------|---------|
| `migrations/20251127_add_entity_search_indexes.sql` | pg_trgm indexes |
| `rust/src/services/entity_search.rs` | Entity search service |
| `rust/src/templates/mod.rs` | Template module |
| `rust/src/templates/slot_types.rs` | Slot type definitions |
| `rust/src/templates/registry.rs` | Built-in templates |
| `rust/src/templates/renderer.rs` | DSL renderer |
| `rust/src/api/entity_routes.rs` | Search API |
| `rust/src/api/template_routes.rs` | Template API |
| `rust/ui/src/components/EntityPicker.ts` | TypeScript picker |

## Files to Modify

| File | Changes |
|------|---------|
| `rust/src/services/mod.rs` | Add entity_search |
| `rust/src/lib.rs` | Add `pub mod templates;` |
| `rust/src/api/mod.rs` | Add entity_routes, template_routes |
| `rust/src/api/dsl_assembler.rs` | Add ref resolution validation |
| `rust/src/bin/agentic_server.rs` | Merge new routers |
| `rust/ui/src/types.ts` | Add entity/template types |
| `rust/ui/src/api.ts` | Add search/template methods |
| `rust/ui/src/style.css` | Add picker styles |

## Testing

```bash
# Run migration
psql $DATABASE_URL -f migrations/20251127_add_entity_search_indexes.sql

# Test search
curl "http://localhost:3000/api/entities/search?q=john&types=PERSON,COMPANY&limit=5"

# Test templates
curl http://localhost:3000/api/templates
curl http://localhost:3000/api/templates/cbu.create

# Test render
curl -X POST http://localhost:3000/api/templates/cbu.create/render \
  -H "Content-Type: application/json" \
  -d '{"values": {"cbu_name": "Test Corp", "client_type": "COMPANY", "jurisdiction": "GB"}}'
```

## Success Criteria

- [ ] pg_trgm extension installed and indexes created
- [ ] Entity search returns results in <10ms
- [ ] Fuzzy matching works ("jon sm" → "John Smith")
- [ ] Template registry has 6+ built-in templates
- [ ] Templates render to valid DSL
- [ ] EntityPicker component works with typeahead
- [ ] @reference validation catches unresolved refs before execution
