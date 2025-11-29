# Design: Contextual Validation Rules for DSL Arguments

## Status: Draft - For Peer Review

## Problem Statement

The current DSL validation checks that argument values exist in reference tables (e.g., document types, jurisdictions), but does not validate **contextual applicability**. For example:

```clojure
;; This is syntactically valid and passes existence checks
(document.catalog :document-type "PASSPORT_GBR" :entity-id @company)
```

But semantically wrong - a passport is for individuals, not companies.

## Current State

### What We Have
1. **Syntax validation** (NOM parser) - validates S-expression grammar
2. **Existence validation** (RefTypeResolver) - checks values exist in DB tables
3. **Type validation** (VerbSchema) - checks argument types match expected types

### What's Missing
- **Contextual rules** - "this document type only applies to individuals"
- **Jurisdiction constraints** - "W-9 is US-specific"
- **Entity type constraints** - "company registration number only for companies"
- **Cross-argument validation** - "if entity type is X, then document type must be Y"

## Proposed Solution

### 1. Metadata Schema in Reference Tables

Extend `document_types` and `attribute_registry` with structured metadata:

```sql
-- document_types.metadata (jsonb)
{
  "applicability": {
    "entity_types": ["PROPER_PERSON_NATURAL", "PROPER_PERSON_BENEFICIAL_OWNER"],
    "jurisdictions": ["GB"],
    "client_types": ["individual"]
  },
  "category": "IDENTITY",
  "tags": ["government_issued", "photo_id"]
}

-- Example: Certificate of Incorporation
{
  "applicability": {
    "entity_types": ["LIMITED_COMPANY_*"],  -- wildcard support
    "client_types": ["corporate"]
  },
  "category": "FORMATION",
  "required_for_onboarding": true
}

-- attribute_registry.metadata (jsonb)
{
  "applicability": {
    "entity_types": ["LIMITED_COMPANY_*"],
    "required_for": ["LIMITED_COMPANY_PRIVATE"]
  },
  "source_documents": ["CERT_OF_INCORPORATION", "ANNUAL_RETURN"]
}
```

### 2. Applicability Rule Types

| Rule Type | Description | Example |
|-----------|-------------|---------|
| `entity_types` | Valid entity types (supports wildcards) | `["PROPER_PERSON_*"]` |
| `jurisdictions` | Valid jurisdictions | `["GB", "US"]` |
| `client_types` | Valid client types | `["individual", "corporate"]` |
| `required_for` | Required when entity has this type | `["LIMITED_COMPANY_PRIVATE"]` |
| `excludes` | Mutually exclusive with | `["PASSPORT_*"]` |
| `requires` | Must have this document/attribute first | `["PROOF_OF_ADDRESS"]` |

### 3. Validation Flow

```
DSL Source
    │
    ▼
┌─────────────────────────────────────┐
│ 1. Syntax Validation (NOM)          │
│    - Valid S-expression?            │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 2. Schema Validation (VerbSchema)   │
│    - Known verb?                    │
│    - Required args present?         │
│    - Arg types match?               │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 3. Existence Validation (RefResolver)│
│    - Document type exists?          │
│    - Entity UUID exists?            │
│    - Jurisdiction code exists?      │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 4. Contextual Validation (NEW)      │  ← NEW LAYER
│    - Document valid for this entity?│
│    - Jurisdiction constraints met?  │
│    - Required documents present?    │
└─────────────────────────────────────┘
    │
    ▼
   VALID → Execute
```

### 4. Contextual Validator Interface

```rust
/// Rule loaded from document_types/attribute_registry metadata
pub struct ApplicabilityRule {
    /// Entity types this applies to (supports glob: "LIMITED_COMPANY_*")
    pub entity_types: Option<Vec<String>>,
    
    /// Jurisdictions this applies to
    pub jurisdictions: Option<Vec<String>>,
    
    /// Client types this applies to
    pub client_types: Option<Vec<String>>,
    
    /// Required when entity has these types
    pub required_for: Option<Vec<String>>,
}

/// Contextual validator that checks business rules
pub struct ContextualValidator {
    pool: PgPool,
    /// Cached applicability rules from DB
    document_rules: HashMap<String, ApplicabilityRule>,
    attribute_rules: HashMap<String, ApplicabilityRule>,
}

impl ContextualValidator {
    /// Validate a document type is applicable in context
    pub async fn validate_document_type(
        &self,
        document_type: &str,
        entity_id: Option<Uuid>,
        cbu_id: Option<Uuid>,
    ) -> Result<(), ContextualError>;
    
    /// Validate an attribute is applicable to an entity
    pub async fn validate_attribute(
        &self,
        attribute_id: &str,
        entity_id: Uuid,
    ) -> Result<(), ContextualError>;
}

/// Contextual validation error with suggestions
pub struct ContextualError {
    pub code: ContextualErrorCode,
    pub message: String,
    pub invalid_value: String,
    pub context: ValidationContext,
    pub suggestions: Vec<String>,  // Valid alternatives
}

pub enum ContextualErrorCode {
    DocumentNotApplicableToEntityType,
    DocumentNotApplicableToJurisdiction,
    DocumentNotApplicableToClientType,
    AttributeNotApplicableToEntityType,
    MissingRequiredDocument,
    MissingRequiredAttribute,
}
```

### 5. Error Messages

```
error[C001]: document type not applicable to entity type
 --> input:3:25
  |
3 | (document.catalog :document-type "PASSPORT_GBR" :entity-id @company)
  |                                  ^^^^^^^^^^^^^^
  |
  = note: "PASSPORT_GBR" is only valid for: PROPER_PERSON_NATURAL
  = note: entity @company has type: LIMITED_COMPANY_PRIVATE
  = help: valid document types for LIMITED_COMPANY_PRIVATE:
          - CERT_OF_INCORPORATION
          - MEMORANDUM_OF_ASSOCIATION
          - REGISTER_OF_DIRECTORS

error[C002]: document type not applicable to jurisdiction
 --> input:5:25
  |
5 | (document.catalog :document-type "W9_FORM" :cbu-id @uk_client)
  |                                  ^^^^^^^^^
  |
  = note: "W9_FORM" is only valid in jurisdictions: US
  = note: CBU @uk_client has jurisdiction: GB
  = help: for GB, consider: HMRC_SELF_ASSESSMENT
```

### 6. Database Schema Changes

```sql
-- Ensure metadata column exists (may already exist)
ALTER TABLE "ob-poc".document_types 
ADD COLUMN IF NOT EXISTS applicability jsonb DEFAULT '{}';

ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN IF NOT EXISTS applicability jsonb DEFAULT '{}';

-- Index for querying by entity type
CREATE INDEX IF NOT EXISTS idx_document_types_applicability 
ON "ob-poc".document_types USING gin (applicability);

-- Example data
UPDATE "ob-poc".document_types 
SET applicability = '{
  "entity_types": ["PROPER_PERSON_NATURAL", "PROPER_PERSON_BENEFICIAL_OWNER"],
  "jurisdictions": ["GB"],
  "client_types": ["individual"]
}'::jsonb
WHERE type_code = 'PASSPORT_GBR';
```

### 7. Integration with Existing Validation

The contextual validator integrates into `SemanticValidator`:

```rust
impl SemanticValidator {
    pub async fn validate(&mut self, request: &ValidationRequest) -> ValidationResult {
        // 1. Parse (existing)
        let ast = parse_program(&request.source)?;
        
        // 2. Schema + Existence validation (existing)
        self.validate_ast(&ast, &request.context)?;
        
        // 3. Contextual validation (NEW)
        self.contextual.validate_contextual_rules(&ast, &request.context)?;
        
        Ok(validated_program)
    }
}
```

## Open Questions

1. **Wildcard syntax**: Should we use glob patterns (`LIMITED_COMPANY_*`) or regex?
2. **Rule inheritance**: Should child entity types inherit parent rules?
3. **Override mechanism**: Can rules be overridden per-CBU or per-case?
4. **Performance**: Cache rules in memory or query on demand?
5. **Rule versioning**: How to handle rule changes for in-flight onboardings?

## Implementation Plan

### Phase 1: Schema & Data Model
- [ ] Add `applicability` column to `document_types` if not exists
- [ ] Add `applicability` column to `attribute_registry` if not exists
- [ ] Define JSON schema for applicability rules
- [ ] Seed initial rules for common document types

### Phase 2: Validator Implementation
- [ ] Create `ContextualValidator` struct
- [ ] Implement rule loading and caching
- [ ] Implement entity type matching (with wildcards)
- [ ] Implement jurisdiction matching
- [ ] Implement client type matching

### Phase 3: Integration
- [ ] Integrate into `SemanticValidator`
- [ ] Add contextual error codes to `DiagnosticCode`
- [ ] Implement suggestion generation
- [ ] Update error formatter for contextual errors

### Phase 4: Testing & Documentation
- [ ] Unit tests for rule matching
- [ ] Integration tests with real DB
- [ ] Update DSL documentation
- [ ] Seed comprehensive rules for all document types

## Alternatives Considered

### A. Rules in Code
- Pros: Fast, type-safe, compile-time checks
- Cons: Requires code changes to update rules, not queryable

### B. Separate Rules Table
- Pros: Normalized, queryable
- Cons: More complex joins, harder to manage

### C. Rules in VerbSchema (current approach extended)
- Pros: Single source of truth for verbs
- Cons: VerbSchema is static, can't handle dynamic rules

**Decision**: Metadata in reference tables (Option B-lite) because:
- Rules are tied to the reference data they constrain
- Queryable for reporting/debugging
- Can be updated without code deployment
- Natural place for domain experts to maintain rules

## References

- `rust/src/dsl_v2/validation.rs` - Current validation types
- `rust/src/dsl_v2/ref_resolver.rs` - Current existence validation
- `rust/src/dsl_v2/verb_schema.rs` - Typed argument definitions
- `rust/src/dsl_v2/semantic_validator.rs` - AST validation walker
