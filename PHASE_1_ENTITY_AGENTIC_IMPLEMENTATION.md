# Phase 1: Entity Management Agentic CRUD Implementation

## 🎯 Overview

**Phase 1 Objective**: Implement comprehensive AI-powered CRUD operations for the Entity Management domain, covering 11 core tables that manage entity lifecycles, relationships, and business rules.

**Priority**: **CRITICAL** - Entities are the foundation of all onboarding operations
**Timeline**: 4 weeks
**Success Metrics**: 100% coverage of entity tables with natural language operations

## 📊 Entity Domain Analysis

### **Core Tables (11 total)**
```sql
-- Primary entity management
entities                    -- Core entity registry (UUID-based)
entity_types               -- Entity type definitions (Corp, Trust, etc.)
entity_limited_companies   -- Corporate entity details
entity_partnerships        -- Partnership structures  
entity_proper_persons      -- Natural person details
entity_trusts             -- Trust structures

-- Entity relationships & rules
entity_product_mappings    -- Entity-product compatibility matrix
entity_crud_rules         -- Business rule definitions
entity_lifecycle_status   -- Status tracking and transitions
entity_validation_rules   -- Validation rule engine
master_entity_xref        -- Cross-reference lookup system
```

### **Current Coverage Gap**
- **Status**: 0% agentic coverage for entity domain
- **Impact**: Manual entity operations, no AI assistance
- **Risk**: Bottleneck for all onboarding workflows

## 🏗️ Implementation Architecture

### **1. Database Service Layer**

#### **File**: `rust/src/database/entity_service.rs`

```rust
//! Entity Database Service - Comprehensive entity management operations
//!
//! This service provides database operations for all entity-related tables,
//! supporting the full entity lifecycle from creation to archival.

use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};

/// Comprehensive entity database service
pub struct EntityDatabaseService {
    pool: PgPool,
}

impl EntityDatabaseService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // CORE ENTITY OPERATIONS
    pub async fn create_entity(&self, request: CreateEntityRequest) -> Result<Entity>;
    pub async fn get_entity(&self, entity_id: Uuid) -> Result<Option<Entity>>;  
    pub async fn update_entity(&self, entity_id: Uuid, request: UpdateEntityRequest) -> Result<Entity>;
    pub async fn delete_entity(&self, entity_id: Uuid) -> Result<()>;
    
    // ENTITY TYPE MANAGEMENT
    pub async fn create_entity_type(&self, request: CreateEntityTypeRequest) -> Result<EntityType>;
    pub async fn get_entity_types(&self) -> Result<Vec<EntityType>>;
    pub async fn get_compatible_products(&self, entity_type_id: Uuid) -> Result<Vec<ProductMapping>>;
    
    // SPECIALIZED ENTITY OPERATIONS
    pub async fn create_limited_company(&self, request: CreateLimitedCompanyRequest) -> Result<LimitedCompany>;
    pub async fn create_partnership(&self, request: CreatePartnershipRequest) -> Result<Partnership>;
    pub async fn create_proper_person(&self, request: CreateProperPersonRequest) -> Result<ProperPerson>;
    pub async fn create_trust(&self, request: CreateTrustRequest) -> Result<Trust>;
    
    // RELATIONSHIP MANAGEMENT
    pub async fn link_entities(&self, relationship: EntityRelationship) -> Result<()>;
    pub async fn get_entity_relationships(&self, entity_id: Uuid) -> Result<Vec<EntityRelationship>>;
    pub async fn validate_relationship(&self, relationship: &EntityRelationship) -> Result<ValidationResult>;
    
    // LIFECYCLE MANAGEMENT
    pub async fn update_entity_status(&self, request: EntityStatusUpdate) -> Result<EntityLifecycleStatus>;
    pub async fn get_entity_status_history(&self, entity_id: Uuid) -> Result<Vec<EntityLifecycleStatus>>;
    pub async fn validate_status_transition(&self, from: &str, to: &str, entity_type: &str) -> Result<bool>;
    
    // VALIDATION & RULES
    pub async fn validate_entity(&self, entity_id: Uuid, rule_set: Vec<String>) -> Result<ValidationResult>;
    pub async fn get_entity_rules(&self, entity_type: &str) -> Result<Vec<EntityCrudRule>>;
    pub async fn create_validation_rule(&self, rule: EntityValidationRule) -> Result<EntityValidationRule>;
    
    // SEARCH & DISCOVERY
    pub async fn search_entities(&self, criteria: EntitySearchCriteria) -> Result<Vec<Entity>>;
    pub async fn discover_entity_network(&self, root_entity_id: Uuid, max_depth: u32) -> Result<EntityNetwork>;
    pub async fn get_entity_xrefs(&self, entity_id: Uuid) -> Result<Vec<MasterEntityXref>>;
}

// Supporting data structures
#[derive(Debug, Serialize, Deserialize)]
pub struct Entity {
    pub entity_id: Uuid,
    pub entity_type_id: Uuid,
    pub external_id: Option<String>,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ... additional 20+ data structures for complete entity management
```

### **2. Models Layer**

#### **File**: `rust/src/models/entity_models.rs`

```rust
//! Entity Models - Complete data structures for entity management
//!
//! This module defines all data structures used in entity operations,
//! following the AttributeID-as-Type pattern and supporting AI operations.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use std::collections::HashMap;

// CORE ENTITY STRUCTURES
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub entity_id: Uuid,
    pub entity_type_id: Uuid,
    pub external_id: Option<String>,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityType {
    pub entity_type_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub table_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// SPECIALIZED ENTITY TYPES
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitedCompany {
    pub limited_company_id: Uuid,
    pub company_name: String,
    pub registration_number: Option<String>,
    pub jurisdiction: Option<String>,
    pub incorporation_date: Option<NaiveDate>,
    pub registered_address: Option<String>,
    pub business_nature: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partnership {
    pub partnership_id: Uuid,
    pub partnership_name: String,
    pub partnership_type: Option<String>,
    pub jurisdiction: Option<String>,
    pub formation_date: Option<NaiveDate>,
    pub principal_place_business: Option<String>,
    pub partnership_agreement_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProperPerson {
    pub proper_person_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: Option<NaiveDate>,
    pub nationality: Option<String>,
    pub passport_number: Option<String>,
    pub residential_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trust {
    pub trust_id: Uuid,
    pub trust_name: String,
    pub trust_type: Option<String>,
    pub governing_law: Option<String>,
    pub establishment_date: Option<NaiveDate>,
    pub trust_deed_date: Option<NaiveDate>,
    pub principal_office_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// AGENTIC CRUD REQUEST/RESPONSE STRUCTURES
#[derive(Debug, Serialize, Deserialize)]
pub struct AgenticEntityCreateRequest {
    pub ai_instruction: String,
    pub entity_type: String,
    pub entity_data: HashMap<String, serde_json::Value>,
    pub relationships: Vec<EntityRelationshipRequest>,
    pub validation_rules: Vec<String>,
    pub confidence_threshold: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgenticEntityResponse {
    pub entity_id: Option<Uuid>,
    pub generated_dsl: String,
    pub execution_status: EntityExecutionStatus,
    pub ai_confidence: f32,
    pub ai_provider: String,
    pub operation_type: EntityOperationType,
    pub affected_records: Vec<Uuid>,
    pub validation_results: Vec<ValidationResult>,
    pub execution_time_ms: u64,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

// OPERATION TYPES
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityOperationType {
    Create,
    Read,
    Update, 
    Delete,
    Link,
    Validate,
    StatusUpdate,
    NetworkDiscovery,
    RuleApplication,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityExecutionStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    PartialSuccess,
}

// VALIDATION & BUSINESS RULES
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityValidationRule {
    pub rule_id: Uuid,
    pub entity_type: String,
    pub rule_name: String,
    pub rule_expression: String,
    pub error_message: String,
    pub severity: ValidationSeverity,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
}

// ... 15+ additional supporting structures for complete entity management
```

### **3. Agentic Service Layer**

#### **File**: `rust/src/ai/agentic_entity_service.rs`

```rust
//! Agentic Entity Service - AI-Powered Entity Management
//!
//! This service provides natural language interfaces for entity management,
//! converting business requirements into DSL operations and database execution.

use crate::ai::crud_prompt_builder::CrudPromptBuilder;
use crate::ai::rag_system::{CrudRagSystem, RetrievedContext};
use crate::ai::{AiService, AiDslRequest, AiResponseType};
use crate::database::EntityDatabaseService;
use crate::models::entity_models::*;
use crate::parser::idiomatic_parser::parse_crud_statement;

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// AI-powered entity management service
pub struct AgenticEntityService {
    /// Database service for entity operations
    db_service: EntityDatabaseService,
    /// RAG system for entity context
    rag_system: CrudRagSystem,
    /// AI prompt builder
    prompt_builder: CrudPromptBuilder,
    /// AI client (OpenAI/Gemini)
    ai_client: Arc<dyn AiService + Send + Sync>,
    /// Service configuration
    config: EntityServiceConfig,
    /// Operation cache
    operation_cache: Arc<RwLock<HashMap<String, CachedEntityOperation>>>,
}

impl AgenticEntityService {
    /// Create new agentic entity service
    pub fn new(
        db_service: EntityDatabaseService,
        ai_client: Arc<dyn AiService + Send + Sync>,
        config: EntityServiceConfig,
    ) -> Self {
        let rag_system = CrudRagSystem::new_with_entity_context();
        let prompt_builder = CrudPromptBuilder::new_for_entities();
        
        Self {
            db_service,
            rag_system,
            prompt_builder,
            ai_client,
            config,
            operation_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create entity from natural language instruction
    pub async fn create_entity_from_instruction(
        &self,
        instruction: String,
    ) -> Result<AgenticEntityResponse> {
        // 1. RAG context retrieval
        let context = self.rag_system.retrieve_entity_context(&instruction).await?;
        
        // 2. Build AI prompt for entity creation
        let prompt = self.prompt_builder.build_entity_creation_prompt(&instruction, &context)?;
        
        // 3. Generate DSL via AI
        let ai_request = AiDslRequest {
            instruction: instruction.clone(),
            context: Some(context.to_context_string()),
            response_type: AiResponseType::EntityDsl,
            model_config: self.config.ai_model_config.clone(),
        };
        
        let ai_response = self.ai_client.generate_dsl(ai_request).await?;
        
        // 4. Parse and validate generated DSL
        let parsed_statement = parse_crud_statement(&ai_response.dsl_content)?;
        
        // 5. Execute entity creation
        let execution_result = self.execute_entity_creation(parsed_statement, &instruction).await?;
        
        // 6. Build comprehensive response
        Ok(AgenticEntityResponse {
            entity_id: execution_result.entity_id,
            generated_dsl: ai_response.dsl_content,
            execution_status: EntityExecutionStatus::Completed,
            ai_confidence: ai_response.confidence,
            ai_provider: ai_response.provider.clone(),
            operation_type: EntityOperationType::Create,
            affected_records: execution_result.affected_records,
            validation_results: execution_result.validation_results,
            execution_time_ms: ai_response.processing_time_ms + execution_result.execution_time_ms,
            error_message: None,
            created_at: chrono::Utc::now(),
        })
    }

    /// Link entities using natural language description
    pub async fn link_entities_from_instruction(
        &self,
        instruction: String,
    ) -> Result<AgenticEntityResponse> {
        // Similar pattern for entity relationship management
        // Natural language → AI DSL generation → Database execution
    }

    /// Validate entity using business rules
    pub async fn validate_entity_from_instruction(
        &self,
        instruction: String,
    ) -> Result<AgenticEntityResponse> {
        // AI-powered entity validation with business rules
    }

    /// Discover entity network and relationships
    pub async fn discover_entity_network_from_instruction(
        &self,
        instruction: String,
    ) -> Result<AgenticEntityResponse> {
        // AI-powered entity network analysis and discovery
    }

    /// Update entity status with natural language reasoning
    pub async fn update_entity_status_from_instruction(
        &self,
        instruction: String,
    ) -> Result<AgenticEntityResponse> {
        // AI-powered entity lifecycle management
    }

    // PRIVATE HELPER METHODS
    async fn execute_entity_creation(
        &self,
        statement: CrudStatement,
        original_instruction: &str,
    ) -> Result<EntityExecutionResult> {
        // Complex entity creation logic with full validation
    }

    async fn build_entity_context(&self, entity_type: &str) -> Result<EntityContext> {
        // Build comprehensive context for AI operations
    }
}

// SUPPORTING STRUCTURES
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityServiceConfig {
    pub ai_model_config: EntityAiModelConfig,
    pub validation_config: EntityValidationConfig,
    pub execution_config: EntityExecutionConfig,
    pub performance_config: EntityPerformanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityExecutionResult {
    pub entity_id: Option<Uuid>,
    pub affected_records: Vec<Uuid>,
    pub validation_results: Vec<ValidationResult>,
    pub execution_time_ms: u64,
}

// ... 10+ additional supporting structures
```

### **4. DSL Verb Extensions**

#### **New Entity Management Verbs** (15 total)

```lisp
;; CORE ENTITY OPERATIONS
(entity.create 
  :type "LIMITED_COMPANY"
  :name "TechCorp Limited" 
  :jurisdiction "GB"
  :incorporation-date "2024-01-15"
  :registration-number "12345678")

(entity.read :entity-id @uuid :include-relationships true)

(entity.update 
  :entity-id @uuid 
  :name "TechCorp Holdings Limited"
  :registered-address "123 Tech Street, London, UK")

(entity.delete :entity-id @uuid :cascade-relationships false)

;; RELATIONSHIP MANAGEMENT  
(entity.link 
  :from-entity @parent-uuid 
  :to-entity @subsidiary-uuid 
  :relationship-type "SUBSIDIARY"
  :ownership-percentage 75.5
  :control-mechanism "VOTING_SHARES")

(entity.unlink :relationship-id @uuid :effective-date "2025-01-15")

(entity.discover-network 
  :root-entity @uuid 
  :max-depth 3 
  :include-dormant false)

;; SPECIALIZED ENTITY CREATION
(entity.create-company
  :name "Hedge Fund GP Limited"
  :jurisdiction "KY" 
  :business-purpose "General Partner activities"
  :directors [@director1-uuid @director2-uuid])

(entity.create-partnership
  :name "TechCorp Investment Partnership"
  :type "LIMITED_LIABILITY" 
  :jurisdiction "DE"
  :general-partner @gp-entity-uuid)

(entity.create-trust
  :name "TechCorp Employee Benefit Trust"
  :governing-law "GB"
  :trustee @trustee-entity-uuid
  :beneficiary-classes ["EMPLOYEES" "RETIREES"])

;; LIFECYCLE MANAGEMENT
(entity.update-status 
  :entity-id @uuid 
  :new-status "DORMANT" 
  :effective-date "2025-03-31"
  :reason "Cessation of business activities")

(entity.validate 
  :entity-id @uuid 
  :rules ["INCORPORATION_VALID" "DIRECTORS_APPOINTED" "REGISTERED_ADDRESS"]
  :jurisdiction-specific true)

;; BUSINESS RULE OPERATIONS
(entity.create-rule
  :name "UK_COMPANY_DIRECTORS_MINIMUM"
  :entity-type "LIMITED_COMPANY"
  :jurisdiction "GB" 
  :rule-expression "directors.count >= 1"
  :severity "ERROR")

(entity.apply-rules 
  :entity-id @uuid 
  :rule-set "ONBOARDING_VALIDATION"
  :stop-on-error false)

;; ADVANCED OPERATIONS
(entity.audit-trail :entity-id @uuid :from-date "2024-01-01" :include-relationships true)
```

## 🧪 Testing Strategy

### **1. Unit Tests** (25 tests)
```rust
// rust/tests/entity_unit_tests.rs
#[tokio::test]
async fn test_entity_creation_from_natural_language() {
    let instruction = "Create a UK limited company called TechCorp with registration number 12345678";
    let response = entity_service.create_entity_from_instruction(instruction).await?;
    
    assert!(response.entity_id.is_some());
    assert!(response.generated_dsl.contains("entity.create"));
    assert_eq!(response.execution_status, EntityExecutionStatus::Completed);
}
```

### **2. Integration Tests** (15 tests) 
```rust
// rust/tests/entity_integration_tests.rs  
#[tokio::test]
async fn test_complete_entity_workflow() {
    // 1. Create parent company
    // 2. Create subsidiary
    // 3. Link entities
    // 4. Validate relationship
    // 5. Update status
    // 6. Audit trail verification
}
```

### **3. Performance Tests** (10 tests)
```rust
// rust/tests/entity_performance_tests.rs
#[tokio::test]
async fn test_bulk_entity_creation_performance() {
    // Test creating 100 entities with relationships
    // Target: <5 seconds total, <2 seconds per operation
}
```

## 📈 Success Metrics

### **Functional Metrics**
- ✅ **100% Entity Table Coverage** - All 11 entity tables have agentic operations
- ✅ **Natural Language Interface** - English instructions → Database operations  
- ✅ **DSL Generation Accuracy** - 95%+ correct DSL from natural language
- ✅ **Operation Success Rate** - 99%+ successful database operations
- ✅ **Relationship Integrity** - 100% referential integrity maintenance

### **Performance Metrics**  
- ✅ **Response Time** - <2 seconds for standard entity operations
- ✅ **Bulk Operations** - Handle 100+ entities in <30 seconds
- ✅ **Network Discovery** - 3-level entity networks in <5 seconds
- ✅ **AI Processing** - <1 second for DSL generation
- ✅ **Database Operations** - <500ms for CRUD operations

### **Quality Metrics**
- ✅ **Test Coverage** - 95%+ code coverage across all entity services
- ✅ **Error Handling** - Graceful degradation and comprehensive error messages
- ✅ **Logging & Monitoring** - Full operation traceability
- ✅ **Documentation** - Complete API and usage documentation
- ✅ **Security** - Role-based access control and audit trails

## 🚀 Deployment Plan

### **Week 1: Foundation**
- ✅ Database service implementation (`entity_service.rs`)
- ✅ Core entity models (`entity_models.rs`)  
- ✅ Basic CRUD operations with 5 entity types
- ✅ Unit test coverage (15 tests)

### **Week 2: Intelligence Layer**
- ✅ Agentic service implementation (`agentic_entity_service.rs`)
- ✅ AI prompt engineering for entity operations
- ✅ DSL verb parsing and validation
- ✅ Integration with existing RAG system

### **Week 3: Advanced Features**  
- ✅ Entity relationship management and validation
- ✅ Business rule engine integration
- ✅ Entity lifecycle status management
- ✅ Network discovery algorithms
- ✅ Performance optimization

### **Week 4: Integration & Testing**
- ✅ Integration with `UnifiedAgenticService`
- ✅ Comprehensive integration tests (15 tests)
- ✅ Performance testing and optimization  
- ✅ Documentation and examples
- ✅ Production readiness validation

## 🔗 Integration Points

### **Existing Services Integration**
```rust
// Enhanced UnifiedAgenticService
impl UnifiedAgenticService {
    pub fn with_entity_service(mut self, entity_service: AgenticEntityService) -> Self {
        self.entity_service = Some(entity_service);
        self
    }
    
    pub async fn route_entity_operation(&self, request: UnifiedRequest) -> Result<UnifiedResponse> {
        match request.domain {
            Domain::Entity => {
                let entity_service = self.entity_service.as_ref()
                    .ok_or_else(|| anyhow!("Entity service not configured"))?;
                entity_service.process_request(request).await
            },
            _ => self.route_to_other_services(request).await,
        }
    }
}
```

### **Database Manager Integration**
```rust
// Enhanced DatabaseManager
impl DatabaseManager {
    pub fn entity_service(&self) -> EntityDatabaseService {
        EntityDatabaseService::new(self.pool.clone())
    }
}
```

## 🎯 Next Steps

### **Phase 1 Completion Criteria**
- [ ] All 11 entity tables have comprehensive agentic CRUD operations
- [ ] 15+ DSL verbs for entity management implemented and tested
- [ ] Natural language → DSL → Database execution pipeline functional
- [ ] 95%+ test coverage with unit, integration, and performance tests
- [ ] Integration with existing unified service architecture
- [ ] Documentation and examples complete

### **Phase 2 Prerequisites**
- ✅ Entity foundation enables product/service relationship validation
- ✅ Entity network discovery supports UBO analysis preparation
- ✅ Entity lifecycle management supports compliance workflows
- ✅ Business rule engine foundation for advanced validation

### **Risk Mitigation**
- **Complexity Management**: Incremental delivery with weekly milestones
- **Performance Concerns**: Early performance testing and optimization
- **Integration Issues**: Continuous integration with existing services
- **AI Reliability**: Confidence thresholds and fallback mechanisms
- **Data Integrity**: Comprehensive validation and rollback capabilities

---

**🎯 Phase 1 Success**: Complete AI-powered entity management enabling natural language entity operations across all 11 entity tables, laying the foundation for comprehensive "AI agents everywhere" implementation.