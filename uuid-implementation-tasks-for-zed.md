# UUID Integration - Implementation Tasks for Claude Agent

**Project:** ob-poc  
**Current State:** UUID components built but NOT connected  
**Objective:** Complete UUID integration so DSL with `@attr{uuid}` works end-to-end

---

## Task 1: Fix AttributeService Integration (CRITICAL - BLOCKING EVERYTHING)

### File: `rust/src/services/attribute_service.rs`

#### Add imports at top of file:
```rust
use crate::domains::attributes::resolver::{AttributeResolver, ResolutionError};
```

#### Update struct definition (line ~24):
```rust
#[derive(Clone)]
pub struct AttributeService {
    repository: AttributeRepository,
    validator: AttributeValidator,
    resolver: AttributeResolver,  // ADD THIS LINE
}
```

#### Update constructor (line ~54):
```rust
pub fn new(repository: AttributeRepository, validator: AttributeValidator) -> Self {
    Self {
        repository,
        validator,
        resolver: AttributeResolver::new(),  // ADD THIS LINE
    }
}
```

#### Replace the extract_attr_ref method (line ~298):
```rust
fn extract_attr_ref(&self, value: &Value) -> Option<String> {
    match value {
        Value::AttrRef(attr_id) => Some(attr_id.clone()),
        Value::AttrUuid(uuid) => {
            // REPLACE THE PLACEHOLDER WITH ACTUAL RESOLUTION
            match self.resolver.uuid_to_semantic(uuid) {
                Ok(semantic_id) => Some(semantic_id),
                Err(e) => {
                    log::error!("Failed to resolve UUID {}: {}", uuid, e);
                    None
                }
            }
        }
        _ => None,
    }
}
```

#### Add new UUID-based methods before the closing brace:
```rust
/// Set attribute value by UUID
pub async fn set_by_uuid(
    &self,
    entity_id: Uuid,
    attr_uuid: Uuid,
    value: serde_json::Value,
    created_by: Option<&str>,
) -> Result<i64> {
    let semantic_id = self.resolver
        .uuid_to_semantic(&attr_uuid)
        .map_err(|e| AttributeServiceError::Resolution(e.to_string()))?;
    
    let attrs = vec![(semantic_id.as_str(), value)];
    let ids = self.repository
        .set_many_transactional(entity_id, attrs, created_by)
        .await?;
    
    Ok(ids.into_iter().next().unwrap_or(-1))
}

/// Get attribute value by UUID
pub async fn get_by_uuid(
    &self,
    entity_id: Uuid,
    attr_uuid: Uuid,
) -> Result<Option<serde_json::Value>> {
    let semantic_id = self.resolver
        .uuid_to_semantic(&attr_uuid)
        .map_err(|e| AttributeServiceError::Resolution(e.to_string()))?;
    
    let attrs = self.get_many_attributes(entity_id, &[&semantic_id]).await?;
    Ok(attrs.get(&semantic_id).cloned())
}
```

#### Update error enum (line ~36):
```rust
#[derive(Debug, thiserror::Error)]
pub enum AttributeServiceError {
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Attribute extraction error: {0}")]
    Extraction(String),
    
    #[error("DSL generation error: {0}")]
    DslGeneration(String),
    
    #[error("UUID resolution error: {0}")]  // ADD THIS
    Resolution(String),
}
```

### Verification Test

Create file: `rust/tests/uuid_service_integration_test.rs`

```rust
use ob_poc::services::AttributeService;
use ob_poc::domains::attributes::validator::AttributeValidator;
use ob_poc::parser_ast::Value;
use uuid::Uuid;

#[test]
fn test_uuid_resolution_works() {
    let validator = AttributeValidator::new_with_defaults();
    let pool = sqlx::PgPool::connect_lazy("postgresql://test").unwrap();
    let service = AttributeService::from_pool(pool, validator);
    
    // First name UUID from uuid_constants.rs
    let first_name_uuid = Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935").unwrap();
    let value = Value::AttrUuid(first_name_uuid);
    
    // Should resolve to semantic ID, not "uuid:{uuid}"
    let result = service.extract_attr_ref(&value).unwrap();
    assert_eq!(result, "attr.identity.first_name");
    assert!(!result.starts_with("uuid:"));  // Verify placeholder is gone
}
```

Run: `cargo test uuid_service_integration_test`

---

## Task 2: Create Source Executor Framework

### Create directory structure:
```
rust/src/domains/attributes/sources/
├── mod.rs
├── default.rs
├── user_input.rs
└── document_extraction.rs
```

### File: `rust/src/domains/attributes/sources/mod.rs`

```rust
use async_trait::async_trait;
use uuid::Uuid;
use serde_json::Value as JsonValue;
use crate::domains::attributes::execution_context::{ExecutionContext, ValueSource};

pub type SourceResult<T> = Result<T, SourceError>;

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("Document not found: {0}")]
    DocumentNotFound(Uuid),
    
    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),
    
    #[error("API call failed: {0}")]
    ApiError(String),
    
    #[error("No valid source for attribute: {0}")]
    NoValidSource(Uuid),
}

#[derive(Debug, Clone)]
pub struct AttributeValue {
    pub uuid: Uuid,
    pub semantic_id: String,
    pub value: JsonValue,
    pub source: ValueSource,
}

#[async_trait]
pub trait SourceExecutor: Send + Sync {
    async fn fetch_value(
        &self,
        attr_uuid: Uuid,
        context: &ExecutionContext,
    ) -> SourceResult<AttributeValue>;
    
    fn can_handle(&self, attr_uuid: &Uuid) -> bool;
    fn priority(&self) -> u32;
}

pub mod default;
pub mod user_input;
pub mod document_extraction;

pub use default::DefaultValueSource;
pub use user_input::UserInputSource;
pub use document_extraction::DocumentExtractionSource;
```

### File: `rust/src/domains/attributes/sources/default.rs`

```rust
use super::*;
use async_trait::async_trait;
use std::collections::HashMap;

pub struct DefaultValueSource {
    defaults: HashMap<Uuid, JsonValue>,
}

impl DefaultValueSource {
    pub fn new() -> Self {
        let mut defaults = HashMap::new();
        
        // Country default (attr.address.country)
        defaults.insert(
            Uuid::parse_str("f47ac10b-58cc-5e7a-a716-446655440037").unwrap(),
            JsonValue::String("US".to_string())
        );
        
        // Risk tolerance default (attr.product.risk_tolerance)
        defaults.insert(
            Uuid::parse_str("f47ac10b-58cc-5e7a-a716-446655440047").unwrap(),
            JsonValue::String("MODERATE".to_string())
        );
        
        Self { defaults }
    }
}

#[async_trait]
impl SourceExecutor for DefaultValueSource {
    async fn fetch_value(
        &self,
        attr_uuid: Uuid,
        context: &ExecutionContext,
    ) -> SourceResult<AttributeValue> {
        let value = self.defaults
            .get(&attr_uuid)
            .ok_or(SourceError::NoValidSource(attr_uuid))?;
        
        let semantic_id = context.resolver
            .uuid_to_semantic(&attr_uuid)
            .map_err(|_| SourceError::NoValidSource(attr_uuid))?;
        
        Ok(AttributeValue {
            uuid: attr_uuid,
            semantic_id,
            value: value.clone(),
            source: ValueSource::Default {
                reason: "System default value".to_string()
            },
        })
    }
    
    fn can_handle(&self, attr_uuid: &Uuid) -> bool {
        self.defaults.contains_key(attr_uuid)
    }
    
    fn priority(&self) -> u32 {
        999  // Lowest priority
    }
}
```

### File: `rust/src/domains/attributes/sources/document_extraction.rs`

```rust
use super::*;
use async_trait::async_trait;
use std::collections::HashMap;

/// Simulated document extraction - replace with real OCR/NLP later
pub struct DocumentExtractionSource {
    extracted_data: HashMap<Uuid, JsonValue>,
}

impl DocumentExtractionSource {
    pub fn new() -> Self {
        let mut extracted_data = HashMap::new();
        
        // Mock passport extraction data
        // First name UUID
        extracted_data.insert(
            Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935").unwrap(),
            JsonValue::String("John".to_string())
        );
        
        // Last name UUID  
        extracted_data.insert(
            Uuid::parse_str("d2c3a812-7b4f-5e6a-8d9c-1f3b5c7e9a2d").unwrap(),
            JsonValue::String("Smith".to_string())
        );
        
        // Passport number UUID
        extracted_data.insert(
            Uuid::parse_str("f47ac10b-58cc-5e7a-a716-446655440006").unwrap(),
            JsonValue::String("AB123456".to_string())
        );
        
        Self { extracted_data }
    }
}

#[async_trait]
impl SourceExecutor for DocumentExtractionSource {
    async fn fetch_value(
        &self,
        attr_uuid: Uuid,
        context: &ExecutionContext,
    ) -> SourceResult<AttributeValue> {
        let value = self.extracted_data
            .get(&attr_uuid)
            .ok_or(SourceError::NoValidSource(attr_uuid))?;
        
        let semantic_id = context.resolver
            .uuid_to_semantic(&attr_uuid)
            .map_err(|_| SourceError::NoValidSource(attr_uuid))?;
        
        Ok(AttributeValue {
            uuid: attr_uuid,
            semantic_id,
            value: value.clone(),
            source: ValueSource::DocumentExtraction {
                document_id: Uuid::new_v4(),
                page: Some(1),
                confidence: 0.95,
            },
        })
    }
    
    fn can_handle(&self, attr_uuid: &Uuid) -> bool {
        self.extracted_data.contains_key(attr_uuid)
    }
    
    fn priority(&self) -> u32 {
        5  // High priority
    }
}
```

### Update: `rust/src/domains/attributes/mod.rs`

Add these lines:
```rust
pub mod sources;
```

---

## Task 3: Create Value Binder

### File: `rust/src/execution/value_binder.rs`

```rust
use uuid::Uuid;
use crate::domains::attributes::sources::{
    SourceExecutor, SourceError, DefaultValueSource, DocumentExtractionSource
};
use crate::domains::attributes::execution_context::ExecutionContext;

pub struct ValueBinder {
    sources: Vec<Box<dyn SourceExecutor>>,
}

impl ValueBinder {
    pub fn new() -> Self {
        let mut sources: Vec<Box<dyn SourceExecutor>> = vec![
            Box::new(DocumentExtractionSource::new()),
            Box::new(DefaultValueSource::new()),
        ];
        
        sources.sort_by_key(|s| s.priority());
        
        Self { sources }
    }
    
    pub async fn bind_attribute(
        &self,
        attr_uuid: Uuid,
        context: &mut ExecutionContext,
    ) -> Result<(), SourceError> {
        for source in &self.sources {
            if !source.can_handle(&attr_uuid) {
                continue;
            }
            
            match source.fetch_value(attr_uuid, context).await {
                Ok(value) => {
                    context.bind_value(attr_uuid, value.value, value.source);
                    return Ok(());
                }
                Err(e) => {
                    log::debug!("Source failed for {}: {}", attr_uuid, e);
                    continue;
                }
            }
        }
        
        Err(SourceError::NoValidSource(attr_uuid))
    }
    
    pub async fn bind_all(
        &self,
        attr_uuids: Vec<Uuid>,
        context: &mut ExecutionContext,
    ) -> Vec<Result<(), SourceError>> {
        let mut results = Vec::new();
        
        for uuid in attr_uuids {
            results.push(self.bind_attribute(uuid, context).await);
        }
        
        results
    }
}
```

### Update: `rust/src/execution/mod.rs`

Add:
```rust
pub mod value_binder;
pub use value_binder::ValueBinder;
```

---

## Task 4: Wire Up DSL Execution

### File: `rust/src/execution/dsl_executor.rs`

Create or update this file:

```rust
use uuid::Uuid;
use crate::parser::parse_program;
use crate::parser_ast::{Program, Form, Value};
use crate::domains::attributes::execution_context::ExecutionContext;
use crate::services::AttributeService;
use crate::execution::ValueBinder;

pub struct DslExecutor {
    service: AttributeService,
    binder: ValueBinder,
}

impl DslExecutor {
    pub fn new(service: AttributeService) -> Self {
        Self {
            service,
            binder: ValueBinder::new(),
        }
    }
    
    /// Extract all UUID references from a program
    fn extract_uuids(&self, program: &Program) -> Vec<Uuid> {
        let mut uuids = Vec::new();
        
        for form in program {
            if let Form::Verb(verb) = form {
                for (_, value) in &verb.pairs {
                    if let Value::AttrUuid(uuid) = value {
                        uuids.push(*uuid);
                    }
                }
            }
        }
        
        uuids
    }
    
    /// Execute DSL with UUID resolution and value binding
    pub async fn execute(
        &self,
        dsl: &str,
        request_id: Uuid,
    ) -> Result<ExecutionResult, String> {
        // 1. Parse DSL
        let program = parse_program(dsl)
            .map_err(|e| format!("Parse error: {:?}", e))?;
        
        // 2. Extract UUIDs
        let uuids = self.extract_uuids(&program);
        
        // 3. Create execution context
        let mut context = ExecutionContext::new();
        
        // 4. Bind values for all UUIDs
        let bind_results = self.binder.bind_all(uuids.clone(), &mut context).await;
        
        // 5. Count successful bindings
        let successful = bind_results.iter().filter(|r| r.is_ok()).count();
        
        // 6. Store bound values in database
        for uuid in &uuids {
            if let Some(value) = context.get_value(uuid) {
                self.service.set_by_uuid(
                    request_id,
                    *uuid,
                    value.clone(),
                    Some("dsl_executor"),
                ).await.ok();
            }
        }
        
        Ok(ExecutionResult {
            request_id,
            attributes_resolved: successful,
            attributes_stored: successful,
            errors: bind_results.into_iter()
                .filter_map(|r| r.err().map(|e| e.to_string()))
                .collect(),
        })
    }
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub request_id: Uuid,
    pub attributes_resolved: usize,
    pub attributes_stored: usize,
    pub errors: Vec<String>,
}
```

---

## Task 5: End-to-End Test

### File: `rust/tests/uuid_e2e_test.rs`

```rust
use ob_poc::services::AttributeService;
use ob_poc::domains::attributes::validator::AttributeValidator;
use ob_poc::execution::dsl_executor::{DslExecutor, ExecutionResult};
use uuid::Uuid;

#[tokio::test]
async fn test_uuid_dsl_end_to_end() {
    // Skip if no database
    if std::env::var("DATABASE_URL").is_err() {
        println!("Skipping - DATABASE_URL not set");
        return;
    }
    
    // Setup
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    
    let validator = AttributeValidator::new_with_defaults();
    let service = AttributeService::from_pool(pool.clone(), validator);
    let executor = DslExecutor::new(service.clone());
    
    // DSL with UUID references
    let dsl = r#"
        (kyc.collect
            :request-id "REQ-001"
            :first-name @attr{3020d46f-472c-5437-9647-1b0682c35935}
            :passport @attr{f47ac10b-58cc-5e7a-a716-446655440006}
        )
    "#;
    
    let request_id = Uuid::new_v4();
    
    // Execute
    let result = executor.execute(dsl, request_id).await.unwrap();
    
    // Verify resolution
    assert!(result.attributes_resolved > 0);
    assert!(result.errors.is_empty());
    
    // Verify storage
    let first_name_uuid = Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935").unwrap();
    let stored = service.get_by_uuid(request_id, first_name_uuid).await.unwrap();
    
    assert!(stored.is_some());
    println!("Stored value: {:?}", stored);
}
```

Run: `cargo test uuid_e2e_test --features database`

---

## Task 6: Verify Everything Works

### Run all tests in order:
```bash
# 1. Test UUID resolution in service
cargo test uuid_service_integration_test

# 2. Test source executors
cargo test --lib sources

# 3. Test value binding
cargo test --lib value_binder

# 4. Test end-to-end
cargo test uuid_e2e_test --features database
```

### Check that DSL with UUIDs works:
```rust
// Should parse
@attr{3020d46f-472c-5437-9647-1b0682c35935}

// Should resolve to
"attr.identity.first_name"

// Should fetch value from source
"John"

// Should store in database with UUID
```

---

## Success Criteria

✅ AttributeService resolves UUIDs to semantic IDs  
✅ Source executors fetch values  
✅ Value binder connects attributes to values  
✅ DSL executor processes UUID-based DSL  
✅ Values persist to database  
✅ All tests pass

---

## Notes for Implementation

1. **Start with Task 1** - It's blocking everything else
2. **Test after each task** - Don't move on until tests pass
3. **Use mock data initially** - Real extraction can come later
4. **Check logs** - Add log statements to debug resolution
5. **Keep backward compatibility** - String-based attributes should still work

This completes the UUID integration. Once all tasks are done, UUID-based DSL will work end-to-end!
