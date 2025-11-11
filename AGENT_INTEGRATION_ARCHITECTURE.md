# AI Agent Integration Architecture

## Overview

The OB-POC system has evolved from a deprecated monolithic agent system to a modern, multi-provider AI integration architecture that seamlessly combines AI-powered DSL generation with robust DSL execution through the DSL Manager.

## Architecture Evolution

### Before: Deprecated Agent System
```
Deprecated Architecture (src/deprecated/agents/)
├── DslAgent (monolithic)
├── DslValidator 
├── DslTemplateEngine
└── Complex coupling with database
```

### After: Modern AI Integration
```
Current Architecture (src/ai/ + src/services/)
├── AI Service Layer (src/ai/)
│   ├── AiService trait (common interface)
│   ├── OpenAiClient (GPT-3.5/GPT-4)
│   ├── GeminiClient (Google Gemini)
│   └── Unified JSON parsing
├── AI DSL Service (src/services/ai_dsl_service.rs)
│   ├── End-to-end workflow orchestration
│   ├── CBU generation
│   └── DSL Manager integration
└── DSL Manager (src/dsl_manager_backup.rs)
    ├── Template-based DSL execution
    ├── Database persistence
    └── Compilation & validation
```

## Core Components

### 1. AI Service Layer (`src/ai/`)

#### AiService Trait
```rust
#[async_trait::async_trait]
pub trait AiService {
    async fn request_dsl(&self, request: AiDslRequest) -> AiResult<AiDslResponse>;
    async fn health_check(&self) -> AiResult<bool>;
    fn config(&self) -> &AiConfig;
}
```

#### Multiple AI Providers
- **OpenAiClient**: GPT-3.5-turbo, GPT-4 integration
- **GeminiClient**: Google Gemini API integration
- **Unified Interface**: Same API regardless of provider

#### Structured Operations
```rust
pub struct AiDslRequest {
    pub instruction: String,              // Natural language requirement
    pub current_dsl: Option<String>,      // Existing DSL (for transforms)
    pub context: HashMap<String, String>, // Business context
    pub response_type: AiResponseType,    // Generate/Transform/Validate/Explain
    pub constraints: Vec<String>,         // AI guidance constraints
}

pub struct AiDslResponse {
    pub dsl_content: String,     // Generated/modified DSL
    pub explanation: String,     // AI's explanation
    pub confidence: f64,         // AI confidence score (0.0-1.0)
    pub changes: Vec<String>,    // List of changes made
    pub warnings: Vec<String>,   // Issues or concerns
    pub suggestions: Vec<String>, // Improvement recommendations
}
```

### 2. AI DSL Service (`src/services/ai_dsl_service.rs`)

The orchestration layer that provides end-to-end business workflows.

#### Core Capabilities
- **AI-Powered DSL Generation**: Convert natural language to DSL
- **CBU Management**: Generate unique Client Business Unit identifiers
- **DSL Execution**: Execute AI-generated DSL through DSL Manager
- **Validation Pipeline**: Syntax + semantic validation
- **State Management**: Full onboarding workflow orchestration

#### Key Methods
```rust
impl AiDslService {
    // Main workflow method
    pub async fn create_ai_onboarding(&self, request: AiOnboardingRequest) -> AiOnboardingResponse;
    
    // Health monitoring
    pub async fn health_check(&self) -> HealthCheckResult;
    
    // AI validation
    pub async fn validate_dsl_with_ai(&self, dsl_content: &str) -> ValidationResult;
}
```

### 3. CBU Generator

Generates unique Client Business Unit identifiers:

```rust
pub struct CbuGenerator;

impl CbuGenerator {
    pub fn generate_cbu_id(client_name: &str, jurisdiction: &str, entity_type: &str) -> String;
    pub fn generate_test_cbu_ids(count: usize) -> Vec<String>;
}
```

**Example Output**: `CBU-TECHCORP-GB-CORP-042`

### 4. DSL Manager Integration

The AI DSL Service integrates with the existing DSL Manager for execution:

```rust
// AI generates DSL → DSL Manager executes it
let dsl_instance = self.dsl_manager.create_dsl_instance(
    "onboarding",
    TemplateType::CustomerOnboarding,
    variables_json,
    "ai_dsl_service",
).await?;

// Add AI-generated content
self.dsl_manager.edit_dsl_instance(
    instance.id,
    ai_generated_dsl,
    "ai_generated_content",
    "ai_dsl_service",
).await?;
```

## Workflow: End-to-End AI Onboarding

### Step 1: Natural Language Input
```rust
let request = AiOnboardingRequest {
    instruction: "Create onboarding for a UK technology company needing custody services".to_string(),
    client_name: "TechCorp Ltd".to_string(),
    jurisdiction: "GB".to_string(),
    entity_type: "CORP".to_string(),
    services: vec!["CUSTODY".to_string()],
    compliance_level: Some("standard".to_string()),
    context: business_context,
    ai_provider: Some("openai".to_string()),
};
```

### Step 2: CBU Generation
```rust
let cbu_id = CbuGenerator::generate_cbu_id(
    &request.client_name,
    &request.jurisdiction, 
    &request.entity_type
);
// Result: "CBU-TECHCORP-GB-CORP-123"
```

### Step 3: AI DSL Generation
The system sends a structured prompt to the AI:

**System Prompt**:
- Complete approved DSL vocabulary (70+ verbs)
- S-expression syntax rules
- Business context awareness
- JSON response format requirement

**User Prompt**:
- Natural language instruction
- Client information and context
- Service requirements
- Compliance constraints

**AI Response** (JSON):
```json
{
  "dsl_content": "(case.create\n  :cbu-id \"CBU-TECHCORP-GB-CORP-123\"\n  :nature-purpose \"Technology services company\"\n  :jurisdiction \"GB\")\n\n(products.add \"CUSTODY\")\n\n(kyc.start\n  :customer-id \"CBU-TECHCORP-GB-CORP-123\"\n  :jurisdictions [\"GB\"]\n  :required-documents [\"CertificateOfIncorporation\" \"ArticlesOfAssociation\"])",
  "explanation": "Generated comprehensive onboarding DSL for TechCorp Ltd including case creation, custody product setup, and KYC initiation with UK corporate requirements.",
  "confidence": 0.92,
  "changes": [],
  "warnings": [],
  "suggestions": ["Consider adding compliance.verify for enhanced due diligence"]
}
```

### Step 4: DSL Validation
```rust
// Syntax validation
match parse_program(&ai_response.dsl_content) {
    Ok(forms) => validate_required_verbs(&forms),
    Err(e) => return validation_error(e),
}

// AI-powered validation (optional)
let validation = service.validate_dsl_with_ai(&ai_response.dsl_content).await?;
```

### Step 5: DSL Execution
```rust
// Create DSL instance through DSL Manager
let instance = dsl_manager.create_dsl_instance(
    "onboarding",
    TemplateType::CustomerOnboarding,
    variables,
    "ai_dsl_service"
).await?;

// Add AI-generated content as edit
let version = dsl_manager.edit_dsl_instance(
    instance.id,
    ai_generated_dsl,
    "ai_generated_content",
    "ai_dsl_service"
).await?;
```

### Step 6: Response Assembly
```rust
AiOnboardingResponse {
    cbu_id: "CBU-TECHCORP-GB-CORP-123",
    dsl_instance: DslInstanceSummary {
        instance_id: uuid,
        domain: "onboarding",
        status: "active",
        created_at: timestamp,
        current_version: 1,
    },
    generated_dsl: ai_response.dsl_content,
    ai_explanation: ai_response.explanation,
    ai_confidence: 0.92,
    execution_details: ExecutionDetails {
        template_used: "onboarding",
        compilation_successful: true,
        validation_passed: true,
        execution_time_ms: 2341,
    },
    warnings: [],
    suggestions: ["Consider adding compliance.verify..."],
}
```

## DSL Operations Supported

### 1. Generate DSL (`AiResponseType::GenerateDsl`)
- Create new DSL from natural language requirements
- Use case: Initial onboarding setup

### 2. Transform DSL (`AiResponseType::TransformDsl`)
- Modify existing DSL based on business changes
- Use case: Add new services, update compliance requirements

### 3. Validate DSL (`AiResponseType::ValidateDsl`)
- Check syntax, vocabulary, and business logic compliance
- Use case: Quality assurance, pre-deployment validation

### 4. Explain DSL (`AiResponseType::ExplainDsl`)
- Analyze and explain DSL structure and meaning
- Use case: Documentation, training, audit support

### 5. Suggest Improvements (`AiResponseType::SuggestImprovements`)
- Recommend enhancements for better structure/compliance
- Use case: Optimization, best practices enforcement

## AI Provider Configuration

### OpenAI Configuration
```rust
let config = AiConfig {
    api_key: env::var("OPENAI_API_KEY")?,
    model: "gpt-3.5-turbo".to_string(),  // Cost-effective
    // model: "gpt-4".to_string(),        // High-quality
    max_tokens: Some(2048),
    temperature: Some(0.1),              // Deterministic
    timeout_seconds: 30,
};
```

### Gemini Configuration
```rust
let config = AiConfig {
    api_key: env::var("GEMINI_API_KEY")?,
    model: "gemini-2.5-flash-preview-09-2025".to_string(),
    max_tokens: Some(8192),
    temperature: Some(0.1),
    timeout_seconds: 30,
};
```

## Robust Response Parsing

Both AI clients use identical structured JSON parsing:

### JSON-First Approach
1. **Force JSON Responses**: Use provider-specific response format controls
2. **Unified Parsing**: `utils::parse_structured_response` for all providers
3. **Error Recovery**: Fallback parsing strategies for malformed responses
4. **Type Safety**: Strong typing prevents runtime parsing errors

### Example Implementation
```rust
// OpenAI: Force JSON response format
response_format: Some(OpenAiResponseFormat {
    format_type: "json_object".to_string(),
})

// Gemini: Prompt-based JSON enforcement
"RESPONSE FORMAT - Respond ONLY with valid JSON: { ... }"

// Unified parsing for both
let parsed = utils::parse_structured_response(&cleaned_response)?;
let response = AiDslResponse {
    dsl_content: parsed["dsl_content"].as_str().unwrap_or("").to_string(),
    explanation: parsed["explanation"].as_str().unwrap_or("").to_string(),
    confidence: parsed["confidence"].as_f64().unwrap_or(0.8),
    // ...
};
```

## Error Handling & Resilience

### Comprehensive Error Types
```rust
pub enum AiDslServiceError {
    AiError(AiError),                    // AI service failures
    DslManagerError(String),             // DSL execution failures
    ValidationError(String),             // Business logic violations
    CbuGenerationError(String),          // CBU ID conflicts
    ParsingError(String),                // DSL syntax errors
    ConfigError(String),                 // Configuration issues
}
```

### Graceful Degradation
- **AI Service Failures**: Fallback to templates or manual intervention
- **Rate Limiting**: Exponential backoff and retry logic
- **Validation Failures**: Detailed error messages for correction
- **Partial Success**: Continue workflow with warnings where possible

## Testing Strategy

### Unit Tests
- CBU generation algorithms
- DSL validation logic  
- AI response parsing
- Error handling scenarios

### Integration Tests
- End-to-end AI → DSL Manager flow
- Database persistence verification
- Multi-provider AI testing
- Performance benchmarking

### Mock Testing
- AI service mocking for deterministic testing
- Database transaction testing
- Error scenario simulation

## Performance Considerations

### AI Service Optimization
- **Model Selection**: GPT-3.5 for cost, GPT-4 for quality
- **Token Management**: Efficient prompting to minimize costs
- **Caching**: Template and response caching where appropriate
- **Async Processing**: Non-blocking AI calls

### DSL Manager Integration
- **Batch Operations**: Multiple DSL operations in single transaction
- **Database Pooling**: Efficient connection management
- **Compilation Optimization**: AST caching and reuse

## Security & Compliance

### API Key Management
```bash
# Environment-based configuration
export OPENAI_API_KEY="sk-..."
export GEMINI_API_KEY="..."

# Keys never hardcoded in source
let api_key = env::var("OPENAI_API_KEY").map_err(|_| ConfigError)?;
```

### Data Privacy
- **No Data Retention**: AI providers configured for zero data retention
- **Anonymization**: PII scrubbing before AI processing where required
- **Audit Trails**: Complete logging of AI interactions
- **Compliance**: GDPR, SOC2 compliance through provider agreements

## Monitoring & Observability

### Health Checks
```rust
pub struct HealthCheckResult {
    pub ai_service_healthy: bool,    // AI provider accessibility
    pub dsl_manager_healthy: bool,   // DSL execution capability
    pub database_healthy: bool,      // Data persistence
    pub overall_healthy: bool,       // System-wide health
}
```

### Metrics
- **AI Response Times**: Latency monitoring
- **Confidence Scores**: Quality tracking
- **Success Rates**: Reliability metrics
- **Error Classification**: Issue categorization

### Logging
```rust
info!("AI DSL generation completed with confidence: {:.2}", response.confidence);
debug!("Generated DSL: {}", response.dsl_content);
warn!("Low confidence AI response: {:.2}", response.confidence);
error!("AI service failure: {}", error);
```

## Future Enhancements

### Planned Features
- **Additional AI Providers**: Claude, Llama2, local models
- **Template Learning**: AI learns from successful patterns
- **Batch Processing**: Multi-client onboarding workflows
- **Real-time Collaboration**: Multi-user DSL editing
- **Advanced Validation**: Domain-specific business rule checking

### Integration Points
- **Web UI**: Interactive DSL generation interface
- **REST API**: External system integration endpoints
- **CLI Tools**: Batch operations and automation
- **Monitoring Dashboards**: Real-time system health visualization

## Migration Guide

### From Deprecated Agents
```rust
// OLD (deprecated)
use ob_poc::agents::{DslAgent, DslTransformationRequest};
let agent = DslAgent::new(config).await?;
let response = agent.create_onboarding_dsl(request).await?;

// NEW (current)
use ob_poc::services::{AiDslService, AiOnboardingRequest};
let service = AiDslService::new_with_openai(database_manager, Some(config)).await?;
let response = service.create_ai_onboarding(request).await?;
```

### Environment Setup
```bash
# Choose AI provider
export OPENAI_API_KEY="your-openai-key"
# OR
export GEMINI_API_KEY="your-gemini-key"

# Database configuration
export DATABASE_URL="postgresql://..."

# Run examples
cargo run --example ai_dsl_onboarding_demo
```

## Conclusion

The AI Agent Integration architecture represents a significant evolution from the deprecated monolithic agent system to a modern, scalable, multi-provider AI integration. Key achievements:

✅ **Unified Interface**: Same API for multiple AI providers  
✅ **Robust Parsing**: JSON-first approach eliminates fragile string parsing  
✅ **End-to-End Workflows**: Complete integration with DSL Manager  
✅ **Type Safety**: Rust's type system prevents runtime errors  
✅ **Comprehensive Testing**: Full test coverage and validation  
✅ **Production Ready**: Error handling, monitoring, and security built-in  

The system successfully bridges the gap between natural language business requirements and executable DSL, providing a powerful foundation for intelligent financial onboarding workflows.