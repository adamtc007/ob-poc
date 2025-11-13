# DSL Manager to DSL Mod Orchestration Implementation Plan

## Overview

This plan outlines the implementation of proper orchestration between `dsl_manager` (the central gateway) and the `dsl` mod (the processing engine), following the call chain pattern:

```
DSL_Manager (Entry Point) → [Template|Agent] Generation → DSL Mod (Processing) → Database/SQLx (Persistence) → Response
```

## Current State Assessment

### ✅ DSL Manager Functions (17 total - COMPREHENSIVE)

**DSL Creation/Generation (7 functions):**
- `process_agentic_crud_request` - AI-powered DSL generation ⭐ **PRIMARY**
- `process_ai_onboarding` - AI onboarding workflows
- `generate_canonical_kyc_case` - KYC DSL generation
- `generate_canonical_ubo_analysis` - UBO DSL generation
- `generate_domain_template` - Template-based generation ⭐ **DIRECT METHOD**
- `create_dsl_instance` - Create DSL instances (supports both Template/Agent)
- `compile_dsl_template` - Template compilation ⭐ **AGENT METHOD**

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

### ⚠️ bacon

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

### Phase 1.5: DSL Generation Method Substitution (Template vs Agent)

**Goal**: Enable DSL Manager factory methods to generate DSL using either Template (Direct) or Agent (AI/RAG) methods before orchestrating to DSL Mod.

#### 1.5.1 Create Generation Domain Library

**File**: `rust/src/generation/mod.rs`

```rust
//! DSL Generation Domain Library
//!
//! Provides pluggable DSL generation capabilities for DSL Manager.
//! Supports Template-based (Direct) and AI-based (Agent) generation methods.

pub mod traits;
pub mod template;
pub mod ai;
pub mod context;

// Public re-exports
pub use traits::{DslGenerator, GenerationMethod};
pub use template::TemplateGenerator;
pub use ai::AiGenerator;
pub use context::{GenerationContext, GenerationResult, OperationType};

/// Generation method selector
#[derive(Debug, Clone)]
pub enum GenerationMethod {
    /// Use hardcoded templates (Direct method)
    Direct,
    /// Use AI agents with RAG/prompt (Agent method)
    Agent(String), // instruction for the agent
}

/// Generation factory
pub struct GenerationFactory;
impl GenerationFactory {
    pub fn create_template_generator() -> Box<dyn DslGenerator> {
        Box::new(TemplateGenerator::new())
    }
    
    pub fn create_ai_generator(ai_service: RealAiEntityService) -> Box<dyn DslGenerator> {
        Box::new(AiGenerator::new(ai_service))
    }
}
```

#### 1.5.2 Define Generation Interface

**File**: `rust/src/generation/traits.rs`

```rust
use async_trait::async_trait;
use crate::generation::{GenerationContext, GenerationResult};

/// Core trait for DSL generation strategies
#[async_trait]
pub trait DslGenerator: Send + Sync {
    /// Generate DSL content based on context
    async fn generate_dsl(&self, context: &GenerationContext) -> GenerationResult<String>;
    
    /// Validate that this generator can handle the operation
    fn can_handle(&self, operation: &str) -> bool;
    
    /// Get generator metadata
    fn metadata(&self) -> GeneratorMetadata;
}
```

#### 1.5.3 Template Generator (Direct Method)

**File**: `rust/src/generation/template.rs`

```rust
/// Template-based DSL generator - preserves current functionality
pub struct TemplateGenerator {
    templates: HashMap<String, String>,
}

#[async_trait]
impl DslGenerator for TemplateGenerator {
    async fn generate_dsl(&self, context: &GenerationContext) -> GenerationResult<String> {
        match &context.operation {
            OperationType::CbuCreate { name, description } => {
                // Use existing template logic from CleanDslManager
                Ok(format!(
                    r#"(case.create :name "CBU Creation - {}" :type "cbu_onboarding")
                       (cbu.create :name "{}" :description "{}" :status "ACTIVE")"#,
                    name, name, description.as_deref().unwrap_or("")
                ))
            }
            OperationType::EntityRegister { entity_id, name, entity_type } => {
                // Use existing entity template logic
                Ok(format!(
                    r#"(entity.register :entity-id "{}" :name "{}" :type "{}")
                       (identity.verify :entity-id "{}" :level "STANDARD")"#,
                    entity_id, name, entity_type, entity_id
                ))
            }
            // Add other operations...
        }
    }
}
```

#### 1.5.4 AI Generator (Agent Method)

**File**: `rust/src/generation/ai.rs`

```rust
/// AI-based DSL generator with RAG for template equivalence
pub struct AiGenerator {
    ai_service: RealAiEntityService,
}

#[async_trait]
impl DslGenerator for AiGenerator {
    async fn generate_dsl(&self, context: &GenerationContext) -> GenerationResult<String> {
        // Build RAG prompt including template patterns for equivalence
        let prompt = self.build_rag_prompt(context)?;
        
        // Call AI service with structured prompt
        match self.ai_service.generate_dsl_from_prompt(prompt).await {
            Ok(dsl) => Ok(dsl),
            Err(e) => Err(GenerationError::AiGenerationFailed {
                reason: e.to_string(),
                context: context.clone(),
            })
        }
    }
    
    /// Build RAG prompt that includes template patterns for equivalence
    fn build_rag_prompt(&self, context: &GenerationContext) -> GenerationResult<String> {
        let template_pattern = self.get_template_pattern(&context.operation);
        
        Ok(format!(
            r#"You are a DSL generator. Follow this EXACT pattern:

TEMPLATE PATTERN:
{template_pattern}

USER INSTRUCTION: {instruction}
OPERATION: {operation:?}

RULES:
- Follow the template structure exactly
- Replace variables with provided values
- Maintain same verbs and properties
- Keep formatting consistent

GENERATE DSL:"#,
            template_pattern = template_pattern,
            instruction = context.instruction.as_deref().unwrap_or(""),
            operation = context.operation
        ))
    }
}
```

#### 1.5.5 Update DSL Manager Factory Methods

**Update**: `rust/src/dsl_manager/clean_manager.rs`

```rust
pub struct CleanDslManager {
    // ... existing fields ...
    /// DSL generator for pluggable generation strategies
    dsl_generator: Box<dyn DslGenerator>,
}

impl CleanDslManager {
    /// Create with template generator (Direct method)
    pub fn new() -> Self {
        Self {
            // ... existing initialization ...
            dsl_generator: GenerationFactory::create_template_generator(),
        }
    }
    
    /// Create with AI generator (Agent method)
    pub fn with_ai_generator(ai_service: RealAiEntityService) -> Self {
        Self {
            // ... existing initialization ...
            dsl_generator: GenerationFactory::create_ai_generator(ai_service),
        }
    }
    
    /// Switch generation method at runtime
    pub fn set_generation_method(&mut self, method: GenerationMethod) {
        self.dsl_generator = match method {
            GenerationMethod::Direct => GenerationFactory::create_template_generator(),
            GenerationMethod::Agent(ai_service) => GenerationFactory::create_ai_generator(ai_service),
        };
    }
}

/// Updated factory method - supports both Template and Agent generation
pub async fn create_cbu_dsl(
    &mut self,
    method: Option<GenerationMethod>, // Optional override
    onboarding_request_id: Uuid,
    cbu_name: &str,
    description: Option<&str>,
    user_id: &str,
) -> Result<CallChainResult, DslManagerError> {
    let case_id = format!("cbu-{}", Uuid::new_v4());

    // Build generation context
    let context = GenerationContext {
        operation: OperationType::CbuCreate {
            name: cbu_name.to_string(),
            description: description.map(|s| s.to_string()),
        },
        instruction: match method {
            Some(GenerationMethod::Agent(instr)) => Some(instr),
            _ => None,
        },
        metadata: GenerationMetadata::default(),
    };

    // Generate DSL using current generator (Template or Agent)
    let cbu_dsl = self.dsl_generator.generate_dsl(&context).await
        .map_err(|e| DslManagerError::ProcessingError { 
            message: format!("DSL generation failed: {}", e) 
        })?;

    // Continue with existing orchestration to DSL Mod
    self.save_and_execute_dsl(case_id, onboarding_request_id, cbu_dsl, user_id, "cbu_create").await
}
```

#### 1.5.6 Equivalence Testing

**Goal**: Ensure Template and Agent methods produce equivalent DSL.

```rust
#[cfg(test)]
mod equivalence_tests {
    #[tokio::test]
    async fn test_template_agent_equivalence() {
        let template_gen = TemplateGenerator::new();
        let ai_gen = AiGenerator::new(mock_ai_service());
        
        let context = GenerationContext {
            operation: OperationType::CbuCreate {
                name: "TechCorp Ltd".to_string(),
                description: Some("UK tech company".to_string()),
            },
            instruction: Some("Create CBU for TechCorp Ltd".to_string()),
            metadata: Default::default(),
        };
        
        let template_dsl = template_gen.generate_dsl(&context).await.unwrap();
        let agent_dsl = ai_gen.generate_dsl(&context).await.unwrap();
        
        // Should produce semantically equivalent DSL
        assert!(assert_equivalent_dsl(&template_dsl, &agent_dsl));
    }
}

pub fn assert_equivalent_dsl(template_dsl: &str, agent_dsl: &str) -> bool {
    let template_ast = parse_dsl(template_dsl).expect("Template DSL should be valid");
    let agent_ast = parse_dsl(agent_dsl).expect("Agent DSL should be valid");
    
    // Compare semantic equivalence (not string equality)
    semantic_equivalent(&template_ast, &agent_ast)
}
```

### Phase 2: Update DSL Manager to Use Orchestration

**Goal**: Modify DSL Manager functions to call DSL Mod through orchestration interface (after DSL generation).

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

2. **`process_agentic_crud_request`** - Route to DSL Mod after Agent generation
3. **`validate_agentic_dsl`** - Route validation to DSL Mod  
4. **`process_operation`** - Route all operations to DSL Mod
5. **All factory methods** - Use generated DSL (Template or Agent) → orchestrate to DSL Mod

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
    
    // This should orchestrate: DSL Manager → [Agent Generation] → DSL Mod → Database → Response
    let result = dsl_manager.process_agentic_crud_request(request, context).await;
    
    assert!(result.is_ok());
    // Verify DSL was generated, validated, and executed
}

#[tokio::test]
async fn test_template_vs_agent_generation_orchestration() {
    let mut template_manager = DslManager::new(DslManagerConfig::default()); // Uses Template
    let mut agent_manager = DslManager::with_ai_generator(mock_ai_service()); // Uses Agent
    
    let params = (Uuid::new_v4(), "TechCorp Ltd", Some("UK tech company"), "user123");
    
    // Both should produce equivalent results through orchestration
    let template_result = template_manager.create_cbu_dsl(
        None, // Use default (Template)
        params.0, params.1, params.2, params.3
    ).await.unwrap();
    
    let agent_result = agent_manager.create_cbu_dsl(
        Some(GenerationMethod::Agent("Create CBU for TechCorp Ltd".to_string())),
        params.0, params.1, params.2, params.3
    ).await.unwrap();
    
    // Should have equivalent outcomes
    assert_eq!(template_result.success, agent_result.success);
    assert_equivalent_dsl(&template_result.generated_dsl, &agent_result.generated_dsl);
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

### ✅ Phase 1.5 Complete When:
- [ ] Generation Domain Library created (`generation/` module)
- [ ] DslGenerator trait defined with Template/Agent implementations
- [ ] TemplateGenerator preserves existing template functionality
- [ ] AiGenerator implements RAG-based DSL generation
- [ ] DSL Manager factory methods updated to use pluggable generators
- [ ] Equivalence tests pass (Template and Agent produce equivalent DSL)

### ✅ Phase 2 Complete When:
- [ ] DSL Manager has reference to DslProcessor
- [ ] Key functions (`execute_dsl`, `process_agentic_crud_request`) route generated DSL to DSL Mod
- [ ] Factory methods integrate: Generation → Orchestration → DSL Mod
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

1. **Start with Phase 1** - Create the orchestration interface (no breaking changes)
2. **Implement Phase 1.5** - Add generation method substitution (Template/Agent)
3. **Implement Phase 2** - Wire up DSL Manager with generation → orchestration
4. **Add Phase 3** - Database integration (real functionality)
5. **Build Phase 4** - Testing (validation)
6. **Polish Phase 5** - Monitoring (production readiness)

**Key Insight**: Phase 1.5 enables the same DSL Manager API to work with either Template (Direct) or Agent (AI/RAG) generation methods, with both feeding into the same orchestration pipeline to DSL Mod.

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

This plan ensures a systematic approach to implementing:
1. **Pluggable DSL Generation** (Template vs Agent methods)
2. **Proper Orchestration** between DSL Manager and DSL Mod
3. **Call Chain Pattern** from generation → orchestration → database → responses

The generation method substitution allows the same DSL Manager interface to work with either hardcoded templates (Direct) or AI-powered generation (Agent), with both methods producing equivalent DSL that flows through the same orchestration pipeline.