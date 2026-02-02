# Hybrid Relational + JSONB Data Architecture Alignment

## Context
ob-poc is a KYC/onboarding platform supporting multiple jurisdictions (Luxembourg UCITS, Irish ICAVs, UK funds, US 40 Act). Each jurisdiction has different regulatory requirements and entity attributes. The codebase uses nom-parsed YAML taxonomies to define entity types and their fields.

## The Architecture Principle

**Core Rule:** Use fixed relational columns for universal, queryable, relationship-critical data. Use JSONB for jurisdiction-specific, taxonomy-driven, variable-structure data.

### What goes WHERE:

| Data Type | Storage | Why |
|-----------|---------|-----|
| Primary keys, foreign keys | Relational UUID | Referential integrity |
| Entity type, jurisdiction | Relational TEXT/ENUM | Every entity has these, frequently filtered |
| Status, workflow state | Relational TEXT | WHERE clause performance |
| Parent/child relationships | Relational FK | Graph traversal, joins |
| Timestamps (created, updated) | Relational TIMESTAMPTZ | Universal, sortable |
| Jurisdiction-specific fields | JSONB `attributes` | Varies by jurisdiction |
| Compliance/regulatory data | JSONB `compliance_data` | Varies by entity type |
| Risk flags, tags | JSONB array | Variable length, searchable via GIN |
| Taxonomy-defined fields | JSONB | Taxonomy is source of truth |

### Target Table Pattern:

```sql
CREATE TABLE kyc_entities (
    -- RELATIONAL: Universal, always present, relationships, status
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    entity_type TEXT NOT NULL,
    jurisdiction TEXT NOT NULL,
    status TEXT NOT NULL,
    parent_id UUID REFERENCES kyc_entities(id),
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    
    -- JSONB: Taxonomy-driven, jurisdiction-specific, variable
    attributes JSONB NOT NULL DEFAULT '{}',
    compliance_data JSONB NOT NULL DEFAULT '{}',
    risk_flags JSONB NOT NULL DEFAULT '[]'
);

-- GIN indexes for JSONB queries
CREATE INDEX idx_entity_attrs ON kyc_entities USING GIN (attributes jsonb_path_ops);
CREATE INDEX idx_entity_compliance ON kyc_entities USING GIN (compliance_data jsonb_path_ops);
CREATE INDEX idx_entity_risk ON kyc_entities USING GIN (risk_flags jsonb_path_ops);
```

### Rust Struct Pattern:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct KycEntity {
    // Relational - type-safe enums
    pub id: Uuid,
    pub entity_type: EntityType,      // Rust enum
    pub jurisdiction: Jurisdiction,   // Rust enum
    pub status: Status,               // Rust enum
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    // JSONB - flexible, validated by taxonomy
    pub attributes: sqlx::types::Json<serde_json::Value>,
    pub compliance_data: sqlx::types::Json<serde_json::Value>,
    pub risk_flags: sqlx::types::Json<Vec<String>>,
}
```

---

## Phase 1: Research Current State

### 1.1 Audit SQL Schema
Location: `sql/` directory, likely `ob-poc-schema.sql` or `migrations/`

**Find and document:**
- [ ] Tables with excessive columns that vary by jurisdiction/entity-type (candidates for JSONB consolidation)
- [ ] Tables storing JSON as TEXT instead of JSONB
- [ ] Missing GIN indexes on existing JSONB columns
- [ ] Tables that should have JSONB flexibility but are purely relational
- [ ] Any `gen_random_uuid()` or `uuid_generate_v4()` defaults (should become `uuidv7()` after PG18 upgrade)

### 1.2 Audit Rust Structs
Location: `rust/` directory, likely in `src/` or `crates/`

**Find and document:**
- [ ] Entity structs with many Option<T> fields (sign of over-relational design)
- [ ] Structs using `HashMap<String, Value>` or similar (good - but check if mapped to JSONB)
- [ ] Structs with jurisdiction-specific fields hardcoded (should be in JSONB)
- [ ] SQLx query mappings - are they using `Json<T>` wrapper for flexible fields?

### 1.3 Audit Taxonomy Files
Location: `rust/config/` directory, YAML files

**Find and document:**
- [ ] Which fields are defined in taxonomies vs hardcoded in schema
- [ ] Jurisdiction-specific field definitions
- [ ] Entity type hierarchies

### 1.4 Produce Findings Report
Create a markdown report summarizing:
- Tables/structs well-aligned with hybrid pattern
- Tables/structs needing refactoring
- Specific migration recommendations
- Estimated effort per change

---

## Phase 2: Implementation

### 2.1 Update claude.md
Add the architecture principle to the project's `claude.md` file (create if doesn't exist):

```markdown
## Data Strategy: Hybrid Relational + JSONB

### Principle
Use fixed relational columns for universal, queryable, relationship-critical data. 
Use JSONB for jurisdiction-specific, taxonomy-driven, variable-structure data.

### Rationale
KYC/onboarding spans multiple jurisdictions each with different regulatory fields. 
Pure relational = schema changes for every jurisdiction. 
Pure document = loses referential integrity and query performance.
Hybrid = best of both.

### Validation Flow
```
YAML Taxonomy → nom parser validates → serde serializes → JSONB stores → GIN indexes query
```

### Rules
1. Universal to all entities → relational column
2. Defines a relationship → relational foreign key
3. Workflow/status → relational (efficient WHERE)
4. Varies by jurisdiction/entity-type → JSONB
5. Defined in taxonomy file → JSONB
6. JSONB paths in WHERE clauses → GIN index
```

### 2.2 Schema Migrations
For each table identified in Phase 1:

1. Create migration to add JSONB columns if missing
2. Create migration to consolidate jurisdiction-specific columns into JSONB
3. Add GIN indexes for queryable JSONB paths
4. Update defaults from `gen_random_uuid()` to `uuidv7()` (post PG18 upgrade)

### 2.3 Rust Struct Updates
For each struct identified in Phase 1:

1. Replace scattered Option<T> jurisdiction fields with `Json<Value>` attributes
2. Ensure SQLx mappings use `sqlx::types::Json<T>` wrapper
3. Add serde attributes for clean serialization
4. Update any direct field access to use JSONB accessor patterns

### 2.4 Query Updates
Find queries that:
- SELECT many nullable columns (refactor to JSONB extraction)
- Filter on jurisdiction-specific fields (ensure GIN index exists)
- Use TEXT columns for JSON (migrate to JSONB)

---

## Validation Checklist

After implementation:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] `sqlx migrate run` succeeds
- [ ] Sample queries use GIN indexes (check with EXPLAIN ANALYZE)
- [ ] Taxonomy-defined fields round-trip correctly through JSONB

---

## Notes

- This aligns with PostgreSQL 18 upgrade (see `pg18_upgrade_todo.md`)
- UUIDv7 migration is separate but related
- Don't break existing functionality - migrations should be additive first
- The nom parser and taxonomy files are the source of truth for what fields exist
