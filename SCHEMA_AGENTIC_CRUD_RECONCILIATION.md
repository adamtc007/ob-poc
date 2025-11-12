# Schema-Agentic CRUD Reconciliation Plan

## 🎯 Executive Summary

This document provides a comprehensive reconciliation between the canonical `ob-poc-schema.sql` (55+ tables) and the current agentic DSL CRUD implementation, identifying gaps and creating a roadmap for complete AI agent coverage across all database operations.

## 📊 Current State Analysis

### ✅ **Schema Status: COMPLETE**
- **Canonical Schema**: `ob-poc-schema.sql` (882 lines, 55+ tables)
- **Architecture**: DSL-as-State + AttributeID-as-Type + AI Integration
- **Domains**: 7 operational domains with 70+ approved verbs
- **Status**: Production-ready, fully consolidated

### ✅ **Existing Agentic CRUD Services**
| Service | Status | Coverage | Database Service |
|---------|--------|----------|------------------|
| `AgenticCrudService` | ✅ Implemented | General CRUD | `CbuRepository` |
| `AgenticDictionaryService` | ✅ Implemented | Dictionary/Attributes | `DictionaryDatabaseService` |
| `AgenticDocumentService` | ✅ Implemented | Document Catalog | (Planned) |
| `UnifiedAgenticService` | ✅ Implemented | Orchestration Layer | Multiple |

### 🔍 **Schema Coverage Analysis**

#### ✅ **COVERED TABLES (8/55 = 14%)**
```
✅ dictionary               - AgenticDictionaryService
✅ attribute_values         - AgenticDictionaryService  
✅ cbus                     - AgenticCrudService
✅ cbu_entity_roles         - AgenticCrudService
✅ document_catalog         - AgenticDocumentService
✅ crud_operations          - Built-in tracking
✅ dsl_examples             - AgenticCrudService
✅ rag_embeddings           - Built-in RAG system
```

#### ❌ **UNCOVERED TABLES (47/55 = 86%)**

**Entity Management Domain (11 tables)**
```
❌ entities                 - Core entity registry
❌ entity_types             - Entity type definitions  
❌ entity_limited_companies - Corporate entities
❌ entity_partnerships      - Partnership structures
❌ entity_proper_persons    - Natural persons
❌ entity_trusts            - Trust structures
❌ entity_product_mappings  - Entity-product compatibility
❌ entity_crud_rules        - Entity validation rules
❌ entity_lifecycle_status  - Status tracking
❌ entity_validation_rules  - Business rules
❌ master_entity_xref       - Cross-reference lookup
```

**Product & Services Domain (7 tables)**
```
❌ products                 - Product definitions
❌ services                 - Service offerings
❌ product_requirements     - Product-specific requirements
❌ product_services         - Product-service relationships
❌ product_workflows        - Workflow definitions
❌ service_resources        - Service resource mapping
❌ prod_resources           - Production resources
```

**UBO & Ownership Domain (6 tables)**
```
❌ ubo_registry             - Ultimate beneficial ownership
❌ partnership_interests    - Ownership percentages
❌ partnership_control_mechanisms - Control relationships
❌ trust_parties            - Trust participant roles
❌ trust_beneficiary_classes - Trust beneficiary structures
❌ trust_protector_powers   - Trust governance powers
```

**DSL Management Domain (8 tables)**
```
❌ dsl_domains              - Domain definitions
❌ dsl_versions             - Version management
❌ dsl_ob                   - DSL instance storage
❌ dsl_execution_log        - Execution tracking
❌ dsl_execution_summary    - Performance metrics
❌ dsl_latest_versions      - Current state tracking
❌ parsed_asts              - Compiled AST storage
❌ domain_vocabularies      - Verb definitions
```

**Document Library V3.1 (4 tables)**
```
❌ document_types           - Document classifications
❌ document_metadata        - EAV document attributes
❌ document_relationships   - Document linking
❌ document_issuers_backup  - Issuer information
```

**Orchestration & Workflow (5 tables)**
```
❌ orchestration_sessions   - Workflow sessions
❌ orchestration_domain_sessions - Domain-specific sessions
❌ orchestration_state_history - State change tracking
❌ orchestration_tasks      - Task management
❌ vocab_registry           - Vocabulary management
```

**Compliance & Governance (6 tables)**
```
❌ vocabulary_audit         - Schema change tracking
❌ schema_changes           - Database evolution log
❌ referential_integrity_check - Data integrity validation
❌ master_jurisdictions     - Jurisdiction definitions
❌ roles                    - Role definitions
❌ grammar_rules            - DSL grammar definitions
```

## 🚀 **RECONCILIATION ROADMAP**

### **Phase 1: Core Entity Management (Priority 1)**
**Target: Complete entity lifecycle AI management**

#### 1.1 Database Services
```rust
// rust/src/database/entity_service.rs
pub struct EntityDatabaseService {
    // CRUD for entities, entity_types, entity_* tables
    // Relationship management
    // Validation rule enforcement
}
```

#### 1.2 Agentic Service
```rust  
// rust/src/ai/agentic_entity_service.rs
pub struct AgenticEntityService {
    // AI-powered entity operations
    // Natural language entity creation
    // Relationship discovery and mapping
}
```

#### 1.3 DSL Verbs
```lisp
(entity.create :type "LIMITED_COMPANY" :name "TechCorp Ltd" :jurisdiction "GB")
(entity.link :from @entity-uuid :to @entity-uuid :relationship "SUBSIDIARY")
(entity.validate :entity-id @uuid :rules ["incorporation", "ownership"])
(entity.lifecycle :entity-id @uuid :status "ACTIVE" :effective-date "2025-01-15")
```

### **Phase 2: Product & Services Management (Priority 2)**
**Target: Complete product portfolio AI management**

#### 2.1 Database Services
```rust
// rust/src/database/product_service.rs
pub struct ProductDatabaseService {
    // Product/service CRUD
    // Requirement management
    // Workflow orchestration
}
```

#### 2.2 Agentic Service
```rust
// rust/src/ai/agentic_product_service.rs  
pub struct AgenticProductService {
    // AI-powered product operations
    // Service discovery and configuration
    // Requirement analysis
}
```

#### 2.3 DSL Verbs
```lisp
(product.create :name "Hedge Fund Services" :category "INVESTMENT")
(service.provision :product-id @uuid :client-id @uuid :config {...})
(workflow.initiate :product-id @uuid :entity-id @uuid :stage "ONBOARDING")
(requirement.validate :product-id @uuid :entity-type "PARTNERSHIP")
```

### **Phase 3: UBO & Ownership Intelligence (Priority 3)**
**Target: Complete ownership chain AI analysis**

#### 3.1 Database Services
```rust
// rust/src/database/ubo_service.rs
pub struct UboDatabaseService {
    // UBO calculation engine
    // Ownership chain analysis
    // Compliance threshold monitoring
}
```

#### 3.2 Agentic Service
```rust
// rust/src/ai/agentic_ubo_service.rs
pub struct AgenticUboService {
    // AI-powered UBO analysis
    // Natural language ownership queries
    // Compliance risk assessment
}
```

#### 3.3 DSL Verbs
```lisp
(ubo.analyze :entity-id @uuid :threshold 0.25 :jurisdiction "GB")
(ownership.calculate :entity-id @uuid :method "DIRECT_INDIRECT")
(control.assess :entity-id @uuid :mechanisms ["VOTING", "ECONOMIC"])
(compliance.validate :ubo-structure @uuid :regulations ["4MLD", "5MLD"])
```

### **Phase 4: Document Intelligence V3.1 (Priority 4)**
**Target: Complete document lifecycle AI management**

#### 4.1 Database Services
```rust
// rust/src/database/document_service.rs (Enhanced)
pub struct DocumentDatabaseService {
    // Document catalog management
    // Metadata EAV operations  
    // Relationship mapping
    // AI extraction integration
}
```

#### 4.2 Enhanced Agentic Service
```rust
// Enhanced rust/src/ai/agentic_document_service.rs
pub struct AgenticDocumentService {
    // AI document analysis
    // Content extraction
    // Relationship discovery
    // Compliance validation
}
```

#### 4.3 DSL Verbs (Enhanced)
```lisp
(document.extract :doc-id @uuid :fields ["company_name", "directors"] :confidence 0.9)
(document.relate :primary-doc @uuid :related-doc @uuid :type "AMENDMENT")
(document.validate :doc-id @uuid :rules ["SIGNATURE", "DATE", "COMPLETENESS"])
(document.type.classify :doc-id @uuid :ai-confidence 0.95)
```

### **Phase 5: DSL Management Intelligence (Priority 5)**
**Target: Self-managing DSL system with AI**

#### 5.1 Database Services
```rust
// rust/src/database/dsl_management_service.rs  
pub struct DslManagementService {
    // DSL version control
    // AST compilation management
    // Performance analytics
    // Domain vocabulary management
}
```

#### 5.2 Agentic Service
```rust
// rust/src/ai/agentic_dsl_service.rs
pub struct AgenticDslService {
    // AI-powered DSL evolution
    // Performance optimization
    // Vocabulary expansion
    // Grammar enhancement
}
```

#### 5.3 DSL Verbs
```lisp
(dsl.evolve :domain "UBO" :capability "TRUST_ANALYSIS" :version "3.2")
(vocabulary.expand :domain "ISDA" :verbs ["margin_dispute", "portfolio_reconcile"])
(performance.optimize :execution-id @uuid :target-latency 100)  
(grammar.validate :dsl-text "..." :version "3.1" :strict true)
```

### **Phase 6: Orchestration & Workflow AI (Priority 6)**
**Target: Intelligent workflow orchestration**

#### 6.1 Database Services
```rust
// rust/src/database/orchestration_service.rs
pub struct OrchestrationService {
    // Workflow session management
    // Task orchestration
    // State transition tracking
}
```

#### 6.2 Agentic Service  
```rust
// rust/src/ai/agentic_orchestration_service.rs
pub struct AgenticOrchestrationService {
    // AI workflow optimization
    // Task prioritization
    // Resource allocation
}
```

#### 6.3 DSL Verbs
```lisp
(workflow.orchestrate :session-id @uuid :tasks [...] :priority "HIGH")
(task.optimize :workflow-id @uuid :resource-constraints {...})
(state.predict :current-state "KYC_REVIEW" :next-states [...])
(session.analyze :performance-metrics {...} :bottlenecks [...])
```

### **Phase 7: Compliance & Governance AI (Priority 7)**
**Target: AI-powered compliance monitoring**

#### 7.1 Database Services
```rust
// rust/src/database/compliance_service.rs
pub struct ComplianceService {
    // Rule validation
    // Audit trail management
    // Integrity checking
    // Jurisdiction compliance
}
```

#### 7.2 Agentic Service
```rust
// rust/src/ai/agentic_compliance_service.rs  
pub struct AgenticComplianceService {
    // AI compliance analysis
    // Risk assessment
    // Regulatory change detection
    // Audit preparation
}
```

#### 7.3 DSL Verbs
```lisp
(compliance.monitor :entity-id @uuid :regulations ["GDPR", "MiFID_II"])
(audit.prepare :scope "UBO_COMPLIANCE" :period "2024-Q4" :format "REGULATORY")
(integrity.validate :table "entities" :rules ["FK_CONSTRAINTS", "BUSINESS_RULES"])
(regulation.analyze :jurisdiction "EU" :changes-since "2024-01-01")
```

## 🎯 **IMPLEMENTATION METRICS**

### **Success Criteria**
- **100% Table Coverage**: All 55+ tables have agentic CRUD operations
- **AI-First Operations**: Natural language → DSL → Database execution
- **Performance**: <2s response time for standard operations  
- **Reliability**: 99.9% operation success rate
- **Extensibility**: New tables auto-generate agentic services

### **Development Timeline**
- **Phase 1-3**: Core business domains (8 weeks)
- **Phase 4-5**: Advanced intelligence (6 weeks)  
- **Phase 6-7**: Orchestration & compliance (4 weeks)
- **Total**: 18 weeks for complete AI agent coverage

### **Resource Requirements**
- **Database Services**: 7 new comprehensive services
- **Agentic Services**: 7 new AI-powered services  
- **DSL Verbs**: 40+ new verbs across domains
- **Test Coverage**: 200+ integration tests
- **Documentation**: Complete API and usage documentation

## 🏗️ **TECHNICAL ARCHITECTURE**

### **Pattern Consistency**
All new services follow the established agentic pattern:

```rust
// Consistent service architecture
pub struct Agentic{Domain}Service {
    db_service: {Domain}DatabaseService,
    rag_system: CrudRagSystem,
    prompt_builder: CrudPromptBuilder,
    ai_client: Arc<dyn AiService + Send + Sync>,
    config: {Domain}ServiceConfig,
}

impl Agentic{Domain}Service {
    pub async fn natural_language_operation(
        &self,
        instruction: String
    ) -> Result<Agentic{Domain}Response> {
        // 1. RAG context retrieval
        // 2. AI DSL generation  
        // 3. DSL parsing & validation
        // 4. Database execution
        // 5. Response formatting
    }
}
```

### **Database Service Pattern**
```rust
// Consistent database service architecture
pub struct {Domain}DatabaseService {
    pool: PgPool,
}

impl {Domain}DatabaseService {
    // Standard CRUD operations
    pub async fn create(&self, request: Create{Domain}Request) -> Result<{Domain}>;
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<{Domain}>>;
    pub async fn update(&self, id: Uuid, request: Update{Domain}Request) -> Result<{Domain}>;
    pub async fn delete(&self, id: Uuid) -> Result<()>;
    
    // Domain-specific operations
    pub async fn search(&self, criteria: {Domain}SearchCriteria) -> Result<Vec<{Domain}>>;
    pub async fn validate(&self, validation: {Domain}Validation) -> Result<ValidationResult>;
}
```

### **Unified Service Integration**
The `UnifiedAgenticService` orchestrates all domain services:

```rust
pub struct UnifiedAgenticService {
    entity_service: AgenticEntityService,
    product_service: AgenticProductService,  
    ubo_service: AgenticUboService,
    document_service: AgenticDocumentService,
    dsl_service: AgenticDslService,
    orchestration_service: AgenticOrchestrationService,
    compliance_service: AgenticComplianceService,
    // ... existing services
}
```

## 🔄 **MIGRATION STRATEGY**

### **Backwards Compatibility**
- All existing services remain functional
- New services integrate seamlessly
- DSL verbs are additive, not replacing
- Database schema unchanged (uses canonical `ob-poc-schema.sql`)

### **Rollout Plan**
1. **Phase-by-phase deployment** - Each domain can be deployed independently
2. **Feature flags** - Enable/disable new agentic capabilities per client
3. **A/B testing** - Compare AI vs manual operations
4. **Performance monitoring** - Track AI operation success rates

### **Risk Mitigation**
- **Comprehensive testing** - Each service has unit, integration, and performance tests
- **Rollback capability** - Instant fallback to manual operations
- **AI confidence thresholds** - High-confidence operations only
- **Human oversight** - Critical operations require approval

## 🎉 **EXPECTED OUTCOMES**

### **Business Benefits**
- **10x Faster Onboarding** - Natural language to production deployment  
- **99% Accuracy** - AI-powered data validation and integrity
- **24/7 Operations** - Autonomous system management
- **Regulatory Compliance** - Automated compliance monitoring and reporting

### **Technical Benefits**  
- **Complete AI Coverage** - Every database table has intelligent operations
- **Self-Healing System** - AI identifies and resolves data issues
- **Predictive Analytics** - AI forecasts system needs and optimizations
- **Knowledge Management** - AI captures and shares operational knowledge

### **Developer Experience**
- **Natural Language APIs** - English → Database operations
- **Auto-generated Services** - New tables automatically get agentic coverage  
- **Intelligent Testing** - AI generates comprehensive test scenarios
- **Living Documentation** - AI maintains up-to-date system documentation

---

**🎯 Target State**: Complete AI agent ecosystem with 100% database coverage, enabling natural language operations across all 55+ tables in the ob-poc schema.

**🚀 Next Steps**: Begin Phase 1 implementation with Entity Management domain services.

**📅 Timeline**: Full "AI agents everywhere" deployment within 18 weeks.