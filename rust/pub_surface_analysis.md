# Public API Surface Analysis Report

Generated: Wed Nov 12 16:39:09 GMT 2025

## Executive Summary

- **Total pub items found**: 1132
- **Conversion candidates**: 613 (can be made pub(crate))
- **True public API**: 519 (needs documentation)
- **Potential reduction**: 613/1132 (54.2%)

## Strategy Impact

By converting internal-use items to `pub(crate)`, we:
- Reduce semantic ambiguity for AI agents
- Eliminate documentation burden for internal APIs
- Create clear boundaries between public contracts and implementation details
- Focus documentation efforts on true public APIs

## Conversion Candidates (pub → pub(crate))

### UnifiedAgenticCrudDemo (struct)
- **File**: examples/unified_agentic_crud_demo.rs:28
- **Reason**: internal_use
- **Internal usage**: 3 locations
```rust
pub struct UnifiedAgenticCrudDemo {
```

### DocumentProcessingError (enum)
- **File**: src/error_enhanced.rs:50
- **Reason**: internal_use
- **Internal usage**: 5 locations
```rust
pub enum DocumentProcessingError {
```

### EnhancedError (enum)
- **File**: src/error_enhanced.rs:148
- **Reason**: internal_use
- **Internal usage**: 2 locations
```rust
pub enum EnhancedError {
```

### DatabaseResult (type)
- **File**: src/error_enhanced.rs:196
- **Reason**: internal_use
- **Internal usage**: 1 locations
```rust
pub type DatabaseResult<T> = Result<T, DatabaseError>;
```

### EnhancedResult (type)
- **File**: src/error_enhanced.rs:199
- **Reason**: internal_use
- **Internal usage**: 1 locations
```rust
pub type EnhancedResult<T> = Result<T, EnhancedError>;
```

### connection_failed (fn)
- **File**: src/error_enhanced.rs:203
- **Reason**: internal_use
- **Internal usage**: 2 locations
```rust
pub fn connection_failed(message: impl Into<String>) -> Self {
```

### query_failed (fn)
- **File**: src/error_enhanced.rs:209
- **Reason**: unused
```rust
pub fn query_failed(message: impl Into<String>) -> Self {
```

### not_found (fn)
- **File**: src/error_enhanced.rs:215
- **Reason**: internal_use
- **Internal usage**: 1 locations
```rust
pub fn not_found(message: impl Into<String>) -> Self {
```

### constraint_violation (fn)
- **File**: src/error_enhanced.rs:221
- **Reason**: internal_use
- **Internal usage**: 4 locations
```rust
pub fn constraint_violation(message: impl Into<String>) -> Self {
```

### invalid_document_type (fn)
- **File**: src/error_enhanced.rs:229
- **Reason**: internal_use
- **Internal usage**: 1 locations
```rust
pub fn invalid_document_type(document_type: impl Into<String>) -> Self {
```

### extraction_failed (fn)
- **File**: src/error_enhanced.rs:235
- **Reason**: unused
```rust
pub fn extraction_failed(attribute: impl Into<String>, reason: impl Into<String>) -> Self {
```

### missing_attribute (fn)
- **File**: src/error_enhanced.rs:242
- **Reason**: internal_use
- **Internal usage**: 5 locations
```rust
pub fn missing_attribute(attribute: impl Into<String>) -> Self {
```

### ai_processing_failed (fn)
- **File**: src/error_enhanced.rs:248
- **Reason**: unused
```rust
pub fn ai_processing_failed(reason: impl Into<String>) -> Self {
```

### compilation_failed (fn)
- **File**: src/error_enhanced.rs:256
- **Reason**: internal_use
- **Internal usage**: 1 locations
```rust
pub fn compilation_failed(reason: impl Into<String>) -> Self {
```

### instance_not_found (fn)
- **File**: src/error_enhanced.rs:262
- **Reason**: unused
```rust
pub fn instance_not_found(instance_id: impl Into<String>) -> Self {
```

### invalid_syntax (fn)
- **File**: src/error_enhanced.rs:268
- **Reason**: unused
```rust
pub fn invalid_syntax(message: impl Into<String>) -> Self {
```

### workflow_execution_failed (fn)
- **File**: src/error_enhanced.rs:274
- **Reason**: unused
```rust
pub fn workflow_execution_failed(stage: impl Into<String>) -> Self {
```

### DSLResult (type)
- **File**: src/error.rs:294
- **Reason**: internal_use
- **Internal usage**: 5 locations
```rust
pub type DSLResult<T> = Result<T, DSLError>;
```

### GrammarResult (type)
- **File**: src/error.rs:296
- **Reason**: internal_use
- **Internal usage**: 1 locations
```rust
pub type GrammarResult<T> = Result<T, GrammarError>;
```

### VocabularyResult (type)
- **File**: src/error.rs:297
- **Reason**: internal_use
- **Internal usage**: 1 locations
```rust
pub type VocabularyResult<T> = Result<T, VocabularyError>;
```

... and 593 more candidates

## True Public API (needs documentation)

### DatabaseError (enum)
- **File**: src/error_enhanced.rs:10
- **Cross-crate usage**: 3 locations
```rust
// TODO: Add documentation
pub enum DatabaseError {
```

### DslManagerError (enum)
- **File**: src/error_enhanced.rs:84
- **Cross-crate usage**: 3 locations
```rust
// TODO: Add documentation
pub enum DslManagerError {
```

### DocumentResult (type)
- **File**: src/error_enhanced.rs:197
- **Cross-crate usage**: 3 locations
```rust
// TODO: Add documentation
pub type DocumentResult<T> = Result<T, DocumentProcessingError>;
```

### DslManagerResult (type)
- **File**: src/error_enhanced.rs:198
- **Cross-crate usage**: 3 locations
```rust
// TODO: Add documentation
pub type DslManagerResult<T> = Result<T, DslManagerError>;
```

### DSLError (enum)
- **File**: src/error.rs:13
- **Cross-crate usage**: 3 locations
```rust
// TODO: Add documentation
pub enum DSLError {
```

### ParseError (enum)
- **File**: src/error.rs:124
- **Cross-crate usage**: 3 locations
```rust
// TODO: Add documentation
pub enum ParseError {
```

### GrammarError (enum)
- **File**: src/error.rs:168
- **Cross-crate usage**: 3 locations
```rust
// TODO: Add documentation
pub enum GrammarError {
```

### VocabularyError (enum)
- **File**: src/error.rs:190
- **Cross-crate usage**: 3 locations
```rust
// TODO: Add documentation
pub enum VocabularyError {
```

### ValidationError (enum)
- **File**: src/error.rs:232
- **Cross-crate usage**: 3 locations
```rust
// TODO: Add documentation
pub enum ValidationError {
```

### RuntimeError (enum)
- **File**: src/error.rs:270
- **Cross-crate usage**: 3 locations
```rust
// TODO: Add documentation
pub enum RuntimeError {
```

... and 509 more public APIs

## Next Steps

1. Review conversion candidates above
2. Run: `python3 scripts/reduce_pub_surface.py --apply`
3. Test compilation: `cargo +1.91 check --workspace`
4. Focus documentation efforts on remaining true public APIs
5. Re-enable `#![warn(missing_docs)]` in lib.rs

