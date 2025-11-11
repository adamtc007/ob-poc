# Agentic DSL CRUD Implementation Plan

## Overview

This plan outlines the implementation of an AI-powered agentic system that uses DSL to perform CRUD (Create, Read, Update, Delete) operations on core data assets in the PostgreSQL database. The system combines RAG (Retrieval-Augmented Generation) with DSL-constrained prompts to provide natural language interfaces for database operations.

## Target Data Assets

### 1. CBU (Client Business Unit) Management
- **Table**: `"ob-poc".cbus`
- **Operations**: Create new CBUs, read CBU details, update CBU metadata, archive CBUs
- **Key Attributes**: name, description, nature_purpose, entity relationships

### 2. Attribute Dictionary Management
- **Table**: `"ob-poc".dictionary`
- **Operations**: Define new attributes, query attribute metadata, update attribute definitions, deprecate attributes
- **Key Attributes**: name, long_description, group_id, mask, domain, source, sink

### 3. Document Library Management
- **Tables**: Document catalog, document types, document relationships
- **Operations**: Catalog documents, query document library, link documents, manage document lifecycle
- **Key Attributes**: document metadata, extraction fields, compliance classifications

## Architecture Design

### Core Components

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Natural       │    │   Agentic DSL   │    │   Database      │
│   Language      │───▶│   CRUD Engine   │───▶│   Operations    │
│   Interface     │    │                 │    │   (SQLX)        │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   RAG Context   │    │   DSL Grammar   │    │   PostgreSQL    │
│   Retrieval     │    │   Validator     │    │   "ob-poc"      │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### 1. Agentic DSL CRUD Engine (`src/services/agentic_crud_service.rs`)

```rust
pub struct AgenticCrudService {
    ai_client: Arc<dyn AiService>,
    database_manager: Arc<DatabaseManager>,
    rag_system: Arc<RagSystem>,
    grammar_validator: Arc<GrammarValidator>,
    crud_executor: Arc<CrudExecutor>,
}

pub enum CrudOperation {
    Create,
    Read,
    Update,
    Delete,
}

pub enum DataAsset {
    Cbu,
    Attribute,
    Document,
}

pub struct AgenticCrudRequest {
    pub instruction: String,           // Natural language instruction
    pub operation: Option<CrudOperation>,
    pub asset_type: Option<DataAsset>,
    pub context: HashMap<String, String>,
    pub constraints: Vec<String>,
}

pub struct AgenticCrudResponse {
    pub generated_dsl: String,
    pub executed_operations: Vec<DatabaseOperation>,
    pub affected_records: usize,
    pub rag_context: Vec<RetrievedContext>,
    pub validation_results: ValidationResults,
    pub ai_explanation: String,
    pub confidence: f64,
}
```

### 2. RAG System for Context Retrieval (`src/services/rag_system.rs`)

```rust
pub struct RagSystem {
    embedding_service: Arc<EmbeddingService>,
    vector_store: Arc<VectorStore>,
    schema_cache: Arc<SchemaCache>,
    example_library: Arc<ExampleLibrary>,
}

pub struct RetrievedContext {
    pub context_type: ContextType,
    pub content: String,
    pub relevance_score: f64,
    pub source: String,
}

pub enum ContextType {
    SchemaDefinition,
    ExampleDsl,
    AttributeMetadata,
    BusinessRule,
    RelatedData,
}
```

### 3. Enhanced DSL Grammar for CRUD Operations

#### New DSL Verbs for CRUD Operations

```ebnf
(* CRUD-specific verbs for data asset management *)
crud-verb = cbu-verb | attribute-verb | document-crud-verb ;

(* CBU Management Verbs *)
cbu-verb = "cbu.create" | "cbu.read" | "cbu.update" | "cbu.delete" | "cbu.query" ;

(* Attribute Management Verbs *)
attribute-verb = "attribute.define" | "attribute.query" | "attribute.update" | "attribute.deprecate" ;

(* Document CRUD Verbs (extending existing document.* verbs) *)
document-crud-verb = "document.create" | "document.read" | "document.update" | "document.delete" ;
```

#### DSL Examples for Each Data Asset

**CBU Operations:**
```lisp
;; Create new CBU
(cbu.create
  :name "Alpha Tech Ventures"
  :description "Technology investment fund"
  :nature-purpose "Private equity investment in technology startups"
  :jurisdiction "US"
  :entity-type "LIMITED_PARTNERSHIP")

;; Query CBU information
(cbu.query
  :filters {:jurisdiction "US" :entity-type "LIMITED_PARTNERSHIP"}
  :include-relationships true
  :include-attributes ["nature_purpose" "created_at"])

;; Update CBU metadata
(cbu.update
  :cbu-id @attr{cbu-id-001}
  :description "Updated: Technology and AI investment fund"
  :nature-purpose "Private equity investment in AI and technology startups")
```

**Attribute Operations:**
```lisp
;; Define new attribute
(attribute.define
  :name "entity.ai_risk_score"
  :description "AI-calculated risk assessment score for entities"
  :group-id "Risk"
  :domain "Risk"
  :mask "decimal"
  :source {:type "ai_model" :model "risk_assessment_v2"}
  :sink {:type "database" :table "risk_scores"})

;; Query attributes by domain
(attribute.query
  :domain "Risk"
  :group-id "Risk"
  :include-usage-stats true)

;; Update attribute metadata
(attribute.update
  :attribute-id @attr{ai-risk-score-001}
  :description "Enhanced: ML-powered entity risk assessment with behavioral analysis"
  :source {:type "ai_model" :model "risk_assessment_v3" :confidence_threshold 0.85})
```

**Document Operations:**
```lisp
;; Create document entry
(document.create
  :document-id "doc-passport-001"
  :document-type "PASSPORT"
  :title "John Smith Passport"
  :issuer "US_STATE_DEPARTMENT"
  :confidentiality-level "restricted"
  :extracted-fields {@attr{document.passport.number} "123456789"
                     @attr{document.passport.full_name} "John Smith"
                     @attr{document.passport.nationality} "US"})

;; Query documents with complex filters
(document.query
  :filters {:document-type ["PASSPORT" "NATIONAL_ID"]
           :issuer-country "US"
           :confidentiality-level "restricted"}
  :extract-fields [@attr{document.passport.number}
                   @attr{document.passport.expiry_date}]
  :sort-by "created_at"
  :limit 100)
```

## Implementation Phases

### Phase 1: Foundation (Week 1-2)

#### 1.1 Database Schema Enhancement
```sql
-- Add CRUD operation tracking
CREATE TABLE IF NOT EXISTS "ob-poc".crud_operations (
    operation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    operation_type VARCHAR(20) NOT NULL, -- CREATE, READ, UPDATE, DELETE
    asset_type VARCHAR(50) NOT NULL,     -- CBU, ATTRIBUTE, DOCUMENT
    generated_dsl TEXT NOT NULL,
    ai_instruction TEXT NOT NULL,
    affected_records JSONB NOT NULL,
    execution_status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    ai_confidence DECIMAL(3,2),
    created_by VARCHAR(255) DEFAULT 'agentic_system',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

-- Add vector embeddings for RAG
CREATE TABLE IF NOT EXISTS "ob-poc".rag_embeddings (
    embedding_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    content_type VARCHAR(50) NOT NULL, -- SCHEMA, EXAMPLE, ATTRIBUTE, RULE
    content_text TEXT NOT NULL,
    embedding vector(1536), -- OpenAI ada-002 dimensions
    metadata JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Add example DSL library
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_examples (
    example_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    operation_type VARCHAR(20) NOT NULL,
    asset_type VARCHAR(50) NOT NULL,
    example_dsl TEXT NOT NULL,
    expected_outcome TEXT,
    tags TEXT[],
    usage_count INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

#### 1.2 Core Service Implementation
- Implement `AgenticCrudService` with basic CRUD operations
- Add SQLX integration for all target tables
- Create DSL grammar extensions for CRUD verbs

#### 1.3 AI Integration Enhancement
- Extend existing AI service to support CRUD-specific prompts
- Add system prompts with comprehensive database schema context
- Implement confidence scoring for generated operations

### Phase 2: RAG System Integration (Week 3-4)

#### 2.1 RAG System Implementation
```rust
impl RagSystem {
    // Retrieve relevant schema information
    pub async fn get_schema_context(&self, asset_type: DataAsset) -> Vec<RetrievedContext>;
    
    // Find similar example DSL operations
    pub async fn find_similar_examples(&self, instruction: &str, operation: CrudOperation) -> Vec<RetrievedContext>;
    
    // Get attribute metadata for AttributeID resolution
    pub async fn get_attribute_context(&self, attribute_refs: Vec<&str>) -> Vec<RetrievedContext>;
    
    // Retrieve business rules and constraints
    pub async fn get_business_rules(&self, asset_type: DataAsset) -> Vec<RetrievedContext>;
}
```

#### 2.2 Context-Aware Prompt Generation
```rust
pub struct CrudPromptBuilder {
    pub fn build_system_prompt(&self, asset_type: DataAsset, operation: CrudOperation) -> String;
    pub fn add_schema_context(&mut self, schema: Vec<RetrievedContext>);
    pub fn add_examples(&mut self, examples: Vec<RetrievedContext>);
    pub fn add_business_rules(&mut self, rules: Vec<RetrievedContext>);
}
```

#### 2.3 Vector Embedding Setup
- Implement document embedding for schema definitions
- Create embeddings for DSL examples
- Set up similarity search with pgvector extension

### Phase 3: Advanced DSL Operations (Week 5-6)

#### 3.1 Complex Query Support
```lisp
;; Multi-table joins with AttributeID resolution
(cbu.query
  :join-entities true
  :join-documents true
  :filters {:created-after "2024-01-01"
           :has-attributes [@attr{entity.legal_name}
                           @attr{entity.registration_number}]}
  :aggregate {:count-documents true
             :sum-attribute-values [@attr{entity.aum}]})

;; Conditional operations
(attribute.update
  :where {:domain "Financial" :group-id "AML"}
  :if-exists {:source.type "manual"}
  :set {:source {:type "ai_enhanced" :confidence_required 0.9}
       :validation_rules ["aml_threshold_check"]})
```

#### 3.2 Transaction Support
```rust
pub struct CrudTransaction {
    pub operations: Vec<AgenticCrudRequest>,
    pub rollback_strategy: RollbackStrategy,
    pub atomic: bool,
}

pub enum RollbackStrategy {
    FullRollback,
    PartialRollback,
    ContinueOnError,
}
```

#### 3.3 Validation and Safety
```rust
pub struct CrudValidator {
    pub fn validate_operation(&self, request: &AgenticCrudRequest) -> ValidationResult;
    pub fn check_permissions(&self, operation: &CrudOperation, asset: &DataAsset) -> bool;
    pub fn validate_referential_integrity(&self, dsl: &str) -> IntegrityResult;
    pub fn simulate_operation(&self, dsl: &str) -> SimulationResult;
}
```

### Phase 4: Production Features (Week 7-8)

#### 4.1 Audit and Compliance
- Complete audit trails for all CRUD operations
- Data lineage tracking for attribute changes
- Compliance reporting for document operations

#### 4.2 Performance Optimization
- Query result caching
- Batch operation support
- Connection pooling optimization

#### 4.3 Security Implementation
- Role-based access control for CRUD operations
- Sensitive data masking in AI prompts
- Secure handling of document content

## Natural Language Interface Examples

### CBU Management
```
Human: "Create a new CBU for Quantum Tech Fund, a Delaware LP focused on quantum computing investments"

AI Response:
Generated DSL:
(cbu.create
  :name "Quantum Tech Fund LP"
  :description "Delaware limited partnership specializing in quantum computing investments"
  :nature-purpose "Private equity investment in quantum computing and related technologies"
  :jurisdiction "US-DE"
  :entity-type "LIMITED_PARTNERSHIP")

Execution Result: Created CBU with ID: cbu-quantum-tech-001
```

### Attribute Management
```
Human: "Add a new attribute to track ESG scores for entities, should be calculated by our ESG model"

AI Response:
Generated DSL:
(attribute.define
  :name "entity.esg_score"
  :description "Environmental, Social, and Governance composite score for entities"
  :group-id "ESG"
  :domain "Compliance"
  :mask "decimal"
  :source {:type "ai_model" :model "esg_calculator_v1" :update_frequency "monthly"}
  :sink {:type "database" :table "esg_scores"})

Execution Result: Attribute defined with ID: attr-esg-score-001
```

### Document Operations
```
Human: "Find all passports issued by the UK for individuals born before 1990"

AI Response:
Generated DSL:
(document.query
  :filters {:document-type "PASSPORT"
           :issuer-country "GB"}
  :where-extracted-field {:field @attr{document.passport.date_of_birth}
                         :condition "before"
                         :value "1990-01-01"}
  :include-extracted-fields [@attr{document.passport.full_name}
                            @attr{document.passport.date_of_birth}
                            @attr{document.passport.expiry_date}])

Execution Result: Found 23 matching documents
```

## Quality Assurance & Testing

### Unit Tests
- CRUD operation validation
- DSL generation accuracy
- Database integration correctness

### Integration Tests
- End-to-end natural language → DSL → database workflows
- RAG context retrieval accuracy
- Multi-operation transaction handling

### Performance Tests
- Large dataset query performance
- Concurrent operation handling
- Vector similarity search benchmarks

### Security Tests
- SQL injection prevention
- Access control validation
- Data privacy compliance

## Monitoring & Observability

### Metrics
- Operation success/failure rates
- AI confidence score distributions
- Query performance metrics
- RAG context relevance scores

### Logging
- Complete audit trails for all operations
- AI decision reasoning logs
- Performance bottleneck identification

### Alerting
- Failed operation notifications
- Low confidence score alerts
- Performance degradation warnings

## Deployment Strategy

### Development Environment
- Local PostgreSQL with test data
- Mock AI services for development
- Comprehensive test suite

### Staging Environment
- Production-like data subset
- Real AI service integration
- Performance testing

### Production Environment
- Full dataset access
- Production AI services
- Comprehensive monitoring

## Success Criteria

1. **Accuracy**: >95% correct DSL generation for common CRUD operations
2. **Performance**: <2 second response time for standard operations
3. **Reliability**: >99.9% uptime for critical operations
4. **Usability**: Natural language interface intuitive for business users
5. **Security**: Zero data breaches or unauthorized access
6. **Compliance**: Full audit trail for all data modifications

## Future Enhancements

### Advanced AI Features
- Multi-step operation planning
- Automatic data quality improvement
- Predictive data maintenance

### Extended Data Assets
- User management through DSL
- Workflow state management
- Configuration management

### Integration Capabilities
- REST API for external systems
- Webhook notifications for data changes
- Real-time data synchronization

## Implementation Timeline

| Phase | Duration | Key Deliverables |
|-------|----------|------------------|
| Phase 1 | 2 weeks | Core CRUD service, basic AI integration |
| Phase 2 | 2 weeks | RAG system, context-aware prompts |
| Phase 3 | 2 weeks | Advanced DSL operations, transactions |
| Phase 4 | 2 weeks | Production features, security, monitoring |
| **Total** | **8 weeks** | **Production-ready agentic CRUD system** |

## Risk Mitigation

### Technical Risks
- **AI Hallucination**: Implement strict DSL validation and simulation
- **Performance Issues**: Comprehensive caching and optimization
- **Data Corruption**: Atomic transactions and rollback capabilities

### Business Risks
- **User Adoption**: Intuitive interface and comprehensive documentation
- **Data Security**: Role-based access and audit trails
- **Compliance**: Built-in regulatory compliance features

This comprehensive plan provides a roadmap for implementing a production-ready agentic DSL CRUD system that combines the power of AI with the precision of database operations while maintaining security, performance, and reliability standards.