# Agentic CRUD Database Integration Plan

**Status**: Ready for Implementation  
**Last Updated**: 2025-01-17  
**Architecture**: DSL-as-State + AttributeID-as-Type + Real Database Integration

## Overview

This plan outlines the steps to connect all agentic CRUD services (CBU, Entity, Document) to the actual PostgreSQL database, enabling real CRUD operations instead of mock simulations.

## Current State Analysis

### ✅ What's Working (Mock Mode)
- **Unified Agentic Service**: Single interface for all CRUD operations
- **AI DSL Generation**: Natural language → DSL conversion
- **DSL Parsing**: Generated DSL → AST structures
- **Operation Detection**: Smart routing between domains
- **EAV Document Schema**: New AttributeID-based document tables created

### ❌ What's Missing (Database Integration)
- **Real Database Connections**: Services not connected to PostgreSQL
- **Actual CRUD Execution**: All operations simulated
- **Schema Validation**: No validation against actual table constraints
- **Transaction Management**: No real transaction handling
- **Error Handling**: Mock errors instead of real DB errors

## Implementation Plan

### Phase 1: Database Infrastructure Setup

#### 1.1 Database Schema Verification
```bash
# Verify all required tables exist
psql $DATABASE_URL -c "
SELECT table_name 
FROM information_schema.tables 
WHERE table_schema = 'ob-poc' 
AND table_name IN (
  'cbus', 'entities', 'dictionary', 
  'document_catalog', 'document_metadata', 
  'document_relationships', 'document_usage'
);"
```

**Expected Tables**:
- ✅ `cbus` - Client Business Units
- ✅ `entities` - Entity management  
- ✅ `dictionary` - AttributeID dictionary
- ✅ `document_catalog` - EAV document catalog
- ✅ `document_metadata` - EAV metadata entries
- ✅ `document_relationships` - Document relationships
- ✅ `document_usage` - Document usage tracking

#### 1.2 Apply Missing Schema Updates
```bash
# Apply new EAV document schema
psql $DATABASE_URL -f sql/15_document_library_eav_schema.sql

# Verify no conflicts with old schema
psql $DATABASE_URL -c "
SELECT COUNT(*) as old_doc_tables
FROM information_schema.tables 
WHERE table_schema = 'ob-poc' 
AND table_name IN ('document_types', 'document_issuers', 'iso_asset_types');"
```

#### 1.3 Seed Required Data
```bash
# Ensure dictionary has required AttributeIDs
psql $DATABASE_URL -f sql/03_seed_dictionary_attributes.sql

# Add document-specific attributes if missing
psql $DATABASE_URL -f sql/10_seed_document_dictionary.sql
```

### Phase 2: Service Layer Integration

#### 2.1 Update Database Service Constructors

**File**: `rust/src/database/mod.rs`
```rust
impl DatabaseManager {
    /// Create unified agentic service with real database connections
    pub fn create_unified_agentic_service(&self) -> UnifiedAgenticService {
        let document_service = DocumentDatabaseService::new(self.pool().clone());
        let cbu_service = CbuRepository::new(self.pool().clone());
        let entity_service = EntityCrudService::new(self.pool().clone());
        
        UnifiedAgenticService::with_database(
            document_service,
            cbu_service, 
            entity_service
        )
    }
}
```

#### 2.2 Update Unified Agentic Service

**File**: `rust/src/ai/unified_agentic_service.rs`

**Add Database Constructor**:
```rust
impl UnifiedAgenticService {
    /// Create service with real database connections
    pub fn with_database(
        document_service: DocumentDatabaseService,
        pool: PgPool,
    ) -> Self {
        let crud_service = AgenticCrudService::with_database(pool.clone());
        let document_service = AgenticDocumentService::with_database(document_service);
        
        Self {
            crud_service,
            document_service,
            rag_system: CrudRagSystem::new(),
            config: UnifiedServiceConfig::with_database(),
        }
    }
}
```

**Update Configuration**:
```rust
impl UnifiedServiceConfig {
    pub fn with_database() -> Self {
        Self {
            service_name: "UnifiedAgenticService-DB".to_string(),
            ai_provider: UnifiedAiProvider::OpenAI {
                api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
                model: "gpt-4".to_string(),
            },
            execute_operations: true, // ← Enable real execution
            ..Default::default()
        }
    }
}
```

#### 2.3 Update Individual Services

**CBU Service Integration**:
```rust
// File: rust/src/ai/agentic_crud_service.rs
impl AgenticCrudService {
    pub fn with_database(pool: PgPool) -> Self {
        Self {
            rag_system: CrudRagSystem::new(),
            prompt_builder: CrudPromptBuilder::new(),
            database_service: Some(CbuRepository::new(pool)),
            config: ServiceConfig::with_database(),
        }
    }
}
```

**Document Service Integration**:
```rust
// File: rust/src/ai/agentic_document_service.rs
impl AgenticDocumentService {
    pub fn with_database(db_service: DocumentDatabaseService) -> Self {
        Self {
            rag_system: CrudRagSystem::new(),
            prompt_builder: CrudPromptBuilder::new(),
            database_service: Some(db_service),
            config: DocumentServiceConfig::with_database(),
        }
    }
}
```

### Phase 3: CRUD Operation Execution

#### 3.1 CBU CRUD Operations

**Create CBU**:
```rust
async fn execute_cbu_create(
    &self, 
    instruction: &str, 
    parsed_dsl: &DataCreate
) -> Result<CbuOperationResult> {
    let new_cbu = NewCbu {
        cbu_id: parsed_dsl.extract_cbu_id()?,
        name: parsed_dsl.extract_name()?,
        entity_type: parsed_dsl.extract_entity_type()?,
        jurisdiction: parsed_dsl.extract_jurisdiction(),
        // ... other fields from DSL
    };
    
    let cbu_id = self.database_service
        .create_cbu(new_cbu)
        .await
        .context("Failed to create CBU")?;
        
    Ok(CbuOperationResult::Created { cbu_id })
}
```

**Query CBUs**:
```rust
async fn execute_cbu_query(
    &self,
    parsed_dsl: &DataRead
) -> Result<CbuOperationResult> {
    let search_criteria = CbuSearchCriteria {
        entity_type: parsed_dsl.extract_entity_type_filter(),
        jurisdiction: parsed_dsl.extract_jurisdiction_filter(),
        status: parsed_dsl.extract_status_filter(),
    };
    
    let cbus = self.database_service
        .search_cbus(search_criteria)
        .await
        .context("Failed to query CBUs")?;
        
    Ok(CbuOperationResult::Found { cbus })
}
```

#### 3.2 Entity CRUD Operations

**Create Entity**:
```rust
async fn execute_entity_create(
    &self,
    entity_type: EntityType,
    parsed_data: &HashMap<String, Value>
) -> Result<EntityOperationResult> {
    match entity_type {
        EntityType::Company => {
            let company = NewLimitedCompany {
                entity_id: Uuid::new_v4(),
                company_name: parsed_data.extract_name()?,
                jurisdiction: parsed_data.extract_jurisdiction()?,
                incorporation_date: parsed_data.extract_date("incorporation_date"),
                // ... other fields
            };
            
            let entity_id = self.database_service
                .create_company(company)
                .await?;
                
            Ok(EntityOperationResult::Created { entity_id })
        },
        EntityType::Partnership => {
            // Similar for partnerships
        },
        EntityType::Trust => {
            // Similar for trusts  
        },
        // ... other entity types
    }
}
```

#### 3.3 Document CRUD Operations

**Catalog Document**:
```rust
async fn execute_document_catalog(
    &self,
    parsed_dsl: &DocumentCatalogDsl
) -> Result<DocumentOperationResult> {
    // Create document catalog entry
    let new_doc = NewDocumentCatalog {
        file_hash_sha256: parsed_dsl.extract_file_hash()?,
        storage_key: parsed_dsl.extract_storage_key()?,
        file_size_bytes: parsed_dsl.extract_file_size(),
        mime_type: parsed_dsl.extract_mime_type(),
        extraction_status: Some("PENDING".to_string()),
    };
    
    let doc_id = self.database_service
        .create_document(new_doc)
        .await?;
    
    // Add metadata attributes
    let metadata = parsed_dsl.extract_metadata()?;
    for (attr_id, value) in metadata {
        let doc_metadata = NewDocumentMetadata {
            doc_id,
            attribute_id: attr_id,
            value: serde_json::to_value(value)?,
        };
        
        self.database_service
            .add_document_metadata(doc_metadata)
            .await?;
    }
    
    Ok(DocumentOperationResult::Cataloged { 
        doc_id,
        metadata_entries: metadata.len()
    })
}
```

**Search Documents**:
```rust
async fn execute_document_search(
    &self,
    parsed_dsl: &DocumentQueryDsl
) -> Result<DocumentOperationResult> {
    let search_request = DocumentSearchRequest {
        query: parsed_dsl.extract_text_query(),
        attribute_filters: parsed_dsl.extract_attribute_filters()?,
        extraction_status: parsed_dsl.extract_status_filter(),
        mime_type: parsed_dsl.extract_mime_type_filter(),
        limit: parsed_dsl.extract_limit().unwrap_or(20),
        offset: parsed_dsl.extract_offset().unwrap_or(0),
    };
    
    let search_response = self.database_service
        .search_documents(search_request)
        .await?;
        
    Ok(DocumentOperationResult::SearchResults { 
        documents: search_response.documents,
        total_count: search_response.total_count
    })
}
```

### Phase 4: Transaction Management

#### 4.1 Add Transaction Support
```rust
// File: rust/src/ai/unified_agentic_service.rs
async fn execute_with_transaction<T>(
    &self,
    operation: impl FnOnce(&mut sqlx::Transaction<'_, Postgres>) -> BoxFuture<'_, Result<T>>
) -> Result<T> {
    let mut tx = self.pool.begin().await?;
    
    match operation(&mut tx).await {
        Ok(result) => {
            tx.commit().await?;
            Ok(result)
        }
        Err(e) => {
            tx.rollback().await?;
            Err(e)
        }
    }
}
```

#### 4.2 Cross-Domain Transactions
```rust
async fn execute_cross_domain_operation(
    &self,
    request: &UnifiedAgenticRequest
) -> Result<UnifiedOperationResult> {
    self.execute_with_transaction(|tx| async move {
        // Step 1: Create CBU
        let cbu_id = self.create_cbu_in_transaction(tx, &request).await?;
        
        // Step 2: Create related entities
        let entity_ids = self.create_entities_in_transaction(tx, cbu_id, &request).await?;
        
        // Step 3: Catalog documents
        let doc_ids = self.catalog_documents_in_transaction(tx, cbu_id, &request).await?;
        
        // Step 4: Link everything together
        self.link_resources_in_transaction(tx, cbu_id, &entity_ids, &doc_ids).await?;
        
        Ok(UnifiedOperationResult::CrossDomain {
            cbu_id,
            entity_ids,
            doc_ids,
        })
    }.boxed()).await
}
```

### Phase 5: Error Handling & Validation

#### 5.1 Database-Specific Error Handling
```rust
impl From<sqlx::Error> for UnifiedAgenticError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::Database(db_err) => {
                if db_err.constraint().is_some() {
                    UnifiedAgenticError::ConstraintViolation {
                        constraint: db_err.constraint().unwrap().to_string(),
                        message: db_err.message().to_string(),
                    }
                } else {
                    UnifiedAgenticError::DatabaseError(db_err.message().to_string())
                }
            }
            sqlx::Error::RowNotFound => UnifiedAgenticError::NotFound,
            _ => UnifiedAgenticError::DatabaseError(err.to_string()),
        }
    }
}
```

#### 5.2 Schema Validation
```rust
async fn validate_operation_against_schema(
    &self,
    operation: &CrudStatement
) -> Result<ValidationResult> {
    match operation {
        CrudStatement::DataCreate(create) => {
            // Validate required fields exist
            self.validate_required_fields(&create.asset, &create.values).await?;
            
            // Validate AttributeIDs exist in dictionary
            self.validate_attribute_ids(&create.values).await?;
            
            // Validate data types match dictionary definitions
            self.validate_data_types(&create.values).await?;
        }
        // ... other operations
    }
    
    Ok(ValidationResult::Valid)
}
```

### Phase 6: Integration Testing

#### 6.1 Create Database Integration Tests

**File**: `rust/tests/database_integration_test.rs`
```rust
#[tokio::test]
async fn test_full_cbu_crud_cycle() {
    let db = setup_test_database().await;
    let service = UnifiedAgenticService::with_database(db.pool());
    
    // Test CREATE
    let create_request = UnifiedAgenticRequest {
        instruction: "Create a hedge fund called Test Capital LP in Cayman Islands".to_string(),
        execute: true, // ← Real execution
        // ...
    };
    
    let create_response = service.process_request(create_request).await.unwrap();
    assert!(create_response.success);
    
    let cbu_id = extract_cbu_id_from_response(&create_response);
    
    // Test READ
    let read_request = UnifiedAgenticRequest {
        instruction: format!("Find CBU with ID {}", cbu_id),
        execute: true,
        // ...
    };
    
    let read_response = service.process_request(read_request).await.unwrap();
    assert!(read_response.success);
    
    // Test UPDATE
    let update_request = UnifiedAgenticRequest {
        instruction: format!("Update CBU {} to set status as active", cbu_id),
        execute: true,
        // ...
    };
    
    let update_response = service.process_request(update_request).await.unwrap();
    assert!(update_response.success);
    
    // Test DELETE
    let delete_request = UnifiedAgenticRequest {
        instruction: format!("Delete CBU {}", cbu_id),
        execute: true,
        // ...
    };
    
    let delete_response = service.process_request(delete_request).await.unwrap();
    assert!(delete_response.success);
}
```

#### 6.2 Create Real Database Demo

**File**: `rust/examples/real_database_crud_demo.rs`
```rust
#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 === Real Database CRUD Demo ===");
    
    // Setup database connection
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url).await?;
    
    // Create unified service with real database
    let service = UnifiedAgenticService::with_database(
        DocumentDatabaseService::new(pool.clone()),
        pool
    );
    
    // Test real operations
    test_real_cbu_operations(&service).await?;
    test_real_entity_operations(&service).await?;
    test_real_document_operations(&service).await?;
    test_cross_domain_operations(&service).await?;
    
    println!("✅ All real database operations completed successfully!");
    Ok(())
}
```

### Phase 7: Performance & Monitoring

#### 7.1 Add Real Performance Metrics
```rust
#[derive(Debug, Clone)]
pub struct DatabasePerformanceMetrics {
    pub connection_time_ms: u64,
    pub query_time_ms: u64,
    pub transaction_time_ms: u64,
    pub rows_affected: usize,
    pub cache_hit_rate: f64,
}

impl UnifiedAgenticService {
    async fn execute_with_metrics<T>(
        &self,
        operation: impl Future<Output = Result<T>>
    ) -> Result<(T, DatabasePerformanceMetrics)> {
        let start = Instant::now();
        let result = operation.await?;
        let total_time = start.elapsed().as_millis() as u64;
        
        let metrics = DatabasePerformanceMetrics {
            connection_time_ms: 0, // Would track actual connection time
            query_time_ms: total_time,
            transaction_time_ms: total_time,
            rows_affected: 1, // Would track actual rows
            cache_hit_rate: 0.0,
        };
        
        Ok((result, metrics))
    }
}
```

#### 7.2 Add Connection Health Monitoring
```rust
impl UnifiedAgenticService {
    pub async fn health_check(&self) -> Result<DatabaseHealthStatus> {
        // Test each service's database connection
        let cbu_health = self.crud_service.health_check().await?;
        let doc_health = self.document_service.health_check().await?;
        let pool_health = self.check_connection_pool_health().await?;
        
        Ok(DatabaseHealthStatus {
            overall_healthy: cbu_health && doc_health && pool_health,
            services: HashMap::from([
                ("cbu".to_string(), cbu_health),
                ("document".to_string(), doc_health),
                ("pool".to_string(), pool_health),
            ]),
            active_connections: self.pool.size(),
            idle_connections: self.pool.num_idle(),
        })
    }
}
```

## Implementation Steps

### Step 1: Environment Setup
```bash
# Set database URL
export DATABASE_URL="postgresql://user:password@localhost:5432/ob_poc_db"

# Set AI API keys (choose one)
export OPENAI_API_KEY="your-openai-key"
# OR
export GEMINI_API_KEY="your-gemini-key"

# Apply schemas
cd ob-poc
psql $DATABASE_URL -f sql/00_init_schema.sql
psql $DATABASE_URL -f sql/15_document_library_eav_schema.sql
psql $DATABASE_URL -f sql/03_seed_dictionary_attributes.sql
```

### Step 2: Update Services (Priority Order)
1. **Update Database Manager** (`database/mod.rs`)
2. **Update Unified Service** (`ai/unified_agentic_service.rs`)
3. **Update Individual Services** (`ai/agentic_crud_service.rs`, `ai/agentic_document_service.rs`)
4. **Add CRUD Execution Logic** (new methods in each service)
5. **Add Transaction Support** (transaction management)

### Step 3: Testing
```bash
# Test with database features
cargo test --features="database" -- database_integration

# Run real database demo
cargo run --example real_database_crud_demo --features="database"

# Run unified demo with real execution
cargo run --example unified_agentic_crud_demo --features="database"
```

### Step 4: Validation
1. **Check all tables have data after operations**
2. **Verify foreign key relationships maintained**
3. **Confirm AttributeID references are valid**
4. **Test rollback on transaction failures**
5. **Validate cross-domain operations work end-to-end**

## Success Criteria

✅ **CBU Operations**: Create, Read, Update, Delete CBUs via natural language  
✅ **Entity Operations**: Create companies, partnerships, trusts with proper relationships  
✅ **Document Operations**: Catalog, search, extract, link documents with EAV metadata  
✅ **Cross-Domain**: Multi-step workflows spanning CBUs, entities, and documents  
✅ **Performance**: Real timing metrics (expect 10-100ms per operation)  
✅ **Reliability**: Proper transaction handling and error recovery  
✅ **Data Integrity**: All foreign keys and constraints maintained  

## Risk Mitigation

- **Backup Database**: Always test against copy of production data
- **Transaction Rollbacks**: Ensure all operations can be safely rolled back
- **Connection Pooling**: Handle connection limits and timeouts
- **Error Logging**: Comprehensive logging for debugging database issues
- **Schema Migration**: Plan for schema changes without breaking existing code

## Timeline Estimate

- **Phase 1-2** (Database + Service Integration): 1-2 days
- **Phase 3-4** (CRUD Execution + Transactions): 2-3 days  
- **Phase 5-6** (Error Handling + Testing): 1-2 days
- **Phase 7** (Performance + Monitoring): 1 day

**Total**: 5-8 days for complete integration

---

**Ready for Implementation**: This plan provides a complete roadmap for connecting the mock agentic CRUD system to real PostgreSQL database operations while maintaining the existing AI-powered natural language interface.