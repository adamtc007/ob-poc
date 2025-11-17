# Clean Architecture Plan: DSL CRUD Operations + Optional AI Layer

## Overview

This plan establishes a clean separation between core DSL CRUD operations and the optional AI agent layer. The DSL system operates independently, with AI as an optional frontend that generates DSL content.

## Architecture Separation

```
┌─────────────────┐    ┌──────────────────────────────────────────────────────────┐
│   AI Agent      │    │                Core DSL System                           │
│   (Optional)    │───▶│  DSL_Manager → DSL_Mod → DB_State_Manager → DSL_Visualizer │
│                 │    │                                                          │
└─────────────────┘    └──────────────────────────────────────────────────────────┘
     │                                            ▲
     │                                            │
     └─────── Direct DSL Input ───────────────────┘
```

## Core Principle: DSL-First Design

### ✅ DSL CRUD Operations (Core System)
- **Direct DSL Processing**: Accept raw DSL content directly
- **No AI Dependencies**: Core system works without any AI components
- **Pure DSL Logic**: Focus on parsing, validation, execution, and persistence
- **Deterministic Operations**: Predictable, testable, reliable

### ✅ AI Agent Layer (Optional Frontend)
- **DSL Generation**: Convert natural language to DSL content
- **Enhancement Layer**: Optional layer that feeds into core DSL system
- **Pluggable Design**: Can be enabled/disabled without affecting core operations
- **Multiple Providers**: Support different AI providers (OpenAI, Gemini, etc.)

## Phase 1: Core DSL CRUD System

### 1.1 DSL Manager (Pure DSL Gateway)
**Role**: Process DSL operations without AI dependency

```rust
#[derive(Debug)]
pub struct DslManager {
    dsl_processor: Arc<DslProcessor>,
    state_manager: Arc<DbStateManager>,
    visualizer: Arc<DslVisualizer>,
    config: DslManagerConfig,
}

impl DslManager {
    // === CORE DSL CRUD OPERATIONS ===
    
    /// Create new onboarding case from DSL
    pub async fn create_case(&self, dsl_content: String, context: DslContext) -> DslResult<CaseCreationResult> {
        // Direct DSL processing - no AI involved
        let operation = OrchestrationOperation::new(
            OrchestrationOperationType::Execute,
            dsl_content,
            context.into(),
        );
        
        self.process_dsl_operation(operation).await
    }
    
    /// Update existing case with additional DSL
    pub async fn update_case(&self, case_id: String, additional_dsl: String) -> DslResult<CaseUpdateResult> {
        // Load existing accumulated DSL
        let existing_state = self.state_manager.load_accumulated_state(&case_id).await?;
        
        // Append new DSL to existing state
        let accumulated_dsl = format!("{}\n\n{}", existing_state.current_dsl, additional_dsl);
        
        // Process accumulated DSL
        let operation = OrchestrationOperation::new(
            OrchestrationOperationType::Execute,
            accumulated_dsl,
            existing_state.context,
        );
        
        self.process_dsl_operation(operation).await
    }
    
    /// Validate DSL content without executing
    pub async fn validate_dsl(&self, dsl_content: String, context: DslContext) -> DslResult<ValidationResult> {
        let operation = OrchestrationOperation::new(
            OrchestrationOperationType::Validate,
            dsl_content,
            context.into(),
        );
        
        self.process_validation_operation(operation).await
    }
    
    /// Execute batch of DSL operations
    pub async fn execute_dsl_batch(&self, operations: Vec<DslBatchOperation>) -> DslResult<BatchExecutionResult> {
        let mut results = Vec::new();
        
        for batch_op in operations {
            let operation = OrchestrationOperation::new(
                OrchestrationOperationType::Execute,
                batch_op.dsl_content,
                batch_op.context.into(),
            );
            
            let result = self.process_dsl_operation(operation).await?;
            results.push(result);
        }
        
        Ok(BatchExecutionResult { results })
    }
    
    // === INTERNAL PROCESSING ===
    async fn process_dsl_operation(&self, operation: OrchestrationOperation) -> DslResult<CaseOperationResult>
    async fn process_validation_operation(&self, operation: OrchestrationOperation) -> DslResult<ValidationResult>
}
```

### 1.2 Core DSL Request Types
Pure DSL operations without AI concepts:

```rust
/// Direct DSL creation request
#[derive(Debug, Clone)]
pub struct DslCreationRequest {
    pub dsl_content: String,
    pub domain: String,
    pub case_type: Option<String>,
    pub client_context: ClientContext,
}

/// DSL update request for incremental operations
#[derive(Debug, Clone)]
pub struct DslUpdateRequest {
    pub case_id: String,
    pub additional_dsl: String,
    pub domain: Option<String>,
    pub operation_metadata: OperationMetadata,
}

/// Batch DSL operation
#[derive(Debug, Clone)]
pub struct DslBatchOperation {
    pub dsl_content: String,
    pub context: DslContext,
    pub operation_id: String,
}

/// Client context (no AI-specific fields)
#[derive(Debug, Clone)]
pub struct ClientContext {
    pub client_name: String,
    pub jurisdiction: String,
    pub entity_type: String,
    pub services: Vec<String>,
    pub compliance_requirements: Vec<String>,
}
```

### 1.3 DSL Results (Pure Processing Results)
Clean results without AI-specific data:

```rust
#[derive(Debug, Clone)]
pub struct CaseCreationResult {
    pub case_id: String,
    pub processing_result: ProcessingResult,
    pub state_snapshot: StateSnapshot,
    pub visualization_data: VisualizationData,
    pub audit_trail: Vec<AuditEntry>,
}

#[derive(Debug, Clone)]
pub struct CaseUpdateResult {
    pub case_id: String,
    pub version_number: u64,
    pub accumulated_dsl: String,
    pub processing_result: ProcessingResult,
    pub state_snapshot: StateSnapshot,
    pub triggered_workflows: Vec<WorkflowTrigger>,
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub suggested_improvements: Vec<String>,
    pub compliance_score: f64,
}
```

## Phase 2: AI Agent Layer (Separate Module)

### 2.1 AI Agent Architecture
**Role**: Optional layer that generates DSL content

```rust
#[derive(Debug)]
pub struct AiDslAgent {
    ai_service: Arc<dyn AiService>,
    dsl_manager: Arc<DslManager>,  // Uses core DSL system
    template_engine: Arc<TemplateEngine>,
    validation_engine: Arc<AiValidationEngine>,
}

impl AiDslAgent {
    // === AI-POWERED DSL GENERATION ===
    
    /// Generate DSL from natural language instruction
    pub async fn generate_dsl_from_instruction(&self, instruction: String) -> AiResult<GeneratedDslResult> {
        // 1. Generate DSL using AI
        let generated_dsl = self.ai_service.generate_dsl(&instruction).await?;
        
        // 2. Validate generated DSL using core system (no AI dependency)
        let validation = self.dsl_manager.validate_dsl(
            generated_dsl.clone(),
            DslContext::from_instruction(&instruction)
        ).await?;
        
        // 3. Return generated DSL + validation (let user decide whether to execute)
        Ok(GeneratedDslResult {
            generated_dsl,
            validation_result: validation,
            confidence_score: generated_dsl.confidence,
            suggested_improvements: validation.suggested_improvements,
        })
    }
    
    /// Generate and immediately execute DSL
    pub async fn generate_and_execute(&self, instruction: String) -> AiResult<ExecutedAiResult> {
        // 1. Generate DSL
        let generated = self.generate_dsl_from_instruction(instruction).await?;
        
        // 2. If validation passes, execute using core DSL system
        if generated.validation_result.valid {
            let execution_result = self.dsl_manager.create_case(
                generated.generated_dsl,
                DslContext::from_instruction(&instruction)
            ).await?;
            
            Ok(ExecutedAiResult {
                generated_dsl: generated.generated_dsl,
                execution_result,
                ai_confidence: generated.confidence_score,
            })
        } else {
            Err(AiError::ValidationFailed(generated.validation_result))
        }
    }
    
    /// Enhance existing DSL with AI suggestions
    pub async fn enhance_existing_dsl(&self, case_id: String, enhancement_instruction: String) -> AiResult<EnhancementResult> {
        // 1. Load existing DSL from core system
        let existing_state = self.dsl_manager.get_case_state(&case_id).await?;
        
        // 2. Generate enhancement DSL
        let enhancement = self.ai_service.generate_enhancement(
            &existing_state.current_dsl,
            &enhancement_instruction
        ).await?;
        
        // 3. Validate enhancement using core system
        let validation = self.dsl_manager.validate_dsl(
            enhancement.clone(),
            existing_state.context
        ).await?;
        
        Ok(EnhancementResult {
            original_dsl: existing_state.current_dsl,
            enhancement_dsl: enhancement,
            validation_result: validation,
        })
    }
}
```

### 2.2 AI Request Types (Separate from Core DSL)
AI-specific request types that generate DSL:

```rust
/// Natural language instruction for DSL generation
#[derive(Debug, Clone)]
pub struct AiInstructionRequest {
    pub instruction: String,
    pub context_hints: Vec<String>,
    pub preferred_domain: Option<String>,
    pub client_context: Option<ClientContext>,
    pub generation_options: AiGenerationOptions,
}

/// AI-specific generation options
#[derive(Debug, Clone)]
pub struct AiGenerationOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub include_explanations: bool,
    pub validation_level: ValidationLevel,
}

/// Enhancement request for existing DSL
#[derive(Debug, Clone)]
pub struct DslEnhancementRequest {
    pub case_id: String,
    pub enhancement_instruction: String,
    pub enhancement_type: EnhancementType,
}

#[derive(Debug, Clone)]
pub enum EnhancementType {
    AddCompliance,
    AddService,
    AddDocumentation,
    OptimizeStructure,
    AddErrorHandling,
}
```

### 2.3 AI Results (Separate from Core Results)
AI-specific results that contain both generated DSL and core processing results:

```rust
#[derive(Debug, Clone)]
pub struct GeneratedDslResult {
    pub generated_dsl: String,
    pub validation_result: ValidationResult,  // From core system
    pub confidence_score: f64,
    pub generation_explanation: String,
    pub suggested_improvements: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ExecutedAiResult {
    pub generated_dsl: String,
    pub execution_result: CaseCreationResult,  // From core system
    pub ai_confidence: f64,
    pub generation_metadata: AiGenerationMetadata,
}

#[derive(Debug, Clone)]
pub struct EnhancementResult {
    pub original_dsl: String,
    pub enhancement_dsl: String,
    pub validation_result: ValidationResult,  // From core system
    pub enhancement_explanation: String,
}
```

## Phase 3: Clean Integration Pattern

### 3.1 Usage Patterns

#### Direct DSL Usage (No AI)
```rust
// Pure DSL operations
let dsl_manager = DslManager::new(config).await?;

// Create case with direct DSL
let dsl = "(case.create :case-id \"CASE-001\" :case-type \"ONBOARDING\" :client-name \"TechCorp Ltd\")";
let result = dsl_manager.create_case(dsl.to_string(), DslContext::onboarding()).await?;

// Update case with additional DSL
let additional_dsl = "(kyc.collect :case-id \"CASE-001\" :collection-type \"ENHANCED\")";
let update_result = dsl_manager.update_case("CASE-001".to_string(), additional_dsl.to_string()).await?;
```

#### AI-Enhanced Usage (Optional)
```rust
// AI-enhanced operations (optional layer)
let ai_agent = AiDslAgent::new(ai_service, dsl_manager).await?;

// Generate DSL from natural language
let instruction = "Create comprehensive onboarding for UK technology company needing custody services";
let generated = ai_agent.generate_dsl_from_instruction(instruction.to_string()).await?;

// User can review generated DSL before executing
if generated.validation_result.valid && user_approves(&generated.generated_dsl) {
    let execution_result = dsl_manager.create_case(
        generated.generated_dsl,
        DslContext::onboarding()
    ).await?;
}

// Or generate and execute immediately (if confidence is high)
let executed = ai_agent.generate_and_execute(instruction.to_string()).await?;
```

### 3.2 Module Organization
```
rust/src/
├── dsl_manager/           # Core DSL CRUD operations
│   ├── core.rs           # DSL Manager implementation
│   ├── requests.rs       # Pure DSL request types
│   ├── results.rs        # Pure DSL result types
│   └── mod.rs
├── dsl/                   # DSL processing engine
│   ├── mod.rs            # DslProcessor (no AI dependencies)
│   ├── orchestration_interface.rs
│   └── ...
├── db_state_manager/      # Database state management
│   ├── mod.rs
│   └── ...
├── dsl_visualizer/        # Visualization engine
│   ├── mod.rs
│   └── ...
├── ai/                    # AI layer (separate module)
│   ├── agent.rs          # AiDslAgent implementation
│   ├── requests.rs       # AI-specific request types
│   ├── results.rs        # AI-specific result types
│   ├── services/         # AI service providers
│   └── mod.rs
└── lib.rs
```

## Phase 4: Clean Test Architecture

### 4.1 Core DSL Tests (No AI Dependencies)
```rust
mod dsl_core_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_direct_dsl_case_creation() {
        let dsl_manager = DslManager::new(test_config()).await.unwrap();
        
        let dsl = "(case.create :case-id \"TEST-001\" :case-type \"ONBOARDING\")";
        let result = dsl_manager.create_case(dsl.to_string(), DslContext::test()).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap().case_id, "TEST-001");
    }
    
    #[tokio::test]
    async fn test_incremental_dsl_updates() {
        let dsl_manager = DslManager::new(test_config()).await.unwrap();
        
        // Create base case
        let base_dsl = "(case.create :case-id \"TEST-002\" :case-type \"ONBOARDING\")";
        let create_result = dsl_manager.create_case(base_dsl.to_string(), DslContext::test()).await.unwrap();
        
        // Add incremental DSL
        let additional_dsl = "(kyc.collect :case-id \"TEST-002\" :collection-type \"STANDARD\")";
        let update_result = dsl_manager.update_case(create_result.case_id, additional_dsl.to_string()).await;
        
        assert!(update_result.is_ok());
        assert!(update_result.unwrap().accumulated_dsl.contains("case.create"));
        assert!(update_result.unwrap().accumulated_dsl.contains("kyc.collect"));
    }
    
    #[tokio::test]
    async fn test_dsl_validation_only() {
        let dsl_manager = DslManager::new(test_config()).await.unwrap();
        
        let dsl = "(case.create :case-id \"TEST-003\" :case-type \"INVALID_TYPE\")";
        let validation = dsl_manager.validate_dsl(dsl.to_string(), DslContext::test()).await.unwrap();
        
        assert!(!validation.valid);
        assert!(!validation.errors.is_empty());
    }
}
```

### 4.2 AI Layer Tests (Separate, Optional)
```rust
#[cfg(feature = "ai")]
mod ai_layer_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_ai_dsl_generation() {
        let dsl_manager = DslManager::new(test_config()).await.unwrap();
        let ai_agent = AiDslAgent::new(mock_ai_service(), dsl_manager).await.unwrap();
        
        let instruction = "Create onboarding for technology company";
        let generated = ai_agent.generate_dsl_from_instruction(instruction.to_string()).await.unwrap();
        
        assert!(!generated.generated_dsl.is_empty());
        assert!(generated.generated_dsl.contains("case.create"));
        assert!(generated.validation_result.valid);
    }
    
    #[tokio::test]
    async fn test_ai_generation_with_core_validation() {
        // Test that AI-generated DSL is validated by core system
        let dsl_manager = DslManager::new(test_config()).await.unwrap();
        let ai_agent = AiDslAgent::new(mock_ai_service(), dsl_manager).await.unwrap();
        
        let instruction = "Create invalid DSL for testing";
        let generated = ai_agent.generate_dsl_from_instruction(instruction.to_string()).await.unwrap();
        
        // Core validation should catch AI generation errors
        if !generated.validation_result.valid {
            assert!(!generated.validation_result.errors.is_empty());
        }
    }
}
```

### 4.3 Integration Tests (Both Systems)
```rust
mod integration_tests {
    #[tokio::test]
    async fn test_end_to_end_flow_direct_dsl() {
        // Test complete flow with direct DSL (no AI)
        let system = setup_complete_system().await;
        
        let dsl = create_complete_onboarding_dsl();
        let result = system.dsl_manager.create_case(dsl, DslContext::onboarding()).await.unwrap();
        
        // Verify complete pipeline worked
        assert_case_created_in_database(&system.db_state_manager, &result.case_id).await;
        assert_visualization_generated(&result.visualization_data).await;
    }
    
    #[cfg(feature = "ai")]
    #[tokio::test]
    async fn test_end_to_end_flow_ai_enhanced() {
        // Test complete flow with AI enhancement
        let system = setup_complete_system_with_ai().await;
        
        let instruction = "Create comprehensive onboarding for UK fintech requiring custody and derivatives";
        let ai_result = system.ai_agent.generate_and_execute(instruction.to_string()).await.unwrap();
        
        // Verify AI → Core DSL → Database → Visualization flow
        assert_ai_dsl_generated(&ai_result.generated_dsl);
        assert_case_created_in_database(&system.db_state_manager, &ai_result.execution_result.case_id).await;
        assert_visualization_reflects_ai_operation(&ai_result.execution_result.visualization_data);
    }
}
```

## Benefits of This Separation

### ✅ **Reliability**
- Core DSL system works without AI dependencies
- Deterministic operations for critical business logic
- AI failures don't break core functionality

### ✅ **Testability**
- Core system can be thoroughly tested without AI mocking
- AI layer tests focus on generation quality
- Clear separation of concerns in test suites

### ✅ **Flexibility**
- Users can choose direct DSL or AI-enhanced workflows
- Different AI providers can be plugged in
- Core system evolution independent of AI advances

### ✅ **Performance**
- Direct DSL operations are fast and efficient
- AI operations only when explicitly requested
- No AI overhead for standard operations

### ✅ **Maintenance**
- Core DSL logic is stable and predictable
- AI experiments don't affect core system
- Independent versioning and deployment possible

---

**Architecture**: Clean separation with AI as optional enhancement layer
**Core Principle**: DSL-first design with AI as value-add, not dependency
**Next Steps**: Implement core DSL CRUD operations first, add AI layer second