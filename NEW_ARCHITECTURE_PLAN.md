# New Architecture Plan: End-to-End DSL Processing Pipeline

## Overview

This plan outlines the complete redesign of the OB-POC system with a clean, linear architecture flow that eliminates legacy complexity and establishes a modern end-to-end processing pipeline.

## Architecture Flow

```
DSL_Manager → DSL_Mod → DB_State_Manager → DSL_Visualizer
(Gateway)    (Engine)   (Persistence)     (Output)
```

## Phase 1: Legacy Cleanup ✅ COMPLETE

### Remove Legacy Tests
- [x] Delete outdated test files
- [x] Remove broken integration tests
- [x] Clear legacy example files
- [x] Clean up obsolete test utilities

### Files to Remove:
```bash
# Legacy tests
rust/tests/agentic_crud_test.rs
rust/tests/agentic_minimal_test.rs
rust/examples/gemini_agent_test.rs

# Broken integration tests
rust/src/ai/tests/end_to_end_agent_tests.rs (if broken)

# Legacy example files that don't follow new pattern
```

## Phase 2: DSL Manager as Entry Point

### 2.1 DSL Manager Architecture
**Goal**: Establish DSL Manager as the single gateway for all DSL operations

#### Core Responsibilities:
- **Request Reception**: All DSL operations enter through DSL Manager
- **Context Creation**: Convert requests to OrchestrationContext
- **Orchestration**: Route operations to DSL Mod via orchestration interface
- **Result Aggregation**: Collect and format responses from downstream components
- **Error Handling**: Centralized error management and recovery

#### Key Functions to Implement:
```rust
impl DslManager {
    // Main entry points
    pub async fn process_onboarding_request(&self, request: OnboardingRequest) -> DslManagerResult<OnboardingResult>
    pub async fn process_incremental_dsl(&self, dsl: String, context: DslContext) -> DslManagerResult<ProcessingResult>
    pub async fn process_agentic_request(&self, request: AgenticRequest) -> DslManagerResult<AgenticResult>
    
    // Orchestration to DSL Mod
    async fn orchestrate_to_dsl_mod(&self, operation: OrchestrationOperation) -> DslManagerResult<OrchestrationResult>
    
    // Result processing
    async fn process_dsl_results(&self, results: Vec<OrchestrationResult>) -> DslManagerResult<AggregatedResult>
}
```

### 2.2 Request Types
Define clean request/response types for all DSL operations:

```rust
// Base onboarding request
#[derive(Debug, Clone)]
pub struct OnboardingRequest {
    pub client_name: String,
    pub jurisdiction: String,
    pub entity_type: String,
    pub services: Vec<String>,
    pub instruction: Option<String>,
    pub cbu_id: Option<String>,
}

// Incremental DSL request
#[derive(Debug, Clone)]
pub struct IncrementalDslRequest {
    pub base_case_id: String,
    pub additional_dsl: String,
    pub domain: String,
    pub operation_type: String,
}

// Agentic request for AI-powered operations
#[derive(Debug, Clone)]
pub struct AgenticRequest {
    pub instruction: String,
    pub context: HashMap<String, String>,
    pub execute_immediately: bool,
    pub target_domain: Option<String>,
}
```

## Phase 3: DSL Mod as Processing Engine

### 3.1 DSL Mod Enhancement
**Goal**: Transform DSL Mod into a robust processing engine that receives orchestrated operations

#### Processing Pipeline Enhancement:
```rust
impl DslProcessor {
    // Enhanced 4-step pipeline
    async fn execute_full_pipeline(&self, operation: OrchestrationOperation) -> DslResult<ProcessingResult> {
        // Step 1: DSL Change Validation
        let validated_input = self.validate_dsl_input(&operation).await?;
        
        // Step 2: AST Parse/Validate
        let parsed_ast = self.parse_and_validate_dsl(&validated_input).await?;
        
        // Step 3: DSL Domain Snapshot (prepare for DB)
        let domain_snapshot = self.create_domain_snapshot(&parsed_ast, &operation.context).await?;
        
        // Step 4: AST Dual Commit (to DB State Manager)
        let commit_result = self.commit_to_state_manager(domain_snapshot).await?;
        
        Ok(ProcessingResult::from_commit(commit_result))
    }
}
```

### 3.2 Domain-Specific Processing
Enhance domain handlers for specific business logic:

```rust
// Onboarding domain handler
impl OnboardingDomainHandler {
    async fn process_case_create(&self, ast: &Program) -> DslResult<OnboardingSnapshot>
    async fn process_case_update(&self, ast: &Program) -> DslResult<OnboardingSnapshot>
    async fn trigger_lifecycle_workflows(&self, snapshot: &OnboardingSnapshot) -> DslResult<Vec<WorkflowTrigger>>
}

// KYC domain handler
impl KycDomainHandler {
    async fn process_kyc_collection(&self, ast: &Program) -> DslResult<KycSnapshot>
    async fn validate_compliance_rules(&self, snapshot: &KycSnapshot) -> DslResult<ComplianceReport>
}

// UBO domain handler
impl UboDomainHandler {
    async fn process_ownership_data(&self, ast: &Program) -> DslResult<UboSnapshot>
    async fn calculate_beneficial_ownership(&self, snapshot: &UboSnapshot) -> DslResult<OwnershipCalculation>
}
```

## Phase 4: DB State Manager (NEW COMPONENT)

### 4.1 State Manager Architecture
**Goal**: Create a dedicated component for all database operations and state management

#### Core Responsibilities:
- **State Persistence**: Save DSL snapshots and AST to database
- **State Retrieval**: Load accumulated DSL state for incremental operations
- **Transaction Management**: Handle database transactions and rollbacks
- **State History**: Maintain complete audit trail of DSL evolution
- **State Queries**: Provide query interface for downstream components

#### Implementation:
```rust
#[derive(Debug)]
pub struct DbStateManager {
    pool: PgPool,
    transaction_manager: Arc<TransactionManager>,
    snapshot_store: Arc<SnapshotStore>,
    audit_logger: Arc<AuditLogger>,
}

impl DbStateManager {
    // State persistence
    pub async fn save_domain_snapshot(&self, snapshot: DomainSnapshot) -> StateResult<SnapshotId>
    pub async fn save_ast_dual_commit(&self, ast: Program, snapshot_id: SnapshotId) -> StateResult<CommitId>
    
    // State retrieval
    pub async fn load_accumulated_state(&self, case_id: &str) -> StateResult<AccumulatedState>
    pub async fn load_domain_history(&self, domain: &str, entity_id: &str) -> StateResult<DomainHistory>
    
    // Transaction management
    pub async fn begin_transaction(&self) -> StateResult<TransactionHandle>
    pub async fn commit_transaction(&self, handle: TransactionHandle) -> StateResult<()>
    pub async fn rollback_transaction(&self, handle: TransactionHandle) -> StateResult<()>
    
    // State queries
    pub async fn query_state_for_visualization(&self, query: StateQuery) -> StateResult<VisualizationData>
}
```

### 4.2 Database Schema Evolution
Enhance database schema for state management:

```sql
-- Core state management tables
CREATE TABLE "ob-poc".dsl_state_snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id VARCHAR(255) NOT NULL,
    domain VARCHAR(100) NOT NULL,
    snapshot_data JSONB NOT NULL,
    ast_data JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    operation_id VARCHAR(255) NOT NULL,
    parent_snapshot_id UUID REFERENCES "ob-poc".dsl_state_snapshots(snapshot_id)
);

-- Accumulated state tracking
CREATE TABLE "ob-poc".dsl_accumulated_state (
    case_id VARCHAR(255) PRIMARY KEY,
    current_dsl TEXT NOT NULL,
    current_ast JSONB NOT NULL,
    last_snapshot_id UUID NOT NULL REFERENCES "ob-poc".dsl_state_snapshots(snapshot_id),
    version_number INTEGER NOT NULL DEFAULT 1,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- State transition audit
CREATE TABLE "ob-poc".dsl_state_transitions (
    transition_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_snapshot_id UUID REFERENCES "ob-poc".dsl_state_snapshots(snapshot_id),
    to_snapshot_id UUID NOT NULL REFERENCES "ob-poc".dsl_state_snapshots(snapshot_id),
    transition_type VARCHAR(100) NOT NULL,
    triggered_workflows JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

## Phase 5: DSL Visualizer (NEW COMPONENT)

### 5.1 Visualizer Architecture
**Goal**: Create a powerful visualization component for DSL state and processing results

#### Core Responsibilities:
- **State Visualization**: Visual representation of accumulated DSL state
- **Process Flow Visualization**: Show DSL processing pipeline steps
- **Domain Relationship Mapping**: Visualize cross-domain relationships
- **Audit Trail Visualization**: Interactive timeline of DSL evolution
- **Real-time Updates**: Live visualization of processing operations

#### Implementation:
```rust
#[derive(Debug)]
pub struct DslVisualizer {
    state_manager: Arc<DbStateManager>,
    renderer: Arc<VisualizationRenderer>,
    layout_engine: Arc<LayoutEngine>,
    export_manager: Arc<ExportManager>,
}

impl DslVisualizer {
    // State visualization
    pub async fn visualize_accumulated_state(&self, case_id: &str) -> VisualizationResult<StateVisualization>
    pub async fn visualize_domain_relationships(&self, case_id: &str) -> VisualizationResult<RelationshipGraph>
    
    // Process visualization
    pub async fn visualize_processing_pipeline(&self, operation_id: &str) -> VisualizationResult<PipelineVisualization>
    pub async fn visualize_audit_trail(&self, case_id: &str) -> VisualizationResult<AuditTimeline>
    
    // Real-time visualization
    pub async fn create_live_visualization(&self, case_id: &str) -> VisualizationResult<LiveVisualizationHandle>
    
    // Export capabilities
    pub async fn export_visualization(&self, viz: &StateVisualization, format: ExportFormat) -> VisualizationResult<ExportedFile>
}
```

### 5.2 Visualization Types

#### State Visualizations:
- **DSL Tree View**: Hierarchical view of accumulated DSL
- **Domain Map**: Visual representation of multi-domain operations
- **Relationship Graph**: Entity relationships and ownership structures
- **Timeline View**: Historical evolution of DSL state

#### Process Visualizations:
- **Pipeline Flow**: 4-step processing pipeline with status
- **Operation Chain**: Sequence of incremental DSL operations
- **Error Flow**: Error propagation and handling visualization
- **Performance Metrics**: Processing time and resource usage

## Phase 6: End-to-End Integration

### 6.1 Complete Flow Implementation
**Goal**: Integrate all components into a seamless end-to-end processing pipeline

#### Flow Sequence:
1. **DSL Manager** receives request
2. **DSL Manager** creates OrchestrationOperation
3. **DSL Mod** processes via 4-step pipeline
4. **DSL Mod** sends domain snapshot to DB State Manager
5. **DB State Manager** persists state and returns commit confirmation
6. **DSL Manager** requests visualization from DSL Visualizer
7. **DSL Visualizer** queries DB State Manager for current state
8. **DSL Visualizer** returns visualization to DSL Manager
9. **DSL Manager** returns complete response with processing results and visualizations

### 6.2 Integration Points
```rust
// DSL Manager → DSL Mod
async fn orchestrate_to_dsl_mod(&self, operation: OrchestrationOperation) -> DslManagerResult<OrchestrationResult>

// DSL Mod → DB State Manager  
async fn commit_to_state_manager(&self, snapshot: DomainSnapshot) -> DslResult<CommitResult>

// DB State Manager → DSL Visualizer
async fn query_state_for_visualization(&self, query: StateQuery) -> StateResult<VisualizationData>

// DSL Visualizer → DSL Manager (via result aggregation)
async fn aggregate_results_with_visualization(&self, results: ProcessingResults, viz: VisualizationData) -> DslManagerResult<CompleteResponse>
```

## Phase 7: New Test Architecture

### 7.1 End-to-End Testing
Create comprehensive tests that follow the complete architecture flow:

```rust
// Integration tests for complete flow
#[tokio::test]
async fn test_onboarding_complete_flow() {
    // Setup: DSL Manager → DSL Mod → DB State Manager → DSL Visualizer
    let system = setup_complete_system().await;
    
    // Execute: Onboarding request through complete pipeline
    let request = OnboardingRequest::new("TechCorp Ltd", "GB", "CORP");
    let result = system.dsl_manager.process_onboarding_request(request).await;
    
    // Verify: All components worked correctly
    assert_complete_flow_success(&result);
    assert_database_state_correct(&system.db_state_manager).await;
    assert_visualization_generated(&result.visualization).await;
}

#[tokio::test]
async fn test_incremental_dsl_flow() {
    // Test incremental DSL accumulation through complete pipeline
    let system = setup_complete_system().await;
    
    // Base DSL
    let base_request = create_base_onboarding_request();
    let base_result = system.dsl_manager.process_onboarding_request(base_request).await;
    
    // Incremental addition
    let incremental_request = IncrementalDslRequest::new(
        base_result.case_id.clone(),
        "(kyc.collect :case-id \"{}\" :collection-type \"ENHANCED\")",
        "kyc"
    );
    let incremental_result = system.dsl_manager.process_incremental_dsl(incremental_request).await;
    
    // Verify accumulated state
    assert_incremental_accumulation_correct(&incremental_result);
}

#[tokio::test] 
async fn test_agentic_workflow_complete_flow() {
    // Test AI-powered DSL generation through complete pipeline
    let system = setup_complete_system_with_ai().await;
    
    let agentic_request = AgenticRequest::new(
        "Create comprehensive onboarding for UK technology company needing custody services"
    );
    
    let result = system.dsl_manager.process_agentic_request(agentic_request).await;
    
    // Verify AI → DSL → Processing → DB → Visualization flow
    assert_ai_generated_dsl_valid(&result.generated_dsl);
    assert_processing_successful(&result.processing_result);
    assert_state_persisted_correctly(&system.db_state_manager, &result.case_id).await;
    assert_visualization_reflects_ai_operation(&result.visualization);
}
```

### 7.2 Component-Level Testing
Individual component tests for isolation and reliability:

```rust
// DSL Manager tests
mod dsl_manager_tests {
    #[tokio::test]
    async fn test_orchestration_context_creation() { }
    
    #[tokio::test]
    async fn test_result_aggregation() { }
}

// DSL Mod tests  
mod dsl_mod_tests {
    #[tokio::test]
    async fn test_4_step_pipeline() { }
    
    #[tokio::test]
    async fn test_domain_snapshot_creation() { }
}

// DB State Manager tests
mod db_state_manager_tests {
    #[tokio::test]
    async fn test_state_persistence() { }
    
    #[tokio::test]
    async fn test_transaction_management() { }
}

// DSL Visualizer tests
mod dsl_visualizer_tests {
    #[tokio::test]
    async fn test_state_visualization() { }
    
    #[tokio::test]
    async fn test_real_time_updates() { }
}
```

## Implementation Timeline

### Week 1: Foundation
- [x] Phase 1: Legacy cleanup (COMPLETE)
- [x] Orchestration interface implementation (COMPLETE)
- [ ] DSL Manager entry point refactoring

### Week 2: Core Pipeline  
- [ ] DSL Mod processing engine enhancement
- [ ] DB State Manager implementation
- [ ] Database schema updates

### Week 3: Visualization & Integration
- [ ] DSL Visualizer implementation
- [ ] End-to-end integration
- [ ] Initial testing framework

### Week 4: Testing & Polish
- [ ] Comprehensive test suite
- [ ] Performance optimization
- [ ] Documentation and examples

## Success Criteria

### ✅ Architecture Quality
- Clean linear flow with clear component boundaries
- No circular dependencies or legacy coupling
- Comprehensive error handling and recovery
- Full audit trail and observability

### ✅ Functionality 
- Complete onboarding lifecycle support
- Incremental DSL accumulation working
- AI-powered DSL generation integrated
- Real-time visualization capabilities

### ✅ Performance
- Sub-second response times for standard operations
- Efficient database operations with proper indexing
- Scalable architecture supporting concurrent operations
- Memory-efficient processing with minimal allocations

### ✅ Maintainability
- Comprehensive test coverage (>90%)
- Clear documentation and examples
- Modular architecture enabling independent development
- Production-ready error handling and monitoring

## Risk Mitigation

### Technical Risks
- **Database Performance**: Implement proper indexing and query optimization
- **Component Integration**: Extensive integration testing at each phase
- **Memory Usage**: Profile and optimize memory allocation patterns
- **Error Propagation**: Comprehensive error handling across all boundaries

### Development Risks
- **Complexity Management**: Keep each component focused on single responsibility
- **Integration Challenges**: Build and test incrementally 
- **Performance Bottlenecks**: Monitor performance metrics throughout development
- **API Stability**: Define stable interfaces early and maintain backward compatibility

---

**Status**: Ready for implementation
**Next Action**: Begin Phase 2 - DSL Manager refactoring as entry point
**Architecture**: Modern, clean, and production-ready end-to-end pipeline