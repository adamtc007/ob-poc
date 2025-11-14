# Document-Attribute Integration Action Plan

## 🚀 Quick Start for Zed Claude

Drop the `document_attribute_integration_impl.rs` file into Zed and tell Claude:

> "Implement all components from the document_attribute_integration_impl.rs file systematically, starting with the database migration"

## 📋 Implementation Order (Critical Path)

### Phase 1: Database Foundation (2 hours)
```sql
-- Run this migration FIRST
CREATE TABLE document_metadata ...
CREATE TABLE document_catalog ...
CREATE TABLE attribute_extraction_log ...
```

### Phase 2: Type Safety Refactor (3 hours)
1. **Global Replace**: `String` → `AttributeId` for all attribute IDs
2. **Fix Imports**: Add `use crate::models::AttributeId;`
3. **Update Signatures**: All functions taking attribute IDs
4. **Fix HashMap Types**: `HashMap<String, _>` → `HashMap<AttributeId, _>`

### Phase 3: Document Source (4 hours)
```rust
// Key files to modify:
src/services/document_service.rs  // Add DocumentCatalogSource
src/models/document.rs           // Add DocumentMetadata struct
src/api/handlers/upload.rs       // Trigger extraction on upload
```

### Phase 4: DSL Parser Update (2 hours)
```rust
// In src/dsl/parser.rs add:
fn parse_attribute_reference() // For @attr{uuid} syntax
fn parse_attribute_with_source() // For @attr{uuid}:doc hint
```

### Phase 5: Execution Engine (3 hours)
```rust
// Create new file: src/services/attribute_executor.rs
impl AttributeExecutor {
    async fn resolve_attribute() // Main resolution logic
}
```

## 🔧 Critical Files to Update

| File | Changes | Priority |
|------|---------|----------|
| `src/models/attribute.rs` | Replace String with AttributeId | HIGH |
| `src/db/schema.sql` | Add new tables | HIGH |
| `src/services/dictionary_service.rs` | Update HashMap types | HIGH |
| `src/dsl/parser.rs` | Add @attr{} parsing | MEDIUM |
| `src/api/handlers/document_handler.rs` | Add extraction trigger | MEDIUM |
| `migrations/004_document_metadata.sql` | Create migration | HIGH |

## ✅ Validation Checkpoints

After each phase, verify:

### Phase 1 ✓
```bash
psql -d ob_poc -c "\dt document_*"  # Should show 3 new tables
```

### Phase 2 ✓
```bash
cargo build  # Should compile with AttributeId everywhere
cargo test   # Existing tests should pass
```

### Phase 3 ✓
```bash
# Upload a test document
curl -X POST /api/documents/upload ...
# Check extraction
psql -c "SELECT * FROM document_metadata"  # Should have entries
```

### Phase 4 ✓
```rust
// Test DSL parsing
let dsl = "@attr{550e8400-e29b-41d4-a716-446655440000}";
assert!(parse_attribute_reference(dsl).is_ok());
```

### Phase 5 ✓
```bash
# Full integration test
cargo test test_document_extraction_flow  # Should pass
```

## 🐛 Common Issues & Fixes

### Issue: UUID parse errors
```rust
// Fix: Use proper conversion
AttributeId::from_string(&uuid_str)?  // Not AttributeId(uuid_str)
```

### Issue: HashMap type mismatch
```rust
// Fix: Update all collections
HashMap<AttributeId, Value>  // Not HashMap<String, Value>
```

### Issue: DSL parser not recognizing @attr
```rust
// Fix: Add to main expression parser
alt((
    parse_field,
    parse_attribute_reference,  // ADD THIS
    parse_literal,
))
```

### Issue: Extraction returns null
```rust
// Fix: Check document catalog
INSERT INTO document_catalog (document_type, supported_attributes)
VALUES ('passport', '["attr-uuid-here"]'::jsonb);
```

## 📊 Progress Tracker

Copy this to track your progress:

```markdown
- [ ] Database migration executed
- [ ] AttributeId type replacement complete
- [ ] DocumentCatalogSource implemented
- [ ] DSL parser supports @attr{} syntax
- [ ] AttributeExecutor wired up
- [ ] ExtractionService working
- [ ] Integration tests passing
- [ ] Document upload triggers extraction
- [ ] Attributes resolve from documents
- [ ] Full end-to-end test passing
```

## 🎯 Success Criteria

You're done when:
1. **Type Safety**: No String IDs anywhere for attributes
2. **DSL Works**: `@attr{uuid}` parses and resolves
3. **Documents Extract**: Upload → automatic attribute extraction
4. **Sources Chain**: Document → Form → API fallback works
5. **Tests Pass**: All integration tests green

## 💡 Pro Tips

1. **Test incrementally** - Don't wait until the end
2. **Use transactions** - Wrap DB operations in transactions
3. **Log everything** - Add debug logging for extraction flow
4. **Cache aggressively** - Extracted values should be cached
5. **Monitor performance** - Track extraction times

## 🚨 If You Get Stuck

1. Check the `document_attribute_integration_impl.rs` for detailed implementation
2. Verify database tables exist with `\dt` in psql
3. Enable debug logging: `RUST_LOG=debug cargo run`
4. Run individual test: `cargo test test_name -- --nocapture`

---

**Estimated Total Time**: 14-16 hours of focused implementation
**Result**: 100% functional document-driven attribute system
