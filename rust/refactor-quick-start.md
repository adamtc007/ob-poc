# Document-Attribute Refactor: Quick Start Guide

## 🎯 Goal
Replace ALL mock document extraction with real database-driven, type-aware extraction that integrates with DSL operations.

## 🔑 Key Changes

### 1. Database: Add Mapping Table
```sql
-- This is what's MISSING and MUST be created
CREATE TABLE "ob-poc".document_attribute_mappings (
    document_type_id UUID,  -- Which document type
    attribute_uuid UUID,    -- Which attribute it provides
    extraction_method VARCHAR(50),  -- How to extract (OCR, MRZ, etc.)
    confidence_threshold NUMERIC(3,2),
    is_required BOOLEAN
);
```

### 2. Replace Mock Extraction
**DELETE**: `/rust/src/domains/attributes/sources/document_extraction.rs` (all mocks)
**CREATE**: Real extraction using database mappings

### 3. Fix ExecutionContext
```rust
// Add these fields
pub struct ExecutionContext {
    pub cbu_id: Uuid,     // MISSING - breaks everything
    pub entity_id: Uuid,  // MISSING - needed for storage
    pub current_document_id: Option<Uuid>, // For document ops
    // ... existing fields
}
```

### 4. Connect DSL Operations
```lisp
;; This DSL should trigger REAL extraction
(document.extract @doc{passport-123} 
    [@attr.identity.first_name @attr.identity.passport_number])

;; Extracted values should be immediately available
(entity.create :first_name @attr.identity.first_name)  ; Uses extracted value
```

## 📁 Critical Files to Change

### Priority 1: Database Schema
- `/sql/migrations/001_document_attribute_mappings.sql` - CREATE
- `/sql/migrations/002_seed_document_types.sql` - CREATE

### Priority 2: Core Models
- `/rust/src/models/document_type_models.rs` - CREATE
- `/rust/src/models/document_models.rs` - UPDATE (add type_id)
- `/rust/src/domains/attributes/execution_context.rs` - UPDATE (add fields)

### Priority 3: Repositories
- `/rust/src/database/document_type_repository.rs` - CREATE
- `/rust/src/database/attribute_repository.rs` - UPDATE

### Priority 4: Services
- `/rust/src/services/document_extraction_service.rs` - REPLACE (remove mocks)
- `/rust/src/domains/attributes/sources/document_extraction.rs` - REPLACE

### Priority 5: DSL Integration
- `/rust/src/execution/dsl_executor.rs` - UPDATE (add document ops)
- `/rust/src/parser/statements.rs` - UPDATE (parse document ops)

## ⚡ Quick Test

After implementation, this should work:
```bash
# 1. Run migrations
psql -d your_db -f sql/migrations/001_document_attribute_mappings.sql

# 2. Test extraction
cargo test test_end_to_end_document_extraction

# 3. Test DSL integration
cargo test test_dsl_document_operations
```

## 🚫 What NOT to Do
- Don't create more mock data
- Don't bypass the database
- Don't ignore document types
- Don't skip confidence thresholds
- Don't hardcode attribute lists

## ✅ Success Criteria
```rust
// This query should return REAL mappings
SELECT * FROM "ob-poc".document_attribute_mappings 
WHERE document_type_id = (SELECT type_id FROM document_types WHERE type_code = 'PASSPORT');

// This should populate document_metadata
let extracted = service.extract_attributes_from_document(doc_id, entity_id).await?;
assert!(extracted.len() > 0);

// This DSL should work end-to-end
(document.extract @doc{passport} [@attr.identity.first_name])
(assert @attr.identity.first_name = "John")  // Real extracted value
```

## 🏗️ Implementation Order
1. **Day 1**: Create database tables and seed data
2. **Day 2**: Create models and repositories
3. **Day 3**: Replace mock extraction service
4. **Day 4**: Update ExecutionContext and DSL executor
5. **Day 5**: Integration testing

## 🔥 Most Common Mistakes to Avoid
1. **Forgetting to add document_type_id** to uploaded documents
2. **Not checking if document type supports attribute** before extraction
3. **Ignoring confidence thresholds** in extraction
4. **Not binding extracted values** to ExecutionContext
5. **Missing the DSL → document_metadata connection**

## 💡 Pro Tips
- Start with passport document type (simplest)
- Use AWS Textract or Azure Form Recognizer for real extraction
- Cache document type mappings for performance
- Log all extraction attempts for debugging
- Test with real PDF files, not just strings

## 📊 Before vs After

### Before (Current - BROKEN)
```
Document Upload → Mock Extraction → Hardcoded Values → No Persistence
                    ↓
              "John" (always)
```

### After (Target - WORKING)
```
Document Upload → Type Detection → Mapping Lookup → Real Extraction → Database
       ↓              ↓                ↓                ↓              ↓
   passport.pdf    PASSPORT     [first_name,...]    OCR/MRZ      document_metadata
```

---

**Remember**: The goal is ZERO mock data. Every extraction must:
1. Check document type
2. Look up mappings
3. Extract via real engine
4. Store in database
5. Be available in DSL

Full plan: `/mnt/user-data/outputs/document-attribute-refactor-plan.md`
