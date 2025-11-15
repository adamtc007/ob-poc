# Document Catalog & Extraction System - Complete Fix Plan

**Target:** Fix all broken document catalog functionality and complete extraction system integration
**Estimated Time:** 4-6 hours
**Priority:** CRITICAL - System is non-functional without these fixes

---

## Phase 1: Fix Rust Models to Match Database Schema (45 min)

### Issue: Rust models missing critical database fields

#### File: `/rust/src/models/document_models.rs`

**Line 18-31 - Fix DocumentCatalog struct:**
```rust
// CURRENT (BROKEN):
pub struct DocumentCatalog {
    pub doc_id: Uuid,
    pub file_hash_sha256: String,
    pub storage_key: String,
    pub file_size_bytes: Option<i64>,
    pub mime_type: Option<String>,
    pub extracted_data: Option<serde_json::Value>,
    pub extraction_status: String,
    pub extraction_confidence: Option<f64>,
    pub last_extracted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// CHANGE TO:
pub struct DocumentCatalog {
    pub doc_id: Uuid,
    pub file_hash_sha256: String,
    pub storage_key: String,
    pub file_size_bytes: Option<i64>,
    pub mime_type: Option<String>,
    pub extracted_data: Option<serde_json::Value>,
    pub extraction_status: String,
    pub extraction_confidence: Option<f64>,
    pub last_extracted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub cbu_id: Option<Uuid>,           // ADD THIS
    pub document_type_id: Option<Uuid>, // ADD THIS
}
```

**Line 34-43 - Fix NewDocumentCatalog struct:**
```rust
// ADD these fields to NewDocumentCatalog:
pub struct NewDocumentCatalog {
    pub file_hash_sha256: String,
    pub storage_key: String,
    pub file_size_bytes: Option<i64>,
    pub mime_type: Option<String>,
    pub extracted_data: Option<serde_json::Value>,
    pub extraction_status: Option<String>,
    pub extraction_confidence: Option<f64>,
    pub cbu_id: Option<Uuid>,           // ADD THIS
    pub document_type_id: Option<Uuid>, // ADD THIS
}
```

---

## Phase 2: Fix DocumentCatalogSource Queries (30 min)

### Issue: References non-existent `document_usage` table

#### File: `/rust/src/services/document_catalog_source.rs`

**Lines 72-87 - Fix find_best_document method:**
```rust
// CURRENT (BROKEN - references document_usage):
let existing_doc = sqlx::query_scalar::<_, Uuid>(
    r#"
    SELECT doc_id
    FROM "ob-poc".document_metadata dm
    JOIN "ob-poc".document_usage du ON dm.doc_id = du.doc_id
    WHERE du.cbu_id = $1
    AND dm.attribute_id = $2
    ORDER BY dm.created_at DESC
    LIMIT 1
    "#,
)

// CHANGE TO:
let existing_doc = sqlx::query_scalar::<_, Uuid>(
    r#"
    SELECT dm.doc_id
    FROM "ob-poc".document_metadata dm
    JOIN "ob-poc".document_catalog dc ON dm.doc_id = dc.doc_id
    WHERE dc.cbu_id = $1
    AND dm.attribute_id = $2
    ORDER BY dm.created_at DESC
    LIMIT 1
    "#,
)
```

**Lines 94-105 - Fix document catalog query:**
```rust
// CURRENT (BROKEN - references document_usage):
let doc_id = sqlx::query_scalar::<_, Uuid>(
    r#"
    SELECT dc.doc_id
    FROM "ob-poc".document_catalog dc
    JOIN "ob-poc".document_usage du ON dc.doc_id = du.doc_id
    WHERE du.cbu_id = $1
    AND dc.extraction_status IN ('PENDING', 'COMPLETED')
    "#,
)

// CHANGE TO:
let doc_id = sqlx::query_scalar::<_, Uuid>(
    r#"
    SELECT doc_id
    FROM "ob-poc".document_catalog
    WHERE cbu_id = $1
    AND extraction_status IN ('PENDING', 'COMPLETED')
    AND document_type_id IS NOT NULL
    ORDER BY created_at DESC
    LIMIT 1
    "#,
)
```

---

## Phase 3: Fix DocumentTypeRepository (30 min)

### Issue: Missing get_typed_document method

#### File: `/rust/src/database/document_type_repository.rs`

**Add after line 150:**
```rust
/// Get typed document with its mappings
pub async fn get_typed_document(
    &self,
    document_id: Uuid,
) -> Result<Option<TypedDocument>, sqlx::Error> {
    // First get the document
    let doc = sqlx::query!(
        r#"
        SELECT 
            doc_id,
            document_type_id,
            cbu_id
        FROM "ob-poc".document_catalog
        WHERE doc_id = $1
        "#,
        document_id
    )
    .fetch_optional(self.pool.as_ref())
    .await?;

    let doc = match doc {
        Some(d) => d,
        None => return Ok(None),
    };

    // If no type assigned, can't get mappings
    let type_id = match doc.document_type_id {
        Some(id) => id,
        None => return Ok(None),
    };

    // Get document type
    let doc_type = self.get_by_id(type_id).await?
        .ok_or_else(|| sqlx::Error::RowNotFound)?;

    // Get mappings
    let mappings = self.get_mappings(type_id).await?;

    Ok(Some(TypedDocument {
        document_id: doc.doc_id,
        document_type: doc_type,
        extractable_attributes: mappings,
    }))
}
```

---

## Phase 4: Fix ExecutionContext (45 min)

### Issue: Missing DocumentExtraction variant in ValueSource

#### File: `/rust/src/domains/attributes/execution_context.rs`

**Find ValueSource enum and add variant:**
```rust
// FIND:
pub enum ValueSource {
    Runtime,
    Database,
    UserInput,
    // other variants...
}

// ADD:
pub enum ValueSource {
    Runtime,
    Database,
    UserInput,
    DocumentExtraction {
        document_id: Uuid,
        extraction_method: String,
        confidence: f64,
        extracted_at: DateTime<Utc>,
    },
    // other variants...
}
```

---

## Phase 5: Wire DocumentExtractionHandler to Engine (1 hour)

### Issue: Handler exists but isn't registered

#### File: `/rust/src/execution/engine.rs` or `/rust/src/execution/mod.rs`

**Find where handlers are registered and add:**
```rust
// FIND something like:
impl EngineBuilder {
    pub fn build(self) -> DslExecutionEngine {
        let mut handlers = HashMap::new();
        // existing handlers...
        
        // ADD:
        if let Some(pool) = &self.pool {
            let doc_handler = Arc::new(
                DocumentExtractionHandler::new(Arc::clone(pool))
            );
            handlers.insert("document.extract".to_string(), doc_handler);
        }
    }
}
```

**OR if there's a default_handlers function:**
```rust
pub fn default_handlers(pool: Arc<PgPool>) -> HashMap<String, Arc<dyn OperationHandler>> {
    let mut handlers = HashMap::new();
    
    // ADD:
    handlers.insert(
        "document.extract".to_string(),
        Arc::new(DocumentExtractionHandler::new(pool.clone()))
    );
    
    handlers
}
```

---

## Phase 6: Fix Document Extraction Handler (30 min)

### Issue: Handler needs to bind values to ExecutionContext

#### File: `/rust/src/execution/document_extraction_handler.rs`

**Lines 110-150 - Fix execute method to bind values:**
```rust
async fn execute(
    &self,
    operation: &DslOperation,
    context: &mut ExecutionContext,
    state: &DslState,
) -> Result<ExecutionResult> {
    let document_id = self.extract_document_id(operation)?;
    let entity_id = self.extract_entity_id(operation)?;
    
    // Extract from document
    let extracted = self.service
        .extract_from_document(document_id, entity_id)
        .await?;
    
    // CRITICAL: Bind extracted values to context
    for attr in &extracted {
        context.bind_value(
            attr.attribute_uuid,
            attr.value.clone(),
            ValueSource::DocumentExtraction {
                document_id,
                extraction_method: attr.extraction_method.to_string(),
                confidence: attr.confidence,
                extracted_at: Utc::now(),
            }
        );
    }
    
    // Store values (dual-write already handled in service)
    
    Ok(ExecutionResult::Success {
        message: format!("Extracted {} attributes", extracted.len()),
        state: state.clone(),
    })
}
```

---

## Phase 7: Add Transaction Support to Dual-Write (30 min)

### Issue: Dual-write operations not transactional

#### File: `/rust/src/database/document_type_repository.rs`

**Lines 221-280 - Wrap in transaction:**
```rust
pub async fn store_extracted_value(
    &self,
    document_id: Uuid,
    entity_id: Uuid,
    extracted: &ExtractedAttribute,
) -> Result<(), sqlx::Error> {
    // START TRANSACTION
    let mut tx = self.pool.begin().await?;
    
    // Store in document_metadata
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".document_metadata
        (doc_id, attribute_id, value, extraction_confidence, 
         extraction_method, extracted_at, extraction_metadata)
        VALUES ($1, $2, $3, $4, $5, NOW(), $6)
        ON CONFLICT (doc_id, attribute_id)
        DO UPDATE SET
            value = EXCLUDED.value,
            extraction_confidence = EXCLUDED.extraction_confidence,
            extraction_method = EXCLUDED.extraction_method,
            extracted_at = EXCLUDED.extracted_at,
            extraction_metadata = EXCLUDED.extraction_metadata
        "#,
    )
    .bind(document_id)
    .bind(extracted.attribute_uuid)
    .bind(&extracted.value)
    .bind(extracted.confidence)
    .bind(extracted.extraction_method.to_string())
    .bind(sqlx::types::Json(&extracted.metadata))
    .execute(&mut *tx)  // Use transaction
    .await?;
    
    // Store in attribute_values_typed
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".attribute_values_typed
        (value_id, cbu_id, attribute_uuid, entity_id, 
         text_value, effective_from)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, NOW())
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(entity_id)  // Using entity_id as cbu_id proxy
    .bind(extracted.attribute_uuid)
    .bind(entity_id)
    .bind(extracted.value.as_str())
    .execute(&mut *tx)  // Use transaction
    .await?;
    
    // COMMIT TRANSACTION
    tx.commit().await?;
    Ok(())
}
```

---

## Phase 8: Implement Document Type Detection (1 hour)

### Issue: Documents uploaded without type assignment

#### Create New File: `/rust/src/services/document_type_detector.rs`
```rust
use uuid::Uuid;
use std::collections::HashMap;

pub struct DocumentTypeDetector;

impl DocumentTypeDetector {
    /// Detect document type based on mime type and content
    pub async fn detect_type(
        mime_type: &str,
        file_name: &str,
        _file_bytes: &[u8],  // For future OCR/AI detection
    ) -> Option<String> {
        // Simple rules for now
        let name_lower = file_name.to_lowercase();
        
        if name_lower.contains("passport") {
            return Some("PASSPORT".to_string());
        } else if name_lower.contains("bank") || name_lower.contains("statement") {
            return Some("BANK_STATEMENT".to_string());
        } else if name_lower.contains("utility") || name_lower.contains("bill") {
            return Some("UTILITY_BILL".to_string());
        } else if name_lower.contains("license") || name_lower.contains("driving") {
            return Some("DRIVERS_LICENSE".to_string());
        } else if name_lower.contains("articles") || name_lower.contains("incorporation") {
            return Some("ARTICLES_OF_INCORPORATION".to_string());
        }
        
        // Default based on mime type
        match mime_type {
            "application/pdf" => Some("GENERIC_PDF".to_string()),
            "image/jpeg" | "image/png" => Some("GENERIC_IMAGE".to_string()),
            _ => None,
        }
    }
}
```

#### Add to document upload flow:
```rust
// In document upload handler
pub async fn handle_document_upload(
    file_bytes: Vec<u8>,
    file_name: String,
    mime_type: String,
    cbu_id: Uuid,
    pool: &PgPool,
) -> Result<Uuid> {
    use sha2::{Sha256, Digest};
    
    // Calculate hash
    let mut hasher = Sha256::new();
    hasher.update(&file_bytes);
    let hash = format!("{:x}", hasher.finalize());
    
    // Detect document type
    let type_code = DocumentTypeDetector::detect_type(
        &mime_type,
        &file_name,
        &file_bytes
    ).await;
    
    // Get type UUID
    let type_id = if let Some(code) = type_code {
        sqlx::query_scalar!(
            r#"
            SELECT type_id
            FROM "ob-poc".document_types
            WHERE type_code = $1
            "#,
            code
        )
        .fetch_optional(pool)
        .await?
    } else {
        None
    };
    
    // Store document WITH type
    let doc_id = sqlx::query_scalar!(
        r#"
        INSERT INTO "ob-poc".document_catalog
        (file_hash_sha256, storage_key, file_size_bytes, 
         mime_type, cbu_id, document_type_id, extraction_status)
        VALUES ($1, $2, $3, $4, $5, $6, 'PENDING')
        ON CONFLICT (file_hash_sha256) 
        DO UPDATE SET 
            updated_at = NOW(),
            cbu_id = COALESCE("ob-poc".document_catalog.cbu_id, $5)
        RETURNING doc_id
        "#,
        hash,
        format!("documents/{}/{}", cbu_id, hash),
        file_bytes.len() as i64,
        mime_type,
        cbu_id,
        type_id
    )
    .fetch_one(pool)
    .await?;
    
    Ok(doc_id)
}
```

---

## Phase 9: Add Missing Database Indexes (15 min)

### Run these SQL commands:
```sql
-- Performance indexes for document catalog
CREATE INDEX IF NOT EXISTS idx_document_catalog_type 
    ON "ob-poc".document_catalog(document_type_id);

CREATE INDEX IF NOT EXISTS idx_document_catalog_type_status 
    ON "ob-poc".document_catalog(document_type_id, extraction_status);

-- Indexes for document attribute mappings
CREATE INDEX IF NOT EXISTS idx_dam_document_type_id 
    ON "ob-poc".document_attribute_mappings(document_type_id);

CREATE INDEX IF NOT EXISTS idx_dam_attribute_uuid 
    ON "ob-poc".document_attribute_mappings(attribute_uuid);

CREATE INDEX IF NOT EXISTS idx_dam_document_type_attribute 
    ON "ob-poc".document_attribute_mappings(document_type_id, attribute_uuid);

-- Composite index for document metadata queries
CREATE INDEX IF NOT EXISTS idx_document_metadata_doc_attr 
    ON "ob-poc".document_metadata(doc_id, attribute_id);
```

---

## Phase 10: Testing & Verification (1 hour)

### Test 1: Compile Check
```bash
cd rust/
cargo check --features database
# Should compile with zero errors
```

### Test 2: Run Integration Tests
```bash
cargo test --features database --lib
# All tests should pass
```

### Test 3: Test Document Extraction Example
```bash
cargo run --example document_extraction_complete_workflow --features database
# Should show extraction workflow
```

### Test 4: Test DSL Integration
```bash
cargo run --example dsl_executor_document_extraction --features database
# Should execute document.extract operation
```

### Test 5: Manual Database Verification
```sql
-- Check document catalog has types assigned
SELECT 
    dc.doc_id,
    dc.cbu_id,
    dt.type_code,
    dc.extraction_status
FROM "ob-poc".document_catalog dc
LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
LIMIT 5;

-- Check extraction mappings
SELECT 
    dt.type_code,
    COUNT(dam.mapping_id) as extractable_attributes
FROM "ob-poc".document_types dt
LEFT JOIN "ob-poc".document_attribute_mappings dam ON dt.type_id = dam.document_type_id
GROUP BY dt.type_code;
```

---

## Common Pitfalls to Avoid

1. **Don't forget the schema prefix**: All tables are in `"ob-poc"` schema
2. **Column name consistency**: Database uses `doc_id` not `document_id`
3. **Check for null document_type_id**: Many operations should skip documents without types
4. **Transaction boundaries**: Always use transactions for dual-write operations
5. **SQLX compile-time checking**: Run `cargo sqlx prepare` after query changes

---

## Expected Outcome

After completing all phases:
- ✅ Documents properly linked to CBUs
- ✅ Document types detected and assigned
- ✅ Extraction service can determine what to extract
- ✅ DSL `document.extract` operations work end-to-end
- ✅ Extracted values flow through ExecutionContext
- ✅ Dual-write storage is transactional
- ✅ All tests pass

---

## Quick Wins (Do First)

If time is limited, prioritize these for maximum impact:
1. **Phase 1**: Fix Rust models (30 min) - Nothing works without this
2. **Phase 2**: Fix queries (30 min) - Removes runtime errors
3. **Phase 5**: Wire handler (1 hour) - Makes DSL operations work

These three fixes alone will make the system ~70% functional.

---

## Verification Commands

After each phase, verify your changes:
```bash
# Quick compile check
cargo check --features database

# Run specific test
cargo test document_extraction --features database

# Check for query errors
cargo sqlx prepare --check

# Format code
cargo fmt

# Lint check
cargo clippy --features database
```

---

## Notes for ZED Agent

- Start with Phase 1-3 (model fixes) as they're foundational
- Phase 5 (wiring handler) is critical for DSL to work
- Phase 8 (type detection) can be simple initially, enhance later
- Use transactions (Phase 7) to prevent partial writes
- Test frequently - each phase should compile independently

Good luck! This plan should take the system from broken to fully functional in 4-6 hours.