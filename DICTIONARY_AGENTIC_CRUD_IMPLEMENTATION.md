# 📋 DICTIONARY AGENTIC CRUD - IMPLEMENTATION TASK

## 🎯 OBJECTIVE
Implement comprehensive agentic CRUD operations for the `"ob-poc".dictionary` table to enable AI-powered management of attribute definitions that form the foundation of our AttributeID-as-Type architecture.

## 📊 CURRENT STATE ANALYSIS

### ✅ EXISTING FOUNDATION
- **Database Schema**: `"ob-poc".dictionary` table with 76+ attributes
- **Agentic Pattern**: Proven with entity CRUD (5/5 tests passing)
- **AI Integration**: OpenAI/Gemini support working
- **Parser System**: DSL verb parsing operational
- **Service Layer**: Established patterns in `ai/agentic_crud_service.rs`

### 🎯 TARGET IMPLEMENTATION
Following the established agentic CRUD pattern, implement dictionary management with these DSL verbs:

```lisp
;; Core operations
(attribute.create :name "kyc.risk_score" :description "Risk assessment score" :mask "decimal")
(attribute.read :name "kyc.risk_score")
(attribute.update :attribute-id @uuid :description "Updated description")
(attribute.delete :attribute-id @uuid)

;; Advanced operations
(attribute.search :domain "KYC" :mask "decimal" :limit 10)
(attribute.validate :attribute-id @uuid :value 85.5)
(attribute.discover :semantic-query "income related fields")
```

## 🏗️ IMPLEMENTATION CHECKLIST

### Phase 1: Core Models & Types
- [ ] **Create `models/dictionary_models.rs`**
  - [ ] `DictionaryAttribute` struct (matches DB schema)
  - [ ] `NewDictionaryAttribute` for creation
  - [ ] `UpdateDictionaryAttribute` for updates
  - [ ] `AgenticAttributeCreateRequest` following entity pattern
  - [ ] `AgenticAttributeCrudResponse` with standard fields
  - [ ] `AttributeOperationType` enum
  - [ ] Export from `models/mod.rs`

### Phase 2: Database Service
- [ ] **Create `database/dictionary_service.rs`**
  - [ ] `DictionaryDatabaseService` struct with PgPool
  - [ ] CRUD methods: create, get_by_id, get_by_name, update, delete
  - [ ] Search methods: by_domain, by_group, semantic_search
  - [ ] Validation method: validate_attribute_value
  - [ ] Export from `database/mod.rs`
  - [ ] Add to `DatabaseManager` as `dictionary_service()`

### Phase 3: AI Integration
- [ ] **Extend `ai/rag_system.rs`**
  - [ ] Add dictionary schema to asset schemas
  - [ ] Add attribute-specific retrieval logic
  - [ ] Update `identify_relevant_assets` for "attribute" queries

- [ ] **Create `ai/agentic_dictionary_service.rs`**
  - [ ] `AgenticDictionaryService` struct following agentic_crud pattern
  - [ ] Core methods: create_agentic, read_agentic, update_agentic, delete_agentic
  - [ ] Advanced: search_agentic, validate_agentic, discover_agentic
  - [ ] Integration with existing AI clients (OpenAI/Gemini)
  - [ ] Prompt generation for dictionary operations

### Phase 4: DSL Integration
- [ ] **Extend parser (`parser/idiomatic_parser.rs`)**
  - [ ] Add `attribute.*` verb parsing
  - [ ] Handle attribute-specific parameters (:name, :mask, :domain, etc.)
  - [ ] Support UUID references for attribute-id

- [ ] **Create `domains/dictionary.rs`**
  - [ ] `DictionaryDomainHandler` implementing `DomainHandler`
  - [ ] Route attribute.* verbs to appropriate service methods
  - [ ] Register in `domains/mod.rs`

### Phase 5: Testing & Validation
- [ ] **Create integration tests**
  - [ ] `tests/dictionary_agentic_crud_integration.rs`
  - [ ] Test all CRUD operations with database
  - [ ] Test AI-generated DSL execution
  - [ ] Test semantic search functionality

- [ ] **Create working example**
  - [ ] `examples/dictionary_agentic_crud_demo.rs`
  - [ ] Demonstrate natural language → DSL → database operations
  - [ ] Show semantic search and validation features

### Phase 6: Integration with Existing System
- [ ] **Update `services/mod.rs`**
  - [ ] Export AgenticDictionaryService
  - [ ] Add to unified service if needed

- [ ] **Update documentation**
  - [ ] Add dictionary verbs to DSL grammar
  - [ ] Update CLAUDE.md with new capabilities

## 📋 DATABASE SCHEMA REFERENCE

```sql
CREATE TABLE "ob-poc".dictionary (
    attribute_id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,           -- "entity.legal_name"
    long_description TEXT,                       -- Human-readable description  
    group_id VARCHAR(100) DEFAULT 'default',    -- "KYC", "Onboarding", "Entity"
    mask VARCHAR(50) DEFAULT 'string',          -- "string", "decimal", "date", "enum"
    domain VARCHAR(100),                         -- "Legal", "KYC", "Investment"
    vector TEXT,                                 -- For semantic search
    source JSONB,                               -- Source metadata (validation, format)
    sink JSONB,                                 -- Sink metadata (destination table)
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

## 🎯 SUCCESS CRITERIA

### Functional Requirements
- [ ] **AI-Generated CRUD**: Natural language → Dictionary DSL → Database operations
- [ ] **Full CRUD Coverage**: Create, Read, Update, Delete attributes
- [ ] **Advanced Operations**: Search, validate, semantic discovery
- [ ] **Integration**: Seamless with existing agentic CRUD system
- [ ] **Database Consistency**: All operations maintain referential integrity

### Quality Standards
- [ ] **Test Coverage**: >90% for dictionary operations  
- [ ] **Performance**: <2 seconds for AI-generated operations
- [ ] **Reliability**: Zero breaking changes to existing functionality
- [ ] **Documentation**: Complete DSL verb documentation

### Business Value
- [ ] **Self-Service**: Business users manage attributes via natural language
- [ ] **Data Discovery**: Semantic search for relevant attributes
- [ ] **Quality Control**: Automated validation and consistency
- [ ] **Audit Trail**: Complete change history via DSL operations

## 🚀 IMPLEMENTATION PRIORITY

**START HERE**: Phase 1 (Models) → Phase 2 (Database) → Phase 3 (AI Integration)

Each phase builds on established patterns from the successful entity agentic CRUD implementation. Follow the exact same architectural approach for consistency.

## 💡 KEY IMPLEMENTATION NOTES

1. **Follow Existing Patterns**: Use `ai/agentic_crud_service.rs` as the template
2. **Reuse Infrastructure**: Leverage existing AI clients, database patterns, parser logic
3. **Maintain Consistency**: Use same naming conventions, error handling, response formats
4. **Test Early**: Create tests alongside each phase for immediate validation
5. **Document Progress**: Update this checklist as each item is completed

## 🎯 EXPECTED OUTCOME

A fully functional dictionary agentic CRUD system that:
- Extends the proven agentic pattern to attribute management
- Provides natural language interface to dictionary operations  
- Maintains the AttributeID-as-Type architecture integrity
- Enables self-service attribute management for business users
- Forms the foundation for extending agentic CRUD to other core tables

**This implementation will complete the core data management layer for the agentic DSL system.**