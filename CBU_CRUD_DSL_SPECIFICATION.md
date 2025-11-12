# CBU CRUD DSL Specification

## Overview

This document specifies the comprehensive Client Business Unit (CBU) CRUD operations using DSL templates. CBU management involves complex multi-table operations across entities, attributes, relationships, and workflows.

## CBU Data Model Architecture

### Core CBU Tables
- `cbus` - Main CBU records
- `cbu_entity_roles` - Entity-to-CBU role mappings  
- `attribute_values` - CBU attribute storage
- `dsl_ob` - CBU DSL version history
- `orchestration_sessions` - CBU workflow management
- `product_workflows` - CBU product associations
- `ubo_registry` - Ultimate Beneficial Ownership calculations

### Relationship Dependencies
```
CBU (1) → (N) CBU_Entity_Roles → (1) Entity
CBU (1) → (N) Attribute_Values → (1) Dictionary
CBU (1) → (N) UBO_Registry → (1) Proper_Person
CBU (1) → (N) Product_Workflows → (1) Product
CBU (1) → (N) Orchestration_Sessions
```

## DSL CRUD Operations Specification

### 1. CBU Creation (Multi-Table Insert)

#### Simple CBU Creation
```lisp
(cbu.create
  :name "TechVenture Master Structure"
  :description "Delaware holding company structure"
  :nature_purpose "Investment holding and management"
  :jurisdiction "US-DE")
```

#### Complex CBU Creation with Entities
```lisp
(cbu.create
  :name "Global Investment Fund Structure"
  :description "Master-feeder fund architecture"
  :nature_purpose "Alternative investment management"
  :entities [
    {:entity_type "partnership" 
     :entity_id @entity{550e8400-e29b-41d4-a716-446655440000}
     :roles ["master_fund" "tax_transparent"]
     :ownership_percentage nil}
    {:entity_type "limited_company"
     :entity_id @entity{550e8400-e29b-41d4-a716-446655440001}
     :roles ["feeder_fund" "regulated_entity"]
     :ownership_percentage nil}
    {:entity_type "proper_person"
     :entity_id @entity{550e8400-e29b-41d4-a716-446655440002}
     :roles ["investment_manager" "key_person"]
     :ownership_percentage 15.0}
  ]
  :attributes [
    {:attribute_id @attr{regulatory_framework} :value "US_INVESTMENT_COMPANY_ACT"}
    {:attribute_id @attr{tax_classification} :value "PARTNERSHIP"}
    {:attribute_id @attr{aum_threshold} :value {:amount 100000000 :currency "USD"}}
    {:attribute_id @attr{investor_restrictions} :value ["ACCREDITED_ONLY" "US_PERSON_PROHIBITED"]}
  ]
  :products ["CUSTODY" "PRIME_BROKERAGE" "DERIVATIVES_CLEARING"]
  :services ["KYC" "AML_SCREENING" "REGULATORY_REPORTING"]
  :workflow_type "HEDGE_FUND_ONBOARDING"
  :auto_calculate_ubo true)
```

#### Enterprise CBU Creation
```lisp
(cbu.create
  :name "Smith Family Multi-Generation Office"
  :description "Complex family office structure across multiple jurisdictions"
  :nature_purpose "Wealth preservation and succession planning"
  :entities [
    {:entity_type "trust"
     :entity_id @entity{trust-cayman-001}
     :roles ["primary_trust" "offshore_vehicle"]
     :jurisdiction "KY"}
    {:entity_type "limited_company"
     :entity_id @entity{company-uk-002}
     :roles ["trustee_company" "regulated_entity"]
     :jurisdiction "GB"}
    {:entity_type "partnership"
     :entity_id @entity{partnership-us-003}
     :roles ["investment_vehicle" "tax_efficient"]
     :jurisdiction "US-DE"}
    {:entity_type "proper_person"
     :entity_id @entity{person-smith-001}
     :roles ["settlor" "ultimate_beneficiary"]
     :ownership_percentage 60.0}
    {:entity_type "proper_person"
     :entity_id @entity{person-smith-002}
     :roles ["beneficiary" "next_generation"]
     :ownership_percentage 40.0}
  ]
  :attributes [
    {:attribute_id @attr{primary_jurisdiction} :value "KY"}
    {:attribute_id @attr{tax_residence} :value ["US" "GB" "KY"]}
    {:attribute_id @attr{wealth_category} :value "UHNW"}
    {:attribute_id @attr{succession_planning} :value true}
    {:attribute_id @attr{privacy_level} :value "MAXIMUM"}
  ]
  :products ["PRIVATE_BANKING" "CUSTODY" "INVESTMENT_ADVISORY" "TAX_PLANNING"]
  :compliance_frameworks ["US_BSA" "UK_MLR" "CAYMAN_AML"]
  :workflow_type "FAMILY_OFFICE_SETUP")
```

### 2. CBU Read Operations

#### Simple CBU Read
```lisp
(cbu.read
  :cbu_id @cbu{550e8400-e29b-41d4-a716-446655440100})
```

#### Complete CBU Read with Relations
```lisp
(cbu.read
  :cbu_id @cbu{550e8400-e29b-41d4-a716-446655440100}
  :include_relations [
    "entities_with_roles"
    "attribute_values_resolved"
    "ubo_calculations_current"
    "product_workflows_active"
    "orchestration_status"
    "document_associations"
    "compliance_status"
  ]
  :with_history true
  :resolve_attribute_names true)
```

#### CBU Search Operations
```lisp
;; Search by business criteria
(cbu.search
  :where {
    :jurisdiction ["US-DE" "KY" "BVI" "LU"]
    :entity_types ["partnership" "trust"]
    :product_types ["CUSTODY" "PRIME_BROKERAGE"]
    :status ["ACTIVE" "PENDING_APPROVAL"]
    :aum_range {:min 50000000 :max 500000000}
    :created_after "2023-01-01"
  }
  :include_relations ["primary_entities" "ubo_summary" "compliance_status"]
  :order_by [["aum" "DESC"] ["created_at" "ASC"]]
  :limit 50
  :offset 0)

;; Search by entity involvement
(cbu.search
  :entity_filter {
    :entity_id @entity{550e8400-e29b-41d4-a716-446655440002}
    :roles ["beneficial_owner" "key_person" "signatory"]
    :ownership_threshold 10.0
  }
  :include_indirect_relationships true)

;; Search by compliance status
(cbu.search
  :compliance_filter {
    :frameworks ["US_BSA" "EU_AMLD5"]
    :status ["COMPLIANT" "PENDING_REVIEW"]
    :review_required_before "2024-03-31"
  }
  :include_relations ["compliance_details"])
```

### 3. CBU Update Operations

#### Simple CBU Update
```lisp
(cbu.update
  :cbu_id @cbu{550e8400-e29b-41d4-a716-446655440100}
  :updates {
    :name "Updated TechVenture Structure"
    :description "Expanded to include EU operations"
    :nature_purpose "Global investment holding and management"
  })
```

#### Complex CBU Update with Entity Operations
```lisp
(cbu.update
  :cbu_id @cbu{550e8400-e29b-41d4-a716-446655440100}
  :updates {
    :description "Restructured following regulatory changes"
  }
  :entity_operations [
    {:operation "add_entity"
     :entity_id @entity{550e8400-e29b-41d4-a716-446655440010}
     :entity_type "limited_company"
     :roles ["eu_subsidiary" "regulated_entity"]
     :jurisdiction "LU"
     :ownership_percentage nil}
    {:operation "update_roles"
     :entity_id @entity{550e8400-e29b-41d4-a716-446655440000}
     :add_roles ["master_entity" "primary_tax_vehicle"]
     :remove_roles ["simple_partnership"]}
    {:operation "update_ownership"
     :entity_id @entity{550e8400-e29b-41d4-a716-446655440002}
     :ownership_percentage 20.0
     :effective_date "2024-01-15"}
    {:operation "remove_entity"
     :entity_id @entity{550e8400-e29b-41d4-a716-446655440003}
     :removal_reason "Entity dissolved"
     :effective_date "2024-01-01"}
  ]
  :attribute_operations [
    {:operation "set"
     :attribute_id @attr{eu_presence} 
     :value true}
    {:operation "update"
     :attribute_id @attr{aum_threshold}
     :value {:amount 150000000 :currency "USD"}}
    {:operation "remove"
     :attribute_id @attr{legacy_classification}}
    {:operation "append"
     :attribute_id @attr{regulatory_frameworks}
     :value "EU_UCITS"}
  ]
  :product_operations [
    {:operation "add" 
     :products ["DERIVATIVES_CLEARING" "REPO_FINANCING"]}
    {:operation "remove" 
     :products ["LEGACY_PRIME_SERVICES"]}
    {:operation "update_status"
     :product "CUSTODY"
     :status "APPROVED"
     :effective_date "2024-01-20"}
  ]
  :recalculate_ubo true
  :update_compliance_status true)
```

### 4. CBU Specialized Operations

#### Entity Management Operations
```lisp
;; Add entity with detailed role assignment
(cbu.add_entity
  :cbu_id @cbu{550e8400-e29b-41d4-a716-446655440100}
  :entity_id @entity{new-entity-001}
  :entity_type "trust"
  :roles ["offshore_vehicle" "tax_efficient" "privacy_protection"]
  :jurisdiction "BVI"
  :ownership_details {
    :ownership_percentage 35.0
    :ownership_type "BENEFICIAL"
    :voting_rights 25.0
    :control_mechanisms ["BOARD_CONTROL" "VETO_RIGHTS"]
  }
  :effective_date "2024-02-01"
  :regulatory_notifications ["US_FBAR" "UK_TRUST_REGISTER"]
  :auto_recalculate_ubo true)

;; Remove entity with dependency management
(cbu.remove_entity
  :cbu_id @cbu{550e8400-e29b-41d4-a716-446655440100}
  :entity_id @entity{old-entity-001}
  :removal_strategy "GRACEFUL_WITHDRAWAL"
  :dependency_handling {
    :ownership_redistribution "PROPORTIONAL_TO_REMAINING"
    :document_references "ARCHIVE_WITH_NOTATION"
    :ubo_calculations "RECALCULATE_IMMEDIATELY"
    :compliance_notifications ["REGULATOR_FILING" "COUNTERPARTY_NOTICE"]
  }
  :effective_date "2024-01-31"
  :removal_reason "Corporate restructuring")
```

#### UBO Calculation Management
```lisp
;; Comprehensive UBO recalculation
(cbu.recalculate_ubo
  :cbu_id @cbu{550e8400-e29b-41d4-a716-446655440100}
  :calculation_parameters {
    :calculation_type "FULL_REFRESH"
    :regulatory_frameworks ["US_BSA" "EU_AMLD5" "UK_MLR"]
    :threshold_percentage 25.0
    :include_indirect_ownership true
    :include_control_mechanisms true
    :consolidation_rules "IFRS_10"
  }
  :verification_required true
  :output_formats ["REGULATORY_REPORT" "INTERNAL_SUMMARY" "AUDIT_TRAIL"])

;; UBO scenario analysis
(cbu.ubo_scenario
  :cbu_id @cbu{550e8400-e29b-41d4-a716-446655440100}
  :scenario_name "Post-acquisition-structure"
  :hypothetical_changes [
    {:entity_id @entity{acquiring-company-001}
     :new_ownership_percentage 51.0
     :acquisition_date "2024-06-01"}
    {:entity_id @entity{current-majority-owner}
     :new_ownership_percentage 25.0}
  ]
  :calculate_impact true)
```

#### Lifecycle and Status Management
```lisp
;; CBU lifecycle state transition
(cbu.update_status
  :cbu_id @cbu{550e8400-e29b-41d4-a716-446655440100}
  :status_transition {
    :from_status "PENDING_APPROVAL"
    :to_status "ACTIVE" 
    :transition_reason "KYC/AML approval completed"
    :approved_by @user{compliance-officer-001}
    :approval_reference "COMP-2024-001523"
    :conditions_met [
      "KYC_COMPLETE"
      "AML_SCREENING_PASSED"
      "REGULATORY_CLEARANCE_OBTAINED"
      "DOCUMENTATION_COMPLETE"
    ]
  }
  :effective_date "2024-01-20"
  :notification_required ["CLIENT" "RELATIONSHIP_MANAGER" "COMPLIANCE"]
  :audit_trail true)

;; CBU compliance review cycle
(cbu.initiate_review
  :cbu_id @cbu{550e8400-e29b-41d4-a716-446655440100}
  :review_type "ANNUAL_COMPLIANCE_REVIEW"
  :review_scope [
    "UBO_VERIFICATION"
    "ENTITY_STATUS_CONFIRMATION"
    "REGULATORY_COMPLIANCE_CHECK"
    "PRODUCT_SUITABILITY_REVIEW"
    "RISK_ASSESSMENT_UPDATE"
  ]
  :assigned_to @user{compliance-team}
  :due_date "2024-03-31"
  :priority "HIGH")
```

### 5. CBU Bulk and Batch Operations

#### Bulk CBU Updates
```lisp
(cbu.bulk_update
  :cbu_filter {
    :jurisdiction "US-DE"
    :status ["ACTIVE"]
    :created_before "2023-12-31"
  }
  :operations [
    {:type "add_attribute" 
     :attribute_id @attr{annual_compliance_review_date}
     :value "2024-12-31"}
    {:type "update_product_workflow"
     :product "CUSTODY"
     :new_compliance_framework "UPDATED_SEC_RULES_2024"}
    {:type "initiate_ubo_refresh"
     :reason "REGULATORY_REQUIREMENT_2024"}
  ]
  :execution_mode "PARALLEL"
  :batch_size 25
  :transaction_mode "ATOMIC_PER_CBU"
  :progress_reporting true)

;; Mass compliance update
(cbu.mass_compliance_update
  :compliance_requirement {
    :regulation "EU_DORA"
    :effective_date "2025-01-17"
    :applies_to_jurisdictions ["EU" "EEA"]
  }
  :cbu_filter {
    :has_eu_presence true
    :product_types ["ELECTRONIC_TRADING" "CUSTODY"]
  }
  :required_actions [
    {:action "ADD_ICT_RISK_FRAMEWORK"}
    {:action "UPDATE_THIRD_PARTY_OVERSIGHT"}
    {:action "IMPLEMENT_INCIDENT_REPORTING"}
  ]
  :implementation_deadline "2024-11-17")
```

### 6. CBU Delete Operations

#### Soft Delete with Archive
```lisp
(cbu.delete
  :cbu_id @cbu{550e8400-e29b-41d4-a716-446655440100}
  :deletion_strategy "SOFT_DELETE"
  :archive_configuration {
    :retention_period_years 7
    :archive_location "COLD_STORAGE"
    :encryption_required true
    :access_restrictions ["COMPLIANCE_TEAM" "SENIOR_MANAGEMENT"]
  }
  :dependency_handling {
    :entity_references "UNLINK_PRESERVE_HISTORY"
    :document_references "ARCHIVE_WITH_CROSS_REFERENCE"
    :ubo_calculations "RETAIN_FOR_AUDIT"
    :orchestration_sessions "COMPLETE_PENDING_ARCHIVE_OTHERS"
    :product_workflows "TERMINATE_WITH_NOTIFICATION"
  }
  :confirmation_token @token{delete-confirm-cbu-100}
  :deletion_reason "Client relationship terminated"
  :authorized_by @user{senior-manager-001}
  :regulatory_notifications_required true)
```

#### Hard Delete with Cascade
```lisp
(cbu.delete
  :cbu_id @cbu{test-cbu-temporary}
  :deletion_strategy "HARD_DELETE"
  :cascade_operations {
    :entity_roles "DELETE_RELATIONS_ONLY"
    :attribute_values "DELETE_ALL"
    :ubo_registry "DELETE_ALL"
    :product_workflows "DELETE_ALL"
    :orchestration_sessions "DELETE_INCOMPLETE_ONLY"
    :document_references "REMOVE_REFERENCES_KEEP_DOCUMENTS"
  }
  :safety_checks {
    :require_empty_balances true
    :require_no_active_trades true
    :require_no_pending_settlements true
    :require_explicit_confirmation true
  }
  :confirmation_token @token{hard-delete-confirm-xyz789}
  :deletion_reason "Test environment cleanup")
```

## Implementation Architecture

### SQLX Multi-Table Transaction Pattern

```rust
pub struct CbuCrudManager {
    pool: PgPool,
    validation_engine: Arc<ValidationEngine>,
    audit_logger: Arc<AuditLogger>,
}

impl CbuCrudManager {
    /// Create CBU with full multi-table transaction
    pub async fn create_cbu_complex(
        &self,
        request: CbuCreateRequest,
    ) -> Result<CbuCreationResult> {
        let mut tx = self.pool.begin().await?;
        
        // 1. Validate request
        self.validation_engine.validate_cbu_create(&request).await?;
        
        // 2. Create main CBU record
        let cbu_id = self.create_cbu_record(&mut tx, &request).await?;
        
        // 3. Create entity relationships
        if !request.entities.is_empty() {
            self.create_cbu_entity_roles(&mut tx, cbu_id, &request.entities).await?;
        }
        
        // 4. Set attribute values
        if !request.attributes.is_empty() {
            self.set_cbu_attributes(&mut tx, cbu_id, &request.attributes).await?;
        }
        
        // 5. Initialize product workflows
        if !request.products.is_empty() {
            self.create_product_workflows(&mut tx, cbu_id, &request.products).await?;
        }
        
        // 6. Create orchestration session
        let session_id = self.create_orchestration_session(&mut tx, cbu_id, &request).await?;
        
        // 7. Initial UBO calculation if needed
        if request.auto_calculate_ubo {
            self.calculate_ubo_initial(&mut tx, cbu_id).await?;
        }
        
        // 8. Audit logging
        self.audit_logger.log_cbu_creation(&mut tx, cbu_id, &request).await?;
        
        tx.commit().await?;
        
        Ok(CbuCreationResult {
            cbu_id,
            session_id,
            created_entities: request.entities.len(),
            created_attributes: request.attributes.len(),
            initialized_products: request.products.len(),
        })
    }
}
```

### Error Handling and Validation

```rust
#[derive(Debug, thiserror::Error)]
pub enum CbuCrudError {
    #[error("CBU validation failed: {details}")]
    ValidationError { details: String },
    
    #[error("Entity relationship conflict: {entity_id} already has conflicting role {role}")]
    EntityRoleConflict { entity_id: Uuid, role: String },
    
    #[error("UBO calculation failed: {reason}")]
    UboCalculationError { reason: String },
    
    #[error("Regulatory constraint violation: {regulation} prevents {operation}")]
    RegulatoryConstraintViolation { regulation: String, operation: String },
    
    #[error("Dependency constraint: Cannot delete CBU {cbu_id} due to {constraint}")]
    DependencyConstraint { cbu_id: Uuid, constraint: String },
}
```

### Performance Optimization Strategies

1. **Bulk Operations**: Use `COPY` and batch inserts for large datasets
2. **Connection Pooling**: Maintain optimal pool size for concurrent operations
3. **Query Optimization**: Use prepared statements and query caching
4. **Parallel Processing**: Execute independent operations concurrently
5. **Audit Trail Optimization**: Asynchronous audit logging to prevent blocking

### Security and Compliance Considerations

1. **Data Encryption**: Encrypt sensitive attributes at rest and in transit
2. **Access Control**: Role-based access control for CBU operations
3. **Audit Logging**: Comprehensive audit trail for all CBU changes
4. **Regulatory Compliance**: Built-in checks for regulatory requirements
5. **Data Retention**: Configurable retention policies for archived CBUs

## Testing Strategy

### Unit Tests
- Individual SQLX query validation
- Transaction rollback scenarios
- Error condition handling

### Integration Tests
- End-to-end CBU lifecycle operations
- Multi-table consistency verification
- Performance benchmarking

### Compliance Tests
- Regulatory constraint validation
- UBO calculation accuracy
- Audit trail completeness

## Migration and Deployment

### Database Migration Scripts
1. Create new CBU CRUD stored procedures
2. Add missing indexes for performance
3. Create audit trigger functions
4. Set up regulatory constraint checks

### Backward Compatibility
- Maintain existing CBU APIs during transition
- Gradual migration of legacy CBU operations
- Comprehensive regression testing

---

This specification provides the foundation for implementing a comprehensive, production-ready CBU CRUD system using DSL-driven operations with proper multi-table transaction management, regulatory compliance, and performance optimization.