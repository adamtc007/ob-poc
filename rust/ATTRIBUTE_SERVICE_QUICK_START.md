# AttributeService Quick Start Guide

## Overview

The `AttributeService` bridges Phase 1-3 of the attribute refactoring, providing a unified interface for DSL processing and database persistence.

## Setup

```rust
use ob_poc::services::AttributeService;
use ob_poc::database::attribute_repository::AttributeRepository;
use ob_poc::domains::attributes::validator::AttributeValidator;
use ob_poc::domains::attributes::kyc::{FirstName, LastName};
use sqlx::PgPool;

// 1. Create database pool
let pool = PgPool::connect(&database_url).await?;

// 2. Setup validator and register attributes
let mut validator = AttributeValidator::new();
validator.register(FirstName::metadata());
validator.register(LastName::metadata());

// 3. Create service
let service = AttributeService::from_pool(pool, validator);
```

## Basic Operations

### Store an Attribute
```rust
let entity_id = Uuid::new_v4();

service.set_attribute::<FirstName>(
    entity_id,
    "John".to_string(),
    Some("user_id")
).await?;
```

### Retrieve an Attribute
```rust
let name = service.get_attribute::<FirstName>(entity_id).await?;
// Returns: Option<String>
```

### Get Multiple Attributes
```rust
let attr_ids = vec![FirstName::ID, LastName::ID];
let attributes = service.get_many_attributes(entity_id, &attr_ids).await?;
// Returns: HashMap<String, serde_json::Value>
```

### Get Attribute History
```rust
let history = service.get_attribute_history::<FirstName>(entity_id, 10).await?;
for entry in history {
    println!("{}: {}", entry.effective_from, entry.value);
}
```

## DSL Processing

### Process DSL with Auto-Persistence
```rust
let dsl = r#"(entity.register
    :entity-id "550e8400-e29b-41d4-a716-446655440000"
    :first-name @attr.identity.first_name
    :last-name @attr.identity.last_name
)"#;

let result = service.process_attribute_dsl(
    entity_id,
    dsl,
    Some("processor_id")
).await?;

println!("Validation: {}", result.validation_passed);
println!("Persisted: {}", result.attributes_persisted);
```

## DSL Generation

### Generate GET DSL
```rust
let dsl = service.generate_get_attribute_dsl::<FirstName>(entity_id)?;
// Output: (entity.get-attribute :entity-id "..." :attribute @attr.identity.first_name)
```

### Generate SET DSL
```rust
let dsl = service.generate_set_attribute_dsl::<FirstName>(
    entity_id,
    &"John".to_string()
)?;
// Output: (entity.set-attribute :entity-id "..." :attribute @attr.identity.first_name)
```

## Running Examples

### Demo Application
```bash
# Set database URL
export DATABASE_URL="postgresql://localhost/ob_poc"

# Run demonstration
cargo run --example attribute_integration_demo --features database
```

### Integration Tests
```bash
# Run all integration tests
cargo test --test attribute_integration_test --features database

# Run specific test
cargo test --test attribute_integration_test test_end_to_end_attribute_storage_and_retrieval --features database
```

## Error Handling

```rust
use ob_poc::services::AttributeServiceError;

match service.set_attribute::<FirstName>(entity_id, value, Some("user")).await {
    Ok(id) => println!("Stored with ID: {}", id),
    Err(AttributeServiceError::Validation(e)) => eprintln!("Validation error: {}", e),
    Err(AttributeServiceError::Repository(e)) => eprintln!("Database error: {}", e),
    Err(e) => eprintln!("Error: {}", e),
}
```

## Common Patterns

### Pattern 1: Onboarding Flow
```rust
// Store all customer attributes
service.set_attribute::<FirstName>(id, first, Some("onboarding")).await?;
service.set_attribute::<LastName>(id, last, Some("onboarding")).await?;
service.set_attribute::<LegalEntityName>(id, company, Some("onboarding")).await?;
```

### Pattern 2: DSL-Driven Updates
```rust
// Process DSL from external source
let dsl = load_dsl_from_file("customer_update.dsl")?;
let result = service.process_attribute_dsl(entity_id, &dsl, Some("batch_processor")).await?;

if !result.is_success() {
    eprintln!("Processing failed: {:?}", result);
}
```

### Pattern 3: Audit Trail
```rust
// Get complete history for compliance
let history = service.get_attribute_history::<FirstName>(entity_id, 100).await?;

for entry in history {
    audit_log!(
        "Attribute changed: {} -> {} by {} at {}",
        FirstName::ID,
        entry.value,
        entry.created_by,
        entry.effective_from
    );
}
```

## Architecture Flow

```
User Input (DSL or Type-Safe Call)
        ↓
AttributeService
        ↓
    Validation (Phase 2)
        ↓
    Type Checking (Phase 1)
        ↓
AttributeRepository (Phase 3)
        ↓
    SQLX Queries
        ↓
  PostgreSQL Database
```

## Key Benefits

1. **Type Safety**: Compile-time checking with `AttributeType` trait
2. **Validation**: DSL validation before persistence
3. **Transactions**: Batch operations are atomic
4. **History**: Full temporal versioning
5. **Performance**: Built-in caching layer
6. **Integration**: Seamless Phase 1-3 connectivity

## Next Steps

1. Register your custom attributes with the validator
2. Use `AttributeService` in your business logic
3. Process DSL with automatic persistence
4. Query attribute history for audit trails
5. Generate DSL from domain operations

For more details, see:
- Full documentation: `PHASE_1_3_INTEGRATION_COMPLETE.md`
- Example code: `examples/attribute_integration_demo.rs`
- Test suite: `tests/attribute_integration_test.rs`
