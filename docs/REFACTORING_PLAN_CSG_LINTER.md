# Refactoring Plan: Context-Sensitive Grammar (CSG) Linter Layer

## Overview

This plan introduces a **CSG Linter** as a new validation layer between the NOM parser (AST) and the semantic validator. The CSG linter enforces context-sensitive rules that cannot be expressed in context-free grammar, ensuring that DSL programs are not just syntactically valid but semantically coherent within their business domain context.

## Current Pipeline

```
DSL Source → Parser (NOM) → AST → SemanticValidator → Executor
                  ↓              ↓
              Syntax Valid   Refs Exist in DB
```

## Target Pipeline

```
DSL Source → Parser (NOM) → AST → CSG Linter → SemanticValidator → Executor
                  ↓              ↓                    ↓
              Syntax Valid   Context Valid       Refs Exist in DB
                             (business rules)
```

## What Exists vs What's Needed

### ✅ EXISTING Components

| Component | Location | Purpose |
|-----------|----------|---------|
| NOM Parser | `dsl_v2/parser.rs` | S-expression syntax → AST |
| AST Types | `dsl_v2/ast.rs` | `Program`, `Statement`, `VerbCall`, `Value` |
| VerbDef Registry | `dsl_v2/verbs.rs` | Static verb definitions with required/optional args |
| VerbSchema | `dsl_v2/verb_schema.rs` | Typed argument definitions with `ArgType` enum |
| Validation Types | `dsl_v2/validation.rs` | `Diagnostic`, `DiagnosticCode`, `ValidationResult` |
| RefTypeResolver | `dsl_v2/ref_resolver.rs` | DB existence validation with fuzzy matching |
| SemanticValidator | `dsl_v2/semantic_validator.rs` | AST walker that validates args against DB |
| Intent Layer | `dsl_v2/semantic_intent.rs` | Agent JSON → DSL generation pipeline |
| Design Doc | `docs/design-contextual-validation-rules.md` | Contextual validation proposal (draft) |

### ❌ MISSING Components

| Component | Purpose | Priority |
|-----------|---------|----------|
| CSG Linter Module | Orchestrates context-sensitive validation | P0 |
| Context Builder | Builds validation context from AST + DB state | P0 |
| Rule Engine | Evaluates applicability rules | P0 |
| Applicability Schema | JSON schema for metadata rules | P0 |
| DB Schema Migration | Add `applicability` JSONB columns | P1 |
| Seed Data Update | Populate applicability rules | P1 |
| CSG Error Codes | New `DiagnosticCode` variants | P0 |
| Cross-Reference Validator | Validates symbol references across statements | P1 |
| State Machine Validator | Validates operation ordering/preconditions | P2 |

---

## Phase 1: Core CSG Linter Infrastructure

### 1.1 New Module: `dsl_v2/csg_linter.rs`

```rust
//! Context-Sensitive Grammar Linter
//!
//! Validates DSL programs against business rules that depend on runtime context:
//! - Entity type constraints (passport → person only)
//! - Jurisdiction constraints (W-9 → US only)
//! - Cross-statement reference validity
//! - Operation sequencing rules
//!
//! Pipeline position: After NOM parsing, before SemanticValidator

use crate::dsl_v2::ast::{Program, Statement, VerbCall, Value};
use crate::dsl_v2::validation::{
    Diagnostic, DiagnosticBuilder, DiagnosticCode, Severity, SourceSpan,
    ValidationContext, ValidationResult,
};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// Result of CSG linting
pub struct LintResult {
    /// The original AST (passed through if valid)
    pub ast: Program,
    /// Any diagnostics generated
    pub diagnostics: Vec<Diagnostic>,
    /// Inferred context for downstream validation
    pub inferred_context: InferredContext,
}

/// Context inferred from AST analysis
#[derive(Debug, Default)]
pub struct InferredContext {
    /// Symbol bindings with their inferred types
    pub symbols: HashMap<String, SymbolType>,
    /// Operations that create entities (for forward reference validation)
    pub entity_creates: Vec<EntityCreate>,
    /// Operations that reference entities
    pub entity_refs: Vec<EntityRef>,
}

#[derive(Debug, Clone)]
pub struct SymbolType {
    pub name: String,
    pub entity_type: Option<String>,  // e.g., "LIMITED_COMPANY", "PROPER_PERSON"
    pub domain: String,               // e.g., "cbu", "entity", "document"
    pub defined_at: SourceSpan,
}

#[derive(Debug)]
pub struct EntityCreate {
    pub symbol: Option<String>,
    pub entity_type: String,
    pub span: SourceSpan,
}

#[derive(Debug)]
pub struct EntityRef {
    pub symbol: String,
    pub expected_type: Option<String>,
    pub span: SourceSpan,
}

/// The CSG Linter
pub struct CsgLinter {
    pool: PgPool,
    rules: ApplicabilityRules,
}

impl CsgLinter {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            rules: ApplicabilityRules::default(),
        }
    }

    /// Load applicability rules from database
    pub async fn load_rules(&mut self) -> Result<(), String> {
        self.rules = ApplicabilityRules::load(&self.pool).await?;
        Ok(())
    }

    /// Lint a parsed AST
    pub async fn lint(
        &self,
        ast: Program,
        context: &ValidationContext,
    ) -> LintResult {
        let mut diagnostics = DiagnosticBuilder::new();
        let mut inferred = InferredContext::default();

        // Pass 1: Build symbol table and infer types
        for statement in &ast.statements {
            if let Statement::VerbCall(vc) = statement {
                self.analyze_statement(vc, &mut inferred, &mut diagnostics);
            }
        }

        // Pass 2: Validate cross-references
        self.validate_references(&inferred, &mut diagnostics);

        // Pass 3: Validate contextual applicability
        for statement in &ast.statements {
            if let Statement::VerbCall(vc) = statement {
                self.validate_applicability(vc, &inferred, context, &mut diagnostics).await;
            }
        }

        LintResult {
            ast,
            diagnostics: diagnostics.build(),
            inferred_context: inferred,
        }
    }

    /// Pass 1: Analyze statement for symbol definitions and type inference
    fn analyze_statement(
        &self,
        vc: &VerbCall,
        inferred: &mut InferredContext,
        _diagnostics: &mut DiagnosticBuilder,
    ) {
        // Extract symbol binding
        if let Some(ref binding) = vc.as_binding {
            let entity_type = self.infer_entity_type(vc);
            inferred.symbols.insert(
                binding.clone(),
                SymbolType {
                    name: binding.clone(),
                    entity_type,
                    domain: vc.domain.clone(),
                    defined_at: span_to_source_span(&vc.span),
                },
            );
        }

        // Track entity creates
        if vc.domain == "entity" && vc.verb.starts_with("create") {
            if let Some(ent_type) = self.extract_entity_type_arg(vc) {
                inferred.entity_creates.push(EntityCreate {
                    symbol: vc.as_binding.clone(),
                    entity_type: ent_type,
                    span: span_to_source_span(&vc.span),
                });
            }
        }

        // Track entity references
        for arg in &vc.arguments {
            if let Value::Reference(ref name) = arg.value {
                inferred.entity_refs.push(EntityRef {
                    symbol: name.clone(),
                    expected_type: self.expected_entity_type_for_arg(&vc.domain, &vc.verb, &arg.key.canonical()),
                    span: span_to_source_span(&arg.value_span),
                });
            }
        }
    }

    /// Pass 2: Validate all cross-references
    fn validate_references(
        &self,
        inferred: &InferredContext,
        diagnostics: &mut DiagnosticBuilder,
    ) {
        for entity_ref in &inferred.entity_refs {
            // Check symbol is defined
            if !inferred.symbols.contains_key(&entity_ref.symbol) {
                diagnostics.error(
                    DiagnosticCode::UndefinedSymbol,
                    entity_ref.span,
                    format!("undefined symbol '@{}'", entity_ref.symbol),
                );
                continue;
            }

            // Check type compatibility
            if let Some(ref expected) = entity_ref.expected_type {
                if let Some(symbol_info) = inferred.symbols.get(&entity_ref.symbol) {
                    if let Some(ref actual) = symbol_info.entity_type {
                        if !self.type_compatible(expected, actual) {
                            diagnostics.error(
                                DiagnosticCode::TypeMismatch,
                                entity_ref.span,
                                format!(
                                    "type mismatch: expected {}, but '@{}' has type {}",
                                    expected, entity_ref.symbol, actual
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    /// Pass 3: Validate contextual applicability rules
    async fn validate_applicability(
        &self,
        vc: &VerbCall,
        inferred: &InferredContext,
        context: &ValidationContext,
        diagnostics: &mut DiagnosticBuilder,
    ) {
        // Document type applicability
        if vc.domain == "document" {
            self.validate_document_applicability(vc, inferred, context, diagnostics).await;
        }

        // Attribute applicability (future)
        // Jurisdiction constraints (future)
    }

    async fn validate_document_applicability(
        &self,
        vc: &VerbCall,
        inferred: &InferredContext,
        _context: &ValidationContext,
        diagnostics: &mut DiagnosticBuilder,
    ) {
        // Find document-type argument
        let doc_type = vc.arguments.iter()
            .find(|a| a.key.canonical() == "document-type")
            .and_then(|a| a.value.as_string());

        // Find entity-id argument
        let entity_ref = vc.arguments.iter()
            .find(|a| a.key.canonical() == "entity-id")
            .and_then(|a| a.value.as_reference());

        if let (Some(doc_type), Some(entity_sym)) = (doc_type, entity_ref) {
            // Get entity type from symbol table
            if let Some(symbol_info) = inferred.symbols.get(entity_sym) {
                if let Some(ref entity_type) = symbol_info.entity_type {
                    // Check applicability rule
                    if let Some(rule) = self.rules.document_rules.get(doc_type) {
                        if !rule.applies_to_entity_type(entity_type) {
                            let span = vc.arguments.iter()
                                .find(|a| a.key.canonical() == "document-type")
                                .map(|a| span_to_source_span(&a.value_span))
                                .unwrap_or_default();

                            diagnostics.error(
                                DiagnosticCode::DocumentNotApplicableToEntityType,
                                span,
                                format!(
                                    "document type '{}' is not applicable to entity type '{}'",
                                    doc_type, entity_type
                                ),
                            ).suggest(
                                "valid document types for this entity",
                                self.rules.suggest_documents_for_entity(entity_type).join(", "),
                                0.8,
                            );
                        }
                    }
                }
            }
        }
    }

    // Helper methods...
    fn infer_entity_type(&self, vc: &VerbCall) -> Option<String> {
        // For entity.create-* verbs, infer from verb name or :type arg
        if vc.domain == "entity" {
            if vc.verb.contains("limited-company") {
                return Some("LIMITED_COMPANY".to_string());
            }
            if vc.verb.contains("proper-person") || vc.verb.contains("natural-person") {
                return Some("PROPER_PERSON".to_string());
            }
            // Check explicit :type argument
            return self.extract_entity_type_arg(vc);
        }
        None
    }

    fn extract_entity_type_arg(&self, vc: &VerbCall) -> Option<String> {
        vc.arguments.iter()
            .find(|a| a.key.canonical() == "type" || a.key.canonical() == "entity-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase().replace('-', "_"))
    }

    fn expected_entity_type_for_arg(&self, _domain: &str, _verb: &str, arg: &str) -> Option<String> {
        // Document linking typically expects the entity to be appropriate for the doc
        // This is where we encode business rules about which args expect which entity types
        match arg {
            "entity-id" => None, // Could be any entity
            "person-id" => Some("PROPER_PERSON".to_string()),
            "company-id" => Some("LIMITED_COMPANY".to_string()),
            _ => None,
        }
    }

    fn type_compatible(&self, expected: &str, actual: &str) -> bool {
        // Support wildcards: "LIMITED_COMPANY_*" matches "LIMITED_COMPANY_PRIVATE"
        if expected.ends_with('*') {
            let prefix = &expected[..expected.len()-1];
            return actual.starts_with(prefix);
        }
        expected == actual
    }
}

fn span_to_source_span(span: &crate::dsl_v2::ast::Span) -> SourceSpan {
    SourceSpan {
        line: 0, // Would need source text to compute
        column: span.start as u32,
        offset: span.start as u32,
        length: (span.end - span.start) as u32,
    }
}
```

### 1.2 New Module: `dsl_v2/applicability_rules.rs`

```rust
//! Applicability Rules - Business rules for context-sensitive validation
//!
//! Rules are loaded from database metadata columns and cached for validation.

use sqlx::PgPool;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// All applicability rules loaded from DB
#[derive(Debug, Default)]
pub struct ApplicabilityRules {
    pub document_rules: HashMap<String, DocumentApplicability>,
    pub attribute_rules: HashMap<String, AttributeApplicability>,
}

/// Applicability rules for a document type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentApplicability {
    /// Entity types this document applies to (supports wildcards)
    #[serde(default)]
    pub entity_types: Vec<String>,

    /// Jurisdictions this document is valid in
    #[serde(default)]
    pub jurisdictions: Vec<String>,

    /// Client types this document applies to
    #[serde(default)]
    pub client_types: Vec<String>,

    /// Category for grouping
    #[serde(default)]
    pub category: Option<String>,
}

impl DocumentApplicability {
    pub fn applies_to_entity_type(&self, entity_type: &str) -> bool {
        if self.entity_types.is_empty() {
            return true; // No restriction = applies to all
        }
        self.entity_types.iter().any(|allowed| {
            if allowed.ends_with('*') {
                let prefix = &allowed[..allowed.len()-1];
                entity_type.starts_with(prefix)
            } else {
                allowed == entity_type
            }
        })
    }

    pub fn applies_to_jurisdiction(&self, jurisdiction: &str) -> bool {
        if self.jurisdictions.is_empty() {
            return true;
        }
        self.jurisdictions.iter().any(|j| j == jurisdiction)
    }
}

/// Applicability rules for an attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeApplicability {
    #[serde(default)]
    pub entity_types: Vec<String>,

    #[serde(default)]
    pub required_for: Vec<String>,

    #[serde(default)]
    pub source_documents: Vec<String>,
}

impl ApplicabilityRules {
    /// Load rules from database
    pub async fn load(pool: &PgPool) -> Result<Self, String> {
        let mut rules = Self::default();

        // Load document type rules
        let doc_rows = sqlx::query!(
            r#"SELECT type_code, applicability
               FROM "ob-poc".document_types
               WHERE applicability IS NOT NULL"#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load document rules: {}", e))?;

        for row in doc_rows {
            if let Some(applicability) = row.applicability {
                if let Ok(rule) = serde_json::from_value::<DocumentApplicability>(applicability) {
                    rules.document_rules.insert(row.type_code, rule);
                }
            }
        }

        // Load attribute rules (similar pattern)
        let attr_rows = sqlx::query!(
            r#"SELECT semantic_id, applicability
               FROM "ob-poc".attribute_registry
               WHERE applicability IS NOT NULL"#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load attribute rules: {}", e))?;

        for row in attr_rows {
            if let Some(applicability) = row.applicability {
                if let Ok(rule) = serde_json::from_value::<AttributeApplicability>(applicability) {
                    rules.attribute_rules.insert(row.semantic_id, rule);
                }
            }
        }

        Ok(rules)
    }

    /// Suggest valid documents for an entity type
    pub fn suggest_documents_for_entity(&self, entity_type: &str) -> Vec<String> {
        self.document_rules.iter()
            .filter(|(_, rule)| rule.applies_to_entity_type(entity_type))
            .map(|(code, _)| code.clone())
            .collect()
    }
}
```

### 1.3 New Diagnostic Codes

Add to `dsl_v2/validation.rs`:

```rust
pub enum DiagnosticCode {
    // ... existing codes ...

    // CSG Error Codes (C0xx)
    /// Document type not applicable to entity type
    DocumentNotApplicableToEntityType,   // C001
    /// Document type not applicable to jurisdiction
    DocumentNotApplicableToJurisdiction, // C002
    /// Document type not applicable to client type
    DocumentNotApplicableToClientType,   // C003
    /// Attribute not applicable to entity type
    AttributeNotApplicableToEntityType,  // C004
    /// Operation requires prerequisite operation
    MissingPrerequisiteOperation,        // C005
    /// Symbol type mismatch in cross-reference
    SymbolTypeMismatch,                  // C006
    /// Forward reference to undefined symbol
    ForwardReferenceError,               // C007
}

impl DiagnosticCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            // ... existing ...
            DiagnosticCode::DocumentNotApplicableToEntityType => "C001",
            DiagnosticCode::DocumentNotApplicableToJurisdiction => "C002",
            DiagnosticCode::DocumentNotApplicableToClientType => "C003",
            DiagnosticCode::AttributeNotApplicableToEntityType => "C004",
            DiagnosticCode::MissingPrerequisiteOperation => "C005",
            DiagnosticCode::SymbolTypeMismatch => "C006",
            DiagnosticCode::ForwardReferenceError => "C007",
        }
    }
}
```

---

## Phase 2: Database Schema & Seed Data

### 2.1 Migration: Add Applicability Columns

```sql
-- File: sql/migrations/add_applicability_columns.sql

BEGIN;

-- Add applicability column to document_types
ALTER TABLE "ob-poc".document_types
ADD COLUMN IF NOT EXISTS applicability JSONB DEFAULT '{}';

-- Add applicability column to attribute_registry
ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN IF NOT EXISTS applicability JSONB DEFAULT '{}';

-- Index for efficient querying
CREATE INDEX IF NOT EXISTS idx_document_types_applicability
ON "ob-poc".document_types USING GIN (applicability);

CREATE INDEX IF NOT EXISTS idx_attribute_registry_applicability
ON "ob-poc".attribute_registry USING GIN (applicability);

COMMIT;
```

### 2.2 Seed Data: Applicability Rules

```sql
-- File: sql/seeds/applicability_rules.sql

BEGIN;

-- Person-only document types
UPDATE "ob-poc".document_types
SET applicability = jsonb_build_object(
    'entity_types', jsonb_build_array('PROPER_PERSON_NATURAL', 'PROPER_PERSON_BENEFICIAL_OWNER'),
    'category', 'IDENTITY'
)
WHERE type_code IN ('PASSPORT', 'DRIVERS_LICENSE', 'NATIONAL_ID');

-- Corporate-only document types
UPDATE "ob-poc".document_types
SET applicability = jsonb_build_object(
    'entity_types', jsonb_build_array('LIMITED_COMPANY', 'LIMITED_COMPANY_PRIVATE', 'LIMITED_COMPANY_PUBLIC'),
    'category', 'FORMATION'
)
WHERE type_code IN ('CERT_INCORPORATION', 'ARTICLES_ASSOC');

-- Partnership-only document types
UPDATE "ob-poc".document_types
SET applicability = jsonb_build_object(
    'entity_types', jsonb_build_array('PARTNERSHIP', 'PARTNERSHIP_LIMITED', 'PARTNERSHIP_LLP'),
    'category', 'FORMATION'
)
WHERE type_code = 'PARTNERSHIP_AGREEMENT';

-- Trust-only document types
UPDATE "ob-poc".document_types
SET applicability = jsonb_build_object(
    'entity_types', jsonb_build_array('TRUST', 'TRUST_DISCRETIONARY', 'TRUST_FIXED_INTEREST'),
    'category', 'FORMATION'
)
WHERE type_code = 'TRUST_DEED';

-- Universal document types (no entity restriction)
UPDATE "ob-poc".document_types
SET applicability = jsonb_build_object(
    'category', 'ADDRESS'
)
WHERE type_code = 'PROOF_ADDRESS';

-- Financial documents (corporate + trust)
UPDATE "ob-poc".document_types
SET applicability = jsonb_build_object(
    'entity_types', jsonb_build_array('LIMITED_COMPANY*', 'TRUST*', 'PARTNERSHIP*'),
    'category', 'FINANCIAL'
)
WHERE type_code = 'FINANCIAL_STATEMENTS';

COMMIT;
```

---

## Phase 3: Integration

### 3.1 Update Module Structure

Modify `dsl_v2/mod.rs`:

```rust
pub mod applicability_rules;
pub mod csg_linter;
// ... existing modules ...

pub use csg_linter::{CsgLinter, LintResult};
pub use applicability_rules::ApplicabilityRules;
```

### 3.2 Integrate into SemanticValidator

Update `dsl_v2/semantic_validator.rs`:

```rust
use crate::dsl_v2::csg_linter::CsgLinter;

pub struct SemanticValidator {
    resolver: RefTypeResolver,
    csg_linter: CsgLinter,  // NEW
}

impl SemanticValidator {
    pub fn new(pool: PgPool) -> Self {
        Self {
            resolver: RefTypeResolver::new(pool.clone()),
            csg_linter: CsgLinter::new(pool),
        }
    }

    /// Initialize (load rules from DB)
    pub async fn initialize(&mut self) -> Result<(), String> {
        self.csg_linter.load_rules().await
    }

    pub async fn validate(&mut self, request: &ValidationRequest) -> ValidationResult {
        // 1. Parse
        let program = match parse_program(&request.source) {
            Ok(p) => p,
            Err(e) => return ValidationResult::Err(vec![/* parse error */]),
        };

        // 2. CSG Linting (NEW)
        let lint_result = self.csg_linter.lint(program, &request.context).await;
        if !lint_result.diagnostics.is_empty() {
            // Check for errors (warnings can continue)
            let errors: Vec<_> = lint_result.diagnostics.iter()
                .filter(|d| d.severity == Severity::Error)
                .cloned()
                .collect();
            if !errors.is_empty() {
                return ValidationResult::Err(errors);
            }
        }

        // 3. Existence validation (existing)
        // ... use lint_result.ast and lint_result.inferred_context
    }
}
```

---

## Phase 4: Testing Strategy

### 4.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_applicability_wildcard() {
        let rule = DocumentApplicability {
            entity_types: vec!["LIMITED_COMPANY_*".to_string()],
            jurisdictions: vec![],
            client_types: vec![],
            category: None,
        };

        assert!(rule.applies_to_entity_type("LIMITED_COMPANY_PRIVATE"));
        assert!(rule.applies_to_entity_type("LIMITED_COMPANY_PUBLIC"));
        assert!(!rule.applies_to_entity_type("PROPER_PERSON_NATURAL"));
    }

    #[test]
    fn test_passport_not_for_company() {
        let rules = ApplicabilityRules {
            document_rules: HashMap::from([(
                "PASSPORT".to_string(),
                DocumentApplicability {
                    entity_types: vec!["PROPER_PERSON_*".to_string()],
                    jurisdictions: vec![],
                    client_types: vec![],
                    category: Some("IDENTITY".to_string()),
                },
            )]),
            ..Default::default()
        };

        let rule = rules.document_rules.get("PASSPORT").unwrap();
        assert!(!rule.applies_to_entity_type("LIMITED_COMPANY"));
        assert!(rule.applies_to_entity_type("PROPER_PERSON_NATURAL"));
    }
}
```

### 4.2 Integration Test: Full Pipeline

```rust
#[tokio::test]
async fn test_csg_rejects_passport_for_company() {
    let pool = get_test_pool().await;
    let mut validator = SemanticValidator::new(pool);
    validator.initialize().await.unwrap();

    let dsl = r#"
        (entity.create-limited-company :name "Acme Corp" :as @company)
        (document.catalog :document-type "PASSPORT" :entity-id @company)
    "#;

    let result = validator.validate(&ValidationRequest {
        source: dsl.to_string(),
        context: ValidationContext::default(),
    }).await;

    assert!(result.is_err());
    let errors = result.diagnostics();
    assert!(errors.iter().any(|d| d.code == DiagnosticCode::DocumentNotApplicableToEntityType));
}
```

---

## Implementation Order

1. **Week 1: Core Infrastructure**
   - [ ] Create `csg_linter.rs` with basic structure
   - [ ] Create `applicability_rules.rs` with types
   - [ ] Add new `DiagnosticCode` variants
   - [ ] Unit tests for rule matching

2. **Week 2: Database Integration**
   - [ ] Run migration to add columns
   - [ ] Populate seed data
   - [ ] Implement rule loading from DB
   - [ ] Integration tests with real DB

3. **Week 3: Full Integration**
   - [ ] Integrate into `SemanticValidator`
   - [ ] Update error formatting
   - [ ] End-to-end pipeline tests
   - [ ] Update documentation

4. **Week 4: Polish & Extensions**
   - [ ] Add jurisdiction constraints
   - [ ] Add attribute applicability
   - [ ] Performance optimization (caching)
   - [ ] LSP integration for real-time linting

---

## Error Message Examples

### C001: Document Not Applicable to Entity Type

```
error[C001]: document type not applicable to entity type
 --> input:3:37
  |
3 | (document.catalog :document-type "PASSPORT" :entity-id @company)
  |                                  ^^^^^^^^^^
  |
  = note: "PASSPORT" is only valid for: PROPER_PERSON_NATURAL, PROPER_PERSON_BENEFICIAL_OWNER
  = note: entity @company has type: LIMITED_COMPANY_PRIVATE
  = help: valid document types for LIMITED_COMPANY_PRIVATE:
          - CERT_INCORPORATION
          - ARTICLES_ASSOC
          - FINANCIAL_STATEMENTS
```

### C006: Symbol Type Mismatch

```
error[C006]: symbol type mismatch
 --> input:5:42
  |
5 | (entity.set-attribute :person-id @company :attribute "date_of_birth" ...)
  |                                  ^^^^^^^^
  |
  = note: :person-id expects PROPER_PERSON, but @company has type LIMITED_COMPANY
```

---

## Open Questions for Review

1. **Rule Precedence**: If multiple rules apply (entity type AND jurisdiction), should they AND or OR?
   
2. **Dynamic Rule Updates**: Should rules be refreshed per-validation or cached at startup?

3. **Soft vs Hard Rules**: Should some rules be warnings rather than errors?

4. **Rule Versioning**: How to handle rule changes for in-flight onboardings?

5. **Custom Rules per CBU**: Should CBU-specific overrides be supported?

---

## References

- `rust/src/dsl_v2/` - All DSL v2 modules
- `docs/design-contextual-validation-rules.md` - Original design proposal
- `sql/00_MASTER_SCHEMA_CONSOLIDATED.sql` - Current schema
- `sql/01_SEED_DATA_CONSOLIDATED.sql` - Current seed data
