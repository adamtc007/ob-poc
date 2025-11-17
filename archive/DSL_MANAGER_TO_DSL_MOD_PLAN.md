# DSL Manager to DSL Mod Orchestration Implementation Plan

## Overview

This plan outlines the implementation of proper orchestration between `dsl_manager` (the central gateway) and the `dsl` mod (the processing engine), following the call chain pattern:

```
DSL_Manager (Entry Point) → DSL Mod (Processing) → Database/SQLx (Persistence) → Response
```

## Current State Assessment

### ✅ DSL Manager Functions (17 total - COMPREHENSIVE)

**DSL Creation/Generation (7 functions):**
- `process_agentic_crud_request` - AI-powered DSL generation ⭐ **PRIMARY**
- `process_ai_onboarding` - AI onboarding workflows
- `generate_canonical_kyc_case` - KYC DSL generation
- `generate_canonical_ubo_analysis` - UBO DSL generation
- `generate_domain_template` - Template-based generation
- `create_dsl_instance` - Create DSL instances
- `compile_dsl_template` - Template compilation

**DSL Processing/Execution (4 functions):**
- `process_operation` - **Main DSL processing entry point** ⭐ **CRITICAL**
- `execute_dsl` - Complete DSL processing pipeline ⭐ **CRITICAL**
- `execute_dsl_call_chain` - Multi-step operations
- `edit_dsl_instance` - Edit existing DSL

**DSL Validation (2 functions):**
- `validate_dsl_with_ai` - AI-powered validation
- `validate_agentic_dsl` - Validate AI-generated DSL

**DSL Management (4 functions):**
- `process_agentic_crud_batch` - Batch operations
- `get_metrics` - Performance metrics
- `comprehensive_health_check` - System health
- `get_active_operations_count` - Operation tracking

### ⚠️ DSL Mod Current Interface (NEEDS ORCHESTRATION INTERFACE)

**Current DSL Mod Structure:**
```rust
pub struct DslProcessor {
    editor: CentralDslEditor,
    registry: DomainRegistry,
    coordinator: ParsingCoordinator,
}

// Main method:
pub async fn process_dsl(&self, dsl_content: &str, context: DomainContext) -> DslResult<ProcessingResult>
```

**Issue**: DSL Mod is not properly receiving orchestrated calls from DSL Manager.

## Implementation Plan

### Phase 1: Create DSL Manager → DSL Mod Interface

**Goal**: Define clean interface for DSL Manager to orchestrate DSL Mod operations.

#### 1.1 Create DSL Orchestration Interface

**File**: `rust/src/dsl/orchestration_interface.rs`

```rust
/// Interface for DSL Manager to orchestrate DSL operations
#[async_trait]
pub trait DslOrchestrationInterface {
    /// Process DSL operation from DSL Manager
    async fn process_orchestrated_operation(
        &self,
        operation: OrchestrationOperation,
    ) -> DslResult<OrchestrationResult>;
    
    /// Validate DSL from DSL Manager
    async fn validate_orchestrated_dsl(
        &self,
        dsl_content: &str,
        context: OrchestrationContext,
    ) -> DslResult<ValidationReport>;
    
    /// Execute DSL from DSL Manager
    async fn execute_orchestrated_dsl(
        &self,
        dsl_content: &str,
        context: OrchestrationContext,
    ) -> DslResult<ExecutionResult>;
}
```

#### 1.2 Define Orchestration Types

```rust
/// Operation from DSL Manager to DSL Mod
#[derive(Debug, Clone)]
pub struct OrchestrationOperation {
    pub operation_id: String,
    pub operation_type: OrchestrationOperationType,
    pub dsl_content: String,
    pub context: OrchestrationContext,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum OrchestrationOperationType {
    Parse,
    Validate,
    Execute,
    Transform,
    Compile,
}

/// Context passed from DSL Manager
#[derive(Debug, Clone)]
pub struct OrchestrationContext {
    pub request_id: String,
    pub user_id: String,
    pub domain: String,
    pub processing_options: ProcessingOptions,
    pub audit_trail: Vec<String>,
}

/// Result back to DSL Manager
#[derive(Debug, Clone)]
pub struct OrchestrationResult {
    pub success: bool,
    pub operation_id: String,
    pub result_data: Option<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub processing_time_ms: u64,
}
```

#### 1.3 Implement Interface in DSL Mod

**Update**: `rust/src/dsl/mod.rs`

```rust
impl DslOrchestrationInterface for DslProcessor {
    async fn process_orchestrated_operation(
        &self,
        operation: OrchestrationOperation,
    ) -> DslResult<OrchestrationResult> {
        let start_time = std::time::Instant::now();
        
        match operation.operation_type {
            OrchestrationOperationType::Parse => {
                // Parse DSL using existing coordinator
                let parse_result = self.coordinator.parse_with_domains(&operation.dsl_content).await?;
                // Convert to OrchestrationResult
                Ok(OrchestrationResult { /* ... */ })
            }
            OrchestrationOperationType::Validate => {
                // Validate using existing validation
                // ...
            }
            OrchestrationOperationType::Execute => {
                // Execute using existing execution pipeline
                // ...
            }
            // ... other types
        }
    }
}
```

### Phase 2: Update DSL Manager to Use Orchestration

**Goal**: Modify DSL Manager functions to call DSL Mod through orchestration interface.

#### 2.1 Add DSL Processor to DSL Manager

**Update**: `rust/src/dsl_manager/core.rs`

```rust
pub struct DslManager {
    // ... existing fields
    dsl_processor: Arc<DslProcessor>,  // ADD THIS
}

impl DslManager {
    pub fn new(config: DslManagerConfig) -> Self {
        Self {
            // ... existing initialization
            dsl_processor: Arc::new(DslProcessor::new()),  // ADD THIS
        }
    }
}
```

#### 2.2 Update Key DSL Manager Functions

**Primary Functions to Update:**

1. **`execute_dsl`** - Route to DSL Mod:
```rust
pub async fn execute_dsl(&self, dsl_text: &str, context: DslContext) -> DslManagerResult<DslProcessingResult> {
    let orchestration_op = OrchestrationOperation {
        operation_id: Uuid::new_v4().to_string(),
        operation_type: OrchestrationOperationType::Execute,
        dsl_content: dsl_text.to_string(),
        context: self.convert_context(context),
        metadata: HashMap::new(),
    };
    
    let result = self.dsl_processor.process_orchestrated_operation(orchestration_op).await?;
    self.convert_orchestration_result(result)
}
```

2. **`process_agentic_crud_request`** - Route to DSL Mod after AI generation
3. **`validate_agentic_dsl`** - Route validation to DSL Mod
4. **`process_operation`** - Route all operations to DSL Mod

### Phase 3: Database Integration Through DSL Mod

**Goal**: Ensure DSL Mod can properly execute database operations.

#### 3.1 Database Connection in DSL Mod

**Update**: `rust/src/dsl/mod.rs`

```rust
impl DslProcessorl {
    pub fn with_database(pool: PgPool) -> Self {
        // Create DSL processor with database connectivity
        let database_service = Arc::new(DmictionaryDatabaseService::new(pool.clone()));k
        
        Self {
            editor: C entralDslEditor::new(
                Arc::new(DomainRegistry::new()),
                database_service,
                EditorConfig::default(),
            ),
            // ... rest of initialization
        }
    }
}
```

#### 3.2 Database Execution in Orchestration

```rust
async fn execute_orchestrated_dsl(&self, dsl_content: &str, context: OrchestrationContext) -> DslResult<ExecutionResult> {
    // 1. Parse DSL
    let ast = self.coordinator.parse_with_domains(dsl_content).await?;
    
    // 2. Validate DSL
    let validation = self.registry.validate_for_domain(&ast, &context.domain).await?;
    
    // 3. Execute against database (if configured)
    let execution_result = if let Some(backend) = &self.backend {
        backend.execute_dsl(&ast, &context).await?
    } else {
        ExecutionResult::mock_success()
    };
    
    Ok(execution_result)
}
```

### Phase 4: Testing Strategy

**Goal**: Ensure the orchestration works end-to-end.

#### 4.1 Create Integration Tests

**File**: `rust/tests/dsl_manager_orchestration_test.rs`

```rust
#[tokio::test]
async fn test_dsl_manager_to_dsl_mod_orchestration() {
    // Setup DSL Manager with DSL Mod
    let dsl_manager = DslManager::new(DslManagerConfig::default());
    
    // Test agentic CRUD request flows to DSL Mod
    let request = AgenticCrudRequest {
        instruction: "Create customer John Doe".to_string(),
        asset_type: Some("customer".to_string()),
        operation_type: Some("create".to_string()),
        execute_dsl: true,  // This should flow through to DSL Mod
        context_hints: vec![],
        metadata: HashMap::new(),
    };
    
    let context = DslContext { /* ... */ };
    
    // This should orchestrate: DSL Manager → DSL Mod → Database → Response
    let result = dsl_manager.process_agentic_crud_request(request, context).await;
    
    assert!(result.is_ok());
    // Verify DSL was generated, validated, and executed
}

#[tokio::test] 
async fn test_direct_dsl_execution_orchestration() {
    let dsl_manager = DslManager::new(DslManagerConfig::default());
    
    let dsl = "(case.create :customer-name \"Test Customer\")";
    let context = DslContext { /* ... */ };
    
    // This should orchestrate: DSL Manager → DSL Mod → Database
    let result = dsl_manager.execute_dsl(dsl, context).await;
    
    assert!(result.is_ok());
    // Verify orchestration worked
}
```

#### 4.2 Test Database Round-Trip

```rust
#[tokio::test]
#[cfg(feature = "database")]
async fn test_database_orchestration() {
    // Setup with real database
    let pool = setup_test_database().await;
    let dsl_manager = DslManager::new_with_database(DslManagerConfig::default(), pool);
    
    // Test complete round-trip
    let dsl = "(case.create :customer-name \"Database Test\")";
    let result = dsl_manager.execute_dsl(dsl, DslContext::default()).await;
    
    // Verify data was actually written to database
    assert!(result.is_ok());
    verify_database_state(&pool, "Database Test").await;
}
```

### Phase 5: Performance and Monitoring

**Goal**: Ensure orchestration is efficient and observable.

#### 5.1 Add Orchestration Metrics

```rust
#[derive(Debug, Clone)]
pub struct OrchestrationMetrics {
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub average_processing_time_ms: f64,
    pub dsl_manager_to_dsl_mod_latency_ms: f64,
}
```

#### 5.2 Add Tracing

```rust
#[tracing::instrument(skip(self, operation))]
async fn process_orchestrated_operation(&self, operation: OrchestrationOperation) -> DslResult<OrchestrationResult> {
    info!("Processing orchestrated operation: {}", operation.operation_id);
    
    let start = Instant::now();
    let result = self.internal_process(operation).await;
    let duration = start.elapsed();
    
    info!("Orchestration completed in {}ms", duration.as_millis());
    result
}
```

## Success Criteria

### ✅ Phase 1 Complete When:
- [ ] DSL Orchestration Interface defined
- [ ] OrchestrationOperation, OrchestrationContext, OrchestrationResult types created
- [ ] DslProcessor implements DslOrchestrationInterface

### ✅ Phase 2 Complete When:
- [ ] DSL Manager has reference to DslProcessor
- [ ] Key functions (`execute_dsl`, `process_agentic_crud_request`) route to DSL Mod
- [ ] Context conversion between DSL Manager and DSL Mod works

### ✅ Phase 3 Complete When:
- [ ] DSL Mod can connect to database through DictionaryDatabaseService
- [ ] DSL execution results in actual database operations
- [ ] Round-trip: Natural Language → AI → DSL → Database → Response works

### ✅ Phase 4 Complete When:
- [ ] Integration tests pass
- [ ] End-to-end agentic CRUD tests work
- [ ] Database round-trip tests pass

### ✅ Phase 5 Complete When:
- [ ] Orchestration metrics collected
- [ ] Tracing provides visibility
- [ ] Performance is acceptable

## Implementation Order

1. **Start with Phase 1** - Create the interface (no breaking changes)
2. **Implement Phase 2** - Wire up DSL Manager (core orchestration)
3. **Add Phase 3** - Database integration (real functionality)
4. **Build Phase 4** - Testing (validation)
5. **Polish Phase 5** - Monitoring (production readiness)

## Risk Mitigation

### Import Issues
- Fix import mismatches incrementally as we encounter them
- Use feature flags to isolate database-dependent code
- Create mock implementations for missing dependencies

### Complexity Management
- Keep orchestration interface simple initially
- Add complexity only when needed
- Maintain backward compatibility where possible

### Testing Strategy
- Test each phase independently
- Build up integration tests incrementally
- Use both unit and integration tests

This plan ensures a systematic approach to implementing proper orchestration between DSL Manager and DSL Mod, following the call chain pattern back to the database and returning with responses.