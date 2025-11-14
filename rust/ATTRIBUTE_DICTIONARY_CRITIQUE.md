# Attribute Dictionary Design & Implementation Critique

**Project**: ob-poc  
**Component**: Attribute Dictionary (AttributeID-as-Type Pattern)  
**Review Date**: November 13, 2025  
**Severity**: HIGH - Core architectural component has significant design flaws

---

## Executive Summary

The attribute dictionary implementation has **critical design and implementation issues** that prevent it from functioning as the intended "AttributeID-as-Type" foundation. While the conceptual design is solid, the execution is incomplete and disconnected across multiple layers.

**Status**: ❌ **NOT PRODUCTION-READY**  
**Recommendation**: **REDESIGN AND REIMPLEMENT** core components

---

## The Intended Design (From Your Description)

### Core Concept
A universal dictionary of attributes where:
1. **UUID as Type**: Each attribute has a UUID that serves as its type identifier
2. **Multi-source Values**: Attributes specify where values come from (multiple sources)
3. **Sink Definition**: Attributes specify where values persist
4. **RAG Support**: Extensive descriptions for AI/agent systems with vector DB indexing
5. **DSL Compilation**: Attributes referenced by UUID in DSL (`@attr{uuid}`), values populated at compile time
6. **Context Scoping**: Values scoped to Onboarding Request ID (CBU ID)

### The Vision
```clojure
;; DSL with embedded attribute UUIDs
(kyc.collect @attr{a8f3c1d2-4b5e-6789-abcd-ef0123456789} 
             :value "John Smith"
             :source "passport-extraction")

;; At compile time:
;; 1. Resolve UUID -> attribute definition from dictionary
;; 2. Validate value against attribute type/constraints
;; 3. Execute source retrieval logic if no value provided
;; 4. Persist to configured sinks
;; 5. Add to execution context for subsequent operations
```

---

## Critical Issues Identified

### Issue #1: Type Safety Violation (CRITICAL)

**Problem**: `AttributeId` newtype wrapper defined but not used consistently

**Evidence**:
```rust
// data_dictionary/attribute.rs
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttributeId(Uuid);  // ✅ Good: Strong typing

// BUT THEN:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeDefinition {
    pub attr_id: String,  // ❌ WRONG: Should be AttributeId!
    // ... rest of fields
}

// models/dictionary_models.rs
pub struct DictionaryAttribute {
    pub attribute_id: Uuid,  // ❌ Also inconsistent
    pub name: String,
    // ...
}
```

**Impact**:
- No type safety for attribute IDs
- Can't distinguish attribute UUIDs from other UUIDs
- Validation bypassed at compile time
- API accepts any string/UUID as "attribute ID"

**Fix Required**:
```rust
// Correct implementation
pub struct AttributeDefinition {
    pub attr_id: AttributeId,  // ✅ Use the newtype!
    pub display_name: String,
    // ... rest
}

pub struct DictionaryAttribute {
    pub attribute_id: AttributeId,  // ✅ Use consistently
    // ... rest
}
```

---

### Issue #2: No DSL Attribute Syntax Support (CRITICAL)

**Problem**: No parser support for `@attr{uuid}` syntax

**Evidence**:
```bash
# Searched entire codebase:
$ grep -r "@attr" rust/src/
# Result: NO MATCHES in parser code

# Checked DSL examples:
$ grep -r "@attr" examples/*.dsl
# Result: NO USAGE in any examples
```

**Current Parser**: Only handles keyword-based attributes like `:customer-name`, not UUID references

**What's Missing**:
1. Lexer token for `@attr{...}` syntax
2. Parser combinator to extract UUID from attribute reference
3. Validation that UUID exists in dictionary
4. Resolution mechanism to fetch attribute definition

**Expected Parser Flow**:
```
DSL Input: @attr{a8f3c1d2-...}
    ↓
Lexer: TokenType::AttributeRef(Uuid)
    ↓
Parser: parse_attribute_ref() 
    ↓
Validator: dictionary.has_attribute(uuid)?
    ↓
AST Node: AttributeReference { id: AttributeId(...), definition: AttributeDefinition {...} }
```

**Current Reality**: None of this exists

---

### Issue #3: Disconnected Source/Sink Definitions (CRITICAL)

**Problem**: Rich source/sink metadata defined but never used

**Evidence**:

**In Code** (`data_dictionary/attribute.rs`):
```rust
pub struct AttributeDefinition {
    // ... other fields
    
    // ✅ Well-designed source structure
    pub sources: DataSources,  
    
    // ✅ Well-designed sink structure
    pub sinks: DataSinks,
}

pub struct DataSources {
    pub primary: Option<SourceDefinition>,
    pub secondary: Option<SourceDefinition>,
    pub tertiary: Option<SourceDefinition>,
}

pub enum SourceType {
    DocumentExtraction,
    Solicitation,
    ThirdPartyService,
    InternalSystem,
    ManualEntry,
}

pub struct DataSinks {
    pub operational: Option<SinkDefinition>,
    pub master: Option<SinkDefinition>,
    pub archive: Option<SinkDefinition>,
    pub audit: Option<SinkDefinition>,
    pub analytics: Option<SinkDefinition>,
}
```

**In Database** (`ob-poc-schema.sql`):
```sql
CREATE TABLE "ob-poc".dictionary (
    attribute_id UUID NOT NULL,
    name VARCHAR(255) NOT NULL,
    long_description TEXT,
    -- ...
    source JSONB,  -- ❌ Unstructured JSONB blob
    sink JSONB,    -- ❌ Unstructured JSONB blob
    -- ...
);
```

**Problems**:
1. **No Schema**: Source/sink stored as opaque JSONB
2. **No Validation**: Can store any JSON, no type checking
3. **No Execution**: Nothing reads these definitions and acts on them
4. **Unused Enums**: SourceType and SinkType enums never referenced

**What's Missing**:
```rust
// Should exist but doesn't:
pub trait SourceExecutor {
    async fn fetch_value(
        &self, 
        attr_id: AttributeId,
        context: &ExecutionContext
    ) -> Result<Value>;
}

pub struct DocumentExtractionSource {
    document_id: Uuid,
    extraction_rules: Vec<ExtractionRule>,
}

impl SourceExecutor for DocumentExtractionSource {
    async fn fetch_value(&self, attr_id: AttributeId, ctx: &ExecutionContext) -> Result<Value> {
        // 1. Load document
        // 2. Apply extraction rules
        // 3. Return extracted value
    }
}

pub trait SinkWriter {
    async fn persist_value(
        &self,
        attr_id: AttributeId, 
        value: &Value,
        context: &ExecutionContext
    ) -> Result<()>;
}
```

---

### Issue #4: No Compilation-Time Value Population (CRITICAL)

**Problem**: No mechanism to populate attribute values during DSL compilation

**What Should Happen**:
```
DSL Compile Time:
1. Parse DSL and find @attr{uuid} references
2. For each attribute without explicit value:
   a. Load AttributeDefinition from dictionary
   b. Check sources in priority order (primary → secondary → tertiary)
   c. Execute source retrieval logic
   d. Validate value against constraints
   e. Add to compilation context
3. During execution:
   a. Use populated values from context
   b. Persist to configured sinks
```

**Current Reality**: 
```rust
// execution/context.rs
pub struct ExecutionContext {
    pub session_id: Uuid,
    pub business_unit_id: String,
    pub domain: String,
    pub executor: String,
    pub started_at: DateTime<Utc>,
    pub environment: HashMap<String, Value>,  // ❌ Generic key-value store
    pub integrations: Vec<String>,
}
```

**Problems**:
- No attribute-specific context
- No tracking of attribute values by AttributeId
- No source information preserved
- No validation state tracked

**Should Be**:
```rust
pub struct ExecutionContext {
    pub session_id: Uuid,
    pub cbu_id: Uuid,  // Onboarding Request ID
    pub domain: String,
    
    // ✅ Attribute-specific context
    pub attribute_values: HashMap<AttributeId, AttributeValue>,
    
    // ✅ Source tracking
    pub value_provenance: HashMap<AttributeId, ValueProvenance>,
    
    pub started_at: DateTime<Utc>,
    pub environment: HashMap<String, Value>,
}

pub struct AttributeValue {
    pub attr_id: AttributeId,
    pub value: Value,
    pub state: ValueState,  // Resolved, Pending, Failed
    pub validated: bool,
    pub confidence: f64,
}

pub struct ValueProvenance {
    pub source_type: SourceType,
    pub source_details: HashMap<String, Value>,
    pub retrieved_at: DateTime<Utc>,
    pub retrieval_method: String,
}
```

---

### Issue #5: Missing RAG/Vector DB Integration (HIGH)

**Problem**: Semantic metadata defined but never used for RAG

**Evidence**:

**Well-Designed Structures**:
```rust
// data_dictionary/attribute.rs
pub struct SemanticMetadata {
    pub description: String,          // ✅ For RAG
    pub context: String,               // ✅ For RAG
    pub related_concepts: Vec<String>, // ✅ For semantic search
    pub usage_examples: Vec<String>,   // ✅ For few-shot learning
    pub regulatory_citations: Vec<String>, // ✅ For compliance
}

pub struct EmbeddingInfo {
    pub vector: Option<Vec<f32>>,  // ✅ 3072-dim vector
    pub model: String,              // ✅ Which embedding model
    pub dimension: usize,
    pub updated_at: String,
}
```

**But No Integration**:
- No vector DB connection (Qdrant, mentioned in docs, but not implemented here)
- No embedding generation service
- No semantic search functionality
- No RAG retrieval logic

**Database Has This**:
```sql
CREATE TABLE "ob-poc".dictionary (
    -- ...
    vector TEXT,  -- ❌ Stored as TEXT, not actual vector type!
    -- ...
);
```

**Should Be**:
```sql
-- Use PostgreSQL pgvector extension
CREATE EXTENSION vector;

CREATE TABLE "ob-poc".dictionary (
    -- ...
    long_description TEXT,
    embedding vector(3072),  -- ✅ Proper vector type
    embedding_model VARCHAR(100),
    embedded_at TIMESTAMPTZ,
    -- ...
);

-- ✅ Vector similarity index
CREATE INDEX idx_dictionary_embedding ON "ob-poc".dictionary 
USING ivfflat (embedding vector_cosine_ops);
```

**Missing Services**:
```rust
// Should exist but doesn't:
pub trait AttributeRagService {
    async fn search_by_semantic(
        &self,
        query: &str,
        limit: usize
    ) -> Result<Vec<(AttributeId, f64)>>;  // (attribute, similarity_score)
    
    async fn embed_attribute_description(
        &self,
        attr_id: AttributeId
    ) -> Result<Vec<f32>>;
    
    async fn find_similar_attributes(
        &self,
        attr_id: AttributeId,
        limit: usize
    ) -> Result<Vec<AttributeId>>;
}
```

---

### Issue #6: Database Schema vs. Code Mismatch (HIGH)

**Problem**: Two different models that don't align

**Code Model** (`data_dictionary/attribute.rs`):
```rust
pub struct AttributeDefinition {
    pub attr_id: String,
    pub display_name: String,
    pub data_type: DataType,
    pub constraints: Option<Constraints>,
    pub semantic: SemanticMetadata,  // Rich structure
    pub embedding: Option<EmbeddingInfo>,  // Rich structure
    pub ui_metadata: UiMetadata,  // Rich structure
    pub sources: DataSources,  // Rich structure
    pub sinks: DataSinks,  // Rich structure
    pub verification: VerificationRules,  // Rich structure
}
```

**Database Model** (`models/dictionary_models.rs`):
```rust
pub struct DictionaryAttribute {
    pub attribute_id: Uuid,
    pub name: String,
    pub long_description: Option<String>,
    pub group_id: String,
    pub mask: String,  // Simple string, not DataType enum
    pub domain: Option<String>,
    pub vector: Option<String>,  // TEXT field, not vector
    pub source: Option<serde_json::Value>,  // Opaque JSON
    pub sink: Option<serde_json::Value>,  // Opaque JSON
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
```

**Mapping Issues**:
1. `display_name` vs `name` - different names
2. `DataType` vs `mask: String` - type safety lost
3. Rich nested structures → flat JSONB blobs
4. No `ui_metadata`, `semantic`, `verification` in DB model
5. `embedding: EmbeddingInfo` → `vector: String` (TEXT)

**Impact**:
- Can't roundtrip: Code → DB → Code loses information
- No validation when writing to database
- No structured queries on source/sink properties
- Vector search impossible with TEXT field

---

### Issue #7: No Dictionary Service Implementation (HIGH)

**Problem**: `DictionaryService` trait defined but not implemented

**Evidence**:
```rust
// data_dictionary/mod.rs
#[async_trait]
pub trait DictionaryService: Send + Sync {
    async fn validate_dsl_attributes(&self, dsl: &str) -> Result<(), String>;
    async fn get_attribute(&self, attribute_id: &str) -> Result<Option<AttributeDefinition>, String>;
    async fn validate_attribute_value(&self, attribute_id: &str, value: &serde_json::Value) -> Result<(), String>;
}

// ❌ NO IMPLEMENTATION FOUND
// Searched for: "impl DictionaryService"
// Result: NOT FOUND
```

**What Exists**: `DictionaryDatabaseService` with CRUD operations, but doesn't implement the `DictionaryService` trait

**Missing**:
```rust
impl DictionaryService for DictionaryDatabaseService {
    async fn validate_dsl_attributes(&self, dsl: &str) -> Result<(), String> {
        // 1. Parse DSL
        // 2. Extract all @attr{uuid} references
        // 3. Verify each UUID exists in dictionary
        // 4. Return errors for unknown attributes
    }
    
    async fn get_attribute(&self, attribute_id: &str) -> Result<Option<AttributeDefinition>, String> {
        // 1. Parse UUID
        // 2. Query database
        // 3. Deserialize source/sink JSON to rich structures
        // 4. Return full AttributeDefinition
    }
    
    async fn validate_attribute_value(&self, attribute_id: &str, value: &serde_json::Value) -> Result<(), String> {
        // 1. Get attribute definition
        // 2. Check data type
        // 3. Validate constraints (min, max, pattern, allowed_values)
        // 4. Run cross-validation rules
    }
}
```

---

### Issue #8: No Attribute Value Lifecycle (HIGH)

**Problem**: No clear lifecycle management for attribute values

**What Should Exist**:
```
Attribute Value Lifecycle:
1. DISCOVERY: Find attribute in DSL (@attr{uuid})
2. RESOLUTION: Fetch value from sources
3. VALIDATION: Check against constraints
4. POPULATION: Add to execution context
5. PERSISTENCE: Write to configured sinks
6. AUDIT: Track provenance and changes
```

**Current Reality**:
- Values go directly to `attribute_values` table
- No state machine (Pending → Resolving → Resolved → Failed)
- No retry logic for failed source fetches
- No conflict resolution for multi-source attributes
- No change tracking beyond timestamps

**Should Be**:
```rust
pub enum AttributeValueState {
    Pending,       // Attribute referenced, not yet resolved
    Resolving,     // Actively fetching from source
    Resolved,      // Value successfully obtained
    Validated,     // Value passed all constraints
    Persisted,     // Written to all configured sinks
    Failed,        // Resolution or validation failed
    Conflicted,    // Multiple sources returned different values
}

pub struct AttributeValueLifecycle {
    attr_id: AttributeId,
    cbu_id: Uuid,
    current_state: AttributeValueState,
    value: Option<Value>,
    source_attempts: Vec<SourceAttempt>,
    validation_results: Vec<ValidationResult>,
    sink_writes: Vec<SinkWrite>,
    state_history: Vec<StateTransition>,
}

impl AttributeValueLifecycle {
    pub async fn resolve(&mut self, dictionary: &DictionaryService) -> Result<()> {
        // State machine logic
    }
}
```

---

## Architecture Recommendations

### Recommendation #1: Fix Type Safety (PRIORITY 1)

**Action**: Use `AttributeId` consistently everywhere

```rust
// Step 1: Fix AttributeDefinition
pub struct AttributeDefinition {
    pub attr_id: AttributeId,  // Changed from String
    // ... rest unchanged
}

// Step 2: Fix DictionaryAttribute
pub struct DictionaryAttribute {
    pub attribute_id: AttributeId,  // Changed from Uuid
    // ... rest unchanged
}

// Step 3: Update all signatures
pub trait DictionaryService {
    async fn get_attribute(&self, attribute_id: AttributeId) -> Result<Option<AttributeDefinition>>;
    // ... rest updated
}
```

---

### Recommendation #2: Implement DSL Attribute Syntax (PRIORITY 1)

**Action**: Add parser support for `@attr{uuid}` references

```rust
// parser/mod.rs
pub enum Token {
    // ... existing tokens
    AttributeRef(Uuid),  // @attr{uuid}
}

// parser/primitives.rs
pub fn parse_attribute_ref(input: &str) -> IResult<&str, AttributeId> {
    // Parse: @attr{a8f3c1d2-4b5e-6789-abcd-ef0123456789}
    preceded(
        tag("@attr{"),
        terminated(
            map_res(
                take_while_m_n(36, 36, |c: char| c.is_alphanumeric() || c == '-'),
                |s: &str| Uuid::parse_str(s).map(AttributeId::from)
            ),
            tag("}")
        )
    )(input)
}

// AST node
pub enum Value {
    // ... existing variants
    AttributeRef(AttributeId),
}
```

---

### Recommendation #3: Implement Source/Sink Execution (PRIORITY 1)

**Action**: Create executor traits and implementations

```rust
// New file: data_dictionary/source_executor.rs
#[async_trait]
pub trait SourceExecutor: Send + Sync {
    async fn fetch_value(
        &self,
        attr_id: AttributeId,
        context: &ExecutionContext,
        config: &HashMap<String, Value>
    ) -> Result<FetchedValue>;
    
    fn source_type(&self) -> SourceType;
}

pub struct FetchedValue {
    pub value: Value,
    pub confidence: f64,
    pub retrieved_at: DateTime<Utc>,
    pub metadata: HashMap<String, Value>,
}

// Concrete implementations
pub struct DocumentExtractionExecutor {
    document_service: Arc<dyn DocumentService>,
}

impl SourceExecutor for DocumentExtractionExecutor {
    async fn fetch_value(&self, attr_id: AttributeId, ctx: &ExecutionContext, config: &HashMap<String, Value>) -> Result<FetchedValue> {
        let doc_id = config.get("document_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing document_id in source config"))?;
        
        let extraction_path = config.get("extraction_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing extraction_path in source config"))?;
        
        // Load document
        let document = self.document_service.get_document(doc_id).await?;
        
        // Apply extraction rules
        let value = document.extract_value(extraction_path)?;
        
        Ok(FetchedValue {
            value,
            confidence: 0.95,  // From OCR confidence
            retrieved_at: Utc::now(),
            metadata: hashmap! {
                "document_id".to_string() => Value::String(doc_id.to_string()),
                "extraction_method".to_string() => Value::String("OCR".to_string()),
            },
        })
    }
}

// Similar for sinks
#[async_trait]
pub trait SinkWriter: Send + Sync {
    async fn write_value(
        &self,
        attr_id: AttributeId,
        value: &Value,
        context: &ExecutionContext,
        config: &HashMap<String, Value>
    ) -> Result<WriteResult>;
}
```

---

### Recommendation #4: Add Compilation Context (PRIORITY 2)

**Action**: Extend ExecutionContext with attribute tracking

```rust
// execution/context.rs
pub struct ExecutionContext {
    pub session_id: Uuid,
    pub cbu_id: Uuid,  // The Onboarding Request ID
    pub domain: String,
    pub executor: String,
    pub started_at: DateTime<Utc>,
    
    // NEW: Attribute-specific context
    pub attributes: AttributeContext,
    
    pub environment: HashMap<String, Value>,
    pub integrations: Vec<String>,
}

pub struct AttributeContext {
    // All attribute values in this context
    values: HashMap<AttributeId, AttributeValue>,
    
    // Track provenance for audit
    provenance: HashMap<AttributeId, ValueProvenance>,
    
    // Dictionary reference for lookups
    dictionary: Arc<dyn DictionaryService>,
}

impl AttributeContext {
    pub fn get_value(&self, attr_id: &AttributeId) -> Option<&Value> {
        self.values.get(attr_id).map(|av| &av.value)
    }
    
    pub async fn resolve_attribute(&mut self, attr_id: AttributeId) -> Result<Value> {
        // 1. Check if already resolved
        if let Some(av) = self.values.get(&attr_id) {
            return Ok(av.value.clone());
        }
        
        // 2. Get attribute definition
        let attr_def = self.dictionary.get_attribute(attr_id.clone()).await?
            .ok_or_else(|| anyhow!("Unknown attribute: {}", attr_id))?;
        
        // 3. Try sources in priority order
        let fetched = self.try_sources(&attr_def.sources).await?;
        
        // 4. Validate value
        self.dictionary.validate_attribute_value(&attr_id, &fetched.value).await?;
        
        // 5. Store in context
        self.values.insert(attr_id.clone(), AttributeValue {
            attr_id: attr_id.clone(),
            value: fetched.value.clone(),
            state: ValueState::Resolved,
            validated: true,
            confidence: fetched.confidence,
        });
        
        self.provenance.insert(attr_id, ValueProvenance {
            source_type: fetched.source_type,
            source_details: fetched.metadata,
            retrieved_at: fetched.retrieved_at,
            retrieval_method: fetched.method,
        });
        
        Ok(fetched.value)
    }
}
```

---

### Recommendation #5: Implement RAG Integration (PRIORITY 2)

**Action**: Add vector DB support and semantic search

```rust
// New file: data_dictionary/rag_service.rs
pub struct AttributeRagService {
    dictionary_db: Arc<DictionaryDatabaseService>,
    vector_db: Arc<dyn VectorDbClient>,
    embedding_service: Arc<dyn EmbeddingService>,
}

impl AttributeRagService {
    pub async fn semantic_search(&self, query: &str, limit: usize) -> Result<Vec<(AttributeId, f64)>> {
        // 1. Generate embedding for query
        let query_embedding = self.embedding_service.embed_text(query).await?;
        
        // 2. Search vector DB
        let results = self.vector_db.search(
            "attributes",
            &query_embedding,
            limit
        ).await?;
        
        // 3. Return (AttributeId, similarity_score) pairs
        Ok(results.into_iter()
            .map(|(id, score)| (AttributeId::from(id), score))
            .collect())
    }
    
    pub async fn index_attribute(&self, attr_id: AttributeId) -> Result<()> {
        // 1. Get attribute definition
        let attr = self.dictionary_db.get_by_id(attr_id.into()).await?
            .ok_or_else(|| anyhow!("Attribute not found"))?;
        
        // 2. Build comprehensive text for embedding
        let text = format!(
            "{}\n\n{}\n\nUsage examples:\n{}",
            attr.name,
            attr.long_description.unwrap_or_default(),
            // Would include usage_examples if we had them
            ""
        );
        
        // 3. Generate embedding
        let embedding = self.embedding_service.embed_text(&text).await?;
        
        // 4. Store in vector DB
        self.vector_db.upsert(
            "attributes",
            attr_id.to_string(),
            &embedding,
            hashmap! {
                "name" => json!(attr.name),
                "domain" => json!(attr.domain),
            }
        ).await?;
        
        Ok(())
    }
}

// Usage in AI-enhanced operations
pub async fn ai_attribute_discovery(
    user_query: &str,
    rag_service: &AttributeRagService
) -> Result<Vec<AttributeId>> {
    // User says: "I need to collect the customer's passport number"
    // AI translates to semantic search
    let candidates = rag_service.semantic_search(user_query, 5).await?;
    
    // Returns: [(passport_number_attr_id, 0.95), (tax_id_attr_id, 0.72), ...]
    Ok(candidates.into_iter().map(|(id, _)| id).collect())
}
```

---

### Recommendation #6: Unify Database Schema (PRIORITY 2)

**Action**: Make database schema match code model

**Option A: Normalize** (Recommended)
```sql
-- Main dictionary table (lean)
CREATE TABLE "ob-poc".dictionary (
    attribute_id UUID PRIMARY KEY,
    name VARCHAR(255) UNIQUE NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    data_type VARCHAR(50) NOT NULL,  -- enum: string, numeric, date, etc.
    group_id VARCHAR(100) NOT NULL DEFAULT 'default',
    domain VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Semantic metadata (separate table)
CREATE TABLE "ob-poc".attribute_semantics (
    attribute_id UUID PRIMARY KEY REFERENCES "ob-poc".dictionary(attribute_id),
    description TEXT NOT NULL,
    context TEXT,
    related_concepts TEXT[],
    usage_examples JSONB,
    regulatory_citations TEXT[],
    embedding vector(3072),  -- Using pgvector
    embedding_model VARCHAR(100),
    embedded_at TIMESTAMPTZ
);

-- Sources (normalized)
CREATE TABLE "ob-poc".attribute_sources (
    source_id UUID PRIMARY KEY,
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id),
    priority INTEGER NOT NULL,  -- 1=primary, 2=secondary, 3=tertiary
    source_type VARCHAR(50) NOT NULL,  -- enum
    config JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Sinks (normalized)
CREATE TABLE "ob-poc".attribute_sinks (
    sink_id UUID PRIMARY KEY,
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id),
    sink_type VARCHAR(50) NOT NULL,  -- operational, master, archive, audit, analytics
    config JSONB NOT NULL,
    retention_policy VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Constraints
CREATE TABLE "ob-poc".attribute_constraints (
    attribute_id UUID PRIMARY KEY REFERENCES "ob-poc".dictionary(attribute_id),
    min_value NUMERIC,
    max_value NUMERIC,
    precision INTEGER,
    pattern VARCHAR(500),
    allowed_values JSONB,
    required BOOLEAN DEFAULT false
);

-- UI Metadata
CREATE TABLE "ob-poc".attribute_ui_metadata (
    attribute_id UUID PRIMARY KEY REFERENCES "ob-poc".dictionary(attribute_id),
    category VARCHAR(100),
    subcategory VARCHAR(100),
    display_order INTEGER,
    form_section VARCHAR(100),
    layout_weight NUMERIC,
    visual_importance VARCHAR(20),  -- critical, high, medium, low
    proximity_preferences TEXT[],
    break_after BOOLEAN DEFAULT false
);
```

**Option B: Keep JSONB but Add Structure**
```sql
-- Use PostgreSQL JSONB schema validation
CREATE TABLE "ob-poc".dictionary (
    attribute_id UUID PRIMARY KEY,
    name VARCHAR(255) UNIQUE NOT NULL,
    -- ... basic fields ...
    
    -- Structured JSONB with schema
    semantic JSONB NOT NULL CHECK (
        jsonb_typeof(semantic) = 'object' AND
        semantic ? 'description' AND
        semantic ? 'context'
    ),
    
    sources JSONB CHECK (
        jsonb_typeof(sources) = 'object'
    ),
    
    sinks JSONB CHECK (
        jsonb_typeof(sinks) = 'object'
    ),
    
    embedding vector(3072),  -- Proper vector type
    embedding_metadata JSONB
);

-- Create typed accessors
CREATE OR REPLACE FUNCTION get_primary_source(attr_id UUID)
RETURNS JSONB AS $$
    SELECT sources->'primary'
    FROM "ob-poc".dictionary
    WHERE attribute_id = attr_id;
$$ LANGUAGE SQL;
```

---

## Proposed Redesign Architecture

### Layer 1: Dictionary Core
```
dictionary/
├── types.rs              # AttributeId, AttributeDefinition, etc.
├── repository.rs         # Database CRUD operations
├── service.rs            # Business logic layer
└── validation.rs         # Constraint validation
```

### Layer 2: Value Resolution
```
resolution/
├── source_executor.rs    # Source execution traits
├── sources/
│   ├── document.rs       # Document extraction
│   ├── solicitation.rs   # User input forms
│   ├── third_party.rs    # External API calls
│   └── internal.rs       # Internal system lookups
├── sink_writer.rs        # Sink persistence traits
├── sinks/
│   ├── postgresql.rs     # DB persistence
│   ├── s3.rs            # S3 archival
│   └── audit.rs         # Audit logging
└── lifecycle.rs          # State machine for value lifecycle
```

### Layer 3: RAG Integration
```
rag/
├── embedding_service.rs  # Generate embeddings
├── vector_db.rs         # Vector DB client
├── semantic_search.rs   # Semantic attribute discovery
└── indexer.rs           # Background job to index attributes
```

### Layer 4: DSL Integration
```
dsl_integration/
├── parser_extensions.rs  # @attr{uuid} syntax support
├── compiler.rs          # Resolve attributes at compile time
└── context.rs           # Enhanced execution context
```

---

## Implementation Roadmap

### Phase 1: Foundation (Week 1-2)
- [ ] Fix type safety issues (use AttributeId everywhere)
- [ ] Align database schema with code model
- [ ] Implement DictionaryService trait
- [ ] Add comprehensive unit tests

### Phase 2: DSL Integration (Week 3)
- [ ] Implement `@attr{uuid}` parser support
- [ ] Add attribute reference AST nodes
- [ ] Implement dictionary validation in parser
- [ ] Add compilation-time attribute resolution

### Phase 3: Value Resolution (Week 4-5)
- [ ] Implement SourceExecutor trait and executors
- [ ] Implement SinkWriter trait and writers
- [ ] Create AttributeValueLifecycle state machine
- [ ] Add ExecutionContext attribute tracking
- [ ] Implement multi-source conflict resolution

### Phase 4: RAG Integration (Week 6)
- [ ] Set up vector DB (Qdrant or pgvector)
- [ ] Implement embedding generation service
- [ ] Create semantic search service
- [ ] Build background indexer
- [ ] Add AI-assisted attribute discovery

### Phase 5: Testing & Documentation (Week 7)
- [ ] End-to-end integration tests
- [ ] Performance testing
- [ ] Documentation and examples
- [ ] Migration guide from current implementation

---

## Estimated Effort

| Phase | Effort | Complexity | Risk |
|-------|--------|------------|------|
| Phase 1: Foundation | 80 hours | Medium | Low |
| Phase 2: DSL Integration | 40 hours | Medium | Medium |
| Phase 3: Value Resolution | 120 hours | High | Medium |
| Phase 4: RAG Integration | 60 hours | Medium | Low |
| Phase 5: Testing/Docs | 40 hours | Low | Low |
| **TOTAL** | **340 hours** | **High** | **Medium** |

**Team**: 1 senior Rust developer + 1 architect  
**Timeline**: 7-8 weeks  
**Cost**: $40-60K (consulting rates)

---

## Critical Success Factors

1. **Type Safety First**: Use AttributeId everywhere, no exceptions
2. **Schema Alignment**: Database schema must match code model
3. **Executable Definitions**: Source/sink definitions must actually execute
4. **Real RAG**: Vector DB integration must be functional, not mock
5. **Testing**: Comprehensive tests for each component
6. **Documentation**: Clear examples showing the complete flow

---

## Conclusion

The current attribute dictionary implementation has the **right architectural ideas** but **poor execution**. The core concept of AttributeID-as-Type with multi-source value resolution and RAG support is sound, but almost none of it actually works.

**Recommendation**: **STOP** using the current implementation in production. It will appear to work (basic CRUD operations function) but will fail when you need:
- Attribute reference in DSL
- Multi-source value resolution
- Semantic attribute discovery
- Type-safe attribute handling

**Path Forward**: Follow the 7-week roadmap above to build a proper implementation. This is foundational infrastructure that everything else depends on - it's worth doing right.

---

**Prepared By**: Claude  
**Review Date**: November 13, 2025  
**Severity**: CRITICAL  
**Status**: REQUIRES IMMEDIATE ATTENTION
